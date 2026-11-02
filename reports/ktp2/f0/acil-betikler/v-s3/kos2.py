import gzip,csv,statistics as st,collections
f='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz'
rows=list(csv.DictReader(gzip.open(f,'rt')))
def fl(x):
    try: return float(x)
    except: return None
k=[r for r in rows if r['arm'] in ('taban','aday')]
print('kama runs',len(k),'arms',collections.Counter(r['arm'] for r in rows))
par=[r for r in k if (fl(r['parity']) or 0)>0]
print('parity>0 runs',len(par))
txf=[r for r in k if (fl(r['tx_fec_B']) or 0)>0]
print('tx_fec_B>0 runs',len(txf))
d1=[r for r in k if '_d1000_' in r['cell'] and ((fl(r['parity']) or 0)>0 or (fl(r['parity_denied']) or 0)>0)]
print('d1000 with parity or denied', len(d1))
for r in sorted(d1,key=lambda r:(r['cell'],r['arm'],int(r['rep'])))[:60]:
    print(r['cell'],r['arm'],r['rep'],'parity',r['parity'],'denied',r['parity_denied'],'tx_fec_B',r['tx_fec_B'])
r12=[r for r in k if r['cell'].startswith('b_r12k')]
print('r12k kama runs',len(r12))
