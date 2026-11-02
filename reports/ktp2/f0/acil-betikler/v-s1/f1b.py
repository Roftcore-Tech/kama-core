import csv, gzip, statistics, collections, re
P1 = re.compile(r'^b_r(12k|1k)_d(300|1000)_l(0|10|20|30)_p(65|1000)$')
by=collections.defaultdict(list); byarm=collections.defaultdict(list)
for row in csv.DictReader(gzip.open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz','rt')):
    if row['arm'] in ('taban','aday') and row['est_btlbw']:
        by[row['cell']].append(float(row['est_btlbw'])); byarm[(row['cell'],row['arm'])].append(float(row['est_btlbw']))
for c in ['b_r12k_d1000_l0_p1000','b_r12k_d1000_l10_p1000','b_r12k_d1000_l20_p1000','b_r12k_d1000_l30_p1000']:
    v=by[c]; print(c,len(v),statistics.median(v),statistics.median_low(v),statistics.median_high(v), 'taban',statistics.median(byarm[(c,'taban')]),'aday',statistics.median(byarm[(c,'aday')]))
