import csv,random,math,statistics as st,re
rows=list(csv.DictReader(open('/home/ryuzaki/kama-k4g-work/wt/ktp2-f0/reports/ktp2/f0/d/tam/kosular.csv')))
cells=sorted(set(r['cell'] for r in rows if re.match(r'^b_r(1k|12k)_d(300|1000)_l(0|10|20|30)_p(65|1000)$',r['cell'])))
print('cells',len(cells))
def vals(cell,arm,cap):
    out=[]
    for r in rows:
        if r['cell']==cell and r['arm']==arm and r['class'] in('ok',):
            v=float(r['lat_p95_s']); D=float(r['deadline_s'])
            out.append(min(v,D) if cap else v)
    return out
def q(v,p):
    v=sorted(v); k=(len(v)-1)*p; f=math.floor(k); c=math.ceil(k)
    return v[f]+(v[c]-v[f])*(k-f) if not (math.isinf(v[f]) or math.isinf(v[c])) else (v[f] if k==f else v[c])
def run(a1,a2,B,alpha,cap,seed=1):
    rnd=random.Random(seed); K=R=E=0; pt=0
    for c in cells:
        x=vals(c,a1,cap); y=vals(c,a2,cap)
        if not x or not y: continue
        if st.median(x)>st.median(y): pt+=1
        diffs=[]
        for _ in range(B):
            mx=st.median([rnd.choice(x) for _ in x]); my=st.median([rnd.choice(y) for _ in y])
            d=mx-my
            diffs.append(d if not math.isnan(d) else 0.0)
        lo=q(diffs,alpha/2); hi=q(diffs,1-alpha/2)
        if hi<0: K+=1
        elif lo>0: R+=1
        else: E+=1
    return K,R,E,pt
for cap in ():
    for a1 in ('taban','aday'):
        print('cap',cap,a1,'-quinn  95%% B=2000 K/R/= %s'%(run(a1,'quinn',2000,0.05,cap),))
        print('cap',cap,a1,'-quinn  Bonf B=10000 K/R/= %s'%(run(a1,'quinn',10000,0.05/32,cap),))
print('--- borderline')
for seed in (1,2,3):
    rnd=random.Random(seed)
    for c in cells:
        x=vals(c,'taban',True); y=vals(c,'quinn',True)
        diffs=sorted(st.median([rnd.choice(x) for _ in x])-st.median([rnd.choice(y) for _ in y]) for _ in range(4000))
        lo=diffs[int(0.025*4000)]; hi=diffs[int(0.975*4000)-1]
        if abs(lo)<0.15 or abs(hi)<0.15: print(seed,c,'%.3f %.3f'%(lo,hi), 'med diff %.3f'%(st.median(x)-st.median(y)))
