# N5 — VERIFY-BEFORE-ADMISSION UYUM-RAPORU [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; kaynak-kod YAZILMADI, cargo koşulmadı). noise-wire-D0-A N5 maddesi (§12).
> **Otorite-spec'ler:** `specs/kama-noise-wire-tdd-v2.0.md` §6 (verify-before-admission SAHİPLİK) · §4 (accept() AEAD-gate + ORDER + "unauth-paket replay-window-İLERLETMEZ") · §7 (per-shard-AEAD) · §2.8 (AAD). Çekirdek-invariant: **CLAUDE.md INV-FEC-1 = authenticate-BEFORE-decode** (AEAD-verify → SONRA FEC-decode; AEAD-fail-eden-paket replay-window'u İLERLETMEZ + FEC-decode'a GİRMEZ).
> **HÜKÜM (özet):** verify/AEAD-open/admission katmanı **YOK** (crypto crate 3-satır iskelet; `grep verify/open/admit` = kripto-kodu-SIFIR). authenticate-before-decode invariantı bugün **İHLAL EDİLMİYOR** — ama **doğru-uygulandığı-için-değil**: (a) hiç AEAD-verify yok, (b) FEC-decode yolu (`kama-fec-codec::reconstruct`) **çağrılmıyor (ÖLÜ; call-site-YOK)**, (c) receiver `RxKind::FecShard`'ı decode-ETMEZ, yalnız buffer+ACK'ler. Yani decode "yanlış-erişilebilir" değil, **henüz-hiç-bağlı-değil**. verify-before-admission seam'i receiver.rs'in DIŞINDA (henüz-var-olmayan noise-wire crate'inde) oturacak; receiver zaten VERIFIED `RxDatagram` bekler (kontratla). **Tüm admission-çekirdeği (AEAD-gate, replay-freeze, sealed-Opened-tip) = D1-EKSİĞİ.** Ayrıntı §3-5.

---

## (1) SPEC-DİYOR (§refs + normatif-alıntı)

### §6 — VERIFY-BEFORE-ADMISSION [NORMATİF] (SoC-analiz-§8; SAHİPLİK bu-katmanda)
- **"AEAD-doğrulanmamış-shard FEC-decoder'a-KABUL-EDİLMEZ" — sahibi FEC-codec-DEĞİL, bu-profil** (satır 414). Sahiplik-bölüşümü (satır 415-418):
  ```
  KamaNoise:      AEAD-verify/open · kripto-replay-check · inner-envelope-decrypt · minimum-parse/bounds
  Transport/FEC:  opened-authenticated-shard-admission · logical-block/profile-validation · decoder-admission
  ```
- "verify-before-FEC-admission FEC-codec'in-kripto-özelliği-DEĞİL, **bu-profilin-zorunlu-admission-kuralı**" (satır 420).
- **TYPE-LEVEL-ENFORCEMENT [NORMATİF] (analiz-P0-5 — call-site-disiplini-YETMEZ, satır 421-426):** Opened/Authenticated-tip **sealed** = alanlar-PRIVATE + `_proof: PrivateAdmissionProof` (public-constructor-YOK) + YALNIZ `KamaNoiseWireProtector::open()` üretir + `#[cfg(test)]`-constructor-ayrı. Opened/Authenticated-tip **noise-wire-crate'inde** (schema yalnız UNSEALED-DTO). "Şu-an-başka-crate-üretebilir-mi" → **kod-henüz-yok; bu-kural D1-tip-tasarımının-ZORUNLU-şartı (K-WIRE-SEALED-TYPE-1)**.

### §4 — accept() AEAD-GATE + ORDER [NORMATİF — KİLİTLİ] (cross-ref N1)
- `accept()` (satır 366): `if AEAD_open(candidate, dh_epoch_wire) fails: REJECT # candidate=dh_epoch_wire-cipher-state-nonce; window-İLERLEMEZ`. SUCCESS → COMMIT (satır 367-368): `window_mark_seen; if candidate>=ew: expected_pn=candidate+1`.
- **8-adım ORDER (satır 387-389):** (1)outer-parse (2)handle→layout (3)candidate (4)too-old/dup/future-precheck **window-İLERLETMEDEN** (5)**dh_epoch-cipher-state-AEAD-attempt** (6)success→COMMIT (7)inner-parse+bounds (8)OpenedPacket-handoff. **"Unauthenticated-paket replay-window-İLERLETMEZ."**

### §7 — PER-SHARD-AEAD [NORMATİF] · §2.8 — AAD [NORMATİF]
- receiver **yalnız-auth/decrypt-shard'ı-decoder'a** (satır 432-433). Block-tek-tag → "forged-shard-decoder'a-girer + decode-DoS + block-sonuna-kadar-unauthenticated-buffering" (satır 430) → YASAK.
- AEAD-open nonce = full-PN-reconstruction (§2.8, satır 325-326); **replay-window-YALNIZ-AEAD-success-sonrası-commit** (satır 326). AAD = `"KAMA-WIRE-EXT-V1"‖h_ext‖wire_format‖wire_flags‖pn_trunc` (§2.8, satır 321).

### §12 N5-kapsam
- "**verify-before-admission mevcut-Noise-open-yolunda-nerede? AEAD-fail-davranışı?**" (satır 530).

---

## (2) KOD-YAPIYOR (dosya:satır)

### 2.a AEAD-verify / open / admit çekirdeği — **YOK (iskelet)**
- `crates/crypto/src/lib.rs:1-3` — **3 satır**, yalnız doc-comment: *"Noise + compact crypto layer … API-first iskelet; trait imzaları sonraki iş."* Hiçbir `open`/`verify`/`admit`/`AEAD` fonksiyonu yok.
- **grep tüm `crates/**/*.rs`:** `nack`=0 (yalnız substring: "unacked"/"acked_seqs"); kripto-anlamında `verify`/`aead_open`/`.open()` = **transport/kripto-katmanında SIFIR** (yalnız yorumlar + bench/quic.rs'in TLS-`SkipVerify`'ı — alakasız). `KamaNoiseWireProtector` / `PrivateAdmissionProof` / `OpenedTransportShard` **hiçbir yerde YOK.**
- §4-`accept()` (AEAD-gate, too-old/dup/future-precheck, replay-window, commit-on-success, unauth-window-freeze) → **hiç uygulanmıyor** (N1-bulgusuyla aynı: replay-window/bitmap/dh_epoch çekirdeği yok).

### 2.b verify-before-admission SEAM'İ nerede OTURACAK — beyanlı, uygulanmamış
- `crates/adaptive-transport/src/receiver.rs:4-7` (modül-doc) — seam-beyanı: *"güvenlik katmanı (Noise+kompakt, §0.2) datagram'ı **çözer/verify eder**; Receiver **VERIFIED** opak birim + metadata ALIR. **verify FAIL → güvenlik katmanı silent drop (transport hiç görmez).** … per-shard AEAD + RS-decode GÜVENLİK katmanında (authenticate-before-decode, INV-FEC-1)."*
- `receiver.rs:51` — `RxDatagram` doc: *"**VERIFIED** gelen datagram (güvenlik katmanı çözdü/verify etti; §0.2)."* → tip-adı niyeti taşır ama **seal/proof YOK** (plain `pub struct`, `pub` alanlar `:53-59`; herkes üretir). §6-TYPE-LEVEL-ENFORCEMENT karşılanmıyor.
- **Seam-konumu:** verify receiver'ın DIŞINDA (noise-wire crate'inde) olacak → receiver zaten-verified datagram alır. Bu SoC beyanı §6-bölüşümüyle hizalı; ama üstteki verify **kod-olarak-yok**.

### 2.c Mevcut receive-yolu: parse→muhasebe arasında verify-gate YOK
- `crates/bench/src/kama.rs:319-328` — `decode_data(&buf[..n])` (cleartext bench-framing parse) → `resolve_seq` → **doğrudan** `rx.on_datagram(RxDatagram{seq, unit_id, kind:Data}, now)`. Parse ile muhasebe-ilerlemesi arasında **hiçbir AEAD/verify adımı yok** (bench'te kripto-yok; ama seam'in tam bu boşlukta oturacağını gösterir).
- `receiver.rs:183` `on_datagram` → `:213` dup-check → `:223` `record_seq` → `:380-393` `received.insert` + `largest_seq` güncelleme. **Muhasebe (rx-set/largest ilerleme) verify-gate'siz akar** — kontrat gereği girdi zaten-VERIFIED sayıldığından bugün doğru; ama bu invariantı GARANTİLEYEN üst-verify yok. §4-"unauth-paket-window-İLERLETMEZ" analoğu (unauth-paket-muhasebe-İLERLETMEZ) yapısal-olarak henüz-korunmuyor.

### 2.d FEC-decode yolu — ERİŞİLEBİLİR-DEĞİL (ÖLÜ; verify-siz-decode fiilen imkânsız)
- `crates/adaptive-transport/src/receiver.rs:225-232` — `RxKind::FecShard { .. } => (Vec::new(), true)`: shard **buffer'lanır + ACK'lenir, decode-EDİLMEZ**. Yorum (`:228-229`): *"RS-decode + reconstruction GÜVENLİK katmanında (authenticate-before-decode, INV-FEC-1). Transport birimi burada teslim ETMEZ."* → transport decode-yolunu açmıyor.
- `crates/kama-fec-codec/src/codec.rs:127-219` — `RsSimdCodec::reconstruct()` = saf RS-decode. **grep: call-site YOK** (yalnız `lib.rs:14` re-export + trait-tanımı) → bugün **hiç çağrılmıyor = ölü-yol**. Dolayısıyla "verify-siz-decode" fiilen KOŞAMAZ (çağıran yok).
- `crates/kama-fec-codec/src/codec.rs:22-28` — `ReceivedSymbol{ esi, is_repair, bytes }` = **plain public struct, sealed-DEĞİL**. `reconstruct` girdisi authenticated-proof TAŞIMAZ. Modül-doc (`codec.rs:1-2`): *"OPAQUE-symbol API — Noise/AEAD/nonce/key TİPLERİ SIZMAZ."* → SoC-doğru ama §6-TYPE-LEVEL-seal YOK: bağlandığında tip-düzeyinde unverified-byte'ı `reconstruct`'a beslemeyi engelleyen bir şey yok (D1-riski, §6 K-WIRE-SEALED-TYPE-1).
- `crates/adaptive-transport/src/fec.rs:5-7,:39-41,:82-85` — `opaque_secured` (ct‖tag) transport için TEK-OPAK-BLOB; **ASLA parse/verify ETMEZ**, kripto-içermez. (N3-bulgusu: bu framing-yolu ayrıca §2.6-EXTENDED'e yapısal-uymuyor.)

### 2.e AEAD/parse-fail davranışı — bugün ne var
- **AEAD-fail:** N/A — AEAD-katmanı yok (§4-accept AEAD-gate uygulanmamış).
- **Parse-fail (yalnız cleartext transport-framing):** `ack.rs:222-251` / `fec.rs:67-99` decode → typed `TransportError` (`Truncated`/`TrailingBytes`/`UnknownFrameType`/`InvalidShardCount`) döner, `error.rs:9-43`; `read.rs:23-33` bounds-before-read (PARSER-02). **Fail-closed-niyet doğru** ama bunlar **post-AEAD-değil**: güvenlik-katmanı AEAD-fail + §4-replay-freeze + §6-inner-parse-after-COMMIT **yok**.

---

## (3) HÜKÜM: UYUMLU / UYUMSUZ / YOK

| Soru (N5) | Bulgu | Hüküm |
|---|---|---|
| verify-before-admission mevcut-open-yolunda **nerede**? | Open-yolu-YOK (crypto 3-satır iskelet); seam receiver.rs:4-7/:51'de **beyanlı** ama uygulanmamış | **YOK → D1-eksiği** |
| AEAD-fail davranışı (§4 REJECT + window-freeze)? | AEAD-katmanı yok; §4-`accept()` hiç uygulanmamış | **YOK → D1-eksiği** |
| authenticate-before-decode (INV-FEC-1) bugün **ihlal mi**? | AEAD-verify yok + FEC-`reconstruct` **çağrılmıyor (ölü)** + receiver FecShard'ı decode-etmez | **İHLAL-YOK** (ama "doğru-uygulandığı-için"-değil; **henüz-bağlı-değil**) |
| FEC-decode yolu verify-siz **erişilebilir mi**? | `reconstruct` call-site-YOK → bugün erişilemez; `ReceivedSymbol` sealed-değil → bağlandığında tip-düzeyi engel yok | **ŞU-AN-ERİŞİLEMEZ; D1-tip-seal-riski** |
| unauth-paket muhasebe/replay-window İLERLETİR mi? | replay-window yok (N1); muhasebe (`record_seq`) verify-gate'siz ama girdi kontratla-VERIFIED | **N/A bugün → §4/§6-gate D1-eksiği** |
| §6 sealed Opened-tip (PrivateAdmissionProof)? | Hiç yok; `RxDatagram`/`ReceivedSymbol` plain-public | **YOK → D1-eksiği** |

**NET HÜKÜM: YOK (verify/admission çekirdeği kod-olarak mevcut-değil).**
1. verify-before-admission, AEAD-open, §4-accept-gate, sealed-Opened-tip: **hiçbiri kodda yok** → tümü D1-eksiği.
2. authenticate-before-decode invariantı **bugün ihlal edilmiyor**, çünkü FEC-decode-yolu bench+receiver'da **hiç tetiklenmiyor** (N3: FecParity emit-yok; receiver: shard buffer-only; fec-codec: `reconstruct` call-site-yok). Bu "yanlış-erişilebilir-decode" DEĞİL — **erişilebilir-değil-henüz**. N3'ün "framing-yapısal-uyumsuz" bulgusuyla birleşince: FEC-yolu D1'de baştan §2.6-EXTENDED + §6-admission ile kurulacak.
3. **Seam-haritası (D1-yerleşimi kesin):** üst-verify **noise-wire crate**'inde (`KamaNoiseWireProtector::open()`), receiver.rs:4-7/:51'in "VERIFIED-RxDatagram" kontratını GERÇEKTEN sağlayacak yer. Bugün `bench/kama.rs:319-328`'deki parse→`on_datagram` boşluğu, D1'de bu boşluğa AEAD-gate-open oturacak noktadır.

---

## (4) KOD-DÜZELTME-PLANI (D1) (spec DEĞİŞMEZ)

> "UYUMSUZ-düzeltme" değil, **eksik-admission-katmanı-yazımı**. noise-wire §4/§6/§7 aynen uygulanır; mevcut receiver SoC-beyanları (receiver.rs:4-7) korunur — altlarındaki verify GERÇEKLENİR.

- **E1 — `KamaNoiseWireProtector::open()` (noise-wire crate, §6):** outer-parse → §4-8-adım-ORDER: candidate → too-old/dup/future-precheck (window-İLERLETMEDEN) → **AEAD_open (§4 satır 366)** → success→COMMIT → inner-parse+bounds → `OpenedTransportShard` handoff. AEAD-fail → REJECT + **replay-window-İLERLEMEZ** + silent-drop (receiver.rs:5-6 kontratını sağlar).
- **E2 — sealed Opened-tip (§6 TYPE-LEVEL, K-WIRE-SEALED-TYPE-1):** `OpenedTransportShard`/`AuthenticatedPacket` = PRIVATE-alanlar + `_proof: PrivateAdmissionProof` + yalnız `open()` üretir + `#[cfg(test)]`-constructor-ayrı. `receiver::RxDatagram` (plain-public, receiver.rs:51-60) ve `kama-fec-codec::ReceivedSymbol` (codec.rs:22-28) bu sealed-tipten TÜRETİLİR → call-site-disiplini yerine tip-düzeyi-zorlama.
- **E3 — §4 accept()/anti-replay (N1-E-D1-2 ile ORTAK):** `expected_pn[dh_epoch]` + `replay_window[dh_epoch]` bitmap (WINDOW=2^14), too-old/dup/future-gap(2^13), commit-yalnız-AEAD-success. **AEAD-open bu accept()'in içinde** (verify-before-window-commit).
- **E4 — decoder-admission (Transport/FEC tarafı, §6):** yalnız `OpenedTransportShard`→`ReceivedSymbol` dönüşümü FEC-decode'a girer; `kama-fec-codec::reconstruct` (codec.rs:127) YALNIZ sealed-kaynaktan beslenir. receiver.rs:225-232 buffer-yolu, opened-shard'ı decoder'a admit-eden yola bağlanır (per-shard-AEAD-verify SONRA, §7).
- **E5 — §2.8 AAD + nonce:** open() AAD'yi (`"KAMA-WIRE-EXT-V1"‖h_ext‖wf‖flags‖pn`) üretir; nonce = full-PN-reconstruction (N1-E-D1-1). golden-vector-AAD-byte-assert.
- **E6 — SESSION_CLOSE tombstone-admission (§2.57 satır 285):** session-state-silinse-de gelen CLOSE-retry'larına bounded-tombstone ACK üretebilme (control-idempotent admission; N6 ile ortak).

---

## (5) D1-EKSİKLERİ

- **[D1-N5-1] AEAD-open/verify çekirdeği (KRİTİK):** `crypto` crate 3-satır iskelet; `open()`/AEAD-verify/inner-decrypt kodu **hiç yok**. Bu tüm N5'in ana-eksiği.
- **[D1-N5-2] §4 accept() AEAD-gate + replay-freeze:** "AEAD-fail → REJECT, window-İLERLEMEZ" + 8-adım-ORDER uygulanmamış (N1-D1-2 ile ortak; replay-window/bitmap/dh_epoch yok).
- **[D1-N5-3] §6 sealed Opened-tip (PrivateAdmissionProof):** `RxDatagram`/`ReceivedSymbol` plain-public → bağlandığında tip-düzeyi verify-before-decode engeli YOK. K-WIRE-SEALED-TYPE-1 D1-tip-tasarımının zorunlu-şartı.
- **[D1-N5-4] decoder-admission bağlama:** `kama-fec-codec::reconstruct` bugün ölü (call-site-yok); D1'de YALNIZ opened-authenticated-shard'tan beslenecek şekilde receiver-buffer-yoluna (receiver.rs:225-232) + §7-per-shard-AEAD sonrası bağlanır. (N3-E-D1: FEC-framing baştan §2.6-EXTENDED'e taşınır.)
- **[D1-N5-5] parse→muhasebe boşluğuna gate:** bugün `bench/kama.rs:319-328` parse→`on_datagram` arası verify-siz; D1'de bu nokta noise-wire-open() ile kapatılır (unauth-paket muhasebe-İLERLETMEZ).
- **[D1-N5-6] AAD/nonce türetimi:** §2.8-AAD + full-PN-nonce open()-içinde (N1-D1-1/7 ile ortak).

**DR-ADAYI:** Yok. §6 sahiplik-bölüşümü ile §4-ORDER (KamaNoise-verify/parse → Transport/FEC-admission) iç-tutarlı; §2.56/§2.57 ile çakışma yok. Tüm bulgular **kod-eksiği** (KURAL-2: kod-sonra-yazılır), spec-hatası değil.

**Cross-ref:** N1 (§4 accept()/replay-window/nonce eksik — bu raporun AEAD-gate'iyle aynı çekirdek) · N3 (FEC-framing §2.6-uyumsuz + `reconstruct` ölü-yol; verify-before-decode seam'i D1'de FEC-yolunu baştan kurarken oturur) · N6 (control-admission: SESSION_CLOSE tombstone + control_id-idempotent, E6).
