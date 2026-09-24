#!/usr/bin/env python3
"""KM-SEAM-AT-001 drift kapisi — spec ↔ test-yerel ayna ↔ kapsam beyani.

    scripts/ci/seam-api-drift.py [--spec P] [--mirror P] [--coverage P] [--quiet]

NEDEN VAR. `crates/kama-integration/tests/common/seam/api.rs` sozlesmenin §1/§3 bloklarinin
test-yerel aynasidir (produksiyon trait'i henuz yok; SEAM-OPEN-4). Ayna, dogasi geregi ikinci bir
hakikattir ve ikinci hakikatler sessizce eskir. Fowler'in contract-test gerekcesi tam olarak bu:
double, tedarikciden ayrisirsa testler yesil kalir ama hicbir sey kanitlamaz. Bu kapi ayrismayi
KIRMIZI yapar.

DENETLENEN DORT ESITLIK:
  A. §1 trait adi + metot kumesi        == aynadaki trait adi + metot kumesi
  B. §1'de bildirilen tipler            ⊆ aynadaki tipler   (ayna alias/yardimci ekleyebilir)
  C. §3 TransportError variant kumesi   == aynadaki variant kumesi   (kume KAPALIDIR)
  D. §6 tablosundaki H-* satir kimlikleri == seam-coverage.json rows[].id
  E. tek hedef (mock) kayitliyken hicbir satir 'implemented' beyan EDILEMEZ (yalniz 'mock_only')

Ek olarak beyan dosyasinin kendi tutarliligi (tekrarsiz kimlik, gecerli durum, ratchet tabani).

Cikis kodu: 0 = ayrisma yok · 1 = drift (kapi kirmizi) · 2 = ortam/kullanim hatasi.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SPEC = ROOT / "docs/seams/KM-SEAM-AT-001.md"
DEFAULT_MIRROR = ROOT / "crates/kama-integration/tests/common/seam/api.rs"
DEFAULT_COVERAGE = ROOT / "scripts/ci/seam-coverage.json"
DEFAULT_TARGETS = ROOT / "crates/kama-integration/tests/common/seam/target.rs"

VALID_STATUS = {"implemented", "mock_only", "not_implemented"}

findings: list[str] = []


def bad(msg: str) -> None:
    findings.append(msg)
    print(f"::error title=SEAM-DRIFT::{msg}")
    print(f"KIRMIZI: {msg}")


def die(msg: str) -> "NoReturn":  # noqa: F821
    print(f"ORTAM HATASI: {msg}", file=sys.stderr)
    sys.exit(2)


# ── spec ayristirma ──────────────────────────────────────────────────────────────────────────


def section(text: str, heading_prefix: str) -> str:
    """`## <prefix>` basligindan bir sonraki `## ` basligina kadarki govde."""
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if line.startswith("## ") and line[3:].startswith(heading_prefix):
            start = i + 1
            break
    if start is None:
        die(f"spec'te '## {heading_prefix}…' basligi bulunamadi")
    end = len(lines)
    for j in range(start, len(lines)):
        if lines[j].startswith("## "):
            end = j
            break
    return "\n".join(lines[start:end])


def first_fence(body: str) -> str:
    m = re.search(r"^```[a-zA-Z]*\n(.*?)^```", body, re.S | re.M)
    if not m:
        die("bolumde fenced kod blogu yok")
    return m.group(1)


def spec_api(spec_text: str) -> tuple[str, set[str], set[str]]:
    block = first_fence(section(spec_text, "1."))
    tm = re.search(r"pub\s+trait\s+(\w+)\s*\{(.*?)\n\}", block, re.S)
    if not tm:
        die("spec §1 blogunda `pub trait … { … }` bulunamadi")
    trait_name, trait_body = tm.group(1), tm.group(2)
    methods = set(re.findall(r"\bfn\s+(\w+)\s*\(", trait_body))
    types = set(re.findall(r"pub\s+(?:struct|enum)\s+(\w+)", block))
    return trait_name, methods, types


def spec_error_variants(spec_text: str) -> set[str]:
    block = first_fence(section(spec_text, "3."))
    m = re.search(r"TransportError\s*∈\s*\{([^}]*)\}", block)
    if not m:
        die("spec §3 blogunda `TransportError ∈ { … }` bulunamadi")
    return {v.strip() for v in m.group(1).split(",") if v.strip()}


def spec_harness_rows(spec_text: str) -> list[str]:
    body = section(spec_text, "6.")
    rows = []
    for line in body.splitlines():
        m = re.match(r"\|\s*(H-[A-Z0-9-]+)\s*\|", line)
        if m:
            rows.append(m.group(1))
    if not rows:
        die("spec §6 tablosunda hic H-* satiri bulunamadi")
    return rows


# ── ayna ayristirma ──────────────────────────────────────────────────────────────────────────


def block_of(text: str, opener_re: str) -> str | None:
    """`opener { … }` govdesi — kapanis, sutun 0'daki `}` satiridir."""
    m = re.search(opener_re, text)
    if not m:
        return None
    lines = text[m.end():].splitlines()
    out = []
    for line in lines:
        if line.startswith("}"):
            return "\n".join(out)
        out.append(line)
    return None


def mirror_api(mirror_text: str) -> tuple[str | None, set[str], set[str], set[str]]:
    tm = re.search(r"pub\s+trait\s+(\w+)\s*\{", mirror_text)
    trait_name = tm.group(1) if tm else None
    trait_body = block_of(mirror_text, r"pub\s+trait\s+\w+\s*\{") or ""
    methods = set(re.findall(r"^\s*fn\s+(\w+)\s*\(", trait_body, re.M))
    types = set(re.findall(r"^pub\s+(?:struct|enum)\s+(\w+)", mirror_text, re.M))
    err_body = block_of(mirror_text, r"pub\s+enum\s+TransportError\s*\{") or ""
    variants = set(re.findall(r"^\s{4}([A-Z]\w*)\s*,\s*$", err_body, re.M))
    return trait_name, methods, types, variants


# ── ana ──────────────────────────────────────────────────────────────────────────────────────


def main() -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--spec", type=Path, default=DEFAULT_SPEC)
    ap.add_argument("--mirror", type=Path, default=DEFAULT_MIRROR)
    ap.add_argument("--coverage", type=Path, default=DEFAULT_COVERAGE)
    ap.add_argument("--targets", type=Path, default=DEFAULT_TARGETS)
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    for p in (args.spec, args.mirror, args.coverage, args.targets):
        if not p.is_file():
            die(f"dosya yok: {p}")

    spec_text = args.spec.read_text(encoding="utf-8")
    mirror_text = args.mirror.read_text(encoding="utf-8")
    try:
        cov = json.loads(args.coverage.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        die(f"{args.coverage}: bozuk JSON ({e})")

    s_trait, s_methods, s_types = spec_api(spec_text)
    s_errs = spec_error_variants(spec_text)
    s_rows = spec_harness_rows(spec_text)
    m_trait, m_methods, m_types, m_errs = mirror_api(mirror_text)

    say = (lambda *a: None) if args.quiet else print
    say(f"  spec  : trait {s_trait} · {len(s_methods)} metot · {len(s_types)} tip · "
        f"{len(s_errs)} hata · {len(s_rows)} §6 satiri")
    say(f"  ayna  : trait {m_trait} · {len(m_methods)} metot · {len(m_types)} tip · "
        f"{len(m_errs)} hata")

    # A — trait adi + metot kumesi
    if m_trait is None:
        bad("aynada `pub trait … {` yok — §1 arayuzu temsil edilmiyor")
    elif m_trait != s_trait:
        bad(f"trait adi ayrisiyor: spec '{s_trait}', ayna '{m_trait}'")
    if s_methods != m_methods:
        bad(f"§1 metot kumesi ayrisiyor: yalniz spec'te {sorted(s_methods - m_methods)}, "
            f"yalniz aynada {sorted(m_methods - s_methods)}")

    # B — §1 tipleri aynada var mi
    missing_types = s_types - m_types
    if missing_types:
        bad(f"§1'de bildirilen tip(ler) aynada yok: {sorted(missing_types)}")
    extra = m_types - s_types
    if extra and not args.quiet:
        say(f"  ayna-ek tipler (beklenen; alias/yardimci): {sorted(extra)}")

    # C — §3 hata kumesi KAPALI
    if s_errs != m_errs:
        bad(f"§3 TransportError kumesi ayrisiyor: yalniz spec'te {sorted(s_errs - m_errs)}, "
            f"yalniz aynada {sorted(m_errs - s_errs)}")

    # D — §6 satirlari ↔ beyan
    rows = cov.get("rows")
    if not isinstance(rows, list):
        bad("seam-coverage.json: `rows` listesi yok")
        rows = []
    cov_ids = [r.get("id") for r in rows]
    if cov_ids != s_rows:
        bad(f"§6 satir listesi beyanla ayrisiyor: spec {s_rows}, beyan {cov_ids}")

    # Beyan oz-tutarliligi
    seen: set[str] = set()
    implemented, mock_only = 0, 0
    for r in rows:
        rid, status = r.get("id"), r.get("status")
        if rid in seen:
            bad(f"seam-coverage.json: '{rid}' iki kez beyan edildi")
        seen.add(rid)
        if status not in VALID_STATUS:
            bad(f"seam-coverage.json: '{rid}' gecersiz durum '{status}' (gecerli: {sorted(VALID_STATUS)})")
        if status == "implemented":
            implemented += 1
        elif status == "mock_only":
            mock_only += 1

    for key, actual in (("implemented_baseline", implemented), ("mock_only_baseline", mock_only)):
        baseline = cov.get(key)
        if not isinstance(baseline, int):
            bad(f"seam-coverage.json: `{key}` tam sayi degil")
        elif baseline != actual:
            bad(f"ratchet tabani gerceklikle ayrisiyor: beyan {actual} satir ({key.split('_baseline')[0]}), "
                f"{key} {baseline} — AYNI commit'te guncelleyin")

    # E — tek hedef varken "iki hedefte kanitlandi" denemez.
    targets_text = args.targets.read_text(encoding="utf-8")
    reg = re.search(r"REGISTERED_TARGETS[^;]*;", targets_text, re.S)
    real_registered = bool(reg and "TargetKind::Real" in reg.group(0))
    say(f"  hedef : kayitli gercek hedef {'VAR' if real_registered else 'YOK'}")
    if not real_registered and implemented:
        names = [r["id"] for r in rows if r.get("status") == "implemented"]
        bad(f"tek hedef (mock) kayitliyken {names} 'implemented' beyan edilmis. Bir satir ancak "
            f"IKI hedefte kostuktan sonra kanitlanmis sayilir; simdilik dogru durum 'mock_only'.")

    # Beyan edilen yollar gercekten denetlenenler mi (yanlis dosyayi denetleme tuzagi)
    for key, used in (("spec_path", args.spec), ("mirror_path", args.mirror)):
        declared = cov.get(key)
        if declared and args.spec == DEFAULT_SPEC and args.mirror == DEFAULT_MIRROR:
            if (ROOT / declared).resolve() != used.resolve():
                bad(f"seam-coverage.json {key}='{declared}' denetlenen dosya degil ({used})")

    if findings:
        print(f"\nSEAM-DRIFT: KIRMIZI ({len(findings)} bulgu)")
        return 1
    say("\nSEAM-DRIFT: YESIL (spec ↔ ayna ↔ beyan hizali)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
