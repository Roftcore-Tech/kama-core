import json, os, sys, glob, re, statistics as st
root = sys.argv[1]
cells = sys.argv[2].split(',')
arms = sys.argv[3].split(',')
for cell in cells:
    owd = 300 if '_d300_' in cell else 1000
    for arm in arms:
        late_med = []; late_p10 = []; late_frac = []; first5_late = []
        for rep in sorted(glob.glob(os.path.join(root, cell, arm, 'rep*'))):
            try:
                lines = open(os.path.join(rep, 'out.jsonl')).read().splitlines()
                meta = json.loads(lines[0])['meta']
                ev = [json.loads(l)['event'] for l in lines[1:] if l.startswith('{"event"')]
                rx = {}
                for l in open(os.path.join(rep, 'rx.jsonl')):
                    d = json.loads(l)
                    if 'u' in d: rx.setdefault(d['u'], d['t'])
            except Exception:
                continue
            t0u = meta['t0_unix_ns']; off = meta['events_epoch_offset_ns']; W = 16
            lat = []
            for e in ev:
                u = e['msg_id'] + W
                if u in rx:
                    s_unix = t0u + (e['sent_ns'] - off)
                    lat.append(((rx[u] - s_unix) / 1e6, (e['sent_ns'] - off) / 1e6))
            if not lat: continue
            thr = owd + 500
            late = [l for l, t in lat if l > thr]
            late_frac.append(len(late) / len(lat))
            if late:
                late_med.append(st.median(late)); late_p10.append(sorted(late)[len(late)//10])
            f5 = [l for l, t in lat if l > thr and t < 5000]
            if f5: first5_late.append(st.median(f5))
        def m(a): return round(st.median(a)) if a else None
        print(f"{cell:24s} {arm:10s} n={len(late_frac)} late_frac_med={st.median(late_frac) if late_frac else 0:.2f} late_lat_ms_med={m(late_med)} late_p10_med={m(late_p10)} first5s_late_med={m(first5_late)}")
