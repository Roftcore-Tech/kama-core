import csv,gzip,statistics,sys
rows=list(csv.DictReader(gzip.open(sys.argv[1],'rt')))
for c in ['b_r12k_d1000_l30_p1000','b_r12k_d1000_l0_p1000','b_r12k_d1000_l10_p1000','b_r12k_d1000_l20_p1000']:
    for arm in ('taban','aday'):
        bw=[float(r['est_btlbw']) for r in rows if r['cell']==c and r['arm']==arm]
        print(c,arm,statistics.median(bw),len(bw))
