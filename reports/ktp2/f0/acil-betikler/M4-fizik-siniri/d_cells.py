import sys, os, json
sys.path.insert(0, sys.argv[1])
from sim import run, pct
ROOT=sys.argv[2]
def meas(rep):
    lines=open(os.path.join(rep,"out.jsonl")).read().splitlines()
    m=json.loads(lines[0])["meta"]; t0=m["t0_unix_ns"]; off=m.get("events_epoch_offset_ns",0)
    offer={}
    for l in lines[1:]:
        e=json.loads(l).get("event")
        if e: offer[e["msg_id"]]=(t0+e["sent_ns"]-off)/1e9
    rx={}
    for l in open(os.path.join(rep,"rx_log.jsonl")).read().splitlines()[1:]:
        o=json.loads(l)
        rx.setdefault(o["u"],o["t"]/1e9)
    v=[rx[i]-offer[i] for i in offer if i in rx]
    return pct(v,.95) if v else None
for name,C,d,s,q,I,M,p in (("d_cc_r12k_d1000_l10",12500,1.0,0.2,0.1,0.2,150,1000),("d_cc_r12k_d300_l30",12500,0.3,0.06,0.3,0.2,150,1000)):
    mod={}
    for pol,kw in (("oracle",{}),("rfc",{}),("fec",{"K":8,"R":2 if q<0.25 else 4,"E":1024}),("fec",{"K":4,"R":1 if q<0.15 else 4,"E":1024})):
        v=[]
        for seed in range(1,61):
            lat,_,_=run(C,d,s,q,I,M,p,pol,seed=seed*31+7,**kw)
            v.append(pct([x for x in lat if x is not None],.95))
        mod[pol+str(kw.get("K",""))]=round(pct(v,.5),2)
    ms={}
    for arm in ("taban","quinn","quinn_cubic","tcp"):
        dd=os.path.join(ROOT,name,arm)
        if not os.path.isdir(dd): continue
        vals=[meas(os.path.join(dd,r)) for r in sorted(os.listdir(dd)) if os.path.exists(os.path.join(dd,r,"rx_log.jsonl"))]
        vals=[x for x in vals if x is not None]
        ms[arm]=(round(pct(vals,.5),2),len(vals)) if vals else None
    print(name,"model",mod,"ölçülen rx p95 (med, n)",ms)
