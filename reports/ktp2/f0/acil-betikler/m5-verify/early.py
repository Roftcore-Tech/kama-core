import sys, re
sys.path.insert(0, sys.argv[1])
from vlib import *
for arm in ('taban','aday'):
    print('==',arm)
    c174=c1374=co=0
    for rep in reps(arm):
        R=load(arm,rep)
        t0=R['t0']; off=R['off']; base=t0-off  # ts t_ms relative to base? use offer epoch
        tok0=int(R['ts'][0]['pacer_tokens'])
        if tok0==174: c174+=1
        elif tok0==1374: c1374+=1
        else: co+=1
        aout=[p for p in R['cli'] if p['dir']=='o' and p['type']=='data']
        bin_pn={p['pn']:p['t'] for p in R['srv'] if p['dir']=='i'}
        first={}
        for p in aout: first.setdefault(p['unit'],p)
        u0=[p for p in aout if p['unit']==0]
        u0first_lost = u0[0]['pn'] not in bin_pn
        u1t=(first[1]['t']-base)/1e9 if 1 in first else None
        lp=re.search(r'left_preconv=(\d+)ms',R['err']).group(1)
        lat=[v for v in R['lat'].values()]
        dl=sum(1 for v in lat if v is not None)
        p95=pctl([v if v is not None else 1e9 for v in lat],0.95)
        print(rep,'tok0',tok0,'u0_first_lost',u0first_lost,'u0_pns',[(p['pn'], p['pn'] in bin_pn) for p in u0][:4],'u1_first_s',round(u1t,3) if u1t else None,'left_preconv_ms',lp,'delivered',dl,'p95',round(p95,1) if p95<1e8 else 'inf')
    print('tok0 174',c174,'1374',c1374,'other',co)
