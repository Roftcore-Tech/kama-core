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

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use adaptive_transport::ack::{AckFrame, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitKind};
use adaptive_transport::seq::resolve_seq;
use tokio::net::UdpSocket;
use tokio::time::interval;

use crate::budget::{BudgetGovernor, BudgetMode, IPV4_UDP_OVERHEAD};
use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};
use adaptive_transport::tail_recovery::TailRecoveryConfig;

use crate::transport::{BenchTransport, ClientCfg, KamaArm, KamaTransport, ServerCfg};

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
/// [KAPI-1 W6] Warmup birim-id tabanı: ölçüm-dışı warmup birimleri bu ofsetle akar — sent/delivered
/// vektörleri ölçüm-id'leriyle indekslendiğinden warmup id'leri kayıt yollarında YAPISAL olarak
/// görünmez (get_mut(huge)→None); teslimleri ayrı sette izlenir.
const WARMUP_BASE: u64 = 1 << 40;

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
    /// [KAPI-1 W2] forward-yön governor'ü. [W6] Arm ölçüm-t0'da (gate sonrası) yapılır;
    /// warmup trafiği pre_arm'da muaf kalır.
    gov: BudgetGovernor,
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
    /// [W6] Teslim edilen warmup birim-id'leri (ölçüm kayıtlarına girmez; faz-geçiş koşulu).
    warmup_delivered: HashSet<u64>,
    /// [P0 teşhis] KAMA_BENCH_TRACE=1 → stderr'e TX/ACK birim izi.
    trace: bool,
}

impl ClientRun {
    fn new(msg_count: usize, arm: KamaArm, budget: BudgetMode) -> Self {
        // KAPI-1 W4: kol seçimi Sender kuruluşunda uygulanır. TailV1 = adlandırılmış doğrulanmış
        // sabit; default bounds ile ENABLE ETME (o bounds hiç kampanya görmedi — tail_recovery.rs).
        let mut sender = Sender::new();
        if arm == KamaArm::TailV1 {
            sender.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
        }
        // [W6] gov burada ARM EDİLMEZ: ölçüm-t0'da (client_loop, gate sonrası) arm edilir —
        // warmup trafiği pre_arm'da görünür, bütçeden düşmez (DÜZLEM-A t=0 semantiği).
        Self {
            sender,
            gov: BudgetGovernor::new(budget),
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
            warmup_delivered: HashSet::new(),
            trace: std::env::var_os("KAMA_BENCH_TRACE").is_some(),
        }
    }

    /// [W6] Gelen datagramı işle: ACK ise teslim damgala (ölçüm-id'leri) + warmup-teslim izle +
    /// sender'ı besle. Warmup ve ölçüm fazları AYNI yolu kullanır (kod-tekilliği; frame doğruluğu
    /// tek yerde). ACK-dışı/bozuk frame yoksayılır.
    async fn handle_datagram(
        &mut self,
        raw: &[u8],
        now: u64,
        sock: &UdpSocket,
        server: SocketAddr,
        payload: &[u8],
    ) {
        self.rx_bytes += raw.len() as u64;
        // [TUR-1x] V2 (0x14, 256µs) + V1 (0x10, µs) kabul — decode versiyona göre çözer.
        if raw.is_empty() || (raw[0] != FRAME_TYPE_ACK && raw[0] != FRAME_TYPE_ACK_V2) {
            return;
        }
        // [inceleme #2] server ACK'leri de +16B AEAD-tag-stub taşır (DATA ile simetrik wire
        // modeli); decode'dan önce sondaki stub soyulur (bench çifti her zaman aynı binary).
        if raw.len() <= TAG_STUB {
            return;
        }
        let Ok(frame) = AckFrame::decode(&raw[..raw.len() - TAG_STUB]) else {
            return;
        };
        let acked = frame.acked_full(self.max_emitted_seq + 1);
        if self.trace {
            let units: Vec<String> = acked
                .iter()
                .map(|s| {
                    format!(
                        "{}:{}",
                        s,
                        self.seq_to_unit.get(s).map_or(-1i64, |u| *u as i64)
                    )
                })
                .collect();
            eprintln!("ACK t={now} seq:unit=[{}]", units.join(" "));
        }
        // delivered_ns: birimin herhangi bir seq'i İLK kez acked → MUTLAK damga (ilk-ack-kazanır).
        for s in &acked {
            let Some(&unit) = self.seq_to_unit.get(s) else {
                continue;
            };
            if unit >= WARMUP_BASE {
                self.warmup_delivered.insert(unit);
                continue;
            }
            if let Some(slot) = self
                .delivered_ns
                .get_mut(usize::try_from(unit).unwrap_or(usize::MAX))
            {
                if slot.is_none() {
                    *slot = Some(now);
                    self.delivered_count += 1;
                }
            }
        }
        // sender'ı besle: inflight boşalt + RTT/BW + ARQ/fwd recovery (bench delivered'ı KENDİ damgaladı).
        self.drive(
            Event::AckReceived(AckInfo {
                acked_seqs: acked,
                ack_delay: frame.ack_delay_duration(),
            }),
            now,
            sock,
            server,
            payload,
        )
        .await;
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
            // [P0 teşhis] birim-düzeyi TX izi (env-gated; ölçüm yoluna dokunmaz).
            if self.trace {
                eprintln!(
                    "TX t={now} unit={} seq={} kind={:?}",
                    d.unit_id, d.seq, d.kind
                );
            }
            // sent_ns burada DAMGALANMAZ — app-offer'da (enqueue) damgalanır (quinn frame'i, aşağıdaki
            // client_loop notu). Wire-emit'te damgalamak kama'nın pacing dwell'ini latency'den dışlardı.
            let frame = encode_data((d.seq & 0xFFFF) as u16, d.unit_id, payload);
            // [inceleme #1] tx sayaçları TRANSPORT-DENEMESİ düzlemi: governor reddi DAHİL sayılır —
            // quinn-proto stats'ı poll_transmit'te (socket'ten ÖNCE) saydığından üç kolun udp_tx'i
            // ancak böyle aynı düzlemde olur. Gerçek-enjekte düzlem budget.injected_bytes'ta.
            let n = frame.len() as u64;
            self.tx_bytes += n;
            match d.kind {
                TransmitKind::Data => self.tx_data += n,
                TransmitKind::Retransmit => self.tx_retx += n,
                TransmitKind::Duplicate => self.tx_dup += n,
                // FecParity emit-edilmez (§10 politika); CsProbe (DR-CS-1, yalnız
                // experimental-cold-start feature'ıyla-var) bench-legacy-sürücüde sayılmaz.
                _ => {}
            }
            // [KAPI-1 W2] packet-atomic bütçe kapısı: ret = enjeksiyon yok (kanal-silinmesi
            // eşdeğeri; sender state'i ZATEN emisyonu işledi — INV-RATE-1 gereği rate düşmez,
            // kayıp kurtarma kolu normal işler). Governor bekletmez (pacing-nötr).
            if !self.gov.permit(n + IPV4_UDP_OVERHEAD, now) {
                continue;
            }
            let _ = sock.send_to(&frame, server).await;
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
    let mut run = ClientRun::new(
        usize::try_from(msg_count).unwrap_or(0),
        cfg.kama_arm,
        cfg.budget,
    );

    // Tüm birimleri t0'da enqueue — engine'in İÇ pacing'i yayılımı belirlesin (yapay serileştirme değil;
    // her iki transport da aynı start'tan aynı max_deadline bütçesini alır → adil karşılaştırma).
    //
    // sent_ns = APP-OFFER anı (quinn ile AYNI zero-point: quic.rs task başında sent_ns=start.elapsed(),
    // quinn'in İÇ pacing'inden ÖNCE → latency quinn'in kuyruğunu İÇERİR). Kama da app-offer'da damgalar →
    // latency kama'nın pacing dwell'ini İÇERİR → elmalar-elmalar. (review CRITICAL: wire-emit'te damgalamak
    // dwell'i dışlar → completion kama lehine ŞİŞER; delivered_ns frame-bug sınıfının sent-tarafı ikizi.)
    //
    // [KAPI-1 W3] interval==0 (burst): bu yol AYNEN korunur. interval>0 (paced): sunumlar select
    // döngüsünde zamanlanır (mesaj i → t=i×interval); sent_ns yine app-offer anı.
    let interval_ns = u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX);

    // [W6] Kuruluş-gate'i: kama'da handshake yok → soket bağlanır bağlanmaz "kurulu" (DÜZLEM-A
    // temiz-ağ kuruluş alanı). Harness bu noktada (A2) netem'i uygular; warmup impaired koşar.
    if let Some(g) = &cfg.establish_gate {
        crate::transport::gate_sync(g).await?;
    }

    // ── [KAPI-1 W6] FAZ-0: warmup (ölçüm-DIŞI) + gate senkronu ─────────────────────────────
    // Warmup birimleri WARMUP_BASE+i id'leriyle AYNI motor/soket yolundan akar; kayıt yollarına
    // yapısal olarak girmez. Amaç: transport state'i (estimator/pacer/RTT) ölçümden önce oturt —
    // A1'de temiz kanalda (converge), A2'de impairment altında (steady-state); farkı harness'ın
    // netem'i NE ZAMAN uyguladığı belirler (r5 B.2). Sonra quiescence + gate: `<gate>.ready`
    // yazılır, harness impairment/barrier işini bitirip `<gate>.go` dokunur; ölçüm-t0 = GO anı.
    if cfg.warmup_msgs > 0 {
        let warm_start = elapsed_ns(start);
        let warm_cap_ns = interval_ns
            .saturating_mul(cfg.warmup_msgs)
            .saturating_add(30_000_000_000); // sunum-planı + 30 s tavan (bounded)
        let mut wpace = interval(PACE_TICK);
        let mut wrtprop = interval(RTPROP_TICK);
        let mut wbuf = [0u8; CLIENT_RECV_BUF];
        let mut wnext: u64 = 0;
        let wdeadline = tokio::time::sleep(Duration::from_nanos(warm_cap_ns));
        tokio::pin!(wdeadline);
        while (run.warmup_delivered.len() as u64) < cfg.warmup_msgs {
            tokio::select! {
                () = &mut wdeadline => break,
                () = tokio::time::sleep(Duration::from_nanos(
                    warm_start
                        .saturating_add(interval_ns.saturating_mul(wnext))
                        .saturating_sub(elapsed_ns(start)),
                )), if wnext < cfg.warmup_msgs => {
                    let now = elapsed_ns(start);
                    let id = WARMUP_BASE + wnext;
                    wnext += 1;
                    run.drive(
                        Event::AppData(OpaqueUnit { id, size: unit_size }),
                        now, &sock, cfg.server, &payload,
                    ).await;
                }
                _ = wpace.tick() => {
                    let now = elapsed_ns(start);
                    run.drive(Event::TimerFired(Timer::Pto), now, &sock, cfg.server, &payload).await;
                }
                _ = wrtprop.tick() => {
                    let now = elapsed_ns(start);
                    run.drive(Event::TimerFired(Timer::RtpropProbe), now, &sock, cfg.server, &payload).await;
                }
                r = sock.recv_from(&mut wbuf) => {
                    let Ok((n, _)) = r else { continue };
                    let now = elapsed_ns(start);
                    let raw = wbuf[..n].to_vec();
                    run.handle_datagram(&raw, now, &sock, cfg.server, &payload).await;
                }
            }
        }
        // [inceleme #0] Quiescence: sabit-uyku DEĞİL — sınırlı drain-pompası: hedef in-flight=0
        // (§3.2 ön-kayıt), ama severe-loss'ta bounded (3 s tavan; motor birim TERK ETMEZ → p3'te
        // pif→0 ulaşılamayabilir). Kalan = residue; t0'da ÖLÇÜLÜR (RunMeta.warmup_residue) ve
        // post-t0 residue trafiği TÜM kollarda simetrik olarak ölçüme dahildir.
        let drain_cap = tokio::time::sleep(Duration::from_secs(3));
        tokio::pin!(drain_cap);
        let mut dpace = interval(PACE_TICK);
        loop {
            if run.sender.diag_pif() == 0 {
                break;
            }
            let mut wbuf2 = [0u8; CLIENT_RECV_BUF];
            tokio::select! {
                () = &mut drain_cap => break,
                _ = dpace.tick() => {
                    let now = elapsed_ns(start);
                    run.drive(Event::TimerFired(Timer::Pto), now, &sock, cfg.server, &payload).await;
                }
                r = sock.recv_from(&mut wbuf2) => {
                    let Ok((n, _)) = r else { continue };
                    let now = elapsed_ns(start);
                    let raw = wbuf2[..n].to_vec();
                    run.handle_datagram(&raw, now, &sock, cfg.server, &payload).await;
                }
            }
        }
    }
    if let Some(gate) = &cfg.gate {
        crate::transport::gate_sync(gate).await?;
    }

    // ── Ölçüm-t0 (DÜZLEM-A): sayaçlar sıfırlanır, TRANSPORT STATE KORUNUR (r5 B.2
    // counter_reset_without_state_reset), bütçe ARM edilir. Warmup/gate yoksa t0 ≈ 0 (eski yol).
    let t0 = elapsed_ns(start);
    let t0_unix_ns = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    )
    .unwrap_or(0);
    let warmup_residue = cfg
        .warmup_msgs
        .saturating_sub(run.warmup_delivered.len() as u64);
    run.gov.arm(t0);
    run.tx_bytes = 0;
    run.rx_bytes = 0;
    run.tx_data = 0;
    run.tx_retx = 0;
    run.tx_dup = 0;

    if interval_ns == 0 {
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
    }
    let mut next_submit: u64 = if interval_ns == 0 { msg_count } else { 0 };

    let mut pace = interval(PACE_TICK);
    let mut rtprop = interval(RTPROP_TICK);
    let mut buf = [0u8; CLIENT_RECV_BUF];
    // Koşu sonu = son-sunum + max_deadline (r5 §G: test_end = last_submission + mission_deadline;
    // burst'ta son-sunum ≈ t0 → mevcut semantik aynen). t0-göreli, start'tan kalan süre olarak.
    let total_deadline_ns = t0
        .saturating_add(interval_ns.saturating_mul(msg_count.saturating_sub(1)))
        .saturating_add(u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX));
    let deadline = tokio::time::sleep(Duration::from_nanos(
        total_deadline_ns.saturating_sub(elapsed_ns(start)),
    ));
    tokio::pin!(deadline);

    // [TUR-A teşhis] rate-trajektori tap'ı (SALT-GÖZLEM; kaldırılabilir). floor→btlbw ramp kilidini ölç.
    let mut traj = interval(Duration::from_millis(500));
    let unit_wire = HDR_DATA as u64 + cfg.payload_bytes + TAG_STUB as u64;

    while run.delivered_count < msg_count {
        tokio::select! {
            () = &mut deadline => break,
            // [KAPI-1 W3] paced sunum: mesaj i, t=i×interval'de app-offer edilir. Sleep her select
            // yinelemesinde kalan-süreden yeniden kurulur (1ms pace tick'i döngüyü sürekli uyandırır
            // → vade geldiğinde kalan=0, kol anında ateşler; sapma ≤ ~1ms ≪ aralıklar 1s+).
            () = tokio::time::sleep(Duration::from_nanos(
                t0.saturating_add(interval_ns.saturating_mul(next_submit))
                    .saturating_sub(elapsed_ns(start)),
            )), if next_submit < msg_count => {
                let now = elapsed_ns(start);
                let id = next_submit;
                next_submit += 1;
                if let Some(slot) = run
                    .sent_ns
                    .get_mut(usize::try_from(id).unwrap_or(usize::MAX))
                {
                    *slot = Some(now);
                }
                run.drive(
                    Event::AppData(OpaqueUnit { id, size: unit_size }),
                    now,
                    &sock,
                    cfg.server,
                    &payload,
                )
                .await;
            }
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
                let fnr = run.sender.diag_fuzzy_no_rule();
                let (lfb, wns) = run.sender.diag_wbtlbw();
                eprintln!(
                    "TRAJ t={:.1} state={:?} btlbw={} rate={} srtt_us={} infl={} sent_u={} deliv={} loss_pkt={} loss_time={} pend={} rtt_rej={} adly_max_us={} unloss={} lppm={} tomb={}/{}/{}/{}/{}/{} pif={} rtprop_us={} qd_us={} probe={}/{}/{}/{} fnr={} wbt={}/{:.1}",
                    now as f64 / 1e9, st, btlbw, rate, srtt_us, infl,
                    run.tx_data / unit_wire.max(1), run.delivered_count, lpkt, ltime, lpend,
                    rtt_rej, adly_max, unloss, lppm, tomb_live, tomb_hit, bw_rec, tomb_ev, ambig, credited, pif, rtprop_us, qd_us, ps, pa, pt, pp, fnr,
                    lfb,
                    wns as f64 / 1e9,
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
                let now = elapsed_ns(start);
                // [W6] tek ACK-yolu: warmup fazıyla AYNI kod (frame doğruluğu tek yerde).
                let raw = buf[..n].to_vec();
                run.handle_datagram(&raw, now, &sock, cfg.server, &payload).await;
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
            // Bu kol security-stub'dır: FEC parity emit ETMEZ, PARAM akışı YOKTUR (dosya
            // başlığındaki INV-FEC-1 gerekçesi) → iki sınıf de yapısal olarak sıfırdır.
            fec: 0,
            ctrl: 0,
        },
        udp_rx_bytes: run.rx_bytes,
        kama_arm: cfg.kama_arm.label().to_string(),
        submit_interval_ns: interval_ns,
        budget: run.gov.snapshot(),
        t0_unix_ns,
        events_epoch_offset_ns: t0, // kama event'leri koşu-epoch'undan damgalanır
        warmup_residue,
    };
    Ok(RunOutput { meta, events })
}

/// Server event pump (§4 receiver): per-peer `Receiver`; select{ DATA→on_datagram→SACK / ACK-timer→SACK }.
/// quinn accept-loop gibi sonsuz (harness process'i öldürür). Tek O(1) recv döngüsü per-mesaj izolasyonu
/// verir → quinn'in stream-başı task fan-out'una gerek yok (HoL yok).
async fn serve_loop(
    sock: UdpSocket,
    start: Instant,
    quota_r: u32,
    budget: BudgetMode,
    gate_watch: Option<std::path::PathBuf>,
) -> Result<(), BenchError> {
    let mut peers: HashMap<SocketAddr, Receiver> = HashMap::new();
    let mut ack_timer = interval(ACK_TICK);
    let mut buf = vec![0u8; SERVER_RECV_BUF];
    let mut rtraj_tick: u32 = 0;
    // [KAPI-1 W2+W6] reverse-yön (ACK-egress) governor'ü. gate_watch verilirse arm, client'ın
    // ölçüm-t0'ına (`<gate>.go` dosyası) senkronlanır — warmup ACK'leri pre_arm'da muaf (r5 D.2);
    // verilmezse hemen arm (eski davranış). Server RunOutput üretmez; BUDGET-REV satırı raporlar.
    let mut gov = BudgetGovernor::new(budget);
    let go_file = gate_watch.map(|g| format!("{}.go", g.display()));
    if go_file.is_none() {
        gov.arm(elapsed_ns(start));
    }
    loop {
        tokio::select! {
            _ = ack_timer.tick() => {
                let now = elapsed_ns(start);
                // [W6] gate-watch: .go belirince arm (20 ms poll — ölçüm-t0 sapması ≤ ACK_TICK).
                if !gov.is_armed() {
                    if let Some(f) = &go_file {
                        if std::path::Path::new(f).exists() {
                            gov.arm(now);
                        }
                    }
                }
                let acks: Vec<(SocketAddr, Vec<u8>)> = peers
                    .iter_mut()
                    .filter_map(|(peer, rx)| rx.on_ack_timer(now).map(|a| (*peer, a.encode())))
                    .collect();
                for (peer, mut bytes) in acks {
                    bytes.resize(bytes.len() + TAG_STUB, 0); // [inceleme #2] ACK'e de tag-stub
                    if gov.permit(bytes.len() as u64 + IPV4_UDP_OVERHEAD, now) {
                        let _ = sock.send_to(&bytes, peer).await;
                    }
                }
                if budget != BudgetMode::Unlimited && rtraj_tick.is_multiple_of(500) {
                    let s = gov.snapshot();
                    eprintln!(
                        "BUDGET-REV t={:.1} injected={}B/{}p refused={}B/{}p first_refusal_ns={}",
                        now as f64 / 1e9,
                        s.injected_bytes,
                        s.injected_pkts,
                        s.refused_bytes,
                        s.refused_pkts,
                        s.first_refusal_ns
                    );
                }
                // [TUR-5 r3 §6] RTRAJ ölçüm-kanalı: K-4 invariant+diagnostic server-tarafta doğar
                // (client rx_bytes yalnız hayatta-kalan-bayt vekili). ~500ms'te bir.
                rtraj_tick += 1;
                if rtraj_tick.is_multiple_of(25) {
                    let (mut ep, mut qb, mut ac, mut tot) = (0u64, 0u64, 0u64, 0u64);
                    for rx in peers.values() {
                        let (e, q, a, t) = rx.diag_return_supply();
                        ep += e; qb += q; ac += a; tot += t;
                    }
                    eprintln!("RTRAJ t={:.1} ep={} quota={} accel={} emit_total={}",
                        now as f64 / 1e9, ep, qb, ac, tot);
                }
            }
            r = sock.recv_from(&mut buf) => {
                let Ok((n, peer)) = r else { continue };
                let Some((wire_seq, unit_id)) = decode_data(&buf[..n]) else { continue };
                let rx = peers.entry(peer).or_insert_with(|| {
                    let mut r = Receiver::new();
                    r.set_quota_r(quota_r);
                    r
                });
                let expected = rx.largest_seq().map_or(0, |m| m + 1);
                let full = resolve_seq(expected, wire_seq);
                let now = elapsed_ns(start);
                let out = rx.on_datagram(RxDatagram { seq: full, unit_id, kind: RxKind::Data }, now);
                if let Some(ack) = out.ack {
                    let mut bytes = ack.encode();
                    bytes.resize(bytes.len() + TAG_STUB, 0); // [inceleme #2] ACK'e de tag-stub
                    if gov.permit(bytes.len() as u64 + IPV4_UDP_OVERHEAD, now) {
                        let _ = sock.send_to(&bytes, peer).await;
                    }
                }
            }
        }
    }
}

impl BenchTransport for KamaTransport {
    async fn run_server(&self, cfg: ServerCfg) -> Result<(), BenchError> {
        let sock = UdpSocket::bind(cfg.bind).await?; // bind = setup hatası → Err (harness abort).
        serve_loop(
            sock,
            Instant::now(),
            cfg.quota_r,
            cfg.budget,
            cfg.gate_watch,
        )
        .await
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
            kama_arm: KamaArm::Arq,
            submit_interval: Duration::ZERO,
            budget: BudgetMode::Unlimited,
            warmup_msgs: 0,
            gate: None,
            establish_gate: None,
            // Bu kolda FEC yok (yalnız kama-product kolu okur).
            fec: crate::transport::FecArm::Off,
        }
    }

    /// KAPI-1 W4 pin: kol bayrağı Sender'a gerçekten işlenmeli — TailV1 doğrulanmış sabiti kurar,
    /// Arq default-OFF bırakır. Bu pin kırılırsa "ON kolu" sessizce OFF koşar (kampanya-geçersiz).
    #[test]
    fn kama_arm_flag_installs_validated_v1() {
        assert!(!ClientRun::new(1, KamaArm::Arq, BudgetMode::Unlimited)
            .sender
            .tail_recovery_enabled());
        assert!(ClientRun::new(1, KamaArm::TailV1, BudgetMode::Unlimited)
            .sender
            .tail_recovery_enabled());
    }

    /// [KAPI-1 W2] Forward bütçesi üründe packet-atomic: frame 155B → ip 183B; cap = 2×183+90 →
    /// TAM 2 paket girer (3.'ye 90B yetmez — partial yok), kalan emisyonlar reddedilir ve sayılır.
    /// Loopback kayıpsız → teslim == enjekte-edilen-DATA (girmeyen asla teslim olamaz).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn burst_budget_caps_injection_packet_atomically() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let saddr = server.local_addr().unwrap();
        tokio::spawn(serve_loop(
            server,
            Instant::now(),
            4,
            BudgetMode::Unlimited,
            None,
        ));

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut c = cfg(saddr, 10, 1500);
        c.budget = BudgetMode::Burst {
            cap_bytes: 2 * 183 + 90,
        };
        let out = client_loop(client, c, Instant::now()).await.unwrap();

        let b = &out.meta.budget;
        assert_eq!(
            (b.injected_pkts, b.injected_bytes),
            (2, 2 * 183),
            "packet-atomic: tam 2 paket, overshoot yok"
        );
        assert!(b.refused_pkts > 0, "kalan emisyon denemeleri reddedilmeli");
        assert!(
            b.first_refusal_ns > 0,
            "budget_exhaustion_time kayıtlı olmalı"
        );
        let delivered = out
            .events
            .iter()
            .filter(|e| e.delivered_ns.is_some())
            .count();
        assert_eq!(
            delivered, 2,
            "teslim = enjekte edilen DATA (loopback kayıpsız)"
        );
    }

    /// [KAPI-1 W6] İki-fazlı koşu pini: warmup ölçüm-DIŞI kalmalı — events yalnız ölçüm
    /// mesajları, tx sayaçları t0'da sıfır (warmup byte'ı sızarsa bu test kırılır), gate
    /// protokolü (ready→go) işler, aynı Sender iki fazda da yaşar (state korunur).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warmup_and_gate_exclude_warmup_from_measurement() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let saddr = server.local_addr().unwrap();
        tokio::spawn(serve_loop(
            server,
            Instant::now(),
            4,
            BudgetMode::Unlimited,
            None,
        ));

        let dir = std::env::temp_dir().join(format!("kapi-gate-kama-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gate = dir.join("g1");
        let g2 = gate.clone();
        // Harness rolü: ready'yi gör → go yaz (barrier'ın karşı ucu).
        let harness = tokio::spawn(async move {
            let ready = format!("{}.ready", g2.display());
            for _ in 0..2000 {
                if std::path::Path::new(&ready).exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            std::fs::write(format!("{}.go", g2.display()), b"go").unwrap();
        });

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut c = cfg(saddr, 5, 5000);
        c.warmup_msgs = 3;
        c.gate = Some(gate.clone());
        let out = client_loop(client, c, Instant::now()).await.unwrap();
        harness.await.unwrap();

        assert_eq!(out.events.len(), 5, "yalnız ölçüm mesajları kayıtta");
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_some()),
            "temiz yol → 5/5 teslim"
        );
        let wire = (HDR_DATA as u64 + 128 + TAG_STUB as u64) * 5;
        assert_eq!(
            out.meta.tx.data, wire,
            "tx.data tam 5 ölçüm-DATA'sı olmalı (warmup 3'ü t0-sıfırlamasıyla dışarıda)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// [KAPI-1 W3] paced workload pini: mesaj i'nin app-offer'ı (sent_ns) plana (i×interval)
    /// göre damgalanmalı — ne öncesinde (plan ihlali) ne çok sonrasında (sunum kayması, latency'yi
    /// sahte şişirir). Burst yolunun "dar t0 penceresi" pini ayrı testte aynen duruyor.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn paced_submission_stamps_app_offer_on_schedule() {
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let saddr = server.local_addr().unwrap();
        tokio::spawn(serve_loop(
            server,
            Instant::now(),
            4,
            BudgetMode::Unlimited,
            None,
        ));

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut c = cfg(saddr, 4, 3000);
        c.submit_interval = Duration::from_millis(100);
        let out = client_loop(client, c, Instant::now()).await.unwrap();

        assert_eq!(out.meta.submit_interval_ns, 100_000_000);
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_some()),
            "temiz yol → tüm mesajlar teslim"
        );
        for (i, e) in out.events.iter().enumerate() {
            let due = 100_000_000u64 * i as u64;
            assert!(
                e.sent_ns >= due,
                "mesaj {i} planından ÖNCE sunuldu: {} < {due}",
                e.sent_ns
            );
            assert!(
                e.sent_ns < due + 80_000_000,
                "mesaj {i} sunumu plandan çok kaydı: {} (due {due})",
                e.sent_ns
            );
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
        tokio::spawn(serve_loop(
            server,
            Instant::now(),
            4,
            BudgetMode::Unlimited,
            None,
        ));

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
        tokio::spawn(serve_loop(
            server,
            Instant::now(),
            4,
            BudgetMode::Unlimited,
            None,
        ));
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
