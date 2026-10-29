#!/usr/bin/env python3
"""[DUR-p2 / şerit: değişkenlik] TAMAMI BETİMSEL, SONRADAN. Ön-kayıtlı kuralı DEĞİŞTİRMEZ.

Girdi (salt-okunur): iki netem-7 oturumu
  A = run-a8c4e8c6  (taban 14b5f7d3 ikilisi @1b1425d2, aday b642a9ec @a8c4e8c6)
  B = run-49864510  (taban 14b5f7d3 ikilisi @1b1425d2, aday fd3089b1 @49864510)
M1/M2 tanımları ONKAYIT.md §2 ve netem7_analyze.py run_metrics() ile birebir:
  M1 = D=20 s içinde teslim / olay sayısı; M2 = teslim edilenlerin (sent→delivered) gecikme medyanı (ms).
Mann–Whitney: netem7_analyze.py mwu_exact_two_sided() ile aynı algoritma (bağ-düzeltmeli kesin
permütasyon, orta-sıra, iki yanlı). Burada üretilen p'ler KARARSAL DEĞİLDİR.
Çıktı yalnız stdout + bu şerit dizinindeki CSV.
"""
import csv
import json
import math
import pathlib
import statistics
import sys

ROOT = pathlib.Path("/home/ryuzaki/kama-k4g-work/k4g05b/netem7")
SESS = {"A": ROOT / "run-a8c4e8c6", "B": ROOT / "run-49864510"}
PROFILES = ["p1_loss50", "p2_loss80", "p3_loss95", "p4_jitter", "p5_revpath", "p6_blackout", "p7_bbr"]
D_NS = 20 * 10**9
ALPHA = 0.05 / 7
OUTDIR = pathlib.Path(sys.argv[1])
COLLAPSE = 0.1


def load_run(f):
    meta = None
    ev = []
    for line in open(f):
        if '"event"' in line:
            ev.append(json.loads(line)["event"])
        elif '"meta"' in line and meta is None:
            meta = json.loads(line)["meta"]
    if not ev:
        return None
    ok = sum(1 for e in ev if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D_NS)
    lat = [(e["delivered_ns"] - e["sent_ns"]) / 1e6 for e in ev if e.get("delivered_ns") is not None]
    act = (meta or {}).get("activation", {})
    return {"m1": ok / len(ev), "m2": statistics.median(lat) if lat else None, "n_ev": len(ev),
            "n_del": len(lat), "n_del_in_D": ok, "t0": (meta or {}).get("t0_unix_ns"),
            "srtt_us": act.get("srtt_us"), "btlbw_bps": act.get("btlbw_bps"),
            "retransmits": act.get("retransmits"), "recover_arq": act.get("recover_arq_entries"),
            "fwd_loss_pm": act.get("return_forward_loss_permille"),
            "ack_loss_pm": act.get("return_ack_loss_permille")}


def mwu_exact_two_sided(x, y):
    """netem7_analyze.py ile aynı: bağ-düzeltmeli kesin permütasyon (2×orta-sıra toplamı, DP)."""
    pooled = sorted([(v, 0) for v in x] + [(v, 1) for v in y], key=lambda t: t[0])
    n = len(pooled)
    ranks2 = [0] * n
    i = 0
    while i < n:
        j = i
        while j + 1 < n and pooled[j + 1][0] == pooled[i][0]:
            j += 1
        r2 = (i + 1) + (j + 1)
        for k in range(i, j + 1):
            ranks2[k] = r2
        i = j + 1
    m = len(x)
    obs = sum(r for r, (_, g) in zip(ranks2, pooled) if g == 0)
    dp = [dict() for _ in range(m + 1)]
    dp[0][0] = 1
    for r in ranks2:
        for k in range(m - 1, -1, -1):
            for s, c in dp[k].items():
                dp[k + 1][s + r] = dp[k + 1].get(s + r, 0) + c
    dist = dp[m]
    total = sum(dist.values())
    mean2 = m * (n + 1)
    dev = abs(obs - mean2)
    tail = sum(c for s, c in dist.items() if abs(s - mean2) >= dev)
    return tail / total


def fisher_two_sided(a, b, c, d):
    """2×2 [[a,b],[c,d]] kesin Fisher, iki yanlı (olasılığı gözlenenden ≤ olan tablolar)."""
    r1, r2, c1 = a + b, c + d, a + c
    n = r1 + r2

    def pr(x):
        return math.comb(r1, x) * math.comb(r2, c1 - x) / math.comb(n, c1)
    p0 = pr(a)
    lo, hi = max(0, c1 - r2), min(r1, c1)
    return sum(pr(x) for x in range(lo, hi + 1) if pr(x) <= p0 * (1 + 1e-9))


def q(v, p):
    """Tip-7 (doğrusal) kantil; v sıralı."""
    if not v:
        return float("nan")
    h = (len(v) - 1) * p
    lo = math.floor(h)
    hi = min(lo + 1, len(v) - 1)
    return v[lo] + (h - lo) * (v[hi] - v[lo])


def load_all():
    data = {}
    for s, root in SESS.items():
        for p in PROFILES:
            for arm in ("taban", "aday"):
                runs = []
                for f in sorted((root / p).glob(f"{arm}/rep*/out.jsonl"), key=lambda x: int(x.parent.name[3:])):
                    r = load_run(f)
                    rc = (f.parent / "rc").read_text().strip() if (f.parent / "rc").exists() else "?"
                    if r is None:
                        r = {"m1": None, "m2": None, "infra": True}
                    r["rep"] = int(f.parent.name[3:])
                    r["rc"] = rc
                    runs.append(r)
                data[(s, p, arm)] = runs
    return data


def vals(runs, key):
    return sorted(r[key] for r in runs if r.get(key) is not None)


def fmt(v, nd=3):
    if v is None or (isinstance(v, float) and math.isnan(v)):
        return "—"
    return f"{v:.{nd}f}"


def summarize(v):
    return f"med={fmt(statistics.median(v))} Q1={fmt(q(v,.25))} Q3={fmt(q(v,.75))} min={fmt(v[0])} max={fmt(v[-1])}"


def main():
    OUTDIR.mkdir(parents=True, exist_ok=True)
    data = load_all()

    # CSV: koşu başına
    with open(OUTDIR / "kosular.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["oturum", "profil", "kol", "rep", "rc", "m1", "m2_ms", "n_ev", "n_del", "n_del_in_D", "t0_unix_ns",
                    "srtt_us", "btlbw_bps", "retransmits", "recover_arq", "fwd_loss_pm", "ack_loss_pm"])
        for (s, p, arm), runs in sorted(data.items()):
            for r in runs:
                w.writerow([s, p, arm, r["rep"], r["rc"], r.get("m1"), r.get("m2"), r.get("n_ev"), r.get("n_del"),
                            r.get("n_del_in_D"), r.get("t0"), r.get("srtt_us"), r.get("btlbw_bps"),
                            r.get("retransmits"), r.get("recover_arq"), r.get("fwd_loss_pm"), r.get("ack_loss_pm")])

    print("# [betimsel, sonradan] DEĞİŞKENLİK — oturum A=run-a8c4e8c6, B=run-49864510")
    print("# p'ler KARARSAL DEĞİL (ön-kayıtlı kural yalnız taban↔aday, oturum içi).\n")

    # 0) enstrüman hijyeni
    print("## 0) rc / altyapı (koşu sayısı, rc dağılımı, olaysız koşu)")
    for (s, p, arm), runs in sorted(data.items()):
        rcs = {}
        for r in runs:
            rcs[r["rc"]] = rcs.get(r["rc"], 0) + 1
        infra = sum(1 for r in runs if r.get("infra"))
        nev = sorted(set(r.get("n_ev") for r in runs if not r.get("infra")))
        if rcs != {"0": 20} or infra or nev != [100]:
            print(f"   {s} {p:<12} {arm:<5} N={len(runs)} rc={rcs} olaysız={infra} n_ev={nev}")
    print("   (yukarıda satır yoksa: tüm hücrelerde N=20, rc=0, olaysız=0, n_ev=100)\n")

    # 1) aynı taban ikilisi iki oturumda; 2) aday kolları
    for title, arm in (("1) TABAN (aynı ikili 14b5f7d3) A↔B", "taban"),
                       ("2) ADAY A(a8c4e8c6)↔B(49864510) — OTURUM ETKİSİYLE KARIŞIK", "aday")):
        print(f"## {title}")
        print(f"   {'profil':<12} {'M1medA':>7} {'M1medB':>7} {'ΔB−A':>7} {'p_M1':>9} | {'M2medA':>8} {'M2medB':>8} {'B/A':>6} {'p_M2':>9}"
              f" | çöküşA çöküşB")
        for p in PROFILES:
            a = data[("A", p, arm)]
            b = data[("B", p, arm)]
            a1, b1 = vals(a, "m1"), vals(b, "m1")
            a2, b2 = vals(a, "m2"), vals(b, "m2")
            pm1 = mwu_exact_two_sided(b1, a1)
            pm2 = mwu_exact_two_sided(b2, a2)
            ma1, mb1 = statistics.median(a1), statistics.median(b1)
            ma2, mb2 = statistics.median(a2), statistics.median(b2)
            ca = sum(1 for v in a1 if v < COLLAPSE)
            cb = sum(1 for v in b1 if v < COLLAPSE)
            flag = " (<α/7 olurdu)" if pm1 < ALPHA or pm2 < ALPHA else ""
            print(f"   {p:<12} {ma1:7.3f} {mb1:7.3f} {mb1-ma1:+7.3f} {pm1:9.4g} | {ma2:8.0f} {mb2:8.0f} {mb2/ma2:6.3f} {pm2:9.4g}"
                  f" | {ca:>2}/20  {cb:>2}/20{flag}")
        print()

    # oturum içi taban↔aday (ANALIZ ile çapraz kontrol)
    print("## 1b) Oturum içi aday↔taban (ANALIZ.txt çapraz kontrolü; aynı algoritma)")
    for s in ("A", "B"):
        for p in PROFILES:
            t1, a1 = vals(data[(s, p, "taban")], "m1"), vals(data[(s, p, "aday")], "m1")
            t2, a2 = vals(data[(s, p, "taban")], "m2"), vals(data[(s, p, "aday")], "m2")
            print(f"   {s} {p:<12} M1 Δ={statistics.median(a1)-statistics.median(t1):+.3f} p={mwu_exact_two_sided(a1,t1):.4g}"
                  f"  M2 oran={statistics.median(a2)/statistics.median(t2):.3f} p={mwu_exact_two_sided(a2,t2):.4g}")
    print()

    # 3) p2 şekli
    p = "p2_loss80"
    print("## 3) p2_loss80 dört dağılım")
    cells = [(s, arm) for s in ("A", "B") for arm in ("taban", "aday")]
    for s, arm in cells:
        v = vals(data[(s, p, arm)], "m1")
        print(f"   {s} {arm:<5} M1 sıralı: {' '.join(f'{x:.2f}' for x in v)}")
    print()
    # histogram 0.05 genişlik
    print("   Histogram (M1 kutusu genişlik 0.1; [0,0.1) ... [0.9,1.0])")
    edges = [i / 10 for i in range(11)]
    print("   " + " " * 9 + " ".join(f"{e:>4.1f}" for e in edges[:-1]))
    for s, arm in cells:
        v = vals(data[(s, p, arm)], "m1")
        cnt = [0] * 10
        for x in v:
            cnt[min(int(x * 10 + 1e-9), 9)] += 1
        print(f"   {s} {arm:<5} " + " ".join(f"{c:>4}" for c in cnt))
    print()
    print("   Çöküş (M1 < 0.1) ve çöküşsüz M1:")
    nc = {}
    for s, arm in cells:
        v = vals(data[(s, p, arm)], "m1")
        col = [x for x in v if x < COLLAPSE]
        ok = [x for x in v if x >= COLLAPSE]
        nc[(s, arm)] = (len(col), ok)
        print(f"   {s} {arm:<5} çöküş={len(col)}/20 (değerler {' '.join(f'{x:.2f}' for x in col)})  "
              f"çöküşsüz n={len(ok)} {summarize(ok) if ok else ''}")
    # boşluk: 0.1 eşiğinin doğal olup olmadığı
    allv = sorted(x for s, arm in cells for x in vals(data[(s, p, arm)], "m1"))
    gap_lo = max((x for x in allv if x < COLLAPSE), default=None)
    gap_hi = min((x for x in allv if x >= COLLAPSE), default=None)
    print(f"   Havuzlu 80 koşuda 0.1 çevresindeki boşluk: en büyük <0.1 = {gap_lo}, en küçük ≥0.1 = {gap_hi}")
    # aralık 0.1–0.6 doluluğu
    mid = [x for x in allv if 0.1 <= x < 0.6]
    print(f"   Havuzlu 80 koşuda 0.1 ≤ M1 < 0.6 olan koşu sayısı: {len(mid)} → {' '.join(f'{x:.2f}' for x in mid)}")
    print()
    print("   Çöküş oranı farkı (Fisher kesin, iki yanlı) [betimsel]:")
    for (s1, a1), (s2, a2) in [(("A", "taban"), ("A", "aday")), (("B", "taban"), ("B", "aday")),
                               (("A", "taban"), ("B", "taban")), (("A", "aday"), ("B", "aday"))]:
        c1, c2 = nc[(s1, a1)][0], nc[(s2, a2)][0]
        print(f"   {s1} {a1:<5} {c1}/20  vs  {s2} {a2:<5} {c2}/20   p={fisher_two_sided(c1, 20-c1, c2, 20-c2):.4g}")
    # havuzlu (oturumlar birleşik) kol bazında
    ct = nc[("A", "taban")][0] + nc[("B", "taban")][0]
    ca = nc[("A", "aday")][0] + nc[("B", "aday")][0]
    print(f"   Havuzlu (A+B) taban {ct}/40 vs aday {ca}/40   p={fisher_two_sided(ct, 40-ct, ca, 40-ca):.4g}")
    print()
    print("   Çöküşsüz koşularda M1 düzey farkı (MWU kesin, iki yanlı) [betimsel]:")
    for (s1, a1), (s2, a2) in [(("A", "taban"), ("A", "aday")), (("B", "taban"), ("B", "aday")),
                               (("A", "taban"), ("B", "taban")), (("A", "aday"), ("B", "aday"))]:
        x, y = nc[(s1, a1)][1], nc[(s2, a2)][1]
        print(f"   {s1} {a1:<5} n={len(x)} med={statistics.median(x):.3f}  vs  {s2} {a2:<5} n={len(y)} med={statistics.median(y):.3f}"
              f"   Δmed={statistics.median(y)-statistics.median(x):+.3f} p={mwu_exact_two_sided(y, x):.4g}")
    xt = sorted(nc[("A", "taban")][1] + nc[("B", "taban")][1])
    xa = sorted(nc[("A", "aday")][1] + nc[("B", "aday")][1])
    print(f"   Havuzlu çöküşsüz taban n={len(xt)} med={statistics.median(xt):.3f} vs aday n={len(xa)} med={statistics.median(xa):.3f}"
          f"  Δmed={statistics.median(xa)-statistics.median(xt):+.3f} p={mwu_exact_two_sided(xa, xt):.4g}")
    print()
    # Ayrıştırma: medyan nereden geliyor? Karışım hesabı
    print("   Medyan ayrıştırması: çöküş sayısı medyanın hangi tarafına düşüyor")
    for s, arm in cells:
        v = vals(data[(s, p, arm)], "m1")
        print(f"   {s} {arm:<5} sıralı 10. ve 11. değer = {v[9]:.3f}, {v[10]:.3f} → medyan {statistics.median(v):.3f}; "
              f"çöküş={nc[(s, arm)][0]} (≥10 ise medyan çöküş kümesine yakın / sınırda)")
    print()
    # p2 M2 ve çöküş ilişkisi
    print("   p2 M2 (ms) — çöküşlü/çöküşsüz ayrı:")
    for s, arm in cells:
        runs = data[(s, p, arm)]
        col = sorted(r["m2"] for r in runs if r["m1"] is not None and r["m1"] < COLLAPSE and r["m2"] is not None)
        ok = sorted(r["m2"] for r in runs if r["m1"] is not None and r["m1"] >= COLLAPSE and r["m2"] is not None)
        nd = sorted(r["n_del"] for r in runs if r["m1"] is not None and r["m1"] < COLLAPSE)
        print(f"   {s} {arm:<5} çöküşlü n_M2={len(col)} med={statistics.median(col) if col else float('nan'):.0f} n_del={nd}  "
              f"çöküşsüz n={len(ok)} med={statistics.median(ok) if ok else float('nan'):.0f} Q1={q(ok,.25):.0f} Q3={q(ok,.75):.0f}")
    print()

    # zaman sırası: çöküşler oturum içinde kümeleniyor mu
    print("   p2 koşu sırası (rep, kol, M1) — çöküş = '*':")
    for s in ("A", "B"):
        line = []
        for rep in range(1, 21):
            for arm in ("taban", "aday"):
                r = next(x for x in data[(s, p, arm)] if x["rep"] == rep)
                line.append((r["t0"], arm, rep, r["m1"]))
        line.sort()
        print(f"   {s}: " + " ".join(f"{'T' if a=='taban' else 'A'}{rep}{'*' if m < COLLAPSE else ''}" for _, a, rep, m in line))
    print()

    # 4) p1/p5 M2 değişkenliği
    print("## 4) M2 (ms) oturumlar arası — p1_loss50, p5_revpath (ve tüm profiller için taban A↔B oranı)")
    for p in ("p1_loss50", "p5_revpath"):
        for s, arm in cells:
            runs = data[(s, p, arm)]
            v2 = vals(runs, "m2")
            okm2 = sorted(r["m2"] for r in runs if r["m2"] is not None and r["m1"] >= 0.99)
            low = [(r["rep"], round(r["m1"], 3), None if r["m2"] is None else round(r["m2"])) for r in runs if r["m1"] < 0.99]
            print(f"   {p} {s} {arm:<5} M2 n={len(v2)} med={statistics.median(v2):.0f} Q1={q(v2,.25):.0f} Q3={q(v2,.75):.0f} "
                  f"min={v2[0]:.0f} max={v2[-1]:.0f} | M1≥0.99 koşularda n={len(okm2)} med={statistics.median(okm2):.0f}"
                  f" | M1<0.99 koşular (rep,M1,M2)={low}")
        ta, tb = vals(data[("A", p, "taban")], "m2"), vals(data[("B", p, "taban")], "m2")
        aa, ab = vals(data[("A", p, "aday")], "m2"), vals(data[("B", p, "aday")], "m2")
        print(f"   {p} oranlar: aday/taban A={statistics.median(aa)/statistics.median(ta):.3f}  B={statistics.median(ab)/statistics.median(tb):.3f}"
              f" | taban B/A={statistics.median(tb)/statistics.median(ta):.3f} p={mwu_exact_two_sided(tb, ta):.4g}"
              f" | aday B/A={statistics.median(ab)/statistics.median(aa):.3f} p={mwu_exact_two_sided(ab, aa):.4g}")
        # havuzlu
        pt, pa = sorted(ta + tb), sorted(aa + ab)
        print(f"   {p} havuzlu A+B: taban med={statistics.median(pt):.0f} aday med={statistics.median(pa):.0f} oran={statistics.median(pa)/statistics.median(pt):.3f}"
              f" p={mwu_exact_two_sided(pa, pt):.4g} [betimsel; oturum tabakası yok sayıldı]")
        print()

    # taban oranı tüm profiller
    print("   Taban (aynı ikili) M2 B/A oranı tüm profiller:")
    for p in PROFILES:
        ta, tb = vals(data[("A", p, "taban")], "m2"), vals(data[("B", p, "taban")], "m2")
        print(f"   {p:<12} A={statistics.median(ta):.0f} B={statistics.median(tb):.0f} B/A={statistics.median(tb)/statistics.median(ta):.3f}"
              f"  p={mwu_exact_two_sided(tb, ta):.4g}  bant[0.8,1.25] {'içi' if 0.8 <= statistics.median(tb)/statistics.median(ta) <= 1.25 else 'DIŞI'}")
    print()
    # A taban vs B taban M2 içinde, koşu-içi gecikme dağılımı ölçeği
    print("   Koşular arası M2 yayılımı (IQR/medyan) taban, p1/p5:")
    for p in ("p1_loss50", "p5_revpath"):
        for s in ("A", "B"):
            v = vals(data[(s, p, "taban")], "m2")
            print(f"   {p} {s} taban IQR/med={(q(v,.75)-q(v,.25))/statistics.median(v):.2f}")
    print()

    # p1, p5 sıralı M2 değerleri
    for p in ("p1_loss50", "p5_revpath"):
        for s, arm in cells:
            v = vals(data[(s, p, arm)], "m2")
            print(f"   {p} {s} {arm:<5} M2 sıralı: {' '.join(f'{x:.0f}' for x in v)}")
        print()

    # Oturum zamanlaması: p2 hücresiyle eş-zamanlı koşan profiller
    print("## 5) Eşzamanlılık: her profilin [ilk t0, son t0] aralığı (dk, oturum başlangıcına göre)")
    for s in ("A", "B"):
        t_all = [r["t0"] for (ss, p, arm), runs in data.items() if ss == s for r in runs if r.get("t0")]
        t_start = min(t_all)
        for p in PROFILES:
            ts = [r["t0"] for arm in ("taban", "aday") for r in data[(s, p, arm)] if r.get("t0")]
            print(f"   {s} {p:<12} {(min(ts)-t_start)/6e10:6.1f} – {(max(ts)-t_start)/6e10:6.1f} dk")
    print()


if __name__ == "__main__":
    main()
