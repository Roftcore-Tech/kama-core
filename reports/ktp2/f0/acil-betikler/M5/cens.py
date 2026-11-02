import sys, os, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
for arm in ("taban", "aday", "quinn"):
    a = []; b = []; nd = []
    for i in range(1, 31):
        meta, ev = read_out(os.path.join(CELL, arm, "rep%d" % i))
        off = meta["events_epoch_offset_ns"]
        d = [(e["delivered_ns"] - e["sent_ns"])/1e9 for e in ev if e["delivered_ns"] is not None]
        allv = [(e["delivered_ns"] - e["sent_ns"])/1e9 if e["delivered_ns"] is not None else 1e9 for e in ev]
        a.append(pct(d, .95)); b.append(pct(allv, .95)); nd.append(len(d))
    print("%s: median per-run p95 delivered-only = %.2f s ; undelivered-as-inf = %.2f s ; runs with <100 delivered: %d (min %d)" % (arm, st.median(a), st.median(b), sum(1 for x in nd if x < 100), min(nd)))
