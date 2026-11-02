import csv, re, sys, os, statistics as st
base='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000'
for arm in ('taban','aday'):
    reps=sorted([d for d in os.listdir(f'{base}/{arm}') if d.startswith('rep')], key=lambda x:int(x[3:]))
    N=0; arbpos=[]; fin=[]; r1=0; finpos=0
    fecp=[]; pd=[]; par=[]
    for r in reps:
        p=f'{base}/{arm}/{r}'
        with open(p+'/ts.csv') as f:
            rows=list(csv.DictReader(f))
        for row in rows:
            N+=1
            a=max(int(row['confirmed_prop']),int(row['structure_prop']),int(row['spec_prop']))
            fr=int(row['final_ratio'])
            if a>0: arbpos.append(a)
            fin.append(fr)
            if fr>0: finpos+=1
            if (fr*8+500)//1000>=1: r1+=1
        e=open(p+'/c.err').read()
        m=re.search(r'fec_parity=(\d+)B',e); fecp.append(int(m.group(1)))
        m=re.search(r'fired=\[tail_probe=\S+ tail_cancel=\d+ parity=(\d+) parity_denied=(\d+)',e)
        par.append(int(m.group(1))); pd.append(int(m.group(2)))
    print(arm,'reps',len(reps),'ts_N',N,'arb>0',len(arbpos),'arb med',st.median(arbpos) if arbpos else None,'arb max',max(arbpos) if arbpos else None,'arb min',min(arbpos) if arbpos else None)
    fp=[x for x in fin if x>0]
    print('  final>0 n',len(fp),'final med(all)',st.median(fin),'final med(>0)',st.median(fp) if fp else None,'max',max(fin),'r>=1 samples',r1)
    print('  fec_parity bytes per rep',fecp,'sum',sum(fecp))
    print('  parity emitted',par,'denied',pd)
