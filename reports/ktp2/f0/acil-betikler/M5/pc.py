import sys, os, re, csv
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    print(arm)
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        A = analyze(rep); T0 = A["T0"]
        s = open(os.path.join(rep, "c.err")).read()
        lp = int(re.search(r"left_preconv=(\d+)ms", s).group(1))
        rows = list(csv.DictReader(open(os.path.join(rep, "ts.csv"))))
        tok0 = rows[0]["pacer_tokens"]
        # u0 transmissions and fate
        u0 = [(t, d["pn"], d["seq"]) for t, ln, d in A["fwd_tx"] if d and d["type"] == "data" and d["unit"] == 0]
        arrived = set(A["pn_arr"].keys())
        fate = "".join("A" if pn in arrived else "x" for _, pn, _ in u0)
        u1 = next((r["first_tx"] for r in A["res"] if r["u"] == 1), None)
        print("  rep%2d left_preconv=%6d ms tokens@%sms=%s u0 tx=%d fate=%s (A=arrived) u1_first_tx=%.2fs" % (
            i, lp, rows[0]["t_ms"], tok0, len(u0), fate, (u1 - T0)/1e9 if u1 else -1))
