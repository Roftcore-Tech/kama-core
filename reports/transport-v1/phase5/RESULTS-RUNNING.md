# FAZ 5 — HÜCRE SONUÇLARI (biriken kayıt)

Taban: `e3c1e8bb740bbc25acd2c9fc5a7fc478a3f15adc` (ERRATUM-1 sonrası).
Ön-kayıt: [`EXPECTATION.md`](EXPECTATION.md) · Erratum: [`ERRATUM-1-blackout-boundary.md`](ERRATUM-1-blackout-boundary.md)
Kıyas düzeyi tüm hücrelerde **NON-NORMALIZED** (Kama `reliable-messages`/sırasız ↔ rakipler
`reliable-ordered-stream`/tam sıralı). Deney koşulları eşleşiyor (BLOCKING yok).

---

## T0 — TEMİZ KONTROL (`h6_rtt20`: delay 10ms loss 1% rate 10mbit, çift yön) · N=30 · D=20s

| kol | n | claim | completion(20s) medyan | IQR | CI | outcome | blackout | stall |
|---|---|---|---|---|---|---|---|---|
| kama-v1-full | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| quinn-reliable-stream | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| tcp-default | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |

completion(5s) ve completion(10s) de üç kolda 1.000.

**Kama aktivasyonu (medyan):** parity=0 · denied=0 · retx=33 · dup=0 · tail_probe=33 ·
srtt=20.9 ms (enjekte 2×10 ms ile tutarlı) · btlbw=13.9 kB/s · fwd_loss=7‰ (enjekte %1).

**Tel maliyeti (medyan):** kama data=28 300 B + retx=9 069 B (+parity=0) ≈ **37.4 kB** ·
quinn **35.8 kB** · tcp 26.8 kB.
*TCP sayısı UYGULAMA katmanıdır; çekirdek retransmit/ACK/başlık trafiği harness'a görünmez ve
Kama/quinn'in tam tel muhasebesiyle DOĞRUDAN kıyaslanamaz (metodoloji §10).*

**Ön-kayıtlı beklentiye karşı:**
- "Kama normal çalışmalı" → ✔ 1.000
- "parity ≈ 0 kalmalı" → ✔ parity=0, yapı-gate temiz yolda hiç açılmadı
- "patolojik blackout olmamalı" → ✔ 0/30
- "patolojik retransmit olmamalı" → retx/data **%32** (33 retx / 100 mesaj), tamamı tail
  probe (retx == probes). Ancak TOPLAM tel Kama 37.4 kB ↔ quinn 35.8 kB, yani **%4 fark**.
  Yani fazladan iletim gecikme karşılığında ödenmiş bir tasarım takası; toplam maliyet QUIC ile
  paritede. **Patolojik DEĞİL.** Bağlam: 50 ms'lik paced geometri her mesajdan sonra doğal bir
  "kuyruk" yaratır, bu yüzden tail probe sık ateşler — yük biçiminin sonucu, kolun kusuru değil.

### T0 HÜKMÜ: **COMPARABLE / PARITY**
Birincil metrikte üç kol da tavanda ve güven aralıkları özdeş. Kontrol hücresi amacını
gerçekleştirdi: **Kama'nın hostile uzmanlaşması sağlıklı yolda regresyon üretmiyor.**

---

## T1a — RANDOM LOSS (`p1_loss50`: loss 50% rate 100kbit, çift yön) · N=30 · D=20s

| kol | n | claim | completion(20s) medyan | IQR | CI | outcome | blackout | stall |
|---|---|---|---|---|---|---|---|---|
| kama-v1-full | 30 | final | **1.000** | [1.000,1.000] | [1.000,1.000] | blackout=18 completed=10 deadline-miss=2 | 18/30 en uzun 15.0s | 18/30 CI[0.406,0.773] |
| quinn-reliable-stream | 30 | final | 0.900 | [0.470,1.000] | [0.530,1.000] | blackout=25 completed=2 conn-fail=1 timeout=2 | 27/30 en uzun 18.0s | 28/30 CI[0.779,0.992] |
| tcp-default | 30 | final | 0.120 | [0.050,0.550] | [0.060,0.510] | blackout=26 conn-fail=1 timeout=3 | 29/30 en uzun 26.0s | 30/30 CI[0.884,1.000] |

Deadline eğrisi (medyan): **D=5s** kama 0.940 / quinn 0.220 / tcp 0.040 —
**D=10s** kama 0.990 / quinn 0.510 / tcp 0.060 — **D=20s** kama 1.000 / quinn 0.900 / tcp 0.120.

### Varsayımsız dağılım ifadeleri (sıra-tabanlı, Gaussian varsayımı YOK)

| | kama-v1-full | quinn | tcp |
|---|---|---|---|
| min | **0.970** | 0.000 | 0.000 |
| medyan | **1.000** | 0.920 | 0.125 |
| ≥0.99 olan koşu | **28/30** | 11/30 | 2/30 |
| <0.50 olan koşu (felaket) | **0/30** | 8/30 | 20/30 |

* Kama'nın **en kötü koşusu (0.970), quinn'in medyanının (0.920) ÜSTÜNDE.**
* `P(Kama koşusu > quinn koşusu) = 0.789` · `P(Kama koşusu > TCP koşusu) = 0.958` (0.5 = fark yok).
* Kama hiçbir koşuda %50'nin altına düşmedi; quinn 8, TCP 20 koşuda düştü.

### Aktivasyon (medyan) — ÖN-KAYITLI BEKLENTİ YANLIŞLANDI

```
parity_emitted = 0   parity_token_denied = 0   ← yapı-gate HİÇ açılmadı
retx = 95   dup = 9   tail_probe = 11   recover_arq = 8   state_transitions = 18
srtt = 99.2 ms   btlbw = 8.1 kB/s   fwd_loss = 496‰ (enjekte %50)   ret_ack_loss = 658‰
tel: data = 28 300 B   retx = 26 427 B   parity = 0 B
```

Ön-kayıt §9'da T1a için *"yapı-gate açılır → parity; spekülatif L1/L2"* yazmıştım.
**Ölçüm bunu yanlışladı:** %50 bağımsız kayıpta parity ne ÜRETİLDİ ne de ÖNERİLİP REDDEDİLDİ
(`denied = 0`). Kama bu hücreyi **saf ARQ ile, sıfır FEC ile** kazandı. Faz 1–4 boyunca
kablolanan kodlama alt-sistemi bu hücreye hiçbir katkı yapmadı. Tahmin geriye dönük
düzeltilmiyor; sapma olduğu gibi kaydediliyor.

Bağlam (Aşama A gözlemiyle tutarlı): iid kayıp hücrelerinde yapı-gate kapalı kalıyor
(T1a %50 → parity 0, T1b %80 → parity 0), buna karşılık korelasyonlu/gemodel hücrede açılıyor
(T2a → parity 22, denied 1943). Bu, `Q18Iid` yapı politikasının bir IID-yapı dedektörü olmasıyla
tutarlıdır. **Faz-5'te ayarlama YAPILMAZ**; T2 sonucuyla birlikte sınıflandırılacaktır.

### Tel maliyeti
kama data 28.3 kB + retx 26.4 kB ≈ **54.7 kB** · quinn **56.5 kB** · tcp 26.8 kB (uygulama
katmanı; çekirdek trafiği görünmez, DOĞRUDAN kıyaslanamaz — metodoloji §10).
Kama, quinn'den **%3 daha az** tel harcayarak completion'ı 0.900 → 1.000'e çıkardı.

### Anlam farkı (NON-NORMALIZED — zorunlu beyan)
quinn ve TCP **tam sıralı akış** garantisi verir; Kama **mesaj granülünde sırasız** teslim eder.
Kama'nın üstünlüğünün bir kısmı doğrudan bu **daha zayıf garantiden** gelir: sıralı akışta tek
bir kayıp ardındaki tüm teslimleri bloklar (head-of-line), mesaj granülünde bloklamaz. Bu fark
normalize EDİLMEMİŞTİR ve edilmemelidir; sonuç bu beyanla birlikte okunmalıdır.

### T1a HÜKMÜ: **KAMA DOMINATES**
Birincil metrikte üç deadline'da da her iki rakibi geçti; en kötü koşusu quinn'in medyanının
üstünde, felaket koşusu yok (0/30 vs 8/30 ve 20/30). Üstünlük **kodlamadan değil, ARQ +
mesaj-granülü tesliminden** geliyor — bu, hücrenin en önemli bulgusudur.

---

## T1b — RANDOM LOSS AŞIRI (`p2_loss80`: loss 80% rate 200kbit, çift yön) · N=30 · D=20s

| kol | n | claim | completion(20s) medyan | IQR | CI | outcome | blackout | stall |
|---|---|---|---|---|---|---|---|---|
| kama-v1-full | 30 | final | **0.010** | [0.000,0.530] | [0.000,0.340] | blackout=19 timeout=11 | 30/30 | 30/30 CI[0.884,1.000] |
| quinn-reliable-stream | 30 | final | 0.000 | [0.000,0.010] | [0.000,0.000] | blackout=8 conn-fail=17 timeout=5 | 13/30 | 30/30 CI[0.884,1.000] |
| tcp-default | 30 | final | 0.000 | [0.000,0.000] | [0.000,0.000] | blackout=7 conn-fail=13 timeout=10 | 17/30 | 30/30 CI[0.884,1.000] |

| | kama | quinn | tcp |
|---|---|---|---|
| min / medyan / max | 0.000 / **0.015** / **0.900** | 0.000 / 0.000 / 0.130 | 0.000 / 0.000 / 0.120 |
| <0.50 olan koşu | 22/30 | 30/30 | 30/30 |

`P(kama > quinn) = 0.729` · `P(kama > tcp) = 0.738`.

**Mutlak anlamda ÜÇ KOL DA BAŞARISIZ.** Kama 30 koşunun 22'sinde %50'nin altında. Bu hücre
hiçbir taşıyıcı için çalışır bir işletim noktası DEĞİLDİR; üstünlük görecelidir.

### BULGU T1b-1 — sıfır-ACK soğuk başlangıcında kalıcı durma (ÇİFT MODLU)

Koşular temiz biçimde iki gruba ayrılıyor:

| grup | n | btlbw | srtt | gönderilen veri | retx | dönüş-yolu tanısı |
|---|---|---|---|---|---|---|
| **DÜŞÜK** (completion<0.1) | **18/30** | **0 B/s** | **0 ms** | **1 698 B** / 25 600 B | **0** | `n=0/0 conf=0‰ Unknown dir_ev=false` |
| YÜKSEK (completion≥0.5) | 8/30 | 5 286 B/s | ölçüldü | 28 300 B (tam) | 100–240 | `n=7/7 … Dead` |

Düşük grupta **hiç ACK dönmemiş** (`n=0/0`). Zincir: ACK yok → RTT örneği yok → yeniden-iletim
zamanlayıcısı kurulamıyor → `retx = 0` ve `tail_probe = 0` → akış ilk pencerede (≈1 698 B, ürün
`initial_window_bytes` = 2 400 B ile tutarlı) **kalıcı olarak duruyor** ve 20 s boyunca bir daha
denemiyor. Yalnız 6 duplikasyon çıkmış.

**Sınıflandırma (taksonomi #5'e komşu — ayarlama YAPILMADI):** bu rejim için tasarlanmış mekanizma
mevcut ama ürün profilinde **bilinçli olarak KAPALI**: `bootstrap_rtt = false`,
`bootstrap_fast_ladder = false`, koşu manifestinde `bootstrap_ladder=OFF(kalibre-değil)`
(`crates/client-core/src/runtime.rs:195` bunu kayıtlı bir kapanış kararı olarak belgeliyor).
Mekanizma *yanlışlıkla* kapalı değil, **kalibre edilmediği için** kapalı.

Bu yüzden Faz-5 içinde **açılmadı**: Faz-5 sonucuna bakarak bir ürün default'unu değiştirmek
sözleşmenin açıkça yasakladığı ayarlamadır. **Faz-6 girdisi olarak kaydediliyor:** v1, sıfır-ACK
başlangıçta kalıcı duran bir soğuk-başlangıç yoluyla sevk edilecek mi? Bu bir sahip kararıdır.

Rakipler de aynı hücrede kuruluş kaybediyor (quinn 17/30, TCP 13/30 `connection-failure`), yani
davranış türü karşılaştırılabilir; fark, Kama'nın "el sıkışma yok" mimari iddiası gereği burada
zarif bozulma göstermesinin beklenmesidir.

### Aktivasyon
`parity = 0`, `denied = 0` — T1a ile aynı desen: **iid kayıpta yapı-gate hiç açılmıyor.**
%80 kayıpta bile sıfır parity önerisi. (T1a %50 → 0, T2a gemodel → 22.)

### T1b HÜKMÜ: **KAMA WINS** *(göreli; mutlak anlamda üç kol da başarısız)*
Kama her iki rakibi de geçiyor (P≈0.73, max 0.900 ↔ 0.130) ama 22/30 koşuda %50'nin altında.
Bu hücrenin ürettiği asıl değer bir üstünlük iddiası değil, **BULGU T1b-1**'dir.

---

## T1c — RANDOM LOSS + YÜKSEK RTT (`iid50`: fwd `delay 200ms loss 50%` / rev `delay 200ms loss 10%`) · N=30 · D=20s

| kol | completion(20s) medyan | IQR | CI | ≥0.99 | <0.50 | blackout | stall |
|---|---|---|---|---|---|---|---|
| kama-v1-full | **1.000** | [1.000,1.000] | [1.000,1.000] | 27/30 | 1/30 | 23/30 | 23/30 CI[0.577,0.901] |
| quinn-reliable-stream | **1.000** | [0.980,1.000] | [1.000,1.000] | 22/30 | 1/30 | 27/30 | 28/30 CI[0.779,0.992] |
| tcp-default | 0.830 | [0.480,1.000] | [0.600,1.000] | 11/30 | 8/30 | 30/30 | 30/30 CI[0.884,1.000] |

`P(kama > quinn) = 0.559` (0.5 = fark yok) · `P(kama > tcp) = 0.762`.

### Deadline eğrisi — asıl ayrım BURADA

| D | kama | quinn | tcp |
|---|---|---|---|
| 5 s | **0.960** | 0.590 | 0.200 |
| 10 s | **0.990** | 0.890 | 0.370 |
| 20 s | 1.000 | 1.000 | 0.830 |

Kama ile quinn **20 s'te eşitleniyor**, ama Kama oraya çok daha erken varıyor: 5 s'te 0.96 ↔ 0.59.
Yani üstünlük nihai teslim oranında değil, **teslim zamanında**. Dar deadline'lı taktik
kullanımda bu fark operasyonel olarak anlamlıdır; 20 s'lik bir bütçede ise fark kapanıyor.

### Ön-kayıtlı beklentiye karşı — **DOĞRULANDI**
Ön-kayıt §10.3'te T1c için *"PARİTE bekliyorum, Kama üstünlüğü DEĞİL"* yazmıştım (denetimin
n=6 sonucuna dayanarak: quinn 0.995 / kp-iw2400 0.990). D=20 s'te ölçülen tam olarak budur:
her iki kol da medyan 1.000, `P = 0.559`.

### Aktivasyon
`parity = 0`, `denied = 0` — **üst üste üçüncü iid hücre.** retx=96.5, dup=9.5, recover_arq=21,
srtt=410 ms (enjekte 2×200 ms ile tutarlı), fwd_loss=520‰ (enjekte %50 ✔),
ret_ack_loss=337‰ (enjekte %10'un üstünde — ACK seyrekleşmesiyle birleşiyor).
Tel: kama 28.3 kB data + 27.2 kB retx ≈ **55.5 kB** ↔ quinn **58.3 kB** (Kama %5 daha az).

Kama'nın 1 koşusu 0.010'da kaldı — **BULGU T1b-1** ile aynı imza (sıfır-ACK soğuk başlangıç);
bu rejimde 30 koşuda 1 kez görüldü.

### T1c HÜKMÜ: **COMPARABLE / PARITY**
Birincil deadline'da (20 s) Kama ile quinn ayrışmıyor (`P = 0.559`, iki medyan da 1.000).
Kama TCP'yi açıkça geçiyor ve D ≤ 10 s'te her iki rakibin de önünde; ancak birincil metrik ve
birincil deadline üstünden hüküm **parite**dir. Üstünlük iddiası KURULMAZ.

---

## T2a — BURST LOSS (`h3b_burst50`: fwd `loss gemodel 50% 25% 90% 10% rate 250kbit` / rev `rate 250kbit`) · N=30 · D=20s

| kol | completion(20s) medyan | IQR | CI | ≥0.99 | <0.50 | blackout | stall + CI |
|---|---|---|---|---|---|---|---|
| kama-v1-full | **1.000** | [0.980,1.000] | [0.980,1.000] | 20/30 | **1/30** | **4/30** | **4/30 CI[0.038,0.307]** |
| quinn-reliable-stream | **1.000** | [1.000,1.000] | [1.000,1.000] | 23/30 | 4/30 | 24/30 | 25/30 CI[0.653,0.944] |
| tcp-default | 0.770 | [0.150,1.000] | [0.210,1.000] | 13/30 | 12/30 | 30/30 | 30/30 CI[0.884,1.000] |

`P(kama > quinn) = 0.438` · `P(kama > tcp) = 0.684`.
Deadline eğrisi: **5 s** kama 0.980 / quinn 0.900 — **10 s** 0.990 / 1.000 — **20 s** 1.000 / 1.000.

### İki farklı okuma, ikisi de doğru

* **Birincil metrikte (completion) ayrışma YOK.** İki medyan da 1.000, CI'lar örtüşüyor,
  `P = 0.438` sıra bazında quinn'i hafifçe önde gösteriyor. Kama üstünlüğü İDDİA EDİLMEZ.
* **Hizmet sürekliliğinde ayrışma KESKİN.** Kama 4/30 blackout (CI [0.038,0.307]), quinn 25/30
  stall (CI [0.653,0.944]) — **güven aralıkları ÖRTÜŞMÜYOR.** Kama ayrıca felaket koşusunda da
  daha tutarlı: 1/30 ↔ quinn 4/30 ↔ TCP 12/30.

Yani bu hücrede quinn aynı nihai teslimi **çok daha fazla kesintiyle** elde ediyor.

### T2 SORUSUNUN CEVABI — `parity_denied ≫ parity_emitted`

Ölçülen (medyan): `parity_emitted = 19`, `parity_token_denied = 1560` (≈82:1),
`parity_bytes = 19 988 B`, `retx = 143`, `dup = 32.5`.

**Sözleşmenin A şıkkı: yüksek ret + GÜÇLÜ teslim → sınırlayıcı/koruyucu tahkim.**

Destekleyen kanıt:
1. Teslim zaten tavanda (medyan 1.000) — bastırılmış bir teslim açığı YOK.
2. Kama üç kolun **en düşük blackout insidansına** sahip (4/30 ↔ 24/30 ↔ 30/30) — kesinti
   yönünden de açlık belirtisi yok.
3. Tel maliyeti: kama 28.3 kB data + 38.1 kB retx + 20.0 kB parity ≈ **86.4 kB** ↔ quinn
   **74.8 kB**. Kama bu hücrede quinn'den **%16 DAHA FAZLA** tel harcıyor. Ret olmasaydı fark
   büyürdü; oysa kazanılacak completion başlığı yok (zaten 1.000).

**Beyan edilen çıkarım sınırı:** `parity_token_denied` *pacer token reddi* sayar, "engellenen
faydalı onarım" değil. Karşı-olgusal ("ret olmasaydı ne olurdu") bu veriden KANITLANAMAZ.
Sonuç, üç dolaylı kanıtla **desteklenmektedir**, ispatlanmış değildir.

### Aktivasyon deseni doğrulandı
Bu, parity'nin **fiilen ateşlediği ilk hücre** (19 emisyon, 20 kB). iid hücrelerde (T1a %50,
T1b %80, T1c %50+RTT) yapı-gate hiç açılmamıştı. **Desen: gate korelasyonlu/gemodel kayıpta
açılıyor, bağımsız kayıpta açılmıyor** — `Q18Iid` yapı politikasının bir IID-yapı dedektörü
olmasıyla tutarlı. Ayarlama YAPILMADI.

### T2a HÜKMÜ: **COMPARABLE / PARITY**
Birincil metrikte quinn ile ayrışma yok (`P = 0.438`, örtüşen CI). Kama TCP'yi geçiyor ve
**hizmet sürekliliğinde quinn'i belirgin biçimde geçiyor** (örtüşmeyen stall CI'ları), ancak
hüküm birincil metrik üzerinden **parite**dir.

---

## T2b — BURST LOSS / KORELASYON (`burst50`: fwd `delay 200ms loss 50% 50%` / rev `delay 200ms loss 10%`) · N=30 · D=20s

> Bu hücre, denetimin n=6 koşusunda kurulan **"Kama burst'te quinn'i yeniyor"** iddiasının
> ön-kayıtlı doğrulama yeridir (EXPECTATION §10.4: *"Kama üstünlüğü bekliyorum. Bu, Faz-5'in
> doğrulamakla yükümlü olduğu tek önceden iddia edilmiş üstünlüktür."*).

| kol | completion(20s) medyan | IQR | CI | ≥0.99 | <0.50 | blackout | stall + CI |
|---|---|---|---|---|---|---|---|
| kama-v1-full | **1.000** | [0.970,1.000] | [0.990,1.000] | 21/30 | 3/30 | 21/30 | 21/30 CI[0.506,0.853] |
| quinn-reliable-stream | **1.000** | [0.790,1.000] | [0.970,1.000] | 19/30 | 5/30 | 22/30 | 24/30 CI[0.614,0.923] |
| tcp-default | 0.430 | [0.140,1.000] | [0.300,0.920] | 8/30 | 15/30 | 27/30 | 29/30 CI[0.828,0.999] |

`P(kama > quinn) = 0.533` · `P(kama > tcp) = 0.751`.
Deadline eğrisi: **5 s** kama 0.940 / quinn 0.660 — **10 s** 0.980 / 0.940 — **20 s** 1.000 / 1.000.

### ⚠ ÖN-KAYITLI BEKLENTİ **YANLIŞLANDI**

| | denetim (n=6, ön-Faz-4) | Faz-5 (N=30, donmuş metodoloji) |
|---|---|---|
| quinn | 0.895 | **1.000** |
| Kama | 0.995 (`kp-iw2400`) | **1.000** |
| ayrışma | "Kama üstün" | **YOK** (`P = 0.533`, örtüşen CI) |

**Denetimin n=6 sonucundaki Kama üstünlüğü N=30'da YENİDEN ÜRETİLEMEDİ.** Bu hücrede iki kol
paritededir. Sebep büyük olasılıkla küçük örneklem: quinn'in N=30 IQR'si [0.790, 1.000] kadar
geniş; 6 örneklik bir medyan bu dağılımdan kolayca 0.895'e düşer.

Ön-kayıt §11'de bu durumu önceden şöyle taahhüt etmiştim: *"N≥30'da düşerse beklenti
yanlışlanmış sayılır ve rapor bunu açıkça yazar."* **Yazılıyor: iddia geri çekilmiştir.**
Hücre tanımı, deadline, N kuralı ya da metrik sonucu kurtarmak için DEĞİŞTİRİLMEDİ.

### Yine de ölçülen gerçek: yakınsama hızı
D = 5 s'te kama 0.940 ↔ quinn 0.660. Kama tavana çok daha erken varıyor; 20 s'lik bütçede fark
kapanıyor. T1c'deki desenin aynısı — üstünlük **nihai teslimde değil, teslim zamanında**.
Bu bir *üstünlük iddiası değil*, ölçülmüş bir davranış farkıdır.

### Aktivasyon
`parity = 1`, `denied = 141`, retx=86.5, recover_arq=21.5, srtt=407 ms, fwd_loss=498‰ (enjekte %50 ✔).
Tel: kama 28.3 kB + 24.1 kB retx + 1.1 kB parity ≈ **53.5 kB** ↔ quinn **49.7 kB** (Kama %8 fazla).

**T2a ile karşıtlık kaydı:** aynı "burst" ailesinde netem `gemodel` (T2a) yapı-gate'i açtı
(parity 19, denied 1560), netem `loss 50% 50%` korelasyonu (T2b) neredeyse hiç açmadı
(parity 1, denied 141). Kayıp *modeli* kodlama kararını belirgin biçimde değiştiriyor.
Sınıflandırıldı, ayarlanmadı.

### T2b HÜKMÜ: **COMPARABLE / PARITY**
quinn ile ayrışma yok (`P = 0.533`, örtüşen CI, iki medyan da 1.000). TCP'ye karşı Kama açık
üstün (medyan 1.000 ↔ 0.430, `P = 0.751`). **Önceki üstünlük iddiası bu hücrede geri çekildi.**

---

## T3a — DÜŞÜK BANT (`h2_cell`: `delay 300ms 200ms distribution normal loss 25% rate 8kbit`, çift yön) · N=30 · D=20s · geo 30×64B

| kol | completion(20s) medyan | min | CI | ≥0.99 | <0.50 | blackout | stall + CI |
|---|---|---|---|---|---|---|---|
| kama-v1-full | **1.000** | **0.967** | [1.000,1.000] | **29/30** | **0/30** | 28/30 | 28/30 CI[0.779,0.992] |
| quinn-reliable-stream | **1.000** | 0.000 | [0.967,1.000] | 18/30 | 5/30 | 30/30 | 30/30 CI[0.884,1.000] |
| tcp-default | **1.000** | 0.067 | [1.000,1.000] | 22/30 | 7/30 | 30/30 | 30/30 CI[0.884,1.000] |

`P(kama > quinn) = 0.688` · `P(kama > tcp) = 0.621`.
Deadline eğrisi: **5 s** kama 0.900 / quinn 0.200 / tcp 0.233 — **10 s** 1.000 / 0.933 / 0.433 —
**20 s** 1.000 / 1.000 / 1.000.

### Ön-kayıtlı RİSK gerçekleşmedi — ve sebebi ölçülebilir

Ön-kayıt §10.6'da açıkça yazmıştım: *"Kama'nın parity'si dar bandı israf ederse Kama
KAYBEDEBİLİR — bu gerçek bir risk olarak kaydediliyor."*

**Gerçekleşmedi.** Sebep telemetride görünüyor: `parity_emitted = 0`, `parity_bytes = 0` —
yapı-gate bu hücrede hiç açılmadı, dolayısıyla dar banda **hiç parity harcanmadı**.

Tel maliyeti: kama 2 730 B data + 910 B retx = **3.64 kB** ↔ quinn **6.36 kB** ↔ tcp 2.28 kB
(uygulama katmanı, kıyaslanamaz). Kama, quinn'den **%43 daha az tel** harcadı.

**Bağlantı kaydı:** T1'de "iid kayıpta gate hiç açılmıyor" bir eksiklik gibi görünüyordu; T3a'da
aynı davranış **koruyucu** çıktı — dar bantta kodlama israfını engelledi. Aynı mekanizmanın iki
rejimde zıt işaretli sonuç vermesi, kodlama politikasının Faz-6'da tek bir "iyi/kötü" etiketiyle
değerlendirilemeyeceğini gösterir. Ayarlama YAPILMADI.

### §8.5 / §11 çelişki köşesi
`would_zero = 0` (btlbw medyan 651 B/s ≫ 124 B/s köşesi) — ön-kayıtlı beklentiyle uyumlu,
bu hücrede spec çelişkisine GİRİLMEDİ.

### Aktivasyon
`parity=0 denied=0 retx=10 dup=1.5 probes=0 recover_arq=7` ·
srtt=2193 ms (300 ms gecikme + 8 kbit kuyruk serileşmesi) · btlbw=651 B/s ·
fwd_loss=350‰ (enjekte %25 üzerinde — jitter'lı yolda muhafazakâr tahminci) ·
ret_ack_loss=368‰.

### T3a HÜKMÜ: **COMPARABLE / PARITY**
Birincil deadline'da üç medyan da 1.000 ve CI'lar örtüşüyor → medyan üstünlüğü İDDİA EDİLMEZ.
Ancak dağılım Kama lehine belirgin: **en kötü koşu 0.967 ↔ 0.000 / 0.067**, felaket koşusu
**0/30 ↔ 5/30 ↔ 7/30**, ve D ≤ 10 s'te Kama her iki rakibin de çok önünde. Bu üstünlük
CI-ayrık DEĞİLDİR (0/30 CP[0,0.116] ile 5/30 CP[0.056,0.347] hafif örtüşür), bu yüzden
hüküm parite olarak bırakılmıştır — gözlem güçlü ama istatistiksel olarak ayrık değil.

---

# ⚠ DÜZELTME (analiz hatası — ölçüm değil): "yapı-gate hiç açılmıyor" İFADESİ YANLIŞTI

T1a, T1c ve T3a bölümlerinde `parity_emitted = 0 ∧ parity_token_denied = 0` gözlemini
*"yapı-gate hiç açılmadı"* diye yorumlamıştım. **Bu yorum yanlıştır.** Manifest bu ayrımı
taşımaz; ayrımı yalnız istemci stderr'indeki `clamp` sayacı gösterir ve o sayaç ancak
`final_ratio > 0` iken artar — yani **parity ÖNERİLDİĞİNDE**.

| hücre | clamp (medyan) | would_zero | parity_emitted | denied | gerçek durum |
|---|---|---|---|---|---|
| T1a | **10 428** | 0 | 0 | 0 | parity ÖNERİLDİ, §8.5 kısıtladı, hiç maddeleşmedi |
| T1b | 0 | 0 | 0 | 0 | gerçekten hiç karar yok (sıfır-ACK durması, BULGU T1b-1) |
| T1c | **12 230** | 0 | 0 | 0 | parity ÖNERİLDİ, kısıtlandı, maddeleşmedi |
| T2a | 5 525 | 0 | **19** | 1 560 | önerildi, kısıtlandı, KISMEN maddeleşti |
| T2b | 12 982 | 0 | 1 | 141 | önerildi, kısıtlandı, neredeyse hiç maddeleşmedi |
| T3a | **7 486** | 0 | 0 | 0 | parity ÖNERİLDİ, kısıtlandı, maddeleşmedi |

## BULGU FEC-1 — kodlama önerisi maddeleşmeden kayboluyor (sessiz)

Altı hostile hücrenin beşinde arbiter parity'yi **binlerce kez önerdi** (clamp 5 500–13 000) ve
§8.5 bant-türevli bütçe kısıtı öneriyi küçülttü. T1a / T1c / T3a'da sonuç **sıfır parity çerçevesi**
oldu — ne emisyon ne de ret.

**Çıkarım (kanıtla desteklenen, ispatlanmamış):** küçültülmüş oran, K=8'de **tek bir onarım
sembolü üretmeye yetmeyecek** kadar düşük kalıyor (`r` 0'a yuvarlanıyor), bu yüzden hiç parity
çerçevesi kurulmuyor — kurulmadığı için pacer'ın reddedeceği bir şey de olmuyor
(`denied = 0`). Bu, kayıtlı `q18-fec-gate-defect` monoton-olmayan ölü-bölge desenine uyar.
Alternatif bir aşağı-akış kapısı da aynı imzayı üretebilir; bu veriden ayırt EDİLEMEZ.

**Bu bir telemetri boşluğudur:** aktivasyon manifesti *"hiç önerilmedi"* ile *"önerildi ama
maddeleşmedi"* durumlarını AYIRT EDEMEZ — ikisi de `parity=0, denied=0` görünür. Faz-3'ün
"yapılandırıldı ≠ ateşledi" ayrımının bir kademe altında aynı sınıf boşluk var.

**Yönü tek değil:** T3a'da bu davranış KORUYUCU çıktı (dar banda parity israfı yok, Kama
quinn'den %43 az tel harcadı); T1a/T1c'de ise kodlama alt-sistemi rejime hiçbir katkı yapmadı.
Aynı mekanizma iki rejimde zıt işaretli.

**Faz-5 eylemi: YOK.** Bu hücrelerde Kama kaybetmiyor (T1a DOMINATES, T1c/T3a PARITY), dolayısıyla
taksonomi tetiklenmiyor ve ayarlama sözleşmece yasak. **Faz-6 girdisi olarak kaydedildi:**
(a) §8.5 kısıtı + `r` granülerliği hostile rejimlerin geniş bir sınıfında FEC'i fiilen devre dışı
bırakıyor mu, (b) aktivasyon manifesti bu ayrımı taşımalı mı?

Yukarıdaki T1a / T1c / T3a bölümlerindeki *"gate hiç açılmadı"* cümleleri bu bölümle
**SUPERSEDE** edilmiştir. Ölçümler değişmedi; değişen yalnız benim yorumumdur.

---

## T4 — YÜKSEK GECİKME (`h6_rtt200`: `delay 100ms loss 1% rate 10mbit`, çift yön) · N=30 · D=20s

| kol | completion(5/10/20 s) | min | CI | outcome | blackout | stall |
|---|---|---|---|---|---|---|
| kama-v1-full | **1.000 / 1.000 / 1.000** | 1.000 | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| quinn-reliable-stream | **1.000 / 1.000 / 1.000** | 1.000 | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| tcp-default | **1.000 / 1.000 / 1.000** | 1.000 | [1.000,1.000] | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |

`P(kama > quinn) = 0.500` · `P(kama > tcp) = 0.500` — **tam eşitlik**, üç kolun da 30/30 koşusu
her deadline'da kusursuz.

### Ön-kayıtlı beklentiye karşı
Ön-kayıt §10.7: *"Parite bekliyorum; cold-start Kama'yı ilk saniyede geriletebilir."*
Parite **doğrulandı**; cold-start endişesi **gerçekleşmedi** — Kama D = 5 s'te bile 1.000
(min 1.000, 30/30). 200 ms RTT tek başına, %1 kayıpla birlikte, hiçbir kolu zorlamıyor.

Bu hücre pratikte **ikinci bir kontrol koşuludur**: yüksek RTT'nin *tek başına* düşman
olmadığını, ancak kayıpla birleştiğinde (T1c, T2b) ayrıştırıcı olduğunu gösterir.

### T4 HÜKMÜ: **COMPARABLE / PARITY**

---

## T5 — ASİMETRİK YOL (`r_returnpath R3`: fwd `delay 100ms rate 1mbit` / rev `delay 100ms rate 64kbit`) · N=30 · D=20s

| kol | completion(5/10/20 s) | min | outcome | blackout | stall |
|---|---|---|---|---|---|
| kama-v1-full | 1.000 / 1.000 / 1.000 | 1.000 | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| quinn-reliable-stream | 1.000 / 1.000 / 1.000 | 1.000 | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |
| tcp-default | 1.000 / 1.000 / 1.000 | 1.000 | completed=30 | 0/30 | 0/30 CI[0.000,0.116] |

`P = 0.500` her iki rakibe karşı. Kama tel: 28.3 kB (retx 0, parity 0) ↔ quinn 35.4 kB.

### ⚠ ÖN-KAYITLI HÜCRE ASİMETRİ EKSENİNİ ZORLAMADI — kaydedilen test-tasarımı sınırı

Sözleşme T5 için şunu şart koşuyor: *"uygulanan ileri bozulma ≠ uygulanan dönüş bozulması **VE**
runtime telemetrisi tek bir simetrik vekil yerine yönlü gözlem göstermeli."*

**Birinci şart sağlandı:** gerçekleşen netem `fwd: rate 1Mbit` ↔ `rev: rate 64Kbit` (arşivli).

**İkinci şart bu hücrede GÖSTERİLEMEDİ.** Kama'nın dönüş-yolu tanısı, **simetrik** T4 hücresiyle
neredeyse birebir aynı:

| | T5 (asimetrik 1Mbit/64Kbit) | T4 (simetrik 10Mbit/10Mbit) |
|---|---|---|
| ölçülen dönüş bw | 281 B/s | 280 B/s |
| ACK aralığı | 113 ms | 114 ms |
| `ret_impair` | 0‰ | 0‰ |
| health | Healthy (n=51/51) | Healthy (n=51/51) |

**Sebep ölçülebilir ve bu bir kablolama hatası DEĞİLDİR:** dönüş yolunda fiilen akan ACK trafiği
**281 B/s**, dönüş kapasitesi ise 64 kbit = **8 000 B/s**. Kullanım oranı **≈ %3.5**. Dönüş yolu
hiç doyuma girmedi, dolayısıyla gözlenecek bir asimetri sinyali OLUŞMADI. Telde var olan
asimetri, bu yükte operasyonel olarak görünmezdir.

**Kablolamanın sağlam olduğunun kanıtı başka hücreden gelir:** Aşama A'da T6'da dönüş yolunun
%82'si kaybolurken ileri kayıp tahmini 68‰'de kaldı (`ret_ack_loss=824‰` ↔ `fwd_loss=68‰`) —
yön ayrımı, gözlenecek bir sinyal VARKEN çalışıyor. Sözleşmenin yönlü-gözlem şartı bu yüzden
**T6 tarafından karşılanmaktadır, T5 tarafından değil.**

Hücre tanımı sonuçlara bakıldıktan sonra **DEĞİŞTİRİLMEDİ** (sözleşme yasağı). Sınır olduğu gibi
kaydedilmiştir: *ileri/dönüş **bant** asimetrisini zorlayan bir hücre, ACK trafiğini dönüş
kapasitesine yaklaştıracak bir yük geometrisi gerektirir* — mevcut 100×256 B yükü bunu yapmıyor.
Faz-6 / gelecek kampanya girdisi.

### T5 HÜKMÜ: **COMPARABLE / PARITY** *(asimetri ekseni için ZAYIF kanıt)*
Kıyas geçerlidir ve parite üretmiştir; ancak bu hücre **"Kama asimetrik yolları iyi yönetiyor"
iddiasını DESTEKLEMEZ** — çünkü hiçbir kolu zorlamamıştır. Böyle bir iddia bu veriden KURULMAZ.
