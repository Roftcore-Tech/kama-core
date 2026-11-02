import json, os, sys, math, re, statistics, random, pickle
ROOT = "/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out"
INF = float("inf")
def load(d):
    p = os.path.join(d, "out.jsonl")
    if not os.path.exists(p): return None
    meta=None; ev=[]
    for line in open(p):
        line=line.strip()
        if not line: continue
        o=json.loads(line)
        if "meta" in o: meta=o["meta"]
        elif "event" in o: ev.append(o["event"])
    rc = open(os.path.join(d,"rc")).read().strip() if os.path.exists(os.path.join(d,"rc")) else None
    return meta, ev, rc
res = {}
for cell in sorted(os.listdir(ROOT)):
    cd = os.path.join(ROOT, cell)
    if not os.path.isdir(cd) or not os.path.exists(os.path.join(cd,"DONE")): continue
    base = re.sub(r"_quinn$","",cell)
    for arm in sorted(os.listdir(cd)):
        ad = os.path.join(cd, arm)
        if not os.path.isdir(ad): continue
        for rep in sorted(os.listdir(ad)):
            if not re.fullmatch(r"rep\d+", rep): continue
            x = load(os.path.join(ad, rep))
            if x is None:
                res.setdefault(base,{}).setdefault(arm,[]).append(dict(rep=rep, ok=False)); continue
            meta, ev, rc = x
            n = meta["msg_count"] if meta else 100
            D = meta["max_deadline_ns"]/1e9 if meta else 60.0
            lat=[]
            for e in ev:
                dn=e.get("delivered_ns")
                lat.append(INF if dn is None else max(0,dn-e["sent_ns"])/1e9)
            lat += [INF]*(n-len(lat))
            lat.sort()
            k = max(1, math.ceil(0.95*len(lat)))
            p95 = lat[k-1]
            cD = sum(1 for v in lat if v<=D)/n
            res.setdefault(base,{}).setdefault(arm,[]).append(dict(rep=rep, ok=(rc=="0"), p95=p95, p95c=min(p95,D), cD=cD, D=D, n=n, rc=rc, sent=[e["sent_ns"] for e in ev], ids=[e["msg_id"] for e in ev]))
pickle.dump(res, open(sys.argv[1],"wb"))
for cell in res:
    s=[]
    for arm in ("taban","aday","quinn","tcp"):
        runs=res[cell].get(arm,[])
        if not runs: s.append(f"{arm}:-"); continue
        D=runs[0].get("D",60)
        vals=[(r["p95c"] if r.get("ok") else D) for r in runs]
        s.append(f"{arm}:N={len(runs)} p95med={statistics.median(vals):.2f} cDmed={statistics.median([r.get('cD',0) if r.get('ok') else 0 for r in runs]):.2f}")
    print(cell, " | ".join(s))
