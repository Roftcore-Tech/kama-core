import sys, os, re, statistics as st
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def port_of(rep):
    s = open(os.path.join(rep, "cmd.txt")).read()
    return int(re.search(r"--bind [0-9.]+:(\d+)", s).group(1))
rows = []
series = {}
for i in range(1, 31):
    rep = os.path.join(CELL, "quinn", "rep%d" % i)
    meta, ev = read_out(rep); rx = read_rx(rep); P = port_of(rep)
    t0u = meta["t0_unix_ns"]; off = meta["events_epoch_offset_ns"]
    offer = {e["msg_id"]: t0u + e["sent_ns"] - off for e in ev}
    conf = {e["msg_id"]: t0u + e["delivered_ns"] - off for e in ev if e["delivered_ns"] is not None}
    cli = read_cap(os.path.join(rep, "capb_cli.csv.gz"))
    fwd = [(t, ln) for (t, d, pr, ln, sp, dp, x) in cli if d == "o" and dp == P]
    # quinn: rx u is msg_id?
    rxl = [(rx[u]-offer[u])/1e9 for u in offer if u in rx]
    cfl = [(conf[u]-offer[u])/1e9 for u in conf]
    # emission: bytes after t0 in 5s bins (ip_len+14)
    bins = {}
    for t, ln in fwd:
        if t < t0u: continue
        b = int((t - t0u)/5e9)
        bins[b] = bins.get(b, 0) + ln + 14
    tlast = (max(t for t, _ in fwd) - t0u)/1e9
    pre = sum(ln+14 for t, ln in fwd if t < t0u)
    rows.append((i, len(rxl), pct(rxl,.5), pct(rxl,.95), pct(cfl,.95), len(cfl), tlast, pre))
    series[i] = bins
print("rep n rx_p50 rx_p95 conf_p95 nconf t_last_tx pre_t0_bytes")
for r in rows: print("%2d %3d %6.2f %6.2f %6.2f %3d %6.2f %d" % r)
print("MED rx_p95=%.2f conf_p95=%.2f rx_p50=%.2f" % (st.median([r[3] for r in rows]), st.median([r[4] for r in rows]), st.median([r[2] for r in rows])))
print("emission B/s per 5 s bin (median over reps):")
for b in range(0, 10):
    v = [series[i].get(b, 0)/5.0 for i in series]
    print(" %2d-%2d s: med %7.0f  q1 %7.0f q3 %7.0f" % (b*5, b*5+5, st.median(v), pct(v,.25), pct(v,.75)))
