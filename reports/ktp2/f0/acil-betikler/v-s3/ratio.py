import gzip,csv,statistics as st
f='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz'
rows=list(csv.DictReader(gzip.open(f,'rt')))
def fl(x):
    try: return float(x)
    except: return None
def med(c,a,col):
    v=[fl(r[col]) for r in rows if r['cell']==c and r['arm']==a]; v=[x for x in v if x is not None]
    return st.median(v) if v else None
for d in ['d1000','d300']:
    for l in ['l0','l10','l20','l30']:
        for p in ['p1000','p65']:
            c='b_r12k_%s_%s_%s'%(d,l,p)
            t=med(c,'taban','conf_p95_s'); q=med(c,'quinn','conf_p95_s'); sw=med(c,'taban','sender_wait_p95_s'); rw=med(c,'taban','recovery_wait_p95_s')
            if t is None: continue
            print(c,'taban p95 %.2f quinn %.2f diff %.2f sw %.2f rw %.2f ratio %.2f'%(t,q,t-q,sw,rw,sw/(t-q) if t!=q else float('nan')))
