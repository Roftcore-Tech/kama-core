# H-2 — `hub-server` `wire.rs` ↔ `vectors/hub` karşılaştırması ve uyuşmazlık listesi

| Alan | Değer |
|---|---|
| İş | S2-H2 (ISEMRI-S2A-v3-HA, HA-M3); v4 III.3 "H-2 wire.rs ↔ vektör uyuşmazlık listesi hub-server sahibine (S4 kazanır)" |
| Sınanan | `crates/hub-server` @ ``a858f3fc` (`wire.rs` son değişiklik `6bec6e1f`, #138; main `65cc7dba`'da aynı)` (`src/wire.rs`, ayrıca açık yardımcılar `bind::cookie`, `bind::HubIdentity::sign_ack`, `relay::valid_combination`, `relay::envelope_digest`, `table::handle_candidate`, `config::parse` ve alım yolu kuralları `lib.rs` `on_datagram`/`on_rpc`, `bind.rs` `on_heartbeat`, `relay.rs` `on_store`) |
| Otorite | `docs/seams/KM-SEAM-TH-001.md` (S4 v1.3.1) §2; vektörler `vectors/hub/` @ `s2a/v3-ha-h1` @ `ad3233c6` (`ref/hub/gen_vectors.py`, PR #155) |
| Yöntem | Salt-okur. `crates/hub-server/**`'a yazılmadı (KK-H14). |
| Tarih | 2026-11-10 |

> Etiket: gösterim; kapılı PoC ölçümü Aralık; ARQ yolu, FEC Aralık'ta.

## (a) Yöntem

1. `vectors/hub/` altındaki dört süitin (109 vaka) her vakası, `kama-vectors` yükleyicisiyle okundu. Yükleyici `SCHEMA.json` kapısını uygular.
2. Her vaka için `hub-server`'ın kendi genel API'si çağrıldı. Hiçbir S4 kuralı denetleyicide yeniden yazılmadı, sonuç hub'ın koduyla üretildi:
   - **pozitif tel vakaları:** girdi alanlarından `wire::*` yükü (`Payload::to_vec`) ve `RpcFrame::encode` / `control_datagram` kuruldu, beklenen baytlarla karşılaştırıldı. Beklenen baytlar `RpcFrame::decode` + ilgili `Payload::decode` ile geri çözüldü.
   - **negatif vakalar:** hub'ın **alım yolu** uygulandı (`on_datagram` boy denetimi → `classify` → `frame_type` → `on_heartbeat` gövde boyu / `on_rpc` flags ve method → `Payload::decode` → `on_store` `valid_combination` + RELAY_OPEN ≤ 64 B). Hub'ın hiç almadığı mesaj yönlerinde (hub → telefon: yanıtlar, DELIVER, ROUTE_MISS) `wire.rs` çözücüsü doğrudan çağrıldı.
   - **bayt birleştiricileri ve kripto:** `bind_proof_tbs`, `bind_ack_tbs`, `BIND_DOMAIN`/`ACK_DOMAIN`, `cookie_input` (sırdan sonraki baytlar), `bind::cookie` (tam BLAKE2s), `table::handle_candidate`, `relay::envelope_digest`. `bind-sig` vakasında imza `ed25519-dalek` `verify_strict` ile `wire.rs` TBS'i üzerinde doğrulandı. `ack-sig` vakasında hub'ın kendi imzalayıcısı (`config::parse` ile kurulan `HubSecret` → `HubIdentity::sign_ack`) imzayı yeniden üretti ve bayt-özdeşliğe bakıldı.
3. Sonuç sınıfları: **eşit** (wire.rs S4 vektörüyle aynı baytı üretiyor ya da aynı ret kararını veriyor) · **farklı** (bayt farkı ya da S4'ün reddettiği girdiyi wire.rs kabul ediyor) · **desteklenmiyor** (wire.rs'te o tür için kod yok ya da o yön hub'ın işi değil).

## (b) Geçici crate'in komutları (KK-H14; depoya girmez)

Crate depo dışında, geçici bir dizinde (`h2-check/`) durur. Path bağımlılıkları depo klonundaki `crates/hub-server` ve `crates/kama-vectors`'tır. Harici bağımlılıklar (`blake2 0.10`, `ed25519-dalek 2`, `serde_json 1`) depo `Cargo.lock`'undaki sürümlerle `--offline` çözüldü; yeni crate yok.

```bash
mkdir -p h2-check/src && cd h2-check
# Cargo.toml: [workspace] (bağımsız) · hub-server, kama-vectors = path "<klon>/crates/…" · blake2, ed25519-dalek, serde_json
cp <klon>/Cargo.lock .
cargo run --offline -j2 -- <vectors/hub dizini> > h2-results.tsv
```
`src/main.rs` her vaka için `id · kind · spec_ref · sonuç · not` satırı basar. Aşağıdaki (c) tablosu bu çıktıdır.

## (c) Vaka başına sonuç

109 vaka · **eşit 93 · farklı 8 · desteklenmiyor 8**. Bütün pozitif vakalar (her bayt düzeni, TBS, cookie, imza, aday) eşit; farklı olanların hepsi negatif.

| vaka | tür | S4 § | wire.rs | not |
|---|---|---|---|---|
| `s4-alloc-v1.relay-open-envelope` | relay-open-envelope | §2.4(f) RELAY_OPEN zarfı = open_nonce bytes16 (call_id yok) | **desteklenmiyor** | wire.rs'te zarf tipi yok (hub için opak, S4 §2.4(f)); yalnız ≤ 64 B sınırı on_store'da |
| `s4-alloc-v1.allocation-key.init` | allocation-key | §2.4(c) allocation_key (KR-19; = idempotency-key, §7) | **desteklenmiyor** | wire::AllocationKey bayt kodlaması yok (HashMap anahtarı, telde değil); alan sırası aynı; envelope_digest eşit |
| `s4-alloc-v1.allocation-key.init-other-recipient` | allocation-key | §2.4(c) allocation_key (KR-19; = idempotency-key, §7) | **desteklenmiyor** | wire::AllocationKey bayt kodlaması yok (HashMap anahtarı, telde değil); alan sırası aynı; envelope_digest eşit |
| `s4-alloc-v1.allocation-key.init-other-envelope` | allocation-key | §2.4(c) allocation_key (KR-19; = idempotency-key, §7) | **desteklenmiyor** | wire::AllocationKey bayt kodlaması yok (HashMap anahtarı, telde değil); alan sırası aynı; envelope_digest eşit |
| `s4-alloc-v1.allocation-key.relay-open` | allocation-key | §2.4(c) allocation_key (KR-19; = idempotency-key, §7) | **desteklenmiyor** | wire::AllocationKey bayt kodlaması yok (HashMap anahtarı, telde değil); alan sırası aynı; envelope_digest eşit |
| `s4-alloc-v1.handle-candidate.c100` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c101` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c102` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c103` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c104` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c105` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c106` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c107` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-candidate.c108` | handle-candidate | §2.6 aday: BLAKE2s-256(handle_secret ‖ alloc_counter u64)[0..4] | **eşit** | table::handle_candidate |
| `s4-alloc-v1.handle-alloc.first-free` | handle-alloc | §2.6 (ilk aday boş → tahsis; sayaç +1) | **eşit** | handle_candidate + pick_handle kuralı (yeniden oynatma) |
| `s4-alloc-v1.handle-alloc.skip-taken` | handle-alloc | §2.6 (aktif/tombstone aday atlanır; sayaç her denemede +1) | **eşit** | handle_candidate + pick_handle kuralı (yeniden oynatma) |
| `s4-alloc-v1.handle-alloc.eight-collisions` | handle-alloc | §2.6 (8 denemede de çakışma → tahsis yok, HUB_CAPACITY; üstüne yazma yok); §7 K6 | **eşit** | handle_candidate + pick_handle kuralı (yeniden oynatma) |
| `s4-alloc-v1.handle-alloc.after-eight-collisions` | handle-alloc | §2.6 (sonraki istek aynı adayları yeniden denemez); KM-HUB-TDD-001 T-M5 | **eşit** | handle_candidate + pick_handle kuralı (yeniden oynatma) |
| `s4-apprpc-v1.frame.request` | apprpc-frame | §2.3 AppRpcFrame (flags 00 istek) | **eşit** | RpcFrame encode/decode |
| `s4-apprpc-v1.frame.empty-payload` | apprpc-frame | §2.3 AppRpcFrame (payload_len = 0) | **eşit** | RpcFrame encode/decode |
| `s4-apprpc-v1.client-bind.request` | client-bind-req | §2.3 mesaj tablosu: CLIENT_BIND istek; §2.4(a) | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.client-bind.response-challenge` | client-bind-resp | §2.3 mesaj tablosu: CLIENT_BIND yanıt (meydan okuma); §2.4(a); §2.5 CHALLENGE | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.client-bind.error` | error | §2.3 mesaj tablosu: CLIENT_BIND hata; §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.bind-proof.request` | bind-proof-req | §2.3 mesaj tablosu: BIND_PROOF istek; §2.4(b) | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.bind-proof.response-bound` | bind-proof-resp | §2.3 mesaj tablosu: BIND_PROOF yanıt (bağlandı); §2.4(b) | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.bind-proof.error.bad-cookie` | error | §2.3 mesaj tablosu: BIND_PROOF hata; §2.4(b); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.bind-proof.error.possession-proof-fail` | error | §2.3 mesaj tablosu: BIND_PROOF hata; §2.4(b); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.bind-proof.error.rate-limited` | error | §2.3 mesaj tablosu: BIND_PROOF hata; §2.4(b); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.bind-proof.error.hub-capacity` | error | §2.3 mesaj tablosu: BIND_PROOF hata; §2.4(b); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.store.request.init-new-link` | store-message-req | §2.3 mesaj tablosu: STORE_MESSAGE istek; §2.4(c) {0, SESSION_INIT} | **eşit** | Payload::to_vec + RpcFrame + valid_combination |
| `s4-apprpc-v1.store.request.response-on-link` | store-message-req | §2.3 mesaj tablosu: STORE_MESSAGE istek; §2.4(c) {h, SESSION_INIT_RESPONSE} | **eşit** | Payload::to_vec + RpcFrame + valid_combination |
| `s4-apprpc-v1.store.request.relay-open-new-link` | store-message-req | §2.3 mesaj tablosu: STORE_MESSAGE istek; §2.4(c) {0, RELAY_OPEN}; §2.4(f) 16 B open_nonce | **eşit** | Payload::to_vec + RpcFrame + valid_combination |
| `s4-apprpc-v1.store.request.max-envelope` | store-message-req | §2.3 payload_len ≤ MAX_DATAGRAM − 31; §6 MAX_DATAGRAM | **eşit** | Payload::to_vec + RpcFrame + valid_combination |
| `s4-apprpc-v1.store.response.ok` | store-message-resp | §2.3 mesaj tablosu: STORE_MESSAGE yanıt; §2.4(c) {OK, h} | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.store.response.peer-unbound` | store-message-resp | §2.3 mesaj tablosu: STORE_MESSAGE yanıt; §2.6 adım 1 {PEER_UNBOUND, 0} | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.store.response.unknown-handle-0` | store-message-resp | §2.3 mesaj tablosu: STORE_MESSAGE yanıt; KR-19 emekli anahtar {UNKNOWN_HANDLE, 0} | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.store.error.bad-request` | error | §2.3 mesaj tablosu: STORE_MESSAGE hata; §2.4(c); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.store.error.rate-limited` | error | §2.3 mesaj tablosu: STORE_MESSAGE hata; §2.4(c); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.store.error.hub-capacity` | error | §2.3 mesaj tablosu: STORE_MESSAGE hata; §2.4(c); §2.5 | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.deliver.init` | deliver | §2.3 mesaj tablosu: DELIVER; §2.4(d) (INIT) | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.deliver.response` | deliver | §2.3 mesaj tablosu: DELIVER; §2.4(d) (RESPONSE; handle ataması §4 C4) | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.deliver.relay-open` | deliver | §2.3 mesaj tablosu: DELIVER; §2.4(d) (RELAY_OPEN; yalnız bilgi, KR-17) | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.route-miss` | route-miss | §2.3 mesaj tablosu: ROUTE_MISS; §2.4(e) reason 0x01 UNKNOWN_HANDLE | **eşit** | Payload::to_vec + RpcFrame |
| `s4-apprpc-v1.neg.header-short` | decode-apprpc | §2.3 (başlık 10 B) | **eşit** | RpcFrame::decode reddeder |
| `s4-apprpc-v1.neg.payload-len-mismatch` | decode-apprpc | §2.3 payload_len (kalan baytla tutmuyor) | **eşit** | RpcFrame::decode reddeder |
| `s4-apprpc-v1.neg.payload-over-max` | decode-apprpc | §2.3 payload_len ≤ MAX_DATAGRAM − 31 | **eşit** | RpcFrame::decode reddeder |
| `s4-apprpc-v1.neg.unknown-method` | decode-apprpc | §2.3 mesaj tablosu (yalnız 0x01/0x05/0x0B/0x81/0x84) | **eşit** | on_rpc drop_unknown_method |
| `s4-apprpc-v1.neg.flags-error-without-response` | decode-apprpc | §2.3 flags (yalnız 00 / 01 / 03) | **eşit** | on_rpc flags ≠ 0 → drop_malformed (alım yolu); codec'te flags denetimi yok |
| `s4-apprpc-v1.neg.flags-reserved-bit` | decode-apprpc | §2.3 flags (diğer bitler 0) | **eşit** | on_rpc flags ≠ 0 → drop_malformed (alım yolu); codec'te flags denetimi yok |
| `s4-apprpc-v1.neg.hub-initiated-with-response-flag` | decode-apprpc | §2.3 (bit7 = hub başlatır, yanıtı yoktur; flags = 0); §8 Y2 | **desteklenmiyor** | çerçeve düzeyi flags/request_id denetimi wire.rs'te yok (hub DELIVER/ROUTE_MISS'i yalnız yollar) |
| `s4-apprpc-v1.neg.hub-initiated-request-id-nonzero` | decode-apprpc | §2.3 request_id (hub başlatıyorsa 0); §8 Y2 | **desteklenmiyor** | çerçeve düzeyi flags/request_id denetimi wire.rs'te yok (hub DELIVER/ROUTE_MISS'i yalnız yollar) |
| `s4-apprpc-v1.neg.client-bind-req-short` | decode-apprpc | §2.4(a) istek (50 B) | **eşit** | truncated (hub alım yolu) |
| `s4-apprpc-v1.neg.client-bind-req-trailing` | decode-apprpc | §2.4(a) istek (50 B) | **eşit** | trailing bytes (hub alım yolu) |
| `s4-apprpc-v1.neg.client-bind-req-cert-mode` | decode-apprpc | §2.4(a) device_cert_mode = 0x00 | **eşit** | fixed field has an unexpected value (hub alım yolu) |
| `s4-apprpc-v1.neg.client-bind-req-presence-flags` | decode-apprpc | §2.4(a) presence_flags (PoC: 0) | **eşit** | fixed field has an unexpected value (hub alım yolu) |
| `s4-apprpc-v1.neg.client-bind-resp-not-challenge` | decode-apprpc | §2.4(a) yanıt error_code = 0x0003 CHALLENGE | **eşit** | fixed field has an unexpected value |
| `s4-apprpc-v1.neg.bind-proof-resp-bind-ok-0` | decode-apprpc | §2.4(b) yanıt bind_ok = 1 | **eşit** | fixed field has an unexpected value |
| `s4-apprpc-v1.neg.bind-proof-req-short` | decode-apprpc | §2.4(b) istek (128 B) | **eşit** | truncated (hub alım yolu) |
| `s4-apprpc-v1.neg.store-envelope-len-mismatch` | decode-apprpc | §2.4(c) envelope_len | **eşit** | length field mismatch (hub alım yolu) |
| `s4-apprpc-v1.neg.store-resp-unknown-status` | decode-apprpc | §2.3 STORE yanıt status ∈ {OK, PEER_UNBOUND, UNKNOWN_HANDLE}; §2.5 | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.store-resp-error-code-as-status` | decode-apprpc | §2.5 (hata kodları ErrorPayload'da, durum kodları yanıt alanında) | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.store-resp-ok-handle-0` | decode-apprpc | §2.1 (hub hub_handle = 0 hiç vermez); §2.4(c) {OK, h} | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.error-payload-status-code` | decode-apprpc | §2.5 (PEER_UNBOUND yanıt alanıdır, ErrorPayload değil) | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.error-payload-detail-len-mismatch` | decode-apprpc | §2.3 ErrorPayload detail_len | **eşit** | fixed field has an unexpected value |
| `s4-apprpc-v1.neg.deliver-handle-0` | decode-apprpc | §2.1 (hub_handle = 0 hiç verilmez); §2.4(d) | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.deliver-unknown-envelope-type` | decode-apprpc | §2.4(c)/(d) envelope_type ∈ {0x01, 0x02, 0x05} | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.route-miss-reason` | decode-apprpc | §2.4(e) reason (0x01 UNKNOWN_HANDLE) | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.route-miss-handle-0` | decode-apprpc | §2.1 (veri datagramında hub_handle ≠ 0); §2.4(e) | **farklı** | wire.rs çözücüsü kabul ediyor |
| `s4-apprpc-v1.neg.store-combo-0-response` | store-validate | §2.4(c) tablo: 0 · 0x02 geçersiz | **eşit** | relay.rs on_store → BAD_REQUEST |
| `s4-apprpc-v1.neg.store-combo-h-init` | store-validate | §2.4(c) tablo: h · 0x01 geçersiz | **eşit** | relay.rs on_store → BAD_REQUEST |
| `s4-apprpc-v1.neg.store-combo-h-relay-open` | store-validate | §2.4(c) tablo: h · 0x05 geçersiz (medya ACK'i E2E MEDIA_LINK, KR-17) | **eşit** | relay.rs on_store → BAD_REQUEST |
| `s4-apprpc-v1.neg.store-envelope-type-03` | store-validate | §2.4(c) tablo: 0x03 geçersiz | **eşit** | relay.rs on_store → BAD_REQUEST |
| `s4-apprpc-v1.neg.store-envelope-type-04` | store-validate | §2.4(c) tablo: 0x04 geçersiz | **eşit** | relay.rs on_store → BAD_REQUEST |
| `s4-apprpc-v1.neg.store-relay-open-65` | store-validate | §2.4(c) kurallar: RELAY_OPEN zarfı ≤ 64 B; §6 RELAY_OPEN_MAX_ENVELOPE | **eşit** | relay.rs on_store → BAD_REQUEST |
| `s4-apprpc-v1.store.request.relay-open-64` | store-message-req | §2.4(c) kurallar: RELAY_OPEN zarfı ≤ 64 B (sınır) | **eşit** | Payload::to_vec + RpcFrame + valid_combination |
| `s4-bind-v1.domain-sep.bind` | domain-sep | §2.4(b) domain_sep("KAMA-HUB-BIND-V1") | **eşit** | BIND_DOMAIN / ACK_DOMAIN |
| `s4-bind-v1.domain-sep.ack` | domain-sep | §2.4(b) domain_sep("KAMA-HUB-ACK-V1") | **eşit** | BIND_DOMAIN / ACK_DOMAIN |
| `s4-bind-v1.cookie-input.ipv4.epoch-0` | cookie-input | §2.4(a) hub_nonce (fam 0x04, bind_epoch 0) | **eşit** | cookie_input (sır sonrası) |
| `s4-bind-v1.cookie.ipv4.epoch-0` | cookie | §2.4(a) hub_nonce (fam 0x04, bind_epoch 0) | **eşit** | bind::cookie |
| `s4-bind-v1.cookie-input.ipv4.epoch-7` | cookie-input | §2.4(a) hub_nonce (fam 0x04, bind_epoch 7) | **eşit** | cookie_input (sır sonrası) |
| `s4-bind-v1.cookie.ipv4.epoch-7` | cookie | §2.4(a) hub_nonce (fam 0x04, bind_epoch 7) | **eşit** | bind::cookie |
| `s4-bind-v1.cookie-input.ipv4.prev-minute` | cookie-input | §2.4(a) minute; §6 COOKIE_WINDOW | **eşit** | cookie_input (sır sonrası) |
| `s4-bind-v1.cookie.ipv4.prev-minute` | cookie | §2.4(a) minute; §6 COOKIE_WINDOW | **eşit** | bind::cookie |
| `s4-bind-v1.cookie-input.ipv6.epoch-0` | cookie-input | §2.4(a) hub_nonce (fam 0x06, 16 B src_ip) | **eşit** | cookie_input (sır sonrası) |
| `s4-bind-v1.cookie.ipv6.epoch-0` | cookie | §2.4(a) hub_nonce (fam 0x06, 16 B src_ip) | **eşit** | bind::cookie |
| `s4-bind-v1.cookie-input.ipv4-mapped.epoch-0` | cookie-input | §2.4(a) IPv4-eşlemeli IPv6 → IPv4 normalizasyonu | **eşit** | cookie_input (sır sonrası) |
| `s4-bind-v1.cookie.ipv4-mapped.epoch-0` | cookie | §2.4(a) IPv4-eşlemeli IPv6 → IPv4 normalizasyonu | **eşit** | bind::cookie |
| `s4-bind-v1.bind-tbs.alice` | bind-tbs | §2.4(b) client_signature TBS (112 B) | **eşit** | bind_proof_tbs |
| `s4-bind-v1.bind-tbs.other-hub` | bind-tbs | §2.4(b) TBS hub_ed25519_pub'a bağlı | **eşit** | bind_proof_tbs |
| `s4-bind-v1.bind-sig.alice` | bind-sig | §2.4(b) client_signature (Ed25519, RFC 8032) | **eşit** | bind_proof_tbs + verify_strict |
| `s4-bind-v1.ack-tbs.alice` | ack-tbs | §2.4(b) hub_signature TBS (83 B) | **eşit** | bind_ack_tbs |
| `s4-bind-v1.ack-sig.alice` | ack-sig | §2.4(b) hub_signature (Ed25519, RFC 8032) | **eşit** | HubIdentity::sign_ack (hub'ın imzası bayt-özdeş) |
| `s4-hub-frame-v1.heartbeat.empty` | hub-frame | §2.2 HEARTBEAT (gövde boş) | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.heartbeat.token` | hub-frame | §2.2 HEARTBEAT (echo_token u32) | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.heartbeat-ack.empty` | hub-frame | §2.2 HEARTBEAT_ACK (gelenle aynı uzunluk) | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.heartbeat-ack.token` | hub-frame | §2.2 HEARTBEAT_ACK (token aynen) | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.plane1-rpc.client-bind-req` | hub-frame | §2.1 / §2.2 PLANE1_RPC; §3 H10 | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.plane1-rpc.client-bind-resp` | hub-frame | §2.1 / §2.2 PLANE1_RPC; §3 H10 | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.plane1-rpc.bind-proof-req` | hub-frame | §2.1 / §2.2 PLANE1_RPC; §3 H10 | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.plane1-rpc.bind-proof-error` | hub-frame | §2.1 / §2.2 PLANE1_RPC; §3 H10 | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.plane1-rpc.route-miss` | hub-frame | §2.1 / §2.2 PLANE1_RPC; §3 H10 | **eşit** | control_datagram + classify |
| `s4-hub-frame-v1.neg.heartbeat-body-len-2` | decode-hub-frame | §2.2 HEARTBEAT (başka uzunluk → reject) | **eşit** | on_heartbeat drop_malformed |
| `s4-hub-frame-v1.neg.heartbeat-ack-body-len-5` | decode-hub-frame | §2.2 HEARTBEAT_ACK | **desteklenmiyor** | hub HEARTBEAT_ACK çözmez (yalnız yollar); tip alımda drop_unknown_frame |
| `s4-hub-frame-v1.neg.unknown-frame-type` | decode-hub-frame | §2.2 (başka frame_type → sessiz drop); §3 H1 | **eşit** | drop_unknown_frame |
| `s4-hub-frame-v1.neg.unknown-frame-type-1e` | decode-hub-frame | §2.2; §8 Y1 (0x1E/0x1F rezerve, PoC'de yok) | **eşit** | drop_unknown_frame |
| `s4-hub-frame-v1.neg.prefix-nonzero` | decode-hub-frame | §2.1 (ilk 4 bayt 0 değilse veri sınıfı) | **eşit** | classify → veri sınıfı (kontrol değil) |
| `s4-hub-frame-v1.neg.trailer-nonzero` | decode-hub-frame | §2.1 (open_hub: son 16 bayt sıfır) | **eşit** | classify: not a hub control datagram → drop_malformed |
| `s4-hub-frame-v1.neg.shorter-than-21` | decode-hub-frame | §2.1 (en az 21 B) | **eşit** | classify: not a hub control datagram → drop_malformed |
| `s4-hub-frame-v1.neg.over-max-datagram` | decode-hub-frame | §2 (datagram ≤ MAX_DATAGRAM); §6 MAX_DATAGRAM = 1280 | **eşit** | on_datagram drop_size |

## (d) Numaralı uyuşmazlık listesi

Hiçbir **bayt düzeni** farkı yok: `wire.rs`'in ürettiği her datagram, yük, TBS, cookie, imza ve handle adayı S4 vektörüyle bayt-özdeş. Uyuşmazlıkların hepsi **çözücü katılığıyla** ilgili ve hepsi hub'ın yalnız **ürettiği**, hiç **almadığı** hub → telefon mesajlarında. Hub bu değerleri üretmiyor (alım yolu ve üretim kodu S4'e uyuyor). Bu yüzden hiçbiri hub davranışını ya da A3-min dumanını etkilemiyor. Etki yalnız `wire.rs` çözücülerini istemci tarafında kullanan bir kod için doğar; istemci (HB1) `kama_contracts::s4` kullanacak.

| # | S4 § | beklenen (S4 / vektör) | wire.rs | vakalar | etki |
|---|---|---|---|---|---|
| **U-1** | §2.3 STORE yanıt satırı, §2.5 | `status ∈ {OK, PEER_UNBOUND, UNKNOWN_HANDLE}` dışı (ör. `CHALLENGE`, `BAD_REQUEST`) → ret; `{OK, 0}` → ret (§2.1 hub `hub_handle = 0` vermez) | `StoreResponse::decode` her `status`'u ve `{OK, 0}`'ı kabul ediyor | `neg.store-resp-unknown-status`, `neg.store-resp-error-code-as-status`, `neg.store-resp-ok-handle-0` | hub: yok (yalnız yollar). İstemci wire.rs ile çözerse hata kodunu durum sanabilir. |
| **U-2** | §2.3 ErrorPayload, §2.5 iki sütun | durum kodu (`PEER_UNBOUND` vb.) taşıyan `ErrorPayload` → ret | `ErrorPayload::decode` kodu denetlemiyor | `neg.error-payload-status-code` | hub: yok. İstemcide sütun karışıklığı. |
| **U-3** | §2.1, §2.4(d) | DELIVER `hub_handle = 0` → ret; `envelope_type ∉ {0x01, 0x02, 0x05}` → ret | `Deliver::decode` ikisini de kabul ediyor | `neg.deliver-handle-0`, `neg.deliver-unknown-envelope-type` | hub: yok. İstemci 0 handle'ı atama sayabilir (C4). |
| **U-4** | §2.1, §2.4(e) | ROUTE_MISS `reason ≠ 0x01` → ret; `hub_handle = 0` → ret | `RouteMiss::decode` ikisini de kabul ediyor | `neg.route-miss-reason`, `neg.route-miss-handle-0` | hub: yok. |

**Desteklenmiyor (uyuşmazlık sayılmadı, bilgi için):**
- `allocation-key` ×4: `wire::AllocationKey` telde yer almıyor (`HashMap` anahtarı), bayt kodlaması yok. Alan sırası ve `relay::envelope_digest = BLAKE2s(envelope)[0..8]` vektörle eşit. Kodlama yalnız S4 §7 "idempotency-key" vektörünün tüketicisi için gerekli, hub için gerekli değil.
- `relay-open-envelope`: hub için opak zarf (§2.4(f)). Hub yalnız ≤ 64 B sınırını uyguluyor (`on_store`) ve bu sınır `store-validate` vakalarında eşit.
- `neg.hub-initiated-with-response-flag`, `neg.hub-initiated-request-id-nonzero`: `wire.rs`'te çerçeve düzeyi (method/flags/request_id) tutarlılık denetimi yok. Hub tarafında gerek de yok, çünkü alımda `flags ≠ 0` ya da bit7'li method düşüyor (`on_rpc` → `drop_malformed`). İstemci tarafında `kama_contracts::s4::RpcMessage` bu denetimi yapıyor.
- `neg.heartbeat-ack-body-len-5`: hub HEARTBEAT_ACK almıyor; `0x1C` alımda `drop_unknown_frame`.

**Gözlemler (vektör dışı, uyuşmazlık değil):**
- `classify` kontrol öneki olup 21 B'tan kısa datagramı `NotControl` adıyla reddediyor (vektör `truncated` diyor). İkisi de sessiz drop; yalnız ad farklı.
- `wire::ErrorPayload::decode` `detail_len = 0` istiyor (S4 §2.3 "detail_len u16 = 0"). `kama_contracts::s4::ErrorPayload` A2.2 kartına uyarak `detail: Vec<u8>`'ı genel çözüyor. Hub `detail`'i boş yollar; `pub use` geçişinde hub'ın davranışı değişmez. S4 metniyle kart arasındaki bu nüans G-S4'te teyit edilmeli.
- `table::pick_handle` (`table.rs:416`) S4 §2.6 kuralıyla satır satır aynı (her deneme sayacı ilerletir; 0 / aktif / tombstone atlanır; 8 deneme). Sayaç dışarıdan kurulamadığı için `handle-alloc` vakaları `table::handle_candidate` ile yeniden oynatıldı (eşit).

## (e) İlke ve geçiş

**S4 kazanır.** Düzeltme hub-server sahibinin PR'ında yapılır, `crates/hub-server/**`'a bu işte yazılmadı. Önerilen geçiş: `kama_contracts::s4` main'e girdikten sonra (S2-A2.2, #156) `wire.rs` içeriği `pub use kama_contracts::s4::*` (+ hub'a özgü yardımcılar: `classify`, `control_datagram`, `cookie_input`'un sırlı biçimi) olur. U-1…U-4 bu geçişte kendiliğinden kapanır, çünkü `s4.rs` çözücüleri dört kuralı da uyguluyor ve her biri için negatif vektör var. Geçişte adlar değişir (`StoreRequest` → `StoreMessageReq`, `RpcFrame<'a>` → `AppRpcFrame` + `RpcMessage`, `WireError` → `S4Error`); bayt düzeni değişmez (bu belgedeki 93 eşit vaka).

## (f) D-28 tarih isteği

**hub-server sahibine soru:** `wire.rs` düzeltme tarihi? (U-1…U-4, ya da doğrudan `pub use kama_contracts::s4::*` geçişi.) Duman için acil değil: dört uyuşmazlığın hiçbiri hub davranışını etkilemiyor (`H-2 U-n DUMANI ETKİLER` satırı yok).

Rapor satırı: `H-2 LİSTESİ: 4 uyuşmazlık · PR #<n>`.
