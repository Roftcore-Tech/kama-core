# Bagimsiz yeniden hesap: B birincil 32 hucre, kol basina kosu p95(L,S,N) -> medyan.
import sys, os, json, gzip, statistics
ROOT = sys.argv[1]
def pct(v, q):
    v = sorted(v)
    if not v: return None
    pos = q*(len(v)-1); lo = int(pos); hi = min(lo+1, len(v)-1)
    return v[lo] + (v[hi]-v[lo])*(pos-lo)
def load(rep, pay, kama):
    try: lines = open(os.path.join(rep, "out.jsonl")).read().splitlines()
    except FileNotFoundError: return None
    meta = json.loads(lines[0])["meta"]
    t0 = meta["t0_unix_ns"]; off = meta.get("events_epoch_offset_ns", 0)
    offer = {}; deliv_cli = {}
    for l in lines[1:]:
        e = json.loads(l).get("event")
        if e:
            offer[e["msg_id"]] = t0 + e["sent_ns"] - off
            if e.get("delivered_ns"): deliv_cli[e["msg_id"]] = e["delivered_ns"] - e["sent_ns"]
    rx = {}
    p = os.path.join(rep, "rx_log.jsonl")
    if os.path.exists(p):
        for l in open(p).read().splitlines()[1:]:
            o = json.loads(l)
            if "u" in o and o["u"] not in rx: rx[o["u"]] = o["t"]
    ftx = {}
    if kama:
        fn = os.path.join(rep, "capb_cli.csv.gz")
        ipl = 20+8+16+11+pay
        if os.path.exists(fn):
            with gzip.open(fn, "rt") as f:
                for line in f:
                    if line.startswith("#"): continue
                    a = line.rstrip("\n").split(",")
                    if len(a) < 7 or a[1] != "o" or a[2] != "17" or int(a[3]) != ipl: continue
                    x = a[6]
                    if x[:2] != "00": continue
                    u = int(x[6:22], 16); t = int(a[0])
                    if u not in ftx or t < ftx[u]: ftx[u] = t
    ids = sorted(offer)
    L = [(rx[i]-offer[i])/1e9 for i in ids if i in rx]
    S = [(ftx[i]-offer[i])/1e9 for i in ids if i in rx and i in ftx]
    N = [(rx[i]-ftx[i])/1e9 for i in ids if i in rx and i in ftx]
    Lc = [deliv_cli[i]/1e9 for i in ids if i in deliv_cli]
    return dict(n=len(ids), nrx=len(L), nftx=len(ftx), L95=pct(L,.95), S95=pct(S,.95), N95=pct(N,.95), Lc95=pct(Lc,.95),
                setup_in=meta.get("setup_in_window"), setup_ns=meta.get("setup_ns"), parity=(meta.get("activation") or {}).get("parity_emitted"))
out = []
for rid in ("r1k","r12k"):
  for did in ("d300","d1000"):
    for loss in (0,10,20,30):
      for p in (65,1000):
        cell = f"b_{rid}_{did}_l{loss}_p{p}"
        for arm in ("taban","aday","quinn"):
            d = os.path.join(ROOT, cell, arm)
            if not os.path.isdir(d): continue
            rows = []
            for rep in sorted(os.listdir(d)):
                r = load(os.path.join(d, rep), p, arm != "quinn")
                if r and r["nrx"] > 0: rows.append(r)
            def med(k):
                v = [r[k] for r in rows if r[k] is not None]
                return round(statistics.median(v),3) if v else None
            o = dict(cell=cell[2:], arm=arm, N=len(rows), L95=med("L95"), S95=med("S95"), N95=med("N95"), Lc95=med("Lc95"),
                     full=sum(1 for r in rows if r["nrx"]==r["n"]), setup_in=sorted({str(r["setup_in"]) for r in rows}),
                     setup_med_s=round(statistics.median([r["setup_ns"] for r in rows if r["setup_ns"] is not None])/1e9,3) if rows else None,
                     parity=sum((r["parity"] or 0) for r in rows))
            out.append(o); print(json.dumps(o), flush=True)
