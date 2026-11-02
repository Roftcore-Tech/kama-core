import sys, gzip, os, glob, re, json, statistics as st
celldir = sys.argv[1]; arms = sys.argv[2].split(','); t_lo=float(sys.argv[3]); t_hi=float(sys.argv[4])
def pct(v,q):
    v=sorted(v)
    if not v: return float('nan')
    k=(len(v)-1)*q; f=int(k); c=min(f+1,len(v)-1); return v[f]+(v[c]-v[f])*(k-f)
for arm in arms:
    paths = sorted(glob.glob(os.path.join(celldir, arm, 'rep*')), key=lambda p:int(p.rsplit('rep',1)[1]))
    res=[]
    for p in paths:
        f=os.path.join(p,'capb_cli.csv.gz')
        if not os.path.exists(f): continue
        rows=[]
        for line in gzip.open(f,'rt'):
            parts=line.strip().split(',')
            if len(parts)<4: continue
            rows.append((int(parts[0]),parts[1],int(parts[3])))
        if not rows: continue
        t0=rows[0][0]
        b=sum(ln for t,d,ln in rows if d=='o' and t_lo<=(t-t0)/1e9<t_hi)
        rate=b/(t_hi-t_lo)
        ce=open(os.path.join(p,'c.err')).read() if os.path.exists(os.path.join(p,'c.err')) else ''
        m=re.search(r'btlbw=(\d+)B/s',ce); bw=int(m.group(1)) if m else -1
        m=re.search(r'probe_up=(\d+)',ce); pup=int(m.group(1)) if m else -1
        m=re.search(r'recover_arq=(\d+)',ce); rarq=int(m.group(1)) if m else -1
        lat=[]
        for line in open(os.path.join(p,'out.jsonl')):
            j=json.loads(line)
            if 'event' in j:
                e=j['event']
                if e.get('delivered_ns') is not None: lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
        res.append((os.path.basename(p),rate,bw,pup,rarq,pct(lat,0.5),pct(lat,0.95),len(lat)))
    print(f"== {arm}  tx-rate window [{t_lo},{t_hi}) s (IP bytes/s, client→server)")
    for r in res: print("  %-6s tx=%6.0f btlbw_end=%5d pup=%2d rarq=%2d p50=%6.2f p95=%6.2f ndel=%3d"%r)
    lo=[r for r in res if r[1]<3000]; hi=[r for r in res if r[1]>=3000]
    def m(v,i): return st.median([x[i] for x in v]) if v else float('nan')
    print(f"  tx<3000: n={len(lo)} p95_med={m(lo,6):.2f} ndel_med={m(lo,7)} | tx>=3000: n={len(hi)} p95_med={m(hi,6):.2f} ndel_med={m(hi,7)}")
