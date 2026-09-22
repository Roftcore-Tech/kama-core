#!/usr/bin/env python3
"""Spec ↔ vektör izlenebilirlik kapısı (T-00.2).

PROBLEM. `validate.py` dosyaların BİÇİMİNİ denetler; KAPSAMINI denetlemez. ccr-format-spec
§30.7 adı geçen 24 negatif vaka tanımlıyor (INVALID-1..22 + DEPRECATED-REJECT-1/2). Hiçbiri
için vektör yazılmasa bile şema kapısı yemyeşil kalır — "testler geçiyor" cümlesi kapsam
hakkında hiçbir şey söylemez.

ÇÖZÜM — RATCHET (cırcır). Spec'ten adlandırılmış vaka kimlikleri ÇIKARILIR, `vectors/`
ağacındaki `spec_ref` alanlarıyla eşleştirilir ve sonuç bir taban dosyasına (`vectors/
spec-coverage.json`) karşı kıyaslanır. Kapsam DÜŞERSE kapı kırmızı olur. Yükseltmek için
taban `--update` ile yeniden yazılır. Böylece bugünkü eksik kapsam görünür ve borç yalnız
azalabilir; "sonra yazarız" sessizce kalıcı olamaz.

Kullanım:
    python3 scripts/vectors/spec_coverage.py            # rapor + ratchet kapısı
    python3 scripts/vectors/spec_coverage.py --report   # yalnız rapor (kapı yok)
    python3 scripts/vectors/spec_coverage.py --update   # tabanı yükselt (kapsam arttıysa)

Çıkış kodu: 0 = kapsam taban seviyesinde ya da üstünde · 1 = kapsam DÜŞTÜ · 2 = ortam hatası.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "scripts" / "vectors" / "spec-coverage.json"

# Spec'te adı geçen negatif-korpus etiketleri. Desen KASITLI olarak dar: yalnız §30.7'nin
# kullandığı iki biçim. Yeni bir korpus ailesi eklenirse bu liste bilinçli genişletilir —
# geniş bir desen spec metnindeki rastgele büyük harfleri "vaka" sanardı.
CORPUS_PATTERNS = [
    ("ccr-format-spec.md", re.compile(r"\b(INVALID-\d+)\b")),
    ("ccr-format-spec.md", re.compile(r"\b(DEPRECATED-REJECT-\d+)\b")),
]


def spec_case_ids() -> dict[str, list[str]]:
    """Spec dosyalarından adlandırılmış vaka kimliklerini çıkarır."""
    out: dict[str, list[str]] = {}
    for filename, pattern in CORPUS_PATTERNS:
        path = ROOT / "specs" / filename
        if not path.is_file():
            print(f"spec bulunamadı: {path}", file=sys.stderr)
            continue
        found = sorted(set(pattern.findall(path.read_text(encoding="utf-8"))), key=_natural)
        out.setdefault(filename, []).extend(found)
    return {k: sorted(set(v), key=_natural) for k, v in out.items()}


def _natural(label: str) -> tuple[str, int]:
    head, _, num = label.rpartition("-")
    return head, int(num) if num.isdigit() else 0


def covered_ids() -> dict[str, str]:
    """`vectors/` ağacındaki `spec_ref` alanlarından kapsanan vaka kimlikleri.

    Anahtar = vaka etiketi, değer = onu kapsayan ilk vektör `id`'si.
    """
    out: dict[str, str] = {}
    for path in sorted((ROOT / "vectors").rglob("*.json")):
        if path.name == "SCHEMA.json":
            continue
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        for case in doc.get("cases", []) if isinstance(doc, dict) else []:
            ref = case.get("spec_ref")
            if not isinstance(ref, str):
                continue
            for _, pattern in CORPUS_PATTERNS:
                for label in pattern.findall(ref):
                    out.setdefault(label, case.get("id", "?"))
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Spec ↔ vektör kapsam ratchet'i")
    ap.add_argument("--report", action="store_true", help="yalnız rapor; kapı uygulanmaz")
    ap.add_argument("--update", action="store_true", help="tabanı yükselt")
    ap.add_argument(
        "--baseline",
        default=str(BASELINE),
        help="taban dosyası (öz-testte ratchet'in gerçekten kırmızı olduğunu kanıtlamak için)",
    )
    args = ap.parse_args(argv)
    baseline_path = Path(args.baseline)

    expected = spec_case_ids()
    covered = covered_ids()
    all_labels = [label for labels in expected.values() for label in labels]
    if not all_labels:
        print("spec'ten hiç vaka etiketi çıkarılamadı — desen bozulmuş olabilir", file=sys.stderr)
        return 2

    hit = [x for x in all_labels if x in covered]
    miss = [x for x in all_labels if x not in covered]

    print(f"spec vakası: {len(all_labels)} · kapsanan: {len(hit)} · eksik: {len(miss)}")
    for label in hit:
        print(f"  ✓ {label:22} {covered[label]}")
    if miss:
        print("  eksik: " + ", ".join(miss))

    current = {
        "comment": (
            "RATCHET TABANI — elle düzenlemeyin. "
            "`python3 scripts/vectors/spec_coverage.py --update` ile yükseltilir. "
            "`covered` listesi KÜÇÜLEMEZ: kapsam düşerse CI kırmızı olur."
        ),
        "total": len(all_labels),
        "covered": sorted(hit, key=_natural),
        "missing": sorted(miss, key=_natural),
    }

    if args.update:
        baseline_path.write_text(json.dumps(current, indent=2, ensure_ascii=True) + "\n", encoding="utf-8")
        print(f"taban yazıldı: {baseline_path}")
        return 0

    if args.report:
        return 0

    if not baseline_path.is_file():
        print(f"taban yok: {baseline_path} — önce --update", file=sys.stderr)
        return 2

    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    lost = sorted(set(baseline.get("covered", [])) - set(hit), key=_natural)
    if lost:
        print(
            "KAPSAM DÜŞTÜ — tabanda kapsanan şu vakalar artık kapsanmıyor: "
            + ", ".join(lost),
            file=sys.stderr,
        )
        return 1

    gained = sorted(set(hit) - set(baseline.get("covered", [])), key=_natural)
    if gained:
        print("kapsam ARTTI: " + ", ".join(gained))
        print("tabanı yükseltin: python3 scripts/vectors/spec_coverage.py --update")
    return 0


if __name__ == "__main__":
    sys.exit(main())
