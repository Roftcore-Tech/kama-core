#!/usr/bin/env python3
"""[K4G-05b NETEM-7] Ön-kayıtlı analiz — taban vs aday, 7 profil.

Ön-kayıt: ~/kama-k4g-work/k4g05b/netem7/ONKAYIT.md (sonuçlardan ÖNCE sabitlendi; bu betik
ondan sapamaz). RNG/bootstrap yok: kesin iki yanlı Mann–Whitney (orta-sıra, bağ-düzeltmeli
tam permütasyon dağılımı, DP).

M1 (birincil): koşu başına D=20 s içinde teslim oranı (phase5_cell_report.py `comp()` ile aynı).
M2 (ikincil): koşu başına teslim edilenlerin gecikme medyanı (sent→delivered, ms).
KAYMA(M1) ⇔ |Δmedyan| ≥ 0.05 ∧ p < 0.05/7.   KAYMA(M2) ⇔ oran ∉ [0.8, 1.25] ∧ p < 0.05/7.
Altyapı arızası: olay içermeyen koşu → N'e sayılmaz, raporlanır; kol-profil başına > 2 → ALTYAPI-DUR.

Kullanım: netem7_analyze.py OUTDIR
"""
import json
import pathlib
import statistics
import sys

PROFILES = ["p1_loss50", "p2_loss80", "p3_loss95", "p4_jitter", "p5_revpath", "p6_blackout", "p7_bbr"]
D_NS = 20 * 10**9
ALPHA = 0.05 / 7


def events(f):
    out = []
    for line in open(f):
        if '"event"' in line:
            out.append(json.loads(line)["event"])
    return out


def run_metrics(f):
    ev = events(f)
    if not ev:
        return None
    ok = sum(1 for e in ev if e.get("delivered_ns") is not None and e["delivered_ns"] - e["sent_ns"] <= D_NS)
    lat = [(e["delivered_ns"] - e["sent_ns"]) / 1e6 for e in ev if e.get("delivered_ns") is not None]
    return {"m1": ok / len(ev), "m2": statistics.median(lat) if lat else None, "n_ev": len(ev)}


def mwu_exact_two_sided(x, y):
    """Bağ-düzeltmeli kesin permütasyon: x'in orta-sıra toplamı (×2, tamsayı) dağılımı DP ile."""
    pooled = sorted([(v, 0) for v in x] + [(v, 1) for v in y], key=lambda t: t[0])
    n = len(pooled)
    ranks2 = [0] * n  # 2×orta-sıra
    i = 0
    while i < n:
        j = i
        while j + 1 < n and pooled[j + 1][0] == pooled[i][0]:
            j += 1
        r2 = (i + 1) + (j + 1)  # 2 × ortalama sıra
        for k in range(i, j + 1):
            ranks2[k] = r2
        i = j + 1
    m = len(x)
    obs = sum(r for r, (_, g) in zip(ranks2, pooled) if g == 0)
    # dp[k] = {toplam: sayı}
    dp = [dict() for _ in range(m + 1)]
    dp[0][0] = 1
    for r in ranks2:
        for k in range(m - 1, -1, -1):
            for s, c in dp[k].items():
                dp[k + 1][s + r] = dp[k + 1].get(s + r, 0) + c
    dist = dp[m]
    total = sum(dist.values())
    mean2 = m * (n + 1)  # E[2×toplam]
    dev = abs(obs - mean2)
    tail = sum(c for s, c in dist.items() if abs(s - mean2) >= dev)
    return tail / total


def main():
    root = pathlib.Path(sys.argv[1])
    verdict = []
    print(f"# NETEM-7 analiz — {root}")
    for p in PROFILES:
        pd = root / p
        if not (pd / "DONE").exists():
            print(f"\n## {p}: DONE YOK — koşu tamamlanmadı")
            verdict.append((p, "EKSİK"))
            continue
        cfg = json.loads((pd / "cell.json").read_text())
        arms = {}
        infra = {}
        for arm in ("taban", "aday"):
            vals = []
            bad = 0
            for f in sorted(pd.glob(f"{arm}/rep*/out.jsonl"), key=lambda q: int(q.parent.name[3:])):
                rm = run_metrics(f)
                if rm is None:
                    bad += 1
                else:
                    vals.append(rm)
            reps_dirs = len(list(pd.glob(f"{arm}/rep*")))
            bad += reps_dirs - len(list(pd.glob(f"{arm}/rep*/out.jsonl")))
            arms[arm] = vals
            infra[arm] = bad
        print(f"\n## {p}  fwd='{cfg['fwd']}' rev='{cfg['rev']}' geo={cfg['msgs']}x{cfg['payload']}B@{cfg['submit_interval_ms']}ms D=20s")
        print(f"   taban {cfg['sha_taban'][:8]}  aday {cfg['sha_aday'][:8]}")
        res = []
        for arm in ("taban", "aday"):
            m1 = sorted(v["m1"] for v in arms[arm])
            m2 = sorted(v["m2"] for v in arms[arm] if v["m2"] is not None)
            if m1:
                print(f"   {arm:<5} N={len(m1)} altyapı={infra[arm]}  M1 min={m1[0]:.3f} med={statistics.median(m1):.3f} "
                      f"max={m1[-1]:.3f} ≥0.99:{sum(1 for v in m1 if v >= 0.99)}/{len(m1)}  "
                      f"M2 med={statistics.median(m2) if m2 else float('nan'):.0f} ms")
            else:
                print(f"   {arm:<5} N=0 altyapı={infra[arm]}")
        flags = []
        if max(infra.values()) > 2:
            flags.append("ALTYAPI-DUR")
        a1 = [v["m1"] for v in arms["aday"]]
        t1 = [v["m1"] for v in arms["taban"]]
        if a1 and t1:
            d1 = statistics.median(a1) - statistics.median(t1)
            p1 = mwu_exact_two_sided(a1, t1)
            k1 = abs(d1) >= 0.05 and p1 < ALPHA
            print(f"   M1 Δmed={d1:+.3f} p={p1:.4g} → {'KAYMA' if k1 else 'kayma yok'}")
            if k1:
                flags.append(f"KAYMA-M1({'düşüş' if d1 < 0 else 'artış'})")
        a2 = [v["m2"] for v in arms["aday"] if v["m2"] is not None]
        t2 = [v["m2"] for v in arms["taban"] if v["m2"] is not None]
        if a2 and t2:
            ratio = statistics.median(a2) / statistics.median(t2) if statistics.median(t2) > 0 else float("inf")
            p2 = mwu_exact_two_sided(a2, t2)
            k2 = (ratio < 0.8 or ratio > 1.25) and p2 < ALPHA
            print(f"   M2 oran={ratio:.3f} p={p2:.4g} → {'KAYMA' if k2 else 'kayma yok'}")
            if k2:
                flags.append(f"KAYMA-M2({'yavaşlama' if ratio > 1 else 'hızlanma'})")
        verdict.append((p, ",".join(flags) if flags else "kayma yok"))
    print("\n# ÖZET")
    for p, v in verdict:
        print(f"   {p:<12} {v}")
    anyk = any(v not in ("kayma yok",) for _, v in verdict)
    print(f"\n# HÜKÜM (ön-kayıtlı kural): {'KAYMA/EKSİK VAR → DUR, sahibe rapor' if anyk else 'yedi profilde ön-kayıtlı kayma saptanmadı'}")


if __name__ == "__main__":
    main()
