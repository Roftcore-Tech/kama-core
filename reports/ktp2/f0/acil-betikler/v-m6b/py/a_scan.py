import json, sys, os, glob
root = sys.argv[1]
K = 8
def pct(xs, q):
    xs = sorted(xs)
    if not xs: return None
    # nearest-rank
    import math
    i = max(0, math.ceil(q*len(xs)) - 1)
    return xs[i]
agg = {}
nruns = 0
for d in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*'))):
    if not os.path.isdir(d): continue
    parts = d.split(os.sep); cell, arm, rep = parts[-3], parts[-2], parts[-1]
    al = os.path.join(d, 'alice.jsonl'); bo = os.path.join(d, 'bob.jsonl')
    if not (os.path.exists(al) and os.path.exists(bo)): continue
    nruns += 1
    a = agg.setdefault((cell, arm), dict(runs=0, dcd_runs=0, dcd_max=0, h_max=0, par_runs=0, ptt=0, never=0, waits=[], contig_bad=0))
    a['runs'] += 1
    S = {}
    par = 0
    for line in open(al, errors='replace'):
        if line.startswith('{"k":"tr"'):
            r = json.loads(line)
            for rec in r['r']:
                if rec[0] == 'S' and rec[1] == 'SessionHandle(0)':
                    S.setdefault(rec[2], (rec[4], rec[3]))
        elif line.startswith('{"k":"ts"'):
            r = json.loads(line)
            for s in r.get('s', []):
                par = max(par, s.get('par') or 0)
    if par > 0: a['par_runs'] += 1
    if S and sorted(S) != list(range(len(S))): a['contig_bad'] += 1
    for u, (t, l) in S.items():
        if l != 53: continue
        a['ptt'] += 1
        last = K*(u//K) + K - 1
        if last in S: a['waits'].append(S[last][0] - t)
        else: a['never'] += 1
    dcd = 0; h = 0
    for line in open(bo, errors='replace'):
        if line.startswith('{"k":"ts"'):
            r = json.loads(line)
            dcd = max(dcd, r.get('dcd') or 0)
            for s in r.get('s', []):
                reo = s.get('reo')
                if reo: h = max(h, reo.get('h') or 0)
            for ro in r.get('ro', []):
                h = max(h, ro.get('h') or 0)
    if dcd > 0: a['dcd_runs'] += 1
    a['dcd_max'] = max(a['dcd_max'], dcd); a['h_max'] = max(a['h_max'], h)
print('runs', nruns)
print('cell arm runs dcd_runs dcd_max held_max par_runs | ptt never p50 p90 >400 >800 max contig_bad')
for k, a in sorted(agg.items()):
    w = a['waits']; n = a['ptt']
    if n:
        f4 = sum(x > 400 for x in w)/n; f8 = sum(x > 800 for x in w)/n
        ws = f"{n} {a['never']} {pct(w,.5):.0f} {pct(w,.9):.0f} {f4:.3f} {f8:.3f} {max(w):.0f}"
    else: ws = '-'
    print(*k, a['runs'], a['dcd_runs'], a['dcd_max'], a['h_max'], a['par_runs'], '|', ws, a['contig_bad'])
