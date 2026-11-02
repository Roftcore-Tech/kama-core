import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
rep = sys.argv[1]; tmax = float(sys.argv[2])
A = analyze(rep); T0 = A["T0"]
ev = []
for t, ln, d in A["fwd_tx"]: ev.append((t, "C>", ln, d))
for t, ln, d in A["fwd_rx"]: ev.append((t, ">S", ln, d))
for t, ln, d in A["ack_tx"]: ev.append((t, "S<", ln, d))
for t, ln, d in A["ack_rx"]: ev.append((t, "<C", ln, d))
ev.sort(key=lambda e: e[0])
for t, k, ln, d in ev:
    s = (t - T0)/1e9
    if s > tmax: break
    if d is None: print("%8.3f %s %4d ?" % (s, k, ln)); continue
    extra = ""
    if "unit" in d: extra = "u=%d seq=%d" % (d["unit"], d["seq"])
    if d["type"] == "ack" and "largest" in d: extra = "L=%d b0=%d r=%s delay=%d seen=%s" % (d["largest"], d["b0"], d["ranges"], d["ack_delay"], d.get("seen"))
    print("%8.3f %s %4d %-9s pn=%d %s" % (s, k, ln, d["type"], d["pn"], extra))
