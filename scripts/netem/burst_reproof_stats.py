#!/usr/bin/env python3
"""[BURST-REPROOF-V1 §3] Dondurulmuş analiz — profil başına eşli Δ + paired-bootstrap %95 CI.

Ön-kayıt: `reports/burst-reproof-v1/PRE-REGISTRATION.md` (2026-08-26, sonuç görülmeden donduruldu).

YÖNTEM-ÖZDEŞLİĞİ: bootstrap `kapi_eval_stats.boot` fonksiyonundan **import edilir** (yeniden
yazılmaz): B=10_000, tohum = SHA256("KAPI2-BOOT" ‖ cell_id) — ön-kayıttaki "kapi-eval-v1 ile
aynı yöntem; RNG tohumu rapor edilir" şartı.

KARAR KURALI (ön-kayıt §3, DEĞİŞTİRİLEMEZ — profil başına):
    CI alt-sınır ≥ 0   -> NOT-WORSE      (sevk Kama jammer-burst'te quinn'i yakalıyor)
    CI üst-sınır  < 0   -> STILL-WORSE    (ARQ-fix yetersiz -> V2/FEC gerekli)
    0'ı kapsıyor        -> INCONCLUSIVE @ n=10
NOT: Holm düzeltmesi bu ön-kayıtta YOK (kapi-eval-v1'den farklı); karar yalnız CI tabanlıdır.
Holm'süz olması, üç profilin AYRI hipotezler değil aynı sorunun kademeli şiddeti olmasındandır.

İKİNCİL (ön-kayıtlı, bağlam — hipotez üretmez):
  (1) tail katkısı: kama-tail-v1 − kama-arq eşli Δ
  (2) maliyet (byte-verimi) eşli Δ — tek başına iddia taşımaz

Kullanım: burst_reproof_stats.py <out_dir>   (analysis/runs.csv okur; FINAL-VERDICTS.csv yazar)
"""

import csv
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from kapi_eval_stats import boot  # noqa: E402  — yöntem-özdeşliği: aynı bootstrap

CELLS = [
    "exploratory_h3a_burst40_paced_A2_d120_u0.75",
    "exploratory_h3b_burst50_paced_A2_d120_u0.75",
    "exploratory_h3c_burst60_paced_A2_d120_u0.75",
]
K, C, TAIL_OFF = "kama-tail-v1", "q-dgram-arq", "kama-arq"


def verdict(lo: float, hi: float) -> str:
    if lo >= 0:
        return "NOT-WORSE"
    if hi < 0:
        return "STILL-WORSE"
    return "INCONCLUSIVE@n=10"


def main() -> int:
    root = Path(sys.argv[1])
    rows = list(csv.DictReader(open(root / "analysis/runs.csv")))
    by = {
        (r["cell"], r["rep"], r["arm"]): r
        for r in rows
        if r.get("missing") == "0" and r.get("invalid", "0") == "0"
    }

    out = []
    for cell in CELLS:
        reps = sorted({r["rep"] for r in rows if r["cell"] == cell and r.get("missing") == "0"})
        d_comp, d_cost, d_tail = [], [], []
        for rep in reps:
            a, b, t = by.get((cell, rep, K)), by.get((cell, rep, C)), by.get((cell, rep, TAIL_OFF))
            if a and b:
                d_comp.append(float(a["completion"]) - float(b["completion"]))
                d_cost.append(float(a["cost"]) - float(b["cost"]))
            if a and t:
                d_tail.append(float(a["completion"]) - float(t["completion"]))
        lo, hi, p = boot(d_comp, f"KAPI2-BOOT|{cell}")
        clo, chi, _ = boot(d_cost, f"KAPI2-BOOT-COST|{cell}")
        tlo, thi, _ = boot(d_tail, f"KAPI2-BOOT-TAIL|{cell}") if d_tail else (0.0, 0.0, 1.0)
        rec = {
            "cell": cell,
            "n": len(d_comp),
            "mean_delta": round(statistics.fmean(d_comp), 4),
            "median_delta": round(statistics.median(d_comp), 4),
            "ci_lo": round(lo, 4),
            "ci_hi": round(hi, 4),
            "boot_p": round(p, 4),
            "verdict": verdict(lo, hi),
            "cost_mean_delta": round(statistics.fmean(d_cost), 4),
            "cost_ci_lo": round(clo, 4),
            "cost_ci_hi": round(chi, 4),
            "tail_mean_delta": round(statistics.fmean(d_tail), 4) if d_tail else "",
            "tail_ci_lo": round(tlo, 4),
            "tail_ci_hi": round(thi, 4),
        }
        out.append(rec)
        print(
            f"{cell}\n  n={rec['n']} Δ̂={rec['mean_delta']:+.4f} med={rec['median_delta']:+.4f} "
            f"CI95=[{rec['ci_lo']:+.4f},{rec['ci_hi']:+.4f}] boot-p={rec['boot_p']:.4f} "
            f"=> {rec['verdict']}\n"
            f"  [ikincil] maliyet Δ̂={rec['cost_mean_delta']:+.4f} "
            f"CI95=[{rec['cost_ci_lo']:+.4f},{rec['cost_ci_hi']:+.4f}] | "
            f"tail katkısı (ON−OFF) Δ̂={rec['tail_mean_delta']} "
            f"CI95=[{rec['tail_ci_lo']:+.4f},{rec['tail_ci_hi']:+.4f}]"
        )

    dst = root / "analysis/FINAL-VERDICTS.csv"
    with open(dst, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(out[0].keys()))
        w.writeheader()
        w.writerows(out)
    print(f"\nyazıldı: {dst}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
