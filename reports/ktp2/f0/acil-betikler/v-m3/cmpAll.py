import pickle,sys,csv,gzip,math,statistics as st
cell=sys.argv[1]; Dwin=float(sys.argv[2])
S='/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/'
res=pickle.load(open(S+'res_%s.pkl'%cell,'rb'))
B={(r['arm'],r['rep']):r for r in csv.DictReader(gzip.open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz','rt')) if r['cell']==cell}
D={(r['arm'],r['rep']):r for r in csv.DictReader(open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/d/tam/kosular.csv')) if r['cell']==cell}
def lin(v,q):
    v=sorted(v); k=(len(v)-1)*q; f=math.floor(k); c=math.ceil(k); return v[f]+(v[c]-v[f])*(k-f)
def F(x):
    return float(x) if x not in ('',None) else float('nan')
def close(a,b): return (math.isinf(a) and math.isinf(b)) or (math.isnan(a) and math.isnan(b)) or abs(a-b)<0.0005
for arm in ('taban','aday','quinn'):
    mm=[];n=0;neg=0
    for r in res[arm]:
        rep=r['rep'][3:]; N=r['n']
        lat=sorted([r['confi'].get(i,float('inf')) for i in range(N)])
        ow=sorted([r['rxow'].get(i,float('inf')) for i in range(N)])
        k=math.ceil(0.95*N)-1
        d=D[(arm,rep)]; b=B[(arm,rep)]
        chk={'D_lat_p95':close(float(d['lat_p95_s']),lat[k]),'D_ow_p95':close(float(d['ow_p95_s']),ow[k]),
             'D_cD':abs(float(d['cD_ss'])-sum(1 for x in lat if x<=Dwin)/N)<1e-9,'D_cDow':abs(float(d['cD_ow'])-sum(1 for x in ow if x<=Dwin)/N)<1e-9,
             'B_conf95':(close(F(b['conf_p95_s']),lin([x for x in lat if not math.isinf(x)],.95)) if any(not math.isinf(x) for x in lat) else math.isnan(F(b['conf_p95_s']))),
             'B_rx95':(close(F(b['rx_ow_p95_s']),lin([x for x in ow if not math.isinf(x)],.95)) if any(not math.isinf(x) for x in ow) else math.isnan(F(b['rx_ow_p95_s']))),
             'B_deliv':abs(float(b['deliv_D'])-sum(1 for x in lat if x<=Dwin)/N)<1e-9,
             'B_rxdeliv':abs(float(b['rx_deliv_D'])-sum(1 for x in ow if x<=Dwin)/N)<1e-9}
        bad=[k2 for k2,v in chk.items() if not v]
        if bad: mm.append((rep,bad))
        neg+=sum(1 for v in r['rxow'].values() if v<0)+sum(1 for v in r['confi'].values() if v<0)
        n+=1
    print(cell,arm,'runs',n,'mismatch runs',len(mm),mm[:4],'neg',neg,'med D lat_p95 %.3f'%st.median([float(D[(arm,r['rep'][3:])]['lat_p95_s']) for r in res[arm]]))
