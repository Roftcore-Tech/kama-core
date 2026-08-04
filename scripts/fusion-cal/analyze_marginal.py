#!/usr/bin/env python3
"""FS-FUSION-CAL-1 — CORRECTED analysis on the program's actual axis.

BINDING FRAMING (docs/design/dr-cs-1b-a2-receiver-design-v2.md:20; reports/d1-exit/
1b-fec-utility-boundary-observability.md:191; 1b-fuzzy-calibration-evidence.md:40):

    "ARQ-vs-FEC" language is FORBIDDEN. ARQ is ACTIVE IN EVERY ARM.
    r0 = ARQ + 0 proactive FEC ; r2/r4 = ARQ + 2/4 proactive parity.
    What is measured is the MARGINAL UTILITY of proactive redundancy on top of always-on ARQ.

Two ORTHOGONAL axes:
    axis-1 ARQ capability : frozen (tail-blind) | tail-capable
    axis-2 proactive redundancy r : 0 | 2 | 4        (blind duplication is a separate, non-coding
                                                      intervention and is reported apart)

Only SINGLE-AXIS contrasts are valid:
    (frozen,r0) vs (frozen,r2) -> marginal utility of proactive FEC        [VALID]
    (frozen,r2) vs (frozen,r4) -> marginal utility of MORE redundancy      [VALID]
    (frozen,r0) vs (tail,r0)   -> marginal utility of tail capability      [VALID]
    (tail,r0)   vs (frozen,r2) -> CHANGES BOTH AXES                        [INVALID — never computed]

Supersedes the ARQ_DOMINANT/FEC_DOMINANT labelling in decision-regions.csv, which used the forbidden
rival framing and produced an invalid cross-axis contrast. No experiment is re-run; same frozen rows.
"""
import csv, json, os, math, random
from collections import defaultdict

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "reports", "fs-fusion-cal-1")
ALPHA, BOOT_N, BOOT_SEED = 0.05, 10000, 20260804

# arm -> (arq_capability, redundancy_r, intervention_kind)
ARM_AXES = {
    "A0":        ("frozen", 0, "coding"),
    "A0-FROZEN": ("frozen", 0, "coding"),
    "off":       ("frozen", 0, "coding"),
    "A0-TAIL":   ("tail",   0, "coding"),
    "on":        ("tail",   0, "coding"),
    "F0":        ("frozen", 2, "coding"),
    "F3":        ("frozen", 4, "coding"),
    "A1-EARLY":  ("frozen", 0, "blind_duplication"),
}


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def binom_two_sided_p(b, c):
    n = b + c
    if n == 0:
        return 1.0
    pmf = lambda k: math.comb(n, k) * (0.5 ** n)
    obs = pmf(min(b, c))
    return min(1.0, sum(pmf(k) for k in range(n + 1) if pmf(k) <= obs + 1e-15))


def boot_diff(pairs, rng):
    if not pairs:
        return (float("nan"),) * 3
    d = [a - b for a, b in pairs]
    m, n = sum(d) / len(d), len(d)
    ms = sorted(sum(d[rng.randrange(n)] for _ in range(n)) / n for _ in range(BOOT_N))
    return (m, ms[int(0.025 * BOOT_N)], ms[int(0.975 * BOOT_N) - 1])


def main():
    rows = list(csv.DictReader(open(os.path.join(OUT, "canonical-calibration-data.csv"))))
    rng = random.Random(BOOT_SEED)
    # (domain, cell) -> (cap, r, kind) -> pairing_group -> row
    cells = defaultdict(lambda: defaultdict(dict))
    for r in rows:
        ax = ARM_AXES.get(r["arm_raw"])
        if ax:
            cells[(r["domain"], r["cell_id"])][ax][r["pairing_group_id"]] = r

    # single-axis contrasts only
    CONTRASTS = [
        ("marginal_proactive_fec_r0_to_r2", ("frozen", 0, "coding"), ("frozen", 2, "coding")),
        ("marginal_more_redundancy_r2_to_r4", ("frozen", 2, "coding"), ("frozen", 4, "coding")),
        ("marginal_tail_capability_at_r0", ("frozen", 0, "coding"), ("tail", 0, "coding")),
    ]

    out, summary = [], defaultdict(lambda: defaultdict(int))
    for (domain, cell), by_ax in sorted(cells.items()):
        for name, base_ax, alt_ax in CONTRASTS:
            if base_ax not in by_ax or alt_ax not in by_ax:
                continue
            common = sorted(set(by_ax[base_ax]) & set(by_ax[alt_ax]))
            if not common:
                continue
            rb = [by_ax[base_ax][g] for g in common]
            ra = [by_ax[alt_ax][g] for g in common]
            n = len(common)
            kb = sum(int(x["horizon_complete"]) for x in rb)
            ka = sum(int(x["horizon_complete"]) for x in ra)
            add_only = sum(1 for x, y in zip(ra, rb)
                           if int(x["horizon_complete"]) == 1 and int(y["horizon_complete"]) == 0)
            base_only = sum(1 for x, y in zip(ra, rb)
                            if int(x["horizon_complete"]) == 0 and int(y["horizon_complete"]) == 1)
            p = binom_two_sided_p(add_only, base_only)
            wpairs = [(float(x["total_wire_bytes"]), float(y["total_wire_bytes"]))
                      for x, y in zip(ra, rb)
                      if x["total_wire_bytes"] != "" and y["total_wire_bytes"] != ""]
            dw, dlo, dhi = boot_diff(wpairs, rng)
            # marginal-utility verdict (never a winner-takes-all label)
            if p < ALPHA and add_only > base_only:
                verdict = "POSITIVE_MARGINAL_UTILITY"
            elif p < ALPHA and base_only > add_only:
                verdict = "NEGATIVE_MARGINAL_UTILITY"
            elif add_only == 0 and base_only == 0:
                verdict = "NO_COMPLETION_CHANGE"
            else:
                verdict = "INDETERMINATE"
            summary[f"{domain}|{name}"][verdict] += 1
            lo_b, hi_b = wilson(kb, n)
            lo_a, hi_a = wilson(ka, n)
            out.append({
                "domain": domain, "cell_id": cell, "contrast": name,
                "base": f"{base_ax[0]}/r{base_ax[1]}", "added": f"{alt_ax[0]}/r{alt_ax[1]}",
                "n_pairs": n,
                "base_complete": f"{kb}/{n}", "added_complete": f"{ka}/{n}",
                "base_wilson95": f"[{lo_b:.3f},{hi_b:.3f}]", "added_wilson95": f"[{lo_a:.3f},{hi_a:.3f}]",
                "added_only": add_only, "base_only": base_only, "mcnemar_p": round(p, 5),
                "delta_wire_mean": round(dw, 1) if not math.isnan(dw) else "",
                "delta_wire_ci95": f"[{dlo:.1f},{dhi:.1f}]" if not math.isnan(dw) else "",
                "marginal_utility": verdict,
            })

    p_out = os.path.join(OUT, "marginal-utility-regions.csv")
    with open(p_out, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(out[0].keys())); w.writeheader(); w.writerows(out)

    res = {
        "framing": "ARQ is ACTIVE IN EVERY ARM. Measured quantity = MARGINAL UTILITY of proactive redundancy (r) on top of always-on ARQ. 'ARQ-vs-FEC' rival framing is forbidden and is NOT used.",
        "valid_single_axis_contrasts": [c[0] for c in CONTRASTS],
        "invalid_contrast_explicitly_excluded": "(tail,r0) vs (frozen,r2) — changes BOTH axes; this was the flawed basis of the earlier NO-GO and is never computed here",
        "coverage_gap": "(tail-capable ARQ, r>0) — the true fusion cell — is absent from every authoritative dataset; marginal FEC utility ON TOP OF tail-capable ARQ is therefore UNMEASURED",
        "rows": len(out),
        "verdicts": {k: dict(v) for k, v in sorted(summary.items())},
    }
    with open(os.path.join(OUT, "marginal-utility-summary.json"), "w") as f:
        json.dump(res, f, indent=2, sort_keys=True)
    print(json.dumps(res, indent=2))


if __name__ == "__main__":
    main()
