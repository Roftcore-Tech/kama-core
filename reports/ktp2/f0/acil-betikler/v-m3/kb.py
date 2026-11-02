import csv,gzip,sys,statistics as st
p='/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz'
cell=sys.argv[1]
cols=sys.argv[2].split(',')
rows=[r for r in csv.DictReader(gzip.open(p,'rt')) if r['cell']==cell]
def f(x):
    try: return float(x)
    except: return None
for arm in ['taban','aday','quinn','tcp']:
    rr=[r for r in rows if r['arm']==arm]
    out=[arm,len(rr)]
    for c in cols:
        v=[f(r.get(c,'')) for r in rr]; v=[x for x in v if x is not None]
        out.append(f"{c}={st.median(v):.4g}[{min(v):.4g},{max(v):.4g}]n{len(v)}" if v else f"{c}=NA")
    print(*out)
