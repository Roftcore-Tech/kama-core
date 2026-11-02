import sys, os, statistics, math, json
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ptt_reorder_v as m
def nr(xs,p):
    xs=sorted(xs); return xs[max(0,math.ceil(p*len(xs))-1)]
for name in sys.argv[1:]:
  for arm in ("taban","aday"):
    base=os.path.join(m.ROOT,name,arm)
    ad=[];co=[];per_ad=[];per_co=[];nrd=[]
    for rp in sorted(os.listdir(base)):
        if not rp.startswith("rep"): continue
        rd=os.path.join(base,rp)
        a=m.load(os.path.join(rd,"alice.jsonl")); b=m.load(os.path.join(rd,"bob.jsonl"))
        S=[r for d in a if d.get("k")=="tr" for r in d["r"] if r[0]=="S"]
        S.sort(key=lambda r:r[2])
        sends=sorted((d for d in a if d.get("k")=="send"), key=lambda d:d["t"])
        it,_=m.run(rd)
        pa=[]; pc=[]
        for i in it:
            u=i["unit"]; pa.append(S[u][4]-i["t_send"]); pc.append(i["rx"]-i["released"])
        per_ad.append(nr(pa,.95)); per_co.append(nr(pc,.95)); ad+=pa; co+=pc
        last=[d for d in b if d.get("k")=="ts"][-1]; nrd.append(last.get("nrd",0))
    print(name,arm,"adapter p95 med-of-run %.3f pooled %.3f | consumer p95 med-of-run %.3f pooled %.3f | nrd median %s max %s"%(
        statistics.median(per_ad), nr(ad,.95), statistics.median(per_co), nr(co,.95), statistics.median(nrd), max(nrd)))
