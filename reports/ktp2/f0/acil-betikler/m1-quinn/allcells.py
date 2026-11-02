import json,sys,math,os,glob
B=sys.argv[1]; INF=float('inf')
def qnr(v,p):
    v=sorted(v); k=max(1,math.ceil(p*len(v))); return v[k-1]
def medinf(v):
    v=sorted(v); n=len(v)
    if n==0: return float('nan')
    if n%2: return v[n//2]
    a,b=v[n//2-1],v[n//2]
    if math.isinf(a) or math.isinf(b): return INF if math.isinf(a) else b if False else (INF if math.isinf(b) and math.isinf(a) else (a+b)/2 if not math.isinf(b) else INF)
    return (a+b)/2
def arm(cell,a):
    p=[];cf=[];st=[]
    for d in sorted(glob.glob(f"{B}/{cell}/{a}/rep*")):
        try: L=[json.loads(l) for l in open(f"{d}/out.jsonl")]
        except Exception: continue
        meta=L[0]['meta']; ev=[x['event'] for x in L[1:] if 'event' in x]
        I=meta['submit_interval_ns']/1e9; s=meta['setup_ns']/1e9
        lat=[INF if e['delivered_ns'] is None else (e['delivered_ns']-e['sent_ns'])/1e9 for e in ev]
        lat+= [INF]*(meta['msg_count']-len(lat))
        p.append(qnr(lat,.95)); cf.append(qnr([l+max(0,s-i*I) for i,l in enumerate(lat)],.95)); st.append(s)
    return medinf(p),medinf(cf),(sorted(st)[len(st)//2] if st else float('nan')),len(p)
flip=0;n=0;qlow=0;qlow_cf=0
for cell in sorted(os.listdir(B)):
    if not cell.startswith('b_r') or cell.startswith('b_r25') or not os.path.isdir(f"{B}/{cell}/quinn"): continue
    if '_l50_' in cell or '_l80_' in cell: continue
    t=arm(cell,'taban'); q=arm(cell,'quinn')
    n+=1
    lo = q[0]<t[0]; lo_cf = q[1]<t[0]
    qlow+=lo; qlow_cf+=lo_cf
    print(f"{cell:28s} taban p95 {t[0]:7.2f}  quinn {q[0]:7.2f}  quinn+hs {q[1]:7.2f}  hs_med {q[2]:5.2f}  N {t[3]}/{q[3]}  {'FLIP' if lo!=lo_cf else ''}")
print("cells",n,"quinn median p95 lower (no hs):",qlow," with hs in window:",qlow_cf)
