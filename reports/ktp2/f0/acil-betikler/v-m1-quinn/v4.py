"""M1-2: (unit, wire-seq) ile birebir eşleşen kayıp/yeniden iletim zamanlaması + quinn anında-çıkış.
python3 -I v4.py <hücre>"""
import gzip, json, math, sys, statistics as st

R = sys.argv[1]


def qnr(v, p):
    v = sorted(v)
    k = max(1, math.ceil(p * len(v)))
    return v[k - 1]


def cap(path):
    with gzip.open(path, 'rt') as f:
        for l in f:
            if l.startswith('#'):
                continue
            p = l.rstrip('\n').split(',')
            if len(p) < 7 or not p[6] or p[2] != '17':
                continue
            yield int(p[0]), p[1], int(p[3]), int(p[4]), int(p[5]), p[6]


def data_us(x):
    b = bytes.fromhex(x.split(':', 1)[0])
    if len(b) >= 11 and b[0] == 0:
        return int.from_bytes(b[3:11], 'big'), int.from_bytes(b[1:3], 'big')
    return None


for arm in ('taban', 'aday'):
    nunits = 0; nlost1 = 0; gap12 = []; rec = []; ok1 = []; ntx_lost = []; gap12_lost_ge2 = 0
    lost_no2 = 0; never = 0
    lost_by_class = {'heuristic_1p6': 0}
    retx_kind_first_gap = []
    for rep in range(1, 31):
        d = f"{R}/{arm}/rep{rep}"
        tx = {}
        for t, dr, ipl, sp, dp, x in cap(f"{d}/capb_cli.csv.gz"):
            if dr != 'o':
                continue
            r = data_us(x)
            if r:
                tx.setdefault(r[0], []).append((t, r[1]))
        arr = {}
        for t, dr, ipl, sp, dp, x in cap(f"{d}/capb_srv.csv.gz"):
            if dr != 'i':
                continue
            r = data_us(x)
            if r:
                arr.setdefault((r[0], r[1]), t)
        for u, txs in tx.items():
            if u >= 100:
                continue
            txs.sort()
            nunits += 1
            t1, s1 = txs[0]
            arrivals = [arr[(u, s)] for (_, s) in txs if (u, s) in arr]
            if (u, s1) in arr:
                ok1.append((arr[(u, s1)] - t1) / 1e9)
                # heuristic of investigator: first arrival - first tx < 1.6 s
                if min(arrivals) - t1 >= 1.6e9:
                    lost_by_class['heuristic_1p6'] += 1
                continue
            nlost1 += 1
            ntx_lost.append(len(txs))
            if len(txs) >= 2:
                gap12.append((txs[1][0] - t1) / 1e9)
            else:
                lost_no2 += 1
            if arrivals:
                rec.append((min(arrivals) - t1) / 1e9)
            else:
                never += 1
    print(f"{arm}: units={nunits} first-tx-lost(exact)={nlost1} ({nlost1/nunits:.3f}) "
          f"ok1 misclassified-as-lost by 1.6s rule={lost_by_class['heuristic_1p6']}")
    print(f"   first-tx ok: tx->arr p50={st.median(ok1):.3f} p95={qnr(ok1,.95):.3f} max={max(ok1):.3f} n={len(ok1)}")
    print(f"   lost-first: 1st->2nd tx p50={st.median(gap12):.2f} p95={qnr(gap12,.95):.2f} n={len(gap12)} (no 2nd tx: {lost_no2})")
    print(f"   lost-first: 1st tx->first arrival p50={st.median(rec):.2f} p95={qnr(rec,.95):.2f} n={len(rec)} never_arrived={never}")
    print(f"   gap12/RTT_fifo(2.137) p50={st.median(gap12)/2.137:.2f}")

# quinn: does a >500 B short-header packet leave within X ms after each offer instant?
for X in (10, 50):
    fr = []
    for rep in range(1, 31):
        d = f"{R}/quinn/rep{rep}"
        L = [json.loads(l) for l in open(f"{d}/out.jsonl")]
        meta = L[0]['meta']; ev = [x['event'] for x in L[1:] if 'event' in x]
        t0 = meta['t0_unix_ns']
        pk = sorted(t for t, dr, ipl, sp, dp, x in cap(f"{d}/capb_cli.csv.gz")
                    if dr == 'o' and ipl > 500 and t >= t0 and not (int(x[:2], 16) & 0x80))
        import bisect
        hit = 0
        for e in ev:
            su = t0 + e['sent_ns']
            i = bisect.bisect_left(pk, su)
            if i < len(pk) and pk[i] - su <= X * 1e6:
                hit += 1
        fr.append(hit / len(ev))
    print(f"quinn: share of offers followed by >500B 1-RTT pkt within {X} ms: median per run={st.median(fr):.3f} min={min(fr):.3f}")
