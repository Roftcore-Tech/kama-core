# KM-DR-STRUCT-GATE-001 — ERRATA

**Statü:** `ERRATA — SUPERSEDES v0.1/v0.2/v0.3`
**v0.3 statüsü:** `NOT REGISTERABLE · NOT IMPLEMENTABLE`
**Taban:** `f54b1cdc` · **Türetim aracı:** `struct_gate_math.py` (tracked)
**Kapsam:** matematik ve iddia düzeltmesi. **Yeni tasarım sürümü DEĞİL** — `S-0/S-1/S-2/S-7` kapanmadan overlay semantiği veya `LOSS_MAX` önerilmiyor.

> ### ⚠ SUPERSESSION — bu belge tek başına okunmaz
>
> **Matematikte otorite `KM-DR-STRUCT-GATE-001-ERRATA-CORRIGENDUM-1-rev2.md`'dir.** Bu belge bağlam ve iddia-muhasebesi için korunur; aşağıdaki iki blok **geçersizdir**:
>
> | blok | statü | gerekçe |
> |---|---|---|
> | **§2.2** — güvensiz köşe tablosu **ve** `N_required(loss,T)` tablosu | **GEÇERSİZ** | Corrigendum C-1: `nreq()` `T`'yi hem cutoff hem hedef burst ortalaması sayıyordu |
> | **§2.3** — `T`↔`N_required` bağlılığı | Metin ayakta, **atıf yaptığı tablo geçersiz** | aynı |
>
> Ayrıca **§1'in E-1 satırındaki sayılar** (`N=231` vb.) geçersizdir; E-1'in *teşhisi* (iki bağımsız sabitin `AND`'lenmesi yanlış yapı) ayaktadır.
> **§2.4 zarf tablosu ayaktadır ve corrigendum C-2 ile `CONSTRUCTED` → `PROVEN` yükseltilmiştir.**
>
> **Aritmetik beyanı (düzeltildi):** bu belgenin zarf ve fixture sayıları **tam kesir**; §2.2'nin (geçersiz) ayrışma hesabı **float** idi. Araç tek tip değildir — bkz. corrigendum rev.2 başlığı.

---

## 1. Düzeltme tablosu

| # | Eski iddia | Düzeltilmiş iddia | Kanıt / türetim | Tasarıma etkisi | Superseded |
|---|---|---|---|---|---|
| **E-1** | `N_DEC = 225` ∧ `LOSS_MIN = 160 000` güvenli | **Tutarsız çift.** `T=300k`'de `loss=160 000` için `N=231` gerekiyor | §2.2, güvensiz köşe listesi | İki bağımsız sabit **kaldırıldı**; yerine tek yüklem `N ≥ N_required(loss, T)` | v0.3 §2.5 sabitleri |
| **E-2** | `T = 300 000` "türetilmiş" | **Seçilmiş nokta.** Türetim aralık verir, nokta vermez. `T` ve `N_required` **bağlıdır ve birlikte** dondurulur | §2.3 | `T` tasarım sabiti değil; bu belgede yalnız `T_PROVISIONAL` (türetim/fixture) | v0.3 §1.4 "türetilmiş eşik" dili |
| **E-3** | T3 merkezî test | T3 saf fonksiyona elle `d` veriyor ⇒ **mekanizmayı doğrulamıyor.** Merkezî test **T3b**, gerçek estimator→Sender yolunda | §4 | Test sözleşmesi iki katmana ayrıldı | v0.3 §6 T3 |
| **E-4** | 4 mutasyon pini | Mutasyonlar **yanlış seviyedeydi**: `diag_loss_structure` mutasyonu evidence'ı doğrudan alan birim testi etkileyemez | §4.3 | Her mutasyon bir **Sender-seviyesi tanığa** eşlenir | v0.3 §6 T20 |
| **E-5** | `LOSS_MAX = 250 000` | **Gerekçesi yok** → `UNRESOLVED — S-2 PENDING` | S-2 | Üst sınır bu turda önerilmiyor | v0.3 §2.5, §5 |
| **E-6** | `σ = 1/√N` kesin, α'dan bağımsız | **Yaklaşımdır.** `Var(d) = p(1−p)[L/(L+2α)² + S/(S+2α)²]`; `1/N` = **α=0 limiti ve üst sınır** | §2.1 | Güvenlik yönü korunur (üst sınır ⇒ aralık daralmış), "kesin/α-bağımsız" dili geri çekildi | v0.3 §1.2 |
| **E-7** | `rule_fired` kilidi korur | **Yanlış invaryant olabilir** — dejenerasyon bant sınıfının özelliği olabilir | S-1 | Bant kapısı gerekebilir; sınır **icat edilmez** | v0.3 §2.3, §2.4 (Z) |
| **E-8** | `W-IID` fixture'ı | **IID DEĞİL.** `(153,51,51,0)` periyodik/anti-korele bir tanık; gerçek IID ortalaması `+8 408`, bu fixture zarfın **tabanı** | §3.1 | **`W-SCATTERED`** olarak yeniden adlandırıldı; T3b artık "IID kurtarma" değil **"yapı duyarlılığı"** kanıtlıyor | v0.3 §6 T1/T3, Prereg-A §2 |
| **E-9** | `W-SINGLE-RUN = (203,1,1,50)` → `d_max = +971 013` | **Sayım ile iddia tutarsızdı.** O sayım **iç run**'a ait ve `+952 555` verir. Gerçek maksimum **terminal run** `(204,1,0,50)` → **`+971 107`** | §3.2 | Fixture pozisyonla tanımlanır; üç sınır varyantı ayrı pinlenir | v0.3 §6, Prereg-A §2 |
| **E-10** | Zarfın `d_max` sütunu | **Tüm `d_max` değerleri yanlıştı** (kapalı formda `n11` ile `L` karıştırılmıştı). `d_min` sütunu doğruydu, `p=0.50` hariç | §2.4 | Zarf tablosu tümüyle yenilendi, insaayla doğrulandı | v0.3 §1'in E-6b tablosu |
| **E-11** | Fixture değerleri tamsayı ppm | **Tam kesirdirler**, ppm tamsayısı **kaynağın yuvarlama kuralına bağlı** | §3.3 | Beklenen değer **rasyonel** olarak yazılır; tamsayı karşılığı S-3/S-6 sonrası | v0.3 §6 tüm beklenen değerler |
| **E-12** | Nötrlük: "bayt-özdeş" | Yeni sayaç/telemetry alanları eklendiğinde **canonical digest'in bayt-özdeş kalacağı varsayılamaz** | §5 | Nötrlük yüzeyleri ayrıldı ve versiyonlandı | v0.3 §4.1, Prereg-A K-6 |
| **E-13** | Prereg-A pilot'u | `N=20` pilot **result-bearing** bir ölçümdür; kampanya içinde serbestçe koşulamaz | Prereg-A §8 | Ayrı mini-prereg, ayrı seed namespace, final analizden kesin dışlama | Prereg-A v0.1 §7 |
| **E-14** | K-2 (sıfır eligible ⇒ DUR-B) | Negatif kontrollerde (kayıpsız reordering, kısa akış) **ineligibility beklenen sonuçtur** | Prereg-A §5 | Hücreler üç strata ayrıldı; K-2 yalnız strata-1'e uygulanır | Prereg-A v0.1 §5/§6 |
| **E-15** | (yeni) | **`S-0`: estimator'ın ürün yolunda olduğu hiç doğrulanmadı.** `pub mod fate_estimator;` bir kez geri alınmıştı | S-0 | Tüm arkın premisi; diğer her sorudan önce | — |

---

## 2. Yeniden türetilen matematik

### 2.1 Varyans (E-6)

```
p̂11 = (n11 + α)/(L + 2α),   n11 ~ Bin(L, p11),   L = n10+n11
Var(p̂11) = p11(1−p11)·L/(L + 2α)²

Var(d) = p(1−p)·[ L/(L+2α)² + S/(S+2α)² ]          ← α'ya BAĞLI
```

`1/N`, `α = 0` limitidir. `L/(L+2α)² ≤ 1/L` (∀α ≥ 0) olduğundan **her α için üst sınırdır**:

| N (p=0.20) | `1/√N` (α=0) | α=0.5 | α=1 | α=2 |
|---:|---:|---:|---:|---:|
| 40 | 158 114 | 143 192 | 131 260 | 113 312 |
| 100 | 100 000 | 95 954 | 92 278 | 85 846 |
| 255 | **62 622** | 61 599 | **60 614** | 58 753 |

Kayda geçen yaklaşımlar: `p̂11 ⊥ p̂01` sayıldı (`L+S` sabit olduğu için tam bağımsız değiller) · küçük `L`'de normal yaklaşım · `α` kaynaktan doğrulanmadı (S-3).

### 2.2 `(N_DEC, LOSS_MIN)` dikdörtgeninin güvensiz köşeleri (E-1) — ⚠ **GEÇERSİZ**

> **Bu bölümün İKİ tablosu da geçersizdir** (Corrigendum-1 rev.2, C-1). Üreten fonksiyon `T`'yi hem karar cutoff'u hem de alternatif burst dağılımının gerçek ortalaması olarak kullanıyordu; bir dağılımın ortalaması cutoff'a eşitse örneklerinin ~yarısı cutoff'un altında kalır. Doğru kriter: `μ_IID + z·σ_IID < C < μ_B − z·σ_B`.
> Tablolar **silinmedi** — hangi sayının neden geri çekildiği izlenebilsin diye duruyor. **Hiçbiri alıntılanamaz.**
> Ayakta kalan: E-1'in *teşhisi* — iki bağımsız sabiti `AND`'lemek yanlış yapıdır, yüklem `N ≥ N_required(loss, T)` olmalıdır.

`LOSS_MIN = 160 000` varsayımıyla, `N ≥ N_DEC` yüklemesinin **yetmediği** loss noktaları (10k ızgara):

| | `N_DEC=200` | `N_DEC=225` | `N_DEC=240` |
|---|---|---|---|
| **T = 300 000** | 160k, 170k, 180k, 190k, 200k, **210k** | **160k** | yok |
| **T = 350 000** | yok | yok | yok |

v0.3'ün `(225, 160 000)` çifti tam olarak `loss = 160 000` köşesinde başarısız. Dikdörtgen kabul edilemez; yüklem `N ≥ N_required(loss, T)` olmalı.

`N_required(loss, T)`, z=2, α=1, tavan `N = 255`:

| loss ppm | T=280k | T=300k | T=320k | T=350k | T=400k |
|---:|---:|---:|---:|---:|---:|
| 100 000 | >255 | >255 | >255 | 220 | 166 |
| 140 000 | >255 | 247 | 217 | 181 | 135 |
| 160 000 | >255 | **231** | 203 | 168 | 126 |
| 180 000 | 251 | 218 | 191 | 158 | 118 |
| 200 000 | 240 | 208 | 182 | 150 | 112 |
| 250 000 | 219 | 190 | 165 | 136 | 101 |
| 300 000 | 206 | 178 | 155 | 127 | 94 |

### 2.3 `T` ile `N_required`'ın bağlılığı (E-2)

Tablo `T`'nin fonksiyonudur. `T` seçilmeden `N_required` dondurulamaz; `N_required` bilinmeden `T`'nin uygulanabilir olup olmadığı bilinemez. **İkisi tek bir ön-kayıtta birlikte dondurulur** (Prereg-B). Bu belgenin donmuş katkısı yalnız *fonksiyon*.

### 2.4 Erişilebilir zarf (E-10) — ✔ **AYAKTA, ve artık İSPATLI**

> Bu bölüm yazıldığında değerler yalnız **inşa** edilmişti (erişilebilirlik gösterilir, globallik gösterilmez). **Corrigendum-1 rev.2, C-2** uygulanabilir sayaç-vektörü uzayının tam sayımıyla bunları **ispatladı** — `d` yalnızca `(r, f, l)`'nin fonksiyonu, uzay `≤ 4L`. **Tüm değerler birebir aynı çıktı.** Statü: `CONSTRUCTED` → `PROVEN`.

Kapalı form kullanılmadı; her uç bir dizi kurularak ve sayaçlar sayılarak bulundu. Sınır yerleşimi (baş / iç / son) sonucu değiştiriyor.

| p | L | `d_min` ppm | sayaçlar (n00/n01/n10/n11) | `d_max` ppm | sayaçlar |
|---:|---:|---:|---|---:|---|
| 0.05 | 13 | **+9 290** | 229/13/13/0 | 920 408 | 242/1/0/12 |
| 0.10 | 26 | −81 169 | 203/26/26/0 | 954 342 | 229/1/0/25 |
| 0.15 | 38 | −153 082 | 179/38/38/0 | 965 268 | 217/1/0/37 |
| **0.20** | 51 | **−233 559** | 153/51/51/0 | **971 107** | 204/1/0/50 |
| 0.25 | 64 | −321 636 | 127/64/64/0 | 974 306 | 191/1/0/63 |
| 0.30 | 76 | −412 594 | 103/76/76/0 | 976 024 | 179/1/0/75 |
| 0.40 | 102 | −654 901 | 51/102/102/0 | 977 471 | 153/1/0/101 |
| 0.50 | 128 | **−984 556** | 0/128/127/0 | 976 863 | 127/0/1/127 |

Notlar: `d_min` her yerde **tam iç** dağıtımla (ilk ve son paket teslim) elde ediliyor · `d_max` `p ≤ 0.40`'ta **terminal run** ile, `p = 0.50`'de **baş run** ile · `p = 0.50`'de `n11 = 0` imkânsız (`L = 128 > 127`) ⇒ v0.3'ün `−992 308` değeri geçersiz.

**`p = 0.05`'te `d_min = +9 290 > 0`:** düşük kayıpta negatif okuma üretmek fiziksel olarak imkânsız.
**`d = −900 000 @ loss = 200 000` erişilemez** (taban −233 559) ⇒ v0.3'ün T11 girdisi geçersizdi.

---

## 3. Fixture denetimi

### 3.1 İsimlendirme (E-8)

`(153, 51, 51, 0)` **IID değildir** — 51 izole kaybın periyodik yerleşimidir, `n11 = 0` ile *anti-korele*. Gerçek IID ortalaması `p=0.20`'de `+8 408`; bu fixture `−233 559`, yani zarfın tabanı.

- Yeni ad: **`W-SCATTERED`**.
- **Bu fixture'ın sonucu "IID performansı" olarak yorumlanamaz.** T3b'nin kanıtladığı şey **yapı duyarlılığıdır**, IID kurtarma değil.
- Gerçek IID davranışı Prereg-A'nın **stokastik** IID dağılımından ölçülür. İstenirse ayrı, **önceden seçilmiş ve cherry-pick edilmemiş** seeded IID witness eklenebilir; seed ön-kayıtta yazılır.

### 3.2 Doğrulanan fixture'lar

Hepsi `W = 256`, `L = 51` (aynı kayıp sayısı, aynı ölçülen loss oranı), α = 1. Sayaçlar diziden **sayıldı**, formülden türetilmedi.

| fixture | yerleşim | n00/n01/n10/n11 | bas/son | `d` (tam kesir) | ppm |
|---|---|---|---|---|---:|
| **W-SCATTERED** | 51 izole, tam iç | 153/51/51/0 | 0/0 | `−1275/5459` | −233 559.26 |
| **W-CLUSTER** | 3'lük 17 run, iç | 187/17/17/34 | 0/0 | `+3128/5459` | +572 998.72 |
| **W-RUN-TERMINAL** | tek 51-run, sonda | 204/1/0/50 | 0/1 | `+10453/10764` | +971 107.40 |
| **W-RUN-INTERIOR** | tek 51-run, iç | 203/1/1/50 | 0/0 | `+5200/5459` | +952 555.41 |
| **W-RUN-LEADING** | tek 51-run, başta | 204/0/1/50 | 1/0 | `+10453/10918` | +957 409.78 |

`W-CLUSTER`'ın `+572 999`'u GE-L3'ün analitik değerine oturuyor ⇒ fixture bankalanmış veriyle çapraz doğrulanmış.
Üç run varyantı arasında **18 552 ppm** fark var — sınır yerleşimi ihmal edilemez.
`W-SCATTERED` de sınır-duyarlı: paket 1'den başlarsa `−228 705`, sonda kayıpla `−231 977`. **Fark ~5 000 ppm ⇒ fixture'lar örüntü adıyla değil POZİSYONLARLA tanımlanır.**

### 3.3 Beklenen değerin biçimi (E-11)

Yukarıdaki `d` değerleri **tam kesirdir**. Ppm tamsayısı, kaynağın sabit-nokta/yuvarlama kuralına bağlıdır ve **bilinmiyor** (S-3, S-6).

Kural: **fixture'ın beklenen değeri, kaynağın kendi tamsayı yolundan türetilir.** Float ile hesaplanmış bir beklenen değer Kural-2 ihlalidir. Bu belgenin rasyonelleri *referanstır*, beklenen değer değil.

---

## 4. Test ve mutasyon sözleşmesi (E-3, E-4)

### 4.1 Üç seviye, karıştırılmaz

| seviye | ne alır | ne pinler |
|---|---|---|
| **L1 — saf overlay birim** | evidence'ı **doğrudan** | karar fonksiyonunun sözleşmesi |
| **L2 — estimator aritmetik fixture** | fate dizisi → **gerçek estimator** | `n**` → `N` → `d` aritmetiği |
| **L3 — Sender wiring** | fate dizisi → gerçek estimator → **gerçek Sender** | kablonun bağlı olduğu |

L1 mutasyon kanıtı **üretemez** (evidence'ı elle alıyor). Mutasyon kanıtı yalnız L3'ten gelir.

### 4.2 T3b — merkezî test

Girdi: `W-SCATTERED` ve `W-CLUSTER`. **Aynı `L = 51`, aynı `N = 255`, aynı ölçülen loss, yalnız diziliş farklı.**
İddia: `parity_planned(W-SCATTERED) > parity_planned(W-CLUSTER)` ∧ `parity_planned(W-CLUSTER) == legacy`.
İddia sınırı: **yapı duyarlılığı**, IID kurtarma değil (E-8).
Boş geçemez: eşitsizlik iddia eder; sabit fonksiyon geçemez; ve yol gerçek estimator'dan geçer.

### 4.3 Mutasyon → tanık eşlemesi

| mutasyon | düşmesi gereken **L3** testi | neden bu test |
|---|---|---|
| `diag_loss_structure` → daima `None` | **T3b** | W-SCATTERED kolu parity açamaz olur |
| `diag_loss_structure` → zorla `Some{d}` sabit | **T3b** | W-CLUSTER kolu parity açar olur |
| `N` → sahte büyük değer | **T-SHORT-L3** (kısa akış tanığı) | kısa akış karar-eligible görünür |
| `rule_fired` → daima `true` | **T-NORULE-L3** | `no_rule` konfigürasyonunda parity çıkar |

Her test için **hangi yanlış implementasyonun onu düşüreceği** ayrıca yazılır; bu olmadan test non-vacuous sayılmaz.

**Bu turda fixture/test implementasyonu YAPILMAZ** — yalnız sözleşme yazıldı.

---

## 5. Nötrlük yüzeyleri (E-12)

"Enstrümantasyon eklenince her şey bayt-özdeş kalır" **varsayılamaz.** Yüzeyler ayrılır:

| yüzey | talep |
|---|---|
| ürün wire çıktısı | **bayt-özdeş** |
| parity / ARQ / pacing kararları | **bayt-özdeş** |
| önceden pinlenmiş semantik trace alanları | **bayt-özdeş** |
| yeni diagnostic alanlar (`N`, `n**`, `rule_fired`) | **değişmesi beklenir** — açıkça versiyonlanır |
| canonical state digest | **kaynak doğrulaması olmadan iddia edilmez**; diagnostic alan içeriyorsa digest **versiyonlanır**, "özdeş" denmez |

Karşılaştırıcı, dışlanan alanları **sabit isim listesiyle** dışlar, "ne farklıysa o" ile değil — aksi hâlde gerçek davranış farkı da sessizce elenir.

---

## 6. Bu errata'nın kapatMADIĞI

`S-0` · `S-1` · `S-2` · `S-7` · `S-3` (α, yuvarlama) · `S-4` (pencere/UNKNOWN) · `S-8` (`N` türetilebilirliği). Hepsi `NOT-DERIVABLE` — kaynak erişimi gerekiyor. **Hiçbiri varsayılan üretmedi.**
