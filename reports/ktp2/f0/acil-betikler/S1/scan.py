import json,os,sys,csv,glob,collections
root=sys.argv[1]
rows=[]
for oj in glob.glob(root+'/*/out/*/*/rep*/out.jsonl'):
    d=os.path.dirname(oj)
    parts=d.split('/')
    lane,cell,arm,rep=parts[-5],parts[-3],parts[-2],parts[-1]
    try:
        with open(oj) as f: meta=json.loads(f.readline()).get('meta',{})
    except Exception: continue
    if meta.get('transport')!='kama-product': continue
    act=meta.get('activation',{}) or {}
    ts=os.path.join(d,'ts.csv')
    pre=fin=n=0; maxpre=maxfin=0; finvals=collections.Counter(); conf=struct=spec=0
    r_any=0
    if os.path.exists(ts):
        with open(ts) as f:
            for row in csv.DictReader(f):
                n+=1
                c=int(row['confirmed_prop']);s=int(row['structure_prop']);sp=int(row['spec_prop']);fr=int(row['final_ratio'])
                p=max(c,s,sp)
                if p>0: pre+=1
                if c>0: conf+=1
                if s>0: struct+=1
                if sp>0: spec+=1
                if fr>0: fin+=1; finvals[fr]+=1
                maxpre=max(maxpre,p); maxfin=max(maxfin,fr)
                if (fr*8+500)//1000>0: r_any+=1
    rows.append(dict(lane=lane,cell=cell,arm=arm,rep=rep,kama_arm=meta.get('kama_arm'),btlbw=act.get('btlbw_bps'),par=act.get('parity_emitted'),fecb=meta.get('tx',{}).get('fec'),n=n,pre=pre,conf=conf,struct=struct,spec=spec,fin=fin,maxpre=maxpre,maxfin=maxfin,r_any=r_any,finvals=dict(finvals.most_common(4))))
json.dump(rows,open(sys.argv[2],'w'))
print(len(rows))
