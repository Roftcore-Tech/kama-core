import json, os, sys, glob
root = sys.argv[1]
res = {}
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    for arm in sorted(os.listdir(cd)):
        ad = os.path.join(cd, arm)
        if not os.path.isdir(ad): continue
        for rep in sorted(glob.glob(os.path.join(ad, 'rep*'))):
            f = os.path.join(rep, 'alice.jsonl')
            if not os.path.exists(f): continue
            last = None; bwmax = 0; parmax = 0
            with open(f) as fh:
                for l in fh:
                    if '"k":"ts"' not in l: continue
                    try: d = json.loads(l)
                    except Exception: continue
                    for s in d.get('s', []):
                        bwmax = max(bwmax, s.get('bw', 0) or 0)
                        parmax = max(parmax, s.get('par', 0) or 0)
            key = (cell, arm)
            r = res.setdefault(key, [0, 0, 0, []])
            r[0] += 1
            if parmax > 0: r[1] += 1
            r[2] += parmax
            r[3].append(bwmax)
for (c, a), (n, npos, tot, bws) in sorted(res.items()):
    bws.sort()
    print(f"{c:32s} {a:6s} runs={n:3d} runs_par>0={npos:3d} par_sum={tot:5d} bw_max_med={bws[len(bws)//2]} bw_max_max={bws[-1]}")
