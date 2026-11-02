import sys, os, csv, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    nb = 0; nb_tok = 0; nb_pl = 0
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        A = analyze(rep); T0 = A["T0"]; R = A["res"]
        offers = sorted(r["offer"] for r in R)
        ftx = sorted(r["first_tx"] for r in R if r["first_tx"])
        import bisect
        for r in csv.DictReader(open(os.path.join(rep, "ts.csv"))):
            t = T0 + int(r["t_ms"]) * 1_000_000
            bl = bisect.bisect_right(offers, t) - bisect.bisect_right(ftx, t)
            if bl >= 1:
                nb += 1
                if int(r["pacer_tokens"]) >= 1027: nb_tok += 1
                if int(r["pending_lost"]) > 0: nb_pl += 1
    print("%s: ts samples with sender backlog>=1: %d; of these pacer_tokens>=1027: %d (%.2f%%); pending_lost>0: %.1f%%" % (arm, nb, nb_tok, 100*nb_tok/nb, 100*nb_pl/nb))
