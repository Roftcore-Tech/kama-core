import os, re, sys, glob, statistics, collections
root=sys.argv[1]
rx=re.compile(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)")
rs=re.compile(r"srtt=(\d+)us")
gaps=collections.defaultdict(list); ratios=collections.defaultdict(list)
for p in glob.glob(os.path.join(root,'*','out','*','*','rep*','c.err')):
    arm=p.split(os.sep)[-3]
    if arm not in ('taban','uyar_taban'): continue
    t=open(p,errors='replace').read()
    if 'adaptive p0x05' not in t: continue
    m=rx.search(t); s=rs.search(t)
    if not m or not s: continue
    ts,tc=int(m.group(1)),int(m.group(2))
    srtt=int(s.group(1))*1000
    cell=p.split(os.sep)[-4]
    d='d1000' if 'd1000' in cell else ('d300' if 'd300' in cell else 'other')
    if ts>0 and tc>0 and tc>=ts:
        gaps[d].append((tc-ts)/1e9)
        if srtt>0: ratios[d].append((tc-ts)/srtt)
for d in gaps:
    g=sorted(gaps[d]); r=sorted(ratios[d])
    print(d,'n',len(g),'t_conf-t_susp s: p50',round(statistics.median(g),2),'p10',round(g[len(g)//10],2),'p90',round(g[9*len(g)//10],2),'| /srtt p50',round(statistics.median(r),2))
