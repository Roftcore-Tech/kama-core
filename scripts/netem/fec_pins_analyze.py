#!/usr/bin/env python3
"""Commit-FK/FL — marjinal-FEC-utility pin-aggregasyonu + utility-sınıflandırması.

ÇERÇEVE: Bu analiz ARQ-vs-FEC KARŞILAŞTIRMASI DEĞİLDİR. ARQ/substrat bütün-kollarda-aktif; ölçülen
ARQ-substratı üzerine eklenen proaktif-FEC-parity'nin marjinal decode-faydasıdır (r>=deficit sınırı).

Girdi: reports/d1-exit/data/fec-pins/*-rxledger.csv (kolon:
  run_id,block,first_ms,complete_ms,rel_ms,dl_op,deficit_applied,decode_via,first_repair_ms,repair_kind)
Yalnız deficit_applied>0 (warm-steady) bloklar sayılır.

Utility-sınıfları (koşul-kapsamlı; DELAY-BAĞIMSIZ kombinatoryal-decodability):
  MarginalFecEssential_isolation   : izolasyonda r-parity blok-tamamlanmasını MÜMKÜN kılar (r>=deficit),
                                     r-1 (veya <deficit) İMKANSIZ (pending). Saf-FEC decode-sınırı.
  NoMarginalFecUtility_thisCondition: hibritte substrat (ARQ/§11) zaten tamamlıyor → düşük-RTT'de
                                     marjinal-LATENCY-fayda ölçülemez (substrat aynı-hızlı; wall-clock
                                     ayrışması harness'ta imkansız).
  HybridResidualArqRequired        : r<deficit → FEC tek-başına yetmez; residual ARQ/§11 tamamlar.
"""
import csv
import glob
import os
import re
import sys
from collections import defaultdict

DATA = sys.argv[1] if len(sys.argv) > 1 else "reports/d1-exit/data/fec-pins"

# run_id şeması: pinN-dD-rR-MODE-REP
RID = re.compile(r"(pin\d)-d(\d+)-r(\d+)-(iso|hyb)-(\d+)")

# agg[(pin,deficit,r,mode)] = {complete, pending, via0, via1, rk_arq, rk_dup, rk_none, reps}
agg = defaultdict(lambda: defaultdict(int))
reps_seen = defaultdict(set)

for path in sorted(glob.glob(os.path.join(DATA, "*-rxledger.csv"))):
    base = os.path.basename(path)[: -len("-rxledger.csv")]
    m = RID.match(base)
    if not m:
        continue
    pin, deficit, r, mode, rep = m.group(1), int(m.group(2)), int(m.group(3)), m.group(4), m.group(5)
    key = (pin, deficit, r, mode)
    reps_seen[key].add(rep)
    with open(path) as f:
        for row in csv.DictReader(f):
            if int(row["deficit_applied"]) == 0:
                continue
            a = agg[key]
            dv = int(row["decode_via"])
            if dv == -1:
                a["pending"] += 1
            else:
                a["complete"] += 1
                a["via0" if dv == 0 else "via1"] += 1
                rk = int(row["repair_kind"])
                a["rk_arq" if rk == 1 else ("rk_dup" if rk == 3 else "rk_none")] += 1

K = 8


def classify(pin, deficit, r, mode, a):
    comp, pend = a["complete"], a["pending"]
    tot = comp + pend
    feasible = (K - deficit) + r >= K  # MDS: r >= deficit
    if mode == "iso":
        if feasible:
            return "MarginalFecDecodable_isolation" if comp == tot and pend == 0 else "INCONSISTENT_iso_feasible"
        return "FecInsufficient_isolation_pending" if pend == tot and comp == 0 else "INCONSISTENT_iso_infeasible"
    # hibrit
    if comp == tot and pend == 0:
        if feasible:
            return "NoMarginalFecUtility_thisCondition(substrate_completes;lowRTT)"
        return "HybridResidualArqRequired(FEC+substrate)"
    return f"INCONSISTENT_hyb(comp={comp},pend={pend})"


print("BU ANALİZ ARQ-vs-FEC DEĞİL: ARQ/substrat bütün-kollarda-aktif; ölçülen marjinal-proaktif-FEC-faydası.\n")
print(f"{'pin':5} {'def':3} {'r':2} {'mode':4} {'N':4} {'comp':5} {'pend':5} "
      f"{'via0':5} {'via1':5} {'rk:arq/dup/none':16} {'feas':5} class")
print("-" * 120)
for key in sorted(agg):
    pin, deficit, r, mode = key
    a = agg[key]
    tot = a["complete"] + a["pending"]
    feas = "YES" if (K - deficit) + r >= K else "no"
    rk = f"{a['rk_arq']}/{a['rk_dup']}/{a['rk_none']}"
    print(f"{pin:5} {deficit:<3} {r:<2} {mode:4} {tot:<4} {a['complete']:<5} {a['pending']:<5} "
          f"{a['via0']:<5} {a['via1']:<5} {rk:16} {feas:5} {classify(pin, deficit, r, mode, a)}")

print("\n=== PIN-AYRIŞMA ÖZETİ (FEC-IZOLASYON; mekanik-ayrışma kapısı) ===")
for pin, deficit, lo, hi in [("pin1", 1, 0, 1), ("pin2", 2, 1, 2), ("pin3", 4, 2, 4)]:
    a_lo = agg.get((pin, deficit, lo, "iso"), {})
    a_hi = agg.get((pin, deficit, hi, "iso"), {})
    lo_pend = a_lo.get("pending", 0)
    lo_comp = a_lo.get("complete", 0)
    hi_comp = a_hi.get("complete", 0)
    hi_pend = a_hi.get("pending", 0)
    sep = (lo_comp == 0 and lo_pend > 0) and (hi_pend == 0 and hi_comp > 0)
    verdict = "SEPARATES ✓" if sep else "DID-NOT-SEPARATE ✗"
    print(f"  {pin} deficit={deficit}: r={lo}(ARQ+{lo}) pending={lo_pend}/comp={lo_comp} | "
          f"r={hi}(ARQ+{hi}) comp={hi_comp}/pend={hi_pend} → {verdict}")
