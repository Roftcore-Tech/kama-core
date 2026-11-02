import os, re, sys, csv, glob
ROOT = '/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'
OUT = '/tmp/claude-1000/-home-ryuzaki-Desktop-kama-core/c4d5f74c-3e03-4ef2-ad8a-a4d0e69ca737/scratchpad/acil/s6-verify/cerr.tsv'

def g(pat, s, cast=int, default=None):
    m = re.search(pat, s)
    if not m:
        return default
    try:
        return cast(m.group(1))
    except Exception:
        return m.group(1)

rows = []
for lane in 'abcd':
    for cerr in glob.glob(f'{ROOT}/{lane}/out/*/*/rep*/c.err'):
        parts = cerr.split('/')
        cell, arm, rep = parts[-4], parts[-3], parts[-2]
        try:
            txt = open(cerr, errors='replace').read()
        except Exception:
            continue
        man = re.search(r'KP-MANIFEST role=client profile=(\S+) .*? fec=(\S+(?: \S+ \S+)?)', txt)
        if not man:
            rows.append(dict(lane=lane, cell=cell, arm=arm, rep=rep, profile='', fecmode='NOMANIFEST'))
            continue
        fecmode = man.group(2)
        kc = re.search(r'KP-CLIENT delivered=.*', txt)
        kc = kc.group(0) if kc else ''
        ka = re.search(r'KP-ACTIVATED .*', txt)
        ka = ka.group(0) if ka else ''
        kv = re.search(r'KP-ACTIVATION .*', txt)
        kv = kv.group(0) if kv else ''
        rt = re.search(r'risk_t=\((\d+), (\d+), (\d+), (\d+)\)', kc)
        r = dict(lane=lane, cell=cell, arm=arm, rep=rep, profile=man.group(1), fecmode=fecmode,
                 delivered=g(r'delivered=(\d+)/', kc), msgs=g(r'delivered=\d+/(\d+)', kc),
                 fec_tx=g(r'fec=(\d+) ctrl', kc),
                 fec_blocks=g(r'fec_blocks=(\d+)', kc), fec_repairs_tx=g(r'fec_repairs_tx=(\d+)', kc),
                 deadline=g(r'deadline_ns=(\S+) fusion', kc, str), fusion=g(r'fusion=(\[[^\]]*\])', kc, str),
                 t_susp=rt.group(1) if rt else '', t_conf=rt.group(2) if rt else '',
                 t_risk=rt.group(3) if rt else '', t_par=rt.group(4) if rt else '',
                 flow_end=g(r'flow_end=(\d+)', kc),
                 preconv_blocked=g(r'preconv_parity_blocked=(\d+)', ka),
                 clamp=g(r'clamp=(\d+)', ka), would_zero=g(r'would_zero=(\d+)', ka),
                 partial_blocks=g(r'partial_blocks=(\d+)', ka),
                 parity=g(r'parity=(\d+) parity_denied', kv), parity_denied=g(r'parity_denied=(\d+)', kv),
                 left_preconv_ms=g(r'left_preconv=(\d+)ms', kv),
                 btlbw=g(r'est=\[btlbw=(\d+)B/s', kv), srtt_us=g(r'srtt=(\d+)us', kv),
                 state=g(r'state=(\S+) left_preconv', kc, str),
                 latch=g(r'latch=(\([^)]*\))', kc, str), suppress=g(r'suppress=(\S+) risk_t', kc, str))
        rows.append(r)

keys = ['lane','cell','arm','rep','profile','fecmode','delivered','msgs','fec_tx','fec_blocks','fec_repairs_tx','deadline','fusion','t_susp','t_conf','t_risk','t_par','flow_end','preconv_blocked','clamp','would_zero','partial_blocks','parity','parity_denied','left_preconv_ms','btlbw','srtt_us','state','latch','suppress']
with open(OUT, 'w', newline='') as f:
    w = csv.DictWriter(f, fieldnames=keys, delimiter='\t', extrasaction='ignore')
    w.writeheader()
    for r in rows:
        w.writerow(r)
print(len(rows))
