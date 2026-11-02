import os, sys, json, statistics
ROOT="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/a/out"
def load(p):
    out=[]
    for l in open(p):
        try: out.append(json.loads(l))
        except Exception: pass
    return out
for cell in sorted(os.listdir(ROOT)):
    cd=os.path.join(ROOT,cell)
    if not os.path.isdir(cd): continue
    for arm in ("taban","aday"):
        ad=os.path.join(cd,arm)
        if not os.path.isdir(ad): continue
        rows=[]
        for rp in sorted(os.listdir(ad), key=lambda s:int(s[3:]) if s[3:].isdigit() else 0):
            f=os.path.join(ad,rp,"alice.jsonl")
            if not os.path.exists(f): continue
            last=None; bws=[]; maxpar=0
            for d in load(f):
                if d.get("k")!="ts": continue
                for s in d.get("s",[]):
                    if s.get("peer")=="bob":
                        last=s; maxpar=max(maxpar,s.get("par",0))
                        if s.get("bw") is not None: bws.append(s["bw"])
            if last is None: continue
            rows.append((last.get("par",0), maxpar, statistics.median(bws) if bws else None))
        pos=[r for r in rows if r[0]>0]; zero=[r for r in rows if r[0]==0]
        mb=lambda rs: statistics.median([r[2] for r in rs if r[2] is not None]) if rs else None
        print(f"{cell:32s} {arm:6s} par>0 {len(pos)}/{len(rows)} max {max((r[0] for r in rows),default=0)} maxpar(any ts) {max((r[1] for r in rows),default=0)} | bw med par>0 {mb(pos)} par=0 {mb(zero)}")
