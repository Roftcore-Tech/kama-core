import json, sys, math, statistics as st
R=sys.argv[1]
def nr(v,p):
    v=sorted(v); i=max(0,math.ceil(p*len(v))-1); return v[i]
for arm in sys.argv[2:]:
    bins={k:[] for k in range(10)}
    p95_up=[];p95_lo=[];p95=[]
    for rep in range(1,31):
        L=[json.loads(l) for l in open(f"{R}/{arm}/rep{rep}/out.jsonl")]
        m=L[0]["meta"]; ev=[x["event"] for x in L[1:]]
        lat=[(e["delivered_ns"]-e["sent_ns"])/1e9 if e["delivered_ns"] is not None else math.inf for e in ev]
        for e,l in zip(ev,lat): bins[e["msg_id"]//10].append(min(l,60))
        s=m["setup_ns"]/1e9; iv=m["submit_interval_ns"]/1e9
        p95.append(min(nr(lat,.95),60))
        p95_up.append(min(nr([l+s for l in lat],.95),60))
        p95_lo.append(min(nr([l+max(0,s-e["msg_id"]*iv) for e,l in zip(ev,lat)],.95),60))
    print(arm, "dilim medyanlari(msg 0-9..90-99):", " ".join(f"{st.median(bins[k]):.1f}" for k in range(10)))
    print(arm, f"p95 med={st.median(p95):.2f}  +setup(ust)={st.median(p95_up):.2f}  +max(0,setup-i*I)(alt)={st.median(p95_lo):.2f}")
