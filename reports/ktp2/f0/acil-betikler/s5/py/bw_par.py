import os, sys, json, statistics
root=sys.argv[1]
for cell in sys.argv[2].split(","):
  for arm in ("taban","aday"):
    ad=os.path.join(root,cell,arm); rows=[]
    for rep in os.listdir(ad):
        f=os.path.join(ad,rep,"alice.jsonl")
        if not os.path.exists(f): continue
        bws=[]; last=None
        for l in open(f):
            if (chr(34)+"k"+chr(34)+":"+chr(34)+"ts"+chr(34)) not in l.replace(" ",""): continue
            o=json.loads(l); s=[x for x in o["s"] if x.get("peer")=="bob"]
            if not s: continue
            last=s[0]; bws.append(s[0].get("bw") or 0)
        if last is None: continue
        rows.append((last.get("par") or 0, statistics.median(bws), max(bws)))
    pos=[r for r in rows if r[0]>0]; zer=[r for r in rows if r[0]==0]
    md=lambda v: round(statistics.median(v)) if v else None
    print(cell,arm,"runs",len(rows),"par>0",len(pos),"bw_med(par>0)",md([r[1] for r in pos]),"bw_max(par>0)",md([r[2] for r in pos]),"bw_med(par=0)",md([r[1] for r in zer]),"bw_max(par=0)",md([r[2] for r in zer]))
