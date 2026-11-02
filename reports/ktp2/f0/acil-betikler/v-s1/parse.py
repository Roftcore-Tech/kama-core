import os, re, sys, json, glob

ROOT = '/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam'

RX = {
    'fec_blocks': re.compile(r'KP-CLIENT .*?fec_blocks=(\d+)'),
    'fec_repairs_tx': re.compile(r'KP-CLIENT .*?fec_repairs_tx=(\d+)'),
    'latch': re.compile(r'KP-CLIENT .*?latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)'),
    'flow_end': re.compile(r'KP-CLIENT .*?flow_end=(\d+)'),
    'risk_t': re.compile(r'KP-CLIENT .*?risk_t=\((\d+), (\d+), (\d+), (\d+)\)'),
    'arb': re.compile(r'KP-CLIENT .*?arb=\[([0-9, ]+)\]'),
    'fec2': re.compile(r'fec2=\[repair_not_arqed=(\d+) clamp=(\d+) would_zero=(\d+) partial_blocks=(\d+)\]'),
    'est': re.compile(r'est=\[btlbw=(\d+)B/s srtt=(\d+)us rtprop=(\d+)us qdelay=(\d+)us pace=(\d+)B/s'),
    'txfec': re.compile(r'KP-CLIENT .*?tx=(\d+)B\(data=(\d+) retx=(\d+) dup=(\d+) fec=(\d+) ctrl=(\d+)\)'),
    'delivered': re.compile(r'KP-CLIENT delivered=(\d+)/(\d+)'),
    'parity_denied': re.compile(r'parity=(\d+) parity_denied=(\d+)'),
}


def parse_cerr(path):
    try:
        t = open(path, errors='replace').read()
    except OSError:
        return None
    d = {}
    for k, rx in RX.items():
        m = rx.search(t)
        if m:
            g = m.groups()
            if k == 'arb':
                d[k] = [int(x) for x in g[0].split(',')]
            elif len(g) == 1:
                d[k] = int(g[0])
            else:
                d[k] = tuple(int(x) for x in g)
    return d


def runs(lane, cell_glob='*', arm_glob='*'):
    for p in sorted(glob.glob(f'{ROOT}/{lane}/out/{cell_glob}/{arm_glob}/rep*/c.err')):
        parts = p.split('/')
        yield parts[-4], parts[-3], parts[-2], p
