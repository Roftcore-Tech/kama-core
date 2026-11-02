import sys, statistics as st
sys.path.insert(0, sys.argv[1])
from vlib import *
for arm in ('taban','aday'):
    A=[];B=[];C=[];tA=[];tB=[];tC=[]
    for rep in reps(arm):
        R=load(arm,rep)
        bin_t={}
        for p in R['srv']:
            if p['dir']=='i' and p['type']=='data': bin_t.setdefault(p['pn'],(p['t'],p['unit']))
        fa={}
        for pn,(t,u) in bin_t.items():
            if u not in fa or t<fa[u]: fa[u]=t
        ft={}
        for p in R['cli']:
            if p['dir']=='o' and p['type']=='data': ft.setdefault(p['unit'],p['t'])
        rows=[]
        for u,o in R['offer'].items():
            c=R['conf'].get(u)
            if c is None or u not in ft or u not in fa: continue
            rows.append(((ft[u]-o)/1e9,(fa[u]-ft[u])/1e9,(c-fa[u])/1e9,(c-o)/1e9))
        p95=pctl([r[3] for r in rows],0.95)
        for a,b,cc,l in rows:
            A.append(a);B.append(b);C.append(cc)
            if l>=p95: tA.append(a);tB.append(b);tC.append(cc)
    T=sum(tA)+sum(tB)+sum(tC)
    print(arm,'all: meanA %.2f B %.2f C %.2f | tail n=%d meanB %.2f shareA %.0f%% B %.0f%% C %.0f%%'%(st.mean(A),st.mean(B),st.mean(C),len(tA),st.mean(tB),100*sum(tA)/T,100*sum(tB)/T,100*sum(tC)/T))
