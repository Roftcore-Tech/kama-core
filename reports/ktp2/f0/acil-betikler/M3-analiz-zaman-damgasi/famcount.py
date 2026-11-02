#!/usr/bin/env python3
"""Aile sayımı (taban/aday − quinn, p95) dört tanımla: gönderen teyidi sansürlü (D'nin), alıcı tek-yön
sansürlü, alıcı yalnız-teslim (B'nin), gönderen yalnız-teslim. Bootstrap B=10000, sabit tohum (kendi).
Kullanım: python3 -I famcount.py B_csv_gz D_csv"""
import csv
import gzip
import hashlib
import math
import random
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
        B[(r["cell"], r["arm"], r["rep"])] = r
D = {}
for r in csv.DictReader(open(sys.argv[2])):
    D.setdefault((r["cell"], r["arm"]), []).append(r)
cells = sorted({c for c, a in D if c.startswith("b_") and (c, "quinn") in D and len(D[(c, "quinn")]) >= 30
                and not any(f"_l{s}_" in c for s in (50, 80))})
NB = 10000


def vals(c, arm, kind):
    out = []
    for r in D[(c, arm)]:
        dl = f(r["deadline_s"])
        b = B.get((c, arm, r["rep"]))
        if kind == "snd_cens":
            v = f(r["lat_p95_s"])
            out.append(min(v, dl) if v is not None else dl)
        elif kind == "rx_cens":
            v = f(r["ow_p95_s"])
            out.append(min(v, dl) if v is not None else dl)
        elif kind == "rx_del":
            v = f(b.get("rx_ow_p95_s")) if b else None
            if v is not None:
                out.append(v)
        elif kind == "snd_del":
            v = f(b.get("conf_p95_s")) if b else None
            if v is not None:
                out.append(v)
    return out


for arm in ("taban", "aday"):
    for kind in ("snd_cens", "rx_cens", "rx_del", "snd_del"):
        cnt = {"düz": [0, 0, 0], "bonf": [0, 0, 0]}
        m = len(cells)
        for c in cells:
            a, q = vals(c, arm, kind), vals(c, "quinn", kind)
            rng = random.Random(int(hashlib.sha256(f"M3|{c}|{arm}|{kind}".encode()).hexdigest()[:12], 16))
            ds = sorted(statistics.median(rng.choices(a, k=len(a))) - statistics.median(rng.choices(q, k=len(q)))
                        for _ in range(NB))
            for tag, al in (("düz", 0.05), ("bonf", 0.05 / m)):
                lo, hi = ds[int(al / 2 * NB)], ds[min(NB - 1, int((1 - al / 2) * NB))]
                i = 1 if lo > 0 else (0 if hi < 0 else 2)  # 0 = Kama düşük (iyi), 1 = quinn düşük
                cnt[tag][i] += 1
        print(f"{arm} − quinn {kind}: m={m} düz Kama-düşük/quinn-düşük/= {cnt['düz']}  Bonf {cnt['bonf']}")
