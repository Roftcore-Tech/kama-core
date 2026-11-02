import json, sys, collections, statistics as st
rows = [json.loads(l) for l in open(sys.argv[1])]
ad = [r for r in rows if r['fec'] and 'adaptive' in r['fec']]
print('adaptive runs', len(ad), collections.Counter(r['fec'] for r in ad))
print('by lane', collections.Counter(r['lane'] for r in ad))
print('profile', collections.Counter(r['profile'] for r in ad))
# key totals
def s(k, rs): return sum((r.get(k) or 0) for r in rs)
for lane in 'bcd':
    rs = [r for r in ad if r['lane']==lane]
    if not rs: continue
    nz_rep = sum(1 for r in rs if (r.get('fec_repairs_tx') or 0) > 0)
    nz_blk = sum(1 for r in rs if (r.get('fec_blocks') or 0) > 0)
    print(f"lane {lane}: runs={len(rs)} runs_with_repairs={nz_rep} runs_with_blocks={nz_blk} sum_blocks={s('fec_blocks',rs)} sum_repairs={s('fec_repairs_tx',rs)} sum_clamp={s('clamp',rs)} sum_would_zero={s('would_zero',rs)}")
    arb = [0]*7
    for r in rs:
        if r.get('arb'):
            for i,v in enumerate(r['arb']): arb[i]+=v
    print('  arbiter reasons [None,ConfFuzzy,ConfPolicy,Gate,SpecL1,SpecL2,Deadline] =', arb)
    bt = [r['btlbw_end'] for r in rs if r.get('btlbw_end')]
    if bt:
        bt.sort(); print('  btlbw_end B/s: min', bt[0], 'p50', bt[len(bt)//2], 'p90', bt[int(len(bt)*0.9)], 'max', bt[-1], ' n>=7875:', sum(1 for b in bt if b>=7875), '/', len(bt))
    tsn = s('ts_n', rs); print('  ts samples', tsn, 'prop>0', s('ts_prop_pos',rs), 'final>0', s('ts_final_pos',rs), 'r8>=1', s('ts_r8_pos',rs))
    fmax = [r.get('ts_final_max') for r in rs if r.get('ts_final_max') is not None]
    c = collections.Counter(fmax); print('  ts final_ratio max per run (top):', c.most_common(8))
# runs with repairs: details
print('--- runs with fec_blocks>0 ---')
for r in ad:
    if (r.get('fec_blocks') or 0) > 0:
        print(r['lane'], r['cell'], r['arm'], r['rep'], 'blocks', r['fec_blocks'], 'rep', r['fec_repairs_tx'], 'would_zero', r.get('would_zero'), 'btlbw_end', r.get('btlbw_end'), 'tsfinalmax', r.get('ts_final_max'), 'latch', r.get('latch'))
