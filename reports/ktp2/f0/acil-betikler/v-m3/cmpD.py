import pickle,sys,csv,math,statistics as st
cell=sys.argv[1]
res=pickle.load(open('/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/res_%s.pkl'%cell,'rb'))
D={(r['arm'],r['rep']):r for r in csv.DictReader(open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/d/tam/kosular.csv')) if r['cell']==cell}
Dwin=float(sys.argv[2]) if len(sys.argv)>2 else 60.0
for arm in ('taban','aday','quinn'):
    mm=0;n=0
    for r in res[arm]:
        rep=r['rep'][3:]
        lat=sorted([ (r['confi'][i] if (i in r['confi'] and r['confi'][i]<=Dwin) else float('inf')) for i in range(100)])
        for name,idx in (('nr',math.ceil(0.95*100)-1),):
            v=lat[idx]
        dv=float(D[(arm,rep)]['lat_p95_s'])
        ok=(math.isinf(dv) and math.isinf(v)) or abs(dv-v)<0.0005
        # ow
        ow=sorted([ (r['rxow'][i] if (i in r['rxow'] and r['rxow'][i]<=Dwin) else float('inf')) for i in range(100)])
        dow=float(D[(arm,rep)]['ow_p95_s']); okow=(math.isinf(dow) and math.isinf(ow[94])) or abs(dow-ow[94])<0.0005
        cD=float(D[(arm,rep)]['cD_ss']); cDow=float(D[(arm,rep)]['cD_ow'])
        myc=sum(1 for x in lat if not math.isinf(x))/100; mycow=sum(1 for x in ow if not math.isinf(x))/100
        if not ok or not okow or abs(cD-myc)>1e-9 or abs(cDow-mycow)>1e-9:
            mm+=1; print('mism',arm,rep,dv,v,dow,ow[94],cD,myc,cDow,mycow)
        n+=1
    print(arm,'n',n,'D mismatches (nearest-rank, censored at D)',mm)
