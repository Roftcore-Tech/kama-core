# F0 taban tablosu — bugünkü motor, ürün hedeflerine karşı (2026-10-30)

**Kollar:** taban = main `a858f3fc`, aday = 05b `49864510` (ikisine de aynı ölçüm kodu), quinn (BBR, akış başına uni-stream), tcp (cubic, tek bağlantı).
**Veri:** 96 hücre, 10 090 koşu; rc = 0 10 088, zaman aşımı 2 (betimsel stres hücresi). Hüküm hücreleri N = 30/kol.
**İki ölçüm yolu:** S1 gerçek ürün yolu (`f0_flows`: Noise, 22 B zarf, oturum sıralaması). S2–S4 bench yolu: kripto ve oturum sıralaması yok, bu yüzden Kama için **üst sınırdır**.
**Etiketler:** [ölçüldü], [kod], [türetilmiş], [çıkarım], [tahmin]. Etiketsiz sayılar ölçülmüştür. Taban / aday ayrımı "/" ile verilir.

## Karar

| soru | cevap |
|---|---|
| (a) Mevcut motor dar düzeltmelerle hedefleri karşılar mı? | **Yalnız dar düzeltmeyle hayır.** Dar işler 25 B/s'yi ve ACK'li düşüşü hedefler; K-1 ve K-6'ya dokunmaz. Motor kuralları ve sözleşme değişiklikleriyle birlikte beklenen etki hedefe yaklaşıyor [tahmin], ama ölçülmedi. |
| (b) KTP-2'ye geçmek daha mı doğru? | **F0 bunu desteklemiyor, reddetmiyor da.** Motorun atılmasını gerektiren bulgu yok. En büyük motor kusuru olan gönderen kilidini KTP-2'nin de taşıması bekleniyor, çünkü §3.5 aynı örnekleyici ve bant filtresini devralıyor [çıkarım; TASLAK-NOTLARI V-25]. Kilidi süren RecoverArq kuralı taslakta yok; bu yüzden sonuç kesin değil. Akış başına sıra ve küçük FEC blokları gerçekten gerekli, ama bugünkü motorda sözleşme değişikliğiyle de yapılabilir [kod]. Süre tahminleri yakın: KTP-2 taslağı 11–15 iş günü, F1 planı 13–18 iş günü (ikisi de tahmin). |
| (c) Hangi değişiklikler? | Aşağıdaki bulgu tablosu; uygulama sırası `F1-PLANI.md`. |

**Önerim:** kademeli yol. Önce tel ve sözleşme değişmeyen Aşama 1 (dar + motor kuralları) yapılır ve aynı hücrelerde ölçülür. Hız ölçümü KTP-2 için de girdidir: aynı filtrenin kilidi taşıyıp taşımadığını gösterir. Sözleşme aşaması mı KTP-2 mi kararı bu ölçümden sonra verilir.

**Bugünkü konum:** hedef ağda QUIC'e karşı ölçülebilir üstünlük yok. Teslim oranı ayırt etmiyor, gecikmede quinn önde. Gösterilen üstünlük %30 kayıpta TCP'ye karşı.

## S1. Akışlar birbirini bloke ediyor mu, nerede, ne kadar? — Evet (ürün yolu, %30 kayıp)

- Dosya varken TEXT p95 +63…+100 s (12,5 kB/s), +80…+83 s (1 kB/s). K-1 TEXT ölçütü 8/8 hücre-kolda kaldı; aşım 62–99 s.
- **Gönderen kuyruğu:** TEXT p95'te 66–99 s [türetilmiş]. Dosya ilk turda tek FIFO'ya dökülüyor.
- **Alıcı sıralaması:** dosyalı hücrelerde p95 taban 12–35 s, aday 2,5–10 s [türetilmiş]. Gönderen kuyruğu boş + kurtarma canlıyken TEXT p50 taban 10–30 s, aday 2,1 s (betimsel; aday n = 7).
- **Motor kurtarma işi:** kuyruk doluyken baytların %16–24'ü (taban), %29'u (aday) yeniden gönderim.
- **Alıcı kapasite reddi:** taşma dışı 420/420 koşuda 0. Taşma hücrelerinde birim teyitlenip düşüyor (aynı oturumda uygulama kurtarma yolu yok [kod]).
- **05b:** sıralama beklemesini kısaltıyor, 1 kB/s'de yükü gönderen kuyruğuna kaydırıyor. Toplam TEXT p95'te etkisi gösterilmedi.
- %30 kayıpta oturum 470/470 koşuda kopmadı.

## S2. Düşük bantta hat kapasitesinin ne kadarı gerçek veriye gidiyor? (bench)

| hız | faydalı / kapasite | hat doluluğu | neden |
|---|---|---|---|
| 25 B/s | **%3,5–10,4** | %94–98,7 | Kapasite değil, protokol yükü (ρ_ARQ ≤ 0,43). Baytların %44–63'ü PARAM_SET, %29–45'i teslime katkısı olmayan kopya. Teslim 0,167 (6 mesajın 1'i); tcp 1,000 |
| 1 kB/s | %10–28 | | 65 B'de başlık + kurtarma. 1 KB'de gereksiz yük %12–36; kayıpsız hücrede bile %35–36 |
| 12,5 kB/s | %8–28 | %13–30 | Hat boş, gecikme gönderende: pacer teslim hızına kilitli |

Onarım payı %0,0: FEC bugün tele çıkmıyor. RTT 2 s'de 12,5 kB/s, kayıpsız: 600 mesajın ≈ 290'ı geç çıkıyor, p95 3,4–3,6 s ↔ quinn 1,45 s.

## S3. Yüksek RTT'de FEC, ARQ'ya göre son tarih içinde ne kazandırıyor? (bench, 20 s konuşma)

- **Ürün FEC'i bugün kazandırmıyor:** 840 koşuda konuşma penceresinde parite 0; `uyar − arq` 22/22 karşılaştırmada fark gösterilmedi.
- **Sabit RS, RTT 2 s, son tarih 1,8 s:** `rs2 − arq` %10 +0,09 [0,07; 0,10], %20 +0,18 [0,15; 0,20], %30 +0,25 [0,20; 0,28]; oran 0,91–0,97. Bant 1,5–2,0 ×, yani eşit bütçe değil.
- **RTT 600 ms, son tarih 800 ms:** +0,27…+0,67, ama farkın ≥ %87'si gönderen kilidinin önlenmesinden; parite katkısı +0,03…+0,06. Parite kuyrukta 0,8–8 s bekliyor.
- **Sıralı teslimde (türetilmiş):** RTT 2 s %20'de arq 0,77 → 0,02, rs2 0,95 → 0,71. FEC'in ürün değeri burada, ama sıra beklemesi kalkmadan son tarih tutmaz.
- RTT 2 s'de 400/800 ms fiziksel olarak olanaksız. ≥ %95 yalnız RTT 2 s + 1,8 s'de görüldü.

## S4. Kama aynı koşullarda QUIC'in ve TCP'nin neresinde? (bench; 40 birincil hücre)

| kıyas | teslim (ön-kayıtlı kural) | gecikme p95 (düzeltmesiz; Bonferroni) |
|---|---|---|
| Kama ↔ quinn (32 hücre) | fark gösterilmedi; iki taraf medyan 1,00, ölçü ayırt etmiyor | quinn 22 hücrede, Kama 2 hücrede düşük (15; 2). En büyük: 32,5 / 26,2 ↔ 8,7 s |
| Kama ↔ tcp (40 hücre) | Kama 6 hücrede kazandı (%30; tcp 0,42–0,86), 8 hücrede kaybetti (25 B/s'nin tamamı) | |
| aday ↔ taban (40 hücre) | 40/40 ayrışmadı | aday 6 hücrede düşük, hepsi 1000 B (5) |

- **25 B/s:** Kama 0,167; tcp 0,83–1,00, quinn 60 s içinde 40/40 bağlanamadı (N = 5; 240 s tavanla 100 s).
- **%80 stres (betimsel, N = 10):** Kama 0,04–0,77, quinn ve tcp 0 (dört hücre). %50'de tablo karışık.
- **tcp sınırlaması (D-13):** makinede `ethtool` yok; veth TSO açık kaldığı için tcp kayıpları ikişerli korelasyonlu. Denetim hücresi (yalnız 12,5 kB/s, RTT 0,6 s, %30; N = 30/kol, betimsel) `gso_max_segs 1` ile sayaç kaybını 0,255'ten 0,303'e getirdi. İstenen ölçülerde fark gösterilmedi; dosyalı kolda TEXT p95 ve dosya bitişi iki durumda da 60 s tavanında. Sonradan eklenen pencere sonu TEXT ve PTT teslimi TSO kapalıyken yüksek (+0,15; +0,17), ama düzeltmesiz, hücreler farklı oturumda koştu ve B ızgarasına genellenmez.

## S5. Hangi somut kod değişiklikleri gerekiyor?

Sınıf: **dar** = tek crate, motor dışı · **sözleşme** = S1, AMP ya da tel değişir, sahip onayı · **motor** = `adaptive-transport` kuralı. Satırlar `a858f3fc`.

| # | ölçüm | dosya:işlev | kök neden | en dar düzeltme | sınıf | 05b | KTP-2 |
|---|---|---|---|---|---|---|---|
| 1 | r25 PARAM hattın %44–63'ü; teslim 0,167 (30/30) | `client-core/runtime.rs:2558` `client_task` | 200 ms × 25 deneme, 5 s'de 93 s hat süresi [kod+ölçüldü] | aralık 200 ms × 2^k | dar | hayır | §3.6'da yazılı değil |
| 2 | kilitli ARQ 0,167 ↔ kilitsiz 0,815 (n = 7, betimsel); hız çöküşü 12/225 ↔ 14/225 | `sender.rs:3993`, `:4252-4255`, `bw.rs:221`, `rate.rs:164-180` | örnek ≈ pacer × (1 − p), RecoverArq kazancı 1 [çıkarım] | kayıp kör, qd işaretli kazanç; sunulan + yeniden gönderilen alt sınır | motor | hayır; 65 B'de kilit arttı | §3.5 aynı filtre, taşıması beklenir [çıkarım; V-25] |
| 3 | sıralama p95 12–35 s; türev 0,77 → 0,02 | `client-core/transport.rs:367-412` `Reorder::push` | oturum geneli tek sıra [ölçüldü] | akış başına sıra | sözleşme (S1 §4); motor değişmez | kısmen | §3.1–3.2 |
| 4 | TEXT fifo p95 66–99 s | `kama-app/file_transfer.rs:866-909` `pump`, `sender.rs:2309` | dosya ilk turda tek FIFO'ya [ölçüldü+kod] | dosya uçuş penceresi W (no. 2'ye bağlı) | sözleşme (S1 teyit sinyali) | hayır | §3.6–3.7 |
| 5 | uyarlamalı parite 0 (840 koşu) | `sender.rs:3763-3796`, `budget.rs:38-41` | §8.5 kırpması, K = 8'de r = 0 [kod+ölçüldü] | ses için `manual_r` | dar | hayır | §3.8 |
| 6 | parite kuyruk beklemesi 0,8–8,1 s | `sender.rs:5319`, `:4292`, `:4311` | parite pacer'da bekliyor [ölçüldü] | son tarihli parite ARQ'dan önce | motor | hayır | §3.8 |
| 7 | ACK'li düşüş: 32–1630 birim/koşu, yalnız taşma hücreleri (5/235, 10/235) | `runtime.rs:3010-3032`, `:3602-3605` | ACK ve FEC `RECOVERED` teslim kanalından bağımsız [kod+ölçüldü] | ACK'ten önce yer ayır | dar | hayır | §3.7 |
| 8 | RTT 2 s kayıpsız p95 3,4–3,6 ↔ quinn 1,45 s | `rate.rs:101` | 4800 B/srtt ≈ 2,3 kB/s < sunulan [kod+ölçüldü] | `min_cwnd` 10 × MTU | motor | hayır | §3.5/§3.10 daha dar (V-26) |
| 9 | kayıpsız 1 kB/s gereksiz yük %35–36 | `tail_recovery.rs:79`, `receiver.rs:76` | PTO ACK tutmayı 25 ms sayıyor [kod+ölçüldü] | pencereli `ack_delay` | motor | hayır (artırıyor) | §3.4 |
| 10 | r25 prob 21 / 4, teslime katkı 0 | `sender.rs:5995`, `tail_recovery.rs:155-165` | örneksiz PTO 1 s, RTT 8,9 s [ölçüldü+kod] | `rtt_floor` | motor | kısmen | §3.4 |
| 11 | pacer hattın %2,5–2,9'u (12,5 kB/s dosyasız) | `sender.rs:4208` | uygulama sınırlı tur Startup platosu sayılıyor [kod] | bu turu saymamak | motor | hayır | §3.5 |
| 12 | 60 s `ReceiptTimeout` 280/280; dosya 195'inde ulaşmış | `file_transfer.rs:1541` | süre `FILE_END` kabulünde başlıyor [ölçüldü+kod] | son ilerlemeden başlat | sözleşme (AMP §3) | hayır | karşılığı yok |
| 13 | tek tüketici: durma +16 s (betimsel, N = 10) | `kama-node/node.rs:465-484` | tek düğüm iş parçacığı [kod] | dinleyiciyi sınırlı kuyrukla ayır | dar | hayır | §3.7 |
| 14 | kuyruk sonu takılması 4/235 ↔ 0/235 (p = 0,12; fark gösterilmedi) | `tail_recovery.rs:221`, `loss.rs:692`, `sender.rs:5954` | prob bitince hiçbir yol emisyon üretmiyor [kod] | 05b prob devri kuralı | motor | kod gereği beklenir | §3.4 |

## ISEMRI §5 — KTP-2 kararı için

1. **Doğrudan engelleyenler:** S5 no. 1–6 (K-1, K-3, K-6). No. 7–14 güvenilirlik ve israf.
2. **Dar düzeltmeyle çözülenler:** no. 1, 5, 7, 13. Beklenen etki [tahmin]; F1'de ölçülecek.
3. **Motor kuralı değişikliği isteyenler:** no. 2, 6, 8, 9, 10, 11, 14. **Motorun değiştirilmesini gerektiren bulgu yok.** Sözleşme değişikliği isteyenler: no. 3, 4, 12.
4. **KTP-2'nin koruduğu avantajlar:** kayıpta hız düşürmeme (§3.5) ve ACK gelmezse kayıp ilan etmeme (§3.4) yazılı. Ölçülen avantajlar: %30'da tcp'ye karşı 6 hücre, oturum 470/470, %80'de teslim (betimsel). **Bozuk dönüş yolu F0'da ölçülmedi.** Riskler: §3.5 kilidi taşıyabilir (V-25, [çıkarım]); kesinti sonrası uçuş tavanı 2 paket, ilk teslim örneğine dek RTT 2 s'de bugünkünden dar (V-26, [türetilmiş]).
5. **FEC nasıl açılır:** pacer alt sınırı → parite önceliği → ses için `manual_r` → K = 2 / E = 64 + akış başına sıra → süre dolumu → sıra atlama. İlk iki adım olmadan parite RTT 600 ms'de ilk 5 s dışında kazanç vermez.

## Kabul testleri (taslak §6) bugün

| test | durum |
|---|---|
| K-1 akış bağımsızlığı | KALDI (ürün yolu; TEXT 8/8). Ses vekili 6/8, r1k tabanda tanımsız |
| K-2 kayıp süpürmesi | ≥ %90 l30'a dek sağlandı (bench üst sınırı). "QUIC'ten ileride": fark gösterilmedi |
| K-3 düşük bant | KALDI: 25 B/s p95 82–118 s (ölçüt ≈ 12 s). İdeal ARQ bile 14,5 s [türetilmiş]; ölçüt gözden geçirilmeli |
| K-6 canlı ses | KALDI: ürün FEC'i parite üretmiyor. ≥ %95 yalnız sabit RS, RTT 2 s + 1,8 s (bench) |
| K-7 israf | 40 hücrenin 17 / 18'inde sınır içinde. tel/teslim r1k 1 KB 1,78 (tcp 1,14, quinn 1,49) |
| K-4, K-5, K-8, K-9, K-10 | ölçülemedi: F0 ızgarasında hücre yok |

## Ölçülemedi

- Ürün yolunda son tarih oranı, FEC ve rakip kıyası: S2–S4 bench üst sınırıdır.
- Gerçek Noise kurulum maliyeti (bench'te yok).
- `b_r25_d300_l0_p1000`: bütçe dışı, koşulmadı.
- 25 B/s'de Kama ↔ quinn hükmü (quinn N = 5).
- Çerçeve başına FEC/ARQ kaynağı; `data_denied` / `retx_denied` (tanı kapalı).

## Kaynak

- Şerit raporları: `a/RAPOR-A.md`, `b/RAPOR-B.md`, `c/RAPOR-C.md` (TS.1–TS.7), `d/RAPOR-D.md` (§9); taslak denetimi `TASLAK-NOTLARI.md`.
- Ham veri ve komutlar: `HAM-VERI.md`. Şerit analizlerinin dördü de orkestratörde bit-bit yeniden üretildi.
- 05b netem arşivi: `05b-netem/`.
