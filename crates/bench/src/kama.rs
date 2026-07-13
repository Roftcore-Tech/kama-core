//! Kama transport — adaptive-transport engine'i (Sender §3 / Receiver §4 state machine'leri) gerçek
//! UDP üstünde sürer; bench harness'ın Kama'yı quinn baseline'ının YANINA koymasını sağlar (ADIM-5).
//!
//! **SoC (INV-ENGINE-SOC):** kripto motorun DIŞINDA. Bench "identity security-stub" yapar — DATA
//! payload düz `0x5A`, seq/unit_id cleartext; anahtar/AEAD/Noise YOK. Motor yalnız `OpaqueUnit{id,size}`
//! (sender) ve `RxDatagram{seq,unit_id}` metadata (receiver) görür; sarmalama bench'in (trivial) işi.
//! FEC parity ÜRETİLMEZ (`TransmitKind::FecParity` emit edilmez) → wire'da yalnız DATA (client→server)
//! + ACK (server→client) → INV-FEC-1 ihlali fiziksel olarak imkânsız.
//!
//! **Ölçüm modeli (quinn ile ELMALAR-ELMALAR — [`crate::event::RawEvent`] aynı frame):** tek `start`
//! saati; `sent_ns` = APP-OFFER anı (enqueue; quinn de task başında/pacing öncesi damgalar → latency her
//! iki tarafta da transport'un İÇ kuyruğunu İÇERİR), `delivered_ns` = birimin herhangi bir seq'ini kapsayan
//! ilk ACK'in MUTLAK anı (retransmit/dup aynı birimi yeni seq'le taşır → ilk-ack-kazanır dedup). Tüm koşu
//! `max_deadline` ile bounded (dead server → all-None Ok, HANG yok). sent_ns'i wire-emit'te damgalamak
//! kama'nın pacing dwell'ini latency'den dışlar → completion şişer (review CRITICAL, frame-bug sınıfı).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use adaptive_transport::seq::resolve_seq;
use tokio::net::UdpSocket;
use tokio::time::interval;

use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};
use crate::transport::{BenchTransport, ClientCfg, KamaTransport, ServerCfg};

/// DATA frame_type (bench-local; ACK 0x10 ack.rs'te, FEC_SHARD 0x20 wire'da hiç görünmez).
const FRAME_DATA: u8 = 0x00;
/// DATA header: frame_type(1) + seq u16 (2) + unit_id u64 (8) = 11 B, sonra payload.
const HDR_DATA: usize = 11;
/// [C/D2] Security-stub AEAD-tag SİMÜLASYONU: her frame'e +16B (v-gerçek Noise/AEAD tag maliyeti) →
/// byte ölçümleri gerçek katmanla DEĞİŞMEZ (revize-D0 §7-C; sent_ns/tag-dolgusu dersi). Sabit-16 pad.
const TAG_STUB: usize = 16;
/// transport-version (RunMeta.quinn_version alanına — alan donmuş, yeniden adlandırma YOK).
const KAMA_VERSION: &str = env!("CARGO_PKG_VERSION");
/// pacing pump: her tick §13 token drain + §5 zaman-eşiği loss (idempotent, yüksek frekans güvenli).
const PACE_TICK: Duration = Duration::from_millis(1);
/// RTprop probe periyodu (§8.3).
const RTPROP_TICK: Duration = Duration::from_millis(200);
/// server ACK timer (§2.1 ACK_TIMER_T) — ACK_FREQ_N'e ulaşmayan son DATA'yı da teyit eder.
const ACK_TICK: Duration = Duration::from_millis(20);
/// server recv buffer (max UDP datagram).
const SERVER_RECV_BUF: usize = 65536;
/// client recv buffer (yalnız küçük ACK frame'leri).
const CLIENT_RECV_BUF: usize = 2048;

/// DATA frame'i canonical fixed-width big-endian kodlar (ADR-05; varint YASAK).
fn encode_data(seq: u16, unit_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(HDR_DATA + payload.len() + TAG_STUB);
    b.push(FRAME_DATA);
    b.extend_from_slice(&seq.to_be_bytes());
    b.extend_from_slice(&unit_id.to_be_bytes());
    b.extend_from_slice(payload);
    b.resize(b.len() + TAG_STUB, 0); // [C] AEAD-tag simülasyonu — decode trailing'i yoksayar
    b
}

/// DATA frame'den (wire_seq, unit_id) çözer; bozuk/eksik → `None`. Payload (identity stub) yoksayılır.
fn decode_data(buf: &[u8]) -> Option<(u16, u64)> {
    if buf.len() < HDR_DATA || buf[0] != FRAME_DATA {
        return None;
    }
    let seq = u16::from_be_bytes([buf[1], buf[2]]);
    let unit_id = u64::from_be_bytes(buf[3..11].try_into().ok()?);
    Some((seq, unit_id))
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Client koşu durumu: sender engine + ölçüm muhasebesi (seq→unit join, sent/delivered damgaları, byte sayaçları).
struct ClientRun {
    sender: Sender,
    seq_to_unit: HashMap<u64, u64>,
    max_emitted_seq: u64,
    sent_ns: Vec<Option<u64>>,
    delivered_ns: Vec<Option<u64>>,
    delivered_count: u64,
    tx_bytes: u64,
    rx_bytes: u64,
    // [D2] tx bileşen ayrımı (teşhis dekompozisyonu — d.kind'e göre)
    tx_data: u64,
    tx_retx: u64,
    tx_dup: u64,
}

impl ClientRun {
    fn new(msg_count: usize) -> Self {
        Self {
            sender: Sender::new(),
            seq_to_unit: HashMap::new(),
            max_emitted_seq: 0,
            sent_ns: vec![None; msg_count],
            delivered_ns: vec![None; msg_count],
            delivered_count: 0,
            tx_bytes: 0,
            rx_bytes: 0,
            tx_data: 0,
            tx_retx: 0,
            tx_dup: 0,
        }
    }

    /// Sender'ı bir olayla sürer + çıkan transmit kararlarını wire'a yazar. seq→unit map + max_emitted_seq
    /// güncellenir (sent_ns app-offer'da damgalanır, burada DEĞİL). send hatası = kayıp paket, ABORT değil.
    async fn drive(
        &mut self,
        ev: Event,
        now: u64,
        sock: &UdpSocket,
        server: SocketAddr,
        payload: &[u8],
    ) {
        for d in self.sender.on_event(ev, now) {
            self.seq_to_unit.insert(d.seq, d.unit_id);
            self.max_emitted_seq = self.max_emitted_seq.max(d.seq);
            // sent_ns burada DAMGALANMAZ — app-offer'da (enqueue) damgalanır (quinn frame'i, aşağıdaki
            // client_loop notu). Wire-emit'te damgalamak kama'nın pacing dwell'ini latency'den dışlardı.
            let frame = encode_data((d.seq & 0xFFFF) as u16, d.unit_id, payload);
            if let Ok(n) = sock.send_to(&frame, server).await {
                let n = n as u64;
                self.tx_bytes += n;
                match d.kind {
                    TransmitKind::Data => self.tx_data += n,
                    TransmitKind::Retransmit => self.tx_retx += n,
                    TransmitKind::Duplicate => self.tx_dup += n,
                    TransmitKind::FecParity => {} // emit edilmez (§10 politika)
                }
            }
        }
    }
}

/// Client event pump (§3 sender): tüm birimleri t0'da enqueue → select{ ACK / pace-tick / rtprop-tick /
/// deadline }. `start` tek koşu saati (sent_ns/delivered_ns frame'i). Her zaman `Ok` döner (ağ hataları
/// yoksayılır; yalnız çağıran bind'i Err olabilir); dead server → all-None 0-completion.
async fn client_loop(
    sock: UdpSocket,
    cfg: ClientCfg,
    start: Instant,
) -> Result<RunOutput, BenchError> {
    let msg_count = cfg.msg_count;
    let payload = vec![0x5Au8; usize::try_from(cfg.payload_bytes).unwrap_or(0)];
    let unit_size = HDR_DATA as u64 + cfg.payload_bytes;
    let mut run = ClientRun::new(usize::try_from(msg_count).unwrap_or(0));

    // Tüm birimleri t0'da enqueue — engine'in İÇ pacing'i yayılımı belirlesin (yapay serileştirme değil;
    // her iki transport da aynı start'tan aynı max_deadline bütçesini alır → adil karşılaştırma).
    //
    // sent_ns = APP-OFFER anı (quinn ile AYNI zero-point: quic.rs task başında sent_ns=start.elapsed(),
    // quinn'in İÇ pacing'inden ÖNCE → latency quinn'in kuyruğunu İÇERİR). Kama da app-offer'da damgalar →
    // latency kama'nın pacing dwell'ini İÇERİR → elmalar-elmalar. (review CRITICAL: wire-emit'te damgalamak
    // dwell'i dışlar → completion kama lehine ŞİŞER; delivered_ns frame-bug sınıfının sent-tarafı ikizi.)
    for id in 0..msg_count {
        let now = elapsed_ns(start);
        if let Some(slot) = run
            .sent_ns
            .get_mut(usize::try_from(id).unwrap_or(usize::MAX))
        {
            *slot = Some(now);
        }
        run.drive(
            Event::AppData(OpaqueUnit {
                id,
                size: unit_size,
            }),
            now,
            &sock,
            cfg.server,
            &payload,
        )
        .await;
    }

    let mut pace = interval(PACE_TICK);
    let mut rtprop = interval(RTPROP_TICK);
    let mut buf = [0u8; CLIENT_RECV_BUF];
    let deadline = tokio::time::sleep(cfg.max_deadline);
    tokio::pin!(deadline);

    // [TUR-A teşhis] rate-trajektori tap'ı (SALT-GÖZLEM; kaldırılabilir). floor→btlbw ramp kilidini ölç.
    let mut traj = interval(Duration::from_millis(500));
    let unit_wire = HDR_DATA as u64 + cfg.payload_bytes + TAG_STUB as u64;

    while run.delivered_count < msg_count {
        tokio::select! {
            () = &mut deadline => break,
            _ = traj.tick() => {
                let now = elapsed_ns(start);
                let (st, btlbw, srtt_us, rate, infl) = run.sender.diag_rate();
                let (lpkt, ltime, lpend) = run.sender.diag_loss();
                let (rtt_rej, adly_max) = run.sender.diag_rtt();
                let (unloss, lppm, _spur) = run.sender.diag_ledger();
                let (tomb_live, tomb_hit, bw_rec, tomb_ev, ambig, credited) = run.sender.diag_tombstone();
                let pif = run.sender.diag_pif();
                let (ps, pa, pt, pp) = run.sender.diag_probe();
                let (rtprop_us, qd_us) = run.sender.diag_qd();
                eprintln!(
                    "TRAJ t={:.1} state={:?} btlbw={} rate={} srtt_us={} infl={} sent_u={} deliv={} loss_pkt={} loss_time={} pend={} rtt_rej={} adly_max_us={} unloss={} lppm={} tomb={}/{}/{}/{}/{}/{} pif={} rtprop_us={} qd_us={} probe={}/{}/{}/{}",
                    now as f64 / 1e9, st, btlbw, rate, srtt_us, infl,
                    run.tx_data / unit_wire.max(1), run.delivered_count, lpkt, ltime, lpend,
                    rtt_rej, adly_max, unloss, lppm, tomb_live, tomb_hit, bw_rec, tomb_ev, ambig, credited, pif, rtprop_us, qd_us, ps, pa, pt, pp
                );
            }
            _ = pace.tick() => {
                let now = elapsed_ns(start);
                run.drive(Event::TimerFired(Timer::Pto), now, &sock, cfg.server, &payload).await;
            }
            _ = rtprop.tick() => {
                let now = elapsed_ns(start);
                run.drive(Event::TimerFired(Timer::RtpropProbe), now, &sock, cfg.server, &payload).await;
            }
            r = sock.recv_from(&mut buf) => {
                let Ok((n, _from)) = r else { continue }; // recv hatası → yoksay (deadline sınırlar).
                run.rx_bytes += n as u64;
                // [TUR-1x] V2 (0x14, 256µs) + V1 (0x10, µs) kabul — decode versiyona göre çözer.
                if n < 1 || (buf[0] != FRAME_TYPE_ACK && buf[0] != FRAME_TYPE_ACK_V2) {
                    continue;
                }
                let Ok(frame) = AckFrame::decode(&buf[..n]) else { continue };
                let now = elapsed_ns(start);
                let acked = frame.acked_full(run.max_emitted_seq + 1);
                // delivered_ns: birimin herhangi bir seq'i İLK kez acked → MUTLAK damga (ilk-ack-kazanır).
                for s in &acked {
                    let Some(&unit) = run.seq_to_unit.get(s) else { continue };
                    if let Some(slot) = run.delivered_ns.get_mut(usize::try_from(unit).unwrap_or(usize::MAX)) {
                        if slot.is_none() {
                            *slot = Some(now);
                            run.delivered_count += 1;
                        }
                    }
                }
                // sender'ı besle: inflight boşalt + RTT/BW + ARQ/fwd recovery (bench delivered'ı KENDİ damgaladı).
                run.drive(
                    Event::AckReceived(AckInfo { acked_seqs: acked, ack_delay: frame.ack_delay_duration() }),
                    now, &sock, cfg.server, &payload,
                ).await;
            }
        }
    }

    let events: Vec<RawEvent> = (0..msg_count)
        .map(|id| {
            let i = usize::try_from(id).unwrap_or(usize::MAX);
            RawEvent {
                msg_id: id,
                sent_ns: run.sent_ns.get(i).copied().flatten().unwrap_or(0),
                delivered_ns: run.delivered_ns.get(i).copied().flatten(),
                payload_bytes: cfg.payload_bytes,
            }
        })
        .collect();

    let meta = RunMeta {
        profile: cfg.profile,
        cc: cfg.cc.label().to_string(),
        transport: "kama".to_string(),
        quinn_version: KAMA_VERSION.to_string(),
        seed: cfg.seed,
        repeat: cfg.repeat,
        msg_count,
        payload_bytes: cfg.payload_bytes,
        max_deadline_ns: u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX),
        udp_tx_bytes: run.tx_bytes,
        tx: TxComponents {
            data: run.tx_data,
            retx: run.tx_retx,
            dup: run.tx_dup,
        },
        udp_rx_bytes: run.rx_bytes,
    };
    Ok(RunOutput { meta, events })
}

/// Server event pump (§4 receiver): per-peer `Receiver`; select{ DATA→on_datagram→SACK / ACK-timer→SACK }.
/// quinn accept-loop gibi sonsuz (harness process'i öldürür). Tek O(1) recv döngüsü per-mesaj izolasyonu
/// verir → quinn'in stream-başı task fan-out'una gerek yok (HoL yok).
async fn serve_loop(sock: UdpSocket, start: Instant) -> Result<(), BenchError> {
    let mut peers: HashMap<SocketAddr, Receiver> = HashMap::new();
    let mut ack_timer = interval(ACK_TICK);
    let mut buf = vec![0u8; SERVER_RECV_BUF];
    loop {
        tokio::select! {
            _ = ack_timer.tick() => {
                let now = elapsed_ns(start);
                let acks: Vec<(SocketAddr, Vec<u8>)> = peers
                    .iter_mut()
                    .filter_map(|(peer, rx)| rx.on_ack_timer(now).map(|a| (*peer, a.encode())))
                    .collect();
                for (peer, bytes) in acks {
                    let _ = sock.send_to(&bytes, peer).await;
                }
            }
            r = sock.recv_from(&mut buf) => {
                let Ok((n, peer)) = r else { continue };
                let Some((wire_seq, unit_id)) = decode_data(&buf[..n]) else { continue };
                let rx = peers.entry(peer).or_default();
                let expected = rx.largest_seq().map_or(0, |m| m + 1);
                let full = resolve_seq(expected, wire_seq);
                let now = elapsed_ns(start);
                let out = rx.on_datagram(RxDatagram { seq: full, unit_id, kind: RxKind::Data }, now);
                if let Some(ack) = out.ack {
                    let _ = sock.send_to(&ack.encode(), peer).await;
                }
            }
        }
    }
}

impl BenchTransport for KamaTransport {
    async fn run_server(&self, cfg: ServerCfg) -> Result<(), BenchError> {
        let sock = UdpSocket::bind(cfg.bind).await?; // bind = setup hatası → Err (harness abort).
        serve_loop(sock, Instant::now()).await
    }

    async fn run_client(&self, cfg: ClientCfg) -> Result<RunOutput, BenchError> {
        let sock = UdpSocket::bind("[::]:0").await?;
        client_loop(sock, cfg, Instant::now()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics;
    use crate::transport::Cc;

    fn cfg(server: SocketAddr, msgs: u64, deadline_ms: u64) -> ClientCfg {
        ClientCfg {
            server,
            msg_count: msgs,
            payload_bytes: 128,
            max_deadline: Duration::from_millis(deadline_ms),
            cc: Cc::Cubic,
            seed: 0,
            repeat: 0,
            profile: "loopback".to_string(),
        }
    }

    #[test]
    fn data_frame_roundtrips() {
        let f = encode_data(0x1234, 42, &[0x5A; 8]);
        assert_eq!(
            f.len(),
            HDR_DATA + 8 + TAG_STUB,
            "hdr + payload + [C] AEAD-tag stub"
        );
        assert_eq!(decode_data(&f), Some((0x1234, 42)));
        assert_eq!(decode_data(&f[..5]), None, "truncated → None");
        assert_eq!(decode_data(&[0x10, 0, 0]), None, "yanlış frame_type → None");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loopback_frame_invariants() {
        // 127.0.0.1 server+client, netem YOK → temiz yol → hepsi teslim + frame doğruluğu.
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let saddr = server.local_addr().unwrap();
        tokio::spawn(serve_loop(server, Instant::now()));

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let out = client_loop(client, cfg(saddr, 10, 5000), Instant::now())
            .await
            .unwrap();

        assert_eq!(out.events.len(), 10);
        assert_eq!(out.meta.transport, "kama");
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_some()),
            "temiz yol → tüm mesajlar teslim edilmeli"
        );
        // FRAME: delivered_ns MUTLAK (sent'ten SONRA), stored-latency DEĞİL (quinn'i bozan bug sınıfı).
        for e in &out.events {
            let d = e.delivered_ns.expect("delivered");
            assert!(
                d > e.sent_ns,
                "delivered_ns ({d}) mutlak + sent_ns ({}) sonrası olmalı",
                e.sent_ns
            );
        }
        // completion EĞRİSİ ayrışır (düz-eğri/frame bug'ını yakalar): 0ns → 0, deadline → 1.
        assert_eq!(metrics::completion(&out.events, Duration::ZERO), 0.0);
        assert_eq!(
            metrics::completion(&out.events, Duration::from_secs(5)),
            1.0
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dead_address_bounded_zero_completion() {
        // Ölü adres (kimse dinlemiyor) → hiç ACK → all-None, max_deadline ile bounded (HANG yok).
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let t0 = Instant::now();
        let out = client_loop(client, cfg(dead, 5, 400), Instant::now())
            .await
            .unwrap();
        assert!(
            t0.elapsed() < Duration::from_secs(3),
            "max_deadline ile bounded (300s HANG değil), süre: {:?}",
            t0.elapsed()
        );
        assert_eq!(out.events.len(), 5);
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_none()),
            "ölü adres → 0 teslim"
        );
        assert_eq!(
            metrics::completion(&out.events, Duration::from_secs(5)),
            0.0
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sent_ns_at_app_offer_not_paced_emit() {
        // 200 birim burst'ü (2400B) taşar → çoğu pacer'da saniyelerce dwell eder. sent_ns app-offer'da
        // (enqueue) damgalandığı için hepsi DAR t0 penceresinde olmalı. Wire-emit'te damgalansaydı
        // dwell'le saniyelere yayılırdı → completion kama lehine şişerdi (review CRITICAL regresyon guard).
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let saddr = server.local_addr().unwrap();
        tokio::spawn(serve_loop(server, Instant::now()));
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let out = client_loop(client, cfg(saddr, 200, 5000), Instant::now())
            .await
            .unwrap();
        assert_eq!(out.events.len(), 200);
        let (lo, hi) = out.events.iter().fold((u64::MAX, 0u64), |(lo, hi), e| {
            (lo.min(e.sent_ns), hi.max(e.sent_ns))
        });
        let span_ms = (hi - lo) as f64 / 1e6;
        assert!(
            span_ms < 500.0,
            "sent_ns app-offer penceresinde olmalı (enqueue, paced-emit değil); span={span_ms:.1}ms"
        );
    }
}
