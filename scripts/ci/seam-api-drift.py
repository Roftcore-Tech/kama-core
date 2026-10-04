#!/usr/bin/env python3
"""Seam drift kapisi — spec ↔ sozlesme kaynagi (`crates/kama-contracts`) ↔ kapsam beyani.

    scripts/ci/seam-api-drift.py [--seam s1|s3] [--spec P] [--mirror P]... [--coverage P] [--quiet]

NEDEN VAR. Sozlesmenin iki yazili hali vardir: belge (`docs/seams/KM-SEAM-*.md` §1 trait bloklari)
ve derlenen kaynak (`crates/kama-contracts/src/{s1,s3,handles}.rs`). Ikinci bir hakikat sessizce
eskir; bu kapi ikisinin ayrismasini KIRMIZI yapar. [B1 2026-10-05] Onceden karsilastirilan sey
test-yerel bir aynaydi (`kama-integration/tests/common/seam/api.rs`); o dosya kalkti, kapi artik
dogrudan sozlesme kaynagina bakar. `--mirror` adi korundu: "spec'in karsisindaki Rust kaynagi".
Birden cok kez verilebilir (dosyalar birlestirilir) — ortak tipler `handles.rs`'te durdugu icin.

--seam s1 (varsayilan; KM-SEAM-AT-001) — DENETLENEN ESITLIKLER:
  A. §1 trait adi + metot kumesi        == kaynaktaki trait adi + metot kumesi
  B. §1'de bildirilen tipler            ⊆ kaynaktaki tipler   (kaynak alias/yardimci ekleyebilir)
  C. §3 TransportError variant kumesi   == kaynaktaki variant kumesi   (kume KAPALIDIR)
  D. §6 tablosundaki H-* satir kimlikleri == seam-coverage.json rows[].id
  E. tek hedef (mock) kayitliyken hicbir satir 'implemented' beyan EDILEMEZ (yalniz 'mock_only')
  Ek olarak beyan dosyasinin kendi tutarliligi (tekrarsiz kimlik, gecerli durum, ratchet tabani).

--seam s3 (KM-SEAM-TC-001) — DENETLENEN ESITLIKLER:
  A. §1'deki HER `pub trait` (PacketProtection, SessionControl) kaynakta var ve metot kumesi ayni
  B. §1'de bildirilen tipler            ⊆ kaynaktaki tipler
  D. §6 tablosundaki her H-* satirinin noise-seam-coverage.json'da en az bir satiri var
     (`H-X` ya da alt satir `H-X/…`) ve beyandaki her satir §6'daki bir satira ait

Cikis kodu: 0 = ayrisma yok · 1 = drift (kapi kirmizi) · 2 = ortam/kullanim hatasi.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONTRACTS = ROOT / "crates/kama-contracts/src"
DEFAULTS = {
    "s1": {
        "spec": ROOT / "docs/seams/KM-SEAM-AT-001.md",
        "mirror": [CONTRACTS / "s1.rs", CONTRACTS / "handles.rs"],
        "coverage": ROOT / "scripts/ci/seam-coverage.json",
    },
    "s3": {
        "spec": ROOT / "docs/seams/KM-SEAM-TC-001.md",
        "mirror": [CONTRACTS / "s3.rs", CONTRACTS / "handles.rs"],
        "coverage": ROOT / "scripts/ci/noise-seam-coverage.json",
    },
}
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


def all_fences(body: str) -> list[str]:
    blocks = re.findall(r"^```[a-zA-Z]*\n(.*?)^```", body, re.S | re.M)
    if not blocks:
        die("bolumde fenced kod blogu yok")
    return blocks


def first_fence(body: str) -> str:
    return all_fences(body)[0]


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


# ── S3 (KM-SEAM-TC-001) ──────────────────────────────────────────────────────────────────────


def spec_traits(spec_text: str) -> tuple[dict[str, set[str]], set[str]]:
    """§1'deki TUM fenced bloklardan {trait adi: metot kumesi} ve bildirilen tipler."""
    traits: dict[str, set[str]] = {}
    types: set[str] = set()
    for block in all_fences(section(spec_text, "1.")):
        for tm in re.finditer(r"pub\s+trait\s+(\w+)[^{\n]*\{(.*?)\n\}", block, re.S):
            traits[tm.group(1)] = set(re.findall(r"\bfn\s+(\w+)\s*\(", tm.group(2)))
        types |= set(re.findall(r"pub\s+(?:struct|enum)\s+(\w+)", block))
    if not traits:
        die("spec §1 bloklarinda hic `pub trait … { … }` bulunamadi")
    return traits, types


def mirror_traits(mirror_text: str) -> tuple[dict[str, set[str]], set[str]]:
    traits: dict[str, set[str]] = {}
    for tm in re.finditer(r"^pub\s+trait\s+(\w+)[^{\n]*\{", mirror_text, re.M):
        body = block_of(mirror_text[tm.start():], r"pub\s+trait\s+\w+[^{\n]*\{") or ""
        traits[tm.group(1)] = set(re.findall(r"^\s*fn\s+(\w+)\s*\(", body, re.M))
    types = set(re.findall(r"^pub\s+(?:struct|enum)\s+(\w+)", mirror_text, re.M))
    return traits, types


def check_s3(spec_text: str, mirror_text: str, cov: dict, say) -> None:
    s_traits, s_types = spec_traits(spec_text)
    m_traits, m_types = mirror_traits(mirror_text)
    s_rows = spec_harness_rows(spec_text)
    say(f"  spec  : {len(s_traits)} trait {sorted(s_traits)} · {len(s_types)} tip · {len(s_rows)} §6 satiri")
    say(f"  kaynak: {len(m_traits)} trait {sorted(m_traits)} · {len(m_types)} tip")

    # A — her trait + metot kumesi
    for name, s_methods in sorted(s_traits.items()):
        if name not in m_traits:
            bad(f"§1 trait'i kaynakta yok: {name}")
            continue
        m_methods = m_traits[name]
        if s_methods != m_methods:
            bad(f"§1 {name} metot kumesi ayrisiyor: yalniz spec'te {sorted(s_methods - m_methods)}, "
                f"yalniz kaynakta {sorted(m_methods - s_methods)}")
    extra_traits = set(m_traits) - set(s_traits)
    if extra_traits:
        bad(f"kaynakta spec §1'de olmayan trait(ler) var: {sorted(extra_traits)}")

    # B — §1 tipleri kaynakta var mi
    missing_types = s_types - m_types
    if missing_types:
        bad(f"§1'de bildirilen tip(ler) kaynakta yok: {sorted(missing_types)}")
    extra = m_types - s_types
    if extra:
        say(f"  kaynak-ek tipler (beklenen; hata tipi/ortak tutamaclar): {sorted(extra)}")

    # D — §6 satirlari ↔ beyan (alt satirlar `H-X/…` serbest)
    rows = cov.get("rows")
    if not isinstance(rows, list):
        bad("noise-seam-coverage.json: `rows` listesi yok")
        rows = []
    seen: set[str] = set()
    bases: set[str] = set()
    for r in rows:
        rid = r.get("id")
        if not isinstance(rid, str) or not rid:
            bad("noise-seam-coverage.json: kimliksiz satir")
            continue
        if rid in seen:
            bad(f"noise-seam-coverage.json: '{rid}' iki kez beyan edildi")
        seen.add(rid)
        base = rid.split("/", 1)[0]
        bases.add(base)
        if base not in s_rows:
            bad(f"noise-seam-coverage.json: '{rid}' spec §6'da olmayan bir satira ait")
    for row in s_rows:
        if row not in bases:
            bad(f"§6 satiri beyanda yok: {row}")


# ── ana ──────────────────────────────────────────────────────────────────────────────────────


def main() -> int:
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--seam", choices=sorted(DEFAULTS), default="s1")
    ap.add_argument("--spec", type=Path, default=None)
    ap.add_argument("--mirror", type=Path, action="append", default=None,
                    help="spec'in karsisindaki Rust kaynagi; birden cok kez verilebilir")
    ap.add_argument("--coverage", type=Path, default=None)
    ap.add_argument("--targets", type=Path, default=DEFAULT_TARGETS)
    ap.add_argument("--quiet", action="store_true")
    args = ap.parse_args()

    d = DEFAULTS[args.seam]
    default_paths = args.spec is None and args.mirror is None
    args.spec = args.spec or d["spec"]
    args.mirror = args.mirror or d["mirror"]
    args.coverage = args.coverage or d["coverage"]

    needed = [args.spec, *args.mirror, args.coverage]
    if args.seam == "s1":
        needed.append(args.targets)
    for p in needed:
        if not p.is_file():
            die(f"dosya yok: {p}")

    spec_text = args.spec.read_text(encoding="utf-8")
    mirror_text = "\n".join(p.read_text(encoding="utf-8") for p in args.mirror)
    try:
        cov = json.loads(args.coverage.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        die(f"{args.coverage}: bozuk JSON ({e})")

    if args.seam == "s3":
        say = (lambda *a: None) if args.quiet else print
        check_s3(spec_text, mirror_text, cov, say)
        if findings:
            print(f"\nSEAM-DRIFT[s3]: KIRMIZI ({len(findings)} bulgu)")
            return 1
        say("\nSEAM-DRIFT[s3]: YESIL (spec ↔ sozlesme kaynagi ↔ beyan hizali)")
        return 0

    s_trait, s_methods, s_types = spec_api(spec_text)
    s_errs = spec_error_variants(spec_text)
    s_rows = spec_harness_rows(spec_text)
    m_trait, m_methods, m_types, m_errs = mirror_api(mirror_text)

    say = (lambda *a: None) if args.quiet else print
    say(f"  spec  : trait {s_trait} · {len(s_methods)} metot · {len(s_types)} tip · "
        f"{len(s_errs)} hata · {len(s_rows)} §6 satiri")
    say(f"  kaynak: trait {m_trait} · {len(m_methods)} metot · {len(m_types)} tip · "
        f"{len(m_errs)} hata")

    # A — trait adi + metot kumesi
    if m_trait is None:
        bad("kaynakta `pub trait … {` yok — §1 arayuzu temsil edilmiyor")
    elif m_trait != s_trait:
        bad(f"trait adi ayrisiyor: spec '{s_trait}', kaynak '{m_trait}'")
    if s_methods != m_methods:
        bad(f"§1 metot kumesi ayrisiyor: yalniz spec'te {sorted(s_methods - m_methods)}, "
            f"yalniz kaynakta {sorted(m_methods - s_methods)}")

    # B — §1 tipleri kaynakta var mi
    missing_types = s_types - m_types
    if missing_types:
        bad(f"§1'de bildirilen tip(ler) kaynakta yok: {sorted(missing_types)}")
    extra = m_types - s_types
    if extra and not args.quiet:
        say(f"  kaynak-ek tipler (beklenen; alias/yardimci/ortak): {sorted(extra)}")

    # C — §3 hata kumesi KAPALI
    if s_errs != m_errs:
        bad(f"§3 TransportError kumesi ayrisiyor: yalniz spec'te {sorted(s_errs - m_errs)}, "
            f"yalniz kaynakta {sorted(m_errs - s_errs)}")

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
    if default_paths:
        declared = cov.get("spec_path")
        if declared and (ROOT / declared).resolve() != args.spec.resolve():
            bad(f"seam-coverage.json spec_path='{declared}' denetlenen dosya degil ({args.spec})")
        declared = cov.get("mirror_paths")
        if declared is not None:
            want = sorted(str((ROOT / p).resolve()) for p in declared)
            used = sorted(str(p.resolve()) for p in args.mirror)
            if want != used:
                bad(f"seam-coverage.json mirror_paths={declared} denetlenen dosyalar degil "
                    f"({[str(p) for p in args.mirror]})")
        if "mirror_path" in cov:
            bad("seam-coverage.json: eski tekil `mirror_path` anahtari — `mirror_paths` listesi kullanin")

    if findings:
        print(f"\nSEAM-DRIFT: KIRMIZI ({len(findings)} bulgu)")
        return 1
    say("\nSEAM-DRIFT: YESIL (spec ↔ sozlesme kaynagi ↔ beyan hizali)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
