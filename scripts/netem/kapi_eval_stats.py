#!/usr/bin/env python3
"""[KAPI-2 §2] Ön-kayıtlı primary test — rep-düzeyi paired Δ, paired-bootstrap %95 CI, Holm-3.

Deterministik: bootstrap tohumu = SHA256("KAPI2-BOOT" ‖ cell_id) (ön-kayıt: "bootstrap-tohumu =
hücre-tohumu"). Başarı kuralı (değiştirilemez): Holm-sonrası anlamlı (adj-p<0.05) VE %95 CI>0
VE Δ̂ ≥ +15pp. Maliyet paired farkı İKİNCİL (aynı ailede raporlanır, tek başına iddia taşımaz).

Kullanım: kapi_eval_stats.py <out_dir>   (runs.csv'yi okur; PRIMARY-VERDICTS.csv yazar)
"""

import csv
import hashlib
import random
import statistics
import sys
from pathlib import Path

PRIMARY_CELLS = [
    "evaluation_p2_loss80_paced_A2_d120_u0.75",
    "evaluation_p3_loss95_paced_A2_d120_u0.75",
    "evaluation_p5_revpath_paced_A2_d120_u0.75",
]
K, C = "kama-tail-v1", "q-dgram-arq"
SIDE_ARMS = ["kama-arq", "q-stream"]
B = 10_000


def boot(deltas, seed_key):
    rng = random.Random(int.from_bytes(hashlib.sha256(seed_key.encode()).digest()[:8], "big"))
    n = len(deltas)
    means = sorted(
        sum(deltas[rng.randrange(n)] for _ in range(n)) / n for _ in range(B)
    )
    lo, hi = means[int(0.025 * B)], means[int(0.975 * B) - 1]
    p_le = sum(1 for m in means if m <= 0) / B
    p_ge = sum(1 for m in means if m >= 0) / B
    p = max(min(1.0, 2 * min(p_le, p_ge)), 1.0 / B)
    return lo, hi, p


def main() -> int:
    root = Path(sys.argv[1])
    rows = list(csv.DictReader(open(root / "analysis/runs.csv")))
    by = {}
    for r in rows:
        if r.get("missing") == "0":
            by[(r["cell"], r["rep"], r["arm"])] = r

    out_rows, prim = [], []
    for cell in PRIMARY_CELLS:
        reps = sorted({r["rep"] for r in rows if r["cell"] == cell and r.get("missing") == "0"})
        d_comp, d_cost = [], []
        for rep in reps:
            a, b = by.get((cell, rep, K)), by.get((cell, rep, C))
            if a and b:
                d_comp.append(float(a["completion"]) - float(b["completion"]))
                d_cost.append(float(a["cost"]) - float(b["cost"]))
        lo, hi, p = boot(d_comp, f"KAPI2-BOOT|{cell}")
        clo, chi, cp = boot(d_cost, f"KAPI2-BOOT-COST|{cell}")
        prim.append(
            dict(cell=cell, n=len(d_comp), mean=statistics.mean(d_comp),
                 median=statistics.median(d_comp), ci=(lo, hi), p=p,
                 cost_mean=statistics.mean(d_cost), cost_ci=(clo, chi), cost_p=cp)
        )

    # Holm (3 hücre, completion-endpoint üstünden)
    order = sorted(range(len(prim)), key=lambda i: prim[i]["p"])
    adj = [None] * len(prim)
    running = 0.0
    for rank, i in enumerate(order):
        a = min(1.0, prim[i]["p"] * (len(prim) - rank))
        running = max(running, a)
        adj[i] = running
    for i, pr in enumerate(prim):
        pr["holm_p"] = adj[i]
        pr["verdict"] = (
            "KAMA-SUPERIOR"
            if adj[i] < 0.05 and pr["ci"][0] > 0 and pr["mean"] >= 0.15
            else ("SEPARATION-NO-CLAIM" if adj[i] < 0.05 and pr["ci"][0] > 0 else "NO-SEPARATION")
        )
        out_rows.append(
            dict(cell=pr["cell"], n=pr["n"], delta_mean=round(pr["mean"], 4),
                 delta_median=round(pr["median"], 4),
                 ci_lo=round(pr["ci"][0], 4), ci_hi=round(pr["ci"][1], 4),
                 p_boot=pr["p"], holm_p=round(pr["holm_p"], 5), verdict=pr["verdict"],
                 cost_delta_mean=round(pr["cost_mean"], 4),
                 cost_ci_lo=round(pr["cost_ci"][0], 4), cost_ci_hi=round(pr["cost_ci"][1], 4))
        )

    with open(root / "analysis/PRIMARY-VERDICTS.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(out_rows[0].keys()))
        w.writeheader()
        w.writerows(out_rows)

    print("== ÖN-KAYITLI PRIMARY TEST (kama-tail-v1 − q-dgram-arq; Holm-3; kural: adj-p<.05 ∧ CI>0 ∧ Δ̂≥+15pp) ==")
    for r in out_rows:
        print(f"  {r['cell']}")
        print(f"    n={r['n']}  Δ̂={r['delta_mean']:+.3f} (med {r['delta_median']:+.3f})  "
              f"%95CI [{r['ci_lo']:+.3f}, {r['ci_hi']:+.3f}]  holm-p={r['holm_p']:.5f}  → {r['verdict']}")
        print(f"    maliyet Δ̂={r['cost_delta_mean']:+.4f}  CI [{r['cost_ci_lo']:+.4f}, {r['cost_ci_hi']:+.4f}] (ikincil)")
    # Yan kollar (rapor zorunluluğu — el-devri kuralı): hücre medyanları
    print("\n== yan kollar (medyan completion; iddia değil, rapor zorunluluğu) ==")
    for cell in PRIMARY_CELLS:
        meds = {}
        for arm in [K, C] + SIDE_ARMS:
            vals = [float(r["completion"]) for r in rows
                    if r["cell"] == cell and r["arm"] == arm and r.get("missing") == "0"]
            meds[arm] = statistics.median(vals) if vals else None
        print(f"  {cell}: " + "  ".join(f"{a}={meds[a]:.3f}" for a in meds))
    return 0


if __name__ == "__main__":
    sys.exit(main())
