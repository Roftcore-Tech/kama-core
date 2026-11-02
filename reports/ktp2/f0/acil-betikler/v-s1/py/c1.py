import os,re,csv,statistics,collections
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/c/out'
rrep=re.compile(r'fec_repairs_tx=(\d+)'); rden=re.compile(r'parity_denied=(\d+)')
def q(xs,p):
    xs=sorted(xs); 
    if not xs: return None
    k=(len(xs)-1)*p; f=int(k); c=min(f+1,len(xs)-1)
    return xs[f]+(xs[c]-xs[f])*(k-f)
res=collections.defaultdict(list); den=collections.Counter(); cnt=collections.Counter()
for c in sorted(os.listdir(base)):
    p0=os.path.join(base,c)
    if not os.path.isdir(p0): continue
    rate='r12k' if '_r12k_' in c else ('r1k' if '_r1k_' in c else '?')
    for arm in os.listdir(p0):
        d=os.path.join(p0,arm)
        if not os.path.isdir(d): continue
        kind=arm.split('_')[0]
        for rep in os.listdir(d):
            pp=os.path.join(d,rep)
            try: ce=open(os.path.join(pp,'c.err')).read()
            except: continue
            m=rrep.search(ce)
            if not m: continue
            n=int(m.group(1)); dn=int(rden.search(ce).group(1)) if rden.search(ce) else 0
            key=(kind,rate)
            cnt[key]+=1; den[key]+=dn
            if n==0: continue
            # integrate pending_parity
            prev=None; integ=0.0
            with open(os.path.join(pp,'ts.csv')) as fh:
                for row in csv.DictReader(fh):
                    t=int(row['t_ms']); pq=int(row['pending_parity'])
                    if prev is not None:
                        integ+=prev[1]*(t-prev[0])
                    prev=(t,pq)
            res[key].append(integ/n)
for key in sorted(cnt):
    w=res[key]
    print(key,'runs',cnt[key],'runs_with_rep',len(w),'denied',den[key],'wait_p50',q(w,.5),'p90',q(w,.9))
