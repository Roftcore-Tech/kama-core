# KTP2-F0 · Ajan A — Akış bağımsızlığı (taslak, ölçüm ÖNCESİ)

**Durum:** taslak 1 (2026-10-29). Bu turda ölçüm YOK. §1–§2 kaynak okumasıdır [kod]; §5'teki sayılar
ölçülmemiş akıl yürütmedir [çıkarım] ve ön-kayıttır: koşu sonrası sapma bu tabloya göre ayrıştırılır.
Taban = main `a858f3fc`; aday = 05b `49864510`. Satır numaraları aksi yazılmadıkça iki ağaçta da
geçerlidir (değişen dosyalarda iki numara verilir: `taban / aday`).

Etiketler: [ölçüldü] koşu çıktısı · [kod] kaynak okuması · [çıkarım] ölçülmemiş · [betimsel] karar dışı.

---

## 1. Veri yolu ve bekleme noktaları [kod]

### 1.1 Yol (alice → bob)

1. **Uygulama → kama-app.** `KamaNode::send_text / ptt_push / send_signaling / send_file` komutu düğüm
   iş parçacığında işlenir (`kama-node/src/node.rs:273` `run`, komut kuyruğu 64 kayıt `:90`).
   TEXT `Messenger::send_text` → `transport.send(.., Reliable)` (`kama-app/src/messaging.rs:635`);
   PTT `MEDIA_BUNDLE` Reliable (`ptt.rs:627`); sinyal Reliable (`signaling.rs:427`); dosya
   `FileTransfer::pump` her kaydı Reliable sunar (`file_transfer.rs:1521`). **Hiçbir sınıfın önceliği
   yok; dört akış da aynı S1 oturumunun aynı birim dizisine girer.**
2. **Dosya bir seferde dökülür.** `FileTransfer::pump` (`file_transfer.rs:866-909`) "hiçbir kaydın kabul
   edilmediği tura dek" sunar: `FILE_BEGIN` + bütün parçalar Transport geri basınç verene kadar
   (offer kanalı 1024 + motor deposu 4096 birim) tek turda kabul edilir. 512 KiB = 530 parça,
   64 KiB = 66 parça → ikisi de tavanın altında, dosyanın tamamı ilk turda motor kuyruğundadır.
3. **client-core adaptörü.** `ClientTransport::send` (`client-core/src/transport.rs:1253-1312`) birimi
   oturum ucuna verir; `RuntimeError::QueueFull` → `Backpressure` (`:1294`).
4. **Çalışma zamanı (gönderen).** Offer kanalı 1024 (`runtime.rs:260 / 264`); depo `store.len() <
   max_inflight_units (4096)` iken alınır (`:2430 / 2511`); her birim `Event::AppData` ile motora
   (`:2486 / 2567`). FEC blokları birim türünden bağımsız ardışık K=8 birimden kurulur
   (`cur_block.push`, `:2499 / 2580`): PTT/TEXT, dosya parçalarıyla aynı bloğa girer.
5. **Motor.** Yeni veri **tek FIFO**: `tx_queue.push_back` (`adaptive-transport/src/sender.rs:2309 /
   2592`), `drain_new_data` önden çeker (`:6109 / 7560`). Tur sırası `schedule_transmit`
   (`:4277 / 4748`): taban'da DATA-önce, `RecoverArq`'ta her `K_ARQ_SHARE = 4`'üncü çağrıda ARQ önce
   (`:1535 / 1722`); aday'da ek olarak kopya payı, karşılık hakları ve tek rezerv (`serve_copy_share
   :7452`, `refresh_reserve :7494`, `on_class_emitted :6418`, `assign_reserve_to_holder :6396`).
   Kayıp ilanı `pending_lost`'a `deadline_ns = now` ile girer (`:3496 / 3975`); ARQ planı vade
   sırasıyla (`arq.rs:111-133`).
6. **Kripto + UDP.** Mühür, `sock.send` (runtime `seal_send!`).
7. **Alıcı çalışma zamanı.** `server_task`: bağlam-önce kapı → `NotReady` ise ACK'siz düşüş
   (`runtime.rs:3466 / 3547`); yuva tablosu doluysa yeni yuva ACK'siz düşer (`:3538 / 3619`); aksi
   hâlde alıcı motor birimi **sırasız** ilk teslim eder (`receiver.rs:193-262`), `admit_chunk →
   push_delivery` teslim kanalına `try_send` (`runtime.rs:3010-3032 / 3091-3113`), **ACK ise teslimden
   bağımsız gider** (`:3602 / 3683`). Teslim kanalı 1024 (`:261 / 265`, `:2843 / 2924`).
8. **Adaptör (alıcı).** `drive_deliveries` (`transport.rs:1053-1082`) yalnız düğüm iş parçacığı
   `drive()` çağırdığında kanalı boşaltır (`inbox` 1024, `:116`, `:1055`) ve birimi **oturum başına
   tek** `Reorder`'a koyar (`:367-412`, çağrı `:1080`). `unit < next` geç kopya atılır (`:394`);
   tutulan > 4096 (`max_inflight_units`, `:1054`) olursa `next` ilk tutulana atlar (`:398-401`).
9. **Uygulama (alıcı).** Düğüm turu (`node.rs:465-484`): `Ptt::poll` → Messenger → FileTransfer →
   Signaling zinciri; turda ≤ 1024 olay (`:95`); `NODE_TICK = 20 ms` (`lib.rs:127`). Dinleyici düğüm
   iş parçacığında çağrılır.

### 1.2 Bekleme noktaları tablosu

| # | Katman | Yer | Koşul | Ne kadar bekletir |
|---|---|---|---|---|
| B1 | Gönderen: komut kuyruğu | `node.rs:90`, `lib.rs` `request` | düğüm turu meşgul | ms (komut başına bir tur) |
| B2 | Gönderen: offer kanalı / depo | `runtime.rs:260, 2430` | 1024 kanal + 4096 depo dolu | geri basınç; uygulama yeniden dener |
| B3 | **Gönderen: motor yeni veri FIFO'su** | `sender.rs:2309, 6109` (aday `:2592, 7560`) | önde kuyruklanmış her birim (dosyanın tamamı) | önündeki tel baytı / yeni veriye düşen hız payı (§1.3) |
| B4 | **Motor kurtarma işi** | taban `schedule_transmit :4277-4296`, `K_ARQ_SHARE :1535`; aday `:4748-4800` | `pending_lost` doluyken | taban: DATA-önce, ARQ ≈ jeton yuvalarının ¼'ü [çıkarım]; aday: ARQ ↔ DATA dönüşümlü ≈ ½ [çıkarım] |
| B5 | Motor: kayıp tespiti | `loss.rs:14, 19-21` | 3 paket ya da 9/8·srtt sonra | ≥ RTT + 3 paket serileştirmesi (r1k: ≈ 0,6 + 3,3 s) |
| B6 | Motor: yeniden kayıp sırası | `sender.rs:3493-3499 / 3972-3978` (`deadline_ns: now_ns`) | yeniden gönderim de kaybolursa | birim kuyruğun SONUNA geçer; alıcıdaki baş boşluk bütün birikim kadar bekler |
| B7 | Ağ | netem | profil | 300 ms ± 60 (d300) + serileştirme (1088 B: r12k 87 ms, r1k 1,09 s) |
| B8 | **Alıcı: oturum geneli sıralama** | `transport.rs:367-412`, `:1053-1082` | daha küçük kimlikli bir birim eksik | en eski eksik birimin kurtarılmasına dek (akış türünden bağımsız) |
| B9 | **Alıcı: teslim kanalı taşması** | `runtime.rs:3025-3031` + ACK `:3602` | düğüm iş parçacığı kanalı boşaltmıyor, > 1024 birim | birim ACK'lenip DÜŞER (teyitli kayıp); `Reorder` kalıcı boşlukta bekler (B10) |
| B10 | Alıcı: `Reorder` tavanı | `transport.rs:398-401` | tutulan > 4096 | o zamana dek bütün sonraki birimler bekler; sonra boşluk sessiz atlanır |
| B11 | Alıcı: NotReady | `runtime.rs:3466` | oturum Ready değil | ACK'siz düşüş; gönderen ARQ'yla yeniden yollar (kuruluşta) |
| B12 | **Alıcı: tüketici** | `node.rs:465-484`, `lib.rs` `NodeListener` | dinleyici ya da tur yavaş | bütün akışlar; dosyaya özgü durma yok |
| B13 | Uygulama zamanlayıcıları | `file_transfer.rs:262` (`FILE_IDLE_TTL 60 s`), `ptt.rs:215` (120 s) | uygulamaya yeni parça gelmiyor | dosya alıcıda `IdleTimeout` ile biter (B8/B6 uzarsa) |

"DeliveryGate / kapasite reddi": taban ve adayda YOKTUR; yalnız durdurulmuş 05c dalında
(`wt/05c-kod`, `runtime.rs` `DeliveryGate`). Taban/adayda teyitsiz düşüş yalnız B11 ve yuva-dolu
yolundadır; teslim kanalı taşması teyitli kayıptır (B9).

### 1.3 Bekleme büyüklüğü (formüller) [çıkarım]

p = 0,3 (iki yön), L = hat hızı, u = pacer'ın hat kullanımı (bilinmiyor; Ajan B ölçer), dosya teli
D = N_parça × 1088 B (1013 birim + 11 DATA + 22 zarf + 42 L2/IP/UDP).

- Dosyanın tamamının teslimi: F ≈ D/((1−p)·u·L) + kuyruk (son kayıpların 1–3 tur yeniden gönderimi).
- Dosyanın İLK geçişi (FIFO'nun boşalması): taban F1 ≈ D/(0,75·u·L); aday F1 ≈ F.
- Dosya FIFO'dayken s anında gönderilen TEXT/PTT/sinyal: gönderen bekleme ≈ max(0, F1 − s);
  alıcı sıralama bekleme ≈ F − F1 (taban) / birkaç RTT (aday); TOPLAM ≈ F − s (iki kolda).
- Taban'da ARQ payı ¼ < yeniden gönderim talebi p/(1−p) ≈ 0,43 → FIFO doluyken birikim büyür; B6
  nedeniyle baş boşluk birikimin tamamını bekleyebilir. Aday'da pay ≈ ½ > 0,43 → birikim sınırlı.

## 2. 05b adayının farkı [kod]

`git diff a858f3fc 49864510 -- crates/`: değişen ürün dosyaları `adaptive-transport/src/{sender,arq,
loss}.rs` ve `client-core/src/runtime.rs`. `kama-app`, `kama-node`, `client-core/src/transport.rs`
DEĞİŞMEDİ.

| Nokta | Taban | Aday | Akış bağımsızlığına etkisi |
|---|---|---|---|
| B3 FIFO | tek FIFO | tek FIFO (aynı) | yok |
| B4 kurtarma payı | DATA-önce + her 4. çağrıda ARQ | + karşılık hakkı ve tek rezerv: ARQ emisyonu DATA'ya hak verir, DATA emisyonu hak vermez; rezerv sırası ARQ > DATA > kopya | ARQ ≈ ½ pay [çıkarım]: B8'deki birikim küçülür, FIFO daha yavaş boşalır; TOPLAM TEXT gecikmesi ≈ aynı |
| Birim teyidi | ACK'teki seq → runtime haritası → depo silinir | yalnız motor teyidiyle (`take_confirmed_units`) | unutulmuş kaydın geç ACK'i teyit etmez → ek yeniden gönderim olabilir (kurtarma işi hacmi) |
| srtt | bilinen şişme | RTT kabul kuralı (`note_rtt_freshness :7010`) | B5 eşikleri ve PTO kısalır → B8 kuyruğu kısalır |
| Sahipsiz birim | — | H ufkundan sonra kopya kuyruğuna (`adopt_unowned :6925`) | kopya baytı; B4'te pay |
| B6 yeniden kayıp vadesi | `deadline_ns: now_ns` | aynı | yok |
| B8 Reorder, B9 teslim kanalı, B10, B12 | — | AYNI (`transport.rs` ve runtime alıcı yolu özdeş; aday `runtime.rs:3091-3113`, ACK `:3683`) | yok |

## 3. Ölçüm düzeneği

### 3.1 İki süreç (hedef gerçekleşti)

`crates/kama-integration/examples/f0_flows.rs`: bob (sunucu) ns-B'de, alice (istemci) ns-A'da;
ortak koşucu `scripts/ktp2/f0/f0_cell.sh` (yön başına ayrı netem, orta ad alanında). Her süreç ürün
yolunu koşar: `KamaNode` + `ClientTransport` ürün profili + `NoiseWireProtection` + kama-app
motorları + gerçek UDP.

**Beyanlı boşluk:** el sıkışma zarfları (INIT/RESPONSE) koşu dizinindeki dosya yolu tabanlı Unix
datagram soketlerinden geçer (`relay-alice.sock`, `relay-bob.sock`), netem DIŞINDA — INT-02'deki
süreç-içi röleyle aynı anlam (DUR-K4G-11 → B). Tutamaç uzayları ayrık (alice tek, bob çift); kayıtsız
eşe zarf gitmez ve saklanmaz.

Provisioning: bob `kama-provision` kütüphanesiyle `prov/`'a yazar; `root.json` dahil otorite dizini
cihaz dosyaları kopyalanınca silinir; her düğüm açıldıktan sonra kendi cihaz dosyası silinir; koşu
sonunda `prov/` ve düğüm veri dizinleri silinir (anahtarlar diskte kalmaz).

### 3.2 Eklenen kancalar (iki kolda aynı)

| Commit | Dosya | Ne | Gerekçe |
|---|---|---|---|
| `c56e0cea` [salt-okunur sayaç] [test-util kanca] | `client-core/src/transport.rs` | birim izi: `Sent` (send kabulü), `Drained` (teslim kanalından alınış), `Released` (`Reorder::next` ilerlemesi); varsayılan KAPALI, `#[cfg(any(test, feature = "test-util"))]`; sınırlı (65 536), okuma boşaltır | canlı ayna birim başına damga vermiyor; katman kırılımı (B3–B8) için zorunlu. `transport.rs` iki ağaçta özdeş |
| `fda5892c` [test-util kanca] | `kama-node/src/{lib,node}.rs` | `start_with_link_for_test(.., link, live_diag, unit_trace)`, `take_unit_trace_for_test()`; ürün `build` → `build_with_link(.., None)` (aynı satır, aynı sıra) | süreç-içi röle iki süreçte çalışmaz; `EnvelopeLink` enjeksiyonu. Not: DUR-K4G-11 → B ürün yolundan `start_with_envelope_link`'i kaldırmıştı; bu kurucu yalnız test-util'dir |

Motor ilk iletimi için kanca EKLENMEDİ (runtime.rs iki kolda farklı; birleşme çakışması riski).
Yerine türetilir: `ilk_iletilen(t) = kabul_sayısı(t) − kanalda(out − un) − txq(t)`; çözünürlük ≈
örnekleme (`--probe-ms 200`) + kurtarma görüntüsü aralığı (200 ms). FIFO-ağ ayrımı bu yüzden
[türetilmiş] etiketlidir; toplamı kesindir.

### 3.3 Senaryo ve zaman çizelgesi (t0 = alice `Connected`)

| Öğe | Zaman | Boy (birim / tel) |
|---|---|---|
| Isınma TEXT (W) ×3 | 0 / 0,5 / 1 s | 75 B / 150 B |
| Dosya (F) | 5 s | r12k 524 700 B (530 × 990), r1k 65 340 B (66 × 990); parça birimi 1013 B / 1088 B |
| TEXT (T) ×40 | 6 s'den 2,5 s arayla (son 103,5 s) | 75 B / 150 B |
| Sinyal (S) ×10 | 7 s'den 10 s arayla | 127 B / 202 B |
| PTT basışı ×2 | 15 s ve 60 s; 25 demet × 200 ms, 40 B | demet 53 B / 128 B; açılış 11 B, kapanış 9 B |
| Tüketici durması (`*_dur`) | 20 s'den 30 s | — |
| Karşı örnek (K, dosyalılar) | alice `txq == 0` ve `un ≥ 3` ilk görüldüğünde | 5 TEXT (59 B / 134 B) 0,5 s arayla + 10 demetlik basış |

**Tüketici durması (beyan):** FileTransfer alıcısının tüketimini durduran uygulama API'si YOK
(`file_transfer.rs` gelen parçayı düğüm turunda işler; tek tüketici düğüm iş parçacığıdır). Durma,
bob dinleyicisinin (düğüm iş parçacığında) arm anından sonraki ilk `Metrics` olayında 30 s
bloklanmasıyla öykünülür. Bu, mevcut mimaride BÜTÜN akışların tüketicisini durdurur. Alıcı motor ve
ACK'ler (tokio görevi) durmaz; teslim kanalı dolmaya devam eder (B9 bu yolla ölçülür).

**Dosya boyu gerekçesi:** K-1'in 2 MB'ı r1k'da tek geçişte ≈ 2,31 MB tel / 1000 B/s ≈ 38 dk, %30
kayıpla ≈ 55 dk/koşu (60 koşu ≈ 55 h) — bütçeye sığmaz. Ölçülen etki "dosyanın önde kalan teli"dir;
dosya, zaman çizelgesinin ~80–150 s'sini kaplayacak boyda seçildi: F ≈ 75–125 s (r12k), ≈ 140–190 s
(r1k) [çıkarım]. Boy 990'ın katıdır (son parça da 1013 B: birim boyu sınıf eşlemesi çakışmaz).

### 3.4 Katman damgaları (öğe başına; Unix ms, iki süreç aynı makinede)

uygulama gönderimi (`send.t`) → Transport kabulü (iz `S`) → motor ilk iletimi [türetilmiş] → alıcı
motor kabulü (iz `D`) → sıralı teslim (iz `R`) → uygulama teslimi (`rx`). Bileşenler: `fifo`, `net`
(ağ + birimin kendi kurtarması), `reorder`, `consumer`, `total`. Analiz:
`scripts/ktp2/f0/a/f0a_analyze.py` (sınıf başına p50/p95/p99, koşular arası medyan/IQR/min–maks,
K-1 tablosu, B9/B10/B11 sayaçları, uygulama sayaçları: `receipts_sent/unsent`, `resent_on_new_session`,
`resend_refused`).

## 4. Hücreler ve bütçe

Profiller KATALOG §2: `r12k_d300_l30` = `delay 300ms 60ms distribution normal loss 30% rate 100kbit`,
`r1k_d300_l30` = `… rate 8kbit`, `r12k_d1000_l30` = `delay 1000ms 200ms distribution normal loss 30%
rate 100kbit`. Kollar `taban aday`. Hücre dosyaları `scripts/ktp2/f0/cells/a/` (üretici
`f0a_cells.py`), duman kopyaları `cells/a/duman/` (REPS=1, ad `+_duman`).

| Hücre | Sınıf | N/kol | Koşu süresi [çıkarım] | Koşu-saati (2 kol) |
|---|---|---|---|---|
| a_r12k_d300_l30_dosyasiz | birincil | 30 | ≈ 2,0 dk | 2,0 h |
| a_r12k_d300_l30_dosyali | birincil | 30 | ≈ 2,1 dk | 2,1 h |
| a_r12k_d300_l30_dosyali_dur | birincil (K-1) | 30 | ≈ 2,1 dk | 2,1 h |
| a_r1k_d300_l30_dosyasiz | birincil | 30 | ≈ 2,1 dk | 2,1 h |
| a_r1k_d300_l30_dosyali | birincil | 30 | ≈ 3,0 dk (en kötü 4,2) | 3,0 h (4,2) |
| a_r1k_d300_l30_dosyali_dur | birincil (K-1) | 30 | ≈ 3,0 dk (en kötü 4,2) | 3,0 h (4,2) |
| a_r12k_d300_l30_tasma_ptt | betimsel (B9 taşması) | 10 | ≈ 3,7 dk (pencere 90 s dolar) | 1,2 h |
| a_r12k_d300_l30_dosyasiz_dur | betimsel | 10 | ≈ 2,0 dk | 0,7 h |
| a_r12k_d1000_l30_dosyasiz | isteğe bağlı | 10 | ≈ 2,3 dk | 0,8 h |
| a_r12k_d1000_l30_dosyali_dur | isteğe bağlı | 10 | ≈ 2,8 dk | 0,9 h |
| **Toplam** | | | | **≈ 17,9 h (en kötü ≈ 20,3 h)** |
| Duman (10 hücre × 2 kol × 1) | | 1 | ≈ 2,5 dk | ≈ 0,8 h |

Koşu süresi = kurulum (≈ 2 s) + plan (103,5 s son TEXT) ya da dosya tamamlanması + alındı payı +
kapanış (≈ 8 s). `BUDGET_S` bir zaman aşımı sınırıdır (600 / 900 / 480 s), beklenen süre değil.

## 5. Ön-kayıtlı tahminler [çıkarım]

Koşu başına p50/p95 → hücrede koşular arası medyan. "≈" merkez; köşeli parantez makul aralık.
u (pacer kullanımı) 0,7–0,95 aralığında varsayıldı; FEC paritesi açılırsa F %25'e dek uzar.

### 5.1 r12k_d300_l30

| Ölçü | dosyasiz | dosyali | dosyali_dur | Kol farkı |
|---|---|---|---|---|
| TEXT p50 | 0,4 s [0,32–0,6] | ≈ 30 s [20–45] | = dosyali (±5 s) | toplamda yok |
| TEXT p95 | ≈ 2 s [1–4] | ≈ 75 s [60–110] | = dosyali (±5 s) | toplamda yok |
| TEXT p95 katman payı (dosyalı) | — | taban: fifo ≈ %85–95, reorder 5–15 s; aday: fifo ≥ %95, reorder 1–3 s | aynı | ayrışma burada |
| PTT ≤ 800 ms % | 60–80 | 0 [0–5] | 0 [0–5] | yok |
| Sinyal p95 | ≈ 2 s | ≈ 70 s | = dosyali | yok |
| Dosya alındı (5 s'den) | — | ≈ 85 s [75–125] | = dosyali | ±10 s |
| Teslim kanalı düşüşü | 0 | 0 | 0 (30 s'de ≈ 240 birim < 1024) | yok |
| K-1 TEXT (eşik ≈ p95_dosyasız + 0,8 s) | — | KALIR (≈ 70 s aşım) | KALIR | iki kolda |
| Karşı örnek TEXT toplamı | — | taban ≈ 5–15 s, %80+ reorder, fifo < 0,2 s; aday: tetik sonda ya da hiç, ≈ 1–5 s | aynı | taban > aday |

### 5.2 r1k_d300_l30

| Ölçü | dosyasiz | dosyali | dosyali_dur |
|---|---|---|---|
| TEXT p50 | ≈ 0,6 s [0,45–1] | ≈ 105 s [80–140] (40 TEXT'in hepsi dosyanın arkasında) | = dosyali |
| TEXT p95 | ≈ 4 s [2–8] | ≈ 150 s [120–190] | = dosyali |
| PTT ≤ 800 ms % | 30–60 (basış hattın ≈ %64'ü) | 0 | 0 |
| Dosya alındı (5 s'den) | — | ≈ 150 s [140–190] | = dosyali |
| Dosya alıcıda `IdleTimeout` (B13) | — | taban: koşuların bir kısmında olası (B6 + ARQ payı ¼) — düşük güven; aday: 0 | aynı |
| Teslim kanalı düşüşü | 0 | 0 | 0 (30 s'de ≈ 20 birim) |
| K-1 TEXT (eşik ≈ p95_dosyasız + 2,8 s) | — | KALIR (≈ 140 s aşım) | KALIR |
| Karşı örnek | — | taban: tetik ≈ 130 s, toplam ≈ 15–40 s, reorder baskın; aday: ≈ 2–10 s | aynı |

### 5.3 Betimsel hücreler

- **tasma_ptt** (dosyasız, durma 20–50 s, PTT 50 demet/s 15–75 s): durmada bob'a ≈ 50 birim/s →
  ≈ 1500 birim; 1024'ü kanala sığar, **≈ 300–600 birim ACK'li düşer** (iki kolda). İlk düşen birimden
  sonraki her birim `Reorder`'da kalır; tutulan ≈ 1500–2500 < 4096 → atlama olmaz → durma başlangıcının
  ≈ 20 s sonrasından itibaren gönderilen TEXT/sinyal/PTT bu oturumda **hiç teslim edilmez**; alice'te bu
  TEXT'lerin alındısı gelmez, Messenger aynı oturumda yeniden göndermez (`messaging.rs` "Retry yok").
  Koşu pencere sonunda `incomplete` biter.
- **dosyasiz_dur** (r12k): durma penceresine düşen öğeler (≈ 12 TEXT, 3 sinyal, 1. basışın son demetleri)
  ≤ 30 s gecikir; TEXT p95 ≈ 25–30 s; teslim kanalı düşüşü 0.
- **r12k_d1000_l30**: dosyasız TEXT p95 ≈ 5–10 s; dosyalı_dur TEXT p95 ≈ 80–130 s.

## 6. Kod bulguları (dört parça; ölçüm sütunu koşu sonrası doldurulur)

**A-1 Gönderen tek FIFO + dosyanın toptan dökümü.**
- Ölçüm: bekleniyor (TEXT/PTT `fifo` bileşeni, §5).
- Dosya/işlev: `file_transfer.rs:866-909` `pump`; `sender.rs:2309/6109` (aday `:2592/7560`) `tx_queue`.
- Kök neden: bütün akışlar tek birim dizisi ve tek yeni-veri FIFO'su; dosya ilk turda kuyruğa dolar.
- En dar düzeltme: dosya göndereninde akış başına uçuş penceresi (kama-app: teyitsiz dosya kaydı ≤ W;
  W = 8 → bir TEXT'in önünde en çok ≈ 8 × 1088 B: r1k 8,7 s, r12k 0,7 s) + motorda etkileşimli/toplu
  iki FIFO (S1'e öncelik ipucu = sözleşme değişikliği, sahip onayı).

**A-2 Alıcı oturum geneli sıralama.**
- Ölçüm: bekleniyor (`reorder` bileşeni; karşı örnek).
- Dosya/işlev: `transport.rs:367-412` `Reorder::push`, `:1053-1082` `drive_deliveries`.
- Kök neden: S1 §4 oturum içi sıralı teslim sözü tek dizide uygulanıyor; dosya parçasının kaybı TEXT ve
  PTT'yi bekletir. Gönderende öncelik bunu çözmez (karşı örnek: FIFO boşken de bekleme).
- En dar düzeltme: sıralamayı akış başına yapmak (birim başlığında akış kimliği = tel değişikliği) ya da
  Reliable birimleri sırasız teslim edip kama-app'te akış başına sıralamak (TEXT sıra istemiyor;
  FileTransfer parçaları her sırada kabul ediyor, ama `FILE_BEGIN`/`PTT_OPEN` önce gelmeli → küçük
  ön-tampon). İkisi de S1 §4 değişikliği.

**A-3 Taban'da kurtarma payı ve yeniden kayıp vadesi.**
- Ölçüm: bekleniyor (taban/aday `reorder` farkı; r1k `IdleTimeout`).
- Dosya/işlev: taban `sender.rs:4277-4296` (ARQ payı ¼), `:3493-3499` (aday `:3972-3978`, `deadline_ns: now_ns`), `arq.rs:111-133`.
- Kök neden: FIFO doluyken ARQ ≈ ¼ pay < p/(1−p); yeniden kaybolan birim vade sırasının sonuna düşer →
  alıcıdaki baş boşluk birikimin tamamını bekler.
- En dar düzeltme: yeniden kayıpta birimin İLK kayıp vadesini korumak (en eski boşluk önce) +
  boşluk teslimi bekletirken ARQ'ya DATA'dan önce pay (aday pay mekanizması bunun bir kısmını yapıyor).

**A-4 Teslim kanalı taşması = teyitli kayıp (orkestratör notu; iki kolda aynı).**
- Ölçüm: bekleniyor (`tasma_ptt`: `bob_delivery_channel_drops`, teslim edilmeyen öğe, `held`).
- Dosya/işlev: `runtime.rs:3010-3032` `push_delivery` (aday `:3091-3113`), ACK `:3602` (aday `:3683`),
  tüketim `transport.rs:1053-1082`.
- Kök neden: ACK teslim kanalına kabulden bağımsız; kanal yalnız uygulama iş parçacığı `drive()`
  çağırınca boşalıyor. Taşan birim gönderende teyitli, alıcıda yok; `Reorder` o boşlukta 4096 birim
  boyunca bekler, sonra sessiz atlar. `transport.rs` modül belgesindeki "bu sıra kalıcı bir boşlukta
  beklemez" iddiası bu yolda geçersiz.
- En dar düzeltme: ACK'ten ÖNCE yer ayırmak (`delivery_tx.try_reserve()`; yer yoksa alıcı motoruna
  vermeden, ACK'siz düşür → gönderen yeniden yollar). Bu, 05c'nin teyitsiz reddine benzer; ret sayacı
  ayrı tutulmalı (Ajan B'nin israf sütunu).

**A-5 Akış başına tüketici yok.**
- Ölçüm: bekleniyor (`dosyasiz_dur`, `dosyali_dur`).
- Dosya/işlev: `node.rs:465-484` `turn`; `lib.rs` `NodeListener` (düğüm iş parçacığında).
- Kök neden: bütün akışlar tek iş parçacığında tüketilir; dosya tüketicisini ayrı durdurmak ifade
  edilemez; yavaş dinleyici hepsini durdurur ve A-4'ü tetikleyebilir.
- En dar düzeltme: dinleyiciyi düğüm iş parçacığından ayırmak (sınırlı olay kuyruğu + ayrı teslim iş
  parçacığı) ve adaptör teslim kanalını uygulamadan bağımsız boşaltmak (A-4 ile birlikte).

**A-6 FEC blokları akışları karıştırır** (Ajan C'ye not). `runtime.rs:2499` (aday `:2580`): K=8 blok
ardışık birimlerden; 40 B'lık PTT birimi 1024 B sembolün içinde korunur; kurtarılan birim de aynı
`Reorder`'dan geçer (A-2).

## 7. Analiz ve sapma ayrıştırması (koşu sonrası)

`python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <dizin> <ızgara kökü>` → `OZET.md`, `cells.json`,
koşu/öğe dosyaları. Sapma: §5 tablosundaki her hücre için ölçülen medyan aralığın dışındaysa bileşen
bazında (fifo / net / reorder / consumer) ayrıştırılır; u'nun ölçülen değeri (Ajan B) ve parite baytı
(Ajan C, `par`) ayrı terim olarak yazılır.

## 8. Sınırlar

- El sıkışma zarfları netem dışında (beyanlı). Kuruluş kayıplı değil; veri yolu kayıplı.
- Motor ilk iletimi türetilmiş (± örnekleme aralığı).
- Tüketici durması tüm akışların durmasıdır (dosyaya özgü durma mevcut ürün yolunda yok).
- Kayıp tohumsuz (KATALOG §2): koşular eşlenemez; birim = koşu.
- Dosya boyu K-1'in 2 MB'ı değil; etki büyüklüğü "önde kalan tel" ile ölçeklenir (§3.3).
