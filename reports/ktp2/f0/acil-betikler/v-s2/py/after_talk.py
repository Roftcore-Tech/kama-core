import json, os, sys, glob, collections, statistics as st
root = sys.argv[1]
out = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(root, '*dosyasiz*', '*', 'rep*', 'alice.jsonl'))):
    p = f.split('/'); cell, arm = p[-4], p[-3]
    sends = collections.defaultdict(list); rows = []
    for l in open(f):
        try: d = json.loads(l)
        except Exception: continue
        if d.get('k') == 'send' and d['c'] in ('O','P','C'): sends[d['sp']].append(d['t'])
        elif d.get('k') == 'ts' and d.get('s'): rows.append((d['t'], d['s'][0]['par']))
    ends = sorted(max(v) for v in sends.values())
    for i in range(1, len(rows)):
        dp = rows[i][1] - rows[i-1][1]
        if dp <= 0: continue
        t = rows[i][0]
        prev = [e for e in ends if e <= t]
        if prev: out[(cell, arm)].append((t - prev[-1]) / 1000)
for k, v in sorted(out.items()):
    v = sorted(v)
    print(k, 'n=', len(v), 'since_talk_end_s med=%.2f min=%.2f max=%.2f' % (st.median(v), v[0], v[-1]))
