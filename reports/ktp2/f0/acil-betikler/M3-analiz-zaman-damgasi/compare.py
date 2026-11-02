#!/usr/bin/env python3
"""rec.tsv (bağımsız hesap) ↔ B kosular_b.csv.gz ↔ D kosular.csv koşu-başı karşılaştırma + hücre medyanları.
Kullanım: python3 -I compare.py rec.tsv B_csv_gz D_csv
"""
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


def med(v):
    v = [x for x in v if x is not None]
    return statistics.median(v) if v else None


rec = list(csv.DictReader(open(sys.argv[1]), delimiter="\t"))
cells = sorted({r["cell"] for r in rec})
B = {}
with gzip.open(sys.argv[2], "rt") as fh:
    for r in csv.DictReader(fh):
        if r["cell"] in cells:
            B[(r["cell"], r["arm"], r["rep"])] = r
Dd = {}
for r in csv.DictReader(open(sys.argv[3])):
    if r["cell"] in cells:
        Dd[(r["cell"], r["arm"], r["rep"])] = r

pairs = [  # (benim, B sütunu, D sütunu)
    ("snd_cD", "deliv_D", "cD_ss"),
    ("snd_p95_del", "conf_p95_s", None),
    ("snd_p95_nr", None, "lat_p95_s"),
    ("rx_cD", "rx_deliv_D", "cD_ow"),
    ("rx_p95_del", "rx_ow_p95_s", None),
    ("rx_p95_nr", None, "ow_p95_s"),
]
print("== koşu-başı uyuşmazlık (|fark| > 1e-3) ==")
mism = 0
tot = 0
for r in rec:
    k = (r["cell"], r["arm"], r["rep"])
    b, d = B.get(k), Dd.get(k)
    for mine, bc, dc in pairs:
        m = f(r[mine])
        for src, row, col in (("B", b, bc), ("D", d, dc)):
            if col is None:
                continue
            if row is None:
                print("satır yok", src, k)
                continue
            o = f(row.get(col))
            tot += 1
            if m is None and o is None:
                continue
            if (m is None) != (o is None) or (math.isinf(m) != math.isinf(o)) or (not math.isinf(m) and abs(m - o) > 1e-3 * max(1, abs(m))):
                mism += 1
                if True:
                    print("UYUŞMAZ", src, k, mine, m, col, o)
print(f"karşılaştırma {tot}, uyuşmaz {mism}")

print()
print("== hücre × kol medyanları (benim) ==")
hdr = ["cell", "arm", "N", "snd_cD", "rx_cD", "snd_p95_del", "snd_p95_cens", "rx_p95_del", "rx_p95_cens",
       "ret_path_med", "n_t0u0"]
print("\t".join(hdr))
for c in cells:
    for a in ("taban", "aday", "quinn", "tcp"):
        rr = [r for r in rec if r["cell"] == c and r["arm"] == a]
        if not rr:
            continue
        row = [c, a, len(rr)]
        for k in ("snd_cD", "rx_cD", "snd_p95_del", "snd_p95_nr_cens", "rx_p95_del", "rx_p95_nr_cens", "rx_minus_snd_med"):
            row.append(med([f(r[k]) for r in rr]))
        row.append(sum(int(r["t0u0"]) for r in rr))
        print("\t".join(f"{x:.3f}" if isinstance(x, float) else str(x) for x in row))
