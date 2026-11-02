import json,sys,os,statistics
def load(p):
    meta=None;ev=[]
    for line in open(p):
        try: o=json.loads(line)
        except: continue
        if 'meta' in o: meta=o['meta']
        elif 'event' in o: ev.append(o['event'])
    return meta,ev
for p in sys.argv[1:]:
    meta,ev=load(p)
    lats=[(e['msg_id'],(e['delivered_ns']-e['sent_ns'])/1e9) for e in ev if e.get('delivered_ns')]
    lats.sort()
    l=[x[1] for x in lats]
    if not l: print(p,'none');continue
    ls=sorted(l)
    meta=meta or {}
    a=meta.get('activation') or {}
    tx=meta.get('tx',{})
    last=max(e['delivered_ns'] for e in ev if e.get('delivered_ns'))/1e9
    print(os.path.relpath(p,'/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'),'n',len(l),'p50 %.1f p95 %.1f'%(ls[len(ls)//2],ls[int(0.95*(len(ls)-1))]),
      'first10 %.1f last10 %.1f'%(statistics.mean(l[:10]),statistics.mean(l[-10:])),'lastdeliv %.1f'%last,
      'btlbw',a.get('btlbw_bps'),'udp_tx',meta.get('udp_tx_bytes'),'tx',tx, 'retx',a.get('retransmits'),'arq_ent',a.get('recover_arq_entries'))
