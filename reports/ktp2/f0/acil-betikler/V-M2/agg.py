import csv,sys,collections
rows=list(csv.DictReader(open(sys.argv[1])))
I=lambda x:int(x) if x not in ('',None,'None') else 0
def agg(f,label):
    sel=[r for r in rows if f(r)]
    t=collections.Counter()
    for r in sel:
        t['runs']+=1
        t['par']+=I(r['parity']); t['par_runs']+=I(r['parity'])>0
        t['rep_tx']+=I(r['fec_repairs_tx'])
        t['rx']+=I(r['srv_rep_rx']); t['rec']+=I(r['srv_rec']); t['rec_runs']+=I(r['srv_rec'])>0
        t['retx']+=I(r['retx_n']); t['retxB']+=I(r['tx_retx_B']); t['fecB']+=I(r['tx_fec_B'])
        t['rec_ctrl']+=I(r['recovered_ctrl']); t['denied']+=I(r['parity_denied'])
        t['blocks']+=I(r['fec_blocks']); t['partial']+=I(r['partial'])
    print(label, dict(t))
agg(lambda r:r['lane']=='b' and r['arm']=='taban','B taban')
agg(lambda r:r['lane']=='b' and r['arm']=='aday','B aday')
print(collections.Counter((r['lane'],r['arm'],r['fecmode']) for r in rows if r['lane']=='b'))
for c in ['b_r12k_d1000_l30_p1000','b_r12k_d300_l20_p65','b_r1k_d300_l20_p1000','b_r12k_d1000_l0_p1000']:
  for a in ['taban','aday']:
    agg(lambda r:r['cell']==c and r['arm']==a, c+'/'+a)
import re
for a in ['taban','aday']:
    agg(lambda r:r['lane']=='b' and r['arm']==a and r['cell'].startswith('b_r12k') and not re.search(r'_l0_',r['cell']), 'B r12k loss>0 '+a)
    agg(lambda r:r['lane']=='b' and r['arm']==a and r['cell'].startswith('b_r25') and not re.search(r'_l0_',r['cell']), 'B r25 loss>0 '+a)
for r in rows:
    if r['cell']=='b_r12k_d1000_l30_p1000' and r['arm']=='aday' and I(r['parity'])>0: print(r)
