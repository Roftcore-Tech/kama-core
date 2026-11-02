import json, sys, os, statistics as st, math
R = sys.argv[1]
D = 60.0
def q(v, p):
    v = sorted(v)
    if not v: return float('nan')
    k = (len(v)-1)*p; f = math.floor(k); c = math.ceil(k)
    return v[f] + (v[c]-v[f])*(k-f)
def nr(v,p):  # nearest-rank
    v=sorted(v); 
    if not v: return float('nan')
    i=max(0,math.ceil(p*len(v))-1); return v[i]
res = {}
for arm in sys.argv[2:]:
    rows=[]
    for rep in range(1,31):
        d=f"{R}/{arm}/rep{rep}"
        if not os.path.exists(d+"/out.jsonl"): continue
        L=[json.loads(l) for l in open(d+"/out.jsonl")]
        meta=L[0]["meta"]; ev=[x["event"] for x in L[1:]]
        t0=meta["t0_unix_ns"]; off=meta.get("events_epoch_offset_ns",0)
        rx={}
        rxl=[json.loads(l) for l in open(d+"/rx_log.jsonl")][1:]
        for r in rxl:
            rx.setdefault(r["u"], r["t"])
        conf=[]; ow=[]; lag=[]; undeliv=0; rx_only=0
        for e in ev:
            s_abs=t0+e["sent_ns"]-off
            dn=e["delivered_ns"]
            if dn is None:
                undeliv+=1; conf.append(math.inf)
                if e["msg_id"] in rx: rx_only+=1
            else:
                conf.append((dn-e["sent_ns"])/1e9)
            if e["msg_id"] in rx:
                ow.append((rx[e["msg_id"]]-s_abs)/1e9)
                if dn is not None: lag.append((t0+dn-off - rx[e["msg_id"]])/1e9)
            else: ow.append(math.inf)
        sent=[e["sent_ns"]-off for e in ev]
        p95c=min(nr(conf,.95),D)
        rows.append(dict(rep=rep,p95c=p95c,p50c=min(nr(conf,.5),D),owp95=nr(ow,.95),owp50=nr(ow,.5),
            lag50=st.median(lag) if lag else float('nan'), lagp95=nr(lag,.95) if lag else float('nan'), lagmin=min(lag) if lag else float('nan'),
            undeliv=undeliv, rx_only=rx_only, setup=meta["setup_ns"]/1e9, first=conf[0], first_ow=ow[0],
            sent0=sent[0]/1e9, sent99=sent[-1]/1e9, txb=meta["udp_tx_bytes"], rxb=meta["udp_rx_bytes"],
            neg=sum(1 for x in lag if x<0)))
    res[arm]=rows
    keys=["p95c","p50c","owp95","owp50","lag50","lagp95","lagmin","undeliv","rx_only","setup","first","first_ow","sent0","sent99","txb","rxb","neg"]
    print(f"== {arm} n={len(rows)}")
    for k in keys:
        v=[r[k] for r in rows if not (isinstance(r[k],float) and math.isinf(r[k]))]
        if v: print(f"  {k:8s} med={st.median(v):.3f} min={min(v):.3f} max={max(v):.3f}")
json.dump(res, open(os.path.dirname(os.path.abspath(__file__))+"/res_"+os.path.basename(R)+".json","w"), default=str)
