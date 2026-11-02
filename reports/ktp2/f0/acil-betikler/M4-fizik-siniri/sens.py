import sys
sys.path.insert(0, sys.argv[1])
from sim import run, pct
cases=[("r12k_d300_l10_p1000",12500,0.3,0.06,0.1,0.285,100,1000),("r12k_d300_l30_p1000",12500,0.3,0.06,0.3,0.285,100,1000),
       ("r1k_d300_l20_p65",1000,0.3,0.06,0.2,0.45,100,65),("r1k_d1000_l20_p65",1000,1.0,0.2,0.2,0.45,100,65),
       ("r12k_d1000_l30_p1000",12500,1.0,0.2,0.3,0.285,100,1000),("r12k_d1000_l30_p65",12500,1.0,0.2,0.3,0.036,600,65)]
for name,C,d,s,q,I,M,p in cases:
    out=[]
    for ae in (1,2):
        v=[]
        for seed in range(1,61):
            lat,_,_=run(C,d,s,q,I,M,p,"rfc",seed=seed*7919+int(q*100),ack_every=ae)
            v.append(pct([x for x in lat if x is not None],.95))
        out.append(f"ack_every={ae}: p95 med {pct(v,.5):.2f} [{pct(v,.25):.2f}-{pct(v,.75):.2f}]")
    print(name," | ".join(out))
