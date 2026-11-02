import sys, os, re, math, statistics, pickle
sys.argv=[None]+sys.argv[1:]
exec(open('recompute.py').read().split('out={}')[0])
def med(v):
    v=[x for x in v if x is not None]
    return statistics.median(v) if v else None
def F(x): return 'None' if x is None else f'{x:.3f}'
res={}
for c in CELLS:
    cd=os.path.join(ROOT,c)
    for arm in ARMS:
        ad=os.path.join(cd,arm)
        if not os.path.isdir(ad): continue
        rs=[]
        for rep in sorted(os.listdir(ad), key=lambda s:int(s[3:])):
            cmd=open(os.path.join(ad,rep,'cmd.txt')).read()
            D=float(re.search(r'--max-deadline-s (\d+)',cmd).group(1))
            r=run(os.path.join(ad,rep),D)
            if r: r['rep']=int(rep[3:]); r['D']=D; rs.append(r)
            else: rs.append(dict(rep=int(rep[3:]),D=D,missing=True))
        res[(c,arm)]=rs
pickle.dump(res,open('all.pkl','wb'))
for (c,arm),rs in res.items():
    ok=[r for r in rs if not r.get('missing')]
    D=rs[0]['D']
    nohs=sum(1 for r in ok if not r['t0'])
    part=sum(1 for r in ok if r['owD']<1)
    cens=med([min(r['ow_nr95'],D) for r in ok]+[D for r in rs if r.get('missing')])
    bsty=med([r['ow_lin95'] for r in ok])
    print(f"{c:28s} {arm:6s} N={len(rs)} miss={len(rs)-len(ok)} nohs={nohs} owD<1={part} owp95_cens={F(cens)} owp95_Bstyle={F(bsty)} confp95_cens={F(med([min(r['conf_nr95'],D) for r in ok]))} owD={F(med([r['owD'] for r in ok]))}")
