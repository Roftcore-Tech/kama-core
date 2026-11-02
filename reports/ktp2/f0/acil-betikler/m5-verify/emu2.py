import sys, re, bisect, statistics as st
sys.path.insert(0, sys.argv[1])
from vlib import *
SIZE=1027
def emulate(R):
    base=R['t0']-R['off']
    offs=sorted(R['offer'].values())
    evs=[]
    for p in R['cli']:
        if p['dir']=='o' and p['type']=='data': evs.append((p['t'],0,p))
        elif p['dir']=='i' and p['type']=='ack' and 'acked16' in p: evs.append((p['t'],1,p))
    evs.sort(key=lambda e:(e[0],e[1]))
    dl=0; dt=0; fs=0; pif=0
    infl={}; low={}; acked=set(); lasttx={}; nsent_units=set()
    samples=[]; lastfull=0; series=[]
    for t,k,p in evs:
        if k==0:
            if pif==0: fs=t; dt=t
            pif+=1
            nsent_units.add(p['unit'])
            q_after=bisect.bisect_right(offs,t)-len(nsent_units)
            al = q_after<=0
            infl[p['pn']]=dict(t=t,dl=dl,dt=dt,fs=fs,al=al,unit=p['unit'])
            low[p['pn']&0xFFFF]=p['pn']; lasttx[p['unit']]=p['pn']
        else:
            W=max(8*(2*267/max(lastfull,1)+2.097),10.0)*1e9
            for l in sorted(p['acked16']):
                pn=low.get(l)
                if pn is None or pn in acked or pn not in infl: continue
                acked.add(pn); P=infl.pop(pn)
                if lasttx[P['unit']]!=pn and lasttx[P['unit']]<pn: pass
                superseded = lasttx[P['unit']]>pn
                dl+=SIZE; dt=t; pif=max(0,pif-1)
                if superseded: continue
                iv=max(t-P['dt'],P['t']-P['fs']); fs=P['t']
                if iv>=1e6:
                    bw=(dl-P['dl'])*1e9/iv
                    cur=max((b for _,b in samples),default=0)
                    if (not P['al']) or bw>=cur: samples.append((t,bw))
                    if not P['al']: lastfull=bw
                last=samples[-1] if samples else None
                samples=[s for s in samples if s[0]>=t-W] or ([last] if last else [])
            series.append((t,max((b for _,b in samples),default=0)))
    return series
def rank(v):
    s=sorted(range(len(v)),key=lambda i:v[i]); r=[0]*len(v)
    for k,i in enumerate(s): r[i]=k
    return r
def spear(a,b):
    ra,rb=rank(a),rank(b); ma,mb=st.mean(ra),st.mean(rb)
    num=sum((x-ma)*(y-mb) for x,y in zip(ra,rb)); den=(sum((x-ma)**2 for x in ra)*sum((y-mb)**2 for y in rb))**.5
    return num/den
for arm in sys.argv[2:]:
    rows=[]; errs=[]
    for rep in reps(arm):
        R=load(arm,rep); base=R['t0']-R['off']
        ser=emulate(R); st_t=[x[0] for x in ser]
        endbw=int(re.search(r'est=\[btlbw=(\d+)',R['err']).group(1))
        emu_end=ser[-1][1] if ser else 0
        errs.append(abs(emu_end-endbw)/endbw)
        offs=sorted(R['offer'].values())
        first={}
        data=[p for p in R['cli'] if p['dir']=='o' and p['type']=='data']
        for p in data: first.setdefault(p['unit'],p['t'])
        fts=sorted(first.values())
        q=lambda t: bisect.bisect_right(offs,t)-bisect.bisect_right(fts,t)
        dts=[p['t'] for p in data]
        ratios=[]; bws=[]; ems=[]
        for kk in range(40):
            a=R['t0']+kk*2.5e9; b=a+2.5e9
            if q(a)<3 or q(b)<3: continue
            i=bisect.bisect_right(st_t,a)-1
            if i<0 or ser[i][1]<=0: continue
            n=bisect.bisect_left(dts,b)-bisect.bisect_left(dts,a)
            em=n*SIZE/2.5; ratios.append(em/ser[i][1]); bws.append(ser[i][1]); ems.append(em)
        lat=[v if v is not None else 1e9 for v in R['lat'].values()]
        p95=pctl(lat,0.95)
        rows.append((rep, st.median(ratios) if ratios else float('nan'), p95, st.median(bws) if bws else 0, st.median(ems) if ems else 0, emu_end, endbw))
    print('==',arm,'emu end within 2%:',sum(1 for e in errs if e<=0.02),'/30; within 10%:',sum(1 for e in errs if e<=0.10))
    g=[r for r in rows if r[1]>=1.35]; b_=[r for r in rows if r[1]<1.35]
    for nm,G in (('>=1.35',g),('<1.35',b_)):
        print('  %s n=%d p95 med %.1f btlbw med %.0f emis med %.0f'%(nm,len(G),st.median([r[2] for r in G]),st.median([r[3] for r in G]),st.median([r[4] for r in G])))
    v=[r for r in rows if r[1]==r[1]]
    print('  spearman',round(spear([r[1] for r in v],[min(r[2],200) for r in v]),2),'n',len(v))
    for r in rows: print('   ',r[0],'ratio %.2f p95 %.1f emu_end %.0f eng_end %d'%(r[1],min(r[2],999),r[5],r[6]))
