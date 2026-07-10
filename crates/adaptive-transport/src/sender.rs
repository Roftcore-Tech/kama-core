//! Sender State Machine (engine spec §3) — 18 bileşeni tek olay-güdümlü döngüde birleştiren ŞEF.
//!
//! **Deterministik çekirdek (ADR-17):** olaylar girer → transmit KARARLARI çıkar. Gerçek I/O ve
//! kripto DIŞINDA (INV-ENGINE-SOC): Sender yalnız opak birim KİMLİĞİ + BOYUTU tutar (`OpaqueUnit`),
//! bytes/AEAD/anahtar TUTMAZ. `on_event` saf: aynı girdi → aynı çıktı → golden-vector'le test edilir.
//!
//! **INV-RATE-1:** loss `send_rate`'i DÜŞÜRMEZ. Recovery durumları (RECOVER_ARQ/FWD) pacing_gain'i
//! `Steady` tutar; loss yalnız [`loss`](crate::loss) + fuzzy `loss_ppm` → FEC/ARQ recovery kolunu besler.
//!
//! **Kapsam (Artım 1 — spine):** state machine (§3 geçişleri) + ölçüm muhasebesi (§3.1/§3.2) +
//! karar pipeline (fusion §8.4 → SRC §8.2 → budget §8.5 → pacer §13) + DATA pacing. Recovery
//! İCRA kolları (ARQ retransmit §9 / FEC parity §10 / duplication §11 emission) Artım 3 — durum
//! makinesi O DURUMLARA GEÇER ve doğru RecoveryAction'ı HESAPLAR, ama ek frame emisyonu sonraki artım.
//!
//! **TASARIM YORUMU (§3 tablosundan sapma, gerekçeli):** §3 geçiş tablosu RECOVER_FWD'yi yalnız
//! STEADY'den gösterir. Ama ACK-yolu çökerse (p5_revpath — baseline'da QUIC'in c(10s)=0.00 olduğu
//! yer) STARTUP'ta BtlBw platoya ulaşamaz (ACK yok → btlbw büyümez) → STEADY'ye hiç geçilemez →
//! RECOVER_FWD erişilemez olurdu. Recovery-obsesif taban gereği: ack_confidence LOW HER aktif
//! fazdan RECOVER_FWD tetikler (forward-recovery ACK'e bağımsızdır, §11 — tam da bu yüzden).

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

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

/// Transmit türü. Artım 1'de yalnız `Data` emit edilir; recovery türleri Artım 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransmitKind {
    /// tx_queue'dan yeni DATA.
    Data,
    /// ARQ selective retransmit (§9) — Artım 3.
    Retransmit,
    /// FEC parity shard (§10) — Artım 3.
    FecParity,
    /// forward-recovery duplication (§11) — Artım 3.
    Duplicate,
}

/// Bir gönderilmiş DATA paketinin muhasebe kaydı (RTT/BW/loss + retransmit).
struct InflightPacket {
    unit: OpaqueUnit,
    send_ns: u64,
    bw_sample: SentPacket,
}

/// İlk RTT ölçülene kadar pacing/eşik için başlangıç RTT [CAL initial].
const INITIAL_RTT: Duration = Duration::from_millis(100);
/// STARTUP→STEADY plato: BtlBw N ardışık round Δ<ε ise plato [CAL].
const STARTUP_PLATEAU_ROUNDS: u32 = 3;
/// plato ε (per-mille): btlbw artışı < %5 → stabil.
const STARTUP_PLATEAU_EPS_PERMILLE: u64 = 50;
/// ack_confidence: bu kadar srtt sessizlik (ACK yok) → tam güvensizlik ölçeği [CAL].
const ACK_SILENCE_SRTT_MULT: u32 = 3;

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

    // ── icra (§13) ──
    pacer: TokenBucket,

    // ── durum sayaçları ──
    next_seq: u64,
    inflight_bytes: u64,
    inflight: BTreeMap<u64, InflightPacket>,
    tx_queue: VecDeque<OpaqueUnit>,
    pending_lost: VecDeque<u64>, // §5 tespit edilen kayıp seq'ler (Artım 3 retransmit kuyruğu)
    last_now_ns: u64,

    // ── ack_confidence (RECOVER_FWD tetiği): sessizlik-tabanlı ──
    last_ack_ns: Option<u64>,   // en son ACK alım anı
    first_send_ns: Option<u64>, // ilk gönderim (hiç ACK yoksa sessizlik buradan)

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
            next_seq: 0,
            inflight_bytes: 0,
            inflight: BTreeMap::new(),
            tx_queue: VecDeque::new(),
            pending_lost: VecDeque::new(),
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
    /// Emit edilen transmit kararlarını döndürür (çağıran gerçekten yollar).
    pub fn on_event(&mut self, event: Event, now_ns: u64) -> Vec<TransmitDecision> {
        // pacer refill: son olaydan bu yana geçen süre kadar token ekle (§13).
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

        // §3 pipeline: karar → state → rate → pacing.
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
        self.last_ack_ns = Some(now_ns); // reverse-path canlı: sessizlik sıfırlanır.

        // RTT: en yüksek yeni-acked paketten örnekle (standart: largest newly-acked).
        let mut rtt_send_ns: Option<u64> = None;
        let mut largest = 0u64;
        for &seq in &ack.acked_seqs {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                let sample = self.bw_sampler.on_ack(now_ns, p.bw_sample);
                let valid = sample.delivered > 0; // §7.3 basit geçerlilik (spine); rafine sonra.
                self.btlbw.update(sample, now_ns, valid);
                if rtt_send_ns.is_none() || seq >= largest {
                    largest = seq;
                    rtt_send_ns = Some(p.send_ns);
                }
            }
        }

        // RTT örneği (§6): raw = now − send; ack_delay düzeltmesi.
        if let Some(send_ns) = rtt_send_ns {
            let raw = Duration::from_nanos(now_ns.saturating_sub(send_ns));
            if let Some(sample) = rtt_sample(raw, ack.ack_delay) {
                self.rtt.observe(now_ns, sample);
            }
        }

        // loss tespiti (§5): packet-threshold. loss = RECOVERY sinyali (INV-RATE-1: rate'e DEĞMEZ).
        let lost = self.loss.on_ack(&ack.acked_seqs);
        for seq in lost {
            if let Some(p) = self.inflight.remove(&seq) {
                self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
            }
            self.pending_lost.push_back(seq);
        }
    }

    /// §12 timer handler (spine: RTprop probe → DRAIN_RTPROP; PTO → zaman-eşiği loss).
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
                for seq in lost {
                    if let Some(p) = self.inflight.remove(&seq) {
                        self.inflight_bytes = self.inflight_bytes.saturating_sub(p.unit.size);
                    }
                    self.pending_lost.push_back(seq);
                }
            }
        }
    }

    /// FuzzyEvaluate (§8.4/§15): ölçümlerden FusionInputs kur → controller kararı.
    fn fuzzy_evaluate(&self, now_ns: u64) -> FusionDecision {
        let srtt = self.rtt.srtt().unwrap_or(INITIAL_RTT);
        let rttvar = self.rtt.rttvar();
        // jitter_ppm = rttvar / srtt (ppm); srtt 0 → 0.
        let jitter_ppm = dur_ns(rttvar)
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

    /// §3 state geçişleri (deterministik). Öncelik: IDLE (sessiz) → RECOVER_FWD (ack_confidence LOW,
    /// HER fazdan — bkz. modül TASARIM YORUMU) → faz-içi (STARTUP plato / RECOVER_ARQ / STEADY).
    fn update_state(&mut self, decision: &FusionDecision, now_ns: u64) {
        // Emergency hysteresis (§11): health = ack_confidence (yüksek=sağlıklı); flap'i önler.
        let health = u32::try_from(self.ack_confidence_permille(now_ns)).unwrap_or(u32::MAX);
        let level = self.emergency.update(health, now_ns);
        self.last_recovery = emergency::action(level, Framing::Tiny, &self.emergency_cfg);

        // Aktif iş: kuyruk + inflight + kurtarılmamış kayıp. Hiçbiri yoksa → IDLE.
        let idle =
            self.tx_queue.is_empty() && self.inflight.is_empty() && self.pending_lost.is_empty();
        let emergency_active =
            level > EmergencyLevel::Arq || decision.mode == Mode::EmergencyForwardRedundancy;

        self.state = if idle {
            SenderState::Idle
        } else if emergency_active {
            // ack_confidence LOW → forward-recovery (ACK-bağımsız, §11), hangi fazdan olursa olsun.
            SenderState::RecoverFwd
        } else {
            match self.state {
                SenderState::Idle => SenderState::Startup, // veri geldi (idle değil)
                SenderState::Startup => {
                    if self.startup_plateau() {
                        SenderState::Steady
                    } else {
                        SenderState::Startup
                    }
                }
                SenderState::Steady
                | SenderState::RecoverArq
                | SenderState::RecoverFwd
                | SenderState::DrainRtprop => {
                    if self.pending_lost.is_empty() {
                        SenderState::Steady
                    } else {
                        SenderState::RecoverArq // residual loss + ack_confidence OK → §9
                    }
                }
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

    /// send_rate (§8.2 + §8.5): SRC pacing × fuzzy gain, sonra budget clamp. INV-RATE-1: pacing_gain
    /// duruma göre (recovery ≠ throttle); fuzzy send_rate_gain YALNIZ bw+queue_delay'den (loss-kör).
    fn compute_send_rate(&self, decision: &FusionDecision) -> u64 {
        let phase = match self.state {
            SenderState::Startup => PacingPhase::Startup,
            SenderState::DrainRtprop => PacingPhase::ProbeDown,
            // Steady + recovery durumları: base rate Steady (loss rate'i düşürmez, INV-RATE-1).
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
        // clamp [MIN_RATE, ∞): MIN_RATE floor rate-düşüklüğünden (loss'tan DEĞİL, INV-RATE-1).
        budget::send_rate_final(fuzzy_rate, self.budget_cfg.min_rate_bps, u64::MAX)
    }

    /// ScheduleTransmit (§3): pacing token'ı elverdikçe DATA gönder (+ OnPacketSent §3.1 muhasebesi).
    /// Artım 1: yalnız DATA. Recovery emisyonu (retransmit/parity/dup) Artım 3 — `last_recovery`
    /// HESAPLANDI ama ek frame henüz emit edilmiyor.
    fn schedule_transmit(&mut self, now_ns: u64) -> Vec<TransmitDecision> {
        let mut out = Vec::new();
        while let Some(front) = self.tx_queue.front().copied() {
            if !self.pacer.try_send(front.size) {
                break; // token yok → pacing bekliyor.
            }
            self.tx_queue.pop_front();
            let seq = self.on_packet_sent(front, now_ns);
            out.push(TransmitDecision {
                kind: TransmitKind::Data,
                seq,
                unit_id: front.id,
                size: front.size,
            });
        }
        out
    }

    /// §3.1 OnPacketSent: seq ata, inflight+muhasebe, RTT/BW/loss kayıt.
    fn on_packet_sent(&mut self, unit: OpaqueUnit, now_ns: u64) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        if self.first_send_ns.is_none() {
            self.first_send_ns = Some(now_ns);
        }
        // app-limited: bu gönderimden sonra kuyruk boşsa (§7.3 estimator şişme koruması).
        let app_limited = self.tx_queue.is_empty();
        let bw_sample = self.bw_sampler.on_sent(now_ns, unit.size, app_limited);
        self.loss.on_sent(seq, now_ns, 1); // 1 orijinal birim / DATA paketi.
        self.inflight_bytes += unit.size;
        self.inflight.insert(
            seq,
            InflightPacket {
                unit,
                send_ns: now_ns,
                bw_sample,
            },
        );
        seq
    }

    /// ack_confidence (per-mille): reverse-path canlılığı = ACK SESSİZLİĞİ ölçüsü. Aktif iş varken
    /// son ACK'ten (ya da ilk gönderimden) bu yana geçen süre srtt'nin katını aşarsa güven düşer.
    /// Yeni gönderilen paket "düşük güven" DEĞİL (henüz ACK'lenme zamanı gelmedi) — sadece SESSİZLİK.
    fn ack_confidence_permille(&self, now_ns: u64) -> u64 {
        let active =
            !self.inflight.is_empty() || !self.pending_lost.is_empty() || !self.tx_queue.is_empty();
        if !active {
            return 1000; // outstanding iş yok → tam güven.
        }
        let anchor = match self.last_ack_ns.or(self.first_send_ns) {
            Some(a) => a,
            None => return 1000, // hiç göndermedi.
        };
        let silence = now_ns.saturating_sub(anchor);
        let srtt = dur_ns(self.rtt.srtt().unwrap_or(INITIAL_RTT));
        let threshold = srtt.saturating_mul(u64::from(ACK_SILENCE_SRTT_MULT));
        if silence <= threshold {
            return 1000;
        }
        // silence > threshold > 0 → bölme güvenli; yine de checked.
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

    #[test]
    fn idle_to_startup_and_first_data_paced() {
        let mut s = Sender::new();
        assert_eq!(s.state(), SenderState::Idle);
        let out = s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.state(), SenderState::Startup);
        assert_eq!(out.len(), 1, "burst token ile ilk DATA gönderilmeli");
        assert_eq!(out[0].kind, TransmitKind::Data);
        assert_eq!(out[0].seq, 0);
        assert_eq!(out[0].unit_id, 0);
        assert_eq!(s.inflight_bytes(), 1200);
    }

    #[test]
    fn ack_frees_inflight_and_updates_rtt() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        assert_eq!(s.inflight_bytes(), 1200);
        let ack = AckInfo {
            acked_seqs: vec![0],
            ack_delay: Duration::ZERO,
        };
        s.on_event(Event::AckReceived(ack), 50 * MS);
        assert_eq!(s.inflight_bytes(), 0, "acked paket inflight'tan düşer");
        assert!(s.rtt_srtt_is_some(), "ACK RTT örneği üretmeli");
    }

    #[test]
    fn ack_confidence_low_drives_recover_fwd() {
        // ACK-yolu çöküşü (p5_revpath): DATA gönder, HİÇ ACK gelme → ack_confidence sessizlikle
        // düşer → RECOVER_FWD. Baseline'da QUIC'in c(10s)=0.00 olduğu nokta; engine forward-recovery'ye
        // geçer (INV-RATE-1: rate DÜŞMEZ, yalnız recovery kolu). §3-tablosu-sapması: STARTUP'tan da erişir.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(Event::AppData(unit(1)), MS);
        // ACK YOK; zaman ilerlet (100ms..2500ms) → sessizlik 3×srtt(300ms)'i çok aşar.
        for t in 1..=25 {
            s.on_event(Event::TimerFired(Timer::Pto), t * 100 * MS);
        }
        assert!(
            s.ack_confidence_permille(2500 * MS) < 300,
            "hiç ACK yok → ack_confidence LOW ({})",
            s.ack_confidence_permille(2500 * MS)
        );
        assert_eq!(
            s.state(),
            SenderState::RecoverFwd,
            "ack_confidence LOW → RECOVER_FWD (§11 forward-recovery)"
        );
        assert!(
            s.emergency_level() > EmergencyLevel::Arq,
            "emergency escalate etmeli (level={:?})",
            s.emergency_level()
        );
    }

    #[test]
    fn healthy_acks_keep_confidence_high() {
        // ACK'ler akıyorsa (reverse-path sağlam) ack_confidence yüksek kalır → RECOVER_FWD YOK.
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        // hemen ACK → sessizlik yok.
        s.on_event(
            Event::AckReceived(AckInfo {
                acked_seqs: vec![0],
                ack_delay: Duration::ZERO,
            }),
            30 * MS,
        );
        assert!(s.ack_confidence_permille(30 * MS) >= 1000);
        assert_ne!(s.state(), SenderState::RecoverFwd);
    }

    #[test]
    fn drains_to_idle_when_quiescent() {
        let mut s = Sender::new();
        s.on_event(Event::AppData(unit(0)), 0);
        s.on_event(
            Event::AckReceived(AckInfo {
                acked_seqs: vec![0],
                ack_delay: Duration::ZERO,
            }),
            30 * MS,
        );
        assert_eq!(s.state(), SenderState::Idle, "sessiz (iş yok) → IDLE");
    }

    impl Sender {
        fn rtt_srtt_is_some(&self) -> bool {
            self.rtt.srtt().is_some()
        }
    }
}
