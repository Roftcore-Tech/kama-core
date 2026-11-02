import sys, os, re, math, statistics, json
sys.argv=[None,'/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out','x','y']
exec(open('recompute.py').read().split('out={}')[0])
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
def owlist(d):
    L=open(os.path.join(d,'out.jsonl')).read().splitlines()
    meta=None; ev=[]
    for l in L:
        j=json.loads(l)
        if 'meta' in j: meta=j['meta']
        elif 'event' in j: ev.append(j['event'])
    t0=meta['t0_unix_ns']; off=meta.get('events_epoch_offset_ns',0)
    first={}
    for l in open(os.path.join(d,'rx_log.jsonl')):
        j=json.loads(l)
        if 'u' in j:
            first[j['u']]=min(first.get(j['u'],1e30),j['t'])
    sent={e['msg_id']:e['sent_ns'] for e in ev}
    return [ (first[i]-(t0+sent[i]-off))/1e9 if i in first else INF for i in range(meta['msg_count'])]
for c in ['b_r12k_d1000_l30_p1000','b_r12k_d1000_l20_p1000','b_r1k_d300_l20_p1000']:
    for a in ['taban','aday','quinn']:
        ad=os.path.join(ROOT,c,a)
        bins=[[] for _ in range(5)]; dl={x:[] for x in (1.5,2,3,5,10)}
        for rep in os.listdir(ad):
            ow=owlist(os.path.join(ad,rep))
            n=len(ow)
            for i,x in enumerate(ow): bins[i*5//n].append(min(x,60))
            for x in dl: dl[x].append(sum(1 for v in ow if v<=x)/n)
        print(c,a,'ow p50 by msg-quintile:',[round(statistics.median(b),2) for b in bins],'| deliv@ow<=s median:',{x:round(statistics.median(v),3) for x,v in dl.items()})
