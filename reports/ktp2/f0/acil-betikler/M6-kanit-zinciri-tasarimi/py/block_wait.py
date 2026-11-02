import json, sys, os, glob, statistics
root = sys.argv[1]
K = 8
def pct(xs, q):
    if not xs: return None
    xs = sorted(xs); i = min(len(xs)-1, int(q*(len(xs)-1)+0.5)); return xs[i]
out = {}
for al in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    parts = al.split(os.sep); cell, arm = parts[-4], parts[-3]
    S = {}; sends = []
    for line in open(al, errors='replace'):
        try: r = json.loads(line)
        except Exception: continue
        if r.get('k') == 'tr':
            for rec in r['r']:
                if rec[0] == 'S':
                    S.setdefault((rec[1], rec[2]), (rec[4], rec[3]))
    # group per session
    sess = {}
    for (s, u), (t, l) in S.items():
        sess.setdefault(s, {})[u] = (t, l)
    a = out.setdefault((cell, arm), dict(runs=0, ptt=0, waits=[], never=0))
    a['runs'] += 1
    for s, units in sess.items():
        for u, (t, l) in units.items():
            if l != 53: continue  # PTT bundle units
            a['ptt'] += 1
            last = K*(u//K) + K-1
            if last in units:
                a['waits'].append(units[last][0] - t)
            else:
                a['never'] += 1
print("cell arm runs ptt_units never_closed p50_ms p90_ms frac>400 frac>800 max_ms")
for k, a in sorted(out.items()):
    w = a['waits']
    if not a['ptt']: continue
    f4 = sum(1 for x in w if x > 400)/a['ptt']; f8 = sum(1 for x in w if x > 800)/a['ptt']
    print(*k, a['runs'], a['ptt'], a['never'], round(pct(w,.5) or 0), round(pct(w,.9) or 0), round(f4,3), round(f8,3), round(max(w) if w else 0))
