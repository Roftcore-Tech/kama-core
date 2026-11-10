# mini-Ö v2 okuması

Ön kayıt: ONKAYIT-V2.md sha256 136f4bc457a04d5708a59cd0fdd971c9b23a26c313f9f1fd924f7b480ba3ca53 (denetlendi) · kol K1A6s · r25 tabanı K0. Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel; sığ tamponlu ağ gösterim kapsamı dışı. Değerler [ölçüldü] (ham veriden, F0 analizörleri), T önerisi ve kıyaslar [türetilmiş]. ∞ = teslim edilmedi (sansür).

## 1. Bağlayıcı dört ölçüt (Y10, 15 s basışı; ONKAYIT §3.4)

| hücre | n | rc=0 | başarılı | ilk demet ≤ 3 s | TEXT ≤ 10 s | dosya ≤ 30 s | kesintisiz | analitik D ≤ T (raporlanan) | eşlenmemiş ilk demet | T (ms) | W |
|---|---|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1A6s | 10 | 10 | **10** | 10 | 10 | 10 | 10 | 9 | 0 | 2000 | 5 |

## 2. EK-A §6 beş madde (hücre başına; basış = 15 s basışı; medyan [en kötü])

| hücre | ilk demet varışı s | bütün demetler varış p95 s | ilk duyulabilir ses s | D s (p95) | duraklama n / ms (toplam) | biriken gecikme son / en çok s | TEXT dosya sürerken en büyük / p95 s | dosya bitiş s |
|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1A6s | 0,81 [1,41] | 1,54 [2,17] | 2,82 [3,43] | 2,00 | 0 / 0 | -0,01 [0,01] / 0,00 [0,02] | 1,30 [1,84] / 1,30 [1,84] | 21,14 [26,35] |
| a_r12k_d300_l0_dosyali__K1A6s | 0,63 [0,74] | 0,50 [0,63] | 2,64 [2,75] | 0,24 | 0 / 0 | -0,01 [0,01] / 0,00 [0,01] | 0,65 [4,98] / 0,62 [1,91] | 83,54 [88,64] |
| a_r12k_d300_l30_dosyali__K1A6s | 2,31 [3,57] | 2,89 [7,04] | 4,33 [5,59] | 6,69 | 1 / 4688 | -0,01 [4,69] / 0,00 [4,69] | 5,98 [8,66] / 4,11 [4,92] | 155,44 [163,13] |

Madde 5 — olay/demet kaybı (toplam, hücre başına) ve kuyrukların en yüksek doluluğu:

| hücre | dinleyici düşüşü | iz düşüşlü koşu | O / C / P eksik | yinelenen | ayrışamayan | sırasız oynatma | motor kuyruğu en yüksek (out/txq/un/inf/pl) | bob Reorder tutulan en yüksek | T1 sayaçları |
|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1A6s | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 12/5/12/12/2 | 24 | tampon tepe 16 demet · tampon duraklama Σ 2 · atılan Σ 0 · şerit ret Σ 11204 · şerit düşen Σ 0 · şerit kuyruğu tepe (örn.) 10 · dosya penceresi tepe (örn.) 5 |
| a_r12k_d300_l0_dosyali__K1A6s | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 12/2/13/13/0 | 0 | tampon tepe 12 demet · tampon duraklama Σ 0 · atılan Σ 0 · şerit ret Σ 47501 · şerit düşen Σ 0 · şerit kuyruğu tepe (örn.) 10 · dosya penceresi tepe (örn.) 5 |
| a_r12k_d300_l30_dosyali__K1A6s | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 16/8/17/17/4 | 44 | tampon tepe 23 demet · tampon duraklama Σ 6 · atılan Σ 0 · şerit ret Σ 86137 · şerit düşen Σ 0 · şerit kuyruğu tepe (örn.) 10 · dosya penceresi tepe (örn.) 5 |

Kuyruk doluluğu ≈ 200 ms örneklemeyle (ts satırları) — gerçek tepe daha yüksek olabilir. T1 şerit kuyruğu ve oynatma tamponu doluluğu bu düzenekte kayıtlı değilse "eksik" (ONKAYIT §3.7).

## 3. T ve W teyidi (ONKAYIT §3.3, §3.5)

- T önerisi = p95(D) + 0,3 s, [1,5; 2,5]: D p95 = 2,00 s → **T = 2,30 s** (15 s basışı, n = 10); iki basış birlikte: D p95 = 2,00 s → 2,30 s (raporlanan).
- ACK'li birim kaybı (K1A6s kolunun bütün A hücreleri, koşu toplamı): 0 → **W = 5**.

| hücre | dcd | Reorder atlaması | süzülmeyen birim | teyitli ∧ alınmayan TEXT | teyitli ∧ alınmayan dosya |
|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1A6s | 0 | 0 | 0 | 0 | 0 |
| a_r12k_d300_l0_dosyali__K1A6s | 0 | 0 | 0 | 0 | 0 |
| a_r12k_d300_l30_dosyali__K1A6s | 0 | 0 | 0 | 0 | 0 |

## 4. K-3 (R3-r25; ONKAYIT §3.6)

- K1A6: n = 20, teslim@60 medyan 6,00, gönderim medyan 2556 B · K0: n = 20, teslim@60 medyan 6,00, gönderim medyan 2292 B → **hayır** (teslim@60 ≥ taban − 1 ∧ gönderim ≤ taban × 1,10).

## 5. G-K8 (d300 hücreleri; raporlanan, N = 10 → UCB yok, betimsel)

| hücre | tür | B1 ilk ses p95 s | B1 ilk demet p95 s | B1 eşik s | B2 60 s'de %100 koşu | B2 TEXT p95 s | B2 eşik s | B5 bozuk / sırasız | B5 D p95 s |
|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d300_l0_dosyali__K1A6s | iyi | 2,75 | 0,74 | 1,50 (+T ses için) | 10/10 | 0,61 | 2,00 | 0 / 0 | 0,44 |
| a_r12k_d300_l30_dosyali__K1A6s | kötü | 5,46 | 3,44 | 3,00 (+T ses için) | 10/10 | 4,34 | 10,00 | 0 / 0 | 2,00 |

B3: 150 KB ölçütü yalnız Y10'da (d300 hücrelerinde dosya 524 700 B, bitiş §2'de). B4 (60 s kesinti): bu hücrelerde yok → ölçülemedi.

## 6. EK-A §3.7 — callback + tur süresi (vekil) ve Kotlin tüketimi

| hücre | TEXT: iz R → dinleyici ms (n / p50 / p95 / en çok) | PTT: ilk oynatma − (varış + T) ms (n / p50 / p95 / en çok) |
|---|---|---|
| a_r12k_d150_l20_foto150k__K1A6s | 400 / 0,0 / 0,1 / 6,9 | 30 / 16,4 / 19,5 / 19,7 |
| a_r12k_d300_l0_dosyali__K1A6s | 400 / 0,0 / 0,0 / 0,1 | 30 / 15,0 / 18,6 / 19,4 |
| a_r12k_d300_l30_dosyali__K1A6s | 400 / 0,1 / 0,2 / 0,4 | 30 / 16,1 / 19,0 / 19,9 |

Düğüm turu süresi doğrudan ölçülmüyor (aynada yok); vekil = aynı turdaki iz R ile dinleyici çağrısı arası. Kotlin tüketimi bu düzenekte yok → ölçülemedi.

## 7. R3 koruma hücreleri (ONKAYIT-V2 §3.6, §3.11; taban K0 aynı kök ve ikili)

| hücre | kol | n | ölçü | kol medyanı | taban K0 medyanı | oran | R3 |
|---|---|---|---|---|---|---|---|
| b_r25_d300_l20_p65 | K1A6 | 20 / K0 20 | teslim@60 · gönderim B | 6,00 · 2556 | 6,00 · 2292 | 1,116 | **hayır** |
| b_r12k_d1000_l0_p1000 | K1A6 | 10 / K0 10 | teyit p95 s | 3,80 | 4,84 | 0,785 | **evet** |

r25 betimsel: bütün mesajlar 60 s içinde K1A6 13/20 · K0 17/20.

## 8. Madde 5 — T1 sayaçları (ONKAYIT-V2 §3.13; raporlanan)

| hücre | tampon tepe (demet) | T1 duraklama Σ / ms | atılan / geç elenen / taşma Σ | şerit ret Σ | şerit düşen / süresi dolan Σ | şerit kuyruğu tepe (örn.) | dosya penceresi tepe (örn.) |
|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1A6s | 16 | 2 / 119 | 0 / 0 / 0 | 11204 | 0 / 0 | 10 | 5 |
| a_r12k_d300_l0_dosyali__K1A6s | 12 | 0 / 0 | 0 / 0 / 0 | 47501 | 0 / 0 | 10 | 5 |
| a_r12k_d300_l30_dosyali__K1A6s | 23 | 6 / 6262 | 0 / 0 / 0 | 86137 | 0 / 0 | 10 | 5 |

Tepe değerleri ts satırlarından (≈ 200 ms); `po.peak` T1'in kendi en yüksek doluluk sayacıdır (koşu içi). T1 duraklama sayacı düğüm başına bütün akışları sayar; bağlayıcı duraklama ölçüsü §3.2'dir (KK-Ö-40).

## 9. Geçerlilik (ONKAYIT-V2 §3.12, §3.13)

- a_r12k_d150_l20_foto150k__K1A6s: başlık (iki uç) 10/10 beklenen set
- a_r12k_d300_l0_dosyali__K1A6s: başlık (iki uç) 10/10 beklenen set
- a_r12k_d300_l30_dosyali__K1A6s: başlık (iki uç) 10/10 beklenen set
- b_r12k_d1000_l0_p1000__K0: manifest 10/10 beklenen set, doğrulanamadı 0
- b_r12k_d1000_l0_p1000__K1A6: manifest 10/10 beklenen set, doğrulanamadı 0
- b_r25_d300_l20_p65__K0: manifest 20/20 beklenen set, doğrulanamadı 0
- b_r25_d300_l20_p65__K1A6: manifest 20/20 beklenen set, doğrulanamadı 0
- T1 sayaçları: K1A6s A koşularının hepsinde iki uçta var.
- Kol doğrulaması: **geçti**

## 10. "Yeşil" satırı (ONKAYIT-V2 §4 = v1 §3.8 ∧ R3-d1000)

- Y10 başarılı 10/10 (eşik ≥ 8): evet · ACK'li birim kaybı = 0: evet · R3-r25: hayır · R3-d1000: evet → **yeşil değil**

## 11. Ablasyon tetiği (ONKAYIT-V2 §6.1)

- r25: TETİK — SIRA-ABL-r25.txt · d1000: yok (R3 "veri yok" tetik değildir; koşu kaydı incelenir).
