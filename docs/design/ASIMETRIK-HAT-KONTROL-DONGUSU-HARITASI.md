# Asimetrik hat kontrol döngüsü — bileşen ilişki haritası

Tarih: 2026-11-25. Durum: çalışma belgesi (ürün kodu değiştirmez).

Bu belge, asimetrik hat kabulündeki beş hedefin (kapasite tahmini, PTT bandı ve teslimi, hız
sınırlama, düşüşten sonra hızlanma, test/CI) neden birbirinden bağımsız düzeltilemediğini
gösterir. Kaynak koddur; her satırda dosya ve fonksiyon adı verilir. Yeni ölçüm yapılmadı: sayılar
mevcut tek kancalı gerçek hat kaydından (`yerel/ham-izler-netem-v1` dalı, commit `844fb172`,
`netem-v1/kancali-1/1/`) alındı.

- Ürün kodu: `125b9afa87e731c1e666ecf63f33b7cc555dc9b1`. Satır numaraları bu commit'e göredir.
- Kip: iki uçta `cc-port` motoru, bitmap ACK, yön hızı profili açık (ileri 12 500 B/s, dönüş 62 B/s).
- Kayıt: dosya yükü var, PTT yok, tohum 1. Hat 20. saniyede 6 250 B/s'ye iner, 60. saniyede
  12 500 B/s'ye döner, 85,0–85,5 s arasında yeniden 6 250 B/s olur.

## 1. Özet

Bu kipte gönderim hızını taşınan BBR değil, `line_guard.rs` içindeki hat koruması belirler
(`sender.rs` `FeedbackStarvedRule`: pacing hat modelinin hızıdır, uçuş tavanı bağlamaz). Hat
koruması tek bir kapalı döngünün parçasıdır: gönderici → dış darboğaz → alıcının ölçümleri → 62 B/s
dönüş hattı → ACK → hat koruması → pacer. Beş hedef bu döngünün beş ayrı görünümüdür.

Döngüde on bir ayrı mekanizma ve yaklaşık yirmi beş ayar sabiti var. Her biri ölçülen tek bir
arızayı kapatmak için eklendi ve çoğu benzetimde ayarlandı. Öngörülemeyen sonuçların kaynağı
mekanizmaların birbirini devre dışı bırakmasıdır (bölüm 4); tek tek hiçbiri bozuk görünmez.

## 2. Bileşenler

| No | Bileşen | Kod | Görevi |
|---|---|---|---|
| B1 | Dönüş hattı | ürün dışı; 62 B/s | ACK, metin alındısı ve denetim datagramları tek sıradan geçer |
| B2 | ACK bütçesi ve yol verme | `receiver.rs` `ack_budget_take` (776), `yield_to_uplink` (844) | ACK'e dönüş hattının %25'i; ACK hat doluyken bekler, alarm ACK'i en çok bir kez bekler |
| B3 | Alıcı ölçümleri | `receiver.rs` `PairRate` (çift aralığı), `rx_span_field` (varış aralığı); `arrival_rate.rs` `RateDropDetector` | İleri hattın hızını ve kuyruk artışını alıcıda ölçer, ACK alanlarına yazar |
| B4 | Alarm mantığı | `receiver.rs` `on_datagram` (≈ 480–560), `PairRate::rise_alarm`, `restart_at_drop_alarm` | Daralma ya da yükseliş görülünce ACK'i bütçe sırasını beklemeden çıkarır |
| B5 | Aşağı yönlü tahmin | `line_guard.rs` `interval_sample` (1185), `overflow_estimate` (1248) | Kuyruk artışı eşiği aşınca tavanı ölçülen hıza indirir |
| B6 | Yukarı yön, çift alt sınırı | `line_guard.rs` `on_pair_rate` (1381), `pair_pacing` (1432); `pacer.rs` `set_line_pairs` | Alıcının çift ölçüsü tavanın %5 üstündeyse tavanı tek ACK'te açar |
| B7 | Yukarı yön, doğrulama ve süreli yoklama | `line_guard.rs` `on_ack_measured` (1448–1847), `begin_probe`, `probe_to` | Kuyruk büyümediyse tavanı adım adım (%6,2'den başlayıp katlanarak) dener |
| B8 | Kuyruk tahmini ve boşaltma | `line_guard.rs` `on_ack_measured` sonu (1818–1846); `pacer.rs` `line_raise_backlog` | Darboğazda biriken baytı hat modeline yazar; pacer o kadar bekler |
| B9 | Başlangıç ve boşluk bütçeleri | `line_guard.rs` `start_budget_admits`, `settle_start_budget`, boşluk kuralı (1484–1511) | İlk ACK'e ve ölçümsüz aralıklara dek dosyayı yarı hızda tutar |
| B10 | Pacer hat modeli | `pacer.rs` `LineState`, `try_send`, `refill` | Tavan hızında serileşen sanal hat; kuyruğu bir datagramı aşınca yeni datagram vermez |
| B11 | Şeritler | `sender.rs` `start_budget_holds`, öncelik kuralı | PTT, metin ve denetim bütçelerde beklemez; dış darboğazda sıra önceliği yoktur |
| B12 | PTT paketleme | `kama-app/src/ptt.rs`, 300 ms birleşik demet | Ses bandını 5 000 bit/s altında tutar |
| B13 | Dış darboğaz | ürün dışı | Tek sıralı kuyruk; dosya baytının arkasına giren metin ve ses bekler |

## 3. İlişkiler ve ağırlıkları

Ağırlık, kayıtta ölçülen süre ya da orandır. "Hedef" sütunu ilişkinin hangi kabul hedefini
taşıdığını gösterir (1 kapasite, 2 PTT, 3 hız sınırlama, 4 hızlanma, 5 test/CI).

| İlişki | Nasıl | Ağırlık (kayıttan) | Hedef |
|---|---|---|---|
| B1 → B2 → bütün tahmin | Her karar bir ACK varışında verilir | Alarm ACK'i: kanıttan varışa ≈ 1,55 s. Olağan ACK hazır olduktan sonra alıcıda 2,4–7,9 s bekliyor | 1, 3, 4 |
| B13 → B3 → B5 | Kuyruk artışı varış aralığını uzatır | Düşüşten 0,64 s sonra kanıt, 2,19 s sonra kesme | 1 |
| B13 → metin, ses | Kesmeye dek gönderici hattın iki katını sunar | Kesme anında darboğazda 2,09 s kuyruk | 2, 3, 5 |
| B8 → B10 → B13 | Boşaltma eksik yazılırsa kuyruk durur | Kesmeden sonraki üç metin 2,43 / 2,11 / 1,08 s | 2, 3, 5 |
| B5 → B7 | Her aşağı örnek yoklama adımını en küçüğe indirir | 6 223'ten 12 500'e dört başarılı yoklama, her biri en az üç ACK | 4 |
| B7 → B6 | Yoklama sürerken çift gönderimi kapalı (`pair_pacing`: `probe.is_none()`) | Toparlanmadan sonraki 24 s'nin ≈ 11 s'sinde çift yok | 4 |
| B4 → B6 | Daralma algısı çift değerlerini siler (`restart_at_drop_alarm`) | Yükseliş alarmından 273 ms sonra silindi (bölüm 4, K1) | 4 |
| Darboğaz türü → B3 → B6 | Jeton kovalı darboğaz çift aralığını kısaltır; alıcı kısa çifti atar (`PairRate::squeezed`) | Aralık / beklenen = 0,88–0,90; eşik 0,90 | 4 |
| B1 → B7, B8 | Yoklama hükmü ve kuyruk tahmini ACK'in varış anına dayanır; ACK dönüş hattında başka datagramın arkasında beklerse ileri kuyruk sanılır | 19. ACK dönüşte 2,57 s (olağan 1,45–1,56 s); yoklama hattı 12 500 iken "başarısız" | 3, 4 |
| B6, B7 → B13 | Çiftin ikinci datagramı ve her yoklama darboğaza fazla bayt bırakır | Kararlı pencerede 188 paketin 14'ü 393 ms'den uzun bekledi, en çok 1,09 s | 3 |
| B12 → B3, B9 | Ses datagramı çiftin arasına girer, öncelikli bayt bütçelerden düşer | Bu kayıtta PTT yok; birleşik koşuda ilk kez görülecek | 2, 4 |
| B6 hızı → B8 bedeli | Yukarı yön hızlıysa fazla boşaltmanın bedeli bir ACK; yavaşsa onlarca saniye | Bugün tavan 40 s sonra hâlâ 6 608 | 3 ↔ 4 |

## 4. Koddan bulunan kusurlar

Her kusur için: kod, kayıttaki karşılığı, kesinlik düzeyi.

**K1 — Yükseliş alarmının taşıdığı ölçü, ardından gelen sahte daralma algısıyla siliniyor.**
`receiver.rs` `on_datagram`: `rise_alarm()` iki geçerli çift değeri son bildirilenin %25 üstüne
çıkınca alarm verir. Aynı ACK aralığında `RateDropDetector` tetiklenirse `restart_at_drop_alarm`
çift değerlerini saklayıp sıfırlar; kanıt "yeterli" sayılınca saklanan değerler atılır
(`alarm_pair_saved = [0, 0]`). ACK yalnız algıdan sonraki çiftleri taşır.
Kayıt, 15. ACK (toparlanmadan sonraki ilk ACK): yükseliş alarmı +614 ms, daralma algısı +886 ms,
kanıt +908 ms, ACK çıkışı +1 699 ms. Gönderici bu ACK'te tavanı açmadı (6 223 → 6 608, yoklama adımı).
Kesinlik: sıra ve zamanlar kayıtla kanıtlı; ACK'in çift alanının değeri kayıtta yok (alan yazılmıyor),
bu yüzden "alan boştu" çıkarımdır. Doğrulama: aynı varış dizisini alıcı birim testinde yeniden oynatmak.

**K2 — İki yukarı yön yolu birbirini dışlıyor ve zayıf olan önce davranıyor.**
`pair_pacing()` yoklama sürerken `false` döner. Doğrulama koşulu (üç saniye tavana yakın gönderim,
kuyruk büyümedi) toparlanan hatta her zaman sağlanır; yoklama başlar, hüküm ACK'ine dek çift
gönderilmez. Kayıt: 63,2–69,9 s ve 80,2–84,8 s aralıklarında yoklama bekliyor.
Kesinlik: kod ve kayıt.

**K3 — Yoklama merdiveni hızlanma ölçütünü yapısal olarak karşılayamaz.**
Adımlar %6,2 → %12,4 → %24,8 → %49,6. 6 223 B/s'den 12 500 B/s'ye dört başarılı yoklama gerekir;
her yoklama doğrulama için iki, hüküm için bir ACK ister. En iyi durumda on iki ACK. Ölçüt üçüncü
ACK'tir. Hedef 4 yalnız çift alt sınırı yoluyla (B6) kapanabilir.
Kesinlik: kod ve aritmetik.

**K4 — Yoklama hükmü ve kuyruk tahmini, dönüş hattındaki beklemeyi ileri kuyruk sayıyor.**
`on_ack_measured` hükmü `τ = varış − ack_delay − ACK'in serileşmesi` üzerinden verir; eşik 40 ms.
Alarm ACK'i tasarım gereği dolu dönüş hattında bir datagramın arkasına girebilir; o bekleme
`ack_delay`'e girmez. 19. ACK dönüşte olağandan ≈ 1,1 s uzun kaldı; hat 12 500 B/s iken yoklama
başarısız sayıldı, adım en küçüğe döndü.
Kesinlik: kod ve kayıt (tek olay).

**K5 — Jeton kovalı darboğazda gerçek çiftler "sıkışmış" sayılıyor.**
`PairRate::squeezed` aralık beklenen serileşmenin 0,9'undan kısaysa çifti ve ardındakini atar.
Test hattının kovası (1 200 B) ikinci datagramı ≈ %12 erken bırakır. Tam serileştiren hatta bu
olmaz. Taslak PR #253 eşiği 0,85'e indiriyor.
Kesinlik: kod ve yakalama kaydı. Darboğazın türü ürün kararıdır (bölüm 7, karar 2).

**K6 — Kesmedeki boşaltma, kuyruğun ACK'ten sonra büyüyen kısmını kırpıyor.**
`backlog_bytes = min(by_bytes, by_delay)`. `by_delay` en büyük teyitli datagramın kendi beklemesidir;
o datagram ≈ 1,5 s önce gönderildi, kuyruk o zamandan beri büyüdü. `by_bytes` (sonra gönderilen −
boşalan) bu kayıtta doğru değeri verir; `min` onu kırpar. Ayrıca ilk örnek 6 638 B/s çıktı (aralık
düşüş anını kapsıyor): ikinci ACK'e dek 3,3 s boyunca hattın %6 üstünde gönderildi.
Kesinlik: kod; kayıttaki 2,09 s kuyruk ve sonraki metin gecikmeleriyle uyumlu. `min` sınırı,
kuyruğun taştığı durumda fazla beklemeyi önlemek için konmuştu; kaldırmanın bedeli orada ve kısa
daralmada ödenir (bkz. B6 hızı → B8 bedeli).

**K7 — Kör pencere fiziksel.**
Gönderici daralmayı ACK gelmeden bilemez. En küçük alarm ACK'i dönüş hattında ≈ 1,15 s serileşir,
0,3 s yol alır; kanıt ≈ 0,6 s sürer. Düşüşten ≈ 0,7 s sonra gönderilen metin 1 s sınırını aşar ve
bu durum, kesmede dosya tamamen durdurulsa bile, düşüşten ≈ 3,7 s sonrasına dek sürer. Sıfır
yüklü bir alarm çerçevesi de ≈ 0,9 s serileşir; pencere kapanmaz. Hat kullanımı %90 tutulurken bu
pencere ürün koduyla kapatılamaz.
Kesinlik: aritmetik ve kayıt. Bu bir ölçüt tanımı kararıdır (bölüm 7, karar 1).

## 5. Hedef başına baskın yol

| Hedef | Bugün | Baskın neden | Kapatan iş |
|---|---|---|---|
| 1 Kapasite tahmini | Tek tohumda geçti (2. ACK'te 6 223) | — | Değişiklik gerekmiyor; K6'nın ilk örnek kısmı iyileştirir. PTT'li koşuda `RateDropDetector`'ın "düzenli aralık" koşulu ayrıca okunmalı |
| 4 Hızlanma | Kaldı (40 s sonra 6 608) | K1, K2, K5; K3 yedek yolu yetersiz kılıyor; K4 yedek yolu geri atıyor | K1 + K2 (+ K5 kararı). Beklenen: toparlanmadan sonraki ilk ACK'te tavan ≥ 11 875 |
| 3 Hız sınırlama | Kaldı | K7 (bir metin), K6 (üç metin), kararlı pencerede B6/B7 fazlası | K6; kararlı pencere için fazlanın kaynağı koddan ayrılmalı; K7 karar |
| 2 PTT | Tek başına geçti; dosyayla sınanmadı | Darboğaz kuyruğu (hedef 3) | Hedef 3 ve 4; PTT katmanında ek iş çözmez |
| 5 Test/CI | Dört benzetim testi kırmızı | Aynı döngü; benzetimin darboğazı gerçek hattan farklı (bölüm 6) | Her kırmızı için: K1–K6'dan hangisi yeşile çevirir, hangisi benzetimin kendi varsayımı |

Sıra: önce hedef 4 (K1, K2), çünkü yukarı yön hızlanınca K6'nın bedeli (kısa daralmadan sonra boş
bekleme) bir ACK'e iner ve hedef 3 düzeltmesi mümkün olur. Hedef 2 ve 5 bunların sonucudur.

## 6. Benzetim ile gerçek hat arasındaki farklar

| Konu | Benzetim (`starved_line_sim`, `asym_product_outcome`) | Gerçek hat düzeneği |
|---|---|---|
| Darboğaz | Tam serileştiren tek sıra | Jeton kovası (1 200 B) + 64 paketlik sıra |
| Sıra uzunluğu | 8 paket (taşma mantığı buna göre ayarlı) | 64 paket; bu senaryoda taşma yok |
| Rastgele kayıp | Darboğazdan önce ya da içinde | Darboğazdan sonra |
| Dönüş hattı | Model; alındı trafiği sınırlı | Gerçek sıra; alındı her 2 s'de bir |

Benzetimde ayarlanan her sabit bu farklardan birine dayanıyor olabilir. Bir düzeltme benzetimde
geçip gerçek hatta kalıyorsa ilk bakılacak yer bu tablodur.

## 7. Sahip kararı gerektirenler

1. **Kör pencerede metin ölçütü (K7).** "Metin ≤ 1 s" ölçütü, habersiz bir yarıya inişten sonraki
   ≈ 0,7–3,7 s aralığında gönderilen metin için fiziksel olarak sağlanamaz. Seçenekler: ölçütü
   düşüşten sonraki belirli bir süre için ayrı tanımlamak (ör. pencere içinde üst sınır, dışında
   1 s), ya da ölçütü olduğu gibi bırakıp bu hedefin dosya yükü altında geçmeyeceğini kabul etmek.
2. **Kabul hattının darboğaz türü (K5).** Kovalı şekillendirici mi, tam serileştiren hat mı.
   Çift ölçüsünün anlamı buna bağlı.
3. **Yukarı yönde birincil kanıt.** K3 gereği hızlanma ölçütünü yalnız çift alt sınırı karşılar.
   Çift yolunu birincil, yoklamayı yalnız çift ölçüsü yokken çalışan yedek yapmak tahmincinin
   yapısını değiştirir (mekanizma eklemez, önceliği değiştirir).
4. **Dönüş hattında sıra (K4).** Alarm ACK'i ile metin alındısının sırası; alındı ≤ 4 s ölçütüyle
   birlikte değerlendirilmeli. Sayılar hazır değil.
5. **Kırmızı dört benzetim testinin kabuldeki yeri.** Sınıflandırma sonrası.
