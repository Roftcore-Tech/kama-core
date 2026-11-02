import json, os, sys, glob, statistics as st
root = sys.argv[1]
THR_PTT = None  # computed per bundle_ms
P_PAR = 22 + 12 + 1024      # parity sealed len, hub envelope 22 (runtime.rs:2501 wire_len(HDR_FEC+e))
def pct(a, q):
    if not a: return None
    a = sorted(a); i = min(len(a)-1, int(round(q*(len(a)-1)))); return a[i]
out = {}
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    for arm in sorted(os.listdir(cd)):
        for rep in sorted(glob.glob(os.path.join(cd, arm, 'rep*'))):
            f = os.path.join(rep, 'alice.jsonl')
            if not os.path.exists(f): continue
            sends = []; probes = []; params = None
            for l in open(f):
                try: d = json.loads(l)
                except Exception: continue
                k = d.get('k')
                if k == 'hdr': params = d.get('params')
                elif k == 'send': sends.append((d['c'], d['t'], d.get('sp')))
                elif k == 'ts':
                    s = d.get('s') or []
                    if not s: continue
                    s0 = s[0]; rec = s0.get('rec') or {}
                    probes.append((d['t'], s0.get('par', 0), s0.get('pr', 0), s0.get('bw', 0), rec.get('txq'), rec.get('pl')))
            if not probes or params is None: continue
            T = params.get('ptt_bundle_ms', 200)
            # spurt windows from O..C sends per spurt index
            win = {}
            for c, t, sp in sends:
                if c in ('O', 'P', 'C'):
                    w = win.setdefault(sp, [t, t]); w[0] = min(w[0], t); w[1] = max(w[1], t)
            wins = list(win.values())
            fsend = [t for c, t, sp in sends if c == 'F']
            def in_spurt(t): return any(a <= t <= b for a, b in wins)
            r = out.setdefault((cell, arm), dict(runs=0, par=0, par_spurt=0, par_out=0, pr_spurt=[], par_txq0=0, par_txqpos=0, pr_at_par=[]))
            r['runs'] += 1
            for i in range(1, len(probes)):
                t0_, p0, *_ = probes[i-1]
                t1, p1, pr1, bw1, txq1, pl1 = probes[i]
                dp = p1 - p0
                if in_spurt(t1): r['pr_spurt'].append(pr1)
                if dp > 0:
                    r['par'] += dp
                    if in_spurt(t1) or in_spurt(t0_): r['par_spurt'] += dp
                    else: r['par_out'] += dp
                    txq0 = probes[i-1][4]
                    if (txq0 == 0) or (txq1 == 0): r['par_txq0'] += dp
                    else: r['par_txqpos'] += dp
                    r['pr_at_par'].append(pr1)
for (c, a), r in sorted(out.items()):
    thr = (P_PAR + 80) / (0.2 + 0.04)
    prs = r['pr_spurt']
    frac = (sum(1 for x in prs if x >= thr) / len(prs)) if prs else None
    print(f"{c:30s} {a:5s} runs={r['runs']:3d} par={r['par']:5d} in_spurt={r['par_spurt']:4d} out={r['par_out']:5d} "
          f"txq0={r['par_txq0']:5d} txq>0={r['par_txqpos']:4d} pr_spurt_med={pct(prs,0.5)} pr_spurt_p90={pct(prs,0.9)} frac_pr>={thr:.0f}={frac if frac is None else round(frac,3)} pr_at_par_med={pct(r['pr_at_par'],0.5)}")
