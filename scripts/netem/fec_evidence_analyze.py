#!/usr/bin/env python3
"""[EVIDENCE] Çözümleyici. Tablolar ÜÇLÜ: tüm-rep · stall-dışı · stall sayısı.
d2 = ≤2×RTT teslim. Saf-reorder hücrelerinde (E2) L2'lerin TAMAMI yanlıştır.
Ek: em@L1 / em@L2 / risk@L1 / pers@L2 (geçiş-diag), t_risk==t_susp invaryantı, ts.csv'den
instant tepe + tepe anındaki (em, seviye, final) ve em yörüngesi (0.5/1/2 s).
Kullanım: fec_evidence_analyze.py ROOT [--all]   (--all: stall-dışı yerine tüm rep medyanı da basar)
"""
import sys, glob, re, json, statistics, os
root = sys.argv[1]
def parse(d):
    if not all(os.path.exists(f"{d}/{f}") for f in ("rc", "c.err", "out.jsonl", "argv.txt")):
        return None
    a = open(f"{d}/argv.txt").read()
    cell = re.search(r"cell=(\S+)", a).group(1); arm = re.search(r"arm=(\S+)", a).group(1)
    msgs = int(re.search(r"msgs=(\d+)", a).group(1)); rtt = int(re.search(r"rtt=(\d+)", a).group(1))
    t = open(f"{d}/c.err", errors="replace").read()
    g = lambda p, x=0: int(re.search(p, t).group(1)) if re.search(p, t) else x
    dl = re.search(r"delivered=(\d+)/(\d+)", t); rt = re.search(r"risk_t=\((\d+), (\d+), (\d+), (\d+)\)", t)
    lv = re.search(r"lvl=\[(\d+), (\d+), (\d+)\]", t); sp = re.search(r"spec=\((\d+), (\d+), (\d+), (\d+), (\d+)\)", t)
    e1 = re.search(r"em_l1=\((\d+), (\d+), (\d+)\)", t); e2 = re.search(r"em_l2=\((\d+), (\d+), (\d+)\)", t)
    if not (dl and rt and lv and sp): return None
    D = 2 * rtt * 1_000_000; n = ok = 0
    for i, line in enumerate(open(f"{d}/out.jsonl")):
        if i == 0: continue
        e = json.loads(line).get("event", {}); n += 1
        if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D: ok += 1
    t_susp, t_conf, t_risk = int(rt.group(1)), int(rt.group(2)), int(rt.group(3))
    # ts.csv: tepe ve yörünge
    pk = dict(inst=0, em=0, lvl=0, fin=0, t=0); traj = {}
    if os.path.exists(f"{d}/ts.csv"):
        rows = [l.strip().split(",") for l in open(f"{d}/ts.csv")][1:]
        for r in rows:
            try: tm, inst, em, lvl, fin = int(r[0]), int(r[1]), int(r[11]), int(r[10]), int(r[8])
            except (IndexError, ValueError): continue
            if inst > pk["inst"]: pk = dict(inst=inst, em=em, lvl=lvl, fin=fin, t=tm)
            for mark in (500, 1000, 2000):
                if mark not in traj and tm >= mark: traj[mark] = em
    return dict(cell=cell, arm=arm, msgs=msgs, rtt=rtt, d2=ok/n if n else 0.0, N=n,
                tx=g(r"tx=(\d+)B"), fec=g(r"fec=(\d+) "), retx=g(r"retx=(\d+) "),
                l1=int(lv.group(2)), l2=int(lv.group(3)), spurious=g(r"spurious=(\d+)"),
                t_l1=int(sp.group(4)), t_l2=int(sp.group(5)), t_susp=t_susp, t_conf=t_conf, t_risk=t_risk,
                em_l1=int(e1.group(1)) if e1 else 0, risk_l1=int(e1.group(2)) if e1 else 0,
                em_l2=int(e2.group(1)) if e2 else 0, pers_l2=int(e2.group(3)) if e2 else 0,
                pk=pk, traj=traj,
                stall=(t_susp == 0 or t_susp > 1_000_000_000), lossy=("loss" in a))
runs = [r for r in (parse(d) for d in sorted(glob.glob(f"{root}/*/rep*"))) if r]
cells = {}
for r in runs: cells.setdefault((r["cell"], r["msgs"]), {}).setdefault(r["arm"], []).append(r)
def med(v): return statistics.median(v) if v else float("nan")
ms = lambda v: "asla" if not v else f"{v/1e6:.0f}"
def order(a):
    if a == "off": return (0, "")
    return (1 if a.startswith("A") else 2, a)
inv_bad = []; inv_n = 0
for (cell, msgs) in sorted(cells, key=lambda k: (k[0], k[1])):
    arms = cells[(cell, msgs)]
    lossy = any(r["lossy"] for rs in arms.values() for r in rs)
    off = arms.get("off", [])
    print("=" * 172)
    print(f"HÜCRE {cell} · {msgs} msg · RTT {off[0]['rtt'] if off else '?'} ms · {'KAYIPLI' if lossy else 'KAYIPSIZ (her L2 YANLIŞ)'}")
    print("=" * 172)
    print(f"{'kol':>6s} {'n':>2s} {'stall':>5s} {'n_ok':>4s} {'d2_all':>6s} {'d2_ok':>6s} {'wireΔ%':>7s} {'parity':>7s} {'retx':>6s} {'L1':>5s} {'L2':>5s} {'spur':>5s} {'t_susp':>6s} {'t_L1':>6s} {'t_L2':>6s} {'em@L1':>5s} {'rsk@L1k':>7s} {'em@L2':>5s} {'prs@L2k':>7s} {'pk_k':>5s} {'em@pk':>5s} {'lvl@pk':>6s} {'em.5s':>5s} {'em1s':>5s} {'em2s':>5s}")
    print("-" * 172)
    off_tx = med([r["tx"] for r in off]) if off else float("nan")
    for arm in sorted(arms, key=order):
        rs_all = arms[arm]
        ok = [r for r in rs_all if not (lossy and r["stall"])]
        stalls = sum(1 for r in rs_all if r["stall"]) if lossy else 0
        if not ok: ok = rs_all
        for r in rs_all:
            if arm != "off" and r["t_risk"] > 0:
                inv_n += 1
                if r["t_risk"] != r["t_susp"]: inv_bad.append((cell, arm, r["t_susp"], r["t_risk"]))
        w = (med([r["tx"] for r in ok]) - off_tx) / off_tx * 100 if off and off_tx else 0.0
        l2s = [r for r in ok if r["t_l2"] > 0]; l1s = [r for r in ok if r["t_l1"] > 0]
        pks = [r["pk"] for r in ok if r["pk"]["inst"] > 0]
        print(f"{arm:>6s} {len(rs_all):>2d} {stalls:>5d} {len(ok):>4d} {med([r['d2'] for r in rs_all]):>6.3f} {med([r['d2'] for r in ok]):>6.3f} {w:>+7.2f} "
              f"{med([r['fec'] for r in ok]):>7.0f} {med([r['retx'] for r in ok]):>6.0f} {med([r['l1'] for r in ok]):>5.0f} {med([r['l2'] for r in ok]):>5.0f} "
              f"{med([r['spurious'] for r in ok]):>5.0f} {ms(med([r['t_susp'] for r in ok if r['t_susp']>0])) if any(r['t_susp']>0 for r in ok) else 'asla':>6s} "
              f"{ms(med([r['t_l1'] for r in l1s])) if l1s else 'asla':>6s} {ms(med([r['t_l2'] for r in l2s])) if l2s else 'asla':>6s} "
              f"{med([r['em_l1'] for r in l1s]) if l1s else float('nan'):>5.0f} {med([r['risk_l1'] for r in l1s])/1000 if l1s else float('nan'):>7.0f} "
              f"{med([r['em_l2'] for r in l2s]) if l2s else float('nan'):>5.0f} {med([r['pers_l2'] for r in l2s])/1000 if l2s else float('nan'):>7.0f} "
              f"{med([p['inst'] for p in pks])/1000 if pks else float('nan'):>5.0f} {med([p['em'] for p in pks]) if pks else float('nan'):>5.0f} {med([p['lvl'] for p in pks]) if pks else float('nan'):>6.0f} "
              f"{med([r['traj'].get(500, float('nan')) for r in ok]):>5.0f} {med([r['traj'].get(1000, float('nan')) for r in ok]):>5.0f} {med([r['traj'].get(2000, float('nan')) for r in ok]):>5.0f}")
    print()
print("=" * 172)
print(f"İNVARYANT t_first_risk == t_first_suspected: {inv_n - len(inv_bad)}/{inv_n} ON-koşu tuttu" + (f"; İHLAL: {inv_bad[:6]}" if inv_bad else " (ihlal yok)"))
# em@L2 dağılımları: E1 (gerçek) vs E2 (yanlış) — eşik seçimi için ham liste
def dist(pred, key):
    v = sorted(r[key] for r in runs if pred(r) and r[key] > 0)
    if not v: return "—"
    q = lambda p: v[min(len(v)-1, max(0, int(round(p*(len(v)-1)))))]
    return f"n={len(v)} min={v[0]} Q10={q(.1)} Q25={q(.25)} med={q(.5)} Q75={q(.75)} Q90={q(.9)} max={v[-1]}"
print("em@L2 · E1 kayıp ≥15% (GERÇEK L2, stall-dışı):", dist(lambda r: r["cell"] in ("E1-L15","E1-L20","E1-L25") and r["arm"]!="off" and not r["stall"], "em_l2"))
print("em@L2 · E2 reorder (YANLIŞ L2):               ", dist(lambda r: r["cell"].startswith("E2") and r["arm"]!="off", "em_l2"))
print("em@L2 · E3 karışık (stall-dışı):              ", dist(lambda r: r["cell"].startswith("E3") and r["arm"]!="off" and not r["stall"], "em_l2"))
print("em@L1 · tüm ON (stall-dışı):                   ", dist(lambda r: r["arm"]!="off" and not r["stall"], "em_l1"))
