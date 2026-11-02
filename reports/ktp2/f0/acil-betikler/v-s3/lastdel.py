import json,os,statistics as st
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/b/out/b_r12k_d1000_l30_p1000'
for arm in ['taban','aday','quinn']:
    v=[]
    for r in range(1,31):
        f=os.path.join(R,arm,'rep%d'%r,'out.jsonl')
        if not os.path.exists(f): continue
        m=0
        for ln in open(f):
            j=json.loads(ln)
            e=j.get('event')
            if e and e.get('delivered_ns'): m=max(m,e['delivered_ns'])
        v.append(m/1e9)
    print(arm,len(v),'last delivered med %.1f'%st.median(v))
