import sys, time
sys.path.insert(0, sys.argv[1])
from sim import run, pct
t0=time.time()
for pol in ("oracle","rfc","fec"):
    p95=[]
    for seed in range(1,31):
        lat,b,dur = run(12500,1.0,0.2,0.3,0.285,100,1000,pol,K=8,R=4,E=1024,seed=seed)
        v=[x for x in lat if x is not None]
        p95.append(pct(v,.95))
    print(pol, "median p95", round(pct(p95,.5),3), "Q1-Q3", round(pct(p95,.25),3), round(pct(p95,.75),3), "undeliv", sum(1 for x in lat if x is None))
print("t", time.time()-t0)
