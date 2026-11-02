import csv, sys, collections, statistics
rows = list(csv.DictReader(open(sys.argv[1])))
lanes = sys.argv[2].split(',')
def I(x):
    try: return int(x)
    except: return 0
agg = collections.OrderedDict()
for r in rows:
    if r['lane'] not in lanes: continue
    if len(sys.argv)>3 and sys.argv[3] not in r['arm']: continue
    k=(r['cell'], r['arm'])
    d=agg.setdefault(k, collections.defaultdict(list))
    for f in ['tx_fec_B','parity_fired','fec_repairs_tx','fec_blocks','srv_fec_repairs_rx','srv_fec_recovered_units','retx_fired','tx_retx_B','gate_opened','fec2_clamp','fec2_partial','btlbw','srtt_us','recover_fwd','fec2_would_zero','tx_data_B','parity_denied']:
        d[f].append(I(r[f]))
print('%-28s %-7s %4s %5s %6s %6s %6s %6s %6s %7s %8s %6s %7s %8s %6s'%('cell','arm','n','nFEC','parSum','blkSum','rRxSum','recSum','retxMed','retxSum','gateOpN','partS','btlbwMd','retx/data','fwdS'))
for (c,a),d in agg.items():
    n=len(d['tx_fec_B'])
    print('%-28s %-7s %4d %5d %6d %6d %6d %6d %6d %7d %8d %6d %7d %8.3f %6d'%(c,a,n,sum(1 for x in d['parity_fired'] if x>0),sum(d['parity_fired']),sum(d['fec_blocks']),sum(d['srv_fec_repairs_rx']),sum(d['srv_fec_recovered_units']),statistics.median(d['retx_fired']),sum(d['retx_fired']),sum(1 for x in d['gate_opened'] if x>0),sum(d['fec2_partial']),statistics.median(d['btlbw']), sum(d['tx_retx_B'])/max(1,sum(d['tx_data_B'])), sum(d['recover_fwd'])))
