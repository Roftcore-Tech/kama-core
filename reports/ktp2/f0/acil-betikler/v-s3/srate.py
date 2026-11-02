import gzip, json, os, sys, statistics as st
BASE=os.environ.get("BASE","t0")
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
def t0(d):
    with open(os.path.join(d,'out.jsonl')) as f:
        m=json.loads(f.readline())['meta']
    return m['t0_unix_ns']
def S(d,a=3,b=28,base='t0'):
    try: z=t0(d)
    except Exception: return None
    tot=0; first=None
    rows=[]
    with gzip.open(os.path.join(d,'capb_cli.csv.gz'),'rt') as f:
        for ln in f:
            if ln.startswith('#'): continue
            p=ln.split(',',6)
            if p[1]!='o': continue
            rows.append((int(p[0]),int(p[3])))
    if base=='first': z=rows[0][0]
    for t,l in rows:
        dt=(t-z)/1e9
        if a<=dt<b: tot+=l
    return tot/(b-a)
for d in sys.argv[1:]:
    line=[]
    for arm in ['taban','aday','quinn']:
        vals=[]
        for r in range(1,31):
            dd=os.path.join(R,d,arm,'rep%d'%r)
            if not os.path.exists(os.path.join(dd,'capb_cli.csv.gz')): continue
            v=S(dd,base=BASE)
            if v is not None: vals.append(v)
        line.append('%s n=%d med=%.0f'%(arm,len(vals),st.median(vals) if vals else -1))
    print(d,' | '.join(line))
