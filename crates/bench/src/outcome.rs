//! [FAZ-4 §8/§12/§13] KOŞU SONUÇ SINIFLARI · BLACKOUT · İSTATİSTİK POLİTİKASI.
//!
//! Bu modül PERFORMANS ÜRETMEZ. Ölçülmüş bir koşunun NE OLDUĞUNU sınıflandırır ve çoklu
//! koşudan hangi istatistiğin çıkarılabileceğini tanımlar. Üç denetim bulgusuna cevap verir:
//!
//! 1. **§13** — timeout'lar etiketsiz biçimde "0 teslim"e çevriliyordu; harness arızası ile
//!    transport arızası ayrılmıyordu. Artık her koşu tek bir [`Outcome`] sınıfına düşer.
//! 2. **§8** — felaket düzeyinde hostile başarısızlık medyan completion sayısının ARKASINDA
//!    gizlenebiliyordu. [`blackout_scan`] ikili (var/yok) bir cevap üretir.
//! 3. **§12** — n=3/n=5 medyanları nihai iddia olarak kullanılıyordu. [`Stats`] her zaman n,
//!    dağılım ve GÜVEN ARALIĞI taşır; `n < MIN_FINAL_CLAIM_N` ise iddia sınıfı `smoke`tur.
//!
//! ADR-17 (determinizm): RNG YOK. Güven aralıkları kapalı-form/bisection ile hesaplanır —
//! bootstrap yeniden-örnekleme KULLANILMAZ. Aynı girdi → aynı çıktı.

use crate::event::RunOutput;
use std::time::Duration;

/// [§12.2] Nihai (T0–T7) iddia için asgari koşu sayısı. Altındaki her şey `smoke`tur.
pub const MIN_FINAL_CLAIM_N: usize = 30;

/// [§12.3] Varsayılan güven düzeyi (‰). 950‰ = %95.
pub const DEFAULT_CI_PERMILLE: u32 = 950;

/// [§8] Blackout tarama penceresi (varsayılan). Bu pencerede teslim eşiğin altına düşerse
/// pencere BLACKOUT'tur.
pub const DEFAULT_BLACKOUT_WINDOW: Duration = Duration::from_secs(1);

/// [§8] Bir pencerenin "kullanılabilir" sayılması için gereken asgari uygulama teslimi.
pub const DEFAULT_MIN_DELIVERIES_PER_WINDOW: u64 = 1;

// ─────────────────────────────────────────────────────────────────────────────────────────
// §13 — SONUÇ SINIFLARI
// ─────────────────────────────────────────────────────────────────────────────────────────

/// [§13] Bir koşunun sonuç sınıfı. Timeout etiketsiz sıfıra ÇEVRİLMEZ; harness arızası
/// transport arızasıyla KARIŞTIRILMAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Tüm mesajlar deadline içinde teslim edildi.
    Completed,
    /// Bağlantı kuruldu, akış çalıştı, bazı mesajlar deadline'ı kaçırdı.
    DeadlineMiss,
    /// Akış sırasında en az bir blackout penceresi var (§8) — teslim tamamen durdu.
    Blackout,
    /// Koşu kendi zaman sınırında bitti; teslim SIFIR ama bağlantı kurulmuştu.
    Timeout,
    /// Bağlantı/el sıkışma hiç kurulamadı (transport arızası — GEÇERLİ ölçüm sonucu).
    ConnectionFailure,
    /// Harness kendi hatasıyla düştü (dosya yok, bozuk JSONL, eksik alan) — ölçüm DEĞİL.
    HarnessFailure,
    /// Altyapı arızası (runner, netns, ağ kurulumu) — ölçüm DEĞİL.
    InfraFailure,
}

impl Outcome {
    /// Bu sonuç bir TRANSPORT ölçümü mü (agregaya girer mi)?
    ///
    /// `ConnectionFailure` ve `Timeout` GİRER: bunlar transport'un gerçek davranışıdır ve
    /// dışlanırsa completion yapay olarak şişer. `HarnessFailure`/`InfraFailure` GİRMEZ ama
    /// SESSİZCE SİLİNMEZ — insidansı ayrıca raporlanır (§12.4).
    #[must_use]
    pub fn is_measurement(self) -> bool {
        !matches!(self, Self::HarnessFailure | Self::InfraFailure)
    }

    /// Rapor etiketi.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::DeadlineMiss => "deadline-miss",
            Self::Blackout => "blackout",
            Self::Timeout => "timeout",
            Self::ConnectionFailure => "connection-failure",
            Self::HarnessFailure => "harness-failure",
            Self::InfraFailure => "infra-failure",
        }
    }
}

/// Koşuyu sınıflandır. `deadline` = iddianın deadline'ı; `blackout` = §8 taraması sonucu.
///
/// ÖNCELİK SIRASI (bir koşu tam olarak BİR sınıfa düşsün diye sabittir):
/// `harness-failure` → `connection-failure` → `timeout` → `blackout` → `deadline-miss` →
/// `completed`. Hiç teslim olmayan koşu `timeout`tur, `blackout` DEĞİL: kesinti değil, hiç
/// başlamama söz konusudur (blackout taraması böyle bir koşuda da `any=true` verir; sınıf
/// daha spesifik olanı seçer).
#[must_use]
pub fn classify(run: &RunOutput, deadline: Duration, blackout: &BlackoutReport) -> Outcome {
    if run.events.is_empty() {
        // Olay yok: harness mesaj üretmemiş → ölçüm değil.
        return Outcome::HarnessFailure;
    }
    let delivered = run
        .events
        .iter()
        .filter(|e| e.delivered_ns.is_some())
        .count();
    // Bağlantı hiç kurulmadıysa arm hiç byte göndermemiştir VE hiç teslim yoktur.
    if delivered == 0 && run.meta.udp_tx_bytes == 0 {
        return Outcome::ConnectionFailure;
    }
    if delivered == 0 {
        return Outcome::Timeout;
    }
    if blackout.any {
        return Outcome::Blackout;
    }
    let dn = u64::try_from(deadline.as_nanos()).unwrap_or(u64::MAX);
    let in_time = run
        .events
        .iter()
        .filter(|e| {
            e.delivered_ns
                .is_some_and(|d| d.saturating_sub(e.sent_ns) <= dn)
        })
        .count();
    if in_time == run.events.len() {
        Outcome::Completed
    } else {
        Outcome::DeadlineMiss
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// §8 — BLACKOUT
// ─────────────────────────────────────────────────────────────────────────────────────────

/// [§8] Blackout tanımı — ölçümden ÖNCE yazılır, T0–T7'de DEĞİŞTİRİLMEZ.
///
/// Akışın ölçüm penceresi `window` uzunluğunda ardışık dilimlere bölünür. Bir dilim
/// **blackout**tur ⟺
///
/// * o dilimde tamamlanan uygulama teslimi sayısı `< min_deliveries`, **ve**
/// * o dilim boyunca en az bir mesaj GÖNDERİLMİŞ ama henüz teslim edilmemiştir
///   (yani teslim edilecek iş VARDI — akışın bittiği sessizlik blackout DEĞİLDİR).
///
/// Bu tanım ikilidir ve medyan completion'ın arkasına saklanamaz.
#[derive(Debug, Clone, Copy)]
pub struct BlackoutSpec {
    /// Dilim uzunluğu.
    pub window: Duration,
    /// Dilimin "kullanılabilir" sayılması için gereken asgari teslim.
    pub min_deliveries: u64,
}

impl Default for BlackoutSpec {
    fn default() -> Self {
        Self {
            window: DEFAULT_BLACKOUT_WINDOW,
            min_deliveries: DEFAULT_MIN_DELIVERIES_PER_WINDOW,
        }
    }
}

/// [§8] Tarama sonucu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlackoutReport {
    /// İKİLİ cevap: en az bir blackout dilimi var mı?
    pub any: bool,
    /// Toplam blackout süresi (ns).
    pub total_ns: u64,
    /// En uzun kesintisiz blackout (ns).
    pub longest_ns: u64,
    /// En uzun blackout'un BAŞINDAN ilk sonraki teslime kadar geçen süre (ns).
    /// Teslim hiç dönmediyse `None` (kurtarma YOK).
    pub time_to_recovery_ns: Option<u64>,
}

/// Koşuyu §8 tanımına göre tara.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn blackout_scan(run: &RunOutput, spec: &BlackoutSpec) -> BlackoutReport {
    let w = u64::try_from(spec.window.as_nanos())
        .unwrap_or(u64::MAX)
        .max(1);
    if run.events.is_empty() {
        return BlackoutReport::default();
    }
    // Akışın AKTİF SPAN'i: son gönderim ile son teslimin ilerisi.
    //
    // BEYAN EDİLEN SINIR: son gönderimden VE son teslimden sonraki sessizlik taranmaz. O
    // bölgedeki kayıp zaten completion metriğinde görünür; blackout metriği akış SÜRERKEN
    // teslimin durmasını ölçer. "3. sn'de teslim durdu, gönderim 20. sn'ye dek sürdü" vakası
    // bu span içinde KALIR ve yakalanır — §8'in asıl hedefi budur.
    let last_sent = run.events.iter().map(|e| e.sent_ns).max().unwrap_or(0);
    let last_delivered = run.events.iter().filter_map(|e| e.delivered_ns).max();
    // [ERRATUM-1] Ufuk SON TESLİMİN BİR NS SONRASIDIR, teslimin kendisi DEĞİL.
    //
    // Hata: ufuk son teslime EŞİT olduğunda, son pencere `[t, ufuk)` yarı-açık olduğu için
    // o teslimi SAYMAZ (`d < end` yanlış) ama AYNI mesajı `delivered_ns >= end` olduğu için
    // "bekleyen iş" SAYAR → teslim=0 ∧ bekleyen=var → sahte blackout. 100/100 teslim eden bir
    // koşu 20 ms'lik "blackout" damgası alıyordu. Üstelik yanlılık YÖNLÜYDÜ: yalnız kuyruğu
    // pencere sınırını aşan kol cezalanıyordu (ölçülen: Kama 16/30, quinn 1/30, TCP 0/30 —
    // üçünün de completion'ı 1.000 iken). Ölçüm yanlılığıydı, taşıyıcı özelliği değil.
    //
    // `max(last_sent, last_delivered+1)`: teslim kendi penceresinin İÇİNDE kalır ve artık
    // "bekleyen" sayılmaz; gönderim-sonrası sessizlik ufku ise UZAMAZ (teslimsiz kuyruk
    // vakası `kurtarma_yoksa_none` testindeki gibi aynen korunur).
    let horizon = last_delivered.map_or(last_sent, |d| last_sent.max(d.saturating_add(1)));
    if horizon == 0 {
        return BlackoutReport::default();
    }
    let mut deliveries: Vec<u64> = run.events.iter().filter_map(|e| e.delivered_ns).collect();
    deliveries.sort_unstable();

    let mut report = BlackoutReport::default();
    let mut streak_start: Option<u64> = None;
    let mut streak_len: u64 = 0;
    let mut longest_start: Option<u64> = None;

    let mut t = 0u64;
    while t < horizon {
        let end = t.saturating_add(w).min(horizon);
        let delivered_in = deliveries
            .iter()
            .filter(|&&d| d >= t && d < end)
            .count()
            .try_into()
            .unwrap_or(u64::MAX);
        // Dilim boyunca teslim bekleyen iş var mıydı? (gönderilmiş ∧ dilim bitmeden teslim yok)
        let outstanding = run
            .events
            .iter()
            .any(|e| e.sent_ns < end && e.delivered_ns.is_none_or(|d| d >= end));
        if delivered_in < spec.min_deliveries && outstanding {
            report.any = true;
            report.total_ns = report.total_ns.saturating_add(end.saturating_sub(t));
            if streak_start.is_none() {
                streak_start = Some(t);
                streak_len = 0;
            }
            streak_len = streak_len.saturating_add(end.saturating_sub(t));
            if streak_len > report.longest_ns {
                report.longest_ns = streak_len;
                longest_start = streak_start;
            }
        } else {
            streak_start = None;
            streak_len = 0;
        }
        t = end;
    }
    if let Some(s) = longest_start {
        report.time_to_recovery_ns = deliveries
            .iter()
            .find(|&&d| d >= s)
            .map(|&d| d.saturating_sub(s));
    }
    report
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// §7 — KURULUM MALİYETİ GÖRÜNÜMLERİ
// ─────────────────────────────────────────────────────────────────────────────────────────

/// [§7] İki GÖRÜNÜM — asla karıştırılmaz. Harness tek ölçüm yapar (`setup-excluded`) ve
/// kurulum maliyetini AYRI kaydeder; uçtan-uca görünüm bundan TÜRETİLİR.
///
/// Türetme geçerlidir çünkü kurulum tüm akışı sabit `setup_ns` kadar öteler: uçtan-uca
/// completion(D) = kararlı-durum completion(D − `setup_ns`). Kollar arası simetriktir —
/// her kol KENDİ ölçülen kurulum maliyetini öder (Kama ≈ mikrosaniyeler, QUIC el sıkışması
/// milisaniyeler). Kama'ya olmayan bir kurulum maliyeti UYDURULMAZ (§7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupView {
    /// Kararlı-durum transport teslimi: kurulum pencere DIŞINDA (harness'ın ölçtüğü).
    SteadyState,
    /// Uçtan-uca akış: kurulum maliyeti DAHİL.
    ColdFlow,
}

impl SetupView {
    /// Bu görünümde geçerli olan efektif deadline. `ColdFlow` deadline'ı kurulum kadar kısaltır.
    #[must_use]
    pub fn effective_deadline(self, deadline: Duration, setup_ns: u64) -> Duration {
        match self {
            Self::SteadyState => deadline,
            Self::ColdFlow => {
                Duration::from_nanos(u64::try_from(deadline.as_nanos()).unwrap_or(u64::MAX))
                    .saturating_sub(Duration::from_nanos(setup_ns))
            }
        }
    }

    /// Rapor etiketi (belirsiz kurulum muhasebesi YASAK — §7.4).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::SteadyState => "steady-state (setup excluded, reported separately)",
            Self::ColdFlow => "cold-flow (end-to-end incl. setup)",
        }
    }
}

/// [§7.4/§M4] Kurulum sınıfı KOLUN YAPISAL GERÇEĞİNDEN türetilir — operatör beyanından değil.
#[must_use]
pub fn derived_setup_class(setup_in_window: bool) -> &'static str {
    if setup_in_window {
        "cold-flow"
    } else {
        "setup-excluded"
    }
}

/// [§M4] Operatörün `--setup-class` iddiasını harness gerçeğiyle karşılaştır.
///
/// `Err` dönerse koşu manifest YAZMADAN durur: sessiz kurulum-sınıfı karışımı imkânsızdır.
/// İddia verilmemişse (`None`) denetim yoktur — türetilmiş sınıf yine de manifeste yazılır.
///
/// # Errors
/// İddia türetilmiş sınıfla çelişirse.
pub fn check_setup_claim(claim: Option<&str>, derived: &str) -> Result<(), String> {
    match claim {
        Some(c) if c != derived => Err(format!(
            "kurulum-sınıfı çelişkisi: --setup-class={c} ama harness gerçeği {derived}"
        )),
        _ => Ok(()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────
// §12 — İSTATİSTİK
// ─────────────────────────────────────────────────────────────────────────────────────────

/// [§12.2] İddia sınıfı — n'e bakarak OTOMATİK belirlenir; operatör yükseltemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimClass {
    /// n < 30: yalnız harness/anlam doğrulama. PERFORMANS/SEVK İDDİASI YASAK.
    Smoke,
    /// n ≥ 30: güven aralığıyla birlikte nihai iddia yapılabilir.
    Final,
}

impl ClaimClass {
    /// n'den türet.
    #[must_use]
    pub fn from_n(n: usize) -> Self {
        if n >= MIN_FINAL_CLAIM_N {
            Self::Final
        } else {
            Self::Smoke
        }
    }

    /// Rapor etiketi.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Smoke => "smoke (n<30 — NO performance claim)",
            Self::Final => "final (n>=30 + CI)",
        }
    }
}

/// [§12.3] Bir metriğin koşular-arası özeti. Gaussian VARSAYMAZ: merkez = medyan,
/// dağılım = IQR, güven aralığı = sıra-istatistiği (dağılımdan bağımsız).
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// Koşu sayısı.
    pub n: usize,
    /// Medyan.
    pub median: f64,
    /// Q1 / Q3 (dağılım).
    pub q1: f64,
    /// Q3.
    pub q3: f64,
    /// Medyanın güven aralığı — sıra istatistiği (dağılımdan bağımsız). n<6'da `None`
    /// (aralık tüm örneği kapsar; anlamlı bilgi taşımaz).
    pub median_ci: Option<(f64, f64)>,
    /// Güven düzeyi (‰).
    pub ci_permille: u32,
    /// İddia sınıfı.
    pub claim: ClaimClass,
}

/// [§12.3] Sıralı örnekten dağılımdan-bağımsız özet.
#[must_use]
pub fn summarize(sample: &[f64], ci_permille: u32) -> Stats {
    let mut s: Vec<f64> = sample.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Stats {
        n: s.len(),
        median: quantile(&s, 0.50),
        q1: quantile(&s, 0.25),
        q3: quantile(&s, 0.75),
        median_ci: median_ci(&s, ci_permille),
        ci_permille,
        claim: ClaimClass::from_n(s.len()),
    }
}

/// Sıra-istatistiği medyan güven aralığı (dağılımdan bağımsız, RNG YOK).
///
/// `X ~ Binom(n, 0.5)` için `k` = `P(X <= k-1) <= alpha/2` sağlayan en büyük indeks; aralık
/// `[s[k-1], s[n-k]]`. Bootstrap KULLANILMAZ (ADR-17 determinizm).
#[must_use]
pub fn median_ci(sorted: &[f64], ci_permille: u32) -> Option<(f64, f64)> {
    let n = sorted.len();
    if n < 6 {
        return None; // aralık tüm örnek olurdu
    }
    let alpha_half = (1.0 - f64::from(ci_permille) / 1000.0) / 2.0;
    let mut k = 0usize;
    while k < n / 2 && binom_cdf(n, 0.5, k) <= alpha_half {
        k += 1;
    }
    if k == 0 {
        return None;
    }
    Some((sorted[k - 1], sorted[n - k]))
}

/// `P(X <= k)` for `X ~ Binom(n, p)`. Deterministik toplam.
#[must_use]
pub fn binom_cdf(n: usize, p: f64, k: usize) -> f64 {
    if k >= n {
        return 1.0;
    }
    // SINIR DEĞERLERİ: `p/(1-p)` yinelemesi p=1'de bölme-sıfır (NaN) üretir ve bisection'ın
    // yön tespitini bozardı — 10/10 alt sınırı 4e-22 çıkmıştı. Sınırlar kapalı biçimde döner.
    if p <= 0.0 {
        return 1.0; // X ≡ 0 ≤ k
    }
    if p >= 1.0 {
        return 0.0; // X ≡ n > k (k < n burada garanti)
    }
    // acc = P(X=0); döngü P(X=1)..P(X=k) ekler. `0..=k` YAZILMAZ: bir fazla terim eklerdi
    // (P(X<=k+1)) ve Clopper-Pearson sınırlarını sistematik olarak GENİŞLETİRDİ — kendi
    // testim yakaladı (0/10 üst sınırı 0.445 çıktı, doğrusu 0.308).
    let mut term = (1.0 - p).powi(i32::try_from(n).unwrap_or(i32::MAX));
    let mut acc = term;
    for i in 0..k {
        let iu = i as f64;
        term *= (n as f64 - iu) / (iu + 1.0) * (p / (1.0 - p));
        acc += term;
    }
    acc.clamp(0.0, 1.0)
}

/// [§12.3/§12.4] Oran için KESİN (Clopper–Pearson) güven aralığı — küçük n'de doğru.
/// Binom CDF üzerinde sabit-adımlı bisection; RNG YOK, deterministik.
#[must_use]
pub fn proportion_ci(k: usize, n: usize, ci_permille: u32) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let alpha_half = (1.0 - f64::from(ci_permille) / 1000.0) / 2.0;
    // Alt sınır: P(X >= k | p) = alpha/2  ⟺  1 - P(X <= k-1 | p) = alpha/2
    let low = if k == 0 {
        0.0
    } else {
        bisect(|p| 1.0 - binom_cdf(n, p, k - 1) - alpha_half)
    };
    // Üst sınır: P(X <= k | p) = alpha/2
    let high = if k == n {
        1.0
    } else {
        bisect(|p| binom_cdf(n, p, k) - alpha_half)
    };
    (low, high)
}

/// [0,1] üzerinde monoton fonksiyon için 80-adım bisection (deterministik, ~1e-24 çözünürlük).
fn bisect<F: Fn(f64) -> f64>(f: F) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    let increasing = f(1.0) > f(0.0);
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        let v = f(mid);
        if (v > 0.0) == increasing {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// [§12.4] STALL KATMANLAMASI — üç sayı BİRLİKTE raporlanır; başarısız koşu SESSİZCE SİLİNMEZ.
#[derive(Debug, Clone, PartialEq)]
pub struct Stratified {
    /// Tüm ölçüm koşuları (stall dahil).
    pub all_runs: Stats,
    /// Yalnız stall'sız koşular.
    pub stall_free: Stats,
    /// Stall sayısı / ölçüm koşusu sayısı.
    pub stall_incidence: (usize, usize),
    /// Stall insidansının kesin güven aralığı.
    pub stall_ci: (f64, f64),
    /// Ölçüm SAYILMAYAN koşular (harness/infra) — dışlanır ama raporlanır.
    pub non_measurement: usize,
}

/// [§12.4] Sonuç sınıflarıyla birlikte katmanlı özet üret.
///
/// `stall` = teslim tamamen durmuş koşu (Blackout · Timeout · ConnectionFailure).
#[must_use]
pub fn stratify(samples: &[(f64, Outcome)], ci_permille: u32) -> Stratified {
    let measurement: Vec<&(f64, Outcome)> =
        samples.iter().filter(|(_, o)| o.is_measurement()).collect();
    let non_measurement = samples.len() - measurement.len();
    let is_stall = |o: Outcome| {
        matches!(
            o,
            Outcome::Blackout | Outcome::Timeout | Outcome::ConnectionFailure
        )
    };
    let all: Vec<f64> = measurement.iter().map(|(v, _)| *v).collect();
    let free: Vec<f64> = measurement
        .iter()
        .filter(|(_, o)| !is_stall(*o))
        .map(|(v, _)| *v)
        .collect();
    let stalls = measurement.len() - free.len();
    Stratified {
        all_runs: summarize(&all, ci_permille),
        stall_free: summarize(&free, ci_permille),
        stall_incidence: (stalls, measurement.len()),
        stall_ci: proportion_ci(stalls, measurement.len(), ci_permille),
        non_measurement,
    }
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};

    fn meta(tx: u64) -> RunMeta {
        RunMeta {
            profile: "t".into(),
            cc: "cubic".into(),
            transport: "kama-product".into(),
            quinn_version: String::new(),
            seed: 0,
            repeat: 0,
            msg_count: 0,
            payload_bytes: 256,
            max_deadline_ns: 120_000_000_000,
            udp_tx_bytes: tx,
            udp_rx_bytes: 0,
            tx: TxComponents::default(),
            kama_arm: String::new(),
            submit_interval_ns: 0,
            budget: Default::default(),
            t0_unix_ns: 0,
            events_epoch_offset_ns: 0,
            warmup_residue: 0,
            activation: None,
            setup_ns: 0,
            setup_in_window: false,
            profile_id: String::new(),
            profile_delta: Vec::new(),
        }
    }

    fn run(tx: u64, ev: Vec<(u64, u64, Option<u64>)>) -> RunOutput {
        RunOutput {
            meta: meta(tx),
            events: ev
                .into_iter()
                .map(|(id, s, d)| RawEvent {
                    msg_id: id,
                    sent_ns: s,
                    delivered_ns: d,
                    payload_bytes: 256,
                })
                .collect(),
        }
    }

    const S: u64 = 1_000_000_000;

    /// [§8] Akışın SONUNDAKİ sessizlik blackout DEĞİLDİR (teslim bekleyen iş yok).
    #[test]
    fn akis_sonu_sessizligi_blackout_degil() {
        let r = run(1000, vec![(0, 0, Some(S / 10)), (1, S / 10, Some(S / 5))]);
        let b = blackout_scan(&r, &BlackoutSpec::default());
        assert!(!b.any, "{b:?}");
    }

    /// [§8] Ortadaki 5 sn'lik tam kesinti YAKALANIR ve kurtarma süresi ölçülür.
    #[test]
    fn ortadaki_kesinti_yakalanir_ve_kurtarma_olculur() {
        // 0. sn'de iki mesaj gönderildi; biri hemen, diğeri 6. sn'de teslim.
        let r = run(1000, vec![(0, 0, Some(S / 10)), (1, 0, Some(6 * S))]);
        let b = blackout_scan(&r, &BlackoutSpec::default());
        assert!(b.any, "kesinti görülmeli: {b:?}");
        // 1..6 sn arası 5 pencere teslimsiz ve iş bekliyor.
        assert_eq!(b.longest_ns, 5 * S, "{b:?}");
        assert_eq!(b.total_ns, 5 * S, "{b:?}");
        // En uzun kesinti 1. sn'de başladı; ilk sonraki teslim 6. sn → 5 sn.
        assert_eq!(b.time_to_recovery_ns, Some(5 * S), "{b:?}");
    }

    /// [ERRATUM-1 REGRESYON] Son teslim pencere sınırını AŞTIĞINDA sahte blackout ÜRETİLMEZ.
    ///
    /// Gerçek kampanya verisinden alınmış vaka (T0/rep11): 100 mesajın 100'ü teslim edildi,
    /// son gönderim 4.952 s, son teslim 5.021 s. Ufuk son teslime eşit olduğunda son pencere
    /// `[5.000, 5.021)` teslimi dışlıyor ama aynı mesajı "bekleyen" sayıyordu → sahte blackout.
    /// Yanlılık YÖNLÜYDÜ: yalnız kuyruğu saniye sınırını aşan kol cezalanıyordu.
    #[test]
    fn son_teslim_pencere_sinirini_asinca_sahte_blackout_yok() {
        // Sıkı paketlenmiş teslimler + son teslim 5.021 s'de (5 s sınırının hemen ötesinde).
        let mut ev: Vec<(u64, u64, Option<u64>)> = Vec::new();
        for i in 0..100u64 {
            let sent = i * 50 * 1_000_000; // 50 ms aralık → son gönderim 4.950 s
            let delivered = sent + 70 * 1_000_000; // her biri +70 ms → son teslim 5.020 s
            ev.push((i, sent, Some(delivered)));
        }
        let r = run(50_000, ev);
        let b = blackout_scan(&r, &BlackoutSpec::default());
        assert!(
            !b.any,
            "100/100 teslim eden koşu blackout DEĞİLDİR (sınır artefaktı): {b:?}"
        );
        assert_eq!(b.total_ns, 0);
        assert_eq!(b.longest_ns, 0);
    }

    /// [§8] Kurtarma HİÇ olmazsa `time_to_recovery` yoktur (None) — 0 diye raporlanmaz.
    ///
    /// İLK FIXTURE YANLIŞTI: her iki mesaj da t=0'da gönderilmişti, ufuk son teslimde
    /// (0.1 sn) bitiyordu ve o teslim "kurtarma" olarak SAYILIYORDU. Doğru senaryo —
    /// §8'in asıl hedefi — gönderim SÜRERKEN teslimin durup bir daha dönmemesidir.
    #[test]
    fn kurtarma_yoksa_none() {
        // 0.1 sn'de son teslim; gönderim 5. sn'ye dek sürüyor; 1. sn sonrası hiç teslim yok.
        let r = run(
            1000,
            vec![(0, 0, Some(S / 10)), (1, S, None), (2, 5 * S, None)],
        );
        let b = blackout_scan(&r, &BlackoutSpec::default());
        assert!(b.any, "{b:?}");
        assert_eq!(b.longest_ns, 4 * S, "1..5 sn arası kesintisiz: {b:?}");
        assert_eq!(
            b.time_to_recovery_ns, None,
            "teslim bir daha DÖNMEDİ: {b:?}"
        );
    }

    /// [§13] Timeout ETİKETSİZ sıfıra çevrilmez; bağlantı arızasından ayrılır.
    #[test]
    fn sonuc_siniflari_ayrilir() {
        let d = Duration::from_secs(30);
        let none = BlackoutReport::default();
        // Byte gitmiş ama hiç teslim yok → timeout (transport çalıştı, teslim edemedi).
        assert_eq!(
            classify(&run(5000, vec![(0, 0, None)]), d, &none),
            Outcome::Timeout
        );
        // Hiç byte gitmemiş ve teslim yok → bağlantı kurulamadı.
        assert_eq!(
            classify(&run(0, vec![(0, 0, None)]), d, &none),
            Outcome::ConnectionFailure
        );
        // Olay yok → harness arızası (ölçüm DEĞİL).
        assert_eq!(classify(&run(0, vec![]), d, &none), Outcome::HarnessFailure);
        assert!(!Outcome::HarnessFailure.is_measurement());
        assert!(
            Outcome::ConnectionFailure.is_measurement(),
            "transport gerçeği agregaya girer"
        );
        // Tümü zamanında → completed; biri geç → deadline-miss.
        assert_eq!(
            classify(&run(5000, vec![(0, 0, Some(S))]), d, &none),
            Outcome::Completed
        );
        assert_eq!(
            classify(&run(5000, vec![(0, 0, Some(60 * S))]), d, &none),
            Outcome::DeadlineMiss
        );
    }

    /// [§13] ÖNCELİK: hiç teslim olmayan koşu `timeout`tur, `blackout` DEĞİL — tarama o koşuda
    /// da `any=true` verir ama sınıf daha spesifik olanı seçer (kesinti değil, hiç başlamama).
    #[test]
    fn sinif_onceligi_sabittir() {
        let d = Duration::from_secs(30);
        // Gönderim 5. sn'ye dek sürdü, hiç teslim yok → tarama blackout görür...
        let r = run(5000, vec![(0, 0, None), (1, 5 * S, None)]);
        let b = blackout_scan(&r, &BlackoutSpec::default());
        assert!(b.any, "tarama kesinti görmeli: {b:?}");
        // ...ama sınıf `timeout` olmalı.
        assert_eq!(classify(&r, d, &b), Outcome::Timeout);
        // Buna karşılık teslim BAŞLAYIP durursa sınıf `blackout`.
        let r2 = run(
            5000,
            vec![(0, 0, Some(S / 10)), (1, S, None), (2, 5 * S, None)],
        );
        let b2 = blackout_scan(&r2, &BlackoutSpec::default());
        assert_eq!(classify(&r2, d, &b2), Outcome::Blackout);
    }

    /// [§12.2] n<30 iddia sınıfını OTOMATİK `smoke` yapar.
    #[test]
    fn kucuk_n_smoke_kalir() {
        let s = summarize(&[0.9, 0.91, 0.92], DEFAULT_CI_PERMILLE);
        assert_eq!(s.n, 3);
        assert_eq!(s.claim, ClaimClass::Smoke);
        assert_eq!(s.median_ci, None, "n<6'da anlamlı aralık YOK");
        let big: Vec<f64> = (0..30).map(|i| f64::from(i) / 100.0).collect();
        assert_eq!(
            summarize(&big, DEFAULT_CI_PERMILLE).claim,
            ClaimClass::Final
        );
    }

    /// [§12.3] Medyan güven aralığı dağılımdan bağımsız ve DETERMİNİSTİK.
    #[test]
    fn medyan_ci_deterministik_ve_kapsayici() {
        let v: Vec<f64> = (1..=31).map(f64::from).collect();
        let a = summarize(&v, DEFAULT_CI_PERMILLE);
        let b = summarize(&v, DEFAULT_CI_PERMILLE);
        assert_eq!(a.median_ci, b.median_ci, "aynı girdi → aynı çıktı (ADR-17)");
        let (lo, hi) = a.median_ci.expect("n=31 aralık verir");
        assert!(lo < a.median && a.median < hi, "{lo} < {} < {hi}", a.median);
        // %95 aralık %80'den GENİŞ olmalı.
        let narrow = summarize(&v, 800).median_ci.expect("aralık");
        assert!(hi - lo >= narrow.1 - narrow.0);
    }

    /// [§12.3] Kesin oran aralığı: 0/10 üst sınırı 0 DEĞİLDİR (sıfır gözlem ≠ sıfır risk).
    #[test]
    fn kesin_oran_araligi_sifirda_bile_bilgi_verir() {
        let (lo, hi) = proportion_ci(0, 10, DEFAULT_CI_PERMILLE);
        assert!((lo - 0.0).abs() < 1e-12);
        assert!(
            hi > 0.25 && hi < 0.35,
            "Clopper-Pearson 0/10 ≈ 0.308, bulunan {hi}"
        );
        let (lo2, hi2) = proportion_ci(10, 10, DEFAULT_CI_PERMILLE);
        assert!(
            lo2 > 0.65 && lo2 < 0.75,
            "10/10 alt sınır ≈ 0.692, bulunan {lo2}"
        );
        assert!((hi2 - 1.0).abs() < 1e-12);
    }

    /// [§12.4] Stall'lı koşu SESSİZCE SİLİNMEZ: üç sayı birlikte döner.
    #[test]
    fn stall_katmanlamasi_gizlemez() {
        let s = stratify(
            &[
                (1.0, Outcome::Completed),
                (0.9, Outcome::DeadlineMiss),
                (0.0, Outcome::Timeout),
                (0.0, Outcome::Blackout),
                (0.5, Outcome::HarnessFailure), // ölçüm DEĞİL → dışlanır ama sayılır
            ],
            DEFAULT_CI_PERMILLE,
        );
        assert_eq!(s.all_runs.n, 4, "harness arızası agregaya girmez");
        assert_eq!(s.stall_free.n, 2);
        assert_eq!(s.stall_incidence, (2, 4));
        assert_eq!(s.non_measurement, 1, "dışlanan koşu RAPORLANIR");
        assert!(
            s.all_runs.median < s.stall_free.median,
            "stall medyanı düşürür"
        );
    }

    /// [§M4] Kurulum sınıfı SESSİZCE karışamaz: yanlış iddia hata verir, doğru iddia geçer,
    /// iddia yokluğu denetimi atlar ama sınıf yine türetilmiş kalır.
    #[test]
    fn kurulum_sinifi_sessizce_karisamaz() {
        assert_eq!(derived_setup_class(false), "setup-excluded");
        assert_eq!(derived_setup_class(true), "cold-flow");
        // Yanlış iddia → hata (koşu manifest yazmadan durur).
        let e = check_setup_claim(Some("cold-flow"), "setup-excluded")
            .expect_err("çelişki yakalanmalı");
        assert!(
            e.contains("cold-flow") && e.contains("setup-excluded"),
            "{e}"
        );
        // Doğru iddia → geçer. İddia yok → denetim yok.
        assert!(check_setup_claim(Some("setup-excluded"), "setup-excluded").is_ok());
        assert!(check_setup_claim(None, "setup-excluded").is_ok());
    }

    /// [§7] Uçtan-uca görünüm kurulum maliyetini deadline'dan DÜŞER; kararlı-durum dokunmaz.
    #[test]
    fn kurulum_gorunumu_simetrik_turetilir() {
        let d = Duration::from_secs(10);
        assert_eq!(SetupView::SteadyState.effective_deadline(d, 2 * S), d);
        assert_eq!(
            SetupView::ColdFlow.effective_deadline(d, 2 * S),
            Duration::from_secs(8)
        );
        // Kurulum deadline'dan uzunsa sıfıra doygunlaşır (negatif süre YOK).
        assert_eq!(
            SetupView::ColdFlow.effective_deadline(d, 30 * S),
            Duration::ZERO
        );
    }
}
