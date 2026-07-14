//! TV-BW: delivered-rate bandwidth estimator (engine spec §7).
//!
//! **bandwidth = delivered-rate'ten ÖLÇÜLÜR, loss'tan ASLA türetilmez**
//! (INV-ENGINE-BW; INV-RATE-1 ailesi). BBR delivery-rate sampler (§7.1) + max-filter
//! BtlBw (§7.2). Kısa-interval / app-limited / dış-flag sample'lar validity-gate'lenir
//! (§7.3). Saf/numerik + deterministik (ADR-17): monoton u64 ns zaman damgaları,
//! integer bytes/sec.

use std::time::Duration;

/// BtlBw max-filter penceresi `[STD: BBR ~10 RTT]`.
const W_BTLBW: Duration = Duration::from_secs(10);
/// Çok kısa interval → güvenilmez sample `[STD: BBR ~1ms]`.
const MIN_RATE_INTERVAL: Duration = Duration::from_millis(1);

/// `Duration` → u64 nanosecond.
fn dur_ns(d: Duration) -> u64 {
    d.as_nanos() as u64
}

/// Gönderim anında bir pakete iliştirilen delivery-rate snapshot'ı (BBR §7.1). Opak
/// token: alanları private, dışarıdan üretilemez.
#[derive(Debug, Clone, Copy)]
pub struct SentPacket {
    size: u64,
    delivered: u64,
    delivered_time_ns: u64,
    first_sent_ns: u64,
    sent_ns: u64,
    is_app_limited: bool,
    /// [TUR-D1 F3] Gönderim-ANI pacing-gain damgası (gain ≥ 1 fazı): shrink-dedektörü
    /// öz-kısma (Drain/DrainRtprop/PreConverge) örneklerini ack-anı-fazından BAĞIMSIZ eler
    /// (Drain-kohortu Steady'de ack'lenir — faz-sınırı-geçen-ack vakası).
    gain_ge1: bool,
}

/// Bir ACK'ten türeyen delivery-rate örneği (engine §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateSample {
    /// Bu interval'de teslim edilen byte.
    pub delivered: u64,
    /// Örnekleme interval'i = max(ack_elapsed, send_elapsed).
    pub interval: Duration,
    /// Snapshot app-limited miydi (§7.3).
    pub is_app_limited: bool,
    /// [TUR-D1 F3] Gönderim-anı gain≥1 damgası (SentPacket'ten taşınır).
    pub gain_ge1: bool,
    /// [TUR-D1 P1] ack_elapsed = now − pkt.delivered_time (taze-snapshot kapısının girdisi;
    /// blackout-yayılımlı örnek ack_elapsed ≫ ack_epoch ile ayrışır).
    pub ack_elapsed: Duration,
    /// [TUR-D1 F2] Paketin gönderim-anı (yaş-kapısı: sent_ns ≥ olay-başı last_raise_ns).
    pub sent_ns: u64,
}

/// BBR delivery-rate sampler (engine §7.1).
#[derive(Debug, Clone, Default)]
pub struct DeliveryRateSampler {
    delivered: u64,
    delivered_time_ns: u64,
    first_sent_ns: u64,
    packets_in_flight: u64,
}

impl DeliveryRateSampler {
    /// Boş sampler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Paket gönderimi → snapshot token (engine §7.1). Pipe boşken first_sent /
    /// delivered_time tazelenir.
    pub fn on_sent(
        &mut self,
        now_ns: u64,
        size: u64,
        app_limited: bool,
        gain_ge1: bool,
    ) -> SentPacket {
        if self.packets_in_flight == 0 {
            self.first_sent_ns = now_ns;
            self.delivered_time_ns = now_ns;
        }
        self.packets_in_flight += 1;
        SentPacket {
            size,
            delivered: self.delivered,
            delivered_time_ns: self.delivered_time_ns,
            first_sent_ns: self.first_sent_ns,
            sent_ns: now_ns,
            is_app_limited: app_limited,
            gain_ge1,
        }
    }

    /// Paket ACK'i → delivery-rate örneği (engine §7.1).
    /// `interval = max(ack_elapsed, send_elapsed)`, `delivered = C.delivered − P.delivered`.
    pub fn on_ack(&mut self, now_ns: u64, pkt: SentPacket) -> RateSample {
        self.delivered += pkt.size;
        self.delivered_time_ns = now_ns;
        self.packets_in_flight = self.packets_in_flight.saturating_sub(1);

        let ack_elapsed = now_ns.saturating_sub(pkt.delivered_time_ns);
        let send_elapsed = pkt.sent_ns.saturating_sub(pkt.first_sent_ns);
        let interval_ns = ack_elapsed.max(send_elapsed);
        self.first_sent_ns = pkt.sent_ns; // sonraki interval'in tabanı

        RateSample {
            delivered: self.delivered - pkt.delivered,
            interval: Duration::from_nanos(interval_ns),
            is_app_limited: pkt.is_app_limited,
            gain_ge1: pkt.gain_ge1,
            ack_elapsed: Duration::from_nanos(ack_elapsed),
            sent_ns: pkt.sent_ns,
        }
    }

    /// Kümülatif delivered (bytes) — [TUR-3c] probe-volume ölçümü (delivered_since_entry).
    #[must_use]
    pub fn delivered(&self) -> u64 {
        self.delivered
    }

    /// Uçuşta-sayılan iletim sayısı (SALT-GÖZLEM — not §3.7 sızıntı-borcu ölçümü [TUR-3a]).
    #[must_use]
    pub fn packets_in_flight(&self) -> u64 {
        self.packets_in_flight
    }

    /// [TUR-3a commit-B, §3.7 borç-fix] ACK'lenmeyecek iletimi uçuş-sayısından düşür:
    /// (a) tombstone TTL-prune, (b) kapasite-eviction, (c) Duplicate-kökenli kayıp (tombstonesuz).
    /// Sızıntı her gerçek-kayıpta +1 kalıcıydı → `packets_in_flight==0` pipe-boş tespiti
    /// (on_sent first_sent/delivered_time yeniden-çıpalaması) kayıplı koşuda bir daha
    /// tetiklenmiyordu (ölçüldü: p5'te pif=23 vs gerçek-uçuş≈14).
    pub fn forget_in_flight(&mut self) {
        self.packets_in_flight = self.packets_in_flight.saturating_sub(1);
    }

    /// [TUR-2c D-1/3 kol-2] Rate-örneği ÜRETMEDEN delivered-kredisi: superseded (retx'li)
    /// iletimin GEÇ-ACK'i — byte'lar wire'ı gerçekten geçti → kümülatif delivered'a GİRER
    /// (teslimat gerçek; gelecek örneklerin delta'sına dürüst yansır) ama muhafazakâr Karn
    /// kuralı gereği BU ack'ten örnek çıkmaz. packets_in_flight kol-1'le simetrik düşer.
    pub fn credit_delivered(&mut self, now_ns: u64, size: u64) {
        self.delivered += size;
        self.delivered_time_ns = now_ns;
        self.packets_in_flight = self.packets_in_flight.saturating_sub(1);
    }
}

/// BtlBw max-filter (engine §7.2/§7.3): geçerli sample'ların pencereli max'ı.
///
/// [TUR-3a D-2/1+2] Pencere KANAL-TÜREVİ (sender `set_window` ile `max(10s, K_bw×ack_epoch)`
/// yazar — ZM-2: wall-clock sabiti sub-kbps'te 0.8-3.3 RTT'ye düşüyordu, btlbw 88→4 sınıfı) +
/// KEEP-LAST (son örnek ASLA expire etmez → `valid=false` güncellemelerde budamanın filtreyi
/// boşaltıp btlbw=0 flap'i üretmesi yapısal biter).
#[derive(Debug, Clone)]
pub struct BtlBwFilter {
    /// (time_ns, bw_bps) örnekleri; pencereli max.
    samples: Vec<(u64, u64)>,
    /// Pencere (ns) — sender kanal-türevi yazar; taban W_BTLBW [STD].
    window_ns: u64,
}

impl Default for BtlBwFilter {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            window_ns: dur_ns(W_BTLBW),
        }
    }
}

impl BtlBwFilter {
    /// Boş filter (pencere = W_BTLBW [STD] tabanı).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// [TUR-3a D-2/1] Kanal-türevi pencere: `max(10s, K_bw × ack_epoch)` — sender yazar.
    pub fn set_window(&mut self, w: Duration) {
        self.window_ns = dur_ns(w).max(dur_ns(W_BTLBW)); // taban [STD]: küçültme yok
    }

    /// Sample'ı validity-gate'leyip max-filter'ı günceller; güncel BtlBw'i (bps) döner.
    ///
    /// `valid=false` (reverse_path_impaired / ack_delay_untrusted — §7.3 dış flag'ler)
    /// veya `interval < MIN_RATE_INTERVAL` → sample max-filter'a GİRMEZ. app-limited
    /// sample yalnız mevcut BtlBw'den ≥ ise girer (yüksek app-limited = geçerli
    /// alt-sınır; §7.3).
    pub fn update(&mut self, sample: RateSample, now_ns: u64, valid: bool) -> u64 {
        if valid && sample.interval >= MIN_RATE_INTERVAL {
            let bw = bw_bps(sample.delivered, sample.interval);
            let app_limited_ok = !sample.is_app_limited || bw >= self.btlbw();
            if app_limited_ok {
                self.samples.push((now_ns, bw));
            }
        }
        let cutoff = now_ns.saturating_sub(self.window_ns);
        // [TUR-3a D-2/2] KEEP-LAST: budama filtreyi boşaltacaksa en-yeni örnek yaşar.
        let last = self.samples.last().copied();
        self.samples.retain(|&(t, _)| t >= cutoff);
        if self.samples.is_empty() {
            if let Some(l) = last {
                self.samples.push(l);
            }
        }
        self.btlbw()
    }

    /// Güncel BtlBw (bytes/sec); sample yoksa 0.
    #[must_use]
    pub fn btlbw(&self) -> u64 {
        self.samples.iter().map(|&(_, v)| v).max().unwrap_or(0)
    }

    /// [TUR-D1 aktüatör — YALNIZ-DÜŞÜRÜR] Shrink-reset: pencere, tetikleyen LOW-olayların
    /// eligible-düşük-örneklerinin MAX'ına reseed edilir (S2-daraltılmış; tek-örnek-reseed'in
    /// outlier-çökmesi yok). Yükseltme-yolu (`update`) BİT-DEĞİŞMEZ — bu metot ayrı-giriş.
    /// Çağıran (sender) generation++ ve diag'ı yönetir.
    pub fn shrink_reset(&mut self, bw: u64, now_ns: u64) {
        self.samples.clear();
        self.samples.push((now_ns, bw.max(1)));
    }
}

/// bw_sample = delivered · 1e9 / interval (bytes/sec, integer truncation). interval 0
/// ise 0.
#[must_use]
pub fn bw_bps(delivered: u64, interval: Duration) -> u64 {
    let ns = dur_ns(interval);
    if ns == 0 {
        return 0;
    }
    (u128::from(delivered) * 1_000_000_000 / u128::from(ns)) as u64
}
