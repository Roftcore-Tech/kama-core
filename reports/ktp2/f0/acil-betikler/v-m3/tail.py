import pickle,sys,statistics as st,math
cell=sys.argv[1]
res=pickle.load(open('/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/res_%s.pkl'%cell,'rb'))
def pct(v,q):
    v=sorted(v); k=(len(v)-1)*q; f=math.floor(k); c=math.ceil(k); return v[f]+(v[c]-v[f])*(k-f)
for arm in ('taban','aday'):
    runs=res[arm]
    shares=[];swt=[];pot=[];multi_post=[];single_post=[];cf=[]
    for r in runs:
        c=r['confi']; 
        if len(c)<20: continue
        thr=pct(list(c.values()),.95)
        top=[i for i in c if c[i]>=thr and i in r['sw']]
        for i in top:
            swt.append(r['sw'][i]); pot.append(r['post'][i]); shares.append(r['sw'][i]/c[i])
        for i in r['post']:
            (multi_post if r['ntx'].get(i,0)>1 else single_post).append(r['post'][i])
        # counterfactual: p95 of post
    print(arm,'top5%% msgs: n=%d sw mean %.2f post mean %.2f sw-share median %.2f'%(len(swt),st.mean(swt),st.mean(pot),st.median(shares)))
    print(arm,'post for single-tx msgs p50 %.2f p95 %.2f n=%d ; multi-tx p50 %.2f p95 %.2f n=%d'%(pct(single_post,.5),pct(single_post,.95),len(single_post),pct(multi_post,.5),pct(multi_post,.95),len(multi_post)))
    # pooled post percentiles
