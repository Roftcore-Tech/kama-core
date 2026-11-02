import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    S = {}
    for i in range(1, 31):
        A = analyze(os.path.join(CELL, arm, "rep%d" % i))
        T0 = A["T0"]
        seen = set()
        bins = {}
        for t, ln, d in A["fwd_tx"]:
            if d is None: continue
            b = int((t - T0)/5e9)
            k = "other"
            if d["type"] == "data":
                k = "first" if d["unit"] not in seen else "retx"
                seen.add(d["unit"])
            bb = bins.setdefault(b, {"first":0, "retx":0, "other":0})
            bb[k] += ln + 14
        # backlog at bin end: offered-not-first-tx
        R = A["res"]
        bl = {}
        for b in range(0, 18):
            te = T0 + (b+1)*5e9
            bl[b] = sum(1 for r in R if r["offer"] <= te and (r["first_tx"] is None or r["first_tx"] > te))
        S[i] = (bins, bl)
    print(arm, " B/s per 5s bin (median over 30 reps); backlog = offered-not-yet-first-sent units at bin end")
    for b in range(0, 18):
        tot = [sum(S[i][0].get(b, {}).values())/5 for i in S]
        fi = [S[i][0].get(b, {}).get("first",0)/5 for i in S]
        rt = [S[i][0].get(b, {}).get("retx",0)/5 for i in S]
        bl = [S[i][1][b] for i in S]
        print(" %3d-%3d s: total %6.0f [q1 %5.0f q3 %5.0f] first %5.0f retx %5.0f  backlog med %4.0f q1 %4.0f q3 %4.0f" % (
            b*5, b*5+5, st.median(tot), pct(tot,.25), pct(tot,.75), st.median(fi), st.median(rt), st.median(bl), pct(bl,.25), pct(bl,.75)))
