import os, csv, sys, collections, re
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
def scan(paths):
    n=pre63=fin_low=fin63=veto=0
    for p in paths:
        with open(p) as f:
            rd=csv.DictReader(f)
            for row in rd:
                try:
                    pre=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop'])); fin=int(row['final_ratio'])
                except Exception: continue
                n+=1
                if pre>=63: pre63+=1
                if 1<=fin<=62: fin_low+=1
                if fin>=63: fin63+=1
                if pre>=63 and fin<63: veto+=1
    return n,pre63,fin_low,fin63,veto
# uyar_taban
ut=[]
for cell in os.listdir(os.path.join(ROOT,'c','out')):
    d=os.path.join(ROOT,'c','out',cell,'uyar_taban')
    if os.path.isdir(d):
        for rep in os.listdir(d):
            p=os.path.join(d,rep,'ts.csv')
            if os.path.isfile(p): ut.append(p)
n,pre63,fl,f63,veto=scan(ut)
print('uyar_taban files',len(ut),'n',n,'pre>=63 %.1f'%(100*pre63/n),'fin 1-62 %.1f'%(100*fl/n),'fin>=63 %.1f'%(100*f63/n))
# B cells by rate class
res=collections.defaultdict(lambda:[0,0,0,0,0])
for cell in sorted(os.listdir(os.path.join(ROOT,'b','out'))):
    cp=os.path.join(ROOT,'b','out',cell)
    if not os.path.isdir(cp): continue
    for arm in ('taban','aday'):
        d=os.path.join(cp,arm)
        if not os.path.isdir(d): continue
        ps=[os.path.join(d,rep,'ts.csv') for rep in os.listdir(d) if os.path.isfile(os.path.join(d,rep,'ts.csv'))]
        r=scan(ps)
        for i in range(5): res[cell][i]+=r[i]
for cell,(n,pre63,fl,f63,veto) in sorted(res.items()):
    if n==0: continue
    print(cell,'n',n,'pre63',pre63,'vetoed %% of pre63: %.1f'%(100*veto/pre63 if pre63 else float('nan')))
