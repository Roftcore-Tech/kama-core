#!/usr/bin/env python3
"""Mesaj sırasına göre alıcı tek-yön gecikme profili + kısa son tarihlerde teslim (ham veri).
Kullanım: python3 -I profile.py RUNROOT CELL ARM [ARM...]
"""
import json
import math
import os
import statistics
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from recompute import load  # noqa: E402

INF = math.inf
root, cell = sys.argv[1], sys.argv[2]
arms = sys.argv[3:]
DLS = (1.0, 1.5, 2.0, 2.5, 3.0, 5.0, 10.0, 20.0, 60.0)
for arm in arms:
    adir = os.path.join(root, cell, arm)
    reps = sorted((d for d in os.listdir(adir) if d.startswith("rep")), key=lambda x: int(x[3:]))
    per_idx = {}
    comp = {d: [] for d in DLS}
    comp_s = {d: [] for d in DLS}
    setup = []
    first10 = []
    last_half = []
    for rep in reps:
        meta, ev, rx, hdr, nrec = load(os.path.join(adir, rep))
        n = int(meta["msg_count"])
        t0u, off = meta["t0_unix_ns"], meta.get("events_epoch_offset_ns", 0)
        setup.append(meta.get("setup_ns", 0) / 1e9)
        lat = []
        slat = []
        for e in sorted(ev, key=lambda e: e["msg_id"]):
            i = e["msg_id"]
            su = t0u + e["sent_ns"] - off
            v = (rx[i] - su) / 1e9 if i in rx else INF
            lat.append(v)
            d = e.get("delivered_ns")
            slat.append((d - e["sent_ns"]) / 1e9 if d is not None else INF)
            per_idx.setdefault(i, []).append(v)
        for d in DLS:
            comp[d].append(sum(1 for x in lat if x <= d) / n)
            comp_s[d].append(sum(1 for x in slat if x <= d) / n)
        fin = [x for x in lat[:10] if x != INF]
        first10.append(statistics.median(fin) if fin else INF)
        lh = sorted(lat[n // 2:])
        last_half.append(lh[max(1, math.ceil(.95 * len(lh))) - 1])
    print(f"## {cell} {arm} N={len(reps)} setup_s medyan={statistics.median(setup):.3f} "
          f"(min {min(setup):.3f} maks {max(setup):.3f})")
    print("  alıcı teslim@d medyan [Q1,Q3]: " + "  ".join(
        f"{d:g}s={statistics.median(comp[d]):.3f}[{sorted(comp[d])[len(comp[d])//4]:.2f},{sorted(comp[d])[3*len(comp[d])//4]:.2f}]" for d in DLS))
    print("  gönderen teyit@d medyan: " + "  ".join(f"{d:g}s={statistics.median(comp_s[d]):.3f}" for d in DLS))
    print(f"  ilk 10 mesaj medyan gecikme (koşu medyanı) {statistics.median(first10):.2f} s; "
          f"ikinci yarı p95 (koşu medyanı) {statistics.median(last_half):.2f} s")
    # sıra profili: 10'luk dilim başına (koşular × mesajlar) medyan ve p95
    n = max(per_idx) + 1
    w = max(1, n // 10)
    segs = []
    for k in range(0, n, w):
        v = sorted(x for i in range(k, min(n, k + w)) for x in per_idx.get(i, []))
        fin = [x for x in v if x != INF]
        p95 = v[max(1, math.ceil(.95 * len(v))) - 1] if v else None
        segs.append(f"[{k}-{min(n, k + w) - 1}] p50={statistics.median(v) if v else float('nan'):.1f} p95={p95:.1f}")
    print("  sıra profili (alıcı tek-yön, s): " + " | ".join(segs))
