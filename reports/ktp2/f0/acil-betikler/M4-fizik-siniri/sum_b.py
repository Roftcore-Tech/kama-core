import json
M={};X={}
for l in open("model_b.jsonl"):
    r=json.loads(l); M[(r["cell"],r["pol"])]=r
for l in open("meas_b.jsonl"):
    r=json.loads(l); X[(r["cell"],r["arm"])]=r
cells=sorted({c for c,_ in M})
for arm in ("aday","taban"):
    rows=[]
    for c in cells:
        rfc=M[(c,"rfc")]["p95_med"]; ora=M[(c,"oracle")]["p95_med"]
        q=X[(c,"quinn")]["L95"][0]; k=X[(c,arm)]
        L,S,N=k["L95"][0],k["S95"][0],k["N95"][0]
        rows.append((c,rfc,ora,q,L,S,N))
    qbig=[r for r in rows if r[4]>r[3]]
    print(arm,"cells Kama p95 > quinn p95:",len(qbig),"/",len(rows))
    for grp,sel in (("r12k",lambda c:c.startswith("r12k")),("r1k",lambda c:c.startswith("r1k"))):
        rr=[r for r in rows if sel(r[0])]
        import statistics as st
        print(f" {grp}: med(quinn-rfc)={st.median([r[3]-r[1] for r in rr]):.2f} max={max(r[3]-r[1] for r in rr):.2f} min={min(r[3]-r[1] for r in rr):.2f}"
              f" | med(Kama L-rfc)={st.median([r[4]-r[1] for r in rr]):.2f} max={max(r[4]-r[1] for r in rr):.2f}"
              f" | med(Kama N-rfc)={st.median([r[6]-r[1] for r in rr]):.2f} max={max(r[6]-r[1] for r in rr):.2f}"
              f" | med S95={st.median([r[5] for r in rr]):.2f} max={max(r[5] for r in rr):.2f}")
    # quinn'in kazandığı hücrelerde fark ve S95
    print(" hücre | Kama L95 | quinn | fark | Kama S95 | Kama N95 | rfc")
    for r in qbig:
        print(f"  {r[0]} | {r[4]:.2f} | {r[3]:.2f} | {r[4]-r[3]:.2f} | {r[5]:.2f} | {r[6]:.2f} | {r[1]:.2f}")
