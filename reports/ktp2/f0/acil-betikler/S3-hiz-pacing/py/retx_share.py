import sys, gzip, os, glob, statistics as st
celldir=sys.argv[1]; arm=sys.argv[2]; t_lo=float(sys.argv[3]); t_hi=float(sys.argv[4]); ival=float(sys.argv[5])
shares=[]; lag_end=[]; firsts=[]
for p in sorted(glob.glob(os.path.join(celldir,arm,'rep*')), key=lambda s:int(s.rsplit('rep',1)[1])):
    f=os.path.join(p,'capb_cli.csv.gz')
    if not os.path.exists(f): continue
    seen=set(); n1=0; nr=0; t0=None; firstsend={}
    for line in gzip.open(f,'rt'):
        parts=line.strip().split(',')
        if len(parts)<7 or parts[1]!='o' or parts[2]!='17': continue
        t=int(parts[0]); 
        if t0 is None: t0=t
        x=parts[6]
        h=x.split(':')[0]
        try: b=bytes.fromhex(h)
        except: continue
        if len(b)<11 or b[0]!=0x00: continue
        u=int.from_bytes(b[3:11],'big')
        ts=(t-t0)/1e9
        if u not in seen:
            seen.add(u); firstsend[u]=ts
            if t_lo<=ts<t_hi: n1+=1
        else:
            if t_lo<=ts<t_hi: nr+=1
    if n1+nr: shares.append(nr/(n1+nr))
    # sender lag: for unit index i (sorted ids), first send time minus nominal submit time i*ival
    ids=sorted(firstsend)
    if ids:
        base=ids[0]
        lags=[firstsend[u]-(u-base)*ival for u in ids]
        lag_end.append(max(lags)); firsts.append(st.median(lags))
    print(f"{os.path.basename(p):6s} first={n1:3d} retx={nr:3d} share={nr/max(1,n1+nr):.3f} sender_lag_max={max(lags) if ids else float('nan'):6.2f}s units={len(ids)}")
print("median retx share",st.median(shares),"median max sender lag",st.median(lag_end))
