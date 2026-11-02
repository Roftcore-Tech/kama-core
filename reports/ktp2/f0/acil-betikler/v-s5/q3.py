import sys, os, statistics, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ptt_reorder_v as m
def nr(xs,p):
    xs=sorted(xs); return xs[max(0,math.ceil(p*len(xs))-1)]
for name in sys.argv[1:]:
  for arm in ("taban","aday"):
    base=os.path.join(m.ROOT,name,arm)
    per=[]; sums=[0,0,0,0]
    for rp in sorted(os.listdir(base)):
        if not rp.startswith("rep"): continue
        rd=os.path.join(base,rp)
        it,_=m.run(rd)
        if not it: continue
        # per-stream counterfactual: release = max drained over PTT-class units (O,P,C) with unit<=u
        a=m.load(os.path.join(rd,"alice.jsonl")); b=m.load(os.path.join(rd,"bob.jsonl"))
        sends=sorted((d for d in a if d.get("k")=="send"), key=lambda d:d["t"])
        D={}
        for d in b:
            if d.get("k")=="tr":
                for r in d["r"]:
                    if r[0]=="D" and r[2] not in D: D[r[2]]=r[4]
        pcls=[k for k,s in enumerate(sends) if s["c"] in ("O","P","C")]
        run=-1; rel_ps={}
        for k in pcls:
            run=max(run,D.get(k,float('inf'))); rel_ps[k]=run
        pre=[i["drained"]-i["t_send"] for i in it]
        ro=[i["reorder"] for i in it]
        ps=[rel_ps[i["unit"]]-i["drained"] for i in it]
        tot=[i["total"] for i in it]
        sums[0]+=sum(ro); sums[1]+=sum(ps); sums[2]+=sum(pre); sums[3]+=sum(tot)
        per.append((nr(pre,.5),nr(ro,.5),nr(tot,.5),nr(pre,.95),nr(ro,.95),nr(tot,.95),
                    sum(1 for i,x in zip(it,ps) if i["drained"]-i["t_send"]+x<=800)/len(it)))
    med=[statistics.median([p[k] for p in per]) for k in range(7)]
    print(name,arm,"p50 pre=%.0f ro=%.0f tot=%.0f | p95 pre=%.0f ro=%.0f tot=%.0f | perstream le800=%.3f"%tuple(med),
          "| sum ro/tot=%.3f pre/tot=%.3f perstream_ro/ro=%.3f"%(sums[0]/sums[3],sums[2]/sums[3],sums[1]/sums[0]))
