import json,sys,collections,os,re
rows=json.load(open(sys.argv[1]))
T="/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
g=collections.defaultdict(list)
for r in rows:
    if r['lane'] in sys.argv[2] : g[(r['cell'],r['arm'])].append(r)
def S(rs,k): return sum((r.get(k) or 0) for r in rs)
tot=collections.Counter()
for k in sorted(g):
    rs=g[k]; bws=[]
    for r in rs:
        m=f"{T}/{r['lane']}/out/{r['cell']}/{r['arm']}/{r['rep']}/manifest.json"
        try: a=json.load(open(m))['activation']; bws.append(a['btlbw_bps'])
        except Exception: pass
    bws.sort(); med=bws[len(bws)//2] if bws else -1
    cap=min(med*1000//125000,900) if med>0 else 900
    rr=(cap*8+500)//1000
    np_=sum(1 for r in rs if (r.get('parity') or 0)>0)
    tot[(k[1],'runs')]+=len(rs); tot[(k[1],'prun')]+=np_; tot[(k[1],'par')]+=S(rs,'parity'); tot[(k[1],'rec')]+=S(rs,'s_fec_recovered_units'); tot[(k[1],'retx')]+=S(rs,'retx')
    if S(rs,'parity')>0 or 'l30_p1000' in k[0] or 'l20' in k[0]:
        print(f"{k[0]:28s} {k[1]:12s} n={len(rs):2d} prun={np_:2d} parity={S(rs,'parity'):4d} srv_rx={S(rs,'s_fec_repairs_rx'):4d} recov={S(rs,'s_fec_recovered_units'):3d} retx={S(rs,'retx'):5d} btlbw_med={med:6d}B/s cap={cap:3d}‰ r@K8={rr}")
for a in sorted(set(x[0] for x in tot)):
    print(a, {kk:tot[(a,kk)] for kk in ['runs','prun','par','rec','retx']})
