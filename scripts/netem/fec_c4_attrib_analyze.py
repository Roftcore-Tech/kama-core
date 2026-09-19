#!/usr/bin/env python3
"""[C4-ATIF] Kanıt: parity nereye gidiyor (alıcı partisyonu) + gönderici sınıf-başına token yer-değiştirmesi.
Tablolar ÜÇLÜ (tüm/stall-dışı/stall). Kullanım: fec_c4_attrib_analyze.py ROOT [ROOT...]"""
import sys, glob, re, json, statistics, os
roots=[a for a in sys.argv[1:] if not a.startswith("--")]
def kv(t,k,d=0):
    m=re.search(rf"\b{k}[:=] ?(\d+)", t); return int(m.group(1)) if m else d
def parse(d):
    if not all(os.path.exists(f"{d}/{f}") for f in ("rc","c.err","s.err","out.jsonl","argv.txt")): return None
    a=open(f"{d}/argv.txt").read(); cell=re.search(r"cell=(\S+)",a).group(1); arm=re.search(r"arm=(\S+)",a).group(1)
    rtt=int(re.search(r"rtt=(\d+)",a).group(1))
    c=open(f"{d}/c.err",errors="replace").read(); s=open(f"{d}/s.err",errors="replace").read()
    kc=re.search(r"KP-CLIENT.*",c); ks=re.search(r"KP-SERVER.*",s)
    if not kc or not ks: return None
    kc=kc.group(0); ks=ks.group(0)
    rt=re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)",kc); sched=re.search(r"sched=\[([\d, ]+)\]",kc)
    sd=[int(x) for x in sched.group(1).split(",")] if sched else [0]*12
    D=2*rtt*1_000_000; n=ok=0
    for i,line in enumerate(open(f"{d}/out.jsonl")):
        if i==0: continue
        e=json.loads(line).get("event",{}); n+=1
        if e.get("delivered_ns") is not None and e["delivered_ns"]-e["sent_ns"]<=D: ok+=1
    t_susp=int(rt.group(1)) if rt else 0
    r=dict(cell=cell,arm=arm,rtt=rtt,d2=ok/n if n else 0.0,tx=kv(kc,"tx"),fec=kv(kc,"fec"),retx=kv(kc,"retx"),data=kv(kc,"data"),
           repairs_tx=kv(kc,"fec_repairs_tx"),spurious=kv(kc,"spurious"),stale=kv(kc,"stale"),
           repairs_rx=kv(ks,"fec_repairs_rx"),recovered=kv(ks,"fec_recovered_units"),decode_fail=kv(ks,"fec_decode_failures"),
           evicted=kv(ks,"fec_blocks_evicted"),w_complete=kv(ks,"fec_repairs_wasted_complete"),w_evicted=kv(ks,"fec_repairs_wasted_evicted"),
           useful=kv(ks,"fec_repairs_useful"),b_cwr=kv(ks,"fec_blocks_complete_with_repairs"),b_dec=kv(ks,"fec_blocks_decoded"),b_ewr=kv(ks,"fec_blocks_evicted_with_repairs"),
           data_sel=sd[0],data_den=sd[1],data_emit=sd[2],retx_sel=sd[3],retx_den=sd[4],retx_emit=sd[5],dup_den=sd[7],par_den=sd[9],par_emit=sd[10],
           t_susp=t_susp,stall=(t_susp==0 or t_susp>1_000_000_000),lossy=("loss" in a),t_risk=int(rt.group(3)) if rt else 0)
    return r
runs=[r for r in (parse(d) for root in roots for d in sorted(glob.glob(f"{root}/*/rep*"))) if r]
cells={}
for r in runs: cells.setdefault(r["cell"],{}).setdefault(r["arm"],[]).append(r)
med=lambda v: statistics.median(v) if v else float("nan")
def order(a): return (0,"") if a=="off" else (1 if a.startswith("A") else 2, a)
print("A) ALICI: parity akıbeti (medyan; stall-dışı) — repairs_rx = useful + wasted_complete + wasted_evicted + (açık kalan)")
print(f"{'hücre':8s} {'kol':6s} {'n':>2s} {'stl':>3s} {'d2_ok':>6s} {'d2_all':>6s} {'wire%':>6s} {'parityB':>7s} {'retxB':>6s} {'rep_tx':>6s} {'rep_rx':>6s} {'useful':>6s} {'w_cmpl':>6s} {'w_evct':>6s} {'recov':>5s} {'rec/rx':>6s} {'blk_dec':>7s} {'blk_cwr':>7s} {'blk_ewr':>7s} {'evict':>5s} {'spur':>4s}")
for cell in sorted(cells):
    off=cells[cell].get("off",[]); off_tx=med([r["tx"] for r in off]) if off else float("nan")
    lossy=any(r["lossy"] for rs in cells[cell].values() for r in rs)
    for arm in sorted(cells[cell],key=order):
        rs=cells[cell][arm]; ok=[r for r in rs if not (lossy and r["stall"])] or rs; stl=sum(1 for r in rs if r["stall"]) if lossy else 0
        w=(med([r["tx"] for r in ok])-off_tx)/off_tx*100 if off and off_tx else 0.0
        rr=med([r["repairs_rx"] for r in ok]); rec=med([r["recovered"] for r in ok])
        print(f"{cell:8s} {arm:6s} {len(rs):>2d} {stl:>3d} {med([r['d2'] for r in ok]):>6.3f} {med([r['d2'] for r in rs]):>6.3f} {w:>+6.1f} {med([r['fec'] for r in ok]):>7.0f} {med([r['retx'] for r in ok]):>6.0f} {med([r['repairs_tx'] for r in ok]):>6.0f} {rr:>6.0f} {med([r['useful'] for r in ok]):>6.0f} {med([r['w_complete'] for r in ok]):>6.0f} {med([r['w_evicted'] for r in ok]):>6.0f} {rec:>5.0f} {(rec/rr if rr else 0):>6.2f} {med([r['b_dec'] for r in ok]):>7.0f} {med([r['b_cwr'] for r in ok]):>7.0f} {med([r['b_ewr'] for r in ok]):>7.0f} {med([r['evicted'] for r in ok]):>5.0f} {med([r['spurious'] for r in ok]):>4.0f}")
    print()
print("B) GÖNDERİCİ: sınıf-başına token kabulü (kümülatif medyan; stall-dışı) — denied = pacer.try_send reddi")
print(f"{'hücre':8s} {'kol':6s} {'data_sel':>8s} {'data_den':>8s} {'data_emit':>9s} {'retx_sel':>8s} {'retx_den':>8s} {'retx_emit':>9s} {'dup_den':>7s} {'par_den':>7s} {'par_emit':>8s} {'den/sel data':>12s} {'den/sel retx':>12s}")
for cell in sorted(cells):
    lossy=any(r["lossy"] for rs in cells[cell].values() for r in rs)
    for arm in sorted(cells[cell],key=order):
        rs=cells[cell][arm]; ok=[r for r in rs if not (lossy and r["stall"])] or rs
        ds=med([r['data_sel'] for r in ok]); dd=med([r['data_den'] for r in ok]); rsl=med([r['retx_sel'] for r in ok]); rd=med([r['retx_den'] for r in ok])
        print(f"{cell:8s} {arm:6s} {ds:>8.0f} {dd:>8.0f} {med([r['data_emit'] for r in ok]):>9.0f} {rsl:>8.0f} {rd:>8.0f} {med([r['retx_emit'] for r in ok]):>9.0f} {med([r['dup_den'] for r in ok]):>7.0f} {med([r['par_den'] for r in ok]):>7.0f} {med([r['par_emit'] for r in ok]):>8.0f} {(dd/ds if ds else 0):>12.3f} {(rd/rsl if rsl else 0):>12.3f}")
    print()
viol=[(r['cell'],r['arm'],r['t_susp']) for r in runs if r['arm']!='off' and r['t_risk']>0 and r['t_risk']!=r['t_susp']]
print(f"İNVARYANT t_risk==t_susp: ihlal {len(viol)} (stall'da: {sum(1 for v in viol if v[2]>1e9)})")
