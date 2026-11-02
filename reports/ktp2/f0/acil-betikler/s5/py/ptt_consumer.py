import os, sys, statistics, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import f0a_analyze_copy as A
root=sys.argv[1]
def q(v,p):
    v=sorted(x for x in v if x is not None)
    return v[max(0,math.ceil(p*len(v))-1)] if v else None
for cell in sys.argv[2].split(","):
  for arm in ("taban","aday"):
    ad=os.path.join(root,cell,arm); c95=[]; a95=[]; nrd=[]
    for rep in sorted(os.listdir(ad)):
        if not rep.startswith("rep"): continue
        res,rows=A.analyze_run(os.path.join(ad,rep),cell,arm,rep)
        P=[r for r in rows if r["c"]=="P"]
        c95.append(q([r["consumer"] for r in P],0.95)); a95.append(q([r["adapter"] for r in P],0.95))
        nrd.append((res.get("counters") or {}).get("bob_not_ready_drops"))
    print(cell,arm,"consumer_p95_med_ms",statistics.median([x for x in c95 if x is not None]),"adapter_p95_med_ms",statistics.median([x for x in a95 if x is not None]),"not_ready_drops_med",statistics.median([x for x in nrd if x is not None]))
