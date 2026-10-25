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

/// [PERSIST] Spurious-yaş halkası boyu (bounded; SR-AUDIT-001). 32: `[u64; N]` için `Default`
/// türetilebilir üst sınır; Q95 için rank 31 → yeterli çözünürlük.
pub const SPURIOUS_AGE_RING: usize = 32;

/// [FEC-RISK v2] Spekülatif kolun girdileri (bkz. [`ThresholdLossDetector::fec_loss_risk_split`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RiskSplit {
    /// Yaş-ağırlıklı Suspected + Confirmed (ppm) — anlık risk.
    pub instant_ppm: u64,
    /// Yalnız ≥ persist_min yaşamış Suspected + Confirmed (ppm) — kalıcı risk.
    pub persistent_ppm: u64,
    /// Penceredeki Spurious birim / tüm olay birimleri (per-mille) — reorder baskısı.
    pub reorder_pressure_permille: u32,
    /// reorder_pressure'ın paydası (küçük-n koruması için).
    pub declared_units: u64,
    /// [EVIDENCE] Ham kanıt kütlesi = paydanın KENDİSİ (`sent_window_units`): rolling W_loss
    /// penceresinde GÖNDERİLMİŞ birimler. Fate'i henüz bilinmeyen (in-flight) birimleri de
    /// İÇERİR; olay üretmiş birimler `declared_units`'tir. Dönüştürülmez, tabanlanmaz.
    pub sent_window_units: u64,
}

/// [FEC-RISK] `Suspected` bir olayın, YAŞINA göre "gerçekten kayıp" güven ağırlığı (per-mille).
///
/// **KALİBRE EDİLMEMİŞ — deneysel.** Bu eğrinin doğru şekli ölçümle belirlenecek; şimdilik
/// basamak fonksiyonu, `[CAL]`. Spec'e YAZILMADI (ölçüm öncesi normatifleştirme yasak).
///
/// Yaş srtt'ye oranlanır: bir paketin geç gelmesi için gereken süre kanalın turudur, bant
/// genişliği değil. Bu ayrım kasıtlıdır — bant çökmesi kayıp KANITI değildir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuspectConfidence {
    /// `age < q25_num/q25_den × srtt` iken ağırlık (per-mille).
    pub w_lt_quarter: u32,
    /// `age < srtt/2` iken ağırlık.
    pub w_lt_half: u32,
    /// `age < srtt` iken ağırlık.
    pub w_lt_one: u32,
    /// `age >= srtt` (ama henüz Confirmed değil) iken ağırlık.
    pub w_ge_one: u32,
}

impl Default for SuspectConfidence {
    /// İlk deneysel şekil (`[CAL]`, kalibre EDİLMEMİŞ): 0.20 / 0.40 / 0.70 / 0.70.
    ///
    /// `age ≥ srtt` bandı `w_lt_one` ile aynı tutulur: olay orada zaten confirm ufkuna doğru
    /// yaşlanıyordur ve onay geldiğinde 1.0'a çıkar. Aradaki bandı 1.0'a çekmek, onay
    /// semantiğini arka kapıdan taklit etmek olurdu.
    fn default() -> Self {
        Self {
            w_lt_quarter: 200,
            w_lt_half: 400,
            w_lt_one: 700,
            w_ge_one: 700,
        }
    }
}

impl SuspectConfidence {
    /// Yaş → ağırlık (per-mille). `srtt_ns > 0` çağıran tarafından garanti edilir.
    #[must_use]
    pub fn for_age(self, age_ns: u64, srtt_ns: u64) -> u32 {
        // Tamsayı karşılaştırma: 4×age < srtt  ⟺  age < srtt/4 (bölme yok, taşma yok).
        if age_ns.saturating_mul(4) < srtt_ns {
            self.w_lt_quarter
        } else if age_ns.saturating_mul(2) < srtt_ns {
            self.w_lt_half
        } else if age_ns < srtt_ns {
            self.w_lt_one
        } else {
            self.w_ge_one
        }
    }
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
    diag_confirmed_total: u64,
    /// [FEC-RISK ENSTRÜMAN] İlk `Suspected` ilanının anı (ns, 0 = hiç). Kayıp KUŞKUSUNUN
    /// ilk anı — FEC'in en erken tepki verebileceği zaman tabanı.
    diag_t_first_suspected_ns: u64,
    /// [FEC-RISK ENSTRÜMAN] İlk `Suspected → Confirmed` geçişinin anı (ns, 0 = hiç).
    diag_t_first_confirmed_ns: u64,
    /// [PERSIST] Un-loss anındaki olay yaşları (declared→Spurious), bounded halka. Reorder
    /// ömrünün ÖLÇÜLEN dağılımı; öğrenilen persistence ufkunun tek kaynağı. Gerçek kayıp asla
    /// Spurious olmadığından bu dağılıma karışmaz.
    spurious_ages: [u64; SPURIOUS_AGE_RING],
    spurious_age_len: usize,
    spurious_age_idx: usize,
    diag_tick_count: u64,
    diag_first_tick_ns: u64,
    diag_last_tick_ns: u64,
    diag_max_susp_age_ns: u64,
    diag_confirm_threshold_ns: u64,
    diag_expire_threshold_ns: u64,
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
        if self.diag_t_first_suspected_ns == 0 {
            self.diag_t_first_suspected_ns = declared_ns;
        }
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

    /// [K4G-05b madde 1] Gönderim kaydını İLAN ETMEDEN unutur (sender'da yalnız-parite kaydının süre
    /// dolumu; [K4G-05c] birim kaydının K_MAP budaması). Yalnız haritadan düşer → sonraki paket/zaman
    /// eşiği taramalarına girmez; `lost_units` / `sent_units` sayaçları, olay-ledger'ı, pencere
    /// paydası ve `largest_acked` DEĞİŞMEZ (ilan yok, `tv_loss` davranışı aynı). Kayıt yoksa NO-OP.
    /// Trait'e eklenmedi: yalnız somut dedektörün haritasına ait bir bakım işlemidir.
    pub fn forget(&mut self, seq: u64) {
        self.inflight.remove(&seq);
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
        // [TEŞHİS] Confirm hiç ateşlemediğinde sebep üç adaydan biridir: olaylar genç · eşik
        // ulaşılamaz · tick geç. `diag_max_susp_age_ns` (en yaşlı Suspected) ile `conf_ns`
        // yan yana okunmadan bu ayrım YAPILAMAZ.
        self.diag_tick_count += 1;
        if self.diag_first_tick_ns == 0 {
            self.diag_first_tick_ns = now_ns;
        }
        self.diag_last_tick_ns = now_ns;
        self.diag_confirm_threshold_ns = conf_ns;
        self.diag_expire_threshold_ns = w_loss.as_nanos() as u64;
        for e in &mut self.events {
            let age = now_ns.saturating_sub(e.declared_ns);
            if e.state == LedgerState::Suspected {
                self.diag_max_susp_age_ns = self.diag_max_susp_age_ns.max(age);
            }
            if e.state == LedgerState::Suspected && age >= conf_ns {
                e.state = LedgerState::Confirmed;
                self.diag_confirmed_total += 1;
                if self.diag_t_first_confirmed_ns == 0 {
                    self.diag_t_first_confirmed_ns = now_ns;
                }
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

    /// [PERSIST] Ölçülen spurious (reorder) yaşlarının `q_permille` quantile'ı (ns); örnek yoksa
    /// `None`. Deterministik: bounded halkanın kopyası sıralanır, rank = ceil(len·q) (ADR-17).
    /// Küçük n'de üst quantile ≈ maksimum → ihtiyatlı (daha uzun ufuk = daha az yanlış L2).
    #[must_use]
    pub fn spurious_age_quantile(&self, q_permille: u32) -> Option<u64> {
        let len = self.spurious_age_len;
        if len == 0 {
            return None;
        }
        let mut buf = [0u64; SPURIOUS_AGE_RING];
        buf[..len].copy_from_slice(&self.spurious_ages[..len]);
        buf[..len].sort_unstable();
        let q = u64::from(q_permille.min(1000));
        let rank = ((len as u64 * q).div_ceil(1000)).clamp(1, len as u64);
        Some(buf[rank as usize - 1])
    }

    /// [PERSIST] Halkadaki spurious-yaş örneği sayısı (≤ [`SPURIOUS_AGE_RING`]).
    #[must_use]
    pub fn spurious_age_samples(&self) -> usize {
        self.spurious_age_len
    }

    /// [FEC-RISK v2] Spekülatif kolun ÜÇ girdisi tek geçişte — instant · persistent · reorder.
    ///
    /// Neden tek sayı yetmiyor (ölçüldü, A2 vs kısa-akış): saf reorder'da instant risk tepesi
    /// 195k ppm'e çıkarken %20 gerçek kayıpta ~180k — yani `risk > X → FEC` biçimindeki HER
    /// tek-eşik kural reorder'ı kayıp sanar. Ayrım BÜYÜKLÜKTE değil, ZAMANDA: gerçek kayıp gap'i
    /// yaşamaya devam eder (yaşlanır → Confirmed); reorder gap'i kısa sürede dolar (→ Spurious).
    ///
    /// * `instant_ppm`    — [`Self::fec_loss_risk_ppm`] ile aynı (yaş-ağırlıklı Suspected + Confirmed)
    /// * `persistent_ppm` — YALNIZ `age ≥ persist_min` yaşamış Suspected (tam ağırlık) + Confirmed;
    ///   `persist_min = srtt × persist_srtt_permille / 1000`. Reorder'ı elemek için kullanılır.
    /// * `reorder_pressure_permille` — pencerede `Spurious` birim / tüm olay birimleri. "Son
    ///   gördüğüm gap'lerin ne kadarı sonradan doldu?" Bir DISCOUNT'tur, veto değil.
    /// * `declared_units` — reorder_pressure'ın paydası; küçük-n koruması çağıranda.
    ///
    /// Payda ve pencere kanonikle AYNI; ek depolama YOK (Spurious olaylar zaten deque'te kalır).
    /// `srtt_ns == 0` → Suspected'a güven verilmez, persistent = Confirmed'a eşit.
    #[must_use]
    pub fn fec_loss_risk_split(
        &self,
        now_ns: u64,
        srtt_ns: u64,
        weights: SuspectConfidence,
        persist_min_ns: u64,
    ) -> RiskSplit {
        let mut inst_milli: u128 = 0;
        let mut pers_units: u128 = 0;
        let mut spur_units: u64 = 0;
        let mut all_units: u64 = 0;
        for e in &self.events {
            all_units = all_units.saturating_add(e.units);
            match e.state {
                LedgerState::Confirmed => {
                    inst_milli += u128::from(e.units) * 1000;
                    pers_units += u128::from(e.units);
                }
                LedgerState::Spurious => spur_units = spur_units.saturating_add(e.units),
                LedgerState::Suspected => {
                    if srtt_ns != 0 {
                        let age = now_ns.saturating_sub(e.declared_ns);
                        inst_milli +=
                            u128::from(e.units) * u128::from(weights.for_age(age, srtt_ns));
                        if age >= persist_min_ns {
                            pers_units += u128::from(e.units);
                        }
                    }
                }
            }
        }
        let den = u128::from(self.sent_window_units);
        let to_ppm = |num_milli: u128| -> u64 {
            if den == 0 {
                0
            } else {
                u64::try_from(num_milli * 1_000_000 / (den * 1000)).unwrap_or(u64::MAX)
            }
        };
        RiskSplit {
            instant_ppm: to_ppm(inst_milli),
            persistent_ppm: to_ppm(pers_units * 1000),
            reorder_pressure_permille: if all_units == 0 {
                0
            } else {
                u32::try_from(u128::from(spur_units) * 1000 / u128::from(all_units)).unwrap_or(1000)
            },
            declared_units: all_units,
            sent_window_units: self.sent_window_units,
        }
    }

    /// [FEC-RISK] **FEC'e özgü kayıp-RİSKİ** (ppm) — kanonik kayıptan AYRI bir okuma.
    ///
    /// İki farklı soru vardır ve aynı sayıyla cevaplanamaz:
    /// * ARQ: *"Bu paketin kaybolduğundan yeterince EMİN miyim?"* → [`Self::loss_windowed_ppm`]
    ///   (yalnız `Confirmed`; reorder'ı bekler, çünkü gereksiz retransmit pahalıdır)
    /// * FEC: *"Parity ekleyecek vaktim varken ŞU AN ne kadar kayıp riski var?"* → bu fonksiyon
    ///
    /// FEC kararı blok tele çıkarken verilmek ZORUNDA. Bir şüphenin onaya dönmesini beklemek
    /// sinyali birden çok RTT geciktirir ve kısa akışlarda tahmin tamamen işe yaramaz hâle gelir.
    ///
    /// Ağırlıklandırma: `Confirmed` = 1.0, `Spurious` = 0.0, `Suspected` = yaşına göre kısmi
    /// güven (srtt'ye oranlı; `weights` ile verilir). Geç ACK gelince olay `Spurious` olur ve
    /// katkısı KENDİLİĞİNDEN sıfırlanır — yani tahmin kendi hatasını düzeltir.
    ///
    /// Payda kanonik metrikle AYNI (`sent_window_units`), pencere AYNI rolling `W_loss`
    /// penceresidir — ayrı/sınırsız bir tarihçe tutulmaz.
    ///
    /// `srtt_ns == 0` (henüz ölçüm yok) → `Suspected` olaylara güven verilmez; yalnız `Confirmed`
    /// sayılır, yani sonuç kanonik metrikle ÖZDEŞ olur. Ölçüm yokken risk uydurulmaz.
    #[must_use]
    pub fn fec_loss_risk_ppm(&self, now_ns: u64, srtt_ns: u64, weights: SuspectConfidence) -> u64 {
        let mut num_milli: u128 = 0; // birim × 1000 (tamsayı; RNG/float yok — ADR-17)
        for e in &self.events {
            let w = match e.state {
                LedgerState::Confirmed => 1000,
                LedgerState::Spurious => 0,
                LedgerState::Suspected => {
                    if srtt_ns == 0 {
                        0
                    } else {
                        weights.for_age(now_ns.saturating_sub(e.declared_ns), srtt_ns)
                    }
                }
            };
            num_milli += u128::from(e.units) * u128::from(w);
        }
        if self.sent_window_units == 0 {
            return 0;
        }
        let ppm = num_milli * 1_000_000 / (u128::from(self.sent_window_units) * 1000);
        u64::try_from(ppm).unwrap_or(u64::MAX)
    }

    /// [TEŞHİS] Ledger iç durumu: `(lost_units, events_len, suspected_units, confirmed_units,
    /// sent_window_units)`. `loss_windowed_ppm` sıfır geldiğinde "hiç olay yok" · "hepsi
    /// Suspected'ta takılı" · "payda sıfır" birbirinden ancak bu ayrımla ayrılır — tek bir
    /// sıfır üç FARKLI arızayı gizler.
    #[must_use]
    pub fn diag_ledger_internals(&self) -> (u64, u64, u64, u64, u64) {
        let mut susp = 0;
        let mut conf = 0;
        for e in &self.events {
            match e.state {
                LedgerState::Suspected => susp += e.units,
                LedgerState::Confirmed => conf += e.units,
                LedgerState::Spurious => {}
            }
        }
        (
            self.lost_units,
            self.events.len() as u64,
            susp,
            conf,
            self.sent_window_units,
        )
    }

    /// [TEŞHİS] Confirm-yolu: `(confirmed_total, max_suspected_age_ns, confirm_threshold_ns,
    /// expire_threshold_ns)`.
    #[must_use]
    pub fn diag_confirm_path(&self) -> (u64, u64, u64, u64) {
        (
            self.diag_confirmed_total,
            self.diag_max_susp_age_ns,
            self.diag_confirm_threshold_ns,
            self.diag_expire_threshold_ns,
        )
    }

    /// [TEŞHİS] Tick kapsamı: `(tick_count, tick_span_ns)`. Confirm eşiği koşunun AKTİF
    /// tick penceresinden uzunsa hiçbir olay o eşiğe ULAŞAMAZ — bu ayrım tick kapsamı
    /// görünmeden yapılamaz.
    #[must_use]
    pub fn diag_tick_span(&self) -> (u64, u64) {
        (
            self.diag_tick_count,
            self.diag_last_tick_ns
                .saturating_sub(self.diag_first_tick_ns),
        )
    }

    /// [FEC-RISK ENSTRÜMAN] `(t_first_suspected_ns, t_first_confirmed_ns)`; 0 = hiç olmadı.
    /// Aradaki fark, kanonik kayıp sinyalinin ÖLÜ ZAMANIDIR.
    #[must_use]
    pub fn diag_first_loss_times(&self) -> (u64, u64) {
        (
            self.diag_t_first_suspected_ns,
            self.diag_t_first_confirmed_ns,
        )
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

impl ThresholdLossDetector {
    /// DR-CS-1 (d) [READ-ONLY türetme; eşik/ledger-semantiği DEĞİŞMEZ]: MEVCUT time-threshold
    /// makinesinin bir-SONRAKİ deadline'ı — eligibility `detect_time_losses` ile BİREBİR
    /// (`have_acked` && `seq < largest_acked`); deadline = send_ns + max(srtt·num/den, floor).
    /// None = eligible-birim-yok (ör. hiç-ACK-kanıtı-yok veya tail > largest_acked —
    /// F0-tespit-(iii)-caveat'i; o-birimler mevcut-RecoverFwd/geometrik-yola-kalır).
    #[must_use]
    pub fn next_time_loss_deadline_ns(
        &self,
        srtt: Duration,
        cadence_floor: Duration,
    ) -> Option<u64> {
        if !self.have_acked {
            return None;
        }
        let floor = self.cfg.min_delay.max(cadence_floor);
        let thresh = scale_duration(srtt, self.cfg.time_num, self.cfg.time_den)
            .max(floor)
            .as_nanos() as u64;
        self.inflight
            .iter()
            .filter(|(&seq, _)| seq < self.largest_acked)
            .map(|(_, s)| s.send_ns.saturating_add(thresh))
            .min()
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
                // [PERSIST] reorder ömrü örneği: ilan → un-loss süresi (bounded halka).
                let age = now_ns.saturating_sub(ev.declared_ns);
                self.spurious_ages[self.spurious_age_idx] = age;
                self.spurious_age_idx = (self.spurious_age_idx + 1) % SPURIOUS_AGE_RING;
                self.spurious_age_len = (self.spurious_age_len + 1).min(SPURIOUS_AGE_RING);
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
