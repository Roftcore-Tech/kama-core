#!/usr/bin/env python3
"""MS1 (1b Budget Characterization — Micro-Sweep 1) analiz: median + deterministik-bootstrap-%95-CI +
validity-sınıflandırması + practical-equivalence + BtlBw/offered tablosu.

PRE-DECLARED kurallar (koşu-sonucuna-bakılmadan-yazıldı; harness-dosyası-başlığıyla-aynı):
- practical-equivalence(arm, baseline): |Δmedian-completion| ≤ 0.01 ∧ |Δmedian-goodput|/max(gp_base,1e-9) ≤ 0.02
  ∧ %95-CI'ler-örtüşür (completion).
- Validity (FEC-arm'ları; realization = par_emit/par_enq):
    CENSORED        : censored-oran ≥ 0.5
    FULLY-COLLAPSED : median-realization == 0 ∧ median-par_enq > 0
    PARTIALLY-COLL. : 0 < median-realization < 0.8
    VALID-SEPARATED : median-realization ≥ 0.8
    BIMODAL         : (ek-etiket) par_emit-vektöründe max-ardışık-gap > 0.5×range ∧ iki-küme ≥ 3-üye
    ANOMALOUS       : (ek-etiket) censored==0 ∧ completion < 0.2
- FULLY-COLLAPSED hücreden FEC-ratio-crossover-hükmü ÇIKARILMAZ (yalnız-budget-teşhisi).
Bootstrap: 1000-resample, deterministik xorshift (seed = hash(cell,arm)); RNG-kütüphanesiz.
"""
import csv, sys, statistics as st
from collections import defaultdict

def xorshift(seed):
    x = seed or 1
    while True:
        x ^= (x << 13) & 0xFFFFFFFFFFFFFFFF
        x ^= x >> 7
        x ^= (x << 17) & 0xFFFFFFFFFFFFFFFF
        yield x

def boot_ci(vals, seed, n=1000):
    if not vals:
        return (0.0, 0.0)
    g = xorshift(seed)
    meds = []
    for _ in range(n):
        sample = [vals[next(g) % len(vals)] for _ in vals]
        meds.append(st.median(sample))
    meds.sort()
    return (meds[int(0.025 * n)], meds[int(0.975 * n)])

def bimodal(vals):
    if len(vals) < 6:
        return False
    v = sorted(vals)
    rng = v[-1] - v[0]
    if rng <= 0:
        return False
    gaps = [(v[i + 1] - v[i], i) for i in range(len(v) - 1)]
    g, i = max(gaps)
    return g > 0.5 * rng and (i + 1) >= 3 and (len(v) - i - 1) >= 3

def main(path):
    rows = list(csv.DictReader(open(f"{path}/ms1-seed-level.csv")))
    by = defaultdict(list)
    for r in rows:
        by[(r["cell"], r["arm"])].append(r)
    cells = sorted({c for c, _ in by})
    out = open(f"{path}/ms1-summary.csv", "w")
    out.write("cell,arm,n_seeds,comp_med,comp_lo,comp_hi,gp_med,gp_lo,gp_hi,real_med,enq_med,emit_med,canc_med,denials_med,first_par_delay_med,validity,extra_labels,equiv_vs_arq,censored_frac,t_p50_med,dl5_med,btlbw_p50_med,rate_p50_med,offered,cap\n")
    bo = open(f"{path}/ms1-btlbw-offered.csv", "w")
    bo.write("cell,arm,cap_Bps,offered_Bps,delivered_Bps_med,btlbw_p50_med,btlbw_max_med,rate_p50_med,rate_max_med,parity_demand_Bps_med,realization_med,denials_med,util_offered_over_cap\n")
    base_med = {}
    for cell in cells:
        for arm in ["ARQ-baseline", "Manual-r2", "Manual-r4", "Manual-r8", "Controller-ratio"]:
            rs = by.get((cell, arm), [])
            if not rs:
                continue
            f = lambda k: [float(r[k]) for r in rs]
            comp, gp = f("completion"), f("goodput")
            real, enq, emit = f("realization"), f("par_enq"), f("par_emit")
            canc, den = f("par_canc"), f("denials")
            seedh = sum(ord(c) for c in cell + arm)
            c_lo, c_hi = boot_ci(comp, seedh)
            g_lo, g_hi = boot_ci(gp, seedh + 7)
            cens = sum(float(r["censored"]) for r in rs) / len(rs)
            med = st.median
            validity = "N/A-ARQ"
            extra = []
            if arm != "ARQ-baseline":
                if cens >= 0.5:
                    validity = "CENSORED"
                elif med(enq) == 0:
                    # Talep-hiç-yok (Controller ratio=0): budget-collapse-DEĞİL — ayrı-sınıf.
                    validity = "NO-PARITY-DEMAND"
                elif med(real) == 0:
                    validity = "FULLY-COLLAPSED"
                elif med(real) < 0.8:
                    validity = "PARTIALLY-COLLAPSED"
                else:
                    validity = "VALID-SEPARATED"
                if bimodal(emit):
                    extra.append("BIMODAL")
            if cens == 0 and med(comp) < 0.2:
                extra.append("ANOMALOUS")
            if arm == "ARQ-baseline":
                base_med[cell] = (med(comp), med(gp), (c_lo, c_hi))
            eq = ""
            if arm != "ARQ-baseline" and cell in base_med:
                bc, bg, bci = base_med[cell]
                ci_overlap = not (c_hi < bci[0] or c_lo > bci[1])
                eq = "EQUIV" if (abs(med(comp) - bc) <= 0.01 and abs(med(gp) - bg) / max(bg, 1e-9) <= 0.02 and ci_overlap) else "DIFF"
            end_t = f("end_t_ms")
            delivered_bps = st.median([float(r["completion"]) * 400 * 1200 * 1000 / max(float(r["end_t_ms"]), 1) for r in rs])
            par_demand = st.median([float(r["par_enq"]) * 1200 * 1000 / max(float(r["end_t_ms"]), 1) for r in rs])
            out.write(f"{cell},{arm},{len(rs)},{med(comp):.4f},{c_lo:.4f},{c_hi:.4f},{med(gp):.4f},{g_lo:.4f},{g_hi:.4f},{med(real):.4f},{med(enq):.0f},{med(emit):.0f},{med(canc):.0f},{med(den):.0f},{med(f('first_par_delay_p50')):.0f},{validity},{'|'.join(extra)},{eq},{cens:.2f},{med(f('t_p50')):.0f},{med(f('dl5')):.4f},{med(f('btlbw_p50')):.0f},{med(f('rate_p50')):.0f},{rs[0]['offered_Bps']},{rs[0]['cap_Bps']}\n")
            bo.write(f"{cell},{arm},{rs[0]['cap_Bps']},{rs[0]['offered_Bps']},{delivered_bps:.0f},{med(f('btlbw_p50')):.0f},{med(f('btlbw_max')):.0f},{med(f('rate_p50')):.0f},{med(f('rate_max')):.0f},{par_demand:.0f},{med(real):.4f},{med(den):.0f},{float(rs[0]['offered_Bps'])/float(rs[0]['cap_Bps']):.3f}\n")
    print("yazıldı: ms1-summary.csv + ms1-btlbw-offered.csv")

if __name__ == "__main__":
    main(sys.argv[1])
