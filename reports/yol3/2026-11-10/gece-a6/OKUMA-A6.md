# A-6 gece ölçümü okuması (betimsel; eşik yok)

kökler: 2026-11-10-a6 · etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta · N < 30: betimsel · tanımlar scripts/yol3/tesis/oku_a6.py belgesinde

## 1. Tamamlanma (rc = 0 ≠ tamamlandı)

dosya teslim = bob dosyayı tam ve doğru aldı; alındı = alice `file.delivered` (tel debisi penceresinin ucu; teslimden az olabilir).

| hücre | kol | netem limit | n | rc = 0 | dosya teslim | alındı | PTT kapandı | eksik koşular |
|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K1 (--e1-arm K1, şeritsiz) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | K1A6 (--e1-arm K1A6, şeritsiz) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | K1A (--e1-arm K1A, şeritsiz) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | K1s (--e1-arm K1, şeritli) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | K1A6s (--e1-arm K1A6, şeritli) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | Q_bbr_rfc (quinn Q_bbr_rfc) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | Q_cubic (quinn Q_cubic) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d150_l20_foto150k | K1A6s_qlim (--e1-arm K1A6, şeritli) | 6 | 10 | 10/10 | 6/10 | 5/10 | 10/10 | rep2 (dosya failed); rep5 (dosya failed); rep8 (dosya failed); rep9 (dosya failed) |
| a_r12k_d150_l20_foto150k | Q_bbr_rfc_qlim (quinn Q_bbr_rfc) | 6 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d300_l30_dosyali | K1 (--e1-arm K1, şeritsiz) | 1000 | 10 | 10/10 | 8/10 | 0/10 | 10/10 | rep1 (dosya failed); rep3 (dosya failed) |
| a_r12k_d300_l30_dosyali | K1A6 (--e1-arm K1A6, şeritsiz) | 1000 | 10 | 10/10 | 6/10 | 0/10 | 10/10 | rep2 (dosya failed); rep3 (dosya failed); rep6 (dosya failed); rep9 (dosya failed) |
| a_r12k_d300_l30_dosyali | K1A (--e1-arm K1A, şeritsiz) | 1000 | 10 | 10/10 | 7/10 | 0/10 | 10/10 | rep1 (dosya failed); rep4 (dosya failed); rep7 (dosya failed) |
| a_r12k_d300_l30_dosyali | K1s (--e1-arm K1, şeritli) | 1000 | 10 | 10/10 | 6/10 | 1/10 | 10/10 | rep1 (dosya failed); rep2 (dosya none, alice incomplete); rep4 (dosya none, alice incomplete); rep5 (dosya none, alice incomplete) |
| a_r12k_d300_l30_dosyali | K1A6s (--e1-arm K1A6, şeritli) | 1000 | 10 | 10/10 | 10/10 | 0/10 | 10/10 | — |
| a_r12k_d300_l30_dosyali | Q_bbr_rfc (quinn Q_bbr_rfc) | 1000 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |
| a_r12k_d300_l30_dosyali | Q_cubic (quinn Q_cubic) | 1000 | 10 | 10/10 | 10/10 | 2/10 | 10/10 | — |
| a_r12k_d300_l30_dosyali | K1A6s_qlim (--e1-arm K1A6, şeritli) | 12 | 10 | 10/10 | 9/10 | 1/10 | 10/10 | rep6 (dosya failed) |
| a_r12k_d300_l30_dosyali | Q_bbr_rfc_qlim (quinn Q_bbr_rfc) | 12 | 10 | 10/10 | 10/10 | 10/10 | 10/10 | — |

## 2. Dosya

Tel debisi ve retx payı: pencere alice file.sent → file.delivered (ayristirma_util mantığı); quinn retx = kayıp ilan edilen bayt / UDP gönderilen bayt (vekil). t(%10): gönderici txb (yeniden gönderim dahil), alıcı ilerlemesi değil; çözünürlük 200 ms.

| hücre | kol | dosya bitiş med / p90 / en kötü (s) | faydalı debi (kB/s) | tel debisi med (B/s) [n] | retx payı med | t(%10) med (s) [gönderici txb; quinn: sokete yazma anı, tel ilerlemesi değil] | ileri netem düşüş payı med |
|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K1 | 19.6 / 21.1 / 23.1 | 7.67 | 10124 [10] | 0.196 | 5.5 | 0.192 |
| a_r12k_d150_l20_foto150k | K1A6 | 17.4 / 19.7 / 20.0 | 8.66 | 11560 [10] | 0.199 | 1.1 | 0.206 |
| a_r12k_d150_l20_foto150k | K1A | 20.0 / 21.1 / 21.1 | 7.51 | 10027 [10] | 0.191 | 2.1 | 0.202 |
| a_r12k_d150_l20_foto150k | K1s | 31.5 / 37.9 / 41.0 | 4.78 | 6489 [10] | 0.183 | 3.6 | 0.193 |
| a_r12k_d150_l20_foto150k | K1A6s | 22.0 / 26.0 / 27.5 | 6.84 | 9354 [10] | 0.195 | 1.5 | 0.200 |
| a_r12k_d150_l20_foto150k | Q_bbr_rfc | 17.0 / 18.9 / 25.9 | 8.86 | 11114 [10] | 0.193 | 0.1 | 0.208 |
| a_r12k_d150_l20_foto150k | Q_cubic | 25.2 / 31.8 / 35.2 | 5.96 | 7280 [10] | 0.192 | 1.1 | 0.188 |
| a_r12k_d150_l20_foto150k | K1A6s_qlim | 91.1 / ∞ / ∞ | 1.65 | 8191 [5] | 0.427 | 1.7 | 0.445 |
| a_r12k_d150_l20_foto150k | Q_bbr_rfc_qlim | 16.4 / 17.0 / 20.4 | 9.17 | 12748 [10] | 0.346 | 0.1 | 0.274 |
| a_r12k_d300_l30_dosyali | K1 | 77.7 / ∞ / ∞ | 6.75 | — [0] | — | 12.5 | 0.302 |
| a_r12k_d300_l30_dosyali | K1A6 | 84.6 / ∞ / ∞ | 6.20 | — [0] | — | 5.1 | 0.306 |
| a_r12k_d300_l30_dosyali | K1A | 75.8 / ∞ / ∞ | 6.92 | — [0] | — | 6.7 | 0.299 |
| a_r12k_d300_l30_dosyali | K1s | 358.4 / ∞ / ∞ | 1.46 | 2640 [1] | 0.302 | 15.3 | 0.299 |
| a_r12k_d300_l30_dosyali | K1A6s | 147.4 / 173.6 / 176.4 | 3.56 | — [0] | — | 9.0 | 0.295 |
| a_r12k_d300_l30_dosyali | Q_bbr_rfc | 49.7 / 54.6 / 56.9 | 10.56 | 15270 [10] | 0.303 | 3.1 | 0.308 |
| a_r12k_d300_l30_dosyali | Q_cubic | 197.2 / 236.0 / 286.5 | 2.66 | 3509 [2] | 0.281 | 12.4 | 0.286 |
| a_r12k_d300_l30_dosyali | K1A6s_qlim | 158.9 / 180.9 / ∞ | 3.30 | 5132 [1] | 0.343 | 8.7 | 0.403 |
| a_r12k_d300_l30_dosyali | Q_bbr_rfc_qlim | 50.9 / 54.7 / 56.2 | 10.32 | 34839 [10] | 0.693 | 3.1 | 0.652 |

## 3. PTT — üç ayrı ölçü (EK-A §10.4)

Anlar alice demet gönderiminden; karşı örnek basışı hariç. Varış: Kama = bob Transport → Application (oynatma tamponu öncesi); quinn = bob stream okuma (tampon yok). İlk duyulabilir ses = s-kollarında ilk demetin bob PttBundle olayı (oynatma tamponu sonrası); şeritsiz ve quinn kollarında —.

| hücre | kol | ilk demet varışı med / en kötü (s) | bütün demetler varış p95 med / en kötü (s) | ilk duyulabilir ses med / en kötü (s) | teslim edilmeyen demet (toplam) |
|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K1 | 9.44 / 12.89 | 9.04 / 12.94 | — / — | 0 |
| a_r12k_d150_l20_foto150k | K1A6 | 7.23 / 9.78 | 6.91 / 9.38 | — / — | 0 |
| a_r12k_d150_l20_foto150k | K1A | 9.83 / 10.98 | 9.48 / 10.58 | — / — | 0 |
| a_r12k_d150_l20_foto150k | K1s | 0.95 / 2.83 | 1.96 / 6.15 | 2.96 / 4.84 | 0 |
| a_r12k_d150_l20_foto150k | K1A6s | 0.84 / 1.41 | 1.38 / 2.37 | 2.85 / 3.43 | 0 |
| a_r12k_d150_l20_foto150k | Q_bbr_rfc | 3.45 / 6.63 | 2.89 / 5.91 | — / — | 0 |
| a_r12k_d150_l20_foto150k | Q_cubic | 0.87 / 1.92 | 1.66 / 4.47 | — / — | 0 |
| a_r12k_d150_l20_foto150k | K1A6s_qlim | 36.98 / 155.70 | 44.38 / 199.08 | 38.99 / 157.71 | 0 |
| a_r12k_d150_l20_foto150k | Q_bbr_rfc_qlim | 1.25 / 2.41 | 1.52 / 2.73 | — / — | 0 |
| a_r12k_d300_l30_dosyali | K1 | 67.50 / 87.88 | 67.10 / 87.48 | — / — | 0 |
| a_r12k_d300_l30_dosyali | K1A6 | 68.10 / 78.84 | 67.70 / 78.44 | — / — | 0 |
| a_r12k_d300_l30_dosyali | K1A | 65.63 / 87.25 | 65.23 / 86.85 | — / — | 0 |
| a_r12k_d300_l30_dosyali | K1s | 3.72 / 16.95 | 6.54 / 16.55 | 5.73 / 18.96 | 0 |
| a_r12k_d300_l30_dosyali | K1A6s | 1.68 / 2.86 | 3.68 / 6.91 | 3.69 / 4.88 | 0 |
| a_r12k_d300_l30_dosyali | Q_bbr_rfc | 4.14 / 5.84 | 13.18 / 30.06 | — / — | 0 |
| a_r12k_d300_l30_dosyali | Q_cubic | 4.12 / 13.85 | 5.39 / 25.51 | — / — | 0 |
| a_r12k_d300_l30_dosyali | K1A6s_qlim | 2.35 / 6.60 | 6.56 / 95.61 | 4.37 / 8.61 | 0 |
| a_r12k_d300_l30_dosyali | Q_bbr_rfc_qlim | 2.51 / 4.79 | 5.13 / 10.43 | — / — | 0 |

## 4. A-6 deney kaydı (EK-A §10.2)

Etkinleşti = dosya penceresinde (file.sent → file.delivered, yoksa koşu sonu) tabanın hızı yükselttiği karar sayısı (`up`) arttı; etkinleşmeyen koşu A-6 ölçülmedi sayılır. Anlar file.sent'e göre (eksi = dosyadan önce): ilk yükseltme ts örneğinden (≤ 200 ms geç); kurulma, kalkış ve tam tur `o_ms` varsa kesin (kesin). Kalkış = SON kalkış. Isınmada tam tur = ilk nitelikli tam tur (kapı kapanışı) dosyadan önce; r0 = ilk kurulmada tur izleyicisi tam tur kapatmıştı.

| hücre | kol | aynada | etkinleşti | etkinleşme (ilk yükseltme) med (s) | ilk kurulma med (s) | kalkış med (s) | kalkış nedeni | ısınmada tam tur | r0 | A-6 ölçülmedi |
|---|---|---|---|---|---|---|---|---|---|---|
| a_r12k_d150_l20_foto150k | K1A6 | 10/10 | 8/10 | 0.09 | -4.48 | 1.40 | FullRound×10 | 2/10 | 0/10 | rep1, rep7 |
| a_r12k_d150_l20_foto150k | K1A6s | 10/10 | 8/10 | 0.09 | -4.18 | 13.66 | FullRound×8, kalkmadı×2 | 2/10 | 0/10 | rep4, rep6 |
| a_r12k_d150_l20_foto150k | K1A6s_qlim | 10/10 | 7/10 | 0.08 | -4.18 | 5.04 | FullRound×10 | 3/10 | 0/10 | rep2, rep6, rep9 |
| a_r12k_d300_l30_dosyali | K1A6 | 10/10 | 6/10 | 0.09 | -4.22 | 1.53 | FullRound×10 | 3/10 | 0/10 | rep1, rep2, rep3, rep9 |
| a_r12k_d300_l30_dosyali | K1A6s | 10/10 | 4/10 | 0.08 | -4.09 | -1.17 | FullRound×10 | 6/10 | 0/10 | rep1, rep5, rep6, rep7, rep8, rep10 |
| a_r12k_d300_l30_dosyali | K1A6s_qlim | 10/10 | 5/10 | 0.08 | -4.37 | 3.86 | FullRound×10 | 1/10 | 0/10 | rep2, rep3, rep4, rep6, rep9 |

## 5. b hücreleri — R3 (oku.py tanımı aynen)

R3 ölçüleri: d1000 l0 hücresinde teyit p95; r25 hücresinde teslim@60 ve gönderim (D = 60 s iki hücrede). Diğer sütunlar betimsel.

| hücre | kol | n | rc = 0 | bütün mesajlar D içinde | teyit p95 med (s) | teslim@D med (adet / mesaj) | gönderim med (B) | R3 (taban K0) |
|---|---|---|---|---|---|---|---|---|
| b_r12k_d1000_l0_p1000 | K1 | 10 | 10/10 | 10/10 | 5.09 | 100.0 / 100 | 104754 | True (K0 p95 5.45; aynı kök ve ikili) |
| b_r12k_d1000_l0_p1000 | K1A6 | 10 | 10/10 | 10/10 | 3.44 | 100.0 / 100 | 104754 | True (K0 p95 5.45; aynı kök ve ikili) |
| b_r12k_d1000_l0_p1000 | K0 | 10 | 10/10 | 10/10 | 5.45 | 100.0 / 100 | 104754 | taban |
| b_r25_d300_l20_p65 | K1 | 10 | 10/10 | 9/10 | 44.20 | 6.0 / 6 | 2386 | True (K0 teslim 6.0, gönderim 2610; aynı kök ve ikili) |
| b_r25_d300_l20_p65 | K1A6 | 10 | 10/10 | 7/10 | 51.86 | 6.0 / 6 | 2546 | True (K0 teslim 6.0, gönderim 2610; aynı kök ve ikili) |
| b_r25_d300_l20_p65 | K1A | 10 | 10/10 | 8/10 | 54.62 | 6.0 / 6 | 2616 | True (K0 teslim 6.0, gönderim 2610; aynı kök ve ikili) |
| b_r25_d300_l20_p65 | K0 | 10 | 10/10 | 9/10 | 48.44 | 6.0 / 6 | 2610 | taban |
