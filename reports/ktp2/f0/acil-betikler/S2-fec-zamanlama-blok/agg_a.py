import json,sys,glob,os,statistics as st
root=sys.argv[1]
for cell in sorted(glob.glob(root+'/a_*')):
    if not os.path.isdir(cell): continue
    for arm in ('taban','aday'):
        pars=[];bws=[];prs=[];n=0;txd=[]
        for rep in glob.glob(f'{cell}/{arm}/rep*'):
            p=rep+'/alice.jsonl'
            if not os.path.exists(p): continue
            last=None; prs_run=[]
            for l in open(p):
                if '"k":"ts"' not in l: continue
                o=json.loads(l)
                if o['s']:
                    last=o['s'][0]
                    if last['st'] not in ('Idle','PreConverge') and last.get('pr'): prs_run.append(last['pr'])
            if last is None: continue
            n+=1; pars.append(last['par']); bws.append(last['bw']); txd.append(last['txd'])
            if prs_run: prs.append(st.median(prs_run))
        if n: print(os.path.basename(cell),arm,'n',n,'par_sum',sum(pars),'runs_par>0',sum(1 for x in pars if x>0),'bw_end_med',st.median(bws),'pr_active_med',st.median(prs) if prs else None,'pr_active_min',min(prs) if prs else None,'pr_active_max',max(prs) if prs else None,'txd_med',st.median(txd))
