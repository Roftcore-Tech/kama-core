# ERRATUM-2 — `recover_fwd` sayımı: "0 (330/330)" yanlış, ham veride 15/330

Durum: **DÜZELTİLDİ (belge düzeltmesi).** Hiçbir ölçüm geçersiz kılınmadı, hiçbir koşu yeniden
koşulmadı. Yanlış olan, ham veriden okunmuş gibi raporlanan bir sayıdır.
Tespit: K4G sprinti, 2026-10-18. Sahip kararı: CEVAP PAKETİ 7, DUR-5 (a).
Etkilediği yer: Faz-5 Aşama B (T0–T7) Kama kolu aktivasyon beyanı → LIM-006, engine-spec
`[AMENDMENT-3]` sınır (b), Faz-6 kapanış belgeleri. Aynı PR'da PHASE5 raporundaki süre etiketi de
düzeltildi (§6).

---

## 1. Yanlış iddia

> "Faz-5 T0–T7, 330/330 Kama koşusu: `recover_fwd_entries = 0`." · "§11 yolu hiçbir doğrulama
> hücresinde ateşlemedi."

İddia `53e0c5c6` (PR #52, Faz 1–6 kapanışı) ile girdi; `9ec8c625` (PR #60) deney defterine taşıdı.
Geçtiği yerler (taban `3439fccd`). Her birine bu PR'da `ERRATUM-2 (2026-10-18)` bandı eklendi;
yanlış ifade silinmedi, üstü çizildi:

| dosya | satır |
|---|---|
| `specs/adaptive-transport-engine-spec.md` | 852–853 (`[AMENDMENT-3]` sınır (b)) |
| `docs/transport-v1/KNOWN-LIMITATIONS.md` | 56 (LIM-006 İfade); 60 (Kanıt — yalnız bant eklendi, metin çizilmedi) |
| `docs/transport-v1/EVOLUTION.md` | 60–61 (§9.6) |
| `docs/transport-v1/HOSTILE-ENVELOPE.md` | 43 (§4) |
| `docs/transport-v1/PRODUCT-PROFILE.md` | 51 |
| `docs/transport-v1/adr/ADR-minimal-parity-exception.md` | 64–65 (§Evidence) |
| `docs/transport-v1/ARCHITECTURE.md` | 94 (§4.7) |
| `reports/transport-v1/FINAL-MECHANISM-AUDIT.md` | 25 |
| `reports/transport-v1/TRANSPORT-V1-CLOSURE.md` | 65 (§6) |
| `docs/DENEY-DEFTERI.md` | 120, 194, 207 |

`PHASE5-T0-T7-REPORT.md` bu iddiayı taşımaz; ona yalnız başlıkta ERRATUM-2 bağlantısı ve süre
etiketi düzeltmesi (bu belgenin §6'sı) eklendi. Kimlik ve kanoniklik cümleleri ("330/330 Kama
koşusu `kama-v1-full`"; taban satırları PHASE5:146, :217 ve DENEY-DEFTERI:168) DOĞRUDUR ve
değiştirilmedi.

## 2. Doğrusu: 330 Kama koşusunun 15'inde `RecoverFwd` girişi var

Sayaç: `ActivationDiag::recover_fwd_entries`. Gönderici `SenderState::RecoverFwd`'e her geçtiğinde
bir artar (kampanya SHA'sı `e3c1e8bb`'de `crates/adaptive-transport/src/sender.rs:4099`,
`update_state` içi). Ürün derlemesinde istemci stderr'ine şu satırda yazılır:
`KP-ACTIVATION … reach=[transitions=… recover_arq=… recover_fwd=N …]`
(`crates/bench/src/kama_product.rs:408`). Her `c.err` dosyasındaki SON `recover_fwd=` değeri
okundu.

Hücre bazında:

| hücre | T0 | T1a | T1b | T1c | T2a | T2b | T3a | T4 | T5 | T6 | T7 | toplam |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `recover_fwd` > 0 | 0/30 | **2/30** | **7/30** | 0/30 | 0/30 | 0/30 | 0/30 | 0/30 | 0/30 | **6/30** | 0/30 | **15/330** |

Koşu listesi:

| hücre | koşu | `recover_fwd` | son state | parity (tel) | parity_denied | dup |
|---|---|---|---|---|---|---|
| T1a | rep1 | 2 | `RecoverFwd` | 0 | 0 | 13 |
| T1a | rep26 | 3 | `RecoverFwd` | 0 | 0 | 3 |
| T1b | rep4 | 1 | `Startup` | 0 | 0 | 36 |
| T1b | rep7 | 1 | `Steady` | 0 | 0 | 81 |
| T1b | rep16 | 1 | `RecoverFwd` | 0 | 0 | 1 |
| T1b | rep17 | 1 | `Steady` | 0 | 0 | 276 |
| T1b | rep20 | 1 | `DrainRtprop` | 0 | 0 | 684 |
| T1b | rep27 | 2 | `RecoverArq` | 12 | 6861 | 95 |
| T1b | rep30 | 1 | `Steady` | 6 | 2251 | 228 |
| T6 | rep5 | 1 | `RecoverFwd` | 0 | 0 | 0 |
| T6 | rep13 | 1 | `RecoverFwd` | 6 | 559 | 41 |
| T6 | rep14 | 1 | `RecoverFwd` | 9 | 1152 | 0 |
| T6 | rep23 | 1 | `RecoverFwd` | 4 | 33 | 20 |
| T6 | rep26 | 1 | `RecoverFwd` | 9 | 2133 | 6 |
| T6 | rep29 | 1 | `RecoverFwd` | 9 | 444 | 28 |

Sütunlar: `parity` ve `parity_denied` = `KP-ACTIVATION fired=[… parity=… parity_denied=…]` (tele
çıkan ve pacer'ın reddettiği parity; kaynak ayrımı YOK). `dup` = aynı satırdaki dup emisyon sayısı
(PreConverge kopyası ile RecoverFwd D× kopyası AYRILMAZ). `son state` = `KP-CLIENT … state=`
(akış sonu anlık görüntüsü; 9/15 koşu `RecoverFwd`'de bitti).

Kimlik: 330/330 `c.err` `commit=e3c1e8bb740bbc25acd2c9fc5a7fc478a3f15adc` ve
`DELTA-vs-v1: (yok …)` satırını taşıyor. Kanonik `kama-v1-full` beyanı bu düzeltmeden etkilenmez.

## 3. Ham veri ve doğrulama komutu

Ham veri (salt okundu, bu PR'da değişmedi):

```
/home/ryuzaki/kama-phase5-results/stageB/<hücre>/kama-product/rep<N>/c.err          sayacın tek kaynağı
/home/ryuzaki/kama-phase5-results/stageB/<hücre>/kama-product/rep<N>/manifest.json  alan YOK (§4)
/home/ryuzaki/kama-phase5-results/campaign.out                                       kampanya başlığı ve süresi (§6)
```

Doğrulama komutu (2026-10-18'de koşuldu):

```
python3 -I -c '
import glob, re, collections
R = "/home/ryuzaki/kama-phase5-results/stageB"
fs = sorted(glob.glob(R + "/*/kama-product/rep*/c.err"))
pos = collections.defaultdict(list)
for f in fs:
    n = int(re.findall(r"recover_fwd=(\d+)", open(f, encoding="utf-8").read())[-1])
    if n > 0:
        p = f.split("/")
        pos[p[-4]].append(int(p[-2][3:]))
print(len(fs), "dosya;", sum(map(len, pos.values())), "koşuda recover_fwd > 0")
for c in sorted(pos):
    print(c, len(pos[c]), sorted(pos[c]))
'
```

Çıktı:

```
330 dosya; 15 koşuda recover_fwd > 0
T1a 2 [1, 26]
T1b 7 [4, 7, 16, 17, 20, 27, 30]
T6 6 [5, 13, 14, 23, 26, 29]
```

## 4. Yanlış sayının kaynağı (belirlenebildiği kadar)

* LIM-006 kanıt olarak "Faz-5 manifest taraması" gösteriyordu. `manifest.json` içindeki
  `activation` nesnesinde `recover_fwd_entries` alanı **yoktur**: şema
  (`crates/bench/src/manifest.rs:41` `ActivationSummary`, kampanya SHA'sında da) yalnız
  `recover_arq_entries`'i taşır. 330 manifestin hiçbirinde alan yok. Manifest taraması bu sayıyı
  üretemezdi.
* Sayaç yalnız `c.err`'dedir ve orada 15/330 koşuda sıfırdan büyüktür.
* İddiayı üreten tarama komutu ya da çıktısı repoda kayıtlı değil. Mekanizma (eksik alanın 0
  okunması, yanlış dosyanın taranması ya da başka bir şey) bu yüzden kesin olarak belirlenemedi.
  Gözlenen desen, eksik alanın sıfır sayılması tuzağıyla (vakum sayaç) tutarlıdır; bu
  kanıtlanmamıştır.

Manifest için doğrulama komutu:

```
python3 -I -c 'import glob, json; fs = glob.glob("/home/ryuzaki/kama-phase5-results/stageB/*/kama-product/rep*/manifest.json"); print(len(fs), "manifest;", sum("recover_fwd_entries" in json.load(open(f))["activation"] for f in fs), "tanesinde alan var")'
```

Çıktı: `330 manifest; 0 tanesinde alan var`

## 5. Bu 15 koşuda §11 parity emisyonu oldu mu

Kısa cevap: **8 koşuda olmadı (kesin). 7 koşuda ölçülmedi (ayrıştırılamaz).**

* §11 MinimalParity ayrı bir emitör değildir. `fec_controller_latch`'in emergency dalında NORMAL
  oranın yerine geçer (`[AMENDMENT-3]`) ve tek emisyon noktasından (`emit_parity_at`,
  `sender.rs:5224`) tele çıkar. Bu nokta her parity'de `parity_emitted`'i artırır
  (`sender.rs:5231`); kaynağa göre ayrı sayaç YOKTUR. Emergency seviyesi (hysteresis
  `Arq`/`Dup2`/`Dup3`) ve fuzzy modu `c.err`'e yazılmaz.
* 15 koşunun 8'inde `parity = 0` ve `parity_denied = 0`. Bu koşularda tele hiç parity çıkmadı,
  dolayısıyla §11 MinimalParity emisyonu da olmadı.
* 7 koşuda `parity > 0`: T1b rep27 (12), rep30 (6); T6 rep13 (6), rep14 (9), rep23 (4), rep26 (9),
  rep29 (9). Bu parity'nin §11 emergency dalından mı NORMAL arbiter yolundan mı geldiği `c.err`'den
  ayrıştırılamaz. **Bu koşularda §11 parity emisyonu ÖLÇÜLMEDİ.**
* Bağlam: Kama kolunda parity > 0 olan 120 koşunun 113'ünde `recover_fwd = 0`. Parity'nin varlığı
  §11'in göstergesi değildir.
* `RecoverFwd` girişi ile §11 MinimalParity aynı şey değildir. Kod (`update_state`, kampanya SHA'sı)
  `RecoverFwd`'e hysteresis seviyesi `> Arq` **VEYA** fuzzy mod `EmergencyForwardRedundancy`
  olduğunda girer. MinimalParity ise ikisinin **BİRLİKTE** sağlanmasını (artı SegmentBlock
  framing) ister. 15/330, iki koşuldan en az birinin akış içinde en az bir kez doğru olduğunu
  gösterir. Hangisinin doğru olduğu `c.err`'den ayrışmaz.

## 6. Süre etiketi düzeltmesi (PHASE5 §4)

`PHASE5-T0-T7-REPORT.md:54` şöyle yazıyordu: "Aşama A: 132 koşu (smoke, kanıt değil). Süre:
2026-09-08T15:02:57Z → 20:23:22Z (**5 sa 20 dk**)." Cümle sırası bu süreyi Aşama A'ya bağlıyor.
Bu yanlıştır:

* 15:02:57Z → 20:23:22Z (5 sa 20 dk 25 sn) **tam Aşama B kampanyasının** süresidir. Kanıt:
  `campaign.out` başlığı `== FAZ-5 KAMPANYA reps=30 D=20s maxpar=1 hücreler=T0 T1a T1b T1c T2a T2b
  T3a T4 T5 T6 T7 ==`, ardından `başlangıç: 2026-09-08T15:02:57Z` ve `bitiş: 2026-09-08T20:23:22Z`
  satırları ve 11/11 `DONE`. Diskte 11 hücre × 3 kol × 30 tekrar = 990 koşu dizini var. T3b
  sondası (5 koşu) bu kampanyanın dışındadır.
* Aşama A'nın gerçek süresi **8 dk 16 sn**'dir (2026-09-08T13:43:17Z → 13:51:33Z; 11 × 3 × 4 = 132
  koşu, 4 eşzamanlı shard). Kaynak: `phase5/STAGE-A-VALIDATION.md:8`.

Zaman damgaları doğruydu ve değişmedi. Yalnız etiket düzeltildi, eski etiketin üstü çizildi.

## 7. Etki

* **LIM-006** "§11 yolu hiç ateşlemedi" iddiası artık şöyle okunur: **"T0–T7'de 15/330 koşuda
  `RecoverFwd`'e giriş var (T1a 2, T1b 7, T6 6); §11 MinimalParity emisyonu ayrı sayaçla
  ölçülmedi."**
* **Değişmeyenler:** "saha-doğrulanmamış" nitelemesi (LIM-006 başlığı, PRODUCT-PROFILE, README)
  ve MinimalParity istisnasının tel doğrulaması (`tests/tv_emergency.rs`). HOSTILE-ENVELOPE §4'teki
  "OUTSIDE" sınıfı değişmez.
* **Ön-kayıt sapması.** Kayda geçirilir, geriye dönük düzeltilmez. EXPECTATION §9 T1a/T1b/T1c için
  "emergency (ack_conf yüksek kalır)" mekanizmasını ateşlemesi BEKLENMEYEN olarak yazmıştı.
  `RecoverFwd`'e giriş emergency koşulunun sağlandığını gösterdiği için (§5) T1a 2/30 ve T1b 7/30
  bu beklentiden sapmadır. T1c 0/30 beklentiyle uyumludur.
* Hücre hükümleri, completion medyanları, CI'lar, kanonik profil (330/330 `kama-v1-full`) ve
  ERRATUM-1 kapsamı ETKİLENMEZ. Bu hükümlerin hiçbiri `recover_fwd` sayacına dayanmıyor.

## 8. Tasarım etkisi

YOK. K4G-05b sahiplendirmesi `RecoverFwd`'e dayanmıyor (sahip kararı, CEVAP PAKETİ 7: "Tasarımı
değiştirmez: sahiplendirme zaten RecoverFwd'e dayanmıyor."). Bu PR kod, test, vektör, Cargo,
Faz-5 betikleri ve ham veriye dokunmaz.
