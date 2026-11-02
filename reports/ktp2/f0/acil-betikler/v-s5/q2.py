import sys, os, statistics
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ptt_reorder_v as m
def nr(xs,p):
    xs=sorted(xs); import math
    return xs[max(0,math.ceil(p*len(xs))-1)]
for name in sys.argv[1:]:
  for arm in ("taban","aday"):
    base=os.path.join(m.ROOT,name,arm)
    allit=[]; per=[]
    for rp in sorted(os.listdir(base)):
        if not rp.startswith("rep"): continue
        it,_=m.run(os.path.join(base,rp))
        if not it: continue
        allit+=it
        tot=[i["total"] for i in it]; ro=[i["reorder"] for i in it]
        per.append((nr(tot,.5),nr(tot,.95),nr(ro,.5),nr(ro,.95), statistics.median(tot)))
    tot=[i["total"] for i in allit]; ro=[i["reorder"] for i in allit]
    print(name,arm,"pooled tot p50 %.0f p95 %.0f ro p50 %.0f p95 %.0f"%(m.q(tot,.5),m.q(tot,.95),m.q(ro,.5),m.q(ro,.95)),
      "| med-of-run nearest-rank tot p50 %.0f p95 %.0f ro p50 %.0f p95 %.0f"%tuple(statistics.median([p[k] for p in per]) for k in range(4)))
