import json,gzip,sys,os,statistics as st,math
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/'
lane=sys.argv[1]; cell=sys.argv[2]
arms=sys.argv[3].split(',')
def pct(v,q):
    # linear interpolation (numpy default)
    v=sorted(v)
    if not v: return float('nan')
    k=(len(v)-1)*q; f=math.floor(k); c=math.ceil(k)
    return v[f]+(v[c]-v[f])*(k-f)
def load(d):
    meta=None;ev={}
    for line in open(os.path.join(d,'out.jsonl')):
        o=json.loads(line)
        if 'meta' in o: meta=o['meta']
        elif 'event' in o:
            e=o['event']; ev[e['msg_id']]=e
    rx={}
    p=os.path.join(d,'rx_log.jsonl')
    if os.path.exists(p):
        for line in open(p):
            o=json.loads(line)
            if 'u' in o and o['u'] not in rx: rx[o['u']]=o['t']
    return meta,ev,rx
def cap(d):
    ftx={};ntx={}
    p=os.path.join(d,'capb_cli.csv.gz')
    if not os.path.exists(p): return None,None
    for line in gzip.open(p,'rt'):
        f=line.rstrip('\n').split(',')
        if len(f)<7 or f[1]!='o': continue
        h=f[6].split(':')[0]
        if len(h)<22 or h[:2]!='00': continue
        u=int(h[6:22],16)
        t=int(f[0])
        ntx[u]=ntx.get(u,0)+1
        if u not in ftx: ftx[u]=t
    return ftx,ntx
res={}
for arm in arms:
    ad=os.path.join(base,lane,'out',cell,arm)
    runs=[]
    for rep in sorted(os.listdir(ad),key=lambda s:int(s[3:]) if s[3:].isdigit() else 0):
        d=os.path.join(ad,rep)
        if not os.path.isdir(d) or not os.path.exists(os.path.join(d,'out.jsonl')): continue
        meta,ev,rx=load(d)
        t0=meta['t0_unix_ns']; off=meta.get('events_epoch_offset_ns',0)
        n=meta['msg_count']
        ftx,ntx=(cap(d) if arm in('taban','aday') else (None,None))
        r={'rep':rep,'n':n}
        pres={i:t0+ev[i]['sent_ns']-off for i in ev}
        conf={i:(t0+ev[i]['delivered_ns']-off) for i in ev if ev[i]['delivered_ns'] is not None}
        r['conf']=[ (conf[i]-pres[i])/1e9 for i in sorted(conf)]
        r['confi']={i:(conf[i]-pres[i])/1e9 for i in conf}
        r['rxow']={i:(rx[i]-pres[i])/1e9 for i in rx if i in pres}
        r['ackret']=[ (conf[i]-rx[i])/1e9 for i in conf if i in rx]
        r['plan_dev']=max(abs(ev[i]['sent_ns']-off-i*meta['submit_interval_ns'] if 'submit_interval_ns' in meta else 0) for i in ev)/1e6 if 'submit_interval_ns' in meta else None
        if ftx is not None:
            r['sw']={i:(ftx[i]-pres[i])/1e9 for i in pres if i in ftx}
            r['post']={i:(conf[i]-ftx[i])/1e9 for i in conf if i in ftx}
            r['ntx']=ntx
        runs.append(r)
    res[arm]=runs
def med(x): return st.median(x) if x else float('nan')
for arm,runs in res.items():
    print('==',arm,len(runs))
    conf95=[pct(r['conf'],.95) for r in runs]; conf50=[pct(r['conf'],.5) for r in runs]
    print(' conf p50/p95 med-of-runs: %.3f / %.3f'%(med(conf50),med(conf95)))
    rx50=[pct(list(r['rxow'].values()),.5) for r in runs]; rx95=[pct(list(r['rxow'].values()),.95) for r in runs]
    print(' rx_ow p50/p95: %.3f / %.3f'%(med(rx50),med(rx95)))
    ar=[pct(r['ackret'],.5) for r in runs if r['ackret']]
    print(' ack-return (conf-rx) per-run median, med: %.3f'%med(ar))
    if runs and 'sw' in runs[0]:
        sw50=[pct(list(r['sw'].values()),.5) for r in runs]; sw95=[pct(list(r['sw'].values()),.95) for r in runs]
        po50=[pct(list(r['post'].values()),.5) for r in runs]; po95=[pct(list(r['post'].values()),.95) for r in runs]
        print(' sender_wait p50/p95: %.3f / %.3f'%(med(sw50),med(sw95)))
        print(' post-first-tx (conf-ftx) p50/p95: %.3f / %.3f'%(med(po50),med(po95)))
        # rx - ftx
        rf95=[pct([r['rxow'][i]-r['sw'][i] for i in r['rxow'] if i in r['sw']],.95) for r in runs]
        rf50=[pct([r['rxow'][i]-r['sw'][i] for i in r['rxow'] if i in r['sw']],.5) for r in runs]
        print(' rx-ftx p50/p95: %.3f / %.3f'%(med(rf50),med(rf95)))
        # messages with single tx vs multi
        one=[sum(1 for i in r['sw'] if r['ntx'].get(i,0)==1)/len(r['sw']) for r in runs]
        print(' frac msgs with exactly one DATA tx (median): %.3f'%med(one))
    # per index medians of rx_ow
    idx=sorted(set().union(*[set(r['rxow']) for r in runs]))
    pm=[med([r['rxow'][i] for r in runs if i in r['rxow']]) for i in idx]
    if pm:
        print(' rx_ow per-index median: first10 %.2f, msg0 %.2f, last10 %.2f, last %.2f'%(med(pm[:10]),pm[0],med(pm[-10:]),pm[-1]))
    h2=[pct([v for i,v in r['rxow'].items() if i>=50],.95) for r in runs]
    print(' second-half rx_ow p95 median: %.3f'%med(h2))
    pdv=[r['plan_dev'] for r in runs if r['plan_dev'] is not None]
    if pdv: print(' max plan dev ms: %.2f'%max(pdv))
import pickle
pickle.dump(res,open('/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/res_%s.pkl'%cell,'wb'))
