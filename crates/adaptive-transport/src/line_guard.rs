//! [HAT-KORUMASI] Geri bildirimi aç profilde gerçek ileri hattın ölçülmesi: ölçüm profilin hat
//! hızını yalnız DÜŞÜRÜR, hiçbir zaman profilin üstüne çıkarmaz.
//!
//! # Sorun
//!
//! Geri bildirimi aç profilde ([`crate::sender::FeedbackStarvedRule`]) gönderici hızı ve hat
//! modelini profilin yukarı yön hızına sert bağlar; uçuş tabanı `hat × (geri bildirim ufku + RTT)`
//! olur (62 B/s dönüşte ≈ 11 s × hat — bütün dosyadan büyük). Profil gerçek hattan iyimserse
//! göndericinin hiçbir sınırı gerçek hatta inmez: darboğaz kuyruğu hat farkı hızında büyür, PTT ve
//! metin kuyruğun arkasında saniyelerce bekler, kuyruk taşınca paketler düşer.
//!
//! # Neden BBR'ın bant tahmini (BtlBw) değil
//!
//! BtlBw teslim hızıdır (ACK'lenen bayt / ACK aralığı). Bu hatta iki nedenle hattı ölçmez:
//! * ileri yönde rastgele kayıp (ürün hücresinde %20) hattı kullanır ama teslim edilmez: teslim
//!   hızı en iyi hâlde hattın `1 − kayıp` katıdır;
//! * ACK'ler 62 B/s dönüşte saniyelerce aralıkla ve kendi serileşmeleri (65–129 B → 1,05–2,08 s)
//!   kadar kayarak gelir; örnek aralığı ACK varış anlarının farkıdır.
//!
//! Ölçüldü (ürün testi, tohum 1): profil 12 500 B/s iken BtlBw gerçek 12 500 B/s hatta 3,9 kB/s,
//! 6 250 B/s hatta 2,8–2,9 kB/s okudu. Tavan olarak kullanılsa dürüst profili üçte bire düşürürdü.
//!
//! # Ölçüm: gönderim cephesinin alıcıya varış gecikmesi
//!
//! Her ACK çerçevesinin en büyük teyitli iletimi `L` (gönderim anı `s`, o ana dek bu motorun hatta
//! verdiği kümülatif tel baytı `c`) alıcıya `r` anında varmıştır. Darboğaz FIFO'dur: `L`'den önce
//! gönderilen her bayt `r`'den önce hattı geçmiş (teslim edilmiş ya da yolda rastgele düşmüş)
//! ya da kuyrukta düşmüştür. Gönderici `r`'yi bilmez ama ACK'in kendi alanlarından tam türetir:
//!
//! ```text
//! τ = ACK varışı − ack_delay − ACK'in dönüş hattında serileşmesi
//!   = r + dönüş yayılımı            (alıcı ACK'i dönüş hattı boşken çıkarır; bkz. aşağı)
//! ```
//!
//! * `ack_delay` alıcının `L`'yi aldıktan sonra ACK'i tuttuğu süredir (bütçe, hatta yol verme);
//! * ACK'in serileşmesi = (ACK datagramı + UDP/IP zarfı) / profilin dönüş hızı; ACK boyu çalışma
//!   zamanından gelir (`Sender::observe_ack_feedback`).
//!
//! İki ACK noktası `j < i` arasında `Δs = s_i − s_j` gönderim süresi, `Δτ = τ_i − τ_j` alıcıdaki
//! varış süresidir. `Δτ − Δs` cephenin kuyrukta beklediği sürenin artışıdır. Artış ölçüm
//! çözünürlüğünü aşıyorsa darboğaz o aralıkta doluydu ve aralıkta gönderilen her bayt hattan
//! geçti: hat hızı `(c_i − c_j) / Δτ`. Rastgele kayıp tahmini düşürmez (düşen bayt hattı kullandı);
//! artış yoksa örnek yoktur (hat en az gönderim hızı kadardır, ama ne kadar fazla olduğu bilinmez).
//!
//! # Gürültünün yönü
//!
//! Dönüş hattında beklemenin (ACK sıkışması) iki ucu vardır:
//! * başlangıç noktası `j`'nin beklemesi `τ_j`'yi büyütür → `Δτ` küçülür → tahmin YÜKSELİR;
//! * bitiş noktası `i`'nin beklemesi tahmini düşürür. Ürün alıcısı bütçeli ACK'i yukarı yön hattı
//!   kendi baytlarından boşalmadan çıkarmaz (`Receiver::set_uplink_yield`); bekleme `ack_delay`'in
//!   içindedir. Kalan sıkışma için tahmin, saklanan bütün başlangıç noktalarına göre aralıkların
//!   EN BÜYÜĞÜdür: tek bir gecikmiş ACK tahmini aşağı çekmez, aralıktaki boşta kalma (hat boş,
//!   gönderici bekliyor) da yalnız o aralığı düşürür.
//! * Kuyrukta düşen bayt aralığın gönderim baytına girer ama hattı kullanmamıştır → tahmin yükselir.
//!   Ölçüldü (ürün testi, gerçek 6 250 B/s, tohum 2): kuyruk taşarken gönderilmiş baytları kapsayan
//!   aralık 10 024 B/s verdi; önceki temiz aralık 6 249 B/s.
//!
//! Yükselten hatalar korumayı kaldırabilir, düşürenler yalnız hızdan yer. Bu yüzden bir örnek
//! hızı hiçbir zaman o an kullanılan tavanın üstüne çıkaramaz (`min(tavan, örnek)`); profil hattı
//! üst sınırdır. Kabul edilen bir örnek, aralığın gönderim hızının yaklaşık %5 altındadır (kuyruk
//! artışı eşiği): gürültü düzeyindeki artış örnek vermez, hız yalnız gerçek kanıtla iner.
//!
//! # Denetimli yeniden yoklama (2026-11-20)
//!
//! #217'de korunan hız oturum boyunca artmıyordu (kalıcı `min`): oturum içinde hızlanan hat
//! yeniden ölçülmüyor, yanlış bir düşük örnek (ör. dönüş hattında gecikme artışı, aşağı) oturum
//! sonuna dek kalıyordu. Artış kanıtı kuyruğun BÜYÜMEMESİDİR: iki ACK noktası arasında gönderici
//! tavana yakın hızda (≥ %90) en az [`VERIFY_MIN_SPAN_NS`] gönderdi ve cephenin kuyruk beklemesi
//! artmadıysa hat en az o gönderim hızındadır. Bu "doğrulama" tavanı bir adım yükseltir (profil
//! hattına dek). Doğrulanan aralık bugünkü tavanla gönderilmiş olmalıdır (ACK gönderimden
//! saniyeler sonra gelir; eski tavanlı aralık yeni bilgi taşımaz). Adım uyarlanır: başarılı
//! yoklamadan sonra iki katına (en çok %100); bir örnek hattı tavanın altında gösterdiğinde en
//! küçük adıma (%6,25) döner: sabit dar hatta her yoklamanın kurduğu kuyruk küçük kalır, hızlanan
//! hatta adım katlanarak toparlanma birkaç ACK aralığına iner. İlk ölçümden önce adım %50'den
//! başlar (temkinli başlangıçtan profile iki doğrulamada).
//!
//! # Temkinli başlangıç
//!
//! İlk ölçümden önce profil hızı kesin hat sayılmaz: tavan profil hattının yarısından
//! ([`START_NUM`]/[`START_DEN`]) başlar ve doğrulamalarla yükselir. Gerekçe (ürün testi, gerçek
//! 6 250 B/s): ilk bilgi taşıyan ACK'e dek profil hızında gönderim darboğaz kuyruğunu 3 s'nin
//! üstüne ve 64 paketlik sınırın dışına taşıyor, bir PTT demetinin beş iletiminin hepsi aynı
//! taşmada düşüyordu. Yarı hız, beyan edilen hattın yarısına dek iyimser profilde kuyruğu
//! büyütmez; bedeli dürüst profilde ilk doğrulamaya dek yarı hızdır.
//!
//! # Dönüş gecikmesi değişimi
//!
//! `τ` dönüş yayılımını içerir: dönüş hattının gecikmesi `d` kadar artarsa sonraki her cephe `d`
//! kadar "geç" görünür. İki koruma: (1) taban (`τ − s` en küçüğü) son [`BASE_WINDOW`] noktanın
//! en küçüğüdür, tüm oturumunki değil: kalıcı bir dönüş gecikmesi artışı birkaç ACK sonra tabana
//! girer ve kuyruk sayılmaz (kalıcı yanlış boşaltma yok); (2) tek bir gecikme basamağının verdiği
//! yanlış düşük örnek (aralığın %5'inden büyük basamak) yeniden yoklamayla geri alınır.
//!
//! # Kuyruk boşaltma
//!
//! Korumanın devreye girdiği ana dek gönderici profil hızında göndermiştir; gerçek hatta o farkın
//! kuyruğu birikmiştir. Hız yalnız ölçülen hatta inerse kuyruk kalıcı olur ve sonraki her PTT
//! demeti onun arkasında bekler. Bu yüzden her ölçüm noktasında darboğaz kuyruğu da tahmin edilir:
//! `L`'nin hattı terk ettiği andan bu yana hat `oran × süre` bayt boşaltmıştır; `L`'den sonra
//! gönderilen baytların kalanı kuyruktadır. Gönderici bu kuyruğu kendi hat modeline
//! (`TokenBucket::line_raise_backlog`) yazar: yeni datagram, gerçek kuyruk boşalınca çıkar.
//! `L`'nin hattı terk ettiği an `τ − taban`'dır; taban, görülen en küçük `τ − s` (kuyruksuz iki
//! yönlü yayılım; oturumun ilk paketleri boş hatta gider).
//!
//! Bayt sayımı kuyrukta düşmüş baytı da kuyrukta sayar (gönderici hangi baytın düştüğünü bilmez);
//! kuyruk taşmışsa tahmin gerçek kuyruktan büyüktür ve gönderici gereğinden uzun bekler — PTT ve
//! metin de bekler. Ölçüldü (ürün testi, gerçek 6 250 B/s, tohum 2–3): kuyruk ≈ 3 s iken bekleme
//! 5,5–6 s. Bu yüzden yazılan kuyruk ayrıca `L`'nin kendisinin kuyrukta beklediği süreyle
//! (`τ − s − taban`) sınırlıdır: gönderici `L`'den bu yana en az hat hızında gönderdiyse kuyruk o
//! andan küçülmemiştir, bu değer gerçek kuyruğun alt sınırıdır. Sınır, kuyruk `L`'den sonra da
//! büyüdüyse eksik boşaltır; kalan, bir sonraki ölçüm noktasında yine ölçülür ve yazılır.
//!
//! Determinizm: tamsayı aritmetiği, RNG yok, sınırlı bellek (ADR-17 / SR-AUDIT-001).

use std::collections::VecDeque;

use crate::pacer::LineModel;

/// Gönderim kaydı tavanı (SR-AUDIT-001). Tavan dolunca en eski kayıt düşer; cephesi düşmüş bir
/// kayda çarpan ACK örnek vermez (tahmin yalnız zayıflar).
const SENT_CAP: usize = 8192;
/// Saklanan ACK noktası (aralık başlangıcı) sayısı.
const POINT_CAP: usize = 8;
/// Kuyruk artışı en az aralığın bu kesri olmalı (1/20 = %5): daha küçük artış ölçüm
/// çözünürlüğündedir (zamanlayıcı, ACK gecikmesi kuantumu, hattı paylaşan küçük denetim
/// datagramları) ve örnek vermez.
const GROWTH_MIN_DIV: u64 = 20;
/// [YENİDEN-YOKLAMA] Doğrulama aralığının en kısa gönderim süresi (ns).
const VERIFY_MIN_SPAN_NS: u64 = 1_000_000_000;
/// [YENİDEN-YOKLAMA] Doğrulamada gönderim hızı tavanın en az bu binde biri olmalı (gönderici
/// uygulama sınırlıyken — ör. yalnız PTT — doğrulama yok).
const VERIFY_MIN_PERMILLE: u64 = 900;
/// [YENİDEN-YOKLAMA] Yoklama adımı (binde): başlangıç, en küçük, en büyük.
const STEP_START_PERMILLE: u64 = 500;
const STEP_MIN_PERMILLE: u64 = 62;
const STEP_MAX_PERMILLE: u64 = 1000;
/// [TEMKİNLİ-BAŞLANGIÇ] İlk tavan = profil hattı × START_NUM / START_DEN.
pub const START_NUM: u64 = 1;
pub const START_DEN: u64 = 2;
/// [DÖNÜŞ-GECİKMESİ] Taban penceresi (ACK noktası).
const BASE_WINDOW: usize = 4;

/// Ölçüm sonucu: korunan hat hızı ve darboğazda tahmin edilen kuyruk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineGuardUpdate {
    /// Korunan hat hızı (B/s, tel baytı — zarf dahil); profil hattını aşmaz.
    pub rate_bps: u64,
    /// Darboğaz kuyruğunda şu an beklediği tahmin edilen kendi baytlarımız (zarf dahil).
    pub backlog_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct SentRec {
    seq: u64,
    send_ns: u64,
    /// Bu iletim dahil hatta verilen kümülatif tel baytı.
    cum: u64,
    /// İletim anındaki gönderim tavanı (B/s, tel).
    pace: u64,
}

#[derive(Debug, Clone, Copy)]
struct Point {
    tau_ns: u64,
    send_ns: u64,
    cum: u64,
    pace: u64,
}

/// Geri bildirimi aç profilde gerçek ileri hattın tahmincisi (modül belgesi).
#[derive(Debug, Clone)]
pub struct LineGuard {
    /// Profilin yukarı yön hattı (üst sınır).
    line: LineModel,
    /// Profilin dönüş hattı (ACK serileşmesi için).
    return_line: LineModel,
    /// Hatta verilen kümülatif tel baytı (motorun iletimleri + motor dışı datagramlar).
    cum: u64,
    sent: VecDeque<SentRec>,
    points: VecDeque<Point>,
    /// Son ölçüm noktasının cephesi (seq); cephe ilerlemeyen ACK nokta üretmez.
    frontier: Option<u64>,
    /// Son [`BASE_WINDOW`] noktanın `τ − s` değerleri (ns); taban bunların en küçüğü.
    lags: VecDeque<u64>,
    /// Bir sonraki ACK çerçevesinin tel boyu (UDP gövdesi; `note_ack_wire`).
    pending_wire: Option<u64>,
    /// Korunan hat hızı (B/s, tel); `None` = ölçüm hattı profilin altında göstermedi (ya da
    /// yoklama profil hattına döndü).
    rate_bps: Option<u64>,
    /// [TEMKİNLİ-BAŞLANGIÇ · YENİDEN-YOKLAMA] Gönderim tavanı (B/s, tel; ≤ profil hattı).
    pace_bps: u64,
    /// [YENİDEN-YOKLAMA] Sonraki yoklama adımı (binde).
    step_permille: u64,
    /// Kabul edilen örnek sayısı (gözlem).
    samples: u64,
    /// [YENİDEN-YOKLAMA] Doğrulama (tavan yükseltmesi) sayısı (gözlem).
    raises: u64,
}

impl LineGuard {
    /// Profilin ileri (`line`) ve dönüş (`return_line`) hatlarıyla boş tahminci; ilk gönderim
    /// tavanı `start_pace_bps` (B/s, tel; profil hattıyla sınırlı). Ürün yolu
    /// [`Self::conservative_start`] kullanır.
    #[must_use]
    pub fn new(line: LineModel, return_line: LineModel, start_pace_bps: u64) -> Self {
        Self {
            line,
            return_line,
            cum: 0,
            sent: VecDeque::new(),
            points: VecDeque::new(),
            frontier: None,
            lags: VecDeque::new(),
            pending_wire: None,
            rate_bps: None,
            pace_bps: start_pace_bps.clamp(1, line.rate_bps.max(1)),
            step_permille: STEP_START_PERMILLE,
            samples: 0,
            raises: 0,
        }
    }

    /// [TEMKİNLİ-BAŞLANGIÇ] Ürün yolunun ilk tavanı: profil hattı × [`START_NUM`] / [`START_DEN`].
    #[must_use]
    pub fn conservative_start(line: LineModel) -> u64 {
        line.rate_bps * START_NUM / START_DEN
    }

    /// Motorun bir iletimi hatta verildi (`udp_bytes` = UDP gövdesi; zarf burada eklenir).
    pub fn on_sent(&mut self, seq: u64, now_ns: u64, udp_bytes: u64) {
        self.cum = self
            .cum
            .saturating_add(udp_bytes.saturating_add(self.line.datagram_overhead));
        if self.sent.len() >= SENT_CAP {
            self.sent.pop_front();
        }
        self.sent.push_back(SentRec {
            seq,
            send_ns: now_ns,
            cum: self.cum,
            pace: self.pace_bps,
        });
    }

    /// Motorun kararı olmadan bu uçtan aynı hatta çıkan datagram (ör. PARAM): hattı kullanır,
    /// sonraki iletimlerin kümülatif baytına girer.
    pub fn on_unsequenced(&mut self, udp_bytes: u64) {
        self.cum = self
            .cum
            .saturating_add(udp_bytes.saturating_add(self.line.datagram_overhead));
    }

    /// Bir sonraki ACK çerçevesini taşıyan datagramın tel boyu (UDP gövdesi).
    pub fn note_ack_wire(&mut self, udp_bytes: u64) {
        self.pending_wire = Some(udp_bytes);
    }

    /// Korunan hat hızı (B/s, tel); `None` = koruma devrede değil.
    #[must_use]
    pub fn rate_bps(&self) -> Option<u64> {
        self.rate_bps
    }

    /// Kabul edilen örnek sayısı (gözlem).
    #[must_use]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// [TEMKİNLİ-BAŞLANGIÇ · YENİDEN-YOKLAMA] Gönderim tavanı (B/s, tel; ≤ profil hattı).
    #[must_use]
    pub fn pace_bps(&self) -> u64 {
        self.pace_bps
    }

    /// [YENİDEN-YOKLAMA] Doğrulama sayısı (gözlem).
    #[must_use]
    pub fn raises(&self) -> u64 {
        self.raises
    }

    /// Bir ACK çerçevesi işlendi: `largest_seq` çerçevenin en büyük teyitli iletimi, `ack_delay_ns`
    /// çerçevenin bildirdiği tutma süresi, `saturated` tutma süresi kodlama tavanında mı.
    /// Koruma devredeyse güncel hız ve kuyruk tahmini döner.
    pub fn on_ack(
        &mut self,
        now_ns: u64,
        ack_delay_ns: u64,
        saturated: bool,
        largest_seq: u64,
    ) -> Option<LineGuardUpdate> {
        let wire = self.pending_wire.take()?;
        if saturated || self.frontier.is_some_and(|f| largest_seq <= f) {
            return None;
        }
        let i = self
            .sent
            .binary_search_by_key(&largest_seq, |r| r.seq)
            .ok()?;
        let rec = self.sent[i];
        let ser_ns = ns_for(
            wire.saturating_add(self.return_line.datagram_overhead),
            self.return_line.rate_bps,
        )?;
        let tau = now_ns.checked_sub(ack_delay_ns)?.checked_sub(ser_ns)?;
        let lag = tau.checked_sub(rec.send_ns)?;
        self.frontier = Some(largest_seq);
        if self.lags.len() >= BASE_WINDOW {
            self.lags.pop_front();
        }
        self.lags.push_back(lag);
        let base = self.lags.iter().copied().min().unwrap_or(lag);
        // Kayıtlar artık bu cepheden eskiye gerekmez (sonraki ACK'ler daha büyük cephe ister).
        self.sent.drain(..i);

        let current = self.rate_bps.unwrap_or(self.line.rate_bps);
        let mtu_ns = ns_for(
            crate::rate::DEFAULT_MTU + self.line.datagram_overhead,
            current,
        )
        .unwrap_or(0);
        let mut best: Option<u64> = None;
        let mut verified = false;
        let newest = self.points.len().saturating_sub(1);
        for (k, p) in self.points.iter().enumerate() {
            let (Some(ds), Some(dtau)) = (
                rec.send_ns.checked_sub(p.send_ns),
                tau.checked_sub(p.tau_ns),
            ) else {
                continue;
            };
            if ds == 0 {
                continue;
            }
            let growth = dtau.saturating_sub(ds);
            let bytes = rec.cum.saturating_sub(p.cum);
            if growth <= mtu_ns.max(ds / GROWTH_MIN_DIV) {
                // [YENİDEN-YOKLAMA] Yalnız en yeni aralık: tavana yakın gönderildi, kuyruk büyümedi.
                // Aralık bugünkü tavanla gönderilmiş olmalı: ACK gönderimden saniyeler sonra gelir,
                // eski (düşük tavanlı) aralığın doğrulaması yeni bilgi taşımaz.
                let interval_pace = p.pace.min(rec.pace);
                if k == newest && ds >= VERIFY_MIN_SPAN_NS && interval_pace >= self.pace_bps {
                    let sent_bps =
                        u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(ds))
                            .unwrap_or(u64::MAX);
                    verified = sent_bps.saturating_mul(1000)
                        >= self.pace_bps.saturating_mul(VERIFY_MIN_PERMILLE);
                }
                continue;
            }
            let est = u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(dtau))
                .unwrap_or(u64::MAX);
            best = Some(best.map_or(est, |b| b.max(est)));
        }
        if self.points.len() >= POINT_CAP {
            self.points.pop_front();
        }
        self.points.push_back(Point {
            tau_ns: tau,
            send_ns: rec.send_ns,
            cum: rec.cum,
            pace: rec.pace,
        });
        if let Some(est) = best {
            // Örnek tavanı aşamaz: kuyrukta düşen bayt aralığı şişirir, hat tavandan hızlıysa
            // kuyruk büyümezdi.
            let est = est.clamp(1, self.pace_bps.min(self.line.rate_bps));
            self.rate_bps = Some(est);
            self.pace_bps = est;
            self.samples = self.samples.saturating_add(1);
            // Ölçülmüş hatta yoklama en küçük adımdan yeniden başlar: sabit dar hatta her yoklama
            // en çok adım × ACK aralığı kadar kuyruk kurar.
            self.step_permille = STEP_MIN_PERMILLE;
        } else if verified && self.pace_bps < self.line.rate_bps {
            let up = self.pace_bps.saturating_mul(1000 + self.step_permille) / 1000;
            self.pace_bps = up.clamp(self.pace_bps + 1, self.line.rate_bps);
            self.rate_bps = self
                .rate_bps
                .filter(|_| self.pace_bps < self.line.rate_bps)
                .map(|r| r.max(self.pace_bps));
            self.raises = self.raises.saturating_add(1);
            self.step_permille = (self.step_permille * 2).min(STEP_MAX_PERMILLE);
        }
        let rate = self.rate_bps?;
        // `L` hattı `τ − taban` anında terk etti; o andan bu yana hat `rate × süre` boşalttı.
        let since_ns = now_ns.saturating_sub(tau).saturating_add(base);
        let drained = u64::try_from(u128::from(rate) * u128::from(since_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        let by_bytes = self.cum.saturating_sub(rec.cum).saturating_sub(drained);
        // `L`'nin kuyrukta beklediği süre kadar hat baytı (gerçek kuyruğun alt sınırı).
        let queued_ns = lag.saturating_sub(base);
        let by_delay = u64::try_from(u128::from(rate) * u128::from(queued_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        Some(LineGuardUpdate {
            rate_bps: rate,
            backlog_bytes: by_bytes.min(by_delay),
        })
    }
}

/// `bytes` baytın `rate_bps` hızında serileşmesi (ns); hız 0 ise `None`.
fn ns_for(bytes: u64, rate_bps: u64) -> Option<u64> {
    if rate_bps == 0 {
        return None;
    }
    u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(rate_bps)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;
    const SEC: u64 = 1_000_000_000;
    const PROFILE: u64 = 12_500;
    const RET: u64 = 62;
    const OVH: u64 = 28;

    /// Birim testleri göndericiyi profil hızında sürer: ilk tavan profil hattı.
    fn guard() -> LineGuard {
        LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            PROFILE,
        )
    }

    /// Profil hızında `n` × 1 172 B gövde (1 200 B tel) gönderir, gerçek hat `real` B/s, ileri
    /// yayılım 300 ms; her iletimin alıcıya varış anını döner.
    fn send_into(g: &mut LineGuard, real: u64, n: u64) -> Vec<u64> {
        let mut free = 0u64;
        let mut arrivals = Vec::new();
        for k in 0..n {
            let s = k * 1200 * SEC / PROFILE;
            g.on_sent(k, s, 1172);
            let done = free.max(s) + 1200 * SEC / real;
            free = done;
            arrivals.push(done + 300 * MS);
        }
        arrivals
    }

    /// Alıcı `seq`'i aldığında hemen `wire` B'lik ACK çıkarır; ACK 300 ms + serileşmeyle gelir.
    fn ack(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        wire: u64,
    ) -> (u64, Option<LineGuardUpdate>) {
        let at = arrivals[seq as usize] + 300 * MS + (wire + OVH) * SEC / RET;
        g.note_ack_wire(wire);
        (at, g.on_ack(at, 0, false, seq))
    }

    #[test]
    fn honest_line_gives_no_sample_even_with_unequal_ack_sizes() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 60);
        // 37 B ve 101 B: ACK serileşmesi 1,05 s ↔ 2,08 s. Düzeltmesiz bu 1 s'lik sahte kuyruk
        // artışı olurdu.
        for (seq, wire) in [(1, 37), (25, 101), (50, 55)] {
            let (_, u) = ack(&mut g, &arr, seq, wire);
            assert_eq!(u, None, "dürüst hatta koruma devreye girmez");
        }
        assert_eq!(g.rate_bps(), None);
    }

    #[test]
    fn slow_line_is_measured_within_one_percent_and_never_above_profile() {
        for real in [6_250u64, 10_000] {
            let mut g = guard();
            let arr = send_into(&mut g, real, 60);
            let _ = ack(&mut g, &arr, 1, 37);
            let (_, u) = ack(&mut g, &arr, 20, 101);
            let u = u.expect("kuyruk büyüdü → ölçüm");
            assert!(
                u.rate_bps.abs_diff(real) * 100 <= real,
                "gerçek {real} B/s, tahmin {} B/s",
                u.rate_bps
            );
            assert!(u.rate_bps <= PROFILE);
        }
    }

    /// Yazılan kuyruk gerçek kuyruğu aşmaz ve cephenin kendi beklediği kuyruktan az değildir;
    /// kuyrukta düşen bayt kuyruk sayılmaz.
    #[test]
    fn backlog_estimate_never_exceeds_the_bottleneck_queue() {
        for dropped in [0u64, 20_000] {
            let real = 6_250u64;
            let mut g = guard();
            let mut free = 0u64;
            let mut arr = Vec::new();
            for k in 0..60u64 {
                let s = k * 1200 * SEC / PROFILE;
                if k == 25 {
                    g.on_unsequenced(dropped);
                }
                g.on_sent(k, s, 1172);
                free = free.max(s) + 1200 * SEC / real;
                arr.push(free + 300 * MS);
            }
            let _ = ack(&mut g, &arr, 0, 37);
            let (at, u) = ack(&mut g, &arr, 20, 101);
            let u = u.expect("ölçüm");
            // Gerçek kuyruk `at` anında: gönderilmiş ama hattı henüz terk etmemiş iletimler.
            let sent_by = (0..60u64)
                .filter(|k| k * 1200 * SEC / PROFILE <= at)
                .count() as u64;
            let left_by = arr.iter().filter(|&&a| a - 300 * MS <= at).count() as u64;
            let queue = (sent_by - left_by) * 1200;
            // Cephe (seq 20) kuyrukta `varış − 300 ms − gönderim − kendi serileşmesi` bekledi.
            let own_wait = arr[20] - 300 * MS - 20 * 1200 * SEC / PROFILE - 1200 * SEC / real;
            let floor = real * own_wait / SEC;
            assert!(
                u.backlog_bytes <= queue && u.backlog_bytes + 1200 >= floor,
                "düşen {dropped} B: kuyruk {queue} B, cephenin kuyruğu {floor} B, yazılan {} B",
                u.backlog_bytes
            );
        }
    }

    /// Kuyruk taşarken gönderilmiş baytları kapsayan aralık (gönderim baytı hattın taşıdığından
    /// fazla) korunan hızı yükseltemez.
    #[test]
    fn inflated_span_cannot_raise_the_guard() {
        let real = 6_250u64;
        let mut g = guard();
        let mut free = 0u64;
        let mut arr = Vec::new();
        for k in 0..60u64 {
            let s = k * 1200 * SEC / PROFILE;
            if k == 25 {
                // 20 000 B gönderildi ama kuyrukta düştü: hattı kullanmadı.
                g.on_unsequenced(20_000);
            }
            g.on_sent(k, s, 1172);
            free = free.max(s) + 1200 * SEC / real;
            arr.push(free + 300 * MS);
        }
        let _ = ack(&mut g, &arr, 1, 37);
        let (_, u) = ack(&mut g, &arr, 20, 101);
        let first = u.expect("temiz aralık").rate_bps;
        assert!(first.abs_diff(real) * 100 <= real, "temiz ölçüm {first}");
        // 20 → 50 aralığı düşen baytı içerir: tek başına ≈ 9,7 kB/s verirdi.
        let (_, later) = ack(&mut g, &arr, 50, 101);
        assert_eq!(
            later.map(|u| u.rate_bps),
            Some(first),
            "şişkin aralık hızı yükseltmez"
        );
    }

    #[test]
    fn stale_or_saturated_acks_give_no_point() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 60);
        let _ = ack(&mut g, &arr, 10, 37);
        // Cephe ilerlemedi.
        let (_, u) = ack(&mut g, &arr, 10, 37);
        assert_eq!(u, None);
        // Tutma süresi kodlama tavanında: alıcının gerçek tutma süresi bilinmez.
        g.note_ack_wire(37);
        assert_eq!(g.on_ack(60 * SEC, 0, true, 30), None);
        // ACK boyu bildirilmeden gelen çerçeve nokta üretmez.
        assert_eq!(g.on_ack(60 * SEC, 0, false, 40), None);
        assert_eq!(g.rate_bps(), None);
    }

    /// Gönderim `send_rate` B/s ile (1 200 B tel), gerçek hat `real`, ileri yayılım 300 ms;
    /// her iletimin alıcıya varış anı.
    fn send_at(g: &mut LineGuard, send_rate: u64, real: u64, n: u64) -> Vec<u64> {
        let mut free = 0u64;
        let mut arrivals = Vec::new();
        for k in 0..n {
            let s = k * 1200 * SEC / send_rate;
            g.on_sent(k, s, 1172);
            let done = free.max(s) + 1200 * SEC / real;
            free = done;
            arrivals.push(done + 300 * MS);
        }
        arrivals
    }

    /// `seq`'in ACK'i; dönüş yayılımı 300 ms + `extra`.
    fn ack_extra(
        g: &mut LineGuard,
        arrivals: &[u64],
        seq: u64,
        extra: u64,
    ) -> Option<LineGuardUpdate> {
        let at = arrivals[seq as usize] + 300 * MS + extra + (101 + OVH) * SEC / RET;
        g.note_ack_wire(101);
        g.on_ack(at, 0, false, seq)
    }

    /// [YENİDEN-YOKLAMA] Tavanın altında gönderen ve kuyruğu büyümeyen aralık tavanı yükseltir;
    /// tavan profil hattına ulaşınca koruma kalkar.
    #[test]
    fn verified_interval_raises_the_ceiling_up_to_the_profile() {
        let mut g = LineGuard::new(
            LineModel {
                rate_bps: PROFILE,
                datagram_overhead: OVH,
            },
            LineModel {
                rate_bps: RET,
                datagram_overhead: OVH,
            },
            PROFILE / 2,
        );
        assert_eq!(g.pace_bps(), PROFILE / 2);
        let arr = send_at(&mut g, PROFILE / 2, PROFILE, 400);
        let _ = ack_extra(&mut g, &arr, 0, 0);
        let mut last = g.pace_bps();
        for seq in [20u64, 60, 120, 200, 300] {
            let _ = ack_extra(&mut g, &arr, seq, 0);
            assert!(g.pace_bps() >= last, "tavan düşmez");
            last = g.pace_bps();
        }
        assert!(g.raises() >= 1, "doğrulama yok");
        assert_eq!(g.rate_bps(), None, "dürüst hatta ölçülen hız yok");
    }

    /// [DÖNÜŞ-GECİKMESİ] Dönüş gecikmesinin tek basamağı (400 ms) yanlış düşük örnek verebilir; sonraki
    /// sabit gecikmeli aralıklar onu yeniden yoklamayla geri alır ve kalıcı kuyruk yazılmaz.
    #[test]
    fn return_delay_step_does_not_lock_a_low_rate() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 2000);
        let _ = ack_extra(&mut g, &arr, 0, 0);
        let _ = ack_extra(&mut g, &arr, 40, 0);
        // Basamak: bundan sonraki her ACK 400 ms geç döner.
        let _ = ack_extra(&mut g, &arr, 80, 400 * MS);
        let mut backlog_late = 0;
        for seq in [200u64, 400, 600, 800, 1000, 1200, 1400] {
            if let Some(u) = ack_extra(&mut g, &arr, seq, 400 * MS) {
                backlog_late = u.backlog_bytes;
            }
        }
        assert_eq!(
            g.rate_bps(),
            None,
            "dönüş gecikmesi ileri hattın daralması kaldı"
        );
        assert_eq!(g.pace_bps(), PROFILE);
        assert_eq!(backlog_late, 0, "kalıcı yanlış kuyruk");
    }

    /// [YENİDEN-YOKLAMA] Ölçülen dar hat oturum içinde hızlanınca tavan yeniden yükselir.
    #[test]
    fn measured_narrow_line_recovers_when_the_line_speeds_up() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 60);
        let _ = ack(&mut g, &arr, 1, 37);
        let _ = ack(&mut g, &arr, 20, 101);
        let low = g.rate_bps().expect("dar hat ölçüldü");
        assert!(low.abs_diff(6_250) * 100 <= 6_250 * 2);
        // Hat hızlandı: gönderici tavanda (≈ 6 250) gönderir, kuyruk büyümez.
        let mut g2 = g.clone();
        let base = arr[59] + SEC;
        let mut free = 0u64;
        let mut arr2 = Vec::new();
        for k in 0..600u64 {
            let s = base + k * 1200 * SEC / g2.pace_bps().max(1);
            g2.on_sent(1000 + k, s, 1172);
            let done = free.max(s) + 1200 * SEC / PROFILE;
            free = done;
            arr2.push(done + 300 * MS);
        }
        for k in [0u64, 100, 200, 300, 400, 500, 599] {
            let at = arr2[k as usize] + 300 * MS + (101 + OVH) * SEC / RET;
            g2.note_ack_wire(101);
            let _ = g2.on_ack(at, 0, false, 1000 + k);
        }
        assert!(
            g2.pace_bps() > low,
            "tavan yükselmedi: {} ≤ {low}",
            g2.pace_bps()
        );
    }
}
