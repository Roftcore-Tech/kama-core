# qd_ratio emulation: qd = min(RTT samples over K_QD*w_epoch) - rtprop; SR1 throttle needs qd_ratio>1500 permille
import sys, os, bisect
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
from emu import emulate
# re-run emulation capturing RTT samples by monkeypatching: simpler - recompute samples from emulate output is not stored; re-derive here
def rtt_samples(A):
    datapn = {}; seen = set(); last = {}
    out = []
    ev = []
    for t, ln, d in A["fwd_tx"]:
        if d and d["type"] == "data": ev.append((t, 0, d))
    for t, ln, d in A["ack_rx"]:
        if d and d["type"] == "ack" and "largest" in d: ev.append((t, 1, d))
    ev.sort(key=lambda e: (e[0], e[1]))
    infl = {}; low2 = {}
    for t, k, d in ev:
        if k == 0:
            o = "data" if d["unit"] not in seen else "re"; seen.add(d["unit"])
            infl[d["pn"]] = (t, o, d["unit"]); low2[d["pn"] & 0xFFFF] = d["pn"]; last[d["unit"]] = d["pn"]
        else:
            cand = None
            for l in ack_low16_set(d):
                pn = low2.get(l)
                if pn is None or pn not in infl: continue
                ts, o, u = infl.pop(pn)
                if o == "data" and last[u] == pn and (cand is None or pn > cand[0]): cand = (pn, ts)
            if cand: out.append((t, t - cand[1], d["ack_delay"]*256000))
    return out
for arm in ("taban", "aday"):
    nb = 0; nhigh = 0; runs_high = 0
    for i in range(1, 31):
        A = analyze(os.path.join(CELL, arm, "rep%d" % i)); R = A["res"]; T0 = A["T0"]
        S = emulate(A)
        smp = rtt_samples(A)
        offers = sorted(r["offer"] for r in R); ftx = sorted(r["first_tx"] for r in R if r["first_tx"])
        hi = 0
        for row in S:
            if row["rtprop"] is None: continue
            t = row["t"]
            wq = 2 * max(2*267/max(row["last_full"],1) + 2.097, 0.1) * 1e9
            vals = [max(s - ad, 0) if (s - ad) >= row["rtprop"] else s for (ts, s, ad) in smp if t - wq <= ts <= t]
            if not vals: continue
            qd = min(vals) - row["rtprop"]
            bl = bisect.bisect_right(offers, t) - bisect.bisect_right(ftx, t)
            if bl >= 1:
                nb += 1
                if qd * 1000 / row["rtprop"] > 1500: nhigh += 1; hi += 1
        runs_high += hi > 0
    print("%s: ACK events with backlog: %d; qd_ratio>1500 permille (SR1 can fire): %d (%.1f%%); runs with any: %d" % (arm, nb, nhigh, 100*nhigh/max(nb,1), runs_high))
