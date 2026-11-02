import json, sys, re, statistics as st
rows=json.load(open(sys.argv[1]))
ad=[r for r in rows if r['has_kp'] and r['fec'] and r['fec'].startswith('ON adaptive p0x05') and r['arm'] in ('taban','uyar_taban')]
def q(xs,p):
    xs=sorted(xs); 
    if not xs: return None
    k=(len(xs)-1)*p; f=int(k); c=min(f+1,len(xs)-1)
    return xs[f]+(xs[c]-xs[f])*(k-f)
for dd in ('d300','d1000'):
    sel=[r for r in ad if re.search(r'_'+dd+r'(_|$)', r['cell']) and r['risk_t'] and r['risk_t'][0]>0 and r['risk_t'][1]>0]
    dt=[(r['risk_t'][1]-r['risk_t'][0])/1e9 for r in sel]
    ratio=[(r['risk_t'][1]-r['risk_t'][0])/1e3/r['srtt_us'] for r in sel if r['srtt_us']]
    srt=[r['srtt_us']/1e6 for r in sel if r['srtt_us']]
    print(dd,'n',len(sel),'p10 %.3f p50 %.3f p90 %.3f min %.3f'%(q(dt,.1),q(dt,.5),q(dt,.9),min(dt)),'median ratio %.2f'%q(ratio,.5),'p50dt/p50srtt %.2f'%(q(dt,.5)/q(srt,.5)), 'median srtt %.3f'%q(srt,.5), 'below 3.145:', sum(1 for x in dt if x<3.145))
    tot=[r for r in ad if re.search(r'_'+dd+r'(_|$)', r['cell'])]
    print('  total runs in class', len(tot), 'with suspected but no confirmed', sum(1 for r in tot if r['risk_t'] and r['risk_t'][0]>0 and r['risk_t'][1]==0))
