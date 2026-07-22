#!/usr/bin/env python3
"""AA — SEED-CLUSTER-inference (blok≠bağımsız-örnek; birim=seed/run-cluster) + spend-matched-denetim +
blackout-kök-neden + recovery-normalized. Cluster-bootstrap: seed'leri-resample (1000; deterministik-
xorshift). Practical-equivalence-spend-bandı (PRE-DECLARED): |Δparity_bytes|/P15 ≤ 0.05."""
import csv, sys, statistics as st
from collections import defaultdict

SP = sys.argv[1]
rows = list(csv.DictReader(open(f"{SP}/pos-block-ledger.csv")))
def xs(seed):
    x = seed or 1
    while True:
        x ^= (x << 13) & (2**64-1); x ^= x >> 7; x ^= (x << 17) & (2**64-1); yield x
def boot_ci(vals, seedh, n=1000):
    if len(vals) < 2: return (min(vals or [0]), max(vals or [0]))
    g = xs(seedh); meds = []
    for _ in range(n):
        s = [vals[next(g) % len(vals)] for _ in vals]; meds.append(st.median(s))
    meds.sort(); return (meds[int(.025*n)], meds[int(.975*n)])

by = defaultdict(list)
for r in rows: by[(r["cell"], r["arm"], r["seed"])].append(r)
cells = sorted({r["cell"] for r in rows})
out = open(f"{SP}/aa-cluster-ci.csv", "w")
out.write("cell,comparison,metric,seed_deltas_median,ci_lo,ci_hi,pos_seeds,neg_seeds,zero_seeds,excludes_zero\n")
METRICS = [
    ("dl_op_rate", lambda rs: sum(int(r["dl_op"]) for r in rs)/max(len(rs),1)),
    ("decodes", lambda rs: sum(int(r["fec_decoded"]) for r in rs)),
    ("rescued_dlop", lambda rs: sum(int(r["dl_op"]) for r in rs)),
    ("avoided_retx", lambda rs: sum(int(r["avoided_retx"]) for r in rs)),
    ("parity_bytes", lambda rs: sum(int(r["parity_bytes"]) for r in rs)),
    ("worst_decile_comp", lambda rs: sorted((int(r["comp_rel_ms"]) if r["comp_rel_ms"]!="-1" else 10**9) for r in rs)[int(len(rs)*0.9)] if rs else 0),
]
for cell in cells:
    seeds = sorted({r["seed"] for r in rows if r["cell"]==cell and r["arm"]=="r4-F15"})
    for (aname, bname, tagn) in [("r4-F15","r4-L","F15-L"), ("r4-F15","r4-P15","F15-P15")]:
        for (mn, fn) in METRICS:
            deltas = []
            for sd in seeds:
                A = by.get((cell, aname, sd)); B = by.get((cell, bname, sd))
                if A and B: deltas.append(fn(A) - fn(B))
            if not deltas: continue
            lo, hi = boot_ci(deltas, sum(ord(c) for c in cell+tagn+mn))
            pos = sum(1 for d in deltas if d > 0); neg = sum(1 for d in deltas if d < 0)
            zer = len(deltas) - pos - neg
            excl = int(lo > 0 or hi < 0)
            out.write(f"{cell},{tagn},{mn},{st.median(deltas):.3f},{lo:.3f},{hi:.3f},{pos},{neg},{zer},{excl}\n")

# rev99 seed-level rescued/harmed (F15-vs-P15; dl_op-bazlı)
sv = open(f"{SP}/aa-rev99-seedlevel.csv", "w")
sv.write("seed,f15_rescue_minus_harm_vs_P15,f15_only,p15_only\n")
blk = {}
for r in rows: blk[(r["cell"], r["seed"], r["block"], r["arm"])] = r
for sd in sorted({r["seed"] for r in rows if r["cell"]=="rev99"}):
    f_r = p_r = fo = po = 0
    for b in {r["block"] for r in rows if r["cell"]=="rev99" and r["seed"]==sd and r["arm"]=="r4-L"}:
        P = blk.get(("rev99", sd, b, "r4-P15")); F = blk.get(("rev99", sd, b, "r4-F15")); L = blk.get(("rev99", sd, b, "r4-L"))
        if not (P and F and L): continue
        p, f, l = P["dl_op"]=="1", F["dl_op"]=="1", L["dl_op"]=="1"
        if f and not p: f_r += 1
        if p and not f: p_r += 1
        if f and not l and not p: fo += 1
        if p and not l and not f: po += 1
    sv.write(f"{sd},{f_r-p_r},{fo},{po}\n")

# Spend-matched (r4: P15-vs-F15 per-seed parity-bytes + oranlar)
sm = open(f"{SP}/aa-spend-matched.csv", "w")
sm.write("cell,seed,pb_P15,pb_F15,spend_delta_frac,decode_delta,dl_delta,decode_per_dByte\n")
for cell in cells:
    for sd in sorted({r["seed"] for r in rows if r["cell"]==cell}):
        P = by.get((cell,"r4-P15",sd)); F = by.get((cell,"r4-F15",sd))
        if not (P and F): continue
        pbP = sum(int(r["parity_bytes"]) for r in P); pbF = sum(int(r["parity_bytes"]) for r in F)
        dP = sum(int(r["fec_decoded"]) for r in P); dF = sum(int(r["fec_decoded"]) for r in F)
        oP = sum(int(r["dl_op"]) for r in P); oF = sum(int(r["dl_op"]) for r in F)
        frac = (pbF-pbP)/max(pbP,1)
        dpb = f"{(dF-dP)*1e6/(pbF-pbP):.2f}" if pbF!=pbP else "NA"
        sm.write(f"{cell},{sd},{pbP},{pbF},{frac:.4f},{dF-dP},{oF-oP},{dpb}\n")

# Blackout-30s kök-neden: harmed-blok-timeline + faz + F10 + recovery-normalized
bo = open(f"{SP}/aa-blackout30-rootcause.csv", "w")
bo.write("seed,block,phase,open_ms,close_ms,first_par,emitted_F15,retx_F15,decode_F15,comp_F15,comp_P15,comp_L,miss_margin_ms,F10_dlop,recovnorm_F15,recovnorm_P15\n")
T_OP = 10_000
def phase(open_ms):
    m = open_ms % 60_000
    return "during_blackout" if m < 30_000 else "recovery_window"
def recovnorm(open_ms, comp_rel):
    if comp_rel < 0: return 0
    comp_abs = open_ms + comp_rel
    k = open_ms // 60_000
    dark_end = k*60_000 + 30_000
    if open_ms >= dark_end: dark_end += 60_000  # sonraki-karanlığın-bitişi
    return int(comp_abs <= dark_end + 10_000)  # allowance=10s (PRE-DECLARED)
for sd in sorted({r["seed"] for r in rows if r["cell"]=="blackout-30s"}):
    for b in sorted({r["block"] for r in rows if r["cell"]=="blackout-30s" and r["seed"]==sd}):
        L = blk.get(("blackout-30s", sd, b, "r4-L")); F = blk.get(("blackout-30s", sd, b, "r4-F15"))
        P = blk.get(("blackout-30s", sd, b, "r4-P15")); F10 = blk.get(("blackout-30s", sd, b, "r4-F10"))
        if not (L and F and P): continue
        if L["dl_op"]=="1" and F["dl_op"]=="0":  # harmed
            comp = int(F["comp_rel_ms"]) if F["comp_rel_ms"]!="-1" else -1
            bo.write(f"{sd},{b},{phase(int(F['open_ms']))},{F['open_ms']},{F['close_ms']},{F['first_par']},{F['emitted']},{F['retx_units']},{F['fec_decoded']},{comp},{P['comp_rel_ms']},{L['comp_rel_ms']},{comp-T_OP},{F10['dl_op'] if F10 else 'NA'},{recovnorm(int(F['open_ms']),comp)},{recovnorm(int(P['open_ms']),int(P['comp_rel_ms']) if P['comp_rel_ms']!='-1' else -1)}\n")
print("yazıldı: aa-cluster-ci / aa-rev99-seedlevel / aa-spend-matched / aa-blackout30-rootcause")
