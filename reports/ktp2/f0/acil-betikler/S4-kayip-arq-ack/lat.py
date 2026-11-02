import sys, json, pathlib
root = pathlib.Path(sys.argv[1])
def q(v,p):
    v=sorted(v); 
    return v[max(0,min(len(v)-1,int(round(p*(len(v)-1)))))] if v else float('nan')
for c in sys.argv[2:]:
    for a in ("taban","aday","quinn","tcp"):
        lat=[]; nd=0; nt=0
        for rep in (root/c/a).glob("rep*"):
            f=rep/"out.jsonl"
            if not f.exists(): continue
            for line in f.open():
                d=json.loads(line)
                e=d.get("event")
                if not e: continue
                nt+=1
                if e.get("delivered_ns") is not None:
                    nd+=1; lat.append((e["delivered_ns"]-e["sent_ns"])/1e9)
        print(f"{c} {a:6s} n={nt} D={nd/max(nt,1):.3f} p50={q(lat,.5):.2f} p70={q(lat,.7):.2f} p80={q(lat,.8):.2f} p90={q(lat,.9):.2f} p95={q(lat,.95):.2f}")
