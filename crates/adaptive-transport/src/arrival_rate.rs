//! [VARIŞ-HIZI · §2.2] ACK V2 `arrival_rate` alanının ölçümü: oturumun ilk varış penceresi.
//!
//! # Sorun
//!
//! Geri bildirimi aç profilde (62 B/s dönüş, %25 ACK bütçesi) gönderici gerçek ileri hattı ancak
//! ikinci ACK'le ölçebiliyordu ([`crate::line_guard`]: iki ACK cephesi gerekir). İkinci ACK
//! dönüş bütçesiyle 3–5 s sonra gelir; bu sürede profilden yavaş bir hatta darboğaz kuyruğu
//! saniyelerce büyür ve PTT kuyruğun arkasında bekler (ürün testi, gerçek 6 250 / beyan 12 500,
//! tohum 1: ilk ölçüm 4,60 s'de, kuyruk 3,4 s, kuyrukta 41 datagram düştü).
//!
//! # Ölçüm
//!
//! Pencere oturumun gövdesi en az [`FIRST_WINDOW_BYTES`] olan ilk datagramıyla başlar (daha önce
//! gelen küçük datagramlar — dosya başlığı, metin, PTT — pencereye girmez: iki datagramın kendi
//! serileşme farkı kuyruk artışı gibi görünürdü). Alıcı bu datagramdan sonra gelenlerin UDP gövde
//! baytını sayar; sayım [`FIRST_WINDOW_BYTES`]'a ulaştığı varışta pencere kapanır. Alan değeri
//! `(ilk datagramdan sonraki bayt) / (son varış − ilk varış)` B/s'dir (§2.2 "receiver-gözlemli
//! pencere-oranı, ilk bw örneği"). Bu değer yalnız oturumun ilk ACK'inde taşınır, sonraki ACK'ler
//! sentinel taşır ([`crate::ack::ARRIVAL_RATE_SENTINEL`]).
//!
//! Pencere bayta göre tanımlıdır, zamana göre değil. Gönderici ACK'in teyit ettiği seq'leri
//! sıralayıp aynı kuralı ([`window_end`]) kendi gönderim kaydına uygular ve pencerenin son seq'ini
//! bulur. Böylece ilk ACK dönüş hattında başka veriye yol verip geç çıksa da (ör. alındı) iki uç
//! aynı datagram kümesini konuşur. Gönderici pencere süresini `bayt / alan` olarak geri kurar ve
//! bunu [`crate::line_guard::LineGuard`]'ın iki nokta tahmincisine verir: kuyruk artışı varsa hat
//! `pencerede gönderilen tel baytı / pencere süresi`; artış yoksa ve pencere tavana yakın
//! gönderildiyse hat en az o hızdadır (doğrulama).
//!
//! Neden alan alıcı baytıyla değil göndericinin baytıyla hız vermiyor: rastgele kayıp darboğazdan
//! sonra olduğunda düşen datagram hattı kullanmıştır ama alıcı onu görmez. Yalnız alınan baytla
//! hesaplanan oran hattı `1 − kayıp` katı okur. Alıcı kaybı göndericinin baytıyla düzeltmez
//! (kayıp nedenlerini ayıramaz); düzeltme göndericide, hat korumasının bugünkü tahmincisiyle aynı
//! kabulle yapılır (pencerede gönderilen her bayt hattı kullandı; kuyruk taşması pencerede
//! beklenmez: pencere birkaç datagramdır).
//!
//! # Varsayımlar ve sentinel
//!
//! * Darboğaz FIFO'dur, pencere sırasız varış görmez. Pencerede sırasız varış (önceki seq'ten
//!   küçük seq) olursa ölçüm geçersizdir → sentinel.
//! * Gövde boyu iki uçta aynıdır: alıcının `recv_from` boyu = göndericinin birim boyu (mühürlü UDP
//!   gövdesi). FEC onarım sembolü birkaç bayt farklı olabilir; ilk pencerede onarım sembolü
//!   beklenmez (blok kapanmadan çıkmaz).
//! * Çift datagram (aynı seq) sayılmaz.
//! * Pencere [`FIRST_ACK_HOLD_MAX_NS`] içinde kapanmazsa ölçüm yoktur → sentinel.
//!
//! Determinizm: tamsayı aritmetiği, RNG yok, sabit bellek (ADR-17 / SR-AUDIT-001).

use crate::ack::ARRIVAL_RATE_SENTINEL;
use crate::rate::DEFAULT_MTU;

/// İlk varış penceresinin ilk datagramdan sonraki gövde baytı: yarım MTU (ürünün dosya parçasıyla
/// ilk datagramdan sonra bir parça). Üç koşuldan çıkar:
/// * pencere göndericinin ilk patlamasının içinde kapanmalıdır
///   ([`crate::line_guard::START_BURST_NS`]): patlamadan sonra gönderilen datagram pencereye
///   girerse kuyruk boşalmış olabilir ve artış görünmez;
/// * ilk ACK pencereyi beklerken dönüş hattı boş kalmalıdır: 62 B/s dönüşte alıcının uygulama
///   verisi (ör. ilk metnin alındısı, ürün testinde ilk varıştan ≈ 0,3 s sonra) ACK'in önüne
///   girerse ilk ACK saniyelerce gecikir (ölçüldü: iki parçalık pencereyle ilk ACK 1,8 s yerine
///   5,5 s'de vardı);
/// * ölçüm çözünürlüğü: yarı hızlı bir hatta (beyanın yarısı) tek parçanın kuyruk artışı bir
///   parça serileşmesi kadardır (≈ 86 ms), gerçek 10 000 B/s'de ≈ 21 ms; eşik pencerenin %5'i
///   (≈ 4 ms). Aradaki kayıp datagram ölçümü bozmaz: gönderici onun baytını kendi kaydından ekler.
///
/// Kısa hâliyle pencere iki datagramdır (ilk + bir parça). Spec §2.2 notu ("pencere-oranı,
/// packet-pair DEĞİL") alıcının tek aralıktan `boy / aralık` hesaplamasına karşıdır; burada oran
/// göndericinin gönderim kaydıyla iki nokta tahmincisine çevrilir (aradaki kayıp ve bekleyen
/// datagramlar dahil). Bu yorum sahip kararı 2026-11-21 madde 2 (koşullu GO) ile spec'e yazıldı
/// (YAMA KAYDI 2026-11-21, §2.2). [CAL]
pub const FIRST_WINDOW_BYTES: u64 = DEFAULT_MTU / 2;

/// İlk ACK'in pencere için en uzun bekletmesi (ilk varıştan, ns). Beyanın yarısı hızında
/// (6 250 B/s) bir parça ve üç kayıp ≈ 0,7 s sürer; daha yavaş hat ölçüm vermez, ilk ACK bu süre
/// dolunca sentinel'le çıkar. [CAL] — spec §12 [R1] etkisi: ilk ACK bugün ilk varışlarda
/// hemen çıkıyordu; bekletme sahip kararı 2026-11-21 madde 1 (koşullu GO) ile spec'e yazıldı
/// (YAMA KAYDI 2026-11-21, §12 [R1] istisnası).
pub const FIRST_ACK_HOLD_MAX_NS: u64 = 700_000_000;

/// Alıcıda oturumun ilk varış penceresi.
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstArrivalWindow {
    /// Boyu bilinen ilk varışın anı (her boyda; bekletme ve süre sınırı buradan sayılır).
    seen_ns: Option<u64>,
    /// Görülen en büyük seq (sırasız varış denetimi; her boyda).
    max_seq: Option<u64>,
    /// Pencerenin ilk datagramının (gövde ≥ [`FIRST_WINDOW_BYTES`]) varış anı (ns).
    first_ns: Option<u64>,
    /// Pencerede görülen en büyük seq (sırasız varış denetimi).
    last_seq: u64,
    /// Pencerenin son varış anı (ns).
    last_ns: u64,
    /// İlk datagramdan sonra gelen gövde baytı.
    bytes_after_first: u64,
    /// Pencere [`FIRST_WINDOW_BYTES`]'a ulaştı.
    complete: bool,
    /// Pencerede sırasız varış görüldü ya da pencere [`FIRST_ACK_HOLD_MAX_NS`] içinde kapanmadı:
    /// ölçüm yok.
    invalid: bool,
}

impl FirstArrivalWindow {
    /// Boyu bilinen yeni (çift olmayan) bir datagram geldi.
    ///
    /// Pencere kapandıktan sonra pencerenin son seq'inden küçük bir seq gelirse (sırasız varış)
    /// ölçüm geçersizdir: gönderici pencereyi ACK'in teyit ettiği seq'lerden kurar ve o seq'i de
    /// sayardı. Pencere [`FIRST_ACK_HOLD_MAX_NS`] içinde kapanmadan gelen varış da ölçümü geçersiz
    /// kılar (pencere ilk patlamanın dışına taşmıştır).
    pub fn on_arrival(&mut self, seq: u64, udp_bytes: u64, now_ns: u64) {
        if self.invalid {
            return;
        }
        let seen_ns = *self.seen_ns.get_or_insert(now_ns);
        let reordered = self.max_seq.is_some_and(|m| seq < m);
        self.max_seq = Some(self.max_seq.map_or(seq, |m| m.max(seq)));
        if self.complete {
            if seq < self.last_seq {
                self.invalid = true;
            }
            return;
        }
        if reordered || now_ns >= seen_ns.saturating_add(FIRST_ACK_HOLD_MAX_NS) {
            self.invalid = true;
            return;
        }
        let Some(first_ns) = self.first_ns else {
            if udp_bytes >= FIRST_WINDOW_BYTES {
                self.first_ns = Some(now_ns);
                self.last_seq = seq;
                self.last_ns = now_ns;
            }
            return;
        };
        self.last_seq = seq;
        self.last_ns = now_ns.max(first_ns);
        self.bytes_after_first = self.bytes_after_first.saturating_add(udp_bytes);
        if self.bytes_after_first >= FIRST_WINDOW_BYTES {
            self.complete = true;
        }
    }

    /// Ölçüm başladı mı (boyu bilinen en az bir varış).
    #[must_use]
    pub fn started(&self) -> bool {
        self.seen_ns.is_some()
    }

    /// İlk ACK pencere kapanana dek bekletilmeli mi: boyu bilinen bir varış oldu, pencere
    /// kapanmadı, geçersiz değil ve en uzun bekletme (ilk varıştan) dolmadı. Pencere henüz
    /// başlamadıysa da (yalnız küçük datagramlar geldi) bekletir: zamanlayıcı ilk ACK'i ilk büyük
    /// datagramdan önce ölçümsüz çıkarmasın.
    #[must_use]
    pub fn holds_first_ack(&self, now_ns: u64) -> bool {
        match self.seen_ns {
            Some(seen) if !self.complete && !self.invalid => {
                now_ns < seen.saturating_add(FIRST_ACK_HOLD_MAX_NS)
            }
            _ => false,
        }
    }

    /// Alan değeri (B/s, gövde baytı): pencere kapandıysa oran, değilse sentinel. Oran
    /// sentinel'e eşit ya da büyükse sentinel ("üst bantta").
    #[must_use]
    pub fn rate_field(&self) -> u16 {
        let Some(first) = self.first_ns else {
            return ARRIVAL_RATE_SENTINEL;
        };
        if !self.complete || self.invalid {
            return ARRIVAL_RATE_SENTINEL;
        }
        let span = self.last_ns.saturating_sub(first);
        if span == 0 {
            return ARRIVAL_RATE_SENTINEL;
        }
        let rate = u128::from(self.bytes_after_first) * 1_000_000_000 / u128::from(span);
        u16::try_from(rate)
            .ok()
            .filter(|&r| r != ARRIVAL_RATE_SENTINEL && r > 0)
            .unwrap_or(ARRIVAL_RATE_SENTINEL)
    }
}

/// Göndericinin pencere kuralı: ACK'in teyit ettiği seq'ler artan sırada ve her birinin gövde
/// boyu verildiğinde pencerenin ilk ve son seq'i ile ilkinden sonraki gövde baytı. Alıcının
/// [`FirstArrivalWindow`] kuralıyla aynı: gövdesi [`FIRST_WINDOW_BYTES`]'tan küçük baştaki seq'ler
/// atlanır, ilk büyük seq pencereyi açar, ondan sonraki bayt toplamı [`FIRST_WINDOW_BYTES`]'a
/// ulaştığı seq pencereyi kapatır. Ulaşılmazsa `None`.
pub fn window_end<I>(acked_ascending: I) -> Option<(u64, u64, u64)>
where
    I: IntoIterator<Item = (u64, u64)>,
{
    let mut it = acked_ascending
        .into_iter()
        .skip_while(|&(_, udp)| udp < FIRST_WINDOW_BYTES);
    let (first, _) = it.next()?;
    let mut bytes = 0u64;
    for (seq, udp) in it {
        bytes = bytes.saturating_add(udp);
        if bytes >= FIRST_WINDOW_BYTES {
            return Some((first, seq, bytes));
        }
    }
    None
}

/// [DARALMA-ALARMI] Kısa pencere (ns): son bu kadar süredeki varışların hızı.
pub const DROP_WINDOW_NS: u64 = 500_000_000;
/// [DARALMA-ALARMI] Pencerede en az bu kadar varış.
pub const DROP_MIN_ARRIVALS: usize = 4;
/// [DARALMA-ALARMI] Alarm eşiği: kısa pencere hızı referansın ‰ bu değerinin altında.
pub const DROP_PERMILLE: u64 = 700;
/// [DARALMA-ALARMI] Referans bu süre yenilenmezse güncel hıza iner (ns).
pub const DROP_REF_HOLD_NS: u64 = 8_000_000_000;

/// [ALARM ARALIĞI · TUR 4 · #175 7087793188] Alarm kanıtının datagram zarfı (B): IPv4 (20) + UDP (8).
/// Alıcı zarfı bilmez; ürün teli IPv4'tür (çalışma zamanının hat modeli aynı zarfı kullanır). Kanıt
/// göndericinin birimiyle, tel baytıyla hesaplanır ([`AlarmEvidence`]). Gövde baytıyla hesaplansaydı
/// referanstan küçük datagramlarda göndericinin gönderim süresi eksik sayılırdı; o durumda
/// `g`'nin alıcıdaki karşılığı göndericininkini aşabilir. Sınır: gerçek zarf bundan büyükse (IPv6,
/// 48 B) türetmenin "erken bırakmaz" sonucu küçük datagramda tutmayabilir.
pub const ALARM_EVIDENCE_OVERHEAD: u64 = 28;

/// [DARALMA-ALARMI] Alıcıda ileri hattın ansızın daralmasını erken görmek için: son
/// [`DROP_WINDOW_NS`] içindeki varışlar sık (aralıklar düzenli: en büyük boşluk ortalamanın 2,5
/// katını aşmıyor; darboğazdan arka arkaya çıkan datagramlar) ve hızları son
/// [`DROP_REF_HOLD_NS`]'deki en yüksek sık-pencere hızının %70'inin altındaysa alarm.
///
/// Neden: göndericinin daralmayı öğrendiği tek yol ACK'tir ve dar dönüşte ACK bütçe sırasını bekler
/// (ürün testi, düşüş hücresi: hat 8 s'de yarıya iniyor, ölçüm ancak 12,3 s'de; kuyruk bu sürede
/// büyüyor, PTT 8 s gecikiyor). Alarm ACK'i bütçenin akış-sonu yedeğinden hemen çıkar; ACK baytı
/// bütçeyi aşmaz (yedek bütçenin içindedir). Yanlış alarm (gönderici azaldı) bir ACK'in erken
/// çıkmasıdır: bilgi taşır, bütçe aynı.
///
/// Sınır: uygulamanın az göndermesi düzensiz aralıklar üretir (PTT demetleri, metin) ve sık-pencere
/// sayılmaz. Gönderici kendisi sık ama yavaş gönderirse (tavanı ölçülen hatta indi) alarm çıkar;
/// bu da bir ACK'in erken çıkmasıdır.
#[derive(Debug, Clone, Default)]
pub struct RateDropDetector {
    /// `(varış anı, gövde baytı, seq)`; seq yalnız [`Self::on_arrival_seq`] ile gelir.
    ring: std::collections::VecDeque<(u64, u64, Option<u64>)>,
    reference_bps: u64,
    reference_ns: u64,
    fired: bool,
}

impl RateDropDetector {
    /// Boyu bilinen yeni datagram; alarm durumuna geçtiyse `true` (bir ACK'e bir kez).
    pub fn on_arrival(&mut self, now_ns: u64, udp_bytes: u64) -> bool {
        self.on_arrival_seq(now_ns, udp_bytes, None)
    }

    /// [ALARM ARALIĞI] [`Self::on_arrival`] ile aynı karar; datagramın seq'i halkada tutulur
    /// ([`Self::onset`] daralma başlangıcının seq'ini ve aradaki kayıpları bilir).
    pub fn on_arrival_seq(&mut self, now_ns: u64, udp_bytes: u64, seq: Option<u64>) -> bool {
        self.ring.push_back((now_ns, udp_bytes, seq));
        while self
            .ring
            .front()
            .is_some_and(|&(t, _, _)| t.saturating_add(DROP_WINDOW_NS) < now_ns)
        {
            self.ring.pop_front();
        }
        if self.ring.len() < DROP_MIN_ARRIVALS {
            return false;
        }
        let first = self.ring.front().map_or(now_ns, |&(t, _, _)| t);
        let span = now_ns.saturating_sub(first);
        if span < DROP_WINDOW_NS / 2 {
            return false;
        }
        let n = self.ring.len() as u64;
        let mean_gap = span / (n - 1);
        let max_gap = self
            .ring
            .iter()
            .zip(self.ring.iter().skip(1))
            .map(|(a, b)| b.0.saturating_sub(a.0))
            .max()
            .unwrap_or(0);
        if max_gap.saturating_mul(2) > mean_gap.saturating_mul(5) {
            return false;
        }
        let bytes: u64 = self.ring.iter().skip(1).map(|&(_, b, _)| b).sum();
        let rate = u64::try_from(u128::from(bytes) * 1_000_000_000 / u128::from(span.max(1)))
            .unwrap_or(u64::MAX);
        if rate >= self.reference_bps || now_ns.saturating_sub(self.reference_ns) > DROP_REF_HOLD_NS
        {
            if rate >= self.reference_bps || !self.fired {
                self.reference_bps = rate;
                self.reference_ns = now_ns;
            }
            return false;
        }
        if !self.fired
            && rate.saturating_mul(1000) < self.reference_bps.saturating_mul(DROP_PERMILLE)
        {
            self.fired = true;
            return true;
        }
        false
    }

    /// [ALARM ARALIĞI · #175 7084078903] Alarm durumundayken penceredeki daralmanın başladığı
    /// varış `(an, seq)`: en yeni varıştan geriye, her ardışık çiftin hızı alarm eşiğinin
    /// (referansın ‰ [`DROP_PERMILLE`]'i) altında kaldıkça uzanan kuyruğun ilk varışı. Ondan sonraki
    /// her varış eşiğin altındaki hızla geldi.
    ///
    /// Çiftin hızı ikincinin gövdesi / aralık; iki seq arasında alınmamış seq varsa her biri
    /// [`crate::rate::DEFAULT_MTU`] sayılır (üst sınır): hatta serileşip düşen datagram aralığı
    /// uzatır, daralma öncesi kayıplı bir çift yeni hız gibi görünmesin (12 500'de 1 000 B'lik bir
    /// kayıp aralığı 160 ms, gövdeyle 6 250, üst sınırla 13 750). Seq bilinmeyen çiftte yalnız gövde.
    ///
    /// Pencerenin ilk varışı değil: alarm pencere hızı eşiğin altına iner inmez verilir, o anda
    /// pencerenin başı hâlâ eski hızla gelmiş varışları taşır (1 000 B gövde, 80 → 160 ms aralık:
    /// alarm düşüşten 360 ms sonra, pencere hızı 8 333, eşik 8 750; kuyruğun hızı 6 250). Alarm
    /// durumunda değilse ya da en yeni çift eşiğin altında değilse `None`. Halka sınırlıdır (son
    /// [`DROP_WINDOW_NS`]).
    #[must_use]
    pub fn onset(&self) -> Option<(u64, Option<u64>)> {
        if !self.fired {
            return None;
        }
        type Arrival = (u64, u64, Option<u64>);
        let below = |a: &Arrival, b: &Arrival| {
            let missing = match (a.2, b.2) {
                (Some(sa), Some(sb)) => sb.saturating_sub(sa).saturating_sub(1),
                _ => 0,
            };
            let bytes =
                u128::from(b.1) + u128::from(missing) * u128::from(crate::rate::DEFAULT_MTU);
            bytes * 1_000_000_000 * 1000
                < u128::from(self.reference_bps)
                    * u128::from(DROP_PERMILLE)
                    * u128::from(b.0.saturating_sub(a.0))
        };
        self.ring
            .iter()
            .rev()
            .skip(1)
            .zip(self.ring.iter().rev())
            .take_while(|(a, b)| below(a, b))
            .last()
            .map(|(a, _)| (a.0, a.2))
    }

    /// [ALARM ARALIĞI · TUR 4 · #175 7087793188] Alarm durumunda penceredeki daralma başlangıcından
    /// ([`Self::onset`]) kurulan kanıt. Aralığın ilk seq'i, halkadaki seq'lerden başlangıç seq'i ile
    /// `min_seq`'in büyüğüne eşit ya da ondan büyük en küçüğüdür. `min_seq`, alıcının aralık
    /// tabanıdır: son çıkan ACK'in en büyüğünün bir fazlası. Halkadaki sonraki varışlar kanıta girer.
    /// Başlangıç yoksa ya da seq'i bilinmiyorsa `None`. Halkayı değiştirmez.
    #[must_use]
    pub fn alarm_evidence(&self, min_seq: u64) -> Option<AlarmEvidence> {
        let (_, Some(onset_seq)) = self.onset()? else {
            return None;
        };
        let bound = onset_seq.max(min_seq);
        let &(first_ns, first_udp, first_seq) = self
            .ring
            .iter()
            .filter(|&&(_, _, s)| s.is_some_and(|s| s >= bound))
            .min_by_key(|&&(_, _, s)| s)?;
        let first_seq = first_seq?;
        let mut ev = AlarmEvidence {
            first_seq,
            first_ns,
            first_wire: first_udp.saturating_add(ALARM_EVIDENCE_OVERHEAD),
            last_seq: first_seq,
            last_ns: first_ns,
            last_wire: None,
            received_wire: 0,
            received: 0,
            reference_bps: self.reference_bps,
        };
        for &(t, b, s) in &self.ring {
            if let Some(s) = s {
                ev.on_arrival(t, Some(b), s);
            }
        }
        Some(ev)
    }

    /// Bir ACK çıktı: sonraki alarm yeniden mümkün; referans alarm hızına iner (aynı daralma için
    /// ikinci alarm yok).
    pub fn on_ack(&mut self, now_ns: u64) {
        if self.fired {
            self.fired = false;
            self.reference_bps = 0;
            self.reference_ns = now_ns;
        }
    }
}

/// [ALARM ARALIĞI · TUR 4 · #175 7087793188] Alarm ACK'inin bırakma kanıtı. Alıcı burada
/// göndericinin aşağı yönlü örnek koşulunu (`line_guard::interval_sample`) kendi bildikleriyle
/// kurar. Sıra farkı da datagram sayısı da tek başına kanıt sayılmaz (sahip, 2026-11-23).
///
/// **Göndericinin koşulu.** Aralık `(f, l]` için kuyruk artışı
/// `g = rx_span − send_span − (ser(l) − ser(f))`, örnek koşulu
/// `g > max(ser(DEFAULT_MTU), send_span / 20)`. `ser(x)` = tel baytı (gövde + zarf) / `G`; `G`
/// göndericinin hat tahminidir (yoksa profil hattı). `rx_span`, ACK'in alanıdır: alıcıda `f` ile
/// `l`'nin ilk varışları arası, 256 µs'ye aşağı yuvarlanmış. `send_span`, `f` ile `l`'nin gönderim
/// anları arasıdır; içinde gönderilmiş ama alınmamış seq'ler de vardır (hatta serileşip düşen ya
/// da darboğazdan önce düşen).
///
/// **Alıcının aynası.** `ref` = daralma dedektörünün düşüş öncesi referansı (UDP gövdesi B/s). `M`
/// = [`DEFAULT_MTU`] + [`ALARM_EVIDENCE_OVERHEAD`].
/// - `V` = `(f, l]`'de alınan datagramların tel baytı + alınmayan her seq için `M` (üst sınır) +
///   `|tel(f) − tel(l)|` (boyu bilinmeyen uç için `M`).
/// - `g_a = rx_span − V / ref`, `eşik_a = max(M / ref, V / ref / 20)`.
/// - Bırakma: `g_a − 1 ns > eşik_a`. 1 ns, göndericinin tamsayı bölmelerinin payıdır.
///
/// **Bırakma erken olmaz (türetme).** Ön koşul: gönderici aralık boyunca en az referansın varış
/// hızıyla göndermiştir (`p`, tel B/s). Daralmanın haberini ancak bu ACK getireceği için gönderici
/// hızını henüz değiştirmemiştir. Referans, aynı göndericinin daha önceki varışlarının hızıdır;
/// varış hızı gönderim hızını aşamaz ve gövde baytı tel baytından küçüktür. Buradan:
/// - `ref ≤ p ≤ G`.
/// - Pacer ardışık iki iletimi birinin tel süresi kadar aralar: `send_span ≤ Σ[f,l) tel / ref` ya da
///   `Σ(f,l] tel / ref`. Seri farkı `≤ (tel(l) − tel(f))⁺ / ref` (`G ≥ ref`). Toplamda
///   `send_span + (ser(l) − ser(f)) ≤ (Σ(f,l] tel + |tel(f) − tel(l)|) / ref ≤ V / ref`; alınmayan
///   seq'in teli ≤ `M`. Yani `g ≥ g_a`.
/// - `ser(DEFAULT_MTU) = M / G ≤ M / ref` ve `send_span ≤ V / ref`, yani eşik ≤ `eşik_a`.
/// - Alıcı bıraktıysa `g > eşik`: gönderici örnek alır.
///
/// Sınır (ön koşul tutmazsa): gönderici aralıkta referanstan yavaş gönderdiyse (önceki bir ACK'le
/// tavanı inmiş, referans [`DROP_REF_HOLD_NS`] tutmada eski), alıcı bırakabilir ama gönderici örnek
/// almaz. O aralıkta darboğaz kuyruğu büyümez. ACK, adayın alarm anındaki ACK'inden erken değildir.
///
/// Biriktiriciler sabit boydadır (SR-AUDIT-001), toplamlar doygun, karşılaştırma u128.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmEvidence {
    first_seq: u64,
    first_ns: u64,
    first_wire: u64,
    last_seq: u64,
    last_ns: u64,
    /// `None`: en büyük seq'in boyu bilinmiyor (ya da henüz `first`'ten sonra varış yok).
    last_wire: Option<u64>,
    /// `(first, last]`'te alınan seq'lerin tel baytı (doygun).
    received_wire: u64,
    /// `(first, last]`'te alınan seq sayısı.
    received: u64,
    reference_bps: u64,
}

impl AlarmEvidence {
    /// Yeni (yinelenmeyen) bir seq alındı. `udp_bytes` = UDP gövdesi; `None` ise seq alınmamış
    /// sayılır (üst sınır `M`). Aralığın ilk seq'inden küçük ya da ona eşit seq etkisizdir.
    pub fn on_arrival(&mut self, now_ns: u64, udp_bytes: Option<u64>, seq: u64) {
        if seq <= self.first_seq {
            return;
        }
        let wire = udp_bytes.map(|b| b.saturating_add(ALARM_EVIDENCE_OVERHEAD));
        if let Some(w) = wire {
            self.received_wire = self.received_wire.saturating_add(w);
            self.received = self.received.saturating_add(1);
        }
        if seq > self.last_seq {
            self.last_seq = seq;
            self.last_ns = now_ns;
            self.last_wire = wire;
        }
    }

    /// Aralığın ilk seq'i (alarm ACK'inin `span_first`'i).
    #[must_use]
    pub fn first_seq(&self) -> u64 {
        self.first_seq
    }

    /// Aralığın en büyük seq'i.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// `V` (tel baytı; türetme: tip belgesi).
    fn upper_wire(&self) -> u128 {
        let mtu = u128::from(DEFAULT_MTU + ALARM_EVIDENCE_OVERHEAD);
        let span = u128::from(self.last_seq - self.first_seq);
        let missing = span.saturating_sub(u128::from(self.received));
        let ends = self
            .last_wire
            .map_or(mtu, |l| u128::from(l.abs_diff(self.first_wire)));
        u128::from(self.received_wire) + missing * mtu + ends
    }

    /// ACK'in taşıyacağı `rx_span` (ns, 256 µs'ye aşağı yuvarlanmış); alan kodlanamıyorsa `None`.
    fn rx_span_ns(&self) -> Option<u64> {
        let unit = crate::ack::ACK_DELAY_UNIT_US * 1000;
        let units = self.last_ns.saturating_sub(self.first_ns) / unit;
        (units < u64::from(crate::ack::RX_SPAN_NONE)).then_some(units * unit)
    }

    /// Kanıt yeterli mi: `g_a − 1 ns > eşik_a`, 20 ile çarpılmış tamsayı biçimiyle:
    /// `(rx_span − 1) · ref · 20 > 20 · V · 10⁹ + max(20 · M · 10⁹, V · 10⁹)`. Aralık bitmap'in
    /// kapsamını ([`crate::ack::ACK_BITMAP_MAX_BITS`]) aşarsa ACK aralığı kısaltır, kanıt sayılmaz.
    #[must_use]
    pub fn sufficient(&self) -> bool {
        let Some(rx) = self.rx_span_ns() else {
            return false;
        };
        if self.reference_bps == 0
            || self.last_seq == self.first_seq
            || self.last_seq - self.first_seq > crate::ack::ACK_BITMAP_MAX_BITS
            || rx == 0
        {
            return false;
        }
        let v = self.upper_wire().saturating_mul(1_000_000_000);
        let mtu = u128::from(DEFAULT_MTU + ALARM_EVIDENCE_OVERHEAD) * 1_000_000_000;
        let lhs = u128::from(rx - 1) * u128::from(self.reference_bps) * 20;
        let rhs = v
            .saturating_mul(20)
            .saturating_add(mtu.saturating_mul(20).max(v));
        lhs > rhs
    }

    /// Gözlem: `(g_a, eşik_a)` ns, aşağı yuvarlanmış (tip belgesi); referans yoksa `None`.
    #[must_use]
    pub fn margin_ns(&self) -> Option<(i64, i64)> {
        let rx = self.rx_span_ns()?;
        let r = u128::from(self.reference_bps);
        if r == 0 {
            return None;
        }
        let v_ns = self.upper_wire().saturating_mul(1_000_000_000) / r;
        let mtu_ns = u128::from(DEFAULT_MTU + ALARM_EVIDENCE_OVERHEAD) * 1_000_000_000 / r;
        let g = i128::from(rx) - i128::try_from(v_ns).unwrap_or(i128::MAX);
        let thr = mtu_ns.max(v_ns / 20);
        Some((
            i64::try_from(g).unwrap_or(i64::MIN),
            i64::try_from(thr).unwrap_or(i64::MAX),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    #[test]
    fn window_closes_at_the_byte_threshold_and_reports_the_rate() {
        let mut w = FirstArrivalWindow::default();
        assert!(!w.started());
        // Küçük ilk datagram (dosya başlığı) pencereyi açmaz ama bekletmeyi başlatır.
        w.on_arrival(0, 128, 290 * MS);
        assert!(w.started());
        assert!(w.holds_first_ack(300 * MS));
        w.on_arrival(1, 1046, 300 * MS);
        assert!(w.holds_first_ack(300 * MS));
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
        // Küçük datagram pencereyi kapatmaz (gövde < yarım MTU).
        w.on_arrival(2, 80, 300 * MS + 30 * MS);
        assert!(w.holds_first_ack(400 * MS));
        // 1 046 B gövde, 6 250 B/s tel hızında ≈ 172,8 ms; seq 3 düştü → 2 × 172,8 ms.
        w.on_arrival(4, 1046, 300 * MS + 346 * MS);
        assert!(!w.holds_first_ack(650 * MS));
        // 1 126 B / 0,346 s = 3 254 B/s.
        assert_eq!(w.rate_field(), 3254);
        // Pencere kapandı: sonraki varış ölçümü değiştirmez.
        w.on_arrival(5, 1046, 900 * MS);
        assert_eq!(w.rate_field(), 3254);
    }

    /// Yalnız küçük datagramlar gelirse pencere açılmaz; bekletme ilk varıştan tavana dek sürer.
    #[test]
    fn small_datagrams_alone_never_open_the_window() {
        let mut w = FirstArrivalWindow::default();
        for k in 0..10u64 {
            w.on_arrival(k, 86, k * 40 * MS);
        }
        assert!(w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS - 1));
        assert!(!w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS));
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn hold_ends_at_the_cap_and_an_unfinished_window_is_the_sentinel() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(0, 1046, 0);
        w.on_arrival(1, 100, 10 * MS);
        w.on_arrival(2, 100, 20 * MS);
        assert!(w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS - 1));
        assert!(!w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS));
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn a_reordered_arrival_voids_the_window() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(5, 1046, 0);
        w.on_arrival(4, 1046, 10 * MS);
        assert!(!w.holds_first_ack(20 * MS));
        w.on_arrival(6, 4000, 30 * MS);
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);

        // Pencere kapandıktan sonra pencerenin içine düşen sırasız seq de ölçümü geçersiz kılar.
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(0, 1046, 0);
        w.on_arrival(2, 1046, 100 * MS);
        assert_ne!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
        w.on_arrival(3, 1046, 200 * MS);
        assert_ne!(w.rate_field(), ARRIVAL_RATE_SENTINEL, "sıralı varış bozmaz");
        w.on_arrival(1, 1046, 300 * MS);
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    /// Bekletme tavanından sonra kapanan pencere ölçüm vermez (ilk ACK tavanda sentinel'le çıkmış
    /// ya da dönüş hattında beklemiş olabilir; her iki durumda pencere patlamanın dışına taşmıştır).
    #[test]
    fn a_window_that_closes_after_the_hold_cap_is_the_sentinel() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(0, 1046, 0);
        w.on_arrival(1, 100, 10 * MS);
        w.on_arrival(2, 1046, FIRST_ACK_HOLD_MAX_NS);
        assert!(!w.holds_first_ack(FIRST_ACK_HOLD_MAX_NS));
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn an_upper_band_rate_is_the_sentinel() {
        let mut w = FirstArrivalWindow::default();
        w.on_arrival(0, 1046, 0);
        // 600 B / 1 ms = 600 kB/s > u16.
        w.on_arrival(1, 600, MS);
        assert_eq!(w.rate_field(), ARRIVAL_RATE_SENTINEL);
    }

    #[test]
    fn sender_rule_finds_the_same_window_as_the_receiver() {
        let acked = [
            (10u64, 128u64),
            (11, 1046),
            (13, 1046),
            (14, 1046),
            (15, 1046),
        ];
        // Baştaki küçük datagram (128 B) atlanır; pencere 11'de açılır, 13'te kapanır.
        assert_eq!(window_end(acked), Some((11, 13, 1046)));
        assert_eq!(window_end(acked[..2].iter().copied()), None);
        let small = [(10u64, 1046u64), (11, 80), (12, 80), (14, 1046)];
        assert_eq!(window_end(small), Some((10, 14, 1206)));
        assert_eq!(window_end(std::iter::empty()), None);
    }

    #[test]
    fn drop_detector_fires_once_on_a_dense_halving_and_not_on_sparse_traffic() {
        let mut d = RateDropDetector::default();
        let mut t = 0u64;
        let mut fired = Vec::new();
        // 12 500 B/s'de 1 200 B (96 ms arayla) 3 s, sonra 6 250 B/s (192 ms) 3 s.
        while t < 3_000_000_000 {
            if d.on_arrival(t, 1200) {
                fired.push(t);
            }
            t += 96_000_000;
        }
        assert!(fired.is_empty(), "dürüst hızda alarm: {fired:?}");
        while t < 6_000_000_000 {
            if d.on_arrival(t, 1200) {
                fired.push(t);
            }
            t += 192_000_000;
        }
        assert_eq!(fired.len(), 1, "yarıya düşüşte tek alarm: {fired:?}");
        assert!(
            fired[0] < 3_000_000_000 + 700_000_000,
            "alarm geç: {}",
            fired[0]
        );
        // Seyrek, düzensiz trafik (PTT demeti benzeri): alarm yok.
        let mut d = RateDropDetector::default();
        let mut t = 0u64;
        for k in 0..200u64 {
            assert!(
                !d.on_arrival(t, if k % 7 == 0 { 1200 } else { 114 }),
                "seyrekte alarm"
            );
            t += if k % 7 == 6 { 160_000_000 } else { 40_000_000 };
        }
    }

    /// [ALARM ARALIĞI · TUR 4] Kanıtın aritmetiği, 2:1 düşüşte (referans 12 150 B/s gövde = 972 B
    /// gövde / 80 ms; yeni hat 160 ms). Başlangıçtan sonra tek datagram: `g_a` = 160 − 1 000/12 150 s
    /// ≈ 77,7 ms < eşik 1 228/12 150 s ≈ 101,1 ms; iki datagram: 320 − 164,6 ≈ 155,4 ms, yeterli.
    /// Aradaki seq alınmadıysa (darboğazdan önce düştü, süre ilerlemedi) `DEFAULT_MTU` üst sınırıyla
    /// sayılır ve yetmez. Boyu bilinmeyen varış da alınmamış sayılır.
    #[test]
    fn alarm_evidence_mirrors_the_sender_growth_rule() {
        let ev0 = AlarmEvidence {
            first_seq: 10,
            first_ns: 0,
            first_wire: 1_000,
            last_seq: 10,
            last_ns: 0,
            last_wire: None,
            received_wire: 0,
            received: 0,
            reference_bps: 12_150,
        };
        let mut ev = ev0;
        ev.on_arrival(160 * MS, Some(972), 11);
        let (g, thr) = ev.margin_ns().expect("ölçüm");
        assert!(!ev.sufficient(), "tek datagram: g {g}, eşik {thr}");
        assert_eq!((g / 100_000, thr / 100_000), (776, 1010));
        ev.on_arrival(320 * MS, Some(972), 12);
        let (g, thr) = ev.margin_ns().expect("ölçüm");
        assert!(ev.sufficient(), "iki datagram: g {g}, eşik {thr}");
        assert_eq!(g / 100_000, 1553);
        // 11 darboğazdan önce düştü: 12, 160 ms sonra.
        let mut lost = ev0;
        lost.on_arrival(160 * MS, Some(972), 12);
        assert!(
            !lost.sufficient(),
            "kayıp seq kanıt sayıldı: {:?}",
            lost.margin_ns()
        );
        // Boyu bilinmeyen varış: alınmamış sayılır (üst sınır).
        let mut no_size = ev0;
        no_size.on_arrival(160 * MS, None, 11);
        no_size.on_arrival(320 * MS, Some(972), 12);
        assert!(
            no_size.margin_ns().expect("ölçüm").0 < ev.margin_ns().expect("ölçüm").0,
            "boyu bilinmeyen varış gövdesiyle sayıldı"
        );
        // Başlangıçtan küçük ya da ona eşit seq etkisiz.
        let mut early = ev;
        early.on_arrival(400 * MS, Some(972), 9);
        assert_eq!(early, ev);
    }
}
