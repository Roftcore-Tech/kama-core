import gzip,csv,statistics as st,collections
f='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz'
rows=list(csv.DictReader(gzip.open(f,'rt')))
def fl(x):
    try: return float(x)
    except: return 0.0
k=[r for r in rows if r['arm'] in ('taban','aday')]
for sel,name in [(lambda r:'_d1000_' in r['cell'],'d1000'),(lambda r:'_d300_' in r['cell'],'d300'),(lambda r:True,'all')]:
    s=[r for r in k if sel(r) and fl(r['parity'])>0]
    if s: print(name,'parity>0 n',len(s),'parity range',min(fl(r['parity']) for r in s),max(fl(r['parity']) for r in s),'denied range',min(fl(r['parity_denied']) for r in s),max(fl(r['parity_denied']) for r in s))
r12=[r for r in k if r['cell'].startswith('b_r12k')]
print('r12k kama',len(r12))
# peak final_ratio not in kosular? check columns containing 'ratio'
print([c for c in rows[0].keys() if 'ratio' in c or 'final' in c or 'cap' in c])
# probe_up l30 d1000
for c in ['b_r12k_d1000_l10_p1000','b_r12k_d1000_l20_p1000','b_r12k_d1000_l30_p1000']:
    for arm in ['taban','aday']:
        rs=[r for r in k if r['cell']==c and r['arm']==arm]
        pu=[fl(r['probe_up']) for r in rs]
        print(c,arm,'probe_up med',st.median(pu),'zero',sum(1 for x in pu if x==0),'>=1',sum(1 for x in pu if x>=1))
