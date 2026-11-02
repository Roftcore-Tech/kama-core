import sys, gzip, json, re, os, statistics as st, bisect
sys.path.insert(0, sys.argv[1])
from vlib import BASE, reps
def run(arm):
    out=[]; qq=[]
    for rep in reps(arm):
        d=f'{BASE}/{arm}/{rep}'
        meta=json.loads(open(d+'/out.jsonl').readline())['meta']; t0=meta['t0_unix_ns']
        port=int(re.search(r'--bind [0-9.]+:(\d+)',open(d+'/cmd.txt').read()).group(1))
        b=[0]*6
        with gzip.open(d+'/capb_cli.csv.gz','rt') as f:
            for line in f:
                if line[0]=='#': continue
                p=line.split(',',6)
                if p[1]!='o' or p[2]!='17' or int(p[5])!=port: continue
                k=int((int(p[0])-t0)/5e9)
                if 0<=k<6: b[k]+=int(p[3])+14
        out.append([x/5 for x in b])
    return [st.median([o[k] for o in out]) for k in range(6)]
for arm in ('taban','quinn'):
    print(arm,[round(x) for x in run(arm)])
