import csv, sys, os, collections, glob
root=sys.argv[1]
cells=sys.argv[2:]
for cell in cells:
  for arm in ('taban','aday','uyar_taban','uyar_aday'):
    reps=sorted(glob.glob(os.path.join(root,'*','out',cell,arm,'rep*','ts.csv')))
    if not reps: continue
    hist=collections.Counter(); n=0
    for p in reps:
        with open(p) as fh:
            rd=csv.DictReader(fh)
            for r in rd:
                fr=int(r['final_ratio']); n+=1
                if fr==0: hist['0']+=1
                elif fr<63: hist['1-62 (r=0 @K8)']+=1
                elif fr<188: hist['63-187 (r=1)']+=1
                else: hist['>=188 (r>=2)']+=1
    print(cell,arm,'reps',len(reps),'samples',n,{k:round(v/n,4) for k,v in sorted(hist.items())})
