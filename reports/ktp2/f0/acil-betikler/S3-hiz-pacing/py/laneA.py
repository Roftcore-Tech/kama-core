import sys, os, glob, json, statistics as st, collections
root=sys.argv[1]; cells=sys.argv[2].split(',')
for cell in cells:
    for arm in ['taban','aday']:
        prs=[]; bws=[]; stc=collections.Counter(); ptt_pr=[]; ptt_txq=[]; n=0; pl_pos=0
        for p in sorted(glob.glob(os.path.join(root,cell,arm,'rep*'))):
            f=os.path.join(p,'alice.jsonl')
            if not os.path.exists(f): continue
            t0=None; ptt=None
            for l in open(f):
                j=json.loads(l)
                if j.get('k')=='hdr': ptt=j['params'].get('ptt_at_ms'); pb=j['params'].get('ptt_bundles',0)*j['params'].get('ptt_bundle_ms',0)
                if j.get('k')=='ev' and j.get('e')=='connected': t0=j['t']
                if j.get('k')!='ts' or t0 is None: continue
                for s in j.get('s',[]):
                    if s.get('peer')!='bob': continue
                    n+=1; prs.append(s['pr']); bws.append(s['bw']); stc[s['st']]+=1
                    if s['rec']['pl']>0: pl_pos+=1
                    tm=j['t']-t0
                    if ptt and any(a<=tm<a+pb+2000 for a in ptt):
                        ptt_pr.append(s['pr']); ptt_txq.append(s['rec']['txq'])
        if not prs: continue
        md=lambda v: st.median(v) if v else float('nan')
        tot=sum(stc.values())
        print(f"{cell} {arm}: samples={n} pacer_p50={md(prs):.0f} B/s btlbw_p50={md(bws):.0f} pending_lost>0={pl_pos/n:.2f} PTT-window pacer_p50={md(ptt_pr):.0f} txq_p50={md(ptt_txq)} txq_max={max(ptt_txq) if ptt_txq else None} states="+', '.join(f"{k}:{v/tot:.2f}" for k,v in stc.most_common()))
