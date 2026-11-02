import sys, gzip, os, glob, re, json, statistics as st
root=sys.argv[1]; pat=sys.argv[2]
def pct(v,q):
    v=sorted(v)
    if not v: return float('nan')
    k=(len(v)-1)*q; f=int(k); c=min(f+1,len(v)-1); return v[f]+(v[c]-v[f])*(k-f)
cells=sorted(d for d in os.listdir(root) if os.path.isdir(os.path.join(root,d)) and re.match(pat,d))
print("cell | arm | N | S_med(B/s,IP,[3,ival*msgs)) | S/A | p95_med | btlbw_end_med | pup_med")
for cell in cells:
    env=open(os.path.join(root,cell,'cell.env')).read() if os.path.exists(os.path.join(root,cell,'cell.env')) else ''
    def g(k):
        m=re.search(rf'^{k}=(\S+)',env,re.M); return m.group(1).strip('"') if m else None
    import re as _r; specf=g("spec"); txt=open(specf).read() if specf and os.path.exists(specf) else ""
    gg=lambda k,d: int(_r.search(rf"^{k}=(\d+)",txt,_r.M).group(1)) if _r.search(rf"^{k}=(\d+)",txt,_r.M) else d
    msgs=gg("MSGS",100); pay=gg("PAY",1000); ival=gg("IVAL_MS",285)/1000; loss=gg("LOSS_PCT",0)/100
    t_hi=max(4.0, msgs*ival)
    for arm in ['taban','aday','quinn']:
        paths=sorted(glob.glob(os.path.join(root,cell,arm,'rep*')))
        S=[];P=[];B=[];U=[]; A=None
        for p in paths:
            f=os.path.join(p,'capb_cli.csv.gz')
            if not os.path.exists(f): continue
            rows=[]
            try:
                for line in gzip.open(f,'rt'):
                    parts=line.strip().split(',')
                    if len(parts)<4: continue
                    rows.append((int(parts[0]),parts[1],int(parts[3])))
            except Exception: continue
            if not rows: continue
            t0=rows[0][0]
            b=sum(ln for t,d,ln in rows if d=='o' and 3<=(t-t0)/1e9<t_hi)
            S.append(b/(t_hi-3))
            lat=[]
            try:
                for line in open(os.path.join(p,'out.jsonl')):
                    j=json.loads(line)
                    if 'event' in j:
                        e=j['event']
                        if e.get('delivered_ns') is not None: lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
            except Exception: pass
            if lat: P.append(pct(lat,0.95))
            ce=open(os.path.join(p,'c.err')).read() if os.path.exists(os.path.join(p,'c.err')) else ''
            m=re.search(r'btlbw=(\d+)B/s',ce)
            if m: B.append(int(m.group(1)))
            m=re.search(r'probe_up=(\d+)',ce)
            if m: U.append(int(m.group(1)))
        if not S: continue
        Aip=(pay+55)/ival  # approx IP bytes/msg (kama 1055 for 1000B)
        md=lambda v: st.median(v) if v else float('nan')
        print(f"{cell} | {arm} | {len(S)} | {md(S):6.0f} | {md(S)/Aip:4.2f} (A={Aip:.0f}, A/(1-p)={Aip/(1-loss) if loss<1 else 0:.0f}) | {md(P):6.2f} | {md(B)} | {md(U)}")
