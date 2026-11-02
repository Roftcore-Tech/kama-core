import json,sys,os,glob,statistics as st
root='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
def load(p):
    meta=None;ev=[]
    for line in open(p):
        try:o=json.loads(line)
        except:continue
        if 'meta' in o:meta=o['meta']
        elif 'event' in o:ev.append(o['event'])
    return meta or {},ev
def q(xs,f):
    xs=sorted(xs); return xs[min(len(xs)-1,int(f*(len(xs)-1)))] if xs else float('nan')
for cell in sys.argv[1:]:
    for arm in ['taban','aday','quinn']:
        rows=[]
        for d in sorted(glob.glob(f'{root}/{cell}/{arm}/rep*')):
            p=d+'/out.jsonl'
            if not os.path.exists(p):continue
            m,ev=load(p)
            n=m.get('msg_count');I=m.get('submit_interval_ns',0)/1e9
            a=m.get('activation') or {}
            tx=m.get('tx') or {}
            lat=[(e['delivered_ns']-e['sent_ns'])/1e9 for e in ev if e.get('delivered_ns')]
            if not lat or not n:continue
            unit=(tx.get('data') or 0)/n
            offered=unit/I if unit else (m.get('payload_bytes',0)+27)/I
            sub_end=(n-1)*I
            lastd=max(e['delivered_ns'] for e in ev if e.get('delivered_ns'))/1e9
            ev_sorted=sorted([e for e in ev if e.get('delivered_ns')],key=lambda e:e['msg_id'])
            l=[(e['delivered_ns']-e['sent_ns'])/1e9 for e in ev_sorted]
            k=max(1,len(l)//10)
            slope=st.mean(l[-k:])-st.mean(l[:k])
            rows.append(dict(p95=q(lat,0.95),p50=q(lat,0.5),bw_off=(a.get('btlbw_bps') or 0)/offered if a else float('nan'),
              tx_rate=m.get('udp_tx_bytes',0)/max(lastd,sub_end),need=offered,lag=lastd-sub_end,slope=slope,
              retx=(tx.get('retx') or 0)/max(1,tx.get('data') or 1),par=a.get('bytes_parity'),arq=a.get('recover_arq_entries')))
        if not rows:continue
        f=lambda k:'%.2f'%st.median([r[k] for r in rows if r[k] is not None and r[k]==r[k]]) if any(r[k] is not None and r[k]==r[k] for r in rows) else 'na'
        print(f'{cell:26s} {arm:6s} n={len(rows):2d} p50={f("p50")} p95={f("p95")} btlbw/offered={f("bw_off")} offered={f("need")} lag_after_submit_end={f("lag")} lat_slope(last10%-first10%)={f("slope")} retx/data={f("retx")} parity_bytes={f("par")}')
