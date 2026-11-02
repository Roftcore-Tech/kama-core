import gzip,os,re,sys
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000/aday'
for r in sys.argv[1:]:
    d=os.path.join(R,'rep'+r)
    rows=[]
    with gzip.open(os.path.join(d,'capb_cli.csv.gz'),'rt') as f:
        for ln in f:
            if ln.startswith('#'): continue
            p=ln.split(',',6)
            if p[1]=='o': rows.append((int(p[0]),int(p[3])))
    tend=rows[-1][0]
    s=sum(l for t,l in rows if t>=tend-19e9)/19
    bw=int(re.search(r'btlbw=(\d+)',open(os.path.join(d,'c.err')).read()).group(1))
    print(r,'btlbw_end',bw,'S_last19 %.0f'%s,'ratio %.2f'%(bw/s))
