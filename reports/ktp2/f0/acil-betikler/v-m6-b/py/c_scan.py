import re, sys, os, glob, collections
root = sys.argv[1]
agg = collections.defaultdict(lambda: dict(runs=0, rep_runs=0, rep_sum=0, rec_runs=0, rec_sum=0, nokp=0, wz=0, clamp_runs=0, btl=[], blocks=0))
for rcf in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*', 'rc'))):
    d = os.path.dirname(rcf); arm = d.split(os.sep)[-2].split('_')[0]; side = d.split(os.sep)[-2].split('_')[1]
    a = agg[(arm, side)]; a['runs'] += 1
    ce = open(os.path.join(d,'c.err'), errors='replace').read()
    se = open(os.path.join(d,'s.err'), errors='replace').read()
    m = re.search(r'KP-CLIENT .*?fec_blocks=(\d+) fec_repairs_tx=(\d+)', ce)
    s = re.search(r'fec_recovered_units: (\d+)', se)
    if not m or not s: a['nokp'] += 1; continue
    rep = int(m.group(2)); rec = int(s.group(1)); a['blocks'] += int(m.group(1))
    if rep>0: a['rep_runs'] += 1
    a['rep_sum'] += rep
    if rec>0: a['rec_runs'] += 1
    a['rec_sum'] += rec
    w = re.search(r'would_zero=(\d+)', ce)
    if w and int(w.group(1))>0: a['wz'] += 1
    b = re.search(r'est=\[btlbw=(\d+)B/s', ce)
    if b: a['btl'].append(int(b.group(1)))
for k,a in sorted(agg.items()):
    bt = sorted(a['btl'])
    q = (lambda p: bt[min(len(bt)-1,int(p*(len(bt)-1)))]) if bt else (lambda p: None)
    print(k, {x:a[x] for x in ('runs','nokp','rep_runs','rep_sum','rec_runs','rec_sum','wz','blocks')}, 'btlbw p10/50/90', q(.1), q(.5), q(.9))
tot = collections.defaultdict(lambda: collections.Counter())
for (arm, side), a in agg.items():
    for x in ('runs','nokp','rep_runs','rep_sum','rec_runs','rec_sum'): tot[arm][x] += a[x]
for arm, c in tot.items(): print(arm, dict(c))
