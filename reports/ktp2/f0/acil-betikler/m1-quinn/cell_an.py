import json, math, sys, os, gzip, statistics as st
R=sys.argv[1]; arms=sys.argv[2].split(',')
INF=float('inf')
def qnr(v,p):
    v=sorted(v); k=max(1,math.ceil(p*len(v))); return v[k-1]
def med(v):
    v=sorted(v); n=len(v)
    if n==0: return float('nan')
    if n%2: return v[n//2]
    a,b=v[n//2-1],v[n//2]
    if math.isinf(a) or math.isinf(b): return INF if (math.isinf(a) and math.isinf(b)) else (a+b)/2
    return (a+b)/2
res={}
for arm in arms:
    rows=[]
    for rep in range(1,31):
        d=f"{R}/{arm}/rep{rep}"
        if not os.path.isdir(d): continue
        L=[json.loads(l) for l in open(f"{d}/out.jsonl")]
        meta=L[0]['meta']; ev=[x['event'] for x in L[1:] if 'event' in x]
        t0=meta['t0_unix_ns']; off=meta['events_epoch_offset_ns']
        rx={}
        RL=[json.loads(l) for l in open(f"{d}/rx_log.jsonl")]
        for o in RL[1:]:
            u=o['u']
            if u not in rx or o['t']<rx[u]: rx[u]=o['t']
        lat=[];ow=[];rev=[];acked_no_rx=0;rx_no_ack=0
        for e in ev:
            s=e['sent_ns']; dn=e['delivered_ns']; m=e['msg_id']
            lat.append(INF if dn is None else (dn-s)/1e9)
            su=t0+s-off
            o=(rx[m]-su)/1e9 if m in rx else INF
            ow.append(o)
            if dn is not None and m in rx:
                rev.append((dn-s)/1e9-o)
            if dn is not None and m not in rx: acked_no_rx+=1
            if dn is None and m in rx: rx_no_ack+=1
        sent_rel=[(e['sent_ns']-off)/1e9 for e in ev]
        # schedule slip: sent_rel vs i*I
        I=meta['submit_interval_ns']/1e9
        slip=max(abs(sent_rel[i]-i*I) for i in range(len(ev)))
        setup=meta['setup_ns']/1e9
        # counterfactual: handshake inside window (offer schedule starts at connect start)
        cf=[l + max(0.0, setup - i*I) for i,l in enumerate(lat)]
        rows.append(dict(rep=rep,p50=qnr(lat,.5),p95=qnr(lat,.95),ow50=qnr(ow,.5),ow95=qnr(ow,.95),
            rev50=(st.median(rev) if rev else float('nan')),rev95=(qnr(rev,.95) if rev else float('nan')),
            deliv=sum(1 for x in lat if not math.isinf(x)), rxn=len(rx), acked_no_rx=acked_no_rx, rx_no_ack=rx_no_ack,
            slip_ms=slip*1e3, setup=setup, cf95=qnr(cf,.95), first10=st.median([x for x in lat[:10]]) if lat else 0,
            minlat=min(lat)))
    res[arm]=rows
    def m(k): return med([r[k] for r in rows])
    print(f"{arm:6s} N={len(rows)} lat_p50={m('p50'):.2f} lat_p95={m('p95'):.2f} ow_p50={m('ow50'):.2f} ow_p95={m('ow95'):.2f} rev_leg_p50={m('rev50'):.3f} rev_leg_p95={m('rev95'):.3f} setup={m('setup'):.3f} cf_p95(hs-in)={m('cf95'):.2f} first10_med={m('first10'):.2f} minlat={m('minlat'):.3f}")
    print(f"        deliv<100 runs={sum(1 for r in rows if r['deliv']<100)} acked_no_rx(total)={sum(r['acked_no_rx'] for r in rows)} rx_no_ack(total)={sum(r['rx_no_ack'] for r in rows)} max_slip_ms={max(r['slip_ms'] for r in rows):.2f} setup_range=({min(r['setup'] for r in rows):.3f},{max(r['setup'] for r in rows):.3f}) p95_inf_runs={sum(1 for r in rows if math.isinf(r['p95']))}")
json.dump(res, open(sys.argv[3],'w'), default=str)
