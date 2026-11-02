#!/usr/bin/env python3
import csv, os, re, statistics as st
OUT = os.path.dirname(os.path.abspath(__file__))
rows = list(csv.DictReader(open(os.path.join(OUT, "vruns.csv"))))
def I(x):
    try:
        return int(x)
    except Exception:
        return 0

def summ(sel, label):
    n = len(sel)
    pr = sum(1 for r in sel if I(r["fec_repairs_tx"]) > 0)
    par = sum(I(r["fec_repairs_tx"]) for r in sel)
    pf = sum(I(r["parity_fired"]) for r in sel)
    fecB = sum(I(r["fecB"]) for r in sel)
    srx = sum(I(r["srv_fec_repairs_rx"]) for r in sel)
    srec = sum(I(r["srv_fec_recovered_units"]) for r in sel)
    rrec = sum(1 for r in sel if I(r["srv_fec_recovered_units"]) > 0)
    retx = sum(I(r["retx_fired"]) for r in sel)
    part = sum(I(r["partial"]) for r in sel)
    blk = sum(I(r["fec_blocks"]) for r in sel)
    pden = sum(I(r["parity_denied"]) for r in sel)
    enq = sum(I(r.get("l_enq")) for r in sel)
    wz = sum(I(r.get("would_zero")) for r in sel)
    print(f"{label}: n={n} runs_par={pr} repairs_tx={par} parity_fired={pf} fecB={fecB} enq={enq} blocks={blk} partial={part} srv_rx={srx} srv_rec={srec} runs_rec={rrec} retx={retx} par_denied={pden} would_zero={wz}")

b = [r for r in rows if r["lane"] == "b" and r["arm"] in ("taban", "aday") and r.get("fec_repairs_tx") not in (None, "")]
for arm in ("taban", "aday"):
    summ([r for r in b if r["arm"] == arm], f"B {arm} all")
def lossof(cell):
    return int(re.search(r"_l(\d+)_", cell).group(1))
def rateof(cell):
    return re.search(r"_r(\w+?)_", cell).group(1)
for rate in ("12k", "1k", "25"):
    for arm in ("taban", "aday"):
        summ([r for r in b if r["arm"] == arm and rateof(r["cell"]) == rate and lossof(r["cell"]) > 0], f"B r{rate} {arm} loss>0")
        summ([r for r in b if r["arm"] == arm and rateof(r["cell"]) == rate and lossof(r["cell"]) == 0], f"B r{rate} {arm} loss=0")
for cell in ("b_r12k_d1000_l30_p1000", "b_r12k_d300_l20_p65", "b_r1k_d300_l20_p1000", "b_r12k_d1000_l0_p1000"):
    for arm in ("taban", "aday"):
        sel = [r for r in b if r["cell"] == cell and r["arm"] == arm]
        summ(sel, f"{cell} {arm}")
        bw = [I(r["btlbw"]) for r in sel]
        sr = [I(r["srtt_us"]) for r in sel]
        print("   btlbw med", st.median(bw) if bw else None, "srtt med", st.median(sr) if sr else None)
        for r in sel:
            if I(r["fec_repairs_tx"]) > 0:
                print("   ", r["rep"], "tx", r["fec_repairs_tx"], "pf", r["parity_fired"], "srx", r["srv_fec_repairs_rx"], "srec", r["srv_fec_recovered_units"], "retx", r["retx_fired"], "blocks", r["fec_blocks"], "partial", r["partial"])
# btlbw medians on r12k cells
print("--- r12k btlbw medians per cell/arm")
cells = sorted(set(r["cell"] for r in b if rateof(r["cell"]) == "12k"))
meds = []
for c in cells:
    for arm in ("taban", "aday"):
        bw = [I(r["btlbw"]) for r in b if r["cell"] == c and r["arm"] == arm]
        meds.append((st.median(bw), c, arm))
meds.sort()
print("min", meds[0], "max", meds[-1])
