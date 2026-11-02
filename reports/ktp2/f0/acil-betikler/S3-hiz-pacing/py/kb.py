import sys, gzip, csv, statistics as st
f=sys.argv[1]; cells=sys.argv[2].split(','); cols=sys.argv[3].split(',')
rows=list(csv.DictReader(gzip.open(f,'rt')))
for c in cells:
    for arm in ['taban','aday','quinn','tcp']:
        rs=[r for r in rows if r['cell']==c and r['arm']==arm]
        if not rs: continue
        out=[]
        for k in cols:
            v=[]
            for r in rs:
                try: v.append(float(r[k]))
                except: pass
            out.append(f"{k}={st.median(v):.3f}" if v else f"{k}=NA")
        print(c, arm, len(rs), ' '.join(out))
