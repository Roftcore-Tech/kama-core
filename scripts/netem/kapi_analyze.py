#!/usr/bin/env python3
"""[KAPI-1] Kampanya çıktı analizi — hücre agregasyonu + eşleştirilmiş farklar + realized özeti.

Girdi: kapi_calrun.sh çıktı ağacı (<out>/<cell>/repN/<arm>/{out.jsonl, cap/*, realized_*.json}).
Çıktı: <out>/analysis/{runs.csv, cells.csv, pairs.csv} + stdout özeti.

İstatistik çerçevesi (r5 I): n = repeat (mesaj-bazlı örneklem YASAK); eşleştirilmiş tasarım —
aynı cell×rep'te kol çiftleri; mesaj-düzeyi eşleştirme YALNIZ discordant-pair-rate için
(KAPI-2 paired-power girdisi — McNemar'ın b/c hücreleri), CI ÜRETMEZ. Bu araç calibration'da
tolerans/power türetir; evaluation'da aynı şema Holm-katmanıyla genişletilir (KAPI-2).

Maliyet metriği sıfır-teslim-güvenli (r5 I): delivered_unique_payload / injected_ip_bytes
(injected = governor sayacı; bütçesiz koşuda udp_tx + pkt×28 yaklaşıklaması, 'approx' işaretli).
"""

import csv
import json
import statistics
import sys
from pathlib import Path

PRIMARY_KAMA = "kama-tail-v1"
PRIMARY_COMP = "q-dgram-arq"


def load_run(d: Path):
    f = d / "out.jsonl"
    if not f.exists():
        return None
    meta, events = None, []
    for line in f.read_text().splitlines():
        if not line.strip():
            continue
        rec = json.loads(line)
        if "meta" in rec:
            meta = rec["meta"]
        elif "event" in rec:
            events.append(rec["event"])
    if meta is None:
        return None
    return meta, events


def completion(events, deadline_ns):
    """Mesaj-başı deadline (r5 G): latency = delivered − sent ≤ D. Boş → 0."""
    if not events:
        return 0.0, []
    ok = [
        e["delivered_ns"] is not None and e["delivered_ns"] - e["sent_ns"] <= deadline_ns
        for e in events
    ]
    return sum(ok) / len(ok), ok


def injected_bytes(meta):
    b = meta.get("budget") or {}
    if b.get("mode"):
        return b.get("injected_bytes", 0), False
    # Bütçesiz koşu: governor yok → udp_tx (UDP payload) + paket-başı 28B yaklaşıklaması yapılamaz
    # (paket sayısı meta'da yok) → udp_tx olduğu gibi, approx işaretli.
    return meta.get("udp_tx_bytes", 0), True


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: kapi_analyze.py <out_dir>", file=sys.stderr)
        return 2
    root = Path(sys.argv[1])
    an = root / "analysis"
    an.mkdir(exist_ok=True)

    runs = []  # satır-başına bir koşu
    for cell_dir in sorted(p for p in root.iterdir() if p.is_dir() and p.name != "analysis"):
        for rep_dir in sorted(cell_dir.glob("rep*")):
            for arm_dir in sorted(p for p in rep_dir.iterdir() if p.is_dir()):
                # Ön-kayıt §5: INVALID işareti out.jsonl VARKEN de geçerli (exogenous-tolerans
                # ihlali sonradan tespit edilir; veri korunur ama aggregation'a girmez).
                r = None if (arm_dir / "INVALID").exists() else load_run(arm_dir)
                if r is None:
                    kind = "INVALID" if (arm_dir / "INVALID").exists() else "MISSING"
                    runs.append(
                        dict(
                            cell=cell_dir.name,
                            rep=rep_dir.name,
                            arm=arm_dir.name,
                            missing=1,
                            invalid_kind=kind,
                        )
                    )
                    continue
                meta, events = r
                d_ns = meta["max_deadline_ns"]
                comp, ok_vec = completion(events, d_ns)
                delivered = sum(1 for e in events if e["delivered_ns"] is not None)
                payload = delivered * meta["payload_bytes"]
                inj, approx = injected_bytes(meta)
                runs.append(
                    dict(
                        cell=cell_dir.name,
                        rep=rep_dir.name,
                        arm=arm_dir.name,
                        missing=0,
                        transport=meta["transport"],
                        completion=round(comp, 6),
                        delivered=delivered,
                        msgs=meta["msg_count"],
                        delivered_payload=payload,
                        injected_ip_bytes=inj,
                        injected_approx=int(approx),
                        cost=round(payload / inj, 6) if inj else 0.0,
                        budget_mode=(meta.get("budget") or {}).get("mode", ""),
                        refused_pkts=(meta.get("budget") or {}).get("refused_pkts", 0),
                        first_refusal_ns=(meta.get("budget") or {}).get("first_refusal_ns", 0),
                        udp_tx=meta["udp_tx_bytes"],
                        udp_rx=meta["udp_rx_bytes"],
                        ok_vec="".join("1" if o else "0" for o in ok_vec),
                    )
                )

    with open(an / "runs.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=sorted({k for r in runs for k in r}))
        w.writeheader()
        w.writerows(runs)

    # Hücre×kol agregasyonu (n = rep; medyan/IQR/min/max).
    cells = {}
    for r in runs:
        if r.get("missing"):
            continue
        cells.setdefault((r["cell"], r["arm"]), []).append(r)
    cell_rows = []
    for (cell, arm), rs in sorted(cells.items()):
        comps = sorted(x["completion"] for x in rs)
        costs = sorted(x["cost"] for x in rs)
        q = lambda v, p: v[min(len(v) - 1, max(0, round(p * (len(v) - 1))))]
        cell_rows.append(
            dict(
                cell=cell,
                arm=arm,
                n=len(rs),
                completion_median=q(comps, 0.5),
                completion_q1=q(comps, 0.25),
                completion_q3=q(comps, 0.75),
                completion_min=comps[0],
                completion_max=comps[-1],
                cost_median=q(costs, 0.5),
                refused_any=sum(1 for x in rs if x["refused_pkts"] > 0),
            )
        )
    with open(an / "cells.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(cell_rows[0].keys()) if cell_rows else ["cell"])
        w.writeheader()
        w.writerows(cell_rows)

    # Eşleştirilmiş fark (primary çift) + discordant-pair-rate (KAPI-2 power girdisi).
    pair_rows = []
    by_key = {(r["cell"], r["rep"], r["arm"]): r for r in runs if not r.get("missing")}
    cells_set = sorted({r["cell"] for r in runs if not r.get("missing")})
    for cell in cells_set:
        reps = sorted({r["rep"] for r in runs if r["cell"] == cell and not r.get("missing")})
        deltas, b, c, total_msgs = [], 0, 0, 0
        for rep in reps:
            ka = by_key.get((cell, rep, PRIMARY_KAMA))
            qd = by_key.get((cell, rep, PRIMARY_COMP))
            if not ka or not qd:
                continue
            deltas.append(ka["completion"] - qd["completion"])
            for x, y in zip(ka["ok_vec"], qd["ok_vec"]):
                total_msgs += 1
                if x == "1" and y == "0":
                    b += 1
                elif x == "0" and y == "1":
                    c += 1
        if deltas:
            pair_rows.append(
                dict(
                    cell=cell,
                    n_pairs=len(deltas),
                    delta_completion_median=round(statistics.median(deltas), 6),
                    delta_completion_mean=round(statistics.mean(deltas), 6),
                    discordant_b_kama_only=b,
                    discordant_c_comp_only=c,
                    discordant_rate=round((b + c) / total_msgs, 6) if total_msgs else 0.0,
                    pair=f"{PRIMARY_KAMA} vs {PRIMARY_COMP}",
                )
            )
    with open(an / "pairs.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(pair_rows[0].keys()) if pair_rows else ["cell"])
        w.writeheader()
        w.writerows(pair_rows)

    n_inv = sum(1 for r in runs if r.get("invalid_kind") == "INVALID")
    n_mis = sum(1 for r in runs if r.get("missing")) - n_inv
    print(f"runs={len(runs)} (INVALID={n_inv} exogenous, MISSING={n_mis})")
    if n_mis:
        print("  UYARI: MISSING satırlar sansür riskidir (survivorship) — nedenleri açıklanmadan"
              " hücre raporlanamaz [inceleme #17]")
    for row in cell_rows:
        print(
            f"  {row['cell']:44s} {row['arm']:14s} n={row['n']:3d} "
            f"comp med={row['completion_median']:.3f} [{row['completion_min']:.3f},{row['completion_max']:.3f}] "
            f"cost med={row['cost_median']:.4f}"
        )
    for row in pair_rows:
        print(
            f"  PAIR {row['cell']:40s} Δcomp med={row['delta_completion_median']:+.3f} "
            f"discordant b={row['discordant_b_kama_only']} c={row['discordant_c_comp_only']} "
            f"rate={row['discordant_rate']:.4f}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
