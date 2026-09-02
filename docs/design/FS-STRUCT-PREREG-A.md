# FS-STRUCT-PREREG-A — Estimator Karakterizasyon Kampanyası

**Statü:** `DRAFT — NOT FROZEN — NOT AUTHORIZED TO RUN`
**Sürüm:** v0.2 (v0.1 adversarial olarak revize edildi)
**Taban:** `f54b1cdc` · **Bağlam:** `KM-DR-STRUCT-GATE-001-ERRATA.md`

---

## §1 — Bu kampanyanın kimliği ve neye KARAR VERMEDİĞİ

Bu bir **enstrüman karakterizasyonudur**, kalibrasyon değil.

| Karar | Bu kampanyada |
|---|---|
| `T` (yapı eşiği) | **SEÇİLMEZ** — Prereg-B |
| `N_required` tablosu | **DONDURULMAZ** — `T` ile birlikte, Prereg-B |
| `LOSS_MIN` / `LOSS_MAX` | **SEÇİLMEZ** |
| overlay davranışı / FEC politikası | **DEĞİŞMEZ** — overlay OFF |

Tek çıktı: `(yapı, loss, bandwidth, RTT)` → `burst_delta` dağılımı, `N` ve `rule_fired` ile birlikte.
**Bu belge bir eşik önerirse ön-kayıt ihlalidir.**

---

## §2 — Bloklayıcı ön koşullar

| # | Ön koşul | Kapanmazsa |
|---|---|---|
| P-0 | **S-0** kapalı: estimator ürün yolunda tracked, derleniyor, çağrılıyor | `ESTIMATOR-NOT-IN-PRODUCT` ⇒ Prereg-A **yazılmaz** |
| P-1 | **S-1, S-2, S-7** kapalı | bant ekseni sayısallaştırılamaz ⇒ **DUR-B** |
| P-2 | **S-3** kapalı: α ve ppm yuvarlama kuralı | §3 fixture'larının beklenen değeri yazılamaz |
| P-3 | **S-8** kapalı: `N` ürün yolunda türetilebilir | ürün değişikliği gerekiyorsa ⇒ `BLOCKED-PENDING-INSTRUMENTATION`; enstrümantasyon **ayrı yetkilendirilmiş kalem** olur, kendi nötrlük kanıtıyla. Bu turda yazılmaz, mevcut varsayılmaz |
| P-4 | §3 fixture'ları **PASS** | aritmetik yanlışsa stokastik veri anlamsız |
| **P-4b** | **S-0.7 ingest sadakati**: bilinen bir kanal realizasyonu ürün yolundan sürüldüğünde `n00/n01/n10/n11` o realizasyona **uyuyor** | **INGEST VAR ≠ INGEST DOĞRU.** §3 fixture'ları estimator'ın aritmetiğini doğruluyor, ürünün doğru fate dizisi ürettiğini DEĞİL. Bu kontrol olmadan tüm kampanya, yanlış bir fate dizisi üzerinde doğru aritmetik yapıyor olabilir |
| **P-4c** | **S-0.6 okuma anı**: `d` ve `N`, parity planlama anında ürün değişikliği olmadan okunabiliyor | §6 kaydı karar anını şart koşuyor; `diag_loss_structure` yalnız post-hoc accessor ise karar anı yakalanamaz ⇒ S-8'in enstrümantasyon dalı işler |
| P-5 | §9 nötrlük yüzeyleri doğrulandı | enstrüman geçersiz |

**P-4 sıfır stokastik koşu maliyetlidir ve İLK yapılır.**

---

## §3 — Enstrüman doğrulaması (deterministik)

Hepsi `W=256`, `L=51`, sayaçlar diziden **sayılır**. Pozisyonlarla tanımlıdır — örüntü adıyla değil (sınır yerleşimi `d`'yi ~5 000–18 500 ppm oynatıyor).

| fixture | yerleşim | n00/n01/n10/n11 | baş/son | referans `d` |
|---|---|---|---|---|
| `W-SCATTERED` | 51 izole, tam iç | 153/51/51/0 | 0/0 | `−1275/5459` |
| `W-CLUSTER` | 3'lük 17 run, iç | 187/17/17/34 | 0/0 | `+3128/5459` |
| `W-RUN-TERMINAL` | tek run, sonda | 204/1/0/50 | 0/1 | `+10453/10764` |
| `W-RUN-INTERIOR` | tek run, iç | 203/1/1/50 | 0/0 | `+5200/5459` |
| `W-RUN-LEADING` | tek run, başta | 204/0/1/50 | 1/0 | `+10453/10918` |
| `W-SHORT` | 51 paketlik akış, izole kayıp | (S-4 sonrası) | — | `N` düşük yolun pini |

Her fixture'da **ayrı ayrı** doğrulanır: sayaç toplamı = `N` · kayıp sayısı · sınır fate'leri (ilk/son paket) · `d`.

**`W-SCATTERED` IID DEĞİLDİR** — periyodik/anti-korele bir tanıktır ve zarfın tabanındadır (gerçek IID ortalaması `p=0.20`'de `+8 408`). Sonucu **"IID performansı" olarak yorumlanamaz**; yalnız estimator aritmetiğini ve yapı duyarlılığını doğrular.

**Beklenen değer kaynağın kendi tamsayı yolundan türetilir** (P-2). Yukarıdaki rasyoneller referanstır. Sapma ⇒ **DUR-B**.

Opsiyonel: ayrı, **ön-kayıtta seed'i yazılmış, cherry-pick edilmemiş** bir seeded IID witness. Seed sonuç görülmeden dondurulur.

---

## §4 — Hücre stratası

Hücreler **üç strata** ayrılır; kabul ölçütleri stratalara göre farklıdır.

### Strata 1 — Karar-eligible karakterizasyon
Ölçmek istediğimiz hücreler. Karar-eligible örnek üretmeleri **beklenir**.

### Strata 2 — Beklenen-ineligible negatif kontroller
- kayıpsız reordering
- eligibility'ye ulaşmadan biten kısa akış
- kayıpsız akış

Burada **ineligibility BEKLENEN SONUÇTUR.** Sıfır eligible örnek bir başarısızlık değil, **doğrulamadır**. Bu hücrelerde eligible örnek **çıkarsa** bu bir bulgudur ve `FALSE-ELIGIBILITY` olarak raporlanır.

### Strata 3 — Structurally-inert bant hücreleri
`rule_fired = false` beklenen bant sınıfları. Burada overlay hiçbir zaman davranamaz; ölçüm **kapsam** sorusunu cevaplar, ayrışma sorusunu değil.

### Çelişkinin çözümü
v0.1'de `K-2 (sıfır eligible ⇒ DUR-B)` ile `rule_fired=false ≥ %50 ⇒ STRUCTURALLY-INERT` birbiriyle çelişiyordu. Tek mantık:

```
strata 1 hücresi, sıfır karar-eligible örnek      → DUR-B
strata 2 hücresi, sıfır karar-eligible örnek      → PASS (beklenen)
strata 2 hücresi, karar-eligible örnek VAR        → FALSE-ELIGIBILITY bulgusu, raporlanır
strata 3 hücresi                                  → K-1 uygulanmaz; kapsam olarak raporlanır
strata 1 hücresi, rule_fired=false baskın         → MISCLASSIFIED-CELL: strata 3'e taşınır,
                                                    taşıma ön-kayıtlı eşikle ve SONUÇTAN BAĞIMSIZ
```

Son satırdaki eşik (`rule_fired=false` oranı) **sonuç görülmeden** dondurulur ve `d` dağılımına bakılarak değiştirilemez.

---

## §5 — Matris ve hücre seçme algoritması

"Tam çarpım koşulmaz" yetersizdir. Sonuç görülmeden şunlar dondurulur.

### 5.1 Eksenler

| eksen | seviyeler |
|---|---|
| **yapı** | IID · MARKOV-BD(b₁) · MARKOV-BD(b₂) · GE-L3 · GE-L10 · **ANTI-CORRELATED** (negatif-`d` üreten gerçek üreteç) · terminal-tail · S4-NaturalBurst · reordering-kayıpsız |
| **loss** | 5 seviye, `[100 000, 300 000]` içinde |
| **bandwidth** | **`PENDING S-1/S-2/S-7/S-10`** — sayısal değerler ve `bw.high` membership sonuçlarıyla tanımlanacak |
| **RTT** | 50 · 250 · 1000 ms (RTT1000 **dışlanmıyor**; §4.2b'nin 0.475'i bayat ürün durumundan) |

**Bant seviyeleri `orta` / `üstü` gibi etiketlerle dondurulamaz.** Her seviye: sayısal bandwidth + o noktada `bw.high` membership değeri + `rule_fired` beklentisi.

**ANTI-CORRELATED ekseni zorunludur** ve gerçek bir üreteçle üretilir. Erişilemeyen sentetik noktalar (örn. `d = −900 000 @ loss = 200 000`) **kullanılmaz** — o nokta zarf dışıdır.

### 5.2 Hücre sınıflandırması ve seçme algoritması

```
MANDATORY        := yapı ∈ {IID, MARKOV-BD(b₁), MARKOV-BD(b₂), GE-L3, ANTI-CORRELATED}
                  × loss ∈ {tam liste}
                  × bandwidth = overlay'in davranabileceği sınıf(lar)
                  × RTT ∈ {50, 250, 1000}
                  → K-1 barı BAĞLAYICI

NEGATIVE-CONTROL := strata 2'nin tamamı × RTT ∈ {50, 1000}
                  → K-1 uygulanmaz; beklenen ineligibility doğrulanır

EXPLORATORY      := kalan her şey (GE-L10, terminal-tail, S4-NaturalBurst,
                    strata-3 bant hücreleri, rejim geçişleri)
                  → K-1 raporlanır, BAĞLAYICI DEĞİL
```

Algoritma deterministik ve sonuç-kördür: `MANDATORY` tam çarpımdır ve **tamamı koşulur**; `EXPLORATORY` bütçe artığına göre **ön-kayıtlı lexicographic sırayla** doldurulur; sıra sonuç görüldükten sonra değiştirilemez.

---

## §6 — Karar anında kaydedilecek alanlar

Kayıt anı: **parity planlama çağrısının hemen öncesi.** Koşu sonu değil.

`d` · `n00, n01, n10, n11` **ayrı ayrı** · `N` · `rule_fired` · `measured_loss_ppm` · `configured_loss_ppm` (ayrı alan) · bandwidth girdisi · `bw.high` membership değeri · `legacy.fec_parity_ratio_permille` · eligibility'ye ulaşma (paket **ve** ms) · ground truth yapı sınıfı + generator sürümü/parametreleri · katman katman bayt.

**§3.4b:** configured etiketler (yapı sınıfı, generator parametreleri) **karar özelliği olarak kullanılamaz** — yalnız ground truth. Analizde yön `d → sınıf`'tır, `sınıf → d` değil.

---

## §7 — K-1: ayrışma metriği (tam tanım)

| soru | karar |
|---|---|
| IID hangi sınıfla karşılaştırılıyor? | **Pairwise.** IID vs her clustered sınıf ayrı ayrı. Pooled sınıf **kullanılmaz** (ağırlıklar sonuç-bağımlı olurdu) |
| Analiz birimi | **Koşu.** Koşu başına özet istatistik (karar anlarının medyanı) hesaplanır, AUC koşu-düzeyi birimler üzerinden |
| Koşu içi çoklu kararların korelasyonu | Koşu-düzeyi özetleme ile ele alınır; ayrıca ham karar-düzeyi AUC **ikincil** olarak raporlanır, bağlayıcı değil |
| Bootstrap | **Cluster bootstrap**, küme = koşu (veya trace). Paket düzeyi resampling **yasak** |
| Bar | AUC bootstrap **%95 alt sınırı ≥ 0.90** (FS-CAL-2 v2.1'den devralındı, yeni icat değil) |
| Yorum | `MANDATORY` hücrelerde **bağlayıcı**; `EXPLORATORY` hücrelerde **yalnız raporlanır** |

### Kabul ve DUR-B tek mantıkta

```
her MANDATORY hücre K-1 barını geçer                       → PASS
≥1 MANDATORY hücre geçmez, ama ≥1 MANDATORY hücre geçer    → PARTIAL —
        geçen hücrelerin birleşimi "desteklenen alan" olarak raporlanır;
        Prereg-B YALNIZ o alana yazılabilir
hiçbir MANDATORY hücre geçmez                              → DUR-B —
        enstrüman ayırt edici değil, tüm yapı-kapısı programı DURUR,
        Prereg-B yazılmaz
```

`PARTIAL` dalı v0.1'de yoktu ve kabul/DUR-B çelişkisinin kaynağıydı.

---

## §8 — Pilot ve sizing

**`N=20` pilot result-bearing bir ölçümdür.** Kampanyanın içinde serbestçe koşulamaz.

| gereklilik | içerik |
|---|---|
| Ayrı mini-prereg | `FS-STRUCT-PREREG-A-PILOT`, kendi kabul/DUR-B'siyle |
| Ayrı seed namespace | `SHA256("FS-STRUCT-PILOT-V1" ‖ rep ‖ cell)`; ana kampanya namespace'inden **ayrık** |
| Final analizden dışlama | Pilot kayıtları `fit_use = FORBIDDEN`; ana veri setine **girmez** |
| **Pilot ÖNCESİ donmuş sizing fonksiyonu** | `n_rep(cell) = f(pilot_ölçümü)` — fonksiyon pilot koşulmadan yazılır ve hash'lenir |
| Eşleme | Hangi pilot sonucunun hangi tekrar sayısını üreteceği **tablo olarak** ön-kayıtlanır |

**Pilot çalıştırma bu turun yetkisinde değildir.**

---

## §9 — Nötrlük yüzeyleri

"Bayt-özdeş" tek bir iddia değildir. Yüzeyler ayrılır ve **yalnız ilk üçü için özdeşlik talep edilir**:

| yüzey | talep |
|---|---|
| ürün wire çıktısı | bayt-özdeş |
| parity / ARQ / pacing kararları | bayt-özdeş |
| önceden pinlenmiş semantik trace alanları | bayt-özdeş |
| yeni diagnostic alanlar (`N`, `n**`, `rule_fired`) | **değişmesi beklenir** — açıkça versiyonlanır |
| canonical state digest | diagnostic alan içeriyorsa **versiyonlanır**; kaynak doğrulaması olmadan "özdeş" **yazılmaz** |

Karşılaştırıcı dışlamaları **sabit isim listesiyle** yapar, "ne farklıysa o" ile değil.

---

## §10 — Schema, lineage ve freeze sözleşmesi

Bu bölüm v0.1'de yoktu. Kural-3 (conformance) gereği, şema **kernel'in gerçekten expose ettiğinden** yazılır ve yürütücü açılışta bir kaydı şemaya karşı **self-validate** eder; uymuyorsa koşmayı **reddeder**.

| kalem | sözleşme |
|---|---|
| Raw schema | Tipli, alan başına: ad · tip · nullability · birim · zorunlu/opsiyonel. Şema hash'lenir |
| Karar anı semantiği | Tek tanım: parity planlama çağrısı öncesi; kayıt bu anın dışında **üretilemez** |
| Kimlik | `run_id` · `trace_id` · `seed` · `cell_id` · `repetition_index` |
| Generator | sürüm + tüm parametreler kayıtta |
| Loss ayrımı | `configured_loss_ppm` ve `measured_loss_ppm` **ayrı alanlar**; karıştırılmaz |
| Yeniden türetim | `n00/n01/n10/n11 → N → d` kayıttan **yeniden hesaplanabilir**; komparatör her satırda doğrular, uyuşmazlık ⇒ satır `INVALID` |
| Duplicate/retry/leakage | `run_id` tekilliği zorunlu; retry ayrı işaretlenir; pilot namespace sızıntısı fail-closed |
| Sonuç sınıfları | `VALID` · `INVALID` · `CENSORED` · `INELIGIBLE` · `STRUCTURALLY-INERT` — beşi **ayrı**, biri diğerine katlanmaz |
| Execution order | Ön-kayıtlı, donmuş, ledger'a yazılır |
| Artifact lineage | manifest hash · binary hash · şema hash · prereg hash · generator hash · rustc sürümü |
| Deterministic serialization | Alan sırası sabit; float **serileştirilmez** (tamsayı/rasyonel) |
| Belirleyicilik pini | **İki sentetik doğrulama koşusu aynı bilimsel hash'i üretmeli** |
| Operasyonel zaman damgaları | Bilimsel artefaktın **dışında** tutulur; hash'e girmez |

---

## §11 — Test ve mutasyon sözleşmesi (gelecek doğrulama için)

Üç seviye karıştırılmaz: **L1** saf overlay birim (evidence'ı doğrudan alır) · **L2** estimator aritmetik fixture (gerçek estimator yolu) · **L3** Sender wiring (gerçek estimator → gerçek Sender).

**Mutasyon kanıtı yalnız L3'ten gelir.** L1 mutasyon kanıtı üretemez.

| mutasyon | düşmesi gereken L3 testi |
|---|---|
| `diag_loss_structure` → `None` | `T3b` (W-SCATTERED kolu) |
| `diag_loss_structure` → zorla sabit `Some{d}` | `T3b` (W-CLUSTER kolu) |
| sahte `N` | `T-SHORT-L3` |
| sahte `rule_fired` | `T-NORULE-L3` |

`T3b`: aynı `L`, aynı `N`, aynı ölçülen loss, yalnız diziliş farkı → farklı parity. Gerçek estimator üzerinden. **İddia sınırı: yapı duyarlılığı, IID kurtarma DEĞİL.**

Her test için **hangi yanlış implementasyonun onu düşüreceği** yazılır. Bu olmadan test non-vacuous sayılmaz.

**Fixture/test implementasyonu bu turda yapılmaz.**
