import json, sys, collections
rows = [json.loads(l) for l in open(sys.argv[1])]
ad = [r for r in rows if r['fec'] and 'adaptive p0x05' in r['fec'] and r['lane'] in ('b','c')]
by = collections.defaultdict(list)
for r in ad:
    arm = r['arm'].replace('uyar_','')
    by[(r['cell'])].append(r)
print("cell | n | btlbw_end p10/p50/p90 | cap‰(p50) | runs parity>0 | ts prop>0 % | ts r8>=1 % | wz>0 runs")
for cell in sorted(by):
    rs = by[cell]
    bt = sorted(r['btlbw_end'] for r in rs if r.get('btlbw_end') is not None)
    if not bt: continue
    p = lambda q: bt[min(len(bt)-1, int(len(bt)*q))]
    n = sum(r.get('ts_n') or 0 for r in rs); pp = sum(r.get('ts_prop_pos') or 0 for r in rs); r8 = sum(r.get('ts_r8_pos') or 0 for r in rs)
    par = sum(1 for r in rs if (r.get('fec_repairs_tx') or 0) > 0)
    wz = sum(1 for r in rs if (r.get('would_zero') or 0) > 0)
    print(f"{cell} | {len(rs)} | {p(.1)}/{p(.5)}/{p(.9)} | {min(p(.5)*1000//125000,900)} | {par} | {100*pp/max(n,1):.1f} | {100*r8/max(n,1):.2f} | {wz}")
