**(a) Hüküm ACK'leri.** Süreler ms. Dönüş sıra beklemesi = (Alice arayüz girişi − Bob arayüz çıkışı) − IP boyu / 62 B/s − 300 ms (yakalamadan). Önce kaydında `verdict` alanı yok; karşılığı `raised` / `probe_failed` satırları ve `g_ns`.

| koşu | ACK sıra | an τ s | outcome | yalnız cephe ölçüsüyle sonuç (hesap) | expect/2 | abs_q | growth | interval_g | dönüş sıra beklemesi | alarm ACK'i | pace önce→sonra | yazılan kuyruk + ek B |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| önce | 16 | 69,871 | (alan yok) raised | – | – | – | – | -76,058 | 0,0 | H | 6223→6608 | yazılmadı |
| önce | 19 | 84,814 | (alan yok) probe_failed | – | – | – | – | 0,024 | 1 129,0 | E | 6608→6608 | yazılmadı |
| sonra · koşu 1 | 12 | 47,204 | Inconclusive | Inconclusive | 45,97 | 0,00 | 131,73 | 25,511 | 0,1 | H | 6224→6224 | yazılmadı |
| sonra · koşu 1 | 14 | 59,573 | Inconclusive | Inconclusive | 45,97 | 5,29 | 112,97 | 114,503 | 0,2 | H | 6224→6224 | yazılmadı |
| sonra · koşu 2 (yineleme) | 12 | 47,202 | Inconclusive | Inconclusive | 45,99 | 0,00 | 121,26 | 18,222 | 0,1 | H | 6223→6223 | yazılmadı |
| sonra · koşu 2 (yineleme) | 14 | 59,510 | Inconclusive | Inconclusive | 45,99 | 0,00 | 103,33 | 105,499 | 0,0 | H | 6223→6223 | yazılmadı |
| sonra · koşu 2 (yineleme) | 21 | 95,618 | Passed | Passed | 40,00 | 0,00 | 0,00 | 0,000 | 0,0 | H | 11199→11893 | 0 |

- önce: `probe_failed` satırı 1 (sıra 19: `g_ns` 24 181 ns, dönüş sıra beklemesi 1 129,0 ms); `raised` satırı 1.
- sonra · koşu 1: hüküm satırı 2; outcome dağılımı Inconclusive 2; `Failed` iken `interval_g_ns ≤ expect_ns/2` olan satır **0**; dönüş sıra beklemesi ≥ 100 ms olan hüküm satırı 0; yalnız cephe ölçüsüyle hesaplanan sonucu kayıttaki sonuçtan farklı satır 0.
- sonra · koşu 2 (yineleme): hüküm satırı 3; outcome dağılımı Inconclusive 2, Passed 1; `Failed` iken `interval_g_ns ≤ expect_ns/2` olan satır **0**; dönüş sıra beklemesi ≥ 100 ms olan hüküm satırı 0; yalnız cephe ölçüsüyle hesaplanan sonucu kayıttaki sonuçtan farklı satır 0.

**(b) Sahte tutma ayrımı.** yazılan = `backlog_bytes + extra_backlog_bytes` (boş = `null`, hat modeline kuyruk yazılmadı); gerçek = aynı anda darboğazda bekleyen bayt (yakalamadan: `MID_ma0 in` ≤ an < `MID_ifb0`; örnekleyicinin (`kuyruk.jsonl`) önceki / sonraki örneği yanında); sahte = yazılan > gerçek + 1 228 B (1 datagram). Yalnız kuyruk yazılan, dönüş sıra beklemesi ≥ 100 ms olan ya da lg anından sonra ≥ 0,5 s gönderim boşluğu izleyen satırlar; tam tablo ham dizinde `k258-ek.txt`.

| koşu | ACK sıra | an τ s | hüküm | front_wait ms | by_bytes / by_delay B | yazılan B | gerçek B / p | örnekleyici önce / sonra B | dönüş sıra beklemesi ms | sonraki veri datagramına dek s | sahte (kayıt) | sahte (türetilmiş) | sınıf |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| önce | 8 | 25,514 | – | – | – / – | yazılmadı | 14690 / 18 | 14690 / 14690 | 0,1 | 1,754 | – | hayır | – |
| önce | 9 | 32,298 | – | – | – / – | yazılmadı | 5530 / 6 | 3358 / 5530 | 0,1 | 0,851 | – | hayır | – |
| önce | 19 | 84,814 | – · probe_failed | – | – / – | yazılmadı | 0 / 0 | 0 / 0 | 1 129,0 | 2,163 | – | EVET | yoklama hükmü (probe_failed) |
| önce | 21 | 93,930 | – | – | – / – | yazılmadı | 0 / 0 | 0 / 0 | 1 133,3 | 1,054 | – | EVET | yoklamasız kuyruk tahmini |
| önce | 22 | 97,209 | – | – | – / – | yazılmadı | 0 / 0 | 0 / 0 | 699,7 | 0,023 | – | – | – |
| sonra · koşu 1 | 7 | 22,188 | – | 151,8 | 12 125 / 1 007 | 1007 (1007 + 0) | 13072 / 13 | 13072 / 13072 | 0,2 | 0,151 | hayır | – | – |
| sonra · koşu 1 | 8 | 25,515 | – | 1 756,6 | 13 990 / 10 933 | 10933 (10933 + 0) | 14690 / 18 | 14690 / 14690 | -0,0 | 1,756 | hayır | hayır | – |
| sonra · koşu 1 | 9 | 32,301 | – | 679,6 | 5 289 / 10 933 | 5289 (5289 + 0) | 5530 / 6 | 3358 / 5530 | 0,1 | 0,848 | hayır | hayır | – |
| sonra · koşu 1 | 15 | 63,949 | – · raised | 883,4 | 3 582 / 10 933 | 3582 (3582 + 0) | 0 / 0 | 0 / 0 | 1 141,1 | 0,285 | EVET | – | yoklamasız kuyruk tahmini |
| sonra · koşu 1 | 16 | 67,384 | – | 672,3 | – / – | yazılmadı | 0 / 0 | 0 / 0 | 1 036,7 | 0,039 | – | – | – |
| sonra · koşu 2 (yineleme) | 7 | 22,186 | – | 151,8 | 12 126 / 1 008 | 1008 (1008 + 0) | 13072 / 13 | 13072 / 13072 | 0,1 | 0,151 | hayır | – | – |
| sonra · koşu 2 (yineleme) | 8 | 25,513 | – | 1 756,2 | 14 001 / 10 929 | 10929 (10929 + 0) | 14690 / 18 | 14690 / 14690 | 0,2 | 1,755 | hayır | hayır | – |
| sonra · koşu 2 (yineleme) | 9 | 32,298 | – | 681,1 | 5 307 / 10 929 | 5307 (5307 + 0) | 5530 / 6 | 3358 / 5530 | 0,1 | 0,851 | hayır | hayır | – |
| sonra · koşu 2 (yineleme) | 15 | 63,886 | – · raised | 895,7 | 4 179 / 10 929 | 4179 (4179 + 0) | 0 / 0 | 0 / 0 | 1 141,3 | 0,333 | EVET | – | yoklamasız kuyruk tahmini |
| sonra · koşu 2 (yineleme) | 19 | 89,581 | – | 1 374,2 | 18 231 / 15 389 | 15389 (15389 + 0) | 2148 / 2 | 3234 / 2148 | 1 145,1 | 1,373 | EVET | EVET | yoklamasız kuyruk tahmini |
| sonra · koşu 2 (yineleme) | 21 | 95,618 | Passed · raised | 0,0 | 0 / 16 342 | 0 (0 + 0) | 0 / 0 | 0 / 0 | 0,0 | 0,058 | hayır | – | – |

| sayım | önce | sonra · koşu 1 | sonra · koşu 2 (yineleme) |
|---|---|---|---|
| `lg` satırı / kuyruk yazılan satır | 22 / alan yok | 21 / 4 | 21 / 6 |
| sahte tutma, kayıttan — yoklama hükmünden | alan yok | 0 | 0 |
| sahte tutma, kayıttan — yoklamasız kuyruk tahmininden | alan yok | 1 (sıra 15; yazılan − gerçek 3582 B = 0,287 s) | 2 (sıra 15, 19; yazılan − gerçek 4179 + 13241 B = 0,334 + 1,182 s) |
| sahte tutma, türetilmiş (boşluk ≥ 0,5 s) — yoklama hükmü satırında (önce: `probe_failed` / `raised`) | 1 (sıra 19: 2,163 s) | 0 | 0 |
| sahte tutma, türetilmiş — yoklamasız satırlarda | 1 (sıra 21: 1,054 s) | 0 | 1 (sıra 19: 1,373 s) |
| gönderici susması ≥ 0,5 s (τ ≥ 2 s): adet / toplam s | 4 / 6,127 | 2 / 2,662 | 3 / 4,125 |
| … darboğaz kuyruğu boşken (boş hat süresi ≥ 0,5 s): adet / süre toplamı s / boş hat süresi toplamı s | 2 / 3,463 / 3,387 | 0 / 0,000 / 0,000 | 1 / 1,458 / 1,229 |
| susmalar (başlangıç → bitiş τ s; başta kuyruk B; boş hat s) | 25,46 → 27,27; 14690; 0,000; 32,29 → 33,15; 4444; 0,000; 84,72 → 86,98; 0; 2,257; 93,78 → 94,98; 1074; 1,131 | 25,46 → 27,27; 13616; 0,000; 32,30 → 33,15; 5530; 0,000 | 25,46 → 27,27; 13616; 0,000; 32,29 → 33,15; 5530; 0,000; 89,50 → 90,95; 2160; 1,229 |

**(c) Önce ↔ sonra.**

| ölçü | önce | sonra · koşu 1 | sonra · koşu 2 (yineleme) |
|---|---|---|---|
| düşüş (20 s basamağı komut başlangıcı) → ilk tavan kesmesi ms | 2 185,7 | 2 187,6 | 2 185,6 |
| 20 s'den sonraki 1. / 2. / 3. ACK'te tavan (`pace_after`, B/s) | 6638 / 6223 / 6223 | 6638 / 6224 / 6224 | 6643 / 6223 / 6223 |
| … o ACK'lerin varış anı τ s | 22,19 / 25,51 / 32,30 | 22,19 / 25,52 / 32,30 | 22,19 / 25,51 / 32,30 |
| 20 s'den önce en düşük tavan B/s | 12500 | 12500 | 12500 |
| τ = 58 s'de tavan: sonda hat-eşdeğeri / son `lg` `pace_after` | 6 222 / 6223 | 6 223 / 6609 | 6 222 / 6608 |
| 60 s'den sonraki 1. / 2. / 3. ACK'te tavan B/s | 6608 / 6608 / 6608 | 12500 / 12500 / 12500 | 12500 / 12500 / 12500 |
| … o ACK'lerin varış anı τ s | 63,19 / 69,87 / 72,25 | 63,95 / 67,38 / 71,95 | 63,89 / 69,56 / 75,56 |
| 60 s'den koşu sonuna en büyük tavan: `lg` / sonda hat-eşdeğeri B/s | 7427 / 7 427 | 12500 / 12 500 | 12500 / 12 500 |
| [60, 85) s ACK'lerinde en düşük tavan (`pace_after`) | 6608 (ACK 15, τ=63,19) | 12500 (ACK 15, τ=63,95) | 12500 (ACK 15, τ=63,89) |
| 85 s'den koşu sonuna ACK'lerde en düşük tavan (`pace_after`) | 6608 (ACK 20, τ=88,33) | 12500 (ACK 19, τ=85,83) | 11199 (ACK 19, τ=89,58) |
| tavanın ≥ 11 875 olduğu ilk an τ s; 84 s'ye dek kalıcı | olmadı | 64,04; evet | 63,94; evet |
| [60, 85) s darboğaza sunulan: B/s; sunulan / kapasite | 6 533; %52,3 | 11 380; %91,0 | 11 383; %91,1 |
| [60, 85) s darboğazdan çıkan / kapasite; kuyrukta düşen | %53,0; 0 | %92,1; 0 | %92,1; 0 |
| [35, 58) s sunulan / kapasite; çıkan / kapasite; kuyrukta düşen | %100,2; %100,2; 0 | %100,9; %100,2; 0 | %100,9; %100,2; 0 |
| [35, 58) s darboğaz beklemesi ms: medyan / p95 / en çok (paket) | 235 / 398 / 438 (188) | 324 / 510 / 536 (188) | 316 / 473 / 527 (188) |
| [35, 58) s > 393 ms bekleyen paket | 14 | 29 | 28 |
| metin, kör pencere [20, 24) s: > 1 s olan / gönderilen (gönderim τ: ms) | 1/2 (τ=22: 2225) | 1/2 (τ=22: 2227) | 1/2 (τ=22: 2216) |
| metin [24, 60) s: > 1 s olan / gönderilen | 3/18 (τ=24: 2434, τ=26: 2106, τ=28: 1076) | 5/18 (τ=24: 2435, τ=26: 2108, τ=28: 1076, τ=50: 1045, τ=58: 1160) | 5/18 (τ=24: 2427, τ=26: 2099, τ=28: 1068, τ=50: 1039, τ=58: 1153) |
| metin [20, 60) s: ≤ 1 s olan / 20; en çok ms | 16/20; 2 434 | 14/20; 2 435 | 14/20; 2 427 |
| metin [0, 20) s: > 1 s olan / gönderilen | 0/9 | 0/9 | 0/9 |
| metin [60, 85) s: > 1 s olan / gönderilen | 0/13 | 1/13 (τ=62: 1003) | 1/13 (τ=62: 1016) |
| metin [85, bitiş) s: > 1 s olan / gönderilen | 2/7 (τ=86: 1280, τ=94: 1297) | 0/7 | 1/7 (τ=90: 1271) |
| metin teslim edilen / gönderilen (koşu) | 49/49 | 49/49 | 49/49 |
| dip [85,0, 85,5) s: darboğaza sunulan B / p; dip başında kuyruk B; dipte çıkan B | 0 / 0; 0; 0 → SINANMADI | 6444 / 6; 0; 3222 | 6456 / 6; 0; 3234 |
| [86,5, 98) s sunulan / kapasite; bekleme medyan / en çok ms | %47,3; 0 / 78 | %99,4; 224 / 227 | %83,6; 0 / 230 |
| Bob ACK'i / alarm ACK'i (`alarm:true`) / yükseliş (`rise:true`) | 23 / 6 / 1 | 21 / 4 / 1 | 21 / 3 / 1 |
| daralma algısı (`cum.drop_detects`, son ACK satırı); `cum.drop_alarms` / `rise_alarms` | 13; 6 / 1 | 14; 3 / 1 | 14; 2 / 1 |
| dönüş sıra beklemesi ≥ 100 ms olan ACK'ler (sıra: ms) | 19: 1 129, 21: 1 133, 22: 700 | 15: 1 141, 16: 1 037 | 15: 1 141, 19: 1 145 |
| darboğazdan çıkan paket; hatta düşen (%); kuyrukta düşen | 946; 178 (%18,82); 0 | 1152; 222 (%19,27); 0 | 1130; 216 (%19,12); 0 |
| dosya, Bob motor kabulü koşu sonunda B | 355 563 | 469 019 | 457 876 |

**(d) Beş hedefin bu koşudaki karşılığı** (PTT'siz teşhis; birleşik kabul değil; hüküm Web'in).

| hedef | önce | sonra · koşu 1 | sonra · koşu 2 (yineleme) |
|---|---|---|---|
| 1 CC (2. ACK'e dek tavan 5 625–6 875) | GEÇTİ — 2. ACK'te (τ=25,51) tavan 6223; 20 s'den önce en düşük 12500 | GEÇTİ — 2. ACK'te (τ=25,52) tavan 6224; 20 s'den önce en düşük 12500 | GEÇTİ — 2. ACK'te (τ=25,51) tavan 6223; 20 s'den önce en düşük 12500 |
| 2 PTT | SINANMADI — koşuda PTT yok | SINANMADI — koşuda PTT yok | SINANMADI — koşuda PTT yok |
| 3 Hız sınırlama | KALDI — [35, 58) sunulan / kapasite %100,2, çıkan / kapasite %100,2 (> %100; işaretlenmedi); kuyrukta düşen 0; ✗ bekleme en çok 438 ms, > 393 ms 14/188 paket; ✗ [20, 60) metin ≤ 1 s 16/20 (kör pencere [20, 24): 1/2); dip SINANMADI (dipte sunulan 0 B) | KALDI — [35, 58) sunulan / kapasite %100,9, çıkan / kapasite %100,2 (> %100; işaretlenmedi); kuyrukta düşen 0; ✗ bekleme en çok 536 ms, > 393 ms 29/188 paket; ✗ [20, 60) metin ≤ 1 s 14/20 (kör pencere [20, 24): 1/2); dip: sunulan 6444 B; 85 s'den sonra boş hatta susma 0; kesmesiz karşılaştırma kolu yok | KALDI — [35, 58) sunulan / kapasite %100,9, çıkan / kapasite %100,2 (> %100; işaretlenmedi); kuyrukta düşen 0; ✗ bekleme en çok 527 ms, > 393 ms 28/188 paket; ✗ [20, 60) metin ≤ 1 s 14/20 (kör pencere [20, 24): 1/2); dip: sunulan 6456 B; 85 s'den sonra boş hatta susma 1; kesmesiz karşılaştırma kolu yok |
| 4 Hızlanma (3. ACK'e dek tavan ≥ 11 875 ve kalıcı) | KALDI — önkoşul τ=58 tavan 6 222; 3. ACK'te (τ=72,25) tavan 6608; ≥ 11 875 ilk an olmadı; en büyük tavan 7427 | GEÇTİ — önkoşul τ=58 tavan 6 223; 3. ACK'te (τ=71,95) tavan 12500; ≥ 11 875 ilk an τ=64,04, 84 s'ye dek kalıcı: evet; en büyük tavan 12500 | GEÇTİ — önkoşul τ=58 tavan 6 222; 3. ACK'te (τ=75,56) tavan 12500; ≥ 11 875 ilk an τ=63,94, 84 s'ye dek kalıcı: evet; en büyük tavan 12500 |
| 5 Ürün/CI testleri | SINANMADI — bu adım test koşmadı, derleme yapmadı | SINANMADI — bu adım test koşmadı, derleme yapmadı | SINANMADI — bu adım test koşmadı, derleme yapmadı |
