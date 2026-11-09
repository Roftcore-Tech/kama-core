import json,sys,glob,statistics as st
root=sys.argv[1]
for arm in sys.argv[2:]:
    rows=[]
    for rep in sorted(glob.glob(f"{root}/a_r12k_d150_l20_foto150k__{arm}/{arm}/rep*")):
        a=json.load(open(rep+"/alice.json")); f=a["file"]
        t0,t1=f["sent"],f["delivered"]
        if not t1: continue
        snaps=[]
        for l in open(rep+"/alice.jsonl"):
            d=json.loads(l)
            if d.get("k")=="ts" and d.get("s"): snaps.append((d["t"],d["s"][0]))
        def at(t):
            best=None
            for ts,s in snaps:
                if ts<=t: best=s
            return best
        s0,s1=at(t0),at(t1)
        dur=(t1-t0)/1000
        txb=s1["txb"]-s0["txb"]; rtxb=s1["rtxb"]-s0["rtxb"]; rtx=s1["rtx"]-s0["rtx"]
        rows.append((dur,txb/dur,rtxb/max(txb,1),rtx))
    if rows:
        print(f"{arm}: n={len(rows)} dosya süresi med {st.median(r[0] for r in rows):.1f} s | tel hızı med {st.median(r[1] for r in rows):.0f} B/s | retx payı med {st.median(r[2] for r in rows):.3f} | retx adet med {st.median(r[3] for r in rows):.0f}")
