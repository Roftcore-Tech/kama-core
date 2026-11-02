import csv, sys, glob, os, statistics as st
for cell in sys.argv[1:]:
    fr=[]
    for f in glob.glob(os.path.join(cell,'taban','rep*','ts.csv')):
        r=list(csv.DictReader(open(f)))
        # only first 30 s (submission window)
        rr=[x for x in r if int(x['t_ms'])<=30000]
        if not rr: continue
        fr.append(sum(1 for x in rr if int(x['pending_lost'])>0)/len(rr))
    print(os.path.basename(cell),'N',len(fr),'median frac ticks pending_lost>0 (t<=30s) %.2f'%st.median(fr), 'min %.2f max %.2f'%(min(fr),max(fr)))
