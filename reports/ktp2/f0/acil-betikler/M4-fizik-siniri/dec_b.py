"""Ölçülen B koşuları: koşu başına p95(L = rx−sunum), p95(S = ilk iletim−sunum), p95(N = rx−ilk iletim).
Ham veri: runs/tam/b/out/<b_cell>/<arm>/repN/{out.jsonl, rx_log.jsonl, capb_cli.csv.gz}. Salt okuma."""
import sys, os, json, gzip, math
sys.path.insert(0, sys.argv[1])
from sim import pct
ROOT = sys.argv[2]
cells=[]
for rid in ("r1k","r12k"):
  for did in ("d300","d1000"):
    for loss in (0,10,20,30):
      for p in (65,1000):
        cells.append((f"b_{rid}_{did}_l{loss}_p{p}",p))
def load(rep, pay, kama):
    try:
        lines=open(os.path.join(rep,"out.jsonl")).read().splitlines()
    except FileNotFoundError:
        return None
    meta=json.loads(lines[0])["meta"]
    t0u=meta["t0_unix_ns"]; off=meta.get("events_epoch_offset_ns",0)
    offer={}
    for l in lines[1:]:
        o=json.loads(l)
        e=o.get("event")
        if e: offer[e["msg_id"]]=(t0u+e["sent_ns"]-off)/1e9
    rx={}
    try:
        for l in open(os.path.join(rep,"rx_log.jsonl")).read().splitlines()[1:]:
            o=json.loads(l); u=o["u"]
            if u not in rx: rx[u]=o["t"]/1e9
    except FileNotFoundError:
        pass
    ftx={}; ntx={}
    if kama:
        ipl=20+8+16+11+pay
        fn=os.path.join(rep,"capb_cli.csv.gz")
        if os.path.exists(fn):
            with gzip.open(fn,"rt") as f:
                for line in f:
                    if line.startswith("#"): continue
                    a=line.rstrip("\n").split(",")
                    if len(a)<7 or a[1]!="o" or int(a[3])!=ipl: continue
                    x=a[6]
                    if not x.startswith("00"): continue
                    u=int(x[6:22],16); t=int(a[0])/1e9
                    ntx[u]=ntx.get(u,0)+1
                    if u not in ftx or t<ftx[u]: ftx[u]=t
    L=[rx[i]-offer[i] for i in offer if i in rx]
    S=[ftx[i]-offer[i] for i in offer if i in ftx and i in rx]
    N=[rx[i]-ftx[i] for i in offer if i in ftx and i in rx]
    return {"n":len(offer),"nrx":len(L),"L95":pct(L,.95),"S95":pct(S,.95) if S else None,"N95":pct(N,.95) if N else None,
            "L50":pct(L,.5),"S50":pct(S,.5) if S else None,"N50":pct(N,.5) if N else None,
            "copies":(sum(ntx.values())/len(ntx)) if ntx else None}
res=[]
for cell,pay in cells:
    for arm in ("taban","aday","quinn"):
        d=os.path.join(ROOT,cell,arm)
        if not os.path.isdir(d): continue
        rows=[]
        for rep in sorted(os.listdir(d)):
            r=load(os.path.join(d,rep),pay,arm!="quinn")
            if r and r["nrx"]>0: rows.append(r)
        def med(k):
            v=[r[k] for r in rows if r[k] is not None]
            return (pct(v,.5),pct(v,.25),pct(v,.75)) if v else (None,None,None)
        o={"cell":cell[2:],"arm":arm,"N":len(rows)}
        for k in ("L95","S95","N95","L50","S50","N50","copies"):
            o[k]=med(k)
        res.append(o); print(json.dumps(o),flush=True)
