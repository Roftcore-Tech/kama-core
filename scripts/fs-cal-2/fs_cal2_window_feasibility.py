#!/usr/bin/env python3
"""FS-CAL-2 v3.2 — OUTCOME-BLIND estimator-window feasibility table for W ∈ {64,128,192,256}.

Inputs (both outcome-blind):
  * reports/fs-cal-2/v3.1/window-feasibility/S0-WINDOW-FEATURES.csv
      the IDENTICAL frozen S0 synthetic traces (seed domain FSFC2-S0), with the pure
      `features_from_window` core recomputed over the last W fate-known bits.
  * reports/fs-cal-2/v3.1/diag-cs/{d1,d2}/*.json
      the FSFC2-DIAG-CS-V32 liveness diagnostic fate-known TIME SERIES (estimator telemetry only).

This script consumes NO calibration outcome, NO R0/R2 completion difference, and NO treatment
effect of any kind. It reports discrimination / bias / coverage / cost / timing per candidate W.

SELECTING a W is NOT done here: any change to the pinned window requires a NEW design version,
re-frozen BEFORE data, plus a fresh S0 and a fresh §4.2b re-qualification (design §3.4/§4.2b).
"""
import csv, glob, json, math, os, random, statistics, sys, hashlib

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
FEAT = os.path.join(REPO, "reports", "fs-cal-2", "v3.1", "window-feasibility", "S0-WINDOW-FEATURES.csv")
DIAG = os.path.join(REPO, "reports", "fs-cal-2", "v3.1", "diag-cs")
OUT = os.path.join(REPO, "reports", "fs-cal-2", "v3.1", "window-feasibility", "WINDOW-FEASIBILITY.json")

BOOT_B = 2000
SEED = 20260805
# design-pinned v3 thresholds (for W=256). Reported two ways for candidate W.
PINNED = {"loss_tr": 8, "nonloss_tr": 32}


def auc_point(pos, neg):
    if not pos or not neg:
        return None
    import bisect
    spos = sorted(pos)
    wins = ties = 0
    for b in neg:
        lo = bisect.bisect_left(spos, b)
        hi = bisect.bisect_right(spos, b)
        wins += len(spos) - hi
        ties += hi - lo
    return (wins + ties / 2) / (len(pos) * len(neg))


def auc_boot_lo(pos, neg, rng):
    base = auc_point(pos, neg)
    if base is None:
        return None, None
    vals = []
    for _ in range(BOOT_B):
        rp = [pos[rng.randrange(len(pos))] for _ in range(len(pos))]
        rn = [neg[rng.randrange(len(neg))] for _ in range(len(neg))]
        vals.append(auc_point(rp, rn))
    vals.sort()
    return base, vals[int(0.05 * BOOT_B)]


def load_features():
    rows = list(csv.DictReader(open(FEAT)))
    for r in rows:
        for k in ("p_ppm", "window", "n", "lost", "measured_loss_ppm", "burst_delta_ppm",
                  "loss_transitions", "nonloss_transitions"):
            r[k] = int(r[k])
    return rows


def fate_known_timing():
    """From the FSFC2-DIAG-CS-V32 sample series: virtual time at which fate_known first reaches W.
    Pure estimator telemetry (no outcome)."""
    out = {}
    files = [f for f in glob.glob(os.path.join(DIAG, "*", "*.json")) if not f.endswith(".sha256")]
    per_w = {64: [], 128: [], 192: [], 256: []}
    reached = {64: 0, 128: 0, 192: 0, 256: 0}
    total = 0
    for p in files:
        # the invariance control re-runs the frozen budget; exclude it from timing statistics
        if "invariance-control" in p:
            continue
        d = json.load(open(p))
        s = d.get("samples") or []
        if not s:
            continue
        total += 1
        seen = set()
        for row in s:
            t_ns, fk = row[0], row[7]
            for w in per_w:
                if w not in seen and fk >= w:
                    seen.add(w)
                    per_w[w].append(t_ns)
                    reached[w] += 1
    for w in per_w:
        v = sorted(per_w[w])
        out[str(w)] = {
            "runs_considered": total,
            "runs_reaching_w": reached[w],
            "fraction_reaching_w": round(reached[w] / total, 4) if total else 0.0,
            "time_to_w_ms_median": round(statistics.median(v) / 1e6, 1) if v else None,
            "time_to_w_ms_min": round(min(v) / 1e6, 1) if v else None,
            "time_to_w_ms_max": round(max(v) / 1e6, 1) if v else None,
        }
    return out


def main():
    rows = load_features()
    windows = sorted({r["window"] for r in rows})
    rng = random.Random(SEED)
    by = {}
    for r in rows:
        by.setdefault((r["window"], r["structure"], r["p_ppm"]), []).append(r)

    table = {}
    for w in windows:
        entry = {"window": w}
        # ── discrimination (AUC, bootstrap 95% lower bound) at p ∈ {10,20,30}% ──
        disc = {}
        for p in (100_000, 200_000, 300_000):
            iid = [x["burst_delta_ppm"] for x in by.get((w, "IID", p), []) if x["split"] == "train"]
            for ge in ("GE-L3", "GE-L10"):
                gev = [x["burst_delta_ppm"] for x in by.get((w, ge, p), []) if x["split"] == "train"]
                pt, lo = auc_boot_lo(gev, iid, rng)
                disc[f"IID_vs_{ge}_p{p//10000}"] = {
                    "auc": None if pt is None else round(pt, 4),
                    "auc_lo95": None if lo is None else round(lo, 4),
                    "meets_0.90_bar": bool(lo is not None and lo >= 0.90),
                }
        entry["discrimination"] = disc
        entry["discrimination_min_lo95"] = min(
            [v["auc_lo95"] for v in disc.values() if v["auc_lo95"] is not None], default=None
        )
        entry["all_auc_bars_met"] = all(v["meets_0.90_bar"] for v in disc.values())

        # ── burst-delta bias + variance vs configured b (MARKOV-BD ground truth) ──
        bias = {}
        for s, b_true in (("MARKOV-BD-b15", 150_000), ("MARKOV-BD-b35", 350_000)):
            for p in (100_000, 200_000, 300_000):
                v = [x["burst_delta_ppm"] for x in by.get((w, s, p), [])]
                if not v:
                    continue
                med = statistics.median(v)
                bias[f"{s}_p{p//10000}"] = {
                    "median_burst_delta_ppm": med,
                    "bias_ppm": med - b_true,
                    "abs_bias_within_50000": abs(med - b_true) <= 50_000,
                    "stdev_ppm": round(statistics.pstdev(v), 1),
                    "iqr_ppm": round(
                        statistics.quantiles(v, n=4)[2] - statistics.quantiles(v, n=4)[0], 1
                    ) if len(v) >= 4 else None,
                }
        entry["burst_delta_bias_variance"] = bias
        entry["max_abs_bias_ppm"] = max((abs(x["bias_ppm"]) for x in bias.values()), default=None)
        entry["mean_stdev_ppm"] = round(
            statistics.mean([x["stdev_ppm"] for x in bias.values()]), 1
        ) if bias else None
        entry["all_bias_within_50000"] = all(x["abs_bias_within_50000"] for x in bias.values())

        # ── transition-threshold coverage, reported BOTH ways ──
        scale = w / 256
        scaled = {"loss_tr": max(1, round(PINNED["loss_tr"] * scale)),
                  "nonloss_tr": max(1, round(PINNED["nonloss_tr"] * scale))}
        cov = {}
        for s in ("IID", "MARKOV-BD-b15", "MARKOV-BD-b35", "GE-L3", "GE-L10"):
            for p in (50_000, 100_000, 200_000, 300_000):
                v = by.get((w, s, p), [])
                if not v:
                    continue
                pin = sum(1 for x in v if x["n"] == w
                          and x["loss_transitions"] >= PINNED["loss_tr"]
                          and x["nonloss_transitions"] >= PINNED["nonloss_tr"])
                sc = sum(1 for x in v if x["n"] == w
                         and x["loss_transitions"] >= scaled["loss_tr"]
                         and x["nonloss_transitions"] >= scaled["nonloss_tr"])
                cov[f"{s}_p{p//10000}"] = {
                    "n": len(v),
                    "pinned_thresholds_frac": round(pin / len(v), 4),
                    "scaled_thresholds_frac": round(sc / len(v), 4),
                }
        entry["threshold_coverage"] = cov
        entry["scaled_thresholds"] = scaled
        entry["pinned_thresholds"] = PINNED
        mbd = [v["pinned_thresholds_frac"] for k, v in cov.items() if k.startswith("MARKOV-BD")
               and not k.endswith("_p5")]
        entry["markov_bd_min_coverage_pinned"] = min(mbd) if mbd else None

        # ── cost (analytic, from the estimator's own representation) ──
        entry["cost"] = {
            "window_state_bytes": w * (8 + 1),
            "note": "w × (u64 logical id + 1 byte fate). Counter update is O(1) per resolution; "
                    "snapshot is O(w). Cost is linear in w and negligible at every candidate.",
        }
        # ── prefix-causality (structural, identical for every w) ──
        entry["prefix_causal"] = True
        entry["prefix_causality_note"] = (
            "Unchanged by w: a fate is finalized only by ACK evidence or by loss-declaration after the "
            "reordering guard, and the window holds only already-finalized fates. Shrinking w removes "
            "OLD fates, never adds future information."
        )
        table[str(w)] = entry

    timing = fate_known_timing()
    for w in table:
        table[w]["fate_known_timing_observed"] = timing.get(w)

    verdict = {
        "analysis": "FS-CAL-2 v3.2 OUTCOME-BLIND estimator-window feasibility",
        "outcome_blind": True,
        "inputs_used": ["S0 synthetic fate-bit traces (frozen seed domain FSFC2-S0)",
                        "FSFC2-DIAG-CS-V32 fate-known time series (estimator telemetry only)"],
        "inputs_explicitly_NOT_used": ["calibration outcomes", "R0/R2 completion difference",
                                       "any treatment effect", "any holdout"],
        "bootstrap_B": BOOT_B, "seed": SEED,
        "windows": table,
        "selection_rule": (
            "NO window is selected here. Any change to the pinned window (256), warm-up length (384) or "
            "the eligibility thresholds requires a NEW design version frozen BEFORE data, plus a fresh S0 "
            "and a fresh §4.2b re-qualification run (design §3.4, §4.2b)."
        ),
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    body = json.dumps(verdict, indent=2, sort_keys=True)
    open(OUT, "w").write(body + "\n")
    open(OUT + ".sha256", "w").write(
        hashlib.sha256((body + "\n").encode()).hexdigest() + "  WINDOW-FEASIBILITY.json\n")

    # compact human table
    print(f"{'W':>5} {'minAUClo95':>11} {'allAUC>=.90':>12} {'maxBias(ppm)':>13} {'meanSD':>9} "
          f"{'MBDcov(pin)':>12} {'stateB':>7} {'reach_frac':>11} {'t_to_W_med(ms)':>15}")
    for w in sorted(table, key=int):
        e = table[w]
        tm = e.get("fate_known_timing_observed") or {}
        print(f"{w:>5} {str(e['discrimination_min_lo95']):>11} {str(e['all_auc_bars_met']):>12} "
              f"{str(e['max_abs_bias_ppm']):>13} {str(e['mean_stdev_ppm']):>9} "
              f"{str(e['markov_bd_min_coverage_pinned']):>12} {e['cost']['window_state_bytes']:>7} "
              f"{str(tm.get('fraction_reaching_w')):>11} {str(tm.get('time_to_w_ms_median')):>15}")
    print("\nwritten:", OUT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
