//! TV-LOSS: loss detection (engine §5). loss = RECOVERY sinyali, RATE sinyali DEĞİL
//! (INV-RATE-1). Packet-threshold (K=3 [STD:BBR/QUIC RFC 9002]) + time-threshold
//! (9/8·srtt [STD], min_delay [CAL] parametre). loss_pre_fec (recovery ÖNCESİ) ve
//! residual_loss (recovery SONRASI) ayrı sayaçlar.
//!
//! Clean-arch: [`LossDetector`] trait = port (füzyon §8.4 bunun üzerinden loss_pre_fec
//! okuyacak); [`ThresholdLossDetector`] = RFC 9002-türevi adapter. FEC/ARQ execution
//! (§9/§10) + W_loss sliding window (§5 [CAL]) sonraki adımlarda.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

/// Packet-threshold K [STD:BBR/QUIC RFC 9002 = 3].
pub const PACKET_THRESHOLD: u64 = 3;
/// [TUR-2a D-1/3] Ledger + sent-pencere kayıt tavanı [CAL] — SR-AUDIT-001 bounded
/// (tombstone MAX'la aynı aile; en-eski-önce eviction + audit sayacı).
pub const LEDGER_MAX: usize = 4096;
/// Time-threshold pay [STD:RFC 9002 = 9].
pub const TIME_THRESH_NUM: u64 = 9;
/// Time-threshold payda [STD:RFC 9002 = 8].
pub const TIME_THRESH_DEN: u64 = 8;

/// Packet-threshold: `largest_acked − seq ≥ k` (RFC 9002 §6.1.1). K altı = reorder
/// (bekle), K üstü = kayıp.
#[must_use]
pub fn packet_threshold_lost(seq: u64, largest_acked: u64, k: u64) -> bool {
    largest_acked.saturating_sub(seq) >= k
}

/// Time-threshold: `elapsed > max(num/den·srtt, min_delay)` (RFC 9002 §6.1.2; §5).
/// `min_delay` [CAL] parametre — reordering tail + tiny-srtt koruması.
#[must_use]
pub fn time_threshold_lost(
    elapsed: Duration,
    srtt: Duration,
    min_delay: Duration,
    num: u64,
    den: u64,
) -> bool {
    elapsed > scale_duration(srtt, num, den).max(min_delay)
}

fn scale_duration(d: Duration, num: u64, den: u64) -> Duration {
    let ns = d.as_nanos() * u128::from(num) / u128::from(den);
    Duration::from_nanos(ns as u64)
}

fn ratio_ppm(num: u64, den: u64) -> u64 {
    if den == 0 {
        return 0;
    }
    (u128::from(num) * 1_000_000 / u128::from(den)) as u64
}

/// Loss detection port (clean-arch). Füzyon (§8.4) `loss_pre_fec`'i buradan okur;
/// loss sayaçları send_rate'e GİRMEZ (INV-RATE-1).
pub trait LossDetector {
    /// Paket gönderimini kaydet.
    fn on_sent(&mut self, seq: u64, send_ns: u64, units: u64);
    /// ACK işle; yeni packet-threshold kayıplarını (artan seq) döner.
    /// [TUR-2a] `now_ns`: ledger ilan-damgası (Suspected) + un-loss tespiti için;
    /// geri-alınan seq'ler [`ThresholdLossDetector::take_unlossed`] ile drain edilir.
    fn on_ack(&mut self, acked: &[u64], now_ns: u64) -> Vec<u64>;
    /// Time-threshold kayıplarını `now`da ilan et; yeni-kayıp seq'leri döner.
    /// [TUR-1b D-1/2] `cadence_floor` = beklenen-ACK-aralığı türevi dinamik taban:
    /// eşik = max(num/den·srtt, min_delay, cadence_floor) — eşiğin saati elapsed'in
    /// içindeki ACK-bekleme sınıfını da temsil eder (ZM-4).
    fn detect_time_losses(
        &mut self,
        now_ns: u64,
        srtt: Duration,
        cadence_floor: Duration,
    ) -> Vec<u64>;
    /// Recovery sonrası kurtarılamayan birim bildir (residual_loss'u besler).
    fn on_unrecovered(&mut self, units: u64);
    /// loss_pre_fec = lost/sent (ppm; recovery ÖNCESİ).
    fn loss_pre_fec_ppm(&self) -> u64;
    /// residual_loss = unrecovered/sent (ppm; recovery SONRASI).
    fn residual_loss_ppm(&self) -> u64;
}

/// Loss detector yapılandırması.
#[derive(Debug, Clone, Copy)]
pub struct LossConfig {
    /// K [STD:3].
    pub packet_threshold: u64,
    /// Time-threshold pay [STD:9].
    pub time_num: u64,
    /// Time-threshold payda [STD:8].
    pub time_den: u64,
    /// min_delay [CAL] parametre (default = RFC kGranularity ~1ms floor).
    pub min_delay: Duration,
}

impl Default for LossConfig {
    fn default() -> Self {
        Self {
            packet_threshold: PACKET_THRESHOLD,
            time_num: TIME_THRESH_NUM,
            time_den: TIME_THRESH_DEN,
            min_delay: Duration::from_millis(1),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sent {
    send_ns: u64,
    units: u64,
}

/// [TUR-2a D-1/3] Loss olay-ledger durumu: kayıp bir OLAY değil HİPOTEZ — geç kanıtla çürüyebilir.
/// `Suspected → Confirmed | Spurious`; pencere-dışına yaşlanan olay ledger'dan düşer (Expired).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerState {
    /// İlan edildi; un-loss ufku (confirm_age) içinde geç-ACK'le çürüyebilir.
    Suspected,
    /// Ufuk kanıtsız geçti → pencere-içi loss_ppm'e sayılır.
    Confirmed,
    /// Geç-ACK geldi → GERİ-ALINDI (un-loss); loss_ppm'e SAYILMAZ.
    Spurious,
}

/// Ledger kaydı (declared_ns artan sırada tutulur — pencere budaması front'tan).
#[derive(Debug, Clone, Copy)]
struct LossEvent {
    seq: u64,
    units: u64,
    declared_ns: u64,
    state: LedgerState,
}

/// RFC 9002-türevi packet/time-threshold loss detector ([`LossDetector`] adapter).
#[derive(Debug, Clone, Default)]
pub struct ThresholdLossDetector {
    cfg: LossConfig,
    inflight: BTreeMap<u64, Sent>,
    largest_acked: u64,
    have_acked: bool,
    sent_units: u64,
    lost_units: u64,
    unrecovered_units: u64,
    // ── [TUR-2a D-1/3, D-1/5] olay-ledger + pencere-türetimli loss_ppm ──
    events: VecDeque<LossEvent>, // declared_ns artan; LEDGER_MAX bounded
    sent_window: VecDeque<Sent>, // pencere-içi gönderimler (payda); LEDGER_MAX bounded
    sent_window_units: u64,      // payda toplamı (O(1))
    unlossed: Vec<u64>,          // son on_ack'te Spurious'a dönen seq'ler (sender drain'ler)
    spurious_units_total: u64,   // audit: toplam geri-alınan birim
    ledger_evicted_total: u64,   // audit: kapasite-eviction (SR-AUDIT-001)
    packet_thresh_dyn: u64,      // 0 = cfg.packet_threshold; sender adaptif k yazar [TUR-2a]
}

impl ThresholdLossDetector {
    /// Default config ([STD] K=3, 9/8; min_delay 1ms).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Verilen config ile.
    #[must_use]
    pub fn with_config(cfg: LossConfig) -> Self {
        Self {
            cfg,
            ..Self::default()
        }
    }

    /// Toplam gönderilen birim.
    #[must_use]
    pub fn sent_units(&self) -> u64 {
        self.sent_units
    }
    /// Kayıp ilan edilen toplam birim (recovery öncesi).
    #[must_use]
    pub fn lost_units(&self) -> u64 {
        self.lost_units
    }
    /// Kurtarılamayan toplam birim (recovery sonrası).
    #[must_use]
    pub fn unrecovered_units(&self) -> u64 {
        self.unrecovered_units
    }

    // ── [TUR-2a D-1/3, D-1/5] ledger API'si (inherent — sender somut tipi tutar) ──

    /// Kayıp-ilanını ledger'a Suspected olarak yazar (bounded; en-eski-önce eviction + audit).
    fn declare(&mut self, seq: u64, units: u64, declared_ns: u64) {
        if self.events.len() >= LEDGER_MAX {
            self.events.pop_front();
            self.ledger_evicted_total += 1;
        }
        self.events.push_back(LossEvent {
            seq,
            units,
            declared_ns,
            state: LedgerState::Suspected,
        });
    }

    /// Son `on_ack`'te GERİ-ALINAN (Spurious) seq'leri boşaltır — sender pending-retransmit
    /// iptali + `note_spurious` kablosu + kol-1 RTT adayı için tüketir.
    pub fn take_unlossed(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.unlossed)
    }

    /// Görülen en yüksek acked seq (un-loss'ta gözlenen-reorder mesafesi = largest − seq).
    #[must_use]
    pub fn largest_acked(&self) -> Option<u64> {
        self.have_acked.then_some(self.largest_acked)
    }

    /// [TUR-2a] Adaptif packet-threshold: sender `max(K, ρ×recent_max_reorder)` yazar
    /// (spurious görüldükçe yükselir — §9.3 nihayet kablolandı). 0 = cfg default.
    pub fn set_packet_threshold(&mut self, k: u64) {
        self.packet_thresh_dyn = k;
    }

    /// Ledger bakımı: pencere-dışı olay/gönderim düşer (Expired); un-loss ufku (`confirm_age`)
    /// kanıtsız geçen Suspected → Confirmed. Deterministik (ADR-17); saat-sınıfı: her iki eşik
    /// de kanal-türevi (W_loss = K_wl×ack_epoch; confirm_age = cadence-taban) — ZM-2/ZM-3.
    pub fn ledger_tick(&mut self, now_ns: u64, w_loss: Duration, confirm_age: Duration) {
        let cutoff = now_ns.saturating_sub(w_loss.as_nanos() as u64);
        while self.events.front().is_some_and(|e| e.declared_ns < cutoff) {
            self.events.pop_front(); // Expired — loss_ppm'den doğal decay (ratchet ölümü)
        }
        while self.sent_window.front().is_some_and(|s| s.send_ns < cutoff) {
            let s = self.sent_window.pop_front().expect("front var");
            self.sent_window_units = self.sent_window_units.saturating_sub(s.units);
        }
        let conf_ns = confirm_age.as_nanos() as u64;
        for e in &mut self.events {
            if e.state == LedgerState::Suspected && now_ns.saturating_sub(e.declared_ns) >= conf_ns
            {
                e.state = LedgerState::Confirmed;
            }
        }
    }

    /// [D-1/3] Pencere-türetimli loss oranı: `Confirmed_units / pencere-içi-gönderim` (ppm).
    /// Spurious SAYILMAZ (un-loss); pencere-dışı olay düşer → monoton ratchet YAPISAL biter.
    #[must_use]
    pub fn loss_windowed_ppm(&self) -> u64 {
        let confirmed: u64 = self
            .events
            .iter()
            .filter(|e| e.state == LedgerState::Confirmed)
            .map(|e| e.units)
            .sum();
        ratio_ppm(confirmed, self.sent_window_units)
    }

    /// Audit: toplam geri-alınan (Spurious) birim.
    #[must_use]
    pub fn spurious_units_total(&self) -> u64 {
        self.spurious_units_total
    }
    /// Audit: kapasite-eviction sayısı (SR-AUDIT-001 gözlemi).
    #[must_use]
    pub fn ledger_evicted_total(&self) -> u64 {
        self.ledger_evicted_total
    }

    fn effective_packet_thresh(&self) -> u64 {
        if self.packet_thresh_dyn == 0 {
            self.cfg.packet_threshold
        } else {
            self.packet_thresh_dyn
        }
    }
}

impl LossDetector for ThresholdLossDetector {
    fn on_sent(&mut self, seq: u64, send_ns: u64, units: u64) {
        self.inflight.insert(seq, Sent { send_ns, units });
        self.sent_units += units;
        // [TUR-2a D-1/5] pencere-paydası (bounded; eviction paydayı küçültür → ppm hafif
        // yukarı-yanlı olur, SAKLANMAZ — audit sayacıyla görünür).
        if self.sent_window.len() >= LEDGER_MAX {
            if let Some(old) = self.sent_window.pop_front() {
                self.sent_window_units = self.sent_window_units.saturating_sub(old.units);
                self.ledger_evicted_total += 1;
            }
        }
        self.sent_window.push_back(Sent { send_ns, units });
        self.sent_window_units += units;
    }

    fn on_ack(&mut self, acked: &[u64], now_ns: u64) -> Vec<u64> {
        for &seq in acked {
            self.inflight.remove(&seq);
            // [TUR-2a D-1/3] UN-LOSS: kayıp-ilan-edilmiş seq'e ACK geldi → hipotez ÇÜRÜDÜ.
            // Ledger'da Spurious'a çevrilir (Confirmed dahil — kanıt kanıttır; retransmit-sent
            // belirsizliği SENDER kolunda retx_count'la ayrışır, D-1/3 kol-2/TUR-2c).
            if let Some(ev) = self
                .events
                .iter_mut()
                .rev()
                .find(|e| e.seq == seq && e.state != LedgerState::Spurious)
            {
                ev.state = LedgerState::Spurious;
                self.spurious_units_total += ev.units;
                self.unlossed.push(seq);
            }
            if !self.have_acked || seq > self.largest_acked {
                self.largest_acked = seq;
                self.have_acked = true;
            }
        }
        if !self.have_acked {
            return Vec::new();
        }
        let la = self.largest_acked;
        let k = self.effective_packet_thresh(); // [TUR-2a] adaptif reorder-toleransı (§9.3)
                                                // Önce kayıp seq'leri topla (immutable borç), sonra kaldır.
        let lost: Vec<u64> = self
            .inflight
            .keys()
            .copied()
            .filter(|&seq| packet_threshold_lost(seq, la, k))
            .collect();
        for seq in &lost {
            if let Some(s) = self.inflight.remove(seq) {
                self.lost_units += s.units;
                self.declare(*seq, s.units, now_ns); // ledger: Suspected
            }
        }
        lost
    }

    fn detect_time_losses(
        &mut self,
        now_ns: u64,
        srtt: Duration,
        cadence_floor: Duration,
    ) -> Vec<u64> {
        // [TUR-1a / D-1/1] LATER-ACK PRECONDITION (RFC 9002 §6.1'in atlanmış yarısı): time-threshold
        // KESİN kayıp sınıflandırması YALNIZ "daha yüksek seq ACK'lendi" kanıtıyla (seq < largest_acked).
        // Saf sessizlik loss metriğini ARTIRMAZ — sessizlikte tail-recovery liveness'ın işi (§5.1
        // ack_confidence → RECOVER_FWD spekülatif kopya; loss-sınıflandırması DEĞİL). Bu olmadan
        // SACK-cadence gecikmesi (K-1) temiz kanalda sistematik false-loss üretiyordu (ölçüldü: 27/27).
        // [TUR-1b / D-1/2] CADENCE-FARKINDA TABAN: kanıt olsa bile elapsed, beklenen-ACK-aralığı
        // türevi tabanı (K_c×EAI) aşmadan sınıflandırma YOK — "SACK'i henüz gelmemiş" ile "kayıp"
        // ayrımı (eşik ile elapsed aynı saat-etki-alanına çekilir, ZM-4).
        if !self.have_acked {
            return Vec::new();
        }
        let cfg = self.cfg;
        let floor = cfg.min_delay.max(cadence_floor);
        let la = self.largest_acked;
        let lost: Vec<u64> = self
            .inflight
            .iter()
            .filter(|(&seq, s)| {
                let elapsed = Duration::from_nanos(now_ns.saturating_sub(s.send_ns));
                seq < la && time_threshold_lost(elapsed, srtt, floor, cfg.time_num, cfg.time_den)
            })
            .map(|(&seq, _)| seq)
            .collect();
        for seq in &lost {
            if let Some(s) = self.inflight.remove(seq) {
                self.lost_units += s.units;
                self.declare(*seq, s.units, now_ns); // ledger: Suspected [TUR-2a]
            }
        }
        lost
    }

    fn on_unrecovered(&mut self, units: u64) {
        self.unrecovered_units += units;
    }

    fn loss_pre_fec_ppm(&self) -> u64 {
        ratio_ppm(self.lost_units, self.sent_units)
    }

    fn residual_loss_ppm(&self) -> u64 {
        ratio_ppm(self.unrecovered_units, self.sent_units)
    }
}
