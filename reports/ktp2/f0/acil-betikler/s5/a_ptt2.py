import json,sys,glob,collections,bisect
root=sys.argv[1]
def q(v,p):
    if not v: return float('nan')
    v=sorted(v); return v[min(len(v)-1,int(p*(len(v)-1)+0.5))]
agg=collections.defaultdict(lambda: collections.defaultdict(list))
nm=collections.Counter()
for rep in sorted(glob.glob(root+'/*/*/rep*')):
    parts=rep.split('/'); cell,arm=parts[-3],parts[-2]
    A=[json.loads(l) for l in open(rep+'/alice.jsonl')]
    B=[json.loads(l) for l in open(rep+'/bob.jsonl')]
    S=sorted((t[4],t[2],t[3]) for r in A if r['k']=='tr' for t in r['r'] if t[0]=='S')
    Sat=[x[0] for x in S]
    cls={}  # unit -> class
    for s in (r for r in A if r['k']=='send' and r.get('tr') is not None):
        j=bisect.bisect_right(Sat,s['tr']+0.001)-1
        if j<0 or s['tr']-Sat[j]>50: nm[s['c']]+=1; continue
        cls[S[j][1]]=(s['c'],s.get('sp'))
    D={}
    for r in B:
        if r['k']=='tr':
            for t in r['r']:
                if t[0]=='D': D.setdefault(t[2],t[4])
    Rev=sorted((t[4],t[2]) for r in B if r['k']=='tr' for t in r['r'] if t[0]=='R')
    Rn=[];Rt=[];best=-1
    for t,n in Rev:
        if n>best: best=n; Rn.append(n); Rt.append(t)
    # per-spurt stream ordering counterfactual
    spurt_units=collections.defaultdict(list)
    for u,(c,sp) in cls.items():
        if c in ('O','P','C'): spurt_units[sp].append(u)
    for sp,us in spurt_units.items():
        us.sort(); run=-1
        for u in us:
            if u not in D: continue
            run=max(run,D[u])
            j=bisect.bisect_right(Rn,u)
            if j>=len(Rn): continue
            rel=Rt[j]
            if cls[u][0]!='P': continue
            hold=(rel-D[u])/1000; hold_ps=(run-D[u])/1000
            agg[(cell,arm)]['hold'].append(hold); agg[(cell,arm)]['hold_ps'].append(hold_ps)
            # blocker class at D time
            k=bisect.bisect_right(Rt,D[u])-1
            nxt=Rn[k] if k>=0 else 0
            if nxt<u:
                bc=cls.get(nxt,('?',None))
                tag='self' if (bc[0] in 'OPC' and bc[1]==sp) else ('file/unmapped' if bc[0]=='?' else 'other:'+bc[0])
            else: tag='none'
            agg[(cell,arm)]['blk_'+tag].append(hold)
for k in sorted(agg):
    v=agg[k]; h=v['hold']; hp=v['hold_ps']; n=len(h)
    tot=sum(h)
    bl={kk[4:]:'%.0f%%'%(100*sum(vv)/tot) for kk,vv in v.items() if kk.startswith('blk_') and tot>0}
    print(k[0],k[1],'n=%d hold p50=%.2f p95=%.2f | per-spurt p50=%.2f p95=%.2f | hold-time share by blocker %s'%(n,q(h,.5),q(h,.95),q(hp,.5),q(hp,.95),bl))
print('nomatch',dict(nm))
