import json, sys, collections
rows=json.load(open(sys.argv[1]))
ad=[r for r in rows if r['has_kp'] and r['fec'] and r['fec'].startswith('ON adaptive p0x05')]
par=[r for r in ad if (r['fec_repairs_tx'] or 0)>0 and (r['would_zero'] or 0)==0]
for r in par:
    print(r['lane'],r['cell'],r['arm'],r['rep'],'rep',r['fec_repairs_tx'],'blk',r['fec_blocks'],'part',r['partial_blocks'],'btlbw',r['btlbw'],'clamp',r['clamp'],'arb',r['arb'],'recfwd',r['recover_fwd'])
