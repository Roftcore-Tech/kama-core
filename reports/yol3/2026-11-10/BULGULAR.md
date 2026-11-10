# YOL3 gece ölçümü 2026-11-10 — A-6, şeritler, quinn, küçük kuyruk, R3, mini-Ö

Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta. N = 10 < 30: **betimsel**. K1 ve A paketi (A-1…A-6) bayrak arkasında deneydir, varsayılan kapalı; bu rapor onay değildir (sahip kararı v4 EK-A §10.5). Etiketler: [ölçüldü] koşulmuş veri · [türetilmiş] ölçülenden hesap · [çıkarım] doğrulanmamış açıklama.

## 1. Ne koşuldu

- Dayanak: sahip kararı v4 EK-A §9.3 (7 kol), §10.3 (küçük kuyruk ayrı kol), §10.4 (PTT üç ayrı ölçü), §2.1 (R3 koruması K1'de).
- Ağaç `yol2/goster-a6` e332991b = `yol2/goster-a` 56313c73 (şeritler T1 + A-1…A-5 + kol merdiveni) + A-6 (`yol3/q2-quinn-pacer-prob` e9992229) + düzenek. quinn kolu `yol3/quinn-kol` d93aef10 (önceki koşunun ikilisi).
- Kollar: `K1` (gösterim seti, şeritsiz) · `K1A6` (K1 + A-1…A-6) · `K1A` (K1 + A-1…A-5) · `K1s` (K1 + şeritler) · `K1A6s` (K1 + A + şeritler) · `Q_bbr_rfc` · `Q_cubic`. Şeritler = `--goster on`: S1 şeritleri W = 5, PTT oynatma tamponu T = 2,0 s.
- Hücreler: gösterim analoğu `a_r12k_d150_l20_foto150k` (12,5 kB/s, 2 × 150 ms, %20, 150 KB dosya) · `a_r12k_d300_l30_dosyali` (2 × 300 ms ± 60 ms, %30, 524 KB) · `b_r25_d300_l20_p65` · `b_r12k_d1000_l0_p1000`. Netem kuyruğu 1000 paket (mevcut profil). Küçük kuyruk kolu `limit` = floor(2 × rate × RTT / 1200 B): 6 ve 12 paket; netem `limit` gecikme hattındaki paketleri de sayar.
- 25 hücre × 10 = 250 koşu, `f0_grid -j 11`, FEC kapalı. 03:44–04:42, 250/250 rc = 0. Kurulan netem dizgisi her hücrede `realized_*.txt` ile doğrulandı (`limit 6`, `limit 12`, `limit 1000`).
- Okuma `scripts/yol3/tesis/oku_a6.py` (dal `yol2/goster-a6`); tam tablo `gece-a6/OKUMA-A6.md`.

## 2. Gösterim analoğu hücresi [ölçüldü]

| kol | dosya bitiş med / p90 / en kötü (s) | ilk demet varışı med / en kötü (s) | bütün demetler varış p95 med (s) | ilk duyulabilir ses med / en kötü (s) |
|---|---|---|---|---|
| K1 | 19,6 / 21,1 / 23,1 | 9,44 / 12,89 | 9,04 | — |
| K1A6 | 17,4 / 19,7 / 20,0 | 7,23 / 9,78 | 6,91 | — |
| K1A | 20,0 / 21,1 / 21,1 | 9,83 / 10,98 | 9,48 | — |
| K1s | 31,5 / 37,9 / 41,0 | 0,95 / 2,83 | 1,96 | 2,96 / 4,84 |
| K1A6s | 22,0 / 26,0 / 27,5 | 0,84 / 1,41 | 1,38 | 2,85 / 3,43 |
| Q_bbr_rfc | 17,0 / 18,9 / 25,9 | 3,45 / 6,63 | 2,89 | — |
| Q_cubic | 25,2 / 31,8 / 35,2 | 0,87 / 1,92 | 1,66 | — |

Hepsi 10/10 teslim. Varış = bob Transport → Application (tampon öncesi); ilk duyulabilir ses = tampon sonrası ilk `PttBundle`, yalnız şeritli kollarda.

- Gösterim ölçütü "ilk demet varışı ≤ 3 s": şeritli iki Kama kolunda 10 koşunun en kötüsü 2,83 s (K1s) ve 1,41 s (K1A6s).
- Gösterim ölçütü "dosya ≤ 30 s": **K1s medyanı 31,5 s (p90 37,9 s).** K1A6s en kötü 27,5 s.
- Dinleyicinin duyduğu gecikme "≤ 3 s + T" etiketiyle: K1A6s ilk duyulabilir ses medyanı 2,85 s, en kötü 3,43 s.
- Şeritler PTT'yi dosyanın arkasından çıkarıyor, dosyayı yavaşlatıyor (K1 19,6 → K1s 31,5 s). A paketi şeritli ağaçta dosyayı geri hızlandırıyor (K1A6s 22,0 s). Aynı yön dünkü ölçümde de vardı (K1 30,3 / K1A 21,2 s, `2026-11-09/goster-a`).

## 3. A-6 (Startup pace tabanı 10·MTU/srtt) [ölçüldü / türetilmiş]

- Şeritsiz: dosya K1 19,6 · K1A 20,0 · K1A6 17,4 s. Gönderici `txb`'sinin dosyanın %10'una ulaşması (t(%10), gönderici sayacı, alıcı ilerlemesi değil): K1 5,5 · K1A 2,1 · K1A6 1,1 s. Başlangıç rampasındaki etki görünüyor. Dosya süresindeki fark, K1'in koşular arası oynamasıyla (dün 19,2 / 20,4 s, bugün 19,6 s) ayrılamıyor.
- Etkinleşme (EK-A §10.2 deney kaydı): tabanın dosya penceresinde hızı yükselttiği koşu, gösterim hücresinde K1A6 8/10, K1A6s 8/10; d300 l30 hücresinde 6/10 ve 4/10. Etkinleşmeyen koşu sayısı, ısınma TEXT'leri sırasında uygulamayla-sınırlı-olmayan tam turun zaten görüldüğü (kapının kapandığı) koşu sayısıyla çoğu hücrede örtüşüyor (gösterim hücresi K1A6: 2 ve 2; d300 K1A6: 4 ve 3); koşu koşu eşleştirme yapılmadı. Etkinleşmeyen koşular "A-6 ölçülmedi" sayıldı. Kalkış nedeni her yerde `FullRound` (iki koşuda kalkmadı).
- A-6'nın "ısınmadan öğrenilen uygulamayla-sınırlı bant Startup'ı tohumlamaz" kuralı kodda `max` olarak okundu: düşük ısınma bandı hızı tabanın altına çekemiyor, daha yüksek bir ısınma örneği yine sayılıyor. Katı okuma tek satırlık değişiklik; bu hücrede fark beklenmiyor [çıkarım].

## 4. Küçük kuyruk [ölçüldü]

| hücre (limit) | kol | dosya bitiş med (s) | dosya teslim | ilk demet varışı med / en kötü (s) | netem düşüş payı med |
|---|---|---|---|---|---|
| gösterim analoğu (6) | K1A6s | 91,1 | 6/10 | 36,98 / 155,70 | 0,445 |
| gösterim analoğu (6) | Q_bbr_rfc | 16,4 | 10/10 | 1,25 / 2,41 | 0,274 |
| d300 l30 (12) | K1A6s | 158,9 | 9/10 | 2,35 / 6,60 | 0,403 |
| d300 l30 (12) | Q_bbr_rfc | 50,9 | 10/10 | 2,51 / 4,79 | 0,652 |

- Kama 6 paketlik kuyrukta çöküyor: netem'in düşürdüğü pay %20 kaybın üstüne %44'e çıkıyor, dosya 91 s, 4/10 teslim yok, PTT 37 s. quinn aynı kuyrukta 16,4 s.
- Açıklama adayı [çıkarım]: Kama teslim hızıyla pace ediyor ve INV-RATE-1 gereği kayıpla hızı düşürmüyor; sığ kuyrukta taşma kaybı bir tıkanıklık sinyali, Kama ise onu rastgele kayıp gibi işliyor. Bu, karşılaştırma belgesindeki "sığ tampon koruması CC-v2 sinyalleriyle" satırına düşüyor (`docs/design/QUINN-FARKI-KOD-KARSILASTIRMA-2026-11-09.md` §1 "Kayıp → hız"). Mekanizma doğrulanmadı.
- Gösterim `tc` profili değişmedi (EK-A §10.3). Gerçek yolun tamponu sığsa gösterim hücresi sonucu orada geçerli olmaz.

## 5. `a_r12k_d300_l30_dosyali` (524 KB, %30, 600 ms, jitter) [ölçüldü]

- Dosya bitiş medyanı: K1 77,7 · K1A6 84,6 · K1A 75,8 · K1s 358,4 · K1A6s 147,4 · Q_bbr_rfc 49,7 · Q_cubic 197,2 s.
- Kama'nın şeritsiz kollarında 2–4/10, K1s'te 4/10 dosya teslim edilmedi. Neden uygulama katmanında: alice dosyayı 60 s'de `Outgoing(ReceiptTimeout)` ile kapatıyor; bob o koşularda ya başarısız oluyor (≈ 80 s) ya da daha geç alıyor (≈ 78 s). "alındı" sütunu bu yüzden Kama kollarında 0–1/10. Bu, taşıma değil ürünün dosya süresi politikası; 60 s'yi aşan her aktarım bu hücrede uygulama hatası verir.
- PTT: şeritsiz kollarda ilk demet 65–68 s (dosyanın arkasında). Şeritli K1A6s'te 1,68 s (en kötü 2,86), bütün demetler p95 3,68 s.

## 6. R3 koruması K1'de (EK-A §2.1) [ölçüldü]

| hücre | kol | ölçü | değer | taban K0 (aynı ikili) | R3 |
|---|---|---|---|---|---|
| d1000 l0 | K1 | teyit p95 med | 5,09 s | 5,45 s | geçti |
| d1000 l0 | K1A6 | teyit p95 med | 3,44 s | 5,45 s | geçti |
| r25 | K1 | teslim@60 / gönderim | 6/6 · 2386 B | 6/6 · 2610 B | geçti |
| r25 | K1A6 | teslim@60 / gönderim | 6/6 · 2546 B | 6/6 · 2610 B | geçti |

Betimsel not: r25'te "bütün mesajlar D içinde" K0 9/10, K1 9/10, K1A 8/10, K1A6 7/10.

## 7. quinn kolu notları

- Gösterim hücresinde Q_bbr_rfc'nin PTT'si bu koşuda dünkünden yüksek (bütün demetler p95 2,89 s; dün 1,51 s). Dünkü quinn koşusu `-j 3`, bugünkü `-j 11` paralellikle koştu. Fark ayrıştırılmadı; yorumlanmıyor.
- d300 limit 12'de quinn tel debisi 34,8 kB/s, netem düşüşü %65; yine 50,9 s'de 10/10 bitiriyor.

## 7b. mini-Ö (EK-A §6) [ölçüldü]

- Ağaç `yol2/goster` 1ee9ee68 = 9e157992 + T1 oynatma tamponu v2 (9273e5b2) + Ö mini-Ö düzeneği (c45d81e4). Kol K1 (EK-A §2 gösterim seti) + şeritler (W = 5) + tampon (T = 2,0 s). İki ayrı süreç (alice/bob, iki netns). 5 hücre × 10 = 50 koşu, 04:45–05:39, 50/50 rc = 0. Ön kayıt `scripts/yol3/mini-o/ONKAYIT.md` sha256 `8a96ca0a…7a8c` koşudan önce donmuştu; okuma ön kayda bağlı (`mini-o/OKUMA.md`).
- Gösterim hücresi (15 s basışı), EK-A §6'nın beş maddesi:

| madde | sonuç (medyan [en kötü]) |
|---|---|
| 1. ilk demet varışı (bağlayıcı ≤ 3 s) / ilk duyulabilir ses | 1,10 [1,99] s · 10/10 ≤ 3 s / 3,12 [4,00] s |
| 2. oynatma duraklaması / biriken gecikme | 10 koşuda toplam 1 duraklama, 761 ms / en çok 0,02 s |
| 3. dosya sürerken TEXT teslimi (≤ 10 s) | en büyük 4,04 [5,50] s · 10/10 |
| 4. dosya bitişi (≤ 30 s) | 28,5 [34,1] s · 8/10 |
| 5. olay/demet kaybı / kuyruk tepe doluluğu | dinleyici düşüşü 0, eksik O/C/P 0, yinelenen 0, sırasız oynatma 0 / motor kuyruğu en çok 14 birim (≈ 200 ms örnekleme); T1 şerit kuyruğu ve tampon doluluğu ayna satırında **yok** (ölçülemedi) |

- Dört ölçütü birden sağlayan koşu 8/10. Ön kayıtlı "yeşil" satırı: Y10 8/10 (eşik ≥ 8) evet · ACK'li birim kaybı 0 evet · R3-r25 **hayır** (gönderim K1 2786 B ↔ K0 1996 B, eşik ×1,10) → **yeşil değil**. Aynı ölçü gece ızgarasında (§6) geçmişti: K1 2386 B ↔ K0 2610 B. r25 gönderimi iki koşu arasında (K0 1996 ↔ 2610 B) çelişiyor; N = 10'da bu ölçü kararlı değil.
- EK-A §3.7 vekili: ilk oynatma − (varış + T) p95 19,8 ms (bir düğüm turu, 20 ms); TEXT iz → dinleyici p95 0,1 ms. Kotlin tüketimi bu düzenekte yok.
- T önerisi (ön kayıt §3.3): D p95 2,77 s → T = 2,50 s (aralığın üst ucu). W = 5 (ACK'li birim kaybı 0).
- d300 hücreleri: l0 dosya 93,9 s (524 KB); l30 dosya teslim edilmedi (60 s uygulama zaman aşımı, §5), ilk demet 3,31 [74,55] s.

## 8. Kararlara girdi (karar değil)

1. **Gösterim seti:** EK-A §2 seti K1. Şeritli K1 "dosya ≤ 30 s" ölçütünde sınırda: gece ızgarasında medyan 31,5 s, mini-Ö'de 28,5 s (8/10). K1 + A + şeritler ölçütleri bu hücrede sağlıyor (dosya en kötü 27,5 s, ilk demet en kötü 1,41 s). A paketi deneydir; kabulü KAPI-MANİFESTİ koşullarına bağlı (EK-A §10.5).
2. **Sığ tampon:** Kama'nın 6 paketlik kuyrukta çökmesi gösterim yolu için risk; ölçülen gerçek yol tamponu bilinmiyor.
3. **mini-Ö yeşil değil:** yalnız R3-r25 yüzünden; o ölçü iki koşu arasında çelişiyor (§6, §7b).
4. **T1 sayaçları:** şerit kuyruğu ve tampon doluluğu ayna satırına yazılmıyor; mini-Ö madde 5'in yarısı ölçülemedi.
5. **A-6 kapısı:** gösterim hücresinde 10 koşunun 2'sinde taban dosya penceresinde hiç devreye girmedi; aday neden ısınmada görülen tam tur (§3).

## 9. Yeniden üretme

- Hücreler `scripts/yol3/tesis/cells-a6/`, sıra `scripts/yol3/tesis/SIRA-A6.txt` (sha256 `c99196ba2cd1ab1fd22512320d6af0cce0626540d6b7d82b7ab2e0c088f782c8`), üretici `gen_cells.py --set a6`, dal `yol2/goster-a6` e332991b.
- İkililer: `f0_flows` 980f1c34…, `bench` 7e6eab9a…, quinn `bench` 26a702dc… (`gece-a6/KOSU-KAYDI.txt`).
- mini-Ö: hücreler `scripts/yol3/mini-o/` (dal `yol2/goster` 1ee9ee68), kayıt `mini-o/KOSU-KAYDI.txt`, okuma `mini-o/OKUMA.md`.
- Ham veri `ham/gece-a6/*.tar.zst`, `ham/mini-o/*.tar.zst`, sha256 `ham-veri.sha256`. Metin dosyalarındaki makine yolları yer tutucuyla değiştirildi (`<ölçüm-kökü>`, `<worktree-kökü>`).
