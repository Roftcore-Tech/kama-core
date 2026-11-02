#!/usr/bin/env python3
"""[K4G-05b NETEM-7] SONRADAN EKLENEN BETİMSEL ANALİZ — karar kuralını DEĞİŞTİRMEZ.

Ön-kayıt (ONKAYIT.md, sha256 6d667853…) karar kuralını tanımlar; bu betik sonuçlar görüldükten SONRA,
sahibin CEVAP PAKETİ 16 v2 §1.3 isteğiyle ("M2'nin anlamı/yönü ve belirsizlik aralığı yayımlanır")
yazıldı. Çıktısı yalnız betimseldir; hiçbir hücre bu çıktıya göre KAYMA/ kayma yok sınıfına alınmaz.

Yöntem (RNG/bootstrap yok):
  - Konum farkı Hodges–Lehmann kestirimi: ikili farkların (aday − taban) medyanı.
  - Güven aralığı: Mann–Whitney tersinmesi (Bauer 1972) — sıralı ikili farklar d_(k) … d_(N+1−k),
    k = bağsız kesin U dağılımından P(U ≤ k−1) ≤ α/2 olan en büyük k. Bağ varken aralık yaklaşıktır
    (bağsız dağılım kullanılır; bağlı veride kapsama nominalin üstünde ya da altında olabilir) — beyan.
  - M1 (D = 20 s içinde teslim oranı) için fark ölçeğinde.
  - M2 (teslim edilenlerin gecikme medyanı) için log ölçeğinde fark → exp → ORAN (aday / taban);
    oran > 1 = aday YAVAŞ. M2 yalnız en az bir teslim olan koşularda tanımlıdır; tanımlı koşu sayısı basılır.
  - İki düzey: %95 (alışılmış) ve 1 − 0,05/7 (ön-kayıtlı karar kuralının α'sı).

Kullanım: netem7_ci.py RUN_DIR CSV_ÇIKTI  →  stdout (Markdown tablo) ve CSV_ÇIKTI (koşu başına satır; ham dizine yazılmaz)
"""
import csv
import json
import math
import pathlib
import statistics
import sys

PROFILES = ["p1_loss50", "p2_loss80", "p3_loss95", "p4_jitter", "p5_revpath", "p6_blackout", "p7_bbr"]
D_NS = 20 * 10**9
ALPHAS = [("%95", 0.05), ("1−0,05/7", 0.05 / 7)]


def run_metrics(f):
    ev = [json.loads(line)["event"] for line in open(f) if '"event"' in line]
    if not ev:
        return None
    ok = sum(1 for e in ev if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D_NS)
    lat = [(e["delivered_ns"] - e["sent_ns"]) / 1e6 for e in ev if e.get("delivered_ns") is not None]
    return {"m1": ok / len(ev), "m2": statistics.median(lat) if lat else None,
            "n_ev": len(ev), "n_delivered": len(lat)}


def u_null_counts(m, n):
    """Bağsız Mann–Whitney U'nun kesin null dağılımı: cnt[u] = U = u veren dizilim sayısı."""
    # f[i][j] = liste: i x-elemanı, j y-elemanı ile U dağılımı
    prev = [[1] for _ in range(n + 1)]  # i = 0: U = 0 tek
    for i in range(1, m + 1):
        cur = [[1]]  # j = 0
        for j in range(1, n + 1):
            a = cur[j - 1]                 # son eleman y → U değişmez
            b = prev[j]                    # son eleman x → U += j
            size = max(len(a), len(b) + j)
            row = [0] * size
            for u, c in enumerate(a):
                row[u] += c
            for u, c in enumerate(b):
                row[u + j] += c
            cur.append(row)
        prev = cur
    return prev[n]


def hl_ci(x, y, alpha):
    """y − x konum farkı: Hodges–Lehmann kestirimi + Mann–Whitney tersinmesi güven aralığı."""
    d = sorted(yi - xi for yi in y for xi in x)
    N = len(d)
    est = statistics.median(d)
    cnt = u_null_counts(len(x), len(y))
    tot = sum(cnt)
    cum = 0
    k = 0
    for u, c in enumerate(cnt):
        if (cum + c) / tot > alpha / 2:
            break
        cum += c
        k = u + 1
    if k < 1:
        return est, None, None
    return est, d[k - 1], d[N - k]


def fmt(v, nd=3):
    return "—" if v is None else f"{v:.{nd}f}"


def main():
    root = pathlib.Path(sys.argv[1])
    rows = []
    out = ["| profil | metrik | n taban / aday | kestirim | %95 aralık | 1−0,05/7 aralık |",
           "|---|---|---|---|---|---|"]
    for p in PROFILES:
        arms = {}
        for arm in ("taban", "aday"):
            vals = []
            for f in sorted((root / p).glob(f"{arm}/rep*/out.jsonl"), key=lambda q: int(q.parent.name[3:])):
                rep = int(f.parent.name[3:])
                rc = (f.parent / "rc").read_text().strip() if (f.parent / "rc").exists() else ""
                rm = run_metrics(f)
                rows.append({"profil": p, "kol": arm, "rep": rep, "rc": rc,
                             "olay": rm["n_ev"] if rm else 0,
                             "teslim": rm["n_delivered"] if rm else 0,
                             "M1": f"{rm['m1']:.4f}" if rm else "",
                             "M2_ms": f"{rm['m2']:.1f}" if rm and rm["m2"] is not None else ""})
                if rm:
                    vals.append(rm)
            arms[arm] = vals
        t1 = [v["m1"] for v in arms["taban"]]
        a1 = [v["m1"] for v in arms["aday"]]
        cis = [hl_ci(t1, a1, al) for _, al in ALPHAS]
        out.append(f"| {p} | M1 Δ (aday − taban) | {len(t1)} / {len(a1)} | {fmt(cis[0][0])} | "
                   f"[{fmt(cis[0][1])}, {fmt(cis[0][2])}] | [{fmt(cis[1][1])}, {fmt(cis[1][2])}] |")
        t2 = [math.log(v["m2"]) for v in arms["taban"] if v["m2"]]
        a2 = [math.log(v["m2"]) for v in arms["aday"] if v["m2"]]
        if t2 and a2:
            cis2 = [hl_ci(t2, a2, al) for _, al in ALPHAS]

            def ex(v):
                return None if v is None else math.exp(v)
            out.append(f"| {p} | M2 oranı (aday / taban; > 1 = aday yavaş) | {len(t2)} / {len(a2)} | "
                       f"{fmt(ex(cis2[0][0]), 2)} | [{fmt(ex(cis2[0][1]), 2)}, {fmt(ex(cis2[0][2]), 2)}] | "
                       f"[{fmt(ex(cis2[1][1]), 2)}, {fmt(ex(cis2[1][2]), 2)}] |")
    with open(sys.argv[2], "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["profil", "kol", "rep", "rc", "olay", "teslim", "M1", "M2_ms"])
        w.writeheader()
        w.writerows(rows)
    print("\n".join(out))
    print(f"\n(kosular.csv: {len(rows)} satır)")


if __name__ == "__main__":
    main()
