#!/usr/bin/env python3
"""EK (POST-HOC, BETİMSEL) tel-maliyeti tablosu — donmuş analizin PARÇASI DEĞİLDİR; hüküm üretmez.

Donmuş analizör `wire_bytes` medyanını TÜM satırlar üzerinden verir; tamamlanmayan koşu daha az bayt
harcadığı için bu, kollar farklı oranda tamamladığında yanıltıcıdır. Bu ek iki betimsel büyüklük verir:
  (a) yalnız horizon-complete satırlarda tel baytı medyanı ve yük baytına oranı,
  (b) TÜM satırlarda  Σ tel baytı / Σ teslim edilen yük baytı  (teslim başına maliyet).
Kullanım: kvt_supplement.py --result R1=... [--result R2=...] --out FILE.md
"""
import collections, csv, statistics, sys

def main():
    a = sys.argv[1:]; res = [a[i + 1].split("=", 1)[1] for i, x in enumerate(a) if x == "--result"]
    out = a[a.index("--out") + 1]; rows = []
    for p in res: rows.extend(csv.DictReader(open(p)))
    g = collections.defaultdict(list)
    for r in rows:
        if r["group"] == "main" and r["mode"] == "warm" and r["status"] == "OK": g[(int(r["loss_pct"]), r["rtt_class"], r["workload"], r["arm"])].append(r)
    L = ["## EK — Tel maliyeti (post-hoc, betimsel; hüküm DEĞİL)\n",
         "Tel baytı = istemci veth arayüzünde (va0) koşu boyunca tx+rx bayt farkı; ısıtma trafiği DAHİL (iki kolda aynı kural).\n",
         "| kayıp | RTT | yük | Kama tamamlananlarda tel/yük (n) | TCP tamamlananlarda tel/yük (n) | Kama Σtel/Σteslim-yük (tüm) | TCP Σtel/Σteslim-yük (tüm) |", "|---|---|---|---|---|---|---|"]
    order = {"low": 0, "med": 1, "high": 2}
    for loss, rc, wl in sorted({k[:3] for k in g}, key=lambda k: (k[0], order[k[1]], k[2])):
        cells = []
        for arm in ("kama-v1-full", "tcp-std"):
            v = g.get((loss, rc, wl, arm), []); pay = int(v[0]["msgs"]) * int(v[0]["payload"]) if v else 1
            c = [int(r["wire_bytes"]) for r in v if r["complete_horizon"] == "1"]
            tw = sum(int(r["wire_bytes"]) for r in v); td = sum(int(r["delivered"] or 0) * int(r["payload"]) for r in v)
            cells.append((f"{statistics.median(c) / pay:.2f}× ({len(c)})" if c else "— (0)", f"{tw / td:.2f}×" if td else "—"))
        L.append(f"| {loss}% | {rc} | {wl} | {cells[0][0]} | {cells[1][0]} | {cells[0][1]} | {cells[1][1]} |")
    open(out, "w").write("\n".join(L) + "\n")

if __name__ == "__main__":
    main()
