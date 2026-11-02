import sys, gzip, os, glob, statistics as st
root=sys.argv[1]; cells=sys.argv[2].split(','); binsec=float(sys.argv[3]); nb=int(sys.argv[4]); A=float(sys.argv[5])
for cell in cells:
    for arm in ['taban','aday','quinn']:
        per=[[] for _ in range(nb)]
        for p in sorted(glob.glob(os.path.join(root,cell,arm,'rep*'))):
            f=os.path.join(p,'capb_cli.csv.gz')
            if not os.path.exists(f): continue
            rows=[]
            for line in gzip.open(f,'rt'):
                parts=line.strip().split(',')
                if len(parts)<4: continue
                rows.append((int(parts[0]),parts[1],int(parts[3])))
            if not rows: continue
            t0=rows[0][0]; bins=[0]*nb
            for t,d,ln in rows:
                if d!='o': continue
                b=int((t-t0)/1e9//binsec)
                if b<nb: bins[b]+=ln
            for i in range(nb): per[i].append(bins[i]/binsec/A)
        print(f"{cell:24s} {arm:6s} S/A median per {binsec:.0f}s bin: "+' '.join(f"{st.median(v):4.2f}" for v in per))
