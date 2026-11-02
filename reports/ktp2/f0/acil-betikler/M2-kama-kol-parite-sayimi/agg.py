import csv, sys, collections
rows = list(csv.DictReader(open(sys.argv[1])))
def I(x):
    try: return int(x)
    except: return None
# arm class
def armclass(r):
    a = r['arm']
    if r['lane'] in ('b','d'):
        return r['lane']+':'+a
    if r['lane']=='c':
        return 'c:'+a
    return r['lane']+':'+a
agg = collections.OrderedDict()
for r in rows:
    k = (r['lane'], r['arm'].replace('_taban','').replace('_aday','') if r['lane']=='c' else r['arm'], r['bin'], r['manifest_fec'])
    d = agg.setdefault(k, collections.Counter())
    d['runs'] += 1
    d['missing_cerr'] += (r['has_cerr']=='0')
    d['missing_serr'] += (r['has_serr']=='0')
    for f in ['tx_fec_B','fec_blocks','fec_repairs_tx','parity_fired','parity_denied','retx_fired','recover_fwd','srv_fec_repairs_rx','srv_fec_recovered_units','srv_fec_blocks_decoded','fec2_clamp','fec2_would_zero','fec2_partial','tx_retx_B','gate_opened']:
        v = I(r[f])
        if v is None: d['na_'+f]+=1; continue
        d[f] += v
        if v>0: d['runs_'+f+'>0'] += 1
print('lane arm bin manifest_fec | runs | runs_fecB>0 | sum fec_B | runs_parity>0 | sum parity_fired | sum repairs_tx | sum fec_blocks | srv_rep_rx | srv_recovered | runs_srv_rec>0 | sum retx_fired | runs_gate_open>0 | missing')
for k,d in agg.items():
    print(k, '|', d['runs'], '|', d['runs_tx_fec_B>0'], '|', d['tx_fec_B'], '|', d['runs_parity_fired>0'], '|', d['parity_fired'], '|', d['fec_repairs_tx'], '|', d['fec_blocks'], '|', d['srv_fec_repairs_rx'], '|', d['srv_fec_recovered_units'], '|', d['runs_srv_fec_recovered_units>0'], '|', d['retx_fired'], '|', d['runs_gate_opened>0'], '|', d['missing_cerr'], d['missing_serr'], {x:d[x] for x in d if x.startswith('na_')})
