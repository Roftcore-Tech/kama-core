import json,os,sys,glob,statistics as st,csv
root=sys.argv[1]
rows={}
for cell in sorted(glob.glob(root+'/c_*')):
    if not os.path.isdir(cell): continue
    for arm in sorted(os.listdir(cell)):
        ad=os.path.join(cell,arm)
        if not os.path.isdir(ad): continue
        acc=dict(n=0,par=[],den=[],bw=[],tokmax=[],tokmed=[],ratio_nz=[],pend_max=[],fecb=[])
        for rep in glob.glob(ad+'/rep*'):
            try:
                with open(rep+'/out.jsonl') as f: meta=json.loads(f.readline())['meta']
            except Exception: continue
            a=meta.get('activation',{})
            acc['n']+=1
            acc['par'].append(a.get('parity_emitted',0)); acc['den'].append(a.get('parity_token_denied',0)); acc['bw'].append(a.get('btlbw_bps',0))
            acc['fecb'].append(meta['tx']['fec'])
            try:
                with open(rep+'/ts.csv') as f:
                    r=list(csv.DictReader(f))
                toks=[int(x['pacer_tokens']) for x in r]
                acc['tokmax'].append(max(toks)); acc['tokmed'].append(st.median(toks))
                acc['ratio_nz'].append(sum(1 for x in r if int(x['final_ratio'])>0))
                acc['pend_max'].append(max(int(x['pending_parity']) for x in r))
            except Exception as e: pass
        if acc['n']==0: continue
        m=lambda l: (st.median(l) if l else None)
        print(os.path.basename(cell),arm,'n=%d'%acc['n'],'par_med=%s'%m(acc['par']),'par_sum=%d'%sum(acc['par']),'den_med=%s'%m(acc['den']),'bw_med=%s'%m(acc['bw']),'tokmax_med=%s'%m(acc['tokmax']),'tokmed_med=%s'%m(acc['tokmed']),'ratioNZsamples_sum=%s'%sum(acc['ratio_nz']),'pendmax_max=%s'%(max(acc['pend_max']) if acc['pend_max'] else None))
