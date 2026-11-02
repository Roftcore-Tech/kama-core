import json, os, sys, glob, statistics as st
root = sys.argv[1]
cells = ['c_r12k_d300_l20_b60','c_r12k_d300_l20_b120','c_r12k_d300_l20_b200','c_r12k_d1000_l20_b60','c_r12k_d1000_l20_b120','c_r12k_d1000_l20_b200','c_r12k_d1000_l10_b120','c_r12k_d1000_l30_b120']
for cell in cells:
    owd = 300 if '_d300_' in cell else 1000
    for arm in ('arq_taban', 'rs2_taban', 'rs4_taban'):
        f5 = []; p10 = []; nl = []
        for rep in sorted(glob.glob(os.path.join(root, cell, arm, 'rep*'))):
            try:
                L = open(os.path.join(rep, 'out.jsonl')).read().splitlines()
                meta = json.loads(L[0])['meta']
                ev = [json.loads(l)['event'] for l in L[1:] if l.startswith('{"event"')]
                rx = {}
                for l in open(os.path.join(rep, 'rx.jsonl')):
                    d = json.loads(l)
                    if 'u' in d and d['u'] not in rx: rx[d['u']] = d['t']
            except Exception:
                continue
            t0 = meta['t0_unix_ns']; off = meta['events_epoch_offset_ns']
            lat = []
            for e in ev:
                u = e['msg_id'] + 16
                if u in rx:
                    s_rel = e['sent_ns'] - off
                    lat.append(((rx[u] - (t0 + s_rel)) / 1e6, s_rel / 1e6))
            late = sorted(l for l, t in lat if l > owd + 500)
            if late: p10.append(late[len(late)//10])
            x = [l for l, t in lat if l > owd + 500 and t < 5000]
            if x: f5.append(st.median(x))
            nl.append(len(late)/len(lat) if lat else 0)
        m = lambda a: round(st.median(a)) if a else None
        print(f"{cell:24s} {arm:10s} n={len(nl)} late_frac={st.median(nl):.2f} first5_late_med={m(f5)} late_p10_med={m(p10)}")
