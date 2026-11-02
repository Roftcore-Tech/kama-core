import csv,os,sys,collections
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
rows=list(csv.DictReader(open('runs.csv')))
out=[]
prevals=collections.Counter()
for r in rows:
    if r['fecmode']!='ON adaptive p0x05': continue
    p=os.path.join(ROOT,r['lane'],'out',r['cell'],r['arm'],r['rep'],'ts.csv')
    if not os.path.exists(p): continue
    n=pre=fin=r0=r1=0; fmin=10**9; fmax=0; finvals=collections.Counter()
    pend_int=0; maxpend=0; maxtok_pend=-1; tok_ge_pend=0; pend_samples=0; first_pend=None; last_t=0; pe_last=0
    for t in csv.DictReader(open(p)):
        n+=1
        a=max(int(t['confirmed_prop']),int(t['structure_prop']),int(t['spec_prop']))
        if a>0: pre+=1; prevals[a]+=1
        f=int(t['final_ratio'])
        if f>0:
            fin+=1; finvals[f]+=1
            fmin=min(fmin,f); fmax=max(fmax,f)
            if (f*8+500)//1000==0: r0+=1
            else: r1+=1
        pp=int(t['pending_parity'])
        if pp>0:
            pend_samples+=1
            if first_pend is None: first_pend=int(t['t_ms'])
            maxtok_pend=max(maxtok_pend,int(t['pacer_tokens']))
            if int(t['pacer_tokens'])>=1052: tok_ge_pend+=1
        pend_int+=pp
        maxpend=max(maxpend,pp)
        pe_last=int(t['parity_emitted'])
    out.append(dict(lane=r['lane'],cell=r['cell'],arm=r['arm'],rep=r['rep'],n=n,pre=pre,fin=fin,r0=r0,r1=r1,fmin=fmin if fin else '',fmax=fmax,pend_int_samples=pend_int,maxpend=maxpend,maxtok_pend=maxtok_pend,tok_ge_pend=tok_ge_pend,pend_samples=pend_samples,first_pend=first_pend,parity=r['parity'],btlbw=r['btlbw'],srtt=r['srtt'],ts_pe_last=pe_last,payload=r['payload'],interval=r['interval']))
w=csv.DictWriter(open('ts_sum.csv','w',newline=''),fieldnames=list(out[0].keys())); w.writeheader(); [w.writerow(x) for x in out]
print(len(out)); print('pre-clamp nonzero values:',dict(prevals))
print(collections.Counter((x['lane'],x['arm']) for x in out))
