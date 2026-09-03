#!/usr/bin/env python3
"""harness/compare.py — Kama vs quinn killer-chart karşılaştırması + netem kalite kapısı (§4.4).

Her iki transport AYNI metrik koduyla, AYNI netem profillerinde (§4.4.9 reproducible), 5 repeat.
kama: results/kama_<profile>_r*.jsonl · quinn: results/<profile>_cubic_r*.jsonl (baseline v1).
stdlib-only. Kama'nın sent_ns'i app-offer frame'inde (review-hardened) → elmalar-elmalar.

MODLAR
------
  compare.py                          Rapor (varsayılan). results/kama-vs-quinn.json yazar.
  compare.py --emit-baseline PATH     Mevcut koşumu baseline olarak dondurur (ilk kurulum).
  compare.py --gate PATH              CI-07 KALİTE KAPISI: baseline'a karşı regresyon sınar.
                                      Regresyon varsa exit=1. results/gate-report.json yazar.

CI-07 / BUL-01 GEREKÇESİ
------------------------
ADR-18 ve CLAUDE.md §7 nightly netem karşılaştırmasını "ZORUNLU KALİTE KAPISI" olarak tanımlar;
ancak ci.yml'de böyle bir job yoktu ve .gitignore'daki `/results` satırı nedeniyle depoda
karşılaştırılacak TEK BİR baseline dosyası bile bulunmuyordu ([ÖLÇÜLDÜ] 0 adet .jsonl;
run-metadata.json / analysis.json / kama-vs-quinn.json üçü de eksik). Ürünün en güçlü iddiası
regresyon korumasızdı. Bu modül o kapının çalışan karşılığıdır.

TOLERANS POLİTİKASI (tek kaynak — burada tanımlıdır)
---------------------------------------------------
Kapı MEDYAN completion üzerinden çalışır ve iki eşik taşır:
  ABS_TOL   Mutlak gürültü payı. netem + serialization jitter altında repeat'ler arası ±%3
            oynama beklenir; bunun altındaki fark regresyon SAYILMAZ.
  REL_TOL   Göreli düşüş payı. Yüksek completion'da mutlak eşik çok gevşer (0,95 → 0,92 kabul
            edilirdi); göreli eşik bunu sıkar.
Regresyon = düşüş HEM ABS_TOL'ü HEM REL_TOL'ü aşarsa. İki eşik AND'lenir çünkü tek başına
her biri bir uçta yanlış alarm üretir (düşük completion'da göreli, yüksekte mutlak).

İYİLEŞME KAPI DÜŞÜRMEZ ama raporlanır — baseline'ın ne zaman tazelenmesi gerektiği görünür olsun.
"""
import argparse, json, glob, os, math, sys

RESULTS = os.path.join(os.path.dirname(__file__), "..", "results")
# iyi → kötü (severity)
PROFILES = ["p7_bbr", "p4_jitter", "p6_blackout", "p1_loss50", "p5_revpath", "p2_loss80", "p3_loss95"]
DEADLINES_S = [10, 30, 120]
LABEL = {
    "p1_loss50": "%50 kayıp", "p2_loss80": "%80 kayıp", "p3_loss95": "%95 kayıp",
    "p4_jitter": "jitter", "p5_revpath": "ACK-yolu %80", "p6_blackout": "aralıklı blackout",
    "p7_bbr": "%2 kayıp",
}


def load_events(path):
    ev = []
    with open(path) as f:
        for line in f:
            o = json.loads(line)
            if "event" in o:
                ev.append(o["event"])
    return ev


def completion(ev, deadline_ns):
    if not ev:
        return 0.0
    ok = sum(1 for e in ev
             if e["delivered_ns"] is not None and (e["delivered_ns"] - e["sent_ns"]) <= deadline_ns)
    return ok / len(ev)


def rank_index(n, p):  # Rust metrics.rs nearest-rank
    return max(1, min(math.ceil(p / 100 * n), n)) - 1


def pct(sorted_vals, p):
    return sorted_vals[rank_index(len(sorted_vals), p)] if sorted_vals else 0.0


def agg(files, deadline_s):
    comps = sorted(completion(load_events(f), deadline_s * 1_000_000_000) for f in files)
    if not comps:
        return None
    return {"median": pct(comps, 50), "q1": pct(comps, 25), "q3": pct(comps, 75),
            "min": comps[0], "max": comps[-1], "n": len(comps)}


def cell(profile):
    kfiles = sorted(glob.glob(os.path.join(RESULTS, f"kama_{profile}_r*.jsonl")))
    qfiles = sorted(glob.glob(os.path.join(RESULTS, f"{profile}_cubic_r*.jsonl")))
    out = {"profile": profile, "label": LABEL[profile], "kama": {}, "quinn": {}}
    for d in DEADLINES_S:
        out["kama"][str(d)] = agg(kfiles, d)
        out["quinn"][str(d)] = agg(qfiles, d)
    out["kama_runs"] = len(kfiles)
    out["quinn_runs"] = len(qfiles)
    return out


# ── CI-07 kalite kapısı ─────────────────────────────────────────────────────

# Tolerans eşikleri — TEK KAYNAK. Değişiklik gerekçesiyle birlikte commit edilmelidir.
ABS_TOL = 0.03   # mutlak completion düşüşü
REL_TOL = 0.05   # göreli completion düşüşü (baseline'ın %5'i)
# Kapının anlamlı olması için gereken minimum repeat sayısı. Altındaysa kapı VERİ YOK der ve düşer
# — "ölçüm yoksa yeşil" davranışı BUL-01/BUL-02'nin tam olarak ürettiği yanlış güvenlik hissidir.
MIN_RUNS = 3


def snapshot():
    """Mevcut results/ koşumundan baseline-uyumlu özet üretir."""
    out = {"deadlines_s": DEADLINES_S, "profiles": {}}
    for p in PROFILES:
        kfiles = sorted(glob.glob(os.path.join(RESULTS, f"kama_{p}_r*.jsonl")))
        entry = {"runs": len(kfiles), "kama": {}}
        for d in DEADLINES_S:
            a = agg(kfiles, d)
            entry["kama"][str(d)] = None if a is None else {"median": a["median"], "n": a["n"]}
        out["profiles"][p] = entry
    return out


def emit_baseline(path):
    snap = snapshot()
    thin = [p for p, e in snap["profiles"].items() if e["runs"] < MIN_RUNS]
    if thin:
        print(f"HATA: su profillerde < {MIN_RUNS} repeat var: {thin}", file=sys.stderr)
        print("Baseline eksik veriyle dondurulmaz (BUL-01: kanitsiz baseline kapi degildir).",
              file=sys.stderr)
        return 1
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    snap["_note"] = ("CI-07 baseline. compare.py --gate bu dosyaya karsi kosar. "
                     "Tazeleme: compare.py --emit-baseline ile yeniden uret ve gerekcesiyle commit et.")
    with open(path, "w") as f:
        json.dump(snap, f, indent=2)
    print(f"-> baseline yazildi: {path}")
    return 0


def gate(path):
    if not os.path.exists(path):
        print(f"HATA: baseline yok: {path}", file=sys.stderr)
        return 1
    with open(path) as f:
        base = json.load(f)
    cur = snapshot()

    regressions, improvements, missing = [], [], []
    for p in PROFILES:
        b_entry = base.get("profiles", {}).get(p)
        c_entry = cur["profiles"][p]
        if b_entry is None:
            missing.append(f"{p}: baseline'da yok")
            continue
        if c_entry["runs"] < MIN_RUNS:
            missing.append(f"{p}: {c_entry['runs']} repeat < {MIN_RUNS}")
            continue
        for d in DEADLINES_S:
            b = (b_entry["kama"] or {}).get(str(d))
            c = c_entry["kama"].get(str(d))
            if b is None or c is None:
                missing.append(f"{p}@{d}s: olcum yok")
                continue
            delta = c["median"] - b["median"]
            rel = delta / b["median"] if b["median"] > 0 else 0.0
            rec = {"profile": p, "deadline_s": d, "baseline": b["median"],
                   "current": c["median"], "delta": delta, "rel": rel}
            # AND'lenmis esik: iki esigi de asan dusus regresyondur.
            if -delta > ABS_TOL and -rel > REL_TOL:
                regressions.append(rec)
            elif delta > ABS_TOL:
                improvements.append(rec)

    report = {"abs_tol": ABS_TOL, "rel_tol": REL_TOL, "min_runs": MIN_RUNS,
              "regressions": regressions, "improvements": improvements, "missing": missing}
    os.makedirs(RESULTS, exist_ok=True)
    with open(os.path.join(RESULTS, "gate-report.json"), "w") as f:
        json.dump(report, f, indent=2)

    print("=" * 78)
    print(f"CI-07 NETEM KALITE KAPISI  (abs>{ABS_TOL} VE rel>{REL_TOL} => regresyon)")
    print("=" * 78)
    for r in regressions:
        print(f"  REGRESYON  {r['profile']:<12} @{r['deadline_s']:>3}s  "
              f"{r['baseline']:.3f} -> {r['current']:.3f}  ({r['rel']*100:+.1f}%)")
    for r in improvements:
        print(f"  iyilesme   {r['profile']:<12} @{r['deadline_s']:>3}s  "
              f"{r['baseline']:.3f} -> {r['current']:.3f}  ({r['rel']*100:+.1f}%)")
    for m in missing:
        print(f"  VERI YOK   {m}")
    print("-" * 78)

    if missing:
        # "Olcum yoksa yesil" YASAK: eksik veri kapiyi DUSURUR.
        print(f"KAPI DUSTU: {len(missing)} eksik olcum. Kanitsiz gecis kabul edilmez.")
        return 1
    if regressions:
        print(f"KAPI DUSTU: {len(regressions)} regresyon.")
        return 1
    if improvements:
        print(f"KAPI GECTI. {len(improvements)} iyilesme var -> baseline tazelenmeli mi? "
              f"(compare.py --emit-baseline)")
    else:
        print("KAPI GECTI.")
    return 0


def report():
    cells = [cell(p) for p in PROFILES]
    os.makedirs(RESULTS, exist_ok=True)
    json.dump({"deadlines_s": DEADLINES_S, "cells": cells},
              open(os.path.join(RESULTS, "kama-vs-quinn.json"), "w"), indent=2)

    print("=" * 92)
    print("KAMA vs quinn — completion medyan (5 repeat) · aynı netem · aynı metrik · §4.4")
    print("=" * 92)
    hdr = f"{'profil':<12}{'koşul':<20}"
    for d in DEADLINES_S:
        hdr += f"{'K'+str(d):>7}{'Q'+str(d):>7}"
    hdr += "   Δc(10s)"
    print(hdr)
    print("-" * 92)
    for c in cells:
        row = f"{c['profile']:<12}{c['label']:<20}"
        k10 = q10 = None
        for d in DEADLINES_S:
            k = c["kama"][str(d)]; q = c["quinn"][str(d)]
            km = k["median"] if k else float("nan")
            qm = q["median"] if q else float("nan")
            if d == 10:
                k10, q10 = km, qm
            row += f"{km:>7.2f}{qm:>7.2f}"
        adv = "" if (k10 is None or q10 is None) else (
            f"  {k10 - q10:+.2f}" + ("  ✓KAMA" if k10 > q10 + 0.02 else ("  ~eş" if abs(k10 - q10) <= 0.02 else "  quinn")))
        print(row + adv)
    print("-" * 92)
    print("K/Q<deadline> = Kama/quinn completion medyanı. Δc(10s) = kısa-deadline (tez) farkı.")
    print(f"→ results/kama-vs-quinn.json yazıldı ({len(cells)} profil)")


def main():
    ap = argparse.ArgumentParser(description="Kama netem karsilastirma + CI-07 kalite kapisi")
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--gate", metavar="BASELINE",
                   help="CI-07: baseline'a karsi regresyon kapisi (regresyonda exit=1)")
    g.add_argument("--emit-baseline", metavar="PATH",
                   help="Mevcut kosumu baseline olarak dondur")
    a = ap.parse_args()
    if a.gate:
        return gate(a.gate)
    if a.emit_baseline:
        return emit_baseline(a.emit_baseline)
    report()
    return 0


if __name__ == "__main__":
    sys.exit(main())