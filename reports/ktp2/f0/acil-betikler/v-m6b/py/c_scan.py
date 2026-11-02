import re, sys, os, glob
root = sys.argv[1]
agg = {}
def last(pat, txt):
    m = re.findall(pat, txt)
    return int(m[-1]) if m else None
for d in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*'))):
    parts = d.split(os.sep); cell, arm = parts[-3], parts[-2]
    kind = arm.rsplit('_', 1)[0]
    try:
        c = open(os.path.join(d, 'c.err'), errors='replace').read()
        s = open(os.path.join(d, 's.err'), errors='replace').read()
    except FileNotFoundError:
        continue
    tx = last(r'KP-CLIENT .*?fec_repairs_tx=(\d+)', c)
    rec = last(r'fec_recovered_units: (\d+)', s)
    rrx = last(r'fec_repairs_rx: (\d+)', s)
    a = agg.setdefault(kind, dict(n=0, miss=0, tx_pos=0, tx_sum=0, rec_pos=0, rec_sum=0, rrx_pos=0))
    a['n'] += 1
    if tx is None or rec is None: a['miss'] += 1; continue
    a['tx_pos'] += tx > 0; a['tx_sum'] += tx; a['rec_pos'] += rec > 0; a['rec_sum'] += rec; a['rrx_pos'] += (rrx or 0) > 0
for k, a in sorted(agg.items()): print(k, a)
