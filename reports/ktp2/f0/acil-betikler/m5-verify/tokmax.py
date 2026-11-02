import sys, bisect, collections
sys.path.insert(0, sys.argv[1])
from vlib import *
for arm in ('taban',):
    c=collections.Counter(); n=0
    for rep in reps(arm):
        R=load(arm,rep); base=R['t0']-R['off']
        offs=sorted(R['offer'].values()); first={}
        for p in R['cli']:
            if p['dir']=='o' and p['type']=='data': first.setdefault(p['unit'],p['t'])
        fts=sorted(first.values())
        for r in R['ts']:
            t=base+int(r['t_ms'])*1_000_000
            if bisect.bisect_right(offs,t)-bisect.bisect_right(fts,t)>=1:
                tk=int(r['pacer_tokens'])
                if tk>=1027: c[tk]+=1
    print(arm,'tokens>=1027 values while queued:',sorted(c.items()))
