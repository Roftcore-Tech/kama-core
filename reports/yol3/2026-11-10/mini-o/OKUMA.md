# mini-Ö okuması

Ön kayıt: ONKAYIT.md sha256 8a96ca0ab660e83a09be9f17e5b3aadde48c17e2e3ad9e6a1f1cc39c2c657a8c (denetlendi) · kol K1 · r25 tabanı K0. Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta; N < 30 betimsel. Değerler [ölçüldü] (ham veriden, F0 analizörleri), T önerisi ve kıyaslar [türetilmiş]. ∞ = teslim edilmedi (sansür).

## 1. Bağlayıcı dört ölçüt (Y10, 15 s basışı; ONKAYIT §3.4)

| hücre | n | rc=0 | başarılı | ilk demet ≤ 3 s | TEXT ≤ 10 s | dosya ≤ 30 s | kesintisiz | analitik D ≤ T (raporlanan) | eşlenmemiş ilk demet | T (ms) | W |
|---|---|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1 | 10 | 10 | **8** | 10 | 10 | 8 | 9 | 9 | 0 | 2000 | 5 |

## 2. EK-A §6 beş madde (hücre başına; basış = 15 s basışı; medyan [en kötü])

| hücre | ilk demet varışı s | bütün demetler varış p95 s | ilk duyulabilir ses s | D s (p95) | duraklama n / ms (toplam) | biriken gecikme son / en çok s | TEXT dosya sürerken en büyük / p95 s | dosya bitiş s |
|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1 | 1,10 [1,99] | 1,75 [2,41] | 3,12 [4,00] | 2,77 | 1 / 761 | -0,00 [0,01] / 0,00 [0,02] | 4,04 [5,50] / 4,04 [5,50] | 28,51 [34,08] |
| a_r12k_d300_l0_dosyali__K1 | 0,62 [0,82] | 0,52 [0,81] | 2,64 [2,83] | 2,81 | 1 / 807 | -0,00 [0,01] / 0,00 [0,02] | 3,49 [3,67] / 1,43 [3,02] | 93,85 [97,38] |
| a_r12k_d300_l30_dosyali__K1 | 3,31 [74,55] | 6,48 [74,15] | 5,33 [76,56] | 4,16 | 5 / 4869 | 0,01 [2,16] / 0,02 [2,16] | 13,86 [82,69] / 9,99 [77,69] | ∞ [∞] |

Madde 5 — olay/demet kaybı (toplam, hücre başına) ve kuyrukların en yüksek doluluğu:

| hücre | dinleyici düşüşü | iz düşüşlü koşu | O / C / P eksik | yinelenen | ayrışamayan | sırasız oynatma | motor kuyruğu en yüksek (out/txq/un/inf/pl) | bob Reorder tutulan en yüksek | T1 sayaçları |
|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 14/8/14/14/3 | 36 | eksik (ts satırında `t1` yok) |
| a_r12k_d300_l0_dosyali__K1 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 15/7/13/13/0 | 0 | eksik (ts satırında `t1` yok) |
| a_r12k_d300_l30_dosyali__K1 | 0 | 0 | 0 / 0 / 0 | 0 | 0 | 0 | 98/96/98/34/24 | 79 | eksik (ts satırında `t1` yok) |

Kuyruk doluluğu ≈ 200 ms örneklemeyle (ts satırları) — gerçek tepe daha yüksek olabilir. T1 şerit kuyruğu ve oynatma tamponu doluluğu bu düzenekte kayıtlı değilse "eksik" (ONKAYIT §3.7).

## 3. T ve W teyidi (ONKAYIT §3.3, §3.5)

- T önerisi = p95(D) + 0,3 s, [1,5; 2,5]: D p95 = 2,77 s → **T = 2,50 s** (15 s basışı, n = 10); iki basış birlikte: D p95 = 1,98 s → 2,28 s (raporlanan).
- ACK'li birim kaybı (K1 kolunun bütün A hücreleri, koşu toplamı): 0 → **W = 5**; belirsiz koşu (alice çıkışta teyitsiz birim > 0 ∧ süzülmeyen birim > 0, sayılmadı): a_r12k_d300_l30_dosyali__K1/rep1, a_r12k_d300_l30_dosyali__K1/rep2, a_r12k_d300_l30_dosyali__K1/rep5, a_r12k_d300_l30_dosyali__K1/rep7, a_r12k_d300_l30_dosyali__K1/rep8, a_r12k_d300_l30_dosyali__K1/rep10.

| hücre | dcd | Reorder atlaması | süzülmeyen birim | teyitli ∧ alınmayan TEXT | teyitli ∧ alınmayan dosya |
|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k__K1 | 0 | 0 | 0 | 0 | 0 |
| a_r12k_d300_l0_dosyali__K1 | 0 | 0 | 0 | 0 | 0 |
| a_r12k_d300_l30_dosyali__K1 | 0 | 0 | 21 | 0 | 0 |

## 4. K-3 (R3-r25; ONKAYIT §3.6)

- K1: n = 10, teslim@60 medyan 6,00, gönderim medyan 2786 B · K0: n = 10, teslim@60 medyan 6,00, gönderim medyan 1996 B → **hayır** (teslim@60 ≥ taban − 1 ∧ gönderim ≤ taban × 1,10).

## 5. G-K8 (d300 hücreleri; raporlanan, N = 10 → UCB yok, betimsel)

| hücre | tür | B1 ilk ses p95 s | B1 ilk demet p95 s | B1 eşik s | B2 60 s'de %100 koşu | B2 TEXT p95 s | B2 eşik s | B5 bozuk / sırasız | B5 D p95 s |
|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d300_l0_dosyali__K1 | iyi | 2,78 | 0,78 | 1,50 (+T ses için) | 10/10 | 1,04 | 2,00 | 0 / 0 | 0,33 |
| a_r12k_d300_l30_dosyali__K1 | kötü | 33,16 | 31,14 | 3,00 (+T ses için) | 9/10 | 35,14 | 10,00 | 0 / 0 | 4,16 |

B3: 150 KB ölçütü yalnız Y10'da (d300 hücrelerinde dosya 524 700 B, bitiş §2'de). B4 (60 s kesinti): bu hücrelerde yok → ölçülemedi.

## 6. EK-A §3.7 — callback + tur süresi (vekil) ve Kotlin tüketimi

| hücre | TEXT: iz R → dinleyici ms (n / p50 / p95 / en çok) | PTT: ilk oynatma − (varış + T) ms (n / p50 / p95 / en çok) |
|---|---|---|
| a_r12k_d150_l20_foto150k__K1 | 400 / 0,0 / 0,1 / 6,4 | 30 / 16,3 / 19,8 / 20,1 |
| a_r12k_d300_l0_dosyali__K1 | 400 / 0,0 / 0,1 / 0,1 | 30 / 14,8 / 19,8 / 20,0 |
| a_r12k_d300_l30_dosyali__K1 | 400 / 0,1 / 0,2 / 0,4 | 30 / 6,3 / 19,7 / 19,9 |

Düğüm turu süresi doğrudan ölçülmüyor (aynada yok); vekil = aynı turdaki iz R ile dinleyici çağrısı arası. Kotlin tüketimi bu düzenekte yok → ölçülemedi.

## 7. "Yeşil" satırı (ONKAYIT §3.8; dondurma şartı Paz yeniden koşusunda)

- Y10 başarılı 8/10 (eşik ≥ 8): evet · ACK'li birim kaybı = 0: evet · R3-r25: hayır → **yeşil değil**
