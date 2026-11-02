import json, sys, collections
rows=json.load(open(sys.argv[1]))
C=collections.Counter
ad=[r for r in rows if r['has_kp'] and r['fec'] and r['fec'].startswith('ON adaptive p0x05')]
print('suppress', C(r['suppress'] for r in ad).most_common())
lat=[0]*5
for r in ad:
    if r['latch']:
        for i in range(5): lat[i]+=r['latch'][i]
print('latch sums (no_elig, mismatch, no_rule, enq, emit)', lat)
print('runs with mismatch>0', sum(1 for r in ad if r['latch'] and r['latch'][1]>0))
print('runs with no_rule>0', sum(1 for r in ad if r['latch'] and r['latch'][2]>0))
# btlbw distribution end
import statistics
bs=[r['btlbw'] for r in ad if r['btlbw'] is not None]
print('end btlbw: <125', sum(1 for b in bs if b<125), '125..7874', sum(1 for b in bs if 125<=b<7875), '>=7875', sum(1 for b in bs if b>=7875), 'n',len(bs))
# arbiter reasons totals
arb=[0]*7
for r in ad:
    if r['arb']:
        for i,v in enumerate(r['arb']): arb[i]+=v
print('arb totals', arb)
