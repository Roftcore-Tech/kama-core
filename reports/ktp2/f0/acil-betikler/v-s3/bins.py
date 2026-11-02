import gzip,json,os,sys,statistics as st
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
off=3702.0
for c in sys.argv[1:]:
    for arm in ['taban','aday','quinn']:
        bins={}
        for r in range(1,31):
            d=os.path.join(R,c,arm,'rep%d'%r)
            try: z=json.loads(open(os.path.join(d,'out.jsonl')).readline())['meta']['t0_unix_ns']
            except Exception: continue
            rows=[]
            with gzip.open(os.path.join(d,'capb_cli.csv.gz'),'rt') as f:
                for ln in f:
                    if ln.startswith('#'): continue
                    p=ln.split(',',6)
                    if p[1]=='o': rows.append((int(p[0]),int(p[3])))
            z=rows[0][0] if arm=='quinn' else z
            b=[0]*12
            for t,l in rows:
                k=int(((t-z)/1e9)//3)
                if 0<=k<12: b[k]+=l
            for k in range(12): bins.setdefault(k,[]).append(b[k]/3/off)
        print(c,arm,' '.join('%.2f'%st.median(bins[k]) for k in range(12)))
