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
//! **TASARIM YORUMU (§3 tablosundan sapma, onaylı):** RECOVER_FWD *ve* RECOVER_ARQ her aktif fazdan
//! erişilir (yalnız STEADY'den değil). ACK-yolu çökerse (p5_revpath — baseline'da QUIC c(10s)=0.00)
//! STARTUP platoya ulaşamaz → STEADY erişilemez → recovery kilitlenirdi; ayrıca loss STARTUP'ta olsa
//! da (yaygın) retransmit tıkanmamalı. Öncelik: ack_confidence LOW → RECOVER_FWD (ACK-bağımsız, §11);
//! değilse pending loss → RECOVER_ARQ (§9). ack_confidence = ACK SESSİZLİĞİ (naif sent/recv oranı değil).
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
use crate::rate::{effective_send_rate, PacingPhase, SrcConfig};
use crate::rtt::{rtt_sample, RttEstimator};

/// Sender durumları (§3). Geçişler event-driven + deterministik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderState {
    /// Gönderilecek yok; timer'lar pasif.
    Idle,
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
                    self.state = SenderState::Startup;
                }
            }
            Event::AckReceived(ack) => self.on_ack_received(&ack, now_ns),
            Event::TimerFired(timer) => self.on_timer(timer, now_ns),
        }

        let decision = self.fuzzy_evaluate(now_ns);
        self.last_decision = decision;
        self.update_state(&decision, now_ns);
        let send_rate = self.compute_send_rate(&decision);
        self.pacer.set_rate(send_rate);

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
        for &seq in &ack.acked_seqs {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample);
                self.btlbw.update(sample, now_ns, sample.delivered > 0);
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

        // loss tespiti (§5). loss = RECOVERY sinyali (INV-RATE-1: rate'e DEĞMEZ).
        let lost = self.loss.on_ack(&ack.acked_seqs);
        self.queue_losses(&lost, now_ns);
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
                let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
                let lost = self.loss.detect_time_losses(now_ns, srtt);
                self.queue_losses(&lost, now_ns);
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
        } else if emergency_active {
            // ack_confidence LOW → forward-recovery (ACK-bağımsız, §11), her aktif fazdan.
            SenderState::RecoverFwd
        } else if !self.pending_lost.is_empty() {
            // residual loss + ack_confidence OK → selective retransmit (§9), her aktif fazdan
            // (RECOVER_FWD ile simetrik: loss STARTUP'ta olsa da recovery tıkanmamalı).
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
    fn startup_plateau(&mut self) -> bool {
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

    /// ScheduleTransmit (§3): recovery + DATA'yı pacing bütçesinde emit et.
    /// Öncelik (recovery-obsesif "veri ulaşsın"): RECOVER_ARQ retransmit / RECOVER_FWD dup-plan →
    /// vadesi gelen duplicate'ler → yeni DATA. Hepsi tek pacer'ı (token bucket) paylaşır (§9/§13).
    fn schedule_transmit(&mut self, now_ns: u64) -> Vec<TransmitDecision> {
        let mut out = Vec::new();
        match self.state {
            SenderState::RecoverArq => self.plan_arq_retransmits(&mut out, now_ns),
            SenderState::RecoverFwd => self.plan_forward_duplication(now_ns),
            _ => {}
        }
        self.drain_due_duplicates(&mut out, now_ns);
        self.drain_new_data(&mut out, now_ns);
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
    fn plan_forward_duplication(&mut self, now_ns: u64) {
        let (factor, spacing) = self.dup_params();
        let recs: Vec<LostRec> = self.pending_lost.drain(..).collect();
        for rec in recs {
            for i in 0..u64::from(factor) {
                self.dup_queue.push(DupRec {
                    due_ns: now_ns + i.saturating_mul(dur_ns(spacing)),
                    unit: rec.unit,
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
        }
    }

    /// Yeni DATA'yı pace ederek emit et. RECOVER_FWD'de her DATA ayrıca (factor−1) kopya planlar.
    fn drain_new_data(&mut self, out: &mut Vec<TransmitDecision>, now_ns: u64) {
        while let Some(front) = self.tx_queue.front().copied() {
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
                    });
                }
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

    /// ack_confidence (per-mille): reverse-path canlılığı = ACK SESSİZLİĞİ. Aktif iş varken son
    /// ACK'ten (ya da ilk gönderimden) geçen süre srtt katını aşarsa güven düşer. Yeni paket "düşük
    /// güven" DEĞİL — yalnız SESSİZLİK reverse-path çöküşüdür.
    fn ack_confidence_permille(&self, now_ns: u64) -> u64 {
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
    fn idle_to_startup_and_first_data_paced() {
        let mut s = Sender::new();
        assert_eq!(s.state(), SenderState::Idle);
        let out = s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.state(), SenderState::Startup);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, TransmitKind::Data);
        assert_eq!(out[0].seq, 0);
        assert_eq!(s.inflight_bytes(), 1200);
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
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(Event::AppData(unit(1)), MS);
        let mut saw_fwd = false;
        let mut saw_escalate = false;
        let mut dup_count = 0usize;
        for t in 1..=25 {
            let out = s.on_event(Event::TimerFired(Timer::Pto), t * 100 * MS);
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
        // 10 küçük paket (burst hepsini yollar): seq0..9.
        for i in 0..10 {
            s.on_event(Event::AppData(small(i)), 0);
        }
        // seq1..9 ACK'li, seq0 DEĞİL → packet-threshold (≥3 yüksek acked) → seq0 kayıp. loss %10.
        let out = s.on_event(ack(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 30 * MS);
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
        s.on_event(Event::AppData(small(42)), 0); // seq0 = unit 42
        for i in 1..10 {
            s.on_event(Event::AppData(small(i)), 0);
        }
        let out = s.on_event(ack(&[1, 2, 3, 4, 5, 6, 7, 8, 9]), 30 * MS);
        let retx = out.iter().find(|d| d.kind == TransmitKind::Retransmit);
        assert_eq!(
            retx.map(|d| d.unit_id),
            Some(42),
            "retransmit kayıp birimin unit_id'sini (42) yeni seq ile taşımalı"
        );
        assert_ne!(
            retx.map(|d| d.seq),
            Some(0),
            "retransmit YENİ seq almalı (0 değil)"
        );
    }

    #[test]
    fn drains_to_idle_when_quiescent() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(ack(&[0]), 30 * MS);
        assert_eq!(s.state(), SenderState::Idle);
    }
}
