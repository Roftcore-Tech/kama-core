"""Bağımsız yeniden hesap: M1-1 / M1-2 (salt okuma). python3 -I v1.py <hücre_dizini>"""
import gzip, json, math, os, sys, statistics as st

R = sys.argv[1]
INF = float('inf')


def qnr(v, p):
    v = sorted(v)
    k = max(1, math.ceil(p * len(v)))
    return v[k - 1]


def med(v):
    v = sorted(v)
    n = len(v)
    if n % 2:
        return v[n // 2]
    a, b = v[n // 2 - 1], v[n // 2]
    if math.isinf(a) or math.isinf(b):
        return INF if (math.isinf(a) and math.isinf(b)) else (a + b) / 2  # not reached normally
    return (a + b) / 2


def load_out(d):
    L = [json.loads(l) for l in open(f"{d}/out.jsonl")]
    meta = L[0]['meta']
    ev = [x['event'] for x in L[1:] if 'event' in x]
    return meta, ev


def load_rx(d):
    rx = {}
    with open(f"{d}/rx_log.jsonl") as f:
        hdr = json.loads(f.readline())
        for l in f:
            o = json.loads(l)
            u = o['u']
            if u not in rx or o['t'] < rx[u]:
                rx[u] = o['t']
    return hdr, rx


def load_cap(path):
    rows = []
    with gzip.open(path, 'rt') as f:
        for l in f:
            if l.startswith('#'):
                continue
            p = l.rstrip('\n').split(',')
            if len(p) < 7:
                continue
            rows.append((int(p[0]), p[1], int(p[2]), int(p[3]), int(p[4]), int(p[5]), p[6]))
    return rows


def kama_data_unit(x):
    h = x.split(':', 1)[0]
    b = bytes.fromhex(h)
    if len(b) >= 11 and b[0] == 0x00:
        return int.from_bytes(b[3:11], 'big')
    return None


out = {}
for arm in ('taban', 'aday', 'quinn'):
    per = []
    for rep in range(1, 31):
        d = f"{R}/{arm}/rep{rep}"
        if not os.path.isdir(d):
            continue
        meta, ev = load_out(d)
        t0 = meta['t0_unix_ns']
        off = meta['events_epoch_offset_ns']
        I = meta['submit_interval_ns']
        lat = []
        for e in ev:
            lat.append(INF if e['delivered_ns'] is None else (e['delivered_ns'] - e['sent_ns']) / 1e9)
        lat_st = [lat[i] for i in range(20, 100)]
        hdr, rx = load_rx(d)
        ow = []
        for e in ev:
            su = t0 + e['sent_ns'] - off
            ow.append((rx[e['msg_id']] - su) / 1e9 if e['msg_id'] in rx else INF)
        slip = max(abs((e['sent_ns'] - off) - e['msg_id'] * I) for e in ev) / 1e6
        rec = dict(rep=rep, p95=qnr(lat, .95), p50=qnr(lat, .5), p95_st=qnr(lat_st, .95),
                   ow95=qnr(ow, .95), ow50=qnr(ow, .5), slip_ms=slip,
                   ndeliv=sum(1 for x in lat if not math.isinf(x)), rxdrop=hdr.get('dropped'))
        # capture
        cli = load_cap(f"{d}/capb_cli.csv.gz")
        if arm != 'quinn':
            # client port = sport of 'o' to server port
            first_out = {}
            outs = {}
            for (t, dr, pr, ipl, sp, dp, x) in cli:
                if dr == 'o' and pr == 17 and x:
                    u = kama_data_unit(x)
                    if u is not None:
                        outs.setdefault(u, []).append(t)
            waits = []
            w10 = 0
            for e in ev:
                su = t0 + e['sent_ns'] - off
                u = e['msg_id']
                if u in outs:
                    w = (min(outs[u]) - su) / 1e9
                    waits.append(w)
                    if w <= 0.010:
                        w10 += 1
                else:
                    waits.append(INF)
            rec['wait_list'] = waits
            rec['w10'] = w10
            rec['btlbw'] = meta['activation']['btlbw_bps'] if meta.get('activation') else None
            rec['srtt_us'] = meta['activation']['srtt_us'] if meta.get('activation') else None
        else:
            # quinn: data packets > 500 B IP in first 10 s after t0
            n10 = sum(1 for (t, dr, pr, ipl, sp, dp, x) in cli
                      if dr == 'o' and pr == 17 and ipl > 500 and t0 <= t < t0 + 10_000_000_000)
            rec['n10'] = n10
            nsub10 = sum(1 for e in ev if e['sent_ns'] < 10e9)
            rec['nsub10'] = nsub10
        per.append(rec)
    out[arm] = per

for arm, per in out.items():
    def m(k):
        return med([r[k] for r in per])
    print(f"{arm}: N={len(per)} lat_p95_med={m('p95'):.2f} lat_p50_med={m('p50'):.2f} p95_msg20-99={m('p95_st'):.2f} "
          f"ow95={m('ow95'):.2f} ow50={m('ow50'):.2f} maxslip_ms={max(r['slip_ms'] for r in per):.2f} "
          f"p95_inf_runs={sum(1 for r in per if math.isinf(r['p95']))} rxdrop={set(r['rxdrop'] for r in per)}")
    if arm != 'quinn':
        allw = [w for r in per for w in r['wait_list']]
        fin = [w for w in allw if not math.isinf(w)]
        print(f"   wait pooled: n={len(allw)} inf={len(allw)-len(fin)} med={med(allw):.2f} p95={qnr(allw,.95):.2f} "
              f"w<=10ms frac={sum(r['w10'] for r in per)/len(allw):.3f} min={min(fin):.3f}")
        pr_med = [med(r['wait_list']) for r in per]
        print(f"   wait per-run medians: median={med(pr_med):.2f} range=({min(pr_med):.2f},{max(pr_med):.2f})")
        bw = [r['btlbw'] for r in per]
        print(f"   btlbw final median={med(bw)} range=({min(bw)},{max(bw)}) srtt_med_us={med([r['srtt_us'] for r in per])}")
    else:
        print(f"   n10 median={med([r['n10'] for r in per])} range=({min(r['n10'] for r in per)},{max(r['n10'] for r in per)}) nsub10={set(r['nsub10'] for r in per)}")

json.dump({a: [{k: v for k, v in r.items() if k != 'wait_list'} for r in p] for a, p in out.items()},
          open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'v1_out.json'), 'w'), default=str)
# rep9 msg 98
for r in out['taban']:
    if r['rep'] == 9:
        print('taban rep9 msg98 wait', r['wait_list'][98])
