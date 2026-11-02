import gzip,json,os,sys,statistics as st
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
def load(d):
    z=json.loads(open(os.path.join(d,'out.jsonl')).readline())['meta']['t0_unix_ns']
    out=[]
    with gzip.open(os.path.join(d,'capb_cli.csv.gz'),'rt') as f:
        for ln in f:
            if ln.startswith('#'): continue
            p=ln.rstrip('\n').split(',',6)
            if p[1]!='o': continue
            t=(int(p[0])-z)/1e9; x=p[6]
            h=x.split(':')[0]
            try:
                b=bytes.fromhex(h)
            except ValueError:
                b=b""
            ft=b[0] if b else None
            unit=int.from_bytes(b[3:11],'big') if ft==0 and len(b)>=11 else None
            out.append((t,int(p[3]),ft,unit))
    return out
def stats(d,a=3,b=28):
    pk=load(d)
    seen=set(); first={}; n=0; rt=0
    for t,l,ft,u in pk:
        if ft==0:
            isr = u in seen
            seen.add(u)
            if u not in first: first[u]=t
            if a<=t<b:
                n+=1; rt+=isr
    fec=[t for t,l,ft,u in pk if ft==1]
    return (rt/n if n else None), max(first.values()) if first else None, fec
mode=sys.argv[1]
if mode=='share':
    for c in sys.argv[2:]:
        for arm in ['taban','aday']:
            v=[];lf=[]
            for r in range(1,31):
                d=os.path.join(R,c,arm,'rep%d'%r)
                if not os.path.exists(os.path.join(d,'capb_cli.csv.gz')): continue
                s,l,_=stats(d); 
                if s is not None: v.append(s)
                if l is not None: lf.append(l)
            print(c,arm,'n',len(v),'retx_share_med %.3f'%st.median(v),'last_first_tx_med %.1f'%st.median(lf))
else:
    for d in sys.argv[2:]:
        s,l,fec=stats(os.path.join(R,d))
        print(d,'retx_share',s,'last_first_tx',l,'fec_tx_times',fec)
