import json, os, sys, glob
root = sys.argv[1]
agg = {}
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    for arm in sorted(os.listdir(cd)):
        for rep in sorted(glob.glob(os.path.join(cd, arm, 'rep*'))):
            f = os.path.join(rep, 'alice.jsonl')
            if not os.path.exists(f): continue
            pr = []
            for l in open(f):
                if '"k":"ts"' not in l: continue
                try: d = json.loads(l)
                except Exception: continue
                s = d.get('s') or []
                if not s: continue
                rec = s[0].get('rec') or {}
                pr.append((d['t'], s[0].get('par', 0), rec.get('txq')))
            a = agg.setdefault((cell, arm), [0, 0, 0, 0, 0])  # intervals, both>0, par_in_both>0, par_total, intervals_par>0
            for i in range(1, len(pr)):
                a[0] += 1
                both = (pr[i-1][2] or 0) > 0 and (pr[i][2] or 0) > 0
                dp = pr[i][1] - pr[i-1][1]
                if both: a[1] += 1
                if dp > 0:
                    a[3] += dp; a[4] += 1
                    if both: a[2] += dp
tot = [0]*5
for (c, a), v in sorted(agg.items()):
    for j in range(5): tot[j] += v[j]
    if v[3] == 0: continue
    print(f"{c:30s} {a:5s} intervals={v[0]:6d} txq>0_both={v[1]:5d} ({v[1]/v[0]:.3f}) par={v[3]:5d} in_par_intervals={v[4]:4d} par_in_txq>0_both={v[2]}  expected_if_indep={v[3]*v[1]/v[0]:.1f}")
print('TOTAL', tot, f"frac_both={tot[1]/tot[0]:.3f} expected_par_if_indep={tot[3]*tot[1]/tot[0]:.1f}")
