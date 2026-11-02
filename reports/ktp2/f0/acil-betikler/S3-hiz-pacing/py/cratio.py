import sys, gzip, os, glob, re, statistics as st
celldir=sys.argv[1]; W=float(sys.argv[2])
for arm in ['taban','aday']:
    cs=[]
    for p in sorted(glob.glob(os.path.join(celldir,arm,'rep*'))):
        ce=open(os.path.join(p,'c.err')).read()
        m=re.search(r'btlbw=(\d+)B/s',ce); 
        if not m: continue
        bw=int(m.group(1))
        m2=re.search(r'inflight=(\d+)B',ce); infl=int(m2.group(1)) if m2 else 0
        rows=[]
        for line in gzip.open(os.path.join(p,'capb_cli.csv.gz'),'rt'):
            parts=line.strip().split(',')
            if len(parts)<4 or parts[1]!='o': continue
            rows.append((int(parts[0]),int(parts[3])))
        if not rows: continue
        t0=rows[0][0]
        data=[(t,l) for t,l in rows if l>500]
        if not data: continue
        tend=data[-1][0]
        dur=(tend-t0)/1e9
        if dur<45: continue
        b=sum(l-28 for t,l in rows if tend-W*1e9<=t<=tend)
        S=b/W
        cs.append(bw/S)
        print(f"{arm} {os.path.basename(p):6s} dur={dur:5.1f}s btlbw_end={bw:5d} S_udp(last {W:.0f}s)={S:6.0f} ratio={bw/S:.2f}")
    if cs: print(arm,"median btlbw/S",round(st.median(cs),3),"n",len(cs))
