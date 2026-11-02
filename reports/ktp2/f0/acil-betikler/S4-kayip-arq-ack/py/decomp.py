import json,sys,os,statistics as st
def q(v,p):
    v=sorted(v); 
    if not v: return float('nan')
    k=max(0,min(len(v)-1,int(round(p*(len(v)-1)))))
    return v[k]
def load(rep):
    meta=None; ev={}
    for l in open(os.path.join(rep,'out.jsonl')):
        d=json.loads(l)
        if 'meta' in d: meta=d['meta']
        elif 'event' in d: e=d['event']; ev[e['msg_id']]=e
    rx={}; dup=0
    for l in open(os.path.join(rep,'rx_log.jsonl')):
        d=json.loads(l)
        if 'u' in d:
            if d['u'] in rx: dup+=1
            else: rx[d['u']]=d['t']
    return meta,ev,rx,dup
cell=sys.argv[1]; arms=sys.argv[2].split(',')
for arm in arms:
    base=os.path.join(cell,arm)
    F=[];A=[];L=[];nrx=0;nev=0;dups=0;miss=0; fwd_big=0
    for r in sorted(os.listdir(base)):
        rep=os.path.join(base,r)
        if not os.path.exists(os.path.join(rep,'rx_log.jsonl')): continue
        meta,ev,rx,dup=load(rep); dups+=dup
        t0=meta['t0_unix_ns']+meta.get('events_epoch_offset_ns',0)
        for m,e in ev.items():
            nev+=1
            if e.get('delivered_ns') is None: miss+=1; continue
            lat=(e['delivered_ns']-e['sent_ns'])/1e9; L.append(lat)
            if m in rx:
                f=(rx[m]-t0-e['sent_ns'])/1e9; a=(e['delivered_ns']-(rx[m]-t0))/1e9
                F.append(f);A.append(a)
    print(arm,'n=',nev,'miss',miss,'rxdup',dups,'matched',len(F))
    for name,v in (('lat',L),('fwd',F),('ack_lag',A)):
        print('  %-8s p50=%.2f p90=%.2f p95=%.2f p99=%.2f max=%.2f mean=%.2f'%(name,q(v,.5),q(v,.9),q(v,.95),q(v,.99),max(v),st.mean(v)))
