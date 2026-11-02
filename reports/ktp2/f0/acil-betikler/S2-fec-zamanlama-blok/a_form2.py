import json, os, sys, glob
root = sys.argv[1]; K = 8
def pct(a, q):
    if not a: return None
    a = sorted(a); return a[min(len(a)-1, int(round(q*(len(a)-1))))]
res = {}
for cell in sorted(os.listdir(root)):
    cd = os.path.join(root, cell)
    if not os.path.isdir(cd): continue
    for arm in sorted(os.listdir(cd)):
        for rep in sorted(glob.glob(os.path.join(cd, arm, 'rep*'))):
            f = os.path.join(rep, 'alice.jsonl')
            if not os.path.exists(f): continue
            S = {}  # unit -> (len, at)
            for l in open(f):
                if '"k":"tr"' not in l: continue
                try: d = json.loads(l)
                except Exception: continue
                for r in d['r']:
                    if r[0] == 'S' and r[1] == 'SessionHandle(0)':
                        S.setdefault(r[2], (r[3], r[4]))
            if not S: continue
            mx = max(S)
            r = res.setdefault((cell, arm), dict(ptt=[], ptt_open=0, ptt_n=0, other=[]))
            for u, (ln, at) in S.items():
                b = u // K; last = b*K + K - 1
                if ln != 53:  # PTT bundle units only (40 B audio + AMP; seen as 53 B)
                    continue
                r['ptt_n'] += 1
                if last in S:
                    r['ptt'].append(S[last][1] - at)
                else:
                    r['ptt_open'] += 1  # block never closed inside the session trace (closes only at shutdown)
for (c, a), r in sorted(res.items()):
    p = r['ptt']
    thr = 2600 if "_d1000_" in c else 1200; fr = sum(1 for x in p if x > thr)/len(p) if p else 0; print(f"{c:30s} {a:5s} n={len(p)} frac_formation>{thr}ms={fr:.3f} mean={sum(p)/len(p):.0f}")
