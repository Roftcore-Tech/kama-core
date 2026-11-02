import csv, gzip, statistics, sys
cols=["sender_wait_p50_s","sender_wait_p95_s","recovery_wait_p95_s","sojourn_p95_s","queue_extra_p95_s","link_busy_frac","est_pace","est_btlbw","sunulan_udp_Bps","btlbw_sunulan","ow_p95_s","rx_ow_p95_s","conf_p95_s","deliv_D","rx_deliv_D","pace_over_link","ilk_iletim_payi","offer_qfull"]
rows=[r for r in csv.DictReader(gzip.open("/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/b/tam/kosular_b.csv.gz","rt")) if r["cell"]==sys.argv[1]]
for arm in ("taban","aday","quinn"):
    rr=[r for r in rows if r["arm"]==arm]
    out=[]
    for c in cols:
        v=[float(r[c]) for r in rr if r.get(c) not in ("",None,"None")]
        out.append(f"{c}={statistics.median(v):.4g}" if v else f"{c}=-")
    print(arm,len(rr)," ".join(out))
