import json, sys, csv, os, collections
ROOT=sys.argv[1]
rows = [json.loads(l) for l in open(sys.argv[2])]
def little(path):
    integ = 0.0; prev_t=None; prev_p=0; tokens=[]
    with open(path) as f:
        for row in csv.DictReader(f):
            try: t=int(row['t_ms']); p=int(row['pending_parity']); tk=int(row['pacer_tokens'])
            except: continue
            if prev_t is not None: integ += prev_p*(t-prev_t)
            prev_t=t; prev_p=p
            if p>0: tokens.append(tk)
    return integ, tokens
out = collections.defaultdict(list)
for r in rows:
    if (r.get('fec_repairs_tx') or 0) <= 0: continue
    ts = os.path.join(ROOT, r['lane'], 'out', r['cell'], r['arm'], r['rep'], 'ts.csv')
    if not os.path.isfile(ts): continue
    integ, tk = little(ts)
    key = ('adaptive' if 'adaptive' in (r['fec'] or '') else r['fec'], r['cell'].split('_')[1] if r['lane']=='b' else r['cell'].split('_')[1])
    out[key].append((integ/ r['fec_repairs_tx'], r['fec_repairs_tx'], r.get('parity_denied')))
for k in sorted(out):
    v = out[k]; w = sorted(x[0] for x in v)
    print(k, 'runs', len(v), 'mean_wait_ms p50', round(w[len(w)//2]), 'p90', round(w[int(len(w)*0.9)]), 'sum_repairs', sum(x[1] for x in v), 'sum_denied', sum((x[2] or 0) for x in v))
