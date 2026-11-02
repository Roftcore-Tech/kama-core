import os, sys, json, bisect, math, statistics
def load(p): return [json.loads(l) for l in open(p) if l.strip()]
def q(v,p):
    v=sorted(v); return v[max(0,math.ceil(p*len(v))-1)] if v else None
root=sys.argv[1]
for cell in sys.argv[2].split(","):
  for arm in ("taban","aday"):
    ad=os.path.join(root,cell,arm); allc=[]; per95=[]; missD=0; missR=0; nP=0
    for rep in sorted(os.listdir(ad)):
        if not rep.startswith("rep"): continue
        A=load(os.path.join(ad,rep,"alice.jsonl")); B=load(os.path.join(ad,rep,"bob.jsonl"))
        S=sorted([r for d in A if d["k"]=="tr" for r in d["r"] if r[0]=="S"],key=lambda r:r[4]); at=[r[4] for r in S]
        key2u={}
        for s in (d for d in A if d["k"]=="send"):
            j=bisect.bisect_left(at,s["tr"]); b=min([k for k in (j-1,j,j+1) if 0<=k<len(at)],key=lambda k:abs(at[k]-s["tr"]))
            key2u[(s["c"],s.get("sp"),s.get("i"))]=S[b][2]
        D={}; Rel=[]
        for d in B:
            if d["k"]=="tr":
                for r in d["r"]:
                    if r[0]=="D" and r[2] not in D: D[r[2]]=r[4]
                    elif r[0]=="R": Rel.append((r[4],r[2]))
        Rel.sort(); rel={}; mx=0
        for a,n in Rel:
            if n>mx:
                for u in range(mx,n): rel.setdefault(u,a)
                mx=n
        cs=[]
        for d in B:
            if d["k"]=="rx" and d["c"]=="P" and d.get("first",True):
                u=key2u.get(("P",d.get("sp"),d.get("i")))
                nP+=1
                if u not in D: missD+=1
                if u not in rel: missR+=1; continue
                cs.append(d["t"]-rel[u])
        allc+=cs
        if cs: per95.append(q(cs,.95))
    print(cell,arm,"nP",nP,"missD",missD,"missR",missR,"cons p50/p95/max pooled",q(allc,.5),q(allc,.95),max(allc),"run-p95 median",statistics.median(per95), "n>100ms",sum(1 for c in allc if c>100))
