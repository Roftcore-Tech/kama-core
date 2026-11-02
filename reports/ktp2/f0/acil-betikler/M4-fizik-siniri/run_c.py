"""C şeridi PTT hücreleri: ideal ARQ / ideal FEC+ARQ son-tarih oranları — MC (bench geometrisi)."""
import sys, json, os, re
sys.path.insert(0, sys.argv[1])
from sim import run, pct
CELLDIR = sys.argv[2]; NS = int(sys.argv[3])
cells=[]
for fn in sorted(os.listdir(CELLDIR)):
    if not fn.endswith(".cell"): continue
    s=open(os.path.join(CELLDIR,fn)).read()
    fw=re.search(r'^FWD="delay (\d+)ms (\d+)ms distribution normal loss (\d+)% rate (\d+)kbit"',s,re.M)
    a=re.search(r'--msgs (\d+) --payload (\d+) --submit-interval-ms (\d+)',s)
    r2=re.search(r'ARM_rs2_taban_CLI=.*?--fec-r (\d+)',s); r4=re.search(r'ARM_rs4_taban_CLI=.*?--fec-r (\d+)',s)
    cells.append(dict(cell=fn[:-5],d=int(fw.group(1))/1000,s=int(fw.group(2))/1000,q=int(fw.group(3))/100,
        C=int(fw.group(4))*1000/8,M=int(a.group(1)),pay=int(a.group(2)),b=int(a.group(3))/1000,
        R2=int(r2.group(1)) if r2 else 2,R4=int(r4.group(1)) if r4 else 2))
for c in cells:
    pols=[("oracle","oracle",{}),("rfc","rfc",{}),("rs2","fec",{"K":2,"R":c["R2"],"E":64}),
          ("rs4","fec",{"K":4,"R":c["R4"],"E":64}),("urun_K8R2E1024","fec",{"K":8,"R":2,"E":1024}),
          ("K8R2E64","fec",{"K":8,"R":2,"E":64}),("K8R4E64","fec",{"K":8,"R":4,"E":64})]
    for name,pol,kw in pols:
        acc={k:[] for k in ("T1_400","T1_800","T2_400","T2_800","p95","rho")}
        for seed in range(1,NS+1):
            lat,by,dur=run(c["C"],c["d"],c["s"],c["q"],c["b"],c["M"],c["pay"],pol,seed=seed*104729+int(c["q"]*100),**kw)
            m2e=[None if x is None else c["b"]+x for x in lat]
            n=len(m2e)
            for key,D in (("T1_400",0.4),("T1_800",0.8),("T2_400",c["d"]+0.4),("T2_800",c["d"]+0.8)):
                acc[key].append(sum(1 for x in m2e if x is not None and x<=D+1e-12)/n)
            acc["p95"].append(pct([x for x in lat if x is not None],.95))
            acc["rho"].append(by/(c["C"]*c["M"]*c["b"]))
        row={"cell":c["cell"],"pol":name,**{k:pct(v,.5) for k,v in acc.items()},"K":kw.get("K"),"R":kw.get("R"),"E":kw.get("E")}
        print(json.dumps(row),flush=True)
