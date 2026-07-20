# N10 — INIT-FLIGHT-MACHINE UYUM-RAPORU [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri; İŞ-B RAPOR-ONLY — hiçbir kaynak dosya değiştirilmedi). D0-N10 kritik-yol maddesi.
> **Kapsam:** noise-wire-D0-A N10 (kama-noise-wire-tdd §12). **Karar-DURUMU:** N10 KİLİTLİ — survival =
> bounded-idempotent-INIT-retransmission-flight-machine (§0.7-B, PATCH-8). Üç-seçenek YENİDEN-AÇILMAZ; bootstrap-FEC
> Alpha'da YOK. Bu rapor mevcut kodu bu **donmuş** karara karşı ENVANTERLER; tasarımı yeniden-açmaz.
> **Otorite:** `specs/kama-noise-wire-tdd-v2.0.md` §0.7-B (mekanizma-kararı) · MADDE-5 (§0.7 EARLY-DATA-REPLAY/
> IDEMPOTENCY — cached-byte-for-byte-INIT-retransmit) · §12-N10 (satır 541-544) · §11 K-WIRE-BOOTSTRAP-A/B/C.
> **SONUÇ (özet):** bounded-idempotent-INIT-retransmission-flight-machine kod-tabanında **YOK.** SESSION_INIT
> handshake katmanı iskelet (`crypto` crate 3-satır; `session_init`/`init_counter`/`cached`/`amplification`
> grep = 0). Mevcut `PreConverge` = **congestion-control cold-start** (DATA-birimi için geometrik-kopya), handshake-
> INIT-datagram survival'ı DEĞİL; `KEYSTONE` yalnız bir yorum-etiketi (`[K-KEYSTONE]`), Noise-AEAD-probe kodu YOK.
> Ayrıntı §2-5.

---

## (1) SPEC NE DİYOR (§0.7-B · MADDE-5 · §12-N10)

### §0.7-B — MEKANİZMA-KARARI [NORMATİF — KARAR-VERİLDİ, PATCH-8] (satır 102-114)
- **NORMATİF karar (satır 102-103):** "**Bounded-idempotent-SESSION_INIT-retransmission-flight-state-machine.**
  Bootstrap-FEC Alpha'da-KULLANILMAZ (Beta/ayrı-araştırma)." → survival = paket-teslim-KATMANI; handshake-PATTERN
  DEĞİL (o §0.9-non-regression'da mühürlü).
- **Kurallar (satır 104-110):**
  - "Aynı-SESSION_INIT aynı-replay-identity-ile-yeniden-gönderilir; her-retry **yeni-session-state-VEYA-ikinci-app-
    delivery-ÜRETMEZ.** Receiver duplicate-INIT'i-mevcut-session'a-map-eder, confirmation'ı-yeniden-gönderebilir."
  - "**Retry-sayısı + toplam-retry-süresi + backoff-profili SABİTLERLE-bounded** (D0-raporu-sabitleri-ÖLÇEREK-kilitler;
    mekanizma-yeniden-seçilmez)." → count/süre/backoff = BOUNDED, sabitler D0/D1'de ölçülerek kilitlenir.
  - "SESSION_INIT **IP-fragmentation'a-GÜVENMEZ**; max-bootstrap-datagram < max_udp_payload_bytes."
  - **ANTI-AMPLIFICATION (satır 109-110):** "**Unauthenticated-input sınırsız-response/amplification-ÜRETMEZ;
    cookie/return-routability YALNIZ-gerçek-amplification/reflection-yüzeyi-varsa-etkinleştirilir.**"
- **BAĞ (satır 113-114):** "mevcut-PRE_CONVERGE-cold-start + KEYSTONE-Noise-AEAD-probe = tam-bu (B)-katmanı → D0'da-
  birleşik. **D0-çıktısı = mühürlü-handshake'in hostile-paket-teslim-KARARI (pattern-DEĞİL).**"

### MADDE-5 — CACHED-BYTE-FOR-BYTE-INIT-RETRANSMIT [NORMATİF] (§0.7, satır 116-134)
- **Replay-identity (satır 118-119):** `INIT-replay-identity = (sender_device_id, recipient_device_id, init_counter)`
  — TEK-KİMLİK (transcript-hash-alternatifi KALDIRILDI).
- **INIT-RETRY tanımı (satır 121-124):** "**INIT-RETRY = CACHED-BYTE-FOR-BYTE-DATAGRAM-RETRANSMISSION (MADDE-5):**
  SESSION_INIT bir-kez-oluşturulur-ve-seal-edilir; retry'larda cached-datagram-byte'ları-AYNEN-yeniden-gönderilir.
  **Aynı-init_counter-ile-yeniden-serialization/padding-değişimi/yeniden-encryption YASAK** (nonce-reuse-riski).
  Payload-değişecekse: YENİ-init_counter + YENİ-ephemeral + YENİ-session-attempt." → bu **established-FEC/data-
  retransmit'in "yeni-PN+yeni-nonce" kuralından (§3) AYRIDIR** (bootstrap istisnası).
- **İdempotency (satır 124-127):** session-state-YENİDEN-KURULMAZ · app-message-İKİNCİ-KEZ-teslim-EDİLMEZ ·
  established-session'da duplicate-INIT → mevcut-session'a-map + confirmation-YENİDEN-gönderilebilir · aynı-
  init_counter-FARKLI-sender-cert → REJECT.
- **RESTART (PATCH-15, satır 127-133):** process-restart=yeni-handshake+yeni-session_id (RESUME-YOK) · sender
  init_counter cihaz-başına-KALICI+MONOTON · receiver peer-başına bounded-PERSISTENT-anti-replay (high-water/bitmap) ·
  full-PN diske-persist'ine-GÜVENİLMEZ. **Production-early-data-admission NO-GO (kod-doğrulanana-dek); bootstrap-POC-GO.**

### §12-N10 (satır 541-544) + §11 kabul-kriterleri
- **N10 soru-listesi:** "mevcut-PRE_CONVERGE/KEYSTONE **cached-byte-for-byte-INIT-retransmit-yapıyor-mu (MADDE-5)?**
  **retry-sabitleri (count/süre/backoff) ne? anti-amplification-var-mı?** forward-%90→A, reverse-%90→B/C-davranışı-ölçülür."
- **K-WIRE-BOOTSTRAP-A/B/C (satır 496-501):** A = forward-%90-loss'ta RECEIVER_ESTABLISHED bounded-retry-ile-oluşur
  (idempotent-duplicate + anti-amplification; **hub-reflection-YOK; retry-amplification'a-dönüşmez**) · B = reverse-
  %90'da SENDER_CONFIRMED eventual (survivor-ACK) · C = FS_UPGRADED gecikebilir/gerçekleşmeyebilir, epoch-0-usable-kalır.

---

## (2) MEVCUT KOD NE YAPIYOR (dosya:satır)

### 2.a Handshake/SESSION_INIT/kripto katmanı — **YOK (iskelet)**
- `crates/crypto/src/lib.rs:1-3` — **3 satır** doc-comment: *"Noise + compact crypto layer … API-first iskelet;
  trait imzaları sonraki iş."* Hiçbir Noise-handshake / SESSION_INIT / seal/open / init_counter kodu yok.
- **NEGATİF-TEYİT (grep `crates/adaptive-transport` + `crates/crypto` + `crates/kama-transport-schema`):**
  `session_init` · `init_counter` · `cached` · `amplification` · `cookie` · `return-routability` · `max_bootstrap` ·
  `byte-for-byte` → **SIFIR eşleşme.** (`handshake`/`SESSION_INIT` yalnız `crates/bench/src/quic.rs`'te = QUIC-baseline
  harness'in kendi handshake'i, Kama-INIT değil; `SESSION_RENEW_REQUIRED` schema'da var ama re-handshake **tetiği**,
  INIT-flight-machine değil — `kama-transport-schema/src/inner_kind.rs:21`, `control.rs:54-55`.)
- **HÜKÜM (a):** cached byte-for-byte SESSION_INIT retransmit → **YOK.** MADDE-5'in konusu olan SESSION_INIT datagramı
  hiç üretilmiyor; cached-datagram-store yok; init_counter/replay-identity yok.

### 2.b `PreConverge` = congestion-control cold-start (DATA geometrik-kopya) — `sender.rs`
- `sender.rs:54` — `SenderState::PreConverge` (§3.3 cold-start). Doc (`sender.rs:52-53`): *"motor HİÇ ölçüm yokken.
  En-kötü-köşe önsel pacing (floor) + **geometrik kopya**; loss-ilanı/PTO/emergency/ack_confidence DONDURULUR.
  İlk geçerli SACK'te → Startup. **[K-KEYSTONE]**"* → bu bir **CC cold-start state'i**, handshake-INIT-datagram-
  survival'ı DEĞİL.
- **Önsel pacing floor:** `sender.rs:202` `const PRE_CONVERGE_FLOOR_BW: u64 = 31;` (B/s, [CAL PLACEHOLDER]);
  `sender.rs:1105-1106` PreConverge'de bu floor döner (kanalı floodlamaz).
- **Geometrik kopya (§3.3-B) — DATA-birimi kopyası:** `sender.rs:172-180` `struct DupRec { due_ns, unit: OpaqueUnit,
  interval_ns }`; `interval_ns>0` = PRE_CONVERGE geometrik kopya. `sender.rs:266` `const PC_COPY_MULTIPLIER: u64 = 2;`
  `sender.rs:1230-1248` `plan_forward_duplication`: unacked orijinal **DATA** inflight birimlerine (`origin ==
  TransmitKind::Data`) spekülatif kopya (taban=srtt, ×2 backoff), *"ACK'le iptal §3.3-B-stop"*.
- **KRİTİK BULGU — kopya SAYI-TAVANI YOK (unbounded):** `sender.rs:1295-1303`: *"§3.3-B geometrik reschedule:
  interval_ns>0 → sonraki kopya ×COPY_MULTIPLIER (**sayı-tavanı YOK, deadline'a dek**; teyit gelince §3.3-B stop ile
  iptal)."* → süre-cinsinden backoff var ama **retry-COUNT bound'u YOK** (MADDE-5'in "retry-sayısı SABİTLERLE-bounded"
  şartının aksine — ama bu DATA-CC-katmanı, INIT-katmanı değil).
- **Kopya YENİ seq taşır (byte-for-byte DEĞİL):** `sender.rs:1290-1294`: `kind: TransmitKind::Duplicate, seq, …` —
  her kopya taze wire-`seq` ile emit edilir (`next_seq++`). Bu, MADDE-5'in "cached-datagram-byte'ları-AYNEN,
  aynı-init_counter" kuralının **tersi** yönde (yeni-PN); yine DATA-scope, INIT değil (§3-yeni-PN kuralına uygun DATA).
- **Stop/iptal:** `sender.rs:647` §3.3-B stop — teyit edilen birimlerin bekleyen geometrik kopyaları İPTAL edilir.
- **Cold-start çıkışı:** `pacer.rs:75-80` `fill_to_cap()` — PRE_CONVERGE→STARTUP (§3.3-D) kanal-türevli micro-burst.
  `receiver.rs:251` — receiver, ilk-SACK'i cadence-exempt yapar (*"client PRE_CONVERGE'den hızlı çıksın"*).

### 2.c `KEYSTONE` — yalnız yorum-etiketi, mekanizma YOK
- Tüm kod-tabanında `KEYSTONE` = **tek eşleşme**: `sender.rs:53` yorumundaki `[K-KEYSTONE]` etiketi. **Noise-AEAD-probe
  kodu YOK** — §0.9/§0.7-B'nin bahsettiği "KEYSTONE-Noise-AEAD-probe" implementasyonu mevcut değil.

### 2.d Bounded-retry KONSTRÜKTÜ var — ama DATA-ARQ scope'unda (INIT değil) — `arq.rs`
- `arq.rs:55-79` `RetxConfig { arq_retx_cap: 3 (fixture), … }`; `arq.rs:117-124` `retx_count >= arq_retx_cap →
  CapExceeded{policy}` (`CapExceedPolicy::HandToForward` default). → **bounded retry cap MEVCUT ama §9 selective
  retransmit (DATA-shard) için**; SESSION_INIT-flight-machine için DEĞİL. Retry-COUNT bound'lu tek yer burası,
  fakat yanlış katman (established-DATA, handshake değil).
- `sender.rs:126-135` `enum TransmitKind { Data, Retransmit, Duplicate, FecParity }` — INIT/handshake variant'ı YOK.

### 2.e Anti-amplification — **YOK**
- `amplification` · `cookie` · `return-routability` · "response ≤ N× request" · path-validation → **grep = 0.**
  Unauthenticated-input'a karşı bounded-response / reflection-guard kod-tabanında hiç yok. (Zaten INIT/open yolu
  olmadığı için amplification-yüzeyi de henüz yok.)

---

## (3) HÜKÜM: UYUMLU / UYUMSUZ / YOK

| N10-sorusu | Bulgu | Hüküm |
|---|---|---|
| (a) cached byte-for-byte INIT retransmit (MADDE-5)? | SESSION_INIT yok; crypto 3-satır iskelet; cached-datagram-store yok; init_counter yok | **YOK** |
| (b) retry sabitleri (count/süre/backoff)? | INIT için YOK. Yakın konstrüktler: `PC_COPY_MULTIPLIER=2` (backoff, count-CAP yok/unbounded, `sender.rs:266,1296`); `PRE_CONVERGE_FLOOR_BW=31`; `arq_retx_cap=3` (DATA-ARQ, `arq.rs:73`) | **YOK** (INIT-scope); DATA-scope kısmî |
| (c) anti-amplification (response ≤ N× request, path-validation)? | grep=0; cookie/return-routability/reflection-guard yok | **YOK** |
| (d) PRE_CONVERGE gerçekte ne yapar? | CC cold-start: floor-pacing (31 B/s) + DATA-birimi geometrik-kopya (×2, count-cap yok), ACK'le iptal; loss/PTO/emergency DONDURULUR | CC-cold-start (INIT-survival DEĞİL) |
| (d) KEYSTONE gerçekte ne yapar? | Yalnız `[K-KEYSTONE]` yorum-etiketi (`sender.rs:53`); Noise-AEAD-probe kodu yok | Etiket-only, mekanizma YOK |

**NET HÜKÜM: YOK.** Bounded-idempotent-SESSION_INIT-retransmission-flight-machine (§0.7-B/MADDE-5) kod-tabanında
mevcut **değil.** Handshake/INIT/kripto katmanı iskelet. §0.7-B'nin "BAĞ" satırında (satır 113) bu (B)-katmanının
*mevcut-PRE_CONVERGE + KEYSTONE* ile birleşeceği söylenir; ancak koddaki `PreConverge` bir **congestion-control cold-
start** (DATA-birimi geometrik-kopyası, §3.3) mekanizmasıdır — SESSION_INIT-datagram survival'ı değil — ve `KEYSTONE`
yalnız bir yorum-etiketidir (probe kodu yok). Bu yüzden N10-konusu KIRMIZI değil (handshake-open yolu henüz kod
olarak yok → ihlal fiziksel imkansız), ama **UYUMLU da değil**: flight-machine'in kendisi D1'de yazılacak.

> **KURAL-2 NOTU:** yukarıdakiler code↔spec **eksik** (mismatch değil-ihlal): kod D1'de yazılır, spec DEĞİŞMEZ.

---

## (4) KOD-DÜZELTME-PLANI (D1) — spec DEĞİŞMEZ

Bu bir "UYUMSUZ-düzeltme" değil, **eksik-katman-yazımı**dır (INIT-flight-machine kripto/handshake katmanında YENİ).
`PreConverge`'in DATA-CC rolü DOKUNULMAZ (§0.9 non-regression); flight-machine AYRI katman.

- **E1 — SESSION_INIT üretim + cached-datagram store (MADDE-5):** SESSION_INIT bir-kez seal → `cached_datagram: Bytes`
  saklanır; retry `cached_datagram`'ı AYNEN yeniden-gönderir (yeniden-serialize/encrypt YASAK — nonce-reuse).
  Replay-identity `(sender_device_id, recipient_device_id, init_counter)`; init_counter cihaz-başına-KALICI+MONOTON.
  (Kripto katmanı — `crypto` crate / yeni `kama-noise-wire` crate; `crypto/src/lib.rs` iskelet doldurulur.)
- **E2 — Bounded flight-state-machine (§0.7-B):** `INIT_RETRY_MAX_COUNT` + `INIT_RETRY_TOTAL_DEADLINE` +
  `INIT_BACKOFF` sabitleri (D1-ölçümle-kilitlenir). PreConverge'in **unbounded** geometrik-kopyasını (`sender.rs:1296`
  "sayı-tavanı YOK") KOPYALAMA — INIT-flight-machine COUNT-bounded olmalı. `arq.rs` cap-pattern'i (arq_retx_cap →
  CapExceeded) burada emsal-alınabilir; deadline→retry-durur, session-fail-closed/eventual (§0.7-B B/C).
- **E3 — Receiver idempotency + duplicate-INIT-map:** duplicate-INIT → mevcut-session'a-map (yeni-state YOK,
  app-2.teslim YOK); confirmation YENİDEN-gönderilebilir; aynı-init_counter-farklı-cert → REJECT. Bounded-PERSISTENT
  anti-replay (high-water/bitmap) peer-başına.
- **E4 — Anti-amplification (§0.7-B satır 109):** unauthenticated-input'a response ≤ N× request'e bounded; gerçek-
  reflection-yüzeyi varsa cookie/return-routability. `max-bootstrap-datagram < max_udp_payload_bytes` (IP-frag-güvenmez).
- **E5 — Restart-politikası (PATCH-15):** process-restart=yeni-session_id+yeni-handshake (RESUME-YOK); full-PN
  persist-EDİLMEZ; eski-session-datagramı yeni-handle'a admission-ALMAZ; replay-cache+TTL.
- **E6 — SenderState/TransmitKind ayrımı:** INIT-flight-machine `PreConverge`'in DATA-CC state'inden AYRI tutulur
  (INV-ENGINE-SOC: transport bytes tutmaz — cached-INIT kripto-katmanında). `KEYSTONE` etiketinin arkasına gerçek
  Noise-AEAD-probe kodu D1'de bağlanır (veya etiket kaldırılır; §0.9 mühürlü-probe tanımına göre).

---

## (5) D1-EKSİKLERİ (kesin liste)

- **[D1-1] KRİTİK — cached byte-for-byte SESSION_INIT retransmit:** üretim/seal/cache/retransmit yolu hiç yok
  (`crypto` 3-satır iskelet). MADDE-5 çekirdeği.
- **[D1-2] Bounded retry sabitleri:** `INIT_RETRY_MAX_COUNT` (count) · `INIT_RETRY_TOTAL_DEADLINE` (süre) ·
  `INIT_BACKOFF` (profil). Kod-tabanında **hiç yok**; en yakın `PC_COPY_MULTIPLIER=2` unbounded (count-cap yok) ve
  `arq_retx_cap=3` yanlış-katman (DATA). "D0-raporu-sabitleri-ÖLÇEREK-kilitler" (§0.7-B satır 106) → sabitler
  ölçümle belirlenecek.
- **[D1-3] Anti-amplification:** response ≤ N× request bound + path-validation/cookie (koşullu) + `max-bootstrap-
  datagram < max_udp_payload_bytes`. Kodda hiç yok.
- **[D1-4] Receiver idempotency + persistent anti-replay:** duplicate-INIT-map, app-2.teslim-YOK, `(device,device,
  init_counter)` high-water/bitmap, farklı-cert→REJECT, replay-cache+TTL.
- **[D1-5] Restart-politikası (PATCH-15):** yeni-session_id-zorunlu, PN-persist-YOK, eski-datagram-admission-red.
- **[D1-6] KEYSTONE-Noise-AEAD-probe:** §0.9/§0.7-B'nin "KEYSTONE-probe"u koddta yok (yalnız `[K-KEYSTONE]` etiketi);
  gerçek probe D1'de yazılır.
- **[D1-7 — ÖLÇÜM, şimdi YAPILMAZ] asimetrik-loss davranış matrisi:** §12-N10 "**forward-%90→A, reverse-%90→B/C-
  davranışı-ölçülür**" (K-WIRE-BOOTSTRAP-A/B/C, satır 496-501). Test-matrisi fwd/rev-AYRI: `{0/90, 90/0, 90/90,
  burst/clean, clean/burst} × {A,B,C}`. Bu bir **D1-ÖLÇÜM gereksinimi** (flight-machine kurulduktan sonra netem/tam-
  matris ile bir-kez); N10-raporunda ÖLÇÜLMEZ — yalnız D1-eksiği olarak listelenir.

### DR-ADAYI (KURAL-2 — spec-değişikliği ÖNERİLMEZ, tasarım-çözümü işaret edilir)
- **DR-ADAYI-N10-1:** §0.7-B "BAĞ" satırı (satır 113) "*mevcut-PRE_CONVERGE-cold-start + KEYSTONE-Noise-AEAD-probe =
  tam-bu (B)-katmanı*" der. Kod-gerçeği: `PreConverge` = **congestion-control cold-start** (DATA-birimi geometrik-
  kopyası, §3.3, `sender.rs:52-54`), backoff'u **unbounded** (`sender.rs:1296` "sayı-tavanı YOK") ve her kopya
  **yeni-seq** taşır (`sender.rs:1290`) — MADDE-5'in *bounded* + *cached-byte-for-byte* (aynı init_counter) şartlarıyla
  yapısal olarak zıt yönde; `KEYSTONE` ise probe-kodu-yok. INIT-datagram-survival ile DATA-CC-cold-start'ın "birleşik"
  mi (aynı mekanizmayı yeniden mi kullanacak) yoksa AYRI katmanlar mı olacağı — ve KEYSTONE-probe'un gerçek varlığı —
  bir **tasarım-çözüm (DR) adayıdır.** Bu rapor spec değişikliği önermez; D1-tip/katman tasarımının netleştirmesi gerekir.
