import pickle,statistics as st,math
S='/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-m3/'
res=pickle.load(open(S+'res_b_r12k_d1000_l30_p1000.pkl','rb'))
for arm in ('taban','aday','quinn'):
    for nb in (10,12):
        out=[]
        for b in range(nb):
            lo=b*100//nb; hi=(b+1)*100//nb
            v=[r['rxow'][i] for r in res[arm] for i in range(lo,hi) if i in r['rxow']]
            out.append(st.median(v))
        print(arm,nb,' '.join('%.1f'%x for x in out))
    if arm!='quinn':
        out=[]
        for b in range(10):
            v=[r['sw'][i] for r in res[arm] for i in range(b*10,b*10+10) if i in r['sw']]
            out.append(st.median(v))
        print(arm,'sender_wait deciles',' '.join('%.1f'%x for x in out))
