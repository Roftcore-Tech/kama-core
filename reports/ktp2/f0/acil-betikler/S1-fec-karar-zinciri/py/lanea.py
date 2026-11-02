import os, sys, json, collections
ROOT=sys.argv[1]
base=os.path.join(ROOT,'a','out'); agg=collections.defaultdict(lambda: [0,0,[],0])
for cell in sorted(os.listdir(base)):
    cd=os.path.join(base,cell)
    if not os.path.isdir(cd): continue
    for arm in ('taban','aday'):
        ad=os.path.join(cd,arm)
        if not os.path.isdir(ad): continue
        for rep in os.listdir(ad):
            for role in ('alice','bob'):
                p=os.path.join(ad,rep,role+'.jsonl')
                if not os.path.isfile(p): continue
                mpar=0; bws=[]
                for line in open(p,errors='replace'):
                    if '"k":"ts"' not in line: continue
                    try: o=json.loads(line)
                    except: continue
                    for s in o.get('s',[]):
                        mpar=max(mpar, s.get('par',0) or 0)
                        if s.get('bw'): bws.append(s['bw'])
                a=agg[(cell,role)]; a[0]+=1; a[1]+= (mpar>0); a[3]+=mpar
                if bws: a[2].append(max(bws))
for k,v in sorted(agg.items()):
    b=sorted(v[2]); print(k,'runs',v[0],'runs_par>0',v[1],'sum_par',v[3],'bw_max p50',b[len(b)//2] if b else None,'max',b[-1] if b else None)
