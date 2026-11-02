import sys, os, re, csv, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
for arm in ("taban", "aday"):
    W = []; frac = []; frac_tok = []
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        s = open(os.path.join(rep, "c.err")).read()
        retx = int(re.search(r"fired=\[[^\]]*retx=(\d+)", s).group(1))
        rows = list(csv.DictReader(open(os.path.join(rep, "ts.csv"))))
        ts = [int(r["t_ms"]) for r in rows]
        pl = [int(r["pending_lost"]) for r in rows]
        tok = [int(r["pacer_tokens"]) for r in rows]
        integ = 0.0
        for k in range(1, len(rows)):
            integ += pl[k-1] * (ts[k]-ts[k-1]) / 1000.0
        n_pos = sum(1 for x in pl if x > 0)
        W.append(integ / max(retx, 1))
        frac.append(n_pos / len(pl))
        # when pending_lost>0, how often tokens >= 1027 (retx could have gone but did not)?
        both = sum(1 for a, b in zip(pl, tok) if a > 0 and b >= 1027)
        frac_tok.append(both / max(n_pos, 1))
    print(arm, "Little: mean time in pending_lost per retx [s]: med %.2f q1 %.2f q3 %.2f ; frac samples pending_lost>0: med %.2f ; frac(pending>0 & tokens>=1027 | pending>0): med %.3f max %.3f" % (
        st.median(W), pct(W,.25), pct(W,.75), st.median(frac), st.median(frac_tok), max(frac_tok)))
