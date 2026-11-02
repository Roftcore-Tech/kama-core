import json, sys, statistics as st
R=sys.argv[1]
for arm in sys.argv[2:]:
    fl=[];rl=[];dirty=0;fb=[];rb=[];fp=[]
    for rep in range(1,31):
        d=f"{R}/{arm}/rep{rep}"
        pre=json.load(open(d+"/realized_pre.json")); post=json.load(open(d+"/realized_post.json"))
        def dev(j,n): return [q for q in j["mid_qdisc"] if q.get("dev")==n][0]
        for n in ("mb0","ma0"):
            if dev(pre,n)["backlog"] or dev(pre,n)["qlen"] or dev(pre,n)["packets"]: dirty+=1
        f=dev(post,"mb0"); r=dev(post,"ma0")
        fl.append(f["drops"]/(f["packets"]+f["drops"])); rl.append(r["drops"]/(r["packets"]+r["drops"]) if r["packets"]+r["drops"] else 0)
        fb.append(f["bytes"]); rb.append(r["bytes"]); fp.append(f["packets"]+f["drops"])
    print(f"{arm}: fwd kayip med={st.median(fl):.3f} [{min(fl):.3f}-{max(fl):.3f}]  rev kayip med={st.median(rl):.3f}  fwd_deneme_pkt med={st.median(fp)}  fwd_bytes_gecen med={st.median(fb)} rev_bytes med={st.median(rb)} kirli_pre={dirty}")
