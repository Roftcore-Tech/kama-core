import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    gap = []; evid = []; wait_after_evid = []; arr = []; blk = []
    for i in range(1, 31):
        A = analyze(os.path.join(CELL, arm, "rep%d" % i)); T0 = A["T0"]
        # ACK arrivals at client: list of (t, set of acked full PN)
        datapn_t = {}
        for t, ln, d in A["fwd_tx"]:
            if d and d["type"] == "data": datapn_t[d["pn"]] = t
        low2 = {pn & 0xFFFF: pn for pn in datapn_t}
        acks = []
        for t, ln, d in A["ack_rx"]:
            if d and d["type"] == "ack" and "largest" in d:
                s = set(low2[l] for l in ack_low16_set(d) if l in low2)
                acks.append((t, s))
        R = A["res"]
        offers = [r["offer"] for r in R]
        for r in R:
            if r["ntx"] < 2 or r["att"] is None or r["att"] < 1: continue
            # first tx lost (att>=1)
            txs = sorted(A["pn_tx"][pn][0] for pn in [] ) if False else None
            tl = r["txt"]
            t0 = tl[0]; t1 = tl[1]
            # pn of first tx
            pn0 = None
            for t, ln, d in A["fwd_tx"]:
                if d and d["type"] == "data" and d["unit"] == r["u"]:
                    pn0 = d["pn"]; break
            ev = None
            for ta, s in acks:
                if ta > t0 and any(p > pn0 for p in s):
                    ev = ta; break
            gap.append((t1 - t0)/1e9)
            if ev:
                evid.append((ev - t0)/1e9); wait_after_evid.append((t1 - ev)/1e9)
            if r["first_arr"]: arr.append((r["first_arr"] - t0)/1e9)
            # backlog at t1
            blk.append(sum(1 for x in R if x["offer"] <= t1 and (x["first_tx"] is None or x["first_tx"] > t1)))
    print(arm, "units whose 1st tx was lost: n=%d" % len(gap))
    for nm, v in (("tx0->tx1 (retx gap)", gap), ("tx0->first SACK evidence", evid), ("evidence->tx1", wait_after_evid), ("tx0->arrival", arr), ("backlog at tx1", blk)):
        print("  %-26s p10 %6.2f p25 %6.2f med %6.2f p75 %6.2f p90 %6.2f max %6.2f" % (nm, pct(v,.1), pct(v,.25), pct(v,.5), pct(v,.75), pct(v,.9), max(v)))
