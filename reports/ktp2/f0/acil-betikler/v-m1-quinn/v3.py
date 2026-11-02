"""Soğuk-başlangıç vs kararlı rejim: kuyruk ne zaman oluşuyor? python3 -I v3.py <hücre>"""
import gzip, json, math, sys, statistics as st

R = sys.argv[1]
I = 0.285


def cap(path):
    with gzip.open(path, 'rt') as f:
        for l in f:
            if l.startswith('#'):
                continue
            p = l.rstrip('\n').split(',')
            if len(p) < 7 or not p[6] or p[2] != "17":
                continue
            yield int(p[0]), p[1], int(p[3]), p[6]


for arm in ('taban', 'aday'):
    rows = []
    for rep in range(1, 31):
        d = f"{R}/{arm}/rep{rep}"
        L = [json.loads(l) for l in open(f"{d}/out.jsonl")]
        meta = L[0]['meta']
        ev = [x['event'] for x in L[1:] if 'event' in x]
        t0 = meta['t0_unix_ns']; off = meta['events_epoch_offset_ns']
        first = {}
        for t, dr, ipl, x in cap(f"{d}/capb_cli.csv.gz"):
            if dr != 'o':
                continue
            b = bytes.fromhex(x.split(':', 1)[0])
            if b[0] == 0 and len(b) >= 11:
                first.setdefault(int.from_bytes(b[3:11], 'big'), (t - t0) / 1e9)
        su = {e['msg_id']: (e['sent_ns'] - off) / 1e9 for e in ev}
        w = {m: first[m] - su[m] for m in su if m in first}
        def wq(m):
            return round(w[m], 1) if m in w else None
        # first-tx time of unit 1 (cold-start release)
        u1 = first.get(1)
        # new-data release rate during [first(20), first(60)] : units/second
        pc = meta['activation']['left_preconverge_ns'] / 1e9
        rows.append((rep, round(pc, 1), None if u1 is None else round(u1, 1), wq(5), wq(20), wq(40), wq(60), wq(80), wq(99),
                     meta['activation']['btlbw_bps']))
    print(arm, '(rep, left_preconverge_s, first_tx_unit1_s, wait@5, @20, @40, @60, @80, @99, btlbw_end)')
    for r in rows:
        print('  ', r)
    pcs = [r[1] for r in rows]
    print('  left_preconverge median=%.1f, >10s: %d/30' % (st.median(pcs), sum(1 for p in pcs if p > 10)))
    for k, idx in (('@20', 4), ('@40', 5), ('@60', 6), ('@80', 7), ('@99', 8)):
        v = [r[idx] for r in rows if r[idx] is not None]
        print('  wait%s median=%.1f (n=%d)' % (k, st.median(v), len(v)))
