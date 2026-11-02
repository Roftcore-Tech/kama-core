import os, sys, glob, re, csv, statistics as st
root = sys.argv[1]
def med(a): return st.median(a) if a else float('nan')
res = {}
for rep in sorted(glob.glob(os.path.join(root, '*', '*', 'rep*'))):
    parts = rep.split('/')
    cell, arm = parts[-3], parts[-2]
    if not any(arm.startswith(x) for x in ('rs2', 'rs4', 'arq', 'uyar')): continue
    try:
        ce = open(os.path.join(rep, 'c.err')).read(); se = open(os.path.join(rep, 's.err')).read()
    except OSError: continue
    g = lambda pat, s=ce: (int(m.group(1)) if (m := re.search(pat, s)) else None)
    den = g(r'parity_denied=(\d+)'); fe = g(r'flow_end=(\d+)')
    lt = re.search(r'latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)', ce)
    enq, emi = (int(lt.group(4)), int(lt.group(5))) if lt else (None, None)
    ftx = g(r'fec_repairs_tx=(\d+)')
    rec = g(r'fec_recovered_units: (\d+)', se)
    deliv = g(r'KP-CLIENT delivered=(\d+)/')
    # ts integral over [0, flow_end]
    integ = 0.0; lt_ = None; lL = 0; Lend = None; Lmax = 0
    fe_ms = fe / 1e6 if fe else None
    try:
        for r in csv.DictReader(open(os.path.join(rep, 'ts.csv'))):
            t = int(r['t_ms']); L = int(r['pending_parity'])
            if fe_ms is not None and t > fe_ms: break
            if lt_ is not None: integ += lL * (t - lt_)
            lt_, lL = t, L; Lend = L; Lmax = max(Lmax, L)
    except OSError: pass
    d = res.setdefault((cell, arm), dict(dpm=[], W=[], Lend=[], Lmax=[], ratio=[], rec=[], enq=[], emi=[], ftx=[]))
    if den is not None and fe: d['dpm'].append(den / (fe / 1e6))
    if emi: d['W'].append(integ / emi)
    if Lend is not None: d['Lend'].append(Lend); d['Lmax'].append(Lmax)
    if enq: d['ratio'].append(emi / enq)
    if rec is not None: d['rec'].append(rec)
    if enq is not None: d['enq'].append(enq); d['emi'].append(emi)
    if ftx is not None: d['ftx'].append(ftx)
for (c, a), d in sorted(res.items()):
    if not a.endswith('taban') and not a.startswith('uyar'): continue
    print(f"{c:24s} {a:11s} n={len(d['dpm']):2d} den/ms={med(d['dpm']):.2f} W_ms={med(d['W']):.0f} Lend={med(d['Lend'])} Lmax={med(d['Lmax'])} emi/enq={med(d['ratio']):.2f} rec={med(d['rec'])} enq={med(d['enq'])} ftx_sum={sum(d['ftx'])} n_ftx>0={sum(1 for x in d['ftx'] if x>0)}/{len(d['ftx'])}")
