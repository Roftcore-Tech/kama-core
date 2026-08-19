# ERRATA-CORRIGENDUM-1-rev2 — KM-DR-STRUCT-GATE-001

**Bu belge matematikte OTORİTEDİR.** Çelişki hâlinde `KM-DR-STRUCT-GATE-001-ERRATA` ve tüm `v0.x` tasarımlarının önüne geçer.

**Kapsam:** yalnız matematik düzeltmesi. Kaynak okunmadı, kod yazılmadı, kampanya koşulmadı.
**Prereg-A freeze:** `DUR-B` (değişmedi). **Overlay implementasyonu:** `NO-GO` (değişmedi).
**rev.1 → rev.2:** üç düzeltme — (a) C-1b'nin "AUC 0.90 imkânsız" çıkarımı **geri çekildi** (b) `d_B*` ile `μ_B*` isim karışıklığı giderildi (c) araç aritmetiği bölüm bölüm doğru beyan edildi.

## Aritmetik beyanı (rev.2 düzeltmesi (c))

Araç **tek tip değil**; bölümler farklı aritmetik kullanıyor ve rev.1'in "tam kesir" başlığı yanıltıcıydı:

| bölüm | aritmetik |
|---|---|
| **C-2** (zarf ispatı) | **Tam kesir** (`fractions.Fraction`), sayaç uzayının tam sayımı — yaklaşım yok |
| **C-1 / C-1b** (ayrışma) | **Float + bisection**, tolerans 1 ppm; **normal yaklaşım** kullanılıyor. rev.1 `0.001`'lik `d` ızgarası kullanıyordu — rev.2'de bisection'a çevrildi |

---

## C-1 — Ayrışma kriteri: cutoff `C` ile hedef ortalama `μ_B` ayrıldı  *(rev.1'den değişmedi)*

`nreq()` şunu kullanıyordu:

```python
iid_upper < T - z * sd_b(T, p, N)          # ← T hem cutoff hem mu_B
```

`sd_b(T, …)` **gerçek ortalaması `T` olan** dağılımın sapmasını hesaplıyor ⇒ kriter burst ortalamasını cutoff'a eşitliyordu ⇒ örneklerin ~yarısı cutoff'un altında = %50 false-open.

Doğru kriter:

```
μ_IID(p,N) + z·σ_IID(p,N)   <   C   <   μ_B − z·σ_B(p, μ_B, N)
```

Markov parametreleşmesi: `p01 = p(1−d)`, `p11 = p(1−d) + d`.

### Kabul edilebilir cutoff aralığı (z=2, α=1, N=255)

| p | IID `μ` | IID `σ` | ALT SINIR `μ+2σ` |
|---:|---:|---:|---:|
| 0.10 | 25 635 | 58 482 | **142 599** |
| 0.20 | 8 408 | 60 614 | **129 637** |
| 0.30 | 2 879 | 61 299 | **125 477** |

Üst sınır **hedef sınıfa göre değişir** (p=0.20): GE-L2 → 229 025 · **GE-L3 → 440 306** · GE-L5 → 622 359 · GE-L10 → 771 599.

### Geçersiz ilan edilenler

"281 000 altında ayrışabilir sınır yok" · "[281 000, 435 000] kabul edilebilir aralık" · tüm `T_min(p)` tablosu · tüm `N_required(loss,T)` tablosu · `LOSS_MIN ≈ 160 000` · `N_DEC = 225` · "tutarlı çiftler" tablosu.
E-1'in **teşhisi** (iki bağımsız sabitin AND'lenmesi yanlış yapı) ayakta; **sayıları** geçersiz.

---

## C-1b (rev.2) — İki AYRI ölçüt; rev.1 bunları birbirine karıştırmıştı

### rev.1'in hatası

rev.1 şunu yazdı: *"`μ_B < 272 000` ise K-1'in 0.90 AUC barı karşılanamaz; bu hücreler mandatory olmamalı."*

**Bu çıkarım geçersizdir ve geri çekilmiştir.** İki 2σ bandının ayrışması, AUC ≥ 0.90'dan **çok daha sert** bir koşuldur. Normal yaklaşımda:

```
AUC = Φ( (μ_B − μ_I) / √(σ_I² + σ_B²) )
```

- **2σ non-overlap** gerektirir: `μ_B − μ_I > 2(σ_I + σ_B)` → `σ_I ≈ σ_B` iken **4.00σ**
- **AUC = 0.90** gerektirir: `(μ_B − μ_I)/√(σ_I²+σ_B²) = 1.2816` → aynı koşulda **1.81σ**

⇒ 2σ ölçütü yaklaşık **2.2× daha sert.** Bantlar çakışırken AUC pekâlâ 0.90'ı geçebilir.

### Sayısal doğrulama (p = 0.20, N = 255)

| `d_true` | `μ_B` | 2σ bantları ayrışık mı | AUC |
|---:|---:|---|---:|
| 150 000 | 153 589 | **HAYIR** | **0.9444** |
| 200 000 | 201 982 | **HAYIR** | **0.9821** |
| 250 000 | 250 376 | **HAYIR** | **0.9954** |
| 271 559 | 271 242 | EVET (sınır) | 0.9976 |
| 300 000 | 298 769 | EVET | 0.9991 |

2σ sınırında AUC zaten **0.9976** — bara çok uzak. İlk üç satır rev.1'in çıkarımını doğrudan çürütüyor.

### İsimlendirme düzeltmesi (rev.2 düzeltmesi (b))

rev.1'in kodu üretecin **gerçek** `d`'sini basıp `μ_B*` diye etiketliyordu. İkisi ayrı büyüklük ve Laplace küçültmesi nedeniyle farklılar:

- **`d_B*`** = üretecin gerçek `p11 − p01` farkı (generator parametresi — bir hücreyi *yapılandırırken* kullanılır)
- **`μ_B*`** = Laplace-smoothed **beklenen gözlenen** ortalama (`mu_sm` — veride *okunacak* değer)

### İki eşik, yan yana (α=1, bisection, tol 1 ppm)

| p | N | 2σ: `d_B*` | 2σ: `μ_B*` | AUC@2σ | **AUC 0.90: `d_B*`** | **AUC 0.90: `μ_B*`** |
|---:|---:|---:|---:|---:|---:|---:|
| 0.10 | 255 | 320 179 | 324 581 | 0.9971 | **134 203** | **150 938** |
| 0.10 | 512 | 218 179 | 224 012 | 0.9973 | 90 883 | 101 078 |
| **0.20** | **255** | **271 559** | **271 242** | 0.9976 | **119 553** | **124 119** |
| 0.20 | 400 | 215 332 | 216 317 | 0.9976 | 94 654 | 98 143 |
| 0.20 | 512 | 189 437 | 190 639 | 0.9976 | 83 309 | 86 241 |
| 0.30 | 255 | 255 254 | 252 733 | 0.9977 | 115 279 | 115 719 |
| 0.30 | 512 | 180 211 | 179 751 | 0.9977 | 81 087 | 81 684 |

**`272 000` yalnızca muhafazakâr 2σ non-overlap sınırıdır.** K-1 ile ilgili olan sayı `p=0.20, N=255` için **`d_B* ≈ 119 553`**'tür.

### Veri boşluğuyla ilişki — rev.1'in tablosu düzeltildi

```
GE veri boşluğu                 : [  88 000 , 371 000 ]
AUC-0.90 ayrışamaz bölge (pop.) : [       0 , 119 553 ]     (p=0.20, N=255)
2σ non-overlap sağlanmayan bölge: [       0 , 271 559 ]     (muhafazakâr, K-1 ölçütü DEĞİL)
```

⇒ **Veri boşluğunun büyük kısmı (`120 000 – 371 000`) AUC 0.90 açısından ayrışabilir.** rev.1'in "boşluğun çoğu karar verilemez" tespiti **yanlıştı**.

### Prereg-A'ya öneri — rev.1'in önerisi geri çekildi

**Geri çekilen:** "mandatory K-1 hücreleri `μ_B ≥ μ_B*(2σ)` ile sınırlansın; altındakiler K-1 barına tabi olmasın." Bu, K-1 hücrelerini **haksız yere** dışlardı.

**Yerine:**
1. Her mandatory hücre için **beklenen popülasyon AUC'si ön-kayıtta hesaplanıp yazılsın** (yukarıdaki formül). Bu, sonuç-kör bir güç beyanıdır ve hiçbir hücreyi dışlamaz.
2. K-1'in gerçek barı **bootstrap %95 alt sınırıdır**, popülasyon AUC'si değil. Alt sınır, ayrışmaya **ve koşu sayısına** bağlıdır ⇒ hangi hücrede kaç koşu gerektiği **Prereg-A'nın pilot + sizing işidir**, bu türetimin işi değil.
3. Bir hücre yalnızca **popülasyon AUC'si 0.90'ın altındaysa** (ör. `d_B* < 119 553` bölgesi) "yapısal olarak barı geçemez" denebilir — ve o zaman bile bu bir **dışlama gerekçesi değil, güç beyanıdır**.
4. `2σ` sütunu belgede **muhafazakâr referans** olarak kalır; hiçbir kapı ona bağlanmaz.

### Pencere büyütmenin getirisi (düzeltilmiş gerekçe)

`W=256 → 512` (`N` 255 → 511), `p=0.20`'de AUC-0.90 eşiğini `d_B* 119 553 → 83 309`'a indiriyor. Kazanç cutoff'u aşağı çekmek değil, **daha zayıf burst'leri ayırt edilebilir kılmak** — ama eşik rev.1'in sandığından **zaten çok daha düşüktü**, dolayısıyla pencere büyütmenin aciliyeti de o kadar yüksek değil.

---

## C-2 — Zarf ispatlı  *(rev.1'den değişmedi; tam kesir)*

`d` yalnızca `(n00, n01, n10, n11)`'in fonksiyonudur ve bu vektör üç serbest parametreyle **tam** belirlenir:

```
r = kayıp-run sayısı (1 ≤ r ≤ L) · f = ilk paket kayıp mı · l = son paket kayıp mı

n11 = L − r    n01 = r − f    n10 = r − l    s = r + 1 − f − l    n00 = S − s
kısıtlar: sayaçlar ≥ 0 · 1 ≤ s ≤ S (S>0 iken) · Σ = W − 1
```

Uzay sonlu (`≤ 4L`) ⇒ **tam sayım = ekstremum ispatı.**

| p | L | aday | `d_min` | (r,f,l) | sayaçlar | `d_max` | (r,f,l) | sayaçlar |
|---:|---:|---:|---:|---|---|---:|---|---|
| 0.05 | 13 | 51 | **+9 290** | (13,0,0) | 229/13/13/0 | 920 408 | (1,0,1) | 242/1/0/12 |
| 0.10 | 26 | 103 | −81 169 | (26,0,0) | 203/26/26/0 | 954 342 | (1,0,1) | 229/1/0/25 |
| 0.20 | 51 | 203 | **−233 559** | (51,0,0) | 153/51/51/0 | **971 107** | (1,0,1) | 204/1/0/50 |
| 0.30 | 76 | 303 | −412 594 | (76,0,0) | 103/76/76/0 | 976 024 | (1,0,1) | 179/1/0/75 |
| 0.50 | 128 | 510 | **−984 556** | (128,0,1) | 0/128/127/0 | 976 863 | (1,1,0) | 127/0/1/127 |

Değerler inşa sonuçlarıyla birebir aynı ⇒ **kanıt yükseltmesi**, değer düzeltmesi değil. Zarf statüsü `CONSTRUCTED` → `PROVEN`.

---

## Değişmeyenler

IID ortalaması ve tablosu · `Var(d) = p(1−p)[L/(L+2α)² + S/(S+2α)²]` ve `1/√N`'in α=0 üst sınırı olması · `p=0.05`'te `d_min > 0` (ispatlı) · fixture'lar ve tam kesir değerleri · `W-SCATTERED` isimlendirmesi · birim sözleşmesi `R2_RATIO(k)=2000/k` · E-2…E-5, E-7, E-8, E-11…E-15 · `LOSS_MAX = UNRESOLVED` · Prereg-A `DUR-B` · overlay `NO-GO`.

## Bu turda yapılmayanlar

Kaynak okunmadı (S-0/S-1/S-2/S-3/S-4/S-7/S-8 `ANSWERS PENDING`) · kod yazılmadı · cargo koşulmadı · kampanya/pilot koşulmadı · commit/push yapılmadı · eşik, `LOSS_MAX`, overlay semantiği, FEC politikası önerilmedi.
