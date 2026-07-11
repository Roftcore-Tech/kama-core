#!/usr/bin/env python3
"""harness/compare.py — Kama vs quinn killer-chart karşılaştırması (§4.4).

Her iki transport AYNI metrik koduyla, AYNI netem profillerinde (§4.4.9 reproducible), 5 repeat.
kama: results/kama_<profile>_r*.jsonl · quinn: results/<profile>_cubic_r*.jsonl (baseline v1).
Çıktı: yan-yana completion(10/30/120s) medyan + IQR + Kama-avantajı; results/kama-vs-quinn.json (görsel).
stdlib-only. Kama'nın sent_ns'i app-offer frame'inde (review-hardened) → elmalar-elmalar.
"""
import json, glob, os, math

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


def main():
    cells = [cell(p) for p in PROFILES]
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


if __name__ == "__main__":
    main()
