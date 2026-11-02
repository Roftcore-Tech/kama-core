import csv, sys
r=list(csv.DictReader(open(sys.argv[1])))
cols=['data_denied','retx_denied','parity_denied','data_emitted','retx_emitted','parity_emitted','pending_parity','pending_lost']
vals={c:[int(x[c]) for x in r] for c in cols}
for c in cols:
    v=vals[c]; print(c,'max',max(v),'last',v[-1],'sum',sum(v),'monotone',all(v[i]<=v[i+1] for i in range(len(v)-1)))
