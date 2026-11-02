#!/usr/bin/env python3
# ts.csv bagimsiz analizi: oneri (clamp oncesi max kol), final (clamp sonrasi), r=(final*8+500)//1000
import os, csv, collections
ROOT = "/home/ryuzaki/kama-k4g-work/ktp2-f0/runs/tam"
OUT = os.path.dirname(os.path.abspath(__file__))
res = []
propvals = collections.Counter()
for lane, armsel in (("b", ("taban", "aday")), ("c", ("uyar_taban", "uyar_aday"))):
    lroot = os.path.join(ROOT, lane, "out")
    for cell in sorted(os.listdir(lroot)):
        cdir = os.path.join(lroot, cell)
        if not os.path.isdir(cdir):
            continue
        for arm in armsel:
            adir = os.path.join(cdir, arm)
            if not os.path.isdir(adir):
                continue
            for rep in sorted(os.listdir(adir)):
                p = os.path.join(adir, rep, "ts.csv")
                if not os.path.exists(p):
                    continue
                n = nprop = nfin = nr1 = nr2 = 0
                fmin = fmax = None
                pmax = 0
                cmax = smax = spmax = 0
                first_pend_ms = None
                pend_int = 0.0
                last_t = None
                last_pend = 0
                with open(p) as f:
                    rd = csv.DictReader(f)
                    for row in rd:
                        n += 1
                        cp = int(row["confirmed_prop"]); sp = int(row["structure_prop"]); spp = int(row["spec_prop"])
                        fr = int(row["final_ratio"])
                        t = int(row["t_ms"]); pend = int(row["pending_parity"])
                        if last_t is not None:
                            pend_int += last_pend * (t - last_t) / 1000.0
                        last_t, last_pend = t, pend
                        if pend > 0 and first_pend_ms is None:
                            first_pend_ms = t
                        cmax = max(cmax, cp); smax = max(smax, sp); spmax = max(spmax, spp)
                        for v in (cp, sp, spp):
                            if v > 0:
                                propvals[v] += 1
                        pr = max(cp, sp, spp)
                        if pr > 0:
                            nprop += 1
                        if fr > 0:
                            nfin += 1
                            fmin = fr if fmin is None else min(fmin, fr)
                            fmax = fr if fmax is None else max(fmax, fr)
                            r = (fr * 8 + 500) // 1000
                            if r >= 1:
                                nr1 += 1
                            if r >= 2:
                                nr2 += 1
                res.append(dict(lane=lane, cell=cell, arm=arm, rep=rep, n=n, nprop=nprop, nfin=nfin, nr1=nr1, nr2=nr2,
                                fmin=fmin, fmax=fmax, cmax=cmax, smax=smax, spmax=spmax,
                                first_pend_ms=first_pend_ms, pend_int_s=round(pend_int, 3)))
with open(os.path.join(OUT, "vts.csv"), "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=list(res[0].keys()))
    w.writeheader()
    for r in res:
        w.writerow(r)
print("runs", len(res))
print("nonzero prop values (top):", propvals.most_common(10), "total", sum(propvals.values()))
