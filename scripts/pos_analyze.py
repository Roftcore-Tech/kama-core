#!/usr/bin/env python3
"""POS (policy-outcome-sweep) Z3-analizi: PAIRED-blok-sınıflandırması (§5) + P15/F15-fairness/
concentration (§15) + recovery-utility (§13; sıfır-bölme='NA'-explicit) + deficit-profilleri (§9) +
Pareto-özeti (§18). Pre-declared: birincil-outcome = dl_op (operational-deadline; T_op-formülü-harness-
başlığında); paired_block_key=(cell,seed,block); r4-ailesi-birincil-karşılaştırma."""
import csv, sys, statistics as st
from collections import defaultdict

SP = sys.argv[1]
rows = list(csv.DictReader(open(f"{SP}/pos-block-ledger.csv")))
runs = list(csv.DictReader(open(f"{SP}/pos-run-level.csv")))

# ── paired-sınıflandırma (r4: L vs P15 vs F15; outcome=dl_op) ──
blk = {}
for r in rows:
    blk[(r["cell"], r["seed"], r["block"], r["arm"])] = r
cells = sorted({r["cell"] for r in rows})
cls_out = open(f"{SP}/pos-paired-classes.csv", "w")
cls_out.write("cell,class,count\n")
fair_out = open(f"{SP}/pos-p15-f15.csv", "w")
fair_out.write("cell,metric,P15,F15,diff\n")
def ok(r): return r is not None and r["dl_op"] == "1"
for cell in cells:
    keys = sorted({(r["seed"], r["block"]) for r in rows if r["cell"] == cell and r["arm"] == "r4-L"})
    cnt = defaultdict(int)
    for (seed, b) in keys:
        L = blk.get((cell, seed, b, "r4-L")); P = blk.get((cell, seed, b, "r4-P15")); F = blk.get((cell, seed, b, "r4-F15"))
        if not (L and P and F): cnt["unpaired"] += 1; continue
        l, p, f = ok(L), ok(P), ok(F)
        if L["never_delivered"] == "1" and P["never_delivered"] == "1" and F["never_delivered"] == "1":
            cnt["pending_censored"] += 1; continue
        if l and p and f: cnt["all_succeeded"] += 1
        elif not l and not p and not f: cnt["all_failed"] += 1
        elif l and not p and not f: cnt["only_L"] += 1
        elif p and not l and not f: cnt["only_P15"] += 1
        elif f and not l and not p: cnt["only_F15"] += 1
        elif p and f and not l: cnt["P15_and_F15_rescued"] += 1
        elif l and p and not f: cnt["F15_harmed"] += 1
        elif l and f and not p: cnt["P15_harmed"] += 1
        else: cnt["mixed_other"] += 1
        if f and not p: cnt["F15_rescued_vs_P15"] += 1
        if p and not f: cnt["P15_rescued_vs_F15"] += 1
        if int(F["emitted"]) > int(L["emitted"]) and ok(F) == ok(L): cnt["more_parity_no_change_F15"] += 1
    for k, v in sorted(cnt.items()):
        cls_out.write(f"{cell},{k},{v}\n")
    # P15-vs-F15-tablosu
    def agg(arm, fn, col=None, flt=None):
        vals = [fn(r) for r in rows if r["cell"] == cell and r["arm"] == arm and (flt is None or flt(r))]
        return vals
    for (m, fn, red) in [
        ("emitted_parity", lambda r: int(r["emitted"]), sum),
        ("zero_parity_blocks", lambda r: 1 if r["emitted"] == "0" and r["planned"] != "0" else 0, sum),
        ("fec_decodes", lambda r: int(r["fec_decoded"]), sum),
        ("dl_op_blocks", lambda r: int(r["dl_op"]), sum),
        ("avoided_retx", lambda r: int(r["avoided_retx"]), sum),
        ("moot_parity", lambda r: int(r["moot_parity"]), sum),
        ("worst_decile_comp", lambda r: int(r["comp_rel_ms"]) if r["comp_rel_ms"] != "-1" else 10**9, None),
    ]:
        pv = agg("r4-P15", fn); fv = agg("r4-F15", fn)
        if red is sum:
            a, b = sum(pv), sum(fv)
        else:  # worst-decile (p90 of comp)
            pv.sort(); fv.sort()
            a = pv[int(len(pv)*0.9)] if pv else 0; b = fv[int(len(fv)*0.9)] if fv else 0
        fair_out.write(f"{cell},{m},{a},{b},{b-a}\n")

# ── deficit-profil × decode (r4-ailesi) ──
dp = open(f"{SP}/pos-deficit-decode.csv", "w")
dp.write("cell,deficit_class,arm,blocks,decoded,dl_op\n")
for cell in cells:
    for dc, lo, hi in [("0",0,0),("1",1,1),("2",2,2),("3+",3,99)]:
        for arm in ["r4-L","r4-P15","r4-F15"]:
            rs = [r for r in rows if r["cell"]==cell and r["arm"]==arm and lo<=int(r["src_lost_first"])<=hi]
            if rs:
                dp.write(f"{cell},{dc},{arm},{len(rs)},{sum(int(r['fec_decoded']) for r in rs)},{sum(int(r['dl_op']) for r in rs)}\n")

# ── utility (arm×cell; NA-explicit) ──
ut = open(f"{SP}/pos-utility.csv", "w")
ut.write("cell,arm,parity_bytes,decodes,avoided_retx_ev,dl_op_rate,retx_eff,decode_per_MB,ndisp,rdisp\n")
runby = defaultdict(list)
for r in runs: runby[(r["cell"], r["arm"])].append(r)
for cell in cells:
    for arm in sorted({r["arm"] for r in rows if r["cell"]==cell}):
        rs = [r for r in rows if r["cell"]==cell and r["arm"]==arm]
        pb = sum(int(r["parity_bytes"]) for r in rs)
        dec = sum(int(r["fec_decoded"]) for r in rs)
        av = sum(int(r["avoided_retx"]) for r in rs)
        dlr = sum(int(r["dl_op"]) for r in rs)/max(len(rs),1)
        retx_eff = f"{av*1200/pb:.4f}" if pb else "NA"
        dpm = f"{dec*1e6/pb:.3f}" if pb else "NA"
        rr = runby.get((cell,arm),[])
        nd = st.median([int(x["ndisp"]) for x in rr]) if rr else 0
        rd = st.median([int(x["rdisp"]) for x in rr]) if rr else 0
        ut.write(f"{cell},{arm},{pb},{dec},{av},{dlr:.4f},{retx_eff},{dpm},{nd:.0f},{rd:.0f}\n")
print("yazıldı: pos-paired-classes / pos-p15-f15 / pos-deficit-decode / pos-utility")
