#!/usr/bin/env python3
"""[FEC-RISK KISA-AKIŞ] Çözümleyici.

DİSİPLİN:
 * "asla" (olay hiç olmadı) YAPAY BÜYÜK DAMGAYA çevrilmez; ayrı sentinel olarak taşınır ve
   medyanlardan DIŞLANIR, ama oran olarak AYRICA raporlanır (gizlenmez).
 * "parity hiç basılmadı" koşuları ortalama içinde ERİTİLMEZ; kendi oranıyla gösterilir.
 * Birim = REP. Merkezî eğilim = medyan (n küçük).
 * Keşfedilen HER hücre raporlanır — sessiz kol düşürme yok.
"""
import re, sys, glob, statistics

NEVER = None

def parse(path):
    t = open(path, errors="replace").read()
    rt = re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)", t)
    rp = re.search(r"risk_ppm=\((\d+), (\d+)\)", t)
    dl = re.search(r"delivered=(\d+)/(\d+)", t)
    fe = re.search(r"flow_end=(\d+)", t)
    if not (dl and rt and fe):
        return None
    g = lambda p, d=0: int(re.search(p, t).group(1)) if re.search(p, t) else d
    z = lambda v: (v if v else NEVER)  # 0 damgası = olay HİÇ olmadı
    return dict(
        delivered=int(dl.group(1)), offered=int(dl.group(2)),
        flow_end=int(fe.group(1)),
        t_susp=z(int(rt.group(1))), t_conf=z(int(rt.group(2))),
        t_risk=z(int(rt.group(3))), t_parity=z(int(rt.group(4))),
        risk_peak=int(rp.group(2)) if rp else 0,
        conf_peak=g(r"ledger_peak=(\d+)"),
        fec=g(r"fec=(\d+) "), retx=g(r"retx=(\d+) "), tx=g(r"tx=(\d+)B"),
        spurious=g(r"spurious=(\d+)"),
        arb=[int(x) for x in re.search(r"arb=\[([0-9, ]*)\]", t).group(1).split(",")] if re.search(r"arb=\[", t) else [0]*7,
        lvl=[int(x) for x in re.search(r"lvl=\[([0-9, ]*)\]", t).group(1).split(",")] if re.search(r"lvl=\[", t) else [0]*3,
        spec=[int(x) for x in re.search(r"spec=\(([0-9, ]*)\)", t).group(1).split(",")] if re.search(r"spec=\(", t) else [0]*5,
    )

def ms(v):
    return "asla" if v is None else f"{v/1e6:.0f}"

def before(ev, end):
    return "-" if ev is None else ("EVET" if ev < end else "hayır")

def med(vals):
    v = [x for x in vals if x is not None]
    return statistics.median(v) if v else None

root = sys.argv[1]
cells = {}
for f in sorted(glob.glob(f"{root}/*/rep*/c.err")):
    parts = f.split("/")
    cell, rep = parts[-3], parts[-2]
    r = parse(f)
    if r:
        r["rep"] = rep
        cells.setdefault(cell, []).append(r)

def loss_of(c):
    return int(re.search(r"l(\d+)_", c).group(1))
def flag_of(c):
    return "ON" if c.endswith("_r1") else "OFF"

order = sorted(cells, key=lambda c: (loss_of(c), flag_of(c)))

print("=" * 132)
print("HAM TABLO — kısa akış (96 mesaj).  Damgalar ns→ms, akış başlangıcı = 0.  'asla' = olay hiç olmadı")
print("=" * 132)
hdr = (f"{'kayıp':>5s} {'kol':>4s} {'rep':>5s} {'süre':>7s} {'kuşku':>7s} {'risk':>7s} {'onay':>7s} "
       f"{'parity':>7s} {'risk<son':>9s} {'onay<son':>9s} {'par<son':>8s} {'parity_B':>9s} "
       f"{'retx_B':>7s} {'tx_B':>7s}")
print(hdr); print("-" * 132)
for c in order:
    for r in sorted(cells[c], key=lambda x: x["rep"]):
        print(f"{loss_of(c):>4d}% {flag_of(c):>4s} {r['rep']:>5s} {r['flow_end']/1e6:>7.0f} "
              f"{ms(r['t_susp']):>7s} {ms(r['t_risk']):>7s} {ms(r['t_conf']):>7s} {ms(r['t_parity']):>7s} "
              f"{before(r['t_risk'], r['flow_end']):>9s} {before(r['t_conf'], r['flow_end']):>9s} "
              f"{before(r['t_parity'], r['flow_end']):>8s} {r['fec']:>9d} {r['retx']:>7d} {r['tx']:>7d}")

print()
print("=" * 132)
print("ÖZET — oranlar payda = o hücrenin rep sayısı.  'X/n' biçimi: gizlenen koşu YOK")
print("=" * 132)
hdr2 = (f"{'kayıp':>5s} {'kol':>4s} {'n':>2s} {'risk>0':>7s} {'onay>0':>7s} {'parity>0':>9s} "
        f"{'risk<son':>9s} {'onay<son':>9s} {'PAR<SON':>8s} {'med_süre':>9s} {'med_risk':>9s} "
        f"{'med_onay':>9s} {'med_par':>8s} {'risk_ppm':>9s} {'med_par_B':>10s} {'med_tx_B':>9s}")
print(hdr2); print("-" * 132)
for c in order:
    rs = cells[c]
    n = len(rs)

    def cnt(k, rs=rs):
        return sum(1 for r in rs if r[k] is not None)

    def bef(k, rs=rs):
        return sum(1 for r in rs if r[k] is not None and r[k] < r["flow_end"])

    def mr(k, rs=rs):
        m = med([r[k] for r in rs])
        return "asla" if m is None else "%.0f" % (m / 1e6)

    c_susp = "%d/%d" % (cnt("t_susp"), n)
    c_conf = "%d/%d" % (cnt("t_conf"), n)
    c_par = "%d/%d" % (cnt("t_parity"), n)
    b_risk = "%d/%d" % (bef("t_risk"), n)
    b_conf = "%d/%d" % (bef("t_conf"), n)
    b_par = "%d/%d" % (bef("t_parity"), n)
    dur = statistics.median([r["flow_end"] for r in rs]) / 1e6
    rpk = statistics.median([r["risk_peak"] for r in rs])
    pby = statistics.median([r["fec"] for r in rs])
    tby = statistics.median([r["tx"] for r in rs])
    print("%4d%% %4s %2d %7s %7s %9s %9s %9s %8s %9.0f %9s %9s %8s %9.0f %10.0f %9.0f"
          % (loss_of(c), flag_of(c), n, c_susp, c_conf, c_par, b_risk, b_conf, b_par,
             dur, mr("t_risk"), mr("t_conf"), mr("t_parity"), rpk, pby, tby))

print()
print("=" * 132)
print("KIYAS 3 — KISA-AKIŞ KANITI:  risk < akış_sonu < onay  (tahmincinin varoluş gerekçesi)")
print("=" * 132)
tot_on = tot_hit = 0
for c in order:
    if flag_of(c) != "ON":
        continue
    rs = cells[c]
    hit = sum(1 for r in rs
              if r["t_risk"] is not None and r["t_risk"] < r["flow_end"]
              and (r["t_conf"] is None or r["t_conf"] > r["flow_end"]))
    if loss_of(c) > 0:
        tot_on += len(rs); tot_hit += hit
    print(f"  kayıp {loss_of(c):>2d}%  ON:  {hit}/{len(rs)} rep'te  risk < akış_sonu < onay")
print(f"\n  TOPLAM (kayıplı hücreler): {tot_hit}/{tot_on}")

print()
print("=" * 132)
print("KIYAS 1 & 2 — tahminci gecikmesi ve kanonik onay ufku (ARQ semantiği değişmedi mi?)")
print("=" * 132)
d_risk, d_conf = [], []
for c in order:
    for r in cells[c]:
        if r["t_risk"] is not None and r["t_susp"] is not None:
            d_risk.append(r["t_risk"] - r["t_susp"])
        if r["t_conf"] is not None and r["t_susp"] is not None:
            d_conf.append(r["t_conf"] - r["t_susp"])
if d_risk:
    print(f"  KIYAS 1  t_risk − t_kuşku : n={len(d_risk)}  min={min(d_risk)/1e6:.3f} ms  "
          f"medyan={statistics.median(d_risk)/1e6:.3f} ms  max={max(d_risk)/1e6:.3f} ms")
if d_conf:
    print(f"  KIYAS 2  t_onay − t_kuşku : n={len(d_conf)}  min={min(d_conf)/1e6:.0f} ms  "
          f"medyan={statistics.median(d_conf)/1e6:.0f} ms  max={max(d_conf)/1e6:.0f} ms")

print()
print("=" * 132)
print("ARBİTER — hücre başına sebep histogramı (toplam) · seviye (L0/L1/L2) · persistent tepe · reorder baskısı tepe")
print("  sebep sırası: None · ConfFuzzy · ConfPolicy · StructGate · SpecBoot · SpecPersist · DeadlineAsst")
print("=" * 132)
for c in order:
    rs = cells[c]
    arb = [sum(r["arb"][i] for r in rs) for i in range(7)]
    lvl = [sum(r["lvl"][i] for r in rs) for i in range(3)]
    pers = statistics.median([r["spec"][0] for r in rs])
    rpk = max(r["spec"][2] for r in rs)
    print("%4d%% %4s  arb=%-40s lvl=%-22s pers_peak=%7.0f rp_peak=%4d‰" % (loss_of(c), flag_of(c), arb, lvl, pers, rpk))

print()
print("=" * 132)
print("WIRE AMPLİFİKASYONU — ON toplam tx / OFF toplam tx (medyan), hücre başına")
print("=" * 132)
by_loss = {}
for c in order:
    by_loss.setdefault(loss_of(c), {})[flag_of(c)] = statistics.median([r["tx"] for r in cells[c]])
for L in sorted(by_loss):
    if "OFF" in by_loss[L] and "ON" in by_loss[L] and by_loss[L]["OFF"]:
        amp = (by_loss[L]["ON"] - by_loss[L]["OFF"]) / by_loss[L]["OFF"] * 100
        print("  seviye %2d%%  OFF=%7.0f B  ON=%7.0f B  fark=%+5.2f%%" % (L, by_loss[L]["OFF"], by_loss[L]["ON"], amp))
