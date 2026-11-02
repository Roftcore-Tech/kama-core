import gzip,os,sys,json
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000'
for spec in sys.argv[1:]:
    arm,r=spec.split(':')
    d=os.path.join(R,arm,'rep'+r)
    z=json.loads(open(os.path.join(d,'out.jsonl')).readline())['meta']['t0_unix_ns']
    rows=[]
    with gzip.open(os.path.join(d,'capb_cli.csv.gz'),'rt') as f:
        for ln in f:
            if ln.startswith('#'): continue
            p=ln.split(',',6)
            if p[1]=='o': rows.append(((int(p[0])-z)/1e9,int(p[3])))
    W=6; n=int(rows[-1][0]//W)+1
    b=[0]*n
    for t,l in rows: b[int(t//W)]+=l
    print(spec,'end %.0fs'%rows[-1][0],' '.join('%d'%(x/W) for x in b))
