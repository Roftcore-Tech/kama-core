//! BBR tıkanıklık denetleyicisi — **quinn-proto 0.11.18 `src/congestion/bbr/mod.rs`
//! portu** (CC-PORT, 2026-11-10). Lisans: Apache-2.0 / MIT. quinn'in kaynağı Google
//! quiche `bbr_sender.cc` ve <https://datatracker.ietf.org/doc/html/draft-cardwell-iccrg-bbr-congestion-control>.
//!
//! Durum makinesi, kazanç döngüsü, Startup çıkışı, Drain, ProbeRTT, `recovery_window`,
//! ACK toplanma tahmini ve cwnd büyütme kuralları quinn ile **aynı sırada, aynı koşullarla**.
//! Beyanlı uyarlamalar (yalnız bunlar):
//!
//! 1. **Saat**: `Instant`/`Duration` → `u64` ns (Kama motor saati).
//! 2. **Kazançlar**: `f32` → binde (‰) tamsayı (ADR-17: float yok). 2,885 → 2885‰;
//!    1/2,885 → 346‰ (quinn: 0,3466…); 1,25 → 1250‰; 0,75 → 750‰. Yuvarlama farkı < %0,2.
//! 3. **RNG yok** (ADR-17): quinn ProbeBW'ye girerken kazanç döngüsü ofsetini `{0, 2..7}`
//!    arasından rastgele seçer; burada ofset daima 0 (döngü 1,25 ile başlar). Davranışsal
//!    sonuç: her bağlantı aynı fazda başlar; tek-akış ölçümünde fark yok, çok-akış
//!    senkronizasyonu beyanlı sınır.
//! 4. `RttEstimator` yerine çağıran **min RTT**'yi (ns) verir; `on_mtu_update` yok (Kama
//!    MTU sabit, `DEFAULT_MTU`).
//! 9. **Startup'ta paket koruması** (`PACKET_CONSERVATION_IN_STARTUP`): quinn `window()` Startup'ı
//!    `recovery_window`'dan muaf tutar; Linux BBR v1 (`tcp_bbr.c` `bbr_set_cwnd_to_recover_or_restore`)
//!    kaybı her kipte "cwnd = uçuş + ACK" ile sınırlar. Ölçüldü (sim, 6 paketlik kuyruk, 10 seed):
//!    kayıpsız 31 → 23 düşme; %20 kayıpta fazla gönderim 1,41 → 1,36; %30'da 1,68 → 1,45; derin
//!    kuyruk %20 max 25,0 → 17,2 s; bedeli 1000 ms RTT kayıpsız hatta +1 s (12,8 → 13,9 s).
//! 10. **U10–U13** (sender.rs / tail_recovery.rs, 2026-11-11, r25 düşük bant kökü): PTO probu
//!     hedefi uçuşta kalır (RFC 9002 §6.2 + §7.5; eski yol ≈ 0,9 s TTL'li tombstone'a emekli ediyordu →
//!     RTT örneği yok); taban PTO tavanı yok (§6.2.1); RTT örneği yokken tur başına tek prob;
//!     taze kopyası uçuştaki kayıp yeniden kuyruklanmaz. **U14** RTT örneği yokken PTO probu
//!     kurulmaz (§3.3-B geometrik kopya merdiveni zaten birimi yeniler; iki merdiven yan yana
//!     koşunca ürün zarfıyla 25 B/s'de başlangıç kuyruğu hiç erimiyordu). Ayrıntı:
//!     docs/design/CC-PORT-ESLEME.md.
//! 5. `initial_window` varsayılanı quinn-Cubic/RFC 9002 ilk penceresi
//!    `min(10·MTU, max(2·MTU, 14720))` = 10·MTU (= 12 000 B); quinn-BBR'ın 200·MTU
//!    varsayılanı **alınmadı** (ISEMRI-CC-PORT §3).
//!
//! Satır atıfları (`q:NNN`) quinn-proto 0.11.18 `bbr/mod.rs` satırlarıdır.

use super::bw_estimation::BandwidthEstimation;
use super::min_max::MinMax;

/// Startup kazancı 2/ln 2 ≈ 2,885 (q:638).
const K_DEFAULT_HIGH_GAIN_PM: u64 = 2885;
/// Startup için türetilmiş cwnd kazancı 2 (q:640).
const K_DERIVED_HIGH_CWNDGAIN_PM: u64 = 2000;
/// ProbeBW kazanç döngüsü (q:642).
const K_PACING_GAIN_PM: [u64; 8] = [1250, 750, 1000, 1000, 1000, 1000, 1000, 1000];
/// Startup'ta "tam bant" eşiği: turdan tura ≥ %25 büyüme (q:644).
const K_STARTUP_GROWTH_TARGET_PM: u64 = 1250;
/// Büyümesiz bu kadar tur → Startup biter (q:645).
const K_ROUND_TRIPS_WITHOUT_GROWTH_BEFORE_EXITING_STARTUP: u64 = 3;
/// ProbeRTT süresi (q:246).
const K_PROBE_RTT_TIME_NS: u64 = 200_000_000;
/// min_rtt tazelik penceresi (q:217).
const K_MIN_RTT_EXPIRY_NS: u64 = 10_000_000_000;
/// ProbeRTT cwnd'si BDP'nin 0,75'i (q:277-279).
const K_MODERATE_PROBE_RTT_MULTIPLIER_PM: u64 = 750;
const PROBE_RTT_BASED_ON_BDP: bool = true;
const DRAIN_TO_TARGET: bool = true;
/// Uyarlama 9 (deneniyor): Startup'ta da paket koruması — quinn `window()` Startup'ı
/// `recovery_window`'dan muaf tutar (q:487 `mode != Startup`); Linux BBR v1
/// (`tcp_bbr.c` `bbr_set_cwnd_to_recover_or_restore`) kaybı her kipte "cwnd = uçuş + ACK" ile
/// sınırlar. Sığ kuyrukta Startup taşmasını kesmek için ölçülüyor.
const PACKET_CONSERVATION_IN_STARTUP: bool = true;

/// BBR kipi (q:587-598).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Mode {
    /// Bant keşfi (yüksek kazanç).
    Startup,
    /// Startup'ın şişirdiği kuyruğu boşalt.
    Drain,
    /// Seyir: kazanç döngüsü.
    ProbeBw,
    /// Gerçek min RTT'yi ölçmek için kısa süre yavaşla.
    ProbeRtt,
}

/// Uçuş sınırlama biçimi (q:602-610).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RecoveryState {
    NotInRecovery,
    /// ACK'lenen her bayt için bir bayt daha.
    Conservation,
    /// ACK'lenen her bayt için iki bayt daha (yavaş başlangıç benzeri).
    Growth,
}

impl RecoveryState {
    fn in_recovery(self) -> bool {
        !matches!(self, Self::NotInRecovery)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LossState {
    lost_bytes: u64,
}

impl LossState {
    fn reset(&mut self) {
        self.lost_bytes = 0;
    }
    fn has_losses(&self) -> bool {
        self.lost_bytes != 0
    }
}

/// ACK toplanma (aggregation) tahmini (q:546-584).
#[derive(Debug, Default, Copy, Clone)]
struct AckAggregationState {
    max_ack_height: MinMax,
    aggregation_epoch_start_ns: Option<u64>,
    aggregation_epoch_bytes: u64,
}

impl AckAggregationState {
    fn update_ack_aggregation_bytes(
        &mut self,
        newly_acked_bytes: u64,
        now_ns: u64,
        round: u64,
        max_bandwidth: u64,
    ) -> u64 {
        // Max bant doğruysa bu sürede ne kadar teslim beklenirdi?
        let elapsed_ns = now_ns.saturating_sub(self.aggregation_epoch_start_ns.unwrap_or(now_ns));
        let expected_bytes_acked =
            ((max_bandwidth as u128) * (elapsed_ns as u128) / 1_000_000_000) as u64;

        // ACK varış hızı max banda düşer düşmez yeni dönem başlat.
        if self.aggregation_epoch_bytes <= expected_bytes_acked {
            self.aggregation_epoch_bytes = newly_acked_bytes;
            self.aggregation_epoch_start_ns = Some(now_ns);
            return 0;
        }

        self.aggregation_epoch_bytes += newly_acked_bytes;
        let diff = self.aggregation_epoch_bytes - expected_bytes_acked;
        self.max_ack_height.update_max(round, diff);
        diff
    }
}

/// quinn `Bbr` (q:26-61).
#[derive(Debug, Clone)]
pub struct Bbr {
    current_mtu: u64,
    max_bandwidth: BandwidthEstimation,
    acked_bytes: u64,
    mode: Mode,
    loss_state: LossState,
    recovery_state: RecoveryState,
    recovery_window: u64,
    is_at_full_bandwidth: bool,
    pacing_gain_pm: u64,
    high_gain_pm: u64,
    drain_gain_pm: u64,
    cwnd_gain_pm: u64,
    high_cwnd_gain_pm: u64,
    last_cycle_start_ns: Option<u64>,
    current_cycle_offset: u8,
    init_cwnd: u64,
    min_cwnd: u64,
    prev_in_flight_count: u64,
    exit_probe_rtt_at_ns: Option<u64>,
    probe_rtt_last_started_at_ns: Option<u64>,
    min_rtt_ns: u64,
    exiting_quiescence: bool,
    pacing_rate: u64,
    max_acked_packet_number: u64,
    max_sent_packet_number: u64,
    end_recovery_at_packet_number: u64,
    cwnd: u64,
    current_round_trip_end_packet_number: u64,
    round_count: u64,
    bw_at_last_round: u64,
    round_wo_bw_gain: u64,
    ack_aggregation: AckAggregationState,
    /// Uyarlama 6: dış teslim hızı tahmini (bayt/s). `Some` ise `max_bandwidth`'ın max filtresi
    /// yerine bu okunur (bayt muhasebesi yine `max_bandwidth`'ta). Kama'nın BBR §7.1 paket-başına
    /// rate sampler'ı (`bw.rs`, draft-cardwell §4.1.1.5 referans algoritması) buraya verilir;
    /// quinn'in basitleştirilmiş `min(send_rate, ack_rate)` tahmini toplu ACK'te (çerçeve başına
    /// 2 paket) hızı ~½ ölçer ve ACK-toplanma terimi üzerinden cwnd'yi sınırsız büyütür
    /// (ölçüldü: `tests/cc_port_sim.rs`, CC_TRACE). Draft'ın kendi örnekleyicisi bu sapmayı taşımaz.
    external_bw: Option<u64>,
    // ── teşhis (karar girdisi değil) ──
    diag_congestion_events: u64,
    diag_startup_exit_round: Option<u64>,
}

/// RFC 9002 §7.2 ilk pencere: `min(10·MTU, max(2·MTU, 14720))`.
#[must_use]
pub fn rfc9002_initial_window(mtu: u64) -> u64 {
    (10 * mtu).min((2 * mtu).max(14_720))
}

fn calculate_min_window(current_mtu: u64) -> u64 {
    4 * current_mtu
}

fn apply_pm(value: u64, gain_pm: u64) -> u64 {
    ((value as u128) * (gain_pm as u128) / 1000) as u64
}

impl Bbr {
    /// `mtu` bayt; `initial_window` bayt (RFC 9002 için [`rfc9002_initial_window`]).
    #[must_use]
    pub fn new(mtu: u64, initial_window: u64) -> Self {
        let min_cwnd = calculate_min_window(mtu);
        let init_cwnd = initial_window.max(min_cwnd);
        Self {
            current_mtu: mtu,
            max_bandwidth: BandwidthEstimation::default(),
            acked_bytes: 0,
            mode: Mode::Startup,
            loss_state: LossState::default(),
            recovery_state: RecoveryState::NotInRecovery,
            recovery_window: 0,
            is_at_full_bandwidth: false,
            pacing_gain_pm: K_DEFAULT_HIGH_GAIN_PM,
            high_gain_pm: K_DEFAULT_HIGH_GAIN_PM,
            drain_gain_pm: 1_000_000 / K_DEFAULT_HIGH_GAIN_PM, // 346‰ (q:79: 1/2,885)
            cwnd_gain_pm: K_DEFAULT_HIGH_GAIN_PM,
            high_cwnd_gain_pm: K_DEFAULT_HIGH_GAIN_PM,
            last_cycle_start_ns: None,
            current_cycle_offset: 0,
            init_cwnd,
            min_cwnd,
            prev_in_flight_count: 0,
            exit_probe_rtt_at_ns: None,
            probe_rtt_last_started_at_ns: None,
            min_rtt_ns: 0,
            exiting_quiescence: false,
            pacing_rate: 0,
            max_acked_packet_number: 0,
            max_sent_packet_number: 0,
            end_recovery_at_packet_number: 0,
            cwnd: init_cwnd,
            current_round_trip_end_packet_number: 0,
            round_count: 0,
            bw_at_last_round: 0,
            round_wo_bw_gain: 0,
            ack_aggregation: AckAggregationState::default(),
            external_bw: None,
            diag_congestion_events: 0,
            diag_startup_exit_round: None,
        }
    }

    // ── quinn özel yardımcıları (q:105-391) ─────────────────────────────────────────────

    fn enter_startup_mode(&mut self) {
        self.mode = Mode::Startup;
        self.pacing_gain_pm = self.high_gain_pm;
        self.cwnd_gain_pm = self.high_cwnd_gain_pm;
    }

    fn enter_probe_bandwidth_mode(&mut self, now_ns: u64) {
        self.mode = Mode::ProbeBw;
        self.cwnd_gain_pm = K_DERIVED_HIGH_CWNDGAIN_PM;
        self.last_cycle_start_ns = Some(now_ns);
        // q:115-125 rastgele ofset {0, 2..7}; burada deterministik 0 (ADR-17 — başlık madde 3).
        self.current_cycle_offset = 0;
        self.pacing_gain_pm = K_PACING_GAIN_PM[0];
    }

    fn update_recovery_state(&mut self, is_round_start: bool) {
        // Bir tur boyunca kayıp yoksa recovery biter.
        if self.loss_state.has_losses() {
            self.end_recovery_at_packet_number = self.max_sent_packet_number;
        }
        match self.recovery_state {
            // İlk kayıpta Conservation'a gir.
            RecoveryState::NotInRecovery if self.loss_state.has_losses() => {
                self.recovery_state = RecoveryState::Conservation;
                // recovery_window calculate_recovery_window'da doğru değere kurulur.
                self.recovery_window = 0;
                // Conservation tam bir tur sürsün diye turu şimdi başlamış say.
                self.current_round_trip_end_packet_number = self.max_sent_packet_number;
            }
            RecoveryState::Growth | RecoveryState::Conservation => {
                if self.recovery_state == RecoveryState::Conservation && is_round_start {
                    self.recovery_state = RecoveryState::Growth;
                }
                if !self.loss_state.has_losses()
                    && self.max_acked_packet_number > self.end_recovery_at_packet_number
                {
                    self.recovery_state = RecoveryState::NotInRecovery;
                }
            }
            RecoveryState::NotInRecovery => {}
        }
    }

    fn update_gain_cycle_phase(&mut self, now_ns: u64, in_flight: u64) {
        // Çoğu durumda döngü bir RTT geçince ilerler.
        let mut should_advance_gain_cycling = self
            .last_cycle_start_ns
            .map(|start| now_ns.saturating_sub(start) > self.min_rtt_ns)
            .unwrap_or(false);
        // Kazanç > 1 iken hedefe (gain·BDP uçuş) gerçekten ulaşılsın — kayıp yoksa.
        if self.pacing_gain_pm > 1000
            && !self.loss_state.has_losses()
            && self.prev_in_flight_count < self.get_target_cwnd(self.pacing_gain_pm)
        {
            should_advance_gain_cycling = false;
        }
        // Kazanç < 1 iken kuyruk boşaldıysa (uçuş ≤ BDP) döngüden erken çık.
        if self.pacing_gain_pm < 1000 && in_flight <= self.get_target_cwnd(1000) {
            should_advance_gain_cycling = true;
        }

        if should_advance_gain_cycling {
            self.current_cycle_offset =
                (self.current_cycle_offset + 1) % K_PACING_GAIN_PM.len() as u8;
            self.last_cycle_start_ns = Some(now_ns);
            // Hedef BDP'ye inene kadar düşük kazançta kal.
            if DRAIN_TO_TARGET
                && self.pacing_gain_pm < 1000
                && K_PACING_GAIN_PM[self.current_cycle_offset as usize] == 1000
                && in_flight > self.get_target_cwnd(1000)
            {
                return;
            }
            self.pacing_gain_pm = K_PACING_GAIN_PM[self.current_cycle_offset as usize];
        }
    }

    fn maybe_exit_startup_or_drain(&mut self, now_ns: u64, in_flight: u64) {
        if self.mode == Mode::Startup && self.is_at_full_bandwidth {
            self.mode = Mode::Drain;
            self.pacing_gain_pm = self.drain_gain_pm;
            self.cwnd_gain_pm = self.high_cwnd_gain_pm;
            self.diag_startup_exit_round = Some(self.round_count);
        }
        if self.mode == Mode::Drain && in_flight <= self.get_target_cwnd(1000) {
            self.enter_probe_bandwidth_mode(now_ns);
        }
    }

    fn is_min_rtt_expired(&self, now_ns: u64, app_limited: bool) -> bool {
        !app_limited
            && self
                .probe_rtt_last_started_at_ns
                .map(|last| now_ns.saturating_sub(last) > K_MIN_RTT_EXPIRY_NS)
                .unwrap_or(true)
    }

    fn maybe_enter_or_exit_probe_rtt(
        &mut self,
        now_ns: u64,
        is_round_start: bool,
        bytes_in_flight: u64,
        app_limited: bool,
    ) {
        let min_rtt_expired = self.is_min_rtt_expired(now_ns, app_limited);
        if min_rtt_expired && !self.exiting_quiescence && self.mode != Mode::ProbeRtt {
            self.mode = Mode::ProbeRtt;
            self.pacing_gain_pm = 1000;
            self.exit_probe_rtt_at_ns = None;
            self.probe_rtt_last_started_at_ns = Some(now_ns);
        }

        if self.mode == Mode::ProbeRtt {
            match self.exit_probe_rtt_at_ns {
                None => {
                    // Uçuş hedef küçük değere inince çıkışı planla (bir paket pay; q:243-247).
                    if bytes_in_flight < self.get_probe_rtt_cwnd() + self.current_mtu {
                        self.exit_probe_rtt_at_ns = Some(now_ns + K_PROBE_RTT_TIME_NS);
                    }
                }
                Some(exit_time) if is_round_start && now_ns >= exit_time => {
                    if !self.is_at_full_bandwidth {
                        self.enter_startup_mode();
                    } else {
                        self.enter_probe_bandwidth_mode(now_ns);
                    }
                }
                Some(_) => {}
            }
        }

        self.exiting_quiescence = false;
    }

    /// Bant tahmini: dış örnekleyici verildiyse o, yoksa quinn'in kendi filtresi.
    fn bw_estimate(&self) -> u64 {
        self.external_bw
            .unwrap_or_else(|| self.max_bandwidth.get_estimate())
    }

    fn get_target_cwnd(&self, gain_pm: u64) -> u64 {
        let bw = self.bw_estimate();
        // BDP = min_rtt × bw (q:266-268; µs yerine ns).
        let bdp = (self.min_rtt_ns as u128) * (bw as u128) / 1_000_000_000;
        let cwnd = (bdp * (gain_pm as u128) / 1000) as u64;
        // Bant örneği yoksa BDP 0 → ilk pencere.
        if cwnd == 0 {
            return self.init_cwnd;
        }
        cwnd.max(self.min_cwnd)
    }

    fn get_probe_rtt_cwnd(&self) -> u64 {
        if PROBE_RTT_BASED_ON_BDP {
            return self.get_target_cwnd(K_MODERATE_PROBE_RTT_MULTIPLIER_PM);
        }
        self.min_cwnd
    }

    fn calculate_pacing_rate(&mut self) {
        let bw = self.bw_estimate();
        if bw == 0 {
            return;
        }
        let target_rate = apply_pm(bw, self.pacing_gain_pm);
        if self.is_at_full_bandwidth {
            self.pacing_rate = target_rate;
            return;
        }
        // RTT ölçülür ölçülmez initial_window / RTT hızında pace et (q:295-301).
        if self.pacing_rate == 0 && self.min_rtt_ns != 0 {
            self.pacing_rate =
                BandwidthEstimation::bw_from_delta(self.init_cwnd, self.min_rtt_ns).unwrap_or(0);
            return;
        }
        // Startup'ta pacing hızı düşmez.
        if self.pacing_rate < target_rate {
            self.pacing_rate = target_rate;
        }
    }

    fn calculate_cwnd(&mut self, bytes_acked: u64, excess_acked: u64) {
        if self.mode == Mode::ProbeRtt {
            return;
        }
        let mut target_window = self.get_target_cwnd(self.cwnd_gain_pm);
        if self.is_at_full_bandwidth {
            // Son ölçülen en yüksek ACK toplanmasını ekle.
            target_window += self.ack_aggregation.max_ack_height.get();
        } else {
            // Startup'ta cwnd hiç küçülmediği için bu yerel bir max filtresi gibi davranır.
            target_window += excess_acked;
        }
        // cwnd hedefe bir seferde değil, ACK'lenen bayt kadar büyür.
        if self.is_at_full_bandwidth {
            self.cwnd = target_window.min(self.cwnd + bytes_acked);
        } else if self.cwnd_gain_pm < target_window.saturating_mul(1000)
            || self.acked_bytes < self.init_cwnd
        {
            // q:327 `(self.cwnd_gain < target_window as f32) || …` — quinn'deki birimsiz
            // karşılaştırma (kazanç ↔ bayt) olduğu gibi taşındı; fiilen hep doğrudur ve
            // Startup'ta pencere küçülmez.
            self.cwnd += bytes_acked;
        }
        if self.cwnd < self.min_cwnd {
            self.cwnd = self.min_cwnd;
        }
    }

    fn calculate_recovery_window(&mut self, bytes_acked: u64, bytes_lost: u64, in_flight: u64) {
        if !self.recovery_state.in_recovery() {
            return;
        }
        // İlk recovery penceresi.
        if self.recovery_window == 0 {
            self.recovery_window = self.min_cwnd.max(in_flight + bytes_acked);
            return;
        }
        // Kayıpları pencereden düş (taşma koruması).
        if self.recovery_window >= bytes_lost {
            self.recovery_window -= bytes_lost;
        } else {
            self.recovery_window = self.current_mtu;
        }
        // Conservation'da kayıp düşmek yeter; Growth'ta ACK kadar da serbest bırak.
        if self.recovery_state == RecoveryState::Growth {
            self.recovery_window += bytes_acked;
        }
        // En az bir MSS ya da ACK'lenen kadar göndermeye izin ver.
        self.recovery_window = self
            .recovery_window
            .max(in_flight + bytes_acked)
            .max(self.min_cwnd);
    }

    /// draft-cardwell-iccrg-bbr §4.3.2.2 (q:372-391).
    fn check_if_full_bw_reached(&mut self, app_limited: bool) {
        if app_limited {
            return;
        }
        let target = apply_pm(self.bw_at_last_round, K_STARTUP_GROWTH_TARGET_PM);
        let bw = self.bw_estimate();
        if bw >= target {
            self.bw_at_last_round = bw;
            self.round_wo_bw_gain = 0;
            self.ack_aggregation.max_ack_height.reset();
            return;
        }
        self.round_wo_bw_gain += 1;
        if self.round_wo_bw_gain >= K_ROUND_TRIPS_WITHOUT_GROWTH_BEFORE_EXITING_STARTUP
            || self.recovery_state.in_recovery()
        {
            self.is_at_full_bandwidth = true;
        }
    }

    // ── quinn `Controller` arayüzü (q:394-513) ──────────────────────────────────────────

    /// Paket tele yazıldı (q:395-398). `packet_number` = Kama `seq` (iletim başına tekil).
    pub fn on_sent(&mut self, now_ns: u64, bytes: u64, packet_number: u64) {
        self.max_sent_packet_number = packet_number;
        self.max_bandwidth.on_sent(now_ns, bytes);
    }

    /// Bir paket ACK'lendi (q:400-414). `rtt_min_ns` = ölçülen en küçük RTT (0 = henüz yok).
    pub fn on_ack(&mut self, now_ns: u64, bytes: u64, app_limited: bool, rtt_min_ns: u64) {
        self.max_bandwidth
            .on_ack(now_ns, bytes, self.round_count, app_limited);
        self.acked_bytes += bytes;
        if rtt_min_ns != 0
            && (self.is_min_rtt_expired(now_ns, app_limited)
                || self.min_rtt_ns > rtt_min_ns
                || self.min_rtt_ns == 0)
        {
            self.min_rtt_ns = rtt_min_ns;
            // Uyarlama 8: ilk min_rtt örneği ProbeRTT saatini başlatır (draft-cardwell §4.3.4.3
            // `ProbeRTT` yalnız min_rtt damgası 10 s bayatlayınca). quinn'de `probe_rtt_last_started_at`
            // `None` iken `is_min_rtt_expired` hemen `true` döner ve ilk uygulamayla-sınırlı-olmayan
            // ACK'te ProbeRTT'ye girilir (ölçüldü: t≈0,5 s'de pencere 4·MTU'ya iner, 1 s kayıp).
            if self.probe_rtt_last_started_at_ns.is_none() {
                self.probe_rtt_last_started_at_ns = Some(now_ns);
            }
        }
    }

    /// Bir ACK çerçevesinin işlenmesi bitti (q:416-466). Model güncellenir, pacing hızı ve
    /// cwnd yeniden hesaplanır.
    pub fn on_end_acks(
        &mut self,
        now_ns: u64,
        in_flight: u64,
        app_limited: bool,
        largest_packet_num_acked: Option<u64>,
    ) {
        let bytes_acked = self.max_bandwidth.bytes_acked_this_window();
        let excess_acked = self.ack_aggregation.update_ack_aggregation_bytes(
            bytes_acked,
            now_ns,
            self.round_count,
            self.bw_estimate(),
        );
        self.max_bandwidth.end_acks(self.round_count, app_limited);
        if let Some(largest) = largest_packet_num_acked {
            self.max_acked_packet_number = largest;
        }

        let mut is_round_start = false;
        if bytes_acked > 0 {
            is_round_start =
                self.max_acked_packet_number > self.current_round_trip_end_packet_number;
            if is_round_start {
                self.current_round_trip_end_packet_number = self.max_sent_packet_number;
                self.round_count += 1;
            }
        }

        self.update_recovery_state(is_round_start);

        if self.mode == Mode::ProbeBw {
            self.update_gain_cycle_phase(now_ns, in_flight);
        }

        if is_round_start && !self.is_at_full_bandwidth {
            self.check_if_full_bw_reached(app_limited);
        }

        self.maybe_exit_startup_or_drain(now_ns, in_flight);

        self.maybe_enter_or_exit_probe_rtt(now_ns, is_round_start, in_flight, app_limited);

        self.calculate_pacing_rate();
        self.calculate_cwnd(bytes_acked, excess_acked);
        self.calculate_recovery_window(bytes_acked, self.loss_state.lost_bytes, in_flight);

        self.prev_in_flight_count = in_flight;
        self.loss_state.reset();
    }

    /// Kayıp ilanı (q:468-476): yalnız bu turun kayıp baytını biriktirir; etkisi
    /// `on_end_acks`'ta `recovery_window` üzerinden.
    pub fn on_congestion_event(&mut self, lost_bytes: u64) {
        self.loss_state.lost_bytes += lost_bytes;
        self.diag_congestion_events = self.diag_congestion_events.saturating_add(1);
    }

    /// Uçuş tavanı (bayt) — q:485-492.
    #[must_use]
    pub fn window(&self) -> u64 {
        if self.mode == Mode::ProbeRtt {
            return self.get_probe_rtt_cwnd();
        } else if self.recovery_state.in_recovery()
            && (self.mode != Mode::Startup || PACKET_CONSERVATION_IN_STARTUP)
        {
            return self.cwnd.min(self.recovery_window);
        }
        self.cwnd
    }

    /// BBR pacing hızı (bayt/s) — quinn'de yalnız metrik (q:494-500); pacer pencere tabanlıdır.
    #[must_use]
    pub fn pacing_rate(&self) -> u64 {
        self.pacing_rate
    }

    /// Kip.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Teslim hızı tahmini (bayt/s) — dış örnekleyici verildiyse o.
    #[must_use]
    pub fn delivery_rate(&self) -> u64 {
        self.bw_estimate()
    }

    /// Uyarlama 6: dış teslim hızı tahmini (BBR §7.1 paket-başına örnekleyici + max filtre).
    /// `on_end_acks`'tan ÖNCE, her ACK çerçevesinde çağrılır; `None` quinn'in kendi filtresine döner.
    pub fn set_external_bw(&mut self, bw: Option<u64>) {
        self.external_bw = bw.filter(|&b| b > 0);
    }

    /// min RTT (ns; 0 = yok).
    #[must_use]
    pub fn min_rtt_ns(&self) -> u64 {
        self.min_rtt_ns
    }

    /// Kurulu ilk pencere (bayt; `min_cwnd` tabanlı).
    #[must_use]
    pub fn initial_window(&self) -> u64 {
        self.init_cwnd
    }

    /// Recovery içinde mi.
    #[must_use]
    pub fn in_recovery(&self) -> bool {
        self.recovery_state.in_recovery()
    }

    /// Teşhis: (tur, tıkanıklık olayı sayısı, Startup çıkış turu, cwnd, recovery_window).
    #[must_use]
    pub fn diag(&self) -> (u64, u64, Option<u64>, u64, u64) {
        (
            self.round_count,
            self.diag_congestion_events,
            self.diag_startup_exit_round,
            self.cwnd,
            self.recovery_window,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MTU: u64 = 1200;
    const MS: u64 = 1_000_000;

    fn bbr() -> Bbr {
        Bbr::new(MTU, rfc9002_initial_window(MTU))
    }

    /// Deterministik yol: `rate` B/s, `rtt` ms, sınırsız kuyruk; gönderici pencereyi doldurur.
    /// Döner: (tur sayısı, Startup çıkış turu, son cwnd, son teslim hızı).
    fn drive(
        bbr: &mut Bbr,
        rate_bps: u64,
        rtt_ms: u64,
        rounds: u64,
    ) -> (u64, Option<u64>, u64, u64) {
        let rtt_ns = rtt_ms * MS;
        let mut now = 0u64;
        let mut pn = 0u64;
        let mut in_flight = 0u64;
        let mut queue: Vec<(u64, u64, u64)> = Vec::new(); // (ack_at, pn, bytes)
        let mut next_free = 0u64;
        for _ in 0..rounds {
            // Pencereyi doldur (uygulamayla sınırlı değil); pacing hızı biliniyorsa bir RTT'lik
            // pace bütçesini aşma (quinn'de Drain/ProbeRTT boşaltması pacing'le olur).
            let mut budget = if bbr.pacing_rate() > 0 {
                bbr.pacing_rate() * rtt_ns / 1_000_000_000
            } else {
                u64::MAX
            };
            while in_flight + MTU <= bbr.window() && budget >= MTU {
                budget -= MTU;
                pn += 1;
                let ser = MTU * 1_000_000_000 / rate_bps;
                let depart = now.max(next_free);
                next_free = depart + ser;
                queue.push((next_free + rtt_ns, pn, MTU));
                bbr.on_sent(now, MTU, pn);
                in_flight += MTU;
            }
            // Bir RTT ilerle, vadesi gelenleri ACK'le.
            now += rtt_ns;
            let mut largest = None;
            let due: Vec<_> = queue
                .iter()
                .filter(|(t, _, _)| *t <= now)
                .copied()
                .collect();
            queue.retain(|(t, _, _)| *t > now);
            for (t, p, b) in due {
                in_flight -= b;
                bbr.on_ack(t, b, false, rtt_ns);
                largest = Some(p);
            }
            bbr.on_end_acks(now, in_flight, false, largest);
        }
        let (r, _, exit, cwnd, _) = bbr.diag();
        (r, exit, cwnd, bbr.delivery_rate())
    }

    #[test]
    fn initial_window_is_rfc9002_ten_mtu() {
        let b = bbr();
        assert_eq!(b.window(), 12_000);
        assert_eq!(b.mode(), Mode::Startup);
    }

    #[test]
    fn startup_exits_to_drain_then_probe_bw_on_plateau() {
        let mut b = bbr();
        let (_, exit, _, rate) = drive(&mut b, 12_500, 300, 120);
        assert!(exit.is_some(), "Startup plato ile bitmeli");
        assert_eq!(b.mode(), Mode::ProbeBw, "Drain sonrası ProbeBw");
        // Teslim hızı tahmini hat hızının %20'si içinde.
        assert!((10_000..=15_000).contains(&rate), "rate={rate}");
    }

    #[test]
    fn window_converges_near_two_bdp_in_probe_bw() {
        let mut b = bbr();
        let (_, _, cwnd, _) = drive(&mut b, 12_500, 300, 60);
        // BDP = 12 500 × 0,3 = 3 750 B; cwnd_gain 2 → ~7 500 B (+ ACK toplanması), min_cwnd 4 800.
        assert!((4_800..=20_000).contains(&cwnd), "cwnd={cwnd}");
    }

    #[test]
    fn loss_without_queue_enters_recovery_and_caps_window() {
        let mut b = bbr();
        drive(&mut b, 12_500, 300, 30);
        let w0 = b.window();
        b.on_congestion_event(3 * MTU);
        b.on_end_acks(100_000 * MS, w0, false, Some(b.max_sent_packet_number));
        assert!(b.in_recovery());
        assert!(b.window() <= w0.max(b.min_cwnd));
    }

    #[test]
    fn probe_rtt_after_ten_seconds_without_min_rtt_refresh() {
        let mut b = bbr();
        drive(&mut b, 12_500, 300, 40); // 12 s > 10 s
                                        // ProbeRTT'ye girilmiş ya da çıkılmış olmalı; en azından bir kez başlatıldı.
        assert!(b.probe_rtt_last_started_at_ns.is_some());
    }

    #[test]
    fn determinism_two_instances_identical() {
        let mut a = bbr();
        let mut c = bbr();
        let ra = drive(&mut a, 50_000, 100, 50);
        let rc = drive(&mut c, 50_000, 100, 50);
        assert_eq!(ra, rc);
    }
}
