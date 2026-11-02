import sys, bisect
sys.path.insert(0, sys.argv[1])
from vlib import *
for arm in ('taban','aday'):
    n=0; pl=0
    for rep in reps(arm):
        R=load(arm,rep); base=R['t0']-R['off']
        offs=sorted(R['offer'].values()); first={}
        for p in R['cli']:
            if p['dir']=='o' and p['type']=='data': first.setdefault(p['unit'],p['t'])
        fts=sorted(first.values())
        for r in R['ts']:
            t=base+int(r['t_ms'])*1_000_000
            if bisect.bisect_right(offs,t)-bisect.bisect_right(fts,t)>=1:
                n+=1; pl+= int(r['pending_lost'])>0
    print(arm,'queue>=1 samples',n,'pending_lost>0',pl,'%.1f%%'%(100*pl/n))
