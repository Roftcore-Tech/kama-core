import sys, statistics as st
sys.path.insert(0, ".")
from an import *
rows=[]
for k in range(1,31):
    rep=BASE/"quinn"/f"rep{k}"
    meta,ev=read_out(rep); rx=read_rx(rep); t0=meta["t0_unix_ns"]
    off={e["msg_id"]:t0+e["sent_ns"] for e in ev}
    # quinn rx_log u: msg id (payload-id)
    rl=[(rx[m]-off[m])/1e9 for m in off if m in rx]
    cap=read_cap(rep/"capb_cli.csv.gz")
    o=[(t,ln) for t,d,pr,ln,sp,dp,x in cap if d=="o" and pr==17]
    big=[(t,ln) for t,ln in o if ln>500]
    t1=min(t for t,_ in big); t2=max(t for t,_ in big)
    tot=sum(ln for _,ln in big)
    last_off=max(off.values())
    # sent bytes from first big pkt until last offer
    b_until=sum(ln for t,ln in big if t<=last_off)
    rows.append(dict(rep=k,n=len(rl),rx_p50=pct(rl,.5),rx_p95=pct(rl,.95),big=len(big),dur=(t2-t1)/1e9,emit=tot/((t2-t1)/1e9),emit_until_lastoff=b_until/((last_off-t1)/1e9), last_big_after_lastoff=(t2-last_off)/1e9, min_off_neg=min(rx.get(m,1e30)-off[m] for m in off)/1e9))
keys=list(rows[0])
print("\t".join(keys))
for r in rows: print("\t".join(f"{r[x]:.3f}" if isinstance(r[x],float) else str(r[x]) for x in keys))
for x in keys[1:]:
    v=[r[x] for r in rows]; print(f"MED {x} {st.median(v):.3f} min {min(v):.3f} max {max(v):.3f}")
