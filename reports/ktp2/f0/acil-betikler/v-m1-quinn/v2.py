"""Çerçeve tipi dağılımı + ilk-çıkış beklemesi (inf ayrıntısı). python3 -I v2.py <hücre>"""
import gzip, json, math, os, sys, statistics as st
from collections import Counter

R = sys.argv[1]
INF = float('inf')


def qnr(v, p):
    v = sorted(v)
    k = max(1, math.ceil(p * len(v)))
    return v[k - 1]


def cap(path):
    with gzip.open(path, 'rt') as f:
        for l in f:
            if l.startswith('#'):
                yield ('#', l.strip())
                continue
            p = l.rstrip('\n').split(',')
            if len(p) < 7:
                continue
            yield (int(p[0]), p[1], int(p[2]), int(p[3]), int(p[4]), int(p[5]), p[6])


for arm in ('taban', 'aday'):
    ft_tot = Counter()
    per_p95_fin = []
    per_med_fin = []
    infs = []
    for rep in range(1, 31):
        d = f"{R}/{arm}/rep{rep}"
        L = [json.loads(l) for l in open(f"{d}/out.jsonl")]
        meta = L[0]['meta']
        ev = [x['event'] for x in L[1:] if 'event' in x]
        t0 = meta['t0_unix_ns']; off = meta['events_epoch_offset_ns']
        first = {}
        last_o = 0
        stats = None
        for r in cap(f"{d}/capb_cli.csv.gz"):
            if r[0] == '#':
                stats = r[1]
                continue
            t, dr, pr, ipl, sp, dp, x = r
            if dr == 'o' and pr == 17 and x:
                b = bytes.fromhex(x.split(':', 1)[0])
                ft_tot[b[0]] += 1
                last_o = max(last_o, t)
                if b[0] == 0 and len(b) >= 11:
                    u = int.from_bytes(b[3:11], 'big')
                    first.setdefault(u, t)
        waits = []
        miss = []
        for e in ev:
            su = t0 + e['sent_ns'] - off
            if e['msg_id'] in first:
                waits.append((first[e['msg_id']] - su) / 1e9)
            else:
                miss.append((e['msg_id'], e['delivered_ns'] is not None))
        per_p95_fin.append(qnr(waits, .95))
        per_med_fin.append(st.median(waits))
        end_rel = (last_o - t0) / 1e9
        if miss:
            infs.append((rep, len(miss), sum(1 for m in miss if m[1]), min(m[0] for m in miss), round(end_rel, 1), stats,
                         meta['activation']['btlbw_bps'], round(st.median(waits), 2)))
    print(arm, 'frame types (o):', {hex(k): v for k, v in sorted(ft_tot.items())})
    print('  median of per-run median(wait, finite)=%.2f, median of per-run p95(wait, finite)=%.2f' % (
        st.median(per_med_fin), st.median(per_p95_fin)))
    print('  runs with never-sent msgs (rep, n_miss, n_miss_but_delivered, first_missing_msg, last_o_rel_s, capstats, btlbw, med_wait):')
    for x in infs:
        print('   ', x)
