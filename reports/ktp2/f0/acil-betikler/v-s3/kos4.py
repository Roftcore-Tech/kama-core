import gzip,csv,statistics as st
f='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz'
rows=list(csv.DictReader(gzip.open(f,'rt')))
def fl(x):
    try: return float(x)
    except: return None
c='b_r12k_d1000_l30_p1000'
for arm in ['taban','aday']:
    rs=[r for r in rows if r['cell']==c and r['arm']==arm]
    e=[r for r in rs if fl(r['probe_up'])>=1]; ne=[r for r in rs if fl(r['probe_up'])==0]
    for nm,g in [('entered',e),('never',ne)]:
        if not g: continue
        bw=[fl(r['est_btlbw']) for r in g]; p95=[fl(r['conf_p95_s']) for r in g]
        p95v=[x for x in p95 if x is not None]
        print(arm,nm,len(g),'btlbw',min(bw),max(bw),'p95 range %.1f-%.1f med %.1f'%(min(p95v),max(p95v),st.median(p95v)),'kilitli',[r['kilitli'] for r in g][:30])
    lp=[fl(r['left_preconv']) for r in rs]
    print(arm,'left_preconv>=29000ms',sum(1 for x in lp if x and x>=29000),[x for x in lp if x and x>=29000], 'max', max(lp))
for c2 in ['b_r12k_d1000_l20_p1000']:
    for arm in ['taban','aday']:
        rs=[r for r in rows if r['cell']==c2 and r['arm']==arm]
        lp=[fl(r['left_preconv']) for r in rs]
        print(c2,arm,'left_preconv>=29000ms',sum(1 for x in lp if x and x>=29000),sorted([x for x in lp if x and x>=29000]),'max',max(lp))
# taban l30 p95 excluding trapped
rs=[r for r in rows if r['cell']==c and r['arm']=='taban']
tr=[r for r in rs if fl(r['left_preconv'])>=29000]
print('trapped p95',[r['conf_p95_s'] for r in tr])
rest=[fl(r['conf_p95_s']) for r in rs if fl(r['left_preconv'])<29000]
print('taban l30 p95 med all %.2f, excl trapped %.2f n=%d'%(st.median([fl(r['conf_p95_s']) for r in rs]),st.median(rest),len(rest)))
