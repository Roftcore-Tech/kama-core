import json, sys, os, statistics as st, math
base = sys.argv[1]
def pct(xs, q):
    xs = sorted(xs)
    if not xs: return float('nan')
    k = (len(xs)-1)*q
    f = math.floor(k); c = math.ceil(k)
    if f == c: return xs[f]
    return xs[f] + (xs[c]-xs[f])*(k-f)
offered = 1069/0.285
for arm in ['taban','aday','quinn','tcp']:
    d = os.path.join(base, arm)
    if not os.path.isdir(d): continue
    rows = []
    for rep in sorted(os.listdir(d), key=lambda s: int(s[3:]) if s[3:].isdigit() else 0):
        p = os.path.join(d, rep, 'out.jsonl')
        if not os.path.exists(p): continue
        meta=None; lat=[]; n=0
        for line in open(p):
            line=line.strip()
            if not line: continue
            o=json.loads(line)
            if 'meta' in o: meta=o['meta']
            elif 'event' in o:
                e=o['event']
                if e.get('delivered_ns') is not None:
                    lat.append((e['delivered_ns']-e['sent_ns'])/1e9)
        act = (meta or {}).get('activation') or {}
        tx = (meta or {}).get('tx') or {}
        rows.append(dict(rep=rep, n=len(lat), p95=pct(lat,0.95), btl=act.get('btlbw_bps'), par=act.get('parity_emitted'), retx=tx.get('retx'), data=tx.get('data'), lpc=act.get('left_preconverge_ns')))
    p95s=[r['p95'] for r in rows]
    print(arm, 'N', len(rows), 'n_deliv min', min(r['n'] for r in rows), 'p95 median %.3f'%st.median(p95s))
    if arm in ('taban','aday'):
        b=[r['btl'] for r in rows]
        print('  btlbw median', st.median(b), 'min', min(b), 'max', max(b), 'btl/offered med %.3f'%(st.median(b)/offered))
        req = offered/0.7
        print('  below required(%.0f): %d/%d'%(req, sum(1 for x in b if x<req), len(b)))
        lo=[r['p95'] for r in rows if r['btl']<offered]; hi=[r['p95'] for r in rows if r['btl']>=offered]
        print('  btl<offered n=%d p95med=%.2f ; btl>=offered n=%d p95med=%.2f'%(len(lo), st.median(lo) if lo else float('nan'), len(hi), st.median(hi) if hi else float('nan')))
        print('  parity total', sum(r['par'] or 0 for r in rows), 'retx/data %.3f'%(sum(r['retx'] for r in rows)/sum(r['data'] for r in rows)))
        print('  left_preconverge_ns med', st.median([r['lpc'] for r in rows]))
