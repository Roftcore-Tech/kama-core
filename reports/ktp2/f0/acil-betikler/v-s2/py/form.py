import json, os, sys, glob, collections
root = sys.argv[1]; K = 8
def q(a, p):
    a = sorted(a); return a[min(len(a)-1, int(round(p*(len(a)-1))))] if a else None
res = collections.defaultdict(lambda: dict(f=[], open=0, lens=collections.Counter(), pos=collections.Counter()))
for f in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    p = f.split('/'); cell, arm = p[-4], p[-3]
    S = {}
    for l in open(f):
        if '"k":"tr"' not in l: continue
        try: d = json.loads(l)
        except Exception: continue
        for r in d['r']:
            if r[0] == 'S' and r[1] == 'SessionHandle(0)' and r[2] not in S:
                S[r[2]] = (r[3], r[4])
    R = res[(cell, arm)]
    for u, (ln, at) in S.items():
        R['lens'][ln] += 1
        if ln != 53: continue
        last = (u // K) * K + K - 1
        if last in S: R['f'].append(S[last][1] - at)
        else: R['open'] += 1
for (c, a), R in sorted(res.items()):
    x = R['f']
    if not x: continue
    thr = 2600 if '_d1000_' in c else 1200
    fr = sum(1 for v in x if v > thr) / len(x)
    print(f"{c:30s} {a:5s} n={len(x):5d} open={R['open']:3d} p50={q(x,.5):.0f} p90={q(x,.9):.0f} p99={q(x,.99):.0f} mean={sum(x)/len(x):.0f} frac>{thr}={fr:.3f}")
