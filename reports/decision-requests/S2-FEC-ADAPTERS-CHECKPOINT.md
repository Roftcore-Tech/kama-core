# S2-FEC-ADAPTERS-CHECKPOINT (FINAL BUILD / S2)

> **STATÜ: S2 TAMAMLANDI + GATE-YEŞİL.** F1/F3/F4 closed-loop adapter'ları tek-common-spine üstünde. DIAGNOSTIC
> (result-bearing-DEĞİL; FS-1 0/540). v1.2 immutable. sender.rs/loss.rs ZERO-DIFF. **F1/F3 bulk parity-wire
> structural-null CONFIRMED** (advancement-YOK). S3+ (IID/reverse/0.80C/manifest/coverage/analysis-spec/v1.3) YOK.

## 1-6. Provenance + scope
```text
branch/HEAD : d1-sprint5c / (bu commit) · foundation d4c08c7 · S0 0ba3612 · S1 5d84f5a
FS-PREREG-v1.2 : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable)
FS-1 result-bearing : 0/540 (S2 DIAGNOSTIC; manifest-DIŞI; budget-harcamaz)
scope : BULK-ONLY (FS-1-BULK-SCOPE-BOUNDARY.md) · pre-data FS-BULK-EMISSION-NULL-PREDECLARATION.md (S1'de committed)
```

## 7-11. F1 (early-logical-close; K=8)
```text
ARCH: SpineArm::F1{r} → common spine; K=8, 6 blok, R/blok; real RsSimdCodec encode/decode; full-workload(2400B).
  F1 differs-from-F0: erken-eligibility (block sources ENQUEUED-anında t_parity_eligible) — AMA wire sender-wide-gated.
REAL-PATH (f1_real_decode_full_workload): tail(g47) → gerçek FEC-decode≥1 (ARQ-imkânsız) · reconstruct_byte_equal ·
  estimator_pure · reassembly 2400B. no-fake-ACK/RTT/delivery.
STRUCTURAL-NULL (f1_bulk_parity_wire_null; paired-deterministic no-loss):
  A: t_first_parity_wire(F0/F1) >= t_final_source_wire_complete (parity final-source-SONRASI). ✓
  C: t_first_parity_wire(F1) == t_first_parity_wire(F0) (deterministik-eşit; ADVANCEMENT-YOK). ✓
  VERDICT: structural_class = BULK_PARITY_WIRE_NULL_CONFIRMED. (advancement olsaydı → StructuralAnalysisFalsified → DUR-A.)
```

## 12-19. F3 (K'=4 partial-block; 12 blok)
```text
ARCH: SpineArm::F3{r} → common spine; K=4, 12 blok (48/4), R/blok; real RsSimdCodec (profile 0x02); full-workload.
  global 0..3→blok0 … 44..47→blok11 (global_to_local_k(gid,4)). Workload AZALMAZ (2400B).
REAL-PATH (f3_k4_12block_real_decode_full_workload): tail(g46,47) → selected_k=4, block_count=12, FEC-decode≥1,
  reconstruct_byte_equal, reassembly 2400B.
STRUCTURAL-NULL (f3_bulk_parity_wire_null): t_first_parity_wire(F3) == t_first_parity_wire(F0) + >= final-source-wire →
  BULK_PARITY_WIRE_NULL_CONFIRMED (S0-B teyit; advancement → DUR-A).
COST/REPAIR (f3_repair_count_and_cost_distinct_from_f0): F0 6-blok×R2=12-repair=1212B · F3 12-blok×R2=24-repair=2424B ·
  base-source AYNI(4848B). F3-intervention 2× F0 → GERÇEK-fark (block-count-effect; BUDGET_MISMATCH-DEĞİL; normalize-YOK).
DECOMPOSITION (yorum): F3 farkı block-size/block-count/repair-count/decode-locality/ARQ-overlap; timing-headstart-DEĞİL
  (structural-null). "coding-gain/emission-headstart" ancak ilgili-causal-kriter sağlanırsa.
```

## 20-26. F4 (adaptive direct-K; PathTimingState-consumer)
```text
ARCH: SpineArm::F4{r} → common spine; f4_decide_k(base_rtt)=CodecK → K∈{2,4,8}; real codec per selected-K; full-workload.
  YALNIZ F4 PathTimingState tüketir (f4_decide_k(cfg.f4_base_rtt_ns); pre-decision base-RTT).
BASE-RTT→K TABLE (S0-A; kod-grounded): base<50ms→K2(0x01,24-blok) · 50-150ms→K4(0x02,12-blok) · ≥150ms→K8(0x03,6-blok) ·
  cold(base=0)→K_COLD=K4. grid: RTT50→K4, RTT250→K8, RTT1000→K8. (mechanistic/exploratory; optimal-iddia-YOK.)
REAL-CODEC PINS: f4_k2_real_codec_24block(base30ms→K2,24-blok,2400B) · f4_k4_...(100ms→K4,12-blok) · f4_k8_...(250ms→K8,
  6-blok). Hepsi reconstruct_byte_equal + reassembly 2400B. K1/K3 ÜRETİLMEZ (f4_no_leakage: {2,4,8}-only pin).
NO-LEAKAGE (f4_no_leakage_predecision_only): f4_decide_k YALNIZ base_rtt alır; aynı-base + farklı-deficit → aynı-K
  (deficit/outcome'dan BAĞIMSIZ); deterministik (pure). future/decode/completion/oracle-input YOK.
```

## 25/27-31. Full-workload equality + six-arm equivalence
```text
MULTIBLOCK_REASSEMBLY_BYTE_EQUALITY_S2 (s2_multiblock_reassembly_byte_equality): F1/F3/F4-K2/K4/K8 recoverable → 2400B. ✓
CANONICAL_SOURCE_EQUALITY_S2 + SOURCE_SCHEDULE_EQUALITY_S2 (s2_canonical_source_and_schedule_equality): 6-arm + F4-K2/K4/K8
  AYNI source_sequence_hash + source_offer_schedule_hash + base_source_bytes(4848B) + source_symbol_count(48). ✓
GLOBAL_DEFICIT_ARM_INVARIANCE_S2 + DEFICIT_NO_MULTIPLICATION_S2 (s2_global_deficit...): 6-arm + F4-K2/K4/K8 AYNI
  dropped_global_ids([2,5]) + configured_schedule_hash(arm-invariant) + faithful(realized==d=2; per-block-çarpım-YOK). ✓
GLOBAL→LOCAL: global_to_local_k(gid,k); global_to_local_mapping_hash per-K (F0/F1 K8, F3 K4, F4 K2/4/8) — codec-layout-metadata.
```

## 32-35. Result schema + ledgers + timing
```text
COMMON_RESULT_SCHEMA_S2: SpineResult tek-aile (A0/A1/F0/F1/F3/F4); S1-field-anlamı-DEĞİŞMEDİ. +selected_k/profile_id/
  source_block_count/configured_r/repair_count/f4_base_rtt/structural_class. non-F4 → f4_base_rtt=0. F1/F3 → structural_class.
COST (§18 iki-defter): INTERVENTION (proaktif dup/repair sealed) vs TOTAL-OUTCOME (base+intervention+arq_retransmit).
  fs_sealed_len(E)=E+37=101 tek-otorite. F3/F4 repair-count-artışı GÖRÜNÜR (normalize-YOK). spurious→total (BUDGET_MISMATCH-DEĞİL).
TIMING: t_first_source_offer · t_final_source_wire_complete · t_first_parity_eligible · t_first_parity_wire (GERÇEK
  channel wire = arr−owd, OFFER-anında capture) · t_first_repair_usable (reconstruct-tamam) · t_full_workload_complete.
```

## 36-39. Gate (bu-turda reproduce edilen KESİN sayılar)
```text
fmt CLEAN · clippy -D (AT-exp · AT-flag-off · KI-fs-harness · KI-flag-off) CLEAN
FLAG-OFF workspace (cargo test --workspace) : 382 passed / 0 failed / 14 ignored (UNCHANGED; INV-CS-5)
adaptive-transport --all-features : 241 passed / 0 failed / 12 ignored (S1-consistent gate-cmd)
  (NOT: --features experimental-fec-budget tek-başına = 215/0/9; --all-features iki-exp-flag = 241 — S1 rakamı)
kama-integration fs-harness (experimental-fs-harness) : 54 passed / 0 failed (S1 42 + 12 S2-pin)
cold-start (experimental-cold-start; S2-değişiklikler fs-harness-gated → yapısal-etkilenmez, teyit):
  KI a2_* suite = 32/0 · kama-engine = 71/0 · AT cold-start/a2 = 29/0 (hepsi-yeşil; sıfır-fail)
  (NOT: S1 checkpoint "cold-start A2 44/0" tek-komutla reproduce-edilemedi [crate-yayılımı + ignore'lı
   netem-testler]; unverifiable-rakam-uydurmak-yerine reproduce-edilen-alt-suite'ler raporlandı — §9 lab-verifiable.)
sender.rs / loss.rs : bu-turda MODIFIED-DEĞİL (git diff --stat BOŞ ✓) · production-graph GENİŞLEMEDİ · defaults DEĞİŞMEDİ
DIAGNOSTIC: tüm S2-run manifest-DIŞI; FS-1-dataset-ingest-YOK; 540-budget-harcamaz.
```

## 40. Limitations
```text
- Parity-emission-modeli: TÜM-source-offered-SONRASI (base_source_bytes==48×101 gate) → sender-wide-DATA-first faithful.
  Gerçek-pacer-token-per-tick-dinamiği DEĞİL (deterministik-approx; structural-null için yeterli + faithful-yön).
- F1/F3 structural-null BULK-ONLY (single-block FS-2; DR-FEC-INT-1 interleaving). F4 mechanistic (validity FS-5B).
- 0.80C measured-source-pacing S3 (şu-an source_offer_pacer_bps config-var ama diag pacer=0; measured-freeze S3).
- F0/F1 non-tail FEC-vs-ARQ race (S0-B; parity DATA-gated) — completion FEC-or-ARQ; tail'de FEC-guaranteed.
```

## 41. S3 handoff
```text
S3 (IID + reverse-pairing + measured 0.80C source-offer pacing): (a) NaturalIid tüm-6-arm'a (global-id-keyed; §16);
  (b) reverse-pairing (configured fwd/rev identity; §17); (c) SOURCE_OFFER_PACING_0P80C — source_offer_pacer_bps=0.80C,
  observed-egress≈0.80C ölç+pin (§18); schedule tüm-arm-özdeş. Spine hooks HAZIR (source_offer_schedule + reverse
  decide_reverse + ExactDeficit/IID trace). S3-SONRASI: S4 manifest+540/540, S5 coverage-v1.2+analysis-spec+F1-note+v1.3-freeze.
```

## 42. STOP
```text
S2 checkpoint tamam. STOP. IID/reverse-campaign/0.80C-measured/manifest/executability/coverage-v1.2/analysis-spec/
prereg-v1.3/FS-1-execution BU-TURDA YOK. Sonraki yetkili-stage: S3. Human-review bekleniyor.
```
