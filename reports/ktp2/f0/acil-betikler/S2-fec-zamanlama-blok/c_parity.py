import os, sys, glob, re, csv, statistics as st
root = sys.argv[1]
def med(a): return st.median(a) if a else None
def q(a, p):
    if not a: return None
    a = sorted(a); return a[min(len(a)-1, int(round(p*(len(a)-1))))]
rows = {}
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    m = re.search(r'_l(\d+)_b(\d+)', cell)
    loss = int(m.group(1))/100 if m else None
    T = int(m.group(2)) if m else None
    for arm in sorted(os.listdir(cd)):
        if not (arm.startswith('rs') or arm.startswith('uyar')): continue
        for rep in sorted(glob.glob(os.path.join(cd, arm, 'rep*'))):
            try:
                ce = open(os.path.join(rep, 'c.err')).read()
                se = open(os.path.join(rep, 's.err')).read()
            except Exception:
                continue
            lm = re.search(r'latch=\((\d+), (\d+), (\d+), (\d+), (\d+)\)', ce)
            if not lm: continue
            enq, emi = int(lm.group(4)), int(lm.group(5))
            den = re.search(r'parity_denied=(\d+)', ce); den = int(den.group(1)) if den else 0
            fe = re.search(r'flow_end=(\d+)', ce); fe = int(fe.group(1))/1e6 if fe else None
            def sv(k):
                mm = re.search(k + r': (\d+)', se); return int(mm.group(1)) if mm else 0
            rec = sv('fec_recovered_units'); rx = sv('fec_repairs_rx'); useful = sv('fec_repairs_useful'); wc = sv('fec_repairs_wasted_complete')
            dec = sv('fec_blocks_decoded')
            # ts integral
            integ = 0.0; last_t = None; last_L = 0; Lend = 0; Lmax = 0; t_first_nz = None
            try:
                with open(os.path.join(rep, 'ts.csv')) as fh:
                    rd = csv.DictReader(fh)
                    for r in rd:
                        t = int(r['t_ms']); L = int(r['pending_parity'])
                        if last_t is not None and (fe is None or t <= fe):
                            integ += last_L * (t - last_t)
                        last_t, last_L = t, L
                        if fe is None or t <= fe:
                            Lend = L; Lmax = max(Lmax, L)
            except Exception:
                pass
            W = integ / emi if emi else None
            r = rows.setdefault((cell, arm), dict(enq=[], emi=[], den=[], W=[], Lend=[], Lmax=[], rec=[], rx=[], useful=[], wc=[], loss=loss, T=T))
            r['enq'].append(enq); r['emi'].append(emi); r['den'].append(den)
            if W is not None: r['W'].append(W)
            r['Lend'].append(Lend); r['Lmax'].append(Lmax); r['rec'].append(rec); r['rx'].append(rx); r['useful'].append(useful); r['wc'].append(wc)
print("cell arm n enq_med emi_med emi/enq denied_med W_little_ms_med[p25-p75] pend_end_med pend_max_med fec_first_units_med rx_rep_med wasted_complete_share_med")
for (c, a), r in sorted(rows.items()):
    n = len(r['enq'])
    ratio = [e/q_ if q_ else 0 for e, q_ in zip(r['emi'], r['enq'])]
    wcs = [w/x if x else 0 for w, x in zip(r['wc'], r['rx'])]
    W = r['W']
    print(f"{c:24s} {a:11s} n={n:2d} enq={med(r['enq'])} emi={med(r['emi'])} emi/enq={med(ratio):.2f} den={med(r['den'])} "
          f"W={(med(W) or 0):.0f}[{(q(W,0.25) or 0):.0f}-{(q(W,0.75) or 0):.0f}] Lend={med(r['Lend'])} Lmax={med(r['Lmax'])} "
          f"fecfirst={med(r['rec'])} rx={med(r['rx'])} wc_share={med(wcs):.2f}")
