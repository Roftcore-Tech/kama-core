import csv,os,statistics as st,re
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
peaks=[];n=0;missing=0
for c in sorted(os.listdir(R)):
    if not c.startswith('b_r12k') or c.endswith('.log'): continue
    for arm in ['taban','aday']:
        base=os.path.join(R,c,arm)
        if not os.path.isdir(base): continue
        for rep in os.listdir(base):
            f=os.path.join(base,rep,'ts.csv')
            if not os.path.exists(f): missing+=1; continue
            n+=1; m=0
            with open(f) as fh:
                rd=csv.DictReader(fh)
                for r in rd:
                    try: v=int(r['final_ratio'])
                    except: continue
                    if v>m: m=v
            peaks.append(m)
print('runs',n,'missing',missing,'median peak',st.median(peaks),'frac peak>=63',sum(1 for x in peaks if x>=63)/len(peaks))
ce=open(os.path.join(R,'b_r12k_d1000_l30_p1000/taban/rep10/c.err')).read()
print(re.findall(r'btlbw=\d+',ce))
