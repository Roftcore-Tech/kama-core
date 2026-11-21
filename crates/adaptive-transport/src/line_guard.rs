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
//! Mekanizma: ilk tavan profil hattının [`START_NUM`]/[`START_DEN`] katıdır, hat modeli de bu
//! tavandadır (pacer'ın burst payı tavanı aşamaz) ve tavan doğrulamalarla yükselir.
//!
//! Seçim ölçümle (ürün testi `asym_product_outcome`, release, sırayla):
//! * 2026-11-20, tohum 1: 1/2'de dürüst dosya 15,0 s, 3/4'te 15,8 s ve 23,2 s (sınır 14 s;
//!   önce 8,7 s) → o gün 1/1 seçildi.
//! * 2026-11-21, tohum 1–3: 17/20'de (0,85) dürüst dosya 9,9–11,0 s; gerçek 10 000 hücresinde
//!   başlangıç PTT 2,1–2,35 s → 0,84–1,87 s. Kalan düşüş: 1 s'lik aralıkta %6'lık aşırı yük
//!   doğrulamayı geçti ve tavan profile çıktı; doğrulama sıkılaştırıldı ([`VERIFY_MIN_SPAN_NS`] =
//!   3 s, [`VERIFY_GROWTH_DIV`]). Sonra: dürüst dosya 10,1–10,3 s, 10 000 başlangıç PTT 0,90–0,97 s.
//!   Bedel: dürüst profilde dosya +1,2–2,0 s (sınır içinde).
//! * AMA 17/20 kalıcı simülasyon kapısını (`tests/line_guard_sim.rs`) bozdu: gerçek 6 250 B/s'de
//!   15 s sonrası darboğaz beklemesi 0,41 s (sınır 0,39 s) ve dürüst profilde dosya tohum 1'de
//!   15,7 → 33,3 s (dosyanın son baytları bir ACK aralığı sonrasına kayıyor). SEÇİLEN DEĞER yine
//!   1/1; 17/20 ölçülmüş seçenektir, bedeliyle sahibe gider. Sıkı doğrulama kalır (1/1'de sim
//!   kapısı geçer).
//! * Gerçek hat tavanın altındaysa (ör. 6 250 B/s) kuyruk ilk ölçüme (ikinci ACK, 3,3–7,9 s) dek
//!   büyür; gönderici ilk ölçümü öne alamaz.
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
//! # Süreli yoklama (İŞ-33 C3, 2026-11-21)
//!
//! Doğrulamadan sonra tavan kalıcı yükseliyordu. Hat değişmediyse yükseltilmiş tavan bir ACK
//! aralığı boyunca (5–8,6 s) kuyruk kurar; bu artış örnek eşiğinin (`max(MTU, Δs/20)`) altında
//! kalabildiği için birkaç aralık fark edilmeden birikir. Ölçüldü (kalıcı sim, gerçek 6 250 B/s,
//! tohum 1): tavan 6 636'ya yükseldi, kuyruk iki aralıkta 0,64 s'ye çıktı ve örnek gelmedi; ürün
//! testinde aynı hücrede PTT 1,17 s.
//!
//! Şimdi doğrulama tavanı yalnız [`VERIFY_MIN_SPAN_NS`] boyunca yükseltir (yoklama), sonra hükme
//! dek önceki tavana döner ([`LineGuard::tick`]). Hat değişmediyse yoklamanın kurduğu kuyruk
//! `adım × VERIFY_MIN_SPAN_NS`'tir (%6,2'de ≈ 0,19 s). Hükmü, cephesi yoklama bittikten sonra
//! gönderilmiş ilk ACK verir: yoklamanın başladığı noktaya göre kuyruk artışı beklenenin (hat
//! değişmediyse yoklamanın kuracağı kuyruk) yarısından küçükse hat yoklama hızındadır, tavan
//! kalıcı yükselir ve adım katlanır; değilse tavan önceki değerde kalır ve ölçülen artış hat
//! modeline yazılır (boşaltma). Örnek eşiğini aşan artış her zamanki gibi örnektir.
//!
//! # Başlangıç bütçesi ve tek ACK'ten ilk ölçüm (İŞ-33, 2026-11-21)
//!
//! İlk ACK'ten önce gönderici gerçek hattı bilmez. Profil iyimserse (gerçek hat beyanın yarısı)
//! profil hızında gönderilen her saniye darboğazda bir saniye kuyruk kurar. Bu yüzden ilk ACK
//! işlenene dek hatta verilen toplam tel baytı (motorun her iletimi ve motor dışı datagramlar)
//!
//! ```text
//! A(t) = max(P × START_BURST_NS, P/2 × t)        P = profil hattı, t = ilk iletimden beri
//! ```
//!
//! ile sınırlıdır ([`LineGuard::start_budget_admits`]). Gerçek hat `P/2` iken kuyruk en çok
//! [`START_BURST_NS`] kadar büyür (ilk patlamanın sonunda) ve `2 × START_BURST_NS`'te boşalır;
//! hat `P/2`'den hızlıysa kuyruk daha küçüktür. Bütçe yalnız öncelikli olmayan iletimleri
//! (dosya, onarım sembolü) bekletir; PTT, metin ve denetim bütçeye sayılır ama beklemez
//! (`Sender`'daki çağrı yerleri).
//!
//! Bütçe ilk ACK'le kalkar. İlk ACK alıcının ilk varış penceresinin hızını taşır
//! ([`crate::arrival_rate`]): ilk patlamanın birkaç datagramı. Gönderici pencerenin ilk ve son
//! seq'ini kendi kaydından bulur, pencere süresini `bayt / hız` olarak geri kurar ve yukarıdaki
//! iki nokta tahmincisini bu tek ACK'e uygular ([`LineGuard::on_first_window`]):
//! * kuyruk artışı pencerenin %5'ini aşıyorsa örnek: tavan ölçülen hatta iner (ilk ölçüm ikinci
//!   ACK'ten ≈ 4,6 s yerine ilk ACK'te ≈ 2 s);
//! * artış yoksa ve pencere tavanın en az %90'ıyla gönderildiyse hat en az tavan hızındadır
//!   (doğrulama): tavan profilde kalır;
//! * alan sentinel ise (eski alıcı, pencere kapanmadı) ya da pencere kurulamazsa bilgi yoktur:
//!   bütçe yine kalkar, gönderici bugünkü yola döner.
//!
//! Patlama pencereyi kapsamalıdır: pencere ilk datagramdan sonra
//! [`crate::arrival_rate::FIRST_WINDOW_BYTES`] (yarım MTU) alınmış gövde baytıdır;
//! `P × START_BURST_NS` = 6 250 B (P = 12 500 B/s) ≈ beş dosya parçası, %20 kayıpla en az ikisi
//! ≈ %99,3 olasılıkla ulaşır.
//! Pencere patlamadan sonra gönderilen datagramla kapanırsa kuyruk boşalmış olabilir: artış
//! görünmez, sonuç `NoInfo`'dur (bugünkü yol). Profil hattının patlaması pencereden küçükse (dar
//! profil) de böyledir.
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
const VERIFY_MIN_SPAN_NS: u64 = 3_000_000_000;
/// [YENİDEN-YOKLAMA] Doğrulamada kuyruk artışı aralığın bu kesrinden (1/40 = %2,5) ve bir tam
/// datagramın serileşmesinden küçük olmalı. Örnek eşiğinden (%5) sıkı: arada kalan artış ne örnek
/// ne doğrulama verir. Ölçüldü (ürün testi, gerçek 10 000, tavan 10 625, tohum 1): 1 s'lik aralıkta
/// %6'lık aşırı yük çözünürlüğün (1 MTU ≈ 115 ms) altında kaldı, tavan profile çıktı, kuyruk 1,5 s.
const VERIFY_GROWTH_DIV: u64 = 40;
/// [YENİDEN-YOKLAMA] Doğrulamada gönderim hızı tavanın en az bu binde biri olmalı (gönderici
/// uygulama sınırlıyken — ör. yalnız PTT — doğrulama yok).
const VERIFY_MIN_PERMILLE: u64 = 900;
/// [YENİDEN-YOKLAMA] Yoklama adımı (binde): başlangıç, en küçük, en büyük.
const STEP_START_PERMILLE: u64 = 500;
const STEP_MIN_PERMILLE: u64 = 62;
const STEP_MAX_PERMILLE: u64 = 1000;
/// [TEMKİNLİ-BAŞLANGIÇ] İlk tavan = profil hattı × START_NUM / START_DEN.
pub const START_NUM: u64 = 1;
pub const START_DEN: u64 = 1;
/// [DÖNÜŞ-GECİKMESİ] Taban penceresi (ACK noktası).
const BASE_WINDOW: usize = 4;
/// [BAŞLANGIÇ-BÜTÇESİ] İlk ACK'ten önce profil hızında patlama süresi (ns): gerçek hat beyanın
/// yarısıysa kurulan en büyük kuyruk (patlamanın sonunda; `2 × START_BURST_NS`'te boşalır). PTT
/// açıldığında (ürün 0,5 s) kuyruk en çok bu kadardır ve sonraki demetlerde iner: PTT sınırı (1 s) −
/// ileri yayılım (ürün testi 0,3 s) − demetin serileşmesi − zamanlama payı. Patlama ilk varış
/// penceresini (ilk datagram + yarım MTU) %20 kayıpla da kapsamalıdır: 12 500 B/s profilde 6 250 B
/// ≈ beş dosya parçası; pencere patlama dışına ancak beşten en çok biri ulaşırsa taşar (≈ %0,7).
/// Dört parçada (0,4 s) bu olasılık ≈ %2,7'dir (simde tohum 1'in kayıp dizisi). [CAL]
pub const START_BURST_NS: u64 = 500_000_000;

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
    /// İletimin UDP gövde boyu (ilk varış penceresinin bayt kuralı).
    udp: u64,
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
    /// [BAŞLANGIÇ-BÜTÇESİ] İlk ACK'e dek bütçe; `Some(None)` = henüz iletim yok, `Some(Some(t))` =
    /// ilk iletim `t` anında, `None` = bütçe kalktı.
    start: Option<Option<u64>>,
    /// [İLK-PENCERE] İlk ACK'in varış penceresinin sonucu (gözlem).
    first_window: Option<FirstWindowOutcome>,
    /// [İLK-PENCERE] Pencerenin ayrıntısı (gözlem): ilk ve son seq, gönderim süresi `Δs`, varış
    /// süresi `Δτ` (ns), aradaki tel baytı.
    first_window_detail: Option<FirstWindowDetail>,
    /// [SÜRELİ-YOKLAMA] Süren ya da hüküm bekleyen yoklama.
    probe: Option<Probe>,
    /// [SÜRELİ-YOKLAMA] Başarısız yoklama sayısı (gözlem).
    probe_failures: u64,
}

/// [İLK-PENCERE] Pencerenin ayrıntısı (gözlem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstWindowDetail {
    pub first_seq: u64,
    pub last_seq: u64,
    pub ds_ns: u64,
    pub dtau_ns: u64,
    pub wire_bytes: u64,
}

/// [SÜRELİ-YOKLAMA] Süren ya da hüküm bekleyen yoklama.
#[derive(Debug, Clone, Copy)]
struct Probe {
    /// Yoklama öncesi tavan (başarısızlıkta dönülen).
    base_pace: u64,
    /// Yoklama tavanı (başarıda kalıcı olan).
    probe_pace: u64,
    /// Yoklamanın bittiği an (ns): o andan sonra tavan hükme dek `base_pace`.
    until_ns: u64,
    /// Hat değişmediyse yoklamanın kuracağı kuyruk (ns).
    expect_ns: u64,
    /// Yoklamanın başladığı ACK noktası.
    reference: Point,
}

/// [İLK-PENCERE] İlk ACK'in varış penceresinden çıkan sonuç (gözlem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstWindowOutcome {
    /// Kuyruk büyüdü: tavan ölçülen hatta indi (B/s, tel).
    Sample(u64),
    /// Kuyruk büyümedi, pencere tavana yakın gönderildi: hat en az tavan hızında.
    Verified,
    /// Pencere kuruldu ama karar vermedi (kuyruk büyümedi, gönderim tavanın altında).
    NoInfo,
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
            start: Some(None),
            first_window: None,
            first_window_detail: None,
            probe: None,
            probe_failures: 0,
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
        if let Some(first) = self.start.as_mut() {
            first.get_or_insert(now_ns);
        }
        if self.sent.len() >= SENT_CAP {
            self.sent.pop_front();
        }
        self.sent.push_back(SentRec {
            seq,
            send_ns: now_ns,
            udp: udp_bytes,
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

    /// [SÜRELİ-YOKLAMA] Başarısız yoklama sayısı (gözlem).
    #[must_use]
    pub fn probe_failures(&self) -> u64 {
        self.probe_failures
    }

    /// [SÜRELİ-YOKLAMA] Yoklamanın süresi dolduysa tavan hükme dek yoklama öncesi değerine döner.
    /// Tavan değiştiyse `true` (gönderici hat modelini yeni tavana alır).
    pub fn tick(&mut self, now_ns: u64) -> bool {
        match self.probe {
            Some(p) if now_ns >= p.until_ns && self.pace_bps == p.probe_pace => {
                self.pace_bps = p.base_pace;
                true
            }
            _ => false,
        }
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Bütçe hâlâ geçerli mi (ilk ACK işlenmedi).
    #[must_use]
    pub fn start_budget_active(&self) -> bool {
        self.start.is_some()
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] `udp_bytes` gövdeli bir iletim şimdi bütçeye sığar mı (modül belgesi:
    /// `A(t)`). Bütçe kalktıysa daima `true`.
    #[must_use]
    pub fn start_budget_admits(&self, now_ns: u64, udp_bytes: u64) -> bool {
        let Some(first) = self.start else {
            return true;
        };
        let elapsed = first.map_or(0, |f| now_ns.saturating_sub(f));
        let p = u128::from(self.line.rate_bps);
        let burst = p * u128::from(START_BURST_NS) / 1_000_000_000;
        let half = p * u128::from(elapsed) / 2_000_000_000;
        let wire = u128::from(udp_bytes.saturating_add(self.line.datagram_overhead));
        u128::from(self.cum) + wire <= burst.max(half)
    }

    /// [BAŞLANGIÇ-BÜTÇESİ] Bütçeyi kaldırır (ilk ACK işlendi). İlk ACK kullanılabilir bir pencere
    /// taşımadıysa (eski alıcı, sentinel, `NoInfo`) o ACK'in noktası aralık başlangıcı olmaktan
    /// çıkar: cephesi bütçenin yarı hız evresinde gönderilmişti ve darboğaz o evrede boşta
    /// kalabilir; böyle bir aralığın `bayt / Δτ` tahmini hattı düşük okur (ölçüldü: kalıcı sim,
    /// gerçek 6 250 B/s, tohum 3, eski alıcı: 5 779 B/s). Taban için gecikme örneği kalır. İlk
    /// örnek bir ACK sonra, bütçesiz gönderilmiş aralıktan gelir.
    pub fn end_start_budget(&mut self) {
        if self.start.take().is_some()
            && !matches!(
                self.first_window,
                Some(FirstWindowOutcome::Sample(_) | FirstWindowOutcome::Verified)
            )
        {
            self.points.clear();
        }
    }

    /// [İLK-PENCERE] İlk ACK'in varış penceresinin sonucu (gözlem; `None` = pencere işlenmedi).
    #[must_use]
    pub fn first_window(&self) -> Option<FirstWindowOutcome> {
        self.first_window
    }

    /// [İLK-PENCERE] Pencerenin ayrıntısı (gözlem).
    #[must_use]
    pub fn first_window_detail(&self) -> Option<FirstWindowDetail> {
        self.first_window_detail
    }

    /// [İLK-PENCERE] Oturumun ilk ACK'i işlenirken: `acked` çerçevenin teyit ettiği seq'ler,
    /// `arrival_rate` çerçevenin §2.2 alanı (alıcının ilk varış penceresinin gövde hızı, B/s).
    /// Pencere göndericinin kaydından kurulur ([`crate::arrival_rate::window_end`]); kuyruk artışı
    /// pencerenin %5'ini aşarsa tavan ölçülen hatta iner. Bütçe kalktıysa, alan sentinel ise ya da
    /// pencere kurulamazsa etkisiz (`None`). Yalnız bir kez çalışır.
    pub fn on_first_window(
        &mut self,
        acked: &[u64],
        arrival_rate: u16,
    ) -> Option<FirstWindowOutcome> {
        if self.start.is_none()
            || self.first_window.is_some()
            || arrival_rate == crate::ack::ARRIVAL_RATE_SENTINEL
            || arrival_rate == 0
        {
            return None;
        }
        let rec_of = |seq: u64| {
            self.sent
                .binary_search_by_key(&seq, |r| r.seq)
                .ok()
                .map(|i| self.sent[i])
        };
        let mut seqs: Vec<u64> = acked.to_vec();
        seqs.sort_unstable();
        seqs.dedup();
        // Alıcının saydığı her seq göndericinin kaydında olmalı; olmayan pencereyi bozar.
        let mut sized = Vec::with_capacity(seqs.len());
        for s in seqs {
            sized.push((s, rec_of(s)?.udp));
        }
        let (first, last, rx_bytes) = crate::arrival_rate::window_end(sized)?;
        let (rf, rl) = (rec_of(first)?, rec_of(last)?);
        let ds = rl.send_ns.checked_sub(rf.send_ns).filter(|&d| d > 0)?;
        let dtau =
            u64::try_from(u128::from(rx_bytes) * 1_000_000_000 / u128::from(arrival_rate)).ok()?;
        let wire = rl.cum.saturating_sub(rf.cum);
        self.first_window_detail = Some(FirstWindowDetail {
            first_seq: first,
            last_seq: last,
            ds_ns: ds,
            dtau_ns: dtau,
            wire_bytes: wire,
        });
        let growth = dtau.saturating_sub(ds);
        let outcome = if dtau > 0 && growth > ds / GROWTH_MIN_DIV {
            let est = u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(dtau))
                .unwrap_or(u64::MAX)
                .clamp(1, self.pace_bps.min(self.line.rate_bps));
            self.rate_bps = Some(est);
            self.pace_bps = est;
            self.samples = self.samples.saturating_add(1);
            self.step_permille = STEP_MIN_PERMILLE;
            FirstWindowOutcome::Sample(est)
        } else {
            let sent_bps = u64::try_from(u128::from(wire) * 1_000_000_000 / u128::from(ds))
                .unwrap_or(u64::MAX);
            if sent_bps.saturating_mul(1000) >= self.pace_bps.saturating_mul(VERIFY_MIN_PERMILLE) {
                FirstWindowOutcome::Verified
            } else {
                FirstWindowOutcome::NoInfo
            }
        };
        self.first_window = Some(outcome);
        Some(outcome)
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
                if k == newest
                    && ds >= VERIFY_MIN_SPAN_NS
                    && interval_pace >= self.pace_bps
                    && growth <= (ds / VERIFY_GROWTH_DIV).min(mtu_ns)
                {
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
        let point = Point {
            tau_ns: tau,
            send_ns: rec.send_ns,
            cum: rec.cum,
            pace: rec.pace,
        };
        self.points.push_back(point);
        // [SÜRELİ-YOKLAMA] Hüküm: cephe yoklama bittikten sonra gönderildi.
        let mut probe_drain_ns = 0u64;
        if let Some(p) = self.probe.filter(|p| rec.send_ns >= p.until_ns) {
            self.probe = None;
            let growth = tau
                .saturating_sub(p.reference.tau_ns)
                .saturating_sub(rec.send_ns.saturating_sub(p.reference.send_ns));
            if best.is_none() {
                if growth.saturating_mul(2) <= p.expect_ns {
                    self.pace_bps = p.probe_pace;
                    self.rate_bps = self
                        .rate_bps
                        .filter(|_| self.pace_bps < self.line.rate_bps)
                        .map(|r| r.max(self.pace_bps));
                    self.raises = self.raises.saturating_add(1);
                    self.step_permille = (self.step_permille * 2).min(STEP_MAX_PERMILLE);
                } else {
                    self.pace_bps = p.base_pace;
                    self.probe_failures = self.probe_failures.saturating_add(1);
                    probe_drain_ns = growth;
                }
                // Hüküm aralığı yeni bir doğrulama değildir.
                verified = false;
            }
        }
        if let Some(est) = best {
            self.probe = None;
            // Örnek tavanı aşamaz: kuyrukta düşen bayt aralığı şişirir, hat tavandan hızlıysa
            // kuyruk büyümezdi.
            let est = est.clamp(1, self.pace_bps.min(self.line.rate_bps));
            self.rate_bps = Some(est);
            self.pace_bps = est;
            self.samples = self.samples.saturating_add(1);
            // Ölçülmüş hatta yoklama en küçük adımdan yeniden başlar: sabit dar hatta her yoklama
            // en çok adım × ACK aralığı kadar kuyruk kurar.
            self.step_permille = STEP_MIN_PERMILLE;
        } else if verified && self.probe.is_none() && self.pace_bps < self.line.rate_bps {
            // [SÜRELİ-YOKLAMA] Yükseltme yalnız `VERIFY_MIN_SPAN_NS` sürer; kalıcılığı hükme bağlı.
            let up = (self.pace_bps.saturating_mul(1000 + self.step_permille) / 1000)
                .clamp(self.pace_bps + 1, self.line.rate_bps);
            let expect_ns = u64::try_from(
                u128::from(up - self.pace_bps) * u128::from(VERIFY_MIN_SPAN_NS)
                    / u128::from(self.pace_bps.max(1)),
            )
            .unwrap_or(u64::MAX);
            self.probe = Some(Probe {
                base_pace: self.pace_bps,
                probe_pace: up,
                until_ns: now_ns.saturating_add(VERIFY_MIN_SPAN_NS),
                expect_ns,
                reference: point,
            });
            self.pace_bps = up;
        }
        let rate = self.rate_bps?;
        // [SÜRELİ-YOKLAMA] Yoklama sürerken kuyruk yazılmaz: yoklama hattı sınar, boşaltma onunla
        // çelişir; gerçek bir kuyruk varsa hüküm ACK'i onu boşaltır.
        if self.probe.is_some() {
            return None;
        }
        // `L` hattı `τ − taban` anında terk etti; o andan bu yana hat `rate × süre` boşalttı.
        let since_ns = now_ns.saturating_sub(tau).saturating_add(base);
        let drained = u64::try_from(u128::from(rate) * u128::from(since_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        let by_bytes = self.cum.saturating_sub(rec.cum).saturating_sub(drained);
        // `L`'nin kuyrukta beklediği süre kadar hat baytı (gerçek kuyruğun alt sınırı).
        let queued_ns = lag.saturating_sub(base);
        let by_delay = u64::try_from(u128::from(rate) * u128::from(queued_ns) / 1_000_000_000)
            .unwrap_or(u64::MAX);
        // [SÜRELİ-YOKLAMA] Başarısız yoklamanın ölçülen artışı boşaltılır; bayt sayımı (gerçek
        // kuyruğun üst sınırı) onu da sınırlar: dönüş gecikmesi basamağı artış gibi görünür ama
        // gönderilmiş fazla bayt yoksa kuyruk yazılmaz.
        let probe_drain =
            u64::try_from(u128::from(rate) * u128::from(probe_drain_ns) / 1_000_000_000)
                .unwrap_or(u64::MAX);
        Some(LineGuardUpdate {
            rate_bps: rate,
            backlog_bytes: by_bytes.min(by_delay.max(probe_drain)),
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

    /// [BAŞLANGIÇ-BÜTÇESİ] İlk ACK'e dek `A(t) = max(P × START_BURST_NS, P/2 × t)`: patlama
    /// `P × START_BURST_NS` = 6 250 B tel (beş 1 200 B datagram), sonra yarı profil hızı; bütçe
    /// kalkınca sınır yok.
    #[test]
    fn start_budget_is_a_burst_then_half_the_profile_line() {
        let mut g = guard();
        assert!(g.start_budget_active());
        let mut sent = 0u64;
        while g.start_budget_admits(0, 1172) {
            g.on_sent(sent, 0, 1172);
            sent += 1;
        }
        assert_eq!(sent, 5, "patlama 6 250 B'ye beş 1 200 B datagram sığar");
        // 1 s: P/2 × 1 s = 6 250 B < 6 000 + 1 200 → yok; 1,2 s: 7 500 B → bir datagram daha.
        assert!(!g.start_budget_admits(SEC, 1172));
        assert!(g.start_budget_admits(1200 * MS, 1172));
        g.end_start_budget();
        assert!(!g.start_budget_active());
        assert!(g.start_budget_admits(0, 1_000_000));
    }

    /// Alıcının ilk varış penceresinin gövde hızı (`arrival_rate::FirstArrivalWindow` kuralı):
    /// `seqs` artan sırada alınan seq'ler, `arrivals` her seq'in varış anı.
    fn window_field(arrivals: &[u64], seqs: &[u64]) -> u16 {
        let mut w = crate::arrival_rate::FirstArrivalWindow::default();
        for &q in seqs {
            w.on_arrival(q, 1172, arrivals[q as usize]);
        }
        w.rate_field()
    }

    /// [İLK-PENCERE] Profil hızında gönderilen ilk patlama gerçek 6 250 B/s hatta: ilk ACK'in
    /// penceresi hattı ölçer (pencere ilk datagram + bir datagram; tahmin ±%1), tavan ölçülen hatta
    /// iner. Aradaki datagram kayıpsa da (seq 1 düştü) tahmin aynı: gönderici düşen datagramın
    /// baytını kendi kaydından ekler (rastgele kayıp darboğazdan sonra; hattı kullandı).
    #[test]
    fn first_window_measures_a_slow_line_in_one_ack() {
        for acked in [vec![0u64, 1, 2, 3], vec![0, 2, 3]] {
            let mut g = guard();
            let arr = send_into(&mut g, 6_250, 6);
            let field = window_field(&arr, &acked);
            let outcome = g.on_first_window(&acked, field).expect("pencere kuruldu");
            let FirstWindowOutcome::Sample(est) = outcome else {
                panic!("örnek bekleniyordu: {outcome:?} (acked {acked:?})");
            };
            assert!(
                est.abs_diff(6_250) * 100 <= 6_250,
                "tahmin {est} (acked {acked:?})"
            );
            assert_eq!(g.pace_bps(), est);
            assert_eq!(g.rate_bps(), Some(est));
            assert_eq!(g.samples(), 1);
            // Yalnız bir kez: ikinci çağrı etkisiz.
            assert_eq!(g.on_first_window(&acked, field), None);
        }
    }

    /// [İLK-PENCERE] Dürüst hatta pencere kuyruk artışı göstermez ve tavanda gönderilmiştir:
    /// doğrulama, tavan profilde, koruma devrede değil.
    #[test]
    fn first_window_verifies_an_honest_line() {
        let mut g = guard();
        let arr = send_into(&mut g, PROFILE, 6);
        let acked = [0u64, 1, 2];
        let field = window_field(&arr, &acked);
        assert_eq!(
            g.on_first_window(&acked, field),
            Some(FirstWindowOutcome::Verified)
        );
        assert_eq!(g.pace_bps(), PROFILE);
        assert_eq!(g.rate_bps(), None);
        assert_eq!(g.samples(), 0);
    }

    /// [İLK-PENCERE] Ölçüm yoksa (sentinel: eski alıcı, pencere kapanmadı) ya da bütçe kalktıysa
    /// etkisiz; pencere uygulama sınırlıyken (gönderim tavanın %90'ının altında) karar yok.
    #[test]
    fn first_window_without_a_usable_measurement_is_inert() {
        let mut g = guard();
        let arr = send_into(&mut g, 6_250, 6);
        assert_eq!(
            g.on_first_window(&[0, 1, 2], crate::ack::ARRIVAL_RATE_SENTINEL),
            None
        );
        g.end_start_budget();
        let field = window_field(&arr, &[0, 1, 2]);
        assert_eq!(g.on_first_window(&[0, 1, 2], field), None);
        assert_eq!(g.pace_bps(), PROFILE);

        // Uygulama sınırlı: iki datagram 1 s arayla (dürüst hat, kuyruk yok).
        let mut g = guard();
        g.on_sent(0, 0, 1172);
        g.on_sent(1, SEC, 1172);
        let arr = [300 * MS + 96 * MS, SEC + 300 * MS + 96 * MS];
        let field = window_field(&arr, &[0, 1]);
        assert_eq!(
            g.on_first_window(&[0, 1], field),
            Some(FirstWindowOutcome::NoInfo)
        );
        assert_eq!(g.pace_bps(), PROFILE);
    }
}
