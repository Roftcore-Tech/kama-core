import re,glob,os,sys,json,collections
root=sys.argv[1]
agg=collections.defaultdict(lambda: collections.Counter())
supp=collections.defaultdict(collections.Counter)
for ce in glob.glob(root+'/[bcd]/out/*/*/rep*/c.err'):
    d=os.path.dirname(ce); p=d.split('/'); lane,cell,arm=p[-5],p[-3],p[-2]
    t=open(ce,errors='replace').read()
    if 'KP-ACTIVATED' not in t: continue
    m=re.search(r'kama_arm|fec_blocks=(\d+)',t)
    oj=os.path.join(d,'out.jsonl')
    try: meta=json.loads(open(oj).readline())['meta']
    except Exception: continue
    ka=meta.get('kama_arm','')
    if 'adaptive-p0x05' not in ka: continue
    key=(lane,cell.split("_")[1] if lane=="b" else "")
    c=agg[key]
    c['runs']+=1
    fb=re.search(r'fec_blocks=(\d+)',t); c['fec_blocks']+=int(fb.group(1))
    la=re.search(r'latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)',t)
    if la:
        a=list(map(int,la.groups())); c['no_elig']+=a[0];c['mismatch']+=a[1];c['no_rule']+=a[2];c['enq']+=a[3];c['emit']+=a[4]
    f2=re.search(r'clamp=(\d+) would_zero=(\d+) partial_blocks=(\d+)',t)
    if f2:
        c['clamp']+=int(f2.group(1)); c['would_zero']+=int(f2.group(2)); c['partial']+=int(f2.group(3))
        if int(f2.group(1))>0: c['runs_clamp>0']+=1
        if int(f2.group(2))>0: c['runs_wz>0']+=1
    pc=re.search(r'preconv_parity_blocked=(\d+)',t); c['preconv_blocked']+=int(pc.group(1))
    s=re.search(r" suppress=(\S+)",t); supp[key][s.group(1) if s else '?']+=1
    c['units']+=meta['msg_count']+0
for k in agg: print(k,dict(agg[k])); print('  last-suppress',dict(supp[k]))
