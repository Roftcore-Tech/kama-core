import json, csv, os, sys, glob
root=sys.argv[1]
tot={}
for cell in sorted(d for d in os.listdir(root) if os.path.isdir(os.path.join(root,d))):
    for arm in ['taban','aday']:
        for rep in glob.glob(os.path.join(root,cell,arm,'rep*')):
            f=os.path.join(rep,'out.jsonl'); t=os.path.join(rep,'ts.csv')
            if not os.path.exists(f): continue
            L=open(f).readline()
            if not L: continue
            m=json.loads(L)['meta']; a=m.get('activation') or {}
            k=(arm,)
            s=tot.setdefault(arm,dict(runs=0,par=0,runs_par=0,rows=0,pre=0,pre_hi=0,pre_fin0=0,btl_ge=0))
            s['runs']+=1; p=a.get('parity_emitted') or 0; s['par']+=p; s['runs_par']+= (p>0)
            if (a.get('btlbw_bps') or 0) >= 7875: s['btl_ge']+=1
            if os.path.exists(t):
                for row in csv.DictReader(open(t)):
                    s['rows']+=1
                    pre=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop']))
                    fin=int(row['final_ratio'])
                    if pre>0:
                        s['pre']+=1
                        if fin>=63: s['pre_hi']+=1
print(tot)
