import json, os, sys, glob, collections
root = sys.argv[1]
tot = collections.Counter()
percell = {}
odd = []
for f in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    parts = f.split('/')
    cell, arm = parts[-4], parts[-3]
    rows = []
    for l in open(f):
        if '"k":"ts"' not in l: continue
        try: d = json.loads(l)
        except Exception: continue
        s = d.get('s') or []
        if len(s) != 1:
            tot['multi_sess_or_empty'] += 1
            if not s: continue
        r = s[0].get('rec')
        if r is None: tot['rec_null']+=1; continue
        rows.append((d['t'], s[0]['par'], r['txq'], s[0].get('pr'), s[0].get('st')))
    c = percell.setdefault((cell, arm), collections.Counter())
    for i in range(1, len(rows)):
        a, b = rows[i-1], rows[i]
        dp = b[1] - a[1]
        if dp < 0: tot['neg_dp'] += 1; continue
        full = a[2] > 0 and b[2] > 0
        c['iv'] += 1; tot['iv'] += 1
        if full: c['full'] += 1; tot['full'] += 1
        if dp:
            c['par'] += dp; tot['par'] += dp
            if full:
                c['par_full'] += dp; tot['par_full'] += dp
                odd.append((f, a, b))
            # txq min at both ends
            if a[2] > 0 or b[2] > 0:
                tot['par_oneend_full'] += dp
for k, c in sorted(percell.items()):
    if c['par']:
        print(k, dict(c), 'exp=%.1f' % (c['par'] * c['full'] / c['iv']))
print(dict(tot), 'frac_full=%.4f' % (tot['full'] / tot['iv']), 'exp=%.1f' % (tot['par'] * tot['full'] / tot['iv']))
for o in odd: print('ODD', o)
