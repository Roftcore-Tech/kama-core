"""El sıkışma karşı-olgusu, ACK/rx tutarlılığı, quinn paket boyu, rep9 örnekleri. python3 -I v5.py <hücre>"""
import gzip, json, math, sys, statistics as st
from collections import Counter

R = sys.argv[1]
INF = float('inf')


def qnr(v, p):
    v = sorted(v)
    k = max(1, math.ceil(p * len(v)))
    return v[k - 1]


def med(v):
    v = sorted(v); n = len(v)
    return v[n // 2] if n % 2 else (v[n // 2 - 1] + v[n // 2]) / 2


def cap(path):
    with gzip.open(path, 'rt') as f:
        for l in f:
            if l.startswith('#'):
                continue
            p = l.rstrip('\n').split(',')
            if len(p) < 7 or not p[6] or p[2] != '17':
                continue
            yield int(p[0]), p[1], int(p[3]), p[6]


for arm in ('taban', 'aday', 'quinn'):
    cf = []; setup = []; ack_no_rx = 0; rx_no_ack = 0; base = []; pc_cf = []
    for rep in range(1, 31):
        d = f"{R}/{arm}/rep{rep}"
        L = [json.loads(l) for l in open(f"{d}/out.jsonl")]
        meta = L[0]['meta']; ev = [x['event'] for x in L[1:] if 'event' in x]
        I = meta['submit_interval_ns'] / 1e9
        s = meta['setup_ns'] / 1e9
        setup.append(s)
        lat = [INF if e['delivered_ns'] is None else (e['delivered_ns'] - e['sent_ns']) / 1e9 for e in ev]
        base.append(qnr(lat, .95))
        cf.append(qnr([l + max(0.0, s - i * I) for i, l in enumerate(lat)], .95))
        rx = set()
        with open(f"{d}/rx_log.jsonl") as f:
            f.readline()
            for l in f:
                rx.add(json.loads(l)['u'])
        for e in ev:
            if e['delivered_ns'] is not None and e['msg_id'] not in rx:
                ack_no_rx += 1
            if e['delivered_ns'] is None and e['msg_id'] in rx:
                rx_no_ack += 1
    print(f"{arm}: setup med={med(setup):.2f}s range=({min(setup):.2f},{max(setup):.2f}) p95 base={med(base):.2f} "
          f"p95 hs-in-window={med(cf):.2f} ack_no_rx={ack_no_rx} rx_no_ack={rx_no_ack}")

# quinn data packet IP size
sz = Counter()
for rep in range(1, 31):
    d = f"{R}/quinn/rep{rep}"
    for t, dr, ipl, x in cap(f"{d}/capb_cli.csv.gz"):
        if dr == 'o' and ipl > 500 and not (int(x[:2], 16) & 0x80):
            sz[ipl] += 1
print('quinn >500B 1-RTT IP size top:', sz.most_common(4))

# taban rep9 msg 7 / 49: tx count and one-way
d = f"{R}/taban/rep9"
L = [json.loads(l) for l in open(f"{d}/out.jsonl")]
meta = L[0]['meta']; ev = {x['event']['msg_id']: x['event'] for x in L[1:] if 'event' in x}
t0 = meta['t0_unix_ns']; off = meta['events_epoch_offset_ns']
tx = {}
for t, dr, ipl, x in cap(f"{d}/capb_cli.csv.gz"):
    if dr == 'o':
        b = bytes.fromhex(x.split(':', 1)[0])
        if b[0] == 0 and len(b) >= 11:
            tx.setdefault(int.from_bytes(b[3:11], 'big'), []).append(t)
rx = {}
with open(f"{d}/rx_log.jsonl") as f:
    f.readline()
    for l in f:
        o = json.loads(l); rx.setdefault(o['u'], o['t'])
for m in (7, 49, 98):
    su = t0 + ev[m]['sent_ns'] - off
    print(f"taban rep9 msg{m}: ntx={len(tx.get(m, []))} first_tx_wait={(min(tx[m]) - su) / 1e9:.2f} "
          f"oneway={(rx[m] - su) / 1e9 if m in rx else None}")
