import sys, re, csv, glob, statistics, collections
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
man=re.compile(r'KP-MANIFEST role=client .*?fec=(ON adaptive p0x\w+|ON static-r\d p0x\w+|OFF)')
def q(v,p):
    v=sorted(v); 
    if not v: return None
    k=(len(v)-1)*p; f=int(k); c=min(f+1,len(v)-1); return v[f]+(v[c]-v[f])*(k-f)
groups=collections.defaultdict(list); denied=collections.Counter(); dt_check=collections.Counter()
for lane in 'bcd':
  for cell, arm, rep, p in runs(lane):
    t=open(p,errors='replace').read()
    m=man.search(t)
    if not m: continue
    mode=m.group(1)
    d=parse_cerr(p)
    rtx=d.get('fec_repairs_tx',0)
    rate='r12k' if '_r12k_' in cell else ('r1k' if '_r1k_' in cell else ('r25' if '_r25_' in cell else 'o'))
    key=None
    if mode.startswith('ON static-r2 p0x01'): key=('static-r2-p01',rate)
    elif mode.startswith('ON static-r4') or 'p0x02' in mode and 'static' in mode: key=('static-'+mode.split()[1]+'-'+mode.split()[2],rate)
    elif mode.startswith('ON adaptive') and rtx>0: key=('adaptive',rate)
    if key is None: continue
    ts=p[:-5]+"ts.csv"
    import os
    if not os.path.exists(ts): continue
    integ=0; prev=None; pd_last=0
    with open(ts) as f:
        rr=csv.reader(f); h=next(rr); ip=h.index('pending_parity'); it=h.index('t_ms'); idn=h.index('parity_denied')
        for row in rr:
            if len(row)<len(h): continue
            tm=int(row[it])
            dtt=50 if prev is None else tm-prev
            dt_check[dtt]+=1
            integ+=int(row[ip])*dtt; prev=tm; pd_last=int(row[idn])
    pdn=d['parity_denied'][1] if d.get('parity_denied') else 0
    denied[key]+=pdn
    if rtx>0: groups[key].append(integ/rtx)
for k in sorted(groups, key=str):
    v=groups[k]; print(k, 'n',len(v), 'p50', round(q(v,.5)), 'p90', round(q(v,.9)), 'denied', denied[k])
print(dt_check.most_common(4))
