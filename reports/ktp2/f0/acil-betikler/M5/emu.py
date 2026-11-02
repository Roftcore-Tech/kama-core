# Engine estimator emulation from client-side capture (approximate; see notes)
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
from run1 import analyze
SIZE = 1027
K_BW = 8; UNIT_HINT = 267; K_RTPROP = 8; K_CONV = 4

def emulate(A):
    T0 = A["T0"]
    R = A["res"]
    offers = sorted(r["offer"] for r in R)
    first_tx = {}
    events = []
    for t, ln, d in A["fwd_tx"]:
        if d and d["type"] == "data":
            events.append((t, 0, d))
    for t, ln, d in A["ack_rx"]:
        if d and d["type"] == "ack" and "largest" in d:
            events.append((t, 1, d))
    events.sort(key=lambda e: (e[0], e[1]))
    delivered = 0; dtime = 0; fsent = 0; pif = 0
    inflight = {}  # pn -> dict
    low2pn = {}
    unit_last_tx = {}
    seen_unit = set()
    samples = []  # (t, bw)
    last_full = 0
    srtt = None; rtprop_s = []; last_rtt_t = None
    out = []  # time series rows
    sent_units = set()
    for t, kind, d in events:
        if kind == 0:
            pn = d["pn"]; u = d["unit"]
            origin = "data" if u not in seen_unit else "re"
            seen_unit.add(u)
            if pif == 0:
                fsent = t; dtime = t
            pif += 1
            sent_units.add(u)
            # app-limited approx: no offered-not-yet-sent unit after this send
            backlog = any(o <= t for o in offers) and sum(1 for o in offers if o <= t) > len(sent_units)
            inflight[pn] = dict(t=t, delivered=delivered, dtime=dtime, fsent=fsent, al=not backlog, origin=origin, unit=u)
            low2pn[pn & 0xFFFF] = pn
            unit_last_tx[u] = (t, pn)
        else:
            acked = ack_low16_set(d)
            rtt_cand = None
            for l in acked:
                pn = low2pn.get(l)
                if pn is None or pn not in inflight: continue
                p = inflight.pop(pn)
                superseded = unit_last_tx[p["unit"]][1] != pn
                delivered += SIZE; dtime = t; pif = max(0, pif - 1)
                if superseded:
                    continue  # credit only (approx of tombstone retx_count>0)
                ack_el = t - p["dtime"]; snd_el = p["t"] - p["fsent"]
                iv = max(ack_el, snd_el)
                fsent = p["t"]
                if iv >= 1e6:
                    bw = (delivered - p["delivered"]) * 1e9 / iv
                    cur = max((b for _, b in samples), default=0)
                    if (not p["al"]) or bw >= cur:
                        samples.append((t, bw))
                    if not p["al"]:
                        last_full = bw
                if p["origin"] == "data":
                    if rtt_cand is None or pn > rtt_cand[0]:
                        rtt_cand = (pn, p["t"])
            # window
            dbw = max(last_full, 1)
            w_epoch = max(2*UNIT_HINT/dbw + 2.097, 0.1)
            W = max(K_BW * w_epoch, 10.0) * 1e9
            last = samples[-1] if samples else None
            samples = [s for s in samples if s[0] >= t - W]
            if not samples and last: samples = [last]
            if rtt_cand:
                raw = t - rtt_cand[1]
                ad = d["ack_delay"] * 256 * 1000
                rtp = min((v for _, v in rtprop_s), default=None)
                c = raw - ad
                s = c if (c >= 0 and (rtp is None or c >= rtp)) else raw
                n = 1
                if rtp and last_rtt_t is not None:
                    n = int(min(max((t - last_rtt_t) // (rtp / K_CONV), 1), 64))
                last_rtt_t = t
                if srtt is None: srtt = s
                else:
                    for _ in range(n): srtt = (7*srtt + s) / 8
                rw = max(K_RTPROP * srtt, 10e9)
                rtprop_s.append((t, s))
                lastr = rtprop_s[-1]
                rtprop_s = [x for x in rtprop_s if x[0] >= t - rw] or [lastr]
            btl = max((b for _, b in samples), default=0)
            rtp = min((v for _, v in rtprop_s), default=None)
            out.append(dict(t=t, btlbw=btl, srtt=srtt, rtprop=rtp, last_full=last_full, W=W))
    return out

def at(series, t):
    # last row with row.t <= t
    lo, hi = 0, len(series)
    while lo < hi:
        m = (lo+hi)//2
        if series[m]["t"] <= t: lo = m+1
        else: hi = m
    return series[lo-1] if lo > 0 else None

if __name__ == "__main__":
    rep = sys.argv[1]
    A = analyze(rep); T0 = A["T0"]
    S = emulate(A)
    # emission rate in 2.5 s windows (engine bytes = 1027 per data pkt)
    tx = [(t, d) for t, ln, d in A["fwd_tx"] if d and d["type"] == "data"]
    R = A["res"]
    step = 2.5
    print(" t     emit_B/s  backlog  btlbw   srtt   rtprop  cwnd/srtt  1.0*bw  2.885*bw")
    for k in range(0, 30):
        a = T0 + k*step*1e9; b = a + step*1e9
        n = sum(1 for t, d in tx if a <= t < b)
        bl = sum(1 for r in R if r["offer"] <= b and (r["first_tx"] is None or r["first_tx"] > b))
        row = at(S, (a+b)/2)
        if row is None or row["srtt"] is None:
            print("%5.1f %7.0f %4d   (no est)" % (k*step, n*SIZE/step, bl)); continue
        bdp = row["btlbw"] * row["rtprop"]/1e9
        cwnd = max(2*bdp, 4800)
        cs = cwnd / (row["srtt"]/1e9)
        print("%5.1f %7.0f %4d  %6.0f  %5.2f  %5.2f   %6.0f   %6.0f  %6.0f" % (k*step, n*SIZE/step, bl, row["btlbw"], row["srtt"]/1e9, row["rtprop"]/1e9, cs, row["btlbw"], 2.885*row["btlbw"]))
