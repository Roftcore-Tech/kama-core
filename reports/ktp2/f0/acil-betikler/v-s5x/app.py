import os, sys, json, math, statistics
def q(v,p):
    v=sorted(v); return v[max(0,math.ceil(p*len(v))-1)] if v else None
root=sys.argv[1]
for cell in sys.argv[2].split(","):
  for arm in ("taban","aday"):
    ad=os.path.join(root,cell,arm); a95=[]; nrd=[]; nrdmax=0
    for rep in sorted(os.listdir(ad)):
        if not rep.startswith("rep"): continue
        ad_=[]; last=None
        for l in open(os.path.join(ad,rep,"alice.jsonl")):
            if '"k":"send"' in l:
                d=json.loads(l)
                if d["c"]=="P" and d.get("tr") is not None: ad_.append(d["tr"]-d["t"])
        for l in open(os.path.join(ad,rep,"bob.jsonl")):
            if '"k":"ts"' in l: last=json.loads(l)
        a95.append(q(ad_,.95)); nrd.append(last["nrd"]); nrdmax=max(nrdmax,last["nrd"])
    print(cell,arm,"adapter(P: send.tr-send.t) p95 med ms",round(statistics.median(a95),3),"range",round(min(a95),3),round(max(a95),3),"| bob nrd med",statistics.median(nrd),"max",nrdmax)
