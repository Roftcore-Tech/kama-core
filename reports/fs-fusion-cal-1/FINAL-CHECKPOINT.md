# FS-FUSION-CAL-1 — Living Checkpoint

Single living checkpoint for the fuzzy ARQ/FEC calibration program. Full detail: `FINAL-REPORT.md`.

## State

| Item | Value |
|---|---|
| Verdict | **CALIBRATION-NO-GO** (calibration-v1's FEC-activation region) |
| Default state | **OFF** — `FusionPolicy::disabled()`; production still runs `FuzzyConfig::fixture()` |
| Shadow/experimental activation | not enabled anywhere; opt-in only, versioned |
| Product impact | **none** — `sender.rs`/`arq.rs`/`loss.rs`/`rtt.rs`/`fusion.rs`/`tail_recovery.rs` byte-unchanged |
| Branch | `d1-sprint5c` |
| Commits | `1e8fe0ba` (Phases B–D) · `96278dbe` (Phase E) · `dbee6674` (Phases F–G) · `b4044aba` (handoff) |

## Phase status

| Phase | Status | Artifact |
|---|---|---|
| A engine/observability audit | DONE | this checkpoint + `FINAL-REPORT.md` §3/§4 |
| B canonical dataset | DONE — 10930/10930, 0 excluded | `canonical-calibration-data.csv` (`d3096454…`), `ingest-ledger.json` |
| C analysis method | DONE (frozen + amendment v1.1) | `analysis-method.json` |
| D decision regions | DONE — 77 cells, 242 contrasts | `decision-regions.csv`, `training-results.json` |
| E calibration + integration | DONE | `calibration-v1.json` (`0197445e…`), `crates/adaptive-transport/src/fusion_policy.rs` |
| F pre-holdout freeze | DONE | `freeze.json` |
| G holdout | DONE — **FAILED primary criterion** | `holdout-policy-results.csv`, `holdout-cell-results.json` |

## Why NO-GO (one paragraph)

The pre-registered, group-disjoint holdout produced **one harmful false positive**: `burst/20/high` at
200 000 ppm is Pareto-preferred `ARQ_TAIL`, but the policy recommended `FecAssist`. At the same observable
loss rate, IID structure wants FEC and burst structure wants ARQ — and loss burstiness/run-length is computed
nowhere in `adaptive-transport`, so the policy provably cannot separate them. The training split had already
signalled it (boundary collapsed to a degenerate `199 999 → 200 000` gap). Reported honestly; **not retuned**.

## Blocking prerequisites (in order)

1. **Prefix-causal runtime loss-structure estimator** (burstiness / run-length / gap statistics from the loss
   events that `loss.rs` already retains). Without it, no safe FEC activation is possible — this is the single
   blocking item.
2. **Product FEC actuator** — safe, feature-gated, bounded. Required for any `PRODUCT-VALIDATED` verdict;
   today FEC is POLICY-ONLY and the fusion ARQ output is an inert `==0` gate.
3. **Bandwidth-varying evidence** — R2's `bw HIGH` antecedent is unfalsifiable in the single tested regime.

## Resume

Same authorization continues; no new scientific GO required for the items below.

```bash
# reproduce the whole analysis chain (pure Python, no cargo, machine-safe)
python3 scripts/fusion-cal/build_canonical.py
python3 scripts/fusion-cal/analyze_regions.py
python3 scripts/fusion-cal/build_calibration.py
python3 scripts/fusion-cal/holdout_replay.py

# local Rust gate (CAPPED — never full-workspace/release locally)
CARGO_BUILD_JOBS=2 cargo test -p adaptive-transport --lib fusion_policy -j 2
```

Next agent: do **not** rerun FS-1 / R2A / product-tail grids; do **not** retune against this holdout; do
**not** enable the policy by default. Start at prerequisite 1 (loss-structure estimator), then re-calibrate
and run a **new** disjoint-seed holdout.

## Machine safety (permanent)

Local: `CARGO_BUILD_JOBS=2`, one cargo process, single target, debug. Full-workspace `--all-features`,
`--release --all`, miri, deny, audit → hosted CI only. Never change earlyoom/systemd/OS settings.
