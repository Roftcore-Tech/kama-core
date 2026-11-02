import json,sys,math,statistics as st
R=sys.argv[1]; INF=float('inf')
def qnr(v,p):
    v=sorted(v); k=max(1,math.ceil(p*len(v))); return v[k-1]
def medinf(v):
    v=sorted(v); n=len(v); a,b=v[n//2-1],v[n//2]
    return a if n%2==0 and a==b else ((a+b)/2 if n%2==0 else v[n//2])
for arm in ('taban','aday','quinn'):
    p_all=[];p_ss=[];p_cs=[]
    for rep in range(1,31):
        L=[json.loads(l) for l in open(f"{R}/{arm}/rep{rep}/out.jsonl")]
        ev=[x['event'] for x in L[1:] if 'event' in x]
        lat=[INF if e['delivered_ns'] is None else (e['delivered_ns']-e['sent_ns'])/1e9 for e in ev]
        p_all.append(qnr(lat,.95)); p_ss.append(qnr(lat[20:],.95)); p_cs.append(qnr(lat[:20],.95))
    print(arm,"p95 all msgs %.2f | msgs 20-99 %.2f | msgs 0-19 %.2f"%(medinf(p_all),medinf(p_ss),medinf(p_cs)))
