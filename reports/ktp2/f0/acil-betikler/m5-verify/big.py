import sys, re, bisect, statistics as st
sys.path.insert(0, sys.argv[1])
from vlib import *
arms=sys.argv[2:] or ['taban','aday']
for arm in arms:
    tokN=0; tokHi=0
    sh_n=0; sh_r=0; all_n=0; all_r=0
    little=[]; f2r=[]; f2a=[]
    ev2r_nq=[]; ev2r_q=[]
    for rep in reps(arm):
        R=load(arm,rep); base=R['t0']-R['off']
        m=re.search(r'left_preconv=(\d+)ms',R['err']); lp=base+int(m.group(1))*1_000_000
        aout=[p for p in R['cli'] if p['dir']=='o']
        bin_t={}
        for p in R['srv']:
            if p['dir']=='i': bin_t.setdefault(p['pn'],p['t'])
        data=[p for p in aout if p['type']=='data']
        first={}
        for p in data: first.setdefault(p['unit'],p['t'])
        offer=R['offer']  # msg_id == unit id (no frag)
        offs=sorted(offer.values()); fts=sorted(first.values())
        def queue(t):
            return bisect.bisect_right(offs,t)-bisect.bisect_right(fts,t)
        ts=R['ts']; tst=[base+int(r['t_ms'])*1_000_000 for r in ts]
        for r,t in zip(ts,tst):
            if queue(t)>=1:
                tokN+=1
                if int(r['pacer_tokens'])>=1027: tokHi+=1
        # retx share at emission
        seen=set()
        for p in data:
            isr = p['unit'] in seen
            seen.add(p['unit'])
            if p['t']<lp: continue
            i=bisect.bisect_right(tst,p['t'])-1
            pl=int(ts[i]['pending_lost']) if i>=0 else 0
            all_n+=1; all_r+=isr
            if queue(p['t']-1)>=1 and pl>0:
                sh_n+=1; sh_r+=isr
        # Little: mean pending_lost * duration / number of retx (post-PC)
        nretx=sum(1 for p in data if p['t']>=lp)-sum(1 for u,t in first.items() if t>=lp)
        # integrate pending_lost over ts samples
        integ=0.0
        for k in range(1,len(ts)):
            integ+=int(ts[k-1]['pending_lost'])*(tst[k]-tst[k-1])/1e9
        if nretx>0: little.append(integ/nretx)
        # per lost-first-tx unit: first send -> first arrival, first send -> first retx
        byu={}
        for p in data: byu.setdefault(p['unit'],[]).append(p)
        for u,lst in byu.items():
            if lst[0]['t']<lp: continue
            if lst[0]['pn'] in bin_t: continue
            arr=[bin_t[q['pn']] for q in lst if q['pn'] in bin_t]
            if arr: f2a.append((min(arr)-lst[0]['t'])/1e9)
            if len(lst)>1: f2r.append((lst[1]['t']-lst[0]['t'])/1e9)
        # evidence (first ACK at client covering a higher PN) -> next retx of that unit
        low2full={p['pn']&0xFFFF:p['pn'] for p in aout}
        acks=[p for p in R['cli'] if p['dir']=='i' and p['type']=='ack' and 'acked16' in p]
        maxcov=[]  # (t, max acked full pn)
        cur=-1
        for a in acks:
            mx=max((low2full.get(x,-1) for x in a['acked16']),default=-1)
            cur=max(cur,mx); maxcov.append((a['t'],cur))
        mt=[x[0] for x in maxcov]; mv=[x[1] for x in maxcov]
        for u,lst in byu.items():
            for j,q in enumerate(lst[:-1]):
                if q['t']<lp or q['pn'] in bin_t: continue
                # first ack time where cumulative max covered > q.pn
                k=next((i for i in range(len(mv)) if mv[i]>q['pn'] and mt[i]>q['t']),None)
                if k is None: continue
                te=mt[k]; nxt=lst[j+1]['t']
                if nxt<te: continue
                (ev2r_q if queue(te)>=1 else ev2r_nq).append((nxt-te)/1e9)
    print('==',arm)
    print(' tokens: queue>=1 ts samples',tokN,'tokens>=1027',tokHi, '%.2f%%'%(100*tokHi/max(tokN,1)))
    print(' retx share (post-PC emissions) queue>0&pl>0: n=%d share=%.3f ; overall n=%d share=%.3f'%(sh_n,sh_r/max(sh_n,1),all_n,all_r/max(all_n,1)))
    print(' little mean wait per retx: median over runs %.2f s'%st.median(little))
    print(' lost-first units: first->first arrival median %.2f s (n=%d); first->retx median %.2f s (n=%d)'%(st.median(f2a),len(f2a),st.median(f2r),len(f2r)))
    if ev2r_nq: print(' evidence->retx no-queue median %.2f n=%d'%(st.median(ev2r_nq),len(ev2r_nq)))
    print(' evidence->retx queue median %.2f p90 %.2f n=%d'%(st.median(ev2r_q),pctl(ev2r_q,0.9),len(ev2r_q)))
