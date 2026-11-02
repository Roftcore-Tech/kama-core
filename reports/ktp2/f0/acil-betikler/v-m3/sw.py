import json, os, sys, gzip, math, statistics, re
ROOT="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out"
cell=sys.argv[1]
def pct(vals,q):
    v=sorted(x for x in vals if x is not None and math.isfinite(x))
    if not v: return None
    if len(v)==1: return v[0]
    pos=q*(len(v)-1); lo=math.floor(pos); hi=min(lo+1,len(v)-1)
    return v[lo]+(v[hi]-v[lo])*(pos-lo)
def nr(vals,q):
    v=sorted(vals); k=max(1,math.ceil(q*len(v))); return v[k-1]
def load(d):
    meta=None; ev={}
    for line in open(os.path.join(d,"out.jsonl")):
        o=json.loads(line)
        if "meta" in o: meta=o["meta"]
        else: e=o["event"]; ev[e["msg_id"]]=e
    return meta, ev
def rxlog(d):
    first={}
    p=os.path.join(d,"rx_log.jsonl")
    if not os.path.exists(p): return None
    lines=open(p).read().splitlines()
    for l in lines[1:]:
        o=json.loads(l); u=o.get("u")
        if u is None or u>=2**64-1: continue
        if u not in first or o["t"]<first[u]: first[u]=o["t"]
    return first
out={}
for arm in ("taban","aday","quinn"):
    ad=os.path.join(ROOT,cell,arm)
    rows=[]
    for rep in sorted(os.listdir(ad), key=lambda x:int(x[3:])):
        d=os.path.join(ad,rep)
        meta,ev=load(d)
        n=meta["msg_count"]; t0=meta["t0_unix_ns"]; off=meta.get("events_epoch_offset_ns",0) or 0
        if arm=="quinn": off=0
        offer={i:t0+ev[i]["sent_ns"]-off for i in ev}
        conf=[(ev[i]["delivered_ns"]-ev[i]["sent_ns"])/1e9 if ev[i]["delivered_ns"] is not None else math.inf for i in range(n)]
        rx=rxlog(d)
        ow={i:(rx[i]-offer[i])/1e9 for i in range(n) if rx and i in rx}
        r=dict(rep=rep, conf_p95=nr(conf,.95), conf_p50=nr(conf,.5))
        if arm!="quinn":
            ftx={}
            with gzip.open(os.path.join(d,"capb_cli.csv.gz"),"rt") as f:
                for line in f:
                    if line.startswith("#"): continue
                    p=line.rstrip("\n").split(",")
                    if p[1]!="o" or p[2]!="17": continue
                    x=p[6]
                    if not x.startswith("00") or len(x)<22: continue
                    u=int(x[6:22],16)
                    t=int(p[0])
                    if u not in ftx or t<ftx[u]: ftx[u]=t
            sw={i:(ftx[i]-offer[i])/1e9 for i in range(n) if i in ftx}
            r["sw_p50"]=pct(list(sw.values()),.5); r["sw_p95"]=pct(list(sw.values()),.95)
            r["sw_nr95"]=nr(list(sw.values()),.95)
            # conf minus sw
            cm=[conf[i]-sw[i] if i in sw else math.inf for i in range(n)]
            r["confmsw_p95"]=nr(cm,.95)
            r["sw_by"]=sw
            r["neg_sw"]=sum(1 for v in sw.values() if v< -0.001)
            r["maxdev_plan"]=max(abs(ev[i]["sent_ns"]-off-i*meta["submit_interval_ns"])/1e6 for i in range(n))
        else:
            r["maxdev_plan"]=max(abs(ev[i]["sent_ns"]-i*285_000_000)/1e6 for i in range(n))
        r["ow_by"]=ow
        r["ow_p95"]=nr([ow.get(i,math.inf) for i in range(n)],.95)
        h2=[ow.get(i,math.inf) for i in range(n//2,n)]
        r["ow2_p95"]=nr(h2,.95)
        # sender confirm - receiver delivery
        r["s_minus_r"]=statistics.median([conf[i]-ow[i] for i in range(n) if i in ow and math.isfinite(conf[i])])
        rows.append(r)
    out[arm]=rows
    med=lambda k: statistics.median([r[k] for r in rows if r.get(k) is not None])
    s=f"{arm}: N={len(rows)} conf_p95 med={med('conf_p95'):.2f} ow_p95={med('ow_p95'):.2f} ow2_p95={med('ow2_p95'):.2f} s-r={med('s_minus_r'):.3f} maxdev_plan_ms={max(r['maxdev_plan'] for r in rows):.2f}"
    if arm!="quinn":
        s+=f" sw_p50={med('sw_p50'):.2f} sw_p95={med('sw_p95'):.2f} sw_nr95={med('sw_nr95'):.2f} conf-sw p95={med('confmsw_p95'):.2f} neg_sw={sum(r['neg_sw'] for r in rows)}"
    print(s)
    # ow p50 by message index (median across runs) deciles
    dec=[]
    for a in range(0,100,10):
        vals=[]
        for r in rows:
            v=[r["ow_by"].get(i,math.inf) for i in range(a,a+10)]
            vals.append(statistics.median(v))
        dec.append(statistics.median(vals))
    print("  ow p50 by decile:", " ".join(f"{v:.1f}" for v in dec))
    if arm!="quinn":
        dec=[]
        for a in range(0,100,10):
            vals=[statistics.median([r["sw_by"].get(i,math.inf) for i in range(a,a+10)]) for r in rows]
            dec.append(statistics.median(vals))
        print("  sw p50 by decile:", " ".join(f"{v:.1f}" for v in dec))
print("--- tail share")
for arm in ("taban","aday"):
    shares=[]; tails=[]
    for r in out[arm]:
        pass
