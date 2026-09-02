#!/usr/bin/env python3
"""FS-FUSION-CAL-1 Phase D — per-cell Pareto action-region analysis.

Implements reports/fs-fusion-cal-1/analysis-method.json EXACTLY (frozen before this ran):
lexicographic endpoints (safety -> horizon completion -> realized wire -> RMST -> conditional latency),
domain-separated, pairwise within-domain, McNemar exact on paired discordance, grouped bootstrap on cost,
no scalar utility, no invented non-inferiority margin, never forces a winner.

Outputs decision-regions.csv + training-results.json. Deterministic (fixed bootstrap seed).
"""
import csv, json, os, math, random
from collections import defaultdict

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "reports", "fs-fusion-cal-1")
ALPHA = 0.05
BOOT_N = 10000
BOOT_SEED = 20260804

ARQ_FAMILY = {"ARQ_FROZEN", "ARQ_TAIL"}
FEC_FAMILY = {"FEC_R2", "FEC_R4"}
DUP_FAMILY = {"BLIND_DUP"}


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def binom_two_sided_p(b, c):
    """McNemar exact: two-sided binomial test on discordant pairs (b, c) with p=0.5."""
    n = b + c
    if n == 0:
        return 1.0
    def pmf(k):
        return math.comb(n, k) * (0.5 ** n)
    obs = pmf(min(b, c))
    # sum all outcomes at most as likely as the observed one (exact two-sided)
    return min(1.0, sum(pmf(k) for k in range(n + 1) if pmf(k) <= obs + 1e-15))


def boot_mean_diff(pairs, rng):
    """Grouped bootstrap of mean(x - y) over paired groups. Returns (mean, lo, hi)."""
    if not pairs:
        return (float("nan"), float("nan"), float("nan"))
    diffs = [a - b for a, b in pairs]
    m = sum(diffs) / len(diffs)
    n = len(diffs)
    means = []
    for _ in range(BOOT_N):
        s = 0.0
        for _ in range(n):
            s += diffs[rng.randrange(n)]
        means.append(s / n)
    means.sort()
    return (m, means[int(0.025 * BOOT_N)], means[int(0.975 * BOOT_N) - 1])


def rmst(rows, horizon_ns):
    """Restricted mean survival-style time-to-completion, restricted to the horizon.
    Censored rows contribute the full horizon. Lower is better (faster completion)."""
    if not rows:
        return float("nan")
    tot = 0.0
    for r in rows:
        if r["horizon_complete"] == 1 and r["completion_time_ns"] != "":
            tot += min(float(r["completion_time_ns"]), horizon_ns)
        else:
            tot += horizon_ns
    return tot / len(rows)


def load():
    rows = []
    with open(os.path.join(OUT, "canonical-calibration-data.csv")) as f:
        for r in csv.DictReader(f):
            r["horizon_complete"] = int(r["horizon_complete"])
            r["invariant_ok"] = int(r["invariant_ok"]) if r["invariant_ok"] != "" else 1
            rows.append(r)
    return rows


def family(action):
    if action in ARQ_FAMILY:
        return "ARQ"
    if action in FEC_FAMILY:
        return "FEC"
    if action in DUP_FAMILY:
        return "DUP"
    return "OTHER"


def main():
    rows = load()
    rng = random.Random(BOOT_SEED)

    # group: (domain, cell_id) -> action -> {pairing_group_id -> row}
    cells = defaultdict(lambda: defaultdict(dict))
    for r in rows:
        cells[(r["domain"], r["cell_id"])][r["action"]][r["pairing_group_id"]] = r

    out_rows = []
    contrast_count = 0
    for (domain, cell), by_action in sorted(cells.items()):
        actions = sorted(by_action)
        horizon_ns = 0
        for a in actions:
            for r in by_action[a].values():
                if r["horizon_ms"]:
                    horizon_ns = max(horizon_ns, int(r["horizon_ms"]) * 1_000_000)

        # rank 1 — safety elimination
        safe_actions, rejected = [], []
        for a in actions:
            rs = list(by_action[a].values())
            if all(r["invariant_ok"] == 1 for r in rs):
                safe_actions.append(a)
            else:
                rejected.append(a)

        stats = {}
        for a in safe_actions:
            rs = list(by_action[a].values())
            n = len(rs)
            k = sum(r["horizon_complete"] for r in rs)
            lo, hi = wilson(k, n)
            wire = [float(r["total_wire_bytes"]) for r in rs if r["total_wire_bytes"] != ""]
            comp_t = [float(r["completion_time_ns"]) / 1e6 for r in rs
                      if r["horizon_complete"] == 1 and r["completion_time_ns"] != ""]
            comp_t.sort()
            stats[a] = {
                "n": n, "complete": k, "rate": k / n if n else 0.0,
                "wilson_lo": round(lo, 4), "wilson_hi": round(hi, 4),
                "mean_wire": round(sum(wire) / len(wire), 1) if wire else None,
                "median_lat_ms": round(comp_t[len(comp_t) // 2], 1) if comp_t else None,
                "rmst_ms": round(rmst(rs, horizon_ns) / 1e6, 1) if horizon_ns else None,
            }

        # pairwise contrasts within the same domain
        winners, contrasts = [], []
        for i in range(len(safe_actions)):
            for j in range(i + 1, len(safe_actions)):
                a, b = safe_actions[i], safe_actions[j]
                common = sorted(set(by_action[a]) & set(by_action[b]))
                if not common:
                    continue
                contrast_count += 1
                ra = [by_action[a][g] for g in common]
                rb = [by_action[b][g] for g in common]
                # rank 2 — completion (McNemar exact on discordant pairs)
                a_only = sum(1 for x, y in zip(ra, rb) if x["horizon_complete"] == 1 and y["horizon_complete"] == 0)
                b_only = sum(1 for x, y in zip(ra, rb) if x["horizon_complete"] == 0 and y["horizon_complete"] == 1)
                p = binom_two_sided_p(a_only, b_only)
                verdict, basis = None, None
                completion_identical = (a_only == 0 and b_only == 0)
                underpowered = (p >= ALPHA) and not completion_identical
                comp_dir = 0 if a_only == b_only else (1 if a_only > b_only else -1)  # +1: a better
                if p < ALPHA and a_only != b_only:
                    verdict = a if a_only > b_only else b
                    basis = "completion"
                else:
                    # rank 3 — realized wire (grouped bootstrap of paired difference)
                    pairs = [(float(x["total_wire_bytes"]), float(y["total_wire_bytes"]))
                             for x, y in zip(ra, rb)
                             if x["total_wire_bytes"] != "" and y["total_wire_bytes"] != ""]
                    m, lo, hi = boot_mean_diff(pairs, rng)
                    if pairs and not math.isnan(m) and (lo > 0 or hi < 0):
                        cheaper = b if m > 0 else a   # positive m => a costs more => b cheaper
                        # amendment v1.1 defect-1: a cost winner may not contradict an (underpowered)
                        # numeric completion direction; if it does, this is a genuine trade-off.
                        contradicts = underpowered and comp_dir != 0 and (
                            (comp_dir > 0 and cheaper == b) or (comp_dir < 0 and cheaper == a))
                        if contradicts:
                            verdict, basis = None, "tradeoff_completion_vs_cost"
                        else:
                            verdict, basis = cheaper, ("wire_cost_equal_completion"
                                                       if completion_identical else "wire_cost")
                    else:
                        # rank 4 — RMST
                        ra_r = rmst(ra, horizon_ns)
                        rb_r = rmst(rb, horizon_ns)
                        if horizon_ns and abs(ra_r - rb_r) > 0.02 * horizon_ns:
                            verdict = a if ra_r < rb_r else b
                            basis = "rmst"
                        else:
                            verdict, basis = None, "indistinguishable"
                contrasts.append({"a": a, "b": b, "n_pairs": len(common), "a_only": a_only,
                                  "b_only": b_only, "mcnemar_p": round(p, 5),
                                  "winner": verdict, "basis": basis,
                                  "completion_underpowered": underpowered})
                if verdict:
                    winners.append((verdict, basis))

        # cell label
        preferred = ""
        if not safe_actions:
            label, note = "SAFETY_REJECTED", f"all actions had invariant failures: {rejected}"
        elif len(safe_actions) == 1:
            label, note = "INSUFFICIENT_EVIDENCE", f"only one action present ({safe_actions[0]}); no contrast"
        else:
            wset = {w for w, _ in winners}
            if not wset:
                label, note = "INSUFFICIENT_EVIDENCE", "no contrast reached a decision at any rank"
            else:
                # an action is dominant if it wins every contrast it appears in and loses none
                losers = set()
                for c in contrasts:
                    if c["winner"]:
                        losers.add(c["b"] if c["winner"] == c["a"] else c["a"])
                undominated = [a for a in safe_actions if a not in losers]
                if len(undominated) == 1:
                    win = undominated[0]
                    preferred = win
                    fam = family(win)
                    label = {"ARQ": "ARQ_DOMINANT", "FEC": "FEC_DOMINANT",
                             "DUP": "NON_DOMINATED_TRADEOFF"}.get(fam, "NON_DOMINATED_TRADEOFF")
                    note = f"undominated={win}"
                else:
                    label = "NON_DOMINATED_TRADEOFF"
                    note = f"undominated={undominated}"

        out_rows.append({
            "domain": domain, "cell_id": cell, "label": label, "preferred_action": preferred, "note": note,
            "actions": "|".join(safe_actions), "safety_rejected": "|".join(rejected),
            "n_per_action": json.dumps({a: stats[a]["n"] for a in safe_actions}),
            "completion": json.dumps({a: f"{stats[a]['complete']}/{stats[a]['n']}" for a in safe_actions}),
            "completion_rate": json.dumps({a: round(stats[a]["rate"], 3) for a in safe_actions}),
            "wilson95": json.dumps({a: [stats[a]["wilson_lo"], stats[a]["wilson_hi"]] for a in safe_actions}),
            "mean_wire": json.dumps({a: stats[a]["mean_wire"] for a in safe_actions}),
            "rmst_ms": json.dumps({a: stats[a]["rmst_ms"] for a in safe_actions}),
            "median_lat_ms_conditional": json.dumps({a: stats[a]["median_lat_ms"] for a in safe_actions}),
            "contrasts": json.dumps(contrasts),
        })

    cols = list(out_rows[0].keys())
    p = os.path.join(OUT, "decision-regions.csv")
    with open(p, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=cols)
        w.writeheader(); w.writerows(out_rows)

    from collections import Counter
    lab = Counter((r["domain"], r["label"]) for r in out_rows)
    pref = Counter((r["domain"], r["preferred_action"] or "NONE") for r in out_rows)
    summary = {
        "method": "FS-FUSION-CAL-1-ANALYSIS-METHOD-v1",
        "cells_analyzed": len(out_rows),
        "pairwise_contrasts": contrast_count,
        "labels_by_domain": {f"{d}|{l}": c for (d, l), c in sorted(lab.items())},
        "preferred_action_by_domain": {f"{d}|{a}": c for (d, a), c in sorted(pref.items())},
        "bootstrap": {"resamples": BOOT_N, "seed": BOOT_SEED, "unit": "pairing_group"},
        "alpha": ALPHA,
        "fusion_coverage": "NO authoritative arm combines tail-capable ARQ with proactive FEC; FUSION_DOMINANT is unreachable from existing data.",
    }
    with open(os.path.join(OUT, "training-results.json"), "w") as f:
        json.dump(summary, f, indent=2, sort_keys=True)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
