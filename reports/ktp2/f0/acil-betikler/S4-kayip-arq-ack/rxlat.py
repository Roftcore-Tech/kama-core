import sys, json, pathlib
root=pathlib.Path(sys.argv[1])
def q(v,p):
    v=sorted(v)
    return v[max(0,min(len(v)-1,int(round(p*(len(v)-1)))))] if v else float('nan')
for c in sys.argv[2:]:
    for a in ("taban","aday","quinn"):
        lat=[]
        for rep in (root/c/a).glob("rep*"):
            try:
                lines=(rep/"out.jsonl").read_text().splitlines()
                meta=json.loads(lines[0])["meta"]; t0=meta["t0_unix_ns"]; off=meta.get("events_epoch_offset_ns",0)
                sent={}
                for l in lines[1:]:
                    e=json.loads(l).get("event")
                    if e: sent[e["msg_id"]]=t0+e["sent_ns"]-off
                first={}
                for l in (rep/"rx_log.jsonl").read_text().splitlines()[1:]:
                    d=json.loads(l)
                    if d["u"] not in first: first[d["u"]]=d["t"]
                for u,t in first.items():
                    if u in sent: lat.append((t-sent[u])/1e9)
            except Exception as ex:
                print("#",rep,ex,file=sys.stderr)
        print(f"{c} {a:6s} rx(offer->first rx) n={len(lat)} p50={q(lat,.5):.2f} p70={q(lat,.7):.2f} p80={q(lat,.8):.2f} p90={q(lat,.9):.2f} p95={q(lat,.95):.2f}")
