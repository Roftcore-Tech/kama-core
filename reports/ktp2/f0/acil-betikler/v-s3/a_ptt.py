import json,os,sys,statistics as st,collections
A='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/a/out'
for c in sys.argv[1:]:
    for arm in ['taban','aday']:
        pr=[]; base=os.path.join(A,c,arm); cls=collections.Counter()
        for rep in sorted(os.listdir(base)):
            f=os.path.join(base,rep,'alice.jsonl')
            if not os.path.exists(f): continue
            ts=[];pt=[]
            for ln in open(f):
                j=json.loads(ln)
                if j['k']=='send':
                    cls[j['c']]+=1
                    if j['c'] in ('P',): pt.append(j['due'])
                elif j['k']=='ts':
                    for s in j['s']:
                        if s.get('peer')=='bob': ts.append((j['t'],s['pr'],s['st']))
            if not pt: continue
            # windows: contiguous ptt clusters
            pt.sort(); wins=[]; a=pt[0]; b=pt[0]
            for x in pt[1:]:
                if x-b>2000: wins.append((a,b)); a=x
                b=x
            wins.append((a,b))
            for a,b in wins:
                pr += [p for t,p,s in ts if a<=t<=b+200 and s!='Idle']
        print(c,arm,'classes',dict(cls),'PTT-window pacer p50', st.median(pr) if pr else None, 'n',len(pr))
