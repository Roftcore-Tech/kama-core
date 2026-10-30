#!/usr/bin/env python3
"""[KTP2-F0 · Ajan C] TAM analiz eki — `tam_c.py`nin kosular.csv'sinden T.19–T.21 (Markdown, stdout).

T.19  taban ↔ aday kilit: Cochran–Mantel–Haenszel (11 N = 30 hücre tabaka), kol başına.
T.20  rs ↔ arq kilit: Fisher (hücre başına), havuz.
T.21  Parite katkısının konuşma içinde değişimi: ort(çeyrek) − R, r12k hücreleri (taban + aday).

Kullanım:
  python3 -I scripts/ktp2/f0/c/tam_c_ek.py reports/ktp2/f0/c/tam/kosular.csv >> reports/ktp2/f0/c/tam/tablolar.md
"""

import csv
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from tam_c import cell_info, f, fisher, fm  # noqa: E402

KOLLAR = ["arq", "uyar", "rs2", "rs4", "tekrar"]


def main():
    rows = list(csv.DictReader(open(sys.argv[1])))
    g = {}
    for r in rows:
        g.setdefault(r["hucre"], {}).setdefault(r["kol"], []).append(r)

    def birincil(c):
        i = cell_info(c)
        return i["on"] == "c" and i["kayip"] < 50 and len(g[c].get("arq_taban", [])) >= 30

    def sira(c):
        i = cell_info(c)
        return (i["hiz"] != "r12k", i["gec"] != "d300", i["kayip"], i["demet"])

    bi = sorted((c for c in g if birincil(c)), key=sira)

    def kil(c, kol):
        return sum(int(r.get("kilitli") or 0) for r in g[c][kol]), len(g[c][kol])

    P = print
    P("\n## T.19 taban ↔ aday kilit: Cochran–Mantel–Haenszel (11 N = 30 hücre tabaka)\n")
    P("Tabaka = hücre. OR > 1: aday daha sık kilitli. Süreklilik düzeltmeli χ² (1 sd). Aile = 5 kol.\n")
    P("| kol | havuz taban | havuz aday | OR_MH | χ² | p |\n|---|---|---|---|---|---|")
    for k in KOLLAR:
        pay = payda = so = se = sv = 0.0
        tt = ta = n = 0
        for c in bi:
            a, n1 = kil(c, f"{k}_aday")
            b = n1 - a
            d_, n2 = kil(c, f"{k}_taban")
            d = n2 - d_
            c_ = d_
            t = n1 + n2
            tt += d_
            ta += a
            n += n1
            pay += a * d / t
            payda += b * c_ / t
            so += a
            m1 = a + c_
            m0 = b + d
            se += n1 * m1 / t
            sv += n1 * n2 * m1 * m0 / (t * t * (t - 1)) if t > 1 else 0
        x = (max(abs(so - se) - 0.5, 0.0) ** 2) / sv if sv else 0.0
        p = math.erfc(math.sqrt(x / 2))
        orv = pay / payda if payda else None
        P(f"| {k} | {tt}/{n} | {ta}/{n} | {fm(orv)} | {fm(x)} | {p:.2g} |".replace(".", ","))

    P("\n## T.20 rs ↔ arq kilit (Fisher, hücre başına)\n")
    P("| hücre | ikili | arq | rs2 | p | rs4 | p |\n|---|---|---|---|---|---|---|")
    top = {}
    for c in bi:
        for b in ("taban", "aday"):
            ka, na = kil(c, f"arq_{b}")
            k2, n2 = kil(c, f"rs2_{b}")
            k4, n4 = kil(c, f"rs4_{b}")
            for key, v in (("arq", ka), ("rs2", k2), ("rs4", k4)):
                top[key] = top.get(key, 0) + v
            P(f"| {c} | {b} | {ka}/{na} | {k2}/{n2} | {fisher(ka, na - ka, k2, n2 - k2):.2g} | {k4}/{n4} | {fisher(ka, na - ka, k4, n4 - k4):.2g} |".replace(".", ","))
    P(f"\nHavuz (660 koşu/kol, betimsel): arq {top['arq']}, rs2 {top['rs2']}, rs4 {top['rs4']}.")

    P("\n## T.21 Parite katkısının konuşma içinde değişimi (r12k, ortalama çeyrek oranı − R; taban + aday)\n")
    P("R = ort(1 − p_ileri) aynı kol. Ç1 ≈ ilk 5 s, Ç4 ≈ son 5 s. Payda = koşu.\n")
    P("| hücre | kol | R | Ç1 − R | Ç2 − R | Ç3 − R | Ç4 − R |\n|---|---|---|---|---|---|---|")
    for c in bi:
        if cell_info(c)["hiz"] != "r12k":
            continue
        for k in ("arq", "rs2", "rs4"):
            rs = g[c][f"{k}_taban"] + g[c][f"{k}_aday"]
            ps = [f(r.get("p_ileri")) for r in rs if f(r.get("p_ileri")) is not None]
            R = 1 - sum(ps) / len(ps)
            cols = []
            for i in (1, 2, 3, 4):
                v = [f(r.get(f"c{i}")) for r in rs if f(r.get(f"c{i}")) is not None]
                cols.append(fm(sum(v) / len(v) - R, 3))
            P(f"| {c} | {k} | {fm(R, 3)} | " + " | ".join(cols) + " |")
    return 0


if __name__ == "__main__":
    sys.exit(main())
