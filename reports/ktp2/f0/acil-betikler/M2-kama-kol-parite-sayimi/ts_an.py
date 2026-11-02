import csv, os, sys, collections, statistics, gzip
ROOT=sys.argv[1]
rows=list(csv.DictReader(open('runs.csv')))
out=[]
for r in rows:
    if r['manifest_fec']!='ON adaptive p0x05': continue
    p=os.path.join(ROOT, r['lane'],'out',r['cell'],r['arm'],r['rep'],'ts.csv')
    if not os.path.exists(p): continue
    n=0; prop=0; fin=0; fin_r0=0; fin_r1=0; clampdown=0; maxprop=0; maxfin=0
    reasons=collections.Counter()
    for t in csv.DictReader(open(p)):
        n+=1
        pre=max(int(t['confirmed_prop']),int(t['structure_prop']),int(t['spec_prop']))
        f=int(t['final_ratio'])
        maxprop=max(maxprop,pre); maxfin=max(maxfin,f)
        if pre>0: prop+=1
        if f>0:
            fin+=1
            rr=(f*8+500)//1000
            if rr==0: fin_r0+=1
            else: fin_r1+=1
        if pre>0 and f<pre: clampdown+=1
        reasons[t['reason_idx']]+=1
    out.append(dict(lane=r['lane'],cell=r['cell'],arm=r['arm'],rep=r['rep'],n=n,prop=prop,fin=fin,fin_r0=fin_r0,fin_r1=fin_r1,clampdown=clampdown,maxprop=maxprop,maxfin=maxfin,parity=r['parity_fired'],btlbw=r['btlbw']))
w=csv.DictWriter(open('ts_summary.csv','w',newline=''),fieldnames=list(out[0].keys())); w.writeheader(); [w.writerow(x) for x in out]
agg=collections.OrderedDict()
for x in out:
    k=(x['lane'],x['cell'],x['arm'])
    a=agg.setdefault(k,collections.Counter())
    a['runs']+=1
    for f in ['n','prop','fin','fin_r0','fin_r1','clampdown']: a[f]+=x[f]
    a['runs_prop']+= x['prop']>0
    a['runs_fin_r1']+= x['fin_r1']>0
    a['maxfin_max']=max(a['maxfin_max'],x['maxfin'])
    a['runs_parity']+= int(x['parity'] or 0)>0
print(len(out))
print('%-28s %-7s %4s %7s %7s %7s %7s %7s %5s %5s %5s'%('cell','arm','runs','samp','prop>0','fin>0','fin_r0','fin_r1','rProp','rR1','rPar'))
for k,a in agg.items():
    print('%-28s %-7s %4d %7d %7d %7d %7d %7d %5d %5d %5d maxfin=%d'%(k[1],k[2],a['runs'],a['n'],a['prop'],a['fin'],a['fin_r0'],a['fin_r1'],a['runs_prop'],a['runs_fin_r1'],a['runs_parity'],a['maxfin_max']))
