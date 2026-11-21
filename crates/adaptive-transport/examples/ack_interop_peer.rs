//! ACK BİRLİKTE ÇALIŞMA EŞİ (ack-interop peer) — ACK referans düzeneğinin (d) ayağı.
//!
//! # Neden var
//!
//! ACK biçimi değişirse eski sürümle yeni sürüm aynı hatta buluşur. Dört eşleşme (eski gönderici –
//! eski alıcı, eski–yeni, yeni–eski, yeni–yeni) ya çalışmalı ya da TANIMLI bir hatayla düşmelidir;
//! sessiz bekleme kabul edilmez. Bu ikili, gerçek `Sender` + gerçek `Receiver` + gerçek ACK codec'i
//! (`AckFrame::encode` / `decode` / `acked_full`) ile geri döngü (loopback) UDP üzerinde bir uç
//! oynar. CI "eski" ikiliyi SABİTLENMİŞ bir SHA'dan (`scripts/ci/ack-interop/PIN`), "yeni"yi PR
//! başından derler ve `scripts/ci/ack_interop.py` dördünü koşturur. Motor ya da ürün tel biçimine
//! dokunmaz; ürün yolunda kullanılmaz.
//!
//! # Düzenek protokolü (yalnız bu eşin; ürün teli DEĞİL)
//!
//! ```text
//! 0xF0 HELLO   'KACK' · harness_ver u8 · emit_n u8 · emit[] u8 · decode_n u8 · decode[] u8
//! 0xF1 REJECT  reason u8 (1 = ortak ACK çerçeve türü yok · 2 = gelen ACK türü çözülemedi); iki yönde
//! 0xF2 FIN     (gönderici → alıcı)         0xF3 FIN_OK (alıcı → gönderici)
//! 0x00 DATA    seq u64 · unit u64 · dolgu
//! diğer        alıcı → gönderici: ham ACK çerçevesi (`AckFrame::encode` çıktısı, zarfsız)
//! ```
//!
//! Alıcı, göndericinin HELLO'daki çözme listesinden kendi yayabildiği ilk türü seçer; ortak tür
//! yoksa REJECT yollar. Gönderici, çözemediği bir ACK türü görürse de (HELLO'ya rağmen) düşer.
//!
//! # Çıkış kodları (tanımlı; betik bunları ayırt eder)
//!
//! `0` başarı · `2` süre doldu (TIMEOUT) · `3` argüman hatası · `4` desteklenmeyen eşleşme
//! (UNSUPPORTED_PAIRING) · `5` protokol ihlali (çözülemeyen ACK, gönderilmemiş seq teyidi, boy
//! tavanı aşımı) · `6` G/Ç hatası.
//!
//! # Kullanım
//!
//! ```text
//! ack_interop_peer rx --report rx.json [--deadline-ms 60000] [--loss-permille 150]
//!                     [--ack-loss-permille 200] [--emit-types 14] [--rewrite-ack-type 99]
//! ack_interop_peer tx --peer 127.0.0.1:PORT --report tx.json [--units 200] [--unit-bytes 300]
//!                     [--deadline-ms 60000]
//! ```
//!
//! `rx` ilk satırda `PORT <n>` basar. `--emit-types`/`--rewrite-ack-type` yalnız betik öz-testi
//! içindir (desteklenmeyen eşleşmenin tanımlı hatayla düştüğünü kanıtlar).

use std::collections::BTreeSet;
use std::io::Write as _;
use std::net::{SocketAddr, UdpSocket};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::error::TransportError;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind,
};

const HARNESS_VER: u8 = 1;
const T_HELLO: u8 = 0xF0;
const T_REJECT: u8 = 0xF1;
const T_FIN: u8 = 0xF2;
const T_FIN_OK: u8 = 0xF3;
const T_DATA: u8 = 0x00;
/// Bu sürümün yayabildiği ACK türleri (ürün bugün yalnız V2 yayar).
const EMIT_TYPES: [u8; 1] = [FRAME_TYPE_ACK_V2];
/// Bu sürümün çözebildiği ACK türleri.
const DECODE_TYPES: [u8; 2] = [FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2];
/// Bugünkü en büyük ACK çerçevesi (bayt; zarfsız). `crates/adaptive-transport/tests/ack_ref_codec.rs`
/// `ACK_FRAME_MAX_BYTES` ile aynı değer.
const ACK_FRAME_MAX_BYTES: usize = 79;

const EXIT_TIMEOUT: u8 = 2;
const EXIT_ARGS: u8 = 3;
const EXIT_UNSUPPORTED: u8 = 4;
const EXIT_PROTOCOL: u8 = 5;
const EXIT_IO: u8 = 6;

struct Fail(u8, String);

fn fail<T>(code: u8, msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail(code, msg.into()))
}

fn io<T>(r: std::io::Result<T>, what: &str) -> Result<T, Fail> {
    r.map_err(|e| Fail(EXIT_IO, format!("{what}: {e}")))
}

#[derive(Debug, Clone)]
struct Args {
    role: String,
    peer: Option<SocketAddr>,
    report: String,
    deadline: Duration,
    units: u64,
    unit_bytes: u64,
    loss_permille: u64,
    ack_loss_permille: u64,
    emit_types: Vec<u8>,
    rewrite_ack_type: Option<u8>,
}

fn parse_hex_u8(v: &str) -> Result<u8, Fail> {
    u8::from_str_radix(v.trim_start_matches("0x"), 16)
        .map_err(|_| Fail(EXIT_ARGS, format!("onaltılık bayt bekleniyordu: {v}")))
}

fn parse_args() -> Result<Args, Fail> {
    let mut it = std::env::args().skip(1);
    let role = it.next().unwrap_or_default();
    if role != "rx" && role != "tx" {
        return fail(EXIT_ARGS, "ilk argüman rx ya da tx");
    }
    let mut a = Args {
        role,
        peer: None,
        report: String::new(),
        deadline: Duration::from_secs(60),
        units: 200,
        unit_bytes: 300,
        loss_permille: 150,
        ack_loss_permille: 200,
        emit_types: EMIT_TYPES.to_vec(),
        rewrite_ack_type: None,
    };
    while let Some(k) = it.next() {
        let v = it
            .next()
            .ok_or_else(|| Fail(EXIT_ARGS, format!("{k} değer ister")))?;
        let num = || {
            v.parse::<u64>()
                .map_err(|_| Fail(EXIT_ARGS, format!("{k}: sayı bekleniyordu")))
        };
        match k.as_str() {
            "--peer" => {
                a.peer = Some(
                    v.parse()
                        .map_err(|_| Fail(EXIT_ARGS, format!("--peer: adres {v}")))?,
                );
            }
            "--report" => a.report.clone_from(&v),
            "--deadline-ms" => a.deadline = Duration::from_millis(num()?),
            "--units" => a.units = num()?,
            "--unit-bytes" => a.unit_bytes = num()?.max(17),
            "--loss-permille" => a.loss_permille = num()?.min(1000),
            "--ack-loss-permille" => a.ack_loss_permille = num()?.min(1000),
            "--emit-types" => {
                a.emit_types = v
                    .split(',')
                    .map(parse_hex_u8)
                    .collect::<Result<Vec<u8>, Fail>>()?;
            }
            "--rewrite-ack-type" => a.rewrite_ack_type = Some(parse_hex_u8(&v)?),
            _ => return fail(EXIT_ARGS, format!("bilinmeyen argüman {k}")),
        }
    }
    if a.report.is_empty() {
        return fail(EXIT_ARGS, "--report zorunlu");
    }
    if a.role == "tx" && a.peer.is_none() {
        return fail(EXIT_ARGS, "tx için --peer zorunlu");
    }
    Ok(a)
}

fn hello(emit: &[u8]) -> Vec<u8> {
    let mut b = vec![T_HELLO, b'K', b'A', b'C', b'K', HARNESS_VER];
    b.push(emit.len() as u8);
    b.extend_from_slice(emit);
    b.push(DECODE_TYPES.len() as u8);
    b.extend_from_slice(&DECODE_TYPES);
    b
}

/// HELLO çözümü → (emit, decode). Bozuk HELLO protokol ihlalidir.
fn parse_hello(b: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Fail> {
    let bad = || Fail(EXIT_PROTOCOL, format!("bozuk HELLO: {b:02x?}"));
    if b.len() < 8 || b[..6] != [T_HELLO, b'K', b'A', b'C', b'K', HARNESS_VER] {
        return Err(bad());
    }
    let en = usize::from(b[6]);
    let emit = b.get(7..7 + en).ok_or_else(bad)?.to_vec();
    let dn = usize::from(*b.get(7 + en).ok_or_else(bad)?);
    let dec = b.get(8 + en..8 + en + dn).ok_or_else(bad)?.to_vec();
    if b.len() != 8 + en + dn {
        return Err(bad());
    }
    Ok((emit, dec))
}

/// Belirlenimli düşürme (eşler arası ortak durum yok; tohum = olay sırası).
fn drop_by_hash(x: u64, permille: u64) -> bool {
    let h = x
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h >> 33) % 1000 < permille
}

fn json_list(v: impl Iterator<Item = u64>) -> String {
    let items: Vec<String> = v.map(|x| x.to_string()).collect();
    format!("[{}]", items.join(","))
}

fn write_report(path: &str, body: &str) -> Result<(), Fail> {
    let mut f = io(std::fs::File::create(path), "rapor dosyası")?;
    io(writeln!(f, "{body}"), "rapor yazımı")
}

fn recv(sock: &UdpSocket, buf: &mut [u8]) -> Result<Option<(usize, SocketAddr)>, Fail> {
    match sock.recv_from(buf) {
        Ok(x) => Ok(Some(x)),
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
                // Karşı uç henüz yok (ICMP port unreachable): süre sınırı karar verir.
                || e.kind() == std::io::ErrorKind::ConnectionRefused =>
        {
            Ok(None)
        }
        Err(e) => fail(EXIT_IO, format!("recv: {e}")),
    }
}

fn run_rx(a: &Args) -> Result<(), Fail> {
    let sock = io(UdpSocket::bind("127.0.0.1:0"), "bind")?;
    io(
        sock.set_read_timeout(Some(Duration::from_millis(1))),
        "timeout",
    )?;
    let port = io(sock.local_addr(), "local_addr")?.port();
    println!("PORT {port}");
    let _ = std::io::stdout().flush();
    let t0 = Instant::now();
    let now_ns = || u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let mut buf = vec![0u8; 2048];
    // 1. HELLO → seçim ya da REJECT.
    let peer = loop {
        if t0.elapsed() > a.deadline {
            return fail(EXIT_TIMEOUT, "TIMEOUT: HELLO gelmedi");
        }
        if let Some((n, from)) = recv(&sock, &mut buf)? {
            if buf[0] == T_HELLO {
                let (_, their_decode) = parse_hello(&buf[..n])?;
                let Some(&pick) = a.emit_types.iter().find(|t| their_decode.contains(t)) else {
                    let _ = sock.send_to(&[T_REJECT, 1], from);
                    return fail(
                        EXIT_UNSUPPORTED,
                        format!(
                            "UNSUPPORTED_PAIRING: alıcı yayar {:02x?}, gönderici çözer {their_decode:02x?}",
                            a.emit_types
                        ),
                    );
                };
                io(sock.send_to(&hello(&[pick]), from), "send")?;
                break from;
            }
        }
    };
    // 2. Veri + ACK.
    let mut r = Receiver::with_cc_port();
    let mut received: BTreeSet<u64> = BTreeSet::new();
    let mut delivered: BTreeSet<u64> = BTreeSet::new();
    let (mut acks, mut acks_dropped, mut max_ack) = (0u64, 0u64, 0usize);
    let mut send_ack = |f: AckFrame, acks: &mut u64, max_ack: &mut usize| -> Result<(), Fail> {
        let mut b = f.encode();
        if b.len() > ACK_FRAME_MAX_BYTES {
            return fail(
                EXIT_PROTOCOL,
                format!("ACK çerçevesi {} B > {ACK_FRAME_MAX_BYTES}", b.len()),
            );
        }
        *max_ack = (*max_ack).max(b.len());
        *acks += 1;
        if drop_by_hash(*acks, a.ack_loss_permille) {
            acks_dropped += 1;
            return Ok(());
        }
        if let Some(t) = a.rewrite_ack_type {
            b[0] = t;
        }
        io(sock.send_to(&b, peer), "send").map(|_| ())
    };
    loop {
        if t0.elapsed() > a.deadline {
            return fail(EXIT_TIMEOUT, "TIMEOUT: FIN gelmedi");
        }
        while let Some((n, from)) = recv(&sock, &mut buf)? {
            if from != peer || n == 0 {
                continue;
            }
            match buf[0] {
                T_HELLO => {
                    // Kayıp yanıtın tekrarı.
                    let pick = a.emit_types[0];
                    io(sock.send_to(&hello(&[pick]), peer), "send")?;
                }
                T_REJECT => {
                    return fail(
                        EXIT_UNSUPPORTED,
                        "UNSUPPORTED_PAIRING: gönderici ACK türünü çözemedi (REJECT)",
                    )
                }
                T_FIN => {
                    io(sock.send_to(&[T_FIN_OK], peer), "send")?;
                    let body = format!(
                        "{{\"role\":\"rx\",\"received\":{},\"delivered_units\":{},\"acks\":{acks},\
                         \"acks_dropped\":{acks_dropped},\"max_ack_frame_bytes\":{max_ack}}}",
                        json_list(received.iter().copied()),
                        delivered.len()
                    );
                    return write_report(&a.report, &body);
                }
                T_DATA if n >= 17 => {
                    let seq = u64::from_be_bytes(buf[1..9].try_into().expect("8 B"));
                    let unit = u64::from_be_bytes(buf[9..17].try_into().expect("8 B"));
                    if drop_by_hash(seq, a.loss_permille) {
                        continue;
                    }
                    received.insert(seq);
                    let out = r.on_datagram(
                        RxDatagram {
                            seq,
                            unit_id: unit,
                            kind: RxKind::Data,
                        },
                        now_ns(),
                    );
                    delivered.extend(out.delivered);
                    if let Some(f) = out.ack {
                        send_ack(f, &mut acks, &mut max_ack)?;
                    }
                }
                t => return fail(EXIT_PROTOCOL, format!("alıcıya beklenmeyen tür 0x{t:02x}")),
            }
        }
        if let Some(f) = r.on_ack_timer(now_ns()) {
            send_ack(f, &mut acks, &mut max_ack)?;
        }
    }
}

fn run_tx(a: &Args) -> Result<(), Fail> {
    let peer = a.peer.expect("doğrulandı");
    let sock = io(UdpSocket::bind("127.0.0.1:0"), "bind")?;
    io(
        sock.set_read_timeout(Some(Duration::from_millis(1))),
        "timeout",
    )?;
    let t0 = Instant::now();
    let now_ns = || u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let mut buf = vec![0u8; 2048];
    // 1. HELLO.
    let mut last_hello: Option<Instant> = None;
    loop {
        if t0.elapsed() > a.deadline {
            return fail(EXIT_TIMEOUT, "TIMEOUT: HELLO yanıtı gelmedi");
        }
        if last_hello.is_none_or(|t| t.elapsed() > Duration::from_millis(200)) {
            io(sock.send_to(&hello(&a.emit_types), peer), "send")?;
            last_hello = Some(Instant::now());
        }
        if let Some((n, from)) = recv(&sock, &mut buf)? {
            if from != peer {
                continue;
            }
            match buf[0] {
                T_REJECT => {
                    return fail(
                        EXIT_UNSUPPORTED,
                        "UNSUPPORTED_PAIRING: alıcı REJECT yolladı (ortak ACK türü yok)",
                    )
                }
                T_HELLO => {
                    let (their_emit, _) = parse_hello(&buf[..n])?;
                    if !their_emit.iter().all(|t| DECODE_TYPES.contains(t)) {
                        let _ = sock.send_to(&[T_REJECT, 1], peer);
                        return fail(
                            EXIT_UNSUPPORTED,
                            format!(
                                "UNSUPPORTED_PAIRING: alıcı {their_emit:02x?} yayar, çözülebilen {DECODE_TYPES:02x?}"
                            ),
                        );
                    }
                    break;
                }
                _ => {}
            }
        }
    }
    // 2. Veri.
    let mut s = Sender::with_cc_port();
    let mut sent: BTreeSet<u64> = BTreeSet::new();
    let mut max_sent: Option<u64> = None;
    let mut acked: BTreeSet<u64> = BTreeSet::new();
    let mut confirmed: BTreeSet<u64> = BTreeSet::new();
    let mut acks_rx = 0u64;
    let send = |decs: Vec<TransmitDecision>,
                sent: &mut BTreeSet<u64>,
                max_sent: &mut Option<u64>|
     -> Result<(), Fail> {
        for d in decs {
            if !matches!(
                d.kind,
                TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate
            ) {
                continue;
            }
            let mut b = vec![T_DATA];
            b.extend_from_slice(&d.seq.to_be_bytes());
            b.extend_from_slice(&d.unit_id.to_be_bytes());
            b.resize(usize::try_from(a.unit_bytes).unwrap_or(300), 0);
            sent.insert(d.seq);
            *max_sent = Some(max_sent.map_or(d.seq, |m| m.max(d.seq)));
            io(sock.send_to(&b, peer), "send")?;
        }
        Ok(())
    };
    for i in 0..a.units {
        let d = s.on_event(
            Event::AppData(OpaqueUnit {
                id: i,
                size: a.unit_bytes,
            }),
            now_ns(),
        );
        send(d, &mut sent, &mut max_sent)?;
    }
    while (confirmed.len() as u64) < a.units {
        if t0.elapsed() > a.deadline {
            return fail(
                EXIT_TIMEOUT,
                format!(
                    "TIMEOUT: teyit {}/{} (ACK {acks_rx})",
                    confirmed.len(),
                    a.units
                ),
            );
        }
        let d = s.on_event(Event::TimerFired(Timer::Pto), now_ns());
        send(d, &mut sent, &mut max_sent)?;
        while let Some((n, from)) = recv(&sock, &mut buf)? {
            if from != peer || n == 0 || buf[0] == T_HELLO {
                continue;
            }
            let f = match AckFrame::decode(&buf[..n]) {
                Ok(f) => f,
                Err(TransportError::UnknownFrameType(t)) => {
                    // Karşı uç da tanımlı hatayla dursun (sessiz bekleme yok).
                    let _ = sock.send_to(&[T_REJECT, 2], peer);
                    return fail(
                        EXIT_UNSUPPORTED,
                        format!("UNSUPPORTED_PAIRING: çözülemeyen ACK türü 0x{t:02x}"),
                    );
                }
                Err(e) => return fail(EXIT_PROTOCOL, format!("ACK çözülemedi: {e}")),
            };
            acks_rx += 1;
            let seqs = f.acked_full(max_sent.map_or(0, |m| m + 1));
            if let Some(q) = seqs.iter().find(|q| !sent.contains(q)) {
                return fail(EXIT_PROTOCOL, format!("gönderilmemiş seq {q} teyitli"));
            }
            acked.extend(seqs.iter().copied());
            let d = s.on_event(
                Event::AckReceived(AckInfo {
                    acked_seqs: seqs,
                    ack_delay: f.ack_delay_duration(),
                }),
                now_ns(),
            );
            send(d, &mut sent, &mut max_sent)?;
            confirmed.extend(s.take_confirmed_units());
        }
        confirmed.extend(s.take_confirmed_units());
    }
    // 3. FIN.
    let mut last_fin: Option<Instant> = None;
    loop {
        if t0.elapsed() > a.deadline {
            return fail(EXIT_TIMEOUT, "TIMEOUT: FIN_OK gelmedi");
        }
        if last_fin.is_none_or(|t| t.elapsed() > Duration::from_millis(100)) {
            io(sock.send_to(&[T_FIN], peer), "send")?;
            last_fin = Some(Instant::now());
        }
        if let Some((n, from)) = recv(&sock, &mut buf)? {
            if from == peer && n == 1 && buf[0] == T_FIN_OK {
                break;
            }
        }
    }
    let body = format!(
        "{{\"role\":\"tx\",\"units\":{},\"confirmed\":{},\"acks_rx\":{acks_rx},\"sent\":{},\"acked\":{}}}",
        a.units,
        confirmed.len(),
        json_list(sent.iter().copied()),
        json_list(acked.iter().copied())
    );
    write_report(&a.report, &body)
}

fn main() -> ExitCode {
    let res = parse_args().and_then(|a| {
        if a.role == "rx" {
            run_rx(&a)
        } else {
            run_tx(&a)
        }
    });
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail(code, msg)) => {
            eprintln!("ack_interop_peer: {msg}");
            ExitCode::from(code)
        }
    }
}
