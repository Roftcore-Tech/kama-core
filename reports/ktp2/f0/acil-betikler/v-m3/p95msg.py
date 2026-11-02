import pickle,sys,statistics as st,math
cell=sys.argv[1]
res=pickle.load(open('/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/res_%s.pkl'%cell,'rb'))
def pct(v,q):
    v=sorted(v); k=(len(v)-1)*q; f=math.floor(k); c=math.ceil(k); return v[f]+(v[c]-v[f])*(k-f)
for arm in ('taban','aday'):
    sws=[];pos=[];sh=[];multi=0;tot=0
    band_sw=[];band_po=[]
    for r in res[arm]:
        c=r['confi']
        if len(c)<90: continue
        order=sorted(c,key=lambda i:c[i])
        # messages ranked 90..97 percentile band around p95
        n=len(order)
        band=order[int(0.90*n):int(0.98*n)]
        for i in band:
            if i in r['sw']:
                band_sw.append(r['sw'][i]); band_po.append(r['post'][i]); tot+=1
                if r['ntx'].get(i,0)>1: multi+=1
        i95=order[min(n-1,int(round(0.95*(n-1))))]
        if i95 in r['sw']:
            sws.append(r['sw'][i95]); pos.append(r['post'][i95]); sh.append(r['sw'][i95]/c[i95])
    print(arm,'p95-rank msg: median sw %.2f post %.2f sw-share %.2f (n=%d)'%(st.median(sws),st.median(pos),st.median(sh),len(sws)))
    print(arm,'p90-p98 band: mean sw %.2f mean post %.2f ; sw share of sum %.2f ; frac multi-tx %.2f (n=%d)'%(st.mean(band_sw),st.mean(band_po),sum(band_sw)/(sum(band_sw)+sum(band_po)),multi/tot,tot))
    # counterfactual p95 of post (if first-tx wait = 0)
    print(arm,'median over runs of p95(conf - sw) = %.2f'%st.median([pct(list(r['post'].values()),.95) for r in res[arm]]))
