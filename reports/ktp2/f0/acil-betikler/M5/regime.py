import sys, os, re, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
from emu import emulate, at, SIZE
step = 2.5
def rank(v):
    s = sorted(range(len(v)), key=lambda i: v[i]); r = [0]*len(v)
    for k, i in enumerate(s): r[i] = k
    return r
def spearman(a, b):
    ra, rb = rank(a), rank(b); n = len(a)
    ma, mb = st.mean(ra), st.mean(rb)
    num = sum((x-ma)*(y-mb) for x, y in zip(ra, rb))
    den = (sum((x-ma)**2 for x in ra) * sum((y-mb)**2 for y in rb))**0.5
    return num/den
for arm in ("taban", "aday"):
    rows = []
    for i in range(1, 31):
        rep = os.path.join(CELL, arm, "rep%d" % i)
        A = analyze(rep); T0 = A["T0"]; S = emulate(A); R = A["res"]
        lat = sorted((r["conf"]-r["offer"])/1e9 for r in R if r["conf"])
        p95 = pct(lat, .95) if len(lat) >= 95 else float('inf')
        # p95 over all 100 with undelivered = inf
        latall = sorted([(r["conf"]-r["offer"])/1e9 if r["conf"] else 1e9 for r in R])
        p95all = pct(latall, .95)
        tx = [t for t, ln, d in A["fwd_tx"] if d and d["type"] == "data"]
        eb = []; bws = []; ems = []
        for k in range(0, 40):
            a = T0 + k*step*1e9; b = a + step*1e9
            bl_a = sum(1 for r in R if r["offer"] <= a and (r["first_tx"] is None or r["first_tx"] > a))
            bl_b = sum(1 for r in R if r["offer"] <= b and (r["first_tx"] is None or r["first_tx"] > b))
            if bl_a < 3 or bl_b < 3: continue
            row = at(S, a)
            if row is None or row["srtt"] is None or row["btlbw"] <= 0: continue
            n = sum(1 for t in tx if a <= t < b); em = n*SIZE/step
            eb.append(em/row["btlbw"]); bws.append(row["btlbw"]); ems.append(em)
        rows.append((i, p95all, st.median(eb) if eb else float('nan'), st.median(bws) if bws else 0, st.median(ems) if ems else 0))
    good = [r for r in rows if r[2] >= 1.35]; bad = [r for r in rows if r[2] < 1.35]
    print(arm, "regime split by median emit/btlbw in backlog windows (>=1.35 ~ cwnd/srtt-bound Startup base; <1.35 ~ gain-1 btlbw-bound)")
    for nm, g in (("cwnd/srtt-bound", good), ("btlbw-bound", bad)):
        print("  %-16s n=%2d  p95(sender) median %.1f s [range %.1f-%.1f]  backlog btlbw median %.0f B/s  emission median %.0f B/s" % (
            nm, len(g), st.median([r[1] for r in g]), min(r[1] for r in g), max(r[1] for r in g), st.median([r[3] for r in g]), st.median([r[4] for r in g])))
    v = [r for r in rows if r[2] == r[2]]
    print("  spearman(emit/btlbw, p95) = %.2f (n=%d)" % (spearman([r[2] for r in v], [min(r[1], 200) for r in v]), len(v)))
