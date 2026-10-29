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

**En dar düzeltme.** Sınıf başına blok zamanlayıcısı: `cur_block` boş değilse ve ilk birimin yaşı T_blok'u geçtiyse, mevcut kısmi-blok kodu (`runtime.rs:2580-2630`, Model-A known-zero) ana döngüde de çalıştırılır. Kısmi blok decode'u alıcıda zaten var (`runtime.rs:3279-3285`).

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

- `rs2_taban` d300'de çözülen 76 bloğun yalnız 22'sinde kurtarılan birim alıcıya ilk kez ulaştı. Kalanında birim ARQ ile çoktan gelmişti; parite geç kaldı.
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

## Ek: Ajan E'ye iletilmek üzere taslak notu adayları [kod]

- Taslak §1 "FEC varsayılan olarak kapalı": kodda açık (`runtime.rs:282-285`). Etkisizliği B-C1 kırpmasından gelir.
- §4.1 "300 B mesajın FEC yolundaki tel boyu 6 × (64 + 37) = 606 B": sevk profilinde (0x05, parçalama kapalı, `runtime.rs:266`) 300 B mesaj tek DATA'dır: 22 + 11 + 300 = 333 B. 606 B yalnız E = 64 / 53 B parçalı yapılandırmanın (R2A modeli) değeridir.
- K-6 örneği "200 + 200 + serileştirme + 300 + titreşim → ≈ 750 ms": sabit terimlerin toplamı 700 ms.
- K-6 ses tel hızları KTP-2 başlığıyla (31 B) ve AMP 13 B hariç verilmiş. Bugünkü motor + uygulama 120 ms'de UDP 533 B/s, netem L2 883 B/s (§3.2).
