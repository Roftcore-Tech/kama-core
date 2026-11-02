import csv, os, re, sys, collections
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out'
pat=re.compile(r'b_r(12k|1k)_d(300|1000)_l(10|20|30)_p(65|1000)$')
N=0; prop=0; fin63=0; finpos=0; files=0
clamp=0; wz=0; clamp_all=0; wz_all=0
finhist=collections.Counter()
reasons=collections.Counter()
patc=re.compile(r'clamp=(\d+) would_zero=(\d+)')
for c in sorted(os.listdir(base)):
    if not os.path.isdir(os.path.join(base,c)): continue
    m=re.match(r'b_r(12k|1k)_d(300|1000)_l(0|10|20|30)_p(65|1000)$',c)
    if not m: continue
    lossy = m.group(3)!='0'
    for arm in ('taban','aday'):
        d=os.path.join(base,c,arm)
        for rep in os.listdir(d):
            p=os.path.join(d,rep)
            ce=open(os.path.join(p,'c.err')).read()
            mm=patc.search(ce)
            if mm:
                clamp_all+=int(mm.group(1)); wz_all+=int(mm.group(2))
                if lossy: clamp+=int(mm.group(1)); wz+=int(mm.group(2))
            if not lossy: continue
            files+=1
            with open(os.path.join(p,'ts.csv')) as fh:
                r=csv.DictReader(fh)
                for row in r:
                    N+=1
                    a=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop']))
                    f=int(row['final_ratio'])
                    if a>0: prop+=1
                    if f>=63: fin63+=1
                    if f>0: finpos+=1
                    finhist[min(f,300)//10*10 if f<300 else 300]+=1
print('files',files,'N',N,'prop>0',prop,prop/N,'fin>=63',fin63,fin63/N,fin63/prop,'fin>0',finpos)
print('clamp lossy',clamp,'wz lossy',wz,'clamp all32',clamp_all,'wz all32',wz_all)
print(sorted(finhist.items()))
