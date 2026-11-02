import json, sys, gzip, statistics as st
R=sys.argv[1]
for arm in sys.argv[2:]:
    r28=[]; tend=[]; pre=[]
    for rep in range(1,31):
        d=f"{R}/{arm}/rep{rep}"
        m=json.loads(open(d+"/out.jsonl").readline())["meta"]; t0=m["t0_unix_ns"]
        b28=0; last=0; bpre=0
        for line in gzip.open(d+"/capb_cli.csv.gz","rt"):
            if line.startswith("#"): continue
            p=line.split(",")
            if p[1]!="o" or p[2]!="17": continue
            t=int(p[0]); n=int(p[3])
            if t<t0: bpre+=n; continue
            if (t-t0)/1e9<=28.5: b28+=n
            if n>500: last=t
        r28.append(b28/28.5); tend.append((last-t0)/1e9); pre.append(bpre)
    print(f"{arm}: ileri IP bayt/s [t0,t0+28.5s] med={st.median(r28):.0f} [{min(r28):.0f}-{max(r28):.0f}]  son veri paketi t med={st.median(tend):.1f}s  t0-oncesi bayt med={st.median(pre):.0f}")
