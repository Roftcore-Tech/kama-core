import json, os, math, statistics, sys
ROOT="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out"
cell=sys.argv[1]
for arm in ("taban","aday","quinn"):
    ad=os.path.join(ROOT,cell,arm)
    per={i:[] for i in range(100)}
    for rep in os.listdir(ad):
        d=os.path.join(ad,rep); meta=None; ev={}
        for l in open(os.path.join(d,"out.jsonl")):
            o=json.loads(l)
            if "meta" in o: meta=o["meta"]
            else: ev[o["event"]["msg_id"]]=o["event"]
        t0=meta["t0_unix_ns"]; off=0 if arm=="quinn" else meta["events_epoch_offset_ns"]
        first={}
        L=open(os.path.join(d,"rx_log.jsonl")).read().splitlines()
        for l in L[1:]:
            o=json.loads(l); u=o.get("u")
            if u is None or u>=2**64-1: continue
            first[u]=min(first.get(u,o["t"]),o["t"])
        for i in range(100):
            per[i].append((first[i]-(t0+ev[i]["sent_ns"]-off))/1e9 if i in first else math.inf)
    med=[statistics.median(per[i]) for i in range(100)]
    print(arm, "msg-index median ow: first5", [round(x,1) for x in med[:5]], "last5", [round(x,1) for x in med[-5:]], "min",round(min(med),1),"max",round(max(med),1))
    pooled_first=statistics.median([v for i in range(10) for v in per[i]]); pooled_last=statistics.median([v for i in range(90,100) for v in per[i]])
    print("   pooled p50 msgs0-9",round(pooled_first,2),"msgs90-99",round(pooled_last,2))
