import sys, statistics as st
sys.path.insert(0, ".")
from an import *
arm=sys.argv[1]; k=int(sys.argv[2]); B=float(sys.argv[3]) if len(sys.argv)>3 else 2.0
rep=BASE/arm/f"rep{k}"
meta,ev,rx,off,conf,tx,acks=kama_run(rep)
t0=meta["t0_unix_ns"]
first={}; seq_t={}; seq_u={}; seq_ln={}; kind={}
seen=set()
for t,u,s,ln in tx:
    seq_t.setdefault(s,t); seq_ln[s]=ln
    if u is None: kind[s]="C"; continue
    if u in seen: kind[s]="R"
    else: kind[s]="D"; seen.add(u); first[u]=t
    seq_u[s]=u
acked={}
for t,s16 in acks:
    for s in s16:
        if s in seq_t and s not in acked and t>seq_t[s]: acked[s]=t
T=lambda x:(x-t0)/1e9
end=max(T(t) for t,*_ in tx)
nb=int(end/B)+1
print(f"# {arm} rep{k}  btlbw_end={meta['activation']['btlbw_bps']} srtt_end={meta['activation']['srtt_us']/1e6:.2f} trans={meta['activation']['state_transitions']} rarq={meta['activation']['recover_arq_entries']} retx={meta['activation']['retransmits']}")
print("bin_s  offered  firsttx  backlog  emitD_Bps emitR_Bps emitTot  ackDeliv_Bps  rtt_med  pace_inf_med  ctrl_o")
data=[(T(t),u,s,ln) for t,u,s,ln in tx if u is not None]
ctrl=[(T(t),ln) for t,u,s,ln in tx if u is None]
for b in range(nb):
    lo,hi=b*B,(b+1)*B
    offd=sum(1 for m in off if T(off[m])<hi)
    ftx=sum(1 for u in first if T(first[u])<hi)
    eD=sum(ln for t,u,s,ln in data if lo<=t<hi and kind[s]=="D")/B
    eR=sum(ln for t,u,s,ln in data if lo<=t<hi and kind[s]=="R")/B
    ad=sum(seq_ln[s] for s in acked if lo<=T(acked[s])<hi)/B
    r=[ (acked[s]-seq_t[s])/1e9 for s in acked if lo<=T(seq_t[s])<hi and kind[s]=="D"]
    ts=sorted(t for t,*_ in data if lo<=t<hi)
    gaps=[ts[i+1]-ts[i] for i in range(len(ts)-1)]
    pi=[1055/g for g in gaps if g>0.002]
    co=sum(1 for t,ln in ctrl if lo<=t<hi)
    print(f"{lo:5.0f} {offd:8d} {ftx:8d} {offd-ftx:8d} {eD:9.0f} {eR:9.0f} {eD+eR:8.0f} {ad:12.0f} {st.median(r) if r else float('nan'):8.2f} {st.median(pi) if pi else float('nan'):12.0f} {co:6d}")
