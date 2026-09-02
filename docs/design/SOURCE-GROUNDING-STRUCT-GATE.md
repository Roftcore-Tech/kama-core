# SOURCE-GROUNDING-STRUCT-GATE

**Statü:** `QUESTIONS + PRE-DECLARED BRANCHES — ANSWERS PENDING`
**Yetki:** bu tur için **tek yetkili iş budur** (`GO`). Prereg-A dondurmak/koşmak `DUR-B`; overlay implementasyonu `NO-GO`.
**Taban:** `transport-baseline-closure` @ `f54b1cdc`
**Not:** Bu belge kaynak erişimi olmadan yazıldı; **hiçbir soru cevaplanmış değildir.** Değeri, cevapların *dalını* cevaplar görülmeden dondurmasıdır. Her cevap için istenen kanıt biçimi: dosya · sembol · satır aralığı · çağrı zinciri · mümkünse küçük doğruluk tablosu.

### Belge hiyerarşisi (çelişki hâlinde bu sıra geçerlidir)

| # | belge | rolü |
|---|---|---|
| 1 | **`ERRATA-CORRIGENDUM-1-rev2`** | **matematikte OTORİTE** — ayrışma kriteri, zarf ispatı, aritmetik beyanı |
| 2 | `KM-DR-STRUCT-GATE-001-ERRATA` | bağlam ve iddia-muhasebesi; **§2.2 ve §1'in E-1 sayıları GEÇERSİZ** (corrigendum tarafından) |
| 3 | **bu belge** | icra edilecek görev |
| — | `KM-DR-STRUCT-GATE-001 v0.1/v0.2/v0.3` | `SUPERSEDED — NOT REGISTERABLE`; yalnız tarihsel |
| — | `FS-STRUCT-PREREG-A` | `DRAFT — NOT FROZEN — NOT AUTHORIZED TO RUN` |

---

## S-0 — Estimator ürün yolunda GERÇEKTEN var mı (yeni; her şeyden önce)

Bu soru diğer hepsinden önce gelir çünkü tüm ark "estimator ürün yoluna bağlandı" premisine dayanıyor ve o premis bir kez geri alınmıştı (`pub mod fate_estimator;` hunk'ı `15a82531`'de reverted, dosya untracked bırakıldı).

| # | Soru |
|---|---|
| S-0.1 | `crates/adaptive-transport/src/fate_estimator.rs` `f54b1cdc`'de **tracked** mı? (`git ls-files`) |
| S-0.2 | `lib.rs`'te `pub mod fate_estimator;` var mı? |
| S-0.3 | **default** feature setiyle derleniyor mu, yoksa bir feature gate arkasında mı? |
| S-0.4 | `Sender::diag_loss_structure`'ın **ürün** çağıranı var mı, yoksa yalnız `#[cfg(test)]` / harness mi çağırıyor? |
| S-0.5 | `advance()` benzeri, estimator'ı besleyen çağrı ürün yolunda mı yoksa tasarımın *önerdiği* bir bağlantı mı? |

**Ön-beyanlı dallar — ⚠ rev.1 KUSURLU, aşağıda düzeltildi**

<details>
<summary>rev.1'in dalları (korundu, izlenebilirlik için — <b>KULLANILMAZ</b>)</summary>

- Tracked + default build + ürün çağıranı var → `ESTIMATOR-PRODUCT-PRESENT`, ark devam.
- Tracked ama feature-gated → `ESTIMATOR-FEATURE-GATED`.
- Untracked veya ürün çağıranı yok → `ESTIMATOR-NOT-IN-PRODUCT` = DUR-B.

</details>

### ⚠ S-0 DAL KUSURU — `INGEST` ile `OUTPUT CONSUMPTION` karıştırılmıştı

rev.1 "ürün çağıranı var mı" ölçütünü **çıktı tüketimi** (S-0.4) üzerinden yazdı. Bu iki nedenle yanlıştı:

1. **Yük taşıyan şey ingest'tir, çıktı tüketimi değil.** Premis "estimator ürün yoluna bağlandı ve **observation-only**" idi. Observation-only'nin *tanımı gereği* çıktısının ürün tüketicisi yoktur. rev.1 kendi premisiyle çelişen bir ölçüt yazdı.
2. **Döngüsel**: çıktı tüketicisini kurmak overlay'in kendi işi, overlay ise `NO-GO`. S-0.4 harfiyen uygulanırsa ark **hiçbir zaman** başlayamaz.

### Düzeltilmiş dallar (ölçüt = INGEST)

| durum | verdict | sonuç |
|---|---|---|
| tracked ∧ default build ∧ **ürün yolunda besleniyor** ∧ okunabilir accessor var | **`ESTIMATOR-PRODUCT-PRESENT — OBSERVATION-ONLY`** | premis **KURULDU**, ark devam. Çıktı tüketicisinin olmaması **beklenen** durumdur |
| tracked ama feature-gated | `ESTIMATOR-FEATURE-GATED` | Prereg-A build ekseni o feature'ı açıkça içerir; feature'ın başka ne derlediği listelenir |
| **ürün yolunda BESLENMİYOR** (besleyen çağrılar yalnız test/harness'ta) | `ESTIMATOR-NOT-IN-PRODUCT` = **DUR-B** | premis kurulamadı; `68d5ec02` ve `v0.3`'ün ortak dayanağı yok |
| untracked veya default build'de derlenmiyor | `ESTIMATOR-NOT-IN-PRODUCT` = **DUR-B** | aynı |

### S-0 DISPOSITION (kapandı)

| # | bulgu | statü |
|---|---|---|
| S-0.1 | tracked | `VERIFIED` |
| S-0.2 | `lib.rs:34` `pub mod fate_estimator;` | `VERIFIED` |
| S-0.3 | feature gate yok, default build | `VERIFIED` |
| S-0.4 | `diag_loss_structure` ürün çağıranı **yok** (yalnız test) | `VERIFIED` — **observation-only'nin beklenen sonucu, kusur değil** |
| S-0.5 | besleyen çağrılar ürün yolunda: `sender.rs:2274`, `:2619` | `VERIFIED` — **yük taşıyan bulgu** |

**VERDICT: `ESTIMATOR-PRODUCT-PRESENT — OBSERVATION-ONLY`. Premis kuruldu. DUR-B DEĞİL.**
Destekleyici: kurulum `996fc9f6`'da bilinçli; `loss_structure_observation.rs` tam üç kullanımı (iki yazma, bir okuma) **pinliyor** ⇒ ingest'in bağlı kalması test tarafından korunuyor.

### Bu verdict'in AÇMADIĞI iki şey — yeni sorular

| # | soru | neden yeni |
|---|---|---|
| **S-0.6** | `d` ve `N`, **parity planlama anında** okunabiliyor mu — ürün değişikliği olmadan? `diag_loss_structure` yalnız post-hoc bir accessor mı, yoksa harness Sender'ı adım adım sürerken karar anı yakalanabiliyor mu? | Prereg-A §6 kaydı **karar anında** şart koşuyor, koşu sonunda değil. S-0.5 ingest'i kanıtlıyor, **okuma anını** kanıtlamıyor. Bunu bilmeden Prereg-A uygulanabilir sayılamaz. Ürün değişikliği gerekiyorsa → S-8'in dalı işler (ayrı yetkilendirilmiş enstrümantasyon kalemi) |
| **S-0.7** | `sender.rs:2274` ve `:2619` estimator'a **hangi fate'i, hangi koşulda** veriyor? (SUCCESS/LOSS/UNKNOWN · reordering guard · geç-ACK düzeltmesi · zincir kırılması) | **INGEST VAR ≠ INGEST DOĞRU.** §3 fixture'ları estimator'ın *aritmetiğini* doğruluyor, ürünün *doğru fate dizisi ürettiğini* değil. Bu ayrı bir sadakat sorusu ve bugüne kadar hiç sorulmadı |

**S-0.7'nin Prereg-A'ya sonucu:** kampanya, bilinen bir kanal realizasyonunu ürün yolundan sürüp `n00/n01/n10/n11`'in o realizasyona uyduğunu doğrulayan bir **ingest-sadakat kontrolü** taşımalı. Bu, §3'ün aritmetik fixture'larından ayrı bir ön koşuldur ve Prereg-A v0.2'de **yok**.

---

## S-1 — `no_rule` ve bant dejenerasyonu (BLOKLAYICI)

| # | Soru |
|---|---|
| S-1.1 | `sender.rs:3256` civarındaki literal-zero FEC davranışının **tam koşulu** nedir? |
| S-1.2 | Davranış `no_rule` provenance'ına mı, bandwidth sınıfına mı, ikisinin birleşimine mi bağlı? |
| S-1.3 | `rule_fired = true` olup bandwidth nedeniyle parity'nin 0 veya `fec_low` kaldığı durumlar var mı? |
| S-1.4 | `rule_fired` runtime **feasibility** göstergesi olarak kullanılabilir mi, yoksa yalnız **provenance** mi taşır? |
| S-1.5 | Kilidi koyduran dejenerasyon ölçümü nerede kayıtlı, ve **hangi bant sınıfı sınırında** ölçüldü? |

**Ön-beyanlı dallar**
- Yalnız `no_rule` provenance → `rule_fired` doğru invaryant; overlay'in (Z) dalı yeterli.
- Bandwidth sınıfına bağlı (S-1.3 = evet) → **`rule_fired` YANLIŞ İNVARYANT**; overlay'in bant kapısı zorunlu ve sınırı S-1.5'ten türetilir.
- S-1.5 bulunamıyor → sınır **icat edilmez**; overlay'in mandası S-7'nin ölçtüğü bant rejimiyle sınırlanır, dışı `OUT-OF-SUPPORTED-DOMAIN`.
- S-1.1 kaynakta izole edilemiyor → **DUR-B**, yeni tasarım sürümü yazılmaz.

---

## S-2 — Q18 ve 250k davranışı (BLOKLAYICI)

| # | Soru |
|---|---|
| S-2.1 | 250 000 ppm'de legacy parity'nin pozitif olması `bw HIGH` membership'ini gerektiriyor mu? |
| S-2.2 | `shoulder_low(200_000, 300_000)` gerçek karar yüzeyinde hangi noktada pozitif parity üretmeye başlıyor? |
| S-2.3 | Bu başlangıç, loss dışında hangi antecedent'lara bağlı? |
| S-2.4 | `LOSS_MAX = 250 000` için kaynakta gerçekten bir gerekçe var mı? |

**Ön-beyanlı dallar**
- `bw HIGH` gerekmiyor → `LOSS_MAX` gerekçesi kurulabilir, değeri S-2.2'den okunur (250 000 varsayılmaz).
- `bw HIGH` gerekiyor → ölçülen bantta legacy **hiçbir loss'ta** parity açmıyor demektir ⇒ **`LOSS_MAX`'ın gerekçesi çöker**; üst sınır yeniden türetilir veya yoktur. (Z) dalı etkilenmez.
- S-2.2 tek bir noktaya indirgenemiyor (çok antecedent'lı yüzey) → `LOSS_MAX` skaler bir sabit **olamaz**; overlay'in mandası yüzey üzerinden tanımlanmalı → yeni tasarım turu gerekir.

---

## S-7 — FS-FUSION-CELL-1 bandwidth lineage (BLOKLAYICI)

| # | Soru |
|---|---|
| S-7.1 | FS-FUSION-CELL-1 hangi gerçek bandwidth / controller girdileriyle koştu? |
| S-7.2 | "IID'de 0/12 → 5–9/12" hangi bant sınıfına ait? |
| S-7.3 | "+%22,4 wire (4 949 → 6 060)" hangi bant sınıfına ait? |
| S-7.4 | Bu hücreler önerilen overlay'in runtime etki alanıyla **eşleşiyor mu**? |

**Ön-beyanlı dallar**
- Eşleşiyor → fayda iddiası overlay'in alanında geçerli.
- Eşleşmiyor → **fayda, overlay'in davranabileceği rejimde ÖLÇÜLMEMİŞTİR**; bu durumda tasarımın tüm gerekçesi bir ekstrapolasyondur ve Prereg-A'nın birincil işi o rejimde faydayı ölçmek olur (ki bu artık enstrüman karakterizasyonu değil, kapsam genişlemesi = ayrı yetkilendirme).
- Lineage bulunamıyor → **DUR-B**.

---

## S-3 … S-8 — destekleyici doğrulamalar

| # | Soru | Neyi açar |
|---|---|---|
| S-3 | Laplace **α** değeri ve smoothing'in tam formu; ürün **tamsayı/sabit-nokta** aritmetiği mi kullanıyor, ppm'e yuvarlama kuralı ne? | Bias tablosu · **tüm fixture beklenen değerleri** · corrigendum'un `σ` ve AUC hesapları (hepsi α'ya bağlı) |
| S-4 | Pencere semantiği: `W` gerçekten 256 fate-known mı; UNKNOWN zinciri **nasıl** kırıyor; kırılma `n**` sayaçlarını nasıl etkiliyor | `N` ve zarf hesabının geçerliliği |
| S-5 | `MIN_LOSS_TRANSITIONS` / `MIN_NONLOSS_TRANSITIONS` gerçek değerleri ve `eligible()`'ın tam yüklemi | gözlem/karar eligibility ayrımının tabanı |
| S-6 | `round_half_up(ratio·k/1000)`, `r_code_max`, desteklenen `k` kümesi | birim sözleşmesi (`R2_RATIO`) |
| S-8 | `n00/n01/n10/n11` ürün yolunda **nerede** tutuluyor; karar anındaki `N` **türetilebilir mi**; `WindowFeatures`'a `transitions` eklemek additive mi | Prereg-A'nın uygulanabilirliği |
| S-9 | `loss_ppm` ile transition penceresinin **olay ve zaman-ufku lineage'ı** — ikisi aynı örneklem üzerinden mi türetiliyor? | `(loss, d)` çiftinin tutarlılığı |
| S-10 | `bw.high` membership fonksiyonu ve **gerçek sayısal** bandwidth eşikleri | Prereg-A'nın bant ekseni |

**S-8 için ön-beyanlı dal (kritik):**
`N` ürün değişikliği olmadan türetilemiyorsa → Prereg-A `BLOCKED-PENDING-INSTRUMENTATION` kalır ve enstrümantasyon **ayrı yetkilendirilmiş bir kalem** olur, kendi nötrlük kanıtıyla. Ajan bu turda enstrümantasyon **yazmaz** ve `N`'in mevcut olduğunu **varsaymaz**.

---

## Kapanış disposition'ı

Her soru şu üç sınıftan biriyle kapanır ve **üçü de raporlanır**:

| sınıf | anlamı |
|---|---|
| `VERIFIED` | kaynakta bulundu, kanıt satırıyla |
| `FALSIFIED` | kaynakta bulundu ve önceki iddiayı çürütüyor |
| `NOT-DERIVABLE` | kaynakta bu HEAD'de kapatılamıyor — **`VERIFIED` sayılmaz** |

`NOT-DERIVABLE` bir varsayılan üretmez. `S-0`, `S-1`, `S-2` veya `S-7` bu sınıfa düşerse yeni tasarım sürümü yazılmaz.
