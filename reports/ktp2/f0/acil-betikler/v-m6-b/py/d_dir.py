import csv, sys, statistics, collections, math
rows = list(csv.DictReader(open(sys.argv[1])))
def f(x):
    try: return float(x)
    except: return float('inf')
g = collections.defaultdict(lambda: collections.defaultdict(lambda: ([], [])))
for r in rows:
    c = r['cell']
    if not (c.startswith('b_r12k') or c.startswith('b_r1k')): continue
    g[c][r['arm']][0].append(f(r['lat_p95_s'])); g[c][r['arm']][1].append(f(r['ow_p95_s']))
cnt = collections.Counter(); n=0
for c, arms in sorted(g.items()):
    if 'quinn' not in arms or 'taban' not in arms: continue
    if len(arms['taban'][0]) < 30: continue
    n += 1
    for i, name in ((0,'gonderen'),(1,'alici')):
        t = statistics.median(arms['taban'][i]); q = statistics.median(arms['quinn'][i])
        if q < t: cnt[name+'_quinn_dusuk'] += 1
        elif t < q: cnt[name+'_taban_dusuk'] += 1
print('hucre', n, dict(cnt))
