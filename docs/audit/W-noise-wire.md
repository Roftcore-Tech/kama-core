# W-noise-wire — `crates/kama-noise-wire/src/` (Crypto katmanı: el sıkışma, KDF, zarflar, replay, oturum kontrolü)

Taban: ürün hattı `125b9afa`. Satır numaraları bu tabandandır. Salt okuma denetimi; kaynak dosyalara yazılmadı, cargo koşulmadı.

## Kapsam

| dosya | okunan aralık | satır |
|---|---|---:|
| `src/lib.rs` | 1–95 | 95 |
| `src/aad.rs` | 1–24 | 24 |
| `src/clock.rs` | 1–102 | 102 |
| `src/control.rs` | 1–358 | 358 |
| `src/control_dedup_window.rs` | 1–117 | 117 |
| `src/control_id_gate.rs` | 1–121 | 121 |
| `src/counter_store.rs` | 1–136 | 136 |
| `src/demux.rs` | 1–46 | 46 |
| `src/deprecated.rs` | 1–63 | 63 |
| `src/ed25519_strict.rs` | 1–158 | 158 |
| `src/entropy.rs` | 1–126 | 126 |
| `src/envelope.rs` | 1–388 | 388 |
| `src/epoch_machine.rs` | 1–172 | 172 |
| `src/error.rs` | 1–72 | 72 |
| `src/handle_assign.rs` | 1–97 | 97 |
| `src/handshake.rs` | 1–694 | 694 |
| `src/hub_sim.rs` | 1–120 | 120 |
| `src/init_flight.rs` | 1–212 | 212 |
| `src/inner.rs` | 1–179 | 179 |
| `src/kdf.rs` | 1–749 (vektör testleri dahil) | 749 |
| `src/legacy.rs` | 1–214 | 214 |
| `src/opened.rs` | 1–78 | 78 |
| `src/outer.rs` | 1–78 | 78 |
| `src/param_bind.rs` | 1–116 | 116 |
| `src/peer_dir.rs` | 1–184 | 184 |
| `src/pn.rs` | 1–173 | 173 |
| `src/protector.rs` | 1–1201 | 1201 |
| `src/replay.rs` | 1–91 | 91 |
| `src/restart.rs` | 1–194 | 194 |
| `src/retry.rs` | 1–118 | 118 |
| `src/seam_mock.rs` | 1–652 (`test-util`) | 652 |
| `src/session.rs` | 1–173 | 173 |
| `src/session_control.rs` | 1–1085 | 1085 |
| **toplam (src)** | | **8386** |

Bağlam için okunan (envantere girmeyen): `Cargo.toml` 1–70; `tests/a2_f3_control_dedup_window.rs` 1–58 (W-NW-08'in pin kapsamı); `kama-contracts/src/s3.rs` 104–121 (`Opened`); depo çapında `grep` ile tüketici aramaları (hangi tipin ürün yolunda kullanıldığı). Ürün kurulumu `kama-node/src/wiring.rs:57` `EnvelopeKind::Legacy` + `NoiseWireProtection::with_control` kullanır; `KamaNoiseWireProtector`, `SessionManager`, `HandleRouter`, `ControlDedupWindow`, `ControlIdIssueGate` yalnız `kama-integration` (deneysel soğuk başlangıç alıcısı) ve testlerde; `HubSim`, `InitFlight`, `InitReceiver`, `AntiAmplification`, `PersistentReplayGuard`, `InitCounterStore`, `RetryClass`, `InboundControlDedup`, `LegacySessionProtector`, `HandleAssign`, `DhEpochMachine` (dış) depoda başka tüketiciye sahip değil.

## Davranış envanteri

### lib.rs
- `#![forbid(unsafe_code)]`; 31 modül; `control_dedup_window`, `control_id_gate` `experimental-cold-start` arkasında, `seam_mock` `test-util` arkasında. `hub_sim`, `init_flight`, `restart`, `retry`, `legacy`, `session`, `demux`, `handle_assign` koşulsuz derlenir ve re-export edilir.
- Uyumlu: 2 (ADR-01 unsafe yok; `test-util` sahtesi ürün derlemesinde yok). Bulgu: W-NW-17.

### aad.rs
- `AAD_DS="KAMA-WIRE-EXT-V1"` (16 B), `AAD_LEN=26` (türetilmiş). `build_aad(outer)`: DS ‖ h_ext_be ‖ `EXTENDED_V1` ‖ wire_flags ‖ pn_trunc_be. Durum tutmaz.
- Uyumlu: 1 (noise-wire §2.8 alan sırası, BE, tag/uzunluk AAD dışı).

### clock.rs
- `MonotonicClock` trait (`now() -> Duration`); `SystemClock` (`Instant` kökenli); `ManualClock` (`test-util`; `Arc<AtomicU64>` paylaşımlı, doyar). Testler: kopyalar aynı zamanı paylaşır, geri gitmez.
- Uyumlu: 2 (KS-GRACE-1 "monotonic"; sahip K2 saat enjeksiyonu).

### control.rs
- Sabitler: `RECOVERED_MAX_ESI_RANGES=16` (DR-1; transport-tdd §AMENDMENT golden notunda "DR-1 tavanı"), `CS_PROBE_PAD_LEN=9`, `CS_PROBE_BODY_LEN=12`, `CS_ECHO_BODY_LEN=11`, `CS_SUBTYPE_COLD_START=0x01` (hepsi §2.57 DR-CS-1'den).
- `encode_control_body(c)` (41–115): §2.57 gövdesi, control_id hariç; RECOVERED sayısını `as u16` yazar, tavan yalnız `debug_assert` (49).
- `control_body_len(c)` (121–141): gövde boyu; RECOVERED > 16 → `ControlCountExceedsCap`.
- `split_for_send(c, next_id)` (153–170): RECOVERED'ı 16'lık parçalara böler, ilk parça aynı id, kalanlar `next_id()`; diğerleri tek eleman.
- `Reader` (172–225): bounds-önce okuyucu; `finish` trailing bayt → ret.
- `decode_control_body(cp, id, body)` (232–343): RECOVERED (sayı tavanı tahsisten önce, `start>end` ret), BLOCK_COMPLETE, PARAM_SET, PARAM_ACK, BLOCK_ABORT, SESSION_CLOSE, SESSION_RENEW_REQUIRED, CS_PROBE (subtype + 9 B sıfır pad zorunlu), CS_ECHO; başka kod noktası ret. `reason_code`/`renew_hint` değer denetimi YOK.
- `decode_control(inner)` (349–358): Control iç çerçevesini çözer; FEC → `None`.
- Uyumlu: 9 (§2.57 tam düzenler, trailing ret, bounds-before-alloc 243–244, K3 0x10/0x11 silimi). Bulgular: W-NW-09, W-NW-10.

### control_dedup_window.rs (`experimental-cold-start`)
- `ControlDedupWindow` (25–117): `highest` + 4096 bitlik halka (`CONTROL_DEDUP_WINDOW`), `below_window_count` teşhisi. `check(id)`: ilk id → Fresh; `id>highest` → aradaki slotları temizle + Fresh; `id <= highest.saturating_sub(WINDOW−1)` → DuplicateAckOnly + sayaç (105); pencere içi işaretli → Duplicate, işaretsiz → Fresh.
- Uyumlu: 3 (A-3 sabit pencere/high-water, ayrık teşhis, A-2 probe/echo dışarıda). Bulgu: W-NW-08.

### control_id_gate.rs (`experimental-cold-start`)
- Sabitler: `MAX_CONTROL_ID_ISSUE_RATE_PER_DIRECTION=32`, `MAX_CONTROL_ID_BURST=64`, `MAX_CONTROL_REORDER_ALLOWANCE=16`, `MAX_RETRY_REORDER_HORIZON_S=105`; `REQUIRED_WINDOW_RAW=32·105+64+16=3440` (türetilmiş), `CONTROL_DEDUP_WINDOW=4096` (next_power_of_two; derleme zamanı assert).
- `ControlIdIssueGate`: `issue()` monoton id; batch başına 64 → `BurstExceeded`; `next_id >= wrap_threshold` (varsayılan `u32::MAX`) → `RenewalRequired`; `end_batch()` sayacı sıfırlar.
- Uyumlu: 2 (A-3 monoton atama, wrap öncesi yenileme). Bulgu: W-NW-20.

### counter_store.rs
- `CounterStore::reserve_next()`: cihaz başına tek `init_counter`; değer dönmeden kalıcılaştırılır; hata = ayrılmadı. `CounterStoreError{Exhausted, Persist}`.
- `MemoryCounterStore` (`test-util`): 0'dan; `u64::MAX` dağıtılmaz; Mutex, zehirlenmede iç değer. Testler: kesin artan, tükenince durum ilerlemez, çok iş parçacığında tekrar yok.
- Uyumlu: 3 (§0.7 RESTART "init_counter cihaz başına KALICI+MONOTON").

### demux.rs
- `HandleRouter`: `HashMap<u32, HandleClass{Legacy, Extended}>`; `classify(datagram)` ilk 4 B BE → sınıf, <4 B ya da bilinmeyen → `None`. Yalnız `kama-integration`'da.
- Uyumlu: 1 (§2.5 YOL-1 decrypt-öncesi seçim). Bulgu: W-NW-17 (ürün yolunda kullanılmıyor).

### deprecated.rs
- `deprecated_format(input)`: ilk 4 B `SAI1/SAA1/SAD1/SAC1/SRAM/SRPK/SRPB` → ilgili tür. `session_control` INIT ve RESPONSE girişinde ret için kullanır. Test: aktif magic'ler işaretlenmez.
- Uyumlu: 2 (ccr §35.12, AC-SM3-12, §30.7 DEPRECATED-REJECT).

### ed25519_strict.rs
- `verify_strict(pk, msg, sig)`: uzunluk → `S<ℓ` → A ve R kanonik (`y<p`, `x=0` ise işaret 0) → A, R çözülür → küçük mertebe A → R → dalek `verify_strict` (kofaktörsüz). Sabitler ℓ, p, p−1 (spec/RFC sabitleri). Testler: geçerli imza, bit bozma, uzunluk önce, sabit tutarlılığı.
- Uyumlu: 5 (ccr §5.2 dört ret nedeni + kofaktörsüz denklem). Bulgu: W-NW-24.

### entropy.rs
- `EntropySource::fill` (tam ya da hata); `OsEntropy` (`OsRng`, hatada tamponu sıfırlar); `FixedEntropy` (`test-util`; tükenince hata, kısmi doldurma yok).
- Uyumlu: 2.

### envelope.rs
- Sabitler: `MAX_UDP_PAYLOAD=65535−20−8=65507` (türetilmiş, IPv4 teorik azami), `WIRE_HANDLE_LEN=4`, `LEGACY_HDR_LEN=6`; `LEGACY_LIMITS`/`EXTENDED_LIMITS`/`HUB_NOOP_LIMITS` = `MAX_UDP_PAYLOAD − ek yük`; `SESSION_CONSERVATIVE_LIMITS` = ikisinin en kısıtlayıcısı.
- `EnvelopeProfile{Legacy{hub_handle, pv, suite, session_id_internal, send_direction}, Extended{h_ext}}`: oturum başına sabit; `overhead()` 22/26, `pn_params()` DR-3 setleri, `wire_handle()`. Debug `session_id_internal` yazmaz.
- `seal_legacy`/`parse_legacy`/`open_legacy`: `[hub_handle][seq16][AEAD(nonce96(full_pn), frame, SSD1-AAD)]`; `open_*` yalnız AEAD, pencereye dokunmaz.
- `seal_extended`/`parse_extended`/`open_extended`: 10 B dış başlık + AEAD; AAD = DS ‖ alınan 10 B.
- `seq16_of`, `pn_trunc_of`: düşük 16/32 bit.
- Uyumlu: 8 (ccr §35.6/§35.9 22 B ve 28 B AAD, hub_handle AAD dışı, noise-wire §2.6/§2.8, §4.5 nonce, sabit zamanlı tag yalnız kütüphanede). Bulgu: W-NW-22.

### epoch_machine.rs
- `GRACE_N_G=64`, `GRACE_T_G=30 s` (KS-GRACE-1), `GRACE_MIN_PACKETS` (eski ad).
- `DhEpochMachine{role, send_epoch, recv0, epoch1_installed}`: `on_response_sent` (Responder epoch-1 alımı kurulu), `on_response_received` (Initiator send=1, `InstallEpoch1AndSend`, idempotent), `on_valid_epoch1_received(now)` (ilk paket: Responder send=1 + grace #1; Initiator yalnız grace #1; sonra her paket sayım, 64'te `DiscardEpoch0Recv`), `on_time(now)` (grace içinde `now−since ≥ 30 s` → discard), `recv_accepts`.
- Uyumlu: 6 (§4 B3 adım 1–5, onaylayan paket #1, discard geri dönmez). Bulgular: W-NW-13, W-NW-14.

### error.rs
- `WireError` (16 değişken; `SendStateRetired` deneysel), `PnReject{TooOld, Duplicate, FutureGap}`; thiserror, içerik sızdırmayan sabit mesajlar.
- Uyumlu: 2 (§9 typed error).

### handle_assign.rs
- `HANDLE_ASSIGN_V1=0x01`, `HANDLE_ASSIGN_LEN=25`, `HANDLE_ASSIGN_ACK_LEN=8`. `HandleAssign::encode/decode` (tam boy, sürüm ret), `replay_identity()=(attempt, init_counter)`; `HandleAssignAck` 8 B.
- Uyumlu: 3 (§2.55 WIRE-FROZEN düzen). Bulgu: W-NW-17 (codec ürün yolunda tüketilmiyor; bkz. çapraz modül).

### handshake.rs
- Sabitler: `PROTOCOL_VERSION_V1=1`, `PROFILE_ID_V3_DEFAULT=1`, `CIPHER_SUITE_ID_V1=0x0001`, `SSI1/SSR1` AAD magic, `COMPACT_DEVICE_CERT_LEN=152`, `DEVICE_CERT_BUNDLE_MAX=16384`, `SSI1_REF_MIN_LEN=121`, `SSI1_INLINE_MIN_LEN=241`, `SSR1_MIN_LEN=57`, `SSR1_AAD_LEN=72`, bayrak maskeleri (hepsi ccr §35.2/§35.3/§35.5/§6.7'den).
- `x25519_public` (tüm-sıfır ret), `dh` (`was_contributory` ret), `session_context_v1` (100 B), `cert_ref_of` (SHA-256), `ssi1_aad`, `ssr1_aad`.
- `ssi1_cert_mode(flags)`: reserved → init_variant → cert_mode (11 ret). `device_cert_bundle_len`: SRDB/1, iki u32 uzunluk (her biri ≤16384), toplam ≤16384 ve girdiye sığar, `device_cert_hash == SHA-256(compact)`. `parse_ssi1`: kipine göre boy, AEAD öncesi.
- `seal_ssi1`: `ss_e`, `ss_s` → D1/D3/D2/D6 → AEAD(`k_init`, nonce96(0)); wire = flags ‖ dest_hash ‖ eph ‖ cert_field ‖ init_counter ‖ ct‖tag. `open_ssi1`: ayrıştır → alıcı biz mi → eph tüm-sıfır → dizinde gönderen → iki DH → D1 → AEAD.
- `seal_ssr1`/`open_ssr1`: `dh_ee`, `dh_se` → D4/D5/D7 → AEAD(`k_resp`, nonce96(0)); SSR1 flags tüm bayt 0.
- Debug sır/kimlik yazmaz. Modül belgesi FS etiketini doğru kullanır ("per-INIT ephemeral confidentiality").
- Uyumlu: 14 (ccr §35.2/§35.2.1/§35.3/§35.3.1/§35.5/§35.5.1, §5.1, AC-SM3-14, CLAUDE.md FS etiketi). Bulgular: W-NW-07, W-NW-16, W-NW-23.

### hub_sim.rs
- `HANDLE_ASSIGN_MAX_RETRIES=5` ([CAL]). `HubSim`: düz sayaçla `h_legacy=n, h_ext=n+1`; aynı (attempt, init_counter) → aynı atama; HANDLE_ASSIGN'ı yanıtlayan kuyruğuna önce koyar; atanmamış h_ext'i iletmez; ACK/retry sayaçları. Sınırsız `HashMap`/`Vec`. Ürün derlemesine girer, tüketicisi yok.
- Uyumlu: 2 (§2.55 sıralama garantisi ve idempotans modeli). Bulgu: W-NW-17.

### init_flight.rs
- `InitRetryConfig::alpha_default()` = 5 retry, 200 ms, ×2, ≤10 s ([CAL], ölçümle kilitlenmemiş). `InitFlight`: önbellekli datagramı aynen döndürür, sayı ve toplam süreyle sınırlı, onayda durur. `InitReceiver`: `(sender, recipient, init_counter)` → cert parmak izi; sınırsız `HashMap`. `AntiAmplification{factor}`: yanıt ≤ factor × istek.
- Uyumlu: 3 (§0.7-B MADDE-5 bayt-özdeş retry modeli). Bulgular: W-NW-17, W-NW-18.

### inner.rs
- `SensitiveSymbolBytes`: Clone yok, Debug yalnız uzunluk; Drop'ta sıfırlama YALNIZ `experimental-cold-start` açıkken.
- `FEC_INNER_HDR_LEN=11`, `CONTROL_INNER_PREFIX_LEN=6`. `encode_fec_inner`, `encode_control_inner`; `parse_inner`: sürüm, `inner_kind::classify` (0x10/0x11, 0x1B/0x1C ret), FEC bayrak ≠0 ret, `k_actual=0` ret, boş sembol ret; Control: control_id + ham gövde.
- Uyumlu: 6 (§2.56 tek alan, §2.6 FEC iç düzeni, B4 `k_actual ≥ 1`, Cryptex metadata ciphertext'te). Bulgu: W-NW-21.

### kdf.rs
- `HASH_LEN=32`, `SESSION_CONTEXT_V1_LEN=100`, `SESSION_ID_INTERNAL_LEN=16`, `DH_LEN=32`, altı `LABEL_*`, `DIR_A2B=1`, `DIR_B2A=2`, `EPOCH_1_CONTEXT=u16_be(1)`; `MAX_OKM_LEN=255·32`.
- `d1_init_root`…`d7_k_resp`: tek sarmalayıcı `kdf<L>` = Extract(salt=label, IKM=parçalar akışkan) + Expand(context, L); PRK sıfırlanır, çıktılar `Zeroizing`. `InitRoot`/`Epoch1Root` Clone yok, Debug redakte.
- Testler (304–749): `vectors/ccr-kdf-v1` dosyasından V-KDF-1..6 bayt-özdeş, adlı D1–D7 fonksiyonları, zincir ve eşitsizlikler (INV-NONCE-DIR-1, INV-NONCE-EPOCH-1, D6/D7 ayrımı, Trunc128), INV-KDF-3 negatif vektör, her vakanın tüketildiği, Debug redaksiyonu.
- Uyumlu: 12 (ccr §35.2.1 D1–D7, INV-KDF-1/2/3, V-KDF-1..7). Kayıtlı kalıntı: `hkdf`/`hmac` 0.12 iç HMAC durumu Drop'ta silinmez (modül 31–33 kendi beyanı; bulgu açılmadı).

### legacy.rs
- `SESSION_DATA_MAGIC="SSD1"`, `LEGACY_AAD_LEN=28`, `LEGACY_OVERHEAD=22`. `build_legacy_aad` (ürün yolunun tek AAD kurucusu, envelope.rs kullanır). `LegacyDirection{AToB=1, BToA=2}`.
- `LegacySessionProtector`: sabit epoch, çağıranın verdiği `full_pn` (tek kural: önceki mühürden büyük olmalı; tavan yok), `ReplayWindow(LEGACY)`; `open`: parse → reconstruct → precheck → AEAD → commit. Depoda tüketici yok.
- Uyumlu: 4 (§35.9 AAD, 22 B zarf, sıra §4-ORDER). Bulgular: W-NW-05, W-NW-26.

### opened.rs
- `PrivateAdmissionProof` (crate-içi kurucu), `OpenedTransportShard{inner, full_pn, dh_epoch, h_ext, _proof}` — yalnız `KamaNoiseWireProtector::open` üretir; `test_new` `test-util`. Testler (trybuild) dış kurucuyu reddeder.
- Uyumlu: 2 (§6 TYPE-LEVEL-ENFORCEMENT; yalnız bu eski yolda). Bulgu: W-NW-03.

### outer.rs
- `TAG_LEN=16`, `OUTER_HDR_LEN=10`, `FLAG_DH_EPOCH=1`. `ExtendedOuter::parse`: <26 B, format ≠ `EXTENDED_V1`, ayrılmış bayrak → ret; `serialize_hdr`, `wire_flags`.
- Uyumlu: 4 (§2.6 KİLİTLİ düzen, reserved → REJECT, sabit maliyet).

### param_bind.rs
- `bind_transport_parameters(fields)` = BLAKE2s-256(schema'nın `canonical_hash_input`'u; B4 ihlali → `ParamInvalid`); `verify_param_hash` uyuşmazlıkta `ParamHashMismatch`. Testler: belirlenimlilik, uyuşmazlık, B4 ret, golden ile backend özdeşliği.
- Uyumlu: 4 (§8 MADDE-8, B4, ADR-03 AMENDMENT-2, INV-ENGINE-SOC).

### peer_dir.rs
- `PeerDirectory::new`: tekrar eden dest_hash/cert_ref ve tüm-sıfır statik anahtar ret; `by_dest_hash`, `by_cert_ref`; çalışma zamanında büyümez. `LocalIdentity` (statik sır + dest_hash + cert_ref); Debug sızdırmaz.
- Uyumlu: 3 (ccr §35.4 KAPALI-SİSTEM KİPİ G1; §5.1).

### pn.rs
- `EXTENDED_PN_PARAMS{32, 2^14, Some(2^13)}`, `LEGACY_PN_PARAMS{16, 1024, None}`.
- `reconstruct`: §4 sözde kodu, üst blokta taşmasız, `cand<span` iken çıkarma yok; eşitlikte geriye aday (alt yarı) korunur, ileri eşitlikte yüksek aday.
- `ReplayWindow::precheck` (saf): too-old `cand+W<ew`, duplicate, `u64::MAX` ret, yalnız extended future-gap. `commit`: ileri sıçramada aralığı temizler (W−1 üstünde tamamı), `expected=cand+1` doygun.
- Uyumlu: 7 (§4 KİLİTLİ sözde kod, DR-3 genişlik başına sabit, D-GAP-1 legacy far-future kabulü, UNDERFLOW-KURALI, [C-9] eşitlik kuralı, AEAD öncesi iz bırakmaz). Not: extended `FUTURE_GAP_MAX=2^13` identity OQ-16 açık kararıdır; kod spec metnine uyar.

### protector.rs
- `SEND_PN_CAP=2^31` (sahip S6 = ccr REKEY_THRESHOLD), `KEY_LEN=32`, `nonce96` (§4.5 LE64).
- Eski API (57–396): `PacketClass`, `LogicalPacketRef`, `SealedPacketMeta{full_wire_pn, dh_epoch, sealed_len, logical_ref, packet_class, send_generation}`, `KamaNoiseWireSizer`, `SealArgs{wire_pn, dh_epoch, send_generation}`, `KamaNoiseWireProtector` (iki epoch anahtar dizisi, tek `last_sealed_pn` + tek pencere; `seal` PN'i çağırandan alır, cap ve monotonluk denetler; `open` §4-ORDER 1–8, `OpenedTransportShard` döner; `drop_send_state` deneysel A-4).
- Ürün API'si (398–1201): `allocate_send_pn` (cap'te `None`, sayaç tüketilmez), `SessionKeyMaterial`, `SessionCrypto{profile, send/recv[2], send_epoch, recv_accepts, next_send_pn, replay, renew_signalled, lifecycle}`; `try_open` saf (legacy: en yeni epoch önce ≤2 anahtar; extended: tek anahtar); `NoiseWireProtection` — `seal_session` (FrameTooLarge → UnknownSession → CapReached + tek `RenewRequired` → mühür → sayaç+1), `open_session` (başarıda tek commit), `seal_hub`/`open_hub` (no-op: `frame ‖ 0×16`; bilinmeyen tutamaçlı ve son 16 B sıfır datagram HubChannel'a atfedilir, `full_wire_pn=0`), kurulum yardımcıları (`install_session_at`, `install_epoch1`, `set_send_epoch` epoch-1'de epoch-0 gönderim anahtarını siler, `discard_epoch0_recv`, `retire_send`, `close`), `PacketProtection::open` (65 507 B üstü ret, demux önce, `session_tick` → `open_session` → `after_session_open`), `sealed_len` kesin, `limits`/`conservative_limits`.
- Uyumlu: 15 (S3 K1/K2/K5, §0.5 `seal().len()==sealed_len()`, PN SÜREKLİ, tek pencere, H-NO-MUTATE, cap→RenewRequired tek sefer, EPOCH-PN-INIT, legacy grace ≤2 anahtar ccr §35.8). Bulgular: W-NW-02, W-NW-03, W-NW-04, W-NW-25.

### replay.rs
- `PrecheckedPn` (yalnız precheck üretir, commit tüketir); `DirectionalReplay{reconstruct, precheck, commit}`; `fingerprint` (`test-util`).
- Uyumlu: 3 (P0-6 precheck saf, commit yalnız AEAD sonrası, yön başına tek pencere).

### restart.rs
- `InitCounterStore` (1'den başlar, `saturating_add` → `u64::MAX`'ta aynı değeri tekrar verir), `PersistentReplayGuard` (pair başına high-water + recent; high-water altı ve recent dışı → fail-closed; pair tavanı 128 ve recent 16 yalnız deneysel bayrakla), `snapshot/restore`, `early_data_policy`, `resume_allowed_after_restart()=false`. Hiçbiri ürün yolunda çağrılmıyor.
- Uyumlu: 2 (model olarak §0.7 PATCH-15). Bulgular: W-NW-01, W-NW-17, W-NW-18.

### retry.rs
- `RetryClass::of(cp)`: PARAM_SET → UntilParamAck; ABORT/COMPLETE/CLOSE/RENEW → UntilNetworkAck; RECOVERED, PARAM_ACK, CS_* ve diğer her şey → NoRetry. `control_produces_bandwidth_credit` hep `false`. `InboundControlDedup`: sınırsız `HashSet seen`; CLOSE tombstone tavanı dolunca yeni id eklenmez (tahliye yok). Ürün yolunda tüketici yok.
- Uyumlu: 3 (§2.57 retry tablosu). Bulgular: W-NW-17, W-NW-18.

### seam_mock.rs (`test-util`)
- `MockCrypto` (S3 §7 sahtesi): FNV tabanlı sahte tag, bağlam başına PN, 64 bitlik replay, `MOCK_MAX_SESSIONS=64`, `MOCK_MAX_EVENTS=64`, H-RETRY önbelleği, K7 en-yeni-önce cap, AssignHandle bağları; `fixture_identity/directory/protection` (SHA-256'dan türetilmiş TEST anahtarları, `MemoryCounterStore`).
- Uyumlu: 3 (güvenlik iddiası yok beyanı; sınırlı tablolar; ürün derlemesinde yok).

### session.rs
- `SessionId`, `Direction`, `SessionLifecycle{Active, Closed, Closing(deneysel)}`; `Session` (`KamaNoiseWireProtector` sarmalar; Closed'da seal → `SessionClosed`); `SessionManager` (sınırsız `HashMap<h_ext, Session>`; `open` dış başlık → tutamaç → protector). Yalnız `kama-integration`.
- Uyumlu: 2 (§2.55 bilinmeyen tutamaç → UnknownHandle; Closed veri kabul etmez). Bulgular: W-NW-04, W-NW-26.

### session_control.rs
- Sabitler: `ENVELOPE_INIT=1`, `ENVELOPE_INIT_RESPONSE=2` (app-rpc AMENDMENT-7), `PENDING_RESPONSE_TRIAL_CAP=16`, `HALF_OPEN_CAP=64`, `HALF_OPEN_TTL=7 gün`, `PROVISIONAL_TTL=HALF_OPEN_TTL`, `SESSION_MAX_AGE=7 gün` (KS-DOS-1, KS-AGE-1, DR-CONNECT-1 K5), `RESPONSE_COUNTER_V1=0`.
- `EnvelopeKind{Legacy, Extended}` (uç başına tek); `ControlConfig` (kimlik, dizin, saat, entropi, kalıcı sayaç deposu — hepsi zorunlu); `ControlFailure::CounterPersist`.
- Komutlar: `Initiate` (dizinde eş; pending ≤16; boş olmayan `early_unit` → Failed; `reserve_next` tek çağrı, hata → Failed + neden; REF kipi SSI1). `AcceptInit` (eski format → Failed; ayrıştır; dest_hash; dizinde cert_ref; `(eş, init_counter)` önbellekte → digest aynıysa önbellekli SSR1, farklıysa Failed; glare: yerel dest_hash < eşin → NoOutput; `open_ssi1`; boş olmayan early_data → Failed; yeni RESPONSE; etkin oturum varsa Provisional, yoksa Established + Ready; eş başına önbellek 64, dolunca en küçük sayaç düşer). `AcceptResponse` (pending en-yeni-önce ≤16 deneme; boş olmayan response_data → Failed; Ready; Renew ise eskiyi drain'e al). `AssignHandle` (tekrar idempotent; çakışma Failed; bekleyen başlatıcının epoch-0'ı hemen kurulur). `Renew` (yalnız Active; yeni INIT). `Close` (bekleyen ya da kurulu; tam `Closed(UserClosed)`).
- Veri yolu kancaları: `after_session_open` (Draining'de her geçerli paket sayılır, 64'te `Closed(Superseded)`; epoch-1 paketi makineyi sürer, Provisional → `Confirmed` + eskiyi drain); `session_tick`/`control_sweep` (T_g grace ve drain; PROVISIONAL_TTL'de sessiz düşüş; SESSION_MAX_AGE'de bir kez RenewRequired; bekleyen INIT HALF_OPEN_TTL'de `Closed(KeyHygiene)`); `try_install` (bağ + anahtar hazırsa protector'a kurar, makine durumunu uygular).
- Uyumlu: 18 (G1 dizin zorunlu, K4G-02 sayaç, S1 early_unit kararı, H-RETRY, K7, K10/DR-CONNECT-1 #3–#4 glare ve provisional, KS-GRACE-1, KS-AGE-1, B3 epoch adımları, INV-INIT-KEY-1 yeniden mühür yok). Bulgular: W-NW-01, W-NW-11, W-NW-12, W-NW-13, W-NW-14, W-NW-15, W-NW-16, W-NW-19, W-NW-27.

## Bulgular

| fid | tür | ciddiyet | konum | davranış | otorite | düzeltme |
|---|---|---|---|---|---|---|
| W-NW-01 | EKSİK | orta | session_control.rs:206–207, 713–733, 817–836; restart.rs:77–166 (bağlı değil) | Yanıtlayanın INIT tekrar koruması yalnız bellek içi `responses` önbelleğidir (eş başına 64, dolunca en küçük sayaç düşer). Süreç yeniden başladıktan sonra ya da aynı eşten 64 yeni INIT geldikten sonra, yakalanmış eski bir INIT yeniden oynatılırsa `open_ssi1` geçer (anahtar statik sırlardan türer) ve yeni oturum kurulur: etkin oturum yoksa `Established` + `Ready(h)` (Bob'un gönderimi kimsenin açamadığı oturuma gider), varsa 7 gün yaşayan `Provisional`. `PersistentReplayGuard` var ama düzleme bağlı değil. | noise-wire §0.7 RESTART-POLİTİKASI (PATCH-15): "receiver peer/device-başına bounded-PERSISTENT-anti-replay (high-water/bitmap) tutar · aynı-(sender_device_id, recipient_device_id, init_counter)-İKİNCİ-app-side-effect-ÜRETMEZ … replay-cache+TTL-tanımlı" | karar-gerekir: kalıcı high-water için `ControlConfig`'e depo arayüzü gerekir (imza değişikliği). Ara kod düzeltmesi (karar gerektirmez): eş başına bellek içi high-water; `init_counter ≤ high-water` ve önbellekte değilse `Failed`; dar test "65. INIT sonrası 1. INIT'in tekrarı → Failed". |
| W-NW-02 | EKSİK | orta | protector.rs:938–959, 984–1004, 1166; envelope.rs:85–86 | HubChannel no-op: mühür `frame ‖ 16×0`, açma yalnız "tutamaç kayıtlı değil ve son 16 B sıfır" sınamasıdır; kimlik doğrulama ve gizlilik yok, alımda `full_wire_pn` sabit 0. Bilinmeyen tutamaçlı her datagram (ör. sahte engine ACK, HANDLE_ASSIGN, heartbeat) `Opened{HubChannel}` olarak üst katmana çıkar. | noise-wire §2.5 ("HubChannel ile hop anahtarı alır ve ayrı PN uzayındadır"), §2.55 ("hub↔client-Noise-transport-kanalında-authenticated"); sahip PoC-KAPSAM-001 §3 "HubChannel gerçek hop anahtarı (no-op kalır)", §5 beyanlı açık (M4); KM-SEAM-TC-001 [OPEN] M4 | karar-gerekir: ertelemeyi sahip verdi (M4). PoC-KAPSAM K10 ("açılan çerçevenin ProtectionContext'ine bakmayan runtime … PoC'den önce düzeltilir") gereği HubChannel çerçevelerinin tüketicide hangi etkiye izinli olduğu ayrıca doğrulanmalı (bu denetimde DOĞRULANAMADI; client-core). |
| W-NW-03 | UYUŞMAZ | orta | protector.rs:973–981, 995–1003; kama-contracts s3.rs:107–113 | Ürün yolunun açma çıktısı `s3::Opened`'dır; alanları `pub`, herkes kurabilir. Mühürlü `OpenedTransportShard` (özel kanıt alanı) yalnız eski `KamaNoiseWireProtector::open` yolunda üretilir. | noise-wire §6 TYPE-LEVEL-ENFORCEMENT: "Opened/Authenticated-tip sealed: alanlar-PRIVATE + `_proof` (public-constructor-YOK) + YALNIZ protector open() üretir" (K-WIRE-SEALED-TYPE-1); karşıt: KM-SEAM-TC-001 (S3 FROZEN) `pub struct Opened { pub ctx, pub frame, pub meta }` | karar-gerekir: spec §6 ile donmuş S3 sözleşmesi çelişiyor; S3 değişikliği sahip onayı ister. |
| W-NW-04 | UYUŞMAZ | orta | protector.rs:57–114, 252–314, 371–396; session.rs:43–115 | Eski `KamaNoiseWireProtector` PN'i çağırandan alır (`SealArgs.wire_pn`); `SealedPacketMeta` `logical_ref/packet_class/send_generation` taşır; `KamaNoiseWireSizer` trait'i duruyor. Tüketicisi `kama-integration` (deneysel alıcı) ve `SessionManager`. | noise-wire §0 [K2, 2026-09-19]: "PN sayacını Crypto tutar; `SealArgs.wire_pn` kaldırılır"; §0.5: "Ayrı `KamaNoiseWireSizer` trait'i KALDIRILDI … `logical_ref` bu struct'tan ÇIKARILDI … `packet_class` da transport bilgisi (kaldırıldı)" | kod-duzeltmesi: tüketicileri `NoiseWireProtection`'a taşı; eski tipi `test-util`/deneysel bayrak arkasına al ya da CLAUDE.md §6 gereği "MUST NOT IMPLEMENT" ekine taşı (sessiz silme yok). Genel API kaldırma olduğu için PR'da açık kayıt. |
| W-NW-05 | ÖLÜ | düşük | legacy.rs:75–210 | `LegacySessionProtector` depoda tüketilmiyor; çağıranın verdiği PN, sabit epoch, PN tavanı yok (u64 sonuna dek mühürler). Ürün legacy yolu `envelope.rs` + `NoiseWireProtection`'dadır. | noise-wire §0 K2 (PN Crypto'da); ccr §35.8 REKEY_THRESHOLD 2^31; CLAUDE.md §6 "Deprecated içerik → appendix + MUST NOT IMPLEMENT" | kod-duzeltmesi: `test-util` arkasına al ya da ek olarak işaretle; `build_legacy_aad` (ürünün tek AAD kurucusu) yerinde kalır. |
| W-NW-06 | UYUŞMAZ | orta | Cargo.toml:37–42; handshake.rs:24–28, 159–180; ed25519_strict.rs:15, 107–116 | X25519 ve Ed25519 üretimde yalnız dalek ile yapılır; libsodium ile çapraz sınama yalnız AEAD için var (`libsodium-oracle`, Cargo.toml:15–18). | ADR-03: "libsodium (primary): Ed25519, X25519, …; dalek … libsodium ile cross-check"; AMENDMENT (DR-5) yalnız AEAD'i, AMENDMENT-2 yalnız BLAKE2s'i değiştirir ("Ed25519/X25519/BLAKE2s/ristretto kararları DEĞİŞMEDİ") | karar-gerekir: sahip ADR-03'e X25519/Ed25519 için DR-5 benzeri ek yazmalı (plan KATMAN 6'da "ADR-03 libsodium/dalek amendment" açık) ya da libsodium çapraz sınaması eklenmeli. |
| W-NW-07 | UYUŞMAZ | orta | handshake.rs:401–402, 497–505 | SSI1 telde `recipient_dest_hash` (32 B), `sender_cert_ref`, `init_counter` cleartext taşır. | ccr §35.3 SESSION_INIT_REF düzeni (dest_hash cleartext, hub routing) — kod buna uyar; karşıt: CLAUDE.md §4 dest_hash "WIRE'da cleartext TAŞINMAZ (şifreli ClientHint içinde / hub hub_handle'a çevirir)"; sahip PoC-KAPSAM-001 §5 "INIT meta alanları telde açık … kapanış M4" | karar-gerekir: spec (ccr) CLAUDE.md'nin üstünde, PoC kararı açığı beyan ediyor; CLAUDE.md §4 satırı ile ccr §35.3'ün uzlaştırılması sahibin. Kod değişmez. |
| W-NW-08 | UYUŞMAZ | orta | control_dedup_window.rs:105; control_id_gate.rs:76–82 | `highest < WINDOW−1` iken `highest.saturating_sub(WINDOW−1)` 0 olur; id 0 pencere içinde olduğu hâlde "pencere altı" sayılır. Kapı ilk id olarak 0 verir: `check(1)` sonra yeniden sıralanmış `check(0)` → `DuplicateAckOnly` + `below_window_count` artar, id 0'ın (örn. ilk PARAM_SET) yan etkisi uygulanmaz. Testler (a2_f3 11, 26) bu vakayı sabitlemiyor. Yalnız `experimental-cold-start`. | noise-wire §2.57 A-3: "window-altına-düşen-id → DuplicateAckOnly + ayrık teşhis" (id 0 pencere altı değil); §2.57 "aynı-id-ikinci-kez … side-effect-YOK" (ilk görülüş uygulanır) | kod-duzeltmesi: koşul `self.highest - id >= WINDOW - 1` (id ≤ highest dalında taşmasız); dar test `check(1); check(0) == Fresh`, `below_window_count()==0`. |
| W-NW-09 | EKSİK | düşük | control.rs:283–304 | `BLOCK_ABORT/SESSION_CLOSE/SESSION_RENEW_REQUIRED` `reason_code` ve `renew_hint` değer denetimi yapılmaz; her bayt kabul edilir. | noise-wire §2.57 "oversized/invalid-range/unknown-reason → FAIL-CLOSED"; spec'te bulunamadı (aranan: reason_code değer kaydı, renew_hint değerleri — noise-wire, ccr, transport-tdd) | karar-gerekir: kayıt yokken "bilinmeyen" tanımsız; sahip değer kümesini verince kod-duzeltmesi. |
| W-NW-10 | UYUŞMAZ | düşük | control.rs:49–51 | `encode_control_body` genel fonksiyonu RECOVERED sayısını `as u16` yazar; tavan yalnız `debug_assert`. Release derlemede >16 (ya da >65535) aralık sessizce bozuk gövde üretir. Seal yolu önce `control_body_len` çağırdığı için ürün yolunda erişilemez. | CLAUDE.md §9 açık hata tipleri; noise-wire §2.57 "her-count BOUNDED" | kod-duzeltmesi: `Result` dönen gövde ya da `pub(crate)`; tavan denetimini kodlayıcıya al. |
| W-NW-11 | TANIMSIZ | düşük | session_control.rs:78–85, 476–482, 817–828 | Hub tarafı sabitleri istemcide başka anlamla kullanılır: `HALF_OPEN_CAP` yanıtlayanın eş başına RESPONSE önbelleği tavanı; `HALF_OPEN_TTL` başlatıcının bekleyen INIT ömrü, süre dolunca `Closed(KeyHygiene)`. | ccr §35.15 KS-DOS-1: "PENDING_RESPONSE_TRIAL_CAP = 16 [İSTEMCİ tarafı — diğer üçü HUB tarafı]"; DR-CONNECT-1 K5 (PROVISIONAL_TTL istemci); spec'te bulunamadı (aranan: istemci bekleyen INIT TTL'i, yanıtlayan RESPONSE önbellek tavanı) | karar-gerekir: istemci sabitleri ayrı adla ve türetimle tanımlanmalı (DR-CONNECT-1 K5'teki ayrı ad ilkesi). |
| W-NW-12 | EKSİK | düşük | session_control.rs:769–816 | Yanıtlayan, dizindeki bir eşten gelen her yeni sayaçlı INIT için yeni (Provisional) oturum açar; eş başına oturum tavanı yok (yalnız RESPONSE önbelleği 64). | sahip PoC-KAPSAM-001 §5 beyanlı açık "Yanıtlayan tarafta eş başına provisional oturum tavanı yok (KS-DOS-1) … sonraki sprint"; CLAUDE.md §9 SR-AUDIT-001 | karar-gerekir: değer KS-DOS-1 ailesinden türetilecek; sahip açığı bilerek bıraktı. |
| W-NW-13 | EKSİK | düşük | epoch_machine.rs:148–171; session_control.rs:817–828, 869–888 | KS-GRACE-1 sayaçları (`grace_closed_by_count`, `grace_closed_by_time`, `epoch0_packet_after_close`) ve KS-DOS-1 sayaçları (`halfopen_drop_oldest`, `response_trial_exhausted`) yok; yalnız test kancası `last_response_trials`. | noise-wire §4 KS-GRACE-1 "Sayaç: grace_closed_by_count · grace_closed_by_time · epoch0_packet_after_close"; ccr §35.15 KS-DOS-1 "Sayaçlar: …" | kod-duzeltmesi: davranışı değiştirmeyen salt-okunur sayaçlar. |
| W-NW-14 | TANIMSIZ | düşük | epoch_machine.rs:18–20, 130–147 | Başlatanın epoch-0 ALIM grace'i ("[YORUM, A5.4]"): Bob'dan ilk geçerli epoch-1 paketiyle açılır, aynı N_g/T_g ile kapanır. | noise-wire §4 B3 adım 4–5 yalnız Bob'un epoch-0 recv grace'ini tanımlar; spec'te bulunamadı (aranan: initiator epoch-0 recv discard) | karar-gerekir: yorum makul ama tanım sahibin; spec yaması ya da karar kaydı. |
| W-NW-15 | TANIMSIZ | düşük | session_control.rs:777–788, 837–839 | Glare kaybedeninin bekleyen INIT'i olay üretmeden kapatılır ("Olay YOK [YORUM]"). | DR-CONNECT-1 #4 ("kaybeden pending'ini bırakıp gelen INIT'i işler") olay kuralı vermez; KM-SEAM-AT-001 K2 early_unit'i Transport'a verir | karar-gerekir (kayıt yoksa davranış böyle kalır; Transport tarafı DOĞRULANAMADI). |
| W-NW-16 | TANIMSIZ | düşük | session_control.rs:89, 763; handshake.rs:681–683 | `response_counter` her RESPONSE'ta sabit 0; başlatan monotonluk denetimi yapmaz (bekleyen INIT ilk yanıtta silindiği için ikinci yanıt zaten `Failed`). | ccr §35.5 "`response_counter` : u64_be # epoch 1 başlangıç" ve "[C-4] `response_counter` monoton olmalı; eşit/geri giden → REJECT"; PN SÜREKLİ (noise-wire §4, 2026-10-06) sonrası "epoch 1 başlangıç" anlamı spec'te bulunamadı | karar-gerekir: alanın PN SÜREKLİ sonrası anlamı tanımlanmalı. |
| W-NW-17 | ÖLÜ | düşük | hub_sim.rs:1–120; init_flight.rs:1–212; restart.rs:14–47, 168–194; retry.rs:14–118; demux.rs:1–46; handle_assign.rs:1–97 | Sprint-4 model tipleri ürün derlemesine koşulsuz girer, ürün yolunda tüketilmez. `HubSim` düz sayaçla tutamaç basar (ccr §35.7 PRF türetimi değil) ve `next_handle+1` u32 sonunda debug'da paniğe gider; `InitCounterStore` 1'den başlar (ürün deposu 0) ve `u64::MAX`'ta aynı değeri tekrar döndürür; `InitRetryConfig::alpha_default` 5/200 ms/×2/10 s [CAL] ölçülmemiş. | CLAUDE.md §6 (yarım kod → feature-flag), §9; ccr §35.7 INV-HANDLE-1; noise-wire §0.7-B "sabitler D0 ölçerek kilitler" | kod-duzeltmesi: `test-util` arkasına al; `InitCounterStore`'u kaldır ya da ek olarak işaretle (CounterStore yerine geçmez, modül notu zaten söylüyor). |
| W-NW-18 | UYUŞMAZ | düşük | retry.rs:61–66, 89–106; init_flight.rs:150–177; restart.rs:77–136 (bayrak kapalı); session.rs:121–134 | `InboundControlDedup.seen`, `InitReceiver.seen`, `PersistentReplayGuard.per_pair` (deneysel bayrak kapalıyken), `SessionManager.by_handle` sınırsız büyür; CLOSE tombstone tavanı dolunca tahliye yok. | CLAUDE.md §9 SR-AUDIT-001 "unbounded alloc yok"; noise-wire §2.57 A-3/D-5 (session-scoped HashSet yerine sabit pencere) | kod-duzeltmesi: ürün yolunda tüketici yok; W-NW-17 ile `test-util` arkasına alınır ya da sabit pencere (`ControlDedupWindow`) kullanılır. |
| W-NW-19 | DOĞRULANAMADI | düşük | session_control.rs:900–901, 919–920; 493–497 | Başlatıcı RESPONSE'u alınca `InstallEpoch1AndSend` eyleminin yalnız "gönderim epoch'u = 1" kısmı uygulanır; "hemen ≥1 paket gönder" yükümlülüğü Crypto'dan Transport'a ayrı bir sinyalle verilmez (yalnız `Ready`). | noise-wire §4 B3 adım 2: "HEMEN epoch-1'de en az bir paket gönderir (… boş batch) … idle-deadlock önlenir" | Transport'un `Ready` üzerine boş batch gönderip göndermediği client-core'da; bu kapsamda DOĞRULANAMADI. |
| W-NW-20 | TANIMSIZ | düşük | control_id_gate.rs:4–45 | `CONTROL_DEDUP_WINDOW=4096` ve girdileri (32 id/s, 105 s, burst 64, reorder 16) spec'te yok; burst gerekçesi silinmiş ACK/SACK bölmesine dayanıyor (23–25), 105 s PeerSlot idle süresinden; modül notu `control.rs:194` atfı bayat (`split_for_send` 153'te). | spec'te bulunamadı (aranan: CONTROL_DEDUP_WINDOW, CAL-10, MAX_CONTROL_ID_BURST — noise-wire, transport-tdd, engine-spec); K3 (0x10/0x11 silindi) | karar-gerekir: türetim girdileri güncel üreticilerle yeniden kaydedilmeli (deneysel kol). |
| W-NW-21 | TANIMSIZ | düşük | inner.rs:15–21, 67–74 | Düz metin FEC sembolünün Drop'ta sıfırlanması yalnız `experimental-cold-start` açıkken; kod "A-5 spec-§0.9-amendmanı" der. Ürün yolu (`s3::Opened.frame`) hiç sıfırlamaz. | spec'te bulunamadı (aranan: A-5 — noise-wire §0.9 yalnız A-4'ü içerir); CLAUDE.md'de sıfırlama zorunluluğu yok | karar-gerekir: A-5'in otoritesi kayda bağlanmalı ya da yorum düzeltilmeli. |
| W-NW-22 | DOĞRULANAMADI | düşük | envelope.rs:37–63, 82–89 | `limits()` azami düz metni IPv4 teorik UDP azamisinden (65 507 − ek yük) türetir; PMTU'yu yansıtmaz. | noise-wire D0-B "Effective PMTU = max_udp_payload_bytes"; §0.7-B "SESSION_INIT IP-fragmentation'a-GÜVENMEZ" | Transport'un birim boyunu bu değerden mi PMTU'dan mı aldığı bu kapsamda DOĞRULANAMADI; spec'te `max_udp_payload_bytes` değeri bulunamadı. |
| W-NW-23 | EKSİK | düşük | handshake.rs:350–380 | DeviceCertBundle ayrıştırıcısı `identity_ccr_len` ve `intermediate_ccr_len` için tek tek 16 384 sınırı uygular; CCR başına 8 192 ve `issuer_key_id == identity_ccr.subject_key_id[0:16]` denetimi yok (zincir doğrulaması kapalı-sistem kipinde kapsam dışı). | ccr §6.7 "CCR total 8192 byte"; §20.3 Validation | kod-duzeltmesi: 8 192 sınırı (sınır denetimi kipten bağımsız); `issuer_key_id` karşılaştırması zincir doğrulamasıyla birlikte (BL KATMAN 6). |
| W-NW-24 | DOĞRULANAMADI | düşük | ed25519_strict.rs:104–108 | A ve R kanoniklik denetimi birlikte, çözülebilirlik ondan sonra yapılır; A çözülemez ve R kanonik değilse `NonCanonicalPointEncoding` döner. | ccr §5.2 ve vektör sözlüğü `ccr-ed25519-neg-v1` precedence: "nokta kodlaması (A, sonra R)" | Vektör dosyası bu denetimde okunmadı; çözülebilirliğin "kodlama"ya dahil olup olmadığı DOĞRULANAMADI. |
| W-NW-25 | DOĞRULANAMADI | düşük | protector.rs:537–588 | Legacy grace içinde epoch-1 anahtarı önce, başarısızsa epoch-0 denenir; deneme sayısı (1 ya da 2) süreye yansır. | noise-wire §11 K-WIRE-TRIAL-1 "trial+decode-timing-uniform"; ccr §35.8 "grace içinde ≤2 anahtar" | Zamanlama testleri (`tests/timing_side_channel.rs`) bu kapsamda okunmadı; DOĞRULANAMADI. |
| W-NW-26 | UYUŞMAZ | düşük | session.rs:3; legacy.rs:6–7 | Belge metinleri eski kuralları anlatıyor: kimlikte `dh_epoch` (2026-10-06c ile çıktı); legacy için `FUTURE_GAP=4096/REORDER=8192` (D-GAP-1 ile kaldırıldı). Kod davranışı güncel. | noise-wire YAMA KAYDI 2026-10-06c; ccr §35.8 [D-GAP-1] | kod-duzeltmesi: yalnız yorum. |
| W-NW-27 | EKSİK | düşük | session_control.rs:91–98, 324–344; envelope.rs:93–113 | Zarf kipi uç başına tektir (ürün `Legacy`); bir oturum hem `h_legacy` (SESSION_DATA) hem `h_ext` (FEC/kontrol) kullanamaz; HANDLE_ASSIGN'ın iki tutamacından yalnız biri bağlanır. Üründe extended sınıf (FEC shard, E2E kontrol) gönderilemez. | noise-wire §2.5 YOL-1 (h_legacy ve h_ext aynı oturumun tek PN uzayında), §2.55 HANDLE_ASSIGN iki tutamaç; sahip PoC-KAPSAM-001 §3 "ciphertext-domain FEC (ARQ-only)" PoC dışı | karar-gerekir: PoC kapsamı FEC'i dışarıda bıraktığı için şimdilik tutarlı; iki tutamaçlı oturum (S3'te `AssignHandle` tek tutamaç) S3 değişikliği ister. |
| W-NW-28 | EKSİK | düşük | protector.rs:39–42, 907–919; session_control.rs:455–461 | Oturum yenileme tetikleri paket (`SEND_PN_CAP`) ve zaman (`SESSION_MAX_AGE`); bayt tavanı yok. | noise-wire §5 madde 4: "packet/byte/time-cap-dolunca SESSION_RENEW_REQUIRED"; K-WIRE-KEYUPD-1 | karar-gerekir: bayt tavanının değeri spec'te bulunamadı (aranan: byte cap, MAX_BYTES_PER_SESSION). |

## Okunan otorite bölümleri

- `specs/kama-noise-wire-tdd-v2.0.md` 1–823 (tamamı): YAMA KAYITLARI; §0, §0.9 (A-4), §0.7 (A/B, EARLY-DATA-REPLAY, RESTART PATCH-15), §0.5, §1, §2, §2.5, §2.55, §2.56, §2.57 (A-1/A-2/A-3), §2.6, §2.7, §2.8, §3, §4 (KS-GRACE-1, EPOCH-PN-INIT, B3), §4.5, §5, §5.5, §6, §7, §8 (A-1), §9, §10, §11, §11.5, §12, §13.5, D0-B tablosu, STATÜ-MERDİVENİ.
- `specs/ccr-format-spec.md` 238–372 (§5.1, §5.2, §6.1–§6.7), 1049–1098 (§20 DeviceCertBundle), 2250–3258 (§35.0–§35.18, KS-DOS-1, KS-HANDLE-1, DR-CONNECT-1 slotları, CRYPTO-P0-FREEZE statüsü).
- `specs/pki-ca-ra-hsm-tdd.md` 450–483 (§13 kripto kaydı).
- `specs/tech-stack-adr.md` 61–120 (ADR-03, AMENDMENT DR-5, AMENDMENT-2).
- CLAUDE.md §3–§9 ve YAMA KAYDI (FS etiketi, INV-FEC-1, INV-ENGINE-SOC, dest_hash, ADR listesi, YAPMA listesi, §9 bounded buffer).
- Sahip kararları: `docs/decisions/PoC-KAPSAM-001.md` 1–105; `docs/decisions/DR-CONNECT-1.md` (H4, H6, glare, K2–K5 satırları, grep); `docs/decisions/KARAR-KAYDI-2026-11.md` (grep: S1/D-09); #175 yorumları 7171705736, 7172579876, 7174289733, 7174845328, 7175487850, 7175498704, 7175888074, 7186813951 — hiçbiri bu crate'in davranışına dokunmuyor (transport, PTT, #262 kapsamı); bu dosyada "kilitli" bulgu yok.
- Bağlam: `docs/seams/KM-SEAM-TC-001.md` (grep: `Opened`, HubChannel no-op, [OPEN] M4), `docs/seams/KM-SEAM-AT-001.md` (grep: S1 early_unit YAMA KAYDI), `docs/gates/G-IMPL-S3-OKUMA-PAKETI.md` (grep: early_unit).
