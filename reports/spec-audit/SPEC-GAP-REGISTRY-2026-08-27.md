# SPEC-GAP REGISTRY — 2026-08-27 (adversarial denetim, doğrulanmış)

**Neden:** transport 3-hafta yerine 8-hafta sürdü; kök-neden spec/TDD gap+belirsizlikleriydi (Q18-[CAL]-sınıfı ·
kama-contract-çelişki-sınıfı · ADR-16-drift-sınıfı). Abkarada-direktifi: sıradaki sprint'lere girmeden AYNI
sınıfları tüm spec'lerde + Claude-yazımı/Abkarada-onaylı implementasyon-belgelerinde avla.

**Yöntem:** 9 adversarial küme-okuyucu (tam-metin) → küme-başına bağımsız ŞÜPHECİ (her BLOCKER/MAJOR'ı
alıntı-satırından ÇÜRÜTME denemesi; desteksiz=REFUTED). 18 ajan, 608 dosya-okuması.
**Sonuç: 58 CONFIRMED · 20 DOWNGRADED (gerçek-ama-abartılı) · 2 REFUTED.** Ham veri: `gap_audit.json`.

**Okuma kılavuzu:** Her bulgu = sprint-kovası (neyi stall eder) altında, önem-sırasıyla. ⬇ = şüphecinin
daralttığı bulgu (gerekçesi altında). Bu belge FIX YAPMAZ; fix-sıralaması ayrı karar.


## S2 — Noise/crypto (SIRADAKİ sprint) — 26 bulgu (BLOCKER=11)

### S2-noise.1 ✅ [CONTRADICTION/BLOCKER] — ccr §35.8 (active, sealed) mandates in-session epoch rollover before 2^31 messages while noise-wire §5 (KİLİTLİ) forbids any in-session rekey and the code caps a single session's PN at 2^62 — the legacy-path implementer 
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:2462 vs /home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:504`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** ccr:2462 'REKEY_THRESHOLD = 2^31 (epoch rollover bundan ÖNCE; seq16 ambiguity oluşmadan)' + ccr:2461 'MAX_MESSAGES_PER_EPOCH = 2^32' vs noise-wire:504 '**Terminoloji [KİLİT]:** yalnız **dh_epoch (0/1)**; `key_phase`/`rekey_epoch`/`KEY_UPDATE`-terimleri Alpha-normatifinde **KULLANILMAZ.**' and noise-
- **Maliyet:** Sprint-6 must silently pick one model (Q18/kama-contract class); a long-lived session that passes 2^31 packets either violates a sealed ccr constant or requires a rekey mechanism the noise profile ban

### S2-noise.2 ⬇ [MISSING-DECISION/BLOCKER] — The spec's structural nonce-uniqueness proof rests on one direction-local full-PN space covering SESSION_DATA plus all EXTENDED classes, but no document decides (a) whether the EXTENDED protector uses the same ccr §35.2 
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:210 + /home/ryuzaki/Desktop/kama-core/crates/kama-noise-wire/src/legacy.rs:9`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** noise-wire:210 '**TEK-direction-local-full-PN-space** TÜM-sealed-packet-class'lar-için (SESSION_DATA + FEC-source/repair + secure-ACK/...)' with rationale 'nonce-uniqueness-yapısal-ispat'; grep of the spec finds no key-derivation for EXTENDED (only Split() references at lines 499/623). ccr:2226-2228
- **Maliyet:** Sprint-6 must invent either a key-derivation label or a cross-protector allocator API; the wrong guess (shared key, independent counters) is a catastrophic nonce-reuse bug that no existing golden or p
- **Şüpheci-daraltması:** (a) is decided in the spec: kama-noise-wire-tdd-v2.0.md:498-499 seals 'Split() : handshake-sonu k_send/k_recv (bir-kez) → KORUNUR (mühürlü)' as the profile's only traffic-key derivation (identified with ccr §35.2 'SR-SESSION-TRAFFIC-V3' via §0.7-A li

### S2-noise.3 ✅ [GAP/BLOCKER] — The INIT-flight retry constants were normatively delegated to 'D0 report locks by measurement', the D0 report explicitly recorded them as absent, and the code shipped an unmeasured [CAL] default (5 retries/200ms/x2/<=10s
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:130 + /home/ryuzaki/Desktop/kama-core/crates/kama-noise-wire/src/init_flight.rs:30`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** Spec:130 'Retry-sayısı + toplam-retry-süresi + backoff-profili SABİTLERLE-bounded (D0-raporu-sabitleri-ÖLÇEREK-kilitler; mekanizma-yeniden-seçilmez).' — reports/noise-d0/N10-flight-machine.md:116 '| (b) retry sabitleri (count/süre/backoff)? | INIT için YOK...' — init_flight.rs:30 '/// Alpha-varsayıl
- **Maliyet:** The S2 hostile-bootstrap gate either fails unexplainably at 90% loss or gets re-tuned mid-campaign, repeating the weeks-long Q18 knee-calibration loop.

### S2-noise.4 ✅ [GAP/BLOCKER] — The active Session Model V3 key schedule gives HKDF calls with no defined convention — no salt, no RFC-5869 salt/IKM/info argument mapping, no hash named in-section, no encoding for the 'direction' input, no output lengt
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:2217 (also 2230, 2234, 2368; cf. 2995 and pki-ca-ra-hsm-tdd.md:456)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** Line 2230: `k_send / k_recv  = HKDF("SR-SESSION-TRAFFIC-V3", init_root, direction)   # yön-ayrık transport keys` — vs the only full convention at line 2995 (deprecated D2 appendix): `salt   = sabit "SR-ASYNC-V2-SALT" (16B ASCII) — tüm async KDF çağrıları için`
- **Maliyet:** Sprint-6 (real Noise handshake-derived Split key) must invent the derivation, producing a non-reproducible or cross-implementation-incompatible key schedule and a rework cycle when the convention is l

### S2-noise.5 ✅ [GAP/BLOCKER] — SESSION_INIT (SSI1) and SESSION_INIT_RESPONSE (SSR1) have no exact AAD byte layout, no defined AEAD key/nonce for their ciphertexts, and no defined role for init_counter/response_counter, even though §1/§6.1 assert their
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:2257-2263 (SESSION_INIT_REF layout) and 2354-2368 (SESSION_INIT_RESPONSE)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** Line 2263: `init_counter         : u64_be` (its AAD/nonce/replay role defined nowhere); line 2087: `AC-05 | Session Model V3 key schedule belirsiz değil (SESSION_INIT/RESPONSE/DATA, session_context_v1 binding, Section 35.2)`
- **Maliyet:** The two one-time-per-conversation messages that establish every session cannot be sealed/opened deterministically; each implementer invents an AAD → guaranteed interop failure and a corrigendum campai

### S2-noise.6 ✅ [CONTRADICTION/BLOCKER] — The identity model's ACTIVE AAD table claims to be the 'ccr-spec §35.2 exact layout' but contradicts ccr §35 on all four message types: a 144B SESSION_INIT_AAD carrying message_counter||timestamp and session_id_internal 
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/identity-and-trust-model.md:1075-1081 (active §10, appendix §27 starts at 2467)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** Line 1075-1080: `**AEAD Associated Data (AAD) — message-type'a göre (ccr-spec §35.2 exact layout):** ... SESSION_DATA_AAD:   "SSD1" || flags(epoch+direction) || session_id_internal || seq   (25B; ...)` vs ccr:2478-2486 SESSION_DATA_AAD = SSD1||protocol_version||cipher_suite_id||direction||epoch||seq
- **Maliyet:** An implementer following the semantic-authority doc writes wrong AAD bytes for every message type; the false 'exact layout' attribution makes the divergence undetectable until AEAD verification fails 

### S2-noise.7 ⬇ [DRIFT/BLOCKER] — AC-APP-2 freezes the hot-path as hub_handle4+seq2+ct+tag16 with no outer AEAD, but the actually-frozen transport-API-v1 product surface ships a different wire: DATA = [0x00][seq u16][unit_id u64] (11B header, no hub_hand
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/application-protocol-and-rpc-spec.md:664`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** app-rpc:58 "Established hot-path: hub_handle(4B) + seq(2B) + ciphertext + AEAD tag(16B)." and app-rpc:664 "AC-APP-2: Established hot-path raw Session Message Data Plane (hub_handle4+seq16+ct+tag16) ile gider; transport-layer-tdd.md byte muhasebesi korunur."; transport-layer-tdd:103 "Established SESS
- **Maliyet:** S2 (Sprint-6 Noise) must reconcile two frozen-but-incompatible layouts: either the P4-frozen API or the frozen spec byte accounting breaks, and the published 22B-overhead claim is already falsified by
- **Şüpheci-daraltması:** The byte divergence is real and all citations check out (app-rpc:58, app-rpc:664, transport-layer-tdd:103 'outer per-hop AEAD tag ... EKLENMEZ', wire.rs:3 11B header, seam.rs:3-4 seal/open on every datagram). But the claim's core assertion — 'no docu

### S2-noise.8 ✅ [MISSING-DECISION/BLOCKER] — Sprint-6 Noise needs an X25519/Ed25519/HKDF production backend, but ADR-03 head still says libsodium-primary while its own DR-5 amendment forbids libsodium in the production dependency graph, and no pure-Rust DH/signatur
- **Yer:** `specs/tech-stack-adr.md:66, 79, 94; crates/crypto/src/lib.rs:1; Cargo.lock (no x25519/dalek/snow entries)`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/tech-stack-adr.md:66: "libsodium (FFI, tek onaylı C bağımlılığı): Ed25519, X25519, ChaCha20-Poly1305, BLAKE2s, ristretto255"; :79: "Ed25519/X25519/BLAKE2s/ristretto kararları DEĞİŞMEDİ" — yet :94: "libsodium test-only/ayrı-CI-job (production-dependency-graph'ına-GİRMEZ)" and AMENDMENT-2 (:106-
- **Maliyet:** Sprint-6 stalls on day one into another DR-4/DR-5-style backend decision cycle (the DR-S5-5 report already documents this exact latent-contradiction pattern costing a decision round for BLAKE2s alone)

### S2-noise.9 ✅ [UNTESTABLE/BLOCKER] — The plan's make-or-break S1 GATE (explicit precondition for starting S2: line 94 'S1-GATE=YEŞİL şartı') has acceptance criteria with no measurable definition — no threshold for 'yaklaştırır', no completion-metric definit
- **Yer:** `docs/roadmap/KAMA-URUN-BITIRME-PLANI-2026-08-26.md:88-89 (and :94, :122)`  · küme: my-approved-docs A (roadmaps+design)
- **Kanıt:** Plan:88-89: '✅ **TEZ GERÇEK** ise: FEC-gate'li Kama (a) burst'ü önde çevirir, (b) %80'i ≥%90'a yaklaştırır, (c) test-edilmemiş rejimde quinn≈0 iken çalışır' — vs reports/s1-gate/S1-GATE-RESULT.md: 'quinn kolu YOK … Screening/ekran-teşhisi; ön-kayıtlı hüküm değil' and reports/s2-region/REPORT.md: '%9
- **Maliyet:** S2 either stalls waiting for a gate verdict nobody can compute, or starts on an unadjudicated gate — repeating the exact Q18-class 'criterion with no acceptance procedure' failure that cost weeks in t

### S2-noise.10 ✅ [MISSING-DECISION/BLOCKER] — The plan makes S2 (crypto/Noise) conditional on 'S1-GATE=YEŞİL' with three exit criteria, but no document records a gate verdict: criterion (a) 'burst'ü önde çevirir' was never measured against quinn (S1-GATE has no quin
- **Yer:** `reports/s1-gate/S1-GATE-RESULT.md:21 (vs docs/roadmap/KAMA-URUN-BITIRME-PLANI-2026-08-26.md:88 and :94)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** Plan:88 '✅ **TEZ GERÇEK** ise: FEC-gate'li Kama (a) burst'ü önde çevirir, (b) %80'i ≥%90'a yaklaştırır,' · Plan:94 '### S2 — GERÇEK OTURUM: crypto handshake *(~2-2.5 sprint)* — S1-GATE=YEŞİL şartı' · S1-GATE-RESULT.md:21 '- quinn kolu YOK (bu harness p1_endpoint; "FEC-ON Kama vs quinn burst-ahead" f
- **Maliyet:** Sprint-6/S2 starts (or is later invalidated) on an unmet frozen gate; if challenged, all S2+ work is exposed as sequenced against a gate that was never formally passed — the exact class of governance 

### S2-noise.11 ⬇ [DRIFT/BLOCKER] — The freeze doc's normative frozen-surface section (§2.2 'DONDURULMUŞ YÜZEY') still presents the superseded SecuritySeam v0 trait, while the shipped code and pin test are v1 with a completely different signature, and no f
- **Yer:** `docs/design/P4-TRANSPORT-API-FREEZE.md:106-108 vs crates/client-core/src/seam.rs:88-106`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** P4:106-108: 'fn seal(&self, plain: Vec<u8>) -> Vec<u8>; / fn open(&self, wire: &[u8]) -> Option<Vec<u8>>; / fn overhead(&self) -> usize;' — but seam.rs:88-106: 'pub trait SecuritySeam: Send + 'static { fn seal(&mut self, peer: SeamPeer, frame: Vec<u8>, args: SealArgs) -> Result<SealedDatagram, SealE
- **Maliyet:** Sprint-6 codes the Noise layer against the v0 signature in the 'frozen surface' section, hits the pin-test wall late, and reworks the seam integration — days of rework in exactly the sprint this doc e
- **Şüpheci-daraltması:** Facts confirmed: P4:106-108 prints the v0 trait (fn seal(&self, plain: Vec<u8>) -> Vec<u8> ...) under §2.2 with prose 'bu aynı üç metoda takar', seam.rs:88-106 is the v1 trait, and only the §3 changelog (P4:145-153) plus the DÜZELTME note describe v1

### S2-noise.12 ✅ [UNTESTABLE/MAJOR] — The three hostile-bootstrap acceptance criteria have no pass/fail thresholds: A has no success-probability or time bound, B is satisfiable by any behavior ('may be eventual'), C explicitly passes even when the property n
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:610-615`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** Spec:610 'K-WIRE-BOOTSTRAP-A: forward-%90-loss-altında RECEIVER_ESTABLISHED bounded-retry-ile-oluşur (...)' — no probability/deadline; spec:612 'K-WIRE-BOOTSTRAP-B: reverse-%90-loss-altında SENDER_CONFIRMED eventual-olabilir (...)'; spec:614 'K-WIRE-BOOTSTRAP-C: FS_UPGRADED gecikebilir-veya-gerçekle
- **Maliyet:** The bootstrap test matrix gets written with invented thresholds that later become disputed evidence (the P5-parasitic pattern), or every outcome 'passes' and the hostile-handshake claim is unbacked.

### S2-noise.13 ⬇ [GAP/MAJOR] — The dh_epoch-transition gate is marked CLOSED while its grace bound is still an open [CAL] with no D1 measurement procedure or acceptance criterion, and the sRTT input it depends on has no defined estimator, sampling rul
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:448-449 + 705`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** Spec:448-449 'GRACE sabit-2s-DEĞİL: bounded = max(4×sRTT, GRACE_MIN_PACKETS=64-paket-eşdeğeri); hostile-reorder/RTO-bütçesine-bağlı [CAL — D1-ölçümle-kilitlenir].' vs closure row 705 '| dh_epoch transition | 5-adım confirmation-safe machine; grace=max(4×sRTT,64pkt) CAL | CLOSED |'. Code punts the sa
- **Maliyet:** A wrong grace bound silently drops legitimate reordered epoch-0 packets after FS-upgrade under hostile RTT, and the 'CLOSED' label hides that the constant was never locked — a corrigendum-class redisc
- **Şüpheci-daraltması:** The sealed-but-CAL half is real; the sRTT half is refuted by another authority doc. Real: kama-noise-wire-tdd-v2.0.md:448-449 'GRACE ... bounded = max(4×sRTT, GRACE_MIN_PACKETS=64...) [CAL — D1-ölçümle-kilitlenir]' sits inside closure row 705 '| dh_e

### S2-noise.14 ✅ [GAP/MAJOR] — A family of normative bounds is named but never valued and has no calibration procedure: control-retry 'deadline' (PARAM_SET and the ABORT/COMPLETE/CLOSE/RENEW group), SESSION_CLOSE tombstone duration, the §5 session pac
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:318,330,502,311`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** Spec:318 'deadline→retry-durur, emission-SÜRER' (no deadline value); spec:330 'bounded-süre-sonra-tombstone-discard' (no duration); spec:502 'cap→new-session : packet/byte/time-cap-dolunca SESSION_RENEW_REQUIRED' (no cap values, yet spec:618 K-WIRE-KEYUPD-1 demands 'Test: cap-tetiği-deterministik');
- **Maliyet:** Each unvalued bound becomes an uncoordinated per-crate constant; peers with different invented deadlines/caps desynchronize (retry storms, premature renewal, tombstone misses) and each mismatch costs 

### S2-noise.15 ⬇ [UNTESTABLE/MAJOR] — noise-wire declares the Session Model V3 canonical fields 'MÜHÜRLÜ' on the strength of the ccr §30.4b test vectors, but §30.4b contains only schematic placeholders (no concrete key, nonce, ciphertext, or tag bytes — noth
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:1864-1893 + 1788`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** noise-wire:97 'CANONICAL-ALANLAR (ccr §30.4b-test-vektörü + §35.2, MÜHÜRLÜ)'. ccr:1867-1876 SESSION_INIT_REF vector fields are 'recipient_dest_hash: <32B>', 'ciphertext(early): <bytes + AEAD tag 16B>' — placeholders only; SESSION_DATA likewise '<bytes>'. Stale pointer ccr:1788 'Session Model V3 test
- **Maliyet:** There is no interop oracle for the sealed handshake bytes: the Sprint-6 implementation will fix its own behavior as the de-facto vector (the self-generated-golden trap §13.5 explicitly forbids), and a
- **Şüpheci-daraltması:** Both facts verified, but the framing overstates. Verified: ccr-format-spec.md:1864-1893 §30.4b fields are schematic ('recipient_dest_hash: <32B>', 'ciphertext(early): <bytes + AEAD tag 16B>', SESSION_DATA '<bytes>') — no concrete key/nonce/ciphertext

### S2-noise.16 ✅ [MISSING-DECISION/MAJOR] — Compression is normative (zstd, default ON for text, a fuzzy output) but no layer is ever assigned to execute it: zstd appears nowhere in the workspace, the engine's computed compress flag has no consumer outside its own
- **Yer:** `specs/transport-layer-tdd.md:652 (§8), :547 (§5.2) vs repo-wide: zero zstd hits; FusionDecision.compress consumers = tests only (crates/adaptive-transport/tests`  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** TDD :652: "compress = zstd; metin mesaj için default ON (düşük bantta büyük kazanç); fuzzy çıktısı (Section 5)." — `grep -rn zstd crates/ fuzz/ harness/` = 0 hits; fusion.rs:551 computes `let compress = bw.low > PERMILLE / 2 || mode == Mode::EmergencyForwardRedundancy;` whose only readers are tv_fus
- **Maliyet:** Sprint-6 SecuritySeam/Noise integration must either invent the compression seam ad hoc or ship without it, silently voiding the emergency-corner byte-budget assumptions (AC-8 'compress=ON') that the §

### S2-noise.17 ✅ [CONTRADICTION/MAJOR] — Identity's active SESSION_INIT replay rule requires a timestamp with MAX_AGE 7 days and a field named message_counter, and verify_session_init step 9 reads init.session_id_internal off the message — but the ccr V3 SESSIO
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/identity-and-trust-model.md:1638 (also 221, 1609, 2383) vs /home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:2257-2`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** identity:1638: `11. replay: key = (sender_identity_hash, init.session_id_internal, init.message_counter) + timestamp MAX_AGE 7 gün` — ccr SESSION_INIT_REF fields (2258-2263): flags, recipient_dest_hash, sender_ephemeral, sender_cert_ref, init_counter, ciphertext, aead_tag (no timestamp field)
- **Maliyet:** The receiver-side INIT anti-replay (the store-and-forward DoS boundary) cannot be implemented as specified; the coder either invents a timestamp source or ships an unbounded-replay receiver, both late

### S2-noise.18 ⬇ [UNTESTABLE/MAJOR] — Every §30 golden vector is a TEST_VECTOR_PLACEHOLDER (TODO-02) and §35.18 freezes with 'GO, pending generated test vectors', yet kama-noise-wire-tdd §0.9 normatively asserts the envelope is 'ccr §35'te MÜHÜRLÜ + test-vek
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:1784 (also 2696, 2709; kama-noise-wire-tdd-v2.0.md:64-65)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** ccr:1784: `Gerçek kriptografik imzalar hesaplanmadıysa TEST_VECTOR_PLACEHOLDER ile işaretlenir` vs kama-noise-wire-tdd:65: `Bu-envelope ccr §35'te-MÜHÜRLÜ + test-vektörlü — noise-wire-onu-OKUR`
- **Maliyet:** ccr-codec gets built against a spec whose conformance cannot be checked; first real vectors generated later become de-facto corrigenda, repeating the measured-vs-claimed failure pattern this audit exi
- **Şüpheci-daraltması:** The factual spine holds: ccr:1784 marks uncomputed signatures TEST_VECTOR_PLACEHOLDER, §30 vectors use <32B>/'11...11' placeholder values (:1795-1810, :1860-1890), :2696 freezes 'GO, pending generated test vectors', TODO-02 (:2705) says 'placeholder 

### S2-noise.19 ✅ [MISSING-DECISION/MAJOR] — The normative "versioned-ALLOWLIST" of FEC profiles and the MANDATORY-BOOTSTRAP-PROFILE are never enumerated in any spec — the only catalog (profile_ids 0x01-0x06 with K/R/E values) was invented in code, and the bootstra
- **Yer:** `specs/kama-noise-wire-tdd-v2.0.md:547,551; crates/kama-fec-codec/src/profile.rs:54-99; crates/client-core/src/runtime.rs:122,372`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/kama-noise-wire-tdd-v2.0.md:547: "profile-seti = versioned-ALLOWLIST"; :551: "Alpha protocol-version'a-bağlı **MANDATORY-BOOTSTRAP-PROFILE** tanımlar (tek-suite-zaten)" — but no spec lists any profile_id/K/R/E tuple (fec-core §6 gives only "K_shape∈{2,4,8,..}"). The catalog exists only at crat
- **Maliyet:** Any second implementation or peer-version mismatch fail-closes on PARAM admission (allowlist divergence), and the sender-may-emit-before-ACK bootstrap guarantee is unverifiable because no one can say 

### S2-noise.20 ✅ [DRIFT/MAJOR] — The document both frozen TDDs designate as the sprint-status authority still says Sprint-5: HOLD and gates on long-dead PR#42/#43, while Sprint-5d was executed and approved in July 2026 — the authority chain ends at a st
- **Yer:** `specs/DR-4-DR-5-karar-paketi-ve-spec-patchleri.md:159-166; specs/fec-core-tdd-v3.0.md:6-7; specs/kama-noise-wire-tdd-v2.0.md:8-9; reports/decision-requests/DR-S`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/DR-4-DR-5-karar-paketi-ve-spec-patchleri.md:164: "Sprint-5: HOLD (ek-şartlar: N1-legacy-interop-kapanışı + PR#43-cross-crate-entegrasyon-testleri ...)"; :159: "PR #42 : şartlı-HOLD"; fec-core-tdd-v3.0.md:6-7: "**Sprint-5: HOLD** ... Sprint-durum-otoritesi: karar-paketi BÖLÜM-5" (same at kama-n
- **Maliyet:** An implementer obeying the packet's own rule ("Agent Sprint-4/5-hold'larını KENDİLİĞİNDEN-KALDIRAMAZ", line 10) must either stall Sprint-6 or silently ignore the deterministic authority order — reprod

### S2-noise.21 ⬇ [CONTRADICTION/MAJOR] — The DR-4 patch was applied incompletely in BOTH frozen TDDs: the closure-table row now says the AEAD-nonce96 gate is CLOSED while the normative paragraph immediately below still carries the pre-patch text declaring it OP
- **Yer:** `specs/fec-core-tdd-v3.0.md:466 vs 468-469; specs/kama-noise-wire-tdd-v2.0.md:707 vs 709-710`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/fec-core-tdd-v3.0.md:466: "| AEAD nonce96 mapping | DR-4 KOŞUL-A LE64-lock (noise-wire §4.5); envanter=NO_VECTOR/debt=NO | CLOSED |" — followed at :468-469 by "**Tüm-ÖNCEKİ-gate'ler-CLOSED; AEAD-nonce96-mapping-OPEN.** ... cryptographic-profile-freeze DR-4-kapanana-kadar-ASKIDA." Identical CLO
- **Maliyet:** The next crypto-profile task reads a sealed doc that simultaneously asserts the freeze is CLOSED and ASKIDA; whichever half an implementer or auditor trusts, the other half forces a stop-and-ask round
- **Şüpheci-daraltması:** The textual defect is real and exactly as cited: fec-core-tdd-v3.0.md:466 '| AEAD nonce96 mapping | ... | CLOSED |' vs :468 '**Tüm-ÖNCEKİ-gate'ler-CLOSED; AEAD-nonce96-mapping-OPEN.** ... ASKIDA', identical pair at kama-noise-wire-tdd-v2.0.md:707/:70

### S2-noise.22 ✅ [GAP/MAJOR] — The dh_epoch-transition grace window is a CAL-marked knob (max(4×sRTT, 64-packet-equivalent)) sitting inside a gate marked CLOSED, with no calibration procedure, no acceptance criterion, and no definition of what "64-pak
- **Yer:** `specs/kama-noise-wire-tdd-v2.0.md:448-449, 705`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/kama-noise-wire-tdd-v2.0.md:448-449: "GRACE sabit-2s-DEĞİL: bounded = max(4×sRTT, GRACE_MIN_PACKETS=64-paket-eşdeğeri); hostile-reorder/RTO-bütçesine-bağlı [CAL — D1-ölçümle-kilitlenir]. Grace-bitince epoch-0-recv-state-discard." Yet the gate table (:705) records it CLOSED: "| dh_epoch transit
- **Maliyet:** Sprint-6's epoch state machine must invent the grace policy (wrong value = either premature epoch-0 discard dropping in-flight packets under hostile reorder, or unbounded dual-key window), then re-lit

### S2-noise.23 ✅ [DRIFT/MAJOR] — P4 §2 'DONDURULMUŞ YÜZEY' still presents the superseded v0 surface as the frozen contract while the code (and §3's own v0.1/v1.0 log) is v1: §2.1 pins connect/bind taking Arc<dyn SecuritySeam> but runtime.rs:279/715 take
- **Yer:** `docs/design/P4-TRANSPORT-API-FREEZE.md:74-76, 90, 94, 106-108`  · küme: my-approved-docs A (roadmaps+design)
- **Kanıt:** P4:74: 'ClientEndpoint::connect(SocketAddr, RuntimeConfig, Arc<dyn SecuritySeam>)' and P4:106: 'fn seal(&self, plain: Vec<u8>) -> Vec<u8>;' — vs crates/client-core/src/runtime.rs:279: 'seam: Box<dyn SecuritySeam>,' and crates/client-core/src/seam.rs:93-99: 'fn seal(&mut self, peer: SeamPeer, frame: 
- **Maliyet:** The S2 (Noise) implementer who reads the freeze document's frozen-surface section to build NoiseWireSeam implements the wrong trait and rediscovers the real contract from code — days of rework at the 

### S2-noise.24 ⬇ [MISSING-DECISION/MAJOR] — The ccr-codec/wire-envelope ownership question (does ccr §35 SESSION_* replace wire-v0, wrap kama-noise-wire's sealed format, or both?) is deferred by each document to a different, non-existent resolution point — the pla
- **Yer:** `KAMA-URUN-BITIRME-PLANI-2026-08-26.md:41 (and P1-RUNTIME-SHELL.md:15)`  · küme: my-approved-docs A (roadmaps+design)
- **Kanıt:** PLAN:41: '| Wire envelope (CCR) | **ccr-codec** | **3-SATIR İSKELET** | SESSION_* zarfları (noise-wire ile örtüşme sprint-0'da netleşir) |' — no sprint named 'sprint-0' exists in the plan's §3 sprint list (S0 = 'Burst re-proof'); P1-RUNTIME-SHELL.md:15: 'ccr §35 SESSION_DATA finalizasyonu güvenlik-d
- **Maliyet:** S2 starts with three documents each pointing the envelope decision at a different deferred milestone; the implementer either invents a framing (spec-drift risk against ccr-format-spec §35) or stalls t
- **Şüpheci-daraltması:** The core gap is real: PLAN:41's 'noise-wire ile örtüşme sprint-0'da netleşir' is a dangling reference — 'sprint-0' appears nowhere else in the repo and the plan's sprint list has S0 = burst re-proof, not a scoping task — and the replace/wrap fork bet

### S2-noise.25 ✅ [DRIFT/MAJOR] — CLAUDE.md's project map says the workspace is 8 crates, but Cargo.toml has 14 members — the six unlisted crates include kama-noise-wire and crypto-adjacent kama-fec-codec, i.e. exactly where Sprint-6 work lands, so the p
- **Yer:** `CLAUDE.md:11-13 vs Cargo.toml:3-17 (+ runbook:15,331)`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** CLAUDE.md:11-13: 'Rust workspace (ADR-12). 8 crate: / crypto · ccr-codec · adaptive-transport · hub-server · client-core · ra-ca · discovery · admin-server' — Cargo.toml:3-17 members additionally include 'crates/kama-transport-schema', 'crates/kama-fec-codec', 'crates/kama-noise-wire', 'crates/kama-
- **Maliyet:** Scope decisions keyed to the crate map (CI gate coverage, audit boundaries, solo-ownership claims) silently exclude six crates including the Sprint-6 security-plane code.

### S2-noise.26 ✅ [GAP/MAJOR] — The CLAUDE.md §3 authority registry (the closed list backing 'spec wins') omits three frozen specs that exist in specs/ — kama-noise-wire-tdd-v2.0.md (the Sprint-6 Noise authority), fec-core-tdd-v3.0.md, and DR-4-DR-5-ka
- **Yer:** `CLAUDE.md:17-30 vs specs/kama-noise-wire-tdd-v2.0.md:1-8 + specs/fec-core-tdd-v3.0.md:1-8`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** CLAUDE.md:17: '## 3. Otorite belgeler (donmuş spec'ler — `docs/specs/`)' lists 11 files ending at 'adaptive-transport-engine-spec.md' with 'Bunlar KAT-1..5 GO-FREEZE.' — but specs/ also holds 'kama-noise-wire-tdd-v2.0.md' whose header reads 'STATÜ — DR-4 CLOSED ... Sprint-4: GO ... Sprint-durum-otor
- **Maliyet:** A Sprint-6 session that trusts the registry as the frozen-spec universe misses the Noise wire TDD entirely or cannot resolve which document wins when it conflicts with a registered spec — a rediscover


## S3 — hub/rendezvous — 9 bulgu (BLOCKER=5)

### S3-hub.1 ✅ [GAP/BLOCKER] — HANDLE_ASSIGN is CLOSED/wire-frozen and rides an authenticated hub<->client Noise channel, but that channel (Noise IK/KK per transport §0.1) has no byte-level specification in any document, no implementation anywhere (cr
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:237 + /home/ryuzaki/Desktop/kama-core/specs/transport-layer-tdd.md:30`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** noise-wire:237 'HANDLE_ASSIGN [WIRE-FROZEN — TEK-yol, exact-layout]: hub→client hub-control-mesajı (hub↔client-Noise-transport-kanalında-authenticated; E2E-endpoint-keyi-DEĞİL)' + transport-layer-tdd:30 'Client↔Hub Noise handshake : IK (yeni kayıt, sk_device possession) / KK (kayıtlı, 0-RTT).' — gre
- **Maliyet:** S3-hub cannot deliver h_ext distribution (the extended parser is unusable without it — spec:237 'extended-parser-bunsuz-çalışmaz'), forcing either an invented hub-channel wire format or a stall while 

### S3-hub.2 ✅ [GAP/BLOCKER] — The hub-mesh RPC layer (Plane 3) is declared frozen/implementation-ready but has no wire framing at all — no request/response envelope, no correlation id for concurrent UDP lookups, no response encoding, and FIND_VALUE's
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/routing-rendezvous-architecture.md:822`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** Line 800: "RPC payload encoding: Kama-native fixed-width (ccr-spec primitive kuralları; JSON/protobuf değil). Her RPC bir method_id (u8) taşır." — that one sentence is the entire framing spec. Line 822-825: "FIND_VALUE { routing_key: bytes16 } → { authorized_hubset: AuthorizedHubSetRecord, routing_r
- **Maliyet:** S3 hub-server stalls on day 1 or invents a mesh wire format that a later corrigendum must break — the exact spec-gap→campaign→corrigendum cycle that turned 3 transport weeks into 8.

### S3-hub.3 ✅ [CONTRADICTION/BLOCKER] — The AHS production pipeline is contradictory and incomplete: ADMIN_PREPARE_AHS is org-wide and carries no dest_hash, while the SRAH record it must produce is mandatory per-dest_hash (routing's verify_routing_record drops
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/admin-control-plane-tdd.md:391`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** admin:380 "Alpha AHS granularity = org-wide AHS." and admin:391-396 "ADMIN_PREPARE_AHS payload { ahs_sequence : u64_be; hub_count : u16_be; authorized_hubs : HubAuthDraft[hub_count]; not_after : u64_be }" (no dest_hash field) vs routing:666 "dest_hash : bytes32 # kapsanan kimlik" (mandatory SRAH fie
- **Maliyet:** With the mandatory-AHS gate (routing:701) unresolvable, every RoutingRecord is DROPped and routing is dead on arrival in S3 integration; the fan-out/trigger/injection decisions get invented ad hoc acr

### S3-hub.4 ⬇ [DRIFT/BLOCKER] — App-RPC mandates hub→client server-initiated frames (0x81 DELIVER, 0x83 REVOCATION_NOTIFY, 0x84 ROUTE_MISS) and request/response pairs on every method, but the frozen transport-API-v1 runtime is strictly unidirectional c
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/application-protocol-and-rpc-spec.md:125`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** app-rpc:125-130 "Hub → Client (server-initiated), method_id 0x80..0xFF (bit7=1): 0x81 DELIVER ... 0x84 ROUTE_MISS" vs crates/client-core/src/runtime.rs:732-750: impl ServerHandle exposes only local_addr()/next_delivery()/shutdown(); ClientHandle (runtime.rs:310-312) exposes only send()/acked()/shutd
- **Maliyet:** S3 hub-server cannot deliver mailbox items, answer any RPC, or push ROUTE_MISS over the product transport without a semver-major API break (per P4 §3) that the freeze did not anticipate — rework of th
- **Şüpheci-daraltması:** The facts are accurate: app-rpc:125-130 defines 0x81/0x83/0x84 server-initiated methods; runtime.rs ServerHandle exposes only local_addr/next_delivery/shutdown and ClientHandle only send/acked/shutdown (verified by awk over both impl blocks); P4-TRAN

### S3-hub.5 ✅ [MISSING-DECISION/BLOCKER] — No wire discriminator distinguishes a Plane-1 AppRpcFrame (first byte = method_id u8) from a Plane-2 hot-path frame (first 4 bytes = opaque PRF-derived hub_handle) arriving on the same established client↔hub link, yet R-
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/application-protocol-and-rpc-spec.md:69`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** app-rpc:69 "Üç düzlem aynı method_id sayı uzayını kullansa bile çakışmaz: farklı plane, farklı parser, farklı session."; app-rpc:52 Plane 1 "Taşıma: kurulu Client↔Hub Noise session (transport substrate)." — the same substrate Plane 2 uses; transport-layer-tdd:77 the hot-path frame begins "hub_handle
- **Maliyet:** Every implementer invents a different demux rule (interop break), an opaque hub_handle can collide with any method_id prefix, and the 0x0406 WRONG_PLANE behavior is untestable as specified.

### S3-hub.6 ✅ [AMBIGUITY/MAJOR] — SAN/nameConstraints IP entries are 'kanonik string formu' with the canonical form never defined (no RFC 5952 citation, no case/compression rule for IPv6, no leading-zero rule for IPv4), yet validate_hub_entry step 12 mus
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:1640 (used by 1196)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** Line 1640: `**DNS/IP:** ASCII; IP IPv4 (4 byte) veya IPv6 (16 byte) için kanonik string formu.` and line 1196: `12. entry.addresses, hub_identity_cert SAN (DNS/IP) policy ile eşleşir (san_matches_addresses).`
- **Maliyet:** Producer and verifier canonicalize IPv6 differently (e.g. '2001:DB8::1' vs '2001:db8:0:0:0:0:0:1') → valid hubs rejected in the field, the exact interop-DoS class the spec's own PARSER-01 note warns a

### S3-hub.7 ✅ [MISSING-DECISION/MAJOR] — The frozen verify_announce algorithm calls valid_hub_noise_rotation_overlap(...), a function defined nowhere — no grace-window duration, no overlap-validity algorithm, no acceptance criterion — even though it alone decid
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/routing-rendezvous-architecture.md:272`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** routing:271-272 "if ann.hub_noise_cert.public_key != expected_noise_cert.public_key: if NOT valid_hub_noise_rotation_overlap(ann.hub_noise_cert, expected_noise_cert, hub_entry, now): DROP" — sole occurrence in the repo, never specified. The only prose is routing:383 "eski/yeni cert overlap grace izi
- **Maliyet:** Hub key rotation behavior is invented per-implementer: too permissive re-opens the key-substitution surface §3.4 exists to close, too strict causes a mesh-wide outage on every routine Noise-key rotati

### S3-hub.8 ✅ [GAP/MAJOR] — The ROUTE_MISS→SESSION_REBIND recovery loop is unimplementable as written: link_ref (bytes16) appears only in these two messages and is defined nowhere in the repo (no derivation, no assignment moment, no relation to the
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/application-protocol-and-rpc-spec.md:337`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** app-rpc:337 "SESSION_REBIND request  { link_ref : bytes16; reason : u8 }" and app-rpc:374 "ROUTE_MISS (0x84): payload: { link_ref bytes16 }" — the only two occurrences of link_ref in the entire repo (verified by repo-wide grep). ccr §35.16 (ccr:2645-2661) defines rebind purely in terms of hub_handle
- **Maliyet:** R-14's "✓ Kapalı (bounded)" verdict rests on an unimplementable message pair; client and hub teams invent incompatible 16-byte refs and different K, and the bounded-DoS property cannot be conformance-

### S3-hub.9 ✅ [GAP/MAJOR] — The framing header promises per-method payload limits "in §11", but §11 is the error-code taxonomy and contains no limits table; the mailbox capacity behind 0x0303 MAILBOX_FULL and the FETCH_MAILBOX max_items bound are l
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/application-protocol-and-rpc-spec.md:80`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** app-rpc:80 "payload_len   : u32_be      # ≤ MAX_APP_PAYLOAD (default 1 MiB; method-bazlı daha sıkı limit §11)" — but §11 (app-rpc:507-569) is titled "Error Code Taksonomisi (u16_be, sınıf = high byte)" and defines only codes; app-rpc:538 "0x0303 MAILBOX_FULL" with no capacity/quota defined anywhere 
- **Maliyet:** A broken internal reference plus missing bounds yields divergent per-hub limits and an unbounded-allocation surface (1 MiB control frames on a hostile link, uncapped mailboxes) that the project's own 


## S4 — PKI + discovery — 15 bulgu (BLOCKER=5)

### S4-pki-discovery.1 ✅ [CONTRADICTION/BLOCKER] — The three authority docs disagree on who signs ra_attestation (identity says Intermediate CA, pki and discovery say RA Identity), and no doc defines its byte format (ccr declares it opaque), so RA cannot produce it and v
- **Yer:** `specs/identity-and-trust-model.md:813 (vs specs/pki-ca-ra-hsm-tdd.md:125, specs/discovery-architecture.md:322)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:813 "ra_attestation : bytes      # Intermediate CA imzalı (phone→identity bağı)" and identity:940 "ICA->>ICA: DiscoveryRecord ra_attestation imzala (phone→identity)" — vs pki:122-125 "RA Identity: → RevocationRecord → RevocationPush → operational RA attestations (ra_attestation, discovery b
- **Maliyet:** ra-ca issuance flow and discovery-server PUBLISH validation get built against different signing keys, guaranteeing an interop failure and a corrigendum cycle in the first S4 integration week.

### S4-pki-discovery.2 ⬇ [MISSING-DECISION/BLOCKER] — The 'Revocation Service' that produces Intermediate-signed Snapshot/Delta checkpoints on a ≤24h cadence is an undefined actor (no placement, key, cert, or trigger path in any spec), and pki's normative rule that Intermed
- **Yer:** `specs/pki-ca-ra-hsm-tdd.md:297-298 (vs specs/revocation-model.md:794,818)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** pki:297-298 "NORMATİF: Intermediate auto-sign YALNIZCA bu 3 gate geçtikten sonra çalışır. RA gate geçmeden Intermediate imza ÜRETMEZ." vs revocation:794 "Revocation Service snapshot/delta sequence'ini günceller (Intermediate-imzalı checkpoint)." — repo-wide grep shows 'Revocation Service' appears on
- **Maliyet:** The ra-ca implementer must invent a new trusted component and a gate-exempt Intermediate signing path in week 1 — exactly the invented-behavior class that later forces a spec corrigendum and re-audit 
- **Şüpheci-daraltması:** The narrow fact holds: 'Revocation Service' appears only as a name (specs/revocation-model.md:23,794,799,818; specs/admin-control-plane-tdd.md:512 — grep-verified) and the pki §2 topology (specs/pki-ca-ra-hsm-tdd.md:56-70) does not list it; its place

### S4-pki-discovery.3 ✅ [CONTRADICTION/BLOCKER] — Identity's normative validator pseudocode and test cases match revocation by serial while frozen revocation-model mandates cert_hash as the sole primary handle (serial = optional scope_hint only), so a validator transcri
- **Yer:** `specs/identity-and-trust-model.md:1753,1772,2412 (vs specs/revocation-model.md:193,210)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:1753 "if fresh_crlite.contains(hub_identity_ccr.serial): return FALSE" (repeated at identity:1772 in verify_hub_announce) and identity:2412 test 33 "Çıplak device_id ile CRLite revocation eşleşmesi → REJECT (issuer_key_id+serial kullanılmalı, 18.3)" and identity:1219 "7. CRLite: bob device 
- **Maliyet:** Hub/RA cert revocation checks silently no-op (filter keyed by cert_hash, lookup by serial), reproducing the exact spec-vs-spec drift class that cost weeks on transport, this time with a security-inver

### S4-pki-discovery.4 ⬇ [CONTRADICTION/BLOCKER] — First-onboarding transport is contradictory: pki §14 says the client connects to RA DIRECTLY (bootstrap-direct, no hub list yet), while identity A-03 states absolutely that the client never connects to RA directly and id
- **Yer:** `specs/pki-ca-ra-hsm-tdd.md:488 (vs specs/identity-and-trust-model.md:134,401-402)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** pki:488 "2. Client → RA (DOĞRUDAN; hub listesi yok): org_id + CSR (Identity + Device pub)." vs identity:134 "`Confirmed (A-03 — RA proxy):` Client RA'ya **doğrudan bağlanmaz**. Client→Hub→RA (hub proxy, gerçek adresi saklar)." and identity:401-402 "PARÇA 2 — RA'dan Onboarding'de (kuruma özel): Clien
- **Maliyet:** REGISTER_INIT — the first flow ra-ca and client-core must implement — has two mutually exclusive network paths with different threat models (client IP exposure to RA vs hub), stalling enrollment imple
- **Şüpheci-daraltması:** The quoted lines are real: specs/pki-ca-ra-hsm-tdd.md:488 'Client → RA (DOĞRUDAN; hub listesi yok)' vs specs/identity-and-trust-model.md:134 'Client RA'ya doğrudan bağlanmaz. Client→Hub→RA' (unqualified) and :401-402 routing onboarding through a hub.

### S4-pki-discovery.5 ✅ [GAP/BLOCKER] — The client↔Discovery Server protocol messages (PUBLISH, VOPRF_BLIND_BATCH, LOOKUP_BATCH, RA→Discovery REGISTER_PUSH, response entries, padding encoding, error surface) are never byte-specified anywhere in the repo; app-r
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/discovery-architecture.md:464`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** app-rpc:282 "payload : bytes[payload_len]   # discovery-architecture.md formatı (OPAK; hub yalnız proxy)" vs discovery:463-469, the entire definition: "PUBLISH { DiscoveryRecord, CompactDeviceCert } → { ack | error }; VOPRF_BLIND_BATCH { blinded_1..blinded_B } → { evaluated_1..B, proof }; LOOKUP_BAT
- **Maliyet:** S4 must invent the entire discovery wire protocol including the Noise_NK E2E channel framing; client and Discovery Server implemented from the same spec will not interoperate, and AC-DISC-3/-10 are un

### S4-pki-discovery.6 ✅ [GAP/MAJOR] — compact_device_cert_tbs — the signature input of the single cert that travels on the wire — begins with domain_sep("SR_CDC_V1") declared as a '16B sabit etiket', but "SR_CDC_V1" is 9 ASCII bytes and the domain_sep() padd
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:536 (also 1848)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** Line 536: `domain_sep("SR_CDC_V1") ||           # domain separation (16B sabit etiket)`
- **Maliyet:** Two independent implementations (Java RA/CA vs Rust validator, per AC-01/AC-06) pad differently → every CompactDeviceCert issuer_signature fails cross-verification; a one-line definition now avoids a 

### S4-pki-discovery.7 ✅ [GAP/MAJOR] — PARSER-02 class on the untrusted onboarding path: HubListRecord hub_count is u16_be (up to 65535 cert-carrying HubEntryV3s), HubEntryV3 address_count and BootstrapBundle ra_server_count are 'u8 # 1..N' with N never bound
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:1105 (also 1120, 1443, 1452; §6.7 at 283-296)`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** Line 1105: `hub_count           : u16_be      # liste uzunluğu` and line 1443: `ra_server_count      : u8          # 1..N` — while §6.7 (283-296) caps only eku/extension/name/ra_attestation/endpoint/CCR-total/bundle-total.
- **Maliyet:** A bounds-before-alloc (SR-AUDIT-001) parser cannot be written without inventing the caps; a hostile bootstrap blob can force multi-MB allocations, and later cap-freezing invalidates deployed records.

### S4-pki-discovery.8 ✅ [GAP/MAJOR] — DeviceRevocationRequest — the user-signed object that starts every device revocation — has no canonical byte format anywhere (identity defers to revocation-model, whose scope list omits it and which mentions it only in a
- **Yer:** `specs/identity-and-trust-model.md:2072,2104 (vs specs/revocation-model.md:57-80,236)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:2072 "issuer_key_id         : bytes32     # Device CCR issuer (Identity CA) key id" vs identity:2094 "5. req.issuer_key_id == CompactDeviceCert.issuer_key_id  # Identity/User Cert subject_key_id[0:16]" (CompactDeviceCert issuer_key_id is 16B per identity:21 "issuer_key_id(16)"); identity:21
- **Maliyet:** The client→RA revocation entry point gets an invented, unfrozen wire format with a latent 16-vs-32-byte field bug, requiring a re-freeze and client/RA lockstep change after S4.

### S4-pki-discovery.9 ✅ [GAP/MAJOR] — The CT plane is sealed-but-incomplete: identity's two TODOs defer CT signing-key lifetime/rotation and checkpoint binary format to a 'revocation-model.md ek' that does not exist (revocation-model declares CT Log encoding
- **Yer:** `specs/identity-and-trust-model.md:747,2171 (vs specs/revocation-model.md:75)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:747 "`TODO(IMPL): CT Log signing key lifetime ve rotation ceremony → revocation-model.md ek.`" and identity:2171 "`TODO(IMPL): CT Log signing key lifetime/rotation; CT checkpoint binary format → revocation-model.md ek.`" vs revocation:75 Non-goals "- CT Log encoding"; identity:744 "issuer |
- **Maliyet:** The audit/CT plane pki §15 requires of ra-ca cannot be implemented without inventing key policy and a checkpoint format, and issuance flows (identity §10.1 logs to CT) will ship with an undefined depe

### S4-pki-discovery.10 ✅ [DRIFT/MAJOR] — The active canonical validator verify_discovery_record (§15.7) still contains three checks against an undefined `pb.*` object — the removed PrekeyBundle — plus an unused x25519 binding, so the frozen pseudocode is uncomp
- **Yer:** `specs/identity-and-trust-model.md:1734-1737`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:1734-1737: "x25519 = record.current_device_x25519\n  if pb.identity_hash != record.identity_hash: return FALSE\n  if pb.device_cert_hash != record.current_device_cert_hash: return FALSE\n  if record.valid_until > pb.record_valid_until: return FALSE" — `pb` is defined nowhere; PrekeyBundle w
- **Maliyet:** The discovery-record validator implementer must guess which of two overlapping validators (15.6b vs 15.7) is canonical and what the pb checks were meant to bind, inviting either dead code or a missed 

### S4-pki-discovery.11 ⬇ [AMBIGUITY/MAJOR] — verify_chain_to_root is defined with signature (ccr, ROOT_CA_SPKI_HASH) but every wrapper calls it with fresh_crlite as the second argument, and the chain-walk body never consults revocation for intermediate hops — leavi
- **Yer:** `specs/identity-and-trust-model.md:1491 (vs 1562,1788,1831)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:1491 "verify_chain_to_root(ccr, ROOT_CA_SPKI_HASH) -> bool:" vs identity:1562 "if NOT verify_chain_to_root(identity_cert, fresh_crlite): return FAIL", identity:1788 "if NOT verify_chain_to_root(intermediate, fresh_crlite): return FALSE", identity:1831 "if NOT verify_chain_to_root(admin_cert
- **Maliyet:** Implementers will diverge on whether chain hops are revocation-checked (a security-relevant choice the spec was supposed to have frozen), producing inconsistent fail-closed behavior across client, hub
- **Şüpheci-daraltması:** Partially verified, twice overstated. Real: the definition at specs/identity-and-trust-model.md:1491 takes (ccr, ROOT_CA_SPKI_HASH), the body :1491-1521 performs pin+signature walking with no revocation lookup for any hop, and three call sites pass f

### S4-pki-discovery.12 ✅ [DRIFT/MAJOR] — Identity's normative freshness algorithm fetches a monolithic 'CRLite filter' via a GET_LATEST_CRLITE RPC that exists in no protocol spec — the actual frozen surface is the cursor-based REVOCATION_FETCH_SNAPSHOT/DELTA/ST
- **Yer:** `specs/identity-and-trust-model.md:2124-2128 (vs specs/application-protocol-and-rpc-spec.md:120-122, specs/revocation-model.md:479-516)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:2124 "C->>P: GET_LATEST_CRLITE" and identity:1432 "f = hub.get_latest_crlite()" — repo-wide grep finds GET_LATEST_CRLITE only in identity (3 hits, lines 2124/2126/2128); the app-RPC namespace defines only "0x09 REVOCATION_FETCH_SNAPSHOT (SRFQ → SRFR snapshot...) / 0x0A REVOCATION_FETCH_DELT
- **Maliyet:** The client-side freshness engine (get_or_refresh_crlite_once_per_connection is the gate for every trust decision) must be reverse-engineered onto a differently-shaped protocol, and the multi-source su

### S4-pki-discovery.13 ✅ [UNTESTABLE/MAJOR] — E-14's uniform-reject rule demands the RA make invalid-selector and unauthorized-user rejections indistinguishable in message, status code, timing (constant-time), and retry behavior, but defines no measurement procedure
- **Yer:** `specs/identity-and-trust-model.md:64 (E-14)`  · küme: pki+identity+revocation (S4-kritik)
- **Kanıt:** identity:64 "**Uniform red yalnız mesaj DEĞİL: response status kodu, timing (constant-time), ve retry davranışı da geçersiz-selector ile yetkisiz-kullanıcı arasında AYIRT ETTİRMEMELİ**" — no acceptance criterion anywhere (identity §26 has no AC for it); the reject path at identity:1821 executes "not
- **Maliyet:** The RA either ships silently non-compliant (enumeration side-channel stays open against the stated threat) or the team burns days inventing a timing-verification harness mid-sprint — the Q18 underspec

### S4-pki-discovery.14 ✅ [CONTRADICTION/MAJOR] — ra_attestation — which the Discovery Server MUST verify field-by-field on every PUBLISH — has no canonical byte format anywhere (ccr declares it opaque and defers back to the RA/discovery docs, which define nothing), and
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/discovery-architecture.md:322`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** discovery:322 "Discovery Server PUBLISH'i REDDEDER eğer: `ra_attestation.identity_hash != DiscoveryRecord.identity_hash` ... veya attestation expired / RA-imzalı değil."; ccr-format-spec:1040 "ra_attestation : varbytes16    # max 512 byte; opaque (RA/discovery docs)"; identity-and-trust-model:813 "r
- **Maliyet:** The PUBLISH anti-spoof gate (AC-DISC-9) is unimplementable and untestable; choosing the wrong signer breaks Discovery↔RA interop and silently weakens the phone→identity binding the whole discovery tru

### S4-pki-discovery.15 ✅ [CONTRADICTION/MAJOR] — Batch size B is simultaneously an open TBD in discovery ("256?", to be measured, with no calibration procedure or acceptance criterion) and a frozen normative constant in app-rpc (B=256); the per-user enumeration quota i
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/discovery-architecture.md:523`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** discovery:523 "Sabit batch boyutu B (256?) — performans/gizlilik dengesi ölçülüp netleşecek (TDD)." vs app-rpc:576 "DISCOVERY_PROXY (online enumeration): per-user + per-org quota + anomali tespiti (Signal modeli); sabit batch B=256 padding." (stated as normative). discovery:197 "Per-user batch quota
- **Maliyet:** Implementers freeze divergent B/N values; if measurement later moves B the already-normative app-rpc line forces a corrigendum, and the D-4 'closed' enumeration claim is unmeasurable until N exists.


## S5 — admin — 1 bulgu (BLOCKER=1)

### S5-admin.1 ✅ [GAP/BLOCKER] — The freeze-ready Admin↔RA contract is unimplementable across its two parties: the payload types HubEntryDraft, HubAuthDraft, DiscoveryEndpointDraft and SanEntry are used in method schemas but defined nowhere in the repo;
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/admin-control-plane-tdd.md:327`  · küme: routing+discovery+app-rpc+admin (S3/S5-kritik)
- **Kanıt:** admin:327 "hub_drafts         : HubEntryDraft[hub_count]", admin:394 "authorized_hubs    : HubAuthDraft[hub_count]", admin:414 "endpoints ... : DiscoveryEndpointDraft[endpoint_count]", admin:351 "expected_sans : SanEntry[expected_san_count]" — none of these four types is defined anywhere (repo-wide 
- **Maliyet:** Admin Server (customer org) and RA (RoftCore) are implemented by different parties from this contract; without the type definitions and transport mapping, S5 stalls or the two sides ship non-interoper


## Transport-bakım (motor/spec hizası) — 14 bulgu (BLOCKER=0)

### transport-maint.1 ⬇ [DRIFT/MAJOR] — The A-3 control_id dedup contract (fixed-window/high-water with a distinct ControlIdBelowDedupWindow diagnostic) is implemented only behind the default-OFF experimental-cold-start feature, while the default build's Inbou
- **Yer:** `/home/ryuzaki/Desktop/kama-core/crates/kama-noise-wire/src/retry.rs:62 vs /home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:346-349`  · küme: noise-wire+crypto (S2-kritik)
- **Kanıt:** Spec:347-349 'Receiver-dedup fixed-window/high-water modeliyle uygulanabilir; window-altına-düşen-id → yanıt `DuplicateAckOnly` + **ayrık teşhis `ControlIdBelowDedupWindow`**...'. Default build: retry.rs:62 'seen: HashSet<ControlId>' (never evicted; no below-window diagnostic). Conformant version ga
- **Maliyet:** Long sessions grow dedup state without bound in the shipped default path, and when the window model is finally promoted, its below-window semantics will differ from what integrators tested against.
- **Şüpheci-daraltması:** The code divergence is real but two of three supporting legs fail. Real: crates/kama-noise-wire/src/retry.rs:62 'seen: HashSet<ControlId>' — never evicted, no below-window diagnostic — is the default-build export (lib.rs:80), while the conformant fix

### transport-maint.2 ✅ [DRIFT/MAJOR] — The shipped Q18-fix — a structure-aware FEC gate that overrides the fuzzy controller's fec=0 output using three unsanctioned magic constants and an entirely unspec'd fate_estimator module — exists only in code; both auth
- **Yer:** `crates/adaptive-transport/src/sender.rs:909-916,2812-2853 vs specs/transport-layer-tdd.md:552-553 (§5.3) and specs/adaptive-transport-engine-spec.md:648-654 (§8`  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** sender.rs:909 `const STRUCTURE_GATE_LOSS_MIN_PPM: u64 = 100_000;` :912 `const STRUCTURE_GATE_MIN_N: u32 = 32;` :916 `const STRUCTURE_GATE_IID_MAX_DELTA_PPM: i64 = 50_000;` and :2812 `if let Some(fec) = self.structure_gate_fec(d.fec_parity_ratio_permille, inputs.loss_ppm)`; tests/structure_gate_fec.r
- **Maliyet:** Next implementer or auditor reading the frozen rule tables will re-derive the known-defective fec=0 behavior or 'fix' the gate away; the three thresholds are exactly the uncalibrated-knee class that a

### transport-maint.3 ✅ [CONTRADICTION/MAJOR] — The new DR-EST-1 establish-probe is a default-ON, token-EXEMPT send path that directly violates the engine spec's normative single-pacer sentence, and its wall-clock constants (0.5s initial, 8s cap) appear in no spec, no
- **Yer:** `crates/adaptive-transport/src/sender.rs:933-934,1293,3193-3197,4325-4345 vs specs/adaptive-transport-engine-spec.md:332-333 (§3.3-B)`  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** Spec :332-333: "Kopyalar da AYNI pacer'dan geçer (§13); pacer'ı baypas eden gönderim yolu YOK (DATA/retransmit/kopya tek token bucket'ı paylaşır)." vs sender.rs:4325: "[DR-EST-1] ... Tek-slot, backoff'lu, token-muaf.", :3195-3196: "Bu kol vadesi gelen EN-ESKİ kopyadan TEK datagram'ı token-MUAF geçir
- **Maliyet:** Either the spec's [R8] 'self-congestion imkânsız' claim is now false (a bounded pacer-bypass exists) or the code must change before merge; unrecorded, this becomes the next corrigendum campaign when a

### transport-maint.4 ✅ [DRIFT/MAJOR] — The normative §12 timer set — IDLE_TIMER, RETRY_BUDGET (deadline-derived), PATH_DEAD_TIMER with failover responsibility — is entirely unimplemented (Timer enum has only Pto and RtpropProbe), which the spec's own inventor
- **Yer:** `specs/adaptive-transport-engine-spec.md:876-885 (§12) and :1084 vs crates/adaptive-transport/src/sender.rs:565-570 (Timer enum)`  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** Spec :878: "RETRY_BUDGET: [K6/R4] SABİT-SAYI DEĞİL, DEADLINE-TÜREVİ ... zaman-tavanı var", :880: "PATH_DEAD_TIMER: [R4] no_ack(T) → PATH_DEAD ... Failover sorumluluğu RETRY_BUDGET'tan buraya taşındı"; :1084: "ölü kanalda sonsuz reschedule (COPY_MAX_INTERVAL/RETRY_BUDGET kodda yok)"; sender.rs:565-57
- **Maliyet:** TDD :179's 'bounded operational envelope' and blackout-survival positioning are unenforceable in the shipped engine (unbounded retry, no path-death escalation, no failover hook), and any Tier-2 surviv

### transport-maint.5 ⬇ [CONTRADICTION/MAJOR] — Canonical-wire-codec ownership is forked three ways: ADR-05 names ccr-codec the single-source crate for canonical encode/decode, kama-noise-wire-tdd §11.5 (PATCH-7, 'KARAR VERİLDİ') assigns 'canonical-serialization' to a
- **Yer:** `/home/ryuzaki/Desktop/kama-core/specs/tech-stack-adr.md:43 (also 126) vs /home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:627-630 vs /home/ryuz`  · küme: ccr-format (S35 + parsers)
- **Kanıt:** tech-stack-adr.md:43: `ccr-codec               (canonical encode/decode/verify — tek kaynak crate)` vs kama-noise-wire-tdd:628: `**`kama-transport-schema`-içeriği:** logical-packet-enums · canonical-serialization ·` vs legacy.rs:1: `//! Legacy SESSION_DATA (22B) codec — ccr §35.6/§35.9 AKTİF-format`
- **Maliyet:** When ccr-codec is finally implemented (S2+), the §35 envelope exists in two crates with two owners and no stated master → duplicate-definition drift, exactly the raw-key-divergence class the spec elim
- **Şüpheci-daraltması:** Each citation checks out literally (tech-stack-adr.md:43 'ccr-codec (canonical encode/decode/verify — tek kaynak crate)' and :126-129; kama-noise-wire-tdd:627-630 PATCH-7 'canonical-serialization' in kama-transport-schema, KARAR VERİLDİ; legacy.rs:1 

### transport-maint.6 ✅ [GAP/MAJOR] — The normative FEC controller is a 6-input profile selector (choose_profile) with an explicit ban on a fixed formula, an unset target_block_failure_prob, and an uncalibrated 1.75-2.0 constant — no mapping, calibration pro
- **Yer:** `specs/fec-core-tdd-v3.0.md:278-280, 287, 372`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/fec-core-tdd-v3.0.md:278: "profile = choose_profile(q_raw, burst_p95, reorder, reverse_health, deadline, target_block_failure_prob)" — target_block_failure_prob is never assigned a value in any spec; :372: "**K-FEC-2 (controller):** q̂=0→parite=0 ...; profile-selector (SABİT-FORMÜL-YOK); deadl
- **Maliyet:** FEC default-ON cannot be decided or defended: every controller attempt is unfalsifiable against the spec (no acceptance criterion), guaranteeing another multi-week calibration campaign before the §4.4

### transport-maint.7 ⬇ [DRIFT/MAJOR] — The frozen normative FecCodec API in fec-core §2 does not match the shipped trait: code added a third reconstruct parameter (known_zero_count, required for the spec's own Model-A decode) and dropped block_id from SourceS
- **Yer:** `specs/fec-core-tdd-v3.0.md:152-157 vs crates/kama-fec-codec/src/codec.rs:23-26, 68-73`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/fec-core-tdd-v3.0.md:152: "pub struct SourceSymbol<'a> { pub block_id: BlockId, pub esi: SourceEsi, pub bytes: &'a [u8] }"; :156: "fn reconstruct(&self, profile: &FecProfile, received: &[ReceivedSymbol<'_>]) -> Result<Vec<RecoveredSource>, FecError>;" vs crates/kama-fec-codec/src/codec.rs:23-2
- **Maliyet:** Any conformance/differential implementation built from the sealed spec API cannot decode partial blocks at all (no K_actual/known-zero input), so cross-impl testing and future audits diverge from the 
- **Şüpheci-daraltması:** The drift itself is real and exactly as cited: fec-core-tdd-v3.0.md:152 'SourceSymbol { pub block_id ... }' and :156 two-parameter reconstruct vs crates/kama-fec-codec/src/codec.rs:23-26 (SourceSymbol has only esi+bytes) and :68-73 (three-parameter r

### transport-maint.8 ✅ [CONTRADICTION/MAJOR] — P3 contradicts itself about the shipped startup-pacing default: the approved-status header (line 6 'G4 onaylı: startup-pacing/bootstrap/FEC OFF kalır'), the context line (line 10 'tüm ürün-varsayılanları şu an OFF (tail 
- **Yer:** `docs/design/P3-DEFAULT-FLIP-DECISION.md:6 and :105 vs :24 and :75`  · küme: my-approved-docs A (roadmaps+design)
- **Kanıt:** P3:6: 'G4 onaylı: startup-pacing/bootstrap/FEC OFF kalır' and P3:105: '| G4 | startup-pacing / bootstrap / FEC OFF kalır | onay |' — vs P3:75: '## 3. Startup-pacing → ZATEN ON (DÜZELTME, 2026-08-26)' and crates/adaptive-transport/src/sender.rs:701: 'Self { enabled: true }'.
- **Maliyet:** The governance record of what Abkarada approved (G4) says a shipped-ON default is OFF; the next defaults audit or flip decision reads the approval table and re-triggers the documented B1-style misflip

### transport-maint.9 ✅ [GAP/MAJOR] — S1-GATE declares the product structure-gate's burst→ARQ rule (STRUCTURE_GATE_IID_MAX_DELTA_PPM = 50_000) must be recalibrated for deadline-constrained operation via a preregistered campaign, but no calibration procedure,
- **Yer:** `reports/s1-gate/S1-GATE-RESULT.md:18 (knob at crates/adaptive-transport/src/sender.rs:916; sibling placeholder at sender.rs:928)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** S1-GATE-RESULT.md:18 '**"burst→ARQ" kuralı deadline-kısıtlı op için yeniden-kalibre edilmeli** (fusion-gate kalibrasyonu; ön-kayıtlı kampanya gerektirir, bu tur YAPILMAZ)' · sender.rs:916 'const STRUCTURE_GATE_IID_MAX_DELTA_PPM: i64 = 50_000;' · sender.rs:927-928 '[CAL PLACEHOLDER]' / 'const PRE_CON
- **Maliyet:** The adaptive Controller keeps handing burst regimes to ARQ in exactly the deadline-pressured cells where S1-GATE measured FEC as the only recovering mechanism; without a defined calibration gate this 

### transport-maint.10 ⬇ [CONTRADICTION/MAJOR] — The seam doc assumes a single signaling blob of 'a few KB' with core handling MTU/fragmentation, but the frozen transport-API surface caps units at 1200 B with a typed refuse and client-core contains zero fragmentation c
- **Yer:** `specs/signaling-seam-v0.md:42 vs docs/design/P4-TRANSPORT-API-FREEZE.md:91,229 + crates/client-core/src/runtime.rs:107,158-159`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** seam:42: '| **max_blob_size** | tek signaling mesajı ≤ birkaç KB (SDP+ICE tipik) | core MTU/fragment davranışı §11 TDD; büyük ICE bundle fragment |' — P4:91: '`max_unit_bytes`(1200)'; runtime.rs:158-159: '#[error("payload {got} B > max_unit_bytes {cap} B")] UnitTooLarge { got: usize, cap: usize }'; 
- **Maliyet:** kama-contract promotion (seam §5) inherits an API that typed-refuses a typical 3-10 KB SDP offer, forcing an unplanned fragmentation layer or a breaking v2 of the just-frozen surface at integration ti
- **Şüpheci-daraltması:** The cited facts hold (seam:42, P4:91, runtime.rs:158-159 UnitTooLarge, 0 'frag' hits in client-core, P4:229-230 'kapasitededir'), but the core assertion 'nobody has decided who fragments' is refuted: transport-layer-tdd.md:633 (§7 NORMATİF) pipeline 

### transport-maint.11 ⬇ [CONTRADICTION/MAJOR] — The runbook's normative C.1 CI definition (and CLAUDE.md §7) still prescribe the all-features-only test pipeline whose blind spot ci.yml itself documents as having hidden two real breakages, so under the 'çelişkide spec 
- **Yer:** `specs/branching-strategy-and-devops-runbook.md:169,186 + CLAUDE.md:91-92 vs .github/workflows/ci.yml:21-25`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** runbook:186: '- run: cargo test --all --all-features                            # unit + conformance vektörleri' and CLAUDE.md:91-92: 'GitHub-hosted (her PR): cargo fmt --check + clippy -D warnings + test --all-features + audit + deny + miri (yalnız crypto, ccr-codec)' — but ci.yml:21: '# DR-RATE-RA
- **Maliyet:** Any CI rewrite or new-repo bootstrap that follows the frozen runbook silently deletes the default-build/feature-matrix gate and re-opens the exact blind spot that already cost a debugging cycle in DR-
- **Şüpheci-daraltması:** Citations check out (runbook:186 'cargo test --all --all-features', CLAUDE.md:91-92, ci.yml:21 blind-spot comment + semantic-matrix at ci.yml:25). But the contradiction/regression framing dissolves: actual ci.yml is a strict superset (the quality job

### transport-maint.12 ✅ [DRIFT/MAJOR] — The runbook's normative branch-protection recipe requires only the original three status checks, so the four later-added gates — including the semantic-matrix default-build gate added specifically because a breakage hid 
- **Yer:** `specs/branching-strategy-and-devops-runbook.md:243,252-254 vs .github/workflows/ci.yml:25,94,104`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** runbook:243: '☑ Require status checks to pass:  quality · security · miri' and runbook:252-254: '-f "required_status_checks[contexts][]=quality" ... =security ... =miri' — ci.yml now defines additional jobs `semantic-matrix` (ci.yml:25), `aead-differential` (ci.yml:94), `fuzz-smoke` (ci.yml:104) tha
- **Maliyet:** A red default-build, AEAD-differential, or fuzz gate can be merged into main while 'all required checks pass', reintroducing the CI-blind-spot failure class the matrix was built to close.

### transport-maint.13 ⬇ [DRIFT/MAJOR] — CLAUDE.md declares the authority-spec tree at `docs/specs/`, which does not exist (specs live at top-level `specs/`), and the signaling seam doc is referenced under three different paths across the three cross-cutting do
- **Yer:** `CLAUDE.md:3,15,17 + specs/signaling-seam-v0.md:3`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** CLAUDE.md:3: 'Otorite belgeler `docs/specs/` altında; çelişkide **spec kazanır**.' and CLAUDE.md:15: '(bkz. `docs/specs/signaling-seam-v0.md`)' — but `ls docs/specs/` → 'No such file or directory'; the file is at `specs/signaling-seam-v0.md`, whose own header (line 3) says '**Belge:** `docs/signalin
- **Maliyet:** Every fresh session, tool, or reviewer that resolves an authority question via CLAUDE.md's path lands on a missing directory and either stalls or answers from priors instead of the frozen spec.
- **Şüpheci-daraltması:** All facts confirmed: CLAUDE.md:3/:17 say 'docs/specs/', CLAUDE.md:15 cites 'docs/specs/signaling-seam-v0.md'; ls docs/specs → No such file or directory; the file is at specs/signaling-seam-v0.md whose own header (line 3) says 'docs/signaling-seam-v0.

### transport-maint.14 ⬇ [DRIFT/MAJOR] — The active branch violates both normative branching rules at once: `transport-baseline-closure` was created 2026-08-12 and is still active on 2026-08-28 (16 days, ~143 ref updates) against the 'MAX 2 gün' rule, and its n
- **Yer:** `specs/branching-strategy-and-devops-runbook.md:78,87 + CLAUDE.md:78 vs .git/logs/refs/heads/transport-baseline-closure:1`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** runbook:78: 'Kısa ömürlü feature branch: saatler – MAX 2 gün. Uzun branch = merge cehennemi.' and runbook:87: 'feat/<alan>-<kısa-açıklama>     feat/adaptive-transport-arq...'; CLAUDE.md:78: 'Trunk-based: tek main, HER ZAMAN yeşil. Kısa branch (≤2 gün).' — .git/logs/refs/heads/transport-baseline-clos
- **Maliyet:** Weeks of unmerged transport work accumulate divergence and single-squash risk on main, and the runbook's freeze-GO status ('Freeze durumu: GO') becomes untrustworthy as precedent for ignoring other no
- **Şüpheci-daraltması:** Facts confirmed: reflog line 1 timestamp 1786538374 = 2026-08-12 creation, last entry 1787881853 = 2026-08-28, 144 lines; name matches none of runbook:87's feat/|fix/|chore/|docs/ prefixes; rules quoted correctly (runbook:78, CLAUDE.md:78). But 'de-f


## İddia/white-paper bütünlüğü — 13 bulgu (BLOCKER=1)

### claims/whitepaper.1 ✅ [GAP/BLOCKER] — The ~30-constant [CAL] inventory has ZERO completed calibrations, the spec's own promotion condition (calibrated values written into golden vectors) is unmet, and the only calibration campaign ever run is structurally in
- **Yer:** `specs/adaptive-transport-engine-spec.md:974 (also :10, :969, :1153); crates/adaptive-transport/src/budget.rs:26; crates/adaptive-transport/src/emergency.rs:89; `  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** Spec :974: "Sayılar PLACEHOLDER — netem lowband baseline'ından" and :969: "Tunable'lar [CAL] → baseline ölçümünden sonra vektörlere GERÇEK değerle işlenir (placeholder değil)"; code budget.rs:26: "/// **KALİBRE DEĞİL** fixture." and emergency.rs:89 same; EVIDENCE-LEDGER:63 Q16: "Is there a standing 
- **Maliyet:** The engine can never legally promote to v1.0 (:1153 requires [CAL]→golden) and every §4.4 'measured, not assumed' claim rests on constants the spec itself labels invented placeholders — a repeat of th

### claims/whitepaper.2 ✅ [DRIFT/MAJOR] — The EMERGENCY hysteresis mechanism — the product's flagship worst-case corner (AC-8) — carries three open clock-class breaks acknowledged in the spec's own inventory (event-count escalation, 1s wall-clock dwell, 20ms wal
- **Yer:** `specs/adaptive-transport-engine-spec.md:1075-1077 vs crates/adaptive-transport/src/emergency.rs:93-95,122 and crates/adaptive-transport/src/sender.rs:925; specs`  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** Spec :1075: "emergency escalate adımı | emergency.rs update() | event-count (çağrı başına) | 1ms pompada Arq→Dup3 2 ms'de (merdiven fiilen binary)"; :1076: "deescalate_dwell=1s ... 1s < 1 srtt → tek ACK'te koruma tam iner (flap)"; :1077: "dup_spacing=20ms ... kopyalar wire'da bitişik (dekorelasyon a
- **Maliyet:** In exactly the 0.025–0.2 kbps / 50–95% loss regime the whole product is sold on, AC-8's normative asymmetric hysteresis does not exist in practice; a §4.4 P_worst run will either flap-waste scarce ban

### claims/whitepaper.3 ✅ [UNTESTABLE/MAJOR] — GO-guardrail G1 is not executable as written: the 'baseline bar' it must hold is defined circularly (calibrated from the first baseline run of the system under test, with no numeric criterion for pass), and the loss/cong
- **Yer:** `specs/transport-layer-tdd.md:478 (§4.4.10 G1), :238 (§4.4.0), :330-338 (§4.4.4)`  · küme: transport-tdd+engine-spec vs code
- **Kanıt:** TDD :478: "[G1] MEKANİZMA: estimator accuracy + loss/congestion confusion baseline barını tutar"; the only bar definition is :238: "ikincil metrik barları ilk baseline run'dan KALİBRE edilir ve rapora ÖLÇÜLMÜŞ değerle yazılır (placeholder yasak)" — no threshold, tolerance, or aggregation is specifie
- **Maliyet:** The §4.4 GO decision — the gate the entire benchmark program feeds — cannot be adjudicated or defended to DCP/ASELSAN/TÜBİTAK reviewers, forcing another methodology-corrigendum cycle at exactly the mo

### claims/whitepaper.4 ✅ [UNTESTABLE/MAJOR] — The PRODUCTION-GO gate "iki-impl-golden-vector-byte-eşitliği" is unverifiable: no spec defines CODE_SUITE_RS_SIMD_V1's mathematics (it is Leopard-RS/GF(2^16) per the crate, while profile.rs even mislabels it GF(2^8)), th
- **Yer:** `specs/fec-core-tdd-v3.0.md:484, 195-196; crates/kama-fec-codec/tests/golden.rs:2; crates/kama-fec-codec/src/profile.rs:7; docs/design/KAPI-1-V2-BENCHMARK-TASARI`  · küme: adr+fec-core+dr45 vs reality
- **Kanıt:** specs/fec-core-tdd-v3.0.md:484: "iki-impl-golden-vector-byte-eşitliği" (same at noise-wire:725); :195-196: "iki-bağımsız-impl-aynı-byte" — but no field/generator/ESI-matrix spec exists. crates/kama-fec-codec/tests/golden.rs:2: "İkinci-bağımsız-referans = pinned-byte-dizisi"; crates/kama-fec-codec/sr
- **Maliyet:** The whitepaper-facing verification gate can never be passed or honestly claimed, and the planned KAPI-1 cross-validation step will be discovered impossible mid-benchmark, forcing a redesign of the val

### claims/whitepaper.5 ✅ [CONTRADICTION/MAJOR] — The two roadmaps flatly contradict each other on the headline claim status — the 08-18 doc (REVİZYON 2026-08-24, the section P3/P4 cite as their authority) says the proof phase CLOSED with p2 KAMA-SUPERIOR +38pp, while t
- **Yer:** `KAMA-URUN-BITIRME-PLANI-2026-08-26.md:47-50 vs BITIRME-YOL-HARITASI-2026-08-18.md:136`  · küme: my-approved-docs A (roadmaps+design)
- **Kanıt:** BITIRME-YOL-HARITASI:136: '**Kanıt fazı (r5 FAZ-1)** | ✅ **KAPANDI** — p2 KAMA-SUPERIOR +38pp (Holm), rapor `d677ab6b`, white-paper v0.1' — vs PLAN:47-49: '**değer-önerisi KANITLANMADI** (tek savunulabilir hücre p2 %80-IID: Kama 0.505 vs quinn 0.040 = asimetrik-üstünlük ama %50<%90 kategorik-bar, üs
- **Maliyet:** The whitepaper author (docs/whitepaper/KAMA-TRANSPORT-WHITEPAPER-v0.2.md exists) must pick one of two approved-but-opposite verdicts about the project's core claim — the exact kama-contract contradict

### claims/whitepaper.6 ✅ [GAP/MAJOR] — S0's declared exit — the frozen NOT-WORSE/STILL-WORSE verdict under the pre-registered CI decision rule — was never produced: the FROZEN pre-registration commits to 30 cell-reps across h3a/h3b/h3c (n=10 each), but driver
- **Yer:** `docs/roadmap/KAMA-URUN-BITIRME-PLANI-2026-08-26.md:77-80 (S0) + reports/burst-reproof-v1/`  · küme: my-approved-docs A (roadmaps+design)
- **Kanıt:** PLAN:79-80: '**Çıkış:** NOT-WORSE / STILL-WORSE hükmü. STILL-WORSE beklenir → S1'i zorunlu kılar. **Boyut:** bitmek üzere.' + reports/burst-reproof-v1/PRE-REGISTRATION.md:61-62: 'CI alt-sınır ≥ 0 | **NOT-WORSE** … CI üst-sınır < 0 | **STILL-WORSE**' — vs driver.log containing only 'exploratory_h3a_b
- **Maliyet:** A frozen pre-registration with no recorded disposition leaves the 'Kama not-worse in jammer-burst' claim (the heart of the jammer/EW pitch per the pre-reg itself) formally unresolved and the partial e

### claims/whitepaper.7 ✅ [GAP/MAJOR] — The whitepaper promises the burst-fix's empirical validation 'ayrı ekte', but the frozen V1 preregistration (single run, ~120 rep-kol, FINAL-REPORT.md promised) was aborted after 15 of ~120 rep-kol (driver.log ends mid-h
- **Yer:** `docs/whitepaper/KAMA-TRANSPORT-WHITEPAPER-v0.2.md:36-37 (vs reports/burst-reproof-v1/PRE-REGISTRATION.md:74-75,83 and driver.log)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** Whitepaper:36-37 '*[Bu belge yazıldıktan sonra teşhis edildi ve düzeltme başladı; düzeltmenin ampirik doğrulaması ayrı ekte raporlanacaktır.]*' · PRE-REGISTRATION.md:74-75 '**Kısıtlar:** post-hoc tuning YOK. Tek koşu. Tek analiz. Yeni kampanya/eksen YOK. Bu belge sonuç görülmeden donduruldu' · :83 '
- **Maliyet:** Whitepaper cannot exit TASLAK with an unfulfilled appendix promise, and an abandoned frozen preregistration with no supersession note is a preregistration-integrity violation an external reviewer will

### claims/whitepaper.8 ✅ [DRIFT/MAJOR] — S1-GATE says the 'FEC-ON Kama vs quinn burst-ahead' thesis is 'zaten 868-0 matched-budget + fusion-cell ile bankada', but the cited 868-0 result is the in-process F0-vs-A1-EARLY (coding vs blind duplication) screening wh
- **Yer:** `reports/s1-gate/S1-GATE-RESULT.md:21 (vs reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md:19,40)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** S1-GATE-RESULT.md:21 'tez zaten 868-0 matched-budget + fusion-cell ile bankada' · DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md:19 '⇒ the FS-1 matched-budget coding>duplication result is **independently confirmed in the disjoint DRARQTAIL1 seed domain** (NOT generalized to burst — R2B)' · :40 'NOT generalized t
- **Maliyet:** A cross-system burst-superiority claim gets banked on evidence its own source disclaims for burst and that never touched quinn — a claim-vs-evidence gap that collapses under the first adversarial read

### claims/whitepaper.9 ✅ [CONTRADICTION/MAJOR] — The hüküm explicitly earmarked 'white-paper ana bulgusu' asserts Kama completed '≥%88' in EVERY run in the B-cells, but the report's own raw data shows l60d0 kama-fec rep2 = 30/40 = 75% (the '~%88' is the cell mean, miss
- **Yer:** `reports/s2-region/REPORT.md:32-33 (vs reports/s2-region/v1/rows.txt:12 and REPORT.md:27)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** REPORT.md:32-33 'Kama aynı hücrelerde HER koşuda ≥%88 ve tutarlı.' · REPORT.md:27 '| %60, 0ms | 40,30 (~%88) | 9,40,24,38,40 | 0/5 (derin-değişken) |' · v1/rows.txt:12 'l60d0 kama-fec 2 30 40'
- **Maliyet:** The sentence flagged as the whitepaper's main finding is falsifiable from the published rows next to it; if lifted verbatim into the whitepaper it hands a reviewer a one-line refutation of the headlin

### claims/whitepaper.10 ⬇ [DRIFT/MAJOR] — The main-body A and C verdicts are superseded by the same file's EK (K1: delivered-metric %88.4/88.75 replaces 'acked %82'; K2: post-DR-EST-1 rt60d300 = 0/6 tam-sıfır, avg %82 replaces '1/3 tam-sıfır, 25.4/40') but carry
- **Yer:** `reports/s2-region/REPORT.md:17 and :45-49 (superseded by :62-65 K1 and :76-83 K2)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** REPORT.md:17 '%90-barı bu harness'ta AÇIK kaldı (%82;' vs :65 'White-paper dili: "%80 çift-yön kayıpta uçtan-uca teslim ~%88-89; quinn 0"' · :79 '| koşular | 33, **0**, 38, 26, **0**, 37 | 40, 40, 36, 37, 6, 37 |' vs rt-fix/rows.txt (only the 6 'sonra' rows: 'rt60d300 kama-fec 1 40 40' ...)
- **Maliyet:** Anyone citing §A/§C (the sections a whitepaper writer will quote) publishes stale pre-fix numbers, and the improved K2 numbers rest on a before-dataset with no raw file and a fix not yet landed on mai
- **Şüpheci-daraltması:** Two of three sub-parts dissolve or weaken on inspection. (i) Supersession: the EK lives in the same 93-line file under the header 'EK — 3 AÇIK-KALEM KAPANIŞI + DR-EST-1', K1/K2 are each titled '(kapandı: ...)', and the main-body A hüküm at :17-20 its

### claims/whitepaper.11 ✅ [DRIFT/MAJOR] — The preregistered secondary cost endpoint is 'delivered_payload/injected_ip_bytes (governor düzlemi)' (KAPI-1 §5.2: 'total_injected_ip_bytes'; r5 H: bytes injected 'by the endpoints'), but the computed and headlined metr
- **Yer:** `docs/whitepaper/KAMA-TRANSPORT-WHITEPAPER-v0.2.md:126-128 (vs docs/design/KAPI-2-ON-KAYIT.md:36-37 and scripts/netem/kapi_analyze.py:55-61)`  · küme: my-approved-docs B (whitepaper+kapi+reports)
- **Kanıt:** Whitepaper:126-128 'byte-verimi = `teslim-payload / ileri-governor-enjekte-IP-byte` (client-egress governor sayacı; ters-yön governor sayacı bu orana girmez — kesin tanım, r5 H toplam-enjeksiyon ile yeniden-hesap FAZ-2 rafine kalemi)' · KAPI-2:36-37 'Maliyet iddiası: sıfır-güvenli `delivered_payload
- **Maliyet:** The whitepaper's second-most prominent number (+0.323) is computed under a definition narrower than the frozen one; the FAZ-2 recompute could shrink or re-sign it, forcing a public correction.

### claims/whitepaper.12 ✅ [DRIFT/MAJOR] — Both CLAUDE.md §7 and runbook C.2 assert a self-hosted nightly netem benchmark workflow (`bench-nightly.yml`, ADR-18 'ZORUNLU KALİTE KAPISI') as an existing CI gate, but no such workflow file exists — `find .github -type
- **Yer:** `CLAUDE.md:93 + specs/branching-strategy-and-devops-runbook.md:205 vs .github/workflows/ (only ci.yml)`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** CLAUDE.md:93: 'Self-hosted (nightly):  netem benchmark (7 profil vs quinn+BBR) — §4.4 gate, regresyon = fail'; runbook:205: '### C.2 Nightly transport benchmark (SELF-HOSTED = senin makinen) — `.github/workflows/bench-nightly.yml`' — actual: `find /home/ryuzaki/Desktop/kama-core/.github -type f` → o
- **Maliyet:** Transport performance regressions vs quinn+BBR land silently between manual campaigns, and any §4.4/whitepaper claim of a 'nightly regression gate' is unverifiable — the exact ölçülmemiş-iddia class C

### claims/whitepaper.13 ✅ [CONTRADICTION/MAJOR] — The roadmap's product-READY exit criterion declares 'PARSER-02/03 kapalı' with P3 marked complete (including cargo-fuzz on ccr-codec), while CLAUDE.md §10 still lists both as open deferred items — and the code sides with
- **Yer:** `CLAUDE.md:120-121 vs docs/roadmap/BITIRME-YOL-HARITASI-2026-08-18.md:153,173`  · küme: cross-cutting (CLAUDE.md + seams + CI)
- **Kanıt:** roadmap:153: 'SR-AUDIT-001 bounded-alloc; PARSER-02/03 kapalı.' and roadmap:173: '| **P3** | Sertleştirme | PARSER-02/03, cargo-fuzz (ccr-codec, ack, fec-codec framing)...' — vs CLAUDE.md:120-121: 'PARSER-02  alloc-before-check OOM → bounds-check before alloc, take(MAX 4096). / PARSER-03  time overf
- **Maliyet:** A security-hardening exit criterion is claimed closed on the strength of fixes in a different crate; the ccr-codec parser will be written later without the PARSER-02/03 constraints, or an external cla


## REFUTED (dürüstlük-kaydı — şüpheci çürüttü, 2)

- ❌ legacy.rs — The sealed legacy SESSION_DATA path has no normative 12-byte AEAD nonce construction anywhere: ccr §35 (the byte-level authority) defines on  · gerekçe: The spec does cover the legacy path; the code comment is a citation, not an invention. (1) noise-wire §4 [NORMATİF — KİLİTLİ] explicitly includes legacy-seq16 in its per-(session, 
- ❌ KAPI-1-V2-BENCHMARK-TASARIM.md — The measured KAMA-ARQ arm is defined by reference to mutable defaults ('Sender::new() defaults; ... startup-pacing ON') and the whitepaper l  · gerekçe: The cited evidence is the documented-stale record, not the code's behavior. StartupPacingConfig::default() returns 'Self { enabled: true }' (sender.rs:709); the '// default OFF' co

## Doğrulanmamış-MINOR ek-listesi (24; şüpheci-taramasından geçmedi)

- [CONTRADICTION] kama-noise-wire-tdd-v2.0.md closure table @ /home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:709-710 vs 707 — The document's own normative gate list is internally inconsistent: the paragraph directly under the closure table still says the nonce96 gat
- [UNTESTABLE] kama-noise-wire-tdd-v2.0.md K-WIRE-TRIAL-1 @ /home/ryuzaki/Desktop/kama-core/specs/kama-noise-wire-tdd-v2.0.md:621 — The K-WIRE-TRIAL-1 production acceptance criterion demands a 'fixed small' key-trial count and 'uniform' trial+decode timing without ever de
- [CONTRADICTION] adaptive-transport-engine-spec.md @ specs/adaptive-transport-engine-spec.md:310-313 and :982 vs :1140-1144 (Ek-B); c — The engine spec contradicts itself on whether the 10000 B/s min_rate constant is cancelled in ALL states or only MUST-NOT-APPLY in PRE_CONVE
- [DRIFT] adaptive-transport-engine-spec.md @ specs/adaptive-transport-engine-spec.md:471,483-485 (§5.1) vs crates/adaptive-tr — §5.1 declares the ack_confidence computation LOCKED, yet code deviates twice: the silence anchor uses .or() instead of the locked max(last_a
- [CONTRADICTION] transport-layer-tdd.md @ specs/transport-layer-tdd.md:675-676 (§10) vs specs/adaptive-transport-engine-sp — The frozen TDD's cold-start section (PROFILE_PROBE: reduce probe rate/MTU on no-ACK, retry budget, repeated no-ACK → PATH_DEAD) contradicts 
- [GAP] adaptive-transport-engine-spec.md @ specs/adaptive-transport-engine-spec.md:1083,1065 vs crates/adaptive-transport/s — MIN_CWND is a byte-absolute 4800B (3–48× BDP at sub-kbps) so the normative §8.2 inflight-cap arm is structurally never selected in the targe
- [DRIFT] ccr-format-spec.md @ /home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:1745 (also 1762, 1546,  — The §35 renumbering left dangling cross-references in normative text: §29 cites 'Section 35.7' for downgrade rules (35.7 is now hub_handle s
- [DRIFT] ccr-format-spec.md @ /home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:1788 — §30.1 points implementers to the active Session Model V3 test vectors as 'Section 30.4b (SESSION_INIT/DATA, magic SAI1/SAD1)' — SAI1/SAD1 ar
- [DRIFT] ccr-format-spec.md @ /home/ryuzaki/Desktop/kama-core/specs/ccr-format-spec.md:2067 (also 296, 1759) — usage_context exists only in the removed SignedNoisePrekey (§18 DEPRECATED/MUST NOT IMPLEMENT), yet it survives in the active parser contrac
- [GAP] revocation-model.md @ specs/revocation-model.md:306-309 (vs 337-349) — The frozen SRRV wire format gives no valid severity value for a CLEAR record: severity is enumerated only as soft_suspend/hard_revoke/emerge
- [CONTRADICTION] identity-and-trust-model.md @ specs/identity-and-trust-model.md:712,1341 (vs 618 and specs/pki-ca-ra-hsm-tdd.m — Identity contradicts itself and pki on Hub Identity Cert's signing roles: §7.5 and §13.2 say it signs ONLY Hub Noise Cert + ANNOUNCE, while 
- [CONTRADICTION] tech-stack-adr.md vs kama-noise-wire-tdd-v2.0.md @ specs/tech-stack-adr.md:332 vs specs/kama-noise-wire-tdd-v2.0.md:352; crates/kam — ADR dependency policy names ccr-codec the ONLY crate allowed to produce/parse canonical bytes, while the frozen noise-wire TDD names kama-tr
- [CONTRADICTION] kama-fec-codec vs fec-core-tdd-v3.0.md @ crates/kama-fec-codec/src/profile.rs:83-84 vs specs/fec-core-tdd-v3.0.md:337-339 — The E=1024 profiles (0x05/0x06) are documented in code as added for "sub-kbps/M3" on overhead-percentage grounds — the exact selection reaso
- [DRIFT] fec-core-tdd-v3.0.md + kama-noise-wire-tdd-v2.0.md @ specs/fec-core-tdd-v3.0.md:416, 443, 466 vs specs/kama-noise-wire-tdd-v2.0.md:17 — Both sealed docs claim a "16-gate" closure table kept "byte-for-byte identical" in the two documents; the table actually has 21 gate rows an
- [DRIFT] tech-stack-adr.md @ specs/tech-stack-adr.md:186-202, 314-316 vs Cargo.toml:3-17 and crates/adaptive- — ADR-12's locked repo map and the ADR-06/matrix claim that adaptive-transport contains the FEC engine no longer describe the codebase: six sh
- [DRIFT] P3-DEFAULT-FLIP-DECISION.md @ docs/design/P3-DEFAULT-FLIP-DECISION.md:76-83 vs crates/adaptive-transport/src/s — P3 §3's assertion that the stale startup-pacing record 'was corrected' ('kayıt sonradan düzeltilmiş: value was right, record was stale') is 
- [CONTRADICTION] KAMA-URUN-BITIRME-PLANI-2026-08-26.md @ docs/roadmap/KAMA-URUN-BITIRME-PLANI-2026-08-26.md:39 — The plan's status table says client-core is 'PARTIAL — opak-birim runtime var, gerçek-kripto/lifecycle yok' although it postdates (commit fe
- [DRIFT] P1-RUNTIME-SHELL.md @ docs/design/P1-RUNTIME-SHELL.md:44-45 vs reports/p1-runtime-shell/P1-CLOSURE.md: — P1's design gate 2 specifies 'iki-netns %5 kayıp + 40 ms RTT → N=100 birim %100 teslim', but the closure that line 47 declares TAMAM against
- [AMBIGUITY] P4-TRANSPORT-API-FREEZE.md @ docs/design/P4-TRANSPORT-API-FREEZE.md:159-161 vs :145 — P4 still states the versioning rule 'v0 → v1 tetikleyicisi: §4.4 substrat kararı VEYA güvenlik-düzlemi (Sprint-6) gerçek seam'i takınca' two
- [CONTRADICTION] KAPI-1-V2-BENCHMARK-TASARIM.md @ docs/design/KAPI-1-V2-BENCHMARK-TASARIM.md:238 (vs docs/design/KAPI-2-ON-KAYIT.m — The two frozen campaign-authority docs define the primary endpoint in different units — KAPI-1 as delivered unique-payload-BYTE ratio, KAPI-
- [AMBIGUITY] KAPI-2-ON-KAYIT.md @ docs/design/KAPI-2-ON-KAYIT.md:37 (vs scripts/netem/kapi_eval_stats.py:66 and wh — The preregistration places the cost endpoint 'aynı Holm ailesinde', but the executed analysis ran Holm over the 3 completion tests only and 
- [UNTESTABLE] KAMA-TRANSPORT-WHITEPAPER-v0.2.md @ docs/whitepaper/KAMA-TRANSPORT-WHITEPAPER-v0.2.md:185-186 (vs reports/kapi-eval- — Both whitepaper and final report state the preregistered §5 realized-loss validity scan covered '756 yeterli-örneklemli koşu' of 960, but ne
- [DRIFT] KAMA-TRANSPORT-WHITEPAPER-v0.2.md @ docs/whitepaper/KAMA-TRANSPORT-WHITEPAPER-v0.2.md:4 (vs reports/kapi-eval-v1/FIN — The whitepaper's evidence-base header pins report hash d677ab6b, which the final report's own ERRATA identifies as the initial revision that
- [DRIFT] branching-strategy-and-devops-runbook.md @ specs/branching-strategy-and-devops-runbook.md:351 vs rust-toolchain.toml:2 — The runbook's executable E.1 bootstrap pins the toolchain at 1.83.0 while the repo actually pins 1.96.0 (which ci.yml's miri job comment dep