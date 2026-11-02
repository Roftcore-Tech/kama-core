import csv, sys, os, collections, glob
root=sys.argv[1]
tot=collections.Counter()
percell={}
for p in glob.glob(os.path.join(root,'*','out','*','*','rep*','ts.csv')):
    parts=p.split(os.sep)
    arm=parts[-3]; cell=parts[-4]
    if arm not in ('taban','aday','uyar_taban','uyar_aday'): continue
    c=percell.setdefault((cell,arm),collections.Counter())
    with open(p) as fh:
        for r in csv.DictReader(fh):
            pre=max(int(r['confirmed_prop']),int(r['structure_prop']),int(r['spec_prop']))
            fr=int(r['final_ratio'])
            c['n']+=1
            if pre>=63: c['pre>=63']+=1
            if pre>=63 and 0<fr<63: c['vetoed']+=1
            if fr>=63: c['final>=63']+=1
            if pre>0 and fr==0: c['pre>0_final0']+=1
for k,c in percell.items():
    for kk,v in c.items(): tot[(k[1],kk)]+=v
for arm in ('taban','aday','uyar_taban','uyar_aday'):
    n=tot[(arm,'n')]
    if n==0: continue
    print(arm,'samples',n,'pre>=63',round(tot[(arm,'pre>=63')]/n,4),'vetoed(pre>=63,final 1..62)',round(tot[(arm,'vetoed')]/n,4),'final>=63',round(tot[(arm,'final>=63')]/n,4),'pre>0&final0',round(tot[(arm,'pre>0_final0')]/n,4))
# by cell for taban: share of pre>=63 samples that got vetoed
rows=[]
for (cell,arm),c in percell.items():
    if arm!='taban' or c['pre>=63']==0: continue
    rows.append((cell,c['pre>=63'],round(c['vetoed']/c['pre>=63'],3),round(c['final>=63']/max(1,c['pre>=63']),3)))
rows.sort()
for r in rows: print(*r)
