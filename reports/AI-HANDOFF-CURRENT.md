# Kama Transport Research — Current AI Handoff

> This is the canonical current-state handoff. Later corrigenda override earlier reports. Do not rerun
> completed datasets or revive superseded claims without explicit authorization.

Built from committed repo evidence (not chat memory). Source priority: newest corrigendum/amendment →
newest final checkpoint → locked final reports/artifacts → freeze/fidelity records → older interim
checkpoints/diagnostics → commit messages. Every scientific number below is cited to a committed artifact.

---

## A. Executive State

- **Current branch:** `d1-sprint5c` (base: `main`).
- **Document source HEAD:** `b0412708` (inspected range `520974e9..b0412708` + FS-1 `bd3a5ea8` artifacts).
- **Latest relevant result commit:** `b0412708` (two-netns product-validation results + GO/NO-GO checkpoint).
- **Hosted-CI status:** GREEN at `8c4b0f11`, `c46e8fa3`, and `b0412708` (quality = fmt/clippy-D/`test --all
  --all-features`/`build --release --all`; security = deny+audit; miri; aead-differential — all success).
  Provenance: checkpoint reports + freeze record `reports/dr-arq-product-tail-1/FIDELITY-AND-FREEZE.md`.
  *No CI was run this turn; this is the last recorded green state.*
- **Product-tail verdict:** **GO — MECHANISM CORRECT, POLICY/TUNING PENDING** (correctness/invariant/fidelity
  gates all pass; specific tuning identified, not a defect).
- **Default state:** the bounded tail probe ships **default-OFF** (`TailRecoveryConfig::default().enabled=false`);
  OFF-mode is byte-identical to pre-decision (INV-CS-4 preserved). ON is opt-in via frozen product config.
- **Completed major work:** FS-1 FEC-vs-ARQ screening (v1.3.2, in-process); DR-ARQ-TAIL-1 R1 + R2A
  deconfounding grid (in-process) + endpoint/budget corrigendum; DR-ARQ-PRODUCT-TAIL-1 product state machine +
  sender wiring + faithful two-netns/netem product validation (2290 rows, GO).
- **NOT authorized / not done:** Quinn/TCP/KCP comparison; default-ON flip; adaptive tuning (product-v2);
  R2B (NaturalBurst + probe-loss-eligible); any product-wide FEC policy; any external-protocol superiority claim.

---

## B. Current Status Matrix

| Workstream | Status | Authoritative artifact | Commit | Proves | Does NOT prove |
|---|---|---|---|---|---|
| FS-1 analysis | DONE (screening) | `reports/fs-1/analysis/v1.3.2/FS-1-ANALYSIS-REPORT-v1.3.2.md` | `bd3a5ea8` | in-process FEC-vs-ARQ screening; O2 FEC-specific ×12 | any program-final verdict; deadline success |
| DR-ARQ-TAIL-1 R1 | DONE (diagnostic) | `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R1-DIAGNOSTIC.md` | `3401f5c` | A0-TAIL recovers 5/5 terminal-tail configs A0 censors | anything statistical (5 configs) |
| DR-ARQ-TAIL-1 R2A | DONE (in-process) | `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md` | `7e3505f6` | C1/C2/C3 on 8400-row grid (horizon) | deadline success; faithful-network behavior |
| R2A corrigendum | AUTHORITATIVE | `.../DR-ARQ-TAIL-1-R2A-ENDPOINT-AND-BUDGET-CORRIGENDUM.md` | `a0d17f89` | completion=horizon (not deadline); C3 realized-wire relation | that F0/A1 realized wire is equal (it is NOT) |
| Product tail state machine | DONE | `crates/adaptive-transport/src/tail_recovery.rs` | `17246324` (+`8c4b0f11` diag) | pure deterministic bounded PTO probe; default-OFF | production tuning; default-ON readiness |
| Production sender wiring | DONE | `crates/adaptive-transport/src/sender.rs` (§5.2) | `21811df7` | input-map + action-apply only; OFF byte-identical | that ON is enabled by default |
| Two-netns product validation | DONE → GO | `reports/dr-arq-product-tail-1/v1/FINAL-CHECKPOINT.md` | `b0412708` | correctness/bounded/faithful; OFF stuck vs ON recovers | deadline SLO; kernel/NIC; product-wide policy |
| Fuzzy ARQ/FEC calibration (FS-FUSION-CAL-1) | DONE → **NO-GO** | `reports/fs-fusion-cal-1/FINAL-REPORT.md` | `dbee6674` | shipped config is uncalibrated; per-cell Pareto regions; safe default-OFF policy architecture; **loss rate alone cannot gate FEC** | any FEC activation threshold; any default-ON |
| Quinn/TCP/KCP comparison | NOT STARTED | — | — | — | (no result exists) |
| White paper | NOT STARTED (this program) | — | — | — | — |
| Product-v2 tuning | NOT STARTED | — | — | — | — |
| R2B burst/probe-loss | DEFERRED | `.../DR-ARQ-TAIL-1-R2A-SCOPE-AMENDMENT.md` | `b02d49ee` | scope split (pre-outcome) | any burst/probe-loss robustness result |

---

## C. Authoritative Scientific Findings

All numbers from committed artifacts. Keep the six separations below distinct — do not merge them.

**1. Coding/FEC vs blind duplication (C3 — strong).** In-process, matched **configured** intervention budget
(1212 B == 1212 B; frozen `coding_causal_fairness` NON_DOMINATED 21/21 cells): F0-only horizon-complete = **868**,
A1-EARLY-only = **0** (of 2100 paired rows). Independently reproduced in the disjoint DRARQTAIL1 seed domain,
consistent with FS-1 v1.3.2 oracle O2 (F0>A1 at 1212B, ×12 cells). *Cite:* R2A final `7e3505f6` §C3 + corrigendum
`a0d17f89` §2 + FS-1 `bd3a5ea8`. **Permitted:** "coding beats blind duplication at matched configured budget,
in-process, horizon endpoint." **Forbidden:** "coding beats duplication generally / on burst" (R2A explicitly not
generalized to burst → R2B); "at equal realized wire" (realized wire is F0-cheaper, not equal — see below).

**2. FEC vs frozen ACK-driven ARQ.** FS-1/R2A frozen ARQ (A0) is **tail-blind** (INV-CS-4: Pto core-path emits no
retransmit) → cannot recover a pure terminal tail. FEC (F0) recovers some. *Cite:* R2A `7e3505f6` §C1/§C2.
**Permitted:** "FEC recovers terminal tails the FROZEN ACK-driven ARQ cannot." **Forbidden:** "FEC > ARQ in
general" — the frozen ARQ is deliberately tail-blind; see finding 3.

**3. FEC vs tail-capable ARQ (C2 — the deconfounding payoff).** Over 900 S1 (terminal-tail) paired rows:
F0-only-complete = **26**, **A0-TAIL-only-complete = 64**, both = 810; and F0 costs **more** realized wire in
every S1 cell (budget UNEQUAL → Pareto/cost-effectiveness, NOT a causal "FEC superior"). Example s1-rtt50-d4:
A0-TAIL 100/100 @5224 B Pareto-dominates F0 83/100 @5767 B. *Cite:* R2A `7e3505f6` §C2. **Permitted:** "F0's FS-1
terminal-tail edge was substantially the frozen ARQ's tail blindness; a bounded tail probe (A0-TAIL, experiment-
only) closes most of it at lower wire cost, in-process." **Forbidden:** "tail-ARQ beats FEC" as a causal/general
claim (budget unequal; A0-TAIL is a deterministic deconfounder, not the product mechanism).

**4. Production tail-recovery OFF vs ON (faithful two-netns/netem — this program's headline).** Terminal-tail
structure, per-arm N=360 (12 cells × 30 reps): **OFF horizon-complete = 8/360 ≈ 2.2%**, **ON = 329/360 ≈ 91.4%**.
ON-only 27–30/30 in 11 of 12 cells; OFF-only = 0 in every cell. Cost bounded: +1–4 retransmitted units, probe_sent
1–3. *Cite:* `reports/dr-arq-product-tail-1/v1/{FINAL-CHECKPOINT.md,PAIRWISE.csv,CELL-RESULTS.csv}` `b0412708`.
**Permitted:** "the product bounded tail probe recovers terminal tails the default (OFF) product cannot, faithfully
over 50/250/1000 ms netns/netem, bounded and correct." **Forbidden:** default-enable readiness (see F/H).

**5. Deadline results vs horizon results (MUST stay separate).** R2A: under the frozen tight deadlines
(199/599/2099 ms) ALL arms are ~0 except A0-TAIL 46/2100; every R2A "completion" is **horizon** completion
(699/2499/9249 ms), NOT deadline. *Cite:* corrigendum `a0d17f89` §1. Product grid: deadline_hit (op) ≈ 0 across
arms; the effect is on horizon completion + conditional latency. **Forbidden:** presenting any horizon completion
as deadline/real-time success; presenting the 3×LB benchmark deadline as a product SLO (no product SLO exists).

**7. Fuzzy calibration (FS-FUSION-CAL-1, `dbee6674`).** Production runs the explicitly NON-calibrated
`FuzzyConfig::fixture()` via `Sender::new()` → `trusted_fixture()` (sender.rs:983-985), a path that also skips
`validate()`. That fixture classifies 200 000 ppm (20 % loss) as **fully LOW** → fires R1 (`fec_zero`), so it
never recommends FEC in the region where the data shows FEC Pareto-dominates (IID-20 %: 59–66/100 vs ARQ_TAIL
12–14/100, at lower wire). Per-cell Pareto regions over all 10 930 authoritative rows: R2A prefers ARQ_TAIL ×16 /
FEC_R2 ×4; PRODUCT prefers ARQ_TAIL ×27 / ARQ_FROZEN ×2. **A pre-registered group-disjoint holdout produced a
harmful false positive** (`burst/20/high`: same 200 000 ppm, but burst structure prefers ARQ while IID prefers
FEC) ⇒ **CALIBRATION-NO-GO**: loss rate alone cannot gate FEC, because loss burstiness/run-length is computed
**nowhere** in `adaptive-transport`. *Cite:* `reports/fs-fusion-cal-1/{FINAL-REPORT.md, freeze.json,
holdout-cell-results.json}`. **Forbidden:** any FEC activation threshold on loss rate; any default-ON.
Blocking prerequisite: a prefix-causal runtime loss-structure estimator. Product impact: NONE (default-OFF;
sender/arq/loss/rtt/fusion/tail_recovery byte-unchanged).

**6. In-process model vs faithful two-netns/netem.** FS-1 and R2A are **in-process deterministic** models
(`run_spine`, SpineArm). The DR-ARQ-PRODUCT-TAIL-1 validation is **faithful rootless two-netns/netem** driving the
real product `Sender` over UDP. **Forbidden:** attributing R2A's A0-TAIL/F0 numbers to the product mechanism, or
the product OFF/ON numbers to the in-process model. Terminal-tail in BOTH is a deterministic positional erasure
(not netem statistical loss); the product path applies real RTT (netem delay) and the probe crosses it.

---

## D. Product Tail-Recovery Implementation State

Verified file/symbol names (committed):

- `crates/adaptive-transport/src/tail_recovery.rs` — pure deterministic, RNG-free, overflow-safe decision/state
  machine. `pub struct TailRecoveryConfig { enabled: bool (default false), min_pto_ns, max_pto_ns, initial_pto_ns,
  max_backoff_pto_ns, max_ack_delay_ns, max_tail_probes }`; `pub struct TailRecovery`; `on_timer(now, srtt_opt,
  rttvar, oldest_inflight_seq) -> ProbeDecision`, `on_probe_sent(now)`, `on_ack_progress(now)`, `disarm()`.
  Base PTO = clamp(srtt + 4·rttvar + max_ack_delay, MIN, MAX); current = base·2^probe_index capped at
  max_backoff_pto; no RTT sample ⇒ initial_pto (Karn). Bounded by `max_tail_probes` per idle window.
- `crates/adaptive-transport/src/sender.rs` — input mapping + action application ONLY:
  `set_tail_recovery_config` (1146), `tail_probe_diag_ext` (1168, observation-only: due/sent/cancelled/bytes/
  spurious/timer_fires), `emit_tail_probe` (3372), `retire_inflight_to_tombstone` (3326). Timer::Pto hook passes
  live srtt/rttvar/oldest-inflight into `on_timer`; ACK hook resets only on REAL progress (inflight shrank),
  disarms when inflight empty; emission goes through the existing `try_send_pre_parity` token gate + `track_sent`
  (Retransmit) path — no wire-format change, no pacing bypass, no confirmed-loss declaration (INV-RATE-1).
- **Behavior:** probe-due ≠ probe-sent (attempt/cap spent only on real emission); target = OLDEST outstanding
  inflight source seq; ACK-race at emission → cancel (attempt not consumed); Karn inherent (probe is a Retransmit
  ⇒ excluded from RTT sampling; superseded original's tombstone retx bumped); reads NO FEC state (FEC-independent).
- **Architecture LOC (sender.rs +394 diff, §5.2):** 142 production / 252 `#[cfg(test)]` (§2 conformance PASS —
  no PTO formula, backoff, or probe-target policy in the sender).
- **Implementation commits:** TDD `b849c3ef` → §5.1 state machine `17246324` → §5.2 wiring `21811df7` → §7 diag
  `8c4b0f11`.

---

## E. Product Validation Evidence (faithful two-netns/netem)

- **Fidelity gates (§4 preflight A–H, PASS):** two distinct netns · veth placement (v0∈parent, v1∈child) ·
  connectivity · netem on BOTH egress + tc-dump expected · RTT+loss on-path (ping 200.36 ms / 10 %) · PTO probe
  traverses the impaired path (ON recovers, OFF stuck) · no-impairment control · teardown isolation. Rootless
  (`unshare -rnm` + child-netns veth-move); admin/docker NOT required. Fail-closed (netem unverified → no run;
  per-row netem retry, else `NETEM-SKIP` logged — never a silent drop).
- **Frozen identities:** source commit `c46e8fa3`; binary(debug) sha256 `a1113236…`; manifest sha256
  `bf849aba…`; analyzer sha256 `98d780e9…`; Cargo.lock `47893d24…`; toolchain rustc 1.96.0. *Cite:*
  `reports/dr-arq-product-tail-1/FIDELITY-AND-FREEZE.md` `76b62eca`.
- **Row accounting:** **2290/2290** = **2160 core** (3 struct × 4 loss × 3 RTT × 2 arms × N=30) + **130
  adversarial**. Missing 0 · extra 0 · duplicate 0 · NETEM-SKIP 0.
- **Terminal-tail per-arm denominators:** OFF 8/360, ON 329/360 (see finding 4). Two arms together = 720 rows.
- **Bounds/invariants (all rows):** global max `probe_sent` = 3 (= cap `max_tail_probes`); `inv_fail` = 0 across
  ALL 2290 rows; `spurious` probes = 0 across ALL adversarial slices.
- **Adversarial (§6):** reorder & delayed-ACK guards hold (probe_sent = 0, no spurious); jitter / pathpause /
  rtt-step-up / rtt-step-down → ON recovers terminal tail (probe_sent 1), OFF stuck; probe-loss slice (40 % fwd
  loss + terminal drop) held the retry cap (max probe_sent = 1 ≤ 3); no-loss → ON probe_sent = 0 and probe_bytes
  = 0 (mechanism inert). *Cite:* `reports/dr-arq-product-tail-1/v1/CELL-RESULTS.csv`.
- **Verdict:** GO — MECHANISM CORRECT, POLICY/TUNING PENDING.

---

## F. Known Limitations and Open Engineering Work

- **Multi-unit-tail fixed cap/backoff weakness.** `terminal-tail L10-med` (drop 3 units, RTT 250) is the only core
  terminal cell where ON fails (0/30): the fixed `max_tail_probes=3` is spent before packet-threshold ARQ engages,
  while L20-med (drop 4) completes because one probe's new higher seq becomes a loss-detection reference that
  unblocks normal ARQ (arq~3). Cap/backoff should ADAPT to tail length. (Correct + bounded throughout — not a defect.)
- **High-loss cold-start / PreConverge token starvation.** At 20–40 % loss a lost bootstrap ACK can strand the flow
  in PreConverge where floor pacing token-starves the probe. This is a DR-CS-1 (cold-start) concern; the tail probe
  targets a CONVERGED flow (the harness warms up first).
- **Default-ON not decided.** Correctness GO does not authorize enabling by default.
- **Adaptive policy/tuning not done** (product-v2 backlog).
- **R2B not done** (NaturalBurst + probe-loss-eligible + C4 probe robustness; needs additive ForwardImpairment
  plumbing).
- **External protocol comparison not done** (Quinn/TCP/KCP). No product-wide, kernel, or NIC superiority claim exists.
- **Deadline ≠ horizon.** Deadline performance must never be conflated with horizon completion (see finding 5).

---

## G. Superseded / Invalidated / Do-Not-Use

| Artifact / dataset / claim | State | Reason | Replacement |
|---|---|---|---|
| FS-1 v1.3.1 dataset (`reports/fs-1/raw/v1.3.1`) | SUPERSEDED-INCOMPLETE-SCHEMA (preserved) | RawRecord = 27 keys vs frozen 54-field schema; serializer non-conformance (not a scientific reason) | FS-1 v1.3.2 (schema-v2, `bd3a5ea8`) |
| First v1.3.2 dataset (git `44e0c50`) | INVALIDATED (preserved immutably) | `arm` label written from internal `SpineResult.arm`="A1" not canonical "A1-EARLY" → zeroed A1-EARLY in analysis | fixed re-run `2f24447` (v1.3.2 final) |
| R2A "completion" as deadline/real-time success | SUPERSEDED wording | completion field is horizon (699/2499/9249 ms), not deadline (199/599/2099 ms) | corrigendum `a0d17f89` §1 |
| R2A "matched/equal realized-wire budget 21/21" | RETRACTED | budget parity is CONFIGURED (1212 B); realized wire is F0-cheaper 18/21, equal 3/21 | corrigendum `a0d17f89` §2 |
| Ambiguous `OFF=8 vs ON=329 /720` terminal-tail phrasing | DO-NOT-USE | denominator is per-arm N=360, not 720 | OFF 8/360 (2.2 %), ON 329/360 (91.4 %) |
| Old FS-1 freeze-lock v1.3 (`5a9bc09d`) | INVALIDATED-BEFORE-FIRST-RESULT (preserved) | executor gap (result_bearing_runs=0) | re-freeze v1.3.1 → superseded by v1.3.2 |

---

## H. Claim Boundary Matrix

**SUPPORTED NOW**
- Tail recovery is correct and bounded (2290/2290 rows, max probe_sent=3, inv_fail=0, spurious=0, guards hold).
- Real (rootless netns/netem) faithful validation of the product Sender's tail probe was performed.
- FEC beats blind duplication at matched CONFIGURED budget, in-process, horizon endpoint (C3).

**SUPPORTED WITH QUALIFICATION**
- "FEC beats ARQ": only vs the FROZEN tail-blind ARQ (finding 2); vs a tail-capable ARQ, FEC is NOT dominant on the
  tail stratum and costs more wire (C2). In-process only.
- Product tail-recovery effectiveness: strong on terminal tails, but POLICY/TUNING PENDING (multi-unit cap;
  high-loss cold-start) and DEFAULT-OFF.

**NOT YET SUPPORTED**
- Tail recovery is ready to default-enable — NO.
- "FEC beats tail-capable ARQ generally" — NO (C2 is Pareto/cost, tail stratum only, in-process).
- "Kama beats Quinn/TCP/KCP" — NO comparison run.
- Deadline SLO success — NO (deadline_hit ≈ 0; no product SLO exists).
- Product-wide FEC policy / activation threshold — NO (loss rate × structure confounded; deferred).
- Burst-loss conclusion — NO (R2B deferred; R2A explicitly not generalized to burst).

---

## I. Exact Next Work Order (PLAN ONLY — do not execute unprompted)

1. If needed, a small docs corrigendum standardizing the per-arm terminal-tail denominators (8/360, 329/360).
2. Comparison harness arms: **KAMA-CURRENT/OFF**, **KAMA-TAIL-V1/ON**, **Quinn**, **TCP**, **KCP**.
3. Build the protocol adapters FIRST; derive the manifest from the executable harness; THEN freeze (pre-outcome).
4. Fair two-netns/netem run (identical workload/netem/seed lineage per arm; faithful RTT/loss).
5. Single comparison report (per-cell, paired; report KAMA OFF and ON TOGETHER; deadline vs horizon separate).
6. Integrate evidence into a white paper (cite artifacts, not chat).
7. Product-v2 tuning backlog (adaptive cap/backoff for multi-unit tails; cold-start interaction).
8. R2B backlog (NaturalBurst + probe-loss-eligible + C4).

For the next agent, explicitly:
- Do NOT rerun completed FS-1 / R2A / product grids.
- Do NOT post-hoc tune Tail-v1 before the comparison.
- Report KAMA OFF and ON results together (never ON alone).
- No scientific threshold may be chosen after seeing an outcome (pre-register).
- Engineering details (harness plumbing, adapters, schema) do NOT require intermediate approval.
- If fairness or fidelity cannot be guaranteed, STOP.

---

## J. Machine-Safety Rules (PERMANENT)

The user's machine froze multiple times during parallel Rust/LLVM release compiles (RAM+swap exhaustion).
- Local Cargo operations MUST use `CARGO_BUILD_JOBS=2` (or `-j 2`), single target, prefer debug.
- Only ONE Cargo process at a time locally.
- Full-workspace `test --all-features`, `build --release --all`, miri, deny, audit → **hosted CI ONLY**, never local.
- Do NOT change earlyoom / systemd / OS settings without the user's explicit approval.
- Treat committed hosted-CI checkpoints as the source of truth for heavy gates; never invent a PASS — cite provenance.

---

## K. Artifact and Commit Index

| Path | Role | Commit | SHA-256 | State |
|---|---|---|---|---|
| `crates/adaptive-transport/src/tail_recovery.rs` | product tail state machine | `17246324` (+`8c4b0f11`) | not independently recorded | current |
| `crates/adaptive-transport/src/sender.rs` | product wiring (§5.2) | `21811df7` (+`8c4b0f11`) | not independently recorded | current |
| `crates/adaptive-transport/examples/dr_arqtail_product.rs` | netns product driver | `c46e8fa3` | binary(debug) `a1113236…` | current |
| `scripts/netem/dr_arqtail_{preflight,manifest,run,launch,analyze}.*` | validation harness | `c46e8fa3` | analyzer `98d780e9…` | current |
| `docs/design/DR-ARQ-PRODUCT-TAIL-1-TDD.md` | TDD (INV-CS-4 supersession, deadline-feasibility) | `b849c3ef` | — | current |
| `reports/dr-arq-product-tail-1/FIDELITY-AND-FREEZE.md` | pre-run freeze record | `76b62eca` | source `c46e8fa3`, manifest `bf849aba…` | current |
| `reports/dr-arq-product-tail-1/manifest.csv` | frozen 2290-row manifest | `76b62eca` | `bf849aba…` | current |
| `reports/dr-arq-product-tail-1/v1/{result.csv,CELL-RESULTS.csv,PAIRWISE.csv,FINAL-REPORT.md,FINAL-CHECKPOINT.md}` | product-validation results + GO/NO-GO | `b0412708` | — | current |
| `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-FINAL-REPORT.md` | in-process R2A C1/C2/C3 | `7e3505f6` | — | current (read with corrigendum) |
| `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R2A-ENDPOINT-AND-BUDGET-CORRIGENDUM.md` | R2A endpoint/budget corrections | `a0d17f89` | — | current AUTHORITATIVE |
| `reports/dr-arq-tail-1/v1/DR-ARQ-TAIL-1-R1-DIAGNOSTIC.md` | R1 replay | `3401f5c` | — | current |
| `reports/fs-1/analysis/v1.3.2/FS-1-ANALYSIS-REPORT-v1.3.2.md` | FS-1 v1.3.2 screening | `bd3a5ea8` | — | current |
| `reports/fs-1/raw/v1.3.1` | FS-1 v1.3.1 dataset | (earlier) | ledger `a3e332bc` | SUPERSEDED-INCOMPLETE-SCHEMA |
| FS-1 first v1.3.2 dataset | arm-label bug | `44e0c50` | — | INVALIDATED (preserved) |

---

## L. Repository Working-Tree Boundary

`git status --short` at the start of this handoff turn (HEAD `b0412708`):
```text
 M reports/d1-exit/data/cold-start/timeline-1200B.csv
 M reports/d1-exit/data/cold-start/timeline-300B.csv
 M reports/d1-exit/data/cold-start/timeline-65B.csv
?? docs/design/D0-onay-revizyon-talimati.md
?? docs/design/pre-converge-v1.md
?? docs/tasks/ADIM-6-coldstart-gorev.md
?? docs/tasks/v2-benchmark-gorev-r5-kosuya-hazir.md
?? reports/d1-exit/data/dr-cs1-w2.log
?? reports/d1-exit/data/dr-cs1-wire.log
?? reports/d1-exit/data/netem-a2/console.txt
?? reports/d1-exit/data/w2b-r3.log
?? reports/d1-exit/data/w365-b1.log
```
These are the user's pre-existing uncommitted/WIP files (cold-start CSV data + design/task docs + run logs).
They are **NOT** part of this handoff work and were **not touched, staged, modified, or cleaned**. The tree is
**not clean** — only `reports/AI-HANDOFF-CURRENT.md` is added by this turn. Do not sweep these WIP files into any commit.
