import gzip,csv,statistics as st,sys,collections
f='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz'
rows=list(csv.DictReader(gzip.open(f,'rt')))
print('rows',len(rows))
cols=sys.argv[2].split(',')
cells=sys.argv[1].split(',')
def fl(x):
    try: return float(x)
    except: return None
for c in cells:
    for arm in ['taban','aday','quinn']:
        rs=[r for r in rows if r['cell']==c and r['arm']==arm]
        out=[]
        for col in cols:
            v=[fl(r[col]) for r in rs]; v=[x for x in v if x is not None]
            out.append('%s=%s'%(col, ('%.3f'%st.median(v)) if v else 'NA'))
        print(c,arm,len(rs),' '.join(out))
