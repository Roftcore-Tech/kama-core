import json, sys, os, glob, bisect, collections
root = sys.argv[1]
K = 8
lenclass = collections.Counter()
unmatched = collections.Counter()
res = collections.defaultdict(lambda: dict(runs=0, ptt=0, waits=[], never=0, sess=collections.Counter()))
def pct(xs, q):
    xs = sorted(xs); 
    if not xs: return None
    i = min(len(xs)-1, int(q*(len(xs)-1)+0.5)); return xs[i]
for al in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    parts = al.split(os.sep); cell, arm = parts[-4], parts[-3]
    S = []; sends = []
    for line in open(al, errors='replace'):
        try: r = json.loads(line)
        except Exception: continue
        k = r.get('k')
        if k == 'tr':
            for rec in r['r']:
                if rec[0] == 'S': S.append((rec[4], rec[1], rec[2], rec[3]))
        elif k == 'send':
            sends.append((r['t'], r['tr'], r['c']))
    sends.sort()
    st = [s[0] for s in sends]
    a = res[(cell, arm)]; a['runs'] += 1
    units = collections.defaultdict(dict)
    cls = {}
    for (t, sess, u, l) in S:
        a['sess'][sess] += 1
        units[sess].setdefault(u, (t, l))
        i = bisect.bisect_right(st, t) - 1
        c = None
        if i >= 0 and sends[i][0] <= t <= sends[i][1] + 0.001:
            c = sends[i][2]
        cls[(sess, u)] = c
        lenclass[(c, l)] += 1
    for sess, us in units.items():
        for u, (t, l) in us.items():
            if cls.get((sess, u)) != 'P': continue
            a['ptt'] += 1
            last = K*(u//K) + K-1
            if last in us: a['waits'].append(us[last][0] - t)
            else: a['never'] += 1
print("len by class (top):")
for (c,l),n in sorted(lenclass.items(), key=lambda x:-x[1])[:25]: print(c, l, n)
print("cell arm runs ptt never p50 p90 f>400 f>800 max sessions")
for k, a in sorted(res.items()):
    w = a['waits']
    if not a['ptt']: continue
    f4 = sum(1 for x in w if x > 400)/a['ptt']; f8 = sum(1 for x in w if x > 800)/a['ptt']
    print(*k, a['runs'], a['ptt'], a['never'], round(pct(w,.5)), round(pct(w,.9)), round(f4,3), round(f8,3), round(max(w)), dict(a['sess']) if len(a['sess'])>1 else '')
