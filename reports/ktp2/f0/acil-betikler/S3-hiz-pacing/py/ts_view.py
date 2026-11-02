import sys, csv
f=sys.argv[1]; step=float(sys.argv[2])
rows=list(csv.DictReader(open(f)))
print("cols:", list(rows[0].keys()))
nxt=0; n=0; nz=0; tok_lo=0
for r in rows:
    n+=1
    if int(r['pending_lost'])>0: nz+=1
    if int(r['pacer_tokens'])<1027: tok_lo+=1
    t=int(r['t_ms'])
    if t>=nxt:
        print(t, 'pend_lost=',r['pending_lost'],'tokens=',r['pacer_tokens'],'data_em=',r['data_emitted'],'retx_em=',r['retx_emitted'],'par_em=',r['parity_emitted'],'final_ratio=',r['final_ratio'],'reason=',r['reason_idx'])
        nxt=t+step*1000
print(f"rows={n} pending_lost>0 fraction={nz/n:.3f} tokens<1027 fraction={tok_lo/n:.3f}")
