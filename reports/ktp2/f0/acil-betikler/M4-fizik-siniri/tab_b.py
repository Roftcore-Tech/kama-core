import json,sys
M={}
for l in open("model_b.jsonl"):
    r=json.loads(l); M[(r["cell"],r["pol"])]=r
X={}
for l in open("meas_b.jsonl"):
    r=json.loads(l); X[(r["cell"],r["arm"])]=r
cells=sorted({c for c,_ in M}, key=lambda c:(c.split('_')[0]!='r1k', int(c.split('_')[1][1:]), int(c.split('_')[2][1:]), int(c.split('_')[3][1:])))
f=lambda x: "–" if x is None else f"{x:.2f}"
print("| hücre | oracle | rfc | F2 | F4 | F8 | F8 ρ | F4fit | F8fit | quinn | q−rfc | taban L/S/N | aday L/S/N | aday−rfc | aday N−rfc |")
print("|"+"---|"*15)
for c in cells:
    g=lambda p,k="p95_med": M.get((c,p),{}).get(k)
    q=X.get((c,"quinn"),{}); t=X.get((c,"taban"),{}); a=X.get((c,"aday"),{})
    qL=q.get("L95",[None])[0]; rfc=g("rfc")
    aL,aS,aN=a["L95"][0],a["S95"][0],a["N95"][0]
    tL,tS,tN=t["L95"][0],t["S95"][0],t["N95"][0]
    print(f"| {c} | {f(g('oracle'))} | {f(rfc)} | {f(g('fec2_E1024'))} | {f(g('fec4_E1024'))} | {f(g('fec8_E1024'))} | {f(g('fec8_E1024','rho_fwd'))} | {f(g('fec4_Efit'))} | {f(g('fec8_Efit'))} | {f(qL)} | {f(qL-rfc) if qL else '–'} | {f(tL)}/{f(tS)}/{f(tN)} | {f(aL)}/{f(aS)}/{f(aN)} | {f(aL-rfc)} | {f(aN-rfc)} |")
