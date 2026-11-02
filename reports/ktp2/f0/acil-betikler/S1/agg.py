import json,sys,collections
rows=json.load(open(sys.argv[1]))
g=collections.defaultdict(list)
for r in rows: g[(r['lane'],r['kama_arm'])].append(r)
for k,v in sorted(g.items(),key=lambda x:str(x[0])):
    bt=sorted(x['btlbw'] or 0 for x in v)
    print(k,'runs',len(v),'par_emit_sum',sum(x['par'] or 0 for x in v),'fec_bytes',sum(x['fecb'] or 0 for x in v),
      'runs_pre>0',sum(1 for x in v if x['pre']>0),'runs_fin>0',sum(1 for x in v if x['fin']>0),'runs_rfin>0',sum(1 for x in v if x['r_any']>0),
      'maxpre',max(x['maxpre'] for x in v),'maxfin',max(x['maxfin'] for x in v),
      'btlbw p0/50/100',bt[0],bt[len(bt)//2],bt[-1])
