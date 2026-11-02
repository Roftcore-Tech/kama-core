import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
from emu import emulate, at, SIZE
import re
def est_end(rep):
    s = open(os.path.join(rep, "c.err")).read()
    m = re.search(r"est=\[btlbw=(\d+)B/s srtt=(\d+)us rtprop=(\d+)us", s)
    return tuple(int(x) for x in m.groups())
step = 2.5
for arm in ("taban", "aday"):
    rc = []; rb = []; chk = []; per = []
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        A = analyze(rep); T0 = A["T0"]; S = emulate(A)
        e = est_end(rep)
        last = S[-1]
        chk.append((i, e[0], round(last["btlbw"]), e[1]/1e6, last["srtt"]/1e9 if last["srtt"] else 0, e[2]/1e6, last["rtprop"]/1e9 if last["rtprop"] else 0))
        tx = [t for t, ln, d in A["fwd_tx"] if d and d["type"] == "data"]
        R = A["res"]
        rcs = []; rbs = []
        for k in range(0, 40):
            a = T0 + k*step*1e9; b = a + step*1e9
            bl_a = sum(1 for r in R if r["offer"] <= a and (r["first_tx"] is None or r["first_tx"] > a))
            bl_b = sum(1 for r in R if r["offer"] <= b and (r["first_tx"] is None or r["first_tx"] > b))
            if bl_a < 3 or bl_b < 3: continue
            row = at(S, a)
            if row is None or row["srtt"] is None or row["btlbw"] <= 0: continue
            n = sum(1 for t in tx if a <= t < b)
            em = n*SIZE/step
            cs = max(2*row["btlbw"]*row["rtprop"]/1e9, 4800)/(row["srtt"]/1e9)
            rcs.append(em/cs); rbs.append(em/row["btlbw"])
        rc += rcs; rb += rbs
        per.append((i, len(rcs), st.median(rcs) if rcs else float('nan'), st.median(rbs) if rbs else float('nan')))
    print(arm, "estimator check (engine end vs emulated end): rep btl_eng btl_emu srtt_eng srtt_emu rtprop_eng rtprop_emu")
    nok = 0
    for c in chk:
        ok = abs(c[1]-c[2]) <= 0.02*max(c[1],1)
        nok += ok
        print("  %2d %5d %5d  %.3f %.3f  %.3f %.3f %s" % (c + ("OK" if ok else "DIFF",)))
    print(" btlbw match within 2%%: %d/30" % nok)
    print(" backlog windows: n=%d  emit/(cwnd/srtt): p10 %.2f p25 %.2f med %.2f p75 %.2f p90 %.2f | emit/btlbw: p10 %.2f p25 %.2f med %.2f p75 %.2f p90 %.2f" % (
        len(rc), pct(rc,.1), pct(rc,.25), pct(rc,.5), pct(rc,.75), pct(rc,.9), pct(rb,.1), pct(rb,.25), pct(rb,.5), pct(rb,.75), pct(rb,.9)))
    print(" per-rep (rep, nwin, med emit/cs, med emit/bw):", " ".join("%d:%d:%.2f:%.2f" % p for p in per))
