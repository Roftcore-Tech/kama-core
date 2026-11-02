import os, sys, glob, re, collections
root = sys.argv[1]
tot = 0; pos = 0; cellpos = collections.Counter(); per = collections.defaultdict(list); hdr = []
for rep in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*'))):
    p = rep.split('/'); cell, arm = p[-3], p[-2]
    if arm not in ('taban', 'aday'): continue
    try: ce = open(os.path.join(rep, 'c.err')).read()
    except OSError: continue
    m = re.findall(r'fec_repairs_tx=(\d+)', ce)
    if not m: continue
    v = int(m[0]); tot += 1
    if v > 0: pos += 1; cellpos[cell] += 1; per[cell].append(v)
    if cell == 'b_r12k_d1000_l30_p1000': hdr.append((arm, p[-1], v))
print('runs', tot, 'parity>0', pos)
for c, n in sorted(cellpos.items()): print(c, n, 'vals', sorted(per[c])[:10], 'max', max(per[c]))
print('headline', len(hdr), 'sum', sum(x[2] for x in hdr), [x for x in hdr if x[2] > 0])
