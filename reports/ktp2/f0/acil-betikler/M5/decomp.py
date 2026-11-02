import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
for arm in ("taban", "aday"):
    allA = []; allB = []; allC = []; tA = []; tB = []; tC = []; tot = []
    perrun = []
    for i in range(1, 31):
        A = analyze(os.path.join(CELL, arm, "rep%d" % i))
        R = [r for r in A["res"] if r["conf"] and r["first_arr"] and r["first_tx"]]
        lat = sorted((r["conf"] - r["offer"])/1e9 for r in R)
        p95 = pct(lat, .95)
        for r in R:
            a = (r["first_tx"] - r["offer"])/1e9; b = (r["first_arr"] - r["first_tx"])/1e9; c = (r["conf"] - r["first_arr"])/1e9
            allA.append(a); allB.append(b); allC.append(c)
            if (r["conf"] - r["offer"])/1e9 >= p95:
                tA.append(a); tB.append(b); tC.append(c); tot.append(a+b+c)
        perrun.append(p95)
    print(arm, "all delivered msgs: mean A(queue)=%.2f B(first_tx->first_arr)=%.2f C(arr->ack@sender)=%.2f" % (st.mean(allA), st.mean(allB), st.mean(allC)))
    print(arm, "tail msgs (>= run p95), n=%d: mean A=%.2f B=%.2f C=%.2f  (median A=%.2f B=%.2f C=%.2f) share A=%.0f%% B=%.0f%% C=%.0f%%" % (
        len(tA), st.mean(tA), st.mean(tB), st.mean(tC), st.median(tA), st.median(tB), st.median(tC),
        100*sum(tA)/sum(tot), 100*sum(tB)/sum(tot), 100*sum(tC)/sum(tot)))
