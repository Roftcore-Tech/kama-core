import os, csv, re, json, sys, collections
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
rows=json.load(open(sys.argv[1]))
# r25 parity runs: partial-only?
r25=[r for r in rows if r['lane']=='b' and r['cell'].startswith('b_r25') and r['arm'] in('taban','aday') and (r['fec_repairs_tx'] or 0)>0]
print('r25 parity runs',len(r25),'partial-only',sum(1 for r in r25 if r['fec_blocks']==r['partial_blocks']))
# first-block close time vs final_ratio
for cell in ['b_r12k_d300_l30_p1000','b_r12k_d300_l30_p65','b_r1k_d300_l30_p65','b_r12k_d1000_l30_p1000','b_r1k_d1000_l30_p65']:
    cnt=collections.Counter()
    for arm in ('taban','aday'):
        d=os.path.join(ROOT,cell,arm)
        for rep in os.listdir(d):
            p=os.path.join(d,rep); cmd=open(os.path.join(p,'cmd.txt')).read()
            iv=int(re.search(r'submit-interval-ms (\d+)',cmd).group(1))
            # t0 approx: first ts sample time? use flow start ~ ts t_ms offset; take block close = first sample with t>=7*iv + t_first
            ts=list(csv.DictReader(open(os.path.join(p,'ts.csv'))))
            if not ts: continue
            t_start=int(ts[0]['t_ms'])
            tc=t_start+7*iv
            s=[r for r in ts if int(r['t_ms'])>=tc]
            if not s: cnt['nosample']+=1; continue
            fr=int(s[0]['final_ratio'])
            pre=max(int(s[0]['confirmed_prop']),int(s[0]['structure_prop']),int(s[0]['spec_prop']))
            k = 'r>=1' if (fr*8+500)//1000>=1 else ('fin0_pre>0' if fr==0 and pre>0 else ('fin0_pre0' if fr==0 else 'fin1-62'))
            cnt[k]+=1
    print(cell, 'iv', iv, dict(cnt))
