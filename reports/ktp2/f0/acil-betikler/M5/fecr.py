import sys, os, csv, re, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
for arm in ("taban", "aday"):
    nS = 0; nfin = 0; nr1 = 0; nprop = 0; nprop_r1 = 0; fins = []; props = []; par = []
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        s = open(os.path.join(rep, "c.err")).read()
        par.append(int(re.search(r"fec_parity=(\d+)B", s).group(1)))
        for r in csv.DictReader(open(os.path.join(rep, "ts.csv"))):
            nS += 1
            f = int(r["final_ratio"]); p = max(int(r["confirmed_prop"]), int(r["structure_prop"]), int(r["spec_prop"]))
            if p > 0:
                nprop += 1; props.append(p)
                if (p*8 + 500)//1000 >= 1: nprop_r1 += 1
            if f > 0:
                nfin += 1; fins.append(f)
                if (f*8 + 500)//1000 >= 1: nr1 += 1
    print("%s: ts samples=%d  arbiter-proposal>0: %d (r>=1 at K=8 if uncapped: %d; proposal med %d max %d) | final_ratio>0 after cap: %d (med %d, max %d) -> r>=1: %d | fec_parity bytes sum over 30 runs = %d" % (
        arm, nS, nprop, nprop_r1, st.median(props) if props else 0, max(props) if props else 0, nfin, st.median(fins) if fins else 0, max(fins) if fins else 0, nr1, sum(par)))
