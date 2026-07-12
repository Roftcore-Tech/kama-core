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
    ArqScheduler, CapExceedPolicy, DeadlineArqScheduler, LostSeq, RetxConfig, RetxDecision,
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
use crate::rtt::{rtt_sample, RttEstimator};

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
            }
        }
        if let Some(send_ns) = rtt_send_ns {
            let raw = Duration::from_nanos(now_ns.saturating_sub(send_ns));
            if let Some(sample) = rtt_sample(raw, ack.ack_delay) {
                self.rtt.observe(now_ns, sample);
            }
        }

        // §3.3-B stop koşulu: teyit edilen birimlerin bekleyen geometrik kopyalarını İPTAL et.
        if !acked_units.is_empty() {
            self.dup_queue.retain(|d| !acked_units.contains(&d.unit.id));
        }

        // loss tespiti (§5). loss = RECOVERY sinyali (INV-RATE-1: rate'e DEĞMEZ).
        // §3.3-C/(1) yasak-1: PRE_CONVERGE'de loss İLANI YOK. §3.3-D: loss kilidi İLK GEÇERLİ SACK'le
        // (rtt.srtt Some) AÇILIR → o SACK'ten itibaren loss işlenir. Detector her zaman sync kalır.
        let lost = self.loss.on_ack(&ack.acked_seqs);
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
                    let lost = self.loss.detect_time_losses(now_ns, srtt);
                    let before = self.pending_lost.len();
                    self.queue_losses(&lost, now_ns);
                    // [TUR-A teşhis] loss-atıflama: time-threshold (§5b) kaynaklı kuyruklanan kayıp.
                    self.diag_loss_time += (self.pending_lost.len() - before) as u64;
                }
            }
        }
    }

    /// Kayıp seq'leri recovery kuyruğuna al. Duplicate-kökenli kayıp RE-QUEUE EDİLMEZ (redundancy'di).
    fn queue_losses(&mut self, lost: &[u64], now_ns: u64) {
        for &seq in lost {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                if p.origin != TransmitKind::Duplicate {
                    self.pending_lost.push_back(LostRec {
                        seq,
                        unit: p.unit,
                        deadline_ns: now_ns,
                        retx_count: p.retx_count,
                    });
                }
            }
        }
    }

    /// FuzzyEvaluate (§8.4/§15): ölçümlerden FusionInputs kur → controller kararı.
    fn fuzzy_evaluate(&self, now_ns: u64) -> FusionDecision {
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let jitter_ppm = dur_ns(self.rtt.rttvar())
            .saturating_mul(1_000_000)
            .checked_div(dur_ns(srtt))
            .unwrap_or(0);
        let inputs = FusionInputs {
            loss_ppm: self.loss.loss_pre_fec_ppm(),
            bandwidth_bps: self.btlbw.btlbw(),
            jitter_ppm,
            queue_delay: self.rtt.queue_delay(),
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
            for i in 0..u64::from(factor) {
                self.dup_queue.push(DupRec {
                    due_ns: now_ns + i.saturating_mul(dur_ns(spacing)),
                    unit: rec.unit,
                    interval_ns: 0, // emergency: fire-once, reschedule YOK
                });
            }
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
}
