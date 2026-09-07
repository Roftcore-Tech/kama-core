#!/usr/bin/env python3
"""[R-SWEEP] Repair-quantum Pareto çözümleyicisi.

Birim = REP; hücre özeti = MEDYAN (n küçük; min/max ayrıca). Birincil uç nokta: ≤2×RTT (400 ms)
teslim oranı. Yanında: wire (tx), parity, retx byte; FEC-kurtarılan birim; deadline kaçırma
(20 s'de teslim edilmeyen); parity_before_flow_end (kısa-akış regresyon kapısı).
ParityEfficiency (PE) = (teslim2rtt(policy) − teslim2rtt(off)) × N / parity_byte  [zamanında-mesaj / parity B]
Marjinal PE = Pareto sınırında komşu politikalar arası Δteslim×N / Δparity.
Kurtarma verimi = fec_recovered_ctrl / (fec_repairs_tx) [birim / sembol].
"""
import sys, glob, re, json, statistics, itertools
root = sys.argv[1]; D = 400_000_000

def parse(d):
    t = open(f"{d}/c.err", errors="replace").read()
    g = lambda p, x=0: int(re.search(p, t).group(1)) if re.search(p, t) else x
    dl = re.search(r"delivered=(\d+)/(\d+)", t); rt = re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)", t); fe = re.search(r"flow_end=(\d+)", t)
    if not (dl and rt and fe): return None
    n = ok = 0
    for i, line in enumerate(open(f"{d}/out.jsonl")):
        if i == 0: continue
        e = json.loads(line).get("event", {}); n += 1
        if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D: ok += 1
    t_par, fend = int(rt.group(4)), int(fe.group(1)); t_susp = int(rt.group(1))
    return dict(delivered=int(dl.group(1)), offered=int(dl.group(2)), d2=ok/n if n else 0.0, n=n,
                stall=(t_susp == 0 or t_susp > 1_000_000_000),  # 0% hücrelerde özet aşamasında yok sayılır
                tx=g(r"tx=(\d+)B"), fec=g(r"fec=(\d+) "), retx=g(r"retx=(\d+) "), repairs=g(r"fec_repairs_tx=(\d+)"),
                recovered=g(r"fec_recovered_ctrl=(\d+)"), par_before_end=(t_par > 0 and t_par < fend),
                t_par=t_par, flow_end=fend)

cells = {}
for d in sorted(glob.glob(f"{root}/*/rep*")):
    r = parse(d)
    if r: cells.setdefault(d.split("/")[-2], []).append(r)
def key(c):
    m = re.match(r"f(\d+)_l(\d+)_(.+)", c); return (int(m.group(1)), int(m.group(2)), m.group(3))
def med(v): return statistics.median(v) if v else float("nan")

# ── STALL-DIŞI TABAKA ─────────────────────────────────────────────────────────────────────
# Kuruluş-stall'ı (ilk kuşku >1 s ya da hiç) politikadan ÖNCE oluşur → politikadan bağımsız
# gürültü sınıfı; n=5'te ≥3 stall medyanı bir stall koşusu yapar. Özet ve Pareto STALL-DIŞI
# rep'ler üzerinden (n_ok); tüm-rep medyanı da yan sütunda raporlanır (gizlenmez).
STRAT = "--all" not in sys.argv
def usable(r, loss):
    return (loss == 0) or (not r["stall"]) if STRAT else True
summ = {}
for c in cells:
    rs_all = cells[c]; f, l, arm = key(c)
    rs = [r for r in rs_all if usable(r, l)] or rs_all
    summ[(f, l, arm)] = dict(n_all=len(rs_all), d2_all=med([r["d2"] for r in rs_all]),n=len(rs), d2=med([r["d2"] for r in rs]), d2min=min(r["d2"] for r in rs), d2max=max(r["d2"] for r in rs),
        tx=med([r["tx"] for r in rs]), fec=med([r["fec"] for r in rs]), retx=med([r["retx"] for r in rs]),
        repairs=med([r["repairs"] for r in rs]), recovered=med([r["recovered"] for r in rs]),
        miss=med([r["offered"]-r["delivered"] for r in rs]), pbe=sum(1 for r in rs if r["par_before_end"]), N=rs[0]["n"],
        stalls=(0 if l == 0 else sum(1 for r in rs_all if r["stall"])))

print("=" * 140)
print("R-SWEEP — hücre×kol özetleri. STALL-DIŞI medyan (n_ok) — 'd2_all' = tüm rep medyanı. d2 = ≤2×RTT teslim; PE = (d2−d2_off)×N/parity_B ×1000 [zamanında-mesaj/KB]" if STRAT else "R-SWEEP — TÜM REP medyanı")
print("=" * 140)
print(f"{'akış':>5s} {'kayıp':>5s} {'kol':>13s} {'n_ok':>4s} {'d2':>6s} {'d2_all':>6s} {'[min,max]':>13s} {'wire_B':>7s} {'wireΔ%':>7s} {'parity_B':>8s} {'retx_B':>7s} {'repair':>6s} {'kurt.':>5s} {'verim':>6s} {'miss':>4s} {'par<son':>7s} {'stall':>5s} {'PE/KB':>7s}")
print("-" * 140)
flows = sorted({k[0] for k in summ}); losses = sorted({k[1] for k in summ})
frontier_rows = []
for f in flows:
    for l in losses:
        arms = sorted([k[2] for k in summ if k[0]==f and k[1]==l], key=lambda a: (a!="off", a))
        off = summ.get((f, l, "off"))
        for a in arms:
            s = summ[(f, l, a)]
            wd = (s["tx"]-off["tx"])/off["tx"]*100 if off and off["tx"] else float("nan")
            pe = ((s["d2"]-off["d2"])*s["N"]/s["fec"]*1000) if (off and s["fec"]>0) else float("nan")
            yld = s["recovered"]/s["repairs"] if s["repairs"]>0 else float("nan")
            print(f"{f:>5d} {l:>4d}% {a:>13s} {s['n']:>4d} {s['d2']:>6.3f} {s['d2_all']:>6.3f} [{s['d2min']:.2f},{s['d2max']:.2f}] {s['tx']:>7.0f} {wd:>+7.2f} {s['fec']:>8.0f} {s['retx']:>7.0f} {s['repairs']:>6.0f} {s['recovered']:>5.0f} {yld:>6.2f} {s['miss']:>4.0f} {s['pbe']:>3d}/{s['n_all']:<3d} {s['stalls']:>5d} {pe:>7.2f}")
            if a != "off": frontier_rows.append((f, l, a, s["d2"], wd, s["fec"], s["N"]))
        print()

print("=" * 140)
print("PARETO SINIRI (teslim ↑, wire ↓) — hücre başına; 'DOMİNE' = başka bir kol hem ≥ teslim hem ≤ wire veriyor")
print("  marjinal PE: sınır boyunca wire'a göre sıralı komşular arası Δd2×N / Δparity_B ×1000")
print("=" * 140)
for f in flows:
    for l in losses:
        rows = [r for r in frontier_rows if r[0]==f and r[1]==l]
        off = summ.get((f, l, "off"))
        worse = [r[2] for r in rows if off and r[3] < off["d2"]]
        rows = [r for r in rows if not (off and r[3] < off["d2"])]
        if len(rows) < 2: continue
        pareto = []
        for r in rows:
            dominated = any((o[3] >= r[3] and o[4] <= r[4]) and (o[3] > r[3] or o[4] < r[4]) for o in rows if o is not r)
            if not dominated: pareto.append(r)
        pareto.sort(key=lambda r: r[4])
        dom = [r[2] for r in rows if r not in pareto]
        print(f"  akış {f:>4d}  kayıp {l:>2d}%  sınır: " + "  →  ".join(f"{r[2]} (d2={r[3]:.3f}, wire{r[4]:+.1f}%)" for r in pareto) + (f"   | DOMİNE: {', '.join(dom)}" if dom else "") + (f"   | OFF'tan kötü: {', '.join(worse)}" if worse else ""))
        for a, b in zip(pareto, pareto[1:]):
            dp = b[5]-a[5]; dd = b[3]-a[3]
            mpe = dd*b[6]/dp*1000 if dp > 0 else float("inf")
            print(f"      {a[2]} → {b[2]}: Δd2={dd:+.3f}  Δparity={dp:+.0f} B  marjinal PE={mpe:.2f} zamanında-mesaj/KB")

print()
print("=" * 140)
print("KISA-AKIŞ REGRESYON KAPISI (96 msg, 15/20/25%): parity_before_flow_end ve d2, MEVCUT (p125-250-500) ile kıyas")
print("=" * 140)
for l in (15, 20, 25):
    cur = summ.get((96, l, "125-250-500"))
    if not cur: continue
    for a in sorted(k[2] for k in summ if k[0]==96 and k[1]==l and k[2] not in ("off",)):
        s = summ[(96, l, a)]
        flag = "OK" if (s["pbe"] >= cur["pbe"] and s["d2"] >= cur["d2"] - 0.02) else "GERİLEME"
        print(f"  kayıp {l:>2d}%  {a:>13s}  par<son {s['pbe']}/{s['n_all']} (mevcut {cur['pbe']}/{cur['n_all']})  d2 {s['d2']:.3f} (mevcut {cur['d2']:.3f})  → {flag}")
