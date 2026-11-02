import csv,sys
for p in sys.argv[1:]:
    rows=list(csv.DictReader(open(p)))
    for col in ['parity_denied','parity_emitted','retx_emitted','data_emitted','retx_denied','data_denied']:
        v=[int(r[col]) for r in rows]
        print(p.split('/out/')[1],col,'sum',sum(v),'max',max(v),'last',v[-1], 'monotone', all(v[i]<=v[i+1] for i in range(len(v)-1)))
    # first time parity emitted
    t=[int(r['t_ms']) for r in rows if int(r['parity_emitted'])>0]
    print(' first parity_emitted>0 at', t[:3])
