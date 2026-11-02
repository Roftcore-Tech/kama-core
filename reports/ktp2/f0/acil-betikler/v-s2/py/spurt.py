import json, os, sys, glob, collections, statistics as st
root = sys.argv[1]
agg = collections.defaultdict(collections.Counter)
prs = collections.defaultdict(list)
prpar = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    p = f.split('/'); cell, arm = p[-4], p[-3]
    sends = collections.defaultdict(list); rows = []
    for l in open(f):
        try: d = json.loads(l)
        except Exception: continue
        k = d.get('k')
        if k == 'send' and d['c'] in ('O', 'P', 'C'): sends[d['sp']].append(d['t'])
        elif k == 'ts' and d.get('s'):
            s0 = d['s'][0]; rows.append((d['t'], s0['par'], s0['pr'], (s0.get('rec') or {}).get('txq')))
    wins = [(min(v), max(v)) for v in sends.values()]
    c = agg[(cell, arm)]
    for i in range(1, len(rows)):
        t0, p0, _, _ = rows[i-1]; t1, p1, pr1, q1 = rows[i]
        dp = p1 - p0
        inside = any(t0 <= b and t1 >= a for a, b in wins)   # overlap
        strict = any(a <= t0 and t1 <= b for a, b in wins)   # fully inside
        if any(a <= t1 <= b for a, b in wins): prs[(cell, arm)].append(pr1)
        if dp > 0:
            c['par'] += dp
            if inside: c['overlap'] += dp
            if strict: c['strict'] += dp
            prpar[(cell, arm)].append(pr1)
for k in sorted(agg):
    v = agg[k]; a = prs[k]
    thr = 4742
    frac = sum(1 for x in a if x >= thr) / len(a) if a else float('nan')
    def q(x, pp):
        x = sorted(x); return x[min(len(x)-1, int(round(pp*(len(x)-1))))] if x else None
    print(f"{k[0]:30s} {k[1]:5s} par={v['par']:5d} overlap={v['overlap']:4d} strict={v['strict']:4d} pr_spurt_med={q(a,.5)} p90={q(a,.9)} frac>=4742={frac:.3f} pr_at_par_med={q(prpar[k],.5)}")
