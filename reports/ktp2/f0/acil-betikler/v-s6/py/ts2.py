import os, csv, collections
ROOT='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
for cell in sorted(os.listdir(os.path.join(ROOT,'b','out'))):
    if not cell.startswith('b_r25') : continue
    cp=os.path.join(ROOT,'b','out',cell)
    if not os.path.isdir(cp): continue
    c=collections.Counter()
    for arm in ('taban','aday'):
        d=os.path.join(cp,arm)
        if not os.path.isdir(d): continue
        for rep in os.listdir(d):
            p=os.path.join(d,rep,'ts.csv')
            if not os.path.isfile(p): continue
            for row in csv.DictReader(open(p)):
                pre=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop'])); fin=int(row['final_ratio'])
                if pre>=63:
                    c['pre63']+=1
                    if fin==0: c['fin0']+=1
                    elif fin<63: c['fin1_62']+=1
                    else: c['fin63']+=1
    print(cell, dict(c))
