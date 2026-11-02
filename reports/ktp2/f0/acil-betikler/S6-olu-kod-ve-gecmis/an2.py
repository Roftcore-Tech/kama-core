import csv, sys, collections
rows=list(csv.DictReader(open(sys.argv[1]),delimiter='\t'))
ad=[r for r in rows if r['fec'].startswith('ON adaptive p0x05')]
def i(x):
    try: return int(x)
    except: return 0
by=collections.defaultdict(lambda: collections.Counter())
for r in ad:
    k=(r['lane'],r['arm'])
    c=by[k]
    c['n']+=1
    rep=i(r['fec_repairs_tx'])>0
    wz=i(r['would_zero'])>0
    c['rep>0']+=rep
    c['wz>0']+=wz
    c['rep&wz']+=rep and wz
    c['rep&!wz']+=rep and not wz
    c['clamp>0']+=i(r['clamp'])>0
    c['gate>0']+=i(r['gate_opened'])>0
    c['repairs_sum']+=i(r['fec_repairs_tx'])
for k in sorted(by): print(k, dict(by[k]))
# cells with rep>0 and not wz
cc=collections.Counter()
for r in ad:
    if i(r['fec_repairs_tx'])>0 and i(r['would_zero'])==0:
        cc[(r['lane'],r['cell'])]+=1
print('rep>0 & would_zero==0 by cell:', cc.most_common(20))
cw=collections.Counter()
for r in ad:
    if i(r['fec_repairs_tx'])>0 and i(r['would_zero'])>0:
        cw[(r['lane'],r['cell'])]+=1
print('rep>0 & would_zero>0 by cell:', cw.most_common(20))
