# Acil inceleme: Kama FEC ve QUIC kıyası (2026-11-03)

**Kapsam.** 13 araştırmacı şeridi 102 bulgu üretti. Bunların 76'sı çekişmeli doğrulamadan geçti: 43 doğru, 33 kısmen, 0 yanlış. Rapor bu 76 bulguya dayanıyor ve sayılar doğrulayıcının düzelttiği değerler. Doğrulanmayan 26 bulgu düşük etkili; kullanıldıkları yerde [doğrulanmadı] etiketi var.

**Kod sürümleri.**
- **Ürün kodu `a858f3fc`.** Bütün satır numaraları bu commit'e ait.
- **Bench `2cd9f68d`.** Ürün crate'leri a858f3fc ile diff'siz.
- **05b adayı `49864510`.** Bench ikilisi `bf18dc35`, ürün kodu 49864510 ile aynı.
  - Aynı kalanlar: §8.5 kırpması, oran düzeyinde sıfırlama reddi, K=8 blok kapanışı, `compute_send_rate` ve `update_state` (adayda sender.rs:1052).
  - Farklı olanlar:
    - Zamanlayıcı: 05b rezervi, `apply_burst_floor` ve `serve_copy_share` var (sender.rs@bf18dc35:4748-4784).
    - PreConverge kilidi büyük ölçüde giderilmiş: çıkış p50 ≤ 3,70 s, en çok 9,88 s.

**Dosya kısaltmaları.**
- `sender/budget/rate/bw/pacer/loss/receiver/tail_recovery/return_path/fusion/emergency.rs` → `crates/adaptive-transport/src/`
- `runtime/transport/wire/seam.rs` → `crates/client-core/src/`
- `ptt/file_transfer.rs` → `crates/kama-app/src/`
- `profile.rs` → `crates/kama-fec-codec/src/`
- `s1.rs` → `crates/kama-contracts/src/`
- `kama-node lib.rs` → `crates/kama-node/src/lib.rs`
- `kama_product/quic/mixed.rs` → `crates/bench/src/` (`2cd9f68d`)
- `f0_flows.rs` → `crates/kama-integration/examples/` (`2cd9f68d`)
- Spec dosyaları:
  - engine = `specs/adaptive-transport-engine-spec.md`
  - fec-core = `specs/fec-core-tdd-v3.0.md`
  - AMP = `specs/application-message-protocol.md`
  - transport-tdd = `specs/transport-layer-tdd.md`
  - noise-wire = `specs/kama-noise-wire-tdd-v2.0.md`
  - S1 = `docs/seams/KM-SEAM-AT-001.md`
  - ARCHITECTURE, INVARIANTS, EVOLUTION, ADR'ler = `docs/transport-v1/`

**Şerit etiketleri.**
- **[A] Ürün yolu.** f0_flows; Noise, S1 adaptörü ve oturum Reorder'ı dahil.
- **[B] Bench ızgarası.** Kama ↔ quinn.
- **[C] Bench PTT hücreleri.** Statik ve uyarlamalı FEC kolları.
- **[D] Bench betimsel hücreler.**
- B, C ve D'de kripto (IdentitySeam), el sıkışma ve oturum Reorder'ı yok.

**Manşet hücre.**
- Hücre: `b_r12k_d1000_l30_p1000`. 12,5 kB/s, RTT ≈ 2,1 s, %30 kayıp, 285 ms'de bir 1000 B; kol başına N = 30.
- Taban ve aday değerleri "/" ile ayrılır.
- p95 = koşu p95'lerinin medyanı. Her satırda yöntem belirtilir:
  - yakın-sıra: 32,49 / 26,15 / 8,70 s
  - doğrusal ara değer: 32,52 / 26,17 / 8,75 s

## Kısa cevap

- **Sorun 1. Manşet farkını açıklayacak bir ölçüm hatası, Kama aleyhine olanı da dahil, bulunmadı.**
  - Teslim tanımı, zaman damgası, pencere sonu ve sansür kuralı iki kolda aynı.
  - Manşet, ham veriden üç bağımsız betikle yeniden hesaplandı ve aynı çıktı [ölçüldü]:
    - gönderen teyidi p95: 32,49 / 26,15 s ↔ quinn 8,70 s
    - alıcı tek yön p95: 31,28 / 24,59 ↔ 7,16 s
  - t0 yerleşimi farklı: quinn ölçüme el sıkışmadan sonra başlıyor, Kama'nın PreConverge'i pencerenin içinde.
  - Bulunan dört asimetrinin ikisi quinn, ikisi Kama lehine. Etkisi ölçülen tek asimetri, el sıkışmanın pencere dışında kalması. Bu asimetri manşette quinn p95'ini 8,70 → 8,75 s oynatıyor [türetilmiş].
  - quinn-BBR'nin 240 kB'lık ilk penceresinin bu farktaki payı ölçülmedi.
- **Sayılar bench yolundan.**
  - Bench `kama-product` kolunda kripto, el sıkışma ve oturum Reorder'ı yok. Bu yüzden alıcı düzleminde Kama için üst sınır.
  - Gönderen düzlemi ACK dönüş yolunu da içeriyor.
  - Ürün yolunda farkın büyüyeceği bir [çıkarım]. Ürün yolunda (f0_flows) quinn kıyası yapılmadı.
- **Teslim ve kayıp ekseni.**
  - D = 60 s son tarihte ve koşu-medyanı kuralıyla teslim ayırt edilmedi. Bu ölçü dağılımın kuyruğunu görmüyor.
  - 32 hücrede 60 s'de eksik kalan koşu: taban 40, aday 17, quinn 53 / 960. Eksik koşu quinn'de daha çok [ölçüldü].
  - Manşette teslim@60 s < 1 olan koşu: taban 13/30, aday 5/30, quinn 1/30 [ölçüldü].
  - Kısa son tarihlerde quinn önde: manşette 2 s'de 0,010 ↔ 0,655 [doğrulanmadı].
- **F0, "FEC'li Kama ↔ QUIC" kıyası değildi.**
  - 32 kıyas hücresindeki 1920 Kama koşusunun yalnız 29'unda onarım çıktı, toplam 84 onarım. Manşette 60 koşuda 2 onarım var [ölçüldü].
  - Ölçülen kol fiilen paritesiz Kama: ARQ, kopya, kuyruk probu ve kuruluş probu.
  - p95 farkının ARQ ve pacing kaynaklı olduğu bir [çıkarım]; mekanizma ayrımı bu veriyle yapılmadı.
- **Manşette açığın yeri.**
  - Kama, gereken hızın %50–55'iyle gönderiyor: 2644 / 2933 ↔ 5288 B/s [ölçüldü].
  - Hız teslim hızına bağlı (rate.rs:164-177, sender.rs:4245-4255) [kod].
  - İlk iletimi kaybolan birimin ikinci iletimi p50'de 3,72 RTT (taban) / 1,96 RTT (aday) sonra çıkıyor [ölçüldü].
  - Kayıp kanıtı geldikten sonra yeniden gönderim, yeni DATA'nın arkasında p50 4,08 s bekliyor. Bu arada medyan 8 yeni orijinal çıkıyor (taban; sender.rs:4285-4294) [ölçüldü+kod].
  - Turun kalanında ACK kaybının ve kayıp ilanı gecikmesinin payı ölçülmedi.
- **Hücre sınıfına göre [ölçüldü, B].**
  - Gönderen bekleyişi yalnız r12k d1000'in l0, l20_p1000 ve l30_p1000 hücrelerinde baskın.
  - Kalıcı hız açığı yalnız l30_p1000'de var: btlbw·(1−p)/sunulan = 0,46–0,53.
  - Kurtarma bileşeni gönderen bekleyişi kadar ya da ondan büyük olan hücreler: l30_p65 ile r12k d300'ün 1 KB kayıplı hücreleri.
  - Nokta karşılaştırmasında Kama'nın açığı iki yerde: r12k'nın 16/16 hücresi ve r1k'nın 65 B kayıplı 5 hücresi. r1k'nın 11/16 hücresinde Kama daha kısa.
- **Fiziksel tavan.**
  - 12,5 kB/s'de quinn ideal RFC-9002 ARQ sınırında duruyor. r12k'nın 16 hücresinde sınırdan farkın medyanı +0,03 s (aralık −0,02…+0,83 s).
  - Manşette quinn'in alıcı tek yön p95'i 7,16 s [ölçüldü], modelin ideal ARQ değeri 6,58 s [türetilmiş].
  - Bu hücrelerde r > 0 paritenin açabileceği p95 aralığının üst sınırı, modeldeki ideal FEC − ideal ARQ farkı:
    - Ürün geometrisiyle (K=8, E=1024) fark yalnız üç r12k d1000 hücresinde ≥ 1,28 s: −1,28 / −2,10 / −2,62 s.
    - K = 4 ile en çok −4,3 s [türetilmiş, ideal gönderen].
  - 1 kB/s'deki kayıplı 1 KB hücrelerinde quinn, kararlı durumda bile sınırın +1,9…+15,4 s üstünde [ölçüldü].
  - Bunlar kazanma öngörüsü değil, ölçülmesi gereken tavanlar.
- **Sahibin cümlesi: "FEC'li sistem gecikmede ARQ'dan iyidir".** ARQ her kolda açık; kıyaslanan eksen marjinal r. Doğru okuma şu:
  - ARQ açıkken r > 0 parite, r = 0'a göre bloğun her konumunda ARQ turunu yalnız üç şart birlikte sağlanınca kaldırır [türetilmiş]:
    1. blok dolumu artı onarım serileştirmesi ARQ turundan kısa olmalı;
    2. R kayba yetmeli;
    3. hatta bant payı olmalı.
  - Şart 1 yalnız geç konumlarda sağlansa da kısmi p95 kazancı oluşabiliyor.
  - İdeal gönderen modelinde ürün geometrisi ve ölçülen öneri düzeyi kullanıldı. Geometri: K=8, E=1024, yalnız K dolunca kapanan blok. Öneri düzeyi: her kayıp düzeyinde 250‰. Bu koşullarda 1 s'yi aşan p95 kazancı yalnız iki grupta var: üç r12k d1000 hücresi ve r1k'nın 1 KB d1000 hücreleri.
  - r1k 65 B'nin 8/8 hücresinde r > 0 kolu, yalnız-ARQ kolundan kötü [türetilmiş, üst sınır].
  - Parite akabildiğinde ARQ turunun çoğu kalkıyor [ölçüldü]:
    - Bench'te statik K2/E64 ve K4/E64 kolları, RTT ≈ 2 s hücreleri.
    - İlk teslimi FEC ile olan birimlerin kaybolan birimlere oranı 0,76–0,88. Payda nominal p·N, teslim sırasız.
    - Bu sonuç K=2/4, E=64 içindir; 0x05 profiline aktarılamaz.
    - Aynı kollarda d300'deki üstünlüğün çoğu FEC'ten değil, gönderen kilidinden kaçıştan geliyor.
- **Sorun 2. Evet, kablosuz kod var; ama gecikmeyi belirleyen sınıf bileşim ve politika.**
  - FEC alt sistemi kablolu ve telde çalışıyor. C statik kolunda rs2, 820/820 koşuda onarım gönderdi ve 819/820'de kurtardı [ölçüldü].
  - Kablosuz ya da dışa taşınmamış kod gerçekten var [kod]:
    - FloorCap feature arkasında.
    - DeadlineBounded uçtan uca kablosuz.
    - return_bw ölçülüyor ama yalnız gözlemciye gidiyor.
    - `rounded_to_zero` ClientStats'a taşınmıyor.
    - Kısmi blok kapanışının canlı oturum tetiği yok.
  - Paritenin tele çıkmamasını ya da zamanında çıkmamasını belirleyen dokuz kalem var: #1–#4, #6–#9 ve #14. Bunların çoğu kablolu ama bileşimde etkisiz olan kurallar ve birbirini bozan politikalar. #5 ise kurtarılan birimin uygulamaya teslimini geciktiriyor.
  - Spec'e göre üç grup var:
    - Tek başına spec'e uyup birleşince etkisiz kalanlar: #1–#4.
    - Spec'ten sapanlar: #6–#8, #12, ve #9'da süre dolumu.
    - Spec'te karşılığı olmayanlar: #13, #14.
  - #1'de kaynak spec'in kendisi; kod spec'i doğru uyguluyor.
- **Sahibin bileşim hipotezi.**
  - Sorduğu her halka kodda ve ham veride doğrulandı. Oturum geneli sıra da buna dahil, ama o halka yalnız ürün yolunda geçerli ve bench gecikmesini açıklamıyor.
  - Hipotezin dışında kalanlar:
    - kova kırpması (#3)
    - hız kilidi (#1)
    - uçtan uca kablosuz son tarih (#9)
- **Düzeltmelerin çoğu sahip kararı gerektiriyor.** Mühürlü bir politikaya, spec metnine, ADR'ye, önceki bir insan kapısı kararına ya da S1/AMP sözleşmesine dokunuyorlar. Bu kalemler son bölümde listeli.

## Sorun 1 — Ölçüm hatası mı?

**İki kolda simetrik olanlar [kod+ölçüldü]**
- **Teslim.** İkisi de gönderende, taşıma düzeyinde ölçülüyor.
  - Kama: birimin ilk ACK'i ya da FEC-RECOVERED'i (`kama_product.rs:855-867`).
  - quinn: `stopped()` → `Ok(None)`, yani eş bütün akış verisini ACK'lemiş (`quic.rs:657-673`).
- **Damga ve pencere.**
  - `sent_ns` planlı sunum anında alınıyor.
  - Plandan en büyük sapma: Kama 8,68 ms (aday 8,48 ms), quinn 2,42–5,72 ms (iki bağımsız hesap).
  - Pencere t0 + (n−1)·I + D'de bitiyor; teslim edilmeyen mesaj ∞ sayılıyor.
  - offer_qfull 0, rc 0.
- **Yeniden üretim.**
  - D'de 4920/4920, B'de 600/600 koşu birebir tuttu.
  - Aile sayımı (Kama önde / quinn önde / ayırt edilemez) 2 / 22 / 8 olarak aynen çıktı.
- **Ağ.**
  - Gerçekleşen kayıp üç kolda eşit düzeyde: ileri yön medyanı 0,297 / 0,313 / 0,310.
  - Kayıpsız ilk iletimde çıkış → varış p50 1,118 s.

**Bulunan asimetriler**

| asimetri | kimin lehine | büyüklük |
|---|---|---|
| quinn'in el sıkışması pencere dışında (medyan 2,93 s, aralık 1,76–5,49 s) ve quinn t0'a hazır bir RTT örneğiyle giriyor. Kama'nın PreConverge'i (≈ 2,4 s) pencere içinde. | quinn | Manşette p95 8,70 → 8,75 s [türetilmiş; el sıkışma süresince sunulan mesajların el sıkışma sonuna kadar beklediği varsayımıyla]. Farkın pozitif olduğu 28 hücrenin 15'inde fark, quinn'in kurulum medyanından küçük. Bu yüzden istatistik testle quinn'in önde çıktığı 22 hücre, kurulumu hariç tutan ('ss') tanıma bağlı; soğuk başlangıç tanımıyla sayımın nasıl değişeceği ölçülmedi [türetilmiş]. Nokta sayımı 28 → 25 [doğrulanmadı]. |
| quinn-BBR'nin ilk penceresi 240 000 B; app-limited Startup'ta kayba kör. Koşu sonu cwnd 324–348 kB; quinn koşu boyunca Startup'ta kalmış görünüyor [türetilmiş]. | quinn (beyanlı "en iyi rakip" seçimi) | İlk pencerenin payı ölçülmedi. RFC ilk pencereli BBR de büyük olasılıkla kısa süre pencereye takılırdı [türetilmiş]. |
| r1k hücrelerinde quinn'in 1228 B'lık el sıkışma paketleri t0'dan sonra da kuyrukta. | Kama | Kayıpsız r1k'da quinn fazlasının tamamı bu kuyruktan geliyor [ölçüldü]. Kama'nın istatistik testte önde olduğu iki hücrenin (r1k_d300_l0_p65, r1k_d1000_l0_p65) bundan kaynaklandığı [doğrulanmadı]. |
| Bench Kama kolunda kripto (IdentitySeam +16 B) ve Reorder yok. | Kama | Alıcı düzleminde Kama için üst sınır. |

**Ölçüm asimetrisi değil, ürün maliyeti: ACK politikası.**
- Gönderen teyidi ölçüsü, Kama'nın ACK politikasını da içeriyor: 2 DATA'da bir ACK (receiver.rs:76).
- Gönderen − alıcı farkının medyanı: 1,337 / 1,259 ↔ 1,114 s [ölçüldü].
- Alıcıya varıp pencere içinde ACK'i dönmeyen mesaj: 17 / 9 ↔ 0 [ölçüldü].

**Kama aleyhine bir yapıt var mı?**
- Manşet farkını açıklayacak büyüklükte, Kama aleyhine bir tanım ya da damga yapıtı bulunmadı.
- quinn, 32 hücrenin gönderen düzleminde 28'inde, alıcı düzleminde 27'sinde düşük [ölçüldü].
- Fark iki ölçüde aynı mertebede, yani ACK dönüş yolundan gelmiyor:
  - tabanda tek yön 24,1 s ↔ gönderen 23,8 s
  - adayda tek yön 17,43 s ↔ gönderen 17,45 s
- r1k 1000 B hücrelerinde Kama'nın ACK tutması gönderen ölçüsünü 1,6–3,8 s şişiriyor. Alıcı ölçüsüne geçilince istatistik sayım 22'den 19'a iniyor [doğrulanmadı].

**F0 neyi kıyasladı [ölçüldü]**
- **B ızgarası.**
  - 2560 Kama koşusunun 185'inde parite çıktı: taban 60 koşuda 130 çerçeve, aday 125 koşuda 237.
  - Bu 185 koşunun 162'si r25 ve r1k 65 B hücrelerinde.
  - Alıcıda FEC kurtarması 29 / 32, yeniden gönderim 61 542 / 62 471. Kurtarma/yeniden gönderim oranı %0,049.
- **Paritenin kaynağı.**
  - 125 koşuda parite akış bittikten sonra kısmi bloktan çıktı (123'ü r25, 2'si r1k).
  - 60 koşuda akış içinde çıktı: 53'ü ret-atlama yolundan, 7'si BtlBw ≥ 7875 B/s yolundan.
- **D ızgarası.** FEC'in tele çıktığı kayda değer iki kıyas var:
  - d_mix dosyalı:
    - taban: 24/30 koşu, 371 parite, 252 alındı, 75 kurtarma, 2 539 764 B yeniden gönderim
    - aday: 401 parite, 272 alındı, 142 kurtarma
  - d_cc_r12k_d1000_l10: 5/10 koşu, 22 parite, 11 kurtarma, 168 yeniden gönderim.
  - Kurtarma/yeniden gönderim payı en çok %3–6; bu bir üst sınır.
  - Dosya parçalarının BtlBw'yi yükselttiği açıklaması bir [çıkarım]: d_mix koşuları BtlBw yazmıyor.
- **Ürün yolu [A].**
  - PTT-yalnız oturumda parite pratikte sıfır; koşu başına en çok 6 parite. Parite çıkan koşu (taban / aday):
    - r12k_d300 dosyasız: 0/30 / 1/30
    - r1k: 4/30 / 1/30
    - r12k_d1000: 6/10 / 3/10
  - Parite esas olarak yüksek BtlBw'li dosya koşularında çıkıyor: r12k_d300 dosyalıda 18/30 / 23/30.
  - BtlBw ile parite arasındaki ayrışma yalnız r12k'da görülüyor.
- **Pozitif kontrol [C].**
  - rs2 (K2/E64): 820/820 koşuda onarım gönderdi, 819/820'de kurtardı.
  - rs4 (K4/E64): 818/820'de kurtardı.
  - Kollar saf K2R2/K4R4 değil: `--fec-r` koşuya göre 1, 2 ya da 4.
  - Düzenek pariteyi görebiliyor; ürün kolundaki sıfır parite ürünün kendi davranışı.
- **Sonuç.**
  - S4 kıyası; ARQ, kopya, kuyruk probu ve kuruluş probu kullanan Kama ile quinn arasında yapıldı. FEC'e gecikme kazancı ya da zararı yazılamaz.
  - FEC açık yapılandırmanın bayt maliyeti PARAM_SET kontrol baytı: koşu başına medyan r12k'da 306–357 B, r1k'da 408–459 B; tx'in ≈ %2'si [ölçüldü].
  - Bu baytın gecikmeye etkisi ölçülmedi.

**Fizik: FEC ARQ turunu ne zaman kaldırır [türetilmiş]**

Model, netem FIFO'nun Monte Carlo benzetimi.
- Kayıpsız hücrelerde ölçüme 0,01–0,02 s yakın.
- Kayıplı hücrelerde model doğrulaması betimsel: kilitsiz arq kıyası, sonuca bağlı tanımlanmış bir alt kümeye dayanıyor (tam_c.py:459).
- Model ideal gönderen varsayıyor; pacing ve kilit yok. Bu yüzden aşağıdaki kazançlar üst sınır.

Blokta j konumundaki kayıp birim için FEC bekleyişi: W_j = (K−1−j)·I + R·s_rep/C. Aşağıdaki üç şart birlikte sağlanırsa bloğun her konumunda ARQ turu kalkar:
1. Her konum için (K−1)·I + R·s_rep/C < D_rt. Burada D_rt ≈ max(9/8·srtt, RTT + I).
2. p95'in ARQ'dan kurtulması için kalan kayıp r_res = q·P[Bin(K+R−1, q) ≥ R] < 0,05.
3. Toplam hat yükü ρ < ~0,8.

Şart 1 her konumda sağlanmasa da p95 kazancı kısmen oluşabiliyor (geç konumlar; onarımın ACK'i tetiklemesi).

**K=8 için hesap:**
- q = 0,1, R = 2: r_res 0,023; şart sağlanıyor.
- q = 0,2, R = 2: r_res 0,113; R = 4 gerekiyor.
- q = 0,3, R = 4 (profil tavanı): r_res 0,129; R = 6 gerekiyor.
- %30 kayıpta, onarımlar da kaybolduğu için blok çözülemezliği R = 2 ile 0,617, R = 4 ile 0,276.

**Onarım boyu E.**
- E = 1024 onarım, 65 B'lık DATA paketinin 8,2 katı (L2 1094 / 134 B). 1 kB/s'de tek onarımın serileştirmesi 1,094 s.
- Fragmenter kapalıyken 65 B için katalogda tek seçenek E=1024; E=64'te birim tavanı 53 B (runtime.rs:236-245).
- Yüke uydurulmuş varsayımsal E=76 ile model p95:

| hücre | E=1024 → E=76 | ideal ARQ |
|---|---|---|
| r12k_d1000_l30_p65 | 8,00 → 3,81 s | 6,29 s |
| r1k_d1000_l30_p65 | 13,35 → 4,42 s | 7,15 s |
| r1k_d300_l30_p65 | 12,70 → 2,49 s | 3,22 s |

**Ürün geometrisiyle ideal p95 kazancı (üst sınır):**
- **r12k.**
  - 1 s'yi aşan kazanç yalnız üç hücrede: d1000_l10_p65 −2,10 s, d1000_l10_p1000 −1,28 s, d1000_l30_p1000 −2,62 s.
  - r12k_d300_p1000: −0,05…−0,70 s.
  - Diğer r12k hücrelerinde ≈ 0 ya da zarar: +0,12, +0,19, +1,72, +4,55 s.
  - K = 4 ile en çok −4,3 s.
- **r1k 1 KB.** d1000_l10 −3,52 s, d1000_l30 −1,86 s, d300_l30 −0,32 s.
- **r1k 65 B.** 8/8 hücrede r > 0 kolu (ARQ açık), yalnız-ARQ kolundan kötü. Neden: E=1024 onarımın bant yükü.

**PTT (son tarih OWD + 0,8 s).**
- İdeal ARQ'nun zamanında oranı ≈ 1 − q.
- Ürün geometrisi (K8/R2/E1024):
  - r12k'da ARQ'ya yalnız +0,01…+0,07 ekliyor.
  - r1k'da oran 0,04–0,07'ye düşüyor (ARQ 0,65–0,89); hat yükü 2,5–4,1.
- K2/E64'ün ideali 0,91–0,99.
- Bloğun bütün konumlarını korumak için gereken K (yaklaşık, E = 64 varsayımıyla):
  - d300 (T1-800), b = 60/120/200 ms: K ≤ 7 / 3 / 2
  - d1000 (T2-800): K ≤ 8 / 4 / 2

**Ölçülen ARQ turu maliyeti [ölçüldü, B]**

Tek kayıp maliyeti şöyle tanımlı: iki iletimle ulaşan mesajın ilk varış p50'si eksi tek iletimle ulaşan mesajınki, ikisi de ilk tel iletiminden itibaren.

| hücre | tek kayıp maliyeti (taban / aday) | srtt (taban / aday) |
|---|---|---|
| r12k_d300_l30_p65 | 1,32 / 0,86 s | 0,68 / 0,64 s |
| r12k_d300_l30_p1000 | 2,74 / 1,88 s | 0,84 / 0,70 s |
| r12k_d1000_l30_p65 | 3,37 / 2,61 s | ≈ 2,2 s |
| r12k_d1000_l30_p1000 | 7,97 / 4,20 s | 2,31 / 2,12 s |
| r1k_d300_l30_p1000 | 7,77 / 5,78 s | 2,98 / 1,83 s |

- Değerler 1,2–3,5 srtt aralığında. Üç iletim gerekirse (ntx = 3) p50 15,92 / 9,64 s.
- Değerler varışa koşullu: tabanda 3000 mesajın 2864'ü ulaştı.
- Zamanında giden paritenin tur başına kazandırabileceği alan [türetilmiş]: d1000 / 1 KB'de 4–8 s, d300'de 0,9–2,7 s.

**Ölçülen statik E=64 kolları [ölçüldü, C; bench, sırasız teslim, payda nominal p·N]**
- Kaybolan birimlerin FEC ile ilk teslim payı:
  - r12k/d1000: 0,76–0,88 (rs2 0,78–0,88, rs4 0,76–0,88)
  - r12k/d300: 0,26–0,58
  - r1k/d300: 0,11–0,42
- d300'de geç çerçeve (> OWD + 500 ms) payı: rs kollarında 0,10–0,18, arq kolunda 0,50–0,80.
- **Çekince: d300'deki üstünlüğün çoğu kilitten kaçıştan geliyor.** c_r12k_d300_l20_b120 hücresinde:
  - rs2 − arq ölçülen farkı +0,63.
  - FEC'in zamanında kurtardığı pay yalnız ≈ +0,03 (ideal +0,18).
  - rs2 koşularının 0/30'u kilitli; arq koşularının 23/30 ve 30/30'u kilitli.
  - Gönderen kuyruğu p95: 92 / 87 ms ↔ 1405 / 3312 ms.
  - Parite bekleyişi 2543 / 2774 ms.
- d1000'de FEC payı ≈ +0,15 (ideal +0,18).
- "Kilitli" tanımı sonuca bağlı (tam_c.py:459). "FEC baytları kilidi önlüyor" yalnız bir hipotez.
- Sıralı teslim karşı-olgusunda rs2'nin son tarih payı 0,944 → 0,662 [türetilmiş].

**Bu bölümden çıkan.** Sahibin cümlesi bu şartlar altında okunmalı. Değişmesi gerekebilecek varsayımlar K, E, blok kapanışı, profil seçimi ve hız modeli. Hangisinin değişeceği kanıt zincirinde ölçülecek.

**Manşet hücrede açığın ayrışması [ölçüldü, B]**

| bileşen | taban / aday | quinn |
|---|---|---|
| p95 (yakın-sıra): gönderen teyidi; alıcı tek yön | 32,49 / 26,15 s; 31,28 / 24,59 s | 8,70 s; 7,16 s |
| sunum → ilk DATA çıkışı (gönderen bekleyişi) | koşu medyanı 13,09 / 11,24 s; hiç çıkmayan mesajlar dahil havuz p95 58,96 / 41,59 s | sunumların koşu başına medyan %100'ü (en düşük %92) 10 ms içinde çıkıyor |
| bekleyişin mesaj sırasıyla büyümesi (msg 20/40/60/80, taban) | 6,5 / 11,8 / 14,3 / 17,6 s; hız açığı imzası | düz |
| gönderim hızı, t ∈ [3, 28) s (gereken 5288 B/s) | 2644 / 2933 B/s | 5285 B/s |
| kayıp eğimi, r12k_d1000_p1000, l0/l10/l20/l30 (gereken 3702/4113/4627/5288 B/s) | taban 4031/4347/3842/2644; aday 4051/4368/4368/2933 B/s | 3745/4128/4612/5285 B/s |
| iki rejim (taban) | kilitli 13 koşu p95 medyanı 59,7 s; diğer 17 koşu 26,3 s (Spearman −0,77) | — |
| DrainRtprop | koşu sonu pace = 0,75 × btlbw: rep2/6/11/16 (c.err `est=`) | — |
| ARQ turu: 1. → 2. iletim p50 | 7,96 s (3,72 RTT) / 4,19 s (1,96 RTT) | ölçülemedi (şifreli) |
| kayıp kanıtı → yeniden gönderim | p50 4,08 s, p95 20,5 s / p50 0,51 s, p95 3,59 s; arada medyan 8 / 1 yeni orijinal | — |
| koşu başına p95(teyit − ilk iletim beklemesi) [türetilmiş: muhasebe ayrıştırması, karşı-olgu değil] | 19,78 / 11,15 s | toplam p95 8,70 s |
| en yavaş %10: gönderen kuyruğu + kurtarma + ACK dönüşü | 40,2 + 13,8 + 2,1 = 56,0 s / 35,0 + 7,6 + 2,5 = 45,1 s | — |
| PreConverge tuzağı (taban 3/30 koşu, 29,6–64,8 s) | bu koşular çıkarılınca taban p95 (doğrusal) 32,52 → 29,60 s | — |
| FEC | öneri 250‰ → kırpma sonrası medyan 15‰ (aralık 1–48) → r = 0; 33 296/33 296 örnek | — |

**Kayıp eğimi.**
- Düşüş tekdüze değil: l0'dan l10'a hız artıyor. Düşüş tabanda l10'dan sonra, adayda yalnız l20 → l30 arasında görülüyor.
- Kilitli koşularda hız zamanla yavaşça iniyor. Max-filtrenin gürültüsü (1−p) çarpanını kısmen telafi ediyor.
- quinn'in l30'daki gönderim/sunulan oranı 1,43.

**Fark iki bileşenli: gönderen bekleyişi ve kurtarma.**
- İki bileşen aynı pacer'ı ve aynı DATA-önce sırasını paylaşıyor (sender.rs:4277-4311) [kod].
- Kurtarma bekleyişi tek başına (recovery_wait p95 14,38 s) quinn'in toplam p95'inden (8,75 s) büyük.
- Aday, tabana göre p95'te 6,3 s önde [ölçüldü]:
  - gönderen bekleyişi p95'i yalnız 0,55 s düşüyor (20,06 → 19,51 s);
  - kurtarma bekleyişi p95'i 14,38 → 8,87 s'ye iniyor.
  - p95'ler toplanmadığı için bu bir gösterge. Kazancın 05b rezervine bağlanması bir [çıkarım].
- 19,78 s'lik muhasebe değeri, bekleyiş kaldırılınca p95'in o değere ineceği anlamına gelmiyor. Gönderim hızı sunulan yükün altında kaldığı sürece bekleme başka bir sınıfa kayar.
- Hız düzeltmesinin farkın ne kadarını kapatacağı ölçülmedi. "p95 quinn düzeyine iner" karşı-olgusu doğrulanamaz.

**Manşet dışı hücreler [ölçüldü, B]**
- **l30_p65.** Kurtarma baskın: recovery_wait p95 7,49 s, sender_wait p95 4,24 s.
- **l10_p65.** 2,61 s'ye karşı 2,36 s.
- **r12k d300 1 KB kayıplı hücreler (l10/l20/l30).** Kurtarma bacağı (N95 − sınır) +0,81 / +1,08 / +1,61 s; gönderen bekleyişi (S95) 0,59 / 1,02 / 1,66 s. İkisi aynı düzeyde.
- **Kayıpsız r12k d1000.** p95 farkı +3,2 s. Fark, ilk ≈ 40–50 mesajı geciktiren kuruluş geçişinden geliyor (#11).

**TABAN.md'de düzeltilmesi gereken ifadeler**

Bu düzeltmelerin erratum/corrigendum yoluyla mı (ERRATUM-2 #135 deseni) yapılacağı **[SAHİP]** kararı.

- **`:18` "Teslim oranı ayırt etmiyor, gecikmede quinn önde."** Önerilen metin:
  > Teslim, D = 60 s (1000 B) / 30 s (65 B) son tarihte ve koşu-medyanı kuralıyla ayırt edilmedi. Bu ölçü dağılımın kuyruğunu görmüyor. Manşette teslim@60 s < 1 olan koşu taban 13/30 (en düşük 0,02), aday 5/30, quinn 1/30. 32 hücrede 60 s'de eksik kalan koşu taban 40, aday 17, quinn 53 / 960. Gecikmede quinn, bench yolunda fiilen paritesiz koşan Kama'dan önde (1920 koşunun 29'unda parite). Kama kolunda kripto ve Reorder yok; alıcı düzleminde Kama için üst sınır.

  Kısa son tarihlerde fark büyük (manşette 2 s'de 0,010 ↔ 0,655) [doğrulanmadı].
- **`:52`.** Aynı D/medyan kaydı eklenmeli. Ayrıca şunlar yazılmalı: p95 gönderen teyidi; quinn el sıkışması pencere dışında; farkın pozitif olduğu 28 hücrenin 15'inde fark quinn kurulum medyanından küçük.
- **`:38` "FEC bugün tele çıkmıyor."** Önerilen metin:
  > B'de 2560 koşunun 185'inde parite çıktı; 162'si r25 ve r1k 65 B hücrelerinde. 125'inde parite akış bittikten sonra kısmi bloktan çıktı (123'ü r25). Kurtarma/yeniden gönderim %0,049.
- **`:5` "Kama için üst sınırdır."** Önerilen metin: "alıcı düzleminde üst sınır; gönderen teyidi ölçüsü ACK dönüş yolunu da içerir."
- **Eklenmemesi gereken cümle.** "Teslim fizikçe doymuş" türünden bir cümle eklenmemeli: 1 − q^k yalnız ideal ARQ'yu bağlar.

## Sorun 2 — Gereksinim → spec → kod → davranış

Tablo etkiye göre sıralı. #15 ve #16 bu revizyonda eklendi; mevcut numaralar bozulmasın diye sona kondu. Satır numaraları `a858f3fc`'ye ait. Her ölçülen etkinin şeridi köşeli parantezde.

| # | kusur | sınıf | spec | dosya:satır | ölçülen etki | en dar düzeltme | sahip kararı |
|---|---|---|---|---|---|---|---|
| 1 | Hız teslim hızına kilitli; onarıma bant ayrılmıyor. Tek kaçış yolu ProbeUp; kayıp vetosu probun başarıyla tamamlanmasını çok olasılıksız kılıyor. Bekleyen her kayıp probu RecoverArq'a kesiyor, proba atfedilen kayıp iptal ettiriyor [çıkarım; başarı/abort sayacı yok]. | spec boşluğu + birbirini bozan politika (INV-RATE-1'in harfine uyuluyor, amacı sağlanmıyor) | engine §8.2 :661, §7.1 :594-601, §9.1 :722, :243, :1072 ↔ :83-85 | `rate.rs:164-177`; `sender.rs:4245-4255`, `:3993-3996`, `:4076`, `:4090-4117`, `:4132-4139`; `bw.rs:98-116`, `:233` | [B] Manşette gereken hızın %50–55'i. Gerekenin altında kalan koşu 28/30 ve 22/30. BtlBw < sunulan olan koşularda p95 taban 50,3 s (n=20), diğerlerinde 22,8 s (n=10); aday 36,69 s (n=19) ↔ 16,44 s (n=11) [ölçüldü; ilişki, nedensellik ayrıştırılmadı]. pending_lost doluluk payı l10/l20/l30: 0,09 / 0,26 / 0,62. Aday l30'da 17/30 koşu ProbeUp'a girdi. | Hıza talep türevli alt sınır: sunulan × (1 + retx/parite payı); tavan cwnd/srtt olarak kalır. ProbeUp'ın kayıp vetosu kaldırılır. | **Evet**: INV-RATE-1 metni (CLAUDE.md §4, engine :83-85), fec-core §0.8 "AYRI-DECISION", engine :1072, 05b startup-floor'unun süreğen hâli |
| 2 | §8.5 kırpması, K=8 yarım-yukarı yuvarlama ve oran düzeyindeki sıfırlama reddi birleşince r = 0 çıkıyor. | kablolu, bileşimde etkisiz; spec içi çelişki | engine §8.5 :691-697, §10.2 :790 ↔ fec-core §8 :279, :288 | `budget.rs:26-31`, `:38-41`; `sender.rs:3744`, `:3763-3797`, `:3786`, `:1026-1031`, `:412`, `:4595` | [B] Kayıplı 24 hücrede önerilerin yalnız %1,19'u r ≥ 1 verebiliyor. Manşette 33 296/33 296 örnek r = 0. | Sıfırlama reddi sembol düzeyine taşınır: kırpılmamış r ≥ 1 ve kırpılmış r = 0 ise r = 1. `profile_target_unmet` sayacı eklenir. | **Evet**: ADR-fec-v1-role Decision §3 ve EVOLUTION §9.2 donması; ADR-minimal-parity-exception; AMENDMENT-3 ↔ fec-core §8 yönü |
| 3 | Kova tavanı her olayda baştaki DATA boyuna kırpılıyor. 1052–1058 B'lık parite, kuyruk doluyken kovaya sığmıyor. | kablolu, bileşimde etkisiz | engine §13 BURST_CAP [CAL] | `sender.rs:2366-2381`, `:1563`; `pacer.rs:46-53`, `:70-73` | [B] Manşette kuyruğun dolu olduğu 31 226 örneğin yalnız 62'sinde jeton ≥ 1027 (hepsi tam 1027). [B ızgarası toplamı] Blok latch edilip hiç parite çıkarmayan koşu 11 / 7. Kuyruk doluyken eşik 26,45 kB/s [türetilmiş]. PTT eşiği modele bağlı [türetilmiş]: T = 60/120/200 ms için 11,4 / 7,2 / 4,8 kB/s ((1058+86)/(T+0,04) modeli) ya da 17,5 / 8,8 / 5,3 kB/s (1052/T modeli). [A] Dosyasız hücrelerde konuşma içi pacer hızı medyanı 223–738 B/s; konuşma içinde çıkan parite taban 1, aday 0. Dosyalı hücrelerde aday konuşma penceresinde 146 / 42 / 218 parite gönderdi; eşik hıza bağlı. 1058 B'lık tavan ürün kolunda TokenDenied düzeyinde ölçülmedi. | Bekleyen parite başının boyu kova alt sınırına katılır (05b burst_floor deseni). Jeton üretmez, INV-RATE-1'e dokunmaz. | **Evet**: burst_floor deseni durdurulan 05b'den geliyor (İŞEMRİ §2). #4 değişmeden tek başına etkisi sınırlı [çıkarım]. |
| 4 | Zamanlayıcı DATA-önce çalışıyor: retx yalnız her 4. çağrıda öne geçiyor, parite en sonda, ilk jeton reddinde döngü kırılıyor. | birbirini bozan politika; spec içi çelişki | engine §13 :962 ↔ §16.1 :1064; donmuş ARCHITECTURE §4.9 :116-120 | `sender.rs:4285-4291`, `:4292`, `:4294`, `:4311`, `:1535`, `:5323-5358`; mühür yorumu `:4308-4310` | [B] Kanıt → retx p50 4,08 s / p95 20,5 s (aday 0,51 / 3,59 s). [A] 4628 paritenin 1'i, kuyruğun iki uçta da dolu olduğu 200 ms aralıklarda çıktı; bağımsızlık altında beklenen 3195. Hız sınırsız hücrede 0/2199 [ölçüldü]. Kilit üç şeyin birleşimi: DATA-önce sırası, ilk rette break ve paritenin her DATA biriminden büyük olması [kod]. | §13 sırası (retx(deadline) > parite > yeni DATA) ya da pariteye ayrı pay. Seçenek: FloorCap rezervi (`sender.rs:912-968`; `try_send_pre_parity` `:5742-5762`). Feature arkasında ve "Product/admin-activation-YOK" (`:916`). | **Evet**: DATA-önce mührü (iki pin testi, roadmap :153), ARCHITECTURE §4.9, 05b rezerv deseni, FloorCap'ın ürüne açılması |
| 5 | Oturum geneli Reorder, FEC ile kurtarılmış PTT birimini de bekletiyor. | birbirini bozan politika (AMP PTT-R0 + S1 §4 ↔ İŞEMRİ §1, INV-011) | AMP :79; S1 :151; INVARIANTS :20 | `transport.rs:361-412`, `:1077-1080`; `ptt.rs:549`, `:627`, `:656` | [A] r12k_d300_l30 dosyasız, taban: Reorder bekleyişi p50 2,30 s, p95 11,2 s. 800 ms içinde teslim %3 (Reorder olmasaydı %32 [türetilmiş]); aday %4 ↔ %22. Reorder'ın PTT gecikmesindeki payı taban %53–63, aday %34–54; aday r12k_d300'de baskın değil. "İlk engelleyici çoğunlukla PTT" yalnız aday için geçerli. [C] PTT-yalnız bench'te sıralı teslim karşı-olgusu: rs2 0,944 → 0,662, arq 0,750 → 0,032 [türetilmiş: ölçülen varışlara sıralı teslim kuralı uygulandı]. Bugün ürün yolunda PTT-yalnız oturumda parite pratikte yok; Reorder, parite açıldıktan sonraki kapı. | PTT → DeadlineBounded ve sırasız ya da son tarihli teslim. Akış başı sıra yetmiyor: bekleyişin %83–97'si kalıyor [türetilmiş]. Alıcının sınıfı bilmesi için DATA başlığında sınıf biti ya da sınıf başına unit_id uzayı gerekir [çıkarım]. | **Evet**: AMP PTT-R0 (K4G-08), S1 §4, tel (byte-layout) |
| 6 | Blok yalnız K=8 birim dolunca kapanıyor. Blok oturum genelinde unit_id/K ile, akışlar arası kuruluyor. Kısmi blok yalnız ana döngü bitince kapanıyor. | spec↔kod ayrışması (donmuş ARCHITECTURE §4.7 kodu belgeliyor) | fec-core PATCH-3 :170-172, §6 :200-203, :382, §8 :269, :379, PATCH-4 :208-210 | `runtime.rs:2498-2500`, `:2579-2635`, `:3617-3618`; `wire.rs:17` | [A] PTT bloğunun kapanma bekleyişi p50 600–800 ms, p90 1,2–3,5 s (alt sınır). Oluşma süresinin 3T+RTT'yi aştığı PTT payı d300'de 0,151–0,278, d1000'de 0,058–0,100 (üst sınır). RTT 0,6 s ve T = 200 ms'de 8 konumun 2'sinde ARQ önce geliyor [türetilmiş]. [B] r25'teki parite koşularının 123/123'ünde parite akış sonunda çıktı; alıcı hiç onarım almadı. [B,D] Uyarlamalı 205 parite koşusunun 128'inde parite yalnız kısmi bloktan. | Süre ya da son tarih tetikli kısmi kapanış. Model-A sıfır dolgu kodu var (`:2585-2631`), ama yalnız oturum sonunda çalışıyor. Canlı oturumda erken kapanışın alıcının unit_id/K eşlemesiyle nasıl uyuşacağı çözülmedi [çıkarım]. | **Evet**: donmuş ARCHITECTURE §4.7 (:76-82), fec-core PATCH-4, tel ve S1 |
| 7 | Tek oturum profili 0x05 (K8/E1024) küçük çerçevelere de uygulanıyor. | spec↔kod ayrışması | fec-core §11 :337-341, K-FEC-2 :372, K-FEC-UNIT-1 :375, K-FEC-DEADLINE-1 :379 | `runtime.rs:233-245`, `:282-285`, `:2471-2477`, `:2501`; `profile.rs:85-91` | 8 birime 2 onarım +%204 bayt ekliyor [türetilmiş]. r1k 65 B'de 8/8 hücrede r > 0 kolu yalnız-ARQ kolundan kötü [türetilmiş, model]. [C] d1000'de son tarih payı (sırasız): arq 0,750, uyar 0,724, K2/E64 0,944, K4/E64 0,922; rs2 − arq 0,194, rs4 − arq 0,172. Gerekçe yorumunun (`runtime.rs:236-239`) işlevsel bir yanı da var: E birim tavanını belirliyor (E=64 → 53 B). | PTT ayrı oturumda taşınır ve o oturumda 0x01 + `manual_r` kullanılır; tel değişmez, geçici yol. Karışık oturumda sınıf başına profil. | **Evet**: profil politikası, PTT'nin ayrı oturuma alınması, fec-core §5 "oturuma tek profil" kuralı |
| 8 | Bütçenin girdisi uygulamayla sınırlı BtlBw; "mevcut tx ihtiyacı" terimi yok; budget_ref 125 kB/s fixture'dan geliyor. Ölçülen öneri her kayıp düzeyinde 250‰; fuzzy kanalı bu bantta tanımsız. | kalibre edilmemiş + spec↔kod ayrışması | engine §8.5 :693, §8.4 :681-688; TDD §5.3 :633 | `budget.rs:24`, `:36-41`; `sender.rs:3767-3772`, `:435-447`; `runtime.rs:252`; `fusion.rs:330-347`, `:365-366` | [B] BtlBw hattın 0,2–0,5'i: r12k_d1000_p1000'de medyan l0 → l30 6530 → 2399 B/s. Kaybın BtlBw'yi düşürmesi yalnız d1000_p1000'de belirgin; d300_p1000 düz. Arbiter çıkışı sıfırdan farklıyken yalnız 250‰ (1 913 474 örnek); bu değeri yapı-gate ve bounded_v1 tavanları veriyor. Confirmed (fuzzy) kolu 500/800‰ verebilir ama hedef bantta ateşlemiyor. [B,C,D] ConfirmedFuzzy 0 (3490 koşu). Hat tam ölçülse bile cap 100‰ olur, yani r ≤ 1 [türetilmiş]. | cap, fec-core §8'in bayt cinsinden boş payına bağlanır. R kayba bağlanır (r_res < 0,05). | **Evet**: [CAL] fixture, EVOLUTION §9.2 yasağı, profil kataloğu (R = 6 > R_max = 4) |
| 9 | DeadlineBounded uçtan uca kablosuz. Süresi dolan birim düşmüyor; deadline oturum boyunca en sıkı değere yapışıyor; FloorCap derleme dışında. | eksik entegrasyon + varsayılan kapalı | S1 :141; transport-tdd :48-49; fec-core :379 | `transport.rs:1272-1281`; `runtime.rs:286`, `:2430-2436`; `sender.rs:4354-4359`, `:912-968` | [B,C,D] 3450 uyarlamalı koşuda deadline_ns=None. [C] Deadline bildiren pol_taban kolunda 20/20 koşu clamp > 0, onarım 0/20. | Süresi dolan birim gönderende düşürülür. Deadline blok kapanışına ve parite önceliğine bağlanır. | Düşürme için hayır (S1 §4 zaten istiyor). PTT'nin DeadlineBounded olması için evet (#5). FloorCap'ın açılması için evet (#4). |
| 10 | Zaman eşiği tabanı her kanalda ≥ 3,145 s (31 B/s önseli). Ölçülen return_bw hiçbir karara bağlı değil. PTO son ACK ilerlemesine çapalı ve tek prob atıyor. Gap görülünce anında ACK yok. | kalibre edilmemiş + eksik entegrasyon + spec boşluğu (§12'de max_ack_delay ↔ cadence spec içi gerilimi) | engine §5(b) :449-456, :1051; §12 :922-936 | `loss.rs:682`; `sender.rs:3396-3412`, `:1494-1501`; `return_path.rs:8-11`; `tail_recovery.rs:72-82`, `:176-180`; `receiver.rs:247-256` | [B,C] Suspected → Confirmed p50 3,31 s (d300; 4,37 srtt), 3,39 s (d1000). [B] r1k_d300'de yeniden gönderimlerin %40 / %61'i sonraki ACK'ten önce (PTO ya da kopya); gönderim → yeniden gönderim p50 4,27 / 3,66 s. Seyrek akışta ACK tutması p95 3,69–4,95 s (üst sınır 8,39 s). | Taban ölçülen return_bw'den türetilir. PTO çapası son ack-eliciting gönderime alınır, 2 prob atılır. Gap'te anında ACK. | Taban için hayır (engine :1051 bunu öngörüyor). PTO ve ACK için spec eki: **evet** |
| 11 | Kuruluş: ilk DATA kaybolur ve ilk pencere 1200 B'a kırpılmışsa (jeton 174 B) hız 31 B/s'de kalıyor. 2400 B'lık ilk pencere bir yarışla 1200 B'a kırpılıyor. PreConverge'de PTO, spec'in aksine 1 s ile kuruluyor. İlk blok paritesiz. | birbirini bozan politika + kalibre edilmemiş + spec↔kod ayrışması (PTO) | engine §3.3, :248, :660; §12 :935-936 ("PTO ARM DONDURULUR") | `sender.rs:3090`, `:3982-3990`, `:1485`, `:2366-2381`, `:3278-3296`, `:3798-3804`, `:6026-6028`; `tail_recovery.rs:77`, `:157`; `runtime.rs:2408`, `:271-272`; `rate.rs:93`, `:100-101` | [B] Taban 3/30 koşuda 29,6–64,8 s'lik PreConverge tuzağı; jetonu 1374 olan rep5/rep10'da kilit oluşmadı. İlk pencere kırpması 20/30 ve 21/30 koşuda. PreConverge çıkışı p50 (en çok), taban / aday: r1k_d300 8,13 (124,2) / 1,82 (6,03) s; r1k_d1000 24,94 (125,35) / 3,70 (9,88) s; r12k_d1000 2,41 (64,80) / 2,34 (9,25) s. r1k_d1000 taban rep20 PreConverge'den hiç çıkmadı (4/20 teslim). 1 s'lik PTO tek ateşlemeyle ilk birimin RTT fırsatını Karn kol-2'ye düşürüyor; fazı uzatan 31 B/s pacing, Karn dışı kopyalar ve orijinal kayıplar. r12k'daki 64,8 s bu mekanizmaya atfedilmez. Kayıpsız d1000'de p95 5,80 / 6,04 ↔ 2,60 s [ölçüldü fark; kuruluş geçişine atfı çıkarım, cwnd/pacing serisi kaydedilmedi]. İlk blok anında öneri 0: manşette 60/60, r12k_d300_l30_p1000'de 56/60. preconv_parity_blocked > 0 olan koşu 390/3470. | Idle'da kova ilk pencerede tutulur. PreConverge'de PTO arm'ı spec'e uygun dondurulur. Kanıt gerektirmeyen bir ilk uçuş tabanı eklenir (kodlanmış ilk uçuş; gereksinim boşluğu). MIN_CWND ≥ 10×MTU geçişi kısaltır, kaldırmaz. | **Evet**: 05b'nin kalan adımları durduruldu (İŞEMRİ §2; aday PreConverge'i büyük ölçüde gideriyor); cwnd_gain spec :660; MIN_CWND [CAL] |
| 12 | Alıcı taşmasında ACK'lenmiş birim kayboluyor. Reorder'da boşluk atlama ve geç kopya sessiz. | spec↔kod ayrışması (modül belgesindeki INV; beyanlı sınır) | `transport.rs:69-73` | `runtime.rs:3010-3032`, `:3048-3065`, `:3602-3605`; `transport.rs:394-402` | [A] 470 koşunun 15'inde, yalnız taşma hücrelerinde, en çok 1630 birim. Birincil ve dur hücrelerinde 0. | ACK gönderilmeden önce teslim kanalında yer ayrılır. | Sözleşme değişmiyor. Ancak sınır G-IMPL-S3 ve INT-02 insan kapılarında beyanlı istisna olarak kabul edildi (#114/#116). Kaldırılması o kaydı günceller: **sahip onayı** |
| 13 | Dosya tek FIFO'ya kabul edildikçe dökülüyor. S1'de öncelik ya da sınıf alanı yok. | spec boşluğu | AMP :94; `s1.rs:62-68`; transport-tdd :65-66 (öncelik ipucuna izin var) | `file_transfer.rs:866-909`; `sender.rs:1595`, `:2309`, `:6109-6129`; `runtime.rs:259-260` | [A] Dosyalı TEXT p95'inin %85–95'i gönderen FIFO'su. TEXT p95 dosyalı 110,5 / 73,5 s ↔ dosyasız 14,0 / 11,0 s (r12k). Saf FIFO ilk geçiş sınırı r12k 46 s, r1k 71,8 s [türetilmiş]. /(1−p) terimli sınıra göre ölçüm/sınır oranı 0,95–1,72. Hız düzeltmesi K-1'i tek başına çözmez. | client-core'da kabul şeridi; S1'e öncelik ipucu. | **Evet**: S1 imzası |
| 14 | Parite kuyruğu tek FIFO. Kaynakları ACK ile tamamlanan bloğun paritesi iptal edilmiyor. RECOVERED yalnız FEC çözümünde gidiyor. | spec boşluğu | fec-core §9 :300-302, K-FEC-PACER-2 :380, §7 :262-264 | `sender.rs:1670`, `:4908`, `:5243`, `:5370`; `runtime.rs:3647-3650` | [C] rs2 kolunun dört d300 hücresinde akış sonunda bekleyen parite medyanı 24,5–157. Medyanı 0 olan d300 hücreleri de var; d1000'de 0. | Blok tamamlanınca parite iptal edilir; kuyruk son tarih sırasına alınır. | **Evet**: spec eki (fec-core §9) |
| 15 | Ürün yolunda pacer PTT temposunda: Startup platosu uygulamayla sınırlı turları da sayıyor; Startup sonrası kazanç 1; DrainRtprop jitter'da sık giriliyor (0,75 kazanç). | spec boşluğu (engine :248 bu konuda sessiz; [STD:BBR] türevi) | engine :248 | `sender.rs:4208-4235`, `:3072-3073`, `:3089`, `:3111`, `:3234-3247`, `:4039-4057`, `:4249`; `rate.rs:39`; `bw.rs:233` | [A] Pacer p50 d300 293 / 363 B/s, d1000 419 / 359 B/s. PTT penceresinde 365 / 305 / 735 / 421 B/s; PTT talebi ≈ 640 B/s, d1000 tabanda pacer talebin üstünde. Faz payları: DrainRtprop 0,26–0,34, Startup 0,29–0,38, ProbeUp 0,08–0,11. Senaryodaki 30 s'lik duraklama ayrıştırılmadı. "Parite için jeton yok" bir [çıkarım]. | Plato sayımından uygulamayla sınırlı turlar çıkarılır (quinn bbr/mod.rs:372-375 deseni). Uygulamayla sınırlı trafikte BtlBw ≈ uygulama hızı her delivered-rate tahmincisinde beklenir; asıl fark Startup sonrası kazanç 1. Etkisi tek başına belirsiz. | **Evet**: spec eki (engine :248); #1 ile aynı INV-RATE-1 / fec-core §0.8 kapısı |
| 16 | r = 0'ın nedenini gösteren sayaç dışa taşınmıyor: `latch_rounded_to_zero` ClientStats'ta yok. clamp ve would_zero yalnız bench stderr'inde basılıyor. | eksik entegrasyon (gözlem) | fec-core §8 :288 ("telemetri-görünür") | `sender.rs:5086-5088`; `runtime.rs:2715-2729` | "Önerildi ama yuvarlandı" durumu koşu başına doğrudan sayılamıyor. F0'daki sıfır parite hükmü ts.csv final_ratio'dan ve latch tuple'ından dolaylı kuruldu [kod]. | Sayaç ClientStats'a ve bench manifestine taşınır; salt gözlem. | Hayır |

**Sahibin üç kusur sınıfına eşleme**

| sahibin sınıfı | tablodaki satırlar |
|---|---|
| eksik entegrasyon | #9; #10 (return_bw); #16 (sayaç ihracı); FloorCap (feature arkasında; #4, #9) |
| bağlı ama etkisiz (kablolu, bileşimde etkisiz) | #2, #3, #8 |
| birbirini bozan politika | #1, #4, #5, #11 |
| sahibin üçlüsünün dışında (spec↔kod ayrışması, spec boşluğu) | #6, #7, #12, #13, #14, #15 |

**Gereksinim eşlemesi (İŞEMRİ §1)**
- **PTT'de FEC ürün düzeyinde kullanılır, RTT beklemeye mahkûm edilmez.** → #2–#9, #11, #15.
- **Akışlar birbirini bloke etmez.** → #5, #12, #13.
- **Düşük bantta protokol yükü faydalı veriyi boğmaz.** → #7, #8.
- **Yüksek RTT altında iletişim sürer.** → #1, #10, #11, #15.
- **QUIC'e karşı ölçülebilir üstünlük.**
  - Bugün bench'teki nokta karşılaştırmasında quinn 32 hücrenin 28'inde önde [ölçüldü].
  - Ölçülen farkın bileşenleri #1 (gönderen bekleyişi) ve #4 (kurtarma bekleyişi) ile ilişkili.
  - Düzeltme sonrası durum, kanıt zincirinin 6. adımında aynı teslim noktasında ölçülecek. Beklenti kaydı koşudan önce yazılır; yön önceden iddia edilmez.

**Doğru bağlanmış olanlar**
- **FEC zinciri telde çalışıyor.** Kodlama → mühür → çözme → RECOVERED. Kanıtı C statik kolları [ölçüldü].
- **PARAM_SET B2 bağı.** FEC açık bütün bench koşularında kuruldu: B 2560, C 2520, D 170 [ölçüldü].
  - Küçük sapma: ön-PARAM taşmasında yeni shard düşüyor (`runtime.rs:3664-3668`). Spec en eskisinin düşmesini istiyor (noise-wire :650).
- **Ortak teslim kapısı.** DATA ve FEC kurtarımı aynı kapıdan ve aynı tekilleştirmeden geçiyor (`runtime.rs:3042-3070`). Bu yalnız taşmasız durumda kusursuz; taşmada #12 devreye giriyor.
- **RECOVERED.**
  - Birimi depodan silip ARQ'yu iptal ediyor (`runtime.rs:2206-2232`).
  - Ama tek atımlık (`runtime.rs:3648-3651`, `:3733-3736`, `:3798-3801`). Kaybolan bir RECOVERED'ın etkisi ölçülmedi [kod].
- **Aynı-olay latch koruması.** 3490 koşuda uyuşmazlık 0 [ölçüldü].
- **Kayıp tespiti.**
  - Paket eşiği K = 3, RFC 9002 ile aynı (`loss.rs:14`) [kod]. Manşette spurious 0 (60/60) [ölçüldü].
  - Zaman eşiği tabanı ve PTO RFC 9002'den sapıyor (#10).
  - Çekirdeğin geri kalanı (un-loss, Karn, ack_delay) [doğrulanmadı].
- **INV-RATE-1'in harfi sağlanıyor.** Hız hesabında kayıp girdisi yok; fuzzy kazancı yalnız bant ve kuyruk gecikmesine bağlı (`bw.rs:98-116`, `rate.rs:164-177`, `fusion.rs:536-540`).
- **Pacer bypass yok.** Parite aynı pacer'dan geçiyor (`sender.rs:5329`) [kod].
- **Uygulama ve düğüm katmanı.** PTT'ye eklediği gecikme p95 < 0,15 ms [ölçüldü, A].
- **Kısmi blok.** Model-A sıfır dolgu kodu var (`runtime.rs:2585-2631`); eksik olan zaman ya da son tarih tetiği. Canlı oturumda bu tetiğin alıcının unit_id/K eşlemesiyle (`runtime.rs:3617-3618`) nasıl uyuşacağı çözülmedi [çıkarım].

## Sahibin bileşim hatası hipotezi

**Hüküm: doğrulandı.** Halkalar kodda izlendi [kod], aritmetik yeniden hesaplandı [türetilmiş], davranış ham veride görüldü [ölçüldü].

**Zincir, halka halka:**
1. Ürün profili `fec: Some(0x05)`, `manual_r: None`; bu Controller modunu açıyor (`runtime.rs:282-285`).
2. Arbiter max() alıyor (`sender.rs:3744`). Spekülatif kolun 63‰ tabanı bu noktada, kırpmadan önce uygulanıyor (`:412`, `:4595`).
3. cap = min(⌊BtlBw·1000/125 000⌋, 900); BtlBw = 0 iken cap 900 (`budget.rs:26-31`, `:38-41`; `sender.rs:3769`).
4. Sıfırlama reddinin koşulu yalnız `clamped == 0 && final_ratio > 0` (`sender.rs:3786`).
5. Latch kırpılmış oranı okuyor (`:3835`, `:4966`) ve r = (‰·k + 500)/1000 hesaplıyor (`:1026-1031`).
6. Sıfıra yuvarlanan latch yalnız sayılıyor (`:5086-5088`); bu sayaç ClientStats'a taşınmıyor (`runtime.rs:2715-2729`; #16).

**Kesin aritmetik (öneri 250‰, K=8):**

| BtlBw (B/s) | cap (‰) | yol | r |
|---|---|---|---|
| 0 (örnek yok) | 900 | kırpma yok | 2 |
| 1–124 (ör. 31, 100) | 0 | ret kırpmayı atlıyor, 250 aynen geçiyor | 2 |
| 500 | 4 | kırpılıyor; (4·8 + 500)/1000 = 0 | 0 |
| 1000 / 3000 | 8 / 24 | kırpılıyor | 0 |
| 7875 | 63 | kırpılıyor | 1 |
| 12 500 (hattın tamamı) | 100 | kırpılıyor | 1 |
| ≥ 23 500 | ≥ 188 | kırpılıyor | 2 |

r ≥ 1 için gereken BtlBw:
- K = 4'te ≥ 15 625 B/s
- K = 16'da ≥ 4000 B/s
- kısmi blokta k = 3 için ≥ 20 875 B/s
- k = 1'de 250‰ bile r = 0 veriyor.

**Davranış tekdüze değil.** En dar bantta (25 B/s; onarım başına ≈ 42 s hat süresi) oran kırpılmadan geçiyor. 125–7874 B/s aralığında ise sıfıra iniyor.

**Ölçüm:**
- Uyarlamalı 3470 koşunun 205'inde parite var (416 onarım).
- Bunların 192'sinde would_zero > 0, yani ret-atlama yolu ateşlemiş. Aynı koşuda BtlBw = 0 yolu da parite üretmiş olabilir; ikisi koşu düzeyinde ayrılmadı. 13 koşuda parite BtlBw ≥ 7875 yolundan geliyor [ölçüldü].
- 128 koşuda parite yalnız akış sonundaki kısmi bloktan çıkıyor.
- uyar_taban'ın 276 332 örneğinde:
  - kırpma öncesi ≥ 63‰ olan örnek %91,9
  - kırpma sonrası 1–62‰ olan %91,4
  - kırpma sonrası ≥ 63‰ olan yalnız %0,5

**Bileşim regresyonu [kod].**
- 63‰ tabanı 2026-09-08'de (`75d8990a`) "K=8'de ≥ 1 onarım" garantisi için kondu.
- §8.5 kırpma çağrısı 2026-09-19'da (`53e0c5c6`) geldi ve o tabanı geri alıyor.

**Spec durumu.**
- Kod engine §8.5 :694'ü, AMENDMENT-3 :696'yı ve §10.2 :790'ı harfiyen uyguluyor.
- fec-core §8 ise başka şey istiyor: :279 bayt cinsinden bütçe; :288 "CAP-BOZUYORSA-SESSİZ-DEVAM-YOK → profile_target_unmet".
- Yani durum "spec doğru, kod yanlış" değil: iki spec çelişiyor ve kod birini izliyor.
- CLAUDE.md'deki "çelişkide spec kazanır" kuralı iki spec arasında karar vermiyor. Çelişkinin hangi yönde çözüleceği sahip kararı.
- Ölü bölge ADR-fec-v1-role'de ve EVOLUTION §9.2'de bilerek donduruldu. ADR-minimal-parity-exception ret-atlama yolunu normatif olarak donduruyor ama 1–62‰ bandını ele almıyor. F0, bu ölü bölgenin baskın rejim olduğunu gösterdi.

**İstisna yol.**
- §11 MinimalParity (100/200‰, `sender.rs:5033-5068`) §8.5'i atlıyor; yol yapısal olarak kapalı değil.
- Ön koşullarından biri (defter tepesi > 550k ppm), l10–l30 hücrelerindeki 2660 koşunun 169'unda sağlandı [ölçüldü].
- Diğer ön koşullar (ack_conf < 400, merdiven ≥ Dup2) ve dalın ateşleyip ateşlemediği F0 telemetrisinden belirlenemiyor. 205 parite koşusunda bu dalın parite ürettiğine dair iz bulunmadı.
- ADR-minimal-parity-exception :54'e göre bu yol sayaçları artırmıyor. Parite katkısı ölçülmedi.

**Sahibin diğer önermeleri:**
- **"Blok yalnız K dolunca kapanır."** Doğru (#6).
- **"Parite DATA, ARQ ve problardan sonra gelir."** Doğru; bu bilinçli bir mühür (#4).
- **"Oturum geneli sıra."** Doğru, ama yalnız ürün yolunda var; bench B/D gecikmesini açıklamaz (#5).
- **"Bütçe formülü spec tabanlı."** Kısmen doğru.
  - engine §8.5 :694 formülüne dayanıyor.
  - Ama spec'teki "mevcut tx ihtiyacı" terimi kodda yok (`budget.rs:36-41`).
  - Ayrıca fec-core §8'in bayt cinsinden bütçesinden sapıyor.
- **"budget.rs fixture kalibre değil."** Doğru (`budget.rs:6-9`, `:24`).
- **Hipotezde olmayan ikinci kilit: kova tavanı (#3).**
  - Kuyrukta DATA varken ve hız < 26,45 kB/s iken 1058 B'lık parite kovaya sığmıyor [kod+türetilmiş].
  - Bugünkü DATA-önce sırasında bu kilit #4'ün içinde kalıyor; parite öne alınınca bağlayıcı olur.
  - Kilit hıza bağlı: dosyalı hücrelerde aday kolu konuşma sırasında 146 / 42 / 218 parite gönderdi.

## Kanıt zinciri planı (6 adım)

**Genel kurallar.**
- **Ölçüm kancaları.**
  - Kancalar test-util ya da bayraklı, sınırlı ve davranış-nötr olacak.
  - Her kanca için ayrı bir PR'da iki eşitlik gösterilecek: bayrak açık/kapalı TransmitDecision dizisi ve tel baytı (desen `runtime.rs:4246`).
  - Kanca tasarımlarının kod atıfları M6-1, M6-2 ve M6-7'den geliyor [doğrulanmadı]. İstisna: `kama-node lib.rs:252` her zaman ürün yapılandırmasını kuruyor [kod].
- **N kuralları.**
  - Mekanizma testi: 20/20 (tek yönlü %95 alt sınır 0,861).
  - Stokastik test: koşu birimli N = 30, ≥ 28/30 (alt sınır 0,805).
  - ACK'lenmiş ama teslim edilmemiş birim: 0 tolerans.
- **Eksen.** ARQ her kolda açık; eksen marjinal artıklık.
- **Ön-kayıt ve yetki.**
  - Her adım şu sırayla koşulur: ön-kayıtlı beklenti (ölçü, yön, eşik) → dry-run → freeze → tek koşu → donmuş analiz.
  - Netem koşularını sahip başlatır. Freeze kilidi başlatma yetkisi değildir **[SAHİP]**.
- **Kapsam dışı.** A2 (ReceiverHost, admit_and_decode kablolaması) ayrı sürüyor ve bu planın dışında.

**1. Tek kontrollü kayıp, ürün yolunda**
- **Yöntem.**
  - Mevcut mühür kancası kullanılır: `EndpointSeam::seal` + `drop_next_units_for_test` (`transport.rs:265-297`, `:1107-1111`). Bugün yalnız `kama-integration/tests/common/seam/real.rs:346` çağırıyor [doğrulanmadı].
  - kama-node'a, kurma ile gönderimi tek komutta yapan atomik bir `ArmDrop` eklenir.
  - f0_flows'a `--drop sp=…,i=…,mode=first|perm` bayrağı eklenir.
  - Kancanın gerçekten ateşlediği, düşürme sayısının alıcıdaki open_failures artışına eşit olmasıyla denetlenir.
- **Sadakat sınırı.** Beyan edilir: bozulan datagram dar boğaz kapasitesini tüketiyor (PTT için ≈ 86 B), netem kaybı tüketmiyor [çıkarım; doğrulanmadı].
- **1a, mekanizma kolu.**
  - `manual_r ∈ {1, 2}`, profil 0x05, netem kaybı 0, sabit gecikme.
  - Düşürme kipi her koşuda kaydedilir.
- **1b, ürün uyarlamalı kolu.**
  - Arka planda IID %10–20 kayıp, ≥ 32 birimlik ısınma (`sender.rs:1456`), ardından hedefli `first` düşürme.
  - Spekülatif kol tek bir şüpheli kayba tepki verecek biçimde tasarlanmış (`sender.rs:407`, `:412`). Kayıp blok kapanmadan Suspected olursa r = 0'ı kanıt yokluğu değil §8.5 kırpması üretir [çıkarım: tespit anı ölçülmedi]. İki durum `latch_rounded_to_zero` sayacıyla ayrılır; bu sayacın adım 0'da dışa taşınması gerekiyor (#16).
  - Kurtarılabilir blok tanımı: alıcıda kaynak + onarım ≥ K.
- **Kabul (düşürülen birim u için).**
  - E1: alıcıda from_fec = true.
  - E2: t_app − t_offer ≤ D.
  - E3: t_srv(u) < t_emit(u) + RTT_min + OWD_min, yani ARQ'nun en erken varışından önce.
- **N.**
  - 1a için 20/20. Negatif kontrol: r = 0 kolunda 20/20 via=ARQ.
  - 1b için N = 30; kurtarılabilir bloklarda ≥ 28/30.
- **Düzeltme öncesi.**
  - 1a koşulabilir. `perm` kipinde beklenti via=FEC (tanım gereği tek yol); gecikme = blok dolumu + jeton beklemesi.
  - `first` kipinde sonuç jeton beklemesine bağlı (1058/send_rate ARQ turunu aşarsa via=ARQ). Beklenti kaydı koşudan önce yazılır.
  - Parite, blok kapanış olayıyla aynı olayda gidemiyor; en erken bir sonraki olayda gidiyor.
  - 1b'nin ön koşusunda beklenti via=ARQ ve rounded_to_zero > 0.
- **Düzeltme sonrası.**
  - 1b'nin kabulü #2, #3 ve #4 düzeltmelerini ister; blok başı konumlar için #6'yı da.
  - #1 (kurtarma payı) olmadan paritenin yalnız kaynağın yerini alıp almadığı ölçülecek açık bir soru [çıkarım]. Bu yüzden 1b, #1'den önce ve sonra ayrı ayrı koşulur.

**2. Aşama damgaları**
- **Yöntem.** `fec_trace` adlı bir çalışma-zamanı bayrağı eklenir (ürün varsayılanı false). Sınırlı bir halka ve düşen kayıt sayacı kullanılır.
- **Gönderen kanca noktaları:**
  - BlockOpen ve BlockClose{k_actual, FecLatchResult, cap, btlbw, kırpma öncesi/sonrası oran}: `runtime.rs:2489-2541`, `:2585-2632`; `sender.rs:3763-3796`.
  - ParityEncoded: `runtime.rs:2524-2537`.
  - TokenDenied{tokens, burst_cap, front_size, rate}: `sender.rs:5323-5358`.
  - ParityEmitted: `runtime.rs:2000-2042`.
  - DataEmitted{Data|Retx|Dup}: `:2083-2094`.
  - AckedVia{WireAck|Recovered}: `:2206-2220`.
- **Alıcı kanca noktaları:**
  - DataRx (`:3578`), RepairRx, BlockDecoded (`:3288-3322`).
  - Delivered{from_fec} ya da DeliveryDropped (`:3010-3032`).
  - client-core'da Reorder'ın Drained/Released izleri ve uygulama rx.
- **from_fec iz kanalı.** from_fec, Delivery yapısında yok: `{peer, unit_id, payload, ctx}` (`runtime.rs:1190-1197`) [kod]. Yapının API'de pinli olduğu (`api_surface_v0.rs:246-257`) [doğrulanmadı]. Bu yüzden from_fec ayrı bir iz kanalından taşınır.
- **Sayaç.** rounded_to_zero ClientStats'a taşınır (#16).
- **Nötrlük.**
  - Deterministik loopback testinde açık/kapalı eşitliği.
  - Netem'de açık/kapalı N = 10 betimsel tx_bytes kıyası [doğrulanmadı].
- **Kabul.**
  - ≥ 1 MB'lık iz koşusunda düşen kayıt 0.
  - Her düşürülen birim için zincir ya FEC yolunda (BlockClose → … → Delivered → Released → rx) ya da ARQ yolunda (Retx → DataRx) eksiksiz olmalı.
  - Kabul canlı `par` sayacına dayanmamalı: repair_store'da bulunamayan parite başlık-yalnız gönderilip sayılıyor (`runtime.rs:2008-2040`) [doğrulanmadı].
  - Uygulama damgası, düğüm turu yüzünden 20 ms'ye kadar nicemlenir.
- Salt gözlem olduğu için düzeltmeden önce koşulur.

**3. Seyrek ve kısa konuşma**
- **Yöntem.**
  - Dosyasız senaryo: `--warmup-texts 3 --texts 0 --sigs 0 --ptt-at-ms 15000,35000 --ptt-bundles 2`.
  - Karşı-örnek yalnız dosya senaryosunda tetikleniyor, bu yüzden dosyasız senaryo kullanılır. İz, ilk birimin 0 olduğunu göstermeli (early_unit yok).
  - Blok 0'da 7 birim var; blok, 35 s'deki ikinci basışın ilk birimi gelene kadar kapanmaz.
  - P1 (birim 5) `perm` kipinde düşürülür.
- **Düzeltme öncesi beklenti.**
  - Statik r kolunda bile teslim ≈ 35 s + jeton beklemesi + OWD. `first` kipinde via=ARQ.
  - Mevcut izden ölçülen alt sınır: PTT bloğunun kapanma bekleyişi p50 600–800 ms, p90 1,2–3,5 s. > 800 ms payı dosyasızda %48–49, dosyalıda %37–42 [ölçüldü, A].
- **Konum taraması.**
  - 3 kol (r0/r1/r2) × 8 konum × 10 tekrar ≈ 240 koşu.
  - Öngörü: t_FEC ≈ (7 − esi)·T + jeton + OWD.
  - Blok dolumunun etkisi ile çizelgelemenin etkisini ayırır.
- **Kabul (düzeltme sonrası).**
  - Blok, kanaldan ve son tarihten türetilen bir τ ile süreli kapanır.
  - `perm` kipinde via=FEC ve t_app − t_offer ≤ τ + jeton + OWD + 20 ms ≤ D, 20/20 koşu.

**4. Karışık trafik ve akışlar arası bloklama**
- **Ölçü.** {u : gönderende AckedVia(WireAck) var ∧ uygulamada rx yok, süre payından sonra}.
- **Kancalar.**
  - `push_delivery`'nin başarısız dalına DeliveryDropped{unit, ack_sent}.
  - Reorder'a skipped_units ve late_dropped sayaçları (`transport.rs:394-402`).
- **Senaryolar.** dosyali_dur, tasma_dosya, tasma_ptt; ayrıca dosya sırasında hedefli bir PTT düşürmesi.
- **Düzeltme öncesi.** Kusur mevcut veride zaten görünüyor: 15/470 koşu, en çok 1630 birim.
- **Kabul.**
  - ACK'li ∧ teslimsiz, skipped_units ve late_dropped: her taşma hücresinde N = 10 × 2 kol ile üç sayı da 0.
  - Dosya sırasındaki hedefli PTT düşürmesi: N = 30 koşu birimli; düzeltme sonrası E1∧E2∧E3 ≥ 28/30.
  - TEXT/PTT p95 için dosyalı ↔ dosyasız fark: koşudan önce yazılan bir eşik.
  - Bugünkü TEXT p95 dosyalı 110,5 / 73,5 ↔ dosyasız 14,0 / 11,0 s [ölçüldü, A]. FIFO ilk geçiş sınırı r12k'da ≥ 46 s olduğundan bu ölçüt #13 düzeltilmeden geçemez [türetilmiş].

**5. Aynı Kama tabanında mekanizma ayrımı**
- **Kollar [tasarım; doğrulanmadı].**
  - A0: `manual_r = Some(0)`; PARAM/FEC çerçeveleri bakımından bayt bayt aynı, paritesiz.
  - A1: `manual_r` 1 ya da 2.
  - A2: ürün uyarlamalı.
  - A3: taşıma düzeyinde zorunlu kopya; aynı unit_id, `dup_queue` ve `drain_due_duplicates` üzerinden, pacer'a bağlı.
- **Gereken kanca.** kama-node'a bir `runtime_override` eklenmeli; bugün `kama-node lib.rs:252` her zaman ürün yapılandırmasını kuruyor [kod].
- **Eşit yük.**
  - Bench'teki `--dup-copies` eşit yük değil: kopyalar ayrı unit_id alıyor ve ARQ'lanıyor [doğrulanmadı].
  - Eşitlik, gerçekleşen mühürlü tel baytıyla tanımlanır: |tx_dup − tx_fec| / tx_fec ≤ %10. Dışında kalan koşu BUDGET_MISMATCH olarak işaretlenir.
- **Profil kısıtı.**
  - Fragmenter kapalıyken E = 64 profilleri birimi 53 B ile sınırlıyor (`runtime.rs:236-245`) [kod]. TEXT'in 75 B ile sığmadığı [doğrulanmadı].
  - Karışık oturumda yalnız 0x05/0x06 kullanılabilir; K2/E64 yalnız PTT-yalnız oturumda.
- **Hücreler ve N.**
  - r12k × {d300, d1000} × {tek düşürme, %10, %20}.
  - Ayrıca r1k × 65 B × {d300, d1000} × %20: E = 1024'ün zararlı olduğu bant burası (8/8 hücre).
  - Koşu birimli N = 30.
- **Kabul.** Ön-kayıtlı A1 − A0 ve A1 − A3 farkları (E1∧E2∧E3 oranı), Bonferroni düzeltmesiyle.
- **Rol.** Düzeltmeden önce koşulabilir. Düzeltmeden sonra aynı kollarla yeniden koşulur ve kazancın kodlamadan mı, erken fazlalıktan mı geldiği ayrılır.

**6. QUIC kıyası, aynı teslim noktasında**
- **İş yükü.** Tek bir JSON plan hem f0_flows'a hem bench `mixed.rs`'e verilir.
- **Teslim noktaları.**
  - Kama: Reorder öncesi, Reorder sonrası ve uygulama rx.
  - quinn: öğe başına stream (sırasız) ve akış başına stream (akış içi sıralı).
- **Hat ve maliyet.**
  - Aynı netns/netem hücresi.
  - Toplam tel maliyeti `mid_qdisc` realized_post − realized_pre farkından, iki yönde okunur (`scripts/netem/kapi3_harness.sh:170`).
- **Kurulum.**
  - f0_flows'ta Kama'nın el sıkışma zarfları netem dışından geçiyor (`f0_flows.rs:9-12`). Ya veri yoluna alınır ya da analitik olarak eklenir.
  - Bench `kama-product` kolunda el sıkışma yok. quinn'in el sıkışması netem içinde ama pencere dışında; kurulum maliyeti ayrı raporlanır.
- **Rakip tanımı.**
  - quinn-BBR iki kolla koşulur: 240 kB ilk pencere ve RFC ilk penceresi.
  - RFC ilk pencereli kol, F0'da beyan edilmiş rakip tanımını (arms_rakip.sh) değiştiriyor **[SAHİP]**.
  - `mixed.rs`'de quinn'e verilen akış önceliği (PTT > TEXT > dosya) ya beyan edilir ya eşitlenir [doğrulanmadı].
- **Hücre seti.**
  - F0 B ızgarasının 32 birincil hücresi (r1k/r12k × d300/d1000 × l0–l30 × 65/1000 B).
  - Ayrıca r25 ve C'nin PTT hücreleri.
  - Set koşudan önce dondurulur.
- **İstatistik.** N = 30, birim koşu. Paket dizileri kollar arasında farklı olduğu için eşleştirme yok.
- **Ölçüler.**
  - Sınıf başına p50/p95/p99.
  - Kuyruğu gören teslim ölçüleri: teslim@D < 1 olan koşu sayısı, havuzlanmış teslim, 2/5/10 s son tarihlerde teslim.
  - PTT son tarih payı (400 ve 800 ms).
  - Teslim edilen uygulama baytı başına tel baytı.
  - Kama için: ilk emisyonu kaybolup t_emit + RTT_min + OWD_min'den önce teslim edilen birimlerin payı.
  - Hücreler arası Bonferroni.
- **Sıra.**
  - Düzeltmeden önce yalnız "düzeltme öncesi taban" etiketiyle koşulur.
  - Nihai kıyas sonucu, yönü ne olursa olsun, 1b, 3 ve 4'ün kabulünden sonra raporlanır.

**Karar kuralı: başarısız sonuç hangi varsayımı değiştirir**

| adım | başarısız sonuç | yeniden açılacak varsayım |
|---|---|---|
| 1a | via=FEC değil ya da gecikme beklenti kaydını aşıyor | blok dolumu (K), parite zamanlaması (kova, sıra) |
| 1b | düzeltme sonrası < 28/30 | bütçe formülü ve R–kayıp bağı (#8), hız modeli (#1) |
| 3 | τ'li kapanışta via=FEC ya da ≤ D sağlanmıyor | K, E, blok kapanış modeli (sınıf başına blok) |
| 4 | ACK'li ∧ teslimsiz > 0 ya da dosyalı/dosyasız eşiği aşılıyor | ACK–teslim bağı (#12), Reorder politikası (#5), kabul şeridi (#13) |
| 5 | eşit yükte A1 − A3 ≤ 0 | küçük birimde E/profil seçimi (kodlama yerine kopya), K-FEC-2 profil seçici |
| 6 | Kama aleyhine fark sürüyor | hız modeli ve öncelik varsayımları (#1, #4, #15) |

## Düzeltme sırası ve sahip kararları

**Genel çerçeve.**
- F1'e geçiş zaten ayrı bir sahip onayına bağlı (İŞEMRİ, 2026-10-29).
- **[SAHİP]** işareti şunlara dokunan kalemler için kullanılıyor: mühürlü bir politika, spec metni, ADR, önceki bir insan kapısı kararı ya da S1/AMP sözleşmesi.
- Her adımdan önce beklenti kaydı yazılır: ölçü, yön, eşik. Adımdan sonra kanıt zincirinin ilgili adımı aynı hücrelerde yeniden koşulur. quinn'e karşı sonuç önceden iddia edilmez.
- 3. ve 4. maddeler, durdurulmuş 05b adayının mekanizmalarıyla örtüşüyor: burst_floor ve rezerv (`sender.rs@bf18dc35:4748-4784`) ile startup-floor'un süreğen hâli. 05b parçalarının yeniden kullanımı (#140 DUR) ayrı bir **[SAHİP]** kararı.

0. **Ölçüm kancaları ve sayaçlar.**
   - Kapsam: adım 1, 2 ve 4'ün kancaları ile #16.
   - Salt gözlem; davranış-nötrlük kanıtıyla birlikte gelir.
   - Ardından düzeltme öncesi koşular: 1a, 1b'nin ön koşusu, 2, 3, 4, 5. 6 yalnız "düzeltme öncesi taban" etiketiyle.
   - **[SAHİP]** netem koşularının başlatılması.
1. **Sembol düzeyinde sıfırlama reddi + `profile_target_unmet`** (`sender.rs:3786`; `runtime.rs:2715-2729`). **[SAHİP]**
   - ADR-fec-v1-role Decision §3 ve EVOLUTION §9.2 donmasının açılması.
   - ADR-minimal-parity-exception. Ret-atlama yolunu normatif olarak donduruyor; "uygulama ne yapıyorsa spec ONU söyler" ilkesiyle yazılmış. Bu madde değişirse ADR de değişmeli.
   - AMENDMENT-3 (engine :696-697) ↔ fec-core §8 (:279, :288) çelişkisinin hangi yönde çözüleceği.
2. **Bütçe birimi ve formülü (#8).** **[SAHİP]** Karar verilecekler:
   - Hangi spec geçerli: engine §8.5 :693 (f(BtlBw, mevcut tx ihtiyacı) [CAL]) mı, fec-core §8 :279/:288 (bayt cinsinden R_wire_max, profile_target_unmet) mı.
   - `budget.rs:24-31`'deki [CAL] fixture (budget_ref 125 000 B/s).
   - Kayıptan bağımsız 250‰ öneri düzeyi (`runtime.rs:252`, `sender.rs:435-447`).
   - R'yi kayba bağlamak (r_res < 0,05) %30 kayıpta R = 6 istiyor. Bu, 0x05'in R_max = 4 değerini aşıyor (`profile.rs:86-90`), yani profil kataloğu değişikliği.
   - EVOLUTION §9.2'deki "budget_ref_bps / K / gate seviyesini sayaca bakarak ayarlama" yasağının kaldırılması.
3. **Kova ve zamanlayıcı.**
   - Kova tavanına bekleyen parite başı katılır (`sender.rs:2366-2381`).
   - Zamanlayıcıda §13 sırası ya da parite/retx payı (`sender.rs:4285-4311`). Seçenek: FloorCap rezervi (`sender.rs:912-968`, `:5742-5762`).
   - **[SAHİP]** Karar verilecekler:
     - DATA-önce mührü: `sender.rs:4308-4310`; pin testleri `crates/adaptive-transport/tests/sprint5c_fec_emit.rs:71-101` (`data_before_fec_parity`) ve `crates/adaptive-transport/tests/budget_equivalence_pin.rs:158-170`; `docs/roadmap/kama-transport-roadmap-status-board.md:153`.
     - Üç metin birlikte: engine §13 :962, §16.1 :1064 ve donmuş ARCHITECTURE §4.9 (:116-120, "Öncelik/tahkim mevcut kod sırasıdır").
     - burst_floor ve rezerv deseni, durdurulan 05b'den geliyor (İŞEMRİ §2).
     - FloorCap'ın "Product/admin-activation-YOK" statüsünden çıkarılması (`:916`).
4. **Hıza talep türevli alt sınır ve ProbeUp'ın kayıp vetosunun kaldırılması** (`rate.rs:164-177`; `sender.rs:4239-4275`, `:3993-3996`, `:4076`, `:4132-4139`). **[SAHİP]**
   - INV-RATE-1 metni: CLAUDE.md §4 ve engine :83-85 ek cümlesi ("faz yalnız kanal ölçümü olaylarıyla değişir"). CLAUDE.md'deki metin yalnız sahip kararıyla değişir.
   - fec-core §0.8 "AYRI-DECISION + Abkarada onayı".
   - engine :1072 abort koşulu.
   - 05b startup-floor'unun süreğen hâli olması.
5. **PTT'yi ayrı oturuma alıp o oturumda 0x01 (K2/E64) + `manual_r`** (`runtime.rs:282-285`); tel değişmez. **[SAHİP]** profil politikası ve PTT'nin ayrı oturuma alınması.
   - Ürün bugün PTT, TEXT, sinyal ve dosyayı aynı oturumda ve aynı blok dizisinde taşıyor (`runtime.rs:2490-2500`, `transport.rs:1077-1080`).
   - E=64'te birim tavanı 53 B (`runtime.rs:236-245`); TEXT ve SIG sığmıyor [boylar doğrulanmadı].
   - fec-core §11 küçük-E profili istiyor. Sabit `manual_r` ise K-FEC-2'nin "son tarih hedefli, sabit formül yok" seçici kuralını karşılamıyor; geçici yol olarak açıkça kaydedilmeli.
6. **Süre ya da son tarih tetikli kısmi blok kapanışı; karışık oturumda sınıf başına blok numarası ve profil.** **[SAHİP]**
   - donmuş ARCHITECTURE §4.7 (`ARCHITECTURE.md:76-82`)
   - fec-core §6 PATCH-4 (:208-210, K_actual açıkça taşınır)
   - fec-core §5 "oturuma tek profil" kuralı
   - tel ve S1 sözleşmesi
7. **PTT `MEDIA_BUNDLE` → DeadlineBounded; sırasız ya da son tarihli teslim.**
   - **[SAHİP]** AMP PTT-R0 (K4G-08) ve S1 §4.
   - **[SAHİP]** Tel: alıcının sınıfı bilmesi için DATA başlığında bir sınıf biti ya da sınıf başına unit_id uzayı gerekir [çıkarım]. Bu bir byte-layout değişikliği ve talimatsız yapılamaz.
   - Süresi dolan birimin gönderende düşmesi S1 §4'ün zaten istediği davranış; o kısım için sahip kararı gerekmez.
8. **Dar kalemler.**
   - Sözleşme değişmeyen kalem: zaman eşiği tabanını ölçülen return_bw'ye bağlama (`sender.rs:3396-3412`; engine :1051 bunu öngörüyor).
   - **[SAHİP]** spec eki gerekenler:
     - PTO çapası ve 2 prob (`tail_recovery.rs:72-82`, `:176-180`)
     - gap'te anında ACK (`receiver.rs:247-256`)
     - tamamlanan bloğun paritesinin iptali (`sender.rs:5243`; fec-core §9 :300-302 kuyruktaki onarımın iptalini tanımlamıyor)
     - plato sayımından uygulamayla sınırlı turların çıkarılması (engine :248; #15)
   - **ACK göndermeden önce teslim kanalında yer** (`runtime.rs:3602-3605`).
     - Sözleşme değişmiyor.
     - Ama sınır, beyanlı istisna olarak kabul edildi (#114/#116): `docs/gates/G-IMPL-S3-OKUMA-PAKETI.md:189-194`, `docs/reports/INT-02-RAPORU.md:296`, `docs/plans/KAMA-PLAN-4GUN-APP-KRIPTO.md:392`.
     - **[SAHİP]** Kaydın güncellenmesi.
9. **Kuruluş.**
   - Idle'da kova kırpması.
   - PreConverge'de PTO arm'ının dondurulması (engine :935-936).
   - PreConverge çıkışı.
   - Kanıt gerektirmeyen ilk uçuş tabanı (kodlanmış ilk uçuş).
   - **[SAHİP]** 05b'nin kalan adımları durduruldu (İŞEMRİ §2); cwnd_gain spec :660; MIN_CWND [CAL] (`rate.rs:93`).
10. **Kabul şeridi ve S1 öncelik ipucu (#13, K-1).**
    - Hız düzeltmesinden bağımsız: FIFO ilk geçiş sınırı r12k'da 46 s, r1k'da 71,8 s.
    - **[SAHİP]** S1 imzası.
11. **Kanıt zincirinin 6. adımı: QUIC kıyası.** **[SAHİP]** RFC ilk pencereli quinn kolunun eklenmesi, rakip tanımını (arms_rakip.sh) değiştiriyor.

## Çürütülenler / düzeltilenler / doğrulanamayanlar

**Çürütülen ya da daraltılan iddialar:**
- **"+24 s'lik farkın tamamı ilk iletim beklemesinden."**
  - Fark iki bileşenli.
  - Koşu başına p95(teyit − ilk iletim beklemesi) tabanda 19,78 s. Bu bir muhasebe ayrıştırması, karşı-olgu değil.
- **"Kama'nın açığı min_cwnd tabanından."**
  - Bağlayan kısıt gain × BtlBw pacing.
  - 4800/srtt tavanı yalnız ilk 0–5 s'de bağlıyor.
- **"quinn'in üstünlüğü 240 kB ilk pencereye özgü."**
  - Test edilmedi.
  - RFC ilk pencereli BBR de büyük olasılıkla kısa süre pencereye takılırdı [türetilmiş].
- **"Teslim fizikçe doymuş, 'ayırt edilemez' sonucu zorunlu."**
  - Yalnız ideal ARQ için doğru.
  - Gerçek kollarda eksik koşular var: taban 40, aday 17, quinn 53 / 960.
- **"ProbeUp 1,25 × 0,7 < 1, p > %20'de aritmetik olarak ulaşılamaz."**
  - ~~Başarı testi ölçekten bağımsız, bu yüzden aritmetik geçersiz.~~
    **[CORRIGENDUM-1 (2026-11-09)]** Bu çürütme yanlış; iddia (aritmetik) geçerli.
    - Başarı testi (`sender.rs:4072-4075`, `a858f3fc`) probun max-filtresini giriş anındaki BtlBw'nin 1,025 katıyla kıyaslıyor.
    - Steady'de pace = 1 × BtlBw (`:4255`). Giriş değeri zaten bir teslim hızı. Prob 1,25 × giriş gönderiyor; teslim örneği ≈ 1,25 · (1 − p) · (1 + b′) × giriş.
    - (1 − p) yalnız bir kez giriyor. %30'da 1,25 × 0,7 = 0,875 < 1,025; başarı ancak gürültü payı b′ > 0,17 olursa geliyor. Ölçülen max-filtre payı b ≈ 0,06–0,11 [türetilmiş].
    - Çürütme yalnız Steady gönderimi BtlBw/(1 − p) olsaydı doğru olurdu.
    - Doğru ifade: ProbeUp %30 kayıpta yalnız gürültüyle başarılı kapanır; ayrıca aşağıdaki mekanizmalar da onu kesiyor. İkisi birlikte çalışıyor.
    - Dayanak: orkestratörün 2026-11-04 sıkışıklık denetleyicisi çalışması §1.3; sahip kararı v4 (2026-11-09) Bölüm I "Corrigendum EVET".
  - İşleyen mekanizma: RecoverArq önceliği, bekleyen kayıpta preempt ve atıflı kayıpta abort.
  - Başarı/abort sayacı yok.
    **[CORRIGENDUM-1 (2026-11-09)]** Sayaç F0 verisinde yok; üründe var: `diag_probe()` (başarı, iptal, zaman aşımı, preempt; `sender.rs:6418-6426`, `a858f3fc`). F0 düzeneği basmadı.
- **"İstenen hücrelerde blok bekleyişi ≥ RTT."**
  - Yalnız r1k, r25 ve c_b120'de doğru.
  - Manşette 2,00 s < srtt.
- **"r1k'da FEC kazancı yok."**
  - Modelde r1k 1 KB d1000'de −1,9…−3,5 s kazanç var.
  - Zarar yalnız r1k 65 B'de.
- **"RTT 0,6 s'de FEC ARQ turunu kaldırmıyor."**
  - Kısmen kaldırıyor: r12k/d300 0,26–0,58, r1k/d300 0,11–0,42 (bench, statik E=64).
- **"r düzeltilse de konuşma sırasında parite akmaz."**
  - Hıza bağlı.
  - Dosyalı hücrelerde aday kolu konuşma penceresinde 146 / 42 / 218 parite gönderdi.
- **"Parite bekleyişi parite boyundan bağımsız."**
  - Boyuna bağlı. E=64'te bile parite (84 B), 64 B'lık PTT çerçevesinden büyük.
- **"DATA-önce sırası tek başına yetiyor."**
  - Kilit üç şeyin birleşimi: DATA-önce sırası, ilk rette break ve paritenin her DATA biriminden büyük olması.
- **"Tek yalıtılmış kayba tasarım gereği parite yok."**
  - Spekülatif kol tek bir şüpheli kayba tepki verecek biçimde tasarlanmış (`sender.rs:407`, `:412`).
  - Kayıp blok kapanmadan Suspected olursa r = 0'ı kırpma üretir [çıkarım: tespit anı ölçülmedi].
- **"Emergency MinimalParity ≤ %30 kayıpta ulaşılamaz."**
  - Yapısal olarak kapalı değil: 169/2660 koşuda defter tepesi > 550k ppm.
  - Ateşleyip ateşlemediği belirlenemiyor; katkısı ölçülmedi.
- **"Hedef bantların tamamında yapısal sıfır."**
  - Normal adaptif yolda 125 ≤ BtlBw < 7875 B/s aralığında r = 0 kesin.
  - r25 dışında 94 parite çıktı. Kaynağı ayrıştırılmadı; adaylar:
    - kırpmasız anlar (BtlBw = 0 penceresi; MinimalParity için iz bulunmadı)
    - latch anındaki BtlBw'nin koşu sonundakinden yüksek olması
- **"Reorder, ürün PTT gecikmesinin baskın bileşeni."**
  - Payı taban %53–63, aday %34–54.
  - "İlk engelleyici çoğunlukla PTT" yalnız aday için geçerli.
- **"Alıcıda ACK tutmanın üst sınırı yok."**
  - Üst sınır 8,39 s.
  - Yeni epizotta ilk ACK ≤ 20 ms.
- **"PreConverge'de orijinalin yerine her seferinde geçiliyor."**
  - Tek ateşleme var.
  - Manşetteki 64,8 s başka bir mekanizmadan: ilk pencere kırpması ve ilk DATA kaybı.
- **"%30 kayıpta blok çözülemezliği 0,448 / 0,058."**
  - Onarımlar da kaybolduğu için doğrusu 0,617 / 0,276.
- **"Statik FEC'te yeniden gönderim sayısı düşmüyor."**
  - Adayda −%17 / −%7; tabanda bayt olarak −%2,5.
  - Kollar arası kıyas, paritenin eklediği yükle karışık.
- **"Zamanlayıcı sırası bir spec↔kod ayrışması."**
  - Aslında spec içi çelişki: §13 :962 ↔ :1064.
  - Donmuş ARCHITECTURE §4.9 kodu belgeliyor.
- **"ACK'li veri taşmada kaybolmaz bir sahip gereksinimi."**
  - İŞEMRİ'de bu sözlerle yok.
  - Bu, beyanlı bir sınır (G-IMPL-S3, INT-02).
- **"Ürün yolunda parite yalnız dosya trafiğiyle çıkıyor."**
  - Parite esas olarak yüksek BtlBw'li dosya koşularında çıkıyor.
  - Dosyasız r12k_d1000'de de 6/10 koşuda çıktı.
- **"Hız düzeltilse p95 ≈ 8,75 s'ye iner, kazanç −20…−24 s."**
  - Doğrulanamayan bir karşı-olgu.
  - Kama'nın kurtarma bekleyişi quinn tabanının üstünde.
- **"Bench Kama, ürün için alt gecikme sınırı."**
  - Bu yalnız alıcı düzleminde kesin.
  - Ürün yolunda farkın büyüyeceği [çıkarım].
- **Sayı düzeltmeleri.**
  - BtlBw medyanı 2370,5 / 2742,5 B/s.
  - "3,7 RTT" yalnız taban için geçerli; aday 1,96 RTT.
  - İlk iletimi kaybolan birim %31,0 / %31,9.
- **"PRODUCT-PROFILE: ölçülen return_bw kablolandı."**
  - Gözlemci düzeyinde doğru, ama onay ufkunu beslemiyor.
  - Bağlamı içinde yanıltıcı.

**Doğrulanamayanlar [doğrulanmadı]:**
- **Ölçüm ve kıyas.**
  - quinn el sıkışması pencereye katılınca 32 hücrede nokta sayımının 28'den 25'e inmesi.
  - Dönüş ayağı p95'i 3,26 ↔ 1,78 s.
  - r1k 1000 B hücrelerinde ACK tutmasının gönderen ölçüsünü 1,6–3,8 s şişirmesi; alıcı ölçüsünde sayımın 22 → 19 olması.
  - Kama'nın önde olduğu iki r1k hücresinin quinn el sıkışma kuyruğundan kaynaklanması.
  - mixed.rs'de quinn'e verilen akış önceliği.
  - Kısa son tarihlerde teslim farkı.
  - Ürün sırası uygulanınca manşetin 31,3 → 42,9 s'ye çıkması; sıralı nokta sayımı 29 / 23.
- **Mimari.**
  - Hub istemci bağının olmaması; early_unit; admit_and_decode'un ölü köprü olması; INV-FEC-1 sırasının korunduğu.
  - Kayıp tespiti çekirdeğinin geri kalanı (un-loss, Karn, ack_delay) RFC 9002 ile aynı.
- **Kanıt zinciri tasarımı.**
  - Kanca tasarımları (M6-1, M6-2, M6-7).
  - `--dup-copies`'in ayrı unit_id alması.
  - TEXT 75 B, SIG 127 B.
  - `par`'ın başlık-yalnız emisyonu sayması.
  - Delivery yapısının API'de pinli olması.
  - Netem'de N = 10 nötrlük kıyası.
- **Telemetri.** ts.csv'de parity_emitted dışındaki zamanlayıcı sütunlarının yapısal olarak sıfır olması. parity_emitted'in her yerde 0 olduğu doğrulandı: diag-sched-telemetry kapalı.

**Hiç ölçülmeyenler:**
- **Kıyas.**
  - Ürün yolunda (f0_flows) Kama ↔ quinn kıyası.
  - quinn üstünlüğünde ilk pencerenin payı; RFC ilk pencereli quinn.
- **Parite ve zamanlayıcı.**
  - Ürün kolunda (0x05, 1058 B) parite jeton reddinin TokenDenied düzeyinde doğrudan ölçümü. RAPOR-C §0 madde 9 E=64'te ve duman-2 düzeyinde (N=1/kol).
  - Parite birikiminin kova/sıra mekanizmasından mı, hız açığından mı geldiği.
  - ARQ turunun 9/8·srtt'nin üstünde kalan kısmında ACK kaybının ve DATA-önce beklemesinin payı.
  - Kaybın blok kapanmadan Suspected olup olmadığı (tespit anı).
  - Kaybolan RECOVERED'ın etkisi (`runtime.rs:3648-3651`, `:3733-3736`, `:3798-3801`).
  - Aday–taban farkının retx payına bağlanması [çıkarım].
- **Mekanizma sayaçları.**
  - MinimalParity yolunun ateşlenmesi ve parite katkısı.
  - ProbeUp'ın başarı/abort oranı.
  - cwnd/pacing zaman serisi; kuruluş geçişinin kök nedeni çıkarım düzeyinde kaldı.
  - PARAM_SET bayt maliyetinin gecikmeye etkisi.
- **Topoloji.**
  - Hub üzerinden iki atlamalı ürün topolojisi.
  - Bozuk dönüş yolu.
