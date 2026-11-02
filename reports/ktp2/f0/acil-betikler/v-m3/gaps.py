import gzip,os,sys,json,statistics as st,math
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/'+sys.argv[1]
def pct(v,q):
    v=sorted(v); k=(len(v)-1)*q; f=math.floor(k); c=math.ceil(k); return v[f]+(v[c]-v[f])*(k-f)
for arm in ('taban','aday'):
    g12=[];g23=[];ntx_top=[];lastgap=[]
    for rep in os.listdir(os.path.join(base,arm)):
        d=os.path.join(base,arm,rep)
        txs={}
        for line in gzip.open(os.path.join(d,'capb_cli.csv.gz'),'rt'):
            f=line.rstrip('\n').split(',')
            if len(f)<7 or f[1]!='o': continue
            h=f[6].split(':')[0]
            if len(h)<22 or h[:2]!='00': continue
            txs.setdefault(int(h[6:22],16),[]).append(int(f[0]))
        for u,t in txs.items():
            if len(t)>1: g12.append((t[1]-t[0])/1e9)
            if len(t)>2: g23.append((t[2]-t[1])/1e9)
    print(arm,'gap tx1->tx2 p25/p50/p75/p95: %.2f %.2f %.2f %.2f n=%d'%(pct(g12,.25),pct(g12,.5),pct(g12,.75),pct(g12,.95),len(g12)), ' tx2->tx3 p50 %.2f n=%d'%(pct(g23,.5),len(g23)))
