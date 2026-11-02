import json, os, glob, collections
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
for arm in ['taban']:
    c=collections.Counter(); cb=collections.defaultdict(list)
    for rep in glob.glob(os.path.join(base,'*',arm,'rep*')):
        f=os.path.join(rep,'out.jsonl')
        if not os.path.exists(f): continue
        cell=rep.split('/')[-3]
        meta=json.loads(open(f).readline())['meta']; act=meta['activation']
        p=act.get('parity_emitted',0)
        if p>0:
            c[cell]+=p; cb[cell].append(act.get('btlbw_bps'))
    for k in sorted(c): print(k,c[k],cb[k])
