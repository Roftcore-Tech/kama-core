# KTP2-F0 · Ajan C — FEC / PTT (RAPOR TASLAĞI · tur 1: koşu yok)

**Durum:** Bu turda koşu yapılmadı. Bölüm 1, 2, 3 ve 6 kaynak okumasıdır [kod]. Bölüm 5 ölçümden önce kaydedilmiş beklentidir [çıkarım]. Bölüm 6.3 arşivdeki 05b netem-7 verisinden okundu [ölçüldü — arşiv, betimsel, PTT dışı iş yükü]. Bölüm 8 koşudan sonra doldurulacak.
**Kollar:** `taban` = main `a858f3fc`. `aday` = 05b `49864510` (salt-okunur ağaç `wt/ktp2-05b`). Bu dalın (`ktp2/f0-c`) `crates/` içeriği `a858f3fc` ile aynıdır. Satır numaraları aksi yazılmadıkça tabana aittir.
**Etiketler:** [ölçüldü] · [kod] · [çıkarım] · [betimsel].

---

## 0. Özet

1. **Ürün profilinde FEC açık. Etkisi ise PTT akışında yapısal olarak sıfır [kod].** `KamaV1Profile` `fec: Some(0x05, Controller)` kurar (`runtime.rs:282-285`). Uyarlamalı kolun kararı §8.5 bütçe kırpmasından geçer: `cap = btlbw × 1000 / 125 000` (`sender.rs:3763-3796`, `budget.rs:26-41`). `btlbw` hat kapasitesi değildir; akışın kendi teslim hızıdır. 1,2 kbit/s PTT'de bu ≈ 380–920 B/s'dir; cap 3–7 ‰ olur. Latch r'yi `round(oran × K / 1000)` ile hesaplar (`sender.rs:1026`); K = 8'de sonuç r = 0'dır. Kırpma yalnız tam 0 ‰ verdiğinde atlanır, burada atlanmaz. K = 8'de r ≥ 1 için btlbw ≥ 7,8 kB/s gerekir.
2. **Blok yalnız K birim dolunca kapanır. Süre sınırı yoktur [kod].** Ürün profilinde K = 8'dir (`runtime.rs:2500`). Kısmi blok yalnız uç kapanırken kapanır (`runtime.rs:2579-2632`). İlk çerçeve 7 × T bekler: T = 60/120/200 ms'de 420/840/1400 ms. Bu, 400 ms son tarihin her demette, 800 ms son tarihin T ≥ 120'de üstündedir.
3. **Onarım sembolü E bayta doldurulur [kod].** Ürün profilinde E = 1024'tür (`runtime.rs:2472`, `:2501`). Bir onarım bench'te 1052 B, hub zarfıyla 1058 B'dir. Ses paketi 55–76 B'dir, onarım bunun 14–19 katıdır. 8 kbit/s hatta tek onarımın serileştirmesi ≈ 1,09 s sürer.
4. **Parite kuyruğa blok kapanınca girer, tele pacer izin verince çıkar [kod].** ACK beklemez. Ancak zamanlayıcının en son sınıfıdır (`sender.rs:4292` → `:4311`). Pacer hızı ACK'le ölçülen btlbw'den türer. Her yeni DATA geldiğinde jeton kovası max(DATA boyu, hız × 40 ms)'ye kırpılır (`sender.rs:2366-2381`). Sonuç: ACK'e dolaylı bağlılık ve paritenin aç kalma riski. Arşivde parite jeton reddi koşu başına 2,5–3,7 bin [ölçüldü — arşiv].
5. **Uygulama PTT'yi `Reliable` gönderir; son tarih taşıma katmanına ulaşmaz [kod].** Bkz. `kama-app/src/ptt.rs:20`, `client-core/src/transport.rs:1272-1281`. Süresi geçen çerçeve yeniden gönderilir. Teslim oturum genelinde sıralıdır (`transport.rs:367` `Reorder`).
6. **Ön-kayıt, birincil hücre (r12k · RTT 600 · %20 · 120 ms · D = 800) [çıkarım]:**

   | kol | beklenen son-tarih oranı |
   |---|---|
   | arq | ≈ 0,80 |
   | uyar | ≈ 0,80 (parite 0) |
   | rs2 (K=2, r=2) | 0,97–0,98 |
   | rs4 (K=4, r=2) | 0,86–0,92 |
   | tekrar | ≈ 0,96 |

   D = 400 ms'de tavan ≈ 0,32. RTT 2 s'de 400/800 ms fiziksel olarak imkânsızdır (§3.4).
7. **Duman (vekil kip, N = 1/kol, 42 koşu) yeni bir davranış gösterdi: gönderen kilidi (B-C9, §9) [ölçüldü — duman, betimsel].**
   - 42 koşunun 21'inde pacer, sunulan + yeniden gönderim hızının altında kaldı.
   - Gönderen kuyruğu konuşma boyunca büyüdü: kaybolmamış çerçevelerin bile en küçük ACK gecikmesi 0,5 s'den 5,5–8,8 s'ye çıktı. Bu koşularda son-tarih oranı ≈ 0,06–0,10.
   - Kilit, kolu ya da ikiliyi izlemiyor; aynı hücrede taban/aday ters yönde düşebiliyor.
   - Parite tel baytı btlbw'yi yükselttiği için `rs*` kolları r12k'da %20'de kilide 0/8 girdi (r1k'da 1/4).
   - Ön-kayıttaki M3 varsayımı ("veri sunum anında gönderilir") kilitli koşularda tutmuyor. Ön-kayıt değiştirilmedi; sapma §9'da.
8. **İSTEK-C1 (Ajan D, yalnız bench).** `serve --rx-log` ile alıcıda birim başına varış damgası gerekir. Bugünkü `delivered_ns`, gönderenin ACK'i gördüğü andır (`kama_product.rs:24`, `:780-790`). Bu değer dönüş yolu gecikmesini (300/1000 ms) içerir; son tarih ölçüsü olarak kullanılamaz. **Durum: yapıldı (taban `2cd9f68d`); duman-2'de doğrulandı (§10).**
9. **Duman-2 (alıcı damgalı, N = 1/kol) [ölçüldü — duman, betimsel]:**
   - B-C4 parite açlığı ölçüldü. r12k/RTT 600/%20'de parite kuyruk beklemesi (Little) 1,2–15 s. Blok T = 120 ms'de kapanıyor. Kaybolan DATA, ARQ ile ≈ 1,1–1,5 s'de geliyor; çoğu zaman parite ondan önce yetişmiyor.
   - Aynı hücre RTT 2 s'de: bekleme 0,11–0,46 s. Son-tarih oranları (T2-800) ön-kayıtla uyumlu: arq 0,81, rs2 0,97–0,99, rs4 0,90–0,92.
   - B-C9 gönderen birikimi alıcıda da görünüyor: pencere-minimum alıcı gecikmesi 0,25 s → 13 s.
10. **Tam set sonuçları:** bu raporun `# TAM SONUÇ` bölümü (4160 koşu). Yukarıdaki maddeler ön-kayıt ve duman dönemine aittir; çelişkide TAM SONUÇ geçerlidir.

---

## 1. Koddan bulgular: mevcut FEC ürün yolunda neden kazanç üretmiyor

Her bulgu dört parçalıdır: ölçüm · dosya/işlev · kök neden · önerilen en dar düzeltme. Düzeltmeler önerilir, uygulanmaz.

### B-C1 · Uyarlamalı paritenin oranı bütçe kırpmasıyla r = 0'a yuvarlanıyor

**Ölçüm.**
- PTT hücreleri için ön-kayıt: `uyar` kolunda parite = 0 (§5.3).
- Arşiv, 05b netem-7, 100 × 256 B @ 50 ms, ürün profili [ölçüldü — arşiv, §6.3]:
  - `p1_loss50`: kırpılan karar sayısının medyanı koşu başına 6,5–9,1 bin. Parite yalnız 11–12/20 koşuda var. Medyan 2–5,5 onarım/akış (12 blok).
  - `p7_bbr` (%2): parite 0, kırpma 2,6–3,6 bin.

**Dosya/işlev.**

| | taban | aday |
|---|---|---|
| `Sender::fuzzy_evaluate` (arbiter çıkışında §8.5) | `sender.rs:3763-3796` | `sender.rs:4240-4267` |
| `budget_cap_permille` | `budget.rs:38-41` | `budget.rs:38-41` |
| `budget_ref_bps = 125 000` | `budget.rs:30` | `budget.rs:30` |
| `compute_requested_r` | `sender.rs:1026-1031` | — |
| latch çağrısı | `runtime.rs:2503-2525` | — |

**Kök neden.**
- `cap = btlbw·1000/125 000` dar bantta bilerek küçüktür. Ancak `btlbw`, akışın kendi uygulama-sınırlı teslim hızıdır (`bw.rs:233` app-limited kapısı).
- PTT 1,2 kbit/s'de btlbw ≈ sunulan UDP hızıdır: T = 120'de 533 B/s → cap 4 ‰.
- Latch r'yi `(oran × K + 500)/1000` ile tamsayıya çevirir. K = 8 ve K = 4'te sonuç 0'dır.
- "Sıfırlama reddi" (`if clamped == 0 && final_ratio > 0`, `sender.rs:3786`) permille düzeyinde bakar, maddeleşen r düzeyinde bakmaz. Bu yüzden 1–62 ‰'lük kırpılmış oran sessizce r = 0 olur.
- `latch_rounded_to_zero` sayacı ürün tanısında var, ama `ClientStats`'a taşınmıyor; bench'te görünmez.

**En dar düzeltme (F1 önerisi).** İki seçenek:
- (a) Sıfırlama reddini r düzeyine taşı: `r(kırpılmış) == 0 ∧ r(kırpılmamış) > 0 ⇒ r = 1`.
- (b) Son tarihli ses sınıfında §8.5 kırpmasını, ölçülen teslim hızı yerine ilan edilen ya da ölçülen **hat** kapasitesine göre uygula.

İkisi de bu turda ölçülemez. Ölçülen hücrelerde `rs*` kolları kırpma olmadan ne kazanıldığını gösterir.

### B-C2 · Blok yalnız K birim dolunca kapanıyor; zaman sınırı yok

**Ölçüm.** Ön-kayıt: `rs4` (K = 4) konuma göre T1-800 oranı. Blok başındaki çerçeve en düşük olmalı (§5.3).

**Dosya/işlev.**
- `client_task` sunum dalı `if cur_block.len() == p.k_shape_usize()`: `runtime.rs:2500` (aday `:2581`).
- Kısmi/son blok yalnız ana döngü bittikten sonra kapanır: `runtime.rs:2579-2632` (aday `:2660`). Ana döngü uç kapanırken biter.

**Kök neden.** Blok oluşturma süresi, bloğun i'nci çerçevesi için (K − 1 − i) × T'dir. Ürün profilinde K = 8: blok başı 7T bekler. Taslağın "blok süresi dolunca kısmi blok" kuralının (§3.8) kodda karşılığı yok. Konuşmanın son, K'ya tamamlanmayan çerçeveleri korumasızdır. Onların paritesi oturum kapanırken üretilir.

**En dar düzeltme.** ~~Sınıf başına blok zamanlayıcısı: mevcut kısmi-blok kodu ana döngüde de çalıştırılır.~~ **Düzeltme (TAM SONUÇ TS.3):** bu öneri tek başına yetmez. Blok ve ESI iki uçta da `unit_id`'den türer (`runtime.rs:2498`, `:3617-3618`); bloğu erken kapatmak sonraki birimlerin eşlemesini bozar, kimlik atlamak da `Reorder`'ı durdurur. Ses için K = 2 profili yeterli bir karşılıktır (ölçüldü).

### B-C3 · Onarım E bayta dolduruluyor; ürün profilinde E = 1024

**Ölçüm.** Ön-kayıt: `B_cerceve_l2` ve parite baytı (§5.2). Tek onarımın serileştirmesi r1k'da 1094 ms, r12k'da 87,5 ms [çıkarım; L2 1094 B].

**Dosya/işlev.**
- Sembol dolgusu `let mut s = vec![0u8; p.e_usize()]`: `runtime.rs:2472` (aday `:2553`).
- `parity_wire = wire_len(HDR_FEC + e)`: `runtime.rs:2501`.
- Profil 0x05 K = 8, R = 4, E = 1024: `kama-fec-codec/src/profile.rs:85-91` (girdi `:86`), `runtime.rs:240`.

**Kök neden.**
- Kaynak birim dolgusuz DATA olarak gider: 11 B başlık + yük + zarf, önek telde yok (`runtime.rs:2462-2486`, `wire.rs:17`).
- Onarım ise her zaman zarf + 12 + E'dir. Bench'te 16 + 12 + 1024 = 1052 B, hub zarfıyla 22 + 12 + 1024 = 1058 B.
- Taslağın "64 B sembole 37 B çerçeve" ifadesi bu yolu anlatmaz. 37 = `EXTENDED_OUTER_OVERHEAD` 26 + `SOURCE_PREFIX_LEN` 11 (`profile.rs:83` yorumu; `kama-transport-schema/src/lib.rs:37,41`). Bu, R2A/FS-1 süreç-içi modelinin 101 B'lık sembolüdür (`kama-integration/tests/common/fs_spine.rs:383`).
- Ürün yolunda gerçek çerçeve yükü:
  - kaynak: 27 B (bench) / 33 B (zarf 22);
  - onarım: 28 B (bench) / 34 B (zarf 22), artı E'ye dolgu.
- 18 B'lık bir ses demetinin onarımı E = 64'te 92 B, E = 1024'te 1052 B'dir (bench).

**En dar düzeltme.** Ses sınıfı için E'si çerçeve boyuna yakın bir profil kullan. Katalogda var: 0x01 K2/E64, 0x02 K4/E64. Ölçülen `rs2`/`rs4` kolları tam bunu kullanır. Profil seçimi PARAM'la bağlanır (`runtime.rs:354` `param_fields_for`); tel biçimi değişmez.

### B-C4 · Parite pacer'ın en son sınıfı; jeton her DATA gelişinde kırpılıyor

**Ölçüm.**
- Ön-kayıt: `parite_bekleme_ms` (Little: ∫kuyruk dt / emisyon, `ts.csv` `pending_parity`).
- Arşiv `p1_loss50`: parite jeton reddi medyanı koşu başına 2,5–3,7 bin [ölçüldü — arşiv].

**Dosya/işlev.**
- `schedule_transmit` sırası: `drain_new_data` `sender.rs:4292` → ARQ/kuyruk probu/kuruluş/kopya → `plan_fec_parity` `:4311`.
- `plan_fec_parity` `pacer.try_send(p.size)` reddinde döngüden çıkar: `:5319-5334`.
- `on_event_core` burst kırpması `set_burst_cap(clamp(hız × 40 ms, ön DATA, …))`: `:2366-2381`.
- Aday: parite ayrıca kopya başı rezervinin altına inemez (05b `sender.rs:5868` `copy_reserve_allows`). `burst_floor_bytes` parite başını kapsamaz (05b `sender.rs:7520-7531`).

**Kök neden.**
- Statik modda parite blok kapanınca kuyruğa girer (`close_fec_block_manual`, `sender.rs:4926-4957`). Bu noktada ACK beklemez. Ajan E'nin H-15 notu doğru: doğrudan ACK saatine bağlı değil.
- Tele çıkış ise pacer jetonuna bağlıdır. Pacer hızı = kazanç × btlbw (`rate.rs:164-180`); btlbw ACK'le ölçülen teslim hızıdır. PTT gibi uygulama-sınırlı akışta hız ≈ sunulan hız olur.
- Her `AppData` olayında kova max(DATA, hız × 40 ms)'ye kırpılır (T = 120'de ≈ 64 B). Önceki çerçeveden artan jeton silinir. Parite ancak iki çerçeve arasında `parite boyu / hız` süresi kadar jeton biriktirebilirse gider.
- Hız = A iken (Steady, kazanç 1) bu birikim parite için yetmeyebilir; model bu durumda pariteyi aç bırakıyor (§5.1 M3, `pacer1.00` varyantı). Dolaylı ACK saati bağlılığı budur.

**En dar düzeltme.** İki seçenek:
- (a) 05b'nin `burst_floor_bytes` tabanına bekleyen parite başını ekle. Bu yalnız kovayı yükseltir, jeton üretmez; INV-RATE-1'e dokunmaz.
- (b) Son tarihli sınıfın paritesini, blok kapanışında bloğun r × parite boyuyla sınırlı tek seferlik bir hakla gönder.

### B-C5 · Uyarlamalı kontrolcünün ek kapıları kısa PTT patlamasında ilk saniyeleri korumasız bırakır

**Dosya/işlev.**
- PreConverge'de arbiter çıkışı 0: `sender.rs:3798-3804`.
- Yapı kapısı:
  - fuzzy = 0 iken uygulanır;
  - `loss_ppm ≥ 100 000`: `:1453`, `:3882`;
  - pencere `n ≥ 32`: `:1456`, `:3886`;
  - IID `burst_delta ≤ 50 000`.
- Spekülatif kol `bounded_v1` tavanı 250 ‰ (`:435-447`).
- Kayıp defterinin akış başı ölü zamanı (~16 × RTT, `reports/loss-ledger-confirm-defect/`).

**Kök neden.** Kural %10 kayıpta sınırdadır: loss_ppm ≈ eşik. RTT 600 ms'de ≈ 10 s, 2 s'de ≈ 32 s kanıt birikmeden yapısal karar yoktur. B-C1 bunların hepsini zaten sıfırlıyor. Bu kapılar ancak B-C1 düzeltilirse ikinci engel olur.

**En dar düzeltme.** Ses sınıfında oranı kontrolcüden değil, ölçülen kayba göre sabit (K, r) tablosundan seç (taslak §3.8). F0'da `rs*` kolları bu tabloyu vekil olarak ölçer.

### B-C6 · Ürün PTT'si `Reliable` ve sıralı; son tarih motora ulaşmıyor

**Dosya/işlev.**
- `kama-app/src/ptt.rs:18-24`: PTT_OPEN / MEDIA_BUNDLE / PTT_CLOSE, `DeliveryPolicy::Reliable`.
- `client-core/src/transport.rs:1272-1281`: süre yalnız `DeadlineBounded`'da üretilir.
- `runtime.rs:2434-2437` → `sender.rs:4354-4359` `declare_delivery_deadline`: yalnız politika girdisidir; motor süresi geçen birimi düşürmez.
- Oturum geneli sıralama: `transport.rs:367-395` `Reorder`.

**Kök neden.**
- Geç çerçeve hem yeniden gönderilir (tel baytı) hem de sonraki çerçeveleri bekletir. Sıralı teslim bir RTT'den uzun duraklar.
- Bench yolu (`kama_product.rs`) runtime'ı doğrudan sürer, `Reorder`'ı atlar; ölçülen oranlar ürün uygulama yolunun **üst sınırıdır**.
- Analiz alıcı damgalarından "sıralı teslim" türevini de hesaplar (`ot_sirali_*`, §4).

**En dar düzeltme.** İki parça:
- MEDIA_BUNDLE'ı `DeadlineBounded` ile ver.
- Motorda süresi geçen birimi yeniden gönderim ve kuyruktan sil (K-6 "süresi dolan parça gönderende silinmiş").

Bu, kayıtlı K4G-08 kararının (PTT Reliable) değiştirilmesi demektir. Sahip kararıdır; burada yalnız ölçülen bedel raporlanır.

### B-C7 · Ölçüm düzeneği: bench teslim anı gönderenin ACK gözlemidir

**Dosya/işlev.**
- `kama_product.rs:24-26` (belge), `:780-790` (`delivered_ns = elapsed` ACK/RECOVERED anı).
- Sunucu teslimleri yalnız sayar, kaydetmez: `kama_product.rs:590-594`.

**Kök neden.** Gönderen teyidi = alıcı varışı + dönüş tek yönü (300/1000 ms ± σ) + ACK gecikmesi (≤ 20 ms, `runtime.rs` `ACK_TICK`) + ACK kaybı. Son tarih ölçüsü için kullanılamaz.

**Düzeltme.** İSTEK-C1 (§7.1). Yalnız bench'e dokunur; ürün kodu değişmez.

### B-C8 · 05b adayı FEC yolunda neyi değiştiriyor

**Dosya/işlev.** Aynı kalanlar (05b satırları):

| | 05b satırı |
|---|---|
| blok kapanışı | `runtime.rs:2581` |
| kısmi blok | `runtime.rs:2660` |
| E dolgusu | `runtime.rs:2553` |
| §8.5 kırpma | `sender.rs:4240-4267` |
| PreConverge kapısı | `sender.rs:4269` |
| yapı kapısı | `sender.rs:4353` |
| parite sırası | `sender.rs:4779` → `:4801` |

Farklar:
- Parite kopya başı rezervinin altına inemez (`:5868`).
- Kova tabanı (`apply_burst_floor`, `:7549`) DATA / ARQ / prob / kopya başını kapsar, pariteyi kapsamaz.

**Kök neden.** 05b dar düzeltmeleri ARQ sahipliği, srtt ve açlık içindir. FEC'e dokunmaz; paritenin önceliğini bir rezerv kadar daha düşürür [kod].

**Ön-kayıt.** FEC kollarında aday ≈ taban; `rs*` parite beklemesinde aday ≥ taban [çıkarım].

### B-C9 · Gönderen kilidi: pacer, kayıplı uygulama-sınırlı akışta gereken hızın altına kilitleniyor (duman bulgusu)

**Ölçüm** [ölçüldü — duman, vekil kip, N = 1/kol; betimsel; §9].
- `c_r12k_d300_l20_b120`'de 12 koşunun 5'i kilitli: arq_taban, uyar_aday, tekrar ×2, pol_taban.
- Kilitli koşularda:
  - pencere-minimum ACK gecikmesi konuşma boyunca 0,5 s'den 6–9 s'ye doğrusal artıyor (`birikim_ms` 5533–8837);
  - son btlbw / sunulan UDP hızı 0,85–1,11;
  - ts.csv'de koşunun son 2/3'ünde jeton < 1 DATA paketi (`jeton_ac_payi`) 0,98–0,99;
  - son-tarih oranı (vekil) 0,06–0,10.
- Kilitsiz koşular:
  - birikim ≈ 0 (−49…156 ms);
  - btlbw/sunulan 1,31–2,88;
  - oran 0,65–0,86.
- %50 kayıpta (`l50`) 10 koşunun 8'i kilitli. Bunların 5'inde ilk iletimlerin yalnız %34–73'ü pencere içinde tele çıktı (aralık 0,34–0,96).
- srtt bütün koşularda gerçek RTT'ye yakın (r12k/d300'de 571–773 ms). Bu, srtt şişmesi değildir.

**Dosya/işlev.**
- `bw.rs:220-245` `BtlBwFilter::update`: uygulama-sınırlı örnek yalnız mevcut btlbw'den büyükse girer; kuyruk doluyken örnek = teslim edilen hız.
- `rate.rs:164-180` `effective_send_rate`: hız = kazanç × btlbw, en çok cwnd/srtt.
- `sender.rs:4237-4268` `compute_send_rate`: Steady kazancı 1.
- ProbeUp periyodu 4 ack-epoch (`sender.rs:1537`, `:4098`); koşu başına 1–3 kez girildi (`probe_up` sayacı).
- Aday aynı yolu kullanıyor (05b `sender.rs:4710-4740`).

**Kök neden [çıkarım, kodla tutarlı].**
- Teslim hızı örneği kaybolan baytı saymaz. Gönderen kuyruğu doluyken örnek ≈ pacer hızı × (1 − p).
- Oysa kayıplı akışın gereken gönderim hızı ≈ sunulan / (1 − p).
- Steady'de kazanç 1 olduğu için btlbw kendi teslim hızına kilitlenir. Kilitten çıkışın tek yolu seyrek ProbeUp'tır (1,25 ×). p = %20'de 1,25 × 0,8 = 1,0: nötr. p = %50'de 0,625: söner.
- Koşunun kilide girip girmemesi, ısınma/Startup'ta yakalanan en yüksek örneğe bağlı görünüyor. Bu iki kararlı davranış N = 30 ile oran olarak ölçülecek.
- FEC paritesi teslim edilen bayta eklendiği için btlbw'yi yükseltir. Duman'da `rs*` kolları r12k/%20'de 8/8 kilitsiz.

INV-RATE-1 açısından: kayıp hızı açıkça düşürmüyor. Ama hız tahmincisi kayıplı teslimi ölçtüğü için yeniden gönderim bandı hiç ayrılmıyor.

**En dar düzeltme (F1 önerisi; ölçülmedi).** Uygulama-sınırlı akışta pacer hızına bir alt sınır: son pencerede sunulan hız × (1 + pay). Sınır cwnd/srtt'yi aşmaz. Bu btlbw'ye sentetik örnek yazmaz; 05b'nin "startup floor" deseninin süreğen karşılığıdır. Ajan B'nin düşük bant hücreleriyle aynı kök olabilir; orkestratöre çapraz not.

### Ajan E notlarının doğrulaması

**H-13 doğru [kod].**
- `runtime.rs:240`: 0x05.
- `:282-285`: `Some(0x05, manual_r: None)`.
- `:1863-1876`: `manual_r` yoksa `FecActuationMode::Controller`.
- Onarım 22 + 12 + 1024 = 1058 B (hub zarfı). Bench'te 1052 B.

**H-15 doğru [kod].**
- `:2500` K dolunca kapanış.
- Kısmi blok `:2579` blok yorumundan itibaren.
- `sender.rs:4292 < :4311`.
- Nüans: parite ACK'le tetiklenmez. Ama beklediği pacer hızı ACK'le ölçülen btlbw'dir, ve DATA gelişindeki kova kırpması birikimi siler (B-C4).

**Ek (E) doğru.** `budget.rs:29` `min_rate_bps: 31`, `sender.rs:4268` `send_rate_final`. PTT hücrelerinde bu taban etkisizdir; sunulan hız ≥ 380 B/s.

---

## 2. Kollar ve bench bayraklarına eşleme

Hepsi `--transport kama-product`, ürün runtime'ı. Sunucu ve istemci aynı FEC bayraklarını alır (`main.rs` `--fec-profile` "İSTEMCİYLE AYNI").

| kol | bayraklar | ne ölçer | beyan / sapma |
|---|---|---|---|
| `arq` | `--fec-profile 0` | ARQ + kuyruk kurtarma, FEC alt sistemi yok (`FecArm::Off`) | ürün profilinden sapma (`kama-custom`) |
| `uyar` | FEC bayrağı yok (0x05, `adaptive`) | **ürün profilinin kendisi**: mevcut uyarlamalı FEC | `profile_delta` boş → `kama-v1-full` / `kama-v1.1-full` |
| `rs2` | `--fec-profile 1 --fec-mode static --fec-r r` | sabit RS, K = 2, E = 64; r kayba göre (§3.5) | kontrolcü ve §8.5 kırpması YOK; pacer VAR (B-C4) |
| `rs4` | `--fec-profile 2 --fec-mode static --fec-r r` | sabit RS, K = 4, E = 64; taslak §3.8 tablosu | (3,2) ve (4,9) katalogda yok → (4,2), (4,4) |
| `tekrar` | `--fec-profile 0`, istemci `--dup-copies 2` (İSTEK-C2; tam set) | tekrar: her sunumda iki kopya arka arkaya, birim = W + 2f + c; çerçeve = ilk varan kopya | duman-1/2'de C2 öncesi vekil koştu (`--msgs 2N --submit-interval-ms T/2`); aşağıda |
| `uyar64` (teşhis) | `--fec-profile 2 --fec-mode adaptive` | uyarlamalı, E = 64: geometriyi kontrolcüden ayırır | yalnız taban, `cx_` hücreleri, N = 10 |
| `pol` (teşhis) | `--fec-mode policy --fec-deadline-ms 800−T` | son tarih politikası (`calibration_v2_deadline`) | yalnız taban, `cx_`, N = 10; %30 bölge dışı (`loss_ppm_max 250 000`) |

**"Sabit RS'in onarımı gerçekten blok dolunca mı gidiyor?" [kod].**
- Kuyruğa evet: `close_fec_block_manual` K'ıncı sunumla aynı olayda çağrılır (`runtime.rs:2503-2507`).
- Tele hayır, garanti değil: pacer'ın son sınıfıdır ve jeton kırpmasına tabidir (B-C4).
- Bu gecikme `parite_bekleme_ms` (Little) olarak ölçülür. "Blok dolunca hemen" bugünkü kodla ürün davranışını değiştirmeden kurulamaz. `rs*` kolları en yakın vekildir ve beyan edilir.

**"K = 1'li profil tekrar sayılır mı?" [kod].**
- Katalogda K = 1 yok (`profile.rs:54-99`, en küçük K_shape = 2).
- Eklemek kodlayıcı kataloğu + PARAM özeti değişikliğidir; ürün kodu olduğu için yasak.
- Olsaydı da onarım E'ye dolgulu ve 12 B FEC başlıklı olurdu. Yani "dolgulu tekrar" olurdu, düz kopya değil.

**Tekrar kolunun önyargıları (beyan).**
- Vekil ve C2'de ortak: her kopya ayrı birimdir ve ayrı ARQ'lanır. Kardeşi gelmiş kopya da yeniden gönderilir; tel baytı ideal tekrardan fazladır.
- Yalnız vekilde: ikinci kopya T/2 sonra gider. C2'de iki kopya aynı sunumda arka arkaya gider.
- Ön-kayıt iki kol için ayrı satırdadır: `tekrar` = vekil; `tekrar2` = C2. `tekrar2` sonradan eklendi; önceki 480 satır bayt bayt aynı kaldı (§10).

**Aynı son tarih, aynı bant bütçesi.**
- Son tarih bir koşu parametresi değil, analiz parametresidir. Aynı koşudan D = 400/800 ve iki tanım hesaplanır (§4).
- Bant: bütün kollar aynı netem hızında koşar.
- Kol başına L2 tel baytı/çerçeve ölçülür.
- Bütçe eşlenmesi: birincil hücrede `rs2` (2,2) 2000 B/s, `tekrar` ×2 1767 B/s L2 (ARQ hariç; +%13) [çıkarım].
- Sonuç yorumu "zamanında çerçeve / kB" ile yapılır.

---

## 3. Ses geometrisi

### 3.1 Kodlayıcı hızı: 1,2 kbit/s

Gerekçe [çıkarım]:
- (a) Taslak §6 K-6 ve §9 bu hızı kullanıyor.
- (b) Dar bant taktik ses sınıfındadır: MELPe STANAG 4591 1200 bit/s modu, Codec2 1200.
- (c) 8 kbit/s hatta bugünkü motorla FEC'siz sesin sığdığı en yüksek mertebedir. 2,4 kbit/s'de 120 ms demet: 36 + 46 = 82 B UDP, L2 124 B × 8,33 = 1033 B/s > 1000 B/s. Sığmaz.

Not: Codec2 1200'ün çerçevesi 40 ms / 6 B'dir. 60 ms bu çerçevenin katı değildir; 9 B nominal alındı.

### 3.2 Ses tel hızı (FEC'siz) [kod + aritmetik]

Paket başına bileşenler:
- hub zarfı 22 B (`kama-noise-wire/src/legacy.rs:25`);
- DATA başlığı 11 B (`wire.rs:17`);
- AMP 13 B (`kama-app/src/codec/limits.rs:24` 5 B + `:93` 8 B);
- netem `rate` L2 başına +42 B.

Bench'te güvenlik dikişi 16 B'dir (`seam.rs:105`). Aradaki 6 B bench yüküne dolgu olarak eklendi; bench UDP boyu ürünle aynıdır: `--payload` = kodlayıcı + 13 + 6.

| demet | kodlayıcı B | paket/s | UDP B | UDP B/s | L2 B/s (netem) | başlık payı (UDP) | ρ r1k | ρ r12k |
|---|---|---|---|---|---|---|---|---|
| 60 ms | 9 | 16,67 | 55 | 917 | 1617 | %84 | **1,62 (sığmaz)** | 0,13 |
| 120 ms | 18 | 8,33 | 64 | 533 | 883 | %72 | 0,88 (ARQ'la 1,06: sığmaz) | 0,07 |
| 200 ms | 30 | 5,00 | 76 | 380 | 590 | %60 | 0,59 | 0,05 |

Taslağın K-6 değerleri (408 B/s @ 120 ms vb.) KTP-2'nin 31 B başlığıyla, AMP ve IP/L2 hariç hesaplanmıştır. Bugünkü motor + uygulama: 120 ms'de UDP 533 B/s, netem L2 883 B/s.

### 3.3 Hat seçimi

Katalogdan **r1k** (8 kbit/s, 1000 B/s L2) ve **r12k** (100 kbit/s, 12 500 B/s) seçildi. r25 (25 B/s) hiçbir demette sığmaz; en küçük ses hızı 590 B/s'dir. r25'te canlı PTT "ölçülemedi: fiziksel olarak sığmaz".

r1k'da yalnız 200 ms demet sığar (ρ = 0,59; ARQ'la 0,71). FEC'li kollarda r kapasiteye göre düşürülür: §3.5, ρ ≤ 0,85, en az 1.

### 3.4 Gecikme bütçesi (K-6) ve fiziksel olabilirlik

İki son tarih tanımı ayrı tutulur:
- **Tanım-1 (ağızdan kulağa):** T + (alıcı varışı − sunum) ≤ D, D ∈ {400, 800} ms.
- **Tanım-2 (tek yön + bütçe):** aynı büyüklük ≤ OWD_nom + b, b ∈ {400, 800} ms; OWD_nom = 300 / 1000 ms.

Bileşenler (`reports/ktp2/f0/c/model/butce.csv`):

| bileşen | değer | kaynak |
|---|---|---|
| demet | T | yapılandırma |
| blok oluşturma | (K − 1 − i) × T; en kötü (K − 1) × T | `runtime.rs:2500` |
| serileştirme | L2 boy / hız: ses 7,8–9,4 ms (r12k), 97–118 ms (r1k); E = 64 onarım 10,7 / 134 ms; E = 1024 onarım 87,5 / 1094 ms | aritmetik |
| ağ | OWD 300 / 1000 ms | netem `delay` |
| titreşim | σ 60 / 200 ms | netem `normal` |

Kayıpsız doğrudan varışın son tarihe yetişme tavanı. Analitik, Φ((D − T − ser − OWD)/σ); netem FIFO etkisi hariç, gerçek değer daha düşük olur:

| demet | gecikme | T1 D = 400 | T1 D = 800 | T2 b = 400 | T2 b = 800 |
|---|---|---|---|---|---|
| 60 | d300 | 0,70 | 1,00 | 1,00 | 1,00 |
| 120 | d300 | 0,32 | 1,00 | 1,00 | 1,00 |
| 200 | d300 | 0,03 | 1,00 | 1,00 | 1,00 |
| 60 | d1000 | 0,0004 | **0,09** | 0,95 | 1,00 |
| 120 | d1000 | 0,0001 | **0,05** | 0,91 | 1,00 |
| 200 | d1000 | 0,0000 | **0,02** | 0,83 | 1,00 |

- **RTT 2 s (d1000), Tanım-1, D = 400/800: fiziksel olarak imkânsız.** Tek yön ortalaması 1000 ms, D'den büyük. Kayıpsız ve kusursuz bir taşımada bile yetişen pay ≤ %9; bu pay yalnız jitter kuyruğundan gelir. Bu hücreler "ölçülemedi: OWD > D" olarak raporlanır. d1000 hücreleri **Tanım-2** ile değerlendirilir.
- **RTT 600 ms, D = 400:** tavan T = 60'ta 0,70, T = 120'de 0,32, T = 200'de 0,03. %95 hedefi fiziksel olarak tutmaz. Karar hücresi **D = 800**'dür.
- **ARQ kurtarması son tarihe yetişmez [çıkarım].** Tespit ≥ max(T + RTT + 20 ms, 9/8 RTT) (`loss.rs:14-21`). Yeniden gönderilen çerçevenin varışı ≥ T + 920 ms (d300). Bu, D = 800 − T bütçesini her demette aşar. ARQ-yalnız kolun T1-800 oranı ≈ 1 − p beklenir.

### 3.5 Sabit RS'te r seçimi

| kayıp | rs2 (K=2, R≤2) | rs4 (K=4, R≤4) | taslak §3.8 |
|---|---|---|---|
| %10 | (2,1) | (4,1) | (4,1) |
| %20 | (2,2) | (4,2) | (3,2) — katalogda K = 3 yok |
| %30 | (2,2) | (4,4) | (4,4) |
| %50 (stres) | (2,2) | (4,4) | (4,9) — R_max 4 |

r1k'da kapasite kuralı (ρ ≤ 0,85, ARQ hariç, en az 1) her iki kolu da r = 1'e indirir. `rs2` (2,1) yine de ρ = 0,93'tür; ARQ'la sığmaz. Bu beyan edilir ve ölçülür.

---

## 4. Karar ölçüsü ve bileşenlerin ölçümü

Birim = koşu. Koşu başına ölçüler (`scripts/ktp2/f0/c/analyze_c.py`):

**Son tarihe yetişen çerçeve oranı.**
- Çerçeve f: `m2e_f = T + (rx_f − offer_f)`.
- `offer_f = t0_unix_ns + sent_ns − events_epoch_offset_ns` (`out.jsonl`).
- `rx_f` = sunucu `--rx-log` damgası (CLOCK_REALTIME; iki süreç aynı makinede, ad alanları saati ayırmaz).
- Tanım-1 ve Tanım-2 için ayrı sütunlar: `ot_T1_400`, `ot_T1_800`, `ot_T2_400`, `ot_T2_800`.
- Tekrar kolunda `rx_f` = iki kopyanın en erkeni; sunum = ilk kopya.

**Türetilmiş ölçüler.**
- `ot_sirali_*`: oturum geneli sıralı teslim, `rel_u = max(rx_v, v ≤ u)`. client-core `Reorder` karşılığıdır.
- `ack_T1_800`: gönderen teyidiyle aynı ölçü; karşılaştırma için.

**Tel.**
- `udp_tx_bytes` ve bileşenleri: data / retx / dup / fec / ctrl, t0 sonrası.
- L2 = `budget.injected_bytes + 14 × injected_pkts`.
- Çerçeve başına UDP ve L2 baytı, `zamaninda_per_kB`, `rho_konusma`.

**Gecikme bileşenleri** (hücre × kol):

| bileşen | nasıl elde edilir |
|---|---|
| demet | T (yapılandırma) |
| blok oluşturma | (K − 1)T/2 ortalama, (K − 1)T en kötü (yapısal); ölçülen karşılığı konuma göre T1-800 (`blok_konum_T1_800`) |
| serileştirme | L2 / hız (hesap) |
| ağ | OWD_nom |
| titreşim + kuyruk | doğrudan sınıf çerçevelerde (varış < OWD + 4σ + ser) `lat − ser − OWD`; p50/p95 [ölçüldü] |
| parite gönderim beklemesi | Little: ∫`pending_parity` dt / `fec_repairs_tx` (`ts.csv`, 50 ms örnek) [ölçüldü] |

**Ürün sayaçları** (`c.err`/`s.err`):
- `fec_repairs_tx`;
- latch `(enq_r, emit_r)`;
- arbiter öneri sayısı;
- §8.5 `clamp` / `would_zero`;
- `preconv_parity_blocked`;
- parite jeton reddi;
- `btlbw`, `srtt`;
- sunucu `fec_recovered_units`, `fec_repairs_useful` / `wasted_*`, `fec_blocks_decoded`.

**Geçerlilik.** rc ≠ 0, `msg_count` uyuşmazlığı ya da sunucu KP-SERVER satırı yoksa koşu geçersizdir. Geçersiz koşu sayılır ve raporlanır, silinmez. Alıcı damgası yoksa ölçü `vekil` etiketi alır (ACK anı − OWD_nom) ve hüküm üretmez.

---

## 5. Ön-kayıt [çıkarım]: ölçümden önce kaydedilmiş beklentiler

Üreten: `scripts/ktp2/f0/c/model_c.py` (tohum 20261029, 120 MC koşu). Tam çıktı: `reports/ktp2/f0/c/model/{beklenen.csv, MODEL-C.md, geometri.csv, butce.csv}`.

### 5.1 Model varsayımları

| | varsayım |
|---|---|
| M1 | Bağımsız Bernoulli kayıp, iki yön. |
| M2 | netem FIFO: varış_i = max(varış_{i−1}, gönderim_i + d_i) + boy/hız; d ~ N(μ, σ). `rate` varken sıra korunur; büyük bir gecikme örneği sonrakileri bekletir. |
| M3 | Parite üç varyantla modellenir: `ideal` (blok kapanınca hemen), `pacer1.25`, `pacer1.00` (B-C4 kovası, hız 1,25 × A / 1,0 × A). Tabloda `pacer1.25` değeri, parantezde üç varyantın aralığı. |
| M4 | ARQ tek yeniden gönderim, tespit max(aralık + RTT + 20 ms, 9/8 RTT). |
| M5 | Kurtarma K'ıncı sembol varışında. |
| M6 | Kurtarılan da yeniden gönderilir (karamsar bant). |
| M7 | Isınma sonrası durağan oturum. |

### 5.2 Birincil hücreler: beklenen ortalama son-tarih oranı

`uyar` beklentisi = `arq` (parite 0, §5.3). `uyar_ates` = uyarlamalı kol ateşleseydi (K8, r2, E1024; karşı-olgu, koşulmaz).

**r12k · d300 · %20 · 120 ms** (öncelik 1, karar hücresi):

| kol | K,r | ρ (ARQ dahil) | binom oynatılabilirlik | T1-400 | **T1-800** | T2-400 | T2-800 |
|---|---|---|---|---|---|---|---|
| arq / uyar | — | 0,09 | 0,800 | 0,24 | **0,80** | 0,80 | 0,80 |
| tekrar | — | 0,17 | 0,960 | 0,16 | **0,96** | 0,96 | 0,97 |
| rs2 | 2,2 | 0,17 | 0,979 | 0,11 (0,11–0,23) | **0,98 (0,97–0,98)** | 0,97 (0,95–0,98) | 0,98 |
| rs4 | 4,2 | 0,13 | 0,948 | 0,15 (0,11–0,23) | **0,88 (0,86–0,92)** | 0,85 (0,84–0,89) | 0,96 (0,95–0,96) |
| uyar_ates | 8,2 | 0,27 | 0,887 | 0,23 | 0,80 (0,80–0,83) | 0,80 | 0,81 (0,81–0,89) |

Diğer birincil hücreler, T1-800 için (d1000'de T2-800; `pacer1.25` değeri ve aralık):

| hücre | arq / uyar | tekrar | rs2 | rs4 |
|---|---|---|---|---|
| r12k d300 %10 120 | 0,90 | 0,99 | 0,98 (0,98–0,98) | 0,95 (0,90–0,95) |
| r12k d300 %30 120 | 0,70 | 0,91 | 0,93 (0,93–0,94) | 0,84 (0,81–0,91) |
| r12k d300 %20 60 | 0,80 | 0,96 | 0,98 | 0,94 (0,94–0,95) |
| r12k d300 %20 200 | 0,80 | 0,96 | 0,92 (0,90–0,97) | 0,83 (0,83–0,87) |
| r12k d1000 %20 120 (T2-800) | 0,79 | 0,96 | 0,98 | 0,93 (0,91–0,94) |
| r1k d300 %20 200 | 0,80 | 0,07 (sığmaz) | 0,74 (0,68–0,78) | 0,77 (0,67–0,80) |
| r12k d1000 %10 120 (T2-800) | 0,90 | 0,99 | 0,98 | 0,96 (0,90–0,96) |
| r12k d1000 %30 120 (T2-800) | 0,70 | 0,91 | 0,94 | 0,91 (0,89–0,94) |
| r1k d300 %10 200 | 0,89 | 0,06 (sığmaz) | 0,82 (0,74–0,86) | 0,87 (0,77–0,90) |
| r1k d300 %30 200 | 0,69 | 0,10 (sığmaz) | 0,68 (0,61–0,71) | 0,67 (0,58–0,70) |
| r12k d300 %50 120 (stres) | 0,50 | 0,75 | 0,75 | 0,62 (0,57–0,70) |

Okuma [çıkarım]:
- r12k'da K = 2 sabit RS, %10–30'da 800 ms son tarihte ARQ'nun 1 − p tavanını 8–23 puan aşar. Bunu benzer bant bütçesiyle tekrardan +0–2 puan önde yapar.
- K = 4 blok oluşturma süresi yüzünden geride kalır: T = 120'de 3T = 360 ms. Yalnız T = 60'ta veya Tanım-2'de yetişir.
- r1k'da tekrar ve ARQ'lu `rs2` hattı aşar. FEC kazancı kapasiteyle sınırlıdır; ARQ'nun geç çerçeve yeniden gönderimi de bant yer (B-C6).
- Ürünün uyarlamalı kolu ateşleseydi bile (`uyar_ates`) K = 8 yüzünden kazancı ≤ 3 puan olurdu. r1k'da E = 1024 onarımı hattı boğardı (ρ 2,08).

### 5.3 Sayaç düzeyi beklentiler

| kol | beklenen |
|---|---|
| `uyar` (taban/aday) | Bütün PTT hücrelerinde `fec_repairs_tx = 0` ve `tx.fec = 0`, koşuların ≥ %95'inde. %≥ 20 kayıpta `clamp > 0` ve arbiter öneri > 0. Latch `enq_r = 0`. T1-800 farkı (uyar − arq) ±0,02 içinde. |
| `uyar64`, `pol` (teşhis) | `uyar64` aynı: K = 4'te r ≥ 1 için cap ≥ 125 ‰, btlbw ≥ 15,6 kB/s gerekir. `pol` %20'de FecAssist önerir ama kırpma onu da 0'a indirir. %30'da bölge dışı (`UnsupportedRegion`) → ArqOnly. Üçünde de parite 0. |
| `rs2` / `rs4` | Parite > 0 koşuların %100'ünde. `fec_repairs_tx ≈ (çerçeve/K) × r` (± son bloklar). Sunucu `fec_recovered_units ≈ çerçeve × p × P(blok çözülür \| kayıp)`. |
| `rs*` konum etkisi | `rs4` konuma göre T1-800 oranı blok başında en düşük, sonda en yüksek: (3 − i) × T bekleme. `rs2`'de fark küçük. |
| parite bekleme (Little) | Medyanı T'den küçük beklenir. ≥ 2T ise B-C4 açlığı ölçülmüş olur. |
| taban ↔ aday | FEC kollarında son-tarih oranı farkı ±0,02. Aday parite beklemesi ≥ taban (B-C8). |

### 5.4 Ön-kaydı çürütecek gözlemler

- `uyar` kolunda PTT hücresinde düzenli parite (> %5 koşu) → B-C1 yanlış ya da btlbw beklenenden çok yüksek. `btlbw` sayacıyla kontrol edilir.
- `rs2` T1-800 < arq + 0,05 (r12k, d300, %20) → pacer/kova açlığı modelden ağır (B-C4) ya da netem jitter FIFO etkisi beklenenden büyük. Ayrıştırma: parite bekleme + titreşim p95.
- arq T1-800, 1 − p'den 0,05+ yukarıda → ARQ beklenenden hızlı kurtarıyor (M4 yanlış).

---

## 6. Önceki ölçümler (korunur) ve ürün yolundan farkları

### 6.1 Nerede ve nasıl ölçüldüler

`git log -- reports/`. Depodaki dosyaların origin'deki ortak atası `ca65e6cb` (#45). İlk commit'ler `7e3505f6` (R2A), `a0d17f89` (R2A düzeltmesi) ve `bd3a5ea8` (FS-1) yerel geçmişte görünür. Origin dallarında değiller; atıf `ca65e6cb` + dosya yoluyladır.

| | R2A (S3 `iid20`) | FS-1 v1.3.2 |
|---|---|---|
| rapor | `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md` + `…-R2A-ENDPOINT-AND-BUDGET-CORRIGENDUM.md` + `…-R2A-CELL-RESULTS.csv` | `reports/fs-1/analysis/v1.3.2/FS-1-ORACLE-VERDICT-v1.3.2.md`, `FS-1-CELL-RESULTS-v1.3.2.csv` |
| düzenek | **süreç-içi belirlenimli model**: gerçek `Sender` ARQ + `BottleneckChannel` + gerçek `RsSimdCodec` (`kama-integration/tests/common/fs_spine.rs`). netem yok, UDP yok, ürün runtime'ı yok | aynı spine; hücre başına N = 5 (tarama) |
| iş yükü | tek mesaj = 48 kaynak sembol × 101 B (E = 64 + 37) | aynı |
| FEC | F0: K = 8, r = 2/blok, 12 onarım = 1212 B; **parite spine tarafından doğrudan kanala verilir**, 48 kaynağın tamamı sunulduktan sonra (`fs_spine.rs:716-760`) | F0/F3 |
| uç nokta | **gözlem ufku içinde tam mesaj tamamlanması** (699/2499/9249 ms). Son tarih değil; son tarih isabeti bütün kollarda ≈ 0 | aynı; `deadline_hits` = 0 (48/48 satır) |
| sonuç | %20 IID'de F0 59/62/66 /100 (RTT 50/250/1000) vs A0 (dondurulmuş ARQ) 12–14/100; tel F0 5082–5592 B vs A0-TAIL 6063–6339 B | `iid20` F0 güvenilirlik 0,2–0,8 (N = 5); O2_FEC_SPECIFIC 12/12 (kodlama > kör çoğaltma, eşit yapılandırılmış bütçe) |

### 6.2 Ürün yolundan farkları [kod]

Bu farklar yüzünden R2A/FS-1 sayıları ürün PTT'sine aktarılamaz. F0 hücreleri bu farkı ölçer.

1. **Parite zamanlayıcıdan geçmez.** Spine pariteyi doğrudan kanala koyar. Ürünü engelleyen hiçbir şey o yolda yok: kontrolcü, §8.5 kırpması (B-C1), yapı kapısı (B-C5), pacer jeton kırpması (B-C4).
2. **Sembol 101 B, ürün profilinde E = 1024.** Spine'ın 64 + 37 B'ı ürün yolunda yalnız E = 64 profillerinde geçerlidir. Sevk profili 0x05'te onarım 1052–1058 B'dir (B-C3).
3. **Uç nokta tek büyük mesajın tamamlanmasıdır; PTT'nin uç noktası çerçeve başına son tarihtir.** R2A'nın 59–66/100'ü ufuk tamamlamasıdır. Son tarih isabeti ≈ 0'dır (düzeltme `a0d17f89` §1).
4. **ARQ kolu dondurulmuş ve kuyruk-körüdür.** Ürünün ARQ'su kuyruk kurtarmalıdır (`validated_v1`). R2A C2: kuyruk-yetenekli ARQ'ya karşı FEC'in terminal-kuyruk üstünlüğü büyük ölçüde kapanır.
5. **netem yok.** Jitter-FIFO etkisi, serileştirme ve hat doluluğu modelde yok.

Korunan bulgu: rastgele kayıpta kodlama, aynı yapılandırılmış bütçede kör çoğaltmayı yener (FS-1 O2 12/12; R2A C3 868'e 0). F0'daki `rs*` ↔ `tekrar` kıyası bunu ürün yolunda, son tarih ölçüsüyle sınar. R2B (patlamalı kayıp) ölçülmedi; F0 da ölçmüyor.

### 6.3 Arşiv kanıtı: 05b netem-7

[ölçüldü — arşiv, betimsel; PTT dışı iş yükü]

Kaynak: `reports/ktp2/f0/05b-netem/oturum-{1-a8c4e8c6,2-49864510}/run-*.tar.xz` içindeki `c.err`. İş yükü: 100 × 256 B @ 50 ms, ürün profili (DELTA yok), N = 20/hücre/kol. Okuyan: `analyze_c.parse_client_err`.

| profil | kol (oturum 2 / 1) | parite > 0 koşu | onarım medyanı (maks) | §8.5 kırpma medyanı | parite jeton reddi medyanı | btlbw medyanı B/s |
|---|---|---|---|---|---|---|
| p1_loss50 (100 kbit, %50) | taban | 12 / 11 | 5,5 (10) / 3 (11) | 7900 / 9062 | 2485 / 3761 | 10 879 / 8568 |
| p1_loss50 | aday | 11 / 12 | 2 (11) / 3 (9) | 6530 / 6487 | 3684 / 3060 | 9717 / 10 862 |
| p5_revpath (%5 / %80) | taban | 10 / 9 | 1 (10) / 0 (11) | 7471 / 6069 | 2 / 0 | 8838 / 10 882 |
| p5_revpath | aday | 16 / 14 | 4,5 (11) / 4,5 (10) | 4035 / 5076 | 1277 / 1748 | 10 958 / 11 008 |
| p7_bbr (50 kbit, %2) | taban / aday | 0 | 0 | 2624–3635 | 0 | ≈ 5600 |
| p4_jitter (kayıp yok) | taban / aday | 0 | 0 | 0 | 0 | 6189–7609 |

Okuma [betimsel]:
- Ürün yolunda uyarlamalı parite 12–13 bloklu bir akışta medyan 0–5,5 onarım üretiyor (r ≤ 1).
- §8.5 kırpması binlerce kararda devrede.
- Parite tele çıkmadan önce binlerce kez jeton reddi alıyor.
- btlbw ≈ 8–11 kB/s ile cap ≈ 64–87 ‰ → K = 8'de r = round(0,5–0,7) = 0–1. B-C1 aritmetiğiyle tutarlı.
- PTT hızlarında (btlbw < 1 kB/s) r = 0 beklenir.

---

## 7. Koşu planı ve istekler

### 7.1 İSTEKLER (Ajan D'ye)

**İSTEK-C1 (zorunlu; tam setin ön koşulu): alıcı varış kaydı, yalnız bench.** — *Yapıldı (Ajan D; taban `2cd9f68d`, aday `bf18dc35`), duman-2'de doğrulandı (§10.1).*
- **Dosya/işlev:**
  - `crates/bench/src/main.rs` `Cmd::Serve`: yeni bayrak.
  - `crates/bench/src/transport.rs` `ServerCfg`: yeni alan `rx_log: Option<PathBuf>`.
  - `crates/bench/src/kama_product.rs` `server_loop`.
- **Bayrak:** `serve --rx-log <PATH>`. Varsayılan yok; verilmezse davranış ve çıktı birebir aynı.
- **Davranış:**
  - (1) `d = srv.next_delivery()` dalında `Some(d)` için şunu sınırlı bir `Vec`'e ekle: `(d.unit_id, unix_ns, d.payload.len())`. `unix_ns = SystemTime::now() − UNIX_EPOCH`; istemcinin `t0_unix_ns`'i ile aynı saat. Tavan 1 048 576 kayıt; aşanlar `dropped` sayacına.
  - (2) Döngü çıkışında (SIGTERM/SIGINT/None), mevcut `KP-SERVER` satırından sonra JSONL yaz, `write_jsonl` gibi tmp + rename ile atomik:
    - 1. satır `{"rx_log_schema":"ktp2-c-rx-v1","dropped":N}`;
    - sonra teslim başına `{"u":<unit_id>,"t":<unix_ns>,"n":<len>}`.
  - (3) Yalnız `kama-product` kolunda; diğer taşıyıcılar bayrağı yok sayar.
  - (4) İki kola (`taban`, `aday`) aynı commit.
- **Ürün kodu değişmez.** `Delivery.unit_id` zaten var (`runtime.rs:1190-1197`). Kayıt döngüye bir `Vec::push` ekler; gecikme etkisi ns mertebesindedir ve ölçülmedi.
- **Gerekçe:** B-C7. `delivered_ns` gönderen ACK anıdır. PTT son tarihi alıcı varışıyla ölçülür.

**İSTEK-C2 (isteğe bağlı, düşük öncelik): gerçek tekrar kolu.** — *Yapıldı (`client --dup-copies N`); tam setin tekrar kolu bunu kullanır (§10.3).*
- `client --dup-copies N` (varsayılan 1).
- Her sunumda aynı yükü N kez art arda `offer` et.
- `sent_ns` çerçeve başına bir kez; `delivered_ns` = kopyalardan ilk teyit.
- Birim kimliği `W + i·N + c`; analiz bunu `cmd.txt`'ten okur.
- Gelmezse vekil (§2) kullanılır ve beyan edilir.

### 7.2 Hücreler ve süre

Hücreler üretilmiştir: `scripts/ktp2/f0/c/gen_cells.py` → `scripts/ktp2/f0/cells/c/*.cell`, dizin `cells/c/INDEX.md`.

Her `c_*` hücresi 10 kol: {arq, uyar, rs2, rs4, tekrar} × {taban, aday}.
- konuşma 20 s (çerçeve 336/168/104), ısınma 16;
- `--max-deadline-s` 6 (d300) / 10 (d1000);
- `BUDGET_S = 120`.

Kol-koşusu tahmini ≈ 30 s (d300), 38 s (d1000), 45 s (%50): yerleşme 1 + ısınma ≈ 3–8 + konuşma 20 + ACK kuyruğu ≈ 3–8 + kapanış 1.

| öncelik | hücreler | N | koşu-saati |
|---|---|---|---|
| duman | `cells/c/duman/*_duman.cell`: r12k_d300_l20_b120 (+ uyar64/pol), r12k_d1000_l20_b120, r1k_d300_l20_b200, r12k_d300_l50_b120 | 1 | **0,41** (42 kol-koşusu) |
| 1 | r12k_d300 {l20_b120, l10_b120, l30_b120, l20_b60, l20_b200} · r12k_d1000_l20_b120 · r1k_d300_l20_b200 | 30 | **18,2** |
| 2 | r12k_d1000 {l10, l30}_b120 · r1k_d300 {l10, l30}_b200 · cx_r12k_d300 {l20, l30}_b120 (teşhis) | 30 / 10 | **11,8** |
| 3 | r12k_d300_l50_b120 (stres) · r1k_d300_l20_b120 (kapasite sınırı) · r12k_d300 {l10, l30}_{b60, b200} · r12k_d1000_l20_{b60, b200} | 10 (betimsel) | **7,5** |
| toplam | | | **37,5 + 0,41 duman** |

Duman ölçütleri (`analyze_c.py --duman`):
- her kol rc = 0;
- `rx.jsonl` satır ≈ teslim edilen birim;
- `uyar` parite sayacı okunuyor;
- `rs*` parite > 0;
- `ts.csv` var;
- KP-SERVER satırı var.

İSTEK-C1 yoksa duman `python3 -I scripts/ktp2/f0/c/gen_cells.py --no-rxlog` ile yeniden üretilen hücrelerle koşabilir. Sonuç o zaman `vekil` etiketlidir.

---

## 8. Sonuçlar

Koşu sonrası doldurulacak: `analyze_c.py --out reports/ktp2/f0/c/sonuc <kök>`. Her hücre × kol için:
- medyan, IQR, min–maks;
- arq'a karşı fark ve %95 bootstrap aralığı;
- ön-kayıttan sapmanın dekompozisyonu (§5.4).

Bu turda yok.

## 9. Duman-1 (vekil kip): enstrüman mı, davranış mı

**Koşu.**
- Kök: `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-c-vekil/out`.
- İkililer: taban `wt/ktp2-f0/target/release/bench` (sha256 `14b5f7d3…`), aday `wt/ktp2-05b/target/release/bench` (`fd3089b1…`).
- 4 duman hücresi, 42 kol-koşusu, hepsi rc = 0.
- Hücrelerden yalnız ` --rx-log @D@/rx.jsonl` çıkarıldı (İSTEK-C1 yok). Ölçü vekil: ACK anı − OWD_nom.

**Bütünlük.**
- 42/42 geçerli; KP-CLIENT, KP-SERVER ve `ts.csv` okunuyor.
- Isınma 40/42 koşuda 16/16 teyitli, 2 koşuda 15/16 (residue 1).
- İstemci süresi medyanı: d300 28,9 s, d1000 35,3 s, l50 39,1 s, r1k 30,8 s. Tahmin (30/38/45/30 s) tutuyor.

**Sayaçlar ön-kayıtla tutarlı [ölçüldü — duman, betimsel].**
- `uyar`, `uyar64`, `pol` kollarında parite 0 (10/10 koşu).
- Arbiter öneri üretiyor: yapı kapısı 0–17 068, spekülatif (L2) 10 747–41 863, politika 13 523.
- §8.5 kırpması 19 134–42 520; latch `enq_r = 0`. B-C1 mekanizması bire bir görünüyor.
- `rs2`/`rs4` paritesi beklenen sayıda:
  - `rs2` (2,2): 184 = (168 + 16)/2 × 2;
  - `rs4` (4,2): 92;
  - r1k'da r = 1.
- Sunucu `fec_recovered_units` > 0.

**Orkestratörün işaret ettiği değerler.**

| gözlem | sınıf | kanıt |
|---|---|---|
| `tekrar_*` ve `pol_taban` r12k/l20'de 0,060 | **davranış** (B-C9 gönderen kilidi) | birikim 5904–8837 ms; btlbw/sunulan 0,85–1,11; jeton aç payı 0,98. 0,060 = 10/168: birikim başlamadan önceki ilk çerçeveler. Payda doğru (168 çerçeve; tekrarda 336 birim → 168 çerçeve). |
| `uyar_aday` 0,101 ↔ `uyar_taban` 0,702 | **davranış**; taban/aday farkı değil, koşu başına iki kararlı durum | uyar_aday kilitli (6150 ms, 1,00), uyar_taban kilitsiz (−49 ms, 1,44). arq'ta tersi: taban kilitli (5533 ms), aday kilitsiz (156 ms). Parite ikisinde de 0. |
| l50'de çoğu kol ≈ 0 | **davranış** | 10 koşunun 8'i kilitli; bunların 5'inde ilk iletim payı 0,34–0,73, yani çerçevelerin bir kısmı pencerede hiç gönderilmedi. Kilitsiz iki koşu: rs4_taban 0,34, uyar_aday 0,19. |
| d1000'de T1-800 = 0 | **tanım gereği** (§3.4: OWD > D) | aynı koşularda T2-800 0,56–0,88; duman tablosu artık T2'yi de basıyor. |
| r1k'da kilitsiz koşular da düşük (T1-800 0,01–0,19) | **enstrüman** (vekil) | r1k'da srtt 834–1023 ms (nominal 600). Dönüş yolunda ACK serileştirmesi + kuyruk vekile ekleniyor. r1k hücreleri ancak `--rx-log` ile yorumlanabilir. |
| kilitsiz r12k/d300 koşuları ön-kayıttan 0,1–0,15 düşük | **enstrüman** (vekil) | ACK gecikmesi + dönüş yolu jitteri (σ 60) + %20 dönüş kaybında bir sonraki ACK'e kayma. Alıcı damgası olmadan ön-kayıtla kıyaslanamaz. |

**Ön-kayıt sapması (beyan).** M3 ("veri sunum anında gönderilir") kilitli koşularda tutmuyor. Model gönderen kuyruğunu içermiyor. Ön-kayıt tabloları değiştirilmedi. Tam sette kilitli/kilitsiz koşular ayrı sayılacak (`kilitli`, `kilit_n`). Son-tarih oranı her iki alt kümeyle birlikte ve ayrı raporlanacak.

**Araç değişikliği** (`analyze_c.py`; hücre ve kol değişmedi). Koşu başına yeni alanlar:
- `sunulan_udp_Bps`, `btlbw_sunulan`;
- `ilk_iletim_payi`;
- `birikim_ms` / `birikim_egim_ms_s` (pencere-minimum gecikme artışı; alıcı damgası varsa ondan);
- `jeton_ac_payi` (ts.csv);
- `kilitli` (birikim ≥ 1 s ya da ilk iletim payı < 0,95).

Duman tablosu T2-800, clamp ve bu alanları basıyor. Özet tablosuna "birikim ms", "btlbw/sunulan", "kilitli koşu" sütunları eklendi.

**Karar.** Duman bütünlük açısından temiz, ölçü açısından değil. Vekil kip son tarih ölçüsü için kullanılamaz: r1k'da ve kilitsiz koşularda dönüş yolu payı var. Tam set İSTEK-C1 derlendikten sonra koşulmalı. Önce `--rx-log`'lu bir duman daha (aynı 4 hücre, 0,41 koşu-saati) ile alıcı damgası doğrulanmalı.

## 10. Duman-2 (alıcı damgalı): denetim ve bulgular

**Koşu.**
- Kök: `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-2/out` (`c_*` hücreleri).
- İkililer: taban `ktp2-f0/bin/taban-2cd9f68d/bench` (`3311d8e0…`), aday `ktp2-f0/bin/aday-bf18dc35/bench` (`c893e840…`). İkisinde de İSTEK-C1/C2 var.
- 4 hücre, 42 kol-koşusu, hepsi rc = 0.
- j = 12 eşzamanlı. Her koşu temiz netem kuyruğuyla başladı (orkestratör beyanı).
- Hücreler duman-1'le aynı; tekrar kolu hâlâ C2 öncesi vekil.
- Çıktılar: `reports/ktp2/f0/c/duman-2/{DUMAN.md, kosular.csv, ozet.csv, OZET-C.md}`.

### 10.1 Enstrüman denetimi: geçti

- **Alıcı kaydı = teslim.**
  - 42/42 koşuda `rx.jsonl` satır sayısı = sunucu `ServerStats.delivered`.
  - `dropped = 0`.
  - Tam teslimde 184 satır (16 ısınma + 168), vekil tekrarda 352.
- **Saat hizası.** Negatif gecikme 0. İstemci `t0_unix_ns` + `sent_ns` ile sunucu CLOCK_REALTIME aynı tabanda.
- **Alıcı gecikmesi ≈ OWD + serileştirme** (kilitsiz koşular, 14 çerçevelik pencere minimumları):

  | hücre | ölçülen | beklenen |
  |---|---|---|
  | r12k/d300 | 194–301 ms | μ − 1,7σ + ser = 300 − 102 + 8,5 ≈ 207 |
  | r12k/d1000 | 770–1100 ms | ≈ 669 + HOL |
  | r1k/d300 | 309–480 ms | 300 − 102 + 118 ≈ 316 |

  Doğrudan sınıf çerçevelerde titreşim + kuyruk artığı p50 10–72 ms, p95 124–224 ms (d300). σ = 60 ms ve netem FIFO etkisiyle tutarlı.
- **Gönderen teyidi alıcı varışından sonra gelir.** Vekil ile alıcı ölçüsü arasındaki fark, duman-1'de "enstrüman" dediğim payı doğruluyor (§9). r1k ve kilitsiz r12k koşularının düşük vekil değerleri alıcı ölçüsünde düzeliyor, örnek: d1000 T2-800 vekil 0,56–0,88 → alıcı 0,78–0,99 (kilitsiz koşular).

### 10.2 Davranış bulguları [ölçüldü — duman, N = 1/kol, betimsel]

**B-C4 ölçüldü: parite pacer'da aç kalıyor.**

| hücre | kol | parite tx / plan | bekleme (Little) | jeton reddi | sunucu kurtarma / çözülen blok |
|---|---|---|---|---|---|
| r12k d300 %20 | rs2_taban | 174 / 184 | 1,2 s | 16 061 | 22 / 76 |
| r12k d300 %20 | rs2_aday | 146 / 184 | 4,2 s | 21 045 | 8 / 47 |
| r12k d300 %20 | rs4_taban | 58 / 92 | 7,9 s | 19 642 | 9 / 9 |
| r12k d300 %20 | rs4_aday | 45 / 92 | 15,1 s | 20 665 | 8 / 7 |
| r12k d1000 %20 | rs2 / rs4 (4 koşu) | 184 / 184 · 92 / 92 | 0,11–0,46 s | 8181–11 644 | 27–42 |
| r1k d300 %20 | rs2 / rs4 (4 koşu) | 16–60 | 1,5–17,8 s | 16 386–22 081 | 1–10 |

- ~~`rs2_taban` d300'de çözülen 76 bloğun yalnız 22'sinde kurtarılan birim alıcıya ilk kez ulaştı. Kalanında birim ARQ ile çoktan gelmişti; parite geç kaldı.~~ **Düzeltme (TS.3):** bu çıkarım yanlıştı. r ≥ K kolunda tamamlanmış blok geç onarımla yeniden çözülür (`fec_ingest`); oran gecikme ölçüsü değildir. Parite gecikmesinin kanıtı kuyruk beklemesi ve FEC'le ilk ulaşan / kaybolan payıdır.
- Geç çerçevelerin alıcı gecikmesi çoğunlukla 1,1–1,5 s (`rs2_taban`: 21 geç çerçevenin 15'i 1,0–1,5 s'de). Bu, ARQ zamanlamasına (T + RTT + OWD ≈ 1,02 s) uyuyor.
- d300 ↔ d1000 farkı için [çıkarım]: d300'de ARQ tespiti ≈ 0,7 s'de olur. Yeniden gönderim pariteden önce jeton alır (`sender.rs:4292-4311` sırası; `K_ARQ_SHARE`). d1000'de tespit ≈ 2 s sürdüğü için parite önce gider.
- Ön-kayıt §5.3 "≥ 2T ise B-C4 açlığı ölçülmüş olur" ölçütü d300 ve r1k'da sağlandı. d1000'de sağlanmadı.

**Son-tarih oranı, kilitsiz koşular, ön-kayda karşı** [betimsel, N = 1]:

| hücre / tanım | kol | ölçülen | ön-kayıt |
|---|---|---|---|
| d1000 T2-800 | arq | 0,81 | 0,79 |
| | rs2 | 0,97 / 0,99 | 0,98 |
| | rs4 | 0,90 / 0,92 | 0,93 (0,91–0,94) |
| d300 T1-800 | rs2 | 0,81 / 0,88 | 0,98 (0,97–0,98) |
| | rs4_taban | 0,79 | 0,88 (0,86–0,92) |
| | tekrar_aday (vekil) | 0,93 | 0,96 |
| | uyar64 | 0,76 | 0,80 |

d300'deki rs açığı, modelin `pacer1.00` varyantından da büyük: model parite beklemesini ≤ 2T varsayıyor, ölçülen 1–15 s. Sapmanın kaynağı B-C4'tür.

**B-C9 alıcıda.**
- Kilitli koşularda pencere-minimum alıcı gecikmesi tırmanıyor: tekrar_taban d300 249 → 13 438 ms; uyar_taban d1000 841 → 8171 ms; arq_aday r1k 416 → 4713 ms.
- Ara durumlar da var. arq_taban ve uyar_taban d300'de birikim tepesi 1053–1339 ms; oran 0,17–0,29.

### 10.3 Araç ve hücre değişiklikleri

Hepsi bu turda; ön-kayıt satırları değişmedi.

**`analyze_c.py`:**
- `--dup-copies N` düzeni: birim = W + f·N + c; sıralı teslim birim sırasıyla.
- `birikim_tepe_ms`: tepe pencere-minimumu − ilk pencere. Birikim konuşma ortasında yükselip sonda sönebiliyor; "son − ilk" bunu kaçırıyordu.
- `kilitli` eşiği tepe ≥ max(500 ms, 4σ) ya da ilk iletim payı < 0,95. **Bu eşik duman-2 verisi görüldükten sonra konuldu.** Betimseldir, karar ölçüsü değildir; tam sette sürekli `birikim_tepe_ms` dağılımı da raporlanır.
- Duman-2'de kilitli koşu sayısı (yeni eşik):

  | kol | kilitli |
  |---|---|
  | arq | 6/8 |
  | uyar | 6/8 |
  | tekrar (vekil) | 6/8 |
  | rs4 | 4/8 |
  | rs2 | 2/8 |
  | pol | 1/1 |
  | uyar64 | 0/1 |

**`gen_cells.py`:** tekrar kolu artık İSTEK-C2 gerçek kolu (istemci `--dup-copies 2`, `--msgs N --submit-interval-ms T`). Vekil `--vekil-tekrar` ile yeniden üretilebilir. Bütün `cells/c/*.cell` yeniden üretildi; diğer kollar bayt bayt aynı.

**`model_c.py`:** `tekrar2` kolu eklendi (iki kopya arka arkaya; ARQ tespiti bir sonraki sunuma göre). Önceki 480 satır bayt bayt aynı; tohum kol adına bağlı.

Beklenen değerler, r12k / 120 ms:

| gecikme | kayıp | T1-800 | T2-800 |
|---|---|---|---|
| d300 | %10 | 0,99 | 0,99 |
| d300 | %20 | 0,96 | 0,96 |
| d300 | %30 | 0,91 | 0,91 |
| d1000 | %10 | — | 0,99 |
| d1000 | %20 | — | 0,96 |
| d1000 | %30 | — | 0,91 |

Model gönderen kilidini içermez.

### 10.4 Karar

Enstrüman temiz. Tam set istenir.

Kararı belirleyecek davranışlar: B-C4 (parite açlığı) ve B-C9 (gönderen birikimi). Tam set ikisini N = 30 ile ölçer:
- kilit insidansı;
- parite beklemesinin dağılımı;
- son-tarih oranı (kilitli ve kilitsiz alt kümeler ayrı).

## 11. Tam set ara denetimi 1: `c_r12k_d300_l20_b120`

[ölçüldü, N = 30/kol, alıcı damgalı]

**Kaynak.**
- Kök: `~/kama-k4g-work/ktp2-f0/runs/tam/c/out/c_r12k_d300_l20_b120`.
- Süre: 2026-10-29 20:19 → 22:44 Z.
- Hücre `spec_sha256 = fc7f1308…`; `b58bf049`'daki hücre dosyasıyla aynı.
- İkililer: taban `2cd9f68d` (`3311d8e0…`), aday `bf18dc35` (`c893e840…`).
- Analiz çıktıları: `reports/ktp2/f0/c/tam-ara-1/{kosular.csv, ozet.csv, OZET-C.md}`.

### 11.1 Geçerlilik: temiz

- 10 kolun hepsi 30/30 rc = 0 ve 30/30 geçerli.
- Her koşuda `out.jsonl` ve `rx.jsonl` var. `rx.jsonl` satırı = sunucu `delivered`: 300/300.
- Isınma artığı 0: 300/300.
- `realized_pre`'de netem `packets = drops = qlen = backlog = 0`: 300/300.
- Gerçekleşen kayıp, koşu başına medyan:
  - ileri yön 0,189–0,213 (min–maks 0,128–0,300);
  - dönüş yönü 0,197–0,209.

  Profil %20 ile tutarlı. Koşu başı yayılım binomdur (≈ 300 paket/koşu).

### 11.2 İlk gözlem

T1-800 = ağızdan kulağa ≤ 800 ms. Tablo: medyan [IQR] (min–maks).

| kol | T1-800 | Δ arq [%95 bootstrap] | kilitli | kilitsiz alt küme T1-800 | parite tx (/plan) | FEC kurtarılan | parite beklemesi (Little) | B/çerçeve L2 |
|---|---|---|---|---|---|---|---|---|
| arq_taban | 0,196 [0,126, 0,443] (0,048–0,875) | — | 23/30 | 0,815 (n = 7) | 0 | 0 | — | 131 |
| arq_aday | 0,217 [0,121, 0,317] (0,042–0,548) | — | 30/30 | — | 0 | 0 | — | 130 |
| uyar_taban | 0,193 [0,112, 0,351] (0,030–0,804) | −0,003 [−0,158, 0,071] | 26/30 | 0,756 (n = 4) | **0 (30/30)** | 0 | — | 132 |
| uyar_aday | 0,146 [0,067, 0,243] (0,036–0,857) | −0,071 [−0,167, 0,048] | 26/30 | 0,830 (n = 4) | **0 (30/30)** | 0 | — | 129 |
| rs2_taban | 0,830 [0,821, 0,869] (0,762–0,988) | +0,634 [0,518, 0,688] | 0/30 | 0,830 | 160 [108–184] / 184 | 13 | 2,5 s | 246 |
| rs2_aday | 0,842 [0,827, 0,871] (0,756–0,929) | +0,625 [0,554, 0,711] | 0/30 | 0,842 | 158 [99–184] / 184 | 13 | 2,8 s | 239 |
| rs4_taban | 0,815 [0,798, 0,832] (0,423–0,893) | +0,619 [0,494, 0,664] | 1/30 | 0,815 | 84 [19–92] / 92 | 12 | 2,2 s | 188 |
| rs4_aday | 0,839 [0,815, 0,860] (0,536–0,929) | +0,622 [0,554, 0,711] | 0/30 | 0,839 | 86 [21–92] / 92 | 13 | 2,5 s | 188 |
| tekrar_taban (C2) | 0,253 [0,042, 0,942] (0,012–0,982) | +0,057 [−0,250, 0,741] | 17/30 | 0,952 (n = 13) | — | — | — | 240 |
| tekrar_aday (C2) | 0,387 [0,048, 0,607] (0,012–0,964) | +0,170 [−0,182, 0,402] | 23/30 | 0,935 (n = 7) | — | — | — | 258 |

**Okuma.** Yalnız bu hücre; diğer hücrelere genelleme yok.

1. **B-C1 tam sette tuttu.**
   - Ürün uyarlamalı FEC'i (`uyar`) 60/60 koşuda parite üretmedi.
   - §8.5 kırpması medyan 28,7–28,9 bin karar.
   - `uyar − arq` farkının aralığı iki ikilide de 0'ı kapsıyor. Bu hücrede uyarlamalı FEC'in ARQ'ya bir katkısı ölçülmedi.
2. **Parite tele çıktı, ama geç.**
   - `rs2`: planlanan 184 paritenin medyan 158–160'ı gitti. `rs4`: 92'nin 84–86'sı gitti.
   - Kuyruk beklemesi medyanı 2,2–2,8 s; blok süresi 120–360 ms. B-C4 N = 30'da da görülüyor.
   - Alıcıda FEC'le ilk kez ulaşan çerçeve medyanı 12–13 (168 çerçevenin ≈ %8'i; kaybolan ≈ 34'ün ≈ 1/3'ü). Kalan kayıplar ARQ ile ve son tarihten sonra geldi.
3. **Kolları ayıran kilit (B-C9), FEC kurtarması değil.**
   - Kilit oranları: arq 23–30/30, uyar 26/30, tekrar 17–23/30, rs 0–1/30. rs kollarında btlbw / sunulan oranı medyanı 1,9–2,5; arq/uyar'da 1,14–1,20.
   - Kilitsiz alt kümelerde oranlar birbirine yakın: arq 0,815 (n = 7), uyar 0,76–0,83 (n = 4), rs 0,82–0,84. Kilitsiz tekrar 0,94–0,95 (n = 7–13) bu üçünden yüksek; rs'nin kurtarma katkısını parite gecikmesi siliyor.
   - Yani rs − arq ≈ +0,62 farkının büyük kısmı, paritenin teslim edilen baytı artırıp pacer kilidini önlemesinden geliyor [çıkarım; kilitsiz alt küme karşılaştırması + sayaçlar]. FEC'in son tarih içindeki kurtarma katkısı ≈ +0,02–0,08.
   - Kilitsiz arq/uyar alt kümeleri n = 4–7'dir; betimseldir.
4. **Ön-kayda karşı.**
   - Kilitsiz arq 0,815 ↔ ön-kayıt 0,80.
   - Kilitsiz tekrar 0,94–0,95 ↔ `tekrar2` 0,96.
   - rs2 0,83–0,84 ↔ 0,97–0,98, rs4 0,82–0,84 ↔ 0,86–0,92. Fark B-C4'ten.
   - Kilitli koşular modelin dışında: M3 sapması, §9.
5. **Sıralı teslim türevi** (client-core `Reorder` karşılığı): kol medyanları 0,02–0,41 (rs 0,31–0,41; diğerleri ≤ 0,05). Ürün uygulama yolunda (PTT `Reliable` + oturum geneli sıra) son-tarih oranının bench'teki sırasız değerden çok daha düşük olacağını gösteriyor [türetilmiş].

**Durdurma gerekmiyor.** Enstrüman ve geçerlilik temiz. Kalan hücreler aynı iki davranışın (B-C4, B-C9) kayıp, RTT ve demet eksenindeki dağılımını ölçecek.

# TAM SONUÇ (2026-10-30)

**Kaynak.** `~/kama-k4g-work/ktp2-f0/runs/tam/c/out/`: 21 hücre, 4160 koşu.
- İkililer: bench taban-2cd9f68d (`3311d8e0…`), aday-bf18dc35 (`c893e840…`).
- Satır numaraları taban `a858f3fc`'ye göredir; adayda (`49864510`) farklıysa "/ aday" ile verilir.
- Bütün sayılar `reports/ktp2/f0/c/tam/` altındaki betik çıktısındandır (T.n = `tablolar.md`).

**Okuma kuralları.**
- ARQ her kolda açıktır. `arq` = ARQ + parite 0. Bütün farklar "ARQ + artıklık − ARQ"dır; "ARQ'ya karşı FEC" karşılaştırması değildir.
- **Bütün sayılar bench üst sınırıdır [D-9]:** S1 `Reorder` ve Noise yok, tek akış. Ürün yolu sayısı değildir; ürün yolu = Ajan A `f0_flows`.
- Ana ölçü (hücre başına tek ölçü): r12k/d300 ve r1k → T1-800 (ağızdan kulağa ≤ 800 ms). d1000 → T2-800 (ağızdan kulağa ≤ 1800 ms = OWD_nom + 800).
- Değerler koşular arası medyan; dörtte birlikler doğrusal ara değerlidir (Ajan A en yakın sıra kullanır; küçük farklar bundandır).
- Payda: oranın paydası **çerçeve**, dağılım ve fark biriminin paydası **koşu**.
- Fark: medyan farkı, koşu birimli yüzdelik bootstrap %95 (B = 4000). Kollar eşlenemez; ayrı yeniden örneklenir. "f.g." = fark gösterilmedi (aralık 0'ı kapsıyor); "sınırda" = aralık 0'ı dışlıyor, yakın uç < 0,02.
- Çoklu karşılaştırma: birincil aile 11 hücre × 2 ikili × 4 kol = 88 karşılaştırma. %95 aralıklar düzeltmesizdir. Bonferroni aralığı (α = 0,05/88, B = 20 000) T.3'te. Diğer karşılaştırmalar (farklar.csv) keşif amaçlıdır.
- N = 10 hücreler ve %50 stres hücresi betimseldir; hüküm cümlesinde kullanılmaz.

## TS.1 Geçerlilik [T.1]

| | değer |
|---|---|
| hücre | 21: 11 birincil (N = 30/kol), 8 betimsel (N = 10/kol), 2 teşhis (`cx_`, 3 kol × N = 10) |
| koşu | 4160; rc = 0: 4160; geçerli: 4160. Düşen, zaman aşımı, yeniden koşu: 0 |
| `rx.jsonl` satırı = sunucu `delivered` | 4160/4160 |
| ısınma artığı > 0 | 16 koşu (12'si %50 stres hücresinde) |
| `realized_pre` temiz | 4159/4160. İstisna: `c_r12k_d300_l10_b60/arq_aday/rep1`, `ma0` kuyruğunda 1 paket (90 B); betimsel hücre, koşu tutuldu |
| gerçekleşen kayıp, hücre medyanı | %10: 0,097–0,105 · %20: 0,198–0,205 · %30: 0,297–0,302 · %50: 0,500 (ileri); dönüş aynı aralıkta. Koşu başına min–maks T.1'de |

**Ölçülemedi:**
- r12k/d1000, Tanım-1 (D = 400 ve 800 ms): fiziksel olarak olanaksız (OWD 1000 ms > D). T1-800 medyanı bütün kollarda ≤ 0,01. d1000 hücreleri Tanım-2 ile raporlanır.
- r25 ve r1k + 60 ms demet: ses hatta sığmaz (590 B/s > 25 B/s; ρ = 1,62). Hücre yok.
- K = 8 / E = 1024 ile son tarih: ürün kolu konuşma penceresinde hiç parite üretmedi (840/840 koşu). Ürün geometrisi yalnız modelde [çıkarım] (`uyar_ates`).
- Çerçeve başına "FEC ile mi ARQ ile mi geldi": `Delivery` kaynak taşımıyor (`runtime.rs:1190-1197`). Yalnız koşu toplamı var.
- r1k'da kilit/parite ayrıştırması: R referansı geçersiz (TS.3 no. 1).
- 20 s'den uzun konuşma ölçülmedi. d1000'de parite kuyruğu ≈ 15 s'de büyümeye başlıyor; sonrası bilinmiyor (TS.3 no. 2).
- Ürün yolu PTT son tarih oranı (bench dışı): Ajan A.

## TS.2 Sonuç tabloları

### Ana ölçü, sırasız [bench üst sınırı, D-9] — taban · aday

Tam dağılım (medyan [Q1–Q3] (min–maks), T1-400/T1-800/T2-400/T2-800, L2 bayt): T.2. N = 10 hücreler de T.2'de.

| hücre | arq | uyar | rs2 | rs4 | tekrar |
|---|---|---|---|---|---|
| r12k_d300_l10_b120 | 0,658 · 0,479 | 0,702 · 0,548 | 0,932 · 0,929 | 0,917 · 0,908 | 0,598 · 0,545 |
| r12k_d300_l20_b120 | 0,196 · 0,217 | 0,193 · 0,146 | 0,830 · 0,842 | 0,815 · 0,839 | 0,253 · 0,387 |
| r12k_d300_l30_b120 | 0,134 · 0,077 | 0,131 · 0,068 | 0,726 · 0,744 | 0,726 · 0,720 | 0,045 · 0,131 |
| r12k_d300_l20_b60 | 0,190 · 0,180 | 0,193 · 0,132 | 0,860 · 0,850 | 0,842 · 0,832 | 0,478 · 0,427 |
| r12k_d300_l20_b200 | 0,389 · 0,212 | 0,457 · 0,173 | 0,856 · 0,846 | 0,817 · 0,784 | 0,053 · 0,062 |
| r12k_d1000_l10_b120 | 0,884 · 0,887 | 0,890 · 0,887 | 0,970 · 0,973 | 0,952 · 0,946 | 0,884 · 0,845 |
| r12k_d1000_l20_b120 | 0,774 · 0,783 | 0,783 · 0,777 | 0,952 · 0,958 | 0,917 · 0,932 | 0,949 · 0,792 |
| r12k_d1000_l30_b120 | 0,661 · 0,598 | 0,673 · 0,634 | 0,911 · 0,911 | 0,887 · 0,887 | 0,598 · 0,789 |
| r1k_d300_l10_b200 | 0,534 · 0,399 | 0,524 · 0,317 | 0,736 · 0,793 | 0,779 · 0,764 | 0,058 · 0,058 |
| r1k_d300_l20_b200 | 0,274 · 0,149 | 0,183 · 0,130 | 0,663 · 0,630 | 0,553 · 0,298 | 0,058 · 0,062 |
| r1k_d300_l30_b200 | 0,106 · 0,096 | 0,130 · 0,135 | 0,442 · 0,260 | 0,245 · 0,120 | 0,062 · 0,038 |

### Fark: kol − arq (ana ölçü, aynı ikili) [%95]

| hücre | ikili | uyar − arq | rs2 − arq | rs4 − arq | tekrar − arq |
|---|---|---|---|---|---|
| r12k_d300_l10_b120 | taban | 0,045 [-0,289, 0,250] (f.g.) | 0,274 [0,205, 0,470] | 0,259 [0,193, 0,452] | -0,060 [-0,438, 0,262] (f.g.) |
| r12k_d300_l10_b120 | aday | 0,068 [-0,256, 0,268] (f.g.) | 0,449 [0,271, 0,530] | 0,429 [0,253, 0,509] | 0,065 [-0,554, 0,318] (f.g.) |
| r12k_d300_l20_b120 | taban | -0,003 [-0,152, 0,074] (f.g.) | 0,634 [0,506, 0,688] | 0,619 [0,491, 0,664] | 0,057 [-0,250, 0,741] (f.g.) |
| r12k_d300_l20_b120 | aday | -0,071 [-0,161, 0,048] (f.g.) | 0,625 [0,554, 0,714] | 0,622 [0,554, 0,711] | 0,170 [-0,182, 0,393] (f.g.) |
| r12k_d300_l30_b120 | taban | -0,003 [-0,086, 0,101] (f.g.) | 0,592 [0,524, 0,676] | 0,592 [0,524, 0,670] | -0,089 [-0,155, 0,033] (f.g.) |
| r12k_d300_l30_b120 | aday | -0,009 [-0,039, 0,021] (f.g.) | 0,667 [0,634, 0,685] | 0,643 [0,607, 0,661] | 0,054 [-0,048, 0,256] (f.g.) |
| r12k_d300_l20_b60 | taban | 0,003 [-0,254, 0,124] (f.g.) | 0,670 [0,429, 0,747] | 0,652 [0,411, 0,735] | 0,287 [-0,226, 0,545] (f.g.) |
| r12k_d300_l20_b60 | aday | -0,048 [-0,152, 0,089] (f.g.) | 0,670 [0,536, 0,731] | 0,652 [0,549, 0,716] | 0,247 [0,126, 0,396] |
| r12k_d300_l20_b200 | taban | 0,067 [-0,269, 0,361] (f.g.) | 0,466 [0,226, 0,740] | 0,428 [0,183, 0,692] | -0,337 [-0,582, -0,072] |
| r12k_d300_l20_b200 | aday | -0,038 [-0,231, 0,058] (f.g.) | 0,635 [0,442, 0,688] | 0,572 [0,389, 0,635] | -0,149 [-0,337, 0,062] (f.g.) |
| r12k_d1000_l10_b120 | taban | 0,006 [-0,009, 0,021] (f.g.) | 0,086 [0,071, 0,101] | 0,068 [0,054, 0,077] | 0,000 [-0,036, 0,074] (f.g.) |
| r12k_d1000_l10_b120 | aday | 0,000 [-0,012, 0,033] (f.g.) | 0,086 [0,074, 0,116] | 0,060 [0,051, 0,095] | -0,042 [-0,077, 0,000] (f.g.) |
| r12k_d1000_l20_b120 | taban | 0,009 [-0,018, 0,030] (f.g.) | 0,179 [0,149, 0,202] | 0,143 [0,110, 0,158] | 0,176 [0,146, 0,193] |
| r12k_d1000_l20_b120 | aday | -0,006 [-0,042, 0,042] (f.g.) | 0,176 [0,158, 0,223] | 0,149 [0,122, 0,196] | 0,009 [-0,030, 0,140] (f.g.) |
| r12k_d1000_l30_b120 | taban | 0,012 [-0,057, 0,039] (f.g.) | 0,250 [0,199, 0,277] | 0,226 [0,185, 0,250] | -0,062 [-0,548, 0,238] (f.g.) |
| r12k_d1000_l30_b120 | aday | 0,036 [-0,402, 0,461] (f.g.) | 0,312 [0,244, 0,744] | 0,289 [0,202, 0,708] | 0,190 [-0,006, 0,637] (f.g.) |
| r1k_d300_l10_b200 | taban | -0,010 [-0,164, 0,154] (f.g.) | 0,202 [0,135, 0,346] | 0,245 [0,159, 0,389] | -0,476 [-0,538, -0,337] |
| r1k_d300_l10_b200 | aday | -0,082 [-0,245, 0,067] (f.g.) | 0,394 [0,226, 0,490] | 0,365 [0,173, 0,447] | -0,341 [-0,505, -0,250] |
| r1k_d300_l20_b200 | taban | -0,091 [-0,269, 0,087] (f.g.) | 0,389 [0,231, 0,534] | 0,279 [0,077, 0,452] | -0,216 [-0,365, -0,072] |
| r1k_d300_l20_b200 | aday | -0,019 [-0,144, 0,053] (f.g.) | 0,481 [0,332, 0,529] | 0,149 [-0,043, 0,274] (f.g.) | -0,087 [-0,212, -0,058] |
| r1k_d300_l30_b200 | taban | 0,024 [-0,038, 0,077] (f.g.) | 0,337 [0,221, 0,452] | 0,139 [0,053, 0,202] | -0,043 [-0,096, -0,010] (sınırda) |
| r1k_d300_l30_b200 | aday | 0,038 [-0,010, 0,091] (f.g.) | 0,163 [0,067, 0,298] | 0,024 [-0,014, 0,091] (f.g.) | -0,058 [-0,101, -0,010] (sınırda) |

Sayım (22 karşılaştırma/kol):
- `uyar`: 22/22 f.g.
- `rs2`: 22/22 ayrıştı; Bonferroni aralığı 22/22 sıfırın üstünde.
- `rs4`: 20/22 ayrıştı, 2/22 f.g. (r1k l20 aday, r1k l30 aday); Bonferroni aralığı 18/22 sıfırın üstünde.
- `tekrar`: 7 ayrıştı (2'si artı yönde, 5'i eksi yönde), 2 sınırda, 13 f.g.

### Reorder türevi (sıralı teslim) [türetilmiş, bench üst sınırı] — taban · aday

PTT-yalnız oturumda S1 `Reorder`'ın karşılığı. Başka akış yok; ürün yolunda sıra başı boşlukları diğer akışlardan da gelir, gerçek değer bundan düşük olur.

| hücre | arq | uyar | rs2 | rs4 | tekrar |
|---|---|---|---|---|---|
| r12k_d300_l10_b120 | 0,202 · 0,235 | 0,226 · 0,280 | 0,708 · 0,685 | 0,628 · 0,586 | 0,128 · 0,155 |
| r12k_d300_l20_b120 | 0,033 · 0,051 | 0,036 · 0,033 | 0,384 · 0,405 | 0,312 · 0,387 | 0,024 · 0,018 |
| r12k_d300_l30_b120 | 0,012 · 0,018 | 0,012 · 0,012 | 0,134 · 0,143 | 0,179 · 0,170 | 0,006 · 0,012 |
| r12k_d300_l20_b60 | 0,028 · 0,018 | 0,019 · 0,016 | 0,396 · 0,338 | 0,333 · 0,217 | 0,006 · 0,009 |
| r12k_d300_l20_b200 | 0,058 · 0,058 | 0,072 · 0,062 | 0,587 · 0,466 | 0,356 · 0,365 | 0,019 · 0,019 |
| r12k_d1000_l10_b120 | 0,125 · 0,125 | 0,146 · 0,113 | 0,690 · 0,714 | 0,664 · 0,649 | 0,024 · 0,039 |
| r12k_d1000_l20_b120 | 0,024 · 0,012 | 0,021 · 0,021 | 0,711 · 0,702 | 0,568 · 0,619 | 0,006 · 0,006 |
| r12k_d1000_l30_b120 | 0,006 · 0,012 | 0,018 · 0,006 | 0,304 · 0,304 | 0,610 · 0,580 | 0,006 · 0,006 |
| r1k_d300_l10_b200 | 0,207 · 0,188 | 0,163 · 0,183 | 0,495 · 0,538 | 0,413 · 0,466 | 0,024 · 0,019 |
| r1k_d300_l20_b200 | 0,067 · 0,058 | 0,038 · 0,019 | 0,240 · 0,231 | 0,130 · 0,077 | 0,010 · 0,010 |
| r1k_d300_l30_b200 | 0,024 · 0,010 | 0,019 · 0,010 | 0,038 · 0,077 | 0,038 · 0,029 | 0,010 · 0,010 |

Sıralı ölçüde arq ve uyar 0,01–0,28 aralığında; rs2 − arq farkları T.4'te. Ajan A'nın ürün yolu ölçümü (`a_r12k_d300_l30_dosyasiz`, 200 ms demet): PTT ≤ 800 ms %2–4. Aynı mertebede bench türevi: `c_r12k_d300_l30_b200` (N = 10, betimsel) `arq`, `uyar` sıralı T1-800 0,01–0,04 [T.2/hucreler.csv].

### Sıkı son tarih — T1-400 (d300, r1k) ve T2-400 (d1000), taban · aday

| hücre | arq | uyar | rs2 | rs4 | tekrar |
|---|---|---|---|---|---|
| r12k_d300_l10_b120 | 0,048 · 0,077 | 0,071 · 0,054 | 0,140 · 0,161 | 0,167 · 0,158 | 0,051 · 0,057 |
| r12k_d300_l20_b120 | 0,024 · 0,030 | 0,030 · 0,021 | 0,101 · 0,110 | 0,125 · 0,119 | 0,030 · 0,039 |
| r12k_d300_l30_b120 | 0,018 · 0,012 | 0,018 · 0,012 | 0,095 · 0,086 | 0,092 · 0,089 | 0,012 · 0,027 |
| r12k_d300_l20_b60 | 0,068 · 0,052 | 0,049 · 0,046 | 0,381 · 0,369 | 0,403 · 0,403 | 0,134 · 0,153 |
| r12k_d300_l20_b200 | 0,000 · 0,005 | 0,000 · 0,000 | 0,010 · 0,010 | 0,010 · 0,005 | 0,000 · 0,000 |
| r12k_d1000_l10_b120 | 0,747 · 0,729 | 0,780 · 0,747 | 0,810 · 0,815 | 0,786 · 0,807 | 0,652 · 0,607 |
| r12k_d1000_l20_b120 | 0,655 · 0,658 | 0,655 · 0,643 | 0,726 · 0,750 | 0,714 · 0,717 | 0,726 · 0,557 |
| r12k_d1000_l30_b120 | 0,557 · 0,446 | 0,554 · 0,485 | 0,682 · 0,705 | 0,613 · 0,619 | 0,387 · 0,583 |
| r1k_d300_l10_b200 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 |
| r1k_d300_l20_b200 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 |
| r1k_d300_l30_b200 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 | 0,000 · 0,000 |

### Kilitli koşu sayısı (n/30) — taban · aday

| hücre | arq | uyar | rs2 | rs4 | tekrar |
|---|---|---|---|---|---|
| r12k_d300_l10_b120 | 16 · 24 | 16 · 20 | 0 · 0 | 0 · 0 | 18 · 20 |
| r12k_d300_l20_b120 | 23 · 30 | 26 · 26 | 0 · 0 | 1 · 0 | 17 · 23 |
| r12k_d300_l30_b120 | 26 · 29 | 28 · 29 | 0 · 0 | 0 · 1 | 22 · 27 |
| r12k_d300_l20_b60 | 25 · 26 | 23 · 27 | 0 · 0 | 0 · 0 | 22 · 24 |
| r12k_d300_l20_b200 | 18 · 27 | 18 · 26 | 0 · 0 | 0 · 1 | 23 · 26 |
| r12k_d1000_l10_b120 | 3 · 7 | 4 · 5 | 0 · 0 | 1 · 0 | 7 · 13 |
| r12k_d1000_l20_b120 | 5 · 9 | 5 · 9 | 0 · 0 | 0 · 0 | 6 · 18 |
| r12k_d1000_l30_b120 | 8 · 18 | 8 · 13 | 0 · 2 | 0 · 1 | 18 · 15 |
| r1k_d300_l10_b200 | 17 · 22 | 17 · 23 | 2 · 1 | 0 · 3 | 30 · 30 |
| r1k_d300_l20_b200 | 19 · 27 | 24 · 29 | 0 · 3 | 10 · 21 | 30 · 30 |
| r1k_d300_l30_b200 | 25 · 30 | 29 · 29 | 12 · 19 | 20 · 28 | 30 · 30 |

### Bant bütçesi [T.12]

- **rs ↔ arq eşit bütçe değildir:** L2 bayt/çerçeve oranı rs2 1,32–2,08 ×, rs4 1,14–1,87 ×. Yukarıdaki rs − arq farkları ek bant karşılığıdır; "aynı bütçede" denemez.
- **rs2 ↔ tekrar** (±%10 eşit) l20 hücrelerinin çoğunda eşit bütçededir (T.12 `eşit bütçe çiftleri`); %10 hücrelerinde değildir (rs2/tekrar 0,75–0,78).
- Kilitli kollarda ölçülen bayt düşüktür, çünkü çerçevelerin bir bölümü pencerede tele çıkmaz. Bu kollarda "daha az bayt" kazanç değildir.
- Bayt başına: r12k/d1000 hücrelerinde `arq` zamanında çerçeve/kB'de rs'den yüksektir (l20: 5,7–5,9 ↔ 3,7–3,9 rs2). Son tarih oranı artıyor, bayt verimi düşüyor.

### Gecikme bütçesi bileşenleri [T.5] (ms; seçili hücreler, tamamı T.5)

`demet`, `blok` yapılandırma · `seri` hesap (L2/hız) · `ağ nominal` netem · diğerleri ölçüm. Gönderen kuyruğu = pencere-minimum gecikmesinin ilk pencerelere göre artışı [türetilmiş]. Titreşim = doğrudan sınıf çerçevelerde artık. Geç = doğrudan sınıf dışı çerçeve.

| hücre | kol | demet | blok ort/maks | seri | ağ nominal | ilk pencere gecikmesi p50 | gönderen kuyruğu med / p95 | titreşim p50 / p95 | geç pay / gecikme | parite alt–üst |
|---|---|---|---|---|---|---|---|---|---|---|
| r12k_d300_l20_b120 | arq_taban | 120 | — | 8,5 | 300 | 347 | 638 / 1405 | 33 / 192 | 0,286 / 1656 | — |
| r12k_d300_l20_b120 | arq_aday | 120 | — | 8,5 | 300 | 333 | 1177 / 3312 | 22 / 203 | 0,384 / 1282 | — |
| r12k_d300_l20_b120 | rs2_taban | 120 | 60/120 | 8,5 | 300 | 315 | 38 / 92 | -6 / 96 | 0,173 / 1238 | 2213–2543 |
| r12k_d300_l20_b120 | rs2_aday | 120 | 60/120 | 8,5 | 300 | 327 | 35 / 87 | -5 / 103 | 0,161 / 1271 | 2354–2774 |
| r12k_d300_l20_b120 | rs4_taban | 120 | 180/360 | 8,5 | 300 | 325 | 36 / 90 | -4 / 113 | 0,190 / 1356 | 2114–2213 |
| r12k_d300_l20_b120 | tekrar_taban | 120 | — | 8,5 | 300 | 360 | 2610 / 4848 | 20 / 181 | 0,220 / 1318 | — |
| r12k_d1000_l20_b120 | arq_taban | 120 | — | 8,5 | 1000 | 1143 | 102 / 309 | -15 / 269 | 0,208 / 3781 | — |
| r12k_d1000_l20_b120 | arq_aday | 120 | — | 8,5 | 1000 | 1135 | 125 / 294 | -1 / 280 | 0,208 / 3689 | — |
| r12k_d1000_l20_b120 | rs2_taban | 120 | 60/120 | 8,5 | 1000 | 1164 | 91 / 212 | 59 / 334 | 0,039 / 3508 | 323–323 |
| r12k_d1000_l20_b120 | rs2_aday | 120 | 60/120 | 8,5 | 1000 | 1122 | 78 / 192 | 75 / 344 | 0,036 / 3546 | 162–162 |
| r12k_d1000_l20_b120 | rs4_taban | 120 | 180/360 | 8,5 | 1000 | 1149 | 121 / 258 | 3 / 367 | 0,071 / 3490 | 203–203 |
| r12k_d1000_l20_b120 | tekrar_taban | 120 | — | 8,5 | 1000 | 1174 | 62 / 290 | 95 / 358 | 0,042 / 3611 | — |
| r1k_d300_l20_b200 | arq_taban | 200 | — | 118,0 | 300 | 444 | 246 / 1258 | 8 / 177 | 0,260 / 2194 | — |
| r1k_d300_l20_b200 | arq_aday | 200 | — | 118,0 | 300 | 447 | 841 / 2399 | 6 / 191 | 0,327 / 2019 | — |
| r1k_d300_l20_b200 | rs2_taban | 200 | 100/200 | 118,0 | 300 | 454 | 63 / 165 | 24 / 158 | 0,192 / 2122 | 3048–3804 |
| r1k_d300_l20_b200 | rs2_aday | 200 | 100/200 | 118,0 | 300 | 471 | 86 / 217 | 8 / 167 | 0,197 / 2033 | 4020–5540 |
| r1k_d300_l20_b200 | rs4_taban | 200 | 300/600 | 118,0 | 300 | 455 | 114 / 259 | 7 / 163 | 0,236 / 2247 | 6304–8979 |
| r1k_d300_l20_b200 | tekrar_taban | 200 | — | 118,0 | 300 | 550 | 7028 / 10618 | 40 / 207 | 0,457 / 1109 | — |

- Ağ + serileştirme ölçümü: ilk pencere p50 d300'de 315–347 ms (nominal 308,5), d1000'de 1143–1164 ms (1008,5), r1k'da 444–454 ms (418). Nominalin üstündeki pay netem FIFO etkisidir.
- Serileştirme denetimi [T.5]: r1k − r12k ilk pencere minimumu farkı 129,6 ms, hesap 108,6 ms; 21 ms kuyruk.
- Bu üç hücrede bileşen sıralaması: `arq`'ta ağın üstündeki en büyük terim gönderen kuyruğu (medyan 0,1–1,2 s, p95 0,3–3,3 s). `rs`'de d300 ve r1k'da en büyük terim parite beklemesi. Blok oluşturma (T = 120, K = 2/4: 60–180 ms ortalama) ikincildir.

### Parite akıbeti [T.10] — taban · aday

| hücre | rs2 tx/plan (taban · aday) | rs2 bekleme alt–üst ms | rs4 tx/plan | rs4 bekleme alt–üst ms | FEC ilk ulaşan rs2 · rs4 (taban) |
|---|---|---|---|---|---|
| r12k_d300_l10_b120 | 91/92 · 90/92 | 1323–1323 · 1364–1507 | 46/46 · 42/46 | 1907–1977 · 3052–3052 | 9 · 7 |
| r12k_d300_l20_b120 | 160/184 · 158/184 | 2213–2543 · 2354–2774 | 84/92 · 86/92 | 2114–2213 · 2286–2464 | 13 · 12 |
| r12k_d300_l30_b120 | 132/184 · 114/184 | 4425–6498 · 5030–8137 | 157/184 · 128/184 | 3533–3990 · 4332–6283 | 14 · 14 |
| r12k_d300_l20_b60 | 333/352 · 315/352 | 1519–1549 · 2159–2368 | 161/176 · 132/176 | 2520–2990 · 3343–4389 | 30 · 22 |
| r12k_d300_l20_b200 | 120/120 · 112/120 | 788–795 · 2118–2186 | 56/60 · 53/60 | 2579–2919 · 1850–2083 | 14 · 8 |
| r12k_d1000_l10_b120 | 92/92 · 92/92 | 103–103 · 89–89 | 46/46 · 46/46 | 105–105 · 91–91 | 15 · 14 |
| r12k_d1000_l20_b120 | 184/184 · 184/184 | 323–323 · 162–162 | 92/92 · 92/92 | 203–203 · 207–207 | 32 · 29 |
| r12k_d1000_l30_b120 | 184/184 · 184/184 | 275–275 · 221–221 | 184/184 · 184/184 | 359–359 · 398–398 | 43 · 48 |
| r1k_d300_l10_b200 | 60/60 · 59/60 | 1390–1493 · 1413–1468 | 30/30 · 30/30 | 3023–3038 · 2071–2071 | 5 · 4 |
| r1k_d300_l20_b200 | 54/60 · 50/60 | 3048–3804 · 4020–5540 | 22/30 · 21/30 | 6304–8979 · 6576–10592 | 6 · 5 |
| r1k_d300_l30_b200 | 34/60 · 35/60 | 9619–17291 · 8781–15400 | 18/30 · 12/30 | 9985–16266 · 11347–29921 | 5 · 4 |

Kaybolan kaynak birim ≈ p × (çerçeve + 16): %20/120 ms'de ≈ 37. FEC'le ilk ulaşan: d300 12–13 (≈ %33), d1000 29–32 (≈ %78–86).

## TS.3 Bulgular (ölçüm · dosya:işlev:satır · kök neden · en dar düzeltme)

**1. Gönderen kilidi (B-C9 = B-K14 = A-9) — asıl engel.**
- **Ölçüm.** 11 birincil hücre, kilitli koşu (taban → aday, /330): arq 185 → 249, uyar 198 → 236, tekrar 223 → 256, rs2 14 → 25, rs4 32 → 55 [T.13, T.19]. Etki, r12k_d300_l20_b120: kilitli arq medyan 0,167 (n = 23), kilitsiz 0,815 (n = 7) [T.6]. Çeyrek profili (arq_taban): Ç1 0,667 → Ç2 0,155 → Ç3 0,000 → Ç4 0,000 [T.17]. N = 30.
- **Dosya.** `bw.rs:BtlBw::update:220-245`; `rate.rs:effective_send_rate:164-180`; `sender.rs:compute_send_rate:4239-4268` / aday `:4710`; `sender.rs:on_event_core:2354-2361` / aday `:2637-2644`; `sender.rs:startup_plateau:4208-4235`.
- **Kök neden.**
  - [ölçüldü] Son `btlbw` / (veri + yeniden gönderim hızı) `hız payı`: arq %20–30 hücrelerinde 0,78–0,99; rs kollarında 1,5–2,2 (r12k) [hucreler.csv]. Darboğaz değil: ρ 0,09, r12k.
  - [çıkarım] Kuyruk doluyken teslim örneği kaybolan baytı saymaz; Steady kazancı 1'de pacer teslim hızına kilitlenir; yeniden gönderim bandı ayrılmaz.
  - [ölçüldü] Kilit başlangıcı medyanı d300'de konuşmanın 3,2–10,4. saniyesi, d1000 %10–20'de 9,2–18,5, %30'da 3,4–5,0; oturum yaşı d300'de 13–22 RTT, d1000'de 9–11 RTT [T.14]. Startup platosu RTT saatlidir [kod]; başlangıcın Startup çıkışına bağlanması [çıkarım], durum zaman çizelgesi kaydedilmedi.
  - Sınır: %10 hücrelerinde arq `hız payı` > 1 iken de kilit var (16–24/30). Son btlbw anlık görüntüdür; kök neden tek başına bu oranla açıklanmaz.
- **En dar düzeltme.** `sender.rs:on_event_core:2361`: uygulama-sınırlı akışta pacer alt sınırı = son pencerede sunulan + yeniden gönderilen hız × (1 + pay), `cwnd_over_srtt` tavanıyla. btlbw'ye sentetik örnek yazılmaz. Mevcut `startup_pacing_floor` deseni. **Motor çekirdeği.**

**2. Parite pacer'da aç kalıyor (B-C4).**
- **Ölçüm.** Kuyruk beklemesi alt–üst (Little), koşular arası medyan, N = 30 [T.10]: r12k/d300 rs2 788–8137 ms, rs4 1850–6283 ms; r1k rs2 1390–17 291 ms, rs4 2071–29 921 ms; d1000 rs2 89–323 ms, rs4 91–398 ms. T = 120 ms. Tele çıkan pay r12k/d300 rs2 0,62–1,00; jeton reddi koşu başına 15,5–21 bin [hücre medyanları, T.10].
- **Zaman profili** [T.18]: rs2_taban r12k_d300_l20_b120 bekleyen parite 0,5 → 3,9 → 10,5 → 20,2 → 26,0 → 29,9 → 34,8 (3 s kutu). d1000_l20_b120: 15 s'ye kadar ≈ 0,5, sonra 2,8 → 10,7. Parite katkısı (Ç − R) d300 %20: Ç1 +0,11…+0,16 → Ç4 ≈ 0; d1000 %20: Ç1–3 +0,16…+0,18, Ç4 +0,09 [T.21].
- **Dosya.** `sender.rs:schedule_transmit:4292` < `:4311`; `sender.rs:plan_fec_parity:5319-5354` / aday `:5857-5868` (`copy_reserve_allows`); `sender.rs:on_event_core:2366-2381` / aday `burst_floor_bytes:7520-7531` (parite yok).
- **Kök neden.** [ölçüldü] parite kuyruğu varış hızından yavaş boşalıyor. [kod] parite son sınıf, jeton her DATA'da kırpılır. [çıkarım] rs kollarında hız payı ≈ 2 olsa da bu payın üstü pariteye gider: (2,2) kolunda parite talebi veri talebinin 1,44 katı. d1000'de kuyruk büyümesi ≈ 12 s geç başlar; neden ayrılmadı (Startup süresi RTT saatli olduğu için [çıkarım]). **d1000 sonuçları 20 s konuşmayla sınırlıdır.**
- **En dar düzeltme.** Bekleyen parite başı kova tabanına girsin; son tarihli sınıfta parite DATA'dan hemen sonra, ARQ'dan önce planlansın. **Motor çekirdeği.** No. 1'e bağlı.

**3. Ürün uyarlamalı FEC konuşmada parite üretmiyor (B-C1).**
- **Ölçüm.** `uyar`: 840 koşu, konuşma penceresinde parite baytı > 0 olan koşu **0** [T.9]. 10 koşuda ısınmada 1 blok (2 onarım, 20 onarım toplam); neden [çıkarım]: btlbw örneği yokken kırpma uygulanmıyor (`:3769`). `uyar64` 20/20 ve `pol` 20/20 parite 0. Kırpma medyanı 23,3–37,7 bin karar; arbiter önerisi 23,3–39,5 bin; btlbw 258–1127 B/s. `uyar − arq`: 22/22 f.g.
- **Dosya.** `sender.rs:fuzzy_evaluate:3763-3796` / aday `:4240-4267`; `budget.rs:budget_cap_permille:38-41`; `sender.rs:compute_requested_r:1026-1031`.
- **Kök neden.** [ölçüldü + kod] cap = btlbw × 1000 / 125 000 = 2–9 ‰; K = 8'de r ≥ 1 için ≥ 63 ‰ gerekir. Sıfırlama reddi permille düzeyinde (`:3786`).
- **En dar düzeltme.** Ses oturumunda denetleyici yerine `manual_r` yolu (`runtime.rs:2503-2507`, §8.5'i atlar; `rs*` kolları bunu kullandı). **Dar**, client-core. Genel akış için r düzeyinde sıfırlama reddi: **motor çekirdeği**, ayrı ölçüm ister.

**4. Blok süresi ve K (B-C2).**
- **Ölçüm.** K = 8: ölçülemedi (no. 3). K = 2 ↔ 4: parite zamanındayken, Ç1 − R (r12k/d300/%20, taban + aday): T = 60 ms rs2 +0,156, rs4 +0,123; T = 120 +0,127, +0,074; T = 200 +0,107, +0,024 [T.21]. Bütün konuşmada bloktaki konum etkisi küçük: rs4 konum 0 → 3 farkı ≤ 0,09 [T.11]; parite gecikmesi bunu örtüyor. Ön-kayıt "konum etkisi belirgin" tutmadı.
- **Dosya.** `runtime.rs:client_task:2500`; `:2579-2632`.
- **Kök neden.** [kod] kapanış yalnız K dolunca. [türetilmiş] T = 200, K = 4'te blok 600 ms; d300 D = 800 payı ≈ 290 ms.
- **En dar düzeltme.** Ses için K = 2 profili (0x01). **Önceki önerim düzeltildi:** kısmi blok kodunu ana döngüye taşımak yetmez; blok ve ESI `unit_id`'den türer (`runtime.rs:2498`, `:3617-3618`), erken kapanış eşlemeyi bozar, kimlik atlamak `Reorder`'ı durdurur. Süre tabanlı kapanış ancak A-2 çözülürse ya da kaynak çerçevesi blok kimliği taşırsa mümkün. **Sözleşme.**

**5. E = 1024 onarımı (B-C3).**
- **Ölçüm.** Konuşma penceresinde ölçülemedi (0 parite). Isınmada 20 onarım × 1052 B (bench; hub zarfıyla 1058 B). Hesap: r1k'da tek onarım 1094 ms, r12k'da 87,5 ms. `rs*` E = 64: 92 B. Karşı-olgu [çıkarım, model]: `uyar_ates` r1k ρ 2,0–2,1; r12k T1-800 kazancı ≤ 0,03.
- **Dosya.** `runtime.rs:2472`, `:2501`; `profile.rs:85-91`.
- **Kök neden.** [kod] sembol E'ye dolgulu; onarım yükten bağımsız.
- **En dar düzeltme.** Ses oturumu profili 0x01/0x02 (E = 64). Oturum başına tek profil var (`runtime.rs:282`) ve bloklar bütün trafiği karıştırıyor (Ajan A): ayrı PTT oturumu ya da akış başına FEC gerekir. **Sözleşme.**

**6. Sıralı teslim ve Reliable (B-C6, A-2).**
- **Ölçüm.** Ana ölçü sırasız → sıralı [T.4], r12k_d1000_l20_b120: arq 0,77 → 0,02; rs2 0,95 → 0,71. r12k_d300_l20_b120: arq 0,20 → 0,03; rs2 0,83 → 0,38. r12k_d300_l30_b120: rs2 0,73 → 0,13. Yeniden gönderim, arq: 6,7–26,3 B/çerçeve, ARQ varışı son tarihten sonra [türetilmiş; T.5 geç gecikme r12k/d300 1,3–1,9 s, d1000 3,7–4,0 s].
- **Dosya.** `kama-app/src/ptt.rs:Ptt::send_bundle:627`; `client-core/src/transport.rs:Reorder:367-412`, `:1272-1281`; `runtime.rs:Delivery:1190-1197`; `sender.rs:declare_delivery_deadline:4354-4359`.
- **Kök neden.** [kod] PTT Reliable, oturum geneli tek sıra; `DeadlineBounded` yalnız FEC politika girdisi; `Delivery` politika taşımıyor. [türetilmiş] kaybolan her çerçeve sonraki çerçeveleri ARQ süresi kadar bekletir; FEC son tarih içinde kurtarırsa bu duruş kalkar (d1000: +0,69).
- **En dar düzeltme.** A'nın F1-A6'sı (akış başına sıra) ya da PTT için sıra atlayan tel bayrağı; K4G-08 sahibi kararı. **Sözleşme.**

**7. 05b adayı kilidi artırıyor.**
- **Ölçüm.** CMH (11 tabaka), aday ↔ taban kilit OR: arq 3,60 (p = 1,2e-09), uyar 2,21 (p = 0,00021), rs4 4,63 (p = 0,00013), tekrar 1,94 (p = 0,0014), rs2 2,74 (p = 0,037; 5 kol Bonferroni'de, α = 0,01, ayrışmıyor) [T.19]. `kilitli` eşiği duman-2 sonrası konmuştur; eşik sonrası sınama olarak okunmalıdır. Ana ölçüde aday − taban: 55 karşılaştırmada 4 ayrıştı (hepsi eksi yönde), 4 sınırda (1'i artı yönde), 47 f.g. [T.13].
- **Dosya.** 05b `sender.rs` tamamı; tek satıra indirilmedi.
- **Kök neden.** Ayrılmadı.
- **En dar düzeltme.** Yok. No. 1 yapılırsa aday ve taban ayrı ölçülmeli.

**8. Küçük kusur: geç onarım tamamlanmış bloğu yeniden çözüyor.**
- **Ölçüm.** rs2 r = 2 kolunda FEC ilk ulaşan / çözülen 0,21–0,62, r < K kollarında ≥ 0,83 [T.10]. Gereksiz RS hesabı; gecikme etkisi ölçülmedi.
- **Dosya.** `runtime.rs:fec_ingest:3232`, `:3255`.
- **Kök neden.** [kod] tamamlanan blok haritadan silinir, geç onarım boş kayıt açar; r ≥ K'da kaynaksız "çözülür".
- **En dar düzeltme.** Tamamlanan blok kimliğini sınırlı süre hatırla. **Dar.** Duman-2'deki "76/22" yorumum bu yüzden yanlıştı; §10.2'de düzeltildi.

### Soruların cevabı

**Ara nottaki çıkarım tam sette [T.7, T.21].** R = ort(1 − p_ileri) referansıyla ort(rs) − ort(arq) = kilit bileşeni + parite bileşeni (ortalama, %95 bootstrap):

| hücre kümesi | toplam | kilit bileşeni | parite bileşeni | kilit payı |
|---|---|---|---|---|
| r12k/d300 (5 hücre) | +0,35…+0,62 | +0,33…+0,58 | −0,03…+0,06 | 0,87–1,06 |
| r12k/d1000 (3 hücre) | +0,07…+0,44 | +0,04…+0,27 | +0,03…+0,19 | 0,25–0,65 |

- **d300: evet.** rs − arq farkının ≥ %87'si kilit bileşenidir. **d1000: hayır.** Farkın %35–75'i son tarih içinde parite katkısıdır. Ara nottaki genelleme yalnız d300 için tutar.
- Son tarih içinde parite ile telafi edilen kayıp payı: d300 rs2 %11–32, rs4 −13…+23 (aralık bazı hücrelerde 0'ı kapsıyor); d1000 rs2 %58–76, rs4 %29–62.
- r1k: R referansı geçersiz (kilitsiz arq 0,58–0,75 < R 0,69–0,90; hat doluluğu T1-800 tavanını düşürüyor). Ölçülemedi.
- Kilitli/kilitsiz alt küme N'leri T.6'da. rs'de kilitli hücreler r1k dışında 0–2/30. arq kilitsiz n: d300 %20–30 hücrelerinde 0–12, %10'da 6–14, d1000'de 12–27. n < 30 betimseldir; `kilitli` sonuçtan türer, alt küme kıyası nedensel değildir. R referansı bu seçimden bağımsızdır.

**Kilitsiz alt kümede tekrar neden rs'den yüksek?** [T.8]
- d300: tekrar 0,92–0,96 (n = 3–13), rs2 0,73–0,86, rs4 0,72–0,84. d1000: tekrar 0,89–0,95, rs2 0,91–0,97.
- [ölçüldü] Fark, parite zamanında olduğu hücrede kapanıyor. tekrar ≈ 1 − p̄² (kopya aynı sunumda, DATA sınıfında gider). rs2 d300'de 1 − p̄'a yakın, binom değerine (0,94–0,98) yalnız Ç1'de yaklaşıyor.
- Yani fark kodlamadan değil zamanlamadan (no. 2). Bütün koşularda ise tekrar, DATA talebini iki katına çıkardığı için en sık kilitlenen koldur (223–256/330) ve r1k'da 30/30.

**R2A/FS-1 ile ilişki.** Çelişki yok; ölçü ve düzenek farklı (§6.2).
- R2A 59–66/100 ↔ 12–14/100: süreç-içi model, 48 sembollük mesaj, ufuk içinde tamamlanma, parite doğrudan kanala. Son tarih isabeti ≈ 0.
- F0: ürün runtime'ı, çerçeve başına son tarih, parite pacer'dan geçiyor. Yön d1000'de aynı (+0,09…+0,31). d300'de parite zamanında olduğu ilk 5 s'de aynı, sonra kayboluyor.
- FS-1'in "kodlama > kör çoğaltma" sonucu: rs2 − tekrar 22 karşılaştırmada 18 ayrıştı, 2 sınırda, 2 f.g. (hepsi artı yönde), ama bu fark kilitten geliyor; kilitsiz alt kümede d300'de ters yönde [T.8, betimsel]. Kodlama kazancı ürün yolunda son tarih ölçüsüyle ayrılmadı. Yeni iddia üretilmedi.

### Ön-kayıt ↔ ölçüm [T.15]

| öngörü | sonuç | sapma ayrıştırması |
|---|---|---|
| arq/uyar T1-800 ≈ 1 − p | **Tutmadı.** r12k/d300 0,07–0,70. Kilitsiz alt küme 0,65–0,84 | Fark = kilit (M3 çürüdü) |
| uyar parite 0, ≥ %95 koşu | **Tuttu.** 830/840; konuşmada 840/840 | — |
| rs2 d300 0,90–0,98 (l20), rs4 0,83–0,95 | **Tutmadı.** rs2 0,83–0,86, rs4 0,78–0,84; Ç1 rs2 0,92–0,98 | −0,13 parite açlığı; pacer1.00 varyantı da yetmedi |
| d1000 T2-800 arq 0,79, rs2 0,98, rs4 0,93 | **Tuttu** (l20): 0,77–0,78, 0,95–0,96, 0,92–0,93 | Ç4'te düşüş başlıyor |
| parite bekleme < T | **Tutmadı.** d300 ≥ 3,9T, r1k ≥ 7T, d1000 0,7–3,3T | B-C4 |
| rs4 konum etkisi belirgin | **Tutmadı** (bütün konuşmada); Ç1'de K etkisi var | parite gecikmesi örtüyor |
| taban ≈ aday (FEC kollarında ±0,02) | **Kısmen.** r12k tuttu, r1k tutmadı (rs4 −0,13…−0,26) | aday kilit artışı |
| tekrar kilitsiz 0,96 | **Tuttu:** 0,92–0,96 | — |

## TS.4 Sınıflandırma (ISEMRI §5)

Sınıf dili: **dar** (tek crate, motor dışı) · **sözleşme** (S1, AMP ya da tel; sahip onayı) · **motor çekirdeği** (`adaptive-transport`).

| davranış | (i) 05b düzeltiyor mu (aday ölçümü) | (ii) sınıf ve gerekçe | (iii) KTP-2 0.3 | (iv) korunacak avantaj |
|---|---|---|---|---|
| Gönderen kilidi (no. 1) | **hayır**; kilit sıklığı aday'da yüksek (OR 1,9–4,6) | **motor çekirdeği**: pacer hız kararı | §3.5 "teslim hızı örnekleyicisi ve bant filtresi aynen": **bu kusuru taşır** [çıkarım]; ses tavanı §3.6, H-3 | kayıpta hız düşürmeme (INV-RATE-1): alt sınır yalnız yükseltir |
| Parite açlığı (no. 2) | **hayır**; bekleme aynı mertebe, aday'da rezerv ek | **motor çekirdeği**: `schedule_transmit` sırası, kova | §3.8 "onarım hemen", §3.6 adım 2 ve 5 | d1000 kazancı +0,09…+0,31 |
| Uyarlamalı parite 0 (no. 3) | **hayır**; aday 410/410 konuşmada parite 0 | **dar** (ses için `manual_r`) ya da **motor çekirdeği** (genel) | §3.8 tablo; denetleyici kalkar | RS kodlayıcı |
| K = 8, süre sınırı yok (no. 4) | **hayır** [kod] | **sözleşme**: blok `unit_id`'ye bağlı | §3.8 blok süresi; H-1 | — |
| E = 1024 (no. 5) | **hayır** [kod] | **sözleşme**: oturum başına tek profil | §3.8 "sembol = parça", H-11, H-12 | — |
| PTT Reliable, sıralı (no. 6) | **hayır** [kod]; sıralı türev aday'da aynı mertebe | **sözleşme**: S1 §4, AMP PTT-R0 | §3.1, §3.2 | mesaj alındısı yükümlülüğü |
| Süre dolumu yok | **hayır** [kod] | **motor çekirdeği** + sözleşme | §3.6 adım 1 | — |

## TS.5 F1 girdisi (ISEMRI §5.5: FEC ürün yolunda son tarihli PTT için nasıl etkinleştirilecek)

Sıra bağımlılığa göredir. Beklenen etkiler [tahmin]dir; dayandığı ölçülen sayı yazılıdır. Bütün dayanak sayılar bench üst sınırıdır.

| # | crate / dosya | değişiklik | sınıf · bağımlılık | ölçü ve beklenen etki [tahmin] | süre |
|---|---|---|---|---|---|
| F1-C1 | adaptive-transport `sender.rs:on_event_core:2361` | Uygulama-sınırlı akışa pacer alt sınırı: son pencere sunulan + yeniden gönderilen × (1 + pay), `cwnd_over_srtt` tavanlı | **motor çekirdeği** · yok. B-K14 / A-9 ile aynı iş | K-6, K-2: kilit 185–249/330 → < %5; arq T1-800 %20 0,20 → ≈ 0,80 (dayanak: kilitsiz 0,815, n = 7; R 0,80). INV-RATE-1 golden'ı güncellenir | 2 |
| F1-C2 | `sender.rs:schedule_transmit:4292-4311`, `plan_fec_parity:5319`, `on_event_core:2366-2381` | Son tarihli blok paritesi DATA'dan hemen sonra, ARQ'dan önce; kova tabanı parite başını kapsar | **motor çekirdeği** · F1-C1 | K-6: rs2 r12k/d300 %20 T1-800 0,83 → 0,93–0,98 (dayanak: Ç1 0,93–0,98; d1000 0,95) | 2 |
| F1-C3 | client-core `runtime.rs` (`fec_manual_r` yolu `:2503`) | Ses oturumunda r'yi ölçülen kayıptan tablo ile seç (taslak §3.8), §8.5 atlanır | **dar** · F1-C2 | K-6: parite 0 → plan = (K, r); bant 1,5–2,1 × arq (T.12) | 1 |
| F1-C4 | kama-node + client-core + S1 | PTT için ayrı oturum/uç ya da akış başına FEC; profil 0x01 (K = 2, E = 64) | **sözleşme** · F1-C3, F1-A6 | K-6: blok ≤ T, onarım 98 B (hub zarfı). Dayanak: rs2 ↔ rs4 Ç1 farkı +0,03…+0,08 | 3 |
| F1-C5 | kama-app `ptt.rs:627`, client-core `transport.rs:1272`, `sender.rs` | PTT `DeadlineBounded`; süresi geçen birim kuyruk ve ARQ'dan silinir (K-6) | **sözleşme** (K4G-08, sahip) + **motor çekirdeği** · F1-C1 | K-6, K-7: son tarih sonrası yeniden gönderim 6,7–26 B/çerçeve → ≈ 0; r1k'da L2 baytın ≈ %7–15'i | 2 |
| F1-C6 | client-core `transport.rs:Reorder`, `runtime.rs:Delivery`, tel bayrağı | PTT sıra beklemesini atlar (A-2); `Delivery` politika taşır ya da akış başına sıra (F1-A6) | **sözleşme** (S1 §4) · F1-A6 | K-6: sıralı → sırasız (d1000 rs2 0,71 → 0,95; d300 rs2 0,38 → 0,83, F1-C2 sonrası ≥ 0,93) | 2 (F1-A6 içinde) |
| F1-C7 | client-core `runtime.rs:fec_ingest:3232` | Tamamlanan bloğu sınırlı hatırla | **dar** · yok | CPU; son tarih etkisi beklenmez | 0,5 |

Sıra: F1-C1 → F1-C2 → F1-C3 → F1-C4 / F1-A6 → F1-C5 → F1-C6. F1-C1 ve F1-C2 olmadan parite açmak d300'de ilk 5 s dışında kazanç vermez (T.21). F1-C6 olmadan sıralı türev rs2'de sırasız oranın %9–76'sında kalır (T.4).

## TS.6 TABAN girdisi (S3)

**S3. Yüksek RTT'de FEC, ARQ'ya göre son tarih içinde ne kazandırıyor?** (ARQ her kolda açık; sayılar bench üst sınırı [D-9], ürün yolu değil. Kama r12k = 100 kbit/s, r1k = 8 kbit/s; 20 s konuşma.)

- **Ürün FEC'i bugün kazandırmıyor.** 840 koşuda konuşma penceresinde parite 0; `uyar − arq` 22/22 fark gösterilmedi. Kök neden §8.5 kırpması (`sender.rs:3763-3796`).
- **Sabit RS (motora dokunmadan), RTT 2 s, ağızdan kulağa ≤ 1,8 s:** rs2 %10 +0,09 [0,07, 0,10] → 0,97; %20 +0,18 [0,15, 0,20] → 0,95; %30 +0,25 [0,20, 0,28] → 0,91. Bant 1,5–2,0 ×. Son tarih içinde kaybın %58–76'sı telafi ediliyor. Bu sonuç 20 s konuşmayla sınırlı: parite kuyruğu ≈ 15 s'den sonra büyüyor.
- **RTT 600 ms, ≤ 800 ms:** rs2 +0,27…+0,67, ama farkın ≥ %87'si gönderen kilidinin önlenmesinden geliyor; son tarih içinde parite katkısı +0,03…+0,06. Parite kuyrukta 0,8–8 s bekliyor; katkı ilk 5 s'de +0,11…+0,16, sonra ≈ 0.
- **RTT 2 s'de 400/800 ms fiziksel olarak olanaksız. RTT 600 ms'de 400 ms'ye en iyi medyan 0,40 (60 ms demet, %20).** ≥ %95 medyanı yalnız d1000 + 1,8 s'de görüldü (rs2 %10–20, rs4 %10 taban).
- **8 kbit/s:** yalnız 200 ms demet sığıyor; rs2 +0,16…+0,48, tekrar 30/30 kilitli.
- **Sıralı teslimde (S1 `Reorder` türevi):** arq 0,01–0,24, rs2 0,04–0,71. FEC'in asıl ürün değeri burada: RTT 2 s %20'de 0,02 → 0,71.
- **Eşit bütçe uyarısı:** rs ↔ arq eşit bütçe değildir; `tekrar` ile rs2 %20'de eşittir, ama tekrar 223–256/330 koşuda kilitleniyor.
- **Gerekli sıra (§5.5):** pacer alt sınırı → parite önceliği → ses için `manual_r` → K = 2/E = 64 → süre dolumu → sıra atlama.

## TS.7 Yeniden üretim

```
python3 -I scripts/ktp2/f0/c/tam_c.py --csv reports/ktp2/f0/c/tam ~/kama-k4g-work/ktp2-f0/runs/tam/c/out > reports/ktp2/f0/c/tam/tablolar.md
python3 -I scripts/ktp2/f0/c/tam_c_ek.py reports/ktp2/f0/c/tam/kosular.csv >> reports/ktp2/f0/c/tam/tablolar.md
```

- Ham kök git dışı: `~/kama-k4g-work/ktp2-f0/runs/tam/c/out/`.
- Betikler: `scripts/ktp2/f0/c/{analyze_c.py, tam_c.py, tam_c_ek.py, model_c.py}`.
- Bu bölümün özet tabloları `hucreler.csv` ve `farklar.csv`'den üretildi.
- Çıktılar iki bağımsız koşuda bayt bayt aynı (tohumlar sabit). sha256:
  - `kosular.csv` `e0e58e7ee64a135df912d89d51de563501668634a011054e0ada3278c4ce09e8`
  - `hucreler.csv` `536700ae389f550f3eb67f82747b6a0045a06411eabecca7593b9ef1cc82e339`
  - `farklar.csv` `42500aeba5c450a31ca2fb0df5f4b2012ef7699f2397fb495d9c0c6b59313dfd`
  - `ayristirma.csv` `0fc6e80cd41f65ead9fe35381a20680f8e027a3db7a3c3b0fe47f94875ac645d`
  - `tablolar.md` `af1745581d396329115e9d0df5301188d69448e2573cf3477121f65d7d5ddcc0`

## Ek: Ajan E'ye iletilmek üzere taslak notu adayları [kod]

- Taslak §1 "FEC varsayılan olarak kapalı": kodda açık (`runtime.rs:282-285`). Etkisizliği B-C1 kırpmasından gelir.
- §4.1 "300 B mesajın FEC yolundaki tel boyu 6 × (64 + 37) = 606 B": sevk profilinde (0x05, parçalama kapalı, `runtime.rs:266`) 300 B mesaj tek DATA'dır: 22 + 11 + 300 = 333 B. 606 B yalnız E = 64 / 53 B parçalı yapılandırmanın (R2A modeli) değeridir.
- K-6 örneği "200 + 200 + serileştirme + 300 + titreşim → ≈ 750 ms": sabit terimlerin toplamı 700 ms.
- K-6 ses tel hızları KTP-2 başlığıyla (31 B) ve AMP 13 B hariç verilmiş. Bugünkü motor + uygulama 120 ms'de UDP 533 B/s, netem L2 883 B/s (§3.2).
