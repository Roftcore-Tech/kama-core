//! TCP taban kolu (`tcp-std`) — KAMA ↔ TCP kampanyası. Yalnız `std`; bağımlılık yok.
//!
//! Ölçüm tanımı `crates/bench` `kama-product` koluyla BİREBİR (aynı katman, aynı saat kuralı):
//!   * teslim anı  = mesajı kapsayan **uygulama-düzeyi ACK'in gönderene varışı**
//!     (Kama kolunda: ilk teslim-teyidinin gönderene varışı). `write()` dönüşü ÇEKİRDEK
//!     TAMPONUNU ölçer, teslimi değil → kullanılmaz.
//!   * gönderim anı = uygulamanın mesajı SUNDUĞU an (Kama kolunda `offer` anı).
//!   * çerçeve     = `u32 len ‖ u64 msg_id ‖ payload`; ACK = `u64 msg_id` (big-endian).
//!   * WARM: bağlantı ÖNCE kurulur, `--warmup-msgs` adet eş-boy ısıtma mesajı gönderilip ACK'i
//!     beklenir (tavan = aralık×W + 30 s — Kama kolundaki `WARMUP_CAP_EXTRA` ile aynı), SONRA t0.
//!   * COLD: t0 `connect()`'ten ÖNCE konur; ısıtma yok (kurulum maliyeti pencerenin İÇİNDE).
//!   * koşu sonu   = t0 + aralık×(N−1) + horizon (Kama kolundaki `total_deadline` ile aynı).
//!
//! TCP ayarı: platform varsayılanı (CUBIC), `TCP_NODELAY` AÇIK, TLS YOK. Hiçbir sysctl/soket
//! tamponu değiştirilmez. Çıktı: bench ile aynı JSONL şeması (`{"meta":…}` + `{"event":…}`).
//!
//! Kullanım:
//!   tcp_adapter server --bind A:P
//!   tcp_adapter client --server A:P --msgs N --payload B --submit-interval-ms I --warmup-msgs W
//!                      --horizon-ms H --mode warm|cold --run-id ID --out FILE

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Çerçeve başlığı: `u32 len ‖ u64 msg_id`.
const HDR_LEN: usize = 12;
/// ACK çerçevesi: `u64 msg_id`.
const ACK_LEN: usize = 8;
/// Tek mesaj için üst sınır (bounds-before-alloc; kampanya yükü ≤ 995 B).
const MAX_PAYLOAD: usize = 64 * 1024;
/// `connect()` tavanı — `crates/bench/src/tcp.rs` ile aynı (20 s).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Isıtma tavanına eklenen sabit — `kama_product.rs::WARMUP_CAP_EXTRA` ile aynı (30 s).
const WARMUP_CAP_EXTRA: Duration = Duration::from_secs(30);
/// "Teslim edilmedi" işareti (atomik hücrede).
const UNSET: u64 = u64::MAX;

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
}
fn arg_u64(a: &[String], k: &str, d: u64) -> u64 {
    arg(a, k).and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn die(msg: &str) -> ! {
    eprintln!("tcp_adapter: {msg}");
    std::process::exit(2);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("server") => server(&a),
        Some("client") => client(&a),
        _ => die("kullanım: tcp_adapter server|client …"),
    }
}

// ── Sunucu: her bağlantıda çerçeve oku → uygulamaya teslim say → ACK yaz ─────────────────────
fn server(a: &[String]) {
    let bind: SocketAddr = arg(a, "--bind")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| die("--bind A:P gerekli"));
    let lst = TcpListener::bind(bind).unwrap_or_else(|e| die(&format!("bind: {e}")));
    eprintln!("TCP-SERVER listening {bind}");
    for conn in lst.incoming() {
        let Ok(mut s) = conn else { continue };
        std::thread::spawn(move || {
            let _ = s.set_nodelay(true);
            let mut hdr = [0u8; HDR_LEN];
            let mut body = vec![0u8; MAX_PAYLOAD];
            let (mut n_msgs, mut n_bytes) = (0u64, 0u64);
            loop {
                if s.read_exact(&mut hdr).is_err() {
                    break;
                }
                let len = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
                if len > MAX_PAYLOAD {
                    eprintln!("TCP-SERVER frame too large ({len}) — bağlantı kapatıldı");
                    break;
                }
                if s.read_exact(&mut body[..len]).is_err() {
                    break;
                }
                // Buradan sonra mesaj UYGULAMAYA teslim edilmiştir → uygulama-düzeyi ACK.
                n_msgs += 1;
                n_bytes += len as u64;
                if s.write_all(&hdr[4..12]).is_err() {
                    break;
                }
            }
            eprintln!("TCP-SERVER conn closed delivered_msgs={n_msgs} payload_bytes={n_bytes}");
        });
    }
}

// ── İstemci ──────────────────────────────────────────────────────────────────────────────────
fn frame(msg_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(HDR_LEN + payload.len());
    f.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    f.extend_from_slice(&msg_id.to_be_bytes());
    f.extend_from_slice(payload);
    f
}

fn ns_since(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_nanos()).unwrap_or(u64::MAX - 1)
}

fn sleep_until(t: Instant) {
    let now = Instant::now();
    if t > now {
        std::thread::sleep(t - now);
    }
}

struct Outcome {
    sent_ns: Vec<Option<u64>>,
    delivered_ns: Vec<Option<u64>>,
    setup_ns: u64,
    connect_failed: bool,
    warm_acked: u64,
    send_errors: u64,
    app_tx: u64,
    app_rx: u64,
}

#[allow(clippy::too_many_lines)]
fn client(a: &[String]) {
    let server: SocketAddr = arg(a, "--server")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| die("--server A:P gerekli"));
    let n = arg_u64(a, "--msgs", 0) as usize;
    let payload_bytes = arg_u64(a, "--payload", 0) as usize;
    let interval = Duration::from_millis(arg_u64(a, "--submit-interval-ms", 0));
    let warm_n = arg_u64(a, "--warmup-msgs", 0);
    let horizon = Duration::from_millis(arg_u64(a, "--horizon-ms", 0));
    let mode = arg(a, "--mode").unwrap_or_else(|| "warm".into());
    let run_id = arg(a, "--run-id").unwrap_or_default();
    let out = arg(a, "--out").unwrap_or_else(|| die("--out FILE gerekli"));
    if n == 0 || payload_bytes == 0 || payload_bytes > MAX_PAYLOAD || horizon.is_zero() {
        die("--msgs/--payload/--horizon-ms geçersiz");
    }
    let cold = match mode.as_str() {
        "warm" => false,
        "cold" => true,
        _ => die("--mode warm|cold"),
    };
    if cold && warm_n != 0 {
        die("cold modda ısıtma olmaz (--warmup-msgs 0)");
    }
    let payload = vec![0xA5u8; payload_bytes];

    // COLD: t0 connect'ten ÖNCE. WARM: t0 ısıtmadan SONRA (aşağıda yeniden atanır).
    let mut t0 = Instant::now();
    let setup_clock = Instant::now();
    let conn = TcpStream::connect_timeout(&server, CONNECT_TIMEOUT);
    let setup_ns = ns_since(setup_clock);
    let mut o = Outcome {
        sent_ns: vec![None; n],
        delivered_ns: vec![None; n],
        setup_ns,
        connect_failed: false,
        warm_acked: 0,
        send_errors: 0,
        app_tx: 0,
        app_rx: 0,
    };
    let Ok(mut wr) = conn else {
        o.connect_failed = true;
        eprintln!("TCP-CLIENT connect FAILED setup_ns={setup_ns}");
        write_out(
            &out,
            &run_id,
            &mode,
            n,
            payload_bytes,
            interval,
            horizon,
            warm_n,
            0,
            &o,
        );
        return;
    };
    let _ = wr.set_nodelay(true);
    let mut rd = wr
        .try_clone()
        .unwrap_or_else(|e| die(&format!("clone: {e}")));

    // ACK okuyucusu: teslim damgası BURADA konur (ilk-ack-kazanır). Damga `epoch`'a göredir;
    // t0'a çevirme koşu sonunda yapılır (epoch sabit, t0 ısıtmadan sonra belli olur).
    let epoch = Instant::now();
    let total = warm_n as usize + n;
    let acks: Arc<Vec<AtomicU64>> = Arc::new((0..total).map(|_| AtomicU64::new(UNSET)).collect());
    let app_rx = Arc::new(AtomicU64::new(0));
    {
        let (acks, app_rx) = (Arc::clone(&acks), Arc::clone(&app_rx));
        std::thread::spawn(move || {
            let mut b = [0u8; ACK_LEN];
            while rd.read_exact(&mut b).is_ok() {
                app_rx.fetch_add(ACK_LEN as u64, Ordering::Relaxed);
                let id = u64::from_be_bytes(b) as usize;
                if let Some(c) = acks.get(id) {
                    let _ = c.compare_exchange(
                        UNSET,
                        ns_since(epoch),
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    );
                }
            }
        });
    }

    // ── Isıtma (yalnız WARM; ölçüm DIŞI) ──
    if warm_n > 0 {
        let cap = interval
            .saturating_mul(warm_n as u32)
            .saturating_add(WARMUP_CAP_EXTRA);
        let wstart = Instant::now();
        let _ = wr.set_write_timeout(Some(cap));
        for w in 0..warm_n {
            sleep_until(wstart + interval.saturating_mul(w as u32));
            if wr.write_all(&frame(w, &payload)).is_err() {
                break;
            }
        }
        while wstart.elapsed() < cap {
            let got = (0..warm_n as usize)
                .filter(|&i| acks[i].load(Ordering::Relaxed) != UNSET)
                .count() as u64;
            o.warm_acked = got;
            if got == warm_n {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if !cold {
        t0 = Instant::now();
    }
    let unix_t0 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let t0_off = u64::try_from(t0.saturating_duration_since(epoch).as_nanos()).unwrap_or(0);
    let t0_before_epoch =
        u64::try_from(epoch.saturating_duration_since(t0).as_nanos()).unwrap_or(0);

    // ── Ölçüm: mesaj i, t0 + i×aralık anında SUNULUR ──
    // COLD'da da taban t0'dır: connect sürerken vadesi geçen sunumlar bağlantı kurulur kurulmaz yapılır.
    let submit_base = t0;
    let run_end = submit_base + interval.saturating_mul(n.saturating_sub(1) as u32) + horizon;
    for i in 0..n {
        sleep_until(submit_base + interval.saturating_mul(i as u32));
        let now = Instant::now();
        if now >= run_end {
            break;
        }
        o.sent_ns[i] = Some(ns_since(t0));
        // Yazma horizon'u AŞAMAZ: tampon doluysa (kayıp altında TCP durur) kalan süre kadar bekle.
        let _ = wr.set_write_timeout(Some(run_end - now));
        let f = frame(warm_n + i as u64, &payload);
        match wr.write_all(&f) {
            Ok(()) => o.app_tx += f.len() as u64,
            Err(_) => {
                o.send_errors += 1;
                break; // akış bozuldu (timeout/RST): sonraki yazmalar çerçeve sınırını kaydırırdı
            }
        }
    }
    // Tüm ölçüm ACK'leri ya da koşu sonu.
    loop {
        let done = (0..n).all(|i| acks[warm_n as usize + i].load(Ordering::Relaxed) != UNSET);
        if done || Instant::now() >= run_end {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let end_ns = ns_since(epoch);
    for i in 0..n {
        let v = acks[warm_n as usize + i].load(Ordering::Relaxed);
        if v != UNSET && v <= end_ns {
            // epoch-tabanlı damgayı t0-tabanına çevir (COLD'da t0 < epoch → ekle; WARM'da çıkar).
            o.delivered_ns[i] = Some(v.saturating_add(t0_before_epoch).saturating_sub(t0_off));
        }
    }
    o.app_rx = app_rx.load(Ordering::Relaxed);
    let _ = wr.shutdown(std::net::Shutdown::Both);
    let delivered = o.delivered_ns.iter().filter(|d| d.is_some()).count();
    eprintln!(
        "TCP-CLIENT run={run_id} mode={mode} delivered={delivered}/{n} setup_ns={} warm_acked={}/{warm_n} \
         send_errors={} app_tx={}B app_rx={}B semantics=[reliable-ordered, app-ack-delivery, nodelay, no-tls]",
        o.setup_ns, o.warm_acked, o.send_errors, o.app_tx, o.app_rx
    );
    write_out(
        &out,
        &run_id,
        &mode,
        n,
        payload_bytes,
        interval,
        horizon,
        warm_n,
        u64::try_from(unix_t0).unwrap_or(0),
        &o,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_out(
    path: &str,
    run_id: &str,
    mode: &str,
    n: usize,
    payload: usize,
    interval: Duration,
    horizon: Duration,
    warm_n: u64,
    unix_t0: u64,
    o: &Outcome,
) {
    let mut s = String::new();
    s.push_str(&format!(
        "{{\"meta\":{{\"transport\":\"tcp-std\",\"run_id\":\"{}\",\"mode\":\"{}\",\"msg_count\":{},\
         \"payload_bytes\":{},\"submit_interval_ns\":{},\"max_deadline_ns\":{},\"warmup_msgs\":{},\
         \"warmup_residue\":{},\"setup_ns\":{},\"setup_in_window\":{},\"connect_failed\":{},\
         \"send_errors\":{},\"app_tx_bytes\":{},\"app_rx_bytes\":{},\"t0_unix_ns\":{},\
         \"profile_id\":\"tcp-std;cubic-default;nodelay;no-tls\"}}}}\n",
        run_id.replace(['"', '\\'], "_"),
        mode,
        n,
        payload,
        interval.as_nanos(),
        horizon.as_nanos(),
        warm_n,
        warm_n.saturating_sub(o.warm_acked),
        o.setup_ns,
        mode == "cold",
        o.connect_failed,
        o.send_errors,
        o.app_tx,
        o.app_rx,
        unix_t0
    ));
    for i in 0..n {
        let sent = o.sent_ns[i].map_or("null".to_string(), |v| v.to_string());
        let del = o.delivered_ns[i].map_or("null".to_string(), |v| v.to_string());
        s.push_str(&format!(
            "{{\"event\":{{\"msg_id\":{i},\"sent_ns\":{sent},\"delivered_ns\":{del},\"payload_bytes\":{payload}}}}}\n"
        ));
    }
    let mut f = std::fs::File::create(path).unwrap_or_else(|e| die(&format!("out: {e}")));
    f.write_all(s.as_bytes())
        .unwrap_or_else(|e| die(&format!("write: {e}")));
}
