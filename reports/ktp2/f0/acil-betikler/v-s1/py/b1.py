import csv, gzip, sys, re, collections, statistics
f = sys.argv[1]
rows = list(csv.DictReader(gzip.open(f,'rt')))
print(len(rows))
cells = sorted(set(r['cell'] for r in rows)); print(len(cells))
arms = collections.Counter((r['arm']) for r in rows); print(arms)
cmp_cells = [c for c in cells if re.match(r'b_r(12k|1k)_d(300|1000)_l(0|10|20|30)_p(65|1000)$', c)]
print(len(cmp_cells))
n=0; nf=0; tot=0; par=0
for r in rows:
    if r['cell'] in cmp_cells and r['arm'] in ('taban','aday'):
        n+=1
        fb = float(r['tx_fec_B'] or 0)
        p = float(r['parity'] or 0)
        if fb>0: nf+=1
        tot += p
        if p>0: par+=1
print('runs',n,'fecB>0',nf,'parity>0',par,'sum parity',tot)
# btlbw per cell
for c in cmp_cells:
    bw = [float(r['est_btlbw']) for r in rows if r['cell']==c and r['arm'] in ('taban','aday') and r['est_btlbw']]
    p = sum(1 for r in rows if r['cell']==c and r['arm'] in ('taban','aday') and float(r['parity'] or 0)>0)
    nn = sum(1 for r in rows if r['cell']==c and r['arm'] in ('taban','aday'))
    m = statistics.median(bw)
    print(c, nn, 'par_runs',p, 'bw_p50',m, 'cap',int(m*1000//125000), 'max',max(bw))
