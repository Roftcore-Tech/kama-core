#!/usr/bin/env python3
"""[PERSIST] Çözümleyici. Her tablo ÜÇLÜ: tüm-rep · stall-dışı · stall sayısı (owner talimatı).
d2 = ≤2×RTT teslim (RTT hücreden). Saf-reorder hücrelerinde L2 ateşlemelerinin TAMAMI yanlıştır.
Cold-start kapısı: aynı politikada t_first_L1 persistence modundan bağımsız olmalı.
"""
import sys, glob, re, json, statistics
root = sys.argv[1]
def parse(d):
    a = open(f"{d}/argv.txt").read()
    cell = re.search(r"cell=(\S+)", a).group(1); arm = re.search(r"arm=(\S+)", a).group(1)
    msgs = int(re.search(r"msgs=(\d+)", a).group(1)); rtt = int(re.search(r"rtt=(\d+)", a).group(1))
    t = open(f"{d}/c.err", errors="replace").read()
    g = lambda p, x=0: int(re.search(p, t).group(1)) if re.search(p, t) else x
    dl = re.search(r"delivered=(\d+)/(\d+)", t); rt = re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)", t)
    lv = re.search(r"lvl=\[(\d+), (\d+), (\d+)\]", t); sp = re.search(r"spec=\((\d+), (\d+), (\d+), (\d+), (\d+)\)", t)
    pe = re.search(r"persist=\((\d+), (\d+), (\d+)\)", t)
    if not (dl and rt and lv and sp): return None
    D = 2 * rtt * 1_000_000; n = ok = 0
    for i, line in enumerate(open(f"{d}/out.jsonl")):
        if i == 0: continue
        e = json.loads(line).get("event", {}); n += 1
        if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D: ok += 1
    t_susp = int(rt.group(1))
    return dict(cell=cell, arm=arm, msgs=msgs, rtt=rtt, d2=ok/n if n else 0.0, N=n,
                tx=g(r"tx=(\d+)B"), fec=g(r"fec=(\d+) "), retx=g(r"retx=(\d+) "),
                l1=int(lv.group(2)), l2=int(lv.group(3)), spurious=g(r"spurious=(\d+)"),
                t_l1=int(sp.group(4)), t_l2=int(sp.group(5)), t_susp=t_susp,
                p_last=int(pe.group(1)) if pe else 0, p_peak=int(pe.group(2)) if pe else 0, p_n=int(pe.group(3)) if pe else 0,
                stall=(t_susp == 0 or t_susp > 1_000_000_000), lossy=("loss" in a))
runs = [r for r in (parse(d) for d in sorted(glob.glob(f"{root}/*/rep*"))) if r]
cells = {}
for r in runs: cells.setdefault((r["cell"], r["msgs"]), {}).setdefault(r["arm"], []).append(r)
def med(v): return statistics.median(v) if v else float("nan")
ms = lambda v: "asla" if not v else f"{v/1e6:.0f}"
ARMS = ["off","A-P0","A-P1","A-P2","A-P3","B-P0","B-P1","B-P2","B-P3"]
for (cell, msgs) in sorted(cells, key=lambda k: (k[0], k[1])):
    arms = cells[(cell, msgs)]
    lossy = any(r["lossy"] for rs in arms.values() for r in rs)
    off = arms.get("off", [])
    print("=" * 150)
    print(f"HÜCRE {cell} · {msgs} msg · RTT {off[0]['rtt'] if off else '?'} ms · {'KAYIPLI' if lossy else 'KAYIPSIZ (her L2 YANLIŞ)'}   fwd={open(glob.glob(f'{root}/{cell}_m{msgs}_off/rep1/argv.txt')[0]).read().split('fwd=')[1].strip()}")
    print("=" * 150)
    print(f"{'kol':>6s} {'n':>2s} {'stall':>5s} {'n_ok':>4s} {'d2_all':>6s} {'d2_ok':>6s} {'wireΔ%':>7s} {'parity':>7s} {'retx':>6s} {'L1':>5s} {'L2':>5s} {'spur':>5s} {'t_L1':>6s} {'t_L2':>6s} {'ufuk_ms':>7s} {'örnek':>5s}")
    print("-" * 150)
    off_ok = [r for r in off if not (lossy and r["stall"])] or off
    off_tx = med([r["tx"] for r in off_ok]) if off else float("nan")
    for a in ARMS:
        rs = arms.get(a)
        if not rs: continue
        ok = [r for r in rs if not (lossy and r["stall"])] or rs
        stalls = sum(1 for r in rs if r["stall"]) if lossy else 0
        wd = (med([r["tx"] for r in ok]) - off_tx) / off_tx * 100 if off else float("nan")
        print(f"{a:>6s} {len(rs):>2d} {stalls:>5d} {len(ok):>4d} {med([r['d2'] for r in rs]):>6.3f} {med([r['d2'] for r in ok]):>6.3f} {wd:>+7.2f} "
              f"{med([r['fec'] for r in ok]):>7.0f} {med([r['retx'] for r in ok]):>6.0f} {med([r['l1'] for r in ok]):>5.0f} {med([r['l2'] for r in ok]):>5.0f} "
              f"{med([r['spurious'] for r in ok]):>5.0f} {ms(med([r['t_l1'] for r in ok])):>6s} {ms(med([r['t_l2'] for r in ok])):>6s} "
              f"{med([r['p_last'] for r in ok])/1e6:>7.0f} {med([r['p_n'] for r in ok]):>5.0f}")
    print()
print("=" * 150)
print("COLD-START KAPISI — aynı politika, farklı persistence modu: t_first_L1 medyanı (ms) DEĞİŞMEMELİ")
print("=" * 150)
for (cell, msgs) in sorted(cells, key=lambda k: (k[0], k[1])):
    arms = cells[(cell, msgs)]
    for pol in ("A", "B"):
        vals = {m: med([r["t_l1"] for r in arms.get(f"{pol}-{m}", []) if r["t_l1"]]) for m in ("P0","P1","P2","P3")}
        if all(v == v for v in vals.values()) and vals:
            spread = max(vals.values()) - min(vals.values())
            print(f"  {cell:>4s} {msgs:>4d} {pol}: " + "  ".join(f"{m}={v/1e6:.0f}" for m, v in vals.items()) + f"   yayılım={spread/1e6:.0f} ms  → {'OK' if spread <= 60e6 else 'İNCELE'}")
