#!/usr/bin/env python3
"""Gömülü beklenen-değer kapısı (C1.4) — conformance testlerinde beklenen byte KODA YAZILAMAZ.

    scripts/ci/no_inline_expected.py [--min-files N]
    scripts/ci/no_inline_expected.py --selftest

NEDEN. Conformance testinin beklediği byte'lar `vectors/` ağacından gelir ve o ağaç Rust'tan
bağımsız üretilir (`ref/crypto/gen_vectors.py`). Beklenen değer test dosyasına hex literal olarak
yazılırsa iki şey olur: (1) değerin kaynağı görünmez olur — implementasyonun kendi çıktısından
kopyalanmış olabilir; (2) spec değişince vektör güncellenir ama gömülü kopya sessizce eskir.

KAPSAM (yalnız conformance testleri):
    crates/*/tests/conf_*.rs            crates/*/tests/conf_*/**/*.rs
    crates/*/tests/*_conformance_core.rs  crates/*/tests/*_conformance_core/**/*.rs
`conf_*` sprint planının adıdır (C1.4); `<seam>_conformance_core` R-7(c) kararıyla inen addır.
İkisi de taranır — yalnız `conf_*` taransaydı kapı bugün HİÇBİR dosyaya bakmazdı.
KAPSAM DIŞI: `tv_*`, `golden*` ve diğer tüm testler (onlar bilinçli olarak gömülü golden taşır).

YASAK (yorum-dışı kodda):
    E1  hex dizge literali: yalnız hex karakter, ≥ 16 karakter          "0a1b2c3d4e5f6071"
    E2  byte-dizge literali: ≥ 8 `\\x..` kaçışı                          b"\\x01\\x02..."
    E3  bayt dizisi literali: ≥ 8 ardışık `0x..` elemanı                 [0x01, 0x02, ...]
MUAF: protokol sabitleri. Kısa oldukları için çoğu eşiğin altındadır ("SSD1", 4 B magic). Eşiği
aşan gerçek bir protokol sabiti için satıra ya da bir üst satıra GEREKÇELİ işaret konur:
    // inline-expected-ok: <neden bu bir beklenen-çıktı değil, protokol sabiti>
Gerekçesiz işaret (iki noktadan sonra boş) sayılmaz.

Çıkış: 0 = temiz · 1 = ihlal · 2 = ortam/kullanım hatası (kapsamda beklenenden az dosya dahil).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MARK = re.compile(r"inline-expected-ok:\s*\S")

RULES = (
    ("E1", "hex dizge literali (≥16 hex karakter)", re.compile(r'(?<![A-Za-z0-9_])"((?:[0-9a-fA-F]{2}){8,})"')),
    ("E2", "byte-dizge literali (≥8 \\x kaçışı)", re.compile(r'b"(?:[^"\\]|\\.)*?(?:\\x[0-9a-fA-F]{2}(?:[^"\\]|\\.)*?){8,}"')),
    ("E3", "bayt dizisi literali (≥8 ardışık 0x.. elemanı)", re.compile(r"\[\s*(?:0x[0-9a-fA-F]{1,2}(?:_?u8)?\s*,\s*){7,}0x[0-9a-fA-F]{1,2}")),
)


def in_scope(rel: Path) -> bool:
    parts = rel.parts
    if "tests" not in parts or rel.suffix != ".rs":
        return False
    after = parts[parts.index("tests") + 1:]
    if not after:
        return False
    head = after[0]
    stem = head[:-3] if head.endswith(".rs") else head
    return stem.startswith("conf_") or stem.endswith("_conformance_core")


def blank_comments(src: str) -> str:
    """Yorumları AYNI UZUNLUKTA boşlukla değiştirir (satır numaraları ve sütunlar korunur)."""
    out, i, n = list(src), 0, len(src)
    while i < n:
        if src[i] == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            i = j + 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            for k in range(i, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        else:
            i += 1
    return "".join(out)


def scan(path: str, src: str) -> list[str]:
    findings = []
    code = blank_comments(src)
    raw_lines = src.split("\n")
    for rule, what, rx in RULES:
        for m in rx.finditer(code):
            line = code.count("\n", 0, m.start())          # 0-tabanlı
            here = raw_lines[line] if line < len(raw_lines) else ""
            above = raw_lines[line - 1] if line > 0 else ""
            if MARK.search(here) or MARK.search(above):
                continue
            findings.append(f"{path}:{line + 1}: {rule} {what} — beklenen değer vectors/ ağacından okunmalı")
    return findings


def selftest() -> int:
    hx = "00112233445566778899aabbccddeeff"
    cases = [
        ("temiz: vektörden okuyor", 'let want = case.expected();\nassert_eq!(got, want);\n', []),
        ("E1 hex dizge", f'assert_eq!(hex(&got), "{hx}");\n', ["E1"]),
        ("E1 değil: kısa magic", 'assert_eq!(&aad[..4], b"SSD1");\nlet m = "53534431";\n', []),
        ("E1 değil: hex olmayan uzun dizge", 'let s = "this is not a hex literal at all";\n', []),
        ("E1 değil: yorumda", f'// örnek: "{hx}"\nlet x = 1;\n', []),
        ("E2 byte-dizge", 'let w = b"\\x01\\x02\\x03\\x04\\x05\\x06\\x07\\x08";\n', ["E2"]),
        ("E3 bayt dizisi", "let w = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];\n", ["E3"]),
        ("E3 değil: 7 eleman", "let w = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];\n", []),
        ("muaf: gerekçeli işaret aynı satırda", f'let k = "{hx}"; // inline-expected-ok: RFC 8439 §2.8.2 anahtarı, girdi\n', []),
        ("muaf: gerekçeli işaret üst satırda", f'// inline-expected-ok: protokol sabiti (ccr §35.9 domain separator)\nlet k = "{hx}";\n', []),
        ("muaf DEĞİL: gerekçesiz işaret", f'let k = "{hx}"; // inline-expected-ok:\n', ["E1"]),
    ]
    bad = 0
    for name, src, want in cases:
        got = sorted({f.split(": ", 1)[1].split()[0] for f in scan("t.rs", src)})
        ok = got == sorted(want)
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    scope = [
        ("crates/a/tests/conf_s1_roundtrip.rs", True), ("crates/a/tests/conf_s3/main.rs", True),
        ("crates/a/tests/seam_conformance_core/main.rs", True), ("crates/a/tests/noise_conformance_core.rs", True),
        ("crates/a/tests/tv_arq.rs", False), ("crates/a/tests/golden_wire.rs", False),
        ("crates/a/tests/seam_mock_fixture/main.rs", False), ("crates/a/src/conf_x.rs", False),
    ]
    for rel, want in scope:
        ok = in_scope(Path(rel)) == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  kapsam {rel}: {'içeride' if want else 'dışarıda'}")
    print(f"no_inline_expected öz-test: {'YEŞİL' if not bad else f'{bad} BAŞARISIZ'}")
    return 1 if bad else 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()
    min_files = 0
    if "--min-files" in argv:
        try:
            min_files = int(argv[argv.index("--min-files") + 1])
        except (IndexError, ValueError):
            print("kullanım: --min-files N", file=sys.stderr)
            return 2
    files = sorted(p for p in (ROOT / "crates").rglob("*.rs")
                   if "target" not in p.parts and in_scope(p.relative_to(ROOT)))
    if len(files) < min_files:
        print(f"ORTAM HATASI: kapsamda {len(files)} dosya var, en az {min_files} bekleniyordu — "
              "kapı boşa koşuyor olabilir (ad kalıbı değişti mi?)", file=sys.stderr)
        return 2
    findings = []
    for f in files:
        findings += scan(str(f.relative_to(ROOT)), f.read_text(encoding="utf-8"))
    for x in findings:
        print(f"::error title=INLINE-EXPECTED::{x}")
        print(f"KIRMIZI: {x}")
    if findings:
        print(f"\nNO-INLINE-EXPECTED: KIRMIZI ({len(findings)} bulgu, {len(files)} dosya)")
        return 1
    print(f"NO-INLINE-EXPECTED: YEŞİL ({len(files)} conformance dosyası tarandı)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
