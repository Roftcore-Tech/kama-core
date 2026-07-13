//! Sender State Machine (engine spec §3) — 18 bileşeni tek olay-güdümlü döngüde birleştiren ŞEF.
//!
//! **Deterministik çekirdek (ADR-17):** olaylar girer → transmit KARARLARI çıkar. Gerçek I/O ve
//! kripto DIŞINDA (INV-ENGINE-SOC): Sender yalnız opak birim KİMLİĞİ + BOYUTU tutar (`OpaqueUnit`),
//! bytes/AEAD/anahtar TUTMAZ. `on_event` saf: aynı girdi → aynı çıktı → golden-vector'le test edilir.
//!
//! **INV-RATE-1:** loss `send_rate`'i DÜŞÜRMEZ. Recovery durumları (RECOVER_ARQ/FWD) pacing_gain'i
//! `Steady` tutar; loss yalnız [`loss`](crate::loss) + fuzzy `loss_ppm` → FEC/ARQ recovery kolunu besler.
//! Retransmit/duplication DATA pacing bütçesini PAYLAŞIR (§9/§13) → recovery fırtınası rate'i patlatamaz.
//!
//! **Recovery İCRA SoC (INV-FEC-1/INV-ENGINE-SOC):** ARQ retransmit (§9) + duplication (§11)
//! transport-owned'dır — transport kayıp birimin METADATA'sını (id/size) tutar, yeniden pace eder.
//! FEC parity (§10) yalnız POLİTİKA'dır (parity_ratio + `RecoveryAction::MinimalParity`); RS-encode +
//! per-shard AEAD GÜVENLİK katmanında (code-then-encrypt-authenticate-shards). Transport parity BYTE'ı
//! ÜRETMEZ — oranı verir, güvenlik katmanı encode eder, transport opak parity birimlerini pace eder.
//!
//! **SPEC-UYUMLU (§3 AMENDMENT-1 + §5.1/AMENDMENT-2 — impl-driven, spec'e patch'lendi):** RECOVER_FWD
//! *ve* RECOVER_ARQ her aktif fazdan erişilir (§3 AMENDMENT-1: ACK-yolu çökerse STARTUP platoya ulaşamaz
//! → recovery kilitlenmesin; forward-recovery ACK-bağımsız). Öncelik: ackconf LOW → RECOVER_FWD (§11) >
//! pending loss → RECOVER_ARQ (§9). ack_confidence = ACK SESSİZLİĞİ (§5.1/AMENDMENT-2: naif sent/recv
//! oranı değil). Bu kararlar önce implementasyonda bulundu, gerekçeleriyle spec'e cerrahi eklendi (çelişki yok).
//!
//! **Kapsam (Artım 1+3):** state machine + ölçüm + karar pipeline + DATA pacing + recovery İCRA
//! (ARQ retransmit / forward duplication). Receiver (§4) = sonraki artım.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::arq::{
    ArqScheduler, CapExceedPolicy, DeadlineArqScheduler, LostSeq, ReorderTracker, RetxConfig,
    RetxDecision,
};
use crate::budget::{self, BudgetConfig};
use crate::bw::{BtlBwFilter, DeliveryRateSampler, SentPacket};
use crate::emergency::{
    self, EmergencyConfig, EmergencyLevel, Framing, HysteresisConfig, HysteresisController,
    RecoveryAction, RecoveryModeController,
};
use crate::fusion::{
    DeterministicFuzzyController, FusionController, FusionDecision, FusionInputs, Mode,
};
use crate::loss::{LossDetector, ThresholdLossDetector};
use crate::pacer::{TokenBucket, DEFAULT_BURST_CAP};
use crate::rate::{effective_send_rate, PacingPhase, SrcConfig, DEFAULT_MTU};
use crate::rtt::{apply_ack_delay, RttEstimator};

/// Sender durumları (§3). Geçişler event-driven + deterministik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderState {
    /// Gönderilecek yok; timer'lar pasif.
    Idle,
    /// §3.3 cold-start: motor HİÇ ölçüm yokken. En-kötü-köşe önsel pacing (floor) + geometrik kopya;
    /// loss-ilanı/PTO/emergency/ack_confidence DONDURULUR. İlk geçerli SACK'te → Startup. [K-KEYSTONE]
    PreConverge,
    /// BtlBw hızlı keşif (pacing_gain yüksek), path doldur.
    Startup,
    /// pace = BtlBw; periyodik BW/RTprop probe.
    Steady,
    /// residual loss + ack_confidence OK → selective retransmit (§9).
    RecoverArq,
    /// ack_confidence LOW → forward redundancy (§11), ACK-bağımsız.
    RecoverFwd,
    /// RTprop probe: inflight düşür, min-RTT ölç (§8.3).
    DrainRtprop,
}

/// Opak güvenli birim — güvenlik katmanının `ct||tag` blob'u. Transport yalnız KİMLİK+BOYUT
/// görür (INV-ENGINE-SOC: bytes YOK). `id` çağıranın gerçek payload'una geri-eşlemesi içindir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueUnit {
    /// Çağıran-tanımlı birim kimliği (bytes'a geri-eşleme).
    pub id: u64,
    /// Wire boyutu (pacing/inflight muhasebesi).
    pub size: u64,
}

/// Sender'a giren olaylar (§3 ana döngü). `PacketSent` DIŞ olay DEĞİL — `ScheduleTransmit`
/// içinde OnPacketSent (§3.1) muhasebesi olarak yapılır.
#[derive(Debug, Clone)]
pub enum Event {
    /// App gönderilecek opak birim verdi → enqueue; IDLE ise STARTUP.
    AppData(OpaqueUnit),
    /// ACK geldi (§2 rekonstrüksiyondan SONRA: yeni acked seq'ler + ack_delay).
    AckReceived(AckInfo),
    /// Timer ateşlendi (§12).
    TimerFired(Timer),
}

/// §2 ACK rekonstrüksiyonunun ÇIKTISI (transport-control codec ayrı; burada çözülmüş girdi).
#[derive(Debug, Clone)]
pub struct AckInfo {
    /// Bu ACK ile YENİ acknowledge edilen seq'ler (full-counter).
    pub acked_seqs: Vec<u64>,
    /// largest_seq alımı → ACK gönderimi arası (§6 RTT düzeltmesi).
    pub ack_delay: Duration,
}

/// Zamanlayıcı türü (§12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {
    /// Probe timeout — zaman-eşiği loss tespiti + probe.
    Pto,
    /// RTprop probe vadesi → DRAIN_RTPROP.
    RtpropProbe,
}

/// Transmit kararı — çağıran `unit_id`'yi gerçekten yollar (güvenlik katmanı AEAD uygular, socket gönderir).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransmitDecision {
    /// Ne tür gönderim.
    pub kind: TransmitKind,
    /// Atanan wire seq (full-counter; wire'da u16, §2).
    pub seq: u64,
    /// Gönderilecek opak birim kimliği.
    pub unit_id: u64,
    /// Wire boyutu.
    pub size: u64,
}

/// Transmit türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmitKind {
    /// tx_queue'dan yeni DATA.
    Data,
    /// ARQ selective retransmit (§9) — kayıp birim, yeni seq.
    Retransmit,
    /// forward-recovery duplication (§11) — ACK-bağımsız redundant kopya.
    Duplicate,
    /// FEC parity shard (§10) — POLİTİKA; transport üretmez (güvenlik katmanı encode eder). Emit EDİLMEZ.
    FecParity,
}

/// Bir gönderilmiş paketin muhasebe kaydı (RTT/BW/loss + retransmit).
struct InflightPacket {
    unit: OpaqueUnit,
    send_ns: u64,
    bw_sample: SentPacket,
    retx_count: u8,
    origin: TransmitKind,
}

/// Kayıp ilan edilmiş, recovery bekleyen kayıt (birim + öncelik metadata).
#[derive(Debug, Clone, Copy)]
struct LostRec {
    seq: u64,
    unit: OpaqueUnit,
    deadline_ns: u64,
    retx_count: u8,
    /// [TUR-2a D-1/3 kol-1] orijinal gönderim anı — un-loss'ta (retransmit HENÜZ yoksa,
    /// retx_count==0) orijinal RTT örneği GEÇERLİDİR; bu damgayla alınır.
    send_ns: u64,
}

/// [TUR-2b D-1/4] TOMBSTONE — kayıp-İLANI sonrası SAKLANAN gönderim kaydı (geç-ACK eşleştirme).
/// Active-flight muhasebesinden DÜŞER (gönderim durmaz) ama kayıt YAŞAR: geç-ACK gelirse
/// kol-1 (retx_count==0) BW-örneği NORMAL + Data'ysa RTT; kol-2 (retx'li) örneksiz sayaç
/// (tam kurallar TUR-2c). Süresi/tavanı dolan kaydın geç-ACK'i → Expired/Unknown (örneksiz).
#[derive(Debug, Clone, Copy)]
struct TombstoneRec {
    unit: OpaqueUnit,
    send_ns: u64,
    bw_sample: SentPacket,
    retx_count: u8,
    origin: TransmitKind,
    expiry_ns: u64, // ilan + K_TOMB×T_loss(ilan-anı)
}

/// Zamanlanmış forward-recovery duplication kopyası (§11 spacing — burst-loss dayanıklılığı).
#[derive(Debug, Clone, Copy)]
struct DupRec {
    due_ns: u64,
    unit: OpaqueUnit,
    /// §3.3-B geometrik kopya reschedule aralığı (ns). 0 = emergency (fire-once, reschedule YOK);
    /// >0 = PRE_CONVERGE geometrik kopya → ateşlenince due += interval, interval ×= COPY_MULTIPLIER.
    interval_ns: u64,
}

/// İlk RTT ölçülene kadar pacing/eşik için başlangıç RTT [CAL initial].
const INITIAL_RTT: Duration = Duration::from_millis(100);
/// STARTUP→STEADY plato: BtlBw N ardışık round Δ<ε ise plato [CAL].
const STARTUP_PLATEAU_ROUNDS: u32 = 3;
/// plato ε (per-mille): btlbw artışı < %5 → stabil.
const STARTUP_PLATEAU_EPS_PERMILLE: u64 = 50;
/// ack_confidence: bu kadar srtt sessizlik (ACK yok) → tam güvensizlik ölçeği [CAL].
const ACK_SILENCE_SRTT_MULT: u32 = 3;
/// last_recovery Duplication değilse forward-recovery için varsayılan faktör/aralık [CAL].
const DEFAULT_DUP_FACTOR: u8 = 2;
const DEFAULT_DUP_SPACING: Duration = Duration::from_millis(20);
/// §3.3-A en-kötü-köşe önsel pacing floor (B/s) — ürün floor'u ~0.25 kbps. min_rate fixture (Ek-B)
/// PRE_CONVERGE'de UYGULANMAZ; bu taban kullanılır → 1 kbps kanalı floodlamaz. [CAL PLACEHOLDER]
const PRE_CONVERGE_FLOOR_BW: u64 = 31;
/// [TUR-1b D-1/2] Cadence-taban bileşenleri — receiver §2.1/§16.1 AYNA sabitleri (sender-yerel,
/// deterministik). Beklenen ACK-frekansı (receiver ACK_FREQ_N) [CAL].
const EXPECTED_ACK_FREQ_N: u64 = 2;
/// SACK wire boyutu (§16.1 `sack_wire` = 65B; V2 15B gövde dahil).
const EAI_SACK_WIRE_BYTES: u64 = 65;
/// Return-bw worst-corner önseli (§16.1; §3.3-E-FULL ölçülen return_bw ile değiştirecek) [CAL].
const EAI_RETURN_BW_PRIOR: u64 = 31;
/// K_c cadence çarpanı = 3/2 (1.5) [CAL kalibrasyon-matrisi {1.25, 1.5, 2.0}].
const CADENCE_KC_NUM: u64 = 3;
const CADENCE_KC_DEN: u64 = 2;
/// [TUR-2a D-1/5] W_loss ledger-penceresi çarpanı: W_loss = K_wl × ack_epoch [CAL]
/// (K_bw/K_rtprop merkezi 8 ile hizalı başlangıç; saat-sınıfı: kanal-türevi, ZM-2).
const K_WL: u32 = 8;
/// [TUR-2b D-1/4] Tombstone TTL çarpanı: TTL = K_tomb × T_loss(ilan-anı) [CAL]. Onaylı formülün
/// `min(mesaj-deadline, ...)` kolu motorda uygulanamaz (per-unit deadline engine'e taşınmıyor —
/// bench/app katmanında) → TTL tek-kollu; spec §5 zeyli. Saat-sınıfı: T_loss-türevi (kanal, ZM-2).
const K_TOMB: u32 = 8;
/// [TUR-3a D-2/1] Estimator pencere çarpanları [CAL, matris {6,8,10}]:
/// W_BTLBW = max(10s, K_BW×ack_epoch); W_RTPROP = max(10s, K_RTPROP×corrected_srtt);
/// ack_epoch = max(corrected_srtt, EAI). Saat-sınıfı: kanal-türevi (ZM-2 — 10s wall-clock
/// sub-kbps'te 0.8-3.3 RTT'ye düşüyordu: btlbw 88→4 çöküş sınıfı, §16.2).
const K_BW: u32 = 8;
const K_RTPROP: u32 = 8;
/// [TUR-2b D-1/4] Tombstone kayıt tavanı [CAL] — SR-AUDIT-001 bounded; en-eski-önce eviction +
/// audit. 4-decade doluş-doğrulaması not D-1/4 (0.25kbps ≈ 11 kayıt; ≲Gbps'te ~450 ≪ 4096).
const TOMBSTONE_MAX: usize = 4096;
/// §3.3-B geometrik kopya: ilk kopya = k_base × RTT_floor; RTT_floor ≈ (unit+sack)/floor_bw.
/// k_base = 3/2 (>1 → temiz kanalda ilk SACK kopyayı ateşlemeden iptal eder). [CAL PLACEHOLDER]
const PC_SACK_WIRE: u64 = 65;
const PC_KBASE_NUM: u64 = 3;
const PC_KBASE_DEN: u64 = 2;
const PC_COPY_MULTIPLIER: u64 = 2;
/// [R3/ADIM-6] Kanal-türevli burst penceresi [CAL]: burst_cap = clamp(rate·BURST_WINDOW, 1 birim, ~2×MTU).
/// Bant-kör 2×MTU=2400B sabitini (kbit-domain'i floodlayan) kanal-zamanı penceresine çevirir → sub-kbps'te
/// 1-birim (backlog-spike yok), hızlı kanalda micro-burst. ~500kbps'te 40ms ≈ 2×MTU (§13 "1-2×MTU").
const BURST_WINDOW_NS: u64 = 40_000_000;

/// Sender state machine (§3). Tüm bileşen state'ini tutar; `on_event` ile sürülür.
pub struct Sender {
    state: SenderState,

    // ── ölçüm (§5/§6/§7) ──
    loss: ThresholdLossDetector,
    rtt: RttEstimator,
    bw_sampler: DeliveryRateSampler,
    btlbw: BtlBwFilter,

    // ── karar (§8.2/§8.4/§8.5/§11) ──
    fuzzy: DeterministicFuzzyController,
    src_cfg: SrcConfig,
    budget_cfg: BudgetConfig,
    emergency: HysteresisController,
    emergency_cfg: EmergencyConfig,

    // ── icra (§9/§11/§13) ──
    pacer: TokenBucket,
    arq: DeadlineArqScheduler,

    // ── durum sayaçları ──
    next_seq: u64,
    inflight_bytes: u64,
    inflight: BTreeMap<u64, InflightPacket>,
    tx_queue: VecDeque<OpaqueUnit>,
    pending_lost: VecDeque<LostRec>, // §5 kayıp seq'ler → RECOVER_ARQ retransmit / RECOVER_FWD dup
    dup_queue: Vec<DupRec>,          // §11 zamanlanmış forward-recovery kopyaları
    last_now_ns: u64,

    // ── ack_confidence (RECOVER_FWD tetiği): sessizlik-tabanlı ──
    last_ack_ns: Option<u64>,
    first_send_ns: Option<u64>,

    // ── STARTUP plato tespiti ──
    startup_last_btlbw: u64,
    startup_stable_rounds: u32,
    /// [TUR-A ölçüm-#1, TEK DEĞİŞKEN] Son plato-değerlendirmesinden bu yana YENİ bw örneği geldi mi.
    /// Plato round'u yalnız yeni örnekte sayılır (pace-tick başına DEĞİL) — saat-sınıfı: event-count →
    /// bw-sample. (Eski: her on_event çağrısı "round" sayılıyordu → 1ms tick'te 3 round = 3ms'de STEADY.)
    bw_sampled_since_round: bool,

    // ── [TUR-A teşhis] loss-atıflama sayaçları (SALT-GÖZLEM) ──
    diag_loss_pkt: u64,
    diag_loss_time: u64,
    // ── [TUR-1x teşhis] ack_delay/RTT-guard sayaçları (SALT-GÖZLEM) ──
    diag_rtt_rejected: u64, // guard'a takılan sample sayısı (underflow / < MIN_RTT_FLOOR)
    diag_ack_delay_max_us: u64, // görülen en büyük (decoded) ack_delay — V2 sadakati kanıtı
    // ── [TUR-2a] adaptif reorder-toleransı + un-loss ──
    reorder: ReorderTracker, // §9.3 spurious→note_spurious kablosu; packet-thresh tüketimi
    retx_cfg: RetxConfig,    // reorder_thresh(ρ, K) parametreleri (arq ile aynı fixture)
    diag_unloss: u64,        // teşhis: geri-alınan (Spurious) pending-iptal sayısı

    // ── [TUR-2b] tombstone (geç-ACK eşleştirme; bounded, SR-AUDIT-001) ──
    tombstones: BTreeMap<u64, TombstoneRec>,
    tombstone_order: VecDeque<u64>, // ekleme sırası ≈ en-eski-TTL-önce (eviction kaynağı)
    tombstone_evicted: u64,         // audit: kapasite-eviction
    diag_tomb_hits: u64,            // geç-ACK tombstone'a çarptı (SALT-GÖZLEM)
    diag_bw_recovered: u64,         // kol-1'den kurtarılan BW-örneği sayısı
    diag_ambiguous_skipped: u64, // [TUR-2c resmî] kol-2 örnek-atlaması (sample_skipped_ambiguous)
    diag_delivered_credited: u64, // [TUR-2c] kol-2 delivered-kredisi (byte)
    diag_qd_no_evidence: u64,    // [TUR-3a D-3/1] rtprop-yokken qd_ratio=0 sayısı

    // ── gözlemlenebilir son kararlar ──
    last_decision: FusionDecision,
    last_recovery: RecoveryAction,
}

impl Sender {
    /// Fixture config ile (KALİBRE DEĞİL — golden/motor testi; gerçek değerler netem'den).
    #[must_use]
    pub fn new() -> Self {
        let budget_cfg = BudgetConfig::fixture();
        Self {
            state: SenderState::Idle,
            loss: ThresholdLossDetector::new(),
            rtt: RttEstimator::new(),
            bw_sampler: DeliveryRateSampler::new(),
            btlbw: BtlBwFilter::new(),
            fuzzy: DeterministicFuzzyController::fixture(),
            src_cfg: SrcConfig::default(),
            budget_cfg,
            emergency: HysteresisController::new(HysteresisConfig::fixture(), EmergencyLevel::Arq),
            emergency_cfg: EmergencyConfig::fixture(),
            pacer: TokenBucket::new(budget_cfg.min_rate_bps, DEFAULT_BURST_CAP),
            arq: DeadlineArqScheduler::new(RetxConfig::fixture()),
            next_seq: 0,
            inflight_bytes: 0,
            inflight: BTreeMap::new(),
            tx_queue: VecDeque::new(),
            pending_lost: VecDeque::new(),
            dup_queue: Vec::new(),
            last_now_ns: 0,
            last_ack_ns: None,
            first_send_ns: None,
            startup_last_btlbw: 0,
            startup_stable_rounds: 0,
            bw_sampled_since_round: false,
            diag_loss_pkt: 0,
            diag_loss_time: 0,
            diag_rtt_rejected: 0,
            diag_ack_delay_max_us: 0,
            reorder: ReorderTracker::new(),
            retx_cfg: RetxConfig::fixture(),
            diag_unloss: 0,
            tombstones: BTreeMap::new(),
            tombstone_order: VecDeque::new(),
            tombstone_evicted: 0,
            diag_tomb_hits: 0,
            diag_bw_recovered: 0,
            diag_ambiguous_skipped: 0,
            diag_delivered_credited: 0,
            diag_qd_no_evidence: 0,
            last_decision: FusionDecision {
                fec_parity_ratio_permille: 0,
                arq_aggressiveness_permille: 0,
                send_rate_gain_permille: 1000,
                compress: false,
                mode: Mode::Normal,
            },
            last_recovery: RecoveryAction::Arq,
        }
    }

    /// Ana döngü (§3): olay işle → FuzzyEvaluate → state → send_rate → UpdatePacing → ScheduleTransmit.
    pub fn on_event(&mut self, event: Event, now_ns: u64) -> Vec<TransmitDecision> {
        let dt = Duration::from_nanos(now_ns.saturating_sub(self.last_now_ns));
        self.pacer.refill(dt);
        self.last_now_ns = now_ns;

        match event {
            Event::AppData(unit) => {
                self.tx_queue.push_back(unit);
                if self.state == SenderState::Idle {
                    // §3.3 cold-start YALNIZ ilk sefer (hiç ölçüm yok). Ölçüm varsa (rtt.srtt Some) önsel
                    // zaten tahliye edilmiş → Idle→Startup (normal resume), PreConverge'e geri DÖNÜLMEZ.
                    self.state = if self.rtt.srtt().is_none() {
                        SenderState::PreConverge
                    } else {
                        SenderState::Startup
                    };
                }
            }
            Event::AckReceived(ack) => self.on_ack_received(&ack, now_ns),
            Event::TimerFired(timer) => self.on_timer(timer, now_ns),
        }

        let decision = self.fuzzy_evaluate(now_ns);
        self.last_decision = decision;
        let prev_state = self.state;
        self.update_state(&decision, now_ns);
        let send_rate = self.compute_send_rate(&decision);
        self.pacer.set_rate(send_rate);
        // [R3/ADIM-6] KANAL-TÜREVLİ burst cap: max(1 birim, rate·pencere), üstten ~2×MTU. Bant-kör 2400B
        // sabiti yerine → PreConverge floor'da 1-birim (self-congestion/backlog-spike yok), Startup+'ta
        // ölçülen bw'ye orantılı micro-burst. front yoksa MTU (güvenli 1-paket tabanı).
        let front_size = self.tx_queue.front().map_or(DEFAULT_MTU, |u| u.size);
        let window_bytes = send_rate.saturating_mul(BURST_WINDOW_NS) / 1_000_000_000;
        let burst_hi = front_size.max(2 * DEFAULT_MTU);
        self.pacer
            .set_burst_cap(window_bytes.clamp(front_size, burst_hi));
        if prev_state == SenderState::PreConverge && self.state != SenderState::PreConverge {
            // §3.3-D cold-start çıkışı: kanal-türevli micro-burst grant (STARTUP probing; kuyruk kanal-uygun
            // küçük burst'le boşalır — bu birimler STARTUP'ta emit → §3.3-B geometrik kopya ALMAZ).
            self.pacer.fill_to_cap();
        }

        self.schedule_transmit(now_ns)
    }

    /// §3.2 OnAckReceived: inflight boşalt + RTT/BW/loss güncelle.
    fn on_ack_received(&mut self, ack: &AckInfo, now_ns: u64) {
        if ack.acked_seqs.is_empty() {
            return;
        }
        self.last_ack_ns = Some(now_ns);

        // [TUR-3a D-2/1] Kanal-türevi estimator pencereleri — örnekler işlenmeden ÖNCE, bir
        // önceki srtt'yle yazılır (deterministik; 10s [STD] tabanı setter'larda).
        {
            let srtt_now = self.rtt.srtt().unwrap_or(INITIAL_RTT);
            let ack_epoch = srtt_now.max(self.expected_ack_interval());
            self.btlbw.set_window(ack_epoch * K_BW);
            self.rtt.set_rtprop_window(srtt_now * K_RTPROP);
        }

        // RTT: en yüksek yeni-acked ORİJİNAL (DATA) paketten örnekle (Karn: retransmit/dup'tan DEĞİL).
        let mut rtt_send_ns: Option<u64> = None;
        let mut largest = 0u64;
        let mut acked_units: Vec<u64> = Vec::new();
        for &seq in &ack.acked_seqs {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                acked_units.push(p.unit.id); // §3.3-B stop koşulu: teyit edilen birimin kopyaları iptal
                let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample);
                self.btlbw.update(sample, now_ns, sample.delivered > 0);
                self.bw_sampled_since_round = true; // [TUR-A ölçüm-#1] plato round'u bw-örneğiyle sayılır
                if p.origin == TransmitKind::Data && (rtt_send_ns.is_none() || seq >= largest) {
                    largest = seq;
                    rtt_send_ns = Some(p.send_ns);
                }
            } else if let Some(t) = self.tombstones.remove(&seq) {
                // [TUR-2b D-1/4] GEÇ-ACK tombstone'a çarptı: kayıt korunmuştu → örnekler
                // KURTARILIR (eski davranış: kayıt düşmüştü → sansür). Süresi geçmiş kayıt
                // prune edilmişse buraya düşmez (Expired/Unknown — örneksiz).
                self.diag_tomb_hits += 1;
                acked_units.push(t.unit.id); // teslim teyidi → bekleyen kopyaları iptal et
                if t.retx_count == 0 {
                    // kol-1 (tek gönderim, belirsizlik yok): BW-örneği NORMAL üretilir.
                    let sample = self.bw_sampler.on_ack(now_ns, t.bw_sample);
                    self.btlbw.update(sample, now_ns, sample.delivered > 0);
                    self.bw_sampled_since_round = true;
                    self.diag_bw_recovered += 1;
                    if t.origin == TransmitKind::Data && (rtt_send_ns.is_none() || seq >= largest) {
                        largest = seq;
                        rtt_send_ns = Some(t.send_ns);
                    }
                } else {
                    // [TUR-2c D-1/3 kol-2] Superseded iletimin geç-ACK'i: RTT YOK + rate-örneği
                    // YOK (muhafazakâr Karn); ama DELIVERED-KREDİSİ VAR — byte'lar wire'ı
                    // gerçekten geçti, sampler kümülatifine girer (gelecek örnek-delta'larına
                    // dürüst yansır). Not: per-transmission seq tasarımında klasik belirsizlik
                    // zayıftır (seq iletimi tekil tanımlar); kural yine muhafazakâr tutuldu.
                    self.bw_sampler.credit_delivered(now_ns, t.unit.size);
                    self.diag_delivered_credited += t.unit.size;
                    self.diag_ambiguous_skipped += 1;
                    // [TUR-2c review-fix, §9.3] KANONİK spurious-retransmit teyidi tam bu vaka
                    // ("retransmit'ten SONRA orijinal ACK geldi") → reorder-toleransı yükselir.
                    // Un-loss yolu (2a) yalnız retx-ÖNCESİ geç-ACK'i yakalıyordu; kol-2 kabloyu
                    // tamamlar. Ledger zaten Spurious'a çevirir (lppm dürüst).
                    let la = self.loss.largest_acked().unwrap_or(seq);
                    self.reorder
                        .note_spurious(u32::try_from(la.saturating_sub(seq)).unwrap_or(u32::MAX));
                }
            }
        }
        // loss tespiti (§5). loss = RECOVERY sinyali (INV-RATE-1: rate'e DEĞMEZ).
        // §3.3-C/(1) yasak-1: PRE_CONVERGE'de loss İLANI YOK. §3.3-D: loss kilidi İLK GEÇERLİ SACK'le
        // (rtt.srtt Some) AÇILIR → o SACK'ten itibaren loss işlenir. Detector her zaman sync kalır.
        // [TUR-2a §9.3] adaptif packet-threshold: spurious gördükçe reorder-toleransı yükselir.
        self.loss
            .set_packet_threshold(u64::from(self.reorder.reorder_thresh(&self.retx_cfg)));
        let lost = self.loss.on_ack(&ack.acked_seqs, now_ns);

        // [TUR-2a D-1/3] UN-LOSS: kayıp-ilan-edilmiş seq'e ACK geldi → hipotez çürüdü.
        // pending-retransmit İPTAL + kopyalar iptal + note_spurious kablosu + kol-1 RTT adayı.
        let unlossed = self.loss.take_unlossed();
        let mut unloss_rtt: Option<(u64, u64)> = None; // (seq, send_ns)
        if !unlossed.is_empty() {
            // Gözlenen-reorder mesafesi görülen EN YÜKSEK acked'e göredir (bu geç-ACK'in
            // kendi max'ı değil — kayıp-şüphesini yaratan önceki yüksek ACK'ler dahil).
            let la = self.loss.largest_acked().unwrap_or(0);
            let mut kept = VecDeque::with_capacity(self.pending_lost.len());
            for rec in self.pending_lost.drain(..) {
                if !unlossed.contains(&rec.seq) {
                    kept.push_back(rec);
                    continue;
                }
                self.diag_unloss += 1;
                acked_units.push(rec.unit.id); // kopyaları aşağıdaki ortak iptale kat
                                               // §9.3: gözlenen reorder mesafesiyle toleransı yükselt (küçülmez).
                self.reorder
                    .note_spurious(u32::try_from(la.saturating_sub(rec.seq)).unwrap_or(u32::MAX));
                // kol-1: retransmit HENÜZ gönderilmedi → orijinal RTT örneği GEÇERLİ.
                if rec.retx_count == 0 && unloss_rtt.is_none_or(|(s, _)| rec.seq >= s) {
                    unloss_rtt = Some((rec.seq, rec.send_ns));
                }
            }
            self.pending_lost = kept;
        }
        // RTT adayı birleşimi: en yüksek yeni-acked orijinal gönderim (inflight ∪ un-loss kol-1).
        if let Some((useq, usend)) = unloss_rtt {
            if rtt_send_ns.is_none() || useq >= largest {
                rtt_send_ns = Some(usend);
            }
        }

        if let Some(send_ns) = rtt_send_ns {
            let raw = Duration::from_nanos(now_ns.saturating_sub(send_ns));
            self.diag_ack_delay_max_us = self
                .diag_ack_delay_max_us
                .max(ack.ack_delay.as_micros() as u64);
            // [TUR-1x] min-rtt guard (RFC 9002 §5.3): düzeltme güvensizse RAW kullanılır —
            // örnek ATLANMAZ (sample-skip = estimator açlığı, K-1 sınıfı).
            let (sample, corrected) = apply_ack_delay(raw, ack.ack_delay, self.rtt.rtprop());
            if !corrected {
                self.diag_rtt_rejected += 1; // §7.3 ack_delay_untrusted (düzeltme reddi; raw işlendi)
            }
            self.rtt.observe(now_ns, sample);
        }

        // §3.3-B stop koşulu: teyit edilen birimlerin bekleyen geometrik kopyalarını İPTAL et.
        if !acked_units.is_empty() {
            self.dup_queue.retain(|d| !acked_units.contains(&d.unit.id));
        }

        if self.state != SenderState::PreConverge || self.rtt.srtt().is_some() {
            let before = self.pending_lost.len();
            self.queue_losses(&lost, now_ns);
            // [TUR-A teşhis] loss-atıflama: packet-threshold (§5a) kaynaklı kuyruklanan kayıp sayısı.
            self.diag_loss_pkt += (self.pending_lost.len() - before) as u64;
        }
    }

    /// §12 timer handler (RTprop probe → DRAIN_RTPROP; PTO → zaman-eşiği loss).
    fn on_timer(&mut self, timer: Timer, now_ns: u64) {
        match timer {
            Timer::RtpropProbe => {
                if self.state == SenderState::Steady {
                    self.state = SenderState::DrainRtprop;
                }
            }
            Timer::Pto => {
                // §3.3-C/(1,5) yasak: PRE_CONVERGE'de time-threshold loss-ilanı + PTO DONDURULUR
                // (srtt BOŞ → tanımsız; geometrik kopya §3.3-B yerini alır). schedule_transmit yine çalışır.
                if self.state != SenderState::PreConverge {
                    let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
                    let lost =
                        self.loss
                            .detect_time_losses(now_ns, srtt, self.cadence_loss_floor());
                    let before = self.pending_lost.len();
                    self.queue_losses(&lost, now_ns);
                    // [TUR-A teşhis] loss-atıflama: time-threshold (§5b) kaynaklı kuyruklanan kayıp.
                    self.diag_loss_time += (self.pending_lost.len() - before) as u64;
                }
                // [TUR-2b] tombstone TTL budaması: süresi dolan kayıt düşer — sonraki geç-ACK'i
                // Expired/Unknown (örneksiz). Ekleme-sırası ≈ TTL-sırası (T_loss yavaş değişir);
                // front'tan tarama deterministik + bounded.
                while let Some(&front) = self.tombstone_order.front() {
                    match self.tombstones.get(&front) {
                        None => {
                            self.tombstone_order.pop_front(); // hit/evict ile zaten düşmüş
                        }
                        Some(t) if t.expiry_ns <= now_ns => {
                            self.tombstones.remove(&front);
                            self.tombstone_order.pop_front();
                            // [commit-B §3.7] artık ACK'lenmeyecek → uçuş-sayısından düş.
                            self.bw_sampler.forget_in_flight();
                        }
                        Some(_) => break,
                    }
                }
            }
        }
    }

    /// [TUR-1b D-1/2] Cadence-farkında loss-eşiği TABANI = K_c × beklenen-ACK-aralığı (EAI).
    /// `EAI = ACK_FREQ_N × ort_birim / send_rate_fiili + sack_dönüş_serialization(önsel)`.
    /// Sender-yerel + deterministik (ADR-17). Saat-sınıfı (ZM-3): kanal-türevi (send_rate,
    /// inflight-ortalama birim) + worst-corner return-önseli [CAL — §3.3-E-FULL'de ölçülen
    /// return_bw ile değişir). Amaç (ZM-4): elapsed'in içindeki "SACK'i henüz gelmedi" süresi
    /// eşiğin saatinde TEMSİL edilsin — kanıtlı-ama-cadence yanlış-sınıflandırması yapısal biter.
    fn cadence_loss_floor(&self) -> Duration {
        Duration::from_nanos(dur_ns(self.expected_ack_interval()) * CADENCE_KC_NUM / CADENCE_KC_DEN)
    }

    /// [TUR-1b/2a] Beklenen-ACK-aralığı EAI (K_c'siz ham değer) — cadence-taban ve
    /// `ack_epoch = max(srtt, EAI)` (ledger W_loss penceresi, D-2/1 tanımı) bundan türer.
    fn expected_ack_interval(&self) -> Duration {
        let unit = if self.inflight.is_empty() {
            DEFAULT_MTU
        } else {
            (self.inflight_bytes / self.inflight.len() as u64).max(1)
        };
        let rate = self.pacer.rate().max(1);
        let eai_ns = EXPECTED_ACK_FREQ_N * unit * 1_000_000_000 / rate
            + EAI_SACK_WIRE_BYTES * 1_000_000_000 / EAI_RETURN_BW_PRIOR;
        Duration::from_nanos(eai_ns)
    }

    /// Kayıp seq'leri recovery kuyruğuna al. Duplicate-kökenli kayıp RE-QUEUE EDİLMEZ (redundancy'di).
    /// [TUR-2b D-1/4] ACTIVE-FLIGHT / TOMBSTONE ayrımı: inflight-muhasebesinden DÜŞER (gönderim
    /// durmaz) ama gönderim-kaydı SİLİNMEZ — geç-ACK eşleştirme için tombstone'a taşınır
    /// (bw_sample DAHİL → kol-1'de BW-örneği kurtarılabilir; eski davranış kaydı düşürüp
    /// geç-ACK örneklerini SANSÜRLÜYORDU — K-1 eviction-sansürü sınıfı).
    fn queue_losses(&mut self, lost: &[u64], now_ns: u64) {
        if lost.is_empty() {
            return;
        }
        // TTL = K_TOMB × T_loss(ilan-anı); T_loss = max(9/8·srtt, cadence-taban) — eşikle AYNI saat.
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let t_loss_ns = (dur_ns(srtt) * 9 / 8).max(dur_ns(self.cadence_loss_floor()));
        let expiry_ns = now_ns.saturating_add(t_loss_ns.saturating_mul(u64::from(K_TOMB)));
        for &seq in lost {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                if p.origin == TransmitKind::Duplicate {
                    // [commit-B §3.7] Dup-kökenli kayıp tombstonesuz düşer → uçuş-sayısı burada.
                    self.bw_sampler.forget_in_flight();
                }
                if p.origin != TransmitKind::Duplicate {
                    self.pending_lost.push_back(LostRec {
                        seq,
                        unit: p.unit,
                        deadline_ns: now_ns,
                        retx_count: p.retx_count,
                        send_ns: p.send_ns, // [TUR-2a] un-loss kol-1 RTT için orijinal damga
                    });
                    // [TUR-2b] tombstone: bounded, en-eski-önce eviction + audit.
                    while self.tombstones.len() >= TOMBSTONE_MAX {
                        match self.tombstone_order.pop_front() {
                            Some(old) => {
                                if self.tombstones.remove(&old).is_some() {
                                    self.tombstone_evicted += 1; // geç-ACK'i artık Expired/Unknown
                                    self.bw_sampler.forget_in_flight(); // [commit-B §3.7]
                                }
                            }
                            None => break,
                        }
                    }
                    self.tombstone_order.push_back(seq);
                    self.tombstones.insert(
                        seq,
                        TombstoneRec {
                            unit: p.unit,
                            send_ns: p.send_ns,
                            bw_sample: p.bw_sample,
                            retx_count: p.retx_count,
                            origin: p.origin,
                            expiry_ns,
                        },
                    );
                }
            }
        }
    }

    /// FuzzyEvaluate (§8.4/§15): ölçümlerden FusionInputs kur → controller kararı.
    /// [TUR-2a D-1/5] loss girdisi LEDGER-penceresinden türer (lifetime-ratchet ÖLDÜ):
    /// önce ledger_tick (W_loss = K_wl×ack_epoch; confirm = cadence-taban = geç-SACK ufku),
    /// sonra `loss_windowed_ppm` — Spurious sayılmaz, pencere-dışı decay eder.
    fn fuzzy_evaluate(&mut self, now_ns: u64) -> FusionDecision {
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let ack_epoch = srtt.max(self.expected_ack_interval()); // D-2/1 tanımı
        self.loss
            .ledger_tick(now_ns, ack_epoch * K_WL, self.cadence_loss_floor());
        let jitter_ppm = dur_ns(self.rtt.rttvar())
            .saturating_mul(1_000_000)
            .checked_div(dur_ns(srtt))
            .unwrap_or(0);
        // [TUR-3a önalım, D-3/1] qd_ratio = qd×1000/rtprop; rtprop yoksa 0 (kanıt yok →
        // boğma yok) + sayaç. Dürüst-rtprop'un zorunlu ikizi: mutlak-ns eşikler sub-kbps'te
        // fiziksel-taban qd'yi (serialization çift-yönü, saniyeler) kalıcı-HIGH okuyordu.
        let qd_ratio_permille = match self.rtt.rtprop() {
            Some(rtp) if !rtp.is_zero() => {
                dur_ns(self.rtt.queue_delay()).saturating_mul(1000) / dur_ns(rtp)
            }
            _ => {
                self.diag_qd_no_evidence += 1;
                0
            }
        };
        let inputs = FusionInputs {
            loss_ppm: self.loss.loss_windowed_ppm(),
            bandwidth_bps: self.btlbw.btlbw(),
            jitter_ppm,
            qd_ratio_permille,
            ack_confidence_permille: self.ack_confidence_permille(now_ns),
        };
        self.fuzzy.decide(&inputs)
    }

    /// §3 state geçişleri. Öncelik: IDLE (iş yok) → RECOVER_FWD (ack_confidence LOW, her fazdan) →
    /// faz-içi (STARTUP plato / RECOVER_ARQ / STEADY).
    fn update_state(&mut self, decision: &FusionDecision, now_ns: u64) {
        let health = u32::try_from(self.ack_confidence_permille(now_ns)).unwrap_or(u32::MAX);
        let level = self.emergency.update(health, now_ns);
        self.last_recovery = emergency::action(level, Framing::Tiny, &self.emergency_cfg);

        let idle = self.tx_queue.is_empty()
            && self.inflight.is_empty()
            && self.pending_lost.is_empty()
            && self.dup_queue.is_empty();
        let emergency_active =
            level > EmergencyLevel::Arq || decision.mode == Mode::EmergencyForwardRedundancy;

        self.state = if idle {
            SenderState::Idle
        } else if self.state == SenderState::PreConverge {
            // §3.3-D kademeli tahliye: ilk geçerli SACK (RTT kilidi açıldı) → Startup; yoksa PreConverge.
            // emergency/ARQ/loss burada BYPASS edilir — recovery cold-start'ta DONDURULMUŞ (§3.3-C).
            if self.rtt.srtt().is_some() {
                SenderState::Startup
            } else {
                SenderState::PreConverge
            }
        } else if emergency_active {
            // ack_confidence LOW → forward-recovery (ACK-bağımsız, §11), her aktif fazdan (§3 AMENDMENT-1).
            SenderState::RecoverFwd
        } else if !self.pending_lost.is_empty() {
            // residual loss + ack_confidence OK → selective retransmit (§9), her aktif fazdan
            // (§3 AMENDMENT-1; RECOVER_FWD ile simetrik: loss STARTUP'ta olsa da recovery tıkanmamalı).
            SenderState::RecoverArq
        } else {
            match self.state {
                SenderState::Idle => SenderState::Startup,
                SenderState::Startup => {
                    if self.startup_plateau() {
                        SenderState::Steady
                    } else {
                        SenderState::Startup
                    }
                }
                _ => SenderState::Steady,
            }
        };
    }

    /// STARTUP plato: BtlBw N round Δ<ε ise stabil (§3 STARTUP→STEADY).
    ///
    /// [TUR-A ölçüm-#1, TEK DEĞİŞKEN] Round = YENİ bw-örneği (ACK'ten). Eski kod her on_event
    /// çağrısını round sayıyordu → 1ms pace-tick'te 3 "round" = STARTUP 3ms'de biter, bant hiç
    /// prob edilmez → motor kendi app-limited hızına kilitlenir (TUR-A trajektorisi, kök).
    fn startup_plateau(&mut self) -> bool {
        if !self.bw_sampled_since_round {
            return false; // yeni ölçüm yok → round İLERLEMEZ (pace-tick zaman-vekili DEĞİL)
        }
        self.bw_sampled_since_round = false;
        let bw = self.btlbw.btlbw();
        if bw == 0 {
            return false;
        }
        let grew = bw
            .saturating_sub(self.startup_last_btlbw)
            .saturating_mul(1000)
            > self
                .startup_last_btlbw
                .saturating_mul(STARTUP_PLATEAU_EPS_PERMILLE);
        if grew {
            self.startup_stable_rounds = 0;
        } else {
            self.startup_stable_rounds += 1;
        }
        self.startup_last_btlbw = bw;
        self.startup_stable_rounds >= STARTUP_PLATEAU_ROUNDS
    }

    /// send_rate (§8.2 + §8.5): SRC pacing × fuzzy gain, sonra budget clamp. INV-RATE-1: recovery
    /// durumları base rate'i (Steady) DÜŞÜRMEZ; fuzzy send_rate_gain yalnız bw+queue_delay'den.
    fn compute_send_rate(&self, decision: &FusionDecision) -> u64 {
        // §3.3-A/[R8]: PRE_CONVERGE en-kötü-köşe önsel pacing = floor. min_rate fixture (Ek-B)
        // UYGULANMAZ → 1 kbps kanalı floodlamaz (self-congestion). İlk ölçümle Startup'ta ramp başlar.
        if self.state == SenderState::PreConverge {
            return PRE_CONVERGE_FLOOR_BW;
        }
        let phase = match self.state {
            SenderState::Startup => PacingPhase::Startup,
            SenderState::DrainRtprop => PacingPhase::ProbeDown,
            _ => PacingPhase::Steady,
        };
        let btlbw = self.btlbw.btlbw();
        let rtprop = self.rtt.rtprop().unwrap_or(INITIAL_RTT);
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let fuzzy_rate = effective_send_rate(
            &self.src_cfg,
            btlbw,
            rtprop,
            srtt,
            phase.gain(),
            decision.send_rate_gain_permille,
        );
        budget::send_rate_final(fuzzy_rate, self.budget_cfg.min_rate_bps, u64::MAX)
    }

    /// ScheduleTransmit (§3): pacing bütçesinde emit — sıra §3'e göre **DATA ÖNCE**, sonra recovery.
    /// Recovery-obsesif "veri ULAŞSIN" AGGREGATE'te en çok teslim demektir: her birim en az bir kez
    /// denensin (completion-optimal) → sonra kaybolanlar artakalan bütçeyle kurtarılsın. Recovery-first
    /// birkaç eski birimi ısrarla retransmit ederken YENİ birimleri aç bırakırdı (loopback bottleneck-yok
    /// flood'unda birimlerin yarısı hiç gönderilmez; rate-limited path'te de daha az distinct teslim).
    /// Hepsi tek pacer'ı (token bucket) paylaşır (§9/§13): recovery fırtınası rate'i patlatamaz.
    fn schedule_transmit(&mut self, now_ns: u64) -> Vec<TransmitDecision> {
        let mut out = Vec::new();
        self.drain_new_data(&mut out, now_ns);
        match self.state {
            SenderState::RecoverArq => self.plan_arq_retransmits(&mut out, now_ns),
            SenderState::RecoverFwd => self.plan_forward_duplication(now_ns),
            _ => {}
        }
        self.drain_due_duplicates(&mut out, now_ns);
        out
    }

    /// RECOVER_ARQ (§9): ACK-güdümlü selective retransmit. arq_aggressiveness=0 → baskıla (fec-dominant).
    fn plan_arq_retransmits(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        let lost: Vec<LostSeq> = self
            .pending_lost
            .iter()
            .map(|r| LostSeq {
                seq: r.seq,
                deadline_ns: r.deadline_ns,
                retx_count: r.retx_count,
            })
            .collect();
        let plan = self
            .arq
            .plan(&lost, self.last_decision.arq_aggressiveness_permille);
        for dec in plan {
            match dec {
                RetxDecision::Retransmit { seq } => {
                    let Some(pos) = self.pending_lost.iter().position(|r| r.seq == seq) else {
                        continue;
                    };
                    let rec = self.pending_lost[pos];
                    if !self.pacer.try_send(rec.unit.size) {
                        break; // token yok → sonraki tick'te (deadline plandan düşmez, §9).
                    }
                    self.pending_lost.remove(pos);
                    // [TUR-2c D-1/3 kol-2] Orijinalin tombstone'u SUPERSEDED: retx-sonrası-
                    // geç-orijinal-ACK kol-1'e düşemez (2b'de kayıt ilan-anında donuyordu).
                    self.mark_tombstone_superseded(seq);
                    let new_seq = self.track_sent(
                        rec.unit,
                        now_ns,
                        rec.retx_count + 1,
                        TransmitKind::Retransmit,
                    );
                    out.push(TransmitDecision {
                        kind: TransmitKind::Retransmit,
                        seq: new_seq,
                        unit_id: rec.unit.id,
                        size: rec.unit.size,
                    });
                }
                RetxDecision::CapExceeded { seq, policy } => {
                    if let Some(pos) = self.pending_lost.iter().position(|r| r.seq == seq) {
                        let rec = self.pending_lost.remove(pos).expect("pos geçerli");
                        if policy == CapExceedPolicy::HandToForward {
                            // ARQ cap doldu → forward-recovery'ye devret (§9.1 "veri ulaşsın").
                            // [TUR-2c review-fix] devir de YENİDEN-EMİSYONDUR → superseded-işaret
                            // (örnekleme politikası recovery-yoluna bağımlı olamaz — tekdüze kural).
                            self.mark_tombstone_superseded(rec.seq);
                            self.dup_queue.push(DupRec {
                                due_ns: now_ns,
                                unit: rec.unit,
                                interval_ns: 0, // emergency: fire-once
                            });
                        }
                        // ConnectionFail: spine'da düşür; gerçek impl bağlantı-fail sinyali verir.
                    }
                }
            }
        }
    }

    /// RECOVER_FWD (§11): ACK-BAĞIMSIZ forward duplication. Kayıp birimleri `factor` kez zaman-aralıklı
    /// kopya olarak planla (ACK beklemeden — reverse-path çöktüğü için ARQ futile).
    ///
    /// [TUR-1a / D-1/1 liveness kolu] pending_lost BOŞKEN (saf-sessizlik: later-ACK preconditionı loss
    /// SINIFLANDIRMASINI blokladı ama tail cevapsız): unacked orijinal inflight birimlerine spekülatif
    /// geometrik kopya (taban=srtt, ×2 backoff; ACK'le iptal §3.3-B-stop). Loss METRİĞİNE dokunmaz —
    /// "saf sessizlik loss'u artırmaz; fakat liveness amacıyla probe/retransmit tetikleyebilir" (§5.1).
    fn plan_forward_duplication(&mut self, now_ns: u64) {
        if self.pending_lost.is_empty() {
            // Saf-sessizlik tail-liveness: kopyası zaten planlı olmayan orijinal-DATA inflight birimleri.
            let srtt_ns = dur_ns(self.rtt.srtt().unwrap_or(INITIAL_RTT));
            let tail: Vec<OpaqueUnit> = self
                .inflight
                .values()
                .filter(|p| p.origin == TransmitKind::Data)
                .map(|p| p.unit)
                .filter(|u| !self.dup_queue.iter().any(|d| d.unit.id == u.id))
                .collect();
            for unit in tail {
                self.dup_queue.push(DupRec {
                    due_ns: now_ns,
                    unit,
                    interval_ns: srtt_ns.max(1), // geometrik reschedule (drain'de ×COPY_MULTIPLIER)
                });
            }
            return;
        }
        let (factor, spacing) = self.dup_params();
        let recs: Vec<LostRec> = self.pending_lost.drain(..).collect();
        for rec in recs {
            // [TUR-2c review-fix] FWD-kopya da YENİDEN-EMİSYONDUR: orijinalin tombstone'u
            // superseded — kol-1/kol-2 seçimi recovery-state'e göre DEĞİŞEMEZ (tekdüze
            // muhafazakâr kural; ARQ-koluyla simetri).
            self.mark_tombstone_superseded(rec.seq);
            for i in 0..u64::from(factor) {
                self.dup_queue.push(DupRec {
                    due_ns: now_ns + i.saturating_mul(dur_ns(spacing)),
                    unit: rec.unit,
                    interval_ns: 0, // emergency: fire-once, reschedule YOK
                });
            }
        }
    }

    /// [TUR-2c] Orijinal iletimin tombstone'unu SUPERSEDED işaretle (retx_count++): birimin
    /// HERHANGİ bir yeniden-emisyonundan (ARQ retransmit / FWD dup / HandToForward) sonra
    /// orijinalin geç-ACK'i kol-1'e (RTT + rate-örneği) düşemez — tekdüze muhafazakâr kural.
    fn mark_tombstone_superseded(&mut self, seq: u64) {
        if let Some(t) = self.tombstones.get_mut(&seq) {
            t.retx_count = t.retx_count.saturating_add(1);
        }
    }

    /// Vadesi gelen duplicate kopyalarını pace ederek emit et (§11 spacing).
    fn drain_due_duplicates(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        let mut i = 0;
        while i < self.dup_queue.len() {
            if self.dup_queue[i].due_ns > now_ns {
                i += 1;
                continue; // henüz vadesi gelmedi.
            }
            if !self.pacer.try_send(self.dup_queue[i].unit.size) {
                break; // token yok.
            }
            let dup = self.dup_queue.remove(i);
            let seq = self.track_sent(dup.unit, now_ns, 0, TransmitKind::Duplicate);
            out.push(TransmitDecision {
                kind: TransmitKind::Duplicate,
                seq,
                unit_id: dup.unit.id,
                size: dup.unit.size,
            });
            // §3.3-B geometrik reschedule: interval_ns>0 (PRE_CONVERGE kopyası) → sonraki kopya
            // ×COPY_MULTIPLIER (sayı-tavanı YOK, deadline'a dek; teyit gelince §3.3-B stop ile iptal).
            if dup.interval_ns > 0 {
                let next_interval = dup.interval_ns.saturating_mul(PC_COPY_MULTIPLIER);
                self.dup_queue.push(DupRec {
                    due_ns: now_ns.saturating_add(next_interval),
                    unit: dup.unit,
                    interval_ns: next_interval,
                });
            }
        }
    }

    /// Yeni DATA'yı pace ederek emit et. RECOVER_FWD'de her DATA ayrıca (factor−1) kopya planlar.
    fn drain_new_data(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        while let Some(front) = self.tx_queue.front().copied() {
            // [R3/ADIM-6] Burst clamp artık pipeline'da kanal-türevli set_burst_cap ile (PreConverge floor
            // rate → burst_cap = 1 birim). Ayrı PreConverge özel-durumu GEREKMEZ (tek kapı, tüm state'ler).
            if !self.pacer.try_send(front.size) {
                break;
            }
            self.tx_queue.pop_front();
            let seq = self.track_sent(front, now_ns, 0, TransmitKind::Data);
            out.push(TransmitDecision {
                kind: TransmitKind::Data,
                seq,
                unit_id: front.id,
                size: front.size,
            });
            if self.state == SenderState::RecoverFwd {
                let (factor, spacing) = self.dup_params();
                for i in 1..u64::from(factor) {
                    self.dup_queue.push(DupRec {
                        due_ns: now_ns + i.saturating_mul(dur_ns(spacing)),
                        unit: front,
                        interval_ns: 0, // emergency: fire-once
                    });
                }
            } else if self.state == SenderState::PreConverge {
                // §3.3-B geometrik kopya: ilk kopya k_base×RTT_floor sonra; ateşlenince ×COPY_MULTIPLIER.
                // RTT_floor ≈ (unit+sack)/floor_bw. Temiz kanalda ilk SACK bunu ateşlemeden iptal eder.
                // [K-D2] t₀ = wire-emit anı = buradaki `now_ns` (birim pacer.try_send'i geçti = WIRE'a çıktı),
                // sunum/enqueue anı DEĞİL → burst kuyruğunun serialization gecikmesi otomatik hesaba girer.
                let rtt_floor = (front.size + PC_SACK_WIRE)
                    .saturating_mul(1_000_000_000)
                    .checked_div(PRE_CONVERGE_FLOOR_BW)
                    .unwrap_or(0);
                let first_delay = rtt_floor.saturating_mul(PC_KBASE_NUM) / PC_KBASE_DEN;
                self.dup_queue.push(DupRec {
                    due_ns: now_ns.saturating_add(first_delay),
                    unit: front,
                    interval_ns: first_delay, // reschedule aralığı (drain'de ×COPY_MULTIPLIER)
                });
            }
        }
    }

    /// last_recovery'den (factor, spacing); Duplication değilse varsayılan (§11 Tiny framing).
    fn dup_params(&self) -> (u8, Duration) {
        match self.last_recovery {
            RecoveryAction::Duplication { factor, spacing } => (factor, spacing),
            _ => (DEFAULT_DUP_FACTOR, DEFAULT_DUP_SPACING),
        }
    }

    /// §3.1 OnPacketSent muhasebe: seq ata, inflight+RTT/BW/loss kayıt. `origin` recovery türünü ayırır
    /// (DATA = orijinal birim → loss sayacı +1; retransmit/dup = recovery → +0, RTT'de Karn dışı).
    fn track_sent(
        &mut self,
        unit: OpaqueUnit,
        now_ns: u64,
        retx_count: u8,
        origin: TransmitKind,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        if self.first_send_ns.is_none() {
            self.first_send_ns = Some(now_ns);
        }
        let more_pending = !self.tx_queue.is_empty()
            || !self.pending_lost.is_empty()
            || !self.dup_queue.is_empty();
        let bw_sample = self.bw_sampler.on_sent(now_ns, unit.size, !more_pending);
        let units = u64::from(origin == TransmitKind::Data); // yalnız orijinal DATA loss sayacına.
        self.loss.on_sent(seq, now_ns, units);
        self.inflight_bytes += unit.size;
        self.inflight.insert(
            seq,
            InflightPacket {
                unit,
                send_ns: now_ns,
                bw_sample,
                retx_count,
                origin,
            },
        );
        seq
    }

    /// ack_confidence (per-mille, §5.1): reverse-path canlılığı = ACK SESSİZLİĞİ. Aktif iş varken son
    /// ACK'ten (ya da ilk gönderimden) geçen süre srtt katını aşarsa güven düşer. Yeni paket "düşük
    /// güven" DEĞİL — yalnız SESSİZLİK reverse-path çöküşüdür.
    fn ack_confidence_permille(&self, now_ns: u64) -> u64 {
        // §3.3-C/(4) yasak-4: PRE_CONVERGE'de ack_confidence FULL sabit — sessizlik cold-start'ta
        // reverse-path çöküşü DEĞİL taban-bw serialization gecikmesi (threshold≈300ms ≪ RTT_floor).
        if self.state == SenderState::PreConverge {
            return 1000;
        }
        let active = !self.inflight.is_empty()
            || !self.pending_lost.is_empty()
            || !self.tx_queue.is_empty()
            || !self.dup_queue.is_empty();
        if !active {
            return 1000;
        }
        let anchor = match self.last_ack_ns.or(self.first_send_ns) {
            Some(a) => a,
            None => return 1000,
        };
        let silence = now_ns.saturating_sub(anchor);
        let srtt = dur_ns(self.rtt.srtt().unwrap_or(INITIAL_RTT));
        let threshold = srtt.saturating_mul(u64::from(ACK_SILENCE_SRTT_MULT));
        if silence <= threshold {
            return 1000;
        }
        threshold
            .saturating_mul(1000)
            .checked_div(silence)
            .unwrap_or(0)
            .min(1000)
    }

    // ── gözlemciler (test + entegrasyon) ──

    /// Güncel state.
    #[must_use]
    pub fn state(&self) -> SenderState {
        self.state
    }
    /// inflight bytes.
    #[must_use]
    pub fn inflight_bytes(&self) -> u64 {
        self.inflight_bytes
    }
    /// Son füzyon kararı.
    #[must_use]
    pub fn last_decision(&self) -> FusionDecision {
        self.last_decision
    }
    /// TUR-A teşhis rate-trajektori snapshot'ı (SALT-GÖZLEM; davranış DEĞİŞMEZ):
    /// `(state, btlbw_bps, srtt_us, send_rate_bps, inflight_bytes)`. floor→btlbw ramp kilidini ölçmek için.
    #[must_use]
    pub fn diag_rate(&self) -> (SenderState, u64, u64, u64, u64) {
        let srtt_us = self
            .rtt
            .srtt()
            .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
        (
            self.state,
            self.btlbw.btlbw(),
            srtt_us,
            self.pacer.rate(),
            self.inflight_bytes,
        )
    }
    /// [TUR-A teşhis, ölçüm-#2] loss-atıflama (SALT-GÖZLEM): `(packet_threshold_queued,
    /// time_threshold_queued, pending_lost_now)`. Temiz kanalda hangi tetik spurious RecoverArq açıyor —
    /// gerçek gap (packet) mi, cadence-gap (time) mi.
    #[must_use]
    pub fn diag_loss(&self) -> (u64, u64, u64) {
        (
            self.diag_loss_pkt,
            self.diag_loss_time,
            self.pending_lost.len() as u64,
        )
    }
    /// [TUR-1x teşhis] ack_delay/RTT-guard (SALT-GÖZLEM): `(rejected_samples, max_decoded_ack_delay_us)`.
    /// V2 kodlama-sadakati kanıtı: max_us cadence-hold mertebesine (saniyeler) çıkabilmeli (eski µs-birim
    /// tavanı 65535'te kelepçeliydi); rejected temiz kanalda ≈0 beklenir.
    #[must_use]
    pub fn diag_rtt(&self) -> (u64, u64) {
        (self.diag_rtt_rejected, self.diag_ack_delay_max_us)
    }
    /// [TUR-2a teşhis] ledger/un-loss (SALT-GÖZLEM): `(unloss_iptal, loss_windowed_ppm,
    /// spurious_units_total)` — ratchet-ölümü + geri-alma hacmi kanıtı.
    #[must_use]
    pub fn diag_ledger(&self) -> (u64, u64, u64) {
        (
            self.diag_unloss,
            self.loss.loss_windowed_ppm(),
            self.loss.spurious_units_total(),
        )
    }
    /// [TUR-3a teşhis] sampler uçuş-sayısı (SALT-GÖZLEM — §3.7 sızıntı-borcu ölçümü).
    #[must_use]
    pub fn diag_pif(&self) -> u64 {
        self.bw_sampler.packets_in_flight()
    }
    /// [TUR-3a teşhis] rtprop + queue_delay (SALT-GÖZLEM — qd-atıflaması, µs).
    #[must_use]
    pub fn diag_qd(&self) -> (u64, u64) {
        (
            self.rtt.rtprop().map_or(0, |d| d.as_micros() as u64),
            self.rtt.queue_delay().as_micros() as u64,
        )
    }
    /// [TUR-2b/2c teşhis] tombstone (SALT-GÖZLEM):
    /// `(canlı, hit, bw_kurtarılan, evicted, ambiguous, credited_bytes)`.
    #[must_use]
    pub fn diag_tombstone(&self) -> (u64, u64, u64, u64, u64, u64) {
        (
            self.tombstones.len() as u64,
            self.diag_tomb_hits,
            self.diag_bw_recovered,
            self.tombstone_evicted,
            self.diag_ambiguous_skipped,
            self.diag_delivered_credited,
        )
    }
    /// Son hesaplanan forward-recovery eylemi (§11).
    #[must_use]
    pub fn last_recovery(&self) -> RecoveryAction {
        self.last_recovery
    }
    /// Güncel emergency seviyesi.
    #[must_use]
    pub fn emergency_level(&self) -> EmergencyLevel {
        self.emergency.level()
    }
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

fn dur_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: u64) -> OpaqueUnit {
        OpaqueUnit { id, size: 1200 }
    }
    const MS: u64 = 1_000_000;

    fn ack(seqs: &[u64]) -> Event {
        Event::AckReceived(AckInfo {
            acked_seqs: seqs.to_vec(),
            ack_delay: Duration::ZERO,
        })
    }

    #[test]
    fn idle_to_preconverge_then_startup_on_first_ack() {
        // §3.3: ilk data → PRE_CONVERGE (cold-start, ölçüm yok) — Startup DEĞİL. İlk geçerli SACK → Startup.
        let mut s = Sender::new();
        assert_eq!(s.state(), SenderState::Idle);
        let out = s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.state(), SenderState::PreConverge);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, TransmitKind::Data);
        assert_eq!(out[0].seq, 0);
        assert_eq!(s.inflight_bytes(), 1200);
        s.on_event(Event::AppData(unit(1)), MS); // ikinci birim inflight kalsın (ack sonrası iş var)
                                                 // §3.3-D kademeli tahliye: ilk geçerli SACK (RTT kilidi) + iş var → Startup.
        s.on_event(ack(&[0]), 50 * MS);
        assert_eq!(s.state(), SenderState::Startup);
    }

    #[test]
    fn preconverge_clean_no_spurious_recovery() {
        // §3.3 yasaklar: temiz cold-start → dup YOK + retx YOK (BEFORE'daki dup=1820B'nin kök çözümü).
        let mut s = Sender::new();
        let o0 = s.on_event(Event::AppData(small(0)), 0);
        assert_eq!(s.state(), SenderState::PreConverge);
        assert!(
            o0.iter().all(|d| d.kind == TransmitKind::Data),
            "PRE_CONVERGE'de yalnız DATA emit"
        );
        // ilk geçerli SACK → cold-start biter (tek birim acked → Idle); hiç Duplicate/Retransmit YOK (yasak-1/2).
        let o1 = s.on_event(ack(&[0]), 5 * MS);
        assert_ne!(
            s.state(),
            SenderState::PreConverge,
            "ilk SACK cold-start'ı bitirir"
        );
        let recov = o1
            .iter()
            .filter(|d| matches!(d.kind, TransmitKind::Duplicate | TransmitKind::Retransmit))
            .count();
        assert_eq!(
            recov, 0,
            "temiz cold-start'ta spurious recovery YOK (dup+retx={recov})"
        );
    }

    #[test]
    fn ack_frees_inflight_and_updates_rtt() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.inflight_bytes(), 1200);
        s.on_event(ack(&[0]), 50 * MS);
        assert_eq!(s.inflight_bytes(), 0);
        assert!(s.rtt.srtt().is_some());
    }

    #[test]
    fn ack_confidence_low_enters_recover_fwd_and_duplicates() {
        // ACK-yolu çöküşü (p5_revpath): DATA gönder, HİÇ ACK gelme → RECOVER_FWD + ACK-bağımsız
        // duplication. Baseline'da QUIC c(10s)=0.00 olduğu nokta; engine forward-recovery yapıyor.
        let mut s = Sender::new();
        // warm-up: PRE_CONVERGE'den çık (§3.3-D) — RECOVER_FWD/emergency POST-cold-start davranışı.
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        assert_ne!(
            s.state(),
            SenderState::PreConverge,
            "warm-up cold-start'ı bitirmeli"
        );
        // artık ACK gelmiyor → ack_confidence düşer → RECOVER_FWD (§11)
        s.on_event(Event::AppData(unit(1)), 20 * MS);
        s.on_event(Event::AppData(unit(2)), 21 * MS);
        let mut saw_fwd = false;
        let mut saw_escalate = false;
        let mut dup_count = 0usize;
        for t in 1..=25 {
            let out = s.on_event(Event::TimerFired(Timer::Pto), (t * 100 + 21) * MS);
            if s.state() == SenderState::RecoverFwd {
                saw_fwd = true;
            }
            if s.emergency_level() > EmergencyLevel::Arq {
                saw_escalate = true;
            }
            dup_count += out
                .iter()
                .filter(|d| d.kind == TransmitKind::Duplicate)
                .count();
        }
        assert!(saw_fwd, "ACK yok → RECOVER_FWD'e girmeli (§11)");
        assert!(
            dup_count > 0,
            "RECOVER_FWD ACK-bağımsız DUPLICATE emit etmeli (dup={dup_count})"
        );
        assert!(saw_escalate, "emergency bir noktada escalate etmeli");
    }

    // Küçük birim → burst'e çok paket sığar; düşük loss oranı (fuzzy R1: loss LOW → arq_light>0).
    fn small(id: u64) -> OpaqueUnit {
        OpaqueUnit { id, size: 100 }
    }

    #[test]
    fn low_loss_with_healthy_acks_retransmits() {
        // ACK'ler akıyor (confidence OK) + DÜŞÜK loss (%10) → RECOVER_ARQ → selective retransmit (§9).
        // (Yüksek loss + düşük bw'de fuzzy ARQ'yu baskılar = FEC-dominant, doğru — o ayrı senaryo.)
        let mut s = Sender::new();
        // warm-up: PRE_CONVERGE'den çık + BtlBw kur (MTU birim → burst kanal-türevli ≥10 küçük birime yeter).
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        // 10 küçük paket (burst): seq1..10 (warm-up seq0 tüketti).
        for i in 0..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS);
        }
        // seq2..10 ACK'li, seq1 DEĞİL → packet-threshold (≥3 yüksek acked) → seq1 kayıp. loss %10.
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS);
        let retx = out
            .iter()
            .filter(|d| d.kind == TransmitKind::Retransmit)
            .count();
        assert!(
            retx > 0,
            "düşük loss + ACK-canlı → RECOVER_ARQ retransmit (retx={retx}, arq_aggr={})",
            s.last_decision().arq_aggressiveness_permille
        );
        assert_eq!(s.state(), SenderState::RecoverArq);
    }

    #[test]
    fn retransmit_references_same_unit_id() {
        // Retransmit AYNI opak birimi (unit_id) yeni seq ile taşımalı (SoC: transport birimi bilir).
        let mut s = Sender::new();
        // warm-up: PRE_CONVERGE'den çık + BtlBw kur (MTU birim → burst 10 küçük birime yeter).
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(42)), 20 * MS); // seq1 = unit 42
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS); // seq2..10
        }
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS); // seq1 kayıp
        let retx = out.iter().find(|d| d.kind == TransmitKind::Retransmit);
        assert_eq!(
            retx.map(|d| d.unit_id),
            Some(42),
            "retransmit kayıp birimin unit_id'sini (42) yeni seq ile taşımalı"
        );
        assert_ne!(
            retx.map(|d| d.seq),
            Some(1),
            "retransmit YENİ seq almalı (kayıp seq1 değil)"
        );
    }

    #[test]
    fn drains_to_idle_when_quiescent() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(ack(&[0]), 30 * MS);
        assert_eq!(s.state(), SenderState::Idle);
    }

    #[test]
    fn tail_loss_liveness_without_loss_classification() {
        // [TUR-1a / D-1/1] Tail kaybı: daha yüksek seq ACK'i YOK → time-threshold loss SINIFLANDIRAMAZ
        // (saf sessizlik loss metriğini ARTIRMAZ); recovery LIVENESS'tan gelir (silence → ack_confidence
        // → RECOVER_FWD → spekülatif geometrik Duplicate; ACK'le iptal edilir, loss sayılmaz).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0); // warm-up seq0
        s.on_event(ack(&[0]), 10 * MS); // srtt=10ms; PreConverge'den çık
        s.on_event(Event::AppData(unit(42)), 20 * MS); // tail: seq1 = unit 42, ACK GELMEYECEK
        let mut dup_seen = false;
        for t in 1..=30 {
            let out = s.on_event(Event::TimerFired(Timer::Pto), (20 + t * 100) * MS);
            dup_seen |= out
                .iter()
                .any(|d| d.kind == TransmitKind::Duplicate && d.unit_id == 42);
        }
        let (lpkt, ltime, pend) = s.diag_loss();
        assert_eq!(
            (lpkt, ltime, pend),
            (0, 0, 0),
            "saf sessizlik loss metriğini artırmamalı (later-ACK preconditionı)"
        );
        assert_eq!(s.loss.loss_pre_fec_ppm(), 0, "loss_ppm da 0 kalmalı");
        assert!(
            dup_seen,
            "tail liveness: RECOVER_FWD spekülatif kopyayı ateşlemeli (unit 42)"
        );
    }

    #[test]
    fn clean_pair_flow_no_time_threshold_loss() {
        // [TUR-1a] Temiz akış, SACK'ler cadence gecikmesiyle: precondition sayesinde time-threshold
        // ASLA ateşlemez (K-1 false-loss sınıfı yapısal ölür). ACK'ler her birimi kapsıyor.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        // 4 birim; SACK'ler GEÇ (cadence) ama tam — her ack'ten önce uzun PTO sessizliği.
        for (i, send_ms) in [(1u64, 20u64), (2, 30), (3, 40), (4, 50)] {
            s.on_event(Event::AppData(unit(i)), send_ms * MS);
        }
        // 3s boyunca PTO tick'leri (eski kod burada hepsini "kayıp" ilan ederdi: 9/8×10ms ≪ 3s).
        for t in 1..=30 {
            s.on_event(Event::TimerFired(Timer::Pto), (50 + t * 100) * MS);
        }
        // Geç ama TAM SACK.
        s.on_event(ack(&[1, 2, 3, 4]), 3200 * MS);
        let (lpkt, ltime, _) = s.diag_loss();
        assert_eq!(
            (lpkt, ltime),
            (0, 0),
            "temiz-ama-geç SACK akışında hiçbir loss sınıflandırılmamalı"
        );
    }

    #[test]
    fn unloss_cancels_pending_retransmit_and_raises_tolerance() {
        // [TUR-2a D-1/3] UN-LOSS: packet-threshold'la kayıp ilan edilen seq'e GEÇ ACK gelirse
        // pending-retransmit İPTAL edilir (hipotez çürüdü) + reorder-toleransı yükselir
        // (note_spurious kablosu) + windowed loss_ppm'e SAYILMAZ.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS); // PreConverge'den çık
        for i in 1..=5u64 {
            s.on_event(Event::AppData(unit(i)), (10 + i) * MS);
        }
        // seq1 atlanır, 2..5 ack'lenir → gap≥3 → seq1 packet-threshold Suspected + pending'e girer.
        s.on_event(ack(&[2, 3, 4, 5]), 100 * MS);
        {
            let (lpkt, _, pend) = s.diag_loss();
            assert_eq!(
                (lpkt, pend),
                (1, 1),
                "seq1 kayıp ilan edilip kuyruğa girmeli"
            );
        }
        // GEÇ ACK seq1 (reorder'mış meğer) → un-loss: pending boşalır, retransmit YOK.
        s.on_event(ack(&[1]), 200 * MS);
        let (unloss, lppm, spur) = s.diag_ledger();
        let (_, _, pend) = s.diag_loss();
        assert_eq!(pend, 0, "un-loss pending-retransmit'i iptal etmeli");
        assert_eq!(unloss, 1);
        assert_eq!(spur, 1);
        assert_eq!(lppm, 0, "Spurious windowed-ppm'e sayılmaz");
        assert!(
            s.reorder.recent_max_reorder() >= 4,
            "note_spurious gözlenen reorder mesafesini kaydetmeli (la=5 − seq=1)"
        );
    }

    #[test]
    fn tombstone_recovers_bw_and_rtt_from_late_ack() {
        // [TUR-2b D-1/4] Kayıp-ilanı kaydı DÜŞÜRMEZ (tombstone) → geç-ACK'te kol-1
        // (retx_count==0) BW-örneği + RTT KURTARILIR (eski davranış: sansür).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        for i in 1..=5u64 {
            s.on_event(Event::AppData(unit(i)), (10 + i) * MS);
        }
        s.on_event(ack(&[2, 3, 4, 5]), 100 * MS); // seq1 → Suspected + tombstone
        {
            let (live, hits, ..) = s.diag_tombstone();
            assert_eq!((live, hits), (1, 0), "ilan tombstone'a yazılmalı");
        }
        let srtt_before = s.rtt.srtt().expect("srtt var");
        s.on_event(ack(&[1]), 500 * MS); // GEÇ-ACK → tombstone hit
        let (live, hits, bw_rec, evicted, ambig, credited) = s.diag_tombstone();
        assert_eq!(
            (live, hits, bw_rec, evicted, ambig, credited),
            (0, 1, 1, 0, 0, 0),
            "geç-ACK: hit + kol-1 BW-örneği kurtarılır (kredi kol-2'nin işi)"
        );
        assert!(
            s.bw_sampled_since_round,
            "kurtarılan BW-örneği plato-round'unu beslemeli"
        );
        // RTT: seq1 en-yüksek-yeni-acked Data → 500ms−11ms ≈ 489ms örneği srtt'yi YUKARI çeker.
        assert!(
            s.rtt.srtt().expect("srtt") > srtt_before,
            "geç-ACK RTT örneği (kol-1) işlenmeli"
        );
    }

    #[test]
    fn karn_superseded_late_ack_credits_without_samples() {
        // [TUR-2c D-1/3 kol-2] Retransmit EMİT edildikten SONRA gelen orijinal-ACK:
        // RTT ALINMAZ + rate-örneği ÜRETİLMEZ + delivered-KREDİSİ girer + ambiguous sayacı.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0); // warm-up (MTU) → BtlBw kur
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(42)), 20 * MS); // seq1 = unit 42
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS); // seq2..10
        }
        // seq1 packet-threshold kayıp + AYNI event'te retransmit emit (mevcut test-kalıbı).
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS);
        assert!(
            out.iter().any(|d| d.kind == TransmitKind::Retransmit),
            "kalıp gereği retransmit emit edilmeli"
        );
        // Superseded işareti: tombstone(seq1).retx_count arttı → kol-1'e düşemez.
        let srtt_before = s.rtt.srtt().expect("srtt");
        let btlbw_before = s.btlbw.btlbw();
        // GEÇ orijinal-ACK (seq1): kol-2.
        s.on_event(ack(&[1]), 400 * MS);
        let (_, hits, bw_rec, _, ambig, credited) = s.diag_tombstone();
        assert_eq!((hits, ambig, bw_rec), (1, 1, 0), "kol-2: örneksiz hit");
        assert!(
            credited > 0,
            "delivered-kredisi girmeli (byte'lar wire'ı geçti)"
        );
        assert_eq!(
            s.rtt.srtt().expect("srtt"),
            srtt_before,
            "kol-2: RTT örneği ALINMAZ (400ms'lik sahte örnek srtt'yi bozardı)"
        );
        assert_eq!(
            s.btlbw.btlbw(),
            btlbw_before,
            "kol-2: bu ACK'ten rate-örneği ÜRETİLMEZ"
        );
        // [review-fix §9.3] kanonik spurious (retx-SONRASI orijinal-ACK) reorder-toleransını yükseltir.
        assert!(
            s.reorder.recent_max_reorder() >= 9,
            "kol-2 note_spurious kablosu: gözlenen mesafe (la=10 − seq=1) kaydedilmeli"
        );
    }

    #[test]
    fn karn_both_transmissions_acked_wire_byte_semantics() {
        // [TUR-2c review-vektörü] Retransmit'in KENDİ ACK'i geldikten SONRA orijinal-geç-ACK:
        // retransmit inflight-yolundan TAM örnek verir (seq iletimi tekil tanımlar); orijinal
        // kol-2'den yalnız delivered-KREDİSİ alır. İKİ kopyanın da sayılması WIRE-BYTE
        // muhasebesidir (kapasite ölçümü, BBR semantiği) — "teslimat BİR KEZ" BİRİM-düzlemidir
        // (kopya-iptal + bench idempotent damga), iki düzlem karışmaz.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(200)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        s.on_event(Event::AppData(small(42)), 20 * MS); // seq1
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 20 * MS);
        }
        let out = s.on_event(ack(&[2, 3, 4, 5, 6, 7, 8, 9, 10]), 30 * MS);
        let retx_seq = out
            .iter()
            .find(|d| d.kind == TransmitKind::Retransmit)
            .expect("retransmit emit")
            .seq;
        // 1) Retransmit'in kendi ACK'i: normal inflight-yolu (örnekli).
        s.on_event(ack(&[retx_seq]), 60 * MS);
        let srtt_after_retx = s.rtt.srtt().expect("srtt");
        let btlbw_after_retx = s.btlbw.btlbw();
        // 2) Orijinal geç-ACK: kol-2 — örneksiz, KREDİLİ.
        s.on_event(ack(&[1]), 400 * MS);
        let (_, hits, bw_rec, _, ambig, credited) = s.diag_tombstone();
        assert_eq!((hits, ambig, bw_rec), (1, 1, 0), "kol-2 örneksiz hit");
        assert!(credited > 0, "wire-byte kredisi (kopya gerçekten geçti)");
        assert_eq!(s.rtt.srtt().expect("srtt"), srtt_after_retx, "RTT değişmez");
        assert_eq!(s.btlbw.btlbw(), btlbw_after_retx, "rate-örneği üretilmez");
    }

    #[test]
    fn tombstone_expiry_yields_no_sample() {
        // [TUR-2b] TTL geçti → prune (PTO) → çok-geç ACK örneksiz (Expired/Unknown).
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(100)), 0);
        s.on_event(ack(&[0]), 10 * MS);
        for i in 1..=5u64 {
            s.on_event(Event::AppData(unit(i)), (10 + i) * MS);
        }
        // Emisyon-flush: tüm birimler ACK'ten ÖNCE wire'a çıksın (pacer-token beklemesi
        // yüzünden geç-emit → phantom-ack + Pto'da MEŞRU yeni sınıflandırma test-amacını
        // bulandırıyordu; motor doğruydu, testin zaman-çizgisi özensizdi).
        for t in [20u64, 30, 40, 60, 80] {
            s.on_event(Event::TimerFired(Timer::Pto), t * MS);
        }
        s.on_event(ack(&[2, 3, 4, 5]), 100 * MS); // seq1 tombstone'da (TTL = 8×T_loss)
        {
            let (live, ..) = s.diag_tombstone();
            assert_eq!(live, 1, "yalnız seq1 tombstone'da olmalı (flush sonrası)");
        }
        // TTL geçsin: T_loss ilan-anı ≈ cadence-taban ~3.2s → TTL ~25s; 400s fazlasıyla aşar.
        s.on_event(Event::TimerFired(Timer::Pto), 400_000 * MS);
        let (live, ..) = s.diag_tombstone();
        assert_eq!(live, 0, "TTL dolan kayıt prune edilmeli");
        s.on_event(ack(&[1]), 401_000 * MS);
        let (_, hits, bw_rec, ..) = s.diag_tombstone();
        assert_eq!((hits, bw_rec), (0, 0), "Expired → geç-ACK örneksiz");
        // [commit-B §3.7] prune uçuş-sayısını düşürmeli: pif == GERÇEK engine-uçuşu (burada
        // liveness-dup seq'i hâlâ uçuşta → eşitlik). Fix'siz pif = uçuş+1 kalırdı (sızıntı).
        assert_eq!(
            s.diag_pif(),
            s.inflight.len() as u64,
            "TTL-prune sonrası pif gerçek-uçuşa eşit olmalı (sızıntı-fix)"
        );
    }
}
