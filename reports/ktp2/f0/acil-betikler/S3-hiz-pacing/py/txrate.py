import sys, gzip, os, glob, statistics as st
# usage: txrate.py <celldir> <arm> <binsec> [reps...]
celldir, arm, binsec = sys.argv[1], sys.argv[2], float(sys.argv[3])
reps = sys.argv[4:] or None
paths = sorted(glob.glob(os.path.join(celldir, arm, 'rep*')), key=lambda p:int(p.rsplit('rep',1)[1]))
if reps: paths=[p for p in paths if p.rsplit('rep',1)[1] in reps]
allbins = {}
for p in paths:
    f = os.path.join(p,'capb_cli.csv.gz')
    if not os.path.exists(f): continue
    rows=[]
    for line in gzip.open(f,'rt'):
        parts=line.strip().split(',')
        if len(parts)<4: continue
        t=int(parts[0]); d=parts[1]; ln=int(parts[3])
        rows.append((t,d,ln))
    if not rows: continue
    t0=rows[0][0]
    bins={}
    big={}
    for t,d,ln in rows:
        if d!='o': continue
        b=int((t-t0)/1e9//binsec)
        bins[b]=bins.get(b,0)+ln
        if ln>500: big[b]=big.get(b,0)+1
    nb=max(bins)+1
    line=' '.join(f"{bins.get(i,0)/binsec:5.0f}" for i in range(nb))
    print(f"{os.path.basename(p):6s} tx B/s per {binsec:.0f}s: {line}")
    lineb=' '.join(f"{big.get(i,0):5d}" for i in range(nb))
    print(f"{'':6s} data-size pkts       : {lineb}")
