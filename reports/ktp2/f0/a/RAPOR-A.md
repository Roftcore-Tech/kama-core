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
| TEXT p95 katmanları (fifo / net / reorder) | 3,7 / 7,3 / 8,2 | 6,7 / 4,7 / 4,6 |
| PTT demeti p50 | 5,29 | 4,78 |
| PTT demeti p95 | 15,22 | 12,35 |
| PTT ≤ 800 ms % | 2 [0–8] | 4 [2–8] |
| Sinyal p95 | 14,26 | 11,20 |
| TEXT alındısı p95 | 19,42 | 13,53 |
| `delivery_channel_drops` | 0 (30/30) | 0 (30/30) |
| NotReady düşüşü | 0 | 0 |
| Alıcıda en çok tutulan birim | 20 (11–37) | 15 (7–24) |
| alice pacer p50 (B/s; hat 12 500) | 362 [244–646] | 318 [247–643] |

- **Dosya olmadan da akışlar birbirini bekletiyor.** p95'te sıralama payı taban'da 8,2 s, aday'da 4,6 s; kaynak, kaybolan küçük birimlerin (TEXT / PTT / sinyal) oturum geneli `Reorder`'daki bekleme etkisi.
- **Gönderen kuyruğu payı A-9'dan geliyor.** Pacer hattın ≈ %3'ünde kalıyor; PTT basışı FIFO'da birikiyor.
- **Ön-kayıtla karşılaştırma:** §5.1'de TEXT p95 ≈ 2 s [1–4] demiştim; ölçülen 10,7–13,5 s. Sapma üç terimden geliyor: fifo (A-9), net (seyrek trafikte kendi kaybının PTO tabanlı kurtarması) ve reorder (A-2). PTT ≤ 800 ms tahminim %60–80'di; ölçülen %2–4.

**A-11 Tabanda kuyruk sonu birimi takılıyor (1/30).**
- Ölçüm: taban rep11'de son TEXT (birim 106, t0 + 103,5 s) ilk iletildi ama bob'a hiç ulaşmadı. alice'te `un = 1`, `inf = 1` olarak koşu sonuna dek (403 s) kaldı.
- Yeniden gönderim sayaçları ≈ 134 s'den sonra durdu (`rtx = 49`, `tlp = 24`); durum DrainRtprop'tan RecoverFwd'ye geçti ama yeni emisyon yok. Aday'da 0/30.
- Dosya/işlev: tail-recovery / PTO bütçesi (`sender.rs` `emit_tail_probe` `:5890`; `tail_max_probes`). Kök neden ve en dar düzeltme tam analizde, diğer taban hücreleriyle birlikte yazılacak.

---

# TAM SONUÇ (2026-10-30; düzeltme turu 1 sonrası)

Kaynak: `~/kama-k4g-work/ktp2-f0/runs/tam/a/out/`. 12 hücre, 470 koşu. İkililer: f0_flows taban-1f6c3433 (`d330b82e…`), aday-0a251074 (`02340156…`). Hücre dosyaları `1f6c3433`'ten beri aynı. Ön-kayıt §5 / §5.4 değiştirilmedi.

Satır numaraları taban `a858f3fc`'ye göredir; adayda (`49864510`) farklıysa "/ aday" ile verilir. Değerler koşular arası medyan [Q1–Q3] (min–maks), saniye. Medyan, Q1 ve Q3 en yakın sıradır (⌈p·n⌉'inci değer; çift N'de medyan alt orta değer); bootstrap da aynı tahminciyi kullanır. N < 30 olan ve stres (rinf) hücreleri betimseldir; hüküm cümlesinde kullanılmaz. Bütün sayılar `reports/ktp2/f0/a/tam/tablolar.md`'den (T.1–T.15) ve `kosular.csv`'den alınmıştır; hangi tablodan geldiği köşeli parantezde verilir.

Düzeltme turu 1 (doğrulama `A-dogrulama-1`, 61 kalem): analiz betikleri genişletildi (T.4 `pending_lost` ayrımı, T.9–T.15, `farklar.csv`'de filtre sonrası n). T.1–T.3, T.5–T.8'in önceki sayıları değişmedi; değişenler eklenen sütun, satır ve tablolar ile T.5'teki "TEXT teslim edilmeyen" sütununun biçimidir (medyan → koşu listesi).

## TS.1 Geçerlilik

| hücre | N/kol | rc | alice tamam/eksik/yok (taban · aday) | ileri / dönüş kayıp % (medyan) |
|---|---|---|---|---|
| r12k_d300 dosyasiz | 30 | 0×30 · 0×30 | 29/1/0 · 30/0/0 | 30,9 / 29,7 · 30,2 / 29,5 |
| r12k_d300 dosyali | 30 | 0×30 · 0×30 | 28/2/0 · 26/4/0 | 30,5 / 29,2 · 29,7 / 29,6 |
| r12k_d300 dosyali_dur | 30 | 0×30 · 0×30 | 28/2/0 · 29/1/0 | 30,1 / 30,3 · 29,9 / 29,5 |
| r1k_d300 dosyasiz | 30 | 0×30 · 0×30 | 29/1/0 · 30/0/0 | 29,5 / 28,1 · 29,0 / 29,8 |
| r1k_d300 dosyali | 30 | 0×30 · 0×30 | 30/0/0 · 30/0/0 | 29,6 / 29,0 · 29,0 / 28,4 |
| r1k_d300 dosyali_dur | 30 | 0×30 · 0×30 | 30/0/0 · 29/1/0 | 30,1 / 30,2 · 29,9 / 30,7 |
| r12k_d300 dosyasiz_dur (betimsel) | 10 | 0×10 · 0×10 | 9/1/0 · 10/0/0 | 30,7 / 28,9 · 30,0 / 31,0 |
| r12k_d300 tasma_ptt (betimsel) | 5 | 0×5 · 0×5 | 0/5/0 · 0/5/0 | 28,0 / 29,5 · 30,2 / 31,1 |
| r12k_d300 tasma_dosya (betimsel) | 10 | 0×10 · 0×10 | 0/10/0 · 0/10/0 | 30,1 / 30,4 · 29,6 / 28,6 |
| rinf_d300 tasma_dosya (betimsel, stres) | 10 | 0×9 + 124×1 · 0×9 + 124×1 | 0/9/1 · 0/9/1 | 30,5 / 29,1 · 29,2 / 29,5 |
| r12k_d1000 dosyasiz (betimsel) | 10 | 0×10 · 0×10 | 10/0/0 · 10/0/0 | 29,0 / 30,6 · 30,6 / 29,7 |
| r12k_d1000 dosyali_dur (betimsel) | 10 | 0×10 · 0×10 | 6/4/0 · 3/7/0 | 30,2 / 30,6 · 31,8 / 30,5 |

**Bütün koşularda [T.1, T.8 altı]:**
- Dinleyici düşüşü, birim izi düşüşü ve JSONL hatası 0.
- netem pre sayaçları 469/470 koşuda 0. İstisna: r12k_d300 dosyali_dur aday rep1, pre mb0 `drops = 1`, `packets = 0`. Kayıp oranı yalnız post sayaçtan hesaplanıyor (`f0a_tam.py` `realized_loss`); etkisi tek paket. (§10'daki "pre sayaçları her koşuda 0" cümlesi yalnız r12k dosyasiz hücresi içindir ve orada doğrudur.)
- Oturum olayları iki uç toplamı: `connected` 940 (= 2 × 470), `Closed` 0. 470/470 koşuda oturum hiç kopmadı ve hiç değişmedi.
- "Eksik", pencere içinde bitmemiş koşudur ve geçerli ölçümdür. Teslim edilmeyen öğe, gözlem sonundan alt sınırla (sansürlü) sayıldı.

**Ölçülemedi:**
- **rinf tasma_dosya, taban rep5 ve aday rep6 (rc = 124).** El sıkışma t0'da tamam. Dosya gönderen deposunu (4096) ve offer kanalını (1024) doldurdu. alice'in TEXT / sinyal gönderimleri geri basınçla bekledi: son gönderim 314 / 301 s'de, plan bitmedi. Pacer 165 / 89 B/s'de, durum RecoverFwd. Plan ve pencere 480 s bütçeyi aştı; `timeout` alice'i öldürdü. bob kendi süresinde (316 s) çıktı. Takılma düzenek kusuru değil, gönderen geri basıncıdır (B2 + A-10).
- **tasma_dosya (r12k ve rinf) TEXT / PTT gecikmesi.** Öğeler 2 MB / 8 MB dosyanın arkasındaydı ve pencere içinde teslim edilmedi. Değer sansürlüdür; gecikme sayısı olarak kullanılmaz.

## TS.2 Sonuç tabloları (birincil, N = 30/kol)

**Bileşen tanımları [türetilmiş; `f0a_analyze.py` belgesi]:**
- **fifo** = Transport kabulü → motorun ilk iletimi (üst sınır, ≈ 200 ms çözünürlük). İçinde offer kanalı, depo, `tx_queue`, pacing ve motorun yeniden gönderim payı var. Türetme FIFO sırasını varsayar.
- **net** = ilk iletim → alıcı adaptörün birimi runtime teslim kanalından çekmesi (`D` izi, `drive_deliveries`, alıcı düğüm iş parçacığında). İçinde ağ, birimin kendi kayıp kurtarması, runtime teslim kanalında bekleme ve alıcı düğüm iş parçacığı blokajı var. *_dur hücrelerinde 30 s durma buraya düşer: dosyasiz_dur T net p95 24,2 / 21,6 s, dosyasız 7,3 / 4,7 s [T.2]. `consumer` bileşeni durmayı ölçmez.
- **reorder** = alıcı oturum geneli sıralama, yani daha küçük kimlikli birimlerin teslimini bekleme.
- Bileşen yüzdelikleri yalnız teslim edilen öğelerden ve ayrı öğe kümelerinden gelir; toplanmaz. Toplam T p95 sansürlü alt sınırları da içerir. "Katman N" (katman p95'i tanımlı koşu): r12k dosyali 28 / 26, r12k dosyali_dur 28 / 29, diğer birincil hücreler 30 / 30 [T.2]. TEXT'i hiç teslim edilmeyen koşular katman sütunlarına girmez.

| hücre | kol | T p50 | T p95 | T p50 fifo/net/reorder | T p95 fifo/net/reorder | P p95 | P ≤ 800 ms % | S p95 |
|---|---|---|---|---|---|---|---|---|
| r12k dosyasiz | taban | 2,5 [1,5–5,8] | 13,5 [10,3–18,9] | 0,1/0,3/0,0 | 3,7/7,3/8,2 | 15,2 [11,4–18,4] | 2 [0–8] | 14,3 |
| r12k dosyasiz | aday | 1,3 [1,0–3,1] | 10,7 [7,2–14,5] | 0,1/0,3/0,0 | 6,7/4,7/4,6 | 12,3 [8,5–17,2] | 4 [2–8] | 11,2 |
| r12k dosyali | taban | 68,6 [33,9–139,3] | 113,6 [78,9–184,3] | 50,5/0,3/13,2 | 94,7/11,0/15,1 | 109,0 | 0 | 117,6 |
| r12k dosyali | aday | 31,2 [23,2–92,8] | 73,9 [67,7–134,7] | 26,8/0,3/1,5 | 70,6/2,1/2,5 | 71,5 | 0 | 77,9 |
| r12k dosyali_dur | taban | 52,8 [33,0–125,1] | 97,8 [77,7–169,8] | 38,0/0,3/10,5 | 82,5/8,9/12,4 | 93,2 | 0 | 101,8 |
| r12k dosyali_dur | aday | 58,8 [32,8–134,6] | 103,8 [76,3–179,0] | 56,3/0,3/1,8 | 99,3/2,2/3,5 | 99,2 | 0 | 107,8 |
| r1k dosyasiz | taban | 6,3 [3,7–14,4] | 22,3 [14,1–35,2] | 0,2/0,4/1,6 | 8,9/11,1/12,0 | 23,2 [16,2–33,1] | 0 | 23,5 |
| r1k dosyasiz | aday | 4,6 [2,5–11,2] | 14,7 [10,1–25,0] | 0,2/0,4/0,4 | 8,9/6,6/8,0 | 16,4 [12,3–30,7] | 2 | 16,2 |
| r1k dosyali | taban | 59,1 [53,3–77,1] | 102,3 [92,8–120,2] | 28,4/1,4/26,7 | 69,0/25,8/34,8 | 98,0 | 0 | 106,2 |
| r1k dosyali | aday | 63,8 [48,8–84,7] | 97,5 [87,5–117,1] | 55,1/0,6/3,5 | 89,3/6,0/8,3 | 94,4 | 0 | 99,9 |
| r1k dosyali_dur | taban | 56,8 [50,6–66,8] | 97,6 [92,3–110,0] | 25,6/2,0/25,6 | 66,3/24,6/33,1 | 96,0 | 0 | 101,6 |
| r1k dosyali_dur | aday | 60,3 [51,0–78,7] | 96,0 [87,0–108,8] | 54,1/0,6/3,7 | 85,9/5,7/9,8 | 97,2 | 0 | 98,0 |

**Kol farkları [T.3].** Taban − aday, medyan farkı (s), %95 aralığı. Yöntem: koşu birimli bootstrap, her kol kendi içinde yerine koyarak örneklendi, B = 10 000, tohum 20261030, %2,5–%97,5 yüzdelikleri. Bu raporda bütün kol farkları bu işaretle (taban − aday) verilir; pozitif = adayda daha kısa.

| hücre | T p95 | T reorder p95 (n) | T fifo p95 (n) |
|---|---|---|---|
| r12k dosyasiz | +2,8 [−0,3, +6,6] | +3,7 [+1,1, +6,0] | −3,0 [−5,1, +2,0] |
| r12k dosyali | +39,7 [+2,2, +65,4] | +12,7 [+8,9, +16,3] (28/26) | +24,1 [+1,0, +56,9] (29/26) |
| r12k dosyali_dur | −6,1 [−51,3, +25,2] | +8,9 [+7,2, +14,1] (28/29) | −16,8 [−63,4, +5,0] (28/29) |
| r1k dosyasiz | +7,5 [−3,9, +13,9] | +3,9 [+0,4, +12,0] | −0,0 [−4,7, +2,9] |
| r1k dosyali | +4,9 [−3,1, +17,6] | +26,5 [+22,0, +32,9] | −20,3 [−28,7, −11,6] |
| r1k dosyali_dur | +1,6 [−7,4, +14,0] | +23,3 [+17,8, +26,2] | −19,5 [−30,8, −12,4] |

T p95'te 6 karşılaştırmadan yalnız r12k dosyali'nin aralığı 0'ı dışlıyor; alt sınır +2,2 s, çoklu karşılaştırma düzeltmesi yok ve dosyali_dur'da tekrarlanmadı. Toplam TEXT p95'te 05b etkisi gösterilmedi.

**Dosya etkisi (kol içi) [T.7].** dosyali − dosyasiz, T p95 farkı:

| profil | taban | aday |
|---|---|---|
| r12k | +100,1 [+70,9, +125,5] | +63,2 [+58,4, +74,3] |
| r1k | +80,1 [+71,9, +91,5] | +82,8 [+70,9, +89,1] |

**Durma etkisi (kol içi) [T.7].** T p95 farkı:

| karşıt | profil | taban | aday |
|---|---|---|---|
| dosyali_dur − dosyali | r12k | −15,9 [−54,0, +24,5] | +29,9 [+1,5, +73,5] |
| dosyali_dur − dosyali | r1k | −4,7 [−12,4, +6,1] | −1,5 [−11,3, +12,6] |
| dosyasiz_dur − dosyasiz (betimsel N = 10) | r12k | +15,9 [+10,8, +24,8] | +16,1 [+14,6, +19,2] |

Aday r12k'daki +29,9 s'nin alt sınırı 0'a yakın ve Monte-Carlo tohumuna duyarlı (doğrulama turu başka tohumlarla 0'ı içeren aralık buldu). Fark sınırda; gösterildi denemez. Dosyalı hücrelerde durma etkisi iki kolda da gösterilmedi.

**K-1 [T.9].** Taslak §6 K-1 koşulu: "2 MB dosya + mesajlar + PTT; %30 kayıp; dosya tüketicisi 30 s durduruluyor"; dört ölçüt.
- **Kapsam.** Bu şerit yalnız (1) mesaj ölçütünü değerlendirdi: TEXT p95 ≤ dosyasız p95 + 2 × 1088 B serileştirmesi + 1 RTT. (2) ses oynatılabilirliği yerine vekil olarak PTT ≤ 800 ms oranı kullanıldı [türetilmiş; 800 ms ISEMRI son tarih adayıdır]. (3) alıcı belleği ≤ kredi ve (4) tüketici açılınca dosya tamamlanır ölçütleri değerlendirilmedi.
- **Koşul farkı.** Dosya 512 KiB (r12k) / 64 KiB (r1k); 2 MB değil. K-1 koşulunu tam karşılayan hücre dosyali_dur'dur; dosyali durmasız ek hücredir.
- **TEXT ölçütü:** 8/8 hücre-kolda KALDI. Her hücrede 30/30 koşunun p95'i eşiğin üstünde. Aşım (hücre medyanı p95 − eşik) r12k'da 62–99 s, r1k'da 73–80 s. Eşik payı r12k'da +0,8 s, r1k'da +2,8 s.
- **PTT vekil ölçütü:** 6/8 hücre-kolda KALDI. r1k tabanda dosyasız oran %0 olduğu için tanımsız. Dosyasız oran zaten %0–4 olduğundan bu ölçüt az bilgi taşıyor.
- **Hüküm.** KALDI hükmü TEXT ölçütünden geliyor. En büyük tel paketi mühürlü FEC parçasında 1088 B'tan büyük olabilir [doğrulanmadı]; eşiğe etkisi < 0,1 s, hükmü değiştirmez.
- **Önceki metin.** OZET-analyze.md'deki K-1 tablosu yalnız teslim edilen öğelerden p95 alıyor (sansürsüz), bu yüzden değerleri biraz farklı. Önceki metindeki "aşım r12k 59–99 s, r1k 75–83 s" yanlıştı; bu bölüm onun yerine geçer.

## TS.3 Bulgular (ölçüm · dosya:işlev:satır · kök neden · en dar düzeltme)

1. **A-1 Gönderen tek FIFO, dosya tek seferde dökülüyor.**
   - Ölçüm:
     - Dosya parçalarının hepsi ilk turda Transport'a kabul ediliyor [ölçüldü]. Son parçanın kabulü dosya gönderiminden 0,011–0,012 s (r12k, 530 parça) / 0,006 s (r1k, 66 parça) sonra (hücre medyanları) [T.15]. Yeni veri kuyruğu dosyadan sonra ilk kez 78–117 s'de boşalıyor [T.15].
     - TEXT p95 fifo bileşeni [türetilmiş, üst sınır]: dosyalı hücrelerde r12k 70,6–99,3 s, r1k 66,3–89,3 s (iki kol); dosyasızda 3,7–8,9 s.
     - Dosya etkisi T p95'te +63…+100 s [T.7].
   - Dosya:işlev:satır: `kama-app/src/file_transfer.rs:866-909` `pump` (kabul edilmeyen kayda dek sunar); `adaptive-transport/src/sender.rs:2309` / aday `:2592` `tx_queue.push_back`; `:6109` / aday `:7560` `drain_new_data`; `client-core/src/runtime.rs:2452-2453` (birim kimliği offer alımında tek sayaçtan).
   - Kök neden:
     - Bütün akışlar tek birim dizisine ve tek yeni-veri FIFO'suna giriyor [kod].
     - Dosya ilk turda kuyruğa doluyor [ölçüldü, T.15].
     - fifo büyüklüğü [türetilmiş]; türetme FIFO sırasını varsaydığı için tek-FIFO yapısını kendi başına sınamaz.
   - En dar düzeltme:
     - FileTransfer'da teyitsiz dosya kaydı ≤ W (F1-A4). Bunun için S1'den oturum başına teyit sinyali gerekir; bu bir sözleşme ekidir.
     - Motorda iki FIFO tek başına etkisiz [kod]. Birim kimliği alımda tek sayaçla veriliyor (`runtime.rs:2452-2453`), alıcı `Reorder` kesin kimlik sırasıyla bırakıyor (`transport.rs:367-412`). Bu yüzden gönderende öne geçen TEXT, alıcıda daha küçük kimlikli dosya birimlerini bekler.
     - İki FIFO yalnız F1-A6 (akış başına sıra) ile anlamlıdır. Ayrıca S1 öncelik ipucu, offer demeti (`runtime.rs:1482` `(Vec<u8>, Option<u64>)`) ve `OpaqueUnit` alanı ister. Dar düzeltme değildir.
2. **A-2 Alıcıda oturum geneli tek `Reorder`.**
   - Ölçüm, birincil (N = 30) [T.3]: T reorder p95 taban − aday:

     | | r12k | r1k |
     |---|---|---|
     | dosyali | +12,7 [+8,9, +16,3] | +26,5 [+22,0, +32,9] |
     | dosyali_dur | +8,9 [+7,2, +14,1] | +23,3 [+17,8, +26,2] |

   - İlk engelleyici [türetilmiş, T.10]. Ölçü: reorder'da ≥ 1 s bekleyen TEXT beklemesinin, bekleme süresiyle ağırlıklı payı.
     - Taban, dosyalı hücreler: beklemenin %98,0–99,8'i dosya parçası boşluğundan.
     - Aday: dosya parçası payı r12k dosyali %69,0, dosyali_dur %34,6; r1k %25,8 / %26,8. Geri kalan TEXT, PTT ve sinyal.
     - Dosyasız birincil hücrelerde engelleyici: TEXT %32–59, PTT %22–55, sinyal %6–38.
   - Karşı örnek [betimsel: ateşlenen alt küme, hücre başına < 30 koşu; T.4]:
     - Tetik: yeni veri kuyruğu 0 + teyitsiz birim ≥ 3 (`f0_flows.rs`). Bu tetik ISEMRI'nin "eski dosya parçalarının yeniden gönderimi canlı" koşulunu garanti etmiyor. Koşul, tetik anında `pending_lost > 0` (pl) olarak ayrıca uygulandı.
     - Taban: pl > 0 koşulu ateşlenen 33/33 (r12k) ve 57/57 (r1k) koşuda sağlandı; hepsi RecoverArq'taydı. Aday r12k'da ateşlenen 24 koşunun yalnız 7'sinde sağlandı; 17'sinde pl = 0, kurtarma beklenmiyordu.
     - pl > 0 koşularda K-TEXT (48 B, koşu başına 5 öğe) koşu p50 medyanı:

       | | taban dosyali | taban dosyali_dur | aday (havuzlu) |
       |---|---|---|---|
       | r12k | 10,9 s (reorder 10,0) | 10,0 s (8,9) | 2,1 s (n = 7) |
       | r1k | 30,4 s (25,1) | 28,8 s (24,9) | 2/60 koşu (65,8 / 10,1 s), ölçülemedi |

     - Havuzlu r12k farkı taban − aday +8,1 s [+6,4, +10,4]. Bootstrap ayrı tohumla yapıldı; iki kolda ateşlenme seçimi farklı olduğu için betimseldir.
     - d1000: aday 3/10 koşuda ateşlendi, üçünde de pl = 0 (15,2 s); taban 4/10, hepsi pl > 0 (10,2 s).
     - K-öğesinin ilk engelleyicisi dosya parçası: taban r12k 164/165, aday r12k 24/120.
     - Karşı örnekte fifo ≈ 0, çünkü tetik tanımı txq = 0 istiyor; bu bir bulgu değildir.
   - Dosya:işlev:satır: `client-core/src/transport.rs:367-412` `Reorder::push`; `:1053-1082` `drive_deliveries`. Motor zaten sırasız teslim ediyor (`adaptive-transport/src/receiver.rs:67`); sıralama adaptörde.
   - Kök neden [ölçüldü + türetilmiş]:
     - TEXT ve PTT, alıcının oturum geneli sırasında daha küçük kimlikli birimlerin kurtarılmasını bekliyor.
     - Tabanda bu birimler neredeyse hep dosya parçası. Adayda kalan beklemenin büyük kısmı küçük birimlerden geliyor (r1k'da dosya payı %26).
     - Gönderende öncelik bu beklemeyi kaldırmaz [kod].
   - En dar düzeltme: akış başına sıralama (F1-A6). Gereken: S1 §4 değişikliği + adaptör/kama-app; motor değişmez.
     - PTT için mevcut `DeadlineBounded` izni tek başına yetmez [kod]. Alıcıdaki `Delivery` politika taşımıyor (`runtime.rs:1190-1197`). Adaptör bütün birimleri aynı `Reorder`'a sokuyor (`transport.rs:22-28`). `DeadlineBounded` bugün gönderende yalnız FEC politika girdisi (`runtime.rs:1500-1511`, `:2434-2436`).
     - Alıcının PTT'yi sırasız vermesi için birim başına politika bayrağı (tel değişikliği) ya da F1-A6 gerekir.
3. **A-3 Motor kurtarma işi: FIFO doluyken ARQ payı ve yeniden kayıp vadesi.**
   - Ölçüm:
     - ARQ yeniden gönderim baytı / toplam tel baytı, yeni veri kuyruğu doluyken, dosya gönderiminden sonra [T.14]:

       | | r12k dosyali | r12k dosyali_dur | r1k dosyali | r1k dosyali_dur |
       |---|---|---|---|---|
       | taban | %23,3 | %24,0 | %16,3 | %16,8 |
       | aday | %28,9 | %29,2 | %29,1 | %28,5 |

     - Bütün koşu boyunca bu pay dosyalı hücrelerde iki kolda %27,4–29,0. Yani taban kurtarmanın bir kısmını FIFO boşaldıktan sonraya bırakıyor; aday FIFO doluyken yapıyor.
     - Taban r12k dosyalı `IdleTimeout` pencerelerinde (34 koşu, hatadan önceki 60 s) [T.11]:
       - alıcıda `next` 34/34 koşuda sabit; tutulan birim 31/34 koşuda pencere boyunca > 0;
       - gönderen örneklerinin en az yarısında txq > 0: 33/34 koşu; pl > 0: 32/34; RecoverArq: 32/34.
   - Dosya:işlev:satır: `sender.rs:4277-4296` `schedule_transmit` (DATA-önce), `K_ARQ_SHARE` `:1535`, `:3493-3499` yeniden kayıp vadesi `deadline_ns: now_ns`, `arq.rs:111-133`. Aday: `:4748-4800`, `:1722`, `:3972-3978`.
   - Kök neden [çıkarım + kod]:
     - FIFO doluyken ARQ payı ¼ ile sınırlı. Yeniden kaybolan birim vade sırasının sonuna düşüyor (B6).
     - Sonuç: baş boşluk geç kurtarılıyor ve alıcı uygulamaya 60 s parça veremiyor (A-8).
     - A-3'ün gecikmeye katkısı doğrudan ölçülmedi, çünkü birim başına kurtarma anı izlenmiyor. Atıf kol farkından ve koddan çıkarımdır.
   - En dar düzeltme (motor çekirdeği): yeniden kayıpta birimin ilk kayıp vadesini korumak + baş boşluk teslimi bekletirken ARQ'ya öncelik. 05b'nin ARQ ↔ DATA dönüşümü bunun bir kısmını yapıyor. KTP-2 §3.4 / §3.6.
4. **A-4 Teslim kanalı taşması = teyitli kayıp.**
   - Ölçüm: yalnız taşma hücrelerinde ateşledi; birincil 360 koşuda (taşma dışı 420/420) 0 [T.5].

     | hücre | taban: koşu (düşüş) | aday: koşu (düşüş) |
     |---|---|---|
     | tasma_dosya | 3/10 (168–531) | 5/10 (32–367) |
     | tasma_ptt | 1/5 (633) | 3/5 (162–535) |
     | rinf | 1/10 (1630) | 2/10 (437–1429) |

   - Sonrası [betimsel, N ≤ 10; T.5, kosular.csv]:
     - **tasma_ptt:** düşüşlü koşularda hiç teslim edilmeyen birim sayısı düşüşe eşit (taban 633; aday 162–535). Düşüşten sonraki öğeler teslim edilmedi (TEXT 26–29 / koşu). Koşu sonuna doğru görülen son `rx` bir teslim değil; `PTT_STREAM_IDLE_TTL` (120 s, `ptt.rs:215`) yerel kapanışıdır.
     - **tasma_dosya, düşüş ile boşluk:** hiç teslim edilmeyen birim düşüşten büyük (taban 531–831 / düşüş 168–531; aday 703–1044 / 32–367), çünkü koşu sonunda gelmemiş birimler de sayılıyor. Takılan boşluğun düşen birim olup olmadığı bu tablodan ayrılamaz.
     - **tasma_dosya, bob dosyası:** düşüşlü koşuların hepsinde `IdleTimeout` ile bitti (taban 3/3, aday 5/5). Ancak tabanda düşüşsüz 7/7 koşu da `IdleTimeout` ile bitti (A-8); bu sonuç düşüşe özgü değil. Düşüşle ilişki yalnız adayda görülüyor: düşüşlü 5/5, düşüşsüz 0/5.
     - **rinf:** tutulan birim 4096'ya ulaşınca boşluk atlandı (atlanan = düşüş, 3/3: 1630, 437, 1429). Sonraki TEXT 2/3 koşuda teslim edildi (taban rep7, aday rep10). Aday rep8'de 24 TEXT teslim edilmedi; gözlem penceresi bitmişti (sansürlü). Dosya bob'da 3/3 başarısız. Stres hücresi.
     - **Uygulama:** aynı oturum içinde uygulama düzeyinde yeniden gönderim yolu yok [kod: `messaging.rs` "Retry yok"]. `resent_on_new_session` yalnız oturum değişiminde çalışır. Oturum hiç değişmedi (TS.1), bu yüzden 0 beklenen değerdir; kurtarma olmadığının kanıtı sayılmaz. Alındısız TEXT kalması da düşüşe özgü değil: tasma hücrelerinin düşüşsüz koşularında da alındı 3–12/43. rinf düşüşlü koşularda alındılar 43/43, 48/48 ve 19/43; sonuncusu sansürlü.
   - Dosya:işlev:satır:
     - DATA yolu: `client-core/src/runtime.rs:3010-3032` `push_delivery` taşmada yalnız sayaç tutuyor (`:3031`); ACK `:3602-3605` teslimden bağımsız.
     - FEC yolu: `fec_ingest` kurtarılan ESI'yi `admit_chunk`'tan önce kaydediyor (`:3294` → `:3308`). `RECOVERED`, `push_delivery` sonucuna bakmadan gidiyor (DATA kolu `:3633`/`:3649`, FEC kolu `:3718`/`:3732-3736`). Gönderen onu alınca birimi depodan siliyor (`:2216` `store.remove`).
     - Aday satırları: `:3091-3113` / `:3683-3686`.
   - Kök neden [ölçüldü + kod]: motor ACK'i ya da FEC `RECOVERED` bildirimi teslim kanalı kabulünden bağımsız. İkisi de gönderende `store.remove`'a yol açıyor. Bu, kodun kendi INV-ACK-DELIVER-1 gerekçesiyle ("teslim yolunun kabul etmeyeceği birim ACK'lenmez", `runtime.rs:3450-3451`, `transport.rs:1066-1068`) çelişiyor.
   - En dar düzeltme (F1-A1):
     - Frag kapalı DATA: `slot.last_seen_ns = now` (`:3544`) satırından sonra, `on_datagram`'dan (`:3578`) önce `delivery_tx.try_reserve()`.
     - FEC kurtarımı: kurtarılan birim sayısı kadar yer ayırma ya da reddedilen ESI'yi `RECOVERED` dışında bırakıp bloğu korumak.
     - Ret yalnız teslim üretecek birime uygulanır (`delivered_seen`'de olmayan, `:3055`).
     - Ayrı `delivery_refused` sayacı tutulur.
     - Riskler: durma süresince yeniden gönderim israfı, BtlBw düşüşü, yuva emekliliği (`idle_hold` 105 s, `:270`, `:3430`).
     - `Reorder` atlaması (`transport.rs:398-401`) ayrı bir kalıcı kayıp yolu olarak kalır. Kalıcı çözüm alıcı kredisidir (KTP-2 §3.7); ret yalnız yerel güvenlik ağıdır.
5. **A-5 Akış başına tüketici yok.**
   - Ölçüm [betimsel, N = 10; T.7]: dosyasiz_dur − dosyasiz r12k'da TEXT p95 +15,9 / +16,1 s, PTT p95 +19,5 / +21,0 s. TEXT p50 +2,5 / +0,5 s. Dosyalı N = 30 hücrelerde durma etkisi gösterilmedi (TS.2).
   - Dosya:işlev:satır: `kama-node/src/node.rs:465-484` `turn`. Dinleyici çağrıları `:697`, `:715`, `:759`; ölçülen durma `:715`'te, `run` `:296` → `publish_metrics` yolunda. Dinleyici sözleşmesi `lib.rs:137-141`: "DÜĞÜM İŞ PARÇACIĞINDA çağrılır: hızlı dönmelidir". Ölçüm ikilisinde (1f6c3433) bu satırlar `:710/:728/:772`.
   - Kök neden [kod]: bütün akışlar tek düğüm iş parçacığında tüketiliyor; durma bu iş parçacığındaki dinleyici bloklanarak öykünüldü. Bu, belgelenmiş "hızlı dön" sözleşmesini bilerek ihlal eden bir dinleyicidir. Ölçüm yalnız büyüklüğü verir.
   - En dar düzeltme (F1-A5): dinleyiciyi sınırlı kuyrukla düğüm iş parçacığından ayırmak.
     - Dolu kuyruk politikası tanımlanmalı. Düşürme, Transport ACK'inden sonra olay kaybıdır (A-4 sınıfı); bloklama yeniden bağlar.
     - Tür başına geri basınç F1-A6 olmadan mümkün değil: `Ptt::poll` tek Messenger okuyucusu, Transport teslimi oturum geneli tek `Reorder` (`transport.rs:367-412`, `:1053-1082`).
6. **A-7 Gönderende 60 s `ReceiptTimeout`.**
   - Ölçüm [T.6]: rinf dışındaki 280/280 dosyalı koşuda alice 60,0–60,2 s'de `SendFailed(ReceiptTimeout)` aldı. bob dosyayı bu koşuların 195'inde bayt-özdeş aldı. Birincil dosyalı 240 koşuda bu sayı 189 (taban 76/120, aday 113/120).
   - Dosya:işlev:satır: `kama-app/src/file_transfer.rs:1541` (`AwaitingReceipt { since: now }`, `FILE_END` Transport'a kabul edildiği anda); `:1471-1476` `receipt_overdue`; `:267` `FILE_RECEIPT_TTL`. Normatif metin: `specs/application-message-protocol.md:91` (AMP §3 K4G-09: "`FILE_END` Transport'a kabul ettirildikten sonra").
   - Kök neden [ölçüldü + kod]: süre teslimden değil `FILE_END` kabulünden başlıyor; `pump` (A-1) bunu dosyanın başına çekiyor [T.15].
   - En dar düzeltme (F1-A3): süreyi son ilerlemeden başlatmak. Gerekenler:
     - AMP §3 amendment (sahip).
     - "Son ilerleme" sinyali: S1 teyit sinyali (F1-A4 ile ortak) ya da AMP'ye alıcıdan ilerleme kaydı.
     - Sözleşme değişmeden yapılabilecek bir seçenek yok; süreyi uzatmak da AMP'deki 60 s'yi değiştirir.
7. **A-8 Alıcıda `IdleTimeout` sıralama beklemesiyle.**
   - Ölçüm [T.8]:

     | hücre | taban | aday | p |
     |---|---|---|---|
     | r12k dosyali | 19/30 | 0/30 | 5,3e-8 |
     | r12k dosyali_dur | 15/30 | 1/30 | 6,4e-5 |
     | r1k dosyali | 6/30 | 2/30 | 0,25 |
     | r1k dosyali_dur | 4/30 | 0/30 | 0,11 |

     - Havuzlu (dosyali + dosyali_dur, 60 koşu): r12k 34/60 → 1/60 (p = 3,7e-12); r1k 10/60 → 2/60 (p = 0,03). r1k'daki p havuzlamaya bağlı ve T.8'deki 24 testten biri, düzeltmesiz. r1k için fark gösterildi denmez.
   - Pencere ölçümü [T.11]: bütün `IdleTimeout` koşularında, hatadan önceki 60 s'de alıcıda `next` sabit (86 koşunun 85'i). Birincil dur hücrelerinde 20 hatanın 7'sinde 60 s penceresi 30 s tüketici durmasını tümüyle içeriyor: taban r12k 5/15, taban r1k 1/4, aday r12k 1/1. Aday r12k'nın tek vakası bu türden.
   - Dosya:işlev:satır: `file_transfer.rs:262` `FILE_IDLE_TTL`, `:1054` `last_chunk_at`, `:1165` `expire_incoming`; normatif metin `specs/application-message-protocol.md:90`.
   - Kök neden:
     - [ölçüldü] A-2 sıra başı tıkanması: `next` 60 s sabit.
     - [çıkarım] A-3 payı: tabanda gönderen RecoverArq'ta, pl > 0 ve yeni veri sürüyor (T.11).
     - dur hücrelerinde A-5 ortak etken. Aday 1/60, 05b'nin A-2/A-3 etkisinin kanıtı sayılmaz.
   - En dar düzeltme: A-2 / A-3; ya da süreyi Transport ilerlemesine bağlamak (AMP §3 amendment + ilerleme sinyali, A-7 ile aynı bağımlılık).
8. **A-9 Uygulama sınırlı trafikte pacer düşük (= Ajan B B-K14).**
   - Ölçüm [T.6]: pacer p50, hat hızına oran:

     | | taban | aday |
     |---|---|---|
     | r12k dosyasız | %2,9 | %2,5 |
     | r1k dosyasız | %20,3 | %20,1 |
     | dosyalı hücreler | %50–89 | %50–89 |

     Dosyasız TEXT p95'te fifo 3,7–8,9 s.
   - Dosya:işlev:satır ve kök neden: Ajan B (RAPOR-B B-K14).
   - En dar düzeltme: Ajan B.
9. **A-10 Hız çöküşü (iki kol).**
   - Ölçüm [T.6, T.8, T.12]: tanım, sonda yeni veri kuyruğu ≥ 50 birimken pacer < hattın %5'i.
     - Taban 12/235, aday 14/235 (p = 0,84). d1000 dosyali_dur'da 4/10 ve 7/10; r12k dosyalıda 2/60 ve 5/60.
     - 26 çöküş koşusunda sonda pacer 38–509 B/s, 20 s sonrası en düşük 31–444 B/s, kuyrukta 130–2800 birim.
     - r12k_d300 dosyalı çöküşlerde (7 koşu) kuyruk 165–622 birim, sonda pacer 38–453 B/s.
   - Dosya:işlev:satır: pacer ≈ BtlBw × faz kazancı (`sender.rs:4239-4268` `compute_send_rate`: `effective_send_rate(btlbw, rtprop, srtt, faz, fuzzy)` + bütçe tabanı).
   - Kök neden [çıkarım]: Ajan B'nin alanı.
   - En dar düzeltme: Ajan B ile ortak.
10. **A-11 Kuyruk sonu birimi takılıyor (yalnız taban).**
    - Ölçüm [T.6, T.8, T.13]: taban 4/235, aday 0/235 (p = 0,12); birincil 3/180, 0/180 (p = 0,25). Fark istatistikle gösterilmedi.
      - Takılan koşular: r12k dosyasiz rep11, r12k dosyasiz_dur rep9, r1k dosyasiz rep22, r12k dosyali rep5.
      - Son emisyon 107,4–253,1 s'de. O anda durum DrainRtprop (3 koşu) ya da Startup (rep9); teyitsiz birim 1 (rep9'da 2).
      - RecoverFwd'ye geçiş son emisyondan 9,0–105,8 s sonra. Koşu sonunda durum RecoverFwd, teyitsiz 1, uçuş kaydı 1–2.
      - Son emisyondan sonra alice 6–9 datagram daha aldı ama uçuş kaydı küçülmedi. Sessiz kuyruk 150–296 s.
    - Dosya:işlev:satır: `tail_recovery.rs:221` (pencere başına en çok `max_tail_probes = 3`, `:80` `validated_v1`); `loss.rs:692` (zaman eşiği kaybı yalnız `seq < largest_acked`); `sender.rs:5926-5928` `emit_tail_probe` (orijinali tombstone'a alıp `track_sent(.., Retransmit)`); `:3222` (prob penceresi yalnız uçuş kaydı küçülünce sıfırlanır); `:5947-5966` `plan_forward_duplication`, sessizlik kolunda yalnız `origin == Data` (`:5954`); `:4290-4296` (RecoverFwd yalnız kopya, RecoverArq yalnız ARQ).
    - Kök neden [kod + ölçüldü]:
      - Son birimi ya da daha büyük bir seq'i teyit eden ACK gelmediği için kayıp ilan edilmiyor. Gelen datagramlar (+6…+9) uçuş kaydını küçültmüyor, bu yüzden `:3222` prob penceresini sıfırlamıyor.
      - İlk kuyruk probu (ya da ARQ) birimin kaydını `Retransmit` kökenli yapıyor. 3 prob harcanınca prob yolu kapanıyor.
      - Sessizlik DrainRtprop / Startup'ta başlıyor. Bu durumlarda ARQ ve kopya yok.
      - RecoverFwd'ye geçtikten sonra da sessizlik kolu `Retransmit` kökenli kaydı kopyalamıyor; ARQ yalnız RecoverArq'ta.
      - rep9 ve rep22'de teyitli birimin artık uçuş kaydı (uçuş 2, teyitsiz 1) prob bütçesini paylaşmış olabilir [çıkarım; prob hedefi loglanmıyor].
    - Aday [kod; hangisinin işlediği ölçülmedi]: teyitsiz birim üç yoldan kopya kuyruğuna devrediliyor:
      - prob bütçesi tükenince devir (`sender.rs:7164-7165`, "K4G-05b madde 4 (b)");
      - RecoverFwd sessizlik kolunda kökenden bağımsız sahiplenme (`:7183`);
      - H süresi sonunda `adopt_unowned` (`:6925`).
    - En dar düzeltme (F1-A2):
      - Durumdan bağımsız seçenek: 05b'nin devir kuralı (prob bütçesi tükenince teyitsiz birimi kopya kuyruğuna devretmek; 3. probta işler).
      - Sessizlik kolunda köken filtresini kaldırmak yalnız RecoverFwd'ye girilince etkilidir; ölçülen bekleme 9–106 s [T.13].

**Ön-kayıttan sapmalar (§5 / §5.4 → ölçüm).**
- Dosyasız TEXT p95: r12k ≈ 2 → 10,7–13,5; r1k ≈ 4 → 14,7–22,3. Öngörü çürüdü. Sapmanın kaynağı fifo (A-9), net (seyrek trafikte PTO ile kurtarma) ve reorder (küçük birimlerin birbirini beklemesi, T.10).
- PTT ≤ 800 ms dosyasız: %60–80 → %0–4. Çürüdü.
- r1k dosyalı TEXT p50: ≈ 105 → 59–64. Dosya daha kısa sürdü; pacer hattın %74–89'u, öngörü u = 0,7–0,95 idi.
- r12k dosyalı TEXT p95: ≈ 75 → 73,9 (aday) / 113,6 (taban).
- `dosyali_dur ≈ dosyali (±5 s)`: sınanamadı. Aralıklar ±5 s'den çok geniş; iki kolda fark gösterilmedi ya da sınırda (TS.2).
- Karşı örnek (pl > 0 alt kümesi, betimsel):
  - taban r12k 5–15 s, reorder baskın → 10,9 / 10,0 s (reorder 10,0 / 8,9); tuttu.
  - taban r1k 15–40 s → 30,4 / 28,8 s (reorder 25,1 / 24,9); tuttu.
  - aday r12k 1–5 s → 2,1 s (n = 7); tuttu.
  - aday r1k ≈ 2–10 s → 2/60 koşu; sınanamadı.
  - Ayrıca adayda tetik 17/24 koşuda kurtarma beklemezken ateşlendi. Bu bir düzenek sınırıdır: tetik ISEMRI koşulunu garanti etmiyordu.
- Teslim kanalı düşüşü: birincilde 0; tuttu. tasma_dosya 400–700 → 32–531, 8/20 koşuda; tasma_ptt 300–600 (her koşu) → 162–633, 4/10 koşuda. Aşağı sapma; alım hızı A-10 / A-9 ile düştü.
- r1k taban `IdleTimeout` "olası" → 10/60; tuttu. Kol farkı r1k'da gösterilmedi.

## TS.4 Sınıflandırma (ISEMRI §5)

Sınıf (tek dil):
- **dar** = tek crate, motor (adaptive-transport) dışında; S1 / AMP / tel değişmez.
- **sözleşme** = S1, AMP ya da tel biçimi değişir; sahip onayı gerekir.
- **motor çekirdeği** = adaptive-transport gönderen zamanlayıcısı ya da kurtarma kuralı.

(i) sütunu ölçüm ya da kod dayanağını ayrı yazar. Karşı örnek betimseldir ve hükme girmez.

| davranış | (i) 05b düzeltiyor mu | (ii) sınıf | (iii) KTP-2 0.3 | (iv) korunacak avantaj |
|---|---|---|---|---|
| A-2 oturum geneli tek Reorder | **kısmen** [ölçüldü, N = 30]: T reorder p95 taban − aday r1k +26,5 [+22,0, +32,9] / +23,3 [+17,8, +26,2], r12k +12,7 / +8,9. Yapı aynı [kod]. Toplam T p95'te 05b etkisi gösterilmedi | **sözleşme** (S1 §4) + adaptör/kama-app; motor değişmez (`receiver.rs:67`) | §3.1, §3.2 | — |
| A-1 gönderen FIFO + toplu döküm | **hayır**: yapı aynı [kod]. fifo p95 taban − aday r1k −20,3 [−28,7, −11,6] / −19,5 [−30,8, −12,4] (adayda daha uzun); r12k dosyali +24,1 [+1,0, +56,9] | W penceresi: **sözleşme** (kama-app + S1 teyit sinyali). İki FIFO: **sözleşme + motor çekirdeği**; yalnız F1-A6 ile etkili | §3.6, §3.7, §3.9 | — |
| A-3 kurtarma payı / yeniden kayıp vadesi | **kısmen** [ölçüldü]: FIFO doluyken ARQ payı taban %16,3–24,0 → aday %28,5–29,2 [T.14]. Etkisi A-2 / A-8 satırlarında | **motor çekirdeği** (`schedule_transmit`, `K_ARQ_SHARE`, yeniden kayıp vadesi) | §3.4, §3.6 | — |
| A-4 ACK'li düşüş | **hayır** [kod: `push_delivery` / ACK / `RECOVERED` iki kolda aynı]. Düşüşlü koşu aday 10/235, taban 5/235; hepsi betimsel/stres hücrelerinden | **dar** (client-core `runtime.rs`; DATA + FEC `RECOVERED` yolu) | §3.7; TASLAK-NOTLARI H-14 | — |
| A-5 tek tüketici | **hayır** [kod: kama-node / kama-app iki kolda özdeş, `git diff a858f3fc 49864510` boş]. Büyüklük betimsel (+15,9 / +16,1 s, N = 10) | **dar** (kama-node) + kama-mobile API notu; tür başına geri basınç F1-A6'ya bağlı | §3.7 | — |
| A-7 ReceiptTimeout | **hayır** (280/280; iki kol) | **sözleşme** (AMP §3 K4G-09, `FILE_RECEIPT_TTL` başlangıcı normatif) | karşılığı yok | — |
| A-8 IdleTimeout | **kısmen**: r12k dosyali 19/30 → 0/30 (p = 5,3e-8), dosyali_dur 15/30 → 1/30 (p = 6,4e-5). r1k 6/30 → 2/30 (p = 0,25), 4/30 → 0/30 (p = 0,11); r1k'da fark gösterilmedi. 24 test, düzeltmesiz | **sözleşme** (AMP §3 `FILE_IDLE_TTL`) ya da A-2 / A-3 üzerinden | §3.2 dolaylı | — |
| A-11 kuyruk sonu takılması | kod gereği giderilmesi beklenir [kod: aday `sender.rs:7164-7165` / `:7183` / `:6925`]. Ölçümde fark gösterilmedi (4/235 vs 0/235, p = 0,12) | **motor çekirdeği** (tek kural; 05b'de var) | §3.4; TASLAK-NOTLARI D-K4 (bugün 3 prob) | — |
| A-10 hız çöküşü | **hayır** (12/235 vs 14/235, p = 0,84) | **motor çekirdeği** (Ajan B) | §3.5 | — |

(iv) Bu şeritte ölçülen korunacak özellik: %30 iki yönlü kayıpta 470/470 koşuda oturum kopmadı; 240 s tüketici durması dahil. Dosya, A-11 / A-10 / A-8 olmayan koşularda bayt-özdeş teslim edildi. Rakip kıyası bu şeridin işi değil (Ajan D).

## TS.5 F1 girdisi (uygulama listesi)

Beklenen etkiler [tahmin]dir; F1'de ölçülecek.

| # | crate / dosya | değişiklik | sınıf ve bağımlılık | ölçü ve beklenen etki [tahmin] | süre |
|---|---|---|---|---|---|
| F1-A1 | client-core `runtime.rs` (`server_task`, `push_delivery`, `fec_ingest`) | DATA: `:3544` sonrası, `:3578` öncesi `try_reserve`; yer yoksa ACK'siz düşür. FEC: kurtarılan sayı kadar yer ya da reddedilen ESI'yi `RECOVERED` dışında bırak, bloğu koru. Ret yalnız `delivered_seen` dışındaki birime. Yeni `delivery_refused` sayacı; `delivery_dropped_mirror_tests` (`:3948-4015`) güncellenir | **dar**; bağımlılık yok. Durdurulan 05c'nin `DeliveryGate` teyitsiz reddine benzer (`wt/05c-kod`); 05c'nin kapsamı ayrı karar | K-1 / K-10: teslim kanalı düşüşü 0 (ölçülen 32–1630 birim/koşu). `Reorder` atlaması ayrı kalıcı kayıp yolu olarak kalır; "boşluğa takılma yok" tek başına garanti değil. Durma boyunca yeniden gönderim israfı + BtlBw düşüşü + yuva emekliliği riski | 1,5 ajan-gün |
| F1-A2 | adaptive-transport `sender.rs` | A-11: prob bütçesi tükenince teyitsiz birimi kopya kuyruğuna devret (05b kuralı, aday `:7164-7165`) | **motor çekirdeği**; 05b kararı | K-4 / K-8: kuyruk sonu takılması 4/235 → 0. Aday 0/235 ölçüldü, fark gösterilmedi (p = 0,12). Yalnız köken filtresini kaldırmak RecoverFwd'ye girişe dek 9–106 s geç kalır [T.13] | 05b alınırsa 0; ayrı ise 1 ajan-gün |
| F1-A3 | kama-app `file_transfer.rs:1541` + `specs/application-message-protocol.md:91` | `FILE_RECEIPT_TTL` süresini son ilerlemeden başlat | **sözleşme**: AMP §3 K4G-09 amendment (sahip) + ilerleme sinyali (S1 teyit sinyali, F1-A4 ile ortak; ya da AMP alıcı ilerleme kaydı) | dosya sonucu: yanlış `ReceiptTimeout` 280/280 → bob'un aldığı koşularda 0 (bob aldı 195/280) | 0,5 ajan-gün + spec turu |
| F1-A4 | kama-app `file_transfer.rs` (`pump`) + kama-contracts S1 (oturum başına teyit sinyali) | Dosya kaydı uçuş penceresi W = 8 | **sözleşme** (S1 ek alanı, sahip onayı); A-9 için Ajan B B-K14'e bağlı | Taban: TEXT fifo p95 ≥ dosyasız fifo p95 (3,7–8,9 s). A-9 düzelirse ek terim ≤ W × 1088 B / hat hızı (r12k 0,7 s, r1k 8,7 s). A-9 düzelmezse W = 8'de dosya uygulama sınırlı olur ve pacer dosyasız rejimine (r12k hattın %2,5–2,9'u, r1k %20) iner; ek terim ≈ W × 1088 / pacer ≈ 24–28 s (r12k), ≈ 43 s (r1k). W × 1088 B ≈ 8,7 kB, r12k'da BDP'ye eşit (12 500 B/s × ≥ 0,6 s RTT ≥ 7,5 kB): %30 kayıpta kurtarmadaki birimler pencereyi tutar, dosya verimi düşer ve A-7 / A-8 süreleriyle etkileşir. Toplam T p95'i A-2 sınırlar | 2 ajan-gün |
| F1-A5 | kama-node `node.rs` | Dinleyiciyi sınırlı kuyrukla düğüm iş parçacığından ayır. Dolu kuyruk politikası açık: tür başına tavan + düşüş sayacı (düşüş = ACK sonrası olay kaybı) | **dar** (kama-node); kama-mobile `ListenerBridge` API notu (`kama-mobile/src/lib.rs:620-628`). Tür başına geri basınç F1-A6'ya bağlı | K-1 durma terimi: dosyasiz_dur'da +16 s (betimsel) → ~0. Koşul: TEXT tüketicisi ayrı ve durmuş kuyruk taşmıyor. Gerçek dosya tüketicisi durmasında kuyruk 30 s'yi taşıyacak kadar büyük olmalı ya da düşürmeye izin verilmeli | 1,5 ajan-gün |
| F1-A6 | client-core `transport.rs` + kama-app (akış başına sıra) | Reliable birimleri sırasız teslim; kama-app akış başına sıralar (`FILE_BEGIN` / `PTT_OPEN` ön-tamponu) | **sözleşme** (S1 §4 değişikliği, sahip); motor değişmez | Taban: dosyalı reorder bileşeninin dosya kaynaklı kısmı (%98,0–99,8, T.10) → ~0. Karşı örnek → ağ + kendi kurtarması. Aday: dosya payı r1k %26–27, r12k %35–69; aynı akış içi bekleme (TEXT → TEXT) kalır. PTT için `DeadlineBounded` + birim başına politika bayrağı (tel) alternatiftir | 4 ajan-gün |

Listede olmayanlar:
- **A-3 (motor çekirdeği).** Yeniden kayıpta ilk vadeyi korumak ve baş boşluğa ARQ önceliği. KTP-2 §3.4 / §3.6 kapsamındadır; 05b kısmen yapıyor.
- **İki FIFO.** F1-A6 olmadan etkisizdir (A-1).

## TS.6 TABAN girdisi (S1)

**S1 — Akışlar birbirini bloke ediyor mu? Evet: 4 dosyalı birincil hücrede iki kolda (N = 30/kol, her hücrede 30/30 koşu K-1 TEXT eşiğinin üstünde [T.9]). Betimsel hücreler (N ≤ 10) aynı yönde.** Ürün yolu `f0_flows`, iki süreç, %30 iki yönlü kayıp.

- **Dosya:**
  - 512 KiB (r12k) / 64 KiB (r1k) dosya varken TEXT p95 dosyasıza göre +63…+100 s (r12k) ve +80…+83 s (r1k) uzuyor [T.7].
  - K-1 TEXT ölçütü 8/8 hücre-kolda KALDI; aşım r12k 62–99 s, r1k 73–80 s [T.9].
  - PTT ≤ 800 ms oranı dosyasızda da %0–4. Dosyanın ek etkisi bu ölçünün tabanında görünmüyor.
- **Gönderen kuyruğu [türetilmiş]:**
  - Dosyalı hücrelerde TEXT fifo bileşeni p95'i 66–99 s. Bu bileşen kabul → ilk iletim süresidir: offer kanalı + depo + `tx_queue` + pacing; motorun yeniden gönderim payı dahil.
  - `FileTransfer::pump` dosyanın tamamını ilk turda Transport'a veriyor (son parça kabulü, hücre medyanı ≤ 0,012 s, T.15) [ölçüldü].
- **Motor kurtarma işi [ölçüldü, T.14]:**
  - FIFO doluyken tel baytının ARQ yeniden gönderim payı taban %16,3–24,0, aday %28,5–29,2; bütün koşuda iki kolda %27,4–29,0.
  - Taban `IdleTimeout` pencerelerinde gönderen RecoverArq'ta, pl > 0 ve yeni veri sürerken alıcının `next`'i 60 s sabit [T.11] (A-3).
  - Karşı örnekte gönderen kuyruğu boş olduğu için bekleme reorder'da ölçülüyor.
  - İşlevler: `schedule_transmit`, `plan_arq_retransmits`, `K_ARQ_SHARE`, yeniden kayıp vadesi.
- **Alıcı sıralaması:** oturum geneli tek `Reorder` (`transport.rs:367-412`).
  - Dosyalı hücrelerde TEXT'in reorder bileşeni p95'i tabanda 12–35 s, adayda 2,5–10 s [türetilmiş; toplam p95'in toplamsal payı değil; r12k'da N = 26–29].
  - Tabanda bu beklemenin %98,0–99,8'i dosya parçası boşluğundan; adayda dosya payı r12k %35–69, r1k %26–27 [T.10].
  - Karşı örnek [betimsel, T.4]: gönderen kuyruğu boş + kurtarma canlıyken (pl > 0) TEXT p50 tabanda 10,0–10,9 s (r12k, 33 koşu) ve 28,8–30,4 s (r1k, 57 koşu); adayda r12k 2,1 s (7 koşu). Aday r1k'da karşı örnek 2/60 koşuda ateşlendi; ölçülemedi.
- **Alıcı kapasite reddi:**
  - Taşma dışı 420/420 koşuda 0.
  - Teslim kanalı taşınca birim teyitleniyor (ACK ya da FEC `RECOVERED`) ve düşüyor.
  - Sonrasında tasma_ptt'de sıra boşlukta takılıyor; rinf'te 4096 birim sonra boşluk sessiz atlanıyor.
  - tasma_dosya'da tabanda düşüşsüz koşular da takılı (A-8); bu sonuç düşüşe özgü değil.
  - Aynı oturumda uygulama kurtarma yolu yok [kod].
- **Tüketici:** tek düğüm iş parçacığı [kod].
  - Betimsel (N = 10): dosyasız r12k'da 30 s durma TEXT p95'e +16 s, PTT p95'e +20–21 s ekledi.
  - Dosyalı N = 30 hücrelerde durma etkisi gösterilmedi; aday r12k +29,9 [+1,5, +73,5] sınırda.
- **05b (taban − aday):**
  - r1k dosyalıda reorder p95'i kısalttı: +26,5 [+22,0, +32,9] / +23,3 [+17,8, +26,2]. Aynı hücrelerde fifo p95'i uzattı: −20,3 [−28,7, −11,6] / −19,5 [−30,8, −12,4].
  - r12k dosyali'de fifo adayda daha kısa (+24,1 [+1,0, +56,9]); r12k dosyali_dur'da fark gösterilmedi. Yani yükün gönderen kuyruğuna kayması yalnız r1k'da gösterildi.
  - Toplam TEXT p95'te 05b etkisi gösterilmedi. Tek hücrede (r12k dosyali) +39,7 [+2,2, +65,4], ama alt sınır 0'a yakın, dosyali_dur'da tekrarlanmadı ve 6 karşılaştırmada düzeltmesiz.
- **Dar düzeltmeyle çözülmesi beklenenler [tahmin/kod; F1'de ölçülecek]:**
  - ACK'li düşüş: F1-A1, DATA + FEC yolu; dar.
  - Kuyruk sonu takılması: F1-A2, 05b kuralı; motor çekirdeğinde tek kural.
  - 60 s yanlış dosya hatası: F1-A3; AMP §3 amendment gerekir, yani sözleşme.
- **Sözleşme değişikliği isteyenler:**
  - Akış başına sıralama (S1 §4; motor değişmez).
  - Gönderen dosya penceresi (S1 teyit sinyali).
  - Dosya zamanlayıcıları (AMP §3).
- **Motor çekirdeği:** kurtarma payı / yeniden kayıp vadesi (A-3), iki FIFO (yalnız F1-A6 ile), hız (A-9 / A-10, Ajan B). KTP-2 §3.2 / §3.4 / §3.6 / §3.7.

## TS.7 Yeniden üretim

```
python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <A> ~/kama-k4g-work/ktp2-f0/runs/tam/a/out
python3 -I scripts/ktp2/f0/a/f0a_tam.py --analiz <A> --csv reports/ktp2/f0/a/tam > reports/ktp2/f0/a/tam/tablolar.md
```

- `<A>/OZET.md` → `reports/ktp2/f0/a/tam/OZET-analyze.md`. Bu dosya düzeltme turunda bayt-özdeş kaldı.
- Ham kök: `~/kama-k4g-work/ktp2-f0/runs/tam/a/out/` (git dışı).
- Düzeltme turunda betik değişiklikleri:
  - `f0a_analyze.py`: öğe başına ilk engelleyici (`blk_unit`, `blk_c`) eklendi; `D` / net / fifo tanımları belgede düzeltildi.
  - `f0a_tam.py`: T.4 pl ayrımı + havuzlu fark; T.9–T.15; `farklar.csv` n = None olmayan koşu; T.1 pre sayacı; T.2 katman N; T.5 düşüşlü/düşüşsüz ayrımı; T.8 hücre başına A-8 ve birincil küme.
- sha256:
  - `kosular.csv` `1ddb44e6bd710f41a1d0ee46f9ca8f499b7205e7bcba1922bf7b8685ebddc0d3`
  - `hucreler.csv` `c9681b3150f74421611f1008b12e041e183af8e1f19151bdea86e2f293649981`
  - `farklar.csv` `12e4b3d5426c64b8bc1fd999f8a7319fb775af189862403dc89d72074ed6624f`
  - `tablolar.md` `2b3ab7cf5bb44deeed9967a0fa8ff935a437d3a3a569194c8148a78c5241a078`
  - `OZET-analyze.md` `2ae5d8052db3334c52e65be949b78407a04e627bb97308fa7ba2b084c76ffc3e`
