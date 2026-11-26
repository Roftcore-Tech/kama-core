# M2 — Küçük crate'ler: davranış envanteri ve otorite karşılaştırması

Taban: `125b9afa` (ürün hattı; aday `1a4f9efc` içinde). Salt okuma; cargo çalıştırılmadı, test koşulmadı. Bütün satır numaraları bu tabandandır. "Ürün yolu" = `kama-node` → `client-core` bağımlılık grafiği (varsayılan özellikler).

## 1. Kapsam

| Dosya | Satır aralığı | Okunan |
|---|---|---|
| `crates/kama-vectors/src/lib.rs` | 1–1271 | 1271 |
| `crates/kama-engine/src/lib.rs` | 1–42 | 42 |
| `crates/kama-engine/src/error.rs` | 1–58 | 58 |
| `crates/kama-engine/src/preparam_hold.rs` | 1–62 | 62 |
| `crates/kama-engine/src/param_bootstrap.rs` | 1–83 | 83 |
| `crates/kama-engine/src/param_state.rs` | 1–86 | 86 |
| `crates/kama-engine/src/admission.rs` | 1–104 | 104 |
| `crates/kama-engine/src/recovered_map.rs` | 1–190 | 190 |
| `crates/kama-engine/src/cold_rx_state.rs` | 1–516 | 516 |
| `crates/kama-fec-codec/src/lib.rs` | 1–18 | 18 |
| `crates/kama-fec-codec/src/error.rs` | 1–41 | 41 |
| `crates/kama-fec-codec/src/profile.rs` | 1–108 | 108 |
| `crates/kama-fec-codec/src/codec.rs` | 1–408 | 408 |
| `crates/kama-transport-schema/src/lib.rs` | 1–100 | 100 |
| `crates/kama-transport-schema/src/inner_kind.rs` | 1–88 | 88 |
| `crates/kama-transport-schema/src/control.rs` | 1–100 | 100 |
| `crates/kama-transport-schema/src/param.rs` | 1–67 | 67 |
| `crates/kama-transport-schema/src/prefix.rs` | 1–106 | 106 |
| `crates/kama-integration/src/lib.rs` | 1–25 | 25 |
| `crates/kama-integration/src/receiver_control_core.rs` | 1–306 | 306 |
| `crates/kama-integration/src/receiver_host.rs` | 1–671 | 671 |
| `crates/admin-server/src/lib.rs`, `crates/ccr-codec/src/lib.rs`, `crates/discovery/src/lib.rs`, `crates/ra-ca/src/lib.rs` | her biri 1–3 | 12 |
| Kök `Cargo.toml` + 20 crate `Cargo.toml` | tamamı | 703 |
| **Toplam** | | **5165** |

Bağlam için okunan (kapsam dışı, yalnız sözleşme doğrulaması): `client-core/src/lib.rs:100–140`, `client-core/src/runtime.rs:871–883` ve `:312–315`, `kama-noise-wire/src/param_bind.rs:18–40`, `kama-noise-wire/src/control.rs` RECOVERED/CS kodlama satırları (grep).

## 2. Davranış envanteri

### 2.1 Dört boş crate (`ccr-codec`, `discovery`, `ra-ca`, `admin-server`)
Her biri yalnız 3 satırlık modül yorumu: "API-first iskelet; trait imzaları sonraki iş". Pub öğe, tip, trait, bağımlılık yok (`[dependencies]` boş). Workspace'te hiçbir crate bunlara bağımlı değil.

### 2.2 `kama-transport-schema`
- `lib.rs`: tip takma adları `ProfileId=u8`, `CodeSuiteId=u8`, `BlockId=u32`, `MessageId=u32`, `Esi=u16`, `KActual=u8`. Sabitler: `SOURCE_PREFIX_LEN=11` (4+4+2+1, türetilmiş), `EXTENDED_OUTER_OVERHEAD=26` (4+1+1+4+16), `LEGACY_SESSION_DATA_OVERHEAD=22` (4+2+16), `EXTENDED_V1=0x01`, `INNER_V1=0x01`. `LogicalShardHeader {kind, profile_id, block_id, esi, k_actual}` ve `UnsealedTransportPacket {FecShard{header, symbol: Vec<u8>}, Control}` DTO'ları; kripto taşımaz. `SchemaError` 6 tipli varyant (thiserror). `#![forbid(unsafe_code)]`.
- `inner_kind.rs`: codepoint sabitleri 0x01/0x02 (FEC), 0x10/0x11 `RESERVED_*`, 0x12–0x1A kontrol. `ShardKind::{codepoint, from_codepoint}`. `classify(cp)`: 0x01/0x02 → `Fec`, 0x12..=0x1A → `Control(cp)`, geri kalan (0x00, 0x03–0x11, 0x1B–0xFF) → `UnknownInnerKind` (fail-closed).
- `control.rs`: `Control` enum'u 9 gövde (Recovered, BlockComplete, ParamSet, ParamAck, BlockAbort, SessionClose, SessionRenewRequired, CsProbe, CsEcho). `codepoint()` ve `control_id()` saf eşlemeler. `Recovered.esi_ranges` tipte sınırsız `Vec`; sınır tel ayrıştırıcısında (`kama-noise-wire` `RECOVERED_MAX_ESI_RANGES=16`).
- `param.rs`: `PARAM_DS=b"KAMA-PARAM-V1"`, `K_SHAPE_MAX=255`, `E_MIN=11`. `ParamFields::validate` 1≤K_shape≤255, E≥11. `canonical_hash_input` DS ‖ 9 alan big-endian, sabit sıra (13+12 bayt); hash almaz.
- `prefix.rs`: `SOURCE_FLAG_FINAL=0x01`, ayrılmış maske `!0x01`. `serialize_symbol(payload, out)`: ayrılmış bit → ret; payload > E−11 → ret; `actual_payload_len ≠ payload.len()` → `SymbolLenMismatch`; 11 B önek + payload + sıfır dolgu yazar. `parse(symbol)`: <11 B → `BufferTooShort`; ayrılmış bit, uzunluk > kapasite → ret. `payload_slice`: önek sonrası `actual_payload_len` baytı (uzunluğa kırpılmış).

### 2.3 `kama-fec-codec`
- `profile.rs`: `CODE_SUITE_RS_SIMD_V1=0x01`. `FecProfile {profile_id, code_suite_id, k_shape, r_code_max, e}` (alanlar pub). `validate`: 1≤K≤255, E≥11, E çift, R≥1. Katalog `PROFILES`: 0x01 (2,2,64), 0x02 (4,4,64), 0x03 (8,4,64), 0x04 (16,8,64), 0x05 (8,4,1024), 0x06 (16,8,1024). `by_id` = izin listesi araması. Yorum (`:83–84`): E=1024 profilleri "sub-kbps/M3"; mühürlü shard 26+11+1024 = 1061 B (11 = iç FEC başlığı; aritmetik doğru).
- `codec.rs`: `FecCodec` trait'i `encode`, `reconstruct(…, known_zero_count)`, varsayılan `recompute_repair` (tümünü kodla, ESI seç). Ortak çekirdek: `ordered_source` (tam K_shape, E uzunluk, aralık, tekrar, boşluk denetimi); `run_encode`; `prepare_received` (K_actual = K_shape − known_zero; known_zero ≥ K → ret; kaynak ESI ≥ K_actual → `SourceInZeroRegion`; tekrar ESI → ret; admitted + known_zero < K → `Insufficient`); `run_decode` ([K_actual, K) bölgesine sıfır sembol enjekte eder, yalnız < K_actual kurtarılanları döndürür, ESI'ye göre sıralar). `RsSimdCodec` her çağrıda yeni motor; `RsSimdEngine` `RefCell` içinde motor tekrar kullanır (`expect` yalnız `Some` atandıktan sonra, erişilemez panik). Kayıp yoksa `reconstruct` boş döner.
- `error.rs`: 9 tipli varyant; RS motor hatası `String` olarak sarılır.

### 2.4 `kama-engine`
- `admission.rs` `admit_and_decode(shards, bound_profile_id, k_actual)`: profil `by_id`; known_zero = k_shape − k_actual (taşmada `Reassembly`); her `OpenedTransportShard` için `header.profile_id ≠ bound` → `ProfileMismatch`; kontrol çerçeveleri sessizce atlanır; `RsSimdCodec.reconstruct`; alınan + kurtarılan kaynakları ESI sırasına koyar; her sembolün önekini ayrıştırıp payload'ları ardışık birleştirir. Girdi tipi yalnız `open()`'ın ürettiği tanık tip (INV-FEC-1). `header.k_actual`, `header.block_id`, önekteki `message_id`/`chunk_offset`/`FINAL` denetlenmez.
- `param_bootstrap.rs`: `plan_param_bootstrap` BLAKE2s param_hash'i (noise-wire) hesaplar, `Control::ParamSet` üretir, `start_fec_immediately=true` (B2). `ParamAckTracker::on_param_ack`: tür yanlışsa `UnexpectedControl`, eko eşitse `confirmed=true`, değilse `ParamAckMismatch`; emisyonu etkilemez.
- `param_state.rs` `ParamState::on_param_set(param_set, expected)`: profile_id eşitliği, `verify_param_hash(expected, hash)`; geçerse `BoundParam{profile_id, code_suite_id (gelen değer), param_hash}` bağlar. Gelen `code_suite_id` beklenenle karşılaştırılmaz.
- `preparam_hold.rs`: `MAX_PREPARAM_SHARDS=256` (türetilmemiş); dolunca en eskiyi düşürür, `dropped` sayacı; `drain` hepsini verir.
- `recovered_map.rs`: `RECOVERED_MAP_CAP=4096` (türetilmemiş). `register(block, esi, unit)`: yeni anahtar sıraya eklenir, tavanda en eski düşer; var olan anahtar değeri günceller ama sırasını yenilemez. `resolve(Recovered)`: ters aralığı sessizce düzeltir (`min/max`), `BTreeMap::range` ile çözer, çözülemeyen sayıyı döner. Testler: gidiş-dönüş, çözülemeyenin sayılması, yanlış kontrol türünün reddi, tavanda en eskinin düşmesi.
- `cold_rx_state.rs` (yalnız `experimental-cold-start`): [CAL] sabitleri 8 açık blok, 24 shard/blok (16+8 katalog türevi), 192 toplam (8×24), 192 PARAM-öncesi (`min(192,256)`), 480 KiB/oturum, 15 MiB global, 64 oturum, 64 tombstone; derleme zamanı türetim pinleri. `admit_cold_shard`: salt-okunur denetim sırası (tekrar/rol çatışması → çatışma; shard/blok; PARAM-öncesi; açık blok; toplam; oturum baytı; global bayt), tek noktada işleme; ret yolunda yalnız teşhis sayaçları artar. `release_block` blok kaynaklarını düşer (alt taşmada tipli hata, ama önceki düşümler geri alınmaz; `_reason` kullanılmaz). `release_session` idempotent. `TombstoneRegistry` `Rc<Cell>` RAII sayaç; jeton ve kira `Drop`'ta panik etmeden iade eder.
- `error.rs`: `EngineError` (8 her zaman + 5 özellikli varyant).

### 2.5 `kama-integration`
- `receiver_control_core.rs` (varsayılan derleme): `MAX_ECHOES=4`, `ECHO_WIRE_MAX=43` (spec sayısının kopyası), `ECHO_BUDGET_BYTES=172`, `MAX_CORE_SESSIONS=1024` (türetilmemiş). `BootstrapEchoResponder`: sabit 4 yuvalı nesil dizisi, tekrar/nesil tavanı/bütçe denetimiyle tek bekleyen eko; `echo_control` `seen_count=arrival_gap=0`, subtype 0x01. `ReceiverControlCore`: oturum kurma (var olan `h_ext`'i sessizce değiştirir), `ingest` (AEAD aç → başarısızsa durum değişmeden `Rejected`; 0x19 çözülürse bekleyen eko; bozuk 0x19 → `Rejected`; diğer kontrol → `OtherControl`; FEC → `NonControl`), `emit_cs_echo` (`dh_epoch=0`, `send_generation=0`, `wire_pn` çağırandan).
- `receiver_host.rs` (yalnız `experimental-cold-start`): sabitler `BOOTSTRAP_PROFILE_ID=0x03`, `SHARD_RETAINED_COST_E64=336`, `RX_COLD_ABANDON_SMALL_NS=75 s`, `SESSION_IDLE_HOLD_NS=105 s`, `TOMBSTONE_GRACE_NS=30 s` (≤ idle, derleme pini). `on_datagram`: <4 B → düş; `h_ext` demux; tombstone → düş; AEAD aç; 0x19/0x1A → sayaç + (0x19 çözülürse) bekleyen eko, her durumda `ProbeEchoed`; diğer kontrol → `ControlDedupWindow` (Fresh/Duplicate); FEC → profil 0x03 değilse düş; `pre_param:false` ile soğuk kabul; blok toplama (ilk shard'ın `k_actual`'ı ve son tarihi); shard sayısı ≥ k_actual → bloğu `blocks`'tan çıkar, `admit_and_decode`; başarı → teslim + `release_block`; başarısızlık → `ShardAdmitted` (soğuk defter kaydı serbest bırakılmaz). `emit_recovered` `ControlIdIssueGate` ile kimlik alır, mühürler. `on_patience_tick` son tarihi geçen blokları `blocks` üzerinden boşaltır; boşta oturumları yalnız sayar. `close_session` iki aşamalı (önce tombstone yuvası ve CLOSE-ACK ön hesabı, sonra yıkım). `expire_tombstone` kaydı siler; yönlendirici kaydı silinmez. `session_manager`, `router` `dead_code`. `unreachable!()` (`:380`) ve `expect` (`:429`, `:629`) mantıksal olarak erişilemez.

### 2.6 `kama-vectors` (test araçları)
JSON vektör yükleyici; `SUPPORTED_SCHEMA_VERSION=2`, `MAX_FILE_BYTES=8 MiB` (ölçüm notuyla), dizin derinliği 32, alan uzunluk sınırları (64…262 144). `hex` yalnız küçük harf, çift uzunluk. Yinelenen anahtar her derinlikte (`UniqueValue`), sürüm kapısı yapısal denetimden önce, `deny_unknown_fields`, `case_count` tutarlılığı, flag/not tutarlılığı, kontrol karakteri, `_hex` alanları, repo geneli tekil `id`, `require_kinds` vakum kapısı (SV-015). Boyut `metadata` ile denetlenip ardından `read_to_string` ile okunur. Sembolik dizin bağları izlenmez. Birim testleri hex, token dilbilgisi, `nearest_key`, kırpma kurallarını sabitler.

### 2.7 Cargo.toml dosyaları
- Kök: 20 üye (`crypto` yok), `resolver=2`, ortak paket metası `Proprietary`, `unexpected_cfgs` için `cfg(kani)`, ortak bağımlılıklar (thiserror 2, serde 1, serde_json 1, blake2 0.10, tokio 1/time).
- `kama-fec-codec`: `reed-solomon-simd = "3.1.0"`. `kama-noise-wire`: üretimde `chacha20poly1305`, `blake2`, `hkdf`+`hmac`, `x25519-dalek`, `ed25519-dalek`, `sha2`, `rand_core`; `sodiumoxide =0.2.7` yalnız `libsodium-oracle` özelliğinde; lisans `UNLICENSED`. `kama-integration`: paket metası workspace'ten devralınmıyor. `[lints] workspace = true` 9 crate'te yok (`admin-server`, `bench`, `ccr-codec`, `client-core`, `discovery`, `kama-engine`, `kama-fec-codec`, `kama-integration`, `ra-ca`). `hub-server`: "async runtime YOK". `bench`: `quinn = "0.11"`, yorum "0.11.11" ile "0.11.18" arasında çelişik. Özellikler: `experimental-cold-start`, `experimental-fec-budget`, `experimental-fs-harness`, `diag-sched-telemetry`, `test-util`, `libsodium-oracle`, `bindgen`; ürün crate'leri (`client-core`, `kama-node`) bunlardan yalnız `diag-sched-telemetry` geçişini tanımlar, deneysel olanları açmaz.

### 2.8 Ürün yolundan erişilebilirlik
- Ürün yolunda: `kama-transport-schema` (tamamı), `kama-fec-codec` `RsSimdCodec` + `by_id` + `PROFILES`, `kama-engine` `plan_param_bootstrap` + `ParamState` (client-core `runtime.rs:42`).
- Ürün yolunda değil: `admit_and_decode` (yalnız `ClientCore::admit`, onu da yalnız kendi testi çağırıyor), `PreParamHold` (client-core kendi tutucusunu kullanıyor, `runtime.rs:5235`), `ParamAckTracker`, `RecoveredMap`, `RsSimdEngine`, `recompute_repair`, `cold_rx_state` ve `kama-integration` kütüphanesinin tamamı (hiçbir ürün crate'i `kama-integration`'a bağlı değil).

## 3. Uyumlu davranışlar (sayım)

| Dosya / crate | Uyumlu | Öne çıkanlar |
|---|---|---|
| schema `lib.rs` | 10 | ID genişlikleri, 11/26/22 B sabitleri, EXTENDED_V1/INNER_V1 (noise-wire §2.6, §2.56) |
| schema `inner_kind.rs` | 5 | §2.56 izin listesi; 0x10/0x11 ve 0x1B/0x1C `reserved(reject)` (DR-CONNECT-1 H4 notu) |
| schema `control.rs` | 4 | §2.57 gövde alanları ve codepoint eşlemesi |
| schema `param.rs` | 4 | §8 MADDE-8 DS, alan sırası, big-endian; B4 sınırları |
| schema `prefix.rs` | 5 | §2.6 önek düzeni, `FINAL` biti, ayrılmış bit reddi, uzunluk ≤ E−11 (MADDE-9) |
| fec-codec | 10 | K-FEC-SOC-1 bağımlılık grafiği, Model-A sıfır enjeksiyonu, karar koşulu, `SourceInZeroRegion`, symbol_len == E, sabit R_code_max, `recompute_repair` == `encode()[esi]`, katalog K∈{2,4,8,16}, E çift/64 katı, varint yok |
| engine | 11 | INV-FEC-1 tip kapısı, profil kapısı, RECOVERED ≠ kredi (N8), B2 proaktif emisyon, PARAM_ACK yalnız onay, hash doğrulaması fail-closed, sınırlı tutucu/defter, soğuk kabulün atomikliği, `Drop`'ta panik yok, §8 A-1 `RX_PREPARAM` formülü |
| integration | 7 | doğrula-sonra-kabul, 0x19/0x1A'nın dedup dışı tutulması (A-2), `seen_count`/`arrival_gap` sıfır (yalnız defter), eko < sonda (43 < 44), diğer kontrollerde dedup, iki aşamalı kapanış, sabit boyutlu yanıtlayıcı |
| kama-vectors | 6 | önce boyut sonra okuma (kısmen), katı hex, yinelenen anahtar, sürüm kapısı, `case_count`, vakum kapısı |

## 4. Bulgular

| fid | tür | ciddiyet | konum | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| M2-01 | UYUŞMAZ | orta | `Cargo.toml:3–24` | 20 üye; `crypto` crate'i yok (kripto `kama-noise-wire`'da); adlar `ra-ca`/`discovery` | CLAUDE.md §2 "8 crate: crypto · ccr-codec · …"; ADR-12 düzeni `crypto/`, `ra-ca-service/`, `discovery-server/` | karar-gerekir |
| M2-02 | UYUŞMAZ | orta | `crates/ccr-codec/src/lib.rs:1–3`; kanonik kodlama `kama-noise-wire`, `kama-transport-schema`, `hub-server/src/wire.rs`, `kama-provision`, `adaptive-transport/src/read.rs` | `ccr-codec` boş, hiçbir crate ona bağlı değil; kanonik yapılar beş ayrı crate'te üretilip ayrıştırılıyor | ADR-05 "`ccr-codec` crate tek kaynak"; tech-stack-adr §4 "Canonical yapı üreten/parse eden TEK crate: ccr-codec (kopya parser YASAK)". Karşı otorite: noise-wire-tdd §11.5 (schema crate bayt otoritesi) → spec↔ADR çelişkisi | karar-gerekir |
| M2-03 | EKSİK | düşük | `crates/ccr-codec/src/lib.rs:1–3` | public trait imzası yok | CLAUDE.md §6 "crypto+ccr-codec public trait imzalarını ÖNCE indir (typed-error stub)" | karar-gerekir (M2-02'ye bağlı) |
| M2-04 | EKSİK | düşük | `discovery`, `ra-ca`, `admin-server` `src/lib.rs:1–3` | hiçbir davranış yok | discovery §14 AC-DISC-1…10; pki §18 AC-PKI-*; admin §19 AC-ADM-*. PoC-KAPSAM-001 §3 bunları PoC dışında bırakır | karar-gerekir (takvim) |
| M2-05 | UYUŞMAZ | orta | `kama-fec-codec/Cargo.toml:11`; `codec.rs:51` | `reed-solomon-simd 3.1.0`, trait adı `FecCodec` | ADR-16 "KARAR: reed-solomon-erasure … reed-solomon-simd Beta'da"; "ErasureCodec trait ZORUNLU"; transport-layer-tdd §4.4.6 "reed-solomon-simd Beta". Karşı otorite: fec-core-tdd v3.0 başlık "Crate: reed-solomon-simd (v3.1.0)" | karar-gerekir |
| M2-06 | UYUŞMAZ | düşük | `kama-noise-wire/Cargo.toml:27–48` (ayrıca `kama-provision`, `hub-server`, `kama-node`) | Ed25519/X25519 üretimde dalek, HKDF RustCrypto; libsodium yalnız AEAD oracle'ı (Ed25519/X25519 çapraz denetimi yok) | ADR-03 "libsodium … Ed25519, X25519 … dalek … cross-check testleri"; DR-5 eki "Ed25519/X25519/BLAKE2s/ristretto kararları DEĞİŞMEDİ" (AEAD ve BLAKE2s ayrıca değiştirildi) | karar-gerekir |
| M2-07 | EKSİK | orta | `kama-engine/src/admission.rs:42–59` | `header.k_actual` parametreyle, `header.block_id` tek blokla karşılaştırılmıyor; iki bloğun shard'ları ESI'leri çakışmadıkça tek blok gibi çözülür | fec-core §6 PATCH-4 "Inner-K_actual ile-active-profile/block-state-uyuşmazsa: packet-REJECT + rank-ARTMAZ"; §6.5 MADDE-9 (block_id çözücü bağlamından) | kod-düzeltmesi |
| M2-08 | EKSİK | orta | `admission.rs:97–102` | payload'lar ESI sırasıyla birleştiriliyor; `message_id` eşitliği, `chunk_offset` sürekliliği, `FINAL` denetlenmiyor | fec-core §6.5 MADDE-9 "chunk_offset-block-tutarlılığı … Mismatch → reject"; §3 PATCH-3 "Bir-FEC-block birden-fazla-application-message-İÇEREMEZ" | kod-düzeltmesi |
| M2-09 | UYUŞMAZ | düşük | `admission.rs:78` (+`codec.rs:217–237`) | tekrar ESI bütün çağrıyı `DuplicateEsi` ile düşürüyor; tekrarı ayıklamak belgelenmemiş biçimde çağırana kalıyor | fec-core §6 "packet-REJECT + rank-ARTMAZ"; §13 "duplicate-ESI-rejection" (paket düzeyi) | kod-düzeltmesi |
| M2-10 | TANIMSIZ | düşük | `admission.rs:57` | kontrol çerçeveleri sessizce yok sayılıyor | spec'te bulunamadı (aranan: noise-wire §6, fec-core §12 admission'da kontrol çerçevesi) | kod-düzeltmesi |
| M2-11 | UYUŞMAZ | düşük | `kama-engine/src/param_state.rs:43–65` | PARAM_SET gövdesindeki `code_suite_id` beklenenle karşılaştırılmıyor; bağlanan değer gelen değer | noise-wire §8 "Alpha'da-TEK-code_suite_id", "Unsupported/hash-mismatch → … FAIL-CLOSED" | kod-düzeltmesi |
| M2-12 | DOĞRULANAMADI | düşük | `param_bootstrap.rs:45–83` | izleyicide eşleşmeme sayacı/son tarih yok; "kalıcı uyuşmazlık → SESSION_CLOSE" motor dışında mı yapılıyor, bu kapsamda doğrulanamadı | noise-wire §8 STATE-MACHINE, §2.57 retry tablosu | karar-gerekir |
| M2-13 | ÖLÜ | düşük | `preparam_hold.rs`, `param_bootstrap.rs:45–83`, `recovered_map.rs`, `admission.rs` (yalnız `ClientCore::admit` testi), `codec.rs:329–408` `RsSimdEngine`, `codec.rs:86–105` `recompute_repair` | ürün yolu bunları çağırmıyor; ürün kendi PARAM-öncesi tutucusunu kullanıyor. `PreParamHold`'un en-eskiyi-düşür kuralı ürün yolunda §8 A-1 ile geçersiz | noise-wire §8 A-1 ("aşımda-en-eski-drop DEĞİL … TYPED-REJECT"); CLAUDE.md §6 "Deprecated → appendix + MUST NOT IMPLEMENT, sessizce SİLME" | karar-gerekir |
| M2-14 | TANIMSIZ | düşük | `preparam_hold.rs:13` (256), `recovered_map.rs:13` (4096), `receiver_control_core.rs:59` (1024) | türetilmemiş tavanlar | noise-wire §8 "max_preparam_shards, küçük"; değer spec'te bulunamadı | karar-gerekir |
| M2-15 | TANIMSIZ | düşük | `recovered_map.rs:75` | ters ESI aralığı sessizce düzeltiliyor (tel ayrıştırıcı zaten reddediyor) | noise-wire §2.57 "invalid-range → FAIL-CLOSED" | kod-düzeltmesi |
| M2-16 | TANIMSIZ | düşük | `recovered_map.rs:44–55` | yeniden kayıt değeri güncelliyor, tahliye sırasını yenilemiyor | spec'te bulunamadı | kod-düzeltmesi |
| M2-17 | ÖLÜ | orta | `kama-engine/src/cold_rx_state.rs` (tamamı), `kama-integration/src/receiver_host.rs` (tamamı), `experimental-fs-harness` | hiçbir ürün crate'i bu özellikleri açmıyor; [CAL] sabitleri (8, 24, 192, 480 KiB, 15 MiB, 64) yalnız deney dondurma belgesinde | noise-wire §8 A-1 yalnız `RX_PREPARAM` formülünü verir; D-23 "bayrak DEMO+1'e kadar" | karar-gerekir |
| M2-18 | UYUŞMAZ | orta | `receiver_host.rs:427–443`, `:553–571` | çözüm başarısız olunca blok `blocks`'tan çıkıyor, soğuk defter kaydı serbest kalmıyor; tik yalnız `blocks`'u taradığı için kayıt hiç boşalmaz. 8 kez tekrarlanırsa oturum yeni blok alamaz. Sonraki shard'lar yeni son tarihle yeni toplama açar | kendi sözleşmesi `:415` "non-renewable deadline (RESETLENMEZ)", `:441` "blok deadline'da evict"; fec-core §13 sınırlar | kod-düzeltmesi |
| M2-19 | TANIMSIZ | orta | `receiver_host.rs:428–439` | teslimden sonra tamamlanan blok kaydı tutulmuyor; aynı `block_id`'nin fazla shard'ları bloğu yeniden açıyor; `k_actual=1` iken her ek onarım shard'ı payload'u yeniden teslim eder | noise-wire §2.6 MADDE-14 "block_id … NO-REUSE"; alıcıda tamamlanmış blok hafızası spec'te bulunamadı | kod-düzeltmesi |
| M2-20 | UYUŞMAZ | düşük | `receiver_host.rs:383–394` | profil 0x03'e sabit; `pre_param:false` sabit; `ParamState` hiç kullanılmıyor (modül yorumu "ParamState/profil-gate" diyor) | noise-wire §8 "authenticated-PARAM_SET'i-alınca-sonraki-FEC-shard'larını-kabul-eder; … bounded-hold" | kod-düzeltmesi |
| M2-21 | EKSİK | düşük | `receiver_host.rs:16–18`, `:116–122`, `:178–181`, `:320–324`, `:572–578`, `:661–663` | tombstone CLOSE tekrarına önbellekli ACK üretmiyor (düşürüyor), `CachedCloseAck` yer tutucu, yönlendirici kaydı silinmiyor, boşta oturum yalnız sayılıyor, taşıma `Receiver`'ı bağlanmamış | noise-wire §2.57 "SESSION_CLOSE-alan-taraf … TOMBSTONE/GRACE … CLOSE-retry'larına-ACK-üretebilsin" | karar-gerekir |
| M2-22 | UYUŞMAZ | düşük | `receiver_host.rs:265–292`; `receiver_control_core.rs:219–240` | var olan `h_ext` yeniden kurulursa: host `session_count`'u ikinci kez artırır ve eski soğuk rezervasyonu sızdırır; çekirdek oturumu (ve tekrar penceresini) sessizce değiştirir | CLAUDE.md §9 (sınırlı tampon); noise-wire §10 (replay penceresi) | kod-düzeltmesi |
| M2-23 | EKSİK | düşük | `receiver_control_core.rs:120–158` | eko için zaman tabanlı hız sınırı yok (yalnız 4 nesil ve 172 B bütçe) | noise-wire §2.57 "rate-limit T_ECHO_MIN/akış"; değer engine-spec'te [CAL]-aday | karar-gerekir |
| M2-24 | TANIMSIZ | düşük | `receiver_host.rs:342–360` | bozuk 0x19 de `ProbeEchoed` döner ve sayılır; çekirdekte aynı girdi `Rejected` | spec'te bulunamadı | kod-düzeltmesi |
| M2-25 | DOĞRULANAMADI | düşük | `receiver_host.rs:479–489`, `:522–532`; `receiver_control_core.rs:302–312` | mühürlemede `dh_epoch=0`, `send_generation=0` sabit, `wire_pn` çağırandan; nonce tekliğini `Session::seal`'ın zorlayıp zorlamadığı bu kapsamda doğrulanamadı | noise-wire §4, §4.5 | karar-gerekir |
| M2-26 | TANIMSIZ | düşük | `cold_rx_state.rs:463–500` | `release_block` alt taşmada hata döner ama blok silinmiş ve önceki sayaçlar düşülmüş kalır; `_reason` kullanılmaz | modülün kendi atomiklik pini (`:8–10`) | kod-düzeltmesi |
| M2-27 | UYUŞMAZ | düşük | `kama-transport-schema/src/prefix.rs:35–57` | `out.len() < 11` ve boş payload'da `out[4..8]` indeks paniği | CLAUDE.md §9 açık hata tipleri; noise-wire §8 B4 `E ≥ 11` | kod-düzeltmesi |
| M2-28 | TANIMSIZ | düşük | `prefix.rs:47–51` | `actual_payload_len ≠ payload.len()` `SymbolLenMismatch` olarak raporlanıyor | spec'te bulunamadı | kod-düzeltmesi |
| M2-29 | EKSİK | düşük | `kama-fec-codec/src/profile.rs:12–21` | `FecProfile`'da `partial_policy` ve `packet_schedule_policy` yok; client-core `runtime.rs:880–881` ikisini 0'a sabitliyor; değerlerin anlamı tanımsız | fec-core §4 "Profile-immutable-tuple: (code_suite_id, K_shape, R_code_max, E, partial_policy, packet_schedule_policy)" | karar-gerekir |
| M2-30 | UYUŞMAZ | orta | `profile.rs:83–84`; ürün seçimi `client-core/src/runtime.rs:312–315` (`FEC_PROFILE_ID=0x05`) | E=1024 profilleri "sub-kbps/M3" olarak etiketli; ürün E=1024 profilini "düşük-bant hedef rejimi" gerekçesiyle kullanıyor | fec-core §11 "sub-kbps ≠ E=1024 … Sub-kbps-adayı = küçük-E (64/96/128)" | kilitli (ürün profil kimliği D-32 (iii); PTT/metin dondurması karar 3) |
| M2-31 | TANIMSIZ | düşük | `profile.rs:28–42`; `codec.rs:299`, `:316` | codec katalog dışı profili kabul ediyor (alanlar pub; izin listesi yalnız `by_id`'de) | fec-core §6 "enumerated-profile", §13 "profile-allowlist" | kod-düzeltmesi |
| M2-32 | ÖLÜ | düşük | `kama-engine/src/error.rs:34–36`, `:47–49`; `receiver_host.rs:76–78` | `FeatureNotImplemented`, `ParamNeverArrived` (engine), `HostError::FeatureNotImplemented` hiç üretilmiyor | — | kod-düzeltmesi |
| M2-33 | TANIMSIZ | düşük | `kama-vectors/src/lib.rs:623–659` | boyut `metadata` ile denetlenip dosya sınırsız `read_to_string` ile okunuyor (arada dosya büyüyebilir) | SR-AUDIT-001 bounds-before-alloc (CLAUDE.md §9) | kod-düzeltmesi |
| M2-34 | TANIMSIZ | düşük | `kama-noise-wire/Cargo.toml:5`; `kama-integration/Cargo.toml:3–6`; 9 crate'te `[lints]` yok; `bench/Cargo.toml:14–16` | lisans `UNLICENSED` (workspace `Proprietary`); paket metası devralınmıyor; workspace lint'i uygulanmıyor; quinn sürüm yorumu çelişik | spec'te bulunamadı (cargo deny lisans politikası tech-stack-adr §4) | kod-düzeltmesi |
| M2-35 | UYUŞMAZ | düşük | `hub-server/Cargo.toml:8–10` | hub tek iş parçacığı, async runtime yok | ADR-02 "Tüm Rust servisleri Tokio üzerinde"; karşı belge `docs/hub/KM-HUB-TDD-001.md:177`; sahip onayı DOĞRULANAMADI | karar-gerekir |
| M2-36 | TANIMSIZ | düşük | `receiver_control_core.rs:214–215`; `kama-vectors/src/lib.rs:119` | bayat belge: "there is no handshake in this repository yet"; `code` alanı "SV-001 … SV-008" (kodlar SV-015'e kadar) | — | kod-düzeltmesi |

Düzeltme notları (taslak, kod yazılmadı):
- M2-07/08/09: `admit_and_decode` başında bütün shard'larda `header.block_id` eşitliği ve `header.k_actual == k_actual` (aksi `Reassembly`/yeni tipli hata); tekrar ESI'yi atla ya da ön koşul olarak belgele; birleştirmede tek `message_id`, `chunk_offset`'in önceki uç + uzunluğa eşitliği, `FINAL` yalnız son kaynakta.
- M2-11: `*code_suite_id != expected.code_suite_id` → yeni `EngineError::CodeSuiteMismatch`.
- M2-18/19: başarısız çözümde `release_block(DeadlineEvicted)` ya da toplamayı geri koy; tamamlanan `block_id`'leri sınırlı bir kümede tut, gelen shard'ı `Duplicate` say.
- M2-27: `out.len() < SOURCE_PREFIX_LEN` → `BufferTooShort`.
- M2-31: `encode`/`reconstruct` başında `by_id(profile.profile_id) == Some(*profile)`.
- M2-33: `File::open(..).take(max_bytes + 1)` ile oku, uzunluk aşılırsa SV-012.

## 5. Okunan otorite bölümleri

- CLAUDE.md §2, §3, §4 (INV-FEC-1, INV-ENGINE-SOC), §5, §6, §8, §9, YAMA KAYDI.
- `specs/tech-stack-adr.md` §0, ADR-01, ADR-02, ADR-03 (+ DR-5 eki, eki-2), ADR-05, ADR-12, ADR-13…ADR-19, §3 karar matrisi, §4 bağımlılık politikası.
- `specs/kama-noise-wire-tdd-v2.0.md` §2.56, §2.57 (A-2, A-3 dahil), §2.6, §2.7, §2.8, §6, §7, §8 (A-1 dahil), §9, §10, §11, §11.5.
- `specs/fec-core-tdd-v3.0.md` başlık notu, §1.5, §2, §3, §4, §5, §6, §6.5, §11, §12, §13, §14.
- `specs/transport-layer-tdd.md` §4.4.6 (FEC crate notu).
- `specs/ccr-format-spec.md` §4 (kanonik kodlama), §32 (birlikte çalışma); bölüm listesi.
- `specs/discovery-architecture.md` §14; `specs/pki-ca-ra-hsm-tdd.md` §13, §18; `specs/admin-control-plane-tdd.md` §19.
- `docs/decisions/KARAR-KAYDI-2026-11.md` §1–§10; `docs/decisions/PoC-KAPSAM-001.md` (tamamı); `docs/decisions/DR-CONNECT-1.md` H4 satırı.
- #175 sahip kararları 7171705736, 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951: bu crate'lerin davranışını doğrudan bağlayan madde yok; karar 3 (PTT/metin dondurması) M2-30'u kilitler.
