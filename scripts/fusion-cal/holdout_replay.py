#!/usr/bin/env python3
"""FS-FUSION-CAL-1 Phase G — grouped decision-replay holdout.

Executes reports/fs-fusion-cal-1/freeze.json EXACTLY (frozen before this ran).

Split: GROUP-DISJOINT BY RTT STRATUM. TRAIN = cells at RTT 50/250 ms, HOLDOUT = cells at RTT 1000 ms.
No cell / seed lineage / pairing group appears on both sides. The loss boundary is RE-DERIVED from
TRAIN cells only, then evaluated on HOLDOUT cells that were never inspected during derivation.

Pre-specified acceptance (asymmetric, safety-grounded): ZERO harmful false-positive FEC recommendations
on held-out cells. False negatives (conservative misses) are permitted and reported.
"""
import csv, json, os, re

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "reports", "fs-fusion-cal-1")

# Mirrors fusion_policy.rs FusionPolicy::loss_high_permille (integer ramp, no rounding-up).
def loss_high_permille(loss_ppm, start, full):
    if loss_ppm >= full:
        return 1000
    if loss_ppm <= start or full <= start:
        return 0
    return ((loss_ppm - start) * 1000) // (full - start)


def cell_rtt_ms(domain, cell_id):
    """Configured RTT of a cell, parsed from its id (offline analysis label — never a runtime input)."""
    m = re.search(r"rtt(\d+)", cell_id)
    if m:
        return int(m.group(1))
    if domain == "PRODUCT_NETNS":
        # product cell ids look like 'struct/loss/rtt' with rtt in {low, med, high}
        tail = cell_id.rsplit("/", 1)[-1]
        return {"low": 50, "med": 250, "high": 1000}.get(tail)
    return None


def cell_loss_ppm(domain, cell_id, rows):
    """Configured loss intensity of a cell, expressed in ppm of source symbols.

    In-process cells: 'exactdN' / '-dN' = N erased of 48 source units; 'iidNN' = NN % per symbol.
    Product cells: 'struct/lossLabel/rtt' where terminal-tail lossLabel maps to a contiguous drop
    count {2:1, 5:2, 10:3, 20:4} of 48 units, and iid/burst lossLabel is a direct percentage.
    """
    m = re.search(r"iid(\d+)", cell_id)
    if m:
        return int(m.group(1)) * 10_000
    m = re.search(r"(?:exactd|-d)(\d+)", cell_id)
    if m:
        return int(m.group(1)) * 1_000_000 // 48
    if domain == "PRODUCT_NETNS":
        parts = cell_id.split("/")
        if len(parts) == 3:
            struct, loss_label = parts[0], parts[1]
            try:
                pct = int(loss_label)
            except ValueError:
                return None
            if struct == "terminal-tail":
                drops = {2: 1, 5: 2, 10: 3, 20: 4}.get(pct)
                return drops * 1_000_000 // 48 if drops else None
            return pct * 10_000
    return None


def main():
    regions = list(csv.DictReader(open(os.path.join(OUT, "decision-regions.csv"))))
    freeze = json.load(open(os.path.join(OUT, "freeze.json")))

    enriched = []
    for r in regions:
        rtt = cell_rtt_ms(r["domain"], r["cell_id"])
        lppm = cell_loss_ppm(r["domain"], r["cell_id"], regions)
        enriched.append({**r, "rtt_ms": rtt, "loss_ppm": lppm})

    train = [r for r in enriched if r["rtt_ms"] in (50, 250) and r["loss_ppm"] is not None]
    hold = [r for r in enriched if r["rtt_ms"] == 1000 and r["loss_ppm"] is not None]
    unusable = [r for r in enriched if r["loss_ppm"] is None or r["rtt_ms"] is None]

    # ── re-derive the boundary from TRAIN ONLY ───────────────────────────────
    arq_tr = [r["loss_ppm"] for r in train if r["preferred_action"] in ("ARQ_TAIL", "ARQ_FROZEN")]
    fec_tr = [r["loss_ppm"] for r in train if r["preferred_action"] == "FEC_R2"]
    if fec_tr:
        hi_arq = max(arq_tr) if arq_tr else 0
        lo_fec = min(fec_tr)
        # boundary placed in the observed evidence gap (same construction as calibration-v1)
        start = hi_arq if hi_arq < lo_fec else max(0, lo_fec - 1)
        full = lo_fec
    else:
        start, full = None, None

    results, fp, fn, match, abstain = [], 0, 0, 0, 0
    for r in hold:
        pref = r["preferred_action"]
        rec = "ArqOnly"
        if start is not None:
            deg = loss_high_permille(r["loss_ppm"], start, full)
            rec = "FecAssist" if deg > 0 else "ArqOnly"
        if pref == "":
            outcome = "NO_PARETO_PREFERENCE(abstain)"
            abstain += 1
        elif pref == "FEC_R2":
            if rec == "FecAssist":
                outcome, = ("MATCH_FEC",); match += 1
            else:
                outcome = "FALSE_NEGATIVE(conservative)"; fn += 1
        else:  # ARQ-family preferred
            if rec == "FecAssist":
                outcome = "HARMFUL_FALSE_POSITIVE"; fp += 1
            else:
                outcome = "MATCH_ARQ"; match += 1
        results.append({
            "domain": r["domain"], "cell_id": r["cell_id"], "rtt_ms": r["rtt_ms"],
            "loss_ppm": r["loss_ppm"], "pareto_preferred": pref or "NONE",
            "policy_recommendation": rec, "outcome": outcome,
            "completion": r["completion"], "mean_wire": r["mean_wire"],
        })

    cols = list(results[0].keys())
    with open(os.path.join(OUT, "holdout-policy-results.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=cols); w.writeheader(); w.writerows(results)

    summary = {
        "freeze_id": freeze["freeze_id"],
        "split": "GROUP-DISJOINT BY RTT: TRAIN={50,250} ms, HOLDOUT={1000} ms",
        "train_cells": len(train), "holdout_cells": len(hold),
        "cells_without_derivable_loss_or_rtt": len(unusable),
        "boundary_derived_from_train_only": {"high_start_ppm": start, "high_full_ppm": full},
        "calibration_v1_boundary_for_comparison": {"high_start_ppm": 160000, "high_full_ppm": 200000},
        "confusion": {
            "match": match,
            "harmful_false_positive": fp,
            "false_negative_conservative": fn,
            "abstain_no_pareto_preference": abstain,
        },
        "primary_criterion_zero_harmful_false_positive": fp == 0,
        "verdict_input": ("PASS (zero harmful false positives)" if fp == 0
                          else f"FAIL ({fp} harmful false positives)"),
    }
    with open(os.path.join(OUT, "holdout-cell-results.json"), "w") as f:
        json.dump({"summary": summary, "cells": results}, f, indent=2, sort_keys=True)
    print(json.dumps(summary, indent=2))
    print("\nheld-out cells with a Pareto preference:")
    for r in results:
        if r["pareto_preferred"] != "NONE":
            print(f"  {r['domain'][:12]:12} {r['cell_id']:22} loss={r['loss_ppm']:>7} "
                  f"pref={r['pareto_preferred']:10} rec={r['policy_recommendation']:10} {r['outcome']}")


if __name__ == "__main__":
    main()
