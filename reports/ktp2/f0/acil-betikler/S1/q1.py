import json,sys,collections
rows=json.load(open(sys.argv[1]))
for r in rows:
    if r['lane'] in('c','d') and 'adaptive-p0x05' in r['kama_arm'] and (r['par'] or 0)>0:
        print(r['lane'],r['cell'],r['arm'],r['rep'],'par',r['par'],'fecb',r['fecb'],'btlbw',r['btlbw'],'fin',r['fin'],'rany',r['r_any'],r['finvals'])
# lane b by cell
g=collections.defaultdict(list)
for r in rows:
    if r['lane']=='b': g[(r['cell'],r['arm'])].append(r)
for k,v in sorted(g.items()):
    bt=sorted(x['btlbw'] or 0 for x in v)
    print(k,len(v),'par',sum(x['par'] or 0 for x in v),'runs_par>0',sum(1 for x in v if (x['par'] or 0)>0),'rany_runs',sum(1 for x in v if x['r_any']>0),'btlbw med',bt[len(bt)//2],'max',bt[-1])
