#!/usr/bin/env python3
"""32 birincil hücrede Kama−quinn p95 farkının tanıma duyarlılığı + kısa son tarih teslimi (CSV'lerden;
CSV'ler 5 hücrede ham veriyle birebir doğrulandı). Kullanım: python3 -I allcells.py B_csv_gz D_csv"""
import csv
import gzip
import math
import statistics
import sys


def f(x):
    if x in ("", None):
        return None
    if x == "inf":
        return math.inf
    try:
        return float(x)
    except ValueError:
        return None


B = {}
with gzip.open(sys.argv[1], "rt") as fh:
    for r in csv.DictReader(fh):
        B.setdefault((r["cell"], r["arm"]), []).append(r)
D = {}
for r in csv.DictReader(open(sys.argv[2])):
    D.setdefault((r["cell"], r["arm"]), []).append(r)

cells = sorted({c for c, a in D if c.startswith("b_") and (c, "quinn") in D and (c, "taban") in D
                and len(D[(c, "quinn")]) >= 30 and not any(f"_l{s}_" in c for s in (50, 80))})
print("birincil hücre", len(cells))
cls = {}
for c in cells:
    for r in D[(c, "quinn")]:
        cls[r["class"]] = cls.get(r["class"], 0) + 1
print("quinn sınıfları (birincil)", cls)


def mcol(rows, col, cap=None):
    v = [f(r[col]) for r in rows]
    v = [x for x in v if x is not None]
    if cap is not None:
        v = [min(x, cap) for x in v]
    return statistics.median(v) if v else None


sign = {"snd_cens": [0, 0], "rx_cens": [0, 0], "rx_del": [0, 0], "snd_del": [0, 0]}
print("hücre\tD\tΔsnd_cens(taban-q)\tΔrx_cens\tΔrx_del(B)\tΔsnd_del(B)\tteslim5s t/q\tteslim10s t/q\tteslim20s t/q")
for c in cells:
    dl = f(D[(c, "taban")][0]["deadline_s"])
    out = []
    for key, src, col, cap in (("snd_cens", D, "lat_p95_s", dl), ("rx_cens", D, "ow_p95_s", dl),
                               ("rx_del", B, "rx_ow_p95_s", None), ("snd_del", B, "conf_p95_s", None)):
        a, q = mcol(src[(c, "taban")], col, cap), mcol(src[(c, "quinn")], col, cap)
        d = a - q if a is not None and q is not None else None
        out.append(d)
        if d is not None:
            sign[key][0 if d > 0 else 1] += 1
    sh = []
    for col in ("deliv_5s", "deliv_10s", "deliv_20s"):
        sh.append(f"{mcol(B[(c, 'taban')], col):.2f}/{mcol(B[(c, 'quinn')], col):.2f}")
    print(f"{c[2:]}\t{dl:g}\t" + "\t".join(f"{x:+.2f}" for x in out) + "\t" + "\t".join(sh))
print("işaret (Kama büyük, Kama küçük) medyan farkı, tanıma göre:", sign)
