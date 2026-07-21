# D1-EXIT — GATE-TO-EVIDENCE MATRİSİ (10-kutu; her-gate KANIT-LİNKLİ, iddia-değil)

> Abkarada-revize-kararı gereği: her-gate için mekanizma-çalışıyor-İDDİASI-değil, **kanıt/artefakt-LİNKİ**.
> Durum-özeti: **10/10 kutu KANITLI-kapalı** — ikisi kapsam-şerhli (aşağıda açık): K1-netem-nightly-açık-madde;
> adaptive-FEC-actuation D1-şartı-DEĞİL → known-limitation/deferred-integration (bkz. §B).

## A. 10-KUTU MATRİS

| # | Gate | Durum | KANIT (dosya/test — link) |
|---|---|---|---|
| 1 | seal==sealed_len tüm-class | ✅ | [sprint1_core.rs](../../crates/kama-noise-wire/tests/sprint1_core.rs) `sealed_len_exact_all_e` (FecShard tüm-E) · [sprint2_control.rs](../../crates/kama-noise-wire/tests/sprint2_control.rs) (control-tipleri, "seal.len==sealed_len") · [m3_sealed_bytes.rs](../../crates/kama-noise-wire/tests/m3_sealed_bytes.rs) `m3_pmtu_all_classes` (7-class byte-eş, normal-CI) · legacy: [sprint5b_legacy.rs](../../crates/kama-noise-wire/tests/sprint5b_legacy.rs) |
| 2 | sealed-type compile-enforce | ✅ | trybuild compile-fail: [tests/ui/forge_opened.rs](../../crates/kama-noise-wire/tests/ui/forge_opened.rs) (OpenedTransportShard dış-üretim E0451) · mühürlü-witness: [opened.rs](../../crates/kama-noise-wire/src/opened.rs) `PrivateAdmissionProof` · production-type-gate: [admission.rs](../../crates/kama-engine/src/admission.rs) `admit_and_decode(Vec<OpenedTransportShard>)` + [admission_5d2.rs](../../crates/kama-engine/tests/admission_5d2.rs) forge-testleri |
| 3 | PMTU-tablosu GERÇEK-seal-byte | ✅ | [M3-M4-M5.md](M3-M4-M5.md) §M3-tablo (E64=101B/E1024=1061B/Control 46-99B; her-satır seal-byte-eş-assert'li) · gate-testi `m3_pmtu_all_classes` (≤1200 tüm-class) |
| 4 | golden-tam (fec+AAD+outer+control+HANDLE_ASSIGN+param-blake2) | ✅ | fec: [golden.rs](../../crates/kama-fec-codec/tests/golden.rs) (profil-0x01-pin) · AAD+outer: [golden_wire.rs](../../crates/kama-noise-wire/tests/golden_wire.rs) (101B@E=64, LE64-re-baseline) + [aad.rs](../../crates/kama-noise-wire/src/aad.rs)-testleri + RFC8439+nonce96-vektörleri ([rfc8439_vectors.rs](../../crates/kama-noise-wire/tests/rfc8439_vectors.rs), [nonce96_vectors.rs](../../crates/kama-noise-wire/tests/nonce96_vectors.rs)) + libsodium-differential (feature-CI) · control: [sprint2_control.rs](../../crates/kama-noise-wire/tests/sprint2_control.rs) · HANDLE_ASSIGN-25B: [sprint4b_handle_assign.rs](../../crates/kama-noise-wire/tests/sprint4b_handle_assign.rs) · param-blake2: [golden_v0.rs](../../crates/kama-transport-schema/tests/golden_v0.rs) `golden_param_blake2s_hash` + [param_bind.rs](../../crates/kama-noise-wire/src/param_bind.rs) `matches_schema_golden_vector` (backend-özdeşlik) |
| 5 | PN-property + epoch-transition | ✅ | [pn_proptest.rs](../../crates/kama-noise-wire/tests/pn_proptest.rs) (7-property) · [sprint4c_epoch.rs](../../crates/kama-noise-wire/tests/sprint4c_epoch.rs) (dh_epoch 5-adım-B3 + grace + epoch1-PN=0) · nonce-reuse-guard + PN-cap: [sprint1_core.rs](../../crates/kama-noise-wire/tests/sprint1_core.rs) `nonce_reuse_guard` + RETRANSMIT-invariant: [pipeline.rs](../../crates/kama-integration/tests/pipeline.rs) `retransmit_new_pn_new_nonce_new_ct_reuse_blocked` |
| 6 | INIT-flight cached-byte-for-byte + anti-amplification | ✅ | [sprint4d_init_flight.rs](../../crates/kama-noise-wire/tests/sprint4d_init_flight.rs) `retransmit_is_byte_for_byte_cached` + anti-amplification/bounded/dup-map/cert-mismatch testleri |
| 7 | fec.rs-migrasyon + cleartext-silindi + decoder-reuse-ölçüm | ✅ | [fec.rs](../../crates/adaptive-transport/src/fec.rs):1-7 "DEPRECATED / MUST NOT IMPLEMENT" (0x20-cleartext-yol kapalı; METADATA-LEAK-01 kaydı; production=EXTENDED) · decoder-reuse-ölçüm: [M3-M4-M5.md](M3-M4-M5.md) §M4 (fresh≈reuse ~1.0×, üçüncü-doğrulama) + [engine_reuse.rs](../../crates/kama-fec-codec/tests/engine_reuse.rs) (byte-eş) |
| 8 | N8-invariant yeniden-doğrulandı (v2, K2-kapalı) | ✅ | [N8-reverify-emisyon-acik.md](N8-reverify-emisyon-acik.md) (emisyon-AÇIK INV-RATE-1 bit-özdeş + recovered≠ACK) · [N8-v2-recovered-wiring.md](N8-v2-recovered-wiring.md) (K2-tam-kapanış; canlı-pipeline btlbw-bit-özdeş + defter-bağlı) · N8-spot-ailesi-testleri: `dr_s5_3_mapping…` / `dr_s5_4_n8…` / `dr_s5_6_n8…` ([sender.rs](../../crates/adaptive-transport/src/sender.rs) test-modülü) + [n8_v2.rs](../../crates/kama-engine/tests/n8_v2.rs) |
| 9 | cargo-audit + deny CI | ✅ | [ci.yml](../../.github/workflows/ci.yml):27-29 (`cargo audit` + `cargo deny check`) + :31-41 miri (crypto, ccr-codec) + libsodium-oracle ayrı-job (sodiumoxide=0.2.7-pin) |
| 10 | non-regression-pin + M1-M5-rapor | ✅ | pin-metodolojisi {A+B+C}: [baseline-pre-5c.md](baseline-pre-5c.md) (§D-kilitli) — güncel-A: nw95/fec25/sch9/integ12/at130/engine17 + B-desen1-5-pin + C-FEC-off · raporlar: [M1](M1-reverse-nis.md) (yeniden-adlandırılmış) · M2→§B-not · [M3-M4-M5](M3-M4-M5.md) · süreç: [5c-kapanis](5c-kapanis.md)/[5d-kapanis](5d-kapanis.md)/[fuzzy-envanter](fuzzy-controller-envanter.md) |

## B. KAPSAM-ŞERHLERİ + KNOWN-LIMITATIONS (release-note-görünürlüğü; gizleme-yok)

1. **adaptive-FEC-actuation D1-ŞARTI-DEĞİL → DEFERRED-INTEGRATION (açık-kayıt):** `fec_parity_ratio_permille`
   fuzzy-çıktısı **DEAD-OUTPUT** (üretim-tüketicisi-yok; §10.2 r=round(ratio×k) kablosuz; parity-emisyonu
   token-artığı-mekanik; `fec_enabled` manuel) — tam-envanter: [fuzzy-controller-envanter.md](fuzzy-controller-envanter.md).
   D1-teslimi = **mekanizma-katmanı** (seal/PN/FEC-codec/admission/RECOVERED-wiring: kanıtlı) + **karar-motoru**
   (fuzzy-matematik golden'lı; eşikler [CAL]-fixture). **Karar→FEC-icra kablosu D1-kapsamı-DIŞI** — paralel-hat
   planı: 1e-injection → 1a-ratio→r (feature-flag; revize-karar-sırası).
2. **M1 yeniden-adlandırıldı:** "fixed-FEC residual-budget characterization" — fuzzy-devre-dışı-iki-uç-modu-ölçümü;
   "niş-çürüdü" değil "sabit-FEC-bu-rejimde-verimsiz (beklenen)" ([M1-raporu üst-notu](M1-reverse-nis.md)).
3. **M2 (deadline-bounded completion):** ayrı-koşu-olarak-DEĞİL, karar-yüzeyi-taramasının deadline-EKSENİ olarak
   plana-taşındı (çerçeve-madde-6: M2/M5 aynı-yüzeyin-eksenleri; fuzzy'nin deadline-GİRDİSİ-YOK — envanter). M5-
   zaman-yüzdelikleri (t_p50/p95 her-trace) deadline-analizinin ham-verisini şimdiden-içeriyor.
4. **K1 (spurious-gap-vergisi):** sim'de-sıfır (idealize-SACK) — **netem-nightly-maddesi AÇIK** ([M3](M3-M4-M5.md)).
5. **PMTU-shrink-ekseni:** transport-PMTU-mekanizması-yok (DEFAULT_MTU-sabit) → taranamadı; deferred.
6. **RsSimdEngine:** TUT-önerisi (zayıf-tercih; hız-nötr-ölçümlü) — Abkarada-onayı-bekliyor ([M4](M3-M4-M5.md)).
7. Yasak-dil-hatırlatması: bu-paket "production/hostile-ready/anti-jamming" İDDİA-ETMEZ; tüm-hükümler
   sim/yerel-ölçüm-sınırlı, netem/saha-doğrulaması nightly/sonraki-faz.

## C. D1-EXIT-HÜKMÜ

Mekanizma-katmanı 10/10-kanıtlı; bilinen-sınırlar §B'de-açık. **D1-EXIT-paketi Abkarada-onayına-hazır.**
Onay-sonrası: paralel-hatlar (TARAMA + MİNİMAL-AKTÜASYON 1e→1a) + Sprint-6-paralel (revize-karar-sırası;
minimal-aktüasyon-borcu Sprint-6-sonrasına-tümden-bırakılmaz).
