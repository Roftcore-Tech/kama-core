import os, sys, json
ROOT="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/a/out"
for cell in sys.argv[1:]:
  for arm in ("taban","aday"):
    ad=os.path.join(ROOT,cell,arm)
    for rp in sorted(os.listdir(ad), key=lambda s:int(s[3:])):
        f=os.path.join(ad,rp,"alice.jsonl")
        prev=0; t0=None; ev=[]; bwmax=0
        for l in open(f):
            try: d=json.loads(l)
            except Exception: continue
            if d.get("k")=="mark" and d.get("m")=="t0": t0=d["t"]
            if d.get("k")!="ts": continue
            for s in d.get("s",[]):
                if s.get("peer")!="bob": continue
                bwmax=max(bwmax,s.get("bw") or 0)
                if s.get("par",0)>prev:
                    ev.append((round((d["t"]-(t0 or d["t"]))/1000,1), s["par"]-prev, s.get("bw"), s.get("st")))
                    prev=s["par"]
        if ev: print(cell,arm,rp,"bwmax",bwmax,ev[:8])
