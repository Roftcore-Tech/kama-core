"""B ızgarası birincil 32 hücre: ideal ARQ (oracle, rfc) ve ideal FEC+ARQ (K, R, E) p95 — MC."""
import sys, json
sys.path.insert(0, sys.argv[1])
from sim import run, pct, data_l2, rep_l2
NS = int(sys.argv[2]) if len(sys.argv) > 2 else 60
# fizik.py GEOMETRY + RATES + DELAYS (scripts/ktp2/f0/b/fizik.py:68-75, 61-62) — kopya
GEOM = {("r1k",65):(450,100),("r1k",1000):(3600,20),("r12k",65):(36,600),("r12k",1000):(285,100)}
RATES = {"r1k":1000,"r12k":12500}
DEL = {"d300":(0.3,0.06),"d1000":(1.0,0.2)}
RT = {2:{0.0:1,0.1:1,0.2:2,0.3:2},4:{0.0:1,0.1:1,0.2:2,0.3:4},8:{0.0:2,0.1:2,0.2:2,0.3:4}}
out=[]
for rid in ("r1k","r12k"):
  for did in ("d300","d1000"):
    for loss in (0,10,20,30):
      for p in (65,1000):
        I_ms,M = GEOM[(rid,p)]; C=RATES[rid]; d,s=DEL[did]; q=loss/100; I=I_ms/1000
        cell=f"{rid}_{did}_l{loss}_p{p}"
        pols=[("oracle",{}),("rfc",{})]
        for K in (2,4,8):
          pols.append((f"fec{K}_E1024",{"K":K,"R":RT[K][q],"E":1024}))
        if p==65:
          for K in (4,8):
            pols.append((f"fec{K}_Efit",{"K":K,"R":RT[K][q],"E":p+11}))
        for name,kw in pols:
          p95s=[];p50s=[];rhos=[];und=0
          for seed in range(1,NS+1):
            lat,b,dur=run(C,d,s,q,I,M,p,"fec" if name.startswith("fec") else name,seed=seed*7919+loss,**kw)
            v=[x for x in lat if x is not None]; und+=M-len(v)
            p95s.append(pct(v,.95)); p50s.append(pct(v,.5)); rhos.append(b/(C*M*I))
          row={"cell":cell,"pol":name,"K":kw.get("K"),"R":kw.get("R"),"E":kw.get("E"),
               "p95_med":pct(p95s,.5),"p95_q1":pct(p95s,.25),"p95_q3":pct(p95s,.75),
               "p50_med":pct(p50s,.5),"rho_fwd":pct(rhos,.5),"undeliv":und}
          out.append(row)
          print(json.dumps(row),flush=True)
