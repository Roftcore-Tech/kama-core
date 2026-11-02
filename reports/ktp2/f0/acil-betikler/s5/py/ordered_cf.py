import os, sys, json, math, statistics, re
NS=1e9
root=sys.argv[1]
def rank(v,p):
    v=sorted(v); 
    return v[max(0,math.ceil(p*len(v))-1)]
def load_run(d):
    try:
        lines=open(os.path.join(d,"out.jsonl")).read().splitlines()
    except FileNotFoundError: return None
    meta=None; sent={}; dl={}
    for l in lines:
        o=json.loads(l)
        if "meta" in o: meta=o["meta"]
        elif "event" in o:
            e=o["event"]; sent[e["msg_id"]]=e["sent_ns"]; dl[e["msg_id"]]=e.get("delivered_ns")
    if meta is None: return None
    n=meta["msg_count"]; D=meta["max_deadline_ns"]/NS
    t0=int(meta.get("t0_unix_ns") or 0); off=int(meta.get("events_epoch_offset_ns") or 0)
    first={}
    try:
        for l in open(os.path.join(d,"rx_log.jsonl")).read().splitlines()[1:]:
            o=json.loads(l); u=o.get("u")
            if u is None or u>=2**64-1: continue
            if u not in first or o["t"]<first[u]: first[u]=o["t"]
    except FileNotFoundError:
        first=None
    snd=[]
    for m in range(n):
        if m in sent and dl.get(m) is not None:
            snd.append(min(D,(dl[m]-sent[m])/NS))
        else: snd.append(D)
    ow=None; owo=None
    if first is not None and t0:
        ow=[]; owo=[]; run_max=-1; broken=False
        for m in range(n):
            if m in first and m in sent:
                x=max(0,first[m]-(t0+sent[m]-off))/NS
                ow.append(min(D,x))
                rel=first[m]
            else:
                ow.append(D); rel=None
            if rel is None: broken=True
            if broken: owo.append(D)
            else:
                run_max=max(run_max,rel)
                owo.append(min(D,max(0,run_max-(t0+sent[m]-off))/NS))
    return dict(n=n,D=D,snd=snd,ow=ow,owo=owo,tr=meta.get("transport"))
res={}
for cell in sorted(os.listdir(root)):
    cd=os.path.join(root,cell)
    if not os.path.isdir(cd) or not cell.startswith("b_r") or "r25" in cell: continue
    for arm in ("taban","aday","quinn"):
        ad=os.path.join(cd,arm)
        if not os.path.isdir(ad): continue
        p_snd=[];p_ow=[];p_owo=[]; w=[]; wo=[]
        for rep in os.listdir(ad):
            r=load_run(os.path.join(ad,rep))
            if r is None: continue
            p_snd.append(rank(r["snd"],0.95))
            if r["ow"]: 
                p_ow.append(rank(r["ow"],0.95)); p_owo.append(rank(r["owo"],0.95))
        if not p_snd: continue
        res[(cell,arm)]=dict(N=len(p_snd),snd=statistics.median(p_snd),
            ow=statistics.median(p_ow) if p_ow else None, owo=statistics.median(p_owo) if p_owo else None)
cells=sorted(set(c for c,_ in res))
print("cell | taban snd/ow/ow_sirali | aday snd/ow/ow_sirali | quinn snd/ow")
cnt=dict(ow=0,owo=0,snd=0); tot=0
for c in cells:
    t=res.get((c,"taban")); a=res.get((c,"aday")); q=res.get((c,"quinn"))
    if not (t and q): continue
    f=lambda x: "-" if x is None else "%.1f"%x
    print(c, "|", f(t["snd"]),f(t["ow"]),f(t["owo"]),"|", f(a["snd"]) if a else "", f(a["ow"]) if a else "", f(a["owo"]) if a else "", "|", f(q["snd"]), f(q["ow"]), "N",t["N"],q["N"])

print()
prim=[c for c in cells if re.search(r"_l(0|10|20|30)_",c)]
for arm in ("taban","aday"):
    n=0; ow_gt=0; owo_gt=0; snd_gt=0; pen=[]
    for c in prim:
        k=res.get((c,arm)); q=res.get((c,"quinn"))
        if not (k and q and k["ow"] is not None and q["ow"] is not None): continue
        n+=1
        ow_gt += k["ow"]>q["ow"]; owo_gt += k["owo"]>q["ow"]; snd_gt += k["snd"]>q["snd"]
        pen.append(k["owo"]-k["ow"])
    print(arm,"cells",n,"snd>quinn",snd_gt,"ow>quinn_ow",ow_gt,"ordered>quinn_ow",owo_gt,
          "order-penalty median %.2f max %.2f"%(statistics.median(pen),max(pen)))
