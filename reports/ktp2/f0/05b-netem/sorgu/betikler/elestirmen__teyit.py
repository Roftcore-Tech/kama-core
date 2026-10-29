#!/usr/bin/env python3
"""Eleştirmen şeridi — bağımsız teyit (salt okunur).

Analiz betiğinden FARKLI algoritma: bağ grupları üzerinde (x_kullanılan, 2U) DP,
kesin rasyonel (Fraction). Olaylar her satırın tam JSON ayrıştırmasıyla okunur.
Kullanım: python3 -I teyit.py RUN_DIR PROFIL
"""
import json
import pathlib
import sys
from fractions import Fraction
from math import comb

D_NS = 20 * 10**9


def m1_of(f):
    n = ok = 0
    for line in open(f):
        line = line.strip()
        if not line:
            continue
        o = json.loads(line)
        if "event" not in o:
            continue
        e = o["event"]
        n += 1
        d = e.get("delivered_ns")
        if d is not None and d - e["sent_ns"] <= D_NS:
            ok += 1
    return Fraction(ok, n), n


def median(v):
    s = sorted(v)
    k = len(s)
    return s[k // 2] if k % 2 else (s[k // 2 - 1] + s[k // 2]) / 2


def mwu_groups(x, y):
    m, n = len(x), len(y)
    vals = sorted(set(x) | set(y))
    groups = [(sum(1 for v in x if v == u), sum(1 for v in y if v == u)) for u in vals]
    # gözlenen 2U_x
    obs2 = 0
    ybelow = 0
    for ax, by in groups:
        obs2 += 2 * ax * ybelow + ax * by
        ybelow += by
    # null DP: durum (x_kullanılan, 2U) -> sayı; y_below = toplam_önce - x_kullanılan
    dp = {(0, 0): 1}
    seen = 0
    for ax, by in groups:
        t = ax + by
        nd = {}
        for (xu, u2), c in dp.items():
            yb = seen - xu
            for a in range(0, t + 1):
                if xu + a > m or (t - a) + yb > n:
                    continue
                b = t - a
                key = (xu + a, u2 + 2 * a * yb + a * b)
                nd[key] = nd.get(key, 0) + c * comb(t, a)
        dp = nd
        seen += t
    dist = {u2: c for (xu, u2), c in dp.items() if xu == m}
    tot = sum(dist.values())
    assert tot == comb(m + n, m)
    mean2 = m * n  # E[2U] = m*n
    dev = abs(obs2 - mean2)
    tail = sum(c for u2, c in dist.items() if abs(u2 - mean2) >= dev)
    return Fraction(obs2, 2), Fraction(tail, tot)


def main():
    root = pathlib.Path(sys.argv[1])
    prof = sys.argv[2]
    arms = {}
    for arm in ("taban", "aday"):
        rows = []
        for d in sorted((root / prof / arm).glob("rep*"), key=lambda q: int(q.name[3:])):
            v, n = m1_of(d / "out.jsonl")
            rows.append((int(d.name[3:]), v, n))
        arms[arm] = rows
        print(arm, "N=", len(rows), "olay:", sorted(set(r[2] for r in rows)))
        print("  rep->M1:", " ".join(f"r{r}={float(v):.2f}" for r, v, _ in rows))
    t = [v for _, v, _ in arms["taban"]]
    a = [v for _, v, _ in arms["aday"]]
    mt, ma = median(t), median(a)
    u, p = mwu_groups(a, t)
    alpha = Fraction(5, 700)
    dmed = ma - mt
    print(f"med taban={float(mt):.4f} ({mt}) med aday={float(ma):.4f} ({ma}) dmed={float(dmed):+.4f}")
    print(f"U_aday={float(u)} p={float(p):.8f} = {p.numerator}/{p.denominator}  alpha={float(alpha):.8f} p<alpha={p < alpha}")
    print(f"KAYMA-M1={abs(dmed) >= Fraction(5, 100) and p < alpha}")


if __name__ == "__main__":
    main()
