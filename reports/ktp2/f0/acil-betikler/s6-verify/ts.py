import csv, glob, collections, sys
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
def scan(pattern):
    n=0; pre63=0; post1_62=0; fin63=0; finpos=0; pre_pos=0
    for f in glob.glob(pattern):
        for r in csv.DictReader(open(f)):
            n+=1
            pre=max(int(r['confirmed_prop']),int(r['structure_prop']),int(r['spec_prop']))
            fin=int(r['final_ratio'])
            if pre>=63: pre63+=1
            if pre>0: pre_pos+=1
            if 1<=fin<=62: post1_62+=1
            if fin>=63: fin63+=1
            if fin>0: finpos+=1
    return n,pre63,post1_62,fin63,finpos,pre_pos
for p in [f'{ROOT}/c/out/*/uyar_taban/rep*/ts.csv']:
    n,a,b,c,d,e=scan(p)
    print(p,n,'pre>=63 %.1f'%(100*a/n),'fin1-62 %.1f'%(100*b/n),'fin>=63 %.1f'%(100*c/n),'fin>0 %.1f'%(100*d/n),'pre>0 %.1f'%(100*e/n))
# per-cell in B: share of samples with pre>=63 that end with fin<63 (and fin>0 or 0?)
res={}
for f in glob.glob(f'{ROOT}/b/out/*/taban/rep*/ts.csv'):
    cell=f.split('/')[-4]
    st=res.setdefault(cell,[0,0,0])
    for r in csv.DictReader(open(f)):
        pre=max(int(r['confirmed_prop']),int(r['structure_prop']),int(r['spec_prop']))
        fin=int(r['final_ratio'])
        if pre>=63:
            st[0]+=1
            if fin<63: st[1]+=1
            if 1<=fin<=62: st[2]+=1
for cell in sorted(res):
    a,b,c=res[cell]
    if a: print(cell, a, 'veto%%=%.1f'%(100*b/a), 'to1-62%%=%.1f'%(100*c/a))
