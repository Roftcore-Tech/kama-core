import json,os,sys,statistics as st,collections
A='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/a/out'
for c in sys.argv[1:]:
    for arm in ['taban','aday']:
        pr=[];bw=[];txqmax=[];stc=collections.Counter();nrep=0
        base=os.path.join(A,c,arm)
        for rep in sorted(os.listdir(base)):
            f=os.path.join(base,rep,'alice.jsonl')
            if not os.path.exists(f): continue
            nrep+=1; tm=0
            for ln in open(f):
                if '"k":"ts"' not in ln: continue
                j=json.loads(ln)
                for s in j['s']:
                    if s.get('peer')!='bob': continue
                    if s['st'] in ('Idle',): 
                        stc['Idle']+=1; continue
                    pr.append(s['pr']); bw.append(s['bw']); stc[s['st']]+=1
                    tm=max(tm,s['rec']['txq'])
            txqmax.append(tm)
        tot=sum(v for k,v in stc.items() if k!='Idle')
        print(c,arm,'reps',nrep,'pr p50',st.median(pr),'bw p50',st.median(bw),'txq max med',st.median(txqmax),'max',max(txqmax),
              {k:round(v/tot,3) for k,v in stc.items() if k!='Idle'}, 'idle_frac_all %.2f'%(stc['Idle']/(tot+stc['Idle'])))
