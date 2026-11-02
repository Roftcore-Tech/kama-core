import os, sys, json, collections, statistics
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import f0a_analyze_copy as A
root = sys.argv[1]
cells = sys.argv[2].split(",")
def med(v):
    v=[x for x in v if x is not None]
    return statistics.median(v) if v else None
def q(v,p):
    v=sorted(x for x in v if x is not None)
    if not v: return None
    import math
    return v[max(0,math.ceil(p*len(v))-1)]
out={}
for cell in cells:
    for arm in ("taban","aday"):
        ad=os.path.join(root,cell,arm)
        if not os.path.isdir(ad): continue
        per_run=[]
        blk_w=collections.Counter(); blk_n=collections.Counter()
        pooled_tot=[]; pooled_pre=[]; pooled_reo=[]
        for rep in sorted(os.listdir(ad)):
            rd=os.path.join(ad,rep)
            if not rep.startswith("rep"): continue
            res, rows = A.analyze_run(rd, cell, arm, rep)
            P=[r for r in rows if r["c"]=="P"]
            if not P: continue
            tot=[r["total"] for r in P]
            pre=[(r["t_drained"]-r["t_send"]) if (r.get("t_drained") is not None) else None for r in P]
            reo=[r["reorder"] for r in P]
            n=len(P)
            w800=sum(1 for t in tot if t is not None and t<=800)/n
            w800pre=sum(1 for t in pre if t is not None and t<=800)/n
            w2000=sum(1 for t in tot if t is not None and t<=2000)/n
            w2000pre=sum(1 for t in pre if t is not None and t<=2000)/n
            held=sum(1 for x in reo if x is not None and x>50)/n
            for r in P:
                if r["reorder"] is not None and r["reorder"]>50 and r["blk_c"]:
                    blk_w[r["blk_c"]]+=r["reorder"]; blk_n[r["blk_c"]]+=1
            pooled_tot+=tot; pooled_pre+=pre; pooled_reo+=reo
            per_run.append(dict(n=n,w800=w800,w800pre=w800pre,w2000=w2000,w2000pre=w2000pre,held=held,
                                reo_p50=q(reo,0.5),reo_p95=q(reo,0.95),tot_p50=q(tot,0.5),tot_p95=q(tot,0.95),
                                pre_p50=q(pre,0.5),pre_p95=q(pre,0.95),
                                par=(res.get("counters") or {}).get("alice_tx",{}).get("par") if res.get("counters") and res["counters"].get("alice_tx") else None))
        if not per_run: continue
        W=sum(blk_w.values()) or 1
        out[(cell,arm)]=dict(
            N=len(per_run),
            units=sum(r["n"] for r in per_run),
            w800_med=med([r["w800"] for r in per_run]), w800pre_med=med([r["w800pre"] for r in per_run]),
            w2000_med=med([r["w2000"] for r in per_run]), w2000pre_med=med([r["w2000pre"] for r in per_run]),
            held_frac_med=med([r["held"] for r in per_run]),
            reo_p50_med=med([r["reo_p50"] for r in per_run]), reo_p95_med=med([r["reo_p95"] for r in per_run]),
            tot_p50_med=med([r["tot_p50"] for r in per_run]), tot_p95_med=med([r["tot_p95"] for r in per_run]),
            pre_p50_med=med([r["pre_p50"] for r in per_run]), pre_p95_med=med([r["pre_p95"] for r in per_run]),
            blk_share={k: round(100*v/W,1) for k,v in blk_w.most_common()},
            blk_count=dict(blk_n.most_common()),
            par_runs_pos=sum(1 for r in per_run if (r["par"] or 0)>0),
        )
for k,v in out.items():
    print(k); print("  ", json.dumps(v))
