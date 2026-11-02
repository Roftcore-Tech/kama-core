import json, glob, statistics, collections
R='/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam/a/out'
for cell in ['a_r12k_d300_l30_dosyali','a_r12k_d300_l30_dosyasiz','a_r1k_d300_l30_dosyali','a_r1k_d300_l30_dosyasiz','a_r12k_d1000_l30_dosyasiz']:
    npar=0; n=0; peaks=[]
    for f in sorted(glob.glob(f'{R}/{cell}/*/rep*/alice.jsonl')):
        n+=1; mxp=0; mxbw=0
        for l in open(f):
            if '"k":"ts"' not in l: continue
            d=json.loads(l)
            for s in d['s']:
                mxp=max(mxp, s.get('par',0) or 0); mxbw=max(mxbw, s.get('bw',0) or 0)
        npar+= mxp>0; peaks.append(mxbw)
    print(cell, 'runs',n,'with par>0',npar,'bw peak p50', statistics.median(peaks) if peaks else None, 'median_high', statistics.median_high(peaks) if peaks else None)
