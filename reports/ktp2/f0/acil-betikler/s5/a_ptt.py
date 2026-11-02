import json,os,sys,glob,collections,bisect
root=sys.argv[1]
def q(v,p):
    if not v: return None
    v=sorted(v); return v[min(len(v)-1,int(p*(len(v)-1)+0.5))]
res=collections.defaultdict(lambda: collections.defaultdict(list))
parmax=collections.defaultdict(list)
match_fail=collections.Counter()
for rep in sorted(glob.glob(root+'/*/*/rep*')):
    parts=rep.split('/'); cell,arm=parts[-3],parts[-2]
    try:
        A=[json.loads(l) for l in open(rep+'/alice.jsonl')]
        B=[json.loads(l) for l in open(rep+'/bob.jsonl')]
    except Exception as e:
        match_fail['io']+=1; continue
    # parity emitted by alice (sender of data)
    pm=0
    for r in A:
        if r['k']=='ts':
            for s in r['s']:
                pm=max(pm,s.get('par',0))
    parmax[(cell,arm)].append(pm)
    S=[]
    for r in A:
        if r['k']=='tr':
            for t in r['r']:
                if t[0]=='S': S.append((t[4],t[2],t[3]))
    S.sort()
    Sat=[x[0] for x in S]
    sends=[r for r in A if r['k']=='send' and r.get('tr') is not None]
    u_of={}
    for s in sends:
        j=bisect.bisect_right(Sat,s['tr']+0.001)-1
        if j<0 or s['tr']-Sat[j]>50: match_fail['nomatch']+=1; continue
        u_of[(s['c'],s.get('sp'),s['i'])]=(S[j][1],s['t'])
    D={};Rev=[]
    for r in B:
        if r['k']=='tr':
            for t in r['r']:
                if t[0]=='D': D.setdefault(t[2],t[4])
                elif t[0]=='R': Rev.append((t[4],t[2]))
    Rev.sort()
    # release time of unit u: first R with next>u
    Rn=[];best=-1;Rt=[]
    for t,n in Rev:
        if n>best: best=n; Rn.append(n); Rt.append(t)
    rx={}
    for r in B:
        if r['k']=='rx' and r.get('first') and 'i' in r:
            rx.setdefault((r['c'],r.get('sp'),r['i']),r['t'])
    for key,(u,ts) in u_of.items():
        c=key[0]
        if c not in ('P','T','O','C','S'): continue
        if u not in D: continue
        j=bisect.bisect_right(Rn,u)
        if j>=len(Rn): continue
        rel=Rt[j]; d=D[u]
        res[(cell,arm)][c+'_hold'].append((rel-d)/1000)
        if key in rx: res[(cell,arm)][c+'_e2e'].append((rx[key]-ts)/1000); res[(cell,arm)][c+'_d'].append((d-ts)/1000)
print('match_fail',dict(match_fail))
for k in sorted(res):
    v=res[k]; pm=parmax[k]
    out=[k[0],k[1],'par_max=%s'%max(pm),'nrep=%d'%len(pm)]
    for c in ('P',):
        h=v.get(c+'_hold',[]); e=v.get(c+'_e2e',[]); d=v.get(c+'_d',[])
        if h:
            frac=sum(1 for x in h if x>0.05)/len(h)
            out.append('%s n=%d hold>50ms=%.2f hold p50=%.2f p95=%.2f | send->D p50=%.2f p95=%.2f | e2e p50=%.2f p95=%.2f'%(c,len(h),frac,q(h,.5),q(h,.95),q(d,.5) or -1,q(d,.95) or -1,q(e,.5) or -1,q(e,.95) or -1))
    print(' '.join(out))
