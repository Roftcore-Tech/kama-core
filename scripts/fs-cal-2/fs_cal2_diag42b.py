#!/usr/bin/env python3
"""FS-CAL-2 v3 §4.2b — product-path eligibility pre-check verdict over the diag-42b estimator rows.

PASS (per cell P1/P2, separately): estimator_eligible fraction >= 90%, runtime_features_present == 100%,
schema/invariant failure == 0. These 80 estimator-only runs never enter training/holdout/fit and are not
result-bearing. Reads the diag-42b JSON records (DIAG_42B phase, estimator telemetry only).
"""
import glob, json, os, sys, hashlib

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
DIR = os.path.join(REPO, "reports", "fs-cal-2", "v3", "grounding", "diag-42b")
OUT = os.path.join(DIR, "DIAG-42B-VERDICT.json")


def main():
    files = [f for f in glob.glob(os.path.join(DIR, "*.json")) if not f.endswith(".sha256")]
    by_cell = {}
    for p in files:
        d = json.load(open(p))
        cell = d["cell_id"]
        by_cell.setdefault(cell, []).append(d)
    cells = {}
    overall = True
    for cell, recs in sorted(by_cell.items()):
        n = len(recs)
        elig = sum(1 for r in recs if r["estimator_eligible"])
        rfp = sum(1 for r in recs if r["runtime_features_present"] == 1)
        # schema failure proxy: missing telemetry keys; invariant: resolved<=256, counters consistent
        schema_fail = sum(
            1 for r in recs
            if not all(k in r for k in ("estimator_resolved", "measured_loss_ppm", "burst_delta_ppm"))
        )
        inv_fail = sum(1 for r in recs if r["estimator_resolved"] > 256)
        elig_frac = elig / n if n else 0.0
        rfp_frac = rfp / n if n else 0.0
        cell_pass = elig_frac >= 0.90 and rfp_frac == 1.0 and schema_fail == 0 and inv_fail == 0
        overall &= cell_pass
        cells[cell] = {
            "n": n, "eligible": elig, "eligible_fraction": round(elig_frac, 4),
            "runtime_features_present_fraction": round(rfp_frac, 4),
            "schema_failures": schema_fail, "invariant_failures": inv_fail,
            "pass": cell_pass,
        }
    verdict = {"phase": "DIAG_42B", "total_runs": sum(c["n"] for c in cells.values()),
               "cells": cells, "overall_pass": overall,
               "note": "estimator-only diagnostic; NOT result-bearing; not part of training/holdout/fit"}
    with open(OUT, "w") as f:
        json.dump(verdict, f, indent=2, sort_keys=True)
    with open(OUT + ".sha256", "w") as f:
        f.write(hashlib.sha256(open(OUT, "rb").read()).hexdigest() + "  DIAG-42B-VERDICT.json\n")
    print(json.dumps(verdict, indent=2))
    sys.exit(0 if overall else 1)


if __name__ == "__main__":
    main()
