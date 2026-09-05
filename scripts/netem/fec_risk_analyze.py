#!/usr/bin/env python3
"""[FEC-RISK] Dört deney ailesinin çözümleyicisi.

Birim = REP. Merkezî eğilim = MEDYAN (küçük n; ortalama tek koşuya duyarlı).
Her kol için KEŞFEDİLEN her hücre raporlanır — sessiz kol düşürme YOK.
"""
import re, sys, glob, os, statistics

def parse(path):
    t = open(path, errors="replace").read()
    g = lambda p, d=None: (re.search(p, t).group(1) if re.search(p, t) else d)
    rt = re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)", t)
    rp = re.search(r"risk_ppm=\((\d+), (\d+)\)", t)
    dl = re.search(r"delivered=(\d+)/(\d+)", t)
    if not dl:
        return None
    return dict(
        delivered=int(dl.group(1)), offered=int(dl.group(2)),
        tx=int(g(r"tx=(\d+)B", 0)), data=int(g(r"data=(\d+) ", 0)),
        retx=int(g(r"retx=(\d+) ", 0)), fec=int(g(r"fec=(\d+) ", 0)),
        repairs=int(g(r"fec_repairs_tx=(\d+)", 0)),
        spurious=int(g(r"spurious=(\d+)", 0)),
        conf_peak=int(g(r"ledger_peak=(\d+)", 0)),
        gate=int(g(r"gate_opened=(\d+)", 0)),
        t_susp=int(rt.group(1)) if rt else 0, t_conf=int(rt.group(2)) if rt else 0,
        t_risk=int(rt.group(3)) if rt else 0, t_parity=int(rt.group(4)) if rt else 0,
        risk_peak=int(rp.group(2)) if rp else 0,
    )

def med(v):
    v = [x for x in v if x is not None]
    return statistics.median(v) if v else float("nan")

def ms(ns):
    return round(ns / 1e6) if ns else None

def collect(root, fam):
    cells = {}
    for f in sorted(glob.glob(f"{root}/{fam}/*/rep*/c.err")):
        cell = f.split("/")[-3]
        r = parse(f)
        if r:
            cells.setdefault(cell, []).append(r)
    return cells

def lat(r):
    """FEC tepki gecikmesi: ilk parity − ilk kuşku (ms). Parity yoksa None."""
    if r["t_parity"] and r["t_susp"]:
        return (r["t_parity"] - r["t_susp"]) / 1e6
    return None

root = sys.argv[1]

# ── AİLE 1 ────────────────────────────────────────────────────────────────────────────────
print("=" * 104)
print("AİLE 1 — kayıp, reorder YOK.  Soru: FEC tepki gecikmesi (ilk parity − ilk kuşku)")
print("=" * 104)
c = collect(root, "f1")
print(f"{'hücre':10s} {'n':>2s} {'risk':>4s} {'teslim':>7s} {'conf_ppm':>9s} {'risk_ppm':>9s} "
      f"{'t_kuşku':>8s} {'t_onay':>8s} {'t_risk':>8s} {'t_parity':>9s} {'GECİKME_ms':>11s} {'parity_B':>9s}")
print("-" * 104)
for cell in sorted(c, key=lambda x: (int(re.search(r'l(\d+)', x).group(1)), x)):
    rs = c[cell]
    risk_on = cell.endswith("_r1")
    lats = [lat(r) for r in rs]
    print(f"{cell:10s} {len(rs):>2d} {'ON' if risk_on else 'OFF':>4s} "
          f"{med([r['delivered'] for r in rs]):>7.0f} {med([r['conf_peak'] for r in rs]):>9.0f} "
          f"{med([r['risk_peak'] for r in rs]):>9.0f} "
          f"{str(ms(int(med([r['t_susp'] for r in rs])))):>8s} "
          f"{str(ms(int(med([r['t_conf'] for r in rs])))):>8s} "
          f"{str(ms(int(med([r['t_risk'] for r in rs])))):>8s} "
          f"{str(ms(int(med([r['t_parity'] for r in rs])))):>9s} "
          f"{(f'{med(lats):.0f}' if any(x is not None for x in lats) else 'PARITY YOK'):>11s} "
          f"{med([r['fec'] for r in rs]):>9.0f}")

# ── AİLE 2 / 3 ────────────────────────────────────────────────────────────────────────────
for fam, title in (("f2", "AİLE 2 — reorder, kayıp YOK.  Soru: yanlış-pozitif parity BEDELİ"),
                   ("f3", "AİLE 3 — kayıp + reorder.  Soru: gerçekçi zor hâlde erken tepki vs bedel")):
    c = collect(root, fam)
    if not c:
        continue
    print()
    print("=" * 104); print(title); print("=" * 104)
    print(f"{'hücre':14s} {'n':>2s} {'risk':>4s} {'teslim':>7s} {'spurious':>9s} {'risk_ppm':>9s} "
          f"{'conf_ppm':>9s} {'parity_B':>9s} {'retx_B':>8s} {'toplam_tx':>10s} {'GECİKME_ms':>11s}")
    print("-" * 104)
    for cell in sorted(c):
        rs = c[cell]
        lats = [lat(r) for r in rs]
        print(f"{cell:14s} {len(rs):>2d} {'ON' if cell.endswith('_r1') else 'OFF':>4s} "
              f"{med([r['delivered'] for r in rs]):>7.0f} {med([r['spurious'] for r in rs]):>9.0f} "
              f"{med([r['risk_peak'] for r in rs]):>9.0f} {med([r['conf_peak'] for r in rs]):>9.0f} "
              f"{med([r['fec'] for r in rs]):>9.0f} {med([r['retx'] for r in rs]):>8.0f} "
              f"{med([r['tx'] for r in rs]):>10.0f} "
              f"{(f'{med(lats):.0f}' if any(x is not None for x in lats) else 'PARITY YOK'):>11s}")

# ── AİLE 4 ────────────────────────────────────────────────────────────────────────────────
print()
print("=" * 104)
print("AİLE 4 — dönüş bandı çöküyor, kayıp YOK.  İNVARYANT: bant ≠ kayıp  (risk ≈ 0 OLMALI)")
print("=" * 104)
print(f"{'dönüş_bw':12s} {'teslim':>9s} {'risk_ppm':>9s} {'conf_ppm':>9s} {'parity_B':>9s} {'HÜKÜM':>28s}")
print("-" * 104)
for f in sorted(glob.glob(f"{root}/f4/*/c.err")):
    bw = os.path.basename(os.path.dirname(f)).replace("rev", "")
    r = parse(f)
    if not r:
        print(f"{bw:12s}  (kayıt yok)"); continue
    ok = r["risk_peak"] == 0 and r["fec"] == 0
    print(f"{bw:12s} {r['delivered']:>4d}/{r['offered']:<4d} {r['risk_peak']:>9d} {r['conf_peak']:>9d} "
          f"{r['fec']:>9d} {('KORUNDU' if ok else 'İHLAL — bant kayıp sanıldı'):>28s}")
