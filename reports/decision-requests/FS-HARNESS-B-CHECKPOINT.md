# FS-HARNESS-B — COMPLETION CHECKPOINT (DR-FEC-FS-1)

> **STATÜ: FS-HARNESS-B TAMAMLANDI.** Result-bearing-YOK. **FS-1 KOŞULMADI.** Sender-recovery-fix YOK.
> FS-PREREG-v1.2 FROZEN. Bu, FS-1-öncesi SON harness-construction slice'ının bittiğine dair checkpoint'tir —
> FEC-hükmü DEĞİL. Abkarada review'ı bekliyor. STOP.

## 0. Kapsam (bağlayıcı sınırlar korundu)
```text
YAPILDI     : B1 · B2-narrow · B3 · F1 · F3 · F4 · family-pins · FS-PREREG-v1.2 freeze/hash.
YAPILMADI   : FS-1 result-bearing runs · production Sender-recovery-fix · controller-calibration · FS-5B ·
              FS-2 · F2 · default-flip · merge. (Reviewer §14 STOP-condition'ları AYNEN.)
```

## 1. HEAD / commits / provenance
```text
FS-HARNESS-B mekanizma : cd336e9 (B1/B2/B3/F1/F3/F4 + cost-split + CLI + smoke)
review-blocking-fix    : c9ee14c (A1_WIRE_ACCOUNTING; base=FS-HARNESS-A)
FS-HARNESS-A           : 4f89eee
branch                 : d1-sprint5c (main-DEĞİL)
prereg FROZEN          : FS-PREREG-v1.2.md sha256=da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
prereg SUPERSEDED      : FS-PREREG-v1.1.md sha256=60353e99…  (SUPERSEDED-BY-v1.2)
```

## 2. B1 reverse-loss implementation
`fs_trace.rs` `ReverseSpec::Scheduled{loss_permille (ordinal-keyed iid), seed}`; `decide_reverse(emit_time,ack_ordinal)`;
forward'dan AYRI `reverse_decision_log_hash`/`reverse_counts`. GROUND-TRUTH (eksik-ACK'ten çıkarım-YOK).

## 3. B2 reverse-blackout implementation
`ReverseSpec::Scheduled{blackout_intervals_ns [lo,hi)}`; içindeki ACK DÜŞER, DIŞI PASS (recovery-after otomatik).
run_arm ACK-yolu `ch.decide_reverse(arr, ack_ordinal)` → düşerse Sender feedback-almaz (ARQ-clock durur; C-P3).

## 4. Reverse determinism ×3 evidence
`reverse_loss_schedule_replays_deterministically` + `reverse_blackout_schedule_replays_deterministically`
(her-biri aynı-reverse-trace ×3 → identical reverse-hash) · `forward_and_reverse_trace_hashes_are_stable`
(forward-hash reverse-kararlardan BAĞIMSIZ; forward≠reverse şema-kimliği). fs_trace = **9 test** (6 A + 3 B). GREEN.

## 5. Forward/reverse paired trace proof (B3)
Pin `b3_paired_reverse_blackout_channel_intervention_identical`: A0/A1 AYNI (fwd+rev) şema → `configured_schedule_hash`
ÖZDEŞ (+ forward_schedule_id/reverse_schedule_id özdeş); Clear≠Scheduled reverse-id (ayrı-görünür). Binary-smoke
[6]: A0/A1 configured_schedule_hash eşit + reverse_dropped=6. **Aynı-PRNG-seed TEK-BAŞINA değil — şema-kimliği pairing.**

## 6. Tail-erasure deterministic fixture (B2)
Pin `b2_tail_erasure_is_real_not_harness_defect`: A0 drop=(k−1) → Tail · delivered=k−1 · completion=None ·
retransmit=0 · largest_acked=k−2 · admitted=true. MIDDLE (drop=3) kontrast → ARQ-kurtarır. Binary-smoke [5] teyit.

## 7. Evidence Timer/PTO faithfully serviced
run_arm loop (fs_harness.rs) `Event::TimerFired(Timer::Pto)` HER-tick KOŞULSUZ + horizon-sonuna-kadar çalışır
(delivered<k iken bile). Tail-stall'da bile `admitted=true` (faithful∧overlap) — runner tail-PTO'yu sadık-servis-eder.

## 8. Code-grounded reason tail remains unrecovered
`loss.rs:383` detect_time_losses filtresi `seq < largest_acked` (RFC9002 §6.1 later-ACK-precondition; temiz-kanal
false-loss'unu önlemek için). Son-unit düşünce largest_acked=k−2'de takılır → seq(son) asla < largest_acked →
loss-ilan-EDİLEMEZ. Timer-kümesi {Pto, RtpropProbe} — RTO/TLP YOK (sender.rs:560-565). Belgelenmiş-caveat loss.rs:277/364.
**Horizon-uzatma ÇÖZMEZ** (yapısal-önkoşul, timeout-değil) — b2 pini 3×-horizon ile teyit.

## 9. Explicit confirmation NO Sender tail fix introduced
adaptive-transport/src/sender.rs + loss.rs bu slice'ta DEĞİŞTİRİLMEDİ (git diff cd336e9: yalnız fs_trace.rs +
fs_harness.rs + kama-integration-testleri + CLI + smoke). RTO/TLP/ack_confidence→RECOVER_FWD EKLENMEDİ.
`affects_slice_a_faithfulness=false`.

## 10. DR-ARQ-TAIL-1 proposal (ÖNERİ; bu turda UYGULANMADI)
```text
DR-ARQ-TAIL-1 — Tail-Erasure Recovery Completeness. Kapsam: mevcut ürün-ARQ'nun tail-erasure sınıfını
(seq>largest_acked; ardıl-ACK-yok) kurtaramaması. Aday-çözümler: gerçek-RTO VEYA TLP (tail-loss-probe) VEYA
§5.1 ack_confidence→RECOVER_FWD tail-liveness (transport-crate). FS tail-loss-kanıtı = bu-DR'nin requirements-kanıtı.
Bu FS-baseline-DIŞI; FS-kampanyası-SONRASI ayrı-DR. ŞİMDİ UYGULANMAZ (baseline-donuk).
```

## 11. Deficit-position metadata example
`fs_runner --arm a0 --deficit 7` → `deficit_position=Tail deficit_is_tail=true source_count=8
largest_acked_observed=Some(6) tail_recovery_path=NoneStalled`. (`--deficit 3` → Middle/Arq; `--deficit 0` → Head.)

## 12. F1 architecture
Early logical-close FULL-block FEC. Experimental-flag (default-OFF); wire-değişikliği-YOK; DATA-before-parity-korunur.
Real-repair-decode = F0 (aynı K=8 blok; gerçek RsSimdCodec). Mekanizma-farkı YALNIZ close-trigger-zamanı (eligibility).

## 13. F1 seal/order proof
PIN-C fixture `ParityWireModel` DATA-first+pacer'ı modeller (sender.rs:2290 mühür). F1 eligibility=0 < F0=K×serialize
AMA wire(F1)=K×serialize == wire(F0) (parity block-DATA'yı GEÇEMEZ). Ordering-mührü gevşetilmedi.

## 14. F1 t_parity_wire advancement fixture
`f1_pin_c_structurally_non_effective_under_ordering` (fs_harness) + `f1_real_decode_is_f0_and_pin_c_non_effective`
(kama-integration): `pin_c_classify(wire_f0, wire_f1)` = **StructurallyNonEffectiveUnderCurrentOrdering**.

## 15. F1 t_repair_usable fixture
`f1_real_decode_is_f0_...`: t_repair_usable (reconstruct-complete) ≥ t_parity_arrival (codec-latency causal); byte-equal.

## 16. F3 architecture
Deadline/timer partial-block FEC. Experimental-flag (default-OFF). Partial-block K'<K close → fixed-prereg-R → gerçek
codec-repair. Close-API K-agnostik (close_fec_block_manual block_id/manual_r/parity_size) → Sender-değişikliği-YOK.

## 17. F3 trigger semantics
Deterministik: sabit-deadline → partial logical-block-close (K'=4). PathTimingState adaptive-karar YOK (F3 ≠ F4).

## 18. F3 t_parity_wire advancement fixture
`f3_pin_c_partial_block_advances_wire_time` (fs_harness) + `f3_partial_block_real_decode_and_pin_c_advances`:
`pin_c_classify(wire(K=8), wire(K'=4))` = **WireTimeAdvanced** (partial-parity 4-DATA-önce < F0 8-DATA-önce).

## 19. F3 t_repair_usable fixture
F3 partial-block gerçek RsSimdCodec (profile 0x02, K=4) encode→erasure(esi=1)→reconstruct → byte-equal (real-decode).

## 20. F4 K-selection semantics
`f4_select_k(&PathTimingState)` FROZEN: confidence-kapısı (sample∧rtprop_fresh değilse K_COLD=2); base-RTT bucket
<50ms→1 · 50–150ms→2 · ≥150ms→3. ADR-17 saf-integer; RNG-YOK; GELECEK-YOK; FEC-utility K'yi ETKİLEMEZ.

## 21. F4 two-state mechanism fixture
`f4_adaptive_k_two_states_differ_no_leakage`: base=30ms→K=1, base=220ms→K=3 (FARKLI). Confidence-kapısı: fresh-değil→K_COLD.

## 22. F4 no-leakage proof
f4_select_k YALNIZ &PathTimingState alır (başka-girdi-YOK; future-YOK). Deterministik: aynı-snapshot iki-kez → AYNI-K.
MECHANISTIC/EXPLORATORY — validity/online-select/controller-readiness KURMAZ (FS-5B).

## 23. Family-sanity table
| Aile | Pin'ler | Durum |
|---|---|---|
| A1 | emission-neutrality · egress-fairness · block-total · blind-target · hit/miss ledger | ✅ (fs_harness) |
| F0 | real-encode · real-reconstruct · byte-equality · residual-ARQ · fixed-R-purity · RECOVERED-estimator-purity | ✅ (fs_f0_closed_loop 7) |
| F1 | real-repair-decode(=F0) · PIN-C **STRUCTURALLY_NON_EFFECTIVE** (açık) | ✅ |
| F3 | real-partial-timer-trigger · real-repair-decode(0x02) · PIN-C **WireTimeAdvanced** | ✅ |
| F4 | real-K-adaptation · no-future-leakage · PathTimingState-only-prereg · exploratory-label | ✅ |
| Tümü | flag-OFF byte-özdeş · DATA-before-parity green · production-default değişmez | ✅ (382 unchanged) |

## 24. A0/A1/F0/F1/F3/F4 behavioral comparison
| Arm | Redundancy | Recovery | PIN-C | Real codec | Path-state consume |
|---|---|---|---|---|---|
| A0 | none | ARQ retransmit | — | — | no |
| A1-EARLY | block-total dup | dup (target∩D) veya ARQ-fallback | — | — | no |
| F0 | fixed-R full-block parity | FEC + residual-ARQ | baseline | ✅ encode/reconstruct | no |
| F1 | early-close full-block parity | FEC (=F0) | **NON_EFFECTIVE** | ✅ (=F0) | no |
| F3 | deadline partial-block parity | FEC (partial) | **WireTimeAdvanced** | ✅ (0x02 K=4) | no |
| F4 | adaptive-K parity | FEC (K=rule) | (mechanistic) | — (mechanism-pin) | **yes (only F4)** |

## 25. Exact sealed-wire accounting
`fs_sealed_len(E)=E+37`; E=64 → **101 B** (source==repair). A1 r=2 = 202 B (128 payload + 74 framing) =
F0 r=2 = 202 B. Pin A1_WIRE_ACCOUNTING_REAL_DATAGRAM: payload+framing==realized==channel-serialization AGREE.

## 26. Intervention-budget table (C-P4)
| Arm (r=2) | payload | framing | realized_sealed_wire |
|---|---|---|---|
| A1-EARLY | 128 | 74 | **202 B** |
| F0 | 128 | 74 | **202 B** |
Matched-r → matched-wire; BUDGET_MISMATCH-guard AKTİF (|wire_A1−wire_F*| tolerans-dışı → NO-CAUSAL-EVIDENCE).

## 27. Total-outcome-cost fields (§8)
RunResult: `arq_retransmit_bytes`, `spurious_retransmits` (A0/A1). F0Result: aynı-ikisi. AYRI-kavram: intervention-budget
(C-P4) ≠ total-outcome-cost (Pareto/oracle). SPURIOUS-ARQ = hybrid-coordination-cost, **BUDGET_MISMATCH-DEĞİL**.

## 28. spurious_retransmit accounting proof
Pin `f0_cost_split_intervention_vs_total_outcome`: F0 D2/R2 → fec_decoded=2, realized_sealed_wire=2×101 (intervention),
arq_retransmit_bytes = retransmits×101 (identity), spurious_retransmits=2>0 (FEC-erken-tamamlar, ARQ-geç → spurious).

## 29. Full timing-ledger completeness
BlockTiming: 9-legacy (t_source_loss/t_parity_{eligible,enqueued,wire,arrival}/t_arq_{eligible,wire,arrival}/
t_block_complete) + generic t_redundancy_* + **t_repair_usable**. ParityDelayClass::UsableBeforeArq = t_repair_usable
< t_arq_arrival. PIN-C wire-time AYRI (ParityWireModel). Utility-timing = t_repair_usable (conflate-YOK).

## 30. Per-run instrument-chain table
| Gate | Alan |
|---|---|
| faithfulness | realized_source_drops == configured_source_drops |
| overlap | receiver_events > 0 |
| provenance | config_hash (binary/arg-parity) |
| trace-hash | decision_log_hash (forward realizasyon) |
| **fwd/rev provenance (B1)** | forward_schedule_id · reverse_schedule_id · **configured_schedule_hash** · reverse_dropped |
| intervention | arm_intervention (RedundancyKind) |
| decision-snapshot | path_timing (ledger-only) |
İhlal → INVALID — NO EVIDENCE (asla protokol-FAIL'e dönüştürülmez). admitted() = faithful ∧ overlap.

## 31. flag-OFF regression
`cargo test --workspace` = **382 passed / 0 failed** (PIN=382 → UNCHANGED; INV-CS-5 byte-özdeş).

## 32. all-features (experimental) regression
adaptive-transport (fec-budget+cold-start) = **240 / 0** · kama-integration (fs-harness) = **22 / 0**.
Binary-smoke = **8/8 GREEN** (clean×3 · f0-real-reconstruct · a1-hit · a1-miss≡a0 · b2-tail · b3-reverse-blackout).

## 33. A1/A2 cold-start pins intact
kama-integration (experimental-cold-start) = **44 / 0** (A2 INTACT). fs-harness+cold-start combined-compile OK.

## 34. production dependency/default invariants
experimental-fs-harness = test-only (dev-dep adaptive-transport + prod-dep kama-fec-codec). Production lib-grafiği
GENİŞLEMEDİ. flag-OFF forward-YOK. Sender/loss production-kodu bu slice'ta DEĞİŞMEDİ.

## 35. Final FS-PREREG-v1.2 contents
§A (Harness-A A1.0–A1.11 özet) · §WIRE-CORRECTION (fs_sealed_len grounded 101/37; 108-SUPERSEDED) · §B (B.0
baseline-freeze · B.1 reverse-trace · B.2 tail=B_REAL + zorunlu-cümle + DR-ARQ-TAIL-1 · B.3 pairing · B.4 F1 ·
B.5 F3 · B.6 F4 · PIN-C/PIN-D) · §D (6-arm-set · statistics · deadline/horizon/faithfulness/oracle/censoring ·
§8 iki-cost-ayrımı · verdict-path). fmt: PROVENANCE'ta code-commit=cd336e9.

## 36. old/new prereg hashes
```text
old (SUPERSEDED) : FS-PREREG-v1.1 sha256=60353e99715cd4e51a42c53a94b6c1ad6963d8c3b3ef017ebc1bf2b9c3aaadf7
new (FROZEN)     : FS-PREREG-v1.2 sha256=da4af3c49195f8cfbec7d26e1b5542a6802391e5d9b86ff193bf1255739da034
reproduces       : sha256(file − "sha256:" satırı) == stored  ✓ (v1.1 konvansiyonuyla teyit)
```

## 37. ZERO result-bearing FS-1 runs before freeze — CONFIRMED
Bu slice'ta hiçbir FS-1 result-bearing-run KOŞULMADI; hiçbir outcome-bearing-veri v1.2-freeze'den ÖNCE ÜRETİLMEDİ.
Tüm koşumlar mechanism-fixture / sanity-pin / binary-smoke (result-bearing-DEĞİL; deterministik; deney-sonucu-DEĞİL).

## 38. Remaining limitations / INVALID tooling
```text
- F1 = STRUCTURALLY NON-EFFECTIVE (mevcut-ordering altında); F2/interleaved açılırsa DR-FEC-INT-1'de yeniden-değerlendir.
- Tail-erasure GERÇEK ama BASELINE-DONUK (DR-ARQ-TAIL-1 ertelendi) — FS-1 analizi tail-vs-nontail STRATIFY etmeli.
- F4 MECHANISTIC/EXPLORATORY — discriminative-validity / online-selectability FS-5B'de (bu slice KURMAZ).
- PIN-C wire-time bir DETERMINISTIK MECHANISM-FIXTURE'dır (ParityWireModel; sender.rs:2290 ordering-modeli) —
  gerçek-pacer-token-dinamiği FS-1 closed-loop'ta gözlenir; fixture ordering-mekanizmasını izole-kanıtlar.
- INVALID-tooling olayı YOK (tüm gate'ler GREEN; instrument-chain ihlali gözlenmedi).
```

## STOP
FS-HARNESS COMPLETE = A + B. FS-PREREG-v1.2 FROZEN. **FS-1 BAŞLATILMADI.** Abkarada review'ı bekleniyor.
No production Sender-recovery-fix · no controller-calibration · no FS-5B · no FS-2 · no F2 · no default-flip · no merge.
