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

---

# TAM SONUÇ (2026-10-30)

Kaynak: `~/kama-k4g-work/ktp2-f0/runs/tam/a/out/`. 12 hücre, 470 koşu. İkililer: f0_flows taban-1f6c3433 (`d330b82e…`), aday-0a251074 (`02340156…`). Hücre dosyaları `1f6c3433`'ten beri aynı. Ön-kayıt §5 / §5.4 değiştirilmedi.

Satır numaraları taban `a858f3fc`'ye göredir; adayda (`49864510`) farklıysa "/ aday" ile verilir. Değerler koşular arası medyan [Q1–Q3] (min–maks), saniye. N < 30 olan ve stres (rinf) hücreleri betimseldir; hüküm cümlesinde kullanılmaz. Tablonun tamamı `reports/ktp2/f0/a/tam/tablolar.md`'de (T.1–T.8).

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

**Bütün koşularda:**
- Dinleyici düşüşü, birim izi düşüşü ve JSONL hatası 0.
- netem pre sayaçları 0 (temiz kuyruk).
- Oturum kapanma olayı (`Closed`) iki uçta toplam 0: 470/470 koşuda oturum hiç kopmadı.
- "Eksik", pencere içinde bitmemiş koşudur ve geçerli ölçümdür. Teslim edilmeyen öğe, gözlem sonundan alt sınırla (sansürlü) sayıldı.

**Ölçülemedi:**
- **rinf tasma_dosya, taban rep5 ve aday rep6 (rc = 124).** El sıkışma t0'da tamam. Dosya gönderen deposunu (4096) ve offer kanalını (1024) doldurdu. alice'in TEXT / sinyal gönderimleri geri basınçla bekledi: son gönderim 314 / 301 s'de, plan bitmedi. Pacer 165 / 89 B/s'de, durum RecoverFwd. Plan ve pencere 480 s bütçeyi aştı; `timeout` alice'i öldürdü. bob kendi süresinde (316 s) çıktı. Takılma düzenek kusuru değil, gönderen geri basıncıdır (B2 + A-10).
- **tasma_dosya (r12k ve rinf) TEXT / PTT gecikmesi.** Öğeler 2 MB / 8 MB dosyanın arkasındaydı ve pencere içinde teslim edilmedi. Değer sansürlüdür; gecikme sayısı olarak kullanılmaz.

## TS.2 Sonuç tabloları (birincil, N = 30/kol)

**TEXT (T) ve PTT demeti (P) teslim gecikmesi.** Bileşenler [türetilmiş]: fifo = gönderen yeni veri kuyruğu; net = ağ + birimin kendi kayıp kurtarması; reorder = alıcı oturum geneli sıralama, yani başka birimlerin kurtarmasını bekleme. Bileşen yüzdelikleri ayrı öğe kümelerindendir, toplanmaz.

| hücre | kol | T p50 | T p95 | T p50 fifo/net/reorder | T p95 fifo/net/reorder | P p95 | P ≤ 800 ms % |
|---|---|---|---|---|---|---|---|
| r12k dosyasiz | taban | 2,5 [1,5–5,8] | 13,5 [10,3–18,9] | 0,1/0,3/0,0 | 3,7/7,3/8,2 | 15,2 [11,4–18,4] | 2 [0–8] |
| r12k dosyasiz | aday | 1,3 [1,0–3,1] | 10,7 [7,2–14,5] | 0,1/0,3/0,0 | 6,7/4,7/4,6 | 12,3 [8,5–17,2] | 4 [2–8] |
| r12k dosyali | taban | 68,6 [33,9–139,3] | 113,6 [78,9–184,3] | 50,5/0,3/13,2 | 94,7/11,0/15,1 | 109,0 | 0 |
| r12k dosyali | aday | 31,2 [23,2–92,8] | 73,9 [67,7–134,7] | 26,8/0,3/1,5 | 70,6/2,1/2,5 | 71,5 | 0 |
| r12k dosyali_dur | taban | 52,8 [33,0–125,1] | 97,8 [77,7–169,8] | 38,0/0,3/10,5 | 82,5/8,9/12,4 | 93,2 | 0 |
| r12k dosyali_dur | aday | 58,8 [32,8–134,6] | 103,8 [76,3–179,0] | 56,3/0,3/1,8 | 99,3/2,2/3,5 | 99,2 | 0 |
| r1k dosyasiz | taban | 6,3 [3,7–14,4] | 22,3 [14,1–35,2] | 0,2/0,4/1,6 | 8,9/11,1/12,0 | 23,2 [16,2–33,1] | 0 |
| r1k dosyasiz | aday | 4,6 [2,5–11,2] | 14,7 [10,1–25,0] | 0,2/0,4/0,4 | 8,9/6,6/8,0 | 16,4 [12,3–30,7] | 2 |
| r1k dosyali | taban | 59,1 [53,3–77,1] | 102,3 [92,8–120,2] | 28,4/1,4/26,7 | 69,0/25,8/34,8 | 98,0 | 0 |
| r1k dosyali | aday | 63,8 [48,8–84,7] | 97,5 [87,5–117,1] | 55,1/0,6/3,5 | 89,3/6,0/8,3 | 94,4 | 0 |
| r1k dosyali_dur | taban | 56,8 [50,6–66,8] | 97,6 [92,3–110,0] | 25,6/2,0/25,6 | 66,3/24,6/33,1 | 96,0 | 0 |
| r1k dosyali_dur | aday | 60,3 [51,0–78,7] | 96,0 [87,0–108,8] | 54,1/0,6/3,7 | 85,9/5,7/9,8 | 97,2 | 0 |

**Kol farkları.** Taban − aday, medyan farkı (s), %95 aralığı. Yöntem: koşu birimli bootstrap, her kol kendi içinde yerine koyarak örneklendi, B = 10 000, tohum 20261030, %2,5–%97,5 yüzdelikleri.

| hücre | T p95 | T reorder p95 | T fifo p95 |
|---|---|---|---|
| r12k dosyasiz | +2,8 [−0,3, +6,6] | +3,7 [+1,1, +6,0] | −3,0 [−5,1, +2,0] |
| r12k dosyali | +39,7 [+2,2, +65,4] | +12,7 [+8,9, +16,3] | +24,1 [+1,0, +56,9] |
| r12k dosyali_dur | −6,1 [−51,3, +25,2] | +8,9 [+7,2, +14,1] | −16,8 [−63,4, +5,0] |
| r1k dosyasiz | +7,5 [−3,9, +13,9] | +3,9 [+0,4, +12,0] | −0,0 [−4,7, +2,9] |
| r1k dosyali | +4,9 [−3,1, +17,6] | +26,5 [+22,0, +32,9] | −20,3 [−28,7, −11,6] |
| r1k dosyali_dur | +1,6 [−7,4, +14,0] | +23,3 [+17,8, +26,2] | −19,5 [−30,8, −12,4] |

**Dosya etkisi (kol içi).** dosyali − dosyasiz, T p95 farkı, aynı yöntem:

| profil | taban | aday |
|---|---|---|
| r12k | +100,1 [+70,9, +125,5] | +63,2 [+58,4, +74,3] |
| r1k | +80,1 [+71,9, +91,5] | +82,8 [+70,9, +89,1] |

**Durma etkisi (kol içi).** dosyali_dur − dosyali ve dosyasiz_dur − dosyasiz, T p95 farkı, aynı yöntem:

| karşıt | profil | taban | aday |
|---|---|---|---|
| dosyali_dur − dosyali | r12k | −15,9 [−54,0, +24,5] | +29,9 [+1,5, +73,5] |
| dosyali_dur − dosyali | r1k | −4,7 [−12,4, +6,1] | −1,5 [−11,3, +12,6] |
| dosyasiz_dur − dosyasiz (betimsel N = 10) | r12k | +15,9 [+10,8, +24,8] | +16,1 [+14,6, +19,2] |

**K-1** (TEXT p95 ≤ dosyasız p95 + 2 × 1088 B serileştirmesi + 1 RTT; PTT ≤ 800 ms oranı ≥ dosyasızın %95'i): 4 birincil hücre × 2 kolun hepsinde KALDI. Aşım r12k'da 59–99 s, r1k'da 75–83 s.

## TS.3 Bulgular (ölçüm · dosya:işlev:satır · kök neden · en dar düzeltme)

1. **A-1 Gönderen tek FIFO, dosya tek seferde dökülüyor.**
   - Ölçüm: TEXT p95'te fifo bileşeni r12k dosyalıda 70,6–99,3 s (iki kol, N = 30); dosyasızda 3,7–6,7 s. Dosya etkisi T p95'te +63…+100 s.
   - Dosya:işlev:satır: `kama-app/src/file_transfer.rs:866-909` `pump` (kabul edilmeyen kayda dek sunar); `adaptive-transport/src/sender.rs:2309` / aday `:2592` `tx_queue.push_back`; `:6109` / aday `:7560` `drain_new_data`.
   - Kök neden [ölçüldü + kod]: bütün akışlar tek birim dizisine ve tek yeni-veri FIFO'suna giriyor; dosya ilk turda kuyruğa doluyor.
   - En dar düzeltme: FileTransfer'da teyitsiz dosya kaydı ≤ W (S1'den oturum başına teyitsiz sayaç gerekir) + motorda etkileşimli / toplu iki FIFO.
2. **A-2 Alıcıda oturum geneli tek `Reorder` (karşı örnek).**
   - Ölçüm: gönderen yeni veri kuyruğu boşken (fifo p50 0,11 s), taban'da TEXT p50 r12k'da 10,2 s (reorder 9,3 s; 33 ateşlenen koşu), r1k'da 30,0 s (reorder 25,1 s; 57 ateşlenen koşu). Aday r12k'da 1,5 s (reorder 0,8 s; 24 ateşlenen koşu); fark +8,6 s [+7,4, +10,7]. PTT demeti de aynı bekliyor (taban r12k 9,7 s). Ateşlenen koşu sayısı hücre başına < 30: betimsel.
   - Dosya:işlev:satır: `client-core/src/transport.rs:367-412` `Reorder::push`; `:1053-1082` `drive_deliveries`.
   - Kök neden [ölçüldü]: TEXT ve PTT, kaybolan eski dosya parçalarının kurtarılmasını alıcının oturum geneli sırasında bekliyor. Gönderende öncelik bu beklemeyi kaldırmaz.
   - En dar düzeltme: akış başına sıralama. Birim başlığında akış kimliği (tel) ya da sırasız teslim + kama-app'te akış başına sıra (S1 §4 değişikliği).
3. **A-4 Teslim kanalı taşması = teyitli kayıp.**
   - Ölçüm: yalnız taşma hücrelerinde ateşledi; birincil 240 koşuda 0.

     | hücre | taban: koşu (düşüş) | aday: koşu (düşüş) |
     |---|---|---|
     | tasma_dosya | 3/10 (168–531) | 5/10 (32–367) |
     | tasma_ptt | 1/5 (633) | 3/5 (162–535) |
     | rinf | 1/10 (1630) | 2/10 (437–1429) |

   - Sonrası:
     - r12k: tutulan birim < 4096; sıralı teslim sınırı boşlukta kaldı. Boşluktan sonraki hiçbir öğe teslim edilmedi (tasma_ptt'de koşu sonunda 1374–1968 birim tutuluyordu). Dosya bob'da `IdleTimeout` ile bitti (8/8 dosyalı düşüşlü koşu).
     - rinf: tutulan birim 4096'yı aşınca boşluk atlandı (atlanan = düşüş, 3/3). Sonraki TEXT teslim edildi, ama düşen birimler kalıcı kayıp; dosya bob'da başarısız.
     - Uygulama kurtarmadı: Messenger `resent_on_new_session` = 0, alındısız TEXT outbox'ta kaldı.
   - Dosya:işlev:satır: `client-core/src/runtime.rs:3010-3032` `push_delivery` (taşmada yalnız sayaç) ve ACK `:3602-3605` (teslimden bağımsız); aday `:3091-3113` / `:3683-3686`. Tüketim `transport.rs:1053-1082`, yalnız düğüm iş parçacığı `drive()` çağırınca.
   - Kök neden [ölçüldü + kod]: ACK, teslim kanalına kabulden önce ve ondan bağımsız üretiliyor.
   - En dar düzeltme: `on_datagram`'dan önce `delivery_tx.try_reserve()`. Yer yoksa NotReady kolu gibi ACK'siz düşür, ayrı sayaçla say; gönderen yeniden yollar.
4. **A-5 Akış başına tüketici yok.**
   - Ölçüm: dosyasiz_dur − dosyasiz T p95 r12k'da +15,9 / +16,1 s (betimsel N = 10).
   - Dosya:işlev:satır: `kama-node/src/node.rs:465-484` `turn`, dinleyici `:697`, `:759` (düğüm iş parçacığında).
   - Kök neden [ölçüldü]: tek tüketici iş parçacığı bütün akışları durduruyor.
   - En dar düzeltme: dinleyiciyi sınırlı kuyrukla düğüm iş parçacığından ayırmak + akış türü başına olay kuyruğu (kama-node).
5. **A-7 Gönderende 60 s `ReceiptTimeout`.**
   - Ölçüm: rinf dışındaki 280/280 dosyalı koşuda alice 60,0–60,2 s'de `SendFailed(ReceiptTimeout)` aldı. bob dosyayı bu koşuların 195'inde bayt-özdeş aldı; birincil 240 koşunun 189'unda (taban 76/120, aday 113/120).
   - Dosya:işlev:satır: `kama-app/src/file_transfer.rs:1541` (`AwaitingReceipt { since: now }`, `FILE_END` Transport'a kabul edildiği anda); `:1471-1476` `receipt_overdue`; `:267` `FILE_RECEIPT_TTL`.
   - Kök neden [ölçüldü + kod]: süre teslimden değil `FILE_END` kabulünden başlıyor; `pump` (A-1) bunu dosyanın başına çekiyor.
   - En dar düzeltme: süreyi son Transport teyidinden ya da son ilerlemeden başlatmak.
6. **A-8 Alıcıda `IdleTimeout` sıralama beklemesiyle.**
   - Ölçüm: r12k dosyalıda taban 34/60, aday 1/60 (Fisher p = 3,7e-12); r1k'da 10/60, 2/60 (p = 0,03).
   - Dosya:işlev:satır: `file_transfer.rs:262` `FILE_IDLE_TTL`, `:1054` `last_chunk_at`, `:1165` `expire_incoming`.
   - Kök neden [ölçüldü]: A-2 + A-3 nedeniyle uygulamaya 60 s parça çıkmıyor.
   - En dar düzeltme: A-2; ya da süreyi Transport ilerlemesine bağlamak.
7. **A-9 Uygulama sınırlı trafikte pacer düşük (= Ajan B B-K14).**
   - Ölçüm: pacer p50 r12k dosyasızda hattın %2,9 (taban) / %2,5 (aday)'i; r1k dosyasızda %20,3 / %20,1. Dosyalıda %50–89. Dosyasız TEXT p95'te fifo 3,7–8,9 s.
   - Dosya:işlev:satır ve kök neden: Ajan B (RAPOR-B B-K14).
   - En dar düzeltme: Ajan B.
8. **A-10 Hız çöküşü (iki kol).**
   - Ölçüm: sonda yeni veri kuyruğu ≥ 50 birimken pacer hattın %5'inin altında. Taban 12/235, aday 14/235 (Fisher p = 0,84). d1000 dosyali_dur'da 4/10 ve 7/10; r12k dosyalıda 2/60 ve 5/60. Çöken koşuda pacer 31–122 B/s'ye iniyor, 600 birim kuyrukta kalıyor.
   - Dosya:işlev:satır: pacer = BtlBw kestirimi (`sender.rs` hız boru hattı).
   - Kök neden [çıkarım]: Ajan B'nin alanı.
   - En dar düzeltme: Ajan B ile ortak.
9. **A-11 Kuyruk sonu birimi takılıyor (yalnız taban).**
   - Ölçüm: taban 4/235 (r12k dosyasiz rep11, r12k dosyasiz_dur rep9, r1k dosyasiz rep22, r12k dosyali rep5), aday 0/235 (Fisher p = 0,12: ölçülen hücrelerde fark istatistikle gösterilmedi).
     - Son birim ilk iletiliyor; alice'te `un ≤ 1`, `txq = 0`, durum RecoverFwd.
     - Son emisyon 107–253 s'de; sonra koşu sonuna (403 s) dek datagram yok.
   - Dosya:işlev:satır: `tail_recovery.rs:221` (pencere başına en çok `max_tail_probes = 3`, `:80` `validated_v1`); `loss.rs:692` (zaman eşiği kaybı yalnız `seq < largest_acked`); `sender.rs:5947-5966` `plan_forward_duplication`, sessizlik kolunda yalnız `origin == Data` (`:5954`).
   - Kök neden [kod + ölçüldü]: son birimden sonra ACK gelmediği için kayıp ilan edilmiyor (`pending_lost` boş). 3 kuyruk probu harcanınca birimin uçuş kaydı `Retransmit` kökenli oluyor; RecoverFwd sessizlik kolu onu kopyalamıyor; ARQ yalnız RecoverArq'ta. Hiçbir yol emisyon üretmiyor.
   - Aday'da sahipsiz birim H süresinden sonra kopya kuyruğuna alınıyor (`sender.rs:6925` `adopt_unowned`).
   - En dar düzeltme: 05b'nin sahiplenme kuralı; ya da sessizlik kolunda kökeni ne olursa olsun teyitsiz birimi kopyalamak.

**Ön-kayıttan sapmalar (§5 / §5.4 → ölçüm).**
- Dosyasız TEXT p95: r12k ≈ 2 → 10,7–13,5; r1k ≈ 4 → 14,7–22,3. Sapmanın kaynağı fifo (A-9), net (seyrek trafikte PTO ile kurtarma) ve reorder (küçük birimlerin birbirini beklemesi). Öngörü çürüdü.
- PTT ≤ 800 ms dosyasız: %60–80 → %0–4. Çürüdü.
- r1k dosyalı TEXT p50: ≈ 105 → 59–64. Dosya daha kısa sürdü; pacer hattın %74–89'u, öngörü u = 0,7–0,95 idi.
- r12k dosyalı TEXT p95: ≈ 75 → 73,9 (aday) / 113,6 (taban).
- `dosyali_dur ≈ dosyali` tutmadı: taban'da fark gösterilmedi, aday r12k'da +29,9 [+1,5, +73,5].
- Karşı örnek: taban 5–15 s (r12k) ve 15–40 s (r1k), reorder baskın → ölçülen 10,2 / 30,0 s, reorder baskın; tuttu. Aday 1–5 s → 1,5 s; tuttu.
- Teslim kanalı düşüşü: birincilde 0; tuttu. tasma_dosya'da 400–700 → 32–531, 8/20 koşuda. Aşağı sapma; alım hızı A-10 / A-9 ile düştü.
- r1k taban `IdleTimeout` "olası" → 10/60; tuttu.

## TS.4 Sınıflandırma (ISEMRI §5)

| davranış | (i) 05b düzeltiyor mu (ölçüm) | (ii) dar mı çekirdek mi | (iii) KTP-2 0.3 | (iv) korunacak avantaj |
|---|---|---|---|---|
| A-2 oturum geneli tek Reorder | **kısmen**: reorder p95 r1k dosyalıda −26,5 s [−32,9, −22,0], karşı örnekte 10,2 → 1,5 s. Yapı aynı, T p95 toplamı r1k'da fark gösterilmedi | **çekirdek / sözleşme**: sıra S1 §4'te oturum geneli; akış kimliği tele ya da S1'e girmeli | §3.1 (parça kimliği = akış + sıra), §3.2 ("hiçbir akış başka akışı sıra yoluyla bekletemez") | — |
| A-1 gönderen FIFO + toplu döküm | **hayır**: fifo p95 aday'da daha büyük (r1k +20,3 s [11,6, 28,7]) | **dar** (kama-app pencere W) + **orta** (motorda iki FIFO, S1 öncelik ipucu) | §3.6 zamanlayıcı, §3.7 akış kredisi, §3.9 parça boyu | — |
| A-4 ACK'li düşüş | **hayır**: aday 10/235, taban 5/235 düşüşlü koşu | **dar**: `runtime.rs` ACK'ten önce yer ayırma | §3.7 ("kredi verdiğini asla reddetmez"); TASLAK-NOTLARI H-14 (taslağın "teyitsiz red" tarifi bu yolda yanlış) | — |
| A-5 tek tüketici | **hayır** (+15,9 / +16,1 s) | **dar** (kama-node dinleyici ayrımı) + uygulama API'sinde akış başına kuyruk | §3.7 akış başına geri basınç | — |
| A-7 ReceiptTimeout | **hayır** (280/280; iki kol) | **dar** (kama-app) | karşılığı yok (uygulama) | — |
| A-8 IdleTimeout | **kısmen**: 34/60 → 1/60 (r12k), 10/60 → 2/60 (r1k) | **dar** (kama-app süresi) ya da A-2 | §3.2 dolaylı | — |
| A-11 kuyruk sonu takılması | **evet** (aday 0/235; mekanizma `adopt_unowned`) | **dar** (05b kuralı) | §3.4 "asla vazgeçmez", PTO 30 s'de bir sürer (TASLAK-NOTLARI D-K4: bugün 3 prob) | — |
| A-10 hız çöküşü | **hayır** (14/235 vs 12/235) | Ajan B | §3.5 hız | — |

(iv) Bu şeritte ölçülen korunacak özellik: %30 iki yönlü kayıpta 470/470 koşuda oturum kopmadı; 240 s tüketici durması dahil. Dosya, A-11 / A-10 / A-8 olmayan koşularda bayt-özdeş teslim edildi. Rakip kıyası bu şeridin işi değil (Ajan D).

## TS.5 F1 girdisi (dar düzeltme yolu için uygulama listesi)

| # | crate / dosya | değişiklik | bağımlılık | ölçü ve beklenen etki [tahmin] | süre |
|---|---|---|---|---|---|
| F1-A1 | client-core `runtime.rs` (`server_task`, `push_delivery`) | ACK'ten önce teslim kanalında yer ayır; yer yoksa ACK'siz düşür + `delivery_refused` sayacı | yok | K-1 / K-10: düşüş 0 (ölçülen 32–1630 birim / koşu), boşluğa takılma yok | 0,5 ajan-gün |
| F1-A2 | adaptive-transport `sender.rs` | A-11 için 05b'nin sahiplenme kuralı ya da sessizlik kolunda köken filtresini kaldırmak | 05b kararı | K-4 / K-8: kuyruk sonu takılması 4/235 → 0 (aday ölçümü 0/235) | 05b alınırsa 0; ayrı ise 1 ajan-gün |
| F1-A3 | kama-app `file_transfer.rs:1541` | `FILE_RECEIPT_TTL` süresini son ilerlemeden başlat | yok | dosya sonucu: yanlış `ReceiptTimeout` 280/280 → bob'un alındığı koşularda 0 (bob alındı 195/280) | 0,5 ajan-gün |
| F1-A4 | kama-app `file_transfer.rs` (`pump`) + kama-contracts S1 (oturum başına teyitsiz sayaç, sahip onayı) | Dosya kaydı uçuş penceresi W = 8 | S1 sözleşme ek alanı | K-1: TEXT fifo p95 70–99 s → ≤ W × 1088 B / hız (r12k 0,7 s, r1k 8,7 s). Toplam T p95'i A-2 sınırlar: dosyasız p95 + reorder (aday 2,5–8,3 s) | 2 ajan-gün |
| F1-A5 | kama-node `node.rs` | dinleyiciyi sınırlı kuyrukla düğüm iş parçacığından ayır; akış türü başına olay kuyruğu | kama-mobile API notu | K-1 durma terimi: dosyasiz_dur'da +16 s → ~0 (TEXT tüketicisi ayrıysa) | 1,5 ajan-gün |
| F1-A6 | client-core `transport.rs` + kama-app (akış başına sıra) | Reliable birimleri sırasız teslim; kama-app akış başına sıralar (FILE_BEGIN / PTT_OPEN ön-tamponu) | S1 §4 değişikliği (sahip) | K-1: karşı örnek 10–30 s → ağ + kendi kurtarması (aday reorder p50 0,8 s ölçüldü). Dosyalı T p95 reorder bileşeni 2,5–34,8 s → ~0 | 4 ajan-gün |

## TS.6 TABAN girdisi (S1)

**S1 — Akışlar birbirini bloke ediyor mu? Evet; ölçülen bütün hücrelerde ve iki kolda.** Ürün yolu `f0_flows`, iki süreç, %30 iki yönlü kayıp, N = 30/kol.

- **Dosya:** 512 KiB (r12k) / 64 KiB (r1k) dosya varken TEXT p95 dosyasıza göre +63…+100 s (r12k) ve +80…+83 s (r1k) uzuyor. K-1 4/4 hücrede iki kolda kalıyor. PTT demetlerinin 800 ms içinde teslimi %0.
- **Gönderen kuyruğu:** beklemenin büyük kısmı (TEXT p95'te 66–99 s) gönderenin tek yeni-veri FIFO'sunda (`sender.rs` `tx_queue`). `FileTransfer::pump` dosyanın tamamını ilk turda kuyruğa döküyor.
- **Alıcı sıralaması:** ikinci katman oturum geneli tek `Reorder` (`transport.rs:367-412`).
  - Tabanda TEXT p95'in 12–35 s'si, adayda 2,5–10 s'si burada.
  - Karşı örnek: gönderen kuyruğu boşken (fifo 0,11 s) TEXT, eski dosya parçalarının kurtarmasını alıcıda bekliyor. Taban'da p50 10,2 s (r12k) / 30,0 s (r1k); 05b adayında 1,5 s (r12k).
- **Alıcı kapasite reddi:** birincil hücrelerde 0. Teslim kanalı taşınca (yalnız taşma hücreleri) birim ACK'lenip düşüyor (`runtime.rs:3010-3032` + `:3602-3605`). Sonraki bütün öğeler oturumda takılıyor ya da 4096 birim sonra sessiz atlanıyor; uygulama kurtarmıyor.
- **Tüketici:** tek düğüm iş parçacığı; 30 s durma bütün akışlara +16 s p95 ekliyor.
- **05b:** sıralama beklemesini azaltıyor (r1k dosyalı reorder p95 −26,5 s [−32,9, −22,0]) ama yükü gönderen kuyruğuna kaydırıyor. Toplam TEXT p95'te yalnız r12k dosyalıda aday daha kısa (−39,7 s [−65,4, −2,2]); diğer hücrelerde fark gösterilmedi.
- **Dar düzeltmeyle çözülenler:** ACK'li düşüş, kuyruk sonu takılması, 60 s yanlış dosya hatası.
- **Çekirdek değişikliği isteyenler:** akış başına sıralama ve kuyruk (KTP-2 §3.2 / §3.6 / §3.7).

## TS.7 Yeniden üretim

```
python3 -I scripts/ktp2/f0/a/f0a_analyze.py --out <A> ~/kama-k4g-work/ktp2-f0/runs/tam/a/out
python3 -I scripts/ktp2/f0/a/f0a_tam.py --analiz <A> --csv reports/ktp2/f0/a/tam > reports/ktp2/f0/a/tam/tablolar.md
```

`<A>/OZET.md` → `reports/ktp2/f0/a/tam/OZET-analyze.md`. Ham kök: `~/kama-k4g-work/ktp2-f0/runs/tam/a/out/` (git dışı). sha256: `kosular.csv` 633e967b…, `hucreler.csv` 3bca1eb3…, `farklar.csv` 22524197…, `tablolar.md` cad5e0f6…, `OZET-analyze.md` 2ae5d805… (tam değerler commit'teki dosyalardan).
