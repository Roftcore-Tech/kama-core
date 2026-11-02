import sys, re, csv, glob, collections
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
P1 = re.compile(r'^b_r(12k|1k)_d(300|1000)_l(10|20|30)_p(65|1000)$')
N=0; prop=0; f63=0; fpos=0; clamp=0; wz=0; nruns=0
fin_hist=collections.Counter()
for cell, arm, rep, p in runs('b'):
    if not P1.match(cell) or arm not in ('taban','aday'): continue
    nruns+=1
    d=parse_cerr(p)
    if d.get('fec2'): clamp+=d['fec2'][1]; wz+=d['fec2'][2]
    ts=p[:-5]+'ts.csv'
    with open(ts) as f:
        r=csv.reader(f); h=next(r)
        ic=h.index('confirmed_prop'); is_=h.index('structure_prop'); isp=h.index('spec_prop'); ifn=h.index('final_ratio')
        for row in r:
            if len(row)<len(h): continue
            N+=1
            pr=max(int(row[ic]),int(row[is_]),int(row[isp]))
            fn=int(row[ifn])
            if pr>0: prop+=1
            if fn>0: fpos+=1
            if fn>=63: f63+=1
            fin_hist[min(fn,300)//10*10]+=1
print('runs',nruns,'N',N,'prop>0',prop,prop/N,'final>0',fpos,'final>=63',f63,f63/N,f63/prop,'clamp',clamp,'would_zero',wz)
print(sorted(fin_hist.items())[:40])
