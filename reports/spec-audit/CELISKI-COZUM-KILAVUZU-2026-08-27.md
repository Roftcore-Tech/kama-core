# ÇELİŞKİ-ÇÖZÜM KILAVUZU — 2026-08-27 (Abkarada düzeltme-oturumu için)

Kaynak: `reports/spec-audit/SPEC-GAP-REGISTRY-2026-08-27.md` (78 doğrulanmış bulgu; ham `gap_audit.json`).
Bu kılavuz REGISTRY'nin **karar-gerektiren + çelişki** alt-kümesini düzeltilebilir-forma getirir: her madde =
iki tarafın BİREBİR alıntısı (satır-no'lu) + seçenekler + gerekçeli öneri + nereye yama yazılacağı.
Öneriler bağlayıcı değil — karar senin; işaretleyip düzelt, ben sonra kod/test hizasını takip ederim.

## NASIL KULLAN
1. Bölüm-A'daki 10 kararı sırayla ver (S2'yi bloklayan ilk 5 öncelikli).
2. Bölüm-B mekanik-yamalar: karar gerektirmez, doğrudan metin düzeltmesi (istersen bir kısmını bana bırak).
3. Her düzeltmede: kazanan-metni öbür belge(ler)e REFERANSLA yaz (kopya değil) — bir-otorite-kuralı.

---
# BÖLÜM A — KARAR GEREKTİREN 10 MADDE


## A1. Rekey modeli: ccr §35.8 (2³¹-öncesi epoch-rollover) vs noise-wire §5 (in-session rekey YASAK)  

**Çelişki/karar:** ccr §35.8 (active, sealed) mandates in-session epoch rollover before 2^31 messages while noise-wire §5 (KİLİTLİ) forbids any in-session rekey and the code caps a single session's PN at 2^62 — the legacy-path implementer cannot obey both key-lifecycle authorities.

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:2462 vs /home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:504`

> ccr:2462 'REKEY_THRESHOLD = 2^31 (epoch rollover bundan ÖNCE; seq16 ambiguity oluşmadan)' + ccr:2461 'MAX_MESSAGES_PER_EPOCH = 2^32' vs noise-wire:504 '**Terminoloji [KİLİT]:** yalnız **dh_epoch (0/1)**; `key_phase`/`rekey_epoch`/`KEY_UPDATE`-terimleri Alpha-normatifinde **KULLANILMAZ.**' and noise-wire:502 'cap→new-session : packet/byte/time-cap-dolunca SESSION_RENEW_REQUIRED → tamamen-YENİ-handshake/session'. Code: crates/kama-noise-wire/src/protector.rs:27 'pub const SEND_PN_CAP: u64 = 1 << 62;' (≫2^31). noise-wire's DR-3 carve-out (line 412) imports only WINDOW/FUTURE_GAP/REORDER from §35.8, not REKEY_THRESHOLD, so the 2^31 constant remains binding and unreconciled. ccr §35.9 also carries 'epoch (u16_be, 2B)' vs noise-wire's 1-bit wire_flags.bit0 dh_epoch.

**Düzeltilmezse:** Sprint-6 must silently pick one model (Q18/kama-contract class); a long-lived session that passes 2^31 packets either violates a sealed ccr constant or requires a rekey mechanism the noise profile bans, surfacing as an interop/audit corrigendum later.

**Seçenekler:**
- **A) noise-wire kazanır (kod-gerçeği):** in-session rekey yok; oturum packet/byte/time CAP'ine ulaşınca SESSION_RENEW_REQUIRED → tamamen yeni handshake. ccr §35.8'deki REKEY_THRESHOLD=2³¹ 'epoch-rollover' dili → 'session-cap; cap ≤ seq16-ambiguity-güvenli-sınır' olarak yeniden yazılır; §35.9'daki epoch(u16) alanı noise-wire'ın 1-bit dh_epoch'una reconcile edilir.
- B) ccr kazanır: in-session epoch-rollover mekanizması tasarlanır → noise-wire §5 KİLİT'i açılır, Split/CipherState çoğullanır → S2 kapsamı büyür, kod (SEND_PN_CAP=2⁶², tek dh_epoch-bit) yeniden yapılır.

**Önerim:** **A.** Kod+noise-wire-KİLİT zaten bu modelde; B, mühürlü-profili açıp S2'yi haftalarca büyütür. A'da tek dikkat: session-cap değerini seq16-ambiguity gerekçesiyle bağla (ccr'ın 2³¹ endişesi kaybolmasın).

**Yama-hedefi:** specs/ccr-format-spec.md §35.8-35.9 (rollover→session-cap dili + epoch-alanı notu); specs/kama-noise-wire-tdd-v2.0.md §5'e ccr-referans satırı.

---

## A2. Crypto-backend: ADR-03 başlığı (libsodium-primary) vs kendi DR-5 eki  

**Çelişki/karar:** Sprint-6 Noise needs an X25519/Ed25519/HKDF production backend, but ADR-03 head still says libsodium-primary while its own DR-5 amendment forbids libsodium in the production dependency graph, and no pure-Rust DH/signature crate exists in the workspace — the decision is open in both directions.

**Yer:** `specs/tech-stack-adr.md:66, 79, 94; crates/crypto/src/lib.rs:1; Cargo.lock (no x25519/dalek/snow entries)`

> specs/tech-stack-adr.md:66: "libsodium (FFI, tek onaylı C bağımlılığı): Ed25519, X25519, ChaCha20-Poly1305, BLAKE2s, ristretto255"; :79: "Ed25519/X25519/BLAKE2s/ristretto kararları DEĞİŞMEDİ" — yet :94: "libsodium test-only/ayrı-CI-job (production-dependency-graph'ına-GİRMEZ)" and AMENDMENT-2 (:106-108) already proved the head wrong for BLAKE2s: "libsodium `crypto_generichash` = BLAKE2b ≠ BLAKE2s ... → libsodium-BLAKE2s-yolu uygulanamaz". Reality: crates/crypto/src/lib.rs:1 is a 3-line skeleton ("libsodium primary + dalek cross-check") and `grep 'name = "x25519' Cargo.lock` / dalek / snow return nothing.

**Düzeltilmezse:** Sprint-6 stalls on day one into another DR-4/DR-5-style backend decision cycle (the DR-S5-5 report already documents this exact latent-contradiction pattern costing a decision round for BLAKE2s alone).

**Seçenekler:**
- **A) Kod-gerçeğine hizala:** RustCrypto-ailesi primary (kama-noise-wire zaten chacha20poly1305-RustCrypto; S2'de x25519-dalek/ed25519-dalek) + ikinci-implementasyon cross-check politikası; ADR-03 başlığı amendment'la güncellenir, pki §13 registry aynı kalır (primitif seti değişmiyor, sağlayıcı netleşiyor).
- B) ADR-03 başlığına sadık kal: libsodium FFI'ı şimdi kur (unsafe-yüzey + build-zinciri işi S2'ye eklenir), dalek cross-check'e iner.

**Önerim:** **A.** DR-5 eki + mevcut kod zaten oradaydı; S2'nin ilk haftasını FFI-tesisatına harcamamak için. (İstersen 'Beta'da libsodium-audit' notu düşülür.)

**Yama-hedefi:** specs/tech-stack-adr.md ADR-03 (başlık-karar satırı amendment); CLAUDE.md §5 ADR-03 satırı.

---

## A3. INIT-flight retry sabitleri: 'D0 ölçüp kilitler' dedi, D0 'YOK' dedi, kod [CAL] uydurdu  

**Çelişki/karar:** The INIT-flight retry constants were normatively delegated to 'D0 report locks by measurement', the D0 report explicitly recorded them as absent, and the code shipped an unmeasured [CAL] default (5 retries/200ms/x2/<=10s) that mathematically cannot satisfy the forward-90%-loss bootstrap gate (6 transmissions -> ~47% establishment probability) — an underspecified knob of the Q18 class with no calibration procedure or acceptance criterion.

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:130 + /home/ryuzaki/Desktop/kama-core/crates/kama-noise-wire/src/init_flight.rs:30`

> Spec:130 'Retry-sayısı + toplam-retry-süresi + backoff-profili SABİTLERLE-bounded (D0-raporu-sabitleri-ÖLÇEREK-kilitler; mekanizma-yeniden-seçilmez).' — reports/noise-d0/N10-flight-machine.md:116 '| (b) retry sabitleri (count/süre/backoff)? | INIT için YOK...' — init_flight.rs:30 '/// Alpha-varsayılan [CAL — ölçümle-kilitlenir]: 5 retry, 200ms base, ×2, ≤10s.' vs acceptance spec:610 'K-WIRE-BOOTSTRAP-A: forward-%90-loss-altında RECEIVER_ESTABLISHED bounded-retry-ile-oluşur'.

**Düzeltilmezse:** The S2 hostile-bootstrap gate either fails unexplainably at 90% loss or gets re-tuned mid-campaign, repeating the weeks-long Q18 knee-calibration loop.

**Seçenekler:**
- **A) Kabul-kriterinden GERİYE-türet ve kilitle:** %90-forward-loss'ta P(kuruluş)≥0.99 hedefi → gereken deneme-sayısı n: 1-(0.9)ⁿ≥0.99 → n≥44 iletim (retry-payload küçükse maliyet önemsiz); backoff 100ms×1.5, toplam-süre ≤30s gibi TÜRETİLMİŞ sabitler spec'e yazılır + tek netem-hücresiyle doğrulanır (kampanya değil, dakikalar).
- B) D0-tarzı ölçüm-kampanyası planla (Q18-senaryosu riski: haftalar).

**Önerim:** **A.** Kabul-kriteri zaten spec'te (K-WIRE-BOOTSTRAP-A); sabit ölçümle değil MATEMATİKLE türetilir, netem tek-hücre yalnız teyit eder.

**Yama-hedefi:** specs/kama-noise-wire-tdd-v2.0.md §0.7-B (sabitler+türetim); crates/kama-noise-wire/src/init_flight.rs [CAL]-değerleri.

---

## A4. HANDLE_ASSIGN'ın taşıyıcı kanalı: wire-frozen mesaj, byte-spec'siz kanal, hub-spec'lerde yok  

**Çelişki/karar:** HANDLE_ASSIGN is CLOSED/wire-frozen and rides an authenticated hub<->client Noise channel, but that channel (Noise IK/KK per transport §0.1) has no byte-level specification in any document, no implementation anywhere (crypto crate is a 3-line skeleton, zero X25519/Noise code repo-wide), and HANDLE_ASSIGN itself is absent from both hub-side authority specs (application-protocol RPC registry and routing-rendezvous) — the hub implementer has no contract to build it from.

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:237 + /home/ryuzaki/Desktop/kama-core/specs/transport-layer-tdd.md:30`

> noise-wire:237 'HANDLE_ASSIGN [WIRE-FROZEN — TEK-yol, exact-layout]: hub→client hub-control-mesajı (hub↔client-Noise-transport-kanalında-authenticated; E2E-endpoint-keyi-DEĞİL)' + transport-layer-tdd:30 'Client↔Hub Noise handshake : IK (yeni kayıt, sk_device possession) / KK (kayıtlı, 0-RTT).' — grep 'HANDLE_ASSIGN' yields 0 hits in application-protocol-and-rpc-spec.md and routing-rendezvous-architecture.md (only fec-core-tdd:462 table copy + DR-4 package:163); crates/crypto/src/lib.rs is 3 lines of doc comment; kama-noise-wire only has hub_sim.rs ('hub-tarafı D1'de SİMÜLE; gerçek-hub ayrı-repo').

**Düzeltilmezse:** S3-hub cannot deliver h_ext distribution (the extended parser is unusable without it — spec:237 'extended-parser-bunsuz-çalışmaz'), forcing either an invented hub-channel wire format or a stall while a new spec is written and re-frozen.

**Seçenekler:**
- **A) Taşıyıcıyı S3-spec'ine havale et + S2'de stub-injection'ı resmileştir:** noise-wire'a not: 'hub↔client-kanalının byte-spec'i routing-rendezvous vX'te; o gelene dek HANDLE_ASSIGN test-injection (hub_sim) yolu NORMATİF-GEÇİCİ'. S2 bloklanmaz; S3-girişinde kanal-spec'i yazılır (Paket-3'e kayıt).
- B) Kanal byte-spec'ini ŞİMDİ yaz (IK/KK handshake-mesaj-düzeni dahil) → S2 kapsamına hub-kanalı da girer.

**Önerim:** **A.** S2'nin hedefi endpoint-Noise+Split; hub-kanalı S3'ün doğal parçası. (B'yi seçersen S2 süresine +1 hafta koy.)

**Yama-hedefi:** specs/kama-noise-wire-tdd-v2.0.md §2.55'e taşıyıcı-notu; specs/routing-rendezvous-architecture.md'ye HANDLE_ASSIGN+kanal TODO-bölümü.

---

## A5. §11 hostile-bootstrap kabul-kriterleri eşiksiz (ölçülemez)  

**Çelişki/karar:** The three hostile-bootstrap acceptance criteria have no pass/fail thresholds: A has no success-probability or time bound, B is satisfiable by any behavior ('may be eventual'), C explicitly passes even when the property never occurs, and the burst profiles in the test matrix are unparameterized — the S0.7 gate cannot be turned into an executable test.

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:610-615`

> Spec:610 'K-WIRE-BOOTSTRAP-A: forward-%90-loss-altında RECEIVER_ESTABLISHED bounded-retry-ile-oluşur (...)' — no probability/deadline; spec:612 'K-WIRE-BOOTSTRAP-B: reverse-%90-loss-altında SENDER_CONFIRMED eventual-olabilir (...)'; spec:614 'K-WIRE-BOOTSTRAP-C: FS_UPGRADED gecikebilir-veya-gerçekleşmeyebilir; epoch-0-usable-KALIR (...) Test-matrisi: fwd/rev {0/90,90/0,90/90,burst}×{A,B,C}' — 'burst' has no GE/duration parameters anywhere in the doc.

**Düzeltilmezse:** The bootstrap test matrix gets written with invented thresholds that later become disputed evidence (the P5-parasitic pattern), or every outcome 'passes' and the hostile-handshake claim is unbacked.

**Seçenekler:**
- **A) Eşikleri şimdi yaz:** A: '%90-loss'ta P(ESTABLISHED)≥0.99, süre≤30s (A3-türetimiyle tutarlı)'; B: 'yalnız-retry-ile-değil: tek-INIT-kaybında kuruluş-medyanı ≤ 2×RTT+backoff₁'; C: 'FS-upgrade %X-reverse-loss'ta ≤Y s medyan / gerçekleşmezse epoch-0-usable pin-testi'.
- B) Eşiksiz bırak → S2 sonunda 'geçti mi?' tartışması (transport'taki iddia-dili döngüsü).

**Önerim:** **A** — A3'le birlikte tek oturuşta.

**Yama-hedefi:** specs/kama-noise-wire-tdd-v2.0.md §11 üç kriter satırı.

---

## A6. ra_attestation imzacısı: identity 'Intermediate CA' vs pki+discovery 'RA Identity'  

**Çelişki/karar:** The three authority docs disagree on who signs ra_attestation (identity says Intermediate CA, pki and discovery say RA Identity), and no doc defines its byte format (ccr declares it opaque), so RA cannot produce it and verify_ra_attestation cannot be written.

**Yer:** `specs/identity-and-trust-model.md:813 (vs specs/pki-ca-ra-hsm-tdd.md:125, specs/discovery-architecture.md:322)`

> identity:813 "ra_attestation : bytes      # Intermediate CA imzalı (phone→identity bağı)" and identity:940 "ICA->>ICA: DiscoveryRecord ra_attestation imzala (phone→identity)" — vs pki:122-125 "RA Identity: → RevocationRecord → RevocationPush → operational RA attestations (ra_attestation, discovery binding)" and discovery:322 "...veya attestation expired / RA-imzalı değil" (reject rule assumes RA-signed). ccr-format-spec.md:1040 "ra_attestation : varbytes16 # max 512 byte; opaque (RA/discovery docs)" — no TBS/layout anywhere.

**Düzeltilmezse:** ra-ca issuance flow and discovery-server PUBLISH validation get built against different signing keys, guaranteeing an interop failure and a corrigendum cycle in the first S4 integration week.

**Seçenekler:**
- **A) RA-Identity imzalar (2/3-belge + operasyonel-mantık):** RA çevrimiçi-operasyonel imzacı; Intermediate soğuk kalır. identity-model reconcile edilir; attestation'ın canonical-byte formatı ccr'a eklenir (ayrı bulgu S4.x).
- B) Intermediate imzalar: her attestation için Intermediate-erişimi gerekir (HSM/offline-politikasıyla çatışır).

**Önerim:** **A.**

**Yama-hedefi:** specs/identity-and-trust-model.md ilgili imzacı-satırları → pki/discovery'ye referans; ccr'a RaAttestation byte-layout TODO.

---

## A7. Revocation eşleşme-anahtarı: identity 'serial' vs revocation-model 'cert_hash' (frozen)  

**Çelişki/karar:** Identity's normative validator pseudocode and test cases match revocation by serial while frozen revocation-model mandates cert_hash as the sole primary handle (serial = optional scope_hint only), so a validator transcribed from identity never matches hash-keyed revocations and revoked hub certs pass.

**Yer:** `specs/identity-and-trust-model.md:1753,1772,2412 (vs specs/revocation-model.md:193,210)`

> identity:1753 "if fresh_crlite.contains(hub_identity_ccr.serial): return FALSE" (repeated at identity:1772 in verify_hub_announce) and identity:2412 test 33 "Çıplak device_id ile CRLite revocation eşleşmesi → REJECT (issuer_key_id+serial kullanılmalı, 18.3)" and identity:1219 "7. CRLite: bob device serial revoke değil" — vs revocation:193 "canonical primary handle = `cert_hash = SHA-256(canonical_ccr_tbs)` ... `serial` yalnızca opsiyonel `scope_hint` / audit referansıdır; matching primary'si değildir" and revocation:1128-1129 AC-REV-1.

**Düzeltilmezse:** Hub/RA cert revocation checks silently no-op (filter keyed by cert_hash, lookup by serial), reproducing the exact spec-vs-spec drift class that cost weeks on transport, this time with a security-inverting outcome.

**Seçenekler:**
- **A) cert_hash kazanır (frozen-model + ccr §13.6 revocation_id=cert_hash zaten):** identity'nin validator-pseudocode+test-case'leri cert_hash'e çevrilir.
- B) serial'e dön: frozen revocation-model + ccr revize (geniş dalga).

**Önerim:** **A.**

**Yama-hedefi:** specs/identity-and-trust-model.md validator/test satırları.

---

## A8. identity'nin AAD-tablosu ccr §35'le 4 mesaj-tipinde çelişiyor  

**Çelişki/karar:** The identity model's ACTIVE AAD table claims to be the 'ccr-spec §35.2 exact layout' but contradicts ccr §35 on all four message types: a 144B SESSION_INIT_AAD carrying message_counter||timestamp and session_id_internal (fields that do not exist on the V3 wire), a 25B SESSION_DATA_AAD vs ccr §35.9's 28B layout (no protocol_version/cipher_suite_id/epoch fields), and a SIGNED_CONTROL tbs incompatible with ccr §35.10 — while identity's header (line 5) declares itself 'implementation-ready (ccr-spec §35 reconciliation sonrası)' and ccr:2696 admits freeze is pending 'cross-doc identity-model reconciliation'.

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/identity-and-trust-model.md:1075-1081 (active §10, appendix §27 starts at 2467)`

> Line 1075-1080: `**AEAD Associated Data (AAD) — message-type'a göre (ccr-spec §35.2 exact layout):** ... SESSION_DATA_AAD:   "SSD1" || flags(epoch+direction) || session_id_internal || seq   (25B; ...)` vs ccr:2478-2486 SESSION_DATA_AAD = SSD1||protocol_version||cipher_suite_id||direction||epoch||seq16||session_id_internal (28B; implemented as 28B in crates/kama-noise-wire/src/legacy.rs:22)

**Düzeltilmezse:** An implementer following the semantic-authority doc writes wrong AAD bytes for every message type; the false 'exact layout' attribution makes the divergence undetectable until AEAD verification fails cross-implementation.

**Seçenekler:**
- **A) ccr §35 otorite (zaten P0-3 istisnası öyle diyor):** identity'deki tablo SİLİNİR, yerine 'AAD: ccr §35.x' referansı (kopya-tablo bir daha sapmasın).
- B) Tabloyu ccr'a eşitleyip tut (gelecekte yine sapar).

**Önerim:** **A** — kopya yerine referans, bir-otorite-kuralı.

**Yama-hedefi:** specs/identity-and-trust-model.md AAD-tablosu bölümü.

---

## A9. App-RPC Plane-1/Plane-2 frame-ayrıştırıcısı yok (ilk-byte çakışması)  

**Çelişki/karar:** No wire discriminator distinguishes a Plane-1 AppRpcFrame (first byte = method_id u8) from a Plane-2 hot-path frame (first 4 bytes = opaque PRF-derived hub_handle) arriving on the same established client↔hub link, yet R-APP-7 declares plane confusion "Closed" via "separate parsers" — a parser cannot be selected without a byte-level rule that no document provides.

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/application-protocol-and-rpc-spec.md:69`

> app-rpc:69 "Üç düzlem aynı method_id sayı uzayını kullansa bile çakışmaz: farklı plane, farklı parser, farklı session."; app-rpc:52 Plane 1 "Taşıma: kurulu Client↔Hub Noise session (transport substrate)." — the same substrate Plane 2 uses; transport-layer-tdd:77 the hot-path frame begins "hub_handle : 4B" with no type byte; app-rpc:624 "R-APP-7 | Plane confusion ... | Closed | 0x0406 WRONG_PLANE; ayrı parser/state machine." No spec defines how the hub routes an incoming datagram to the right parser (port split? outer type byte? reserved handle space? — undecided).

**Düzeltilmezse:** Every implementer invents a different demux rule (interop break), an opaque hub_handle can collide with any method_id prefix, and the 0x0406 WRONG_PLANE behavior is untestable as specified.

**Seçenekler:**
- **A) İlk-byte bölüşümü kilitle:** hot-path handle'ın ilk byte'ına ayrılmış-aralık (örn. ≥0xF0 method_id-yasak) YA DA Plane-1'e sabit magic-prefix; app-rpc §'e tek paragraf.
- B) Port/transport-ayrımı (iki soket) — dağıtım-karmaşası.

**Önerim:** **A** (magic-prefix en az-riskli: mevcut hot-path formatına dokunmaz).

**Yama-hedefi:** specs/application-protocol-and-rpc-spec.md framing-bölümü.

---

## A10. Discovery batch-B: aynı anda 'TBD-ölçülecek' ve sabit-değer  

**Çelişki/karar:** Batch size B is simultaneously an open TBD in discovery ("256?", to be measured, with no calibration procedure or acceptance criterion) and a frozen normative constant in app-rpc (B=256); the per-user enumeration quota is likewise the placeholder "N sorgu/gün" with no value — the Q18 knob class on a privacy-load-bearing parameter (padding hides address-book size; quota is what closes threat D-4).

**Yer:** `/home/ryuzaki/Desktop/kama-core/specs/discovery-architecture.md:523`

> discovery:523 "Sabit batch boyutu B (256?) — performans/gizlilik dengesi ölçülüp netleşecek (TDD)." vs app-rpc:576 "DISCOVERY_PROXY (online enumeration): per-user + per-org quota + anomali tespiti (Signal modeli); sabit batch B=256 padding." (stated as normative). discovery:197 "Per-user batch quota (ör. N sorgu/gün), per-org global quota." — N never pinned, no calibration procedure anywhere; yet D-4 (discovery:484) is marked "✓" mitigated on the strength of that quota.

**Düzeltilmezse:** Implementers freeze divergent B/N values; if measurement later moves B the already-normative app-rpc line forces a corrigendum, and the D-4 'closed' enumeration claim is unmeasurable until N exists.

**Seçenekler:**
- **A) B=256 şimdi kilitle** + 'Beta'da ölçümle revize' notu (padding/anonimlik-analizi 256-üzerinden yazılır).
- B) TBD bırak → S4-discovery girişinde yine duvar.

**Önerim:** **A.**

**Yama-hedefi:** specs/discovery-architecture.md B-geçen satırlar (tek değer + tek not).

---

# BÖLÜM B — MEKANİK YAMALAR (karar gerektirmez; istersen bir kısmını bana bırak)

| # | Belge | Düzeltme |
|---|---|---|
| B1 | CLAUDE.md | `docs/specs/` → `specs/` (2 yer); '8 crate' → 14-üyeli gerçek liste; §3 otorite-listesine ekle: kama-noise-wire-tdd-v2.0, fec-core-tdd-v3.0, DR-4-DR-5-karar-paketi; §7 CI-tanımını gerçek ci.yml'e (5-bacak matrix + fuzz-smoke + security + miri + aead) eşitle |
| B2 | docs/design/P4-TRANSPORT-API-FREEZE.md §2.2 | v0 trait-metnini v1-imzayla değiştir (§3'teki v1.0-girişi zaten doğru; §2 gövdesi bayat) |
| B3 | docs/roadmap/BITIRME-YOL-HARITASI-2018-08-18 vs KAMA-URUN-BITIRME-PLANI | İddia-durumu çelişkisi: tek-otorite = URUN-BITIRME-PLANI; eski roadmap'ın çelişen satırına supersede-notu |
| B4 | docs/design/P3-DEFAULT-FLIP-DECISION.md | startup-pacing default'unun kendi-içi çelişkisi: kod-gerçeği (Default enabled=true, 925dea66) tek-cümleyle sabitle |
| B5 | reports/s1-gate/S1-GATE-RESULT.md | RESMÎ GATE-VERDİKT satırı ekle (plan S2'yi buna şartlıyordu): 'NO-HARM=PASS; kategorik-bar AÇIK; S1-GATE=YEŞİL (sevk-güvenliği anlamında)' — ve 868-0 atfını 'kama-içi kol-karşılaştırması' diye düzelt (quinn-karşılaştırması değil) |
| B6 | reports/s2-region/REPORT.md | '"her koşu ≥%88" (B-hücreleri)' cümlesini ham-veriyle uzlaştır (l60d0 rep2=30/40=%75 → '≥%75, medyan ≥%88' de) |
| B7 | docs/whitepaper/...v0.2 | 'burst-fix ayrı-ekte' vaadi → V1/burst-reproof gerçek-sonuç referansı; maliyet-payda tanımını KAPI-1 §5.2 diliyle eşitle |
| B8 | specs/branching-...runbook.md + CLAUDE.md §6 | Branch-yaş kuralı ihlal-notu: transport-baseline-closure için tek-seferlik istisna kaydı + PR#45-üzerinden-kapanış planı; runbook C.1/C.2 CI-tanımı ci.yml'e eşitlenir; branch-protection check-listesi 4-yeni-gate'i içerir |
| B9 | specs/DR-4-DR-5-karar-paketi | 'Sprint-5 HOLD / PR#42-43' bayat-statü bloğuna güncel-durum amendment'ı |
| B10 | specs/fec-core-tdd-v3.0.md | FecCodec-trait imzası koda eşitle (reconstruct 3. parametre known_zero) + AEAD-nonce96 closure-satırı ile gövde tutarlılığı |

# BÖLÜM C — AÇILACAK BELGELER (düzeltme-oturumu seti)
- reports/spec-audit/SPEC-GAP-REGISTRY-2026-08-27.md  ← tam-envanter (78)
- reports/spec-audit/gap_audit.json                    ← ham-kanıt (tam alıntılar)
- Bölüm-A hedefleri: specs/ccr-format-spec.md · specs/kama-noise-wire-tdd-v2.0.md · specs/tech-stack-adr.md ·
  specs/identity-and-trust-model.md · specs/revocation-model.md · specs/pki-ca-ra-hsm-tdd.md ·
  specs/application-protocol-and-rpc-spec.md · specs/discovery-architecture.md · specs/routing-rendezvous-architecture.md
- Bölüm-B hedefleri: CLAUDE.md · docs/design/P3,P4 · docs/roadmap/×2 · reports/s1-gate,s2-region · docs/whitepaper · specs/runbook,DR-4-5,fec-core

**Not:** S3/S4'ün kalan BLOCKER'ları (hub-mesh RPC-framing'in yazılması, discovery-mesaj-byte'ları, admin↔RA
payload-tipleri, DeviceRevocationRequest-formatı…) bu kılavuzda YOK — onlar 'çelişki-düzeltme' değil 'spec-yazma'
işi; kendi sprint'lerinin giriş-kapısı (Paket-3, registry'de listeli).
