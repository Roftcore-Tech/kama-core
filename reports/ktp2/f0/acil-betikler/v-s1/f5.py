import sys, re, csv
sys.path.insert(0, '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/v-s1')
from parse import runs, parse_cerr
rfx=re.compile(r'recover_fwd=(\d+)')
lpk=re.compile(r'ledger_peak=(\d+)')
out=[]
for lane in ('b',):
  for cell, arm, rep, p in runs(lane):
    if arm not in ('taban','aday'): continue
    d=parse_cerr(p)
    r=d.get('fec_repairs_tx',0)
    if r==0: continue
    t=open(p,errors='replace').read()
    rf=int(rfx.search(t).group(1)) if rfx.search(t) else -1
    lp=int(lpk.search(t).group(1))
    fe=d['flow_end']; tfp=d['risk_t'][3]
    # ts: max final and final>=63 count, min final when >0
    ts=p[:-5]+'ts.csv'; mx=0; n63=0; n250=0
    with open(ts) as f:
        rr=csv.reader(f); h=next(rr); i=h.index('final_ratio')
        for row in rr:
            if len(row)<len(h): continue
            v=int(row[i]); mx=max(mx,v); n63+= v>=63; n250+= v>=250
    out.append((cell,arm,rep,r,d['fec_blocks'],d['fec2'],d['est'][0],rf,lp,tfp>=fe, (tfp-fe)/1e9, mx,n63,n250))
inflow=[o for o in out if not o[9]]; after=[o for o in out if o[9]]
print('total adaptive runs with repairs', len(out), 'after flow_end', len(after), 'r25 after', sum(1 for o in after if 'r25' in o[0]), 'r1k after', sum(1 for o in after if 'r1k' in o[0]))
print('inflow', len(inflow), 'wz>0', sum(1 for o in inflow if o[5][2]>0))
for o in inflow:
    if o[5][2]==0: print('WZ0',o)
print('--- inflow wz>0 sample')
for o in inflow[:60]:
    if o[5][2]>0: print(o[0],o[1],o[2],'rep',o[3],'wz',o[5][2],'bw',o[6],'rf',o[7],'mx',o[11],'n63',o[12],'n250',o[13])
