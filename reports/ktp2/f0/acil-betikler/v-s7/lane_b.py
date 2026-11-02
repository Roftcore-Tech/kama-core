import json, os, glob, csv, sys, collections
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
res={}
for arm in ['taban','aday']:
    nrun=0; par=0; runs_par=0; btl7875=0; rows=0; rows_ge63=0; fec_bytes=0
    r25=collections.Counter(); r25b=collections.Counter()
    maxfinal_cell=collections.defaultdict(int)
    keys=set()
    for rep in glob.glob(os.path.join(base,'*',arm,'rep*')):
        f=os.path.join(rep,'out.jsonl')
        if not os.path.exists(f): continue
        cell=rep.split('/')[-3]
        with open(f) as fh:
            meta=json.loads(fh.readline()).get('meta',{})
        act=meta.get('activation') or {}
        keys|=set(act.keys())
        nrun+=1
        p=act.get('parity_emitted',0) or 0
        par+=p; runs_par+= p>0
        fb=(meta.get('tx') or {}).get('fec',0) or 0
        fec_bytes+=fb
        if (act.get('btlbw_bps') or 0)>=7875: btl7875+=1
        if cell.startswith('b_r25_'):
            r25[cell]+=p; r25b[cell]+=fb
        ts=os.path.join(rep,'ts.csv')
        if os.path.exists(ts):
            with open(ts) as th:
                rd=csv.reader(th); h=next(rd)
                ic=h.index('confirmed_prop'); isd=h.index('structure_prop'); isp=h.index('spec_prop'); ifr=h.index('final_ratio')
                for row in rd:
                    pre=max(int(row[ic]),int(row[isd]),int(row[isp]))
                    if pre>0:
                        rows+=1
                        fr=int(row[ifr])
                        if fr>=63: rows_ge63+=1
                        if cell=='b_r12k_d1000_l30_p1000':
                            maxfinal_cell['rows']+=1
                            if fr<63: maxfinal_cell['lt63']+=1
                            maxfinal_cell['max']=max(maxfinal_cell['max'],fr)
    print(arm,'runs',nrun,'parity',par,'runs_with_parity',runs_par,'fec_bytes',fec_bytes,'btl>=7875',btl7875)
    print('  rows pre>0',rows,'final>=63',rows_ge63,'%.1f%%'%(100*rows_ge63/rows if rows else 0))
    print('  big cell',dict(maxfinal_cell))
    print('  r25 parity',sum(r25.values()),'bytes',sum(r25b.values()),{k:(v,r25b[k]) for k,v in r25.items() if v})
    print('  act keys', sorted(keys))
