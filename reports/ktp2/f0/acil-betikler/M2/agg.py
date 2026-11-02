import json,sys,collections
rows=json.load(open(sys.argv[1]))
def S(rs,k): return sum((r.get(k) or 0) for r in rs)
g=collections.defaultdict(list)
for r in rows:
    a=r['arm']
    base=a.split('_')[0] if r['lane']=='c' else a
    g[(r['lane'],base,r.get('fec','?'),r['fecargs'])].append(r)
print("lane arm fec fecargs | n | parity(ACT) tx_fec_B fec_blocks repairs_tx | s_rep_rx s_recov s_decoded | retx(ACT) retx_B | clamp>0 would_zero | wire o01(cli) o02(srv) | gate_opened>0")
for k,rs in sorted(g.items()):
    w01=sum(r.get('cap_cli',{}).get('o01',0) for r in rs); w02=sum(r.get('cap_srv',{}).get('o02',0) for r in rs)
    capn=sum(1 for r in rs if 'cap_cli' in r)
    print(k,'|',len(rs),'|',S(rs,'parity'),S(rs,'tx_fec_B'),S(rs,'fec_blocks'),S(rs,'fec_repairs_tx'),'|',S(rs,'s_fec_repairs_rx'),S(rs,'s_fec_recovered_units'),S(rs,'s_fec_blocks_decoded'),'|',S(rs,'retx'),S(rs,'retx_B'),'|',sum(1 for r in rs if (r.get('clamp') or 0)>0),S(rs,'would_zero'),'|',w01,w02,f'(cap n={capn})','|',sum(1 for r in rs if (r.get('gate_opened') or 0)>0), 'missing_act=',sum(1 for r in rs if r.get('parity') is None))
