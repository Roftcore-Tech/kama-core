# DR-CS-3-TWO-NETNS

**Status: `NOT RUN`**

## What this artifact would have covered
§7 targeted two-netns component validation over real UDP

## Why it was not run
§4 locked panel FAIL. No network campaign was started; machine-safety constraints (single Cargo process, -j2) were respected throughout.

## What exists instead
- `DR-CS-3-PANEL-PREFLIGHT` — §1 conformance gate, **PASS** (10/10 default, 12/12 feature build).
- `DR-CS-3-LOCKED-PANEL` — §4 locked regression panel, **FAIL** on the efficacy gate (2/3), every
  safety/accounting gate PASS, with the exact failure lineage for rep14.
- `DR-CS-3-DEFAULT-DECISION` — the §8 FAIL branch.

No data for this artifact was generated, estimated, extrapolated or fabricated. The harness mode
exists in `examples/dr_cs3_panel_runner` and can be run unchanged once the locked-panel gate is
resolved by an explicit decision.
