import json, sys, os, glob, collections
root = sys.argv[1]
agg = collections.defaultdict(lambda: [0,0,0,0])  # runs, par>0, dcd>0, maxdcd
mx_h = collections.defaultdict(int)
dcd_runs = []
for al in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'alice.jsonl'))):
    d = os.path.dirname(al); parts = d.split(os.sep); cell, arm = parts[-3], parts[-2]
    par = 0
    for line in open(al, errors='replace'):
        if '"k":"ts"' not in line: continue
        try: r = json.loads(line)
        except Exception: continue
        for s in r.get('s', []):
            if s.get('peer') == 'bob': par = max(par, s.get('par') or 0)
    dcd = 0; h = 0
    bob = os.path.join(d, 'bob.jsonl')
    for line in open(bob, errors='replace'):
        if '"k":"ts"' not in line: continue
        try: r = json.loads(line)
        except Exception: continue
        dcd = max(dcd, r.get('dcd') or 0)
        for s in r.get('s', []):
            if s.get('reo'): h = max(h, s['reo'].get('h') or 0)
    a = agg[(cell, arm)]; a[0] += 1; a[1] += par > 0; a[2] += dcd > 0; a[3] = max(a[3], dcd)
    mx_h[(cell,arm)] = max(mx_h[(cell,arm)], h)
    if dcd>0: dcd_runs.append((cell, arm, parts[-1], dcd))
tot=0
for k, a in sorted(agg.items()):
    print(*k, 'runs', a[0], 'par>0', a[1], 'dcd>0', a[2], 'maxdcd', a[3], 'max_reo_h', mx_h[k]); tot += a[0]
print('total runs', tot, 'dcd runs', len(dcd_runs))
