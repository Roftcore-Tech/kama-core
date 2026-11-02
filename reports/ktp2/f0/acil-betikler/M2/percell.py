import json,sys,collections
rows=json.load(open(sys.argv[1]))
want=sys.argv[2].split(',')
g=collections.defaultdict(list)
for r in rows:
    if r['cell'] in want: g[(r['cell'],r['arm'])].append(r)
def S(rs,k): return sum((r.get(k) or 0) for r in rs)
for k in sorted(g):
    rs=g[k]
    print(k,'n=',len(rs),'runs_parity>0=',sum(1 for r in rs if (r.get('parity') or 0)>0),'parity=',S(rs,'parity'),'wire_o01=',sum(r.get('cap_cli',{}).get('o01',0) for r in rs),
      'tx_fec_B=',S(rs,'tx_fec_B'),'srv_rep_rx=',S(rs,'s_fec_repairs_rx'),'srv_recov=',S(rs,'s_fec_recovered_units'),'wire_srv_o02=',sum(r.get('cap_srv',{}).get('o02',0) for r in rs),
      'retx=',S(rs,'retx'),'retx/run=%.1f'%(S(rs,'retx')/len(rs)),'recover_fwd=',S(rs,'recover_fwd'),'gate>0=',sum(1 for r in rs if (r.get('gate_opened') or 0)>0),'clamp>0=',sum(1 for r in rs if (r.get('clamp') or 0)>0),'wz>0=',sum(1 for r in rs if (r.get('would_zero') or 0)>0), 'blocks=',S(rs,'fec_blocks'))
