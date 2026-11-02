import pickle,sys,csv,gzip,math,statistics as st,json,os
cell=sys.argv[1]
res=pickle.load(open('/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/res_%s.pkl'%cell,'rb'))
B={(r['arm'],r['rep']):r for r in csv.DictReader(gzip.open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz','rt')) if r['cell']==cell}
D={(r['arm'],r['rep']):r for r in csv.DictReader(open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/d/tam/kosular.csv')) if r['cell']==cell}
def pct_lin(v,q):
    v=sorted(v); k=(len(v)-1)*q; f=math.floor(k); c=math.ceil(k); return v[f]+(v[c]-v[f])*(k-f)
Dwin=60.0
for arm in ('taban','aday','quinn'):
    mmB=0;mmD=0;mmBrx=0;n=0;negs=0;cens=[]
    for r in res[arm]:
        rep=r['rep'][3:]
        n_msgs=100
        lat=[r['confi'].get(i,float('inf')) for i in range(n_msgs)]
        # censored: undelivered or >D -> inf
        latc=[x if x<=Dwin else float('inf') for x in lat]
        dl=[x for x in latc if x!=float('inf')]
        # B conf_p95 (delivered only?) 
        bv=float(B[(arm,rep)]['conf_p95_s']) if B[(arm,rep)]['conf_p95_s'] not in ('','nan') else None
        dv=D[(arm,rep)]['lat_p95_s']; dv=float(dv)
        mine_del=pct_lin([x for x in lat if x!=float('inf')],.95)
        # censored linear
        try: mine_c=pct_lin(latc,.95)
        except: mine_c=float('nan')
        if bv is not None and abs(bv-mine_del)>0.002: mmB+=1; print('B mismatch',arm,rep,bv,mine_del)
        if not (math.isinf(dv) and math.isinf(mine_c)) and abs(dv-mine_c)>0.002: mmD+=1; print('D mismatch',arm,rep,dv,mine_c,mine_del)
        rxb=float(B[(arm,rep)]['rx_ow_p95_s']); mine_rx=pct_lin(list(r['rxow'].values()),.95)
        if abs(rxb-mine_rx)>0.002: mmBrx+=1; print('Brx mismatch',arm,rep,rxb,mine_rx)
        negs+=sum(1 for v in r['rxow'].values() if v<0)+sum(1 for v in r['confi'].values() if v<0)
        delivB=float(B[(arm,rep)]['deliv_D']); delivD=float(D[(arm,rep)]['cD_ss'])
        mydel=sum(1 for x in latc if x!=float('inf'))/n_msgs
        if abs(delivB-mydel)>1e-9 or abs(delivD-mydel)>1e-9: print('deliv mismatch',arm,rep,delivB,delivD,mydel)
        n+=1
    print(arm,'runs',n,'B conf mism',mmB,'D lat mism',mmD,'B rx mism',mmBrx,'neg',negs)
    print(arm,'median D lat_p95 (inf as inf):',st.median([float(D[(arm,r['rep'][3:])]['lat_p95_s']) for r in res[arm]]))
