# N6 — CONTROL-FRAME ALLOWLIST UYUM-RAPORU [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; kaynak-kod YAZILMADI, cargo koşulmadı). noise-wire-D0-A N6 maddesi (§12).
> **Otorite-spec'ler:** `specs/kama-noise-wire-tdd-v2.0.md` §2.56 (INNER_KIND-registry; codepoint'ler 0x10..0x18) · §2.57 (control-body-layout'ları, exact) · §2.7 (secure-control-format; **KEY_UPDATE-Alpha'da-YOK**). LOCKED-allowlist = 9 control (Ack/Sack/Recovered/BlockComplete/ParamSet/ParamAck/BlockAbort/SessionClose/SessionRenewRequired). Normatif: **NACK-YOK (NACK→SACK), KEY_UPDATE-YOK (KEY_UPDATE→Alpha-dışı).**
> **HÜKÜM (özet):** Yeni **schema-crate `kama-transport-schema`** §2.56/§2.57-allowlist'i **BYTE-DOĞRU uygular** (9 control, codepoint 0x10..0x18 exact; FEC 0x01/0x02; fail-closed) → **UYUMLU**. Mevcut **eski transport-framing** (`ack.rs` frame_type 0x10/0x14, `fec.rs` 0x20, `bench` DATA 0x00) **AYRI-NAMESPACE** (cleartext transport-wire frame_type) — §2.56 inner_kind'e **eşleşmez** (encrypted-inner); sayısal-örtüşme (0x14=ACK_V2↔PARAM_SET, 0x10=ACK_V1↔ACK) **rastlantısal, ihlal-değil ama uyum-da-değil** → D1'de cleartext-transport-frame_type kalkar (§2.57 "cleartext-feedback-YOK"). **NACK: hiç yok ✓. KEY_UPDATE: hiç yok ✓** (yalnız "YOK" beyan-yorumu). Ayrıntı §3-5.

---

## (1) SPEC-DİYOR (§2.56 · §2.57 · §2.7)

### §2.56 — INNER_KIND-REGISTRY [NORMATİF] (decrypt-sonrası-tek-parser-seçimi)
UNIFIED-INNER-HEADER = `inner_version u8(=0x01) · inner_kind u8 · variant_body`. **Codepoint'ler (satır 246-250):**
```
0x01 FEC_SOURCE   0x02 FEC_REPAIR
0x10 ACK          0x11 SACK          0x12 RECOVERED      0x13 BLOCK_COMPLETE
0x14 PARAM_SET    0x15 PARAM_ACK     0x16 BLOCK_ABORT    0x17 SESSION_CLOSE
0x18 SESSION_RENEW_REQUIRED
0x00 reserved(reject) · 0x03..0x0F reserved-FEC-range · 0x19..0xFF reserved(reject)
```
**"Bilinmeyen-inner_kind → FAIL-CLOSED-reject"** (satır 251). "packet_type↔control_type-çakışması-YOK" (tek-birleşik-alan; satır 241-242).

### §2.57 — CONTROL-BODY-LAYOUT'LARI [NORMATİF — exact] (satır 255-287)
Ortak: `UNIFIED-INNER-HEADER(inner_kind∈0x10..0x18) · control_id-u32_be · body`. `control_id`: session+direction-scoped, retry'da-AYNI, yeni-control'da-YENİ (idempotent-dedup). Body'ler:
`ACK(0x10)`: acked_dh_epoch·ack_range_count·[pn_start,pn_end]× · `SACK(0x11)`: acked_dh_epoch·base_pn·gap_ack_block_count·[gap,ack_len]× · `RECOVERED(0x12)`: block_id·esi_range_count·[esi_start,esi_end]× · `BLOCK_COMPLETE(0x13)`: block_id·final_source_count · `PARAM_SET(0x14)`: param_hash[32]·profile_id·code_suite_id · `PARAM_ACK(0x15)`: param_hash[32] · `BLOCK_ABORT(0x16)`: block_id·reason_code · `SESSION_CLOSE(0x17)`: reason_code · `SESSION_RENEW_REQUIRED(0x18)`: reason_code·renew_hint. Kurallar (satır 272-274): count-profile-BOUNDED · oversized/invalid-range/unknown-reason/reserved-nonzero → FAIL-CLOSED · **cleartext-feedback-YOK (hepsi-EXTENDED-AEAD).**

### §2.7 — SECURE-CONTROL-FORMAT [NORMATİF] (satır 312-317)
"Alpha-allowlist = §2.56-codepoint'leri (0x10..0x18); **KEY_UPDATE-Alpha'da-YOK** (in-session-key-phase-yok §5; eski→Beta/SESSION_RENEW_REQUIRED)." unknown-kind/oversized/invalid-range/reserved-nonzero → FAIL-CLOSED.

### §12 N6-kapsam (satır 531-533)
"mevcut-kodda control-frame-VARSA §2.56-codepoint'lere (0x10..0x18) uyuyor-mu? **NACK/KEY_UPDATE-YOK** (NACK→SACK; KEY_UPDATE→Alpha-dışı). Eksik-control'lar-D1'de-§2.57-body'lerine-göre-eklenir."

---

## (2) KOD-YAPIYOR (dosya:satır)

### 2.a §2.56 allowlist — `kama-transport-schema` (YENİ schema-crate, byte-otoritesi)
- `crates/kama-transport-schema/src/inner_kind.rs:13-21` — **9 control-codepoint'i EXACT:** `ACK=0x10 · SACK=0x11 · RECOVERED=0x12 · BLOCK_COMPLETE=0x13 · PARAM_SET=0x14 · PARAM_ACK=0x15 · BLOCK_ABORT=0x16 · SESSION_CLOSE=0x17 · SESSION_RENEW_REQUIRED=0x18`. §2.56 tablosuyla **byte-for-byte-aynı**.
- `inner_kind.rs:8-10` — FEC: `FEC_SOURCE=0x01 · FEC_REPAIR=0x02` (§2.56).
- `inner_kind.rs:12` — yorum: *"Control codepoint'leri (§2.56 allowlist; **KEY_UPDATE/NACK YOK**)."*
- `inner_kind.rs:62-77` — `classify(cp)`: 0x01/0x02→`Fec`, 0x10..0x18→`Control(cp)`, **`other => Err(SchemaError::UnknownInnerKind)`** → **FAIL-CLOSED** (§2.56 satır 251; §2.56 satır 250'nin 0x00/0x03..0x0F/0x19..0xFF reserved-reject'i bu default-kolla kapanır). Doc: *"0x00 reserved, 0x03..0x0F FEC-reserved, 0x19..0xFF reserved."*

### 2.b §2.57 control-body'leri — `kama-transport-schema/src/control.rs`
- `control.rs:11-60` — `enum Control` **9 varyant**, doc-comment'lerinde codepoint+ACK-eliciting-durumu: `Ack{id,acked_dh_epoch,ranges}`(0x10) · `Sack{id,acked_dh_epoch,base_pn,blocks}`(0x11) · `Recovered{id,block_id,esi_ranges}`(0x12) · `BlockComplete{id,block_id,final_source_count}`(0x13) · `ParamSet{id,param_hash[32],profile_id,code_suite_id}`(0x14) · `ParamAck{id,param_hash[32]}`(0x15) · `BlockAbort{id,block_id,reason_code}`(0x16) · `SessionClose{id,reason_code}`(0x17) · `SessionRenewRequired{id,reason_code,renew_hint}`(0x18). Alan-kümeleri §2.57-body'leriyle **birebir** (logical-form; wire-seal noise-wire'da).
- `control.rs:65-78` — `codepoint()`: 9 varyant → `inner_kind::*` sabitleri (tam eşleme). `control.rs:82-94` — `control_id()` ortak-alan (§2.57 idempotent-dedup).
- `control.rs:1-2` — modül-doc: *"Wire-seal + AEAD noise-wire'da; bu modül yalnız logical-form (schema-owned)."* → §2.7 "hepsi-EXTENDED-AEAD" ile hizalı (bu crate cleartext-emit-etmez, yalnız logical-DTO).
- `lib.rs:63-74` — `UnsealedTransportPacket::{FecShard, Control}`: control-varyantı `Control(control::Control)` sarar (§12 UNSEALED-DTO).

### 2.c Eski transport-framing (AYRI-namespace: cleartext transport-wire frame_type)
- `crates/adaptive-transport/src/ack.rs:26` — `FRAME_TYPE_ACK: u8 = 0x10` (**V1**, µs; `:23-25` "yalnız-DECODE-uyumluluk"). `ack.rs:30` — `FRAME_TYPE_ACK_V2: u8 = 0x14` (**V2**, emit; 256µs + ölçüm-eki). Bu `AckFrame` **hem ACK hem SACK** (gap+len range'leri taşır; modül-doc `ack.rs:1`: *"SACK feedback frame"*).
- `crates/adaptive-transport/src/fec.rs:17` — `FRAME_TYPE_FEC_SHARD: u8 = 0x20` (N3: framing-yolu §2.6-EXTENDED'e uymaz).
- `crates/bench/src/kama.rs:32-33` — `FRAME_DATA: u8 = 0x00` (bench-local SESSION_DATA; yorum: *"ACK 0x10 ack.rs'te, FEC_SHARD 0x20 wire'da hiç görünmez"*).
- **NOT (namespace):** bunlar decrypt-**öncesi** cleartext transport-wire frame_type byte'larıdır — §2.56 inner_kind (decrypt-**sonrası** encrypted-inner) ile **farklı katman/uzay**. Sayısal-örtüşmeler (0x10, 0x14) katmanlar-arası **rastlantı**, aynı-uzayda-çakışma değil.

### 2.d NACK / KEY_UPDATE taraması
- **NACK:** `grep -ni nack crates/**/*.rs` → **hiçbir NACK-frame/tip/codepoint YOK.** Tüm eşleşmeler substring: `unacked` (sender.rs:1227), `acked_seqs`/`OnAckReceived` (ack.rs:157, sender.rs:492), `inner_kind.rs:12` beyan-yorumu. ✓
- **KEY_UPDATE:** `grep -ni key_update` → **yalnız** `inner_kind.rs:12` yorum ("KEY_UPDATE/NACK YOK"). Hiçbir key_update/key-phase/rekey frame-codepoint'i YOK. ✓ (§2.7/§5 ile uyumlu.)

---

## (3) HÜKÜM: UYUMLU / UYUMSUZ / YOK — eşleştirme-tablosu

**Mevcut-frame → §2.56-codepoint → hüküm:**

| Mevcut-frame (dosya:satır) | Katman | §2.56-codepoint | Hüküm |
|---|---|---|---|
| `Control::Ack`..`SessionRenewRequired` (control.rs:11-60) | encrypted-inner logical-DTO | 0x10..0x18 (9/9 exact) | **UYUMLU** |
| `inner_kind` 0x10..0x18 sabitleri (inner_kind.rs:13-21) | inner_kind-registry | 0x10..0x18 exact | **UYUMLU** |
| `FEC_SOURCE/FEC_REPAIR` (inner_kind.rs:8-10) | inner_kind | 0x01/0x02 | **UYUMLU** (FEC-range) |
| `classify()` fail-closed (inner_kind.rs:62-77) | parser | §2.56 "unknown→reject" | **UYUMLU** |
| `FRAME_TYPE_ACK=0x10` V1 (ack.rs:26) | cleartext transport frame_type | eşleşme-YOK (ayrı-namespace; 0x10-örtüşme rastlantı) | **UYUMSUZ-katman** (D1'de kalkar) |
| `FRAME_TYPE_ACK_V2=0x14` V2/SACK (ack.rs:30) | cleartext transport frame_type | eşleşme-YOK (0x14=PARAM_SET §2.56 ile örtüşür ama farklı-uzay); semantik→ACK(0x10)/SACK(0x11) | **UYUMSUZ-katman** (D1'de EXTENDED-AEAD-inner'a taşınır) |
| `FRAME_TYPE_FEC_SHARD=0x20` (fec.rs:17) | cleartext transport frame_type | eşleşme-YOK (0x20 aralık-dışı; FEC→inner 0x01/0x02) | **UYUMSUZ** (N3: §2.6-EXTENDED'e taşınır) |
| `FRAME_DATA=0x00` bench (kama.rs:32) | cleartext bench-framing | eşleşme-YOK (SESSION_DATA; control-değil) | **N/A** (control-frame değil) |
| **NACK** | — | — | **YOK ✓** (NACK→SACK; hiç tanımlı-değil) |
| **KEY_UPDATE** | — | — | **YOK ✓** (Alpha-dışı; yalnız beyan-yorumu) |

**NET HÜKÜM:**
1. **§2.56/§2.57 allowlist = UYUMLU (schema-crate).** 9 control byte-doğru, FEC 0x01/0x02 doğru, fail-closed doğru, control-body alan-kümeleri §2.57-exact. NACK-yok ✓, KEY_UPDATE-yok ✓ — allowlist **temiz** (yasak-codepoint sızıntısı YOK).
2. **Eski transport-framing (0x10/0x14/0x20) = katman-uyumsuz, ihlal-değil.** Bunlar decrypt-öncesi cleartext frame_type; §2.56 encrypted-inner-codepoint DEĞİL. Sayısal-örtüşme rastlantısal. Ama §2.57 "**cleartext-feedback-YOK (hepsi-EXTENDED-AEAD)**" gereği, cleartext ACK/SACK/FEC-framing D1'de kalkıp EXTENDED-outer+inner_kind'e taşınır (schema-crate `Control` zaten doğru-hedef).
3. **Mevcut transport'ta 5 control-semantiği fiilen var (ACK+SACK tek AckFrame'de birleşik), 4'ü ise yalnızca schema-logical-form olarak var; wire-seal/emit hiçbiri için yok** (control.rs:1-2: seal noise-wire'da, o crate yok).

---

## (4) KOD-DÜZELTME-PLANI (D1) (spec DEĞİŞMEZ)

> schema-crate allowlist zaten UYUMLU → **düzeltme değil, wire-seal + eksik-control emisyonu**. Eski cleartext transport-framing EXTENDED-AEAD-inner'a taşınır (§2.7 "cleartext-YOK").

- **E1 — control wire-seal (noise-wire crate, §2.7/§2.8):** `Control` (control.rs) → UNIFIED-INNER-HEADER(`inner_version=0x01·inner_kind·control_id·body`) → EXTENDED-outer(26B)+per-packet-AEAD. Emit/parse simetrisi + §2.57-body byte-layout + count-BOUNDED enforce. schema-crate `codepoint()`/`classify()` bu seal'in tek-otoritesi.
- **E2 — cleartext ACK/SACK'i EXTENDED-AEAD'e taşı:** `ack.rs::AckFrame` (0x10/0x14 cleartext) → §2.57 `ACK(0x10)`+`SACK(0x11)` inner_kind'lerine ayrıştırılıp AEAD-encrypted-inner olur; `FRAME_TYPE_ACK/_V2` transport-frame_type-byte'ları kalkar. `acked_dh_epoch` (§2.57) mevcut `ack_epoch:u8`'in (transport-wrap-epoch, N1) yerine kripto-dh_epoch'a bağlanır.
- **E3 — FEC-framing → §2.6-EXTENDED (N3-E1..E9 ile ORTAK):** `fec.rs` 0x20-framing kalkar; FEC inner_kind 0x01/0x02 encrypted-inner'da (`classify`→`Fec(ShardKind)` zaten hazır).
- **E4 — eksik-control body-parser'ları (§2.57, exact):** her control için wire encode/decode + golden-vector; oversized/invalid-range/unknown-reason/reserved-nonzero → FAIL-CLOSED (schema `SchemaError`).
- **E5 — control_id idempotent-dedup + retry-state-machine (§2.57 satır 275-287):** PARAM_SET→PARAM_ACK-retry; BLOCK_ABORT/BLOCK_COMPLETE/SESSION_CLOSE/SESSION_RENEW_REQUIRED→network-ACK-kapsanana-dek-retry; ACK/SACK/RECOVERED→retry-EDİLMEZ; SESSION_CLOSE tombstone-grace (N5-E6 ile ortak).

---

## (5) D1-EKSİKLERİ

- **[D1-N6-1] control wire-seal/emit YOK:** `Control` logical-form var (control.rs) ama EXTENDED-outer+AEAD-inner encode/decode **hiç yok** (noise-wire crate yok). 9 control'un HİÇBİRİ wire'a serialize-edilemiyor.
- **[D1-N6-2] §2.57 body wire-codec YOK:** control_id-u32_be + per-control body byte-layout + count-BOUNDED + fail-closed parser yazılacak (logical→wire).
- **[D1-N6-3] cleartext ACK/SACK/FEC-framing kaldırma:** `ack.rs` 0x10/0x14 + `fec.rs` 0x20 transport-frame_type'ları EXTENDED-AEAD-inner'a taşınacak (§2.57 "cleartext-YOK"). Mevcut birleşik `AckFrame` → ACK(0x10)+SACK(0x11) ayrımı.
- **[D1-N6-4] control_id retry/idempotent-state-machine (§2.57):** dedup + retry-sınıflandırması + SESSION_CLOSE-tombstone; kod-tabanında YOK.
- **[D1-N6-5] `acked_dh_epoch` bağı:** §2.57 ACK/SACK `acked_dh_epoch u8` kripto-dh_epoch'a; mevcut `ack_epoch:u8` transport-wrap-epoch (N1-bulgusu) — ayrı-bağlama.

### §2.56/§2.57 allowlist kapsama-özeti
| Control (§2.56) | codepoint | schema logical (control.rs) | wire-seal/emit |
|---|---|---|---|
| ACK | 0x10 | ✅ (+ eski cleartext AckFrame semantik) | ⛔ D1 |
| SACK | 0x11 | ✅ (eski AckFrame ACK+SACK birleşik) | ⛔ D1 |
| RECOVERED | 0x12 | ✅ | ⛔ D1 |
| BLOCK_COMPLETE | 0x13 | ✅ | ⛔ D1 |
| PARAM_SET | 0x14 | ✅ | ⛔ D1 |
| PARAM_ACK | 0x15 | ✅ | ⛔ D1 |
| BLOCK_ABORT | 0x16 | ✅ | ⛔ D1 |
| SESSION_CLOSE | 0x17 | ✅ | ⛔ D1 |
| SESSION_RENEW_REQUIRED | 0x18 | ✅ | ⛔ D1 |
| **NACK** | — | ❌ yok (doğru) | — |
| **KEY_UPDATE** | — | ❌ yok (doğru) | — |

**DR-ADAYI:** Yok. schema-crate §2.56/§2.57 ile byte-for-byte uyumlu; eski transport-frame_type'lar farklı-namespace (spec-hatası değil, D1-kod-taşıması). KURAL-2: kod-sonra-düzeltilir, spec değişmez.

**Cross-ref:** N3 (FEC-framing 0x20 → §2.6-EXTENDED-inner 0x01/0x02; E3 ortak) · N5 (control-admission: SESSION_CLOSE-tombstone + control_id-idempotent, E5/N5-E6; tüm control EXTENDED-AEAD = verify-before-admission kapsamında) · N1 (`ack_epoch` transport-wrap ≠ §2.57 `acked_dh_epoch` kripto-epoch; E5/D1-N6-5).
