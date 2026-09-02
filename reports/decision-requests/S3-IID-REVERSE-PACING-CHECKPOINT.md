# S3-IID-REVERSE-PACING-CHECKPOINT (FINAL BUILD / S3)

> **STATÜ: S3 TAMAMLANDI + GATE-YEŞİL.** ALT-1 amendment (12-hücre/360) + typed-impairment + NaturalIid
> plumbing (deficit-free) + reverse-pairing + measured 0.80C pacing. DIAGNOSTIC (FS-1 0/360). v1.2 immutable
> (v1.3-freeze S3'te YOK). sender.rs/loss.rs ZERO-DIFF. structural-null korundu. S4 (manifest+360/360) — review-sonrası.

## 1-6. Provenance + amendment
```text
1. branch/HEAD : d1-sprint5c / (bu commit) · staged: foundation d4c08c7 · S0 0ba3612 · S1 5d84f5a · S2 e511ef4 ·
     S3-A 6e8f78f · S3-R0 22052e9 · S3-R1 b6d6adb · S3-R2 27e06dc · S3-R3 (bu commit)
2. FS-PREREG-v1.2 : da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034 (immutable; DEĞİŞMEDİ)
3. FS-1 result-bearing : 0/360 (0/540 historical). Hiçbir sonuç-verisi ÜRETİLMEDİ; tüm S3 DIAGNOSTIC (manifest-DIŞI).
4. S3-A audit : FS-IID-CELL-SEMANTICS-AUDIT.md (6e8f78f) → DUR-B (IID-COVERAGE-AXIS AMBIGUITY).
5. ALT-1 human-decision : Abkarada (S3 RESUMPTION §0, BINDING) — "COLLAPSE IID20 DEFICIT AXIS"; "540 tarihsel-süreklilik
     için KORUNMAZ".
6. amendment-decision : FS-1-IID-COVERAGE-AMENDMENT-DECISION.md · sha256 e30d2fa841a897a84c2e1bd4a2b849df75970d1851b5ce76bb43d0374b9b9e61
```

## 7-10. Matris + 540→360
```text
7.  OLD 18-cell : RTT{50,250,1000} × deficit{1,2,4} × impairment{ExactDeficit,IID20} = 18 (9 ED + 9 IID).
8.  AMENDED 12-cell : 9 ExactDeficit (RTT×d) + 3 NaturalIid (RTT×p=0.20; d-ekseni ÇÖKTÜ) = 12.
9.  540→360 : OLD 18×6×5=540 · NEW 12×6×5=360.
10. 180 duplicate-removal : IID20-9-hücrenin 6'sı (RTT-başına 2) execution-semantic-duplicate → 6×6×5=180 avoided.
     Pin historical_iid_duplicate_detection: tarihsel-18 → benzersiz-12, duplicate-6; (18−12)×6×5==180.
```

## 11-17. Typed impairment + semantic hashes + candidate artifact (S3-R0)
```text
11. SUM-TYPE : FsImpairment{ExactDeficit{drop_count} | NaturalIid{loss_permille}} (fs_cell.rs). RULE-9.
    ForwardImpairment (fs_spine.rs) execution-layer AYNI sum-type.
12. invalid-combo proof : parse_impairment REDDEDER — NaturalIid+deficit(IidWithDeficit) · ExactDeficit-count'suz
    (ExactDeficitWithoutCount) · variant-yabancı-param(ExactDeficitWithIid) · InvalidDropCount(∉{1,2,4}) ·
    InvalidLossPermille(≠200). Pin impairment_sum_type_validation. NaturalIid{deficit} TİPTE temsil-EDİLEMEZ.
13. BASE_CELL_SEMANTICS_HASH : RTT+workload-geometri+impairment-variant+variant-param+util/source-offer+(cell-def)reverse;
    arm/rep/seed/row-id/label HARİÇ. IID'de deficit GİRMEZ → RTT-başına IID d-satırları özdeş-hash.
14. RUN_EXECUTION_SEMANTICS_HASH : base-cell + arm + configured_r + seed + config-version; filename/label/outcome HARİÇ.
15. ROW_IDENTITY_HASH : exec-hash + row-label + display-deficit-label. RULE-8: aynı exec-hash + farklı row-id = duplicate-run.
16. HISTORICAL_IID_DUPLICATE_DETECTION : pin (yukarıda §10). IID RTT-başına 1 benzersiz base-hash (d inert);
    ExactDeficit RTT-başına 3 (d grounded).
17. candidate artifact : FS-1-CELL-MATRIX-v1.3-CANDIDATE.json (NON-FROZEN) · sha256
    399bfcd4b5c75a25c1c036ed56719e0afe725a15f0df2c3d0d53116b67313b11 · 12-cell/12-uniq/0-dup/0-invalid/360-projected.
    Pin candidate_matrix_artifact: committed == candidate_matrix_json() (drift-YOK; read-only include_str!).
```

## 18-19. ExactDeficit regression + NaturalIid authority
```text
18. EXACT_DEFICIT_VARIANT_REGRESSION : typed-refactor run_spine'ı DEĞİŞTİRMEDİ. d∈{1,2,4} = tam-d global
    InitialSource-drop (per-block çarpım-YOK); 6-arm+F4-K2/4/8 global-deficit-arm-invariance + faithful. Pins
    exact_deficit_variant_regression · global_deficit_arm_invariance_and_no_multiplication_s3.
19. NaturalIid authority : mevcut fs_trace.rs TraceSpec::NaturalIid{loss_permille,seed} + decide()+keyed()+
    hash_forward_spec DEĞİŞTİRİLMEDEN spine'e plumb-edildi (ForwardImpairment::NaturalIid → forward_spec match).
    loss_permille=200 (p=0.20 SABİT); redesign YOK.
```

## 20-25. IID mekanizma (S3-R2)
```text
20. ROLE-ADMISSIBILITY tablosu (pin iid_role_admissibility):
      role            | loss-eligible | identity-source          | attempt | domain     | authority
      InitialSource   | EVET          | (0,gid)                  | 0       | forward    | keyed(role,src,att,seed)
      RetransmitSource| EVET          | (0,gid)                  | 1       | forward    | keyed(...)
      Duplicate       | EVET          | (0,gid)                  | 1       | forward    | keyed(...)
      FecRepair       | EVET          | (block,esi)              | 0       | forward    | keyed(...)
      Control(fwd)    | HAYIR (muaf)  | —                        | —       | forward    | decide→false
      Reverse ACK/ctrl| EVET (ayrı)   | ordinal-keyed            | —       | reverse    | decide_reverse
    garantili-teslim-imtiyazı YOK (p=1000 → repair/dup/retransmit de düşer).
21. IID semantic-key : keyed(role_disc, source_id.0, source_id.1, attempt, seed) — emit_time/arm/ordinal GİRMEZ.
22. IID_INITIAL_SOURCE_ARM_INVARIANCE (pin): 6-arm + F4-K2/4/8 aynı IID-cfg → her global InitialSource 0..47 AYNI
    drop-kararı (realized_initial_source_drop_ids arm-ÖZDEŞ).
23. IID_REPLAY_X3 (pin): aynı typed-cfg+arm ×3 → özdeş (forward/reverse/configured-hash + realized-drops +
    retransmit + fec-decoded + completion + timing). RNG-YOK.
24. no-arm-key proof (pin iid_no_arm_name_keying): A0/F0/A1 farklı-recovery/ACK → drop-set ÖZDEŞ.
25. no-wire-order proof (pin iid_no_wire_ordinal_keying): aynı-id farklı-emit_time → aynı karar; önceki-kararlar/ordinal
    kararı DEĞİŞTİRMEZ.
    (probability sanity: pin iid_probability_distribution_sanity — 10000-id @ p=200 → frac ∈ önceden-ilan [0.17,0.23];
     p=0→0; p=1000→hepsi. MEKANİZMA-diagnostiği, FS-1-KANIT-DEĞİL; hash/threshold tune-EDİLMEDİ.)
```

## 26-28. Reverse (S3-R1)
```text
26. reverse authority : ReverseSpec (Clear | Scheduled{blackout_intervals_ns, loss_permille, seed}); ground-truth =
    configured schedule (missing-ACK'ten ÇIKARIM-YOK). SpineCfg::with_reverse. result: reverse_schedule_id +
    reverse_spec_kind + reverse_blackout_intervals + reverse_event_count/dropped + reverse_event_hash.
27. forward/reverse separation (pin forward_reverse_domain_separation): yalnız-reverse-değişimi reverse_schedule_id'yi
    değiştirir forward_schedule_id'yi DEĞİL (ve tersi). configured_schedule_hash İKİSİNE de commit.
28. configured reverse pairing (pin reverse_configured_schedule_pairing): paired-arm configured_schedule_hash +
    reverse_schedule_id + blackout ÖZDEŞ; realized forward-trace/ACK arm'a göre FARKLI (pairing = configured-authority,
    realized-ACK-eşitliği DEĞİL).
```

## 29-37. Pacing + deadline (S3-R1/R3)
```text
29. pacer mimarisi : deterministik harness-level source-offer pacer (source_offer_schedule; elig[g]=g×sealed×8×1e9/pacer).
30. bottleneck/source-offer AYRIMI : bottleneck_capacity_bps(C=1M) · source_offer_pacer_bps(0.80C=800k) ·
    configured_source_offer_bps · realized_source_wire_bps AYRI alanlar. bottleneck DÜŞÜRÜLMEZ.
31. 48.48-ms türetme : 48×101=4848B × 8 / 800000 = 48_480_000 ns (fs_t_serialize_source_ns). Pin source_offer_configured_0p80c.
32. integer/rounding : tam-sayı ns aritmetiği (float-drift YOK); elig[g] dt(10µs)-katı → quantization 0.
33. frozen diagnostic tolerance : PACING_TOL_BPS=500 (önceden-ilan; observed'tan TÜRETİLMEDİ; §16 derivation: 1×dt/span×rate
    ≈169bps; max-teorik-hata 0). 
34. configured 0.80C proof (pin): configured_source_offer_bps==800000==0.80×C; source_offer_end_ns==48_480_000;
    SCHEDULE_EQUALITY (6-arm+F4-K2/4/8 aynı schedule/rate/envelope).
35. A0 no-contention realized proof (pin source_offer_realized_a0_no_contention): dt=10µs+app-limited → realized_source_wire
    = TAM 800000 bps (TOL içinde). grounding-failure → DUR-B (olmadı).
36. intervention-contention (pin intervention_contention_distinct): A1-dup realized-source-wire ≤ A0 (configured-schedule
    DEĞİŞMEZ; realized-wire ≠ configured-offer eşitliği zorlanmaz).
37. deadline-candidate eligibility (pin deadline_candidate_eligibility; v1.3-freeze YOK): source=4848B · eff=100000 B/s ·
    serialization=48.48ms · candidate RTT50→199 / 250→599 / 1000→2099 / 2500→5099 ms. ELIGIBLE (dondurulmadı).
```

## 38-41. Firewall + structural-null + schema (S3-R1/R3)
```text
38. F4_PREDECISION_FIREWALL_S3 (pin): F4-K yalnız pre-decision base-RTT'den; deficit/reverse/pacing/IID/completion K'yı
    DEĞİŞTİRMEZ.
39. F1_BULK_NULL_REGRESSION_S3 (pin): S3-integration sonrası paired F0/F1 binding-fixture → BulkParityWireNullConfirmed
    (t_first_parity_wire(F1)==(F0)); advancement → DUR-A (olmadı).
40. F3_BULK_NULL_REGRESSION_S3 (pin): aynı, F3.
41. COMMON_RESULT_SCHEMA_S3 (pin): impairment_kind + configured_exact_drop_count/configured_iid_loss_permille (typed
    None/N-A; sahte-sıfır YOK) + realized_initial/retransmission/duplicate/repair losses + reverse_event_hash +
    pairing_gate_passed/failure_reason. S1/S2 field-anlamları DEĞİŞMEDİ.
```

## 42-43. Reproducible gate (RULE-7)
```text
42. exact commands (working-dir=repo-root):
    fmt        : cargo fmt --check
    clippy     : cargo clippy -p adaptive-transport --all-features --tests -- -D warnings
                 cargo clippy -p adaptive-transport --tests -- -D warnings
                 cargo clippy -p kama-integration --features experimental-fs-harness --tests -- -D warnings
                 cargo clippy -p kama-integration --tests -- -D warnings
    flag-off   : cargo test --workspace
    AT         : cargo test -p adaptive-transport --all-features
    KI fs      : cargo test -p kama-integration --features experimental-fs-harness
43. results (bu-turda reproduce):
    fmt CLEAN · clippy 4-config CLEAN
    FLAG-OFF workspace : 382 passed / 0 failed / 14 ignored
    AT --all-features  : 241 passed / 0 failed / 12 ignored  (NOT: --features experimental-fec-budget tek = 215)
    KI fs-harness      : 79 passed / 0 failed  (S1 14 + S2 12 + S3-R0 5 + S3-R1 11 + S3-R2 6 + S3-R3 3 [matrix1+schema2]
                         + FS-HARNESS-A önceki 28 [fs_f0 7 · fs_fec_arms 3 · fs_workload_pins 6 · control_path 3 · pipeline 9])
    cold-start (experimental-cold-start; S3-değişiklikler fs-harness-gated → yapısal-etkilenmez):
      KI a2_* = 32/0 · kama-engine = 71/0 · AT cold/a2 = 29/0 (S2-checkpoint'te reproduce; bu-turda kod-dokunulmadı)
```

## 44-46. Hygiene + zero-diff
```text
44. artifact cleanup : cold-start CSV regen'leri REVERT; diagnostic-output FS-1 raw-data-path'inde YOK; FS-1 result-artifact
    YOK (0/360); yalnız kasıtlı S3 kod/doc/test/artifact staged. Manifest-run-ID KULLANILMADI.
45. sender.rs / loss.rs : ZERO-DIFF (git diff --stat BOŞ; tüm-S3 boyunca).
46. production graph/defaults : DEĞİŞMEDİ (tüm-S3 experimental-fs-harness-gated; INV-CS-5; INV-ENGINE-SOC).
```

## 47. Limitations
```text
- NaturalIid completion IID-realizasyonuna bağlı (tail-drop → A0 kurtaramaz; FEC kurtarabilir) — pairing/determinism
  completion-BAĞIMSIZ kanıtlandı (InitialSource-drop-set + event-hash).
- realized_source_wire measured yalnız A0-no-contention'da tam-0.80C; intervention'lı arm'larda contention (beklenen).
- probability-sanity MEKANİZMA-diagnostiği (FS-1-kanıt-DEĞİL); tek-48-workload'dan %20 zorlanmaz.
- BULK-ONLY (FS-2 single-block; DR-FEC-INT-1 interleaving). F4 mechanistic/exploratory.
- deadline candidate (v1.3-freeze YOK); 305ms historical-anchor (küçük-geometri; FS-1-deadline DEĞİL).
- ForwardImpairment henüz yalnız {ExactDeficit, NaturalIid} (NaturalBurst FS-2).
```

## 48. S4 handoff
```text
S4 (review-sonrası; yetkili): 360-row manifest + 360/360 executability. Girdiler HAZIR: FsImpairment sum-type +
base_cell/run_execution/row_identity hash (RULE-8/9) + amended 12-cell (candidate JSON) + 6-arm spine (ExactDeficit +
NaturalIid + reverse + 0.80C-pacing) + typed-config. Manifest = 12-cell × 6-arm × 5-seed = 360 satır; her satır
run_execution_semantics_hash-benzersiz; ROW_IDENTITY farklı ama exec-özdeş = duplicate (RULE-8). S4-SONRASI: S5
coverage-v1.2/analysis-spec + FS-PREREG-v1.3-freeze (Abkarada + re-freeze).
```

## 49. STOP
```text
S3 tamam. STOP. S4 result-manifest / 360-360 executability / coverage-v1.2-freeze / analysis-spec-freeze /
FS-PREREG-v1.3-freeze / FS-1 result-bearing-execution BU-TURDA YOK. Sonraki yetkili-stage: S4. v1.2 immutable.
FS-1 0/360. Human-review (Abkarada) bekleniyor.
```
