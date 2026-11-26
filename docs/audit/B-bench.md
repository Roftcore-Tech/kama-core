# B-bench — `crates/bench/src/` (benchmark düzeneği) davranış denetimi

Taban: ürün hattı `125b9afa` (yerel/kod-denetimi). Salt okuma; kod değiştirilmedi, cargo koşulmadı.
Satır numaraları `125b9afa`'dandır.

## 1. Kapsam

| dosya | okunan aralık | satır |
|---|---|---|
| `crates/bench/src/main.rs` | 1–1066 (tamamı) | 1066 |
| `crates/bench/src/budget.rs` | 1–315 (tamamı; testler 212–315) | 315 |
| `crates/bench/src/error.rs` | 1–23 | 23 |
| `crates/bench/src/event.rs` | 1–185 | 185 |
| `crates/bench/src/kama.rs` | 1–1224 (tamamı; testler 742–1224) | 1224 |
| `crates/bench/src/kama_product.rs` | 1–2041 (tamamı; testler 1117–2041) | 2041 |
| `crates/bench/src/manifest.rs` | 1–456 (tamamı; testler 308–456) | 456 |
| `crates/bench/src/metrics.rs` | 1–299 (tamamı; testler 192–299) | 299 |
| `crates/bench/src/mixed.rs` | 1–1014 (tamamı; testler 940–1014) | 1014 |
| `crates/bench/src/outcome.rs` | 1–806 (tamamı; testler 533–806) | 806 |
| `crates/bench/src/qdgram.rs` | 1–736 (tamamı; testler 547–736) | 736 |
| `crates/bench/src/quic.rs` | 1–1109 (tamamı; testler 787–1109) | 1109 |
| `crates/bench/src/rival.rs` | 1–213 | 213 |
| `crates/bench/src/rxlog.rs` | 1–407 | 407 |
| `crates/bench/src/tcp.rs` | 1–671 | 671 |
| `crates/bench/src/transport.rs` | 1–311 | 311 |
| **toplam** | | **10 876** |

Karşılaştırma için ayrıca okunan (kapsam dışı, yalnız sözleşme doğrulaması): `crates/bench/Cargo.toml` (tamamı);
`crates/client-core/src/runtime.rs` 90–380 (`RuntimeConfig`, `KamaV1Profile`), 705–785 (`TransportEngine`,
`Default`, `validate_fec`); `crates/adaptive-transport/src/sender.rs` 346–455 (`FecLossRiskConfig`, `bounded_v1`);
`Cargo.lock` (quinn 0.11.11, quinn-proto 0.11.19); `scripts/ktp2/f0/arms_rakip.sh` 57–76 (grep: kollar `--cc bbr`
veriyor); `scripts/netem/*.sh` (grep: MTU ayarı yok).

## 2. Davranış envanteri

### 2.1 `main.rs` (CLI)

- **Komutlar.** `serve`, `client`, `fairness`, `metrics`, `mixed-serve`, `mixed-client` (41–363).
- **`serve` / `client` bayrakları** (43–330): taşıma (`quinn` varsayılan), CC (`cubic` varsayılan, 46/176), bütçe dörtlüsü,
  gate/warmup, FEC profili (varsayılan `KamaV1Profile::FEC_PROFILE_ID`, 72/217 — türetilmiş), `fec_mode` (varsayılan `adaptive`),
  deney kolları (`loss_confirm_k`, `fec_risk`, `spec_l1/l2`, `gate_permille`, `persist_*`, `rp_suppress`, `fec_burst_fusion`,
  `bootstrap_rtt`, `initial_window_bytes`, `bootstrap_fast`), rakip kol ayarları (`rival_delivery`, `quinn_*`, `handshake_timeout_s`,
  `rival_stats`, `early_exit`, `payload_id`), `dup_copies` (1..=64), `cc_port` (yalnız sunucu), `rx_log`, `manifest`, bozulma dizeleri,
  `setup_class` iddiası.
- **`budget_mode`** (441–476): burst XOR paced; paced için kova zorunlu; tavan 0 → `u64::MAX`; yanlış kombinasyon typed hata.
- **`serve` dalı** (492–586): `--rx-log` desteklenmeyen kolda (qdgram) ret; `rival::set`; `ts_out` sunucuda **yok sayılır** (508, 567).
- **`client` dalı** (587–754): `dup_copies > 1` yalnız `kama-product` ile; `rival::set`; koşu; kurulum sınıfı kolun `setup_in_window`
  alanından türetilir, `--setup-class` yalnız denetlenir (702–710); manifest yazımı (711–750): `arm` = `profile_id` boşsa
  `manifest::arm_name(transport)`; `commit` = derleme ortamı `KAMA_GIT_SHA` ya da `"unknown"`; tcp için tel notu.
- **`fairness`** (755–840): manifestleri okur, şema denetler; `kama-v1-full` etiketi + sapma → INVALID; ikili kıyas `fairness_diff`;
  çıkış kodu 2 (koşul farkı), 1 (anlam farkı), 0.
- **`metrics`** (841–1041): okunamayan koşu `HarnessFailure`/`InfraFailure` olarak sayılır; görünüm (`steady-state`/`cold-flow`) önce
  seçilir; deadline başına completion medyan/IQR/min/max; ikincil: bytes/teslim medyanı, ACK oranı medyanı; tel dökümü; blackout
  taraması; sonuç sınıfları; katmanlı istatistik; iddia sınıfı n'den. **p50/p95/ilk teslim yazdırılmaz.**
- **`run_server`/`run_client`** (1048–1066): taşıma → modül eşlemesi.

### 2.2 `budget.rs`

- `IPV4_UDP_OVERHEAD = 28` (18; IPv4 20 + UDP 8, türetilmiş). `NS_PER_S`.
- `BudgetMode {Unlimited, Burst{cap}, Paced{refill, bucket, ceiling}}` + `label`.
- `BudgetGovernor`: `arm` (idempotent, 129–135), `permit`/`permit_n` (159–194): arm öncesi her şey geçer ve `pre_arm_*`'a sayılır;
  sonra burst tavanı / paced kova+tavan; paket-atomik; bekletmez; ilk ret anı kaydı. Tamsayı byte·ns aritmetiği (RNG yok).
- Testler (216–314): paket-atomik ret, paced dolum/tavan, arm öncesi muafiyet, sınırsız modda sayım, çok-segment ya hep ya hiç.

### 2.3 `error.rs`

`BenchError {Io, Json, Quic, Tls, Other}` (thiserror).

### 2.4 `event.rs`

- `RawEvent {msg_id, sent_ns, delivered_ns: Option, payload_bytes}`; `TxComponents {data, retx, dup, fec, ctrl}`.
- `RunMeta` (41–116): profil, cc, transport, `quinn_version`, seed, repeat, sayımlar, udp tx/rx, bütçe fotoğrafı, t0 damgaları,
  warmup artığı, aktivasyon, `setup_ns`/`setup_in_window`, `profile_id`/`profile_delta`, `engine` (boşsa serileşmez).
- `write_jsonl` (149–163): tmp + rename ile atomik. `read_jsonl` (169–184): meta satırı zorunlu.

### 2.5 `kama.rs` (security-stub kolu, motoru doğrudan sürer)

- Sabitler: `FRAME_DATA = 0x00`, `HDR_DATA = 11` (1+2+8, türetilmiş), `TAG_STUB = 16` (AEAD tag taklidi), `PACE_TICK = 1 ms`,
  `RTPROP_TICK = 200 ms`, `ACK_TICK = 20 ms` (§2.1 ACK_TIMER_T atfı), `SERVER_RECV_BUF = 65536`, `CLIENT_RECV_BUF = 2048`,
  `WARMUP_BASE = 2^40`.
- `encode_data`/`decode_data` (60–78): sabit genişlik big-endian; 16 B sıfır dolgu.
- `Engine` (88–133): etiket kurulmuş `Sender::cc_on()`'dan; sunucuda `--cc-port` → `Receiver::with_cc_port()`.
- `ClientRun::new` (159–187): `--kama-arm cc-port` → `Sender::with_cc_port()`, aksi `Sender::new()` (eski motor); `tail-v1` →
  `validated_v1`; `KAMA_BENCH_TRACE` ortam değişkeni izi açar.
- `handle_datagram` (192–258): V1/V2 ACK kabul, sondaki 16 B soyulur; ilk-ACK-kazanır teslim damgası; warmup kimlikleri ayrı kümede;
  motora `AckReceived`.
- `drive` (262–304): motor kararlarını tele yazar; tx sayaçları **deneme düzlemi** (governor reddi dahil); governor reddi = enjeksiyon yok.
- `client_loop` (310–605): kuruluş gate'i → warmup (sunum planı + 30 s tavan) → en çok 3 s boşaltma → ölçüm gate'i → t0 (sayaçlar
  sıfır, motor durumu korunur, bütçe arm) → burst ya da paced sunum; `sent_ns` = sunum anı; 1 ms PTO pompası, 200 ms rtprop, her 500 ms
  koşulsuz `TRAJ` stderr satırı; deadline = son sunum + `max_deadline`. `RunMeta.cc` = CLI CC etiketi (bu kolda etkisiz).
- `serve_loop` (614–709): eş başına `Receiver` (sınırsız `HashMap`); 20 ms ACK zamanlayıcısı; reverse governor (gate-watch ile arm);
  `BUDGET-REV` (her 500 tur) ve `RTRAJ` (her 25 tur) satırları; `rx_log` satırı ACK gönderiminden sonra.
- Testler (742–1224): kol bayrağının Sender'a işlenmesi, paket-atomik bütçe, warmup dışlama, paced sunum planı, çerçeve gidiş-dönüş,
  loopback çerçeve değişmezleri, ölü adreste sınırlı süre, `sent_ns` sunum anında, motor etiketi, alıcı kurucusu, rx-log biçimi ve dup yolları.

### 2.6 `kama_product.rs` (ürün runtime kolu)

- Sabitler: `GATE_POLL = 20 ms`, `OFFER_RETRY = 1 ms`, `WARMUP_CAP_EXTRA = 30 s`.
- `BudgetEgress` (63–79): governor'ü `EgressPolicy`'ye takar; zehirli kilit → ret (fail-closed).
- `runtime_cfg` (115–258): `KamaV1Profile::runtime_config()`'ten başlar; birim tavanı = profilin sembol kapasitesi; payload > birim →
  fragmenter; motor `KAMA_TRANSPORT_ENGINE` ortam değişkeninden (tanınmayan değer → **panik**, 156–162; boş dize → eski motor);
  `drain_cap = 0`; IW 0 → ürün değeri; `fec_risk` → `experimental_v1` aksi `bounded_v1`; `persist_mode` "learned" dışındaki her
  dize → `FixedSrtt` (225–231); `gate` yoksa ürün değeri 250; politika kolu `calibration_v2_deadline`.
- `profile_delta` (267–346): `max_unit_bytes`, `max_message_bytes`, IW, FEC profil/r, gate seviyesi, risk kolu (enabled, l1, l2,
  rp_suppress, persist_mode), bootstrap_rtt, politika varlığı, yapı politikası, confirm ufku. **Karşılaştırılmayan:** `engine`,
  `persist_srtt/rho/q_permille`, `quota_r`, `drain_cap`.
- `print_manifest` (349–401): `KP-MANIFEST` satırı; "arq=ON tail=ON establish_probe=ON" sabit metin; motor yazılmaz.
- `print_activation` (404–478): koşu sonrası ateşleme sayaçları.
- `canonical_cfg_for_mixed` (486–518): bayraksız profil (quota 4).
- `offer` (533–552): `QueueFull` → 1 ms bekle-yeniden dene; diğer hata koşuyu düşürür.
- `server_loop` (585–698): SIGTERM/SIGINT ile çıkış, rx kaydı, `BUDGET-REV`/`KP-SERVER` satırları.
- `client_loop` (702–1115): kurulum süresi ayrı; warmup (`base_id = warmup_msgs` varsayımı); t0 işareti; `dup_copies` kadar ayrı
  birim; teslim = ilk kopyanın teyidi; `KP-CLIENT*` satırları; `ts_out` CSV; `RunMeta.engine` boş, `kama_arm` etiketi motor ekini
  ayrıca ortam değişkeninden okur (1051–1055); `RunMeta.cc` = CLI CC.
- Testler (1117–2041): bayraksız yolun ürün profili olması, tek alan sapmasının görünmesi, statik r=0 kontrol kolu, override akışı,
  ts örnekleme, drenaj kapalı, fragmenter, 256 B uçtan uca FEC, dup kopyalar, PARAM bağlanması, tam kol, sıfır bütçe.

### 2.7 `manifest.rs`

- `MANIFEST_SCHEMA = "kama-bench-manifest/v1"`; `Semantics`, `Impairment`, `ActivationSummary`, `RunManifest` (63–90: **cc, quinn
  sürümü, motor, bütçe modu, warmup, seed→var, profil adı yok**).
- `fairness_diff` (143–224): anlam farkları (reliability, ordering, delivery_definition, retry_owner, tel notu) → beyan; koşul
  farkları (setup_class, payload, msg_count, deadline, submit_interval, ileri/dönüş bozulma) → engel.
- `write` (228–237): hata yalnız stderr. `arm_name` (243–254): `kama` → `kama-stub-legacy` (motor bakılmaz).
  `semantics_for` (258–306).

### 2.8 `metrics.rs`

`completion` (44–54: gecikme ≤ deadline oranı), `run_metrics` (58–93: deadline başına completion, ilk teslim, p50, p95; bytes/teslim
(0 teslimde NaN), ACK oranı), `aggregate`, `aggregate_secondary`, nearest-rank yüzdelik. Testler deadline sınırı, yüzdelik, varyans, eğri.

### 2.9 `mixed.rs` (karışık akış)

- Sabitler: `HDR = 9`, `LEN = 4`, `MAX_ITEM = 1 MiB`, `OFFER_RETRY = 1 ms`, şema `kama-bench-mixed/v1`.
- İş yükü varsayılanları: TEXT 65 B × 15 / 2 s; PTT 39 B × 150 / 200 ms; dosya 0; parça 1024 B; kuyruk 20 s; el sıkışma 60 s.
- `plan` (258–308): boy doğrulaması, aynı anda öncelik PTT > TEXT > dosya. `encode`/`framed`/`decode`.
- Sunucular: kama-product (`canonical_cfg_for_mixed`, egress politikası yok), quinn (uni-stream, öncelik), tcp (tek bağlantı);
  `RxLog` satır satır.
- İstemciler: kama-product tek sunum kuyruğu; quinn akış başına stream (isteğe bağlı öğe başına); tcp öncelikli tek yazıcı (üç
  **sınırsız** mpsc kanalı, 878–880). `sent_unix_ns` = plan anı.

### 2.10 `outcome.rs`

`MIN_FINAL_CLAIM_N = 30`, `DEFAULT_CI_PERMILLE = 950`, `DEFAULT_BLACKOUT_WINDOW = 1 s`, `DEFAULT_MIN_DELIVERIES_PER_WINDOW = 1`.
`Outcome` yedi sınıf + `classify` öncelik sırası; `blackout_scan` (pencere başına teslim < eşik ∧ bekleyen iş → blackout; ufuk
son teslim+1 ns); `SetupView` ve `effective_deadline`; `derived_setup_class`/`check_setup_claim`; `ClaimClass`; `summarize`,
sıra-istatistiği medyan GA, binom CDF, Clopper–Pearson, `stratify`. RNG yok.

### 2.11 `qdgram.rs` (quinn DATAGRAM + uygulama ARQ)

Sabitler: `DG_DATA = 0x01`, `DG_ACK = 0x11`, `ACK_FREQ_N = 2`, `ACK_TICK = 20 ms`, `RTO 100 ms–10 s`, `WARMUP_BASE = 2^40`.
Sunucu kümülatif aralık ACK'i (her 2 yeni DATA'da + 20 ms'de kirliyse + 2 s sessizlikte `quota_r` kadar); `to_ranges` en düşük
kimlikten başlayıp `max_ranges`'te keser (95–109). İstemci mesaj başı RTO = clamp(2×rtt) × 2^deneme, mesajın kendi deadline'ına
dek; el sıkışma `max_deadline` ile sınırlı; sayaçlar t0 farkı.

### 2.12 `quic.rs` (quinn akış kolu)

`GovernedUdpSocket` (paket-atomik governor, GSO kapalı); `build_endpoint`; `QUINN_VERSION = "0.11.11"` (sabit dize); `transport_config`:
idle 300 s, MTU keşfi kapalı, `initial_mtu(1452)`, GSO kapalı, 512 bi-stream (tack'te 512 uni); isteğe bağlı initial_rtt / keep-alive /
IW; Cubic ya da BBR. `run_client`: el sıkışma `handshake_timeout` ya da `max_deadline`; başarısızlık 0-completion; warmup 10 s/mesaj
+ reset; mesaj başı bi-stream (1 B uygulama yanıtı) ya da uni-stream + `stopped()`; gözlem penceresi = `max_deadline` + kalan sunum.
`expect("governor lock")` üç yerde (46, 50, 88).

### 2.13 `rival.rs`

`Delivery {AppReply, TransportAck}`; `LEGACY_TCP_CONNECT_TIMEOUT = 20 s`; `RivalTuning` + `from_cli`, `quinn_delta`, `tcp_delta`,
`quinn_label`; süreç geneli `OnceLock`.

### 2.14 `rxlog.rs`

`RX_LOG_SCHEMA`, `RX_LOG_CAP = 2^20`; `RxRecorder` (sınırlı bellek, çıkışta atomik yazım); `write_on_signal`; `RxStream` (satır satır,
tamponsuz; tavanda bir kez `cap_reached` satırı, `"dropped":1` sabit).

### 2.15 `tcp.rs`

Çerçeve `u32 len ‖ u64 msg_id ‖ payload` (12 B), ACK 8 B. Sunucu: çerçeve ≤ 1 MiB; tack'te ACK yok. İstemci (app-reply): kurulum ayrı;
`sent_ns` yazmadan önce; tüm yazımdan sonra **`min(max_deadline, 60 s)`** bekleme (189), isteğe bağlı erken çıkış. Tack: plan anı,
`/proc/self/net/tcp` `tx_queue` 5 ms örnekleme, pencere = son plan + `max_deadline`. Bütçe, warmup, gate, establish_gate
**kullanılmaz**; `t0_unix_ns = 0`; tel sayaçları uygulama baytı.

### 2.16 `transport.rs`

`Cc`, `KamaArm`, `FecModeArg` (Default `Static`), `FecArm` (Default `Off`) + `from_args` (profil 0 → Off; mod ortogonal) + `label`;
`ClientCfg`, `ServerCfg`; `gate_sync` (`.ready` yaz, `.go` için 5 ms × 24 000 = 120 s, sonra typed hata); `BenchTransport` trait.

## 3. Bulgular

Uyumlu davranışlar (tek tek yazılmadı):
- `budget.rs`: **6 uyumlu.** Paket-atomik, bekletmeyen, yön başına governor; tamsayı/RNG'siz aritmetik (ADR-17); arm öncesi muafiyet ayrı sayılır.
- `kama.rs`: **9 uyumlu.** Sabit genişlik BE çerçeve, varint yok (ADR-05); motor kripto/anahtar tutmaz, stub parity üretmez (INV-ENGINE-SOC,
  INV-FEC-1 yapısal); `sent_ns` sunum anı, ilk-ACK-kazanır; deadline'la sınırlı koşu (§4.4.3 ①); warmup ölçüm dışı; motor etiketi kurulmuş Sender'dan.
- `kama_product.rs`: **10 uyumlu.** Ürün profilinden başlayıp açık override (tek doğruluk kaynağı); `IdentitySeam` ile kripto ayrı (INV-ENGINE-SOC);
  egress zehirli kilitte fail-closed; `offer` hatası yutulmaz; kurulum ayrı ölçülür; aktivasyon özeti; dup kopya eşlemesi; politika kolu fail-safe.
- `main.rs`: **9 uyumlu.** Bütçe kombinasyonu typed red; desteklenmeyen kolda `--rx-log`/`--dup-copies` reddi; kurulum sınıfı koddan türer;
  okunamayan koşu sessizce silinmez; iddia sınıfı n'den (§4.4.9 N ≥ 30); TCP tel görünürlüğü beyan.
- `outcome.rs`: **8 uyumlu.** N ≥ 30 nihai iddia (§4.4.9); RNG'siz GA (ADR-17); el sıkışma başarısızlığı ölçüm sonucu (§4.4.3 ④); harness/altyapı
  arızası ayrı; Clopper–Pearson; sınıf önceliği sabit; kurulum görünümü simetrik.
- `quic.rs`: **7 uyumlu.** BBR seçilebilir; akış ve datagram ayrı kollar (§4.4.1); el sıkışma sınırlı ve 0-completion olarak agregaya girer;
  idle 300 s (oturumu yapay öldürmez); GSO kapalı paket-atomik sayım; t0 farkı.
- `qdgram.rs` 5, `metrics.rs` 4 (deadline-bounded completion §4.4.3 ①), `manifest.rs` 5, `mixed.rs` 6 (bounds-before-alloc), `rxlog.rs` 5
  (sınırlı kayıt, SR-AUDIT-001), `tcp.rs` 5, `rival.rs` 3, `transport.rs` 3 (`gate_sync` sınırlı typed hata), `event.rs` 4, `error.rs` 1.

| fid | tür | önem | konum | davranış | otorite | düzeltme türü | düzeltme taslağı |
|---|---|---|---|---|---|---|---|
| B-01 | UYUSMAZ | orta | kama_product.rs:156-162, 267-346, 349-401, 1112 | `profile_delta` motoru (`RuntimeConfig::engine`) karşılaştırmaz. `KAMA_TRANSPORT_ENGINE=legacy` ile koşan ürün kolu boş sapma listesiyle `kama-v1-full` kimliğini alır; `RunMeta.engine` bu kolda boş yazılır, `KP-MANIFEST` motoru göstermez. Motor yalnız `kama_arm` dizesinin ekinden (ek yoksa eski motor) çıkarılabilir; manifest bu alanı taşımaz. | CLAUDE.md YAMA KAYDI 2026-11-11: "Ürün varsayılanı port (`RuntimeConfig::engine`), geri alma `KAMA_TRANSPORT_ENGINE=legacy`"; runtime.rs:300-302 "Yalnız `profile_delta` BOŞ olan koşu bu adı taşıyabilir; herhangi bir sapma `kama-custom`tur" | kod-duzeltmesi | `profile_delta`'ya `c.engine != v.engine` satırı; `RunMeta.engine = c.engine` etiketi; `KP-MANIFEST`'e `engine=`. |
| B-02 | UYUSMAZ | orta | kama_product.rs:232-240, 302-317, 713 | `--persist-alpha/--persist-rho/--persist-q` risk kolunu değiştirir ama `profile_delta` yalnız `persist_mode`'u karşılaştırır: bu bayraklarla koşu `kama-v1-full` kalır. İstemci `quota_r` sabit 4, sunucu `--quota-r`'den; fark sapma listesine girmez. | runtime.rs:300-302 (aynı kural); `bounded_v1` alanları sender.rs:435-447 | kod-duzeltmesi | `persist_srtt/rho/q_permille` ve `quota_r` alanlarını `profile_delta`'ya ekle (ya da `FecLossRiskConfig`'in tüm alanlarını karşılaştır). |
| B-03 | UYUSMAZ | dusuk | kama_product.rs:156-162 | Tanınmayan `KAMA_TRANSPORT_ENGINE` değeri `expect` ile süreç paniğidir (erişilebilir panik). Boş dize sessizce eski motoru seçer (`TransportEngine::parse("")`). | CLAUDE.md §6 "Yarım kod → TYPED-ERROR"; §9 "açık hata tipleri" | kod-duzeltmesi | `runtime_cfg` `Result<RuntimeConfig, BenchError>` dönsün; boş değer de açık hata ya da açık "legacy" beyanı istesin. |
| B-04 | UYUSMAZ | orta | quic.rs:171-177 | quinn kolu MTU keşfi kapalı, `initial_mtu(1452)` ile koşar: QUIC paketleri 1452 B UDP yüküne kadar çıkar. Kama ürün kolunun en büyük mühürlü datagramı ≈ 1061 B. Düzenek betiklerinde bağlantı MTU'su ayarlanmıyor (grep). İki kol aynı MTU altında ölçülmüyor. | transport-tdd §4.4.1: "Aynı netem/tc script, aynı MTU=1200 … Confound YOK"; §11 "default path_mtu_target = 1200B" | kod-duzeltmesi | `initial_mtu(1200)` (MTU keşfi kapalı kalır). Yorumdaki ön-kayıt pini (1452) spec'in altında kalır; pin manifestte değiştirilmeli. |
| B-05 | UYUSMAZ | orta | main.rs:46-47, 176-177; quic.rs:1-2; transport.rs:10-17; manifest.rs:63-90 | Bayraksız quinn kolu Cubic koşar; belge "§4.4.1: Cubic default vs Bbr experimental" der. Manifest CC'yi taşımaz, `fairness` farkı yakalayamaz: Cubic'li bir quinn koşusu BBR'lı koşuyla "NORMALIZED" görünür. (Mevcut betikler `--cc bbr` veriyor; risk bayraksız koşuda.) | transport-tdd §4.4.1: "En güçlü mevcut congestion control … BBR/BBRv2 (Cubic/Reno'ya karşı kazanmak anlamsız)"; AC-QBENCH-1; ADR-18 / CLAUDE.md §7 "7 profil vs quinn+BBR" | kod-duzeltmesi | `RunManifest`'e `cc` alanı ve `fairness_diff`'te rakip kollar için CC farkı engel; varsayılan `--cc` `bbr` olsun ya da quinn kolunda zorunlu; `transport.rs` belgesi düzeltilsin. |
| B-06 | UYUSMAZ | dusuk | quic.rs:159, 202; Cargo.toml:15-16; transport.rs:40 | Raporlanan sürüm `QUINN_VERSION = "0.11.11"` elle yazılmış dize; CC ve kayıp kodu `quinn-proto`'dadır ve Cargo.lock'ta 0.11.19'dur. Yorumlar 0.11.18 der. Rakibin CC sürümü hiçbir çıktıya girmez. | transport-tdd §4.4.1: "versiyon PİNLİ + Cargo.lock … Kullanılan QUIC impl + CC profile RAPORDA açıkça yazılır"; §4.4.9 | kod-duzeltmesi | Sürümleri derlemede Cargo.lock'tan (build script / `env!`) türet; `RunMeta`'ya `quinn_proto_version` ekle; yorumları düzelt. |
| B-07 | EKSIK | orta | manifest.rs:63-90, 143-224 | `RunManifest` ve `fairness_diff` bütçe modunu, warmup sayısını, gate kullanımını, CC'yi, rakip sürümünü, motoru ve netem profil adını taşımaz/karşılaştırmaz. Farklı bütçe modlu iki koşu "NORMALIZED" çıkar. `commit = "unknown"` engellemez. | transport-tdd §4.4.1: "tek değişken = transport … Confound YOK"; §4.4.9 "versioned harness (git commit)" | kod-duzeltmesi | Alanları manifeste ekle; bütçe/warmup/gate/CC farkı `blocking`, `commit == "unknown"` en az `non_normalized`. Şema sürümünü `v2` yap. |
| B-08 | UYUSMAZ | orta | tcp.rs:188-206 | TCP `app-reply` kolu, tüm yazımlar bittikten sonra `min(max_deadline, 60 s)` bekler. `max_deadline` 120 s iken 60–120 s arası teslimler kaydedilmez; diğer kollar son sunum + `max_deadline` boyunca gözler. completion(120 s) TCP için eksik ölçülür. 60 s türetilmemiş sabit. | transport-tdd §4.4.1: "aynı ölçüm penceresi"; §4.4.3 ① deadline-bounded completion | kod-duzeltmesi | Gözlem penceresini `t0 + son sunum + max_deadline` yap (tack kolu 411. satırda zaten böyle). |
| B-09 | TANIMSIZ | orta | tcp.rs:112-243, 351-473; main.rs:587-696 | TCP kolu `--budget-*`, `--warmup-msgs`, `--gate`, `--establish-gate` bayraklarını sessizce yok sayar: bütçe uygulanmaz (manifestte mod boş), `<gate>.ready` yazılmaz (düzenek kendi zaman aşımına dek bekler). `dup_copies` için var olan "sessizce yok sayılmaz" kuralı bu bayraklara uygulanmıyor. | main.rs:640 kendi kuralı; CLAUDE.md §9 açık hata; spec'te bulunamadı (aranan: TCP ek baseline'ında bütçe/warmup semantiği, §4.4.1 SECONDARY) | kod-duzeltmesi | `transport == Tcp` iken bu bayraklar verilirse typed hata (ya da gate için `gate_sync` çağrısı ekle). |
| B-10 | UYUSMAZ | orta | qdgram.rs:92-109, 199-202, 223-244 | Sunucunun kümülatif ACK'i aralık sayısı `max_ranges`'i aşınca EN DÜŞÜK kimlikli aralıkları tutar, yenileri keser. Deadline'ı geçmiş (istemcinin artık yeniden göndermediği) mesajların bıraktığı delikler kalıcıdır; yeterince delik birikince yeni mesajlar hiç ACK'lenmez, rakip kolun completion'ı yapay düşer. Belgedeki "kırpma kendini onarır" iddiası bu durumda tutmaz. Kama alıcısı taşmada en yeni blokları öne alır. | transport-tdd §4.4.1 "best-available; sakat kurulmaz" ("strawman"); runtime.rs:231-233 "donmuş spec §2.1 'taşarsa en yeni bloklar öncelik'"; qdgram.rs:12 "Kama SACK'inin app-düzeyi eşleniği" | kod-duzeltmesi | `to_ranges` en yüksek kimlikten geriye doğru `max_ranges` aralık toplasın (Kama ile aynı öncelik). Bu kusurun geçmiş koşulara etkisi DOĞRULANAMADI (ham veri okunmadı). |
| B-11 | EKSIK | orta | metrics.rs:58-93; main.rs:841-1041 | `run_metrics` ilk teslim, p50, p95'i hesaplar ama `metrics` komutu hiçbirini yazdırmaz (alanlar yalnız testte okunur); p99 hiç hesaplanmaz. | transport-tdd §4.4.3 SECONDARY-BUT-REQUIRED: "time-to-first-delivery; deadline-altı p50/p95 delivery latency … ZORUNLU"; §4.4.9 "median + p95/p99 + %95 güven aralığı" | kod-duzeltmesi | Deadline başına koşular arası ilk teslim / p50 / p95 / p99 medyanı ve GA'sı yazdırılsın; `percentile_ns(.., 99)` eklensin. |
| B-12 | EKSIK | orta | crates/bench/src (tamamı) | Mekanizma metrikleri yok: netem ground-truth'a karşı bant/RTT tahmin hatası, kayıp↔tıkanıklık karışıklık matrisi, FEC verimi (parite baytı / kurtarılan yük, net goodput), kayıp→yeniden kurma süresi, CPU/bellek. `ActivationSummary` btlbw/srtt taşır ama karşılaştırılacak gerçek değer yok (bozulma serbest dizedir). | transport-tdd §4.4.4 (estimator accuracy, confusion matrix, FEC efficiency, recovery time); §4.4.6; §4.4.10 [G1] "GO'nun ön-şartı"; AC-QBENCH-4/5 | karar-gerekir | Bu metriklerin bench'te mi, düzenek betiklerinde mi üretileceği ve ground-truth'un makine-okunur biçimi (yapılandırılmış bozulma alanları) karara bağlanmalı. Betiklerde var olup olmadığı DOĞRULANAMADI (kapsam dışı). |
| B-13 | EKSIK | orta | outcome.rs:126-252; main.rs:1002-1017 | §4.4.3 ② blackout survival (N sn tam kesinti sonrası oturum yaşıyor mu) ölçülmüyor. `blackout_scan` teslimin pencere içinde durmasını (stall) ölçer ve "# blackout (§8): true/false" yazar; bu çıktı oturum sağkalımı diye okunabilir. | transport-tdd §4.4.3 ② "BLACKOUT SURVIVAL (ikili) — birincil"; §4.4.10 Tier-2; CLAUDE.md §10 BENCH-01 (ertelenmiş iş) | karar-gerekir | BENCH-01 kapsamında ikili metrik tanımı ve sahibi; o zamana dek çıktı etiketi "stall (teslim durması)" olsun, "survival" ile karışmasın. |
| B-14 | TANIMSIZ | dusuk | outcome.rs:1-30, 130-139, 255-264; main.rs:337-357, 918-1040 | "§7/§8/§12/§13/§15/§16/§17/§M4" atıfları donmuş spec bölümleri değil (transport-tdd §8 = Sıkıştırma, §12 = ACK stratejisi, §13 = Layer accounting); bir FAZ-4 denetim raporuna aittir. Blackout penceresi 1 s ve asgari 1 teslim türetilmemiş. | spec'te bulunamadı (aranan: transport-tdd §7/§8/§12/§13'te blackout tanımı, sonuç sınıfları, istatistik politikası) | kod-duzeltmesi | Atıflar `§4.4.x` ya da rapor adıyla yazılsın; pencere/eşik varsayılanı bir karar kaydına bağlansın ya da çıktıda beyan edilsin (şimdi yazdırılıyor). |
| B-15 | TANIMSIZ | dusuk | kama_product.rs:225-231; main.rs:110-112, 255-257 | `--persist-mode` serbest dize: "learned" dışındaki her değer (yazım hatası dahil) sessizce `FixedSrtt` olur. | CLAUDE.md §9 açık hata | kod-duzeltmesi | `ValueEnum` (`fixed`/`learned`); tanınmayan değer CLI'da reddedilsin. |
| B-16 | TANIMSIZ | dusuk | kama_product.rs:363-368 | `KP-MANIFEST` satırı "arq=ON(selective+dup) tail=ON(validated_v1) establish_probe=ON" ifadesini yapılandırmadan okumadan sabit yazar; motor alanı yok. | kama_product.rs:260-264 kendi amacı ("NE koştuğunu … kendisi beyan eder") | kod-duzeltmesi | Bu alanlar `RuntimeConfig`'ten türetilsin, `engine=` eklensin. |
| B-17 | UYUSMAZ | dusuk | manifest.rs:251; main.rs:717-721; kama.rs:162-165; kama_product.rs:1051-1055 | Stub `kama` kolu `--kama-arm cc-port` ile port motorunu koşsa da manifest `arm` alanı `kama-stub-legacy` yazar (`arm_name` motora bakmaz). Motor seçimi kola göre iki ayrı mekanizmadır: stub bayrak (`--kama-arm`, `--cc-port`), ürün kolu ortam değişkeni; her biri diğerinin anahtarını sessizce yok sayar. | CLAUDE.md YAMA KAYDI 2026-11-11 (ürün varsayılanı port); manifest.rs:239-241 "belirsiz etiket YASAK" | kod-duzeltmesi | `arm_name` `RunMeta.engine`'i okusun (`kama-stub-<engine>`); ürün kolunda `--cc-port`/`--kama-arm cc-port` verilirse hata ya da tek anahtar. |
| B-18 | TANIMSIZ | dusuk | main.rs:97-100, 508, 567 | `serve --ts-out` kabul edilir ve yok sayılır. | main.rs:640 "sessizce yok sayılmaz" kuralı | kod-duzeltmesi | Sunucu komutundan bayrağı kaldır ya da verilince hata dön. |
| B-19 | TANIMSIZ | dusuk | kama.rs:570; kama_product.rs:1025 | Kama kolları `RunMeta.cc`'ye quinn CC bayrağının etiketini ("cubic" varsayılan) yazar; bu kollarda etkisizdir. Kayıtlar Kama'nın Cubic koştuğu izlenimi verir. | event.rs:44 "Congestion control" alanı tanımı | kod-duzeltmesi | Kama kollarında `cc` = motor etiketi (`legacy`/`cc-port`) ya da `"n/a"`. |
| B-20 | UYUSMAZ | dusuk | kama.rs:141, 271, 623, 685; mixed.rs:878-880; quic.rs:589-631; tcp.rs:66-108; mixed.rs:447-504 | Sınırsız büyüyen yapılar: stub sunucunun kaynak adresi başına `Receiver` haritası, istemcinin her emisyonda büyüyen `seq_to_unit` haritası, karışık tcp istemcisinin üç `unbounded_channel`'ı, quinn/tcp sunucularının bağlantı/stream başına sınırsız görev açması. Koşu süresiyle sınırlı; ölçüm aracı. | CLAUDE.md §9 "bounded buffer (SR-AUDIT-001: unbounded alloc yok)" | kod-duzeltmesi | Eş tablosuna tavan (ör. 64, ürün `max_peers` hizası), `seq_to_unit`'i ACK'lenen seq'lerde budama, sınırlı kanal (plan uzunluğu kadar), görev sayısına tavan. |
| B-21 | UYUSMAZ | dusuk | quic.rs:46, 50, 88 | Governor kilidi `expect("governor lock")` ile alınır; zehirli kilit `try_send` içinde paniğe döner. Ürün kolu aynı durumda fail-closed reddeder. | CLAUDE.md §6/§9 (typed error, erişilebilir panik yok) | kod-duzeltmesi | `lock().map_or(false, …)` ile ret (kama_product.rs:71 deseni); `snapshot` için varsayılan fotoğraf. |
| B-22 | TANIMSIZ | dusuk | manifest.rs:227-237; main.rs:711-751 | Manifest yazım hatası yalnız stderr'e düşer, koşu 0 ile biter; `out.jsonl` manifestten sonra yazıldığından ikisinden biri eksik kalabilir. | manifest.rs:227 "Hata sessizce yutulmaz"; §4.4.9 tekrar-üretilebilirlik | kod-duzeltmesi | `write` `Result` dönsün, `client` komutu hatayı yaysın. |
| B-23 | UYUSMAZ | dusuk | kama_product.rs:774-798, 821 | Ölçüm birimlerinin kimlik tabanı `base_id = warmup_msgs` varsayılır. Warmup döngüsü 30 s tavanıyla (`WARMUP_CAP_EXTRA`) tüm warmup sunumları yapılmadan (`QueueFull` beklemesi) kırılırsa ilk ölçüm birimi `wnext < warmup_msgs` kimliğini alır; teslim eşlemesi kayar. | kama_product.rs:528-532 kendi sözleşmesi: "sessizce atlanan bir offer tüm eşlemeyi kaydırırdı" | kod-duzeltmesi | `base_id = wnext` (fiilen sunulan warmup sayısı) ya da warmup tamamlanmadan ölçüme geçilirse typed hata. |
| B-24 | OLU | dusuk | transport.rs:1-2, 10-17, 35-36, 63-67, 79-81, 87; quic.rs:1-6; event.rs:46-47; outcome.rs:25-30, 148-155; qdgram.rs:704-735 | Bayat belge ve yalnız testte kullanılan öğeler: "Kama … şimdi typed-error stub" (uygulanmış); `FecArm::Off` "Ürün sevk default'u" (ürün FEC 0x05 adaptif); `StaticR.r` "1..=r_code_max" (r=0 geçerli); `KamaArm::Arq` "Ürün defaults'ları (tail OFF)" (ürün tail açık, port motoru); `FecModeArg` Default `Static` (CLI `adaptive`); `transport` alanı "quinn \| kama"; `BlackoutSpec::default` / `DEFAULT_*` yalnız testte; `app_arq_retransmits_and_recovers` testi adının söylediğini sınamıyor (yorumu kabul ediyor). | CLAUDE.md §6 (deprecated içerik açıkça işaretlenir); runtime.rs:366-369 | kod-duzeltmesi | Yorumlar düzeltilsin; test yeniden adlandırılsın; kullanılmayan `Default` türetmeleri kaldırılsın ya da CLI varsayılanıyla eşlensin. |
| B-25 | TANIMSIZ | dusuk | kama.rs:154, 185, 485-537, 669-680 | `KAMA_BENCH_TRACE` ortam anahtarı (belge "=1", kod herhangi bir değerle, "0" dahil, açar); `TRAJ` (500 ms) ve `RTRAJ` satırları koşulsuz stderr'e yazılır ("kaldırılabilir" teşhis). | spec'te bulunamadı (aranan: bench teşhis anahtarları); CLAUDE.md §9 | kod-duzeltmesi | Anahtar bir CLI bayrağına taşınsın ya da değer denetlensin; `TRAJ`/`RTRAJ` bir bayrağa bağlansın. |
| B-26 | TANIMSIZ | dusuk | tcp.rs:276; rxlog.rs:244-262 | TCP kollarında `t0_unix_ns = 0`: `--rx-log` tcp sunucusunda destekli ama alıcı damgaları istemci saatine hizalanamaz (quinn ve kama kolları yazar). `RxStream` tavan satırı gerçek düşen sayısı yerine sabit `"dropped":1` yazar. | rxlog.rs:168-170 (ortak saat amacı); spec'te bulunamadı (aranan: rx-log şeması) | kod-duzeltmesi | TCP istemcisinde t0'da `unix_ns()` kaydı; `RxStream` kapanışta ya da periyodik gerçek `dropped` sayısını yazsın. |
| B-27 | DOGRULANAMADI | dusuk | kama.rs:35-41, 60-68; kama_product.rs:17-20 | Stub tel modeli DATA başlığı 11 B + 16 B tag taklidi = 27 B; ürün kolu `IdentitySeam` +16 B. "byte ölçümleri gerçek katmanla DEĞİŞMEZ" iddiası spec'in katman muhasebesiyle (SESSION_DATA UDP yükü ek yükü 22 B) eşleştirilemedi. | transport-tdd §13 Layer Accounting; §4.4.3 bytes-per-delivered-message | karar-gerekir | Stub ve `IdentitySeam` ek yükünün §13 / ccr §35 ile bayt bayt eşlemesi yazılsın; fark varsa bayt kıyaslarında beyan edilsin. |
| B-28 | DOGRULANAMADI | dusuk | mixed.rs:214-222, 205-213 | Karışık akış iş yükü varsayılanları PTT 39 B / 200 ms ve TEXT 65 B / 2 s. Bu düzeneğin PoC ürün testi olarak kullanılıp kullanılmadığı doğrulanamadı; kullanılıyorsa 300 ms pilot açıkça seçilmiyor. | Sahip kararı 9 (#175 7171705736): "PoC testi 300 ms pilotu açıkça seçsin"; karar 3: "Transport öncesi ve sonrası aynı PTT biçimi ve metin yüküyle ölçülsün" | kilitli | Karar 3 gereği PTT/metin yükü değiştirilmez. Kullanım bağlamı sahipçe netleşirse yalnız `mixed_meta`'ya pilot etiketi yazılabilir. |

## 4. Okunan otorite bölümleri

- `CLAUDE.md` §3, §4 (INV-RATE-1 2026-11-11 metni + 2026-11-16 profil istisnası, INV-FEC-1, INV-ENGINE-SOC), §5 (ADR-05, ADR-14, ADR-17,
  ADR-18), §6, §7, §8, §9, §10 (BENCH-01), YAMA KAYDI.
- `specs/tech-stack-adr.md` ADR-18 (274–290: yedi asgari profil, quinn yan yana, nightly).
- `specs/transport-layer-tdd.md` §4.4.0–§4.4.12 (281–592; özellikle §4.4.1 adil kurulum ve MTU=1200, §4.4.3 birincil/ikincil metrikler,
  §4.4.4 mekanizma metrikleri, §4.4.6, §4.4.7, §4.4.9 istatistik, §4.4.10 GO/NO-GO), §11 MTU (837–841), §13 Layer Accounting (966–981),
  §16 AC-QBENCH-1..9 (1042–1050), bölüm başlıkları (§7/§8/§12/§13 atıf denetimi için).
- `docs/decisions/` (grep: bench/netem/quinn/manifest; `KARAR-KAYDI-2026-11.md` CC-PORT kaydı satır 76–82;
  `SAHIP-KARARI-v4-EK-A-G0-AKSAM-2026-11-09.md` §9 netem kuyruğu notları).
- Sahip kararları (#175): 7171705736 (dokuz karar; karar 3 PTT/metin dondurma, karar 9 300 ms pilot), 7172579876, 7174289733, 7174845328,
  7175487850, 7175498704, 7175888074, 7186813951 — bench crate'ine doğrudan dokunan madde yok; ilgili olanlar B-28'de.
- Sözleşme doğrulaması için kod: `client-core/src/runtime.rs` `KamaV1Profile` / `TRANSPORT_V1_PROFILE_ID` belgesi (279–380),
  `TransportEngine` (705–741); `adaptive-transport/src/sender.rs` `FecLossRiskConfig` (346–455).
