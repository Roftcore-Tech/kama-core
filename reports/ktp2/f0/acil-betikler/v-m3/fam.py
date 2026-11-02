import pickle, sys, statistics, random, csv, math
res = pickle.load(open(sys.argv[1],"rb"))
def p95c(r):
    D=r.get("D",60.0)
    return r["p95c"] if r.get("ok") else D
prim=[c for c in sorted(res) if c.startswith("b_") and not any(f"_l{s}_" in c for s in (50,80))]
B=10000
for x,y in (("taban","quinn"),("aday","quinn")):
    fam=[c for c in prim if x in res[c] and y in res[c] and len(res[c][x])>=30 and len(res[c][y])>=30]
    m=len(fam)
    for seed in (12345, 999):
        rng=random.Random(seed)
        cnt={"d":[0,0,0],"b":[0,0,0]}; border=[]
        for c in fam:
            a=[p95c(r) for r in res[c][x]]; b=[p95c(r) for r in res[c][y]]
            diffs=sorted(statistics.median(rng.choices(a,k=len(a)))-statistics.median(rng.choices(b,k=len(b))) for _ in range(B))
            for tag,al in (("d",0.05),("b",0.05/m)):
                lo=diffs[int(al/2*B)]; hi=diffs[min(B-1,int((1-al/2)*B))]
                i = 1 if lo>0 else (0 if hi<0 else 2)
                cnt[tag][i]+=1
                if tag=="b" and abs(lo)<0.05*abs(statistics.median(a)-statistics.median(b)+1e-9): border.append((c,lo,hi))
        print(x,y,"m",m,"seed",seed,"duz K/R/=",cnt["d"],"bonf",cnt["b"])
# compare with D's kosular.csv lat_p95
D={}
for row in csv.DictReader(open("/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/d/tam/kosular.csv")):
    D[(row["cell"],row["arm"],int(row["rep"]))]=row
mis=0; tot=0; maxd=0
for c in res:
    for arm,runs in res[c].items():
        for r in runs:
            k=(c,arm,int(r["rep"][3:]))
            if k not in D or "p95" not in r: continue
            v=D[k]["lat_p95_s"]
            dv=float("inf") if v=="inf" else (float(v) if v else float("nan"))
            tot+=1
            if not (dv==r["p95"] or (math.isfinite(dv) and abs(dv-r["p95"])<1e-4*max(1,r["p95"]))):
                mis+=1
                if mis<5: print("MIS",k,dv,r["p95"])
            cd=float(D[k]["cD_ss"]) if D[k]["cD_ss"] else float("nan")
            if abs(cd-r["cD"])>1e-9: print("cD MIS",k,cd,r["cD"])
print("D kosular match: tot",tot,"mis",mis)
