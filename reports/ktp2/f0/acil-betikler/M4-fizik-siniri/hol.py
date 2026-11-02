import sys
sys.path.insert(0, sys.argv[1])
from sim import run, pct
def inorder(lat,I):
    out=[];m=-1e18
    for i,x in enumerate(lat):
        dv=float('inf') if x is None else x+i*I
        m=max(m,dv); out.append(m-i*I)
    return out
B=[("r12k_d1000_l30_p1000",12500,1.0,0.2,0.3,0.285,100,1000,{"K":4,"R":4,"E":1024}),
   ("r12k_d1000_l10_p65",12500,1.0,0.2,0.1,0.036,600,65,{"K":8,"R":2,"E":1024}),
   ("r12k_d300_l20_p1000",12500,0.3,0.06,0.2,0.285,100,1000,{"K":2,"R":2,"E":1024})]
for name,C,d,s,q,I,M,p,kw in B:
    res=[]
    for pol,k in (("rfc",{}),("fec",kw)):
        a=[];b=[]
        for seed in range(1,61):
            lat,_,_=run(C,d,s,q,I,M,p,pol,seed=seed*7919+int(q*100),**k)
            a.append(pct([x for x in lat if x is not None],.95)); io=inorder(lat,I); b.append(pct(io,.95))
        res.append(f"{pol}{k.get('K','')}: sırasız p95 {pct(a,.5):.2f} sıralı p95 {pct(b,.5):.2f}")
    print(name," | ".join(res))
# PTT
C_=[("c_r12k_d300_l20_b120",12500,0.3,0.06,0.2,0.12,168,37,0.8,{"K":2,"R":2,"E":64}),
    ("c_r12k_d1000_l20_b120",12500,1.0,0.2,0.2,0.12,168,37,1.8,{"K":2,"R":2,"E":64})]
for name,C,d,s,q,I,M,p,D,kw in C_:
    res=[]
    for pol,k in (("rfc",{}),("fec",kw),("fec",{"K":8,"R":2,"E":1024})):
        a=[];b=[]
        for seed in range(1,61):
            lat,_,_=run(C,d,s,q,I,M,p,pol,seed=seed*104729+int(q*100),**k)
            a.append(sum(1 for x in lat if x is not None and I+x<=D)/M); io=inorder(lat,I); b.append(sum(1 for x in io if I+x<=D)/M)
        res.append(f"{pol}{k.get('K','')}: sırasız {pct(a,.5):.3f} sıralı {pct(b,.5):.3f}")
    print(name," | ".join(res))
