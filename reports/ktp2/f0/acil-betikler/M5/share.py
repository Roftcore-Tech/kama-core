import sys, os, csv, bisect, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    n_both = 0; n_both_re = 0; n_bl = 0; n_bl_re = 0; n_all = 0; n_all_re = 0
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        A = analyze(rep); T0 = A["T0"]; R = A["res"]
        offers = sorted(r["offer"] for r in R)
        ftx = sorted(r["first_tx"] for r in R if r["first_tx"])
        rows = list(csv.DictReader(open(os.path.join(rep, "ts.csv"))))
        tts = [T0 + int(r["t_ms"])*1_000_000 for r in rows]
        pl = [int(r["pending_lost"]) for r in rows]
        seen = set()
        for t, ln, d in A["fwd_tx"]:
            if not d or d["type"] != "data": continue
            re_ = d["unit"] in seen; seen.add(d["unit"])
            k = bisect.bisect_right(tts, t) - 1
            p = pl[k] if k >= 0 else 0
            bl = bisect.bisect_right(offers, t) - bisect.bisect_right(ftx, t - 1)  # backlog just before this tx
            n_all += 1; n_all_re += re_
            if bl >= 1:
                n_bl += 1; n_bl_re += re_
                if p > 0:
                    n_both += 1; n_both_re += re_
    print("%s: retx share of data-plane emissions: all %.3f (n=%d) | with sender backlog %.3f (n=%d) | backlog & pending_lost>0 %.3f (n=%d)" % (
        arm, n_all_re/n_all, n_all, n_bl_re/n_bl, n_bl, n_both_re/n_both, n_both))
