import pickle,statistics as st
S='/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/'
res=pickle.load(open(S+'res_b_r12k_d1000_l30_p1000.pkl','rb'))
for arm in ('taban','quinn'):
    for nb in (5,10):
        out=[]
        for b in range(nb):
            lo=b*100//nb; hi=(b+1)*100//nb
            pr=[st.median([r['rxow'][i] for i in range(lo,hi) if i in r['rxow']]) for r in res[arm] if any(i in r['rxow'] for i in range(lo,hi))]
            out.append(st.median(pr))
        print(arm,nb,'median-of-run-medians',' '.join('%.1f'%x for x in out))
