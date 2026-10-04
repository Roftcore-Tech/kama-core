#!/usr/bin/env python3
"""Branch kapsamı ratchet kapısı.

    scripts/ci/branch-coverage-gate.py <llvm-cov.json> <coverage-floor.json>

NEDEN VAR. Eski adım `cargo llvm-cov --fail-under-branches 78` idi; cargo-llvm-cov'da böyle bir
seçenek YOKTUR (yalnız --fail-under-lines / -functions / -regions). İş kurulduğu günden beri
"error: invalid option '--fail-under-branches'" ile ~1 dakikada düşüyordu, yani branch kapsamı
HİÇ ölçülmedi ve 78 sayısı ölçülmemiş bir tahmindi.

BU KAPI: yüzdeyi llvm-cov'un JSON özetinden okur ve repoda duran TABANLA karşılaştırır.
  · taban null  → RAPOR-ONLY: ölçülen değer yazılır, kapı yeşil (ilk ölçümden önce eşik uydurulmaz).
  · taban sayı  → ölçülen < taban ise KIRMIZI. Taban yalnız elle, ölçülmüş bir değere yükseltilir.

Çıkış: 0 = geçti/rapor · 1 = taban altı · 2 = girdi hatası.
"""

import json
import sys


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    try:
        cov = json.load(open(sys.argv[1], encoding="utf-8"))
        floor_doc = json.load(open(sys.argv[2], encoding="utf-8"))
        b = cov["data"][0]["totals"]["branches"]
        count, covered, pct = int(b["count"]), int(b["covered"]), float(b["percent"])
    except (OSError, ValueError, KeyError, IndexError) as e:
        print(f"::error title=COVERAGE-GATE::girdi okunamadı: {e!r}")
        return 2

    if count == 0:
        # --branch verilmediyse ya da hiçbir test koşmadıysa sayaç 0 gelir; bu "yüzde 0" değil,
        # ÖLÇÜLMEDİ demektir ve sessizce yeşil geçmemeli.
        print("::error title=COVERAGE-GATE::branch sayacı 0 — kapsam ölçülmedi (--branch eksik ya da test koşmadı)")
        return 2

    floor = floor_doc.get("branch_percent_floor")
    print(f"branch kapsamı: {covered}/{count} = {pct:.2f} %  (taban: {floor})")
    summary = f"branch kapsamı **{pct:.2f} %** ({covered}/{count}); taban: {floor}"

    import os

    out = os.environ.get("GITHUB_STEP_SUMMARY")
    if out:
        with open(out, "a", encoding="utf-8") as f:
            f.write(summary + "\n")

    if floor is None:
        print("::notice title=COVERAGE-GATE::taban henüz konmadı — RAPOR-ONLY; ölçülen değer tabana yazılmalı")
        return 0
    if pct + 1e-9 < float(floor):
        print(f"::error title=COVERAGE-GATE::branch kapsamı {pct:.2f} % < taban {floor} %")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
