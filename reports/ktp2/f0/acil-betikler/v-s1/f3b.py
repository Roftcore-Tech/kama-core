import sys, re, csv, os, collections
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
man=re.compile(r'KP-MANIFEST role=client .*?fec=(ON adaptive p0x05|ON static-r\d p0x\w+)')
agg=collections.defaultdict(lambda:[0,0,0,0])  # runs, pending samples, tokens>=psize samples, tokmax
for lane in 'bc':
  for cell, arm, rep, p in runs(lane):
    t=open(p,errors='replace').read(); m=man.search(t)
    if not m: continue
    d=parse_cerr(p)
    if d['latch'][3]==0: continue
    psize = (d['txfec'][4]//d['fec_repairs_tx']) if d['fec_repairs_tx'] else (1052 if 'p0x05' in m.group(1) else None)
    if psize is None: continue
    ts=p[:-5]+'ts.csv'
    if not os.path.exists(ts): continue
    rate='r12k' if '_r12k_' in cell else ('r1k' if '_r1k_' in cell else 'r25')
    key=(m.group(1).split()[-1], rate, psize)
    a=agg[key]; a[0]+=1
    with open(ts) as f:
        for r in csv.DictReader(f):
            if int(r['pending_parity'])>0:
                a[1]+=1; tk=int(r['pacer_tokens'])
                if tk>=psize: a[2]+=1
                a[3]=max(a[3],tk)
for k,v in sorted(agg.items(), key=str): print(k, 'runs',v[0],'pending samples',v[1],'tokens>=psize',v[2], 'frac', round(v[2]/v[1],4) if v[1] else None, 'tokmax', v[3])
