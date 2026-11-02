import sys
sys.path.insert(0, sys.argv[1])
from vlib import *
for rep in ('rep8','rep12','rep14'):
    R=load('taban',rep); base=R['t0']-R['off']
    print('==',rep, [l for l in R['err'].split('\n') if 'fired=' in l][0].split('reach')[0][-160:])
    aout=[p for p in R['cli'] if p['dir']=='o']
    bin_pn={p['pn']:p['t'] for p in R['srv'] if p['dir']=='i'}
    for p in aout:
        tt=(p['t']-base)/1e9
        if tt>32: break
        print('  o %.3f pn=%d %s unit=%s udp=%d arrived=%s'%(tt,p['pn'],p['type'],p.get('unit'),p['udp'],p['pn'] in bin_pn))
    ain=[p for p in R['cli'] if p['dir']=='i' and p['type']=='ack']
    for p in ain:
        tt=(p['t']-base)/1e9
        if tt>32: break
        print('  i-ack %.3f pn=%d acked16=%s'%(tt,p['pn'],sorted(p.get('acked16',()))))
