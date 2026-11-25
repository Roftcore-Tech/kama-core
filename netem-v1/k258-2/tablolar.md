**(a) Hüküm satırları — b0659064 koşusu (kapsama.py çıktısı; τ s, farklar ms).**

HÜKÜM SATIRLARI — aralığın uçları ↔ yoklama penceresi (τ s; farklar ms)
| ACK sıra | hüküm an τ | outcome | first_seq–last_seq | ilk uç gönderim τ (k, IP boyu; D2 farkı ms) | son uç gönderim τ (k, IP boyu) | yoklama başlangıç τ (ACK sıra) | yoklama bitiş τ (süre ms) | ilk uç − başlangıç ms | son uç − bitiş ms | kapsıyor (ürün ölçütü: ilk ≤ başlangıç) | tamamını kapsıyor (ilk ≤ başlangıç VE son ≥ bitiş) | expect denetimi (hesap − kayıt ns) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 8 | 39.285 | Failed | 340–397 | 30.416 (k 349, 136 B; D2 farkı -0.158) | 37.034 (k 406, 1086 B; ACK'in largest_pn'i 427) | 32.256 (7) | 33.745 (1488.5) | -1840.4 | +3289.1 | EVET | EVET | +0 (taban 6215, tavan 6600; kayıtta pace_after 6600) |
| 18 | 89.833 | Inconclusive | 759–784 | 84.188 (k 768, 1074 B; D2 farkı +0.123) | 86.785 (k 793, 1074 B; ACK'in largest_pn'i 845) | 83.219 (17) | 84.531 (1312.1) | +969.1 | +2254.2 | HAYIR | HAYIR | +0 (taban 7050, tavan 7487; kayıtta pace_after 7487) |

HÜKÜM SATIRLARI — kuralın girdileri (kayıt) ve türetilen kapsama ile kural hesabı (ms)
| ACK sıra | outcome (kayıt) | expect | expect/2 | abs_q | growth | interval_g | mtu_ns (hesap) | expect + mtu | cephe geçti / kaldı | growth ≤ expect + mtu | kapsıyor (türetme) | kural hesabı (b0659064) | kayıtla aynı | yazılan backlog + ek B | taban × expect B (hesap) | pace önce→sonra | rate önce→sonra | probe_failed / raised |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 8 | Failed | 92.21 | 46.10 | 0.00 | 0.00 | 121.94 | 197.59 | 289.79 | EVET / HAYIR | EVET | EVET | Failed | evet | 0 + 1680 | 573 | 6215→6215 | 6215→6215 | True / False |
| 18 | Inconclusive | 81.33 | 40.67 | 970.92 | 1124.29 | 0.00 | 174.18 | 255.52 | HAYIR / EVET | HAYIR | HAYIR | Inconclusive | evet | 0 + 573 | 573 | 7050→7050 | 7050→7050 | False / False |

YOKLAMA BAŞLANGIÇ SATIRLARI (pace_after > pace_before VE raised = false): sıra 7 τ=32.256 6215→6600; sıra 17 τ=83.219 7050→7487; sıra 19 τ=93.409 7050→7487; sıra 21 τ=99.200 7465→7927

Kayıttaki girdilerle üç kuralın hesabı (eski_kural.py; eski ikililerle koşu değildir):

| ACK sıra | kayıttaki outcome (b0659064 ikilisi) | yalnız cephe (hesap) | 911f2c6d kuralı (hesap) | b0659064 kuralı (hesap) |
|---|---|---|---|---|
| 8 | Failed | Passed | Inconclusive | Failed |
| 18 | Inconclusive | Failed | Inconclusive | Inconclusive |

**(b) Sahte tutma ayrımı.** yazılan = `backlog_bytes + extra_backlog_bytes` (boş = `null`, hat modeline kuyruk yazılmadı); gerçek = aynı anda darboğazda bekleyen bayt (yakalamadan: `MID_ma0 in` ≤ an < `MID_ifb0`; örnekleyicinin (`kuyruk.jsonl`) önceki / sonraki örneği yanında); sahte = yazılan > gerçek + 1 228 B (1 datagram). Yalnız kuyruk yazılan, dönüş sıra beklemesi ≥ 100 ms olan ya da lg anından sonra ≥ 0,5 s gönderim boşluğu izleyen satırlar; tam tablo ham dizinde `k258-ek.txt`.

| koşu | ACK sıra | an τ s | hüküm | front_wait ms | by_bytes / by_delay B | yazılan B | gerçek B / p | örnekleyici önce / sonra B | dönüş sıra beklemesi ms | sonraki veri datagramına dek s | sahte (kayıt) | sahte (türetilmiş) | sınıf |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| önce | 8 | 25,514 | – | – | – / – | yazılmadı | 14690 / 18 | 14690 / 14690 | 0,1 | 1,754 | – | hayır | – |
| önce | 9 | 32,298 | – | – | – / – | yazılmadı | 5530 / 6 | 3358 / 5530 | 0,1 | 0,851 | – | hayır | – |
| önce | 19 | 84,814 | – · probe_failed | – | – / – | yazılmadı | 0 / 0 | 0 / 0 | 1 129,0 | 2,163 | – | EVET | yoklama hükmü (probe_failed) |
| önce | 21 | 93,930 | – | – | – / – | yazılmadı | 0 / 0 | 0 / 0 | 1 133,3 | 1,054 | – | EVET | yoklamasız kuyruk tahmini |
| önce | 22 | 97,209 | – | – | – / – | yazılmadı | 0 / 0 | 0 / 0 | 699,7 | 0,023 | – | – | – |
| 911f2c6d · koşu 1 | 7 | 22,188 | – | 151,8 | 12 125 / 1 007 | 1007 (1007 + 0) | 13072 / 13 | 13072 / 13072 | 0,2 | 0,151 | hayır | – | – |
| 911f2c6d · koşu 1 | 8 | 25,515 | – | 1 756,6 | 13 990 / 10 933 | 10933 (10933 + 0) | 14690 / 18 | 14690 / 14690 | -0,0 | 1,756 | hayır | hayır | – |
| 911f2c6d · koşu 1 | 9 | 32,301 | – | 679,6 | 5 289 / 10 933 | 5289 (5289 + 0) | 5530 / 6 | 3358 / 5530 | 0,1 | 0,848 | hayır | hayır | – |
| 911f2c6d · koşu 1 | 15 | 63,949 | – · raised | 883,4 | 3 582 / 10 933 | 3582 (3582 + 0) | 0 / 0 | 0 / 0 | 1 141,1 | 0,285 | EVET | – | yoklamasız kuyruk tahmini |
| 911f2c6d · koşu 1 | 16 | 67,384 | – | 672,3 | – / – | yazılmadı | 0 / 0 | 0 / 0 | 1 036,7 | 0,039 | – | – | – |
| 911f2c6d · koşu 2 | 7 | 22,186 | – | 151,8 | 12 126 / 1 008 | 1008 (1008 + 0) | 13072 / 13 | 13072 / 13072 | 0,1 | 0,151 | hayır | – | – |
| 911f2c6d · koşu 2 | 8 | 25,513 | – | 1 756,2 | 14 001 / 10 929 | 10929 (10929 + 0) | 14690 / 18 | 14690 / 14690 | 0,2 | 1,755 | hayır | hayır | – |
| 911f2c6d · koşu 2 | 9 | 32,298 | – | 681,1 | 5 307 / 10 929 | 5307 (5307 + 0) | 5530 / 6 | 3358 / 5530 | 0,1 | 0,851 | hayır | hayır | – |
| 911f2c6d · koşu 2 | 15 | 63,886 | – · raised | 895,7 | 4 179 / 10 929 | 4179 (4179 + 0) | 0 / 0 | 0 / 0 | 1 141,3 | 0,333 | EVET | – | yoklamasız kuyruk tahmini |
| 911f2c6d · koşu 2 | 19 | 89,581 | – | 1 374,2 | 18 231 / 15 389 | 15389 (15389 + 0) | 2148 / 2 | 3234 / 2148 | 1 145,1 | 1,373 | EVET | EVET | yoklamasız kuyruk tahmini |
| 911f2c6d · koşu 2 | 21 | 95,618 | Passed · raised | 0,0 | 0 / 16 342 | 0 (0 + 0) | 0 / 0 | 0 / 0 | 0,0 | 0,058 | hayır | – | – |
| b0659064 | 5 | 24,236 | – | 1 697,3 | 32 750 / 10 686 | 10686 (10686 + 0) | 26629 / 30 | 26629 / 26629 | 1 144,5 | 1,696 | hayır | hayır | – |
| b0659064 | 6 | 27,776 | – | 2 862,1 | 16 415 / 17 787 | 16415 (16415 + 0) | 17248 / 24 | 17248 / 17248 | 0,1 | 2,640 | hayır | hayır | – |
| b0659064 | 8 | 39,285 | Failed · probe_failed | 0,0 | 0 / 17 787 | 1680 (0 + 1680) | 2172 / 2 | 2172 / 2172 | 0,1 | 0,385 | hayır | – | – |
| b0659064 | 9 | 41,693 | – | 0,0 | 0 / 17 787 | 0 (0 + 0) | 1074 / 1 | 0 / 1074 | 0,1 | 0,162 | hayır | – | – |
| b0659064 | 10 | 45,140 | – | 173,5 | 0 / 17 787 | 0 (0 + 0) | 0 / 0 | 0 / 0 | -0,0 | 0,070 | hayır | – | – |
| b0659064 | 11 | 48,559 | – · raised | 1 311,8 | 7 544 / 17 787 | 7544 (7544 + 0) | 1086 / 1 | 2160 / 1086 | 1 135,2 | 0,602 | EVET | EVET | yoklamasız kuyruk tahmini |
| b0659064 | 12 | 51,999 | – | 449,8 | 18 558 / 15 546 | 15546 (15546 + 0) | 18447 / 18 | 17373 / 18447 | 0,1 | 3,100 | hayır | hayır | – |
| b0659064 | 13 | 53,176 | – | 811,6 | 10 456 / 15 546 | 10456 (10456 + 0) | 10788 / 10 | 10788 / 10788 | -0,0 | 1,924 | hayır | hayır | – |
| b0659064 | 15 | 65,708 | – | 1 064,3 | 6 662 / 9 247 | 6662 (6662 + 0) | 0 / 0 | 0 / 0 | 1 140,3 | 0,944 | EVET | EVET | yoklamasız kuyruk tahmini |
| b0659064 | 16 | 71,504 | – | 0,3 | 0 / 9 247 | 0 (0 + 0) | 0 / 0 | 0 / 0 | 0,1 | 0,174 | hayır | – | – |
| b0659064 | 18 | 89,833 | Inconclusive | 1 125,0 | – / – | 573 (0 + 573) | 0 / 0 | 0 / 0 | 1 125,2 | 0,190 | hayır | – | – |

| sayım | önce | 911f2c6d · koşu 1 | 911f2c6d · koşu 2 | b0659064 |
|---|---|---|---|---|
| `lg` satırı / kuyruk yazılan satır | 22 / alan yok | 21 / 4 | 21 / 6 | 21 / 11 |
| sahte tutma, kayıttan — yoklama hükmünden | alan yok | 0 | 0 | 0 |
| sahte tutma, kayıttan — yoklamasız kuyruk tahmininden | alan yok | 1 (sıra 15; yazılan − gerçek 3582 B = 0,287 s) | 2 (sıra 15, 19; yazılan − gerçek 4179 + 13241 B = 0,334 + 1,182 s) | 2 (sıra 11, 15; yazılan − gerçek 6458 + 6662 B = 0,517 + 0,945 s) |
| sahte tutma, türetilmiş (boşluk ≥ 0,5 s) — yoklama hükmü satırında (önce: `probe_failed` / `raised`) | 1 (sıra 19: 2,163 s) | 0 | 0 | 0 |
| sahte tutma, türetilmiş — yoklamasız satırlarda | 1 (sıra 21: 1,054 s) | 0 | 1 (sıra 19: 1,373 s) | 2 (sıra 11: 0,602 s, 15: 0,944 s) |
| gönderici susması ≥ 0,5 s (τ ≥ 2 s): adet / toplam s | 4 / 6,127 | 2 / 2,662 | 3 / 4,125 | 6 / 10,245 |
| … darboğaz kuyruğu boşken (boş hat süresi ≥ 0,5 s): adet / süre toplamı s / boş hat süresi toplamı s | 2 / 3,463 / 3,387 | 0 / 0,000 / 0,000 | 1 / 1,458 / 1,229 | 2 / 1,903 / 1,826 |
| susmalar (başlangıç → bitiş τ s; başta kuyruk B; boş hat s) | 25,46 → 27,27; 14690; 0,000; 32,29 → 33,15; 4444; 0,000; 84,72 → 86,98; 0; 2,257; 93,78 → 94,98; 1074; 1,131 | 25,46 → 27,27; 13616; 0,000; 32,30 → 33,15; 5530; 0,000 | 25,46 → 27,27; 13616; 0,000; 32,29 → 33,15; 5530; 0,000; 89,50 → 90,95; 2160; 1,229 | 24,21 → 25,93; 25555; 0,000; 27,60 → 30,42; 18334; 0,000; 48,46 → 49,16; 1074; 0,456; 51,99 → 55,10; 18447; 0,271; 65,45 → 66,65; 0; 1,128; 82,52 → 83,22; 0; 0,697 |

**(c) Önce ↔ sonra.**

| ölçü | önce | 911f2c6d · koşu 1 | 911f2c6d · koşu 2 | b0659064 |
|---|---|---|---|---|
| düşüş (20 s basamağı komut başlangıcı) → ilk tavan kesmesi ms | 2 185,7 | 2 187,6 | 2 185,6 | 4 236,3 |
| 20 s'den sonraki 1. / 2. / 3. ACK'te tavan (`pace_after`, B/s) | 6638 / 6223 / 6223 | 6638 / 6224 / 6224 | 6643 / 6223 / 6223 | 6296 / 6215 / 6600 |
| … o ACK'lerin varış anı τ s | 22,19 / 25,51 / 32,30 | 22,19 / 25,52 / 32,30 | 22,19 / 25,51 / 32,30 | 24,24 / 27,78 / 32,26 |
| 20 s'den önce en düşük tavan B/s | 12500 | 12500 | 12500 | 12500 |
| τ = 58 s'de tavan: sonda hat-eşdeğeri / son `lg` `pace_after` | 6 222 / 6223 | 6 223 / 6609 | 6 222 / 6608 | 5 431 / 5432 |
| 60 s'den sonraki 1. / 2. / 3. ACK'te tavan B/s | 6608 / 6608 / 6608 | 12500 / 12500 / 12500 | 12500 / 12500 / 12500 | 7050 / 7050 / 7050 |
| … o ACK'lerin varış anı τ s | 63,19 / 69,87 / 72,25 | 63,95 / 67,38 / 71,95 | 63,89 / 69,56 / 75,56 | 63,45 / 65,71 / 71,50 |
| 60 s'den koşu sonuna en büyük tavan: `lg` / sonda hat-eşdeğeri B/s | 7427 / 7 427 | 12500 / 12 500 | 12500 / 12 500 | 7927 / 7 926 |
| [60, 85) s ACK'lerinde en düşük tavan (`pace_after`) | 6608 (ACK 15, τ=63,19) | 12500 (ACK 15, τ=63,95) | 12500 (ACK 15, τ=63,89) | 7050 (ACK 14, τ=63,45) |
| 85 s'den koşu sonuna ACK'lerde en düşük tavan (`pace_after`) | 6608 (ACK 20, τ=88,33) | 12500 (ACK 19, τ=85,83) | 11199 (ACK 19, τ=89,58) | 7050 (ACK 18, τ=89,83) |
| tavanın ≥ 11 875 olduğu ilk an τ s; 84 s'ye dek kalıcı | olmadı | 64,04; evet | 63,94; evet | olmadı |
| [60, 85) s darboğaza sunulan: B/s; sunulan / kapasite | 6 533; %52,3 | 11 380; %91,0 | 11 383; %91,1 | 6 521; %52,2 |
| [60, 85) s darboğazdan çıkan / kapasite; kuyrukta düşen | %53,0; 0 | %92,1; 0 | %92,1; 0 | %52,2; 0 |
| [35, 58) s sunulan / kapasite; çıkan / kapasite; kuyrukta düşen | %100,2; %100,2; 0 | %100,9; %100,2; 0 | %100,9; %100,2; 0 | %95,8; %96,6; 0 |
| [35, 58) s darboğaz beklemesi ms: medyan / p95 / en çok (paket) | 235 / 398 / 438 (188) | 324 / 510 / 536 (188) | 316 / 473 / 527 (188) | 156 / 1 953 / 2 816 (186) |
| [35, 58) s > 393 ms bekleyen paket | 14 | 29 | 28 | 36 |
| metin, kör pencere [20, 24) s: > 1 s olan / gönderilen (gönderim τ: ms) | 1/2 (τ=22: 2225) | 1/2 (τ=22: 2227) | 1/2 (τ=22: 2216) | 1/2 (τ=22: 2267) |
| metin [24, 60) s: > 1 s olan / gönderilen | 3/18 (τ=24: 2434, τ=26: 2106, τ=28: 1076) | 5/18 (τ=24: 2435, τ=26: 2108, τ=28: 1076, τ=50: 1045, τ=58: 1160) | 5/18 (τ=24: 2427, τ=26: 2099, τ=28: 1068, τ=50: 1039, τ=58: 1153) | 6/18 (τ=24: 4184, τ=26: 2927, τ=28: 2766, τ=50: 1825, τ=52: 3419, τ=54: 1459) |
| metin [20, 60) s: ≤ 1 s olan / 20; en çok ms | 16/20; 2 434 | 14/20; 2 435 | 14/20; 2 427 | 13/20; 4 184 |
| metin [0, 20) s: > 1 s olan / gönderilen | 0/9 | 0/9 | 0/9 | 0/9 |
| metin [60, 85) s: > 1 s olan / gönderilen | 0/13 | 1/13 (τ=62: 1003) | 1/13 (τ=62: 1016) | 1/13 (τ=82: 1555) |
| metin [85, bitiş) s: > 1 s olan / gönderilen | 2/7 (τ=86: 1280, τ=94: 1297) | 0/7 | 1/7 (τ=90: 1271) | 0/7 |
| metin teslim edilen / gönderilen (koşu) | 49/49 | 49/49 | 49/49 | 49/49 |
| dip [85,0, 85,5) s: darboğaza sunulan B / p; dip başında kuyruk B; dipte çıkan B | 0 / 0; 0; 0 → SINANMADI | 6444 / 6; 0; 3222 | 6456 / 6; 0; 3234 | 4473 / 5; 0; 3387 |
| [86,5, 98) s sunulan / kapasite; bekleme medyan / en çok ms | %47,3; 0 / 78 | %99,4; 224 / 227 | %83,6; 0 / 230 | %57,2; 0 / 78 |
| Bob ACK'i / alarm ACK'i (`alarm:true`) / yükseliş (`rise:true`) | 23 / 6 / 1 | 21 / 4 / 1 | 21 / 3 / 1 | 21 / 7 / 1 |
| daralma algısı (`cum.drop_detects`, son ACK satırı); `cum.drop_alarms` / `rise_alarms` | 13; 6 / 1 | 14; 3 / 1 | 14; 2 / 1 | 13; 6 / 1 |
| dönüş sıra beklemesi ≥ 100 ms olan ACK'ler (sıra: ms) | 19: 1 129, 21: 1 133, 22: 700 | 15: 1 141, 16: 1 037 | 15: 1 141, 19: 1 145 | 5: 1 145, 11: 1 135, 15: 1 140, 18: 1 125 |
| darboğazdan çıkan paket; hatta düşen (%); kuyrukta düşen | 946; 178 (%18,82); 0 | 1152; 222 (%19,27); 0 | 1130; 216 (%19,12); 0 | 963; 180 (%18,69); 0 |
| dosya, Bob motor kabulü koşu sonunda B | 355 563 | 469 019 | 457 876 | 372 784 |

**(d) Beş hedefin bu koşudaki karşılığı** (PTT'siz teşhis; birleşik kabul değil; hüküm Web'in).

| hedef | önce | 911f2c6d · koşu 1 | 911f2c6d · koşu 2 | b0659064 |
|---|---|---|---|---|
| 1 CC (2. ACK'e dek tavan 5 625–6 875) | GEÇTİ — 2. ACK'te (τ=25,51) tavan 6223; 20 s'den önce en düşük 12500 | GEÇTİ — 2. ACK'te (τ=25,52) tavan 6224; 20 s'den önce en düşük 12500 | GEÇTİ — 2. ACK'te (τ=25,51) tavan 6223; 20 s'den önce en düşük 12500 | GEÇTİ — 2. ACK'te (τ=27,78) tavan 6215; 20 s'den önce en düşük 12500 |
| 2 PTT | SINANMADI — koşuda PTT yok | SINANMADI — koşuda PTT yok | SINANMADI — koşuda PTT yok | SINANMADI — koşuda PTT yok |
| 3 Hız sınırlama | KALDI — [35, 58) sunulan / kapasite %100,2, çıkan / kapasite %100,2 (> %100; işaretlenmedi); kuyrukta düşen 0; ✗ bekleme en çok 438 ms, > 393 ms 14/188 paket; ✗ [20, 60) metin ≤ 1 s 16/20 (kör pencere [20, 24): 1/2); dip SINANMADI (dipte sunulan 0 B) | KALDI — [35, 58) sunulan / kapasite %100,9, çıkan / kapasite %100,2 (> %100; işaretlenmedi); kuyrukta düşen 0; ✗ bekleme en çok 536 ms, > 393 ms 29/188 paket; ✗ [20, 60) metin ≤ 1 s 14/20 (kör pencere [20, 24): 1/2); dip: sunulan 6444 B; 85 s'den sonra boş hatta susma 0; kesmesiz karşılaştırma kolu yok | KALDI — [35, 58) sunulan / kapasite %100,9, çıkan / kapasite %100,2 (> %100; işaretlenmedi); kuyrukta düşen 0; ✗ bekleme en çok 527 ms, > 393 ms 28/188 paket; ✗ [20, 60) metin ≤ 1 s 14/20 (kör pencere [20, 24): 1/2); dip: sunulan 6456 B; 85 s'den sonra boş hatta susma 1; kesmesiz karşılaştırma kolu yok | KALDI — [35, 58) sunulan / kapasite %95,8, çıkan / kapasite %96,6; kuyrukta düşen 0; ✗ bekleme en çok 2 816 ms, > 393 ms 36/186 paket; ✗ [20, 60) metin ≤ 1 s 13/20 (kör pencere [20, 24): 1/2); dip: sunulan 4473 B; 85 s'den sonra boş hatta susma 0; kesmesiz karşılaştırma kolu yok |
| 4 Hızlanma (3. ACK'e dek tavan ≥ 11 875 ve kalıcı) | KALDI — önkoşul τ=58 tavan 6 222; 3. ACK'te (τ=72,25) tavan 6608; ≥ 11 875 ilk an olmadı; en büyük tavan 7427 | GEÇTİ — önkoşul τ=58 tavan 6 223; 3. ACK'te (τ=71,95) tavan 12500; ≥ 11 875 ilk an τ=64,04, 84 s'ye dek kalıcı: evet; en büyük tavan 12500 | GEÇTİ — önkoşul τ=58 tavan 6 222; 3. ACK'te (τ=75,56) tavan 12500; ≥ 11 875 ilk an τ=63,94, 84 s'ye dek kalıcı: evet; en büyük tavan 12500 | SINANMADI — önkoşul τ=58 tavan 5 431 |
| 5 Ürün/CI testleri | SINANMADI — bu adım test koşmadı, derleme yapmadı | SINANMADI — bu adım test koşmadı, derleme yapmadı | SINANMADI — bu adım test koşmadı, derleme yapmadı | SINANMADI — bu adım test koşmadı, derleme yapmadı |
