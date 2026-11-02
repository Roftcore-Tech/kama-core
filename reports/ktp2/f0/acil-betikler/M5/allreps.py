import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
out = []
for arm in ("taban", "aday"):
    rows = []
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        A = analyze(rep)
        R = A["res"]
        rxl = [(r["rx"]-r["offer"])/1e9 for r in R if r["rx"]]
        cfl = [(r["conf"]-r["offer"])/1e9 for r in R if r["conf"]]
        wait = [(r["first_tx"]-r["offer"])/1e9 for r in R if r["first_tx"]]
        rec = [(r["first_arr"]-r["first_tx"])/1e9 for r in R if r["first_arr"] and r["first_tx"]]
        hold = [(r["rx"]-r["first_arr"])/1e9 for r in R if r["first_arr"] and r["rx"]]
        ntx = [r["ntx"] for r in R]
        lost1 = sum(1 for r in R if r["att"] is not None and r["att"] >= 1)
        # first tx: time of 1st and last first-tx
        ftx = sorted(r["first_tx"] for r in R if r["first_tx"])
        T0 = A["T0"]
        rows.append((i, len(rxl), pct(rxl,.5), pct(rxl,.95), max(rxl), pct(cfl,.95), max(wait), pct(wait,.5), pct(rec,.5), pct(rec,.95), max(hold), sum(ntx), lost1, (ftx[1]-T0)/1e9, (ftx[-1]-T0)/1e9))
    print(arm)
    print("rep n  rx_p50 rx_p95 rx_max conf_p95 wait_max wait_p50 rec_p50 rec_p95 hold_max ntx lost>=1 t_ftx1 t_ftx_last")
    for r in rows:
        print("%2d %3d %6.2f %6.2f %6.2f %6.2f %6.2f %6.2f %6.2f %6.2f %6.3f %4d %3d %5.2f %6.2f" % r)
    col = lambda k: [r[k] for r in rows]
    print("MED rx_p95=%.2f conf_p95=%.2f wait_max=%.2f wait_p50=%.2f rec_p50=%.2f rec_p95=%.2f t_ftx1=%.2f t_ftx_last=%.2f" % (
        st.median(col(3)), st.median(col(5)), st.median(col(6)), st.median(col(7)), st.median(col(8)), st.median(col(9)), st.median(col(13)), st.median(col(14))))
