import json, sys, os, glob
root = sys.argv[1]
rows = []
for bob in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'bob.jsonl'))):
    parts = bob.split(os.sep)
    cell, arm, rep = parts[-4], parts[-3], parts[-2]
    dcd_max = 0; nrd_max = 0; ro_hmax = 0; inb_max=0; n_ts=0
    rx_counts = {}
    ts_par_max = 0
    for line in open(bob, errors='replace'):
        try:
            r = json.loads(line)
        except Exception:
            continue
        k = r.get('k')
        if k == 'ts':
            n_ts += 1
            dcd_max = max(dcd_max, r.get('dcd', 0) or 0)
            nrd_max = max(nrd_max, r.get('nrd', 0) or 0)
            for s in r.get('s', []):
                reo = s.get('reo')
                if reo: ro_hmax = max(ro_hmax, reo.get('h', 0) or 0)
        elif k == 'rx':
            c = r.get('c'); rx_counts[c] = rx_counts.get(c, 0) + 1
    # alice parity
    al = bob.replace('bob.jsonl', 'alice.jsonl')
    par_max = 0; rtx_max = 0
    if os.path.exists(al):
        for line in open(al, errors='replace'):
            if '"k":"ts"' not in line: continue
            try: r = json.loads(line)
            except Exception: continue
            for s in r.get('s', []):
                par_max = max(par_max, s.get('par', 0) or 0)
                rtx_max = max(rtx_max, s.get('rtx', 0) or 0)
    rows.append((cell, arm, rep, n_ts, dcd_max, nrd_max, ro_hmax, par_max, rtx_max))
agg = {}
for cell, arm, rep, n_ts, dcd, nrd, ro, par, rtx in rows:
    a = agg.setdefault((cell, arm), [0, 0, 0, 0, 0, 0])
    a[0] += 1; a[1] += (dcd > 0); a[2] = max(a[2], dcd); a[3] = max(a[3], ro); a[4] += (par > 0); a[5] = max(a[5], rtx)
print("cell arm n reps_dcd>0 dcd_max reorder_held_max reps_parity>0 rtx_max")
for (cell, arm), a in sorted(agg.items()):
    print(cell, arm, *a)
print("total runs", len(rows), "runs with dcd>0", sum(1 for r in rows if r[4] > 0), "runs parity>0", sum(1 for r in rows if r[7] > 0))
