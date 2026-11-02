import csv, statistics, sys
rows=[r for r in csv.DictReader(open(sys.argv[1])) if r['cell']==sys.argv[2]]
by={}
for r in rows: by.setdefault(r['arm'],[]).append(r)
for a,rs in by.items():
    def med(k):
        v=[float(r[k]) for r in rs if r[k] not in ('','inf','nan')]
        return (statistics.median(v) if v else None, len(v))
    print(a,len(rs),'lat_p50',med('lat_p50_s'),'lat_p95',med('lat_p95_s'),'ow_p95',med('ow_p95_s'),'cD_ss',med('cD_ss'),'setup',med('setup_s'),'fwd_bytes',med('fwd_bytes'),'loss_fwd',med('loss_fwd'),'loss_rev',med('loss_rev'))
