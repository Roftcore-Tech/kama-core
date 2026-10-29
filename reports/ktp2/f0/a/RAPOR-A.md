# KTP2-F0 · Ajan A — Akış bağımsızlığı (taslak, ölçüm ÖNCESİ)

**Durum:** taslak 2 (2026-10-29). §1–§8 taslak 1'dir (ön-kayıt `c42fcd0b`; §5 DEĞİŞTİRİLMEDİ).
§9 duman-2 gözlemleridir [ölçüldü, betimsel: hücre × kol başına N = 1]; §5.4 yeni hücrelerin ön-kaydıdır.
Taslak 1'de ölçüm yoktu. §1–§2 kaynak okumasıdır [kod]; §5'teki sayılar
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
| a_r12k_d300_l30_tasma_ptt | betimsel (§9: gönderen sınırlı) | 5 | 3,2 dk [ölçüldü duman-2] | 0,55 h |
| a_r12k_d300_l30_tasma_dosya | betimsel (B9: 2 MB + 240 s durma) | 10 | ≈ 7,3 dk | 2,45 h |
| a_rinf_d300_l30_tasma_dosya | betimsel, STRES, KATALOG dışı (B9: hız sınırsız + 30 s) | 10 | ≈ 3,3 dk | 1,1 h |
| a_r12k_d300_l30_dosyasiz_dur | betimsel | 10 | ≈ 2,0 dk | 0,7 h |
| a_r12k_d1000_l30_dosyasiz | isteğe bağlı | 10 | ≈ 2,3 dk | 0,8 h |
| a_r12k_d1000_l30_dosyali_dur | isteğe bağlı | 10 | ≈ 2,8 dk | 0,9 h |
| **Toplam (taslak 2)** | | | | **≈ 21,8 h** (duman-2 süreleriyle; §9.4) |
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

## 9. Duman-2 gözlemleri [ölçüldü, betimsel: N = 1]

Kaynak: `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-2/out/a_*_duman` (20 koşu, rc = 0); ikililer
taban `bin/taban-2cd9f68d/f0_flows`, aday `bin/aday-bf18dc35/f0_flows`; analiz `f0a_analyze.py`
(`6d69092e`). N = 1: hüküm yok; yalnız düzenek doğrulaması ve tam setin tasarımı için.

### 9.1 Düzenek denetimi

| Denetim | Sonuç |
|---|---|
| Zarf soketleri el sıkışmayı taşıyor | 20/20: alice `inits = 1`, `received = 1`, `send_errors = 0`, `malformed = 0`; bağlantı her koşuda kuruldu |
| Öğe → birim eşlemesi (ısınma dahil) | 20/20 koşuda her sınıfın her öğesi bir birime eşlendi |
| `net` negatif | ilk araçta vardı (dosyasız −0,8 s'ye dek, `tasma_ptt` −8 s): `out` sondanın anında, `txq` görüntünün anında okunuyordu. Düzeltme `6d69092e`; sonra koşu başına 0–7 kırpma |
| bob çıkışı | 20/20 `bob_exit = true` |
| Birim izi düşüşü / dinleyici düşüşü | 0 / 0 (20/20) |
| Teslim kanalı düşüşü (`delivery_channel_drops`) | 0 (20/20; `tasma_ptt` dahil, §9.3) |
| NotReady düşüşü | 0 (20/20) |
| Karşı örnek | HİÇ ateşlenmedi (kusur, §9.2-c; düzeltme `2d64a013`) |

### 9.2 Yeni bulgular

**a) A-7 Dosya gönderende 60 s'de "başarısız" görünüyor.**
- Ölçüm: 10/10 dosyalı koşuda alice, dosya gönderiminden 60,0 s sonra `SendFailed(ReceiptTimeout)` aldı. bob dosyayı 8/10 koşuda bayt-özdeş aldı (gönderimden 77,6–268,5 s sonra).
- Dosya/işlev: `kama-app/src/file_transfer.rs:267` (`FILE_RECEIPT_TTL = 60 s`), `:111` (süre `FILE_END`'in Transport kabulünden başlar), `pump` `:866-909`.
- Kök neden: `pump` dosyanın tamamını ilk turda Transport'a verdiği için `FILE_END` gönderimden birkaç ms sonra kabul ediliyor. 60 s'yi aşan her aktarım gönderende başarısız sayılıyor, teslim edilmiş olsa bile. Duman-2 hızlarıyla sınır yaklaşık r1k'da ≈ 40 KB, r12k'da ≈ 400 KB [çıkarım].
- En dar düzeltme: alındı süresini `FILE_END`'in Transport teyidinden ya da son parça teyidinden başlatmak; ya da son ilerlemeden itibaren saymak.

**b) A-8 Sıra beklemesi dosyayı alıcıda düşürüyor.**
- Ölçüm: 2/10 koşuda bob'da `IdleTimeout` oluştu; ikisi de taban'da (`r12k_d300 dosyali_dur`, `r12k_d1000 dosyali_dur`).
- Dosya/işlev: `file_transfer.rs:262` (`FILE_IDLE_TTL`) + B8/B6.
- Kök neden: uygulamaya ≥ 60 s yeni parça çıkmıyor; parçalar ya `Reorder`'da ya da gönderende (A-10).
- En dar düzeltme: A-2/A-3; ya da süreyi Transport ilerlemesine bağlamak.

**c) Düzenek kusuru (düzeltildi, `2d64a013`).** Karşı örnek tetiği ve tamamlanma, alice'in kendi dosya sonucuna bakıyordu. A-7 nedeniyle bu sonuç 65. s'de "bitti" oluyordu. Artık bob'un durumuna bakılıyor.

**d) A-9 Uygulamanın düşük hızla gönderdiği trafikte pacer hattın küçük bir kesrinde kalıyor (Ajan B'nin alanı; akış bağımsızlığına etkisi burada).**
- Ölçüm: dosyasız r12k'da alice pacer p50'si 241–825 B/s (hat 12 500 B/s).
- PTT basışı (≈ 640 B/s) bu hızda gönderen FIFO'sunda kuyruklanıyor. Dosyasız TEXT p95: taban 9,7 s, aday 19,5 s (aday p95 fifo payı 12,3 s). §5.1'deki "≈ 2 s [1–4]" tahmininden sapma burada.
- `tasma_ptt`'de 6,4 kB/s PTT talebine karşı pacer p50 76 / 334 B/s (taban / aday), FIFO ≈ 2900 birim; bob'a 3000 demetten 61 / 385'i ulaştı.

**e) A-10 Taban r12k_d1000'de hız çöküşü.**
- Ölçüm: pacer 4,2 kB/s'ten 0,16 kB/s'e indi (400 s); `pending_lost` 30–37'de takıldı; `txq` 521'den 301'e indi.
- bob 400 s'de yalnız 3 ısınma TEXT'ini teslim etti; 335 birim hiç alınmadı, `Reorder` 253 birim tuttu.
- Aday aynı hücrede tamamlandı (dosya 263 s; pacer p50 2,6 kB/s).

### 9.3 Orkestratör sorusu: tüketici durunca teslim kanalı doluyor mu?

- **[ölçüldü, N = 1]** Birincil `*_dur` hücrelerinde (r12k / r1k, 30 s durma) teslim kanalı düşüşü iki kolda 0; TEXT / sinyal / PTT kaybı 0. Durma bu hücrelerde yalnız gecikme ekliyor.
- **[çıkarım]** Koşul: durma süresi × alım hızı > 1024. Ölçülen alım hızları dosyalı r12k'da ≈ 6–7 birim/s (30 s'de ≈ 200), r1k'da ≈ 0,7 birim/s (30 s'de ≈ 20).
- **[ölçüldü]** `tasma_ptt` taşmayı ölçemedi: yük gönderen FIFO'sunda kaldı (A-9).
- Taşmayı doğrudan ölçmek için iki yeni hücre eklendi (§5.4):
  - `a_r12k_d300_l30_tasma_dosya`: 2 MB dosya + 240 s durma.
  - `a_rinf_d300_l30_tasma_dosya`: hız sınırsız + 30 s durma; STRES, KATALOG dışı.
- **[kod]** Uygulama düzeyi: aynı oturumda yeniden gönderim yok. TEXT outbox'ta alındısız kalır (`messaging.rs` "Retry yok"); dosya `FILE_IDLE_TTL`'de biter (A-8); PTT kaybı sessiz; sinyal yalnız oturum değişiminde yeniden sunulur. Bu, A-4 taşması olursa kaybın kalıcı olduğu anlamına gelir.

### 9.4 Duman-2 süreleri ve bütçe

Duman-2'de ölçülen koşu süreleri:
- dosyasız: 108–121 s;
- dosyalı r12k: 104–151 s;
- dosyalı r1k: 124–170 s;
- r12k_d1000 dosyalı_dur: 291 s (aday) / 404 s (taban);
- `tasma_ptt`: 194 s.

Alındı payı 30 s olunca dosyalı koşulara ≤ 20 s eklenir. Toplam ≈ 21,8 koşu-saati (§4 tablosu).

### 9.5 Duman-3 [ölçüldü, betimsel: N = 1]

Kaynak: `/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/duman-3/out`; 8/8 rc = 0. İkililer: taban `bin/taban-1f6c3433`, aday `bin/aday-0a251074`. Ortam beyanı: koşular ortak yuvanın dışındaydı ve aynı anda B'nin 11 hücresi çalışıyordu.

**Karşı örnek (A-2'nin doğrudan ölçümü).** 4 dosyalı koşunun 2'sinde ateşlendi, ikisi de taban'da. Gönderenin yeni veri kuyruğu boş, eski dosya parçalarının kurtarması sürüyordu.

| Ölçü | r12k_d300 | r1k_d300 |
|---|---|---|
| Tetik anı (t0'dan) | 82,3 s | 76,4 s |
| K TEXT toplam gecikme p50 | 18,5 s | 26,4 s |
| Gönderen kuyruğu (fifo) | 0,13 s | 0,14 s |
| Alıcı sıralaması (reorder) | 17,6 s | 22,8 s |
| Ağ + kendi kurtarması (net) | — | 3,3 s |
| K PTT demeti p50 | 18,0 s | 25,9 s |

Gönderende öncelik bu beklemeyi kaldırmaz; bekleme alıcının oturum geneli sıralamasındadır. Adayda tetik oluşmadı: FIFO dosyanın sonunda boşaldı (r12k 126,6 s, r1k 133,2 s), dosya o sırada bob'a ulaşmıştı.

**Teslim kanalı taşması (A-4).** Taban `a_r12k_d300_l30_tasma_dosya` (2 MB, 240 s durma):
- `delivery_channel_drops` = 312. Sayaç ilk kez 250,2 s'de görüldü; bob'un sondası durma boyunca bloklu olduğundan durma sonrası okundu.
- bob'un sıralı teslim sınırı birim 10'da kaldı: o birim taşmada ACK'li düştü.
- 1476 birim koşu sonuna dek tutuldu (< 4096, atlama yok). Durma sonrasında TEXT ve PTT teslimi %0.
- Dosya bob'da `IdleTimeout` ile bitti (310 s).

Aynı hücrede aday'da düşüş 0; o koşuda durmada gelen birim 1024'ü aşmadı. Hız sınırsız `rinf` hücresinde iki kolda da düşüş 0: pacer p50 7,9 / 3,2 kB/s (aday / taban) kaldı, alım hızı eşiğin altındaydı (ölçülemedi: alım hızı < 34 birim/s). §5.4 tahminiyle karşılaştırma: taban r12k'da tahmin 400–700, ölçülen 312.

Diğer sayaçlar 8/8 koşuda: dinleyici düşüşü 0, birim izi düşüşü 0, kırpılan ilk iletim tahmini 0–1. Gönderende `ReceiptTimeout` 6/6 dosyalı koşuda (A-7).

### 5.4 Yeni hücrelerin ön-kaydı [çıkarım] (duman-2'den SONRA, ölçülmeden önce)

- **`a_r12k_d300_l30_tasma_dosya`** (2 MB, durma 10–250 s): durmada bob'a ≈ 240 s × 6–7 birim/s ≈ 1400–1700 birim gelir. Bundan **≈ 400–700 birim ACK'li düşer** (iki kolda). İlk düşen birimden sonra bütün birimler `Reorder`'da kalır (tutulan < 4096 → atlama yok). Dosya bob'da `IdleTimeout` ile biter; durma bitiminden sonraki TEXT / sinyal / PTT teslim edilmez. Taban'da A-10 türü hız düşüşü olursa alım hızı düşer ve taşma azalır ya da hiç olmaz.
- **`a_rinf_d300_l30_tasma_dosya`** (8 MB, 20–50 s): bu tahmin düşük güvenli; hız sınırsız hatta pacer'ın varacağı hız bilinmiyor. Dosyalı r12k'daki 9–11 kB/s'nin üstünde kalırsa ≥ 37 kB/s'de taşma olur. Altında kalırsa `delivery_channel_drops = 0` olur ve hücre "ölçülemedi: alım hızı < 34 birim/s" sayılır.
- **Karşı örnek** (dosyalı hücreler, `2d64a013` sonrası): taban'da yeni veri kuyruğu dosyanın ilk geçişinden sonra boşalır. Taban r12k'da tetik ≈ 70–90 s'de, karşı örnek TEXT toplamı ≈ 5–30 s, reorder baskın. Aday'da tetik dosya sonuna yakın ya da hiç (§5.1 ile aynı).

## 10. Tam set ara notu 1 — `a_r12k_d300_l30_dosyasiz` [ölçüldü, N = 30/kol]

Kaynak: `~/kama-k4g-work/ktp2-f0/runs/tam/a/out/a_r12k_d300_l30_dosyasiz`. DONE 2026-10-29T22:12:35Z. Hücre dosyası sha256 `550d6fc6…`, dalda commit'li olanla aynı. İkililer: taban `taban-1f6c3433` (`d330b82e…`), aday `aday-0a251074` (`02340156…`). Tam analiz bütün hücreler bitince yapılacak; bu not yalnız geçerlilik ve ilk gözlemdir.

**Geçerlilik (60/60 koşu).**
- rc = 0 her koşuda; alice.json / bob.json tam.
- Olay sayıları sıfır değil (send, rx, ts ≥ 100).
- Zarf taşıyıcısı `inits = 1` ve hatasız.
- Dinleyici düşüşü, iz düşüşü ve JSONL hatası 0.
- Netem: pre sayaçları her koşuda 0 (temiz kuyruk); seçenekler profille aynı (delay 0,3 s, jitter 0,06 s, loss 0,30, rate 12 500 B/s).
- Gerçekleşen kayıp, koşu medyanı (min–maks):

  | Yön | taban | aday |
  |---|---|---|
  | ileri | %31,1 (21,4–35,2) | %30,3 (23,6–35,3) |
  | dönüş | %29,8 (22,9–35,6) | %29,6 (20,1–35,3) |

- 1/60 koşu `incomplete` (taban rep11). Düzenek kusuru değil, ürün davranışı (aşağıda A-11).

**İlk gözlem.** Hücre içi değerler koşular arası medyan [Q1–Q3], saniye.

| Ölçü | taban | aday |
|---|---|---|
| TEXT p50 | 2,53 [1,52–5,77] | 1,31 [0,96–3,09] |
| TEXT p95 | 13,49 [10,27–18,88] | 10,74 [7,19–14,49] |
| TEXT p95 katmanları (fifo / net / reorder) | 3,7 / 7,3 / 8,3 | 6,7 / 4,7 / 4,6 |
| PTT demeti p50 | 5,29 | 4,78 |
| PTT demeti p95 | 15,22 | 12,35 |
| PTT ≤ 800 ms % | 2 [0–8] | 4 [2–8] |
| Sinyal p95 | 14,26 | 11,20 |
| TEXT alındısı p95 | 19,42 | 13,53 |
| `delivery_channel_drops` | 0 (30/30) | 0 (30/30) |
| NotReady düşüşü | 0 | 0 |
| Alıcıda en çok tutulan birim | 20 (11–37) | 15 (7–24) |
| alice pacer p50 (B/s; hat 12 500) | 362 [244–646] | 318 [247–643] |

- **Dosya olmadan da akışlar birbirini bekletiyor.** p95'te sıralama payı taban'da 8,3 s, aday'da 4,6 s; kaynak, kaybolan küçük birimlerin (TEXT / PTT / sinyal) oturum geneli `Reorder`'daki bekleme etkisi.
- **Gönderen kuyruğu payı A-9'dan geliyor.** Pacer hattın ≈ %3'ünde kalıyor; PTT basışı FIFO'da birikiyor.
- **Ön-kayıtla karşılaştırma:** §5.1'de TEXT p95 ≈ 2 s [1–4] demiştim; ölçülen 10,7–13,5 s. Sapma üç terimden geliyor: fifo (A-9), net (seyrek trafikte kendi kaybının PTO tabanlı kurtarması) ve reorder (A-2). PTT ≤ 800 ms tahminim %60–80'di; ölçülen %2–4.

**A-11 Tabanda kuyruk sonu birimi takılıyor (1/30).**
- Ölçüm: taban rep11'de son TEXT (birim 106, t0 + 103,5 s) ilk iletildi ama bob'a hiç ulaşmadı. alice'te `un = 1`, `inf = 1` olarak koşu sonuna dek (403 s) kaldı.
- Yeniden gönderim sayaçları ≈ 134 s'den sonra durdu (`rtx = 49`, `tlp = 24`); durum DrainRtprop'tan RecoverFwd'ye geçti ama yeni emisyon yok. Aday'da 0/30.
- Dosya/işlev: tail-recovery / PTO bütçesi (`sender.rs` `emit_tail_probe` `:5890`; `tail_max_probes`). Kök neden ve en dar düzeltme tam analizde, diğer taban hücreleriyle birlikte yazılacak.
