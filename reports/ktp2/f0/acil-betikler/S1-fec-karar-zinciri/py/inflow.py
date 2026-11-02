import os, re, sys, json, collections
ROOT=sys.argv[1]; rows=[json.loads(l) for l in open(sys.argv[2])]
res=collections.Counter(); det=collections.defaultdict(list)
for r in rows:
    if not (r['fec'] and 'adaptive' in r['fec']): continue
    if (r.get('fec_repairs_tx') or 0) <= 0: continue
    p=os.path.join(ROOT,r['lane'],'out',r['cell'],r['arm'],r['rep'],'c.err')
    t=open(p,errors='replace').read()
    m=re.search(r'risk_t=\((\d+), (\d+), (\d+), (\d+)\)',t); f=re.search(r'flow_end=(\d+)',t)
    if not m or not f: res['noparse']+=1; continue
    tfp=int(m.group(4)); fe=int(f.group(1))
    # warmup: bench resets stats at t0; tx.fec==0 with repairs>0 => parity before measurement window
    tx=json.loads(open(os.path.join(ROOT,r['lane'],'out',r['cell'],r['arm'],r['rep'],'out.jsonl')).readline())['meta']['tx']
    kind = 'after_flow_end' if tfp >= fe else ('in_flow' if tx.get('fec',0)>0 else 'warmup_only')
    res[(r["lane"],kind,"bypass" if (r.get("would_zero") or 0)>0 else "cap>=63")]+=1
    det[(r['lane'],kind)].append(r['cell'])
print(res)
for k,v in det.items():
    print(k, collections.Counter(c.split('_')[1] for c in v))
