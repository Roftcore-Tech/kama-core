import json, os, sys, glob, collections
root = sys.argv[1]; K = 8
res = collections.defaultdict(collections.Counter)
for f in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    p = f.split('/'); cell, arm = p[-4], p[-3]
    rtt = 2000.0 if '_d1000_' in cell else 600.0
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
        if ln != 53: continue
        last = (u // K) * K + K - 1
        if last not in S or (u + 3) not in S: continue
        form = S[last][1] - at
        a3 = S[u + 3][1] - at + rtt          # 3. sonraki birim + RTT (paket esigi modeli, kuyruk yok)
        fixed = 600.0 + rtt                  # 3T+RTT, T=200
        R['n'] += 1
        if form > fixed: R['form_gt_fixed'] += 1
        if form > a3: R['form_gt_a3'] += 1
for (c, a), R in sorted(res.items()):
    if not R['n']: continue
    print(f"{c:30s} {a:5s} n={R['n']:5d} form>3T+RTT={R['form_gt_fixed']/R['n']:.3f} form>(t3+RTT)={R['form_gt_a3']/R['n']:.3f}")
