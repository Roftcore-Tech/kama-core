# N9 — ALPHA KEY-LIFECYCLE UYUM-RAPORU (wire-key-phase · cap→session · cross-session-red) [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; kod YAZILMADI). noise-wire-D0-A N9 maddesi (§12; karar-VERİLDİ §5).
> **Kapsam (§12/§5):** (a) mevcut-kodda **wire-visible-key-phase/rekey-state-machine** var-mı (varsa-Alpha-yolundan-
> kaldırılır)? (b) **cap-tetikleri yeni-session/handshake** açıyor-mu (in-session-rekey-yerine)? (c) **eski-session-
> packet'ı yeni-session'a KABUL-EDİLMİYOR-mu** (cross-session anti-replay/izolasyon)?
> **Otorite-spec:** `specs/kama-noise-wire-tdd-v2.0.md` §5 (session-renewal, KİLİTLİ) · §2.56 (inner_kind-allowlist) ·
> §2.57 (SESSION_RENEW_REQUIRED-body) · §4 (WRAP + accept()) · §0.7 (RESTART-politikası) · §11 K-WIRE-KEYUPD-1.
> **HÜKÜM (özet):** **YOK (uygulanmamış).** Karar-uyumlu-parçalar yalnız **inert-schema** olarak var
> (SESSION_RENEW_REQUIRED-DTO/codepoint doğru; allowlist'te KEY_UPDATE-YOK) — **zorlayıcı-mekanizmalar (cap-tespiti→
> emit→re-handshake, `session_id`, cross-session-red) tümüyle YOK.** İHLAL yok (wire-key-phase-machine mevcut-değil),
> ama (b)+(c) inşa-edilmemiş. `crypto`/session-kodu iskelet. **DR-ADAYI: yok** (spec-tutarlı; farklar KOD-tarafı).
> Split/dh_epoch-0→1-modeli + rekey-terminoloji-sızıntısı = **bkz. N2.**

---

## 1. SPEC-DİYOR

### §5 — cap→new-session [NORMATİF — KİLİTLİ] (satır 398-403)
```
3. in-session-symmetric-key-phase-state-machine                → ALPHA'DA-YOK
4. cap→new-session : packet/byte/time-cap-dolunca SESSION_RENEW_REQUIRED → tamamen-YENİ-handshake/session
```
"Cap/wrap-öncesi session-renewal-**ZORUNLU** (ID-scope §2.6)." Terminoloji: yalnız `dh_epoch (0/1)` (satır 400).

### §2.56 — INNER_KIND allowlist (satır 244-251)
Control-codepoint'leri `0x10..0x18`; **KEY_UPDATE-YOK** (Alpha, §2.7 satır 315). `0x18 SESSION_RENEW_REQUIRED`
mevcut. Bilinmeyen-inner_kind → **FAIL-CLOSED-reject.**

### §2.57 — SESSION_RENEW_REQUIRED body (satır 270)
`SESSION_RENEW_REQUIRED(0x18): reason_code u8 · renew_hint u8 (cap/wrap-yaklaşımı; yeni-handshake-tetiği §5)`.
Retry-kuralı (satır 279-282): kendi-wire-PN'i network-ACK'lenene/deadline'a-kadar-retry; control_id-idempotent.

### §4 — WRAP + accept() (satır 360-390)
"**WRAP:** full-PN-u64-tükenmez; cap/wrap-yaklaşımında **SESSION_RENEW_REQUIRED → yeni-handshake (§5).**" (satır 373).
`accept()` — epoch-membership + too-old/dup/future-gap + AEAD-gate + commit-on-success (bkz. N1-D1-eksiği).
**RESTART (satır 385-386):** process-restart=yeni-session; full-PN-persist-EDİLMEZ; nonce-reuse-yapısal-imkansız.

### §0.7 / §0.7-EARLY-DATA — RESTART + cross-session-red (satır 127-133)
"eski-session-datagramları **yeni-handle/session_id-ile-admission-ALAMAZ**" · "receiver peer/device-başına bounded-
PERSISTENT-anti-replay (high-water/bitmap)" · "aynı-(sender_device_id, recipient_device_id, init_counter)-İKİNCİ-app-
side-effect-ÜRETMEZ" · revoked/eski-cert-replay → cert-doğrulama-reddeder.

### §11 — K-WIRE-KEYUPD-1 (satır 493-494) + §2.55 unknown-handle (satır 233)
"Alpha-KARAR (§5): in-session-key-phase-YOK; cap (packet/byte/time) → tamamen-YENİ-handshake/session. **Test:
cap-tetiği-deterministik + eski-session-datagramı-yeni-session'a-admission-EDİLMEZ.**" §2.55: farklı-session-hub-
collision-kontrolü (ccr §35.7-unknown-handle → cheap-drop).

### N9-görev-cümlesi (§12 satır 538-540)
"wire-visible-key-phase/rekey-state-machine-var-mı (varsa-kaldırılır)? cap-tetikleri-yeni-session/handshake-açıyor-mu?
eski-session-packet'ı-yeni-session'a-kabul-EDİLMİYOR-mu?"

---

## 2. KOD-YAPIYOR (dosya:satır)

### 2.a (a) Wire-visible key-phase / rekey STATE-MACHINE — **YOK (doğru-şekilde-yok)**
- **KEY_UPDATE codepoint YOK:** `crates/kama-transport-schema/src/inner_kind.rs:12` yorum *"(§2.56 allowlist;
  **KEY_UPDATE/NACK YOK**)"*; allowlist sabitleri `:13-21` = `ACK..SESSION_RENEW_REQUIRED` (0x10..0x18). `classify()`
  `:62-76` bunları-kabul, diğerini `SchemaError::UnknownInnerKind` → **FAIL-CLOSED** (§2.56-uyumlu).
- **key_phase / rekey_epoch alanı YOK:** grep `key_phase`/`rekey_epoch`/`KeyUpdate` → SIFIR-eşleşme. Hiçbir wire-
  struct'ta key-phase-bit'i / current-next-previous-key state'i yok.
- **Wire-epoch-seçici yalnız DATA-alanı:** `dh_epoch` grep → yalnız `control.rs:15,21` `acked_dh_epoch:u8` (ACK/SACK
  logical-DTO'su). §4-outer `wire_flags.bit0=dh_epoch` seçicisi + EXTENDED-outer-header **kod-değil** — yalnız
  `EXTENDED_OUTER_OVERHEAD=26` sabiti (`lib.rs:41`). Transport `ack_epoch:u8` (`ack.rs:51`) = kalıcı-0 seq-wrap-
  placeholder, key-phase DEĞİL (detay N2 §2.e). → **kaldırılacak wire-key-phase-machine mevcut-değil.**

### 2.b (b) cap-tetiği → yeni-session/handshake — **YOK (yalnız-inert-DTO)**
- **SESSION_RENEW_REQUIRED yalnız logical-DTO + codepoint:** `control.rs:54-59`
  `Control::SessionRenewRequired { id, reason_code, renew_hint }` + `inner_kind.rs:21` `SESSION_RENEW_REQUIRED=0x18`.
  §2.57-body-şekli **doğru** (reason_code + renew_hint). `codepoint()` `control.rs:76` ve `control_id()` `:92` eşler.
- **AMA hiçbir TETİK yok:** grep — packet/byte/time-**cap-sayacı-yok**, cap-eşiğiyle-karşılaştırma-yok,
  `SessionRenewRequired`-**üreten-kod-yok** (yalnız enum-tanımı), re-handshake/session-teardown-yok. Trigger→renewal
  yolu **kablosuz** (schema-only). `crates/crypto/src/lib.rs:1-3` iskelet → handshake-tetikleyici-taban-yok.
- **Tek cap/wrap-imâsı yanlış-sözcükle:** `fec.rs:29` `block_id` doc *"wrap→**rekey**"* — arkasında-kod-yok +
  terminoloji-yanlış (§5: wrap→session-renewal). `block_id` u32 (`lib.rs:27` `BlockId=u32`, "NO-REUSE"-yalnız-doc);
  wrap/cap-guard'ı kodda-yok. (Terminoloji-düzeltmesi = N2-E1.)

### 2.c (c) Eski-session-packet'ı yeni-session'a-red (cross-session izolasyon) — **YOK**
- **`session_id` HİÇ-YOK:** grep `session_id`/`SessionId`/`session_attempt`/`new_session` → SIFIR-eşleşme. Cross-
  session-red için gereken **session-kimliği-yüzeyi-yok** → üzerinde-red-yapılacak-eksen-yok.
- **open()/admission + anti-replay accept() YOK:** grep `replay`/`bitmap` çekirdek-yok (bkz. N1 §2.g). §4-`accept()`
  (too-old/dup/future/AEAD-gate/commit) + §0.7-persistent-anti-replay (high-water/bitmap) + unknown-handle→cheap-drop
  (§2.55/ccr §35.7) — hiçbiri-kodda-yok. `crypto` iskelet → open-yolu-mevcut-değil.
- **RESTART=yeni-session (§0.7/§4):** kod-yok — persist/no-persist-kararı, restart-sonrası-eski-datagram-red-yok.
- **Sonuç:** cross-session-izolasyon **inşa-edilmemiş** (D1). İhlal-değil ama zorlayıcı-mekanizma-yok.

---

## 3. HÜKÜM: UYUMLU / UYUMSUZ / YOK

**HÜKÜM: YOK (uygulanmamış).** Karar-uyumlu-parçalar inert-schema; zorlayıcı-mekanizmalar tümüyle-eksik.

| N9 alt-sorusu | Bulgu | Hüküm |
|---|---|---|
| (a) wire-visible key-phase/rekey **state-machine** var-mı? | Yok; KEY_UPDATE allowlist-dışı (`inner_kind.rs:12`); key_phase-state=0 | **YOK — ve doğru-şekilde-yok** (kaldırılacak-machine-mevcut-değil) |
| (b) cap-tetiği yeni-session/handshake açıyor-mu? | SESSION_RENEW_REQUIRED yalnız-DTO/codepoint (`control.rs:54-59`,`inner_kind.rs:21`); cap-sayacı/üretici/re-handshake YOK | **YOK — D1** (schema-only, kablosuz) |
| (c) eski-session-packet'ı yeni-session'a red-mi? | `session_id`-yok (grep=0); accept()/persistent-anti-replay/unknown-handle-drop YOK | **YOK — D1** (izolasyon-ekseni-yok) |
| KEY_UPDATE control'da-mı? | Değil (`inner_kind.rs:12`; allowlist 0x10..0x18) | **UYUMLU** (§2.56/§5) |
| SESSION_RENEW_REQUIRED body §2.57'ye-uyuyor-mu? | `reason_code`+`renew_hint` (`control.rs:55-58`) — şekil-doğru | **UYUMLU-schema** (üretici-yok) |

**NET:** Wire-görünür-key-phase/rekey-machine **hiç-yok** (İHLAL-yok; kaldırılacak-şey-yok). cap→yeni-session (b) ve
eski→yeni-session-red (c) **karar-uyumlu-ama-uygulanmamış** — SESSION_RENEW_REQUIRED bir-veri-yapısı olarak-var, hiçbir
tetiğe/handshake'e/izolasyona bağlı-değil; `session_id`+open()-admission tümüyle D1. K-WIRE-KEYUPD-1-testi (cap-
deterministik + eski-datagram-red) bugün-yazılamaz.

---

## 4. KOD-DÜZELTME-PLANI (D1)

> Bunlar UYUMSUZ-düzeltme değil, **eksik-mekanizma-yazımı** (schema-DTO'lar hazır; tetik/session/izolasyon eklenecek). Spec-DEĞİŞMEZ.

- **E1 — `session_id` + session-state (kama-noise-wire/engine):** SECURE-WIRE-IDENTITY `(session_id, direction,
  dh_epoch, full_pn)` (§3). Cross-session-red bunsuz-imkansız. Şu-an grep=0. (N1-D1-5 / N2-D1-N2-4 ile-örtüşür.)
- **E2 — cap-tespiti + emisyon:** packet/byte/time-sayaçları vs cap-eşikleri; yaklaşımda `Control::SessionRenewRequired`
  **üret** (DTO `control.rs:54` hazır) + §2.57-retry (control_id-sabit, network-ACK/deadline'a-kadar). §2.6/§4-WRAP:
  cap/wrap-öncesi session-renewal-ZORUNLU. `block_id`/`message_id`/full-PN-wrap-guard'ları ekle.
- **E3 — re-handshake orkestrasyonu:** SESSION_RENEW_REQUIRED → mevcut-session-teardown → **yeni-handshake + yeni-
  session_id + yeni-h_legacy/h_ext** (§2.55 HANDLE_ASSIGN). In-session-rekey-YOK (§5-(3)). Kod-tabanında-hiç-yok.
- **E4 — cross-session izolasyon (open()/admission):** eski-session-datagramı (eski-handle/session_id) → unknown-
  handle-**cheap-drop** (ccr §35.7) / REJECT; peer/device-başına **persistent-anti-replay** (§0.7 high-water/bitmap);
  RESTART=yeni-session (full-PN-persist-yok). §4-`accept()` (bkz. N1-D1-2) + session-demux gerekir.
- **E5 — terminoloji:** `fec.rs:29` `"wrap→rekey"` → `"wrap→session-renewal (§2.6/§5)"` (N2-E1 ile-aynı düzeltme).
- **E6 — negatif-invariant-testi (K-WIRE-KEYUPD-1 / K-WIRE-CTRL-1):** KEY_UPDATE-allowlist-dışı-kalır (bugün-doğru,
  `inner_kind.rs:12`) — assert; cap-tetiği-deterministik + eski-session-datagram-admission-EDİLMEZ testi.

---

## 5. D1-EKSİKLERİ

- **[D1-N9-1] `session_id` + session yaşam-döngüsü** (oluştur/teardown) — YOK (grep=0). Cross-session-red-ön-koşulu.
- **[D1-N9-2] cap-sayaçları + tetik + SESSION_RENEW_REQUIRED-emisyonu** — YOK (yalnız-DTO `control.rs:54-59`).
- **[D1-N9-3] re-handshake orkestrasyonu** (renew → yeni-session_id → yeni-HANDLE_ASSIGN §2.55) — YOK.
- **[D1-N9-4] cross-session/eski-session-datagram-red + persistent-anti-replay + RESTART=yeni-session** (§0.7/§4) —
  YOK. §4-`accept()` + unknown-handle-drop (§2.55/ccr §35.7) gerekir (N1-D1-2/D1-3 ile-örtüşür).
- **[D1-N9-5] dh_epoch grace-sonrası-discard** (§4 5-adım) — YOK (N9-görev-cümlesindeki "eski-dh_epoch-discard"; bkz. N2-D1-N2-2).
- **[D1-N9-6] K-WIRE-KEYUPD-1 test-seti** (cap-deterministik + eski-datagram-red) + KEY_UPDATE-allowlist-dışı-assert.
- Split/dh_epoch-0→1-modeli + `rekey` terminoloji-envanteri = **bkz. N2.**
