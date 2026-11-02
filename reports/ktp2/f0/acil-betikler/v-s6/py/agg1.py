import json, sys, collections
rows=json.load(open(sys.argv[1]))
C=collections.Counter
print('fec variants', C((r['fec'],r['profile']) for r in rows if r['has_kp']).most_common())
ad=[r for r in rows if r['has_kp'] and r['fec'] and r['fec'].startswith('ON adaptive p0x05')]
print('adaptive runs',len(ad))
par=[r for r in ad if (r['fec_repairs_tx'] or 0)>0]
print('with repairs',len(par),'sum repairs',sum(r['fec_repairs_tx'] for r in par))
wz=[r for r in par if (r['would_zero'] or 0)>0]
print('par&wz',len(wz), 'by lane', C(r['lane'] for r in wz), 'par by lane', C(r['lane'] for r in par))
print('par by cell top', C(r['cell'] for r in wz).most_common(8))
pb=[r for r in par if r['fec_blocks']==r['partial_blocks']]
print('par fec_blocks==partial', len(pb), C(r['lane'] for r in pb))
print('par fec_blocks>partial', len([r for r in par if r['fec_blocks']>r['partial_blocks']]))
print('preconv>0', len([r for r in ad if (r['preconv_blocked'] or 0)>0]))
print('arms of adaptive', C(r['arm'] for r in ad))
uy=[r for r in ad if r['arm'] in ('uyar_taban','uyar_aday')]
print('uyar runs',len(uy),'with par',len([r for r in uy if r['fec_repairs_tx']>0]),'sum',sum(r['fec_repairs_tx'] for r in uy), 'wz among par', len([r for r in uy if r['fec_repairs_tx']>0 and r['would_zero']>0]))
x=[r for r in ad if r['cell']=='b_r12k_d1000_l30_p1000']
print('b_r12k_d1000_l30_p1000', len(x), 'btlbw range', min(r['btlbw'] for r in x), max(r['btlbw'] for r in x), 'clamp>0', sum(1 for r in x if r['clamp']>0), 'sum par', sum(r['fec_repairs_tx'] for r in x), C(r['arm'] for r in x))
print('deadline values', C(r['deadline'] for r in ad).most_common(5))
print('fusion nonzero', len([r for r in ad if r['fusion'] and any(r['fusion'])]))
pol=[r for r in rows if r['arm']=='pol_taban']
print('pol_taban', len(pol), C(r['fec'] for r in pol), 'clamp>0', sum(1 for r in pol if (r['clamp'] or 0)>0), 'rep>0', sum(1 for r in pol if (r['fec_repairs_tx'] or 0)>0), C(r['deadline'] for r in pol).most_common(3))
