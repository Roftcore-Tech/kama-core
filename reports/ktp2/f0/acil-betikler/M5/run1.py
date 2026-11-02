import sys, os, re
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *

def port_of(rep):
    s = open(os.path.join(rep, "cmd.txt")).read()
    return int(re.search(r"--bind [0-9.]+:(\d+)", s).group(1))

def analyze(rep, verbose=False):
    meta, ev = read_out(rep)
    rx = read_rx(rep)
    P = port_of(rep)
    t0u = meta["t0_unix_ns"]; off = meta["events_epoch_offset_ns"]
    cli = read_cap(os.path.join(rep, "capb_cli.csv.gz"))
    srv = read_cap(os.path.join(rep, "capb_srv.csv.gz"))
    fwd_tx = [(t, ln, kdecode(x)) for (t, d, pr, ln, sp, dp, x) in cli if d == "o" and dp == P and pr == 17]
    ack_rx = [(t, ln, kdecode(x)) for (t, d, pr, ln, sp, dp, x) in cli if d == "i" and sp == P and pr == 17]
    fwd_rx = [(t, ln, kdecode(x)) for (t, d, pr, ln, sp, dp, x) in srv if d == "i" and dp == P and pr == 17]
    ack_tx = [(t, ln, kdecode(x)) for (t, d, pr, ln, sp, dp, x) in srv if d == "o" and sp == P and pr == 17]
    T0 = t0u
    offer = {e["msg_id"]: t0u + e["sent_ns"] - off for e in ev}
    conf = {e["msg_id"]: (t0u + e["delivered_ns"] - off) if e["delivered_ns"] is not None else None for e in ev}
    txs = {}
    pn_tx = {}
    for t, ln, d in fwd_tx:
        if d is None: continue
        pn_tx[d["pn"]] = (t, ln, d)
        if d["type"] == "data":
            txs.setdefault(d["unit"], []).append((t, d["pn"], ln))
    arr = {}
    pn_arr = {}
    for t, ln, d in fwd_rx:
        if d is None: continue
        pn_arr.setdefault(d["pn"], t)
        if d["type"] == "data":
            arr.setdefault(d["unit"], []).append((t, d["pn"]))
    res = []
    for u in sorted(offer):
        o = offer[u]
        tl = txs.get(u, [])
        al = sorted(arr.get(u, []))
        first_tx = tl[0][0] if tl else None
        fa = al[0] if al else None
        # which attempt succeeded
        att = None
        if fa:
            for i, (t, pn, ln) in enumerate(tl):
                if pn == fa[1]: att = i; break
        r = rx.get(u)
        res.append(dict(u=u, offer=o, first_tx=first_tx, ntx=len(tl), first_arr=fa[0] if fa else None,
                        att=att, rx=r, conf=conf[u], txt=[t for t, _, _ in tl]))
    return dict(meta=meta, res=res, fwd_tx=fwd_tx, ack_rx=ack_rx, fwd_rx=fwd_rx, ack_tx=ack_tx, T0=T0, pn_tx=pn_tx, pn_arr=pn_arr)

if __name__ == "__main__":
    rep = sys.argv[1]
    A = analyze(rep)
    T0 = A["T0"]
    s = lambda x: (x - T0)/1e9 if x is not None else float('nan')
    print("u  offer  firstTx  wait  ntx att  firstArr  rx   rx-offer  conf-offer")
    for r in A["res"]:
        print("%3d %6.2f %6.2f %6.2f %2d %s %6.2f %6.2f %6.2f %6.2f" % (r["u"], s(r["offer"]), s(r["first_tx"]),
              (r["first_tx"]-r["offer"])/1e9 if r["first_tx"] else float('nan'), r["ntx"], r["att"],
              s(r["first_arr"]), s(r["rx"]), (r["rx"]-r["offer"])/1e9 if r["rx"] else float('nan'),
              (r["conf"]-r["offer"])/1e9 if r["conf"] else float('nan')))
