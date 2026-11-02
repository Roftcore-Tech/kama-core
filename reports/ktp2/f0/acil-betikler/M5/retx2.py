import sys, os, bisect
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    g0 = []; g1 = []
    for i in range(1, 31):
        A = analyze(os.path.join(CELL, arm, "rep%d" % i)); R = A["res"]
        offers = sorted(r["offer"] for r in R); ftx = sorted(r["first_tx"] for r in R if r["first_tx"])
        datapn = {}
        for t, ln, d in A["fwd_tx"]:
            if d and d["type"] == "data": datapn[d["pn"]] = (t, d["unit"])
        low2 = {pn & 0xFFFF: pn for pn in datapn}
        acks = []
        for t, ln, d in A["ack_rx"]:
            if d and d["type"] == "ack" and "largest" in d:
                acks.append((t, max((low2[l] for l in ack_low16_set(d) if l in low2), default=-1)))
        first_pn = {}
        for pn in sorted(datapn):
            first_pn.setdefault(datapn[pn][1], pn)
        for r in R:
            if r["ntx"] < 2 or r["att"] is None or r["att"] < 1: continue
            pn0 = first_pn[r["u"]]; t0, t1 = r["txt"][0], r["txt"][1]
            ev = next((ta for ta, mx in acks if ta > t0 and mx > pn0), None)
            if ev is None or ev > t1: continue
            bl = bisect.bisect_right(offers, ev) - bisect.bisect_right(ftx, ev)
            (g0 if bl == 0 else g1).append((t1 - ev)/1e9)
    print("%s evidence->retx: backlog==0 at evidence n=%d med %.2f p75 %.2f p90 %.2f | backlog>0 n=%d med %.2f p75 %.2f p90 %.2f" % (
        arm, len(g0), pct(g0,.5), pct(g0,.75), pct(g0,.9), len(g1), pct(g1,.5), pct(g1,.75), pct(g1,.9)))
