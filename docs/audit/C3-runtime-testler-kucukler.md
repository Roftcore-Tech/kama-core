# C3-runtime-testler-kucukler — davranış envanteri ve otorite karşılaştırması

Taban: `125b9afa87e731c1e666ecf63f33b7cc555dc9b1` (ürün hattı; aday `1a4f9efc` bunun içinde). Satır numaraları bu tabandandır. Salt okuma; kaynak dosyalara yazılmadı, cargo çalıştırılmadı.

## Kapsam

| dosya | satır aralığı | okunan satır |
|---|---|---|
| `crates/client-core/src/runtime.rs` (test modülleri) | 6462–8717 | 2 256 |
| `crates/client-core/src/memory_link.rs` | 1–336 | 336 |
| `crates/client-core/src/seam.rs` | 1–331 | 331 |
| `crates/client-core/src/lib.rs` | 1–273 | 273 |
| `crates/client-core/src/unit_id.rs` | 1–189 | 189 |
| `crates/client-core/src/wire.rs` | 1–448 | 448 |
| `crates/client-core/src/batch.rs` | 1–627 | 627 |
| **toplam (kapsam)** | | **4 460** |

Testlerin sabitlediği üretim işlevlerini anlamak için kapsam DIŞINDAN bağlam olarak okunanlar (bulgu bu dosyalara atanmadı, yalnız atıf): `runtime.rs` 75, 248, 335–338, 383, 395–565, 590–665, 780–839, 2280–2295, 2943–2948, 3073–3074, 4156, 4419–4436, 5300–5434, 6070, 6205–6239, 6440–6461; `lanes.rs` 186–205; `kama-noise-wire/src/entropy.rs` 23–29; `kama-engine/src/preparam_hold.rs` 13; `adaptive-transport/src/sender.rs` 194, 1121–1122.

## Davranış envanteri

### lib.rs (273)

- **Modül belgesi 1–37:** crate'i "kompozisyon kökü" olarak tanıtır; 20–37 "henüz yok" tablosu UDP döngüsü, bağlantı yaşam döngüsü, Noise, anahtar türetme, çıkış drenajı yok der. (Bayat: `runtime.rs` UDP döngüsü, `hub/` Noise bağı içeriyor — bulgu C3-08.)
- **Modül listesi 41–53:** `memory_link` yalnız `test`/`test-util` ile derlenir; diğerleri koşulsuz.
- **Yeniden dışa aktarım 55–64:** motor tipleri (`Sender`, `Event`, `TransmitDecision` …), `admit_and_decode`, `OpenedTransportShard`, `SourcePrefix`, `KamaV1Profile`, `TransportEngine`.
- **`ClientSession::new` 94–98:** `Sender::new()` + `TailRecoveryConfig::validated_v1()` (G2 kararı, 2026-08-24). Durum: gönderici sahipliği.
- **`ClientSession::with_cc_port` 105–109:** `Sender::with_cc_port()`; kuyruk kurtarması kurucuda. Belge "ürün varsayılanı `new` olmaya devam eder" der (C3-08).
- **`on_event` 115–117:** gönderici olayına geçiş; kararları döner.
- **`admit` 126–133:** `admit_and_decode(shards, profile, k_actual)`; argüman tipi `OpenedTransportShard` yalnız `protector.open()` ile elde edilir → INV-FEC-1 authenticate-before-decode tip düzeyinde.
- **`sender`/`sender_mut`/`Default` 137–152.**
- **Testler 154–273:** (a) iki yarı tek üretim grafiğinde: veri kararı üretilir, bilinmeyen profil `UnknownProfile(0xFF)` ile reddedilir; (b) `probes_after_converge_then_stall` ufku yapılandırmadan türetir (`initial_pto + max_ack_delay + max_backoff_pto × (max_tail_probes+1)`, adım `min_pto`); (c) `ClientSession::new` kuyruk yoklaması üretir (tail ON); (d) `Sender::new` aynı süpürmede 0 yoklama (R1: ham varsayılan OFF).
- Uyumlu: **4** (G2 tail-ON yalnız kurucuda; INV-FEC-1 tip kapısı; R1 ikiz bekçi; ufkun türetilmiş olması).

### seam.rs (331)

- **`EntropyError`, `EntropySource` 31–47:** `fill` ya tam doldurur ya hata. **`FixedEntropy` 53–84 (test/test-util):** sırayla verir, tükenince `EntropyError` (kısmi doldurma yok, `remaining` değişmez). Aynı trait ve tip `kama_noise_wire::entropy`'de de var; üretim (`hub/link.rs`, `hub/bind.rs`, `hub-server`) oradakini kullanıyor (C3-07).
- **`IdentitySeam` 95–211:** +16 B sahte etiket. Sabitler: `IDENTITY_TAG = 16` (ChaCha20-Poly1305 etiketiyle eşit), `PN_TAG_PREFIX = 8`, `MAX_UDP_PAYLOAD = 65 507` (= 65 535 − 20 − 8, türetilmiş), `LIMITS = {overhead 16, max_plaintext 65 491}`.
  - `check` 134–144: `Session(h)` için `h ≠ SESSION(0)` → `UnknownSession`; boy > 65 491 → `FrameTooLarge`.
  - `seal` 148–177: bağlam sınıfı başına ayrı PN sayacı (Session / HubChannel); `checked_add` taşarsa `CapReached`, sayaç ilerlemez; Session etiketinin ilk 8 baytı PN (BE), HubChannel etiketi sıfır; `meta = {pn, dh_epoch 0, sealed_len}`.
  - `open` 179–197: `len ≤ 16` ya da `> 65 507` → `None` (sessiz); aksi her datagram `Session(SESSION)`'a atfedilir, PN etiketin ilk 8 baytından okunur, etiket doğrulanmaz.
  - `sealed_len` kesin `n + 16`; `conservative_limits`/`limits` sabit `LIMITS`.
- **Testler 213–331:** v1 tel baytlarıyla eşitlik (PN 0'da etiket sıfır); PN ardışık ve bağlam sınıfı başına ayrı; etiket PN taşır, `open` okur; PN sarmaz ve reddedilen seal sayacı ilerletmez; yapısal retler tipli, `open` sessiz; `FixedEntropy` kesin ve fail-closed.
- Uyumlu: **7** (KM-SEAM-TC-001 K5 sessiz `open`, durum değişmez; `SealError` yalnız yapısal; K2 PN Crypto'da ve sarmaz; bağlam sınıfı başına ayrı PN uzayı (§1.1 S); `sealed_len` kesin; `Send + 'static`; "güvenlik iddiası yok" beyanı).

### unit_id.rs (189)

- **Düzen 13–47:** `unit_id = [şerit 2 bit][şerit-içi sıra 62 bit]`; şerit sırası PTT 0, SIG 1, TEXT 2, FILE 3; şeritler kapalıyken etiket 0 → eski kimliklerle bayt-aynı. FEC `block_id (u32) = [şerit 2 bit][şerit-içi blok 30 bit]`, "aşım `wrapping_add` ile şerit alanı içinde kalır" (C3-01).
- Sabitler: `LANE_SHIFT 62`, `LANE_TAG_MASK`, `LANE_SEQ_MASK`, `LANE_COUNT 4`, `FEC_BLOCK_LANE_SHIFT 30`, `FEC_BLOCK_SEQ_MASK 2^30−1` (hepsi bit düzeninden türetilmiş).
- `lane_index_of`, `lane_seq_of`, `tagged` (şerit ve sıra maskeyle kırpılır, sessiz), `index_of_lane`.
- `fec_block_of` 102–106: `(sıra / max(k,1)) as u32 & mask` (u64 → u32 kesme + maske; 2^30 blokta aynı kimliğe döner). `fec_esi_of` 111–113: `sıra % max(k,1)`.
- `first_fec_block_of_lane`, `next_fec_block` 125–127 (şerit içinde sarar), `unit_of_fec` 132–136 (tersi; `k_shape` 0'a karşı `max(1)` YOK — `fec_block_of` ile simetri bozuk, C3-01 notu).
- **Testler:** şerit 0 bayt-aynı; her şeritte gidiş-dönüş; şeritler çakışmaz ve `TEXT < FILE` sıralaması.
- Uyumlu: **5** (D-09 sınıf içi sıra garantisinin alıcıda türetilmesi, tel değişmeden; şerit 0 geriye uyum; blok/ESI tersinirliği; şerit blok uzaylarının ayrılması; tavanlı kümelerin "en eski / en kalabalık şerit" ile düşmesine atıf).

### wire.rs (448)

- **0x00 DATA 14–39:** `[0x00][seq u16][unit_id u64][payload]`, `HDR_DATA 11`; `decode_data` kısa/farklı tip → `None`. `seq` alanı alıcıda okunmaz (ARQ kimliği PN — transport-layer-tdd S4 notu).
- **0x09 PTT 41–82:** `[0x09][lane_seq lo16][payload]`, `HDR_PTT 3`; `unwrap_lane_seq(lo, limit)`: `limit` altındaki, alt 16 biti `lo` olan en büyük sıra; `limit = 0` → `None`. Doğruluk varsayımı "pencere ≤ 256 ≪ 2^16" yalnız yorumda (C3-13).
- **0x05 DATA_FRAG 84–135:** `[0x05][seq][unit_id][SourcePrefix 11 B ‖ payload]`, `HDR_DATA_FRAG 22`; çözücü prefix'i ayrıştırır ve `actual_payload_len`'i gövde boyuyla çapraz doğrular (fail-closed).
- **0x01 FEC 137–201:** `[0x01][kind][profile][block u32][esi u16][k_actual][seq u16][symbol]`, `HDR_FEC 12`; `k_actual = 0` → `None`; tür/profil/boy denetimi çözücüde değil, `runtime::fec_shard_fits`'te. `FecFrameHeader.seq` belgesi "motor wire-seq (ACK yolu)" (C3-15).
- **0x02 RECOVERED 203–247:** `[0x02][block u32][count u8][esi u16 × n]`, `RECOVERED_MAX_ESI 16` (noise-wire `RECOVERED_MAX_ESI_RANGES = 16` hizası); kodlayıcı fazlasını sessizce kırpar (C3-14); çözücü tam boy ister.
- **0x06/0x07 CS probe/echo 249–323:** sabit gövde 28 / 27 B; boylar "mühür +16 B → 44 / 43 B" hesabıyla seçilmiş (C3-06); amp ≤ 1 (echo < probe); çözücü tam boy ister. `seen_count/arrival_gap` yalnız kayıt (karar girdisi değil).
- **0x08 CAPS 325–363:** `[0x08][caps u16]`, bitler `CAP_ACK_BITMAP 0x1`, `CAP_PTT_LEAN 0x2`, `CAP_PTT_COALESCE 0x4` (deney); bilinmeyen bitler yok sayılır; kodun kendi notu: "[TEL DEĞİŞİKLİĞİ — sahip onayı gerekir.]" (C3-03).
- **0x03 PARAM / 0x04 PARAM_ACK 365–421:** `[0x03][profile][suite][hash 32]` (35 B), `[0x04][hash 32]` (33 B); tam boy denetimi.
- **Test 423–448:** PTT gidiş-dönüş; `[head, head+256)` penceresinde her sıra tek adaya açılır (2^16 sarmasında da); `limit 0` ve `300 < 256` → `None`.
- Uyumlu: **12** (ADR-05 sabit genişlik BE, varint yok — 8 çerçevenin hepsi; sabit boylu çerçevelerde tam boy fail-closed; `k_actual ≠ 0` (B4); RECOVERED sınırlı; DATA_FRAG uzunluk çapraz doğrulaması; CS amp ≤ 1; PARAM hash 32 B, PARAM_ACK hash yankısı (noise-wire §8); mühür bu katmanda yok — INV-ENGINE-SOC).

### batch.rs (627)

- ccr §35.14 `BatchPayload` codec'i; sabitler `SUBMSG_COUNT_LEN 1`, `SUBMSG_COUNT_MAX 255`, `LEN_MODE_INLINE_MAX 0xEF` (239), `LEN_MODE_EXTENDED 0xF0`, `EXTENDED_LENGTH_MAX 65 535`, `EMPTY_BATCH [0x00]` (hepsi spec alanlarından türetilmiş).
- `BatchError` 7 varyant; payload baytı taşımaz. `BatchPayload` ödünç dilimler; `Debug` yalnız sayı ve boylar.
- `LenHeader::for_len` tek canonical biçim; `plan` tahsisten önce tavanlar; `encoded_len`, `encode` (kısmi çıktı yok).
- `decode` 257–313: boş girdi; `count > kalan` tahsisten önce; kesik `len_mode`/uzunluk/payload; reserved `0xF1..0xFF`; inline'a sığan uzunluğun extended yazımı (`NonCanonicalLength`); artık bayt.
- **Testler (21):** boş batch tam 1 B, artık bayt reddi, boş girdi; sınırlar 0/1/239/240/65 535/65 536; 255 alt-mesaj ve 256 reddi; her reserved `len_mode`; canonical olmayan extended; tahsis öncesi sayı reddi; her kesik önek; LEB128 denemesi reddi; `Debug` sızıntısı yok; ödünç/sahipli eşitlik.
- Uyumlu: **11** (§35.14 yapısının tamamı; K3 boş batch; tek canonical encoding (SUPERSEDED alternatif form reddi); varint reddi; SR-AUDIT-001 bounds-before-alloc; hassas veri Debug'a girmez). **Üretimde çağıranı yok** (C3-05).

### memory_link.rs (336, yalnız test/test-util)

- `MemoryHub` (paylaşılan `Arc<Mutex<HubState>>`), `MemoryLink: EnvelopeLink`. Kilit zehirlenmesi `into_inner` ile yutulur (test).
- `relay` 113–180: kayıtsız varışa giden kuyrukta kalır; INIT başına `next_handle = wrapping_add(1).max(1)` (0 atlanır, ardışık); başlatana `HandleAssigned`, alıcıya `IncomingInit` aynı tutamaçla; `withhold_next_assignments`, `redeliver_next_inits` (ikinci teslim yeni tutamaçla, bayt-özdeş) arıza enjeksiyonu; RESPONSE verilen tutamaçla döner.
- `send_init` `refuse_next_inits` ile `LinkError`; `cancel_init` en çok `MAX_RECORDED_CANCELS = 64` kayıt (en eski düşer); `outbound` ve gelen kutuları sınırsız (C3-17).
- Test: röle olmadan hiçbir şey taşınmaz; kayıtsız varış bekler; INIT/HandleAssigned/RESPONSE akışı; `is_idle`.
- Uyumlu: **4** (direct-mode iki yön aynı tutamaç; röle beklemesi S1 `pump`; hub rölesi/hop-ARQ/HubChannel yokluğu beyanı; tutamaç tipi `HubHandle` ≠ `SessionHandle`).

### runtime.rs testleri 6462–8717 (2 256)

| modül | satır | sabitlediği davranış |
|---|---|---|
| `fec_payload_cap_tests` | 6462–6511 | `fec_payload_r_max`: sembol ölçülmüş yol hızında ≤ 2 s serileşiyorsa profil tavanı (r = 4), değilse `r ≤ payload_wire / parity_wire`; hız yok/0 → bayt kapısı; 540 B/s sınırda açık, 500 B/s kapalı |
| `skip_tests` | 6513–6576 | `unit_frame`: depoda gövde yoksa çerçeve yok, varsa gövdenin tamamı (0x00 ve 0x05); `repair_symbol`: blok/ESI yoksa `None`. `drive!` içindeki `let … else` kolu hiçbir testte koşmaz (kendi beyanı) |
| `outstanding_counter_tests` | 6578–6653 | `admit_counted`: artış kuyruğa koymadan önce, teyit üreticinin dönüşünden önce gelse de hayalet sayım yok; tavan / dolu kuyruk / kapalı kuyruk reddi geri alınır; `release_outstanding` 0'da doyar |
| `ctx_first_tests` | 6655–6895 | `param_ack_due`: yalnız bütçeli port yuvasında yinelenen PARAM_ACK `budgeted_ack_interval_ns(62)` aralığında bir; şerit kabul penceresi tabanı `max(sıra başı, kabul sınırı)`; pencere geri gitmez, en az 1, HubChannel pencereye tabi değil; `route_client` tam tutamaç ister; `route_server` tablo + yuva bağı, `NotReady` ayrı kol (INV-ACK-DELIVER-1); `HubDispatch` yalnız sayar; kapanan oturumun yuvası emekli, `NotReady` yuvası kalır |
| `delivery_dropped_mirror_tests` | 6897–6982 | teslim kanalı doluyken birim motora verilmez, ACK'lenmez, düşmez (`delivery_dropped = 0`, `data_not_admitted_full ≥ 15`); canlı ayna = kapanış sayacı. Kanıt 2 s boyunca "olmadı" gözlemidir |
| `live_diag_mirror_tests` | 6984–7397 | canlı ayna varsayılan kapalı (DUR-19), açıkken retransmit/RTT/ACK halkası/denetim durumu görünür; halkalar sınırlı (`ACK_LOG_LEN 32`, `STATE_LOG_LEN 32`, `LINE_GUARD_LOG_LEN`); hat koruması izi artımlı ve sınırlı, kural yoksa boş; `set_recovery(false)` retransmit'i tele çıkarmaz; ARQ kurtarması bayraktan bağımsız; dar dönüşte (12 500/62) son tarihli PTT birimi 1 s'de bırakılır, sayaç bir kez düşer, güvenilir PTT kaydı ACK'le teyit edilir, `ptt_lean_tx ≥ 1` |
| `pre_param_fec_guard_tests` | 7399–8002 | bind öncesi uyumsuz shard (boş, E±1, yanlış profil, bilinmeyen tür, aralık dışı ESI, k_actual > K) bekletme yuvası almaz, `unknown_frames`'e sayılır, **yine ACK'lenir**; bind sonrası aynı shard'lar **ACK'lenmez**; k_actual çelişkileri reddedilir; kaynak ESI'si onarımın k'sını çürütürse o onarımlar atılır; teslim edilemeyen kurtarılmış kaynak RECOVERED'a girmez |
| `lane_eviction_tests` | 8004–8088 | onarım deposu en eski gireni düşürür; alıcı FEC birleştiricisi ve dedup penceresi en kalabalık şeridin en eskisini düşürür; eşitlikte büyük indeks (düşük öncelik) düşer |
| `link_profile_window_tests` | 8090–8146 | `link_profile_window_bytes = uplink × budgeted_ack_interval(downlink)` (12 500 × 5,4 s = 67 500 B), yalnız 4·MTU tabanını aşınca; eski motor / yön hızı yok / aşağı yön 0 → `None`; tavan 1 MiB |
| `uplink_line_tests` | 8148–8360 | `param_retry_due` hat modeli yokken eski geri çekilmeyle bit-aynı (200 ms × 2^(n−1), ≤ 5 s); dar hatta PARAM hattı terk edip bir taban PTO (1 s) geçmeden yeniden denenmez; `link_feedback_starved` yalnız port + yön hızı + aralık ≥ 1 s; `fec_feedback_starved_permille(0x05) = 500`; `with_fec_prior` oranı yükseltir, düşürmez; dar hatta denetim hat doluyken bekler; `UplinkShare` her datagramı +28 B ile hat hızında serileştirir; 62 B/s yukarı yönde FEC kurulmaz, PARAM çıkmaz |
| `param_forward_copy_tests` | 8362–8656 | geri bildirimi aç profilde ve geniş yukarı yönde PARAM ACK'siz kopyaları `[200, 400, 800, 1 600]` ms, yarım tik payıyla; ilk PARAM ve ilk PARAM_ACK düşse de bağlama ≤ 1 s; PARAM_ACK'ten sonra kopya çıkmaz; profil kapalıyken 0/0,2/0,6 s geri çekilme |
| `ack_tick_priority_tests` | 8658–8717 | dar dönüşte (62 B/s) aynı turda bekleyen ACK ve bekletilen RECOVERED varsa hatta önce ACK çıkar; turda tek denetim datagramı |

Uyumlu: **~40 davranış**. En önemlileri: INV-ACK-DELIVER-1 (dolu kanalda ACK yok, teslim edilmeyen kurtarım RECOVERED'a girmez, `NotReady` Session koluna girmez); INV-FEC-1 alıcı yarısı (uyumsuz shard birleştiriciye girmez; noise-wire §2.6 MADDE-6 SYMBOL_LEN, §2.56 bilinmeyen codepoint fail-closed, B4 `1 ≤ k_actual ≤ K_shape`); CLAUDE.md YAMA 2026-11-16 profil kuralları (`link_feedback_starved` ≥ 1 s, 500 ‰ ACK'siz FEC); karar 7'nin %25 ACK bütçesi (`budgeted_ack_interval_ns(62) = 81 B / 15 B/s = 5,4 s`); SR-AUDIT-001 sınırlı halkalar; noise-wire §8 PARAM sınırlı yeniden deneme; DUR-19 varsayılan kapalı ayna.

## Bulgular

| fid | tür | ciddiyet | konum | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| C3-01 | UYUŞMAZ | düşük | `unit_id.rs:44-47`, `:102-106`, `:122-127`; `:132-136` | FEC `block_id` şerit başına 30 bitte sessizce sarar (`as u32 & mask`, `wrapping_add`); 2^30 bloktan sonra aynı `block_id` yeniden kullanılır. `unit_of_fec`'te `k_shape.max(1)` yok (tersi simetrik değil) | kama-noise-wire-tdd §2.6 MADDE-14: "block_id = session+direction-scoped-monoton-u32, **NO-REUSE** … **cap/wrap-öncesi session-renewal-ZORUNLU**" | kod-duzeltmesi |
| C3-02 | UYUŞMAZ | orta | `runtime.rs:4156`, `:6070` (kapsam dışı); sabitleyen test `runtime.rs:7450`; düzen `unit_id.rs:13-24` | 0x00 DATA'nın canonical source sembolünde `message_id = unit_id & 0xFFFF_FFFF`: şerit etiketi (bit 63–62) düşer, şeritler açıkken (`transport.rs:852`: `lane_ids = lanes.is_some()`) TEXT sıra 3 ile FILE sıra 3 aynı `message_id`'yi taşır | kama-noise-wire-tdd §2.6 MADDE-14: "message_id = session+direction-scoped-monoton-u32, **NO-REUSE**" | karar-gerekir (şerit 1–3 sembol baytı değişir; D-32 (iii) tel sınıfı) |
| C3-03 | TANIMSIZ | orta | `wire.rs:99-135` (0x05), `:266-323` (0x06/0x07), `:325-363` (0x08 CAPS + bitler) | client-core çerçeve düzleminin 0x05–0x08 codepoint'leri ve CAPS bit anlamları hiçbir donmuş spec'te yok; kodun kendi notu "[TEL DEĞİŞİKLİĞİ — sahip onayı gerekir.]" | spec'te bulunamadı (aranan: `FRAME_CAPS`, `CAP_ACK_BITMAP`, `CAP_PTT_LEAN`, `DATA_FRAG`, client-core frame_type tablosu — specs/, docs/seams/, docs/decisions/). KARAR-KAYDI §4 D-32 (iii): tel biçimi değişikliği sahip "MERGE" ister | karar-gerekir (spec YAMA kaydı) |
| C3-04 | TANIMSIZ | orta | `wire.rs:41-82` (0x09 PTT, `unwrap_lane_seq`) | yalın PTT çerçevesi ve alt-16-bit sıra geri kurulumu spec'te yok | spec'te bulunamadı (aranan: `FRAME_PTT`, `encode_ptt`, "0x09" client-core düzlemi — AMP'deki 0x09 `SIGNALING_BLOB` ayrı düzlem); karar 3 (7171705736): "PTT paketleme … yeni değişiklik yapmayın" | kilitli |
| C3-05 | EKSİK | orta | `batch.rs` (tamamı; üretimde çağıran yok); `wire.rs:14-39` | canonical birim çerçevesi `BatchPayload` kodlanmış ama veri yoluna bağlı değil; runtime interim 11 B `[type][seq][unit_id]` çerçeveyi kullanıyor | KM-SEAM-TC-001 K4/KARAR-1: "interim … 11 B çerçeve **emekliye ayrılır** (transport v1.x)"; ccr §35.14 AMENDMENT-6 notu "CANONICAL = BU YAPI"; transport-layer-tdd 862–864 "bu sprint beyanlı KARŞILANMAZ" | karar-gerekir (tel değişikliği; karar 3 PTT/metin dondurması) |
| C3-06 | UYUŞMAZ | düşük | `wire.rs:259-274` | CS probe/echo gövdeleri (28/27 B) "mühür +16 B → 44/43 B" hesabıyla seçilmiş; gerçek dikişte (legacy 22, extended 26 B ek) mühürlü probe 50/54 B olur, motorun `BS_PROBE_WIRE_EXACT = 44` bütçesiyle tutmaz (amp ≤ 1 korunur) | KM-SEAM-TC-001 §1.1: "[2026-09-19] Transport kripto format matematiğini tekrar ETMEZ"; fec-core §11: `sealed_len (EXTENDED) = 26 + …`, legacy 22 | karar-gerekir (gövde mi, şema sayısı mı değişir) |
| C3-07 | UYUŞMAZ | düşük | `seam.rs:29-84` | `EntropySource`/`EntropyError`/`FixedEntropy` ikinci kopya; üretim `kama_noise_wire::entropy`'yi kullanıyor, buradaki kopyanın üretim çağıranı yok (yalnız `tests/api_surface_v0.rs`) | KM-SEAM-TC-001 YAMA 2026-10-06b: "`EntropySource` … `kama-noise-wire`'ın public API'sinde tanımlanır … bugün `client-core/src/seam.rs`'te duran tanım oraya **taşınır**" | kod-duzeltmesi (`pub use kama_noise_wire::entropy::{…}` ile değiştir; API yüzey testi yolu korunur) |
| C3-08 | UYUŞMAZ | düşük | `lib.rs:100-104`, `:20-37`, `:80-89` | belge: "Ürün varsayılanı `Self::new` olmaya devam eder; `with_cc_port` ölçüm/gösterim anahtarı". Kod: `RuntimeConfig::default().engine = CcPort` (`runtime.rs:338`), `runtime.rs:3073-3074` ile port kurucusu seçilir. "Henüz yok" tablosu (UDP döngüsü, Noise, yaşam döngüsü) bayat. `new` belgesindeki "FEC `Manual{enabled:false}`" DOĞRULANAMADI | CLAUDE.md YAMA KAYDI 2026-11-11: "Ürün varsayılanı port (`RuntimeConfig::engine`), geri alma `KAMA_TRANSPORT_ENGINE=legacy`"; KARAR-KAYDI §8 D-CC-1 | kod-duzeltmesi (yalnız belge) |
| C3-09 | UYUŞMAZ | orta | `seam.rs:104-132` (`IDENTITY_TAG`, `LIMITS`) | stub'ın tel ek yükü 16 B; spec'teki Session ek yükü legacy 18 (+4 hub_handle) / extended 26 B. `bench` (`kama_product.rs`, `mixed.rs`) ürün kıyaslarını bu stub'la koşuyor → tel baytı 6–10 B/datagram eksik sayılır | KM-SEAM-TC-001 §1.1: "Session: legacy 18(+4 hub_handle)/extended 26"; fec-core §11; CLAUDE.md §8 "ölçülmemiş byte tahmini" yasağı | karar-gerekir (hangi profil modellenecek; bütün bayt tabanları değişir) |
| C3-10 | TANIMSIZ | orta | `runtime.rs:780-815` (kapsam dışı); sabitleyen testler `6462-6511` | parite tavanında sabit 2 s zaman kapısı + `r · parity_wire ≤ payload_wire` bayt kapısı; eşik türetilmemiş ("Sınır sabittir") | spec'te bulunamadı (aranan: parite serileştirme süresi eşiği). fec-core §11 yalnız "sub-kbps ≠ E=1024 … Sub-kbps-adayı = küçük-E" der; kod küçük-E seçimi yerine sabit kapı uygular | karar-gerekir |
| C3-11 | TANIMSIZ | düşük | `runtime.rs:6221-6231` (kapsam dışı); testler `7786-7801`, `7951-7970` | aynı uyumsuz FEC shard'ı bind öncesi ACK'lenir, bind sonrası ACK'lenmez; bekletme tavanından taşan geçerli shard da (`fec_pre_param_dropped`) ACK'lenir | spec'te bulunamadı (aranan: kabul edilmeyen shard'ın ACK kuralı). noise-wire §8 A-1: "cap aşımı sessiz eviction değil typed diagnostic" (sayaç var) | karar-gerekir |
| C3-12 | TANIMSIZ | düşük | `runtime.rs:5340` (kapsam dışı); test `7775-7784` | `PRE_PARAM_FEC_CAP = 64`, belge "MAX_PREPARAM_SHARDS hizası" der; `kama-engine` `MAX_PREPARAM_SHARDS = 256`; değer spec formülünden türetilmemiş | noise-wire §8 A-1: `RX_PREPARAM_SHARD_CAP_PER_SESSION = min(RX_COLD_TOTAL_SHARD_CAP_PER_SESSION, MAX_PREPARAM_SHARDS)` | karar-gerekir (`RX_COLD_TOTAL_SHARD_CAP_PER_SESSION` değeri bu kapsamda DOĞRULANAMADI) |
| C3-13 | EKSİK | düşük | `wire.rs:45-48`, `:75-82`; `lanes.rs:186-205` | yalın PTT kimlik geri kurulumu pencerenin 2^16'dan küçük olmasına dayanır; `LaneConfig::validate` PTT `admit_widths` üst sınırını denetlemiyor; ≥ 65 536 genişlikte `unwrap_lane_seq` yanlış kimliği sessizce seçer (varsayılan 256 etkilenmez) | ADR-05/§9 açık hata tipleri; yorumdaki varsayım ("pencere (≤ 256 birim)") kodda zorlanmıyor | kilitli (karar 3: PTT paketleme) |
| C3-14 | TANIMSIZ | düşük | `wire.rs:217-229` | `encode_recovered` 16'dan fazla ESI'yi sessizce kırpar (hata/sayaç yok); profil 0x05'te K = 8 olduğundan bugün erişilemez | CLAUDE.md §9 açık hata tipleri; noise-wire `RECOVERED_MAX_ESI_RANGES = 16` | kod-duzeltmesi (kırpılan sayıyı döndür ya da `debug_assert`) |
| C3-15 | ÖLÜ | düşük | `wire.rs:150-165` (`FecFrameHeader.seq`), `:14-39` (`seq u16`) | `seq` alanları yazılır (`runtime.rs:3607`) ama alıcı okumaz; FEC başlığı belgesi "Motor wire-seq (ACK yolu)" der | transport-layer-tdd S4 notu (862–866): "Alan motor seq'inin alt 16 bitini taşımaya devam eder ama alıcı OKUMAZ — kimlik değildir" | kod-duzeltmesi (yalnız belge; bayt düzeni değişmez) |
| C3-16 | EKSİK | düşük | `seam.rs:186-196`; test `runtime.rs:6834-6856` | HubChannel düzlemi yok: `HubDispatch` yalnız sayar ve düşürür; `IdentitySeam::open` her datagramı Session'a atfettiğinden stub'la Hub kolu erişilemez | KM-SEAM-TC-001 §1.1 S: "`HubChannel`'a giden her şey (engine ACK_FRAME, HANDLE_ASSIGN, Plane 1 RPC) ayrı PN uzayı ve ayrı anahtar"; §8 [OPEN] "HubChannel gerçek anahtar/PN (M4; PoC no-op)" | karar-gerekir ([OPEN] kalemi) |
| C3-17 | TANIMSIZ | düşük | `memory_link.rs:48-62`, `:131`, `:143` | test taşıyıcısında `outbound` ve gelen kutuları sınırsız; hub tutamacı ardışık (`wrapping_add(1).max(1)`). Yalnız `test`/`test-util` grafiğinde | CLAUDE.md §9 / SR-AUDIT-001 bounded buffer; ccr INV-HANDLE-1 (PRF türetimli tutamaç — hub'a ait) | kod-duzeltmesi (isteğe bağlı; üretim etkisi yok) |
| C3-18 | EKSİK | düşük | `runtime.rs:6513-6521` (beyan) | gövdesi olmayan kararın datagram üretmemesi kuralının runtime dalı (`drive!` içindeki `let … else`: sayaçlar, seal/PN yokluğu, `continue`) hiçbir testte koşmuyor; yalnız yardımcılar sınanıyor | test içinde atıf: sahip kararı 2026-11-11, #174 madde 2 (metin bu denetimde DOĞRULANAMADI) | kod-duzeltmesi (dalı koşturan dar test) |
| C3-19 | TANIMSIZ | düşük | `runtime.rs:2661` (kapsam dışı); test `6647-6652` | `release_outstanding` 0'da sessizce doyar: çift düşüm hatası gizlenir | CLAUDE.md §9 açık hata tipleri (sessiz geri dönüş) | kod-duzeltmesi (`debug_assert` ya da sayaç) |
| C3-20 | TANIMSIZ | düşük | testler `runtime.rs:8658-8717`; `runtime.rs:654` | dar dönüşte ACK, bekletilen RECOVERED'ın önüne geçer; denetim datagramları dar hatta veriye yol verir | spec'te bulunamadı (aranan: ACK/RECOVERED öncelik sırası). Karar 7 (7171705736) yalnız "bütçesi hazır ACK, henüz gönderime verilmemiş metin alındısının önüne geçebilir" için ayrı PR (K10) onaylar; ACK↔RECOVERED sırası orada yok | karar-gerekir |
| C3-21 | DOĞRULANAMADI | düşük | testler `runtime.rs:8362-8656`; `runtime.rs:5389`, `:5412` | ACK'siz PARAM kopyaları `[200, 400, 800, 1 600]` ms, yarım tik payı | atıf "#175 Y3 iş emri"; bu denetime verilen karar listesinde metin yok. Testler gerçek saatle koşuyor (zaman duyarlı) | karar-gerekir (dayanak metni kayda bağlanmalı) |
| C3-22 | TANIMSIZ | düşük | testler `runtime.rs:8321-8359`; `runtime.rs:4419-4436` | 62 B/s yukarı yönde oturum FEC kurmaz, PARAM hiç çıkmaz (zaman kapısı) | spec'te bulunamadı (aranan: hat hızına göre FEC'i oturum düzeyinde kapatma); fec-core §11 küçük-E profil seçimini önerir | karar-gerekir |

Not (kapsam dışı, bulgu değil): `ClientHandle::set_recovery` (`runtime.rs:2948`) belgede "yalnız test yapılandırması" der ama koşulsuz `pub`; `transport.rs:1652/1935` onu S1 yolunda `recovery_on` ile çağırıyor. Ana gövde denetçisine aktarıldı.

## Okunan otorite bölümleri

- CLAUDE.md §3, §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §5 ADR-05/ADR-17, §8, §9, YAMA KAYDI.
- `docs/decisions/KARAR-KAYDI-2026-11.md` §1–§10 (tamamı; özellikle §4 D-32 (iii), §8 D-CC-1).
- #175 sahip kararları: 7171705736 (dokuz karar; 3 ve 7 bu kapsamda bağlayıcı), 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 (bu dosyalarla doğrudan ilgili madde yok; `receiver.rs` alarm aralığı kapsam dışı).
- `specs/ccr-format-spec.md` 11, 19, 28 (YAMA KAYITLARI), §35.14 (2982–3047), INV-HANDLE-1 (3192), hub_handle tahsisi (2743–2748).
- `docs/seams/KM-SEAM-TC-001.md` 5, 9, 20, §1.1 (36–68), K4/KARAR-1, K5 (80–90), 137, 143–149, §6 (157–163), §7–§8 (167–179).
- `specs/transport-layer-tdd.md` 21, 141, 855–870 (S4 notu, interim `seq` alanı, D2).
- `specs/kama-noise-wire-tdd-v2.0.md` 327, 419–436 (§2.6 MADDE-6, MADDE-14), 650–672 (§8 PARAM, A-1).
- `specs/fec-core-tdd-v3.0.md` §11 (328–343).
- `specs/signaling-seam-v0.md` (tamamı, 70 satır) — kapsamdaki dosyalarda signaling taşıyıcısı yok; `memory_link` zarf taşıyıcısıdır, signaling değil.
- `specs/application-protocol-and-rpc-spec.md` — `HANDLE_ASSIGN`, `cancel_init`, `STORE` araması (81, 271, 467, 688); `EnvelopeLink`/`LinkAttempt` için tanım bulunamadı.
- `specs/application-message-protocol.md` 6, 18, 69, 98 (0x09 `SIGNALING_BLOB` — client-core 0x09 PTT ile ayrı düzlem).
