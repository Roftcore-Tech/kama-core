import re, sys, os, glob
root = sys.argv[1]
agg = {}
for d in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*'))):
    parts = d.split(os.sep)
    cell, arm = parts[-3], parts[-2]
    armk = arm.rsplit('_', 1)[0]
    try:
        c = open(os.path.join(d, 'c.err'), errors='replace').read()
        s = open(os.path.join(d, 's.err'), errors='replace').read()
    except Exception:
        continue
    def g(pat, txt):
        m = re.search(pat, txt)
        return int(m.group(1)) if m else None
    rep_tx = g(r'fec_repairs_tx=(\d+)', c)
    rec = g(r'fec_recovered_units: (\d+)', s)
    rrx = g(r'fec_repairs_rx: (\d+)', s)
    od = g(r'open_failures: (\d+)', s)
    ddrop = g(r'delivery_dropped: (\d+)', s)
    a = agg.setdefault(armk, dict(n=0, tx_pos=0, rec_pos=0, rec_sum=0, rep_tx_sum=0, miss=0, ddrop=0))
    a['n'] += 1
    if rep_tx is None or rec is None:
        a['miss'] += 1; continue
    a['tx_pos'] += rep_tx > 0; a['rec_pos'] += rec > 0; a['rec_sum'] += rec; a['rep_tx_sum'] += rep_tx
    a['ddrop'] += (ddrop or 0) > 0
for k, a in sorted(agg.items()):
    print(k, a)
