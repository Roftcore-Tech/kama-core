import sys, json, pathlib, importlib.util
spec = importlib.util.spec_from_file_location("rt", str(pathlib.Path(__file__).with_name("retx_timeline.py")))
rt = importlib.util.module_from_spec(spec); spec.loader.exec_module(rt)
def q(v,p):
    v=sorted(v)
    return v[max(0,min(len(v)-1,int(round(p*(len(v)-1)))))] if v else float('nan')
root = pathlib.Path(sys.argv[1])
for c in sys.argv[2:]:
    for a in ("taban","aday"):
        rows=[]
        for rep in sorted((root/c/a).glob("rep*")):
            try:
                meta=None; ev=[]
                for line in (rep/"out.jsonl").open():
                    d=json.loads(line)
                    if "meta" in d: meta=d["meta"]
                    elif "event" in d: ev.append(d["event"])
                t0=meta["t0_unix_ns"]/1e9; off=meta.get("events_epoch_offset_ns",0)
                cli=rt.read_cap(rep/"capb_cli.csv.gz"); srv=rt.read_cap(rep/"capb_srv.csv.gz")
                port=rt.port_of(rep)
                a_out=rt.udp(cli,"o",True,port); b_in=rt.udp(srv,"i",True,port)
            except Exception as e:
                print("#",rep,e,file=sys.stderr); continue
            arr={}
            for p in b_in: arr.setdefault(p["pn"],p["t"])
            tx={}
            for p in a_out:
                if p["type"] in ("data","data_frag") and "unit" in p:
                    tx.setdefault(p["unit"],[]).append(p)
            # unit id == msg id for single-unit messages (bench kama-product: unit per msg)
            for e in ev:
                if e.get("delivered_ns") is None: continue
                u=e["msg_id"]
                lst=tx.get(u)
                if not lst: continue
                offer=t0+(e["sent_ns"]-off)/1e9; conf=t0+(e["delivered_ns"]-off)/1e9
                ftx=min(p["t"] for p in lst)
                fa=min((arr[p["pn"]] for p in lst if p["pn"] in arr), default=None)
                if fa is None: continue
                rows.append((conf-offer, ftx-offer, fa-ftx, conf-fa))
        if not rows: continue
        tot=[r[0] for r in rows]; p90=q(tot,.9)
        top=[r for r in rows if r[0]>=p90]
        f=lambda i,s: sum(r[i] for r in s)/len(s)
        print(f"{c} {a}: n={len(rows)} total p50={q(tot,.5):.2f} p95={q(tot,.95):.2f} | "
              f"queue p50={q([r[1] for r in rows],.5):.2f} rec p50={q([r[2] for r in rows],.5):.2f} fb p50={q([r[3] for r in rows],.5):.2f} | "
              f"top10% mean: total={f(0,top):.2f} queue={f(1,top):.2f} rec={f(2,top):.2f} fb={f(3,top):.2f}")
