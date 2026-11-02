import json, sys, os, statistics as st, glob
def pct(xs, q):
    xs = sorted(xs)
    if not xs: return None
    # nearest-rank
    import math
    k = max(1, math.ceil(q*len(xs)))
    return xs[k-1]
def pct_lin(xs,q):
    xs=sorted(xs); n=len(xs)
    if n==0: return None
    pos=q*(n-1); lo=int(pos); hi=min(lo+1,n-1)
    return xs[lo]+(xs[hi]-xs[lo])*(pos-lo)
cell = sys.argv[1]
offered = None
for arm in ['taban','aday','quinn','tcp']:
    rows=[]
    for rep in sorted(glob.glob(os.path.join(cell,arm,'rep*'))):
        f=os.path.join(rep,'out.jsonl')
        if not os.path.exists(f): continue
        meta=None; lat=[]; n=0
        with open(f) as fh:
            for line in fh:
                o=json.loads(line)
                if 'meta' in o: meta=o['meta']
                elif 'event' in o:
                    e=o['event']; n+=1
                    if e.get('delivered_ns') is not None:
                        lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
        act=(meta or {}).get('activation') or {}
        tx=(meta or {}).get('tx') or {}
        rows.append(dict(rep=os.path.basename(rep), n=n, ndel=len(lat), p95n=pct(lat,0.95) if lat else None, p95l=pct_lin(lat,0.95) if lat else None,
            btl=act.get('btlbw_bps'), data=tx.get('data'), retx=tx.get('retx'), fec=tx.get('fec'), par=act.get('parity_emitted'), lp=act.get('left_preconverge_ns')))
    if not rows: continue
    p95s=[r['p95n'] for r in rows if r['p95n'] is not None]
    p95l=[r['p95l'] for r in rows if r['p95l'] is not None]
    btl=[r['btl'] for r in rows if r['btl'] is not None]
    print(arm, 'N',len(rows),'ndel med',st.median([r['ndel'] for r in rows]),'p95 nearest med %.2f lin med %.2f'%(st.median(p95s),st.median(p95l)))
    if btl:
        print('  btl med',st.median(btl),'min',min(btl),'max',max(btl))
        d=sum(r['data'] or 0 for r in rows); rx=sum(r['retx'] or 0 for r in rows)
        print('  retx/data %.3f fec sum %s parity sum %s'%(rx/d if d else 0, sum(r['fec'] or 0 for r in rows), sum(r['par'] or 0 for r in rows)))
        for thr in [3751,5359]:
            print('  btl < %d: %d'%(thr, sum(1 for b in btl if b<thr)))
        lo=[r['p95n'] for r in rows if r['btl'] is not None and r['btl']<3751 and r['p95n'] is not None]
        hi=[r['p95n'] for r in rows if r['btl'] is not None and r['btl']>=3751 and r['p95n'] is not None]
        if lo: print('  btl<3751 n=%d p95med %.2f'%(len(lo),st.median(lo)))
        if hi: print('  btl>=3751 n=%d p95med %.2f'%(len(hi),st.median(hi)))
