import sys, json
sys.path.insert(0, sys.argv[1])
from vlib import *
import statistics as st
for arm in ('taban','aday'):
    res=[]
    for rep in reps(arm):
        d=f'{BASE}/{arm}/{rep}'
        L=[json.loads(l) for l in open(d+'/out.jsonl')]
        ev=[l['event'] for l in L if 'event' in l]
        lat=[(e['delivered_ns']-e['sent_ns'])/1e9 for e in ev if e.get('delivered_ns') is not None]
        n=len(ev)
        rx=sum(1 for l in open(d+'/rx_log.jsonl') if '"u"' in l)
        # p95 delivered-only, p95 with undelivered=60s
        a=pctl(lat,0.95)
        b=pctl(lat+[60.0]*(n-len(lat)),0.95)
        res.append((rep,len(lat),rx,a,b))
    print(arm)
    for r in res:
        if r[0] in ('rep8','rep12','rep14','rep7','rep1'): print('  ',r)
    print('  median p95 delivered-only',st.median([r[3] for r in res]),' median p95 undelivered=60',st.median([r[4] for r in res]))
