import json, csv, os, sys, glob, statistics as st
root=sys.argv[1]
for cell in sys.argv[2:]:
    for arm in ['rs2_taban','rs4_taban','uyar_taban','arq_taban']:
        out=[]
        for rep in sorted(glob.glob(os.path.join(root,cell,arm,'rep*'))):
            f=os.path.join(rep,'out.jsonl')
            if not os.path.exists(f): continue
            L=open(f).read().splitlines()
            if not L: continue
            m=json.loads(L[0])['meta']; a=m.get('activation') or {}
            ival=m['submit_interval_ns']/1e9; n=m['msg_count']
            fec=m['tx']['fec']; par=a.get('parity_emitted') or 0
            pw = fec/par if par else None
            frame = m['tx']['data']/max(1,n)  # approx per-msg data wire (bench)
            pend=[];  
            tsf=os.path.join(rep,'ts.csv')
            if os.path.exists(tsf):
                for row in csv.DictReader(open(tsf)):
                    pend.append(int(row['pending_parity']))
            mp = st.mean(pend) if pend else None
            out.append((a.get('btlbw_bps'), par, pw, frame, mp, ival, a.get('parity_token_denied')))
        if not out: continue
        ival=out[0][5]
        pws=[o[2] for o in out if o[2]]
        pw=st.median(pws) if pws else None
        need = pw/ival if pw else None
        lo=[o for o in out if need and o[0] is not None and o[0]<need]
        hi=[o for o in out if need and o[0] is not None and o[0]>=need]
        f=lambda xs: round(st.median(xs),2) if xs else None
        print(cell, arm, 'N',len(out),'T',ival,'frame_wire~',round(st.median([o[3] for o in out]),1),'parity_wire~',round(pw,1) if pw else None,
              'need_rate(pw/T)',round(need) if need else None,'btlbw_med',f([o[0] for o in out]),
              '| btlbw<need: n',len(lo),'mean_pending_med',f([o[4] for o in lo]),'| btlbw>=need: n',len(hi),'mean_pending_med',f([o[4] for o in hi]),
              'parity_sum',sum(o[1] for o in out),'denied_med',f([o[6] for o in out if o[6] is not None]))
