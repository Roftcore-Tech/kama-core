# S-ANSWERS-STRUCT-GATE — kaynak tabanlama sonuçları

**Statü:** `ANSWERED` · **Taban:** `transport-baseline-closure` @ `68d5ec02` (S-0.7 düzeltmesi sonrası `359ca287`)
**Yetki:** `SOURCE-GROUNDING-STRUCT-GATE`'in tanımladığı tek yetkili iş. Prereg-A `DUR-B`, overlay `NO-GO` — ikisine de dokunulmadı.
**Yöntem:** salt okuma. Hiçbir cargo/rustc/test/kampanya koşulmadı; aritmetik kontroller python3 ile.

## Disposition özeti

| soru | disposition | tetiklenen ön-beyanlı dal |
|---|---|---|
| S-0.1–0.5 | `VERIFIED` | `ESTIMATOR-PRODUCT-PRESENT — OBSERVATION-ONLY` |
| **S-0.7** | **`FALSIFIED` → düzeltildi (`359ca287`)** | ingest sadık değildi |
| **S-0.5 niteleme** | **`FALSIFIED`** | yaşam döngüsü eksik — aşağıda |
| S-1.1 | `VERIFIED` | DUR-B tetiklenmedi |
| S-1.2 | `VERIFIED` | iki ayrık sıfır-parity yolu |
| **S-1.3** | **`FALSIFIED`** | **`rule_fired` YANLIŞ İNVARYANT; bant kapısı zorunlu** |
| **S-1.4** | **`FALSIFIED`** | (Z) dalı **yetersiz** |
| S-1.5 | `NOT-DERIVABLE` | sınır **icat edilmedi**; manda S-7'nin bandıyla sınırlı |
| **S-2.1** | **`FALSIFIED`** | `bw HIGH` gerekmiyor — ama sebebi beklenen değil |
| **S-2.2** | `VERIFIED` | **`LOSS_MAX` skaler OLAMAZ → yeni tasarım turu** |
| S-2.3 | `VERIFIED` | üç canlı antecedent + aşama-2 kuantizer |
| S-2.4 | `NOT-DERIVABLE` | `LOSS_MAX` sembolü kaynakta **yok** |
| S-7.1–7.3 | `VERIFIED` | lineage bulundu (DUR-B değil) |
| **S-7.4** | **`FALSIFIED`** | **fayda, overlay'in davranabileceği rejimde ÖLÇÜLMEMİŞ** |
| S-3, S-4, S-5, S-6a | `VERIFIED` | α=1; W=256 tam eşitlik; 8/32; half-up tamsayı |
| S-6b | `NOT-DERIVABLE` | `R2_RATIO` kaynakta yok |
| **S-8** | **`FALSIFIED`** | **`N` karar anında türetilemez → `BLOCKED-PENDING-INSTRUMENTATION`** |
| **S-9** | **`FALSIFIED`** | `(loss, d)` çifti ortak örneklemden **değil** |
| S-10 | `VERIFIED` | bant eşikleri sayısallaştı |

---

## En ağır bulgu — estimator ürün yolunda DEJENERE

`advance()`'ın **sıfır ürün çağıranı** var (14 kullanımın hepsi `fate_estimator.rs`'in kendi test modülünde).

```
on_loss_evidence  ->  provisional
advance(now)      ->  guard dolunca provisional -> finalized     <-- ÜRÜN YOLUNDA YOK
snapshot()        ->  YALNIZ finalized okur
```

Üründe her kayıp **provisional'da kalıyor**; `finalized`'a yalnız `on_original_ack` (başarı) yazıyor.

**Sonuç:** ürün penceresi yalnız başarılardan oluşur ⇒ `n10 + n11 = 0` yapısal olarak ⇒ `eligible()`'ın
`n10 + n11 >= 8` koşulu **üründe asla sağlanamaz** ⇒ `burst_delta` dejenere.

**Bu, `loss_structure_observation::estimator_observes_real_loss` testinin boş geçtiğini de gösteriyor:**
test `f.n > 0` iddia ediyor, bu yalnız başarılarla sağlanıyor. Boş-geçmeyi engellemek için yazılmış
bir testin kendisi boş geçmiş. Test düzeltilmeli — `n10 + n11 > 0` istemeli.

**Düzeltme bu turda YAPILMADI:** `advance()`'ın çağrı yeri bir tasarım kararıdır (her olayda mı, timer'da mı)
ve S-8 zaten enstrümantasyonu **ayrı yetkilendirilmiş kalem** ilan ediyor.

## S-1 — `rule_fired` yanlış invaryant

İki **ayrık** sıfır-parity yolu var:

* **A (provenance):** `sender.rs:3256`, `s.no_rule` — sayaçlı, `suppression: Some(NoRule)`, LİTERAL sıfır.
* **B (bant kaynaklı, sessiz):** `Some(s)` fallthrough — `compute_requested_r` küçük ratio'yu 0'a
  yuvarlıyor (`k=8`'de `ratio <= 62 -> r = 0`), `suppression: None`, **sayaç yok**.

Bant yalnız R2 (`bw.high`) ve R3 (`bw.med`) içinde — yani **sıfır-olmayan fec consequent taşıyan tek iki
kural**. R1/R4 bant-bağımsız ve `fec_zero` taşıyor. Dolayısıyla bant, `total_weight > 0` bırakırken
(yani `no_rule = false` iken) tüm parity'yi susturabiliyor. `no_rule` buna **yapısal olarak kör**.

Sayısal bant sınırları (kaynaktan türetildi):
`bw.med > 0` ⟺ `50 048 ≤ bw < 199 680` B/s · `bw.high > 0` ⟺ `bw ≥ 150 016` B/s.

**S-1.5 NOT-DERIVABLE:** kilidi koyduran dejenerasyon ölçümü yok; `9a302406`'nın gerekçesi
config-bağımsızlık argümanı, bant ölçümü değil. Sınır **icat edilmedi**.

**Kapsam notu:** `fec_controller_latch`'in de **sıfır ürün çağıranı** var. Tek src-seviyesi FEC blok-kapanışı
`udp_runner.rs:760` sabit-r oracle harness'ı ve `no_rule`'a hiç bakmıyor. `fuzzy_evaluate` ise ürün
yolunda (`sender.rs:1562`, olay başına koşulsuz).

## S-2 — `LOSS_MAX` skaler olamaz

**S-2.1 FALSIFIED, ama beklenmedik sebeple:** 250 000 ppm'de `bw HIGH` **ilgisiz**, çünkü
`loss.high(250 000) = 0` ⇒ R2 her bant için özdeş sıfır. 250k'daki parity R3'ten geliyor (`bw.med` gerekli).
Önceki tur "125 kB/s'de `bw.high = 0`" gözlemi **doğru** (log2_fp(125 000) = 4328 < 4388) ama ondan
çıkarılan sonuç bu bulguyla **geçersizleşti**.

**S-2.2:** onset bir nokta değil, `(loss, bandwidth, jitter)` üzerinde **üç rejimli yüzey**, artı aktüasyon
aşamasında dördüncü eksen `k`. ⇒ ön-beyanlı **dal 3** ateşlendi: manda yüzey üzerinden tanımlanmalı,
**yeni tasarım turu gerekir**.

**S-2.4:** `LOSS_MAX` sembolü hiçbir tracked dosyada yok. Yerine değer **konmadı**.

## S-7 — fayda, overlay'in rejiminde ölçülmemiş

FS-FUSION-CELL-1'in 12 hücresinin tamamı **tek bant noktasında** koştu:
`C = 1 000 000 bit/s = 125 000 B/s`, pacer `800 000 bit/s = 100 000 B/s`. **Bant ekseni yok.**
Sınıf: `μ_med = 1000`, `μ_high = 0`.

**S-7.4 FALSIFIED:** hücreler overlay'in runtime alanıyla **eşleşmiyor**, üç eksenin ikisinde fark marjinal
değil. ⇒ ön-beyanlı dal: *"fayda, overlay'in davranabileceği rejimde ÖLÇÜLMEMİŞTİR; tasarımın tüm
gerekçesi bir ekstrapolasyondur"*.

## S-9 — `(loss, d)` çifti tutarlı değil

`loss_windowed_ppm` ile fate-estimator penceresi **her eksende farklı örneklem**: farklı anahtar, farklı
zaman ufku, farklı olay popülasyonu. İki boyutlu bir kapı için bu çiftin ortak örneklemden gelmesi
gerekirdi.

## S-8 — `N` karar anında türetilemez

Sayaçlar ürün yolunda **hiçbir yerde tutulmuyor**: `features_from_window` içinde yerel olarak
hesaplanıp değerle dönüyor. Kalıcı ürün durumu yalnız iki private `BTreeMap`.
⇒ **kritik dal ateşlendi:** Prereg-A `BLOCKED-PENDING-INSTRUMENTATION` kalır; enstrümantasyon **ayrı
yetkilendirilmiş kalem**, kendi nötrlük kanıtıyla. Bu turda yazılmadı, mevcut varsayılmadı.

## S-3/S-4/S-5/S-6 — dayanaklar

`α = 1` (klasik Laplace, `+2 = 2α`) ⇒ corrigendum'un `Var(d)`, `σ`, AUC türetimleri **doğrulandı**.
`W = 256` ve `eligible()` **tam eşitlik** istiyor (`n == FATE_WINDOW`), `>=` değil. UNKNOWN zinciri
**yokluğuyla** kırıyor — sentinel değerle değil. Eşikler `8 / 32`, `pub const`, PINNED.
`round_half_up` = `(ratio * k + 500) / 1000`, tamsayı, `sender.rs:538`.
`R2_RATIO` sembolü kaynakta **yok** ⇒ `NOT-DERIVABLE`, değer konmadı.

## Kapanış

`NOT-DERIVABLE` hiçbir yerde varsayılan üretmedi. `S-1`, `S-2`, `S-7` bloklayıcılarının üçü de
`FALSIFIED` bileşen taşıyor ve **üçü birden yeni tasarım turu gerektiriyor**:

1. `rule_fired` invaryantı düşürüldü — bant kapısı zorunlu, sınırı S-1.5'ten gelemiyor.
2. `LOSS_MAX` skaler olamaz — manda yüzey üzerinden tanımlanmalı.
3. Fayda, overlay'in rejiminde ölçülmemiş — gerekçe ekstrapolasyon.

Ve hepsinin önünde: **estimator ürün yolunda dejenere** (`advance()` yok) ⇒ yapı sinyali şu an
üretilemiyor. Bu kapanmadan diğer üçü akademik.

---

## ÇÖZÜM KAYDI (2026-08-15, sonradan)

"En ağır bulgu"nun `advance()` bacağı **kapatıldı** — tasarım ve gerekçe:
`docs/design/ESTIMATOR-LIFECYCLE.md`. `advance(now)` motor saatiyle birlikte `on_event_core`
girişinde; `finalized` FATE_WINDOW'a sınırlandı (denklik kanıtıyla); üç zorunlu doğrulama
(`n10+n11>0` gerçek akışta, deterministik dizide tam sayaçlar, legacy neutrality) iki build'de de
yeşil. Boş geçen `f.n > 0` testi `n10+n11 > 0` ile değiştirildi.

Karşıt denetimin yeni bulgusu — **in-guard düzeltme bacağı ürün yolunda erişilemez** (geç ACK
tombstone/un-loss'a düşüyor, estimator'ı beslemiyor) — `ESTIMATOR-LIFECYCLE.md §6.1`'de bağlayıcı
sınır olarak kayıtlı: **bu kapanmadan hiçbir kalibrasyon `burst_delta` tüketemez.** S-8'in
enstrümantasyon dalı ve Prereg-A `BLOCKED-PENDING-INSTRUMENTATION` durumu değişmedi.
