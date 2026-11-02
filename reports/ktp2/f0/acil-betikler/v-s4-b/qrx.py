import json, pathlib, sys
def q(v, p):
    v = sorted(v); return float("nan") if not v else v[max(0, min(len(v)-1, int(round(p*(len(v)-1)))))]
cell = pathlib.Path(sys.argv[1])
for arm in sys.argv[2:]:
    lat = []; nmsg = 0
    for rep in cell.joinpath(arm).glob("rep*"):
        L = (rep/"out.jsonl").read_text().splitlines()
        meta = json.loads(L[0])["meta"]; t0 = meta["t0_unix_ns"]; off = meta.get("events_epoch_offset_ns", 0)
        sub = {}
        for l in L[1:]:
            e = json.loads(l).get("event")
            if e: sub[e["msg_id"]] = t0 + e["sent_ns"] - off; nmsg += 1
        first = {}
        rl = rep/"rx_log.jsonl"
        if not rl.exists(): continue
        for l in rl.read_text().splitlines()[1:]:
            d = json.loads(l); first.setdefault(d["u"], d["t"])
        lat += [(t - sub[u])/1e9 for u, t in first.items() if u in sub]
    print(cell.name, arm, "msgs", nmsg, "arrived", len(lat), " ".join(f"p{int(p*100)}={q(lat,p):.2f}" for p in (.5,.7,.8,.9,.95)))
