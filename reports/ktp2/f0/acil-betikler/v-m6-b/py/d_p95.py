import csv, sys, statistics, collections
rows = list(csv.DictReader(open(sys.argv[1])))
cell = sys.argv[2]
g = collections.defaultdict(lambda: ([], []))
for r in rows:
    if r['cell'] != cell: continue
    def f(x):
        try: return float(x)
        except: return float('inf')
    g[r['arm']][0].append(f(r['lat_p95_s'])); g[r['arm']][1].append(f(r['ow_p95_s']))
for arm,(a,b) in sorted(g.items()):
    print(arm, len(a), 'median lat_p95 (gonderen)', round(statistics.median(a),2), 'median ow_p95 (alici)', round(statistics.median(b),2))
