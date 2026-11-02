import json, os, sys, collections
root = sys.argv[1]
agg = collections.defaultdict(lambda: [0,0,0,0])  # runs, runs_par>0, max_par, sum_par
keys=None
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    for arm in ("taban","aday"):
        ad = os.path.join(cd, arm)
        if not os.path.isdir(ad): continue
        for rep in os.listdir(ad):
            f = os.path.join(ad, rep, "alice.jsonl")
            if not os.path.exists(f): continue
            last=None
            for l in open(f):
                if '"k": "ts"' in l or '"k":"ts"' in l:
                    last=l
            if last is None: continue
            o=json.loads(last)
            s=[x for x in o["s"] if x.get("peer")=="bob"]
            if not s: continue
            s=s[0]
            if keys is None: keys=list(s.keys())
            p=s.get("par",0) or 0
            a=agg[(cell,arm)]
            a[0]+=1; a[1]+= (p>0); a[2]=max(a[2],p); a[3]+=p
print(keys)
for k,v in sorted(agg.items()): print(k, "runs=%d par>0=%d max=%d sum=%d"%tuple(v))
