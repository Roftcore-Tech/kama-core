#!/usr/bin/env python3
"""harness/analyze.py — quinn baseline HAM jsonl'den zengin metrik çıkarımı → results/analysis.json.

`bench metrics` (Rust) completion medyan/IQR otoritesidir; bu betik AYNI nearest-rank ile
onu ÇOĞALTIR (cross-check) + ek metrikler çıkarır (per-repeat spread, handshake başarı,
first-delivery, p50/p95, delivered oranı, secondary). Belge + görsel bunu tüketir. stdlib-only.
"""
import json, glob, os, math

RESULTS = os.path.join(os.path.dirname(__file__), "..", "results")
PROFILES = ["p7_bbr", "p4_jitter", "p6_blackout", "p1_loss50", "p5_revpath", "p2_loss80", "p3_loss95"]
CCS = ["cubic", "bbr"]
DEADLINES_S = [10, 30, 120]
LABEL = {
    "p1_loss50": "%50 kayıp", "p2_loss80": "%80 kayıp", "p3_loss95": "%95 kayıp",
    "p4_jitter": "jitter 200±100ms", "p5_revpath": "ACK-yolu %80 (asimetrik)",
    "p6_blackout": "aralıklı blackout 2s/8s", "p7_bbr": "%2 kayıp (iyi ağ)",
}


def load_run(path):
    ev = []
    meta = None
    with open(path) as f:
        for line in f:
            o = json.loads(line)
            if "meta" in o:
                meta = o["meta"]
            elif "event" in o:
                ev.append(o["event"])
    return meta, ev


def latency_ns(e):
    return None if e["delivered_ns"] is None else e["delivered_ns"] - e["sent_ns"]


def completion(ev, deadline_ns):
    if not ev:
        return 0.0
    ok = sum(1 for e in ev if (l := latency_ns(e)) is not None and l <= deadline_ns)
    return ok / len(ev)


def rank_index(n, p):  # Rust metrics.rs ile AYNI nearest-rank
    return max(1, min(math.ceil(p / 100 * n), n)) - 1


def pct(sorted_vals, p):
    if not sorted_vals:
        return None
    return sorted_vals[rank_index(len(sorted_vals), p)]


def cell(profile, cc):
    files = sorted(glob.glob(os.path.join(RESULTS, f"{profile}_{cc}_r*.jsonl")))
    per_repeat = {d: [] for d in DEADLINES_S}
    all_lat, delivered, total, connected = [], 0, 0, 0
    udp_tx = udp_rx = payload_total = 0
    for fp in files:
        meta, ev = load_run(fp)
        total += len(ev)
        for d in DEADLINES_S:
            per_repeat[d].append(completion(ev, d * 1_000_000_000))
        lat = sorted(l for e in ev if (l := latency_ns(e)) is not None)
        delivered += len(lat)
        all_lat += lat
        if meta and meta["udp_tx_bytes"] > 0:
            connected += 1
        if meta:
            udp_tx += meta["udp_tx_bytes"]
            udp_rx += meta["udp_rx_bytes"]
        payload_total += sum(e["payload_bytes"] for e in ev)
    all_lat.sort()
    agg = {}
    for d in DEADLINES_S:
        vals = sorted(per_repeat[d])
        agg[d] = dict(median=pct(vals, 50), q1=pct(vals, 25), q3=pct(vals, 75),
                      min=min(vals), max=max(vals), per_repeat=per_repeat[d])
    return dict(
        profile=profile, cc=cc, label=LABEL[profile], runs=len(files),
        completion=agg,
        handshake_success=connected / len(files) if files else 0.0,
        delivered=delivered, total=total,
        delivered_frac=delivered / total if total else 0.0,
        first_delivery_ms=(all_lat[0] / 1e6) if all_lat else None,
        p50_ms=(pct(all_lat, 50) / 1e6) if all_lat else None,
        p95_ms=(pct(all_lat, 95) / 1e6) if all_lat else None,
        max_lat_ms=(all_lat[-1] / 1e6) if all_lat else None,
        bytes_per_delivered=(udp_tx / delivered) if delivered else None,
        ack_overhead=(udp_rx / payload_total) if payload_total else 0.0,
    )


def main():
    meta_path = os.path.join(RESULTS, "run-metadata.json")
    repro = json.load(open(meta_path)) if os.path.exists(meta_path) else {}
    cells = [cell(p, cc) for p in PROFILES for cc in CCS]
    out = dict(repro=repro, deadlines_s=DEADLINES_S, cells=cells)
    outpath = os.path.join(RESULTS, "analysis.json")
    json.dump(out, open(outpath, "w"), indent=2)
    print(f"wrote {outpath} ({len(cells)} hücre)")
    # kısa cross-check tablosu
    print(f"\n{'profil':<12}{'cc':<6}{'hs%':>5}{'c10':>7}{'c30':>7}{'c120':>7}"
          f"{'deliv%':>8}{'first':>8}{'p50':>9}{'p95':>9}")
    for c in cells:
        hs = f"{c['handshake_success']*100:.0f}"
        c10, c30, c120 = (c["completion"][d]["median"] for d in DEADLINES_S)
        fd = f"{c['first_delivery_ms']:.0f}ms" if c["first_delivery_ms"] is not None else "—"
        p50 = f"{c['p50_ms']:.0f}ms" if c["p50_ms"] is not None else "—"
        p95 = f"{c['p95_ms']:.0f}ms" if c["p95_ms"] is not None else "—"
        print(f"{c['profile']:<12}{c['cc']:<6}{hs:>5}{c10:>7.2f}{c30:>7.2f}{c120:>7.2f}"
              f"{c['delivered_frac']*100:>7.1f}%{fd:>8}{p50:>9}{p95:>9}")


if __name__ == "__main__":
    main()
