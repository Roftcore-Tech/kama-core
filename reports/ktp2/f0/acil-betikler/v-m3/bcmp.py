import csv,gzip,json,os,math,sys
ROOT="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out"
B={(r["cell"],r["arm"],int(r["rep"])):r for r in csv.DictReader(gzip.open("/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz","rt"))}
def pct(vals,q):
    v=sorted(vals)
    if not v: return None
    if len(v)==1: return v[0]
    pos=q*(len(v)-1); lo=math.floor(pos); hi=min(lo+1,len(v)-1)
    if math.isinf(v[hi]) or math.isinf(v[lo]): return v[hi] if pos-lo>0 else v[lo]
    return v[lo]+(v[hi]-v[lo])*(pos-lo)
mis=0;tot=0;maxd=0
for cell in sys.argv[1:]:
    for arm in ("taban","aday","quinn","tcp"):
        ad=os.path.join(ROOT,cell,arm)
        if not os.path.isdir(ad): continue
        for rep in os.listdir(ad):
            k=(cell,arm,int(rep[3:]))
            if k not in B: continue
            ev=[json.loads(l)["event"] for l in open(os.path.join(ad,rep,"out.jsonl")) if '"event"' in l]
            n=100 if len(ev)<=100 else len(ev)
            conf_all=[(e["delivered_ns"]-e["sent_ns"])/1e9 if e["delivered_ns"] is not None else math.inf for e in ev]
            conf=[c for c in conf_all if math.isfinite(c)]
            D=float(B[k].get("deliv_D") or "nan")
            dl=60.0 if "p1000" in cell else (30.0)
            myD=sum(1 for c in conf if c<=dl)/len(conf_all)
            bp=B[k]["conf_p95_s"]; mp=pct(conf,.95)
            tot+=1
            okp = (bp in ("","None") and mp is None) or (bp not in ("","None") and (abs(float(bp)-mp)<1e-6*max(1,mp) if math.isfinite(mp) else math.isinf(float(bp))))
            if not okp or abs(D-myD)>1e-9:
                mis+=1
                if mis<6: print("MIS",k,bp,mp,D,myD)
print("tot",tot,"mis",mis)
