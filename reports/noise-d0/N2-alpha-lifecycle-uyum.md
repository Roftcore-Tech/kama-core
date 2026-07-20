# N2 — ALPHA KEY-LIFECYCLE UYUM-RAPORU (Split + dh_epoch-0→1) [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; kod YAZILMADI). noise-wire-D0-A N2 maddesi (§12).
> **Kapsam (§12/§5):** mevcut-kod `Split()`+`dh_epoch-0→1`-modeline-uyuyor-mu? in-session-key-phase/rekey-kodu
> **VARSA** Alpha-yolundan-kaldırılacak-envanter. Gerçek key-material-rekey (YOK-olmalı) ile yalnız-seq-wrap-
> disambiguation için kullanılan zararsız-epoch-sayaçlarını AYIRIR.
> **Otorite-spec:** `specs/kama-noise-wire-tdd-v2.0.md` §5 (Alpha-dh_epoch + session-renewal, KİLİTLİ) · §0.9
> (L4-Noise-split non-regression) · §9 (Noise-güvenlik-sınırı) · §4 (dh_epoch-0→1-geçiş) · §11 K-WIRE-NONREG-1.
> **HÜKÜM (özet):** **UYUMLU-BOŞ.** Kaldırılacak in-session-rekey/key-phase **state-machine'i YOK** (Alpha-invariant
> ihlali yok). `Split()`+`dh_epoch-0→1` kripto-modelinin KENDİSİ de KODDA-YOK (`crypto` crate 3-satır-iskelet) → D1.
> Tek bulgu: **yorum-düzeyi terminoloji-sızıntısı** (`rekey`/`wrap→rekey` sözcükleri) — Alpha-yolundan-kaldırılacak.
> **DR-ADAYI: yok.** (Tüm code↔spec farkları KOD-tarafı; spec-tutarlı.) Cap→yeni-session + cross-session = **bkz. N9.**

---

## 1. SPEC-DİYOR

### §5 — ALPHA DH-EPOCH + SESSION RENEWAL [NORMATİF — KİLİTLİ, MADDE-7] (in-session-key-phase-YOK)
Dört-kavram (satır 394-399, quote):
```
1. Split()                : handshake-sonu k_send/k_recv (bir-kez)                        → KORUNUR (mühürlü)
2. dh_epoch 0→1 upgrade   : SESSION_INIT_RESPONSE asenkron-full-FS (epoch-0-usable-önce)  → KORUNUR (mühürlü)
3. in-session-symmetric-key-phase-state-machine                                           → ALPHA'DA-YOK
4. cap→new-session        : packet/byte/time-cap-dolunca SESSION_RENEW_REQUIRED → tamamen-YENİ-handshake/session
```
- **Terminoloji-KİLİDİ (satır 400-402):** "yalnız **dh_epoch (0/1)**; `key_phase`/`rekey_epoch`/`KEY_UPDATE`-terimleri
  Alpha-normatifinde **KULLANILMAZ.**" Kimlik = `(session_id, direction, dh_epoch, full_pn)`.
- **N2-görev-cümlesi (satır 409-411):** "mevcut-kodda in-session-rekey/key-phase-kodu-VAR-mı (varsa-Alpha-
  yolundan-kaldırma-envanteri); cap-tetiği-SESSION_RENEW_REQUIRED→re-handshake-açıyor-mu? eski-dh_epoch-grace-
  sonrası-discard-ediliyor-mu?" [cap→re-handshake + eski-discard = **N9-ana-eksen**, bu-rapor Split/rekey-envanteri.]

### §0.9 — L4-NOISE-SPLIT NON-REGRESSION [NORMATİF, EN-ÜST-ÖNCELİK]
- "**Noise-Split() + asynchronous-dh_epoch-upgrade (epoch-0→1)**" MEVCUT-MÜHÜRLÜ-KRİPTO'nun-parçası; **DEĞİŞMEZ**;
  noise-wire-onu-**OKUR**. (satır 51-58). Yani D1'de Split/dh_epoch-0→1 mühürlü-handshake'ten TÜRETİLİR, yeniden-tanımlanmaz.

### §4 — dh_epoch-0→1-GEÇİŞ [B3 — CONFIRMATION-SAFE-STATE-MACHINE] (satır 373-383)
5-adım grace-machine; `send_epoch`/`recv_epoch` **direction-local**; grace = `max(4×sRTT, GRACE_MIN_PACKETS=64)`;
grace-bitince eski-epoch-recv-state-**discard**. **EPOCH-PN-INIT (satır 371):** epoch-1'de `expected_pn[1]` ve
send-counter **0'dan-başlar** (epoch-0-değerinden-devam-ETMEZ).

### §9 — NOISE-GÜVENLİK-SINIRI (satır 466)
"dh-epoch-0 = klasik-FS-DEĞİL (per-INIT-ephemeral-confidentiality); dh-epoch-1 = FULL-FS (RESPONSE-sonrası). Profil-
bunun-ÖTESİNDE-ek-FS/PCS-mekanizması-EKLEMEZ; **REKEY-Alpha'da-cap→re-handshake (§5).**"

### §5.5 [BETA-APPENDIX, NON-NORMATIVE] — "Alpha-implementasyonu-bu-bölümü-UYGULAMAZ" (satır 405-408)
in-session-symmetric-key-phase-machine açık-soruları Beta'ya-ertelendi; **Alpha kod-olarak-uygulamaz.**

---

## 2. KOD-YAPIYOR (dosya:satır)

### 2.a Split() / k_send / k_recv / handshake — **YOK (iskelet)**
- `crates/crypto/src/lib.rs:1-3` — **3 satır**, yalnız doc-comment: *"Noise + compact crypto layer … API-first
  iskelet; trait imzaları sonraki iş."* Hiçbir `Split()`/`k_send`/`k_recv`/handshake/dh_epoch kodu yok.
- **grep `Split` / `k_send` / `k_recv` tüm `crates/**/*.rs` → SIFIR eşleşme.** Split-modeli kod-olarak mevcut değil.
- **grep `session_id` / `SessionId` / `session_attempt` → SIFIR eşleşme.** SECURE-WIRE-IDENTITY (§3) kod-yüzeyi yok.
- `crates/bench/src/quic.rs` içindeki "handshake" eşleşmeleri = **QUIC/rustls-TLS-handshake** (benchmark-karşılaştırma
  harness'i), Kama-Noise-handshake DEĞİL; kapsam-dışı.

### 2.b dh_epoch — yalnız WIRE-ALANI olarak var, STATE-MACHINE olarak YOK
- **grep `dh_epoch` → yalnız 2 eşleşme:** `crates/kama-transport-schema/src/control.rs:15` ve `:21` —
  `Control::Ack { … acked_dh_epoch: u8 … }` / `Control::Sack { … acked_dh_epoch: u8 … }`. Bu §2.57-ACK/SACK'in
  hangi-epoch'un-PN'ini-onayladığını-taşıyan **logical-DTO-alanıdır** (data), epoch-state-machine DEĞİL. Üretici/
  doğrulayıcı yok (schema-only).
- `dh_epoch-0→1` upgrade-machine (§4 5-adım, grace, `expected_pn[epoch]`) **hiçbir yerde YOK** (bkz. N1 §4-accept()-eksiği).

### 2.c in-session-rekey / key-phase STATE-MACHINE — **YOK (Alpha-invariant KORUNUYOR)**
- **grep `key_update`/`KeyUpdate` → yalnız 1 eşleşme:** `inner_kind.rs:12` yorum: *"Control codepoint'leri (§2.56
  allowlist; **KEY_UPDATE/NACK YOK**)"*. Yani KEY_UPDATE allowlist'e **girmemiş** (§2.56/§5-uyumlu, doğru).
- **grep `key_phase`/`rekey_epoch` → SIFIR eşleşme.** current/next/previous-key state, bounded-key-trial, key-phase-bit
  — hiçbiri kodda yok. §5.5-Beta-machine uygulanmamış (spec-gereği).

### 2.d TERMİNOLOJİ-SIZINTISI — yorum-düzeyi (Alpha-yolundan-kaldırılacak-ENVANTER)
Gerçek-rekey-kodu-YOK ama §5-terminoloji-kilidini-ihlal-eden **yorumlar** var:
- `crates/adaptive-transport/src/fec.rs:29` — `block_id` doc: *"Blok kimliği (hop-scoped, **wrap→rekey**)."*
  → §2.6/§5'e-göre `block_id`-wrap → **session-renewal (yeni-handshake)**, in-session-rekey DEĞİL. Sözcük-yanlış.
  [Not: `fec.rs` FEC_SHARD(0x20) yolu N3'e-göre zaten §2.6-EXTENDED'e-taşınacak; terminoloji-yine-de-düzeltilir.]
- `crates/adaptive-transport/src/ack.rs:50` — `AckFrame.ack_epoch` doc: *"**Rekey** / counter-wrap epoch'u."*
- `crates/adaptive-transport/src/ack.rs:15-16` — modül-yorumu: *"`ack_epoch` yalnız **rekey** sınırını disambiguate
  eder — u16 sarması epoch değişimi DEĞİLDİR."* → niyet-doğru (kripto-epoch-değil-diyor) ama `rekey`-sözcüğü §5-yasaklı.

### 2.e transport `ack_epoch:u8` — ZARARSIZ seq-wrap-disambiguation SAYACI (key-material DEĞİL)
- `crates/adaptive-transport/src/ack.rs:51` — `AckFrame.ack_epoch: u8` (wire-alanı).
- `crates/adaptive-transport/src/receiver.rs:126` — `ack_epoch: u8` (receiver-state); `:160` init `ack_epoch: 0`;
  `:448` `build_ack` → `from_received(..., self.ack_epoch, ...)`. **grep → 126/160/448 dışında MUTASYON YOK** →
  değer **kalıcı-0**, hiç artmıyor; taşınan-placeholder. Kripto-anahtar/dh_epoch DEĞİL (§5-ayrımına-göre "zararsız-
  seq-wrap-disambiguation sayacı"). Ayrıca §2.5-tek-full-PN-space'te u16-wrap zaten `seq::resolve_seq`-ile-çözülür
  (bkz. N1 §2.d) → bu sayaç yapısal-olarak-VESTIGIAL; işlevsel-değil.

### 2.f İSİM-ÇAKIŞMASI (conflation-uyarısı; key-lifecycle-DIŞI) — `ack_epoch` = kanal-ZAMAN-tabanı
Aynı `ack_epoch` adı `rtt.rs`/`bw.rs`/`loss.rs`'te **tamamen-farklı** anlamda (bir SÜRE/ACK-kadans-aralığı, ns):
`rtt.rs:79,117` `W_qd = K_qd × ack_epoch` · `bw.rs:152,185` `max(10s, K_bw × ack_epoch)` · `loss.rs:221`
`W_loss = K_wl × ack_epoch`. Bunlar zaman-modeli-pencere-boyutlarıdır; **anahtar/kripto-epoch DEĞİL** ve §2.e-wire-
sayacı da DEĞİL. `rtt.rs:84` açıkça: *"ack_epoch ÖZ-REFERANS tuzağı hesaplandı ve REDDEDİLDİ"*. Yalnızca-conflation-
önlemek-için not-edildi; N2/N9-kapsamı-dışı.

---

## 3. HÜKÜM: UYUMLU / UYUMSUZ / YOK

**HÜKÜM: UYUMLU (rekey-kaldırma-ekseni) — Split/dh_epoch-modeli ise KODDA-YOK (iskelet, D1).**

| N2 alt-sorusu | Bulgu | Hüküm |
|---|---|---|
| in-session-rekey/key-phase **state-machine** var-mı? | Hiç yok (grep `key_phase`/`rekey_epoch`=0; `key_update`=yalnız "YOK"-yorumu) | **UYUMLU** (kaldırılacak-machine-YOK; Alpha-invariant-ihlali-yok) |
| KEY_UPDATE control allowlist'te-mi? | Değil — `inner_kind.rs:12` "KEY_UPDATE/NACK YOK"; allowlist 0x10..0x18 (`inner_kind.rs:13-21`) | **UYUMLU** (§2.56/§5) |
| `Split()`+`k_send/k_recv` modeli uygulanmış-mı? | `crypto` 3-satır-iskelet; grep=0 | **YOK → D1** |
| `dh_epoch-0→1` upgrade-machine (§4 5-adım) uygulanmış-mı? | Yok; `dh_epoch` yalnız `acked_dh_epoch:u8` wire-alanı (`control.rs:15,21`) | **YOK → D1** |
| gerçek-key-rekey ↔ zararsız-epoch-sayacı ayrımı | `ack_epoch:u8` (`ack.rs:51`,`receiver.rs:126`) = kalıcı-0 seq-wrap-placeholder, key-material-DEĞİL | **ZARARSIZ** (kaldırma-gerekmez; yalnız terminoloji) |
| terminoloji-kilidi (§5: `rekey` yasak) | `fec.rs:29`,`ack.rs:15-16,50` `rekey`/`wrap→rekey` yorumları | **UYUMSUZ-yorum** (kaldırılacak-envanter) |

**NET:** Alpha-invariant (dh_epoch 0/1; in-session-rekey-YOK) **İHLAL EDİLMİYOR** — kaldırılacak fiili-rekey-kodu-yok.
Mühürlü `Split()`+`dh_epoch-0→1` modeli de henüz-kod-değil; D1'de §0.9-non-regression-altında mühürlü-handshake'ten
türetilecek. Tek somut-düzeltme = üç-dosyadaki **`rekey`/`wrap→rekey` terminoloji-sızıntısı** (yorum-only).

---

## 4. KOD-DÜZELTME-PLANI (D1)

> Bunlar UYUMSUZ-yapı-düzeltmesi değil; (E1-E2) **terminoloji-temizliği** + (E3-E5) **eksik-modelin-yazımı**. Spec-DEĞİŞMEZ.

- **E1 — `fec.rs:29` terminoloji:** `"(hop-scoped, wrap→rekey)"` → `"(session+direction-scoped, wrap→session-renewal
  §2.6/§5)"`. (`block_id` u32 NO-REUSE, §2.6.) [Not: bu-satır N3-kapsamında zaten §2.6-inner'a-taşınıyor; terminoloji
  taşımadan-önce-de-düzeltilebilir.]
- **E2 — `ack.rs:15-16, 50` terminoloji:** `"Rekey / counter-wrap epoch'u"` ve `"rekey sınırını"` içinden `rekey`
  sözcüğünü-çıkar. §5-kilidi: yalnız `dh_epoch`. Bu-alan ya §2.57-`acked_dh_epoch`'e-yeniden-eşlenir (aşağıda E4) ya
  "counter-wrap disambiguator (kripto-epoch-DEĞİL)" olarak-net-yeniden-adlandırılır.
- **E3 — `Split()` (D1, `crypto`/`kama-noise-wire` crate):** handshake-sonu **bir-kez** `k_send`/`k_recv` türetimi
  (§5-(1)). §0.9-non-regression: mevcut-mühürlü-N-türevi-handshake'ten OKUNUR, yeniden-tanımlanmaz.
- **E4 — `dh_epoch-0→1` upgrade-machine (§4 satır 373-383, §5-(2)):** SESSION_INIT_RESPONSE-sonrası asenkron-FS;
  5-adım-confirmation-safe; `send_epoch/recv_epoch` direction-local; grace = `max(4×sRTT, 64pkt)`; grace-bitince-eski-
  epoch-discard; `expected_pn[1]`+send-counter epoch-1'de-0'dan (EPOCH-PN-INIT). Wire-epoch-seçici = §4 `wire_flags.bit0`
  (§2.5) — outer-EXTENDED-header henüz-kod-değil (yalnız `EXTENDED_OUTER_OVERHEAD` sabiti `lib.rs:41`).
- **E5 — in-session-key-phase machine EKLENMEZ (§5-(3), §5.5):** Alpha-yolunda **YAZILMAZ** (negatif-invariant; test
  ile assert: allowlist'te KEY_UPDATE-yok, `key_phase`-state-yok — K-WIRE-KEYUPD-1).

---

## 5. D1-EKSİKLERİ

- **[D1-N2-1] `Split()` / `k_send` / `k_recv`** — YOK (`crypto` iskelet). Mühürlü-handshake'ten türetilecek (§0.9).
- **[D1-N2-2] `dh_epoch-0→1` confirmation-safe upgrade-machine** (§4 5-adım + grace + EPOCH-PN-INIT) — YOK.
- **[D1-N2-3] SESSION_INIT_RESPONSE asenkron-FS-upgrade** (epoch-0-usable-önce) — YOK.
- **[D1-N2-4] SECURE-WIRE-IDENTITY `(session_id, direction, dh_epoch, full_pn)`** (§3) — `session_id`/`dh_epoch`-state
  yok (grep=0). N1-D1-5 ile-örtüşür.
- **[D1-N2-5] Terminoloji-temizliği** (`rekey`/`wrap→rekey`: `fec.rs:29`, `ack.rs:15-16,50`) — E1/E2.
- **[D1-N2-6] Negatif-invariant-testi** (in-session-key-phase-YOK; KEY_UPDATE-allowlist-dışı) — K-WIRE-KEYUPD-1.
- **cap→re-handshake + eski-dh_epoch-discard + cross-session-red = bkz. N9** (N2-görev-cümlesinin-ikinci-yarısı orada).
