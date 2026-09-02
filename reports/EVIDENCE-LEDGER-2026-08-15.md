# KAMA-CORE — CONSOLIDATED EVIDENCE LEDGER
**Compiled 2026-08-15 · branch `transport-baseline-closure` · read-only synthesis of 5 surveys, spot-checked against source**

> Every row below was opened and confirmed. Verdict sentences are quoted verbatim from the cited file. Where two documents disagree, both are cited in §4 and neither is silently preferred.

---

## 0. ERRATUM — corrections applied 2026-08-15 after direct re-measurement

Two rows in §1.1 were overstated in the first cut of this ledger. Both were relayed from a survey
agent and committed without my re-measuring them. Corrected here against the raw data:

* **Q5 RETRACTED.** The claimed "cell where the probe is measurably worse" is an adversarial slice
  that drops probes at 40 %, where `mean_probe_sent` = 0.2 and n = 10. Two runs of difference is
  noise. **No completion regression is established anywhere in the corpus.**
* **Q6 RECLASSIFIED.** The shipped-default / validated-config mismatch is real, but
  `set_tail_recovery_config()` already exists, so enabling correctly is a wiring call, not a
  re-measurement. It does **not** belong on the list of things blocking the tail result.

**Net effect on the tail verdict: it is stronger than the first cut implied.** 12 terminal-tail
cells, n = 30 each, 2290 rows: OFF 0-3/30 → ON 29-30/30 in **11 of 12**, with the single exception
(`terminal-tail,10,med`) failing loudly through its own `cap_exhausted` = 30 counter. **The tail
probe needs no further measurement.** One tuning item remains, and it was already recorded as
tuning-pending by the campaign itself.

This does not weaken §5 — it sharpens it. The tail mechanism is measured, proven, and still
unshippable, because it is `enabled: false` behind an unwired dependency edge. That is a coding
deficit, not a testing deficit.

---

## 1. EVIDENCE LEDGER

### 1.1 ARQ / tail recovery

| # | Product question | Status | Numbers | Source |
|---|---|---|---|---|
| Q1 | Does a bounded PTO tail probe recover terminal-tail erasures the shipped ARQ censors, **on a real wire**? | **PROVEN** | Verbatim: *"**GO — MECHANISM CORRECT, POLICY/TUNING PENDING**"*. Terminal-tail, 12 cells × 30 paired reps, N=360/arm: OFF **8/360 (2.2 %)** vs ON **329/360 (91.4 %)**. ON-only 27–30/30 in 11 of 12 cells; **OFF-only = 0 in every cell**. Cost +101…+404 B, 1–4 retransmitted units. | `reports/dr-arq-product-tail-1/v1/FINAL-CHECKPOINT.md` |
| Q2 | Is it bounded and invariant-clean? | **PROVEN** | 2290/2290 rows (2160 core + 130 adversarial), 0 missing/extra/duplicate/skip. **global max `probe_sent` = 3 = cap**; `inv_fail` = 0 across all 2290; `sum_spurious` = 0 across all adversarial slices; OFF rows with probe activity = 0. Frozen: commit `c46e8fa3`, binary `a1113236`, manifest `bf849aba`, rustc 1.96.0. | same, §2 |
| Q3 | Do the reorder / delayed-ACK guards hold? | **PROVEN** | reorder OFF 1.00 / ON 1.00, `probe_sent` 0. delayedack OFF 0.00 / ON 0.20, `probe_sent` 0. jitter, pathpause (mid-run 100 % blackout), rttstepup, rttstepdown: OFF 0.00 → ON 1.00. | same, §5 + `v1/CELL-RESULTS.csv` |
| Q4 | Is tail recovery ready to ship default-ON? | **NO — PARTIAL** | **One real tuning blocker**, plus two items reclassified after direct re-measurement (see Q5 retracted, Q6 wiring). (T1) `terminal-tail,10,med,on`: **0/30 complete, `sum_cap_exhausted` = 30, `probe_sent` = 3.0, +303 B pure cost** — cap spent before packet-threshold ARQ engages. (T2) high-loss cold-start token starvation in PreConverge (harness warms up first; a default-ON product would not). (T3) `dl_hit` = 0.0 in **every** terminal cell, both arms — no product SLO exists. | `reports/dr-arq-product-tail-1/v1/FINAL-CHECKPOINT.md` §6 + `CELL-RESULTS.csv` |
| Q5 | Is there a cell where ON is measurably **worse**? | **RETRACTED — NOT SUPPORTED** | Original reading: `probeloss,40,med` OFF **3/10** vs ON **1/10**. Re-measured directly from `CELL-RESULTS.csv`: it is an **adversarial slice that drops the probes themselves at 40 %**, and `mean_probe_sent` = **0.2** with `max_probe_sent` = 1 — the probe barely fires. A 2-run difference at **n=10** is indistinguishable from noise. The checkpoint was right to record only that the cap held. **No completion regression is established anywhere in the corpus.** |
| Q6 | Was the **shipped** config the one validated? | **NO — but this is a WIRING decision, not a measurement gap** | `TailRecoveryConfig::default()` (`tail_recovery.rs:36-46`) = cap **2**, min_pto 1 ms, max_pto 60 s, backoff 120 s. Validated `tail_on_config()` (`dr_arqtail_product.rs:36-46`) = cap **3**, 10 ms, 10 s, 30 s. `sender.rs:1225` constructs from `default()`. **But `set_tail_recovery_config()` already exists at `sender.rs:1420`** — so the correct way to enable is to call the setter with the validated config, never to flip the default. That path is already in the code. **No re-measurement is required.** |
| Q7 | In-process tail effect (precursor evidence) | **PROVEN (in-process only)** | R2A C1, 9 S1 cells × N=100: A0-FROZEN 0/100 → A0-TAIL 100/100 at d1 (all RTTs, +101 B); rtt1000-d4 29/100 → 74/100 (+650 B). A0-FROZEN-only-complete = 0 in every cell. | `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md` §C1 |

### 1.2 FEC / proactive redundancy

| # | Product question | Status | Numbers | Source |
|---|---|---|---|---|
| Q8 | Does **coding** beat blind duplication at matched configured budget? | **PROVEN** (strongest, most framing-safe FEC result in the repo) | R2A C3: 2100 paired rows, 21 cells, N=100. **F0-only-complete = 868, A1-EARLY-only-complete = 0.** Example s1-rtt50-d1: F0 100/100 vs A1 0/100 at equal 6060 B. Independently in FS-1: `O2_FEC_SPECIFIC` in **12/12** cells, F0>A1 true 12/12, at 1212 B == 1212 B `NON_DOMINATED`. | `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md` §C3; `reports/fs-1/analysis/v1.3.2/FS-1-ORACLE-VERDICT-v1.3.2.md` |
| Q9 | Does adding proactive redundancy on always-on ARQ ever **reduce** completion? | **PROVEN — never** | r0→r2 over 21 R2A cells: **`base_only` = 0 in 21/21**. Verdicts 15 POSITIVE / 5 NO_COMPLETION_CHANGE / 1 INDETERMINATE. | `reports/fs-fusion-cal-1/FRAMING-CORRIGENDUM.md` §4 |
| Q10 | Where does redundancy pay best? | **PROVEN** | S3 IID-20 %: 12/100→66/100, 12→62, 14→59 (p≈0) at the **cheapest** cost +772…+842 B. Realized wire: F0 5526 B vs A0-TAIL 6188 B at rtt1000 — FEC is both better and cheaper here. S1 tail: 0→100, 32→97, 29→81 at +1017…+1212 B. S2 non-tail: +5…+8 or pure cost. | same §4; `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-CELL-RESULTS.csv` |
| Q11 | Is FEC's tail advantage coding-causal, or was it ARQ tail-blindness? | **CONTESTED — two live readings** | R2A C2 over 900 S1 paired rows: A0-TAIL-only **64** > F0-only **26**, both 810; F0 costlier in 8/9 S1 cells; s1-rtt50-d4 A0-TAIL 100/100 @5224 B Pareto-dominates F0 83/100 @5767 B. **But** that exact contrast was later ruled an invalid cross-axis comparison. See §4.3. | `…R2A-FINAL-REPORT.md` §C2 vs `reports/fs-fusion-cal-1/FRAMING-CORRIGENDUM.md` §2 |
| Q12 | Is proactive redundancy worth its bytes **on top of tail-capable ARQ** (the actual fusion question)? | **NOT-MEASURED** | Coverage matrix: ARQ-frozen has r0/r2/r4; ARQ **tail-capable has only r0** — r2 and r4 literally *"absent"*. Absent from all 10,930 authoritative rows. Verbatim: *"the (tail-capable, r>0) fusion cell is genuinely absent from every authoritative dataset."* | `reports/fs-fusion-cal-1/FRAMING-CORRIGENDUM.md` §3 |
| Q13 | FEC marginal utility vs RTT, wall-clock, faithful netem | **PARTIAL (emission-model-bound)** | 48 runs / 0 fail, measured RTT 250.09 / 1000.18 / 2500.33 ms. rtt250 p50 completion r≥d vs r<d: d1 −46 ms, d2 −52 ms, d4 −77 ms; FEC-path 30/30. rtt1000 & rtt2500: `NoMarginalFecUtility`, FEC-path 15/30 → 6/30 → 0/30. Mechanism: parity emitted at **block close** under ACK-clocked source ⇒ round-trip-skipping advantage erased at high RTT. | `reports/d1-exit/FS-wallclock-fec-utility.md` |
| Q14 | Does FEC make the 305 ms deadline? | **PARTIAL — deficit ≤ 2 only** | Exact hits/N, N=30/cell: d1 0/30 (0.0 %) → **30/30 (100.0 %)**; d2 0/30 → **28/30 (93.3 %)**; d4 0/30 → **4/30 (13.3 %)**, p50 = 315 ms > 305 ms. Verbatim: *"deficit=4: GEÇERSİZ … d4'te MarginalFecEssential YOK."* Data hole recorded: true zero-parity d2/r0 and d4/r0 were **never run** at rtt250. | `reports/d1-exit/FS-CORRIGENDUM-deadline.md` |
| Q15 | Does **any** arm meet a real-time deadline? | **PROVEN — essentially no** | R2A `deadline_hit` of 2100 rows/arm: A0-FROZEN **0**, A0-TAIL **46**, A1-EARLY **0**, F0 **0**. Verbatim: *"under the frozen tight deadlines NONE of these arms is a real-time success."* FS-1: `deadline_hits` = 0 in all 48 cell/arm rows. Product tail: `dl_hit` = 0.0 in every terminal cell. | `…R2A-ENDPOINT-AND-BUDGET-CORRIGENDUM.md` §1; `FS-1-CELL-RESULTS-v1.3.2.csv` |

### 1.3 Fusion / calibration

| # | Product question | Status | Numbers | Source |
|---|---|---|---|---|
| Q16 | Is there a standing fuzzy calibration verdict? | **NO VERDICT** | Verbatim: *"The earlier **CALIBRATION-NO-GO** in `FINAL-REPORT.md` is **WITHDRAWN AS INVALID**."* and *"No verdict is asserted here."* Cause: the decisive contrast changed both orthogonal axes, and the harmful false positive came from `PRODUCT_NETNS`, *"a dataset that contains no FEC arm at all."* | `reports/fs-fusion-cal-1/FRAMING-CORRIGENDUM.md` §1–2, §5 |
| Q17 | Can loss **rate** alone gate FEC activation? | **PROVEN — no** | Pre-registered group-disjoint holdout (TRAIN 52 cells @50/250 ms, HOLDOUT 25 @1000 ms): 12 MATCH / **1 harmful false positive** / 1 conservative FN / 11 abstain. At the identical 200 000 ppm, IID structure wants FEC and burst structure wants ARQ. TRAIN-only boundary collapsed to a degenerate 199 999 → 200 000 gap. | `reports/fs-fusion-cal-1/FINAL-REPORT.md` §5 |
| Q18 | Does the **shipped** fuzzy controller work? | **PROVEN DEFECTIVE (a code finding)** | `Sender::new()` → `ValidatedFuzzyConfig::trusted_fixture()` (`sender.rs:1188`), which skips `validate()`. Fixture sets `loss.low = shoulder_low(200_000, 300_000)` (`fusion.rs:330`) ⇒ **μ_LOW = 1.0 at 200 000 ppm** ⇒ 20 % loss classifies fully LOW ⇒ rule R1 fires `fec_zero`. R2 — the only rule that can raise FEC to `fec_high` — is conjoined with bw HIGH at 150–200 kB/s while the entire tested regime is ≈125 kB/s. **The engine cannot open FEC in exactly the region (S3 IID-20 %) where FEC Pareto-dominates.** | `reports/fs-fusion-cal-1/FINAL-REPORT.md` §3, verified in source |
| Q19 | Was the calibrated policy wired into the product? | **PROVEN — no, it is dead code** | `fusion_policy.rs` = 659 lines, 15/15 tests. Exhaustive grep: total references outside the module are **3 lines** — `lib.rs:44 pub mod fusion_policy;` plus two **untracked** FS-CAL-2 test files importing `FusionSafetyConfig` as a grounding constant. `FusionPolicy::calibration_v1()` is constructed only inside its own `#[cfg(test)]`. | verified by grep over `crates/` |
| Q20 | Can any future calibration be fitted from banked data? | **NOT-MEASURED — structurally blocked** | **`runtime_features_present` = 0 on 10 930 / 10 930 rows.** Verbatim: *"they cannot fit a runtime-feature → action map. Every downstream limitation in this report descends from that fact."* No `loss_ppm`, bandwidth, jitter, `qd_ratio`, `ack_confidence` was ever recorded alongside an outcome. | `reports/fs-fusion-cal-1/FINAL-REPORT.md` §1 + `ingest-ledger.json` |
| Q21 | Loss **structure** estimator | **NOT BUILT** | Burstiness / run-length / gap statistics computed nowhere in `adaptive-transport`. `loss.rs` retains per-event `seq` and `declared_ns` but never differences, sorts for adjacency, or run-length-encodes them. Named blocking prerequisite #1. | `reports/fs-fusion-cal-1/FINAL-CHECKPOINT.md` |

### 1.4 Cold start

| # | Product question | Status | Numbers | Source |
|---|---|---|---|---|
| Q22 | Can bounded duplicate-credit open the cold-start stall? | **PROVEN — no, structurally** | Verbatim: *"structurally incapable … must not be retried by adjusting permit counts, debit size, or budget."* Baseline stalls 3, **rescued 0/3, 1 NEW stall created** (rep39 62 904 ms). rep9: 2 permits, 202 B emitted, `first_delivery_ns` 8.533 s, `first_ack_ns` 9.033 s, `ack_batches` 2 — yet **`max srtt_us` = 0** all run. Cause: Karn gate at `sender.rs:2060` (`p.origin == TransmitKind::Data`). `tx_data` 20→18. | `reports/dr-cs-2/DR-CS-2-RECOVERY-PERMIT-FAILED.md` |
| Q23 | Does the bootstrap RTT probe/echo ladder open it? | **PARTIAL — panel FAIL, default OFF** | Verbatim: *"`DR-CS-3 PANEL PREFLIGHT PASS · LOCKED PANEL FAIL (EFFICACY 2/3) · BOOTSTRAP RTT DEFAULT-OFF · PRODUCT RUNTIME INTEGRATION PENDING`"*. rep9 ✓, rep17 ✓, rep14 ✗. rep14: 4/4 probes emitted within budget (176 B ≤ 200 B), 4/4 authenticated, 4/4 echoes at 172 B, **0 echoes reached the wire** (reverse stream [D,D,D,D]). Where a round trip survived: 62 904→4000, 27 065→4000, 23 807→4000 ms. **6 of 13 §8 gates NOT RUN.** | `reports/dr-cs-3/panel/DR-CS-3-DEFAULT-DECISION.md` |
| Q24 | Did the V2 panel re-run resolve it? | **PROVEN — it answered nothing** | Verbatim: *"`UNEVALUABLE — FROZEN CHALLENGE PHENOTYPE NOT REPRODUCED UNDER V2`"*. Cause measured, not assumed: RNG **key re-domaining** — reps 9/14/17 went from `DDDDDDDDDDDDDDDDDDDD` to `....................` at draw 0. Re-keying to bring the stalls back is forbidden (§2c). | `reports/transport-baseline-closure/EXECUTION-STATE.md:311-315` |
| Q25 | Does startup pacing / rate ramp help? | **PROVEN — but the evidence is untracked** | 115 pairs / 260 runs. Locked replays 2→0 (median epoch 65 000→47 595 ms). GE-L10 p=20 % RTT=1000: 1→0 (14 029→8 567). IID p=5 %: 11 427→7 017. No-loss: 2 446→1 427. **Delivery worse in 0/115 pairs.** | `reports/fs-cal-2/v3.5/DR-RATE-RAMP-2-PRODUCT-REPORT.md` — **0 of 1801 files in `reports/fs-cal-2/` are tracked by git** |
| Q26 | Product cold-start end-to-end? | **PARTIAL** | Verbatim: *"PRODUCT COLD-START PARTIAL — real Noise handshake-derived Split key EKSİK (Sprint-6 kripto)"*. Cell (i) unified 65 B heal CLOSED (`c710cff`). Cell (ii) real ReceiverHost + RSS × 7 netem profiles × 3 = **21/21 HARD-boundedness**, retained max **0.21 % of cap**, cleanup→0 (`8a1616b`). Cell (iii) **OPEN**, sealed to Sprint-6. | `docs/reports/dr-cs-1b-a2-dur-son.md` |

### 1.5 Competitive position / infrastructure

| # | Product question | Status | Numbers | Source |
|---|---|---|---|---|
| Q27 | Does Kama beat QUIC? | **CLAIMED-BUT-UNVERIFIED — headline retracted elsewhere** | Published: p5_revpath Kama c10=c120 **1.00** IQR[1.00,1.00] vs quinn 0.00 / 0.27; p1_loss50 0.53/0.85 vs 0.13/0.15; p2_loss80 0.44 vs 0.00; good network 0.97–1.00 vs 1.00. **But** the p5 headline was measured as a false-loss-storm parasite: *"p5 100/100'ün false-loss-fırtınası PARAZİT recovery'si olduğu | TUR-1a smoke: p5 24/100, dup 93673→0 | **ÖLÇÜLDÜ**"*; later bimodal {11, 24, 100, 100, 100, 100}. **`docs/results/kama-vs-quinn-v1.md` carries no retraction note.** | `docs/results/kama-vs-quinn-v1.md:21,55` vs `docs/design/time-model-v1.md:37,47,282` |
| Q28 | QUIC baseline | **PARTIAL — good data, contaminated instrument** | 70/70 runs, quinn 0.11.11, 5 repeats/cell. Three regimes: AYAKTA c120=1.00; SALLANIYOR (50 % loss) cubic 0.15; ÖLÜ p2/p3 c120=0.00, p5 c120=0.27. **But**: *"pin'siz TÜM geçmiş sub-kbps ölçümler (quinn baseline dahil) bu artefaktı içerir"* (kernel ARP/neighbor drop; netem `dropped=0`). Argued symmetric; required to be re-run pinned; **not re-run**. | `docs/results/quinn-baseline-v1.md`; `docs/design/time-model-v1.md:68-73` |
| Q29 | Quinn / TCP / KCP comparison on the faithful harness | **NOT-MEASURED** | Verbatim: *"Quinn/TCP/KCP comparison \| NOT STARTED \| — \| — \| — \| (no result exists)"* and *"'Kama beats Quinn/TCP/KCP' — NO comparison run."* | `reports/AI-HANDOFF-CURRENT.md:45,209` |
| Q30 | Blackout **survival** as a binary metric | **NOT-MEASURED** | BENCH-01, open since D1. `p6_blackout` = 1.00 is *intermittent* blackout only (2 s/8 s, idle-timeout 300 s). Without it the §4.4 Tier-2 claim cannot be made. | `docs/results/quinn-baseline-v1.md` §10; `CLAUDE.md` §10 |
| Q31 | Burst-loss behaviour of any mechanism | **NOT-MEASURED** | R2B (S4 NaturalBurst + S5 probe-loss-eligible + C4) split out pre-outcome, never executed. Every coding-vs-duplication result is explicitly *"NOT generalized to burst."* | `…R2A-FINAL-REPORT.md` §C3 |
| Q32 | Does a product runtime exist? | **PROVEN — no** | Verbatim: *"Binding technical debt. Until it is closed, the transport is **not** a usable product."* Nine missing pieces enumerated. Verified: **7 of 14 crates are 3-line doc-comment stubs** (crypto, ccr-codec, client-core, hub-server, discovery, ra-ca, admin-server); **only `crates/bench` declares `[[bin]]`**. | `reports/dr-cs-3/PRODUCT-RUNTIME-INTEGRATION-PENDING.md` |
| Q33 | Is the ADR-18 mandatory nightly netem gate live? | **NOT-MEASURED — the gate does not exist** | One workflow (`ci.yml`). No `schedule`/`cron`, no netem job, no 7-profile matrix. Every netem run in the record was hand-launched from `scripts/netem/*.sh`. Worse: `cargo +nightly miri test -p crypto -p ccr-codec` targets two **3-line stubs** — the miri gate is vacuous. | `.github/workflows/ci.yml:86` (verified) |
| Q34 | Effort allocation | **PROVEN** | src **24 636 LOC** (adaptive-transport 16 687 = 68 %; seven crates 3 LOC each) vs tests+examples **42 584 LOC** vs **318 .md / 50 720 lines** vs **21 874 files under `reports/`**. Ratio test:src = **1.73:1**; prose:src = **2.06:1**. | measured directly |

---

## 2. WHAT IS GENUINELY MISSING
*Ranked by information gain per unit of work.*

**1. A prefix-causal loss-STRUCTURE estimator (burstiness / run-length / gap).** ~1 file. `loss.rs` already retains per-event `seq` and `declared_ns` and simply never differences them. It is the named blocking prerequisite for *every* FEC-activation policy, and Q17 proves loss rate alone is provably insufficient (at 200 000 ppm, IID wants FEC and burst wants ARQ). Nothing else unblocks so much for so little. An untracked `crates/adaptive-transport/src/fate_estimator.rs` may already be a start — status unverified, and it is outside version control.

**2. The fusion cell: (tail-capable ARQ, r>0), one new arm.** The single named unmeasured cell in the whole corpus, and the only genuinely open scientific question. Both mechanisms already exist and are already validated **separately** (`tail_recovery.rs` GO on 2290 rows; F0/F3 arms in the spine harness). This is one arm on an existing harness, not a campaign. Every "fusion" conclusion in the tree is currently about redundancy on top of tail-**blind** ARQ.

**3. Runtime telemetry recording in the harness.** Small work, permanent unblock. `runtime_features_present = 0` on 10 930/10 930 rows means **re-running any existing grid without this reproduces the gap exactly**. Do it before item 2 so the fusion arm banks telemetry the first time.

**4. Adaptive tail cap/backoff + reconcile the shipped default with the validated config.** Bounded, well-specified, closes the only named tuning blocker (T1: `terminal-tail,10,med,on` 0/30, cap exhausted 30/30). Must be paired with the config reconciliation from Q6 — flipping `enabled=true` today ships `max_tail_probes: 2`, a value that has never been run.

**5. Product runtime (M4) — the nine pieces.** Highest absolute value, highest cost, and **everything downstream is unassessable by construction until it lands**: egress displacement, backpressure, datagram batching, any deployment claim. The repo says so itself.

**6. Quinn / TCP / KCP on the faithful two-netns harness.** The only thing that can support the §4.4 Tier-1 claim. The harness already exists (`examples/dr_arqtail_product` + `scripts/netem/`). Ranked below the others only because the current baseline is contaminated (Q28) and would have to be re-run pinned anyway.

Not on this list, deliberately: **BENCH-01** (blackout survival binary metric) and **R2B** (burst) are real gaps, but both are downstream of items 3–6 and neither changes a product decision this quarter.

---

## 3. REDUNDANCY FINDINGS

**The tail question was asked five times at escalating fidelity.** That is mostly *correct* practice — but three of the five are now dead weight and should stop being cited or maintained:

| Instance | Scale | Disposition |
|---|---|---|
| FS-1 tail strata | **5 rows per arm** | **RETIRE as tail evidence.** The artifact prints `tail_rows=1` beside `complete+censored=5` — the completion columns are **full-cell n=5 values, not the 1-row tail subset**. It makes FS-1's tail evidence look 5× larger than it is. Verified against `FS-1-CELL-RESULTS-v1.3.2.csv`. |
| DR-ARQ-TAIL-1 R1 diagnostic | 5 configs | **SUPERSEDED** by R2A. Self-labelled *"diagnostic; not confirmatory."* |
| R2A C1 | 9 cells × N=100 | **KEEP as dataset**, stop citing as product evidence — in-process deterministic channel, A0-TAIL is an experiment-only deconfounder. |
| **DR-ARQ-PRODUCT-TAIL-1** | 12 cells × 30 × 2 arms, real netns/netem, real product `Sender` | **AUTHORITATIVE.** This is the one to cite. |
| FS-FUSION-CAL-1 `marginal_tail_capability_at_r0` | re-analysis of the same 10 930 rows | **NOT INDEPENDENT.** Presenting it as confirmation double-counts. |

**Four episodes of genuine, unambiguous waste:**

1. **FS-1 was frozen and executed four times to bank one 240-row dataset** — ≈720 rows executed and discarded. v1.3 invalidated before first result; v1.3.1 superseded on schema non-conformance (RawRecord 27 keys vs frozen 54-field schema); first v1.3.2 invalidated by an **arm-label serializer bug** (wrote internal `"A1"` instead of canonical `"A1-EARLY"`, zeroing an arm). **Two of three failures were serializer defects, not science defects.** The repo's own KALICI-KURAL-3 was created from this.
2. **FS-CAL-2 ran six generations (v3 → v3.5) and produced zero result-bearing calibration runs.** 1801 files on disk, **0 tracked by git** (verified). The string "result-bearing calibration runs" appears in 10 separate checkpoint documents. `v3.3` and `v3.4` carry duplicate same-named checkpoints (`DR-ARQ-TAIL-2-REPORT.md`, `DR-CS-2-PRODUCT-REPORT.md`, `PRODUCT-ESTIMATOR-INTEGRATION-CHECKPOINT.md`, `FS-CAL-2-YETKI1-REGROUND-CHECKPOINT.md`) with `NOT STARTED` boilerplate. The one real output — the 260-run rate-ramp fix — is the sole evidence for a **currently-shipping product default** and is **not in version control**.
3. **The bootstrap probe ladder is implemented twice in the same file, both live.** `sender.rs:192` `CS_PROBE_FIRES_NS = [3,9,21,45]s` (DR-CS-1, feature-gated) and `sender.rs:595` `BS_FIRES_NS = [3,9,21,45]s` (DR-CS-3, default build). The second one's own doc comment reads verbatim: *"Same values as the legacy `CS_PROBE_FIRES_NS`; no new constant."* Same codepoints 0x19/0x1A, same 44 B/43 B, same 176 B ≤ 200 B budget. The legacy one is documented as superseded and was **never removed**.
4. **The DR-CS-3 panel was rebuilt and re-run and answered nothing** (`UNEVALUABLE`), and the displacement metric written to gate it was later found hard-wired to zero (`first_probe_tick` never initialised). Separately, the 32×18 neutrality PASS was committed (`832e6ef7`) and reclassified *"NOT CITED AS EVIDENCE / NOT A PASS GATE"* the same day (`578952e7`) because the comparator lived in `/tmp` and was lost.

**Harnesses that duplicate each other:** `run_spine` (in-process — FS-1, R2A), `dr_arqtail_product` (netns/netem, real product Sender), `fs_cal2_spine` (untracked). **`dr_arqtail_product` is authoritative for any product claim.** `fs_cal2_spine` has produced nothing and is untracked — stop maintaining it or commit it.

**Where the redundancy hypothesis does NOT hold:** the four major campaigns are *not* the same test re-run. FS-1 (in-process screening) → R2A (deconfounding with a tail-capable arm) → product-tail (real wire) → wallclock (RTT boundary) each answer a genuinely different question, and the corrigendum culture caught real defects each time. The measurement *discipline* is the best thing in this repository and should not be cut.

---

## 4. CONTRADICTIONS

**4.1 Fusion verdict — retracted in one file, still headline in two others.**
- `reports/fs-fusion-cal-1/FINAL-REPORT.md:6` — *"## VERDICT: **CALIBRATION-NO-GO**"*
- `reports/fs-fusion-cal-1/FINAL-CHECKPOINT.md:9` — *"| Verdict | **CALIBRATION-NO-GO** |"*
- `reports/fs-fusion-cal-1/FRAMING-CORRIGENDUM.md:3` — *"is **WITHDRAWN AS INVALID**"*
Neither of the first two carries a retraction banner. **The corrigendum wins** (later, explicit, self-describing as binding). Correct standing status: **no verdict**. `calibration-v1.json` is SUSPENDED and `holdout-*.{csv,json}` are VOID — yet the checkpoint still presents them as DONE evidence.

**4.2 Startup pacing default — the code says ON, four records say OFF, and the cited reason is withdrawn.**
- `crates/adaptive-transport/src/sender.rs:684` — `Self { enabled: true }` (flipped by commit `925dea66` *"restore startup pacing default-ON"*)
- The **same function's** comment block, immediately above that line: *"Default stays OFF until that is root-caused."* And the struct doc: *"Master gate. Default false → inert."*
- `reports/transport-baseline-closure/EXECUTION-STATE.md:132` — *"| startup pacing | **OFF** (blocked by the FEC-parity starvation above) |"*
- `reports/dr-rate-ramp-2b/FEC-PARITY-OBSERVATION-CORRIGENDUM.md` — that very FEC-parity blocker is **withdrawn**: both arms emit all four parity units; floor ON emits them **25 ms earlier**; the "defect" was a harness bug (two `on_event` calls per iteration, ACK return value discarded).
Flip history: ON → OFF (`68165aca`) → ON (`925dea66`). **A live product default is governed by a stale comment citing a retracted defect, and its only validation report is untracked.** This is the most dangerous inconsistency in the tree.

**4.3 R2A C2 vs the fusion framing corrigendum — never reconciled.**
- `…R2A-FINAL-REPORT.md:17` presents F0-vs-A0-TAIL as *"the deconfounding payoff."*
- `FRAMING-CORRIGENDUM.md` §2 says that exact contrast *"changes both orthogonal axes at once. It is not a valid contrast under any framing."*
No corrigendum was ever filed against R2A for this. Both documents are current by their own labels. The C2 **numbers** are real; the C2 **conclusion** is under a formal objection.

**4.4 R2A budget wording — retracted in an adjacent file.**
- `…R2A-FINAL-REPORT.md:19` — *"at matched realized-wire budget … NON_DOMINATED in 21/21 cells"*
- `…R2A-ENDPOINT-AND-BUDGET-CORRIGENDUM.md` §2 — *"is **RETRACTED**"*; realized wire: equal 3/21, F0 cheaper 18/21, never more expensive. Also re-scopes every "completion" figure from deadline to **horizon**. The locked report was deliberately not modified; both texts stand side by side.

**4.5 Quinn comparison — three documents, three answers.**
- `docs/results/kama-vs-quinn-v1.md:21` — a completed 35+35-run comparison, *"Kama 1.00, quinn 0.00."*
- `docs/design/time-model-v1.md:37` — that number **measured** as a false-loss parasite.
- `reports/AI-HANDOFF-CURRENT.md:45` — *"NOT STARTED … (no result exists)."*
Honest reading: a comparison **was** run, its headline **is** an artifact of a since-fixed false-loss storm, it has **never been re-proven**, and the handoff overstates in the other direction. `docs/results/` still publishes the retracted numbers un-annotated.

**4.6 "No product FEC send-path encoder" — true and misleading at the same time.**
- `reports/fs-fusion-cal-1/FINAL-REPORT.md` §4 — *"there is no product send-path FEC encoder."*
- But `crates/kama-fec-codec` **exists** with `reed-solomon-simd = "3.1.0"` and full `encode`/`decode`; and `sender.rs` has carried a complete controller-driven parity chain since 2026-07-22 (`FecActuationMode::Controller` → `fec_controller_latch` → `compute_requested_r` → `push_parity_authorized` → paced emit), default `Manual { enabled: false }` at `sender.rs:1244`.
**Reconciliation:** both are literally true at different layers — `adaptive-transport` declares only `thiserror` + `tokio` and by design *"parity BYTE'i ÜRETMEZ."* The missing piece is **the dependency edge**, not the encoder. Reading the checkpoint's prerequisite #2 as "build an encoder from scratch" would re-derive banked work.

**4.7 Milestone state disagrees with itself inside one file.**
- `EXECUTION-STATE.md:15` — *"| M2 bootstrap cold-start | not started |"*
- `EXECUTION-STATE.md:180` — *"# M2 — DR-CS-3 (IN PROGRESS)"*
- `RESUME-NEXT-SESSION.md:119` — *"## M2 STATUS: RED — 5 blocking items."*

**4.8 FEC deadline utility.** `FS-wallclock-fec-utility.md` §49-51 asserts *"~%100"* for deficit 1/2/4; `FS-CORRIGENDUM-deadline.md` recomputes **13.3 %** at d4. Same directory; the first is unmarked.

**4.9 Status board vs the shipping audit.** `docs/roadmap/kama-transport-roadmap-status-board.md:32` (2026-07-26) — *"Probe/echo bootstrap RTT | Uygulandı | **GO**"* — for a mechanism that was subsequently rebuilt, panel-tested, **failed 2/3** and left default-OFF. The board is stale by four campaigns and is described as the artifact that goes to customer technical evaluation.

---

## 5. THE CODING DEFICIT

**The hypothesis holds. But the sharpest evidence is not the one anybody has written down.**

### 5.1 The finding: the workspace is two disconnected dependency islands

```
ISLAND A (decision engine)          ISLAND B (wire / FEC / crypto / receiver)
adaptive-transport  16 687 LOC      kama-noise-wire      3 182 LOC
  deps: thiserror, tokio            kama-engine          1 140 LOC
  ── NO internal crate deps ──      kama-integration     1 004 LOC
                                    kama-fec-codec         575 LOC (reed-solomon-simd)
                                    kama-transport-schema  472 LOC
```

`kama-engine`'s own doc comment states it: *"`adaptive-transport`'u üretimde **TUTMAZ** (yalnız direktif üretir — B2-bootstrap testi Sender'ı **dev-dep** ile sürer)."* And `kama-integration`'s Cargo.toml says: *"adaptive-transport **prod-dep'i F1'de** (kompozisyon-doldurma) **gelir**."*

**No crate in this workspace has both halves in its production dependency graph. The only edge between them is a `[dev-dependencies]` entry.** The transport that decides *what to send* and the engine that *seals, encodes and recovers it* are joined exclusively inside test builds. That is the coding deficit in one sentence, and it explains every downstream symptom.

### 5.2 Measured but not wired into the product

| Mechanism | Evidence | Product state |
|---|---|---|
| Bounded tail probe | **GO**, 2290/2290 rows, faithful netem | `enabled: false` — **and the default config was never validated** (cap 2 vs the tested 3) |
| Bootstrap RTT ladder | mechanism 4/4 correct; panel 2/3 | `enabled: false`; 6 of 13 gates NOT RUN; **implemented twice** |
| Calibrated fusion policy | 659 LOC, 15/15 tests, holdout-evaluated | **ZERO product call sites.** Dead code. |
| FEC actuation | full plan→latch→enqueue→pace chain since 2026-07-22 | `Manual { enabled: false }`; **no dependency edge to the encoder that exists** |
| Startup pacing | 260 runs, 0/115 pairs worse | **ON** — governed by a stale comment saying OFF, citing a withdrawn defect, validated by an untracked report |
| Loss-structure estimator | proven necessary (Q17) | **not written** |

### 5.3 Corroborating structural facts

- 7 of 14 crates are 3-line doc-comment stubs — including `ccr-codec`, which CLAUDE.md §3 names as the **byte-level wire authority**, and `crypto`, which blocks the last open cell of product cold-start.
- Only `crates/bench` declares a `[[bin]]`. There is no product binary.
- The `miri` CI job targets `-p crypto -p ccr-codec` — **both stubs**. The gate is vacuous.
- The ADR-18 nightly netem gate, declared MANDATORY in CLAUDE.md §7, **is not in CI at all**.
- 42 584 LOC of test/example code and 50 720 lines of prose against 24 636 LOC of src, 68 % of which is one crate.

### 5.4 Verdict on the hypothesis

**The user is right, with one correction and one defence.**

*Correction:* the waste is **not** primarily "the same test run repeatedly." Four campaigns asked four genuinely different questions and each caught real defects. The identifiable duplicated effort is narrower than suspected — FS-1's four freezes, FS-CAL-2's six empty generations, the double-implemented ladder, the startup-pacing flip cycle, and three superseded tail measurements. Call it a meaningful minority of the effort.

*The real deficit is worse than redundancy:* **~11 000 rigorously banked rows measured mechanisms that have no dependency path to a product, because the product does not exist and the two halves of the codebase are not connected.** Four validated mechanisms sit default-OFF. A 659-line calibrated policy has zero call sites. The FEC encoder and the FEC ratio planner live in crates that cannot see each other outside a test build. That is not over-testing — it is testing that outran wiring by months.

*The defence:* the corrigendum culture — withdrawn verdicts, pre-registered holdouts, honest PARTIAL rulings, retractions filed against your own results — is unusually good and is the reason this ledger could be written at all. **Do not cut the discipline. Cut the scope it is applied to, and point it at wiring.**

---

## 6. RECOMMENDED NEXT ACTIONS

**1. Resolve the startup-pacing default contradiction — today, before anything else.**
A live product default is ON while its own in-code rationale, the execution ledger and the user's memory index all say OFF, the cited blocker is formally withdrawn, and the only validating report is one of 1801 untracked files. Either the comment or the code is wrong and nobody currently knows which. *Beats the alternatives because it is under an hour, it is the only item on this list that can silently corrupt every future measurement, and it requires no new data.* Same commit: `git add` the FS-CAL-2 evidence, or delete the default.

**2. Write the loss-structure estimator (burstiness / run-length / gap) in `loss.rs`.**
~1 file, on data the module already retains and never differences. *Beats the alternatives because it is the named blocking prerequisite for the entire FEC-activation program, Q17 proves the current input (loss rate) is provably insufficient, and no amount of further measurement can substitute for it.* Check whether the untracked `fate_estimator.rs` already starts this before writing a second one — that is exactly how the ladder got implemented twice.

**3. Draw the missing dependency edge: one composition crate that holds `adaptive-transport` **and** `kama-engine` in production deps.**
This is M4 item 1 ("name the real product crate/binary owner") and it unblocks items 2–9 of `PRODUCT-RUNTIME-INTEGRATION-PENDING.md`. *Beats the alternatives because until it exists, every mechanism validated in this ledger is unshippable by construction, `fusion_policy.rs` cannot be called from anywhere, the FEC ratio can never reach the encoder that already exists, and egress/displacement/backpressure remain permanently unassessable.* It is also the cheapest way to make the existing 11 000 rows mean something.

**4. Run the fusion cell — (tail-capable ARQ, r>0) — with runtime telemetry recorded.**
One new arm on an existing harness, plus the telemetry fields. *Beats a new campaign because it closes the single named unmeasured cell in the corpus, both mechanisms are already independently validated, and recording telemetry the first time prevents reproducing the `runtime_features_present = 0` gap that voided three campaigns' calibration value.* Do not re-run FS-1, R2A or the product grid — they are banked and their questions are answered.

**5. Add retraction banners to the four superseded documents, and delete or commit the untracked trees.**
`fs-fusion-cal-1/FINAL-REPORT.md` + `FINAL-CHECKPOINT.md` (verdict withdrawn), `docs/results/kama-vs-quinn-v1.md` (p5 headline is an artifact), `FS-wallclock-fec-utility.md` §49-51 (superseded by the deadline corrigendum), `docs/roadmap/…status-board.md` (stale by four campaigns, and it is the customer-facing artifact). Then either commit `reports/fs-cal-2/` (1801 files) and `crates/adaptive-transport/src/fate_estimator.rs` or delete them. *Beats more measurement because a reader landing on any of these first takes away a retracted number, and the comparator-lost-in-`/tmp` incident proves untracked artifacts become permanently void.*

**Explicitly do NOT do next:** enable tail recovery by flipping `TailRecoveryConfig::default()` — enable it by calling `set_tail_recovery_config()` with the validated config (Q6); re-run any FS-1/R2A/product-tail grid, start R2B, start the Quinn/TCP/KCP comparison (the baseline is contaminated and must be re-run pinned first), or resume the DR-CS-3 neutrality chain (the challenge phenotype is destroyed and may not be re-keyed).