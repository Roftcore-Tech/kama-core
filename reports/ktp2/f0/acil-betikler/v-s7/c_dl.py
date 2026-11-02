import json, os, glob, sys, math, statistics as st, re
cell=sys.argv[1]; T=int(sys.argv[2]); mu=float(sys.argv[3]); budget=0.8
for arm in ['arq_taban','uyar_taban','rs2_taban','rs4_taban','arq_aday','rs2_aday']:
    res={'uo':[], 'o':[], 'uo_T':[], 'o_T':[], 'par':[], 'ppend':[], 'tden':[]}
    for rep in sorted(glob.glob(os.path.join(cell,arm,'rep*'))):
        try:
            lines=open(os.path.join(rep,'out.jsonl')).read().splitlines()
        except FileNotFoundError: continue
        meta=None; ev=[]
        for l in lines:
            if not l.strip(): continue
            o=json.loads(l)
            if 'meta' in o: meta=o['meta']
            elif 'event' in o: ev.append(o['event'])
        cmd=open(os.path.join(rep,'cmd.txt')).read()
        m=re.search(r'--warmup-msgs (\d+)',cmd); W=int(m.group(1)) if m else 0
        t0=int(meta['t0_unix_ns']); off=int(meta['events_epoch_offset_ns'])
        sent={e['msg_id']:e['sent_ns'] for e in ev}
        rx={}
        for l in open(os.path.join(rep,'rx.jsonl')):
            o=json.loads(l)
            if 'u' in o:
                if o['u'] not in rx or o['t']<rx[o['u']]: rx[o['u']]=o['t']
        n=len(sent)
        lim=(mu+budget)*1e9
        lat=[]; 
        for mid in range(n):
            off_u=t0+sent[mid]-off
            tr=rx.get(W+mid)
            lat.append((tr-off_u) if tr is not None else math.inf)
        # ordered
        rm=-math.inf; olat=[]
        for mid in range(n):
            tr=rx.get(W+mid, math.inf)
            rm=max(rm,tr)
            olat.append(rm-(t0+sent[mid]-off))
        res['uo'].append(sum(1 for v in lat if v<=lim)/n)
        res['o'].append(sum(1 for v in olat if v<=lim)/n)
        res['uo_T'].append(sum(1 for v in lat if v+T*1e6<=lim)/n)
        res['o_T'].append(sum(1 for v in olat if v+T*1e6<=lim)/n)
        act=meta.get('activation',{})
        res['par'].append(act.get('parity_emitted',0)); res['tden'].append(act.get('parity_token_denied',0))
    if not res['uo']: continue
    f=lambda k: '%.3f'%st.median(res[k])
    print(arm,'N',len(res['uo']),'unordered',f('uo'),'ordered',f('o'),'| with +T: unordered',f('uo_T'),'ordered',f('o_T'),'| parity med',st.median(res['par']),'tokden med',st.median(res['tden']))
