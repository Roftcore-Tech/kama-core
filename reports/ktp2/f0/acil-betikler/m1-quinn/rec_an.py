import json,gzip,sys,re,math,statistics as st
R=sys.argv[1]
def qnr(v,p):
    v=sorted(v); k=max(1,math.ceil(p*len(v))); return v[k-1]
def load(d):
    L=[json.loads(l) for l in open(f"{d}/out.jsonl")]
    meta=L[0]['meta']; ev=[x['event'] for x in L[1:] if 'event' in x]
    port=int(re.search(r'--server 10\.10\.1\.1:(\d+)',open(f"{d}/cmd.txt").read()).group(1))
    return meta,ev,port
def caprows(p):
    for l in gzip.open(p,'rt'):
        if l.startswith('#'):continue
        a=l.strip().split(',')
        if len(a)>=7: yield int(a[0]),a[1],int(a[2]),int(a[3]),int(a[4]),int(a[5]),a[6]
# quinn sliding window bytes
mx=[];tot=[]
for rep in range(1,31):
    d=f"{R}/quinn/rep{rep}"; meta,ev,port=load(d); t0=meta['t0_unix_ns']
    pk=[(t,ipl) for t,dr,pr,ipl,sp,dp,x in caprows(f"{d}/capb_cli.csv.gz") if dr=='o' and dp==port and t>=t0 and not (int(x[:2],16)&0x80)]
    W=2.137e9; j=0; s=0; m=0
    for i,(t,b) in enumerate(pk):
        s+=b
        while pk[j][0] < t-W: s-=pk[j][1]; j+=1
        m=max(m,s)
    mx.append(m); tot.append(sum(b for _,b in pk))
print("quinn max IP bytes sent in any 2.137s window: median",st.median(mx),"max",max(mx),"total sent median",st.median(tot))
# Kama: retransmission gap and recovery
for arm in ('taban','aday'):
    gap1=[];rec=[];first_ok=[];n_lost_first=0;n=0
    for rep in range(1,31):
        d=f"{R}/{arm}/rep{rep}"; meta,ev,port=load(d)
        tx={}
        for t,dr,pr,ipl,sp,dp,x in caprows(f"{d}/capb_cli.csv.gz"):
            if dr!='o' or dp!=port: continue
            b=bytes.fromhex(x.split(':')[0])
            if b and b[0]==0 and len(b)>=11:
                tx.setdefault(int.from_bytes(b[3:11],'big'),[]).append(t)
        arr={}
        for t,dr,pr,ipl,sp,dp,x in caprows(f"{d}/capb_srv.csv.gz"):
            if dr!='i' or dp!=port: continue
            b=bytes.fromhex(x.split(':')[0])
            if b and b[0]==0 and len(b)>=11:
                arr.setdefault(int.from_bytes(b[3:11],'big'),[]).append(t)
        for u,ts in tx.items():
            if u>=100: continue
            n+=1
            a=arr.get(u,[])
            if not a: continue
            fa=min(a)
            # was first transmission delivered? first arrival within 1.6s of first tx (one-way <= ~1.6s)
            if fa-ts[0] < 1.6e9:
                first_ok.append((fa-ts[0])/1e9)
            else:
                n_lost_first+=1
                rec.append((fa-ts[0])/1e9)
                if len(ts)>=2: gap1.append((ts[1]-ts[0])/1e9)
    print(arm,"units",n,"first-tx-lost",n_lost_first,"| first-tx->first-arrival (first ok) p50 %.2f"%st.median(first_ok),
          "| lost-first: first-tx->2nd-tx p50 %.2f p95 %.2f"%(st.median(gap1),qnr(gap1,.95)),
          "| first-tx->first-arrival p50 %.2f p95 %.2f"%(st.median(rec),qnr(rec,.95)))
