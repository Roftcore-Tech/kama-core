#!/usr/bin/env python3
"""cargo test günlüğünden iş özeti: düşen test tam adıyla, ham günlük bağlantısı, V2 / bitmap kolu.

    scripts/ci/test_summary.py LOG --arm {v2,bitmap} [--title BAŞLIK] [--log-url URL]
    scripts/ci/test_summary.py --selftest

NEDEN (ölçüldü, 2026-11-23, #240 7095575771). Ürün hücreleri `--nocapture` ile koşar. libtest önce
`test <ad> ... ` yazar, testin kendi çıktısı aynı satırdan devam eder, `ok` / `FAILED` ayrı bir
satıra düşer. `grep -E '^test .* (ok|FAILED)$'` bu yüzden yalnız çıktı basmayan testlerin sonucunu
görür. Aday 86c5602c'de 4 testlik bir koşuda (3 geçti, 1 düştü) özet tek bir `ok` gösterdi,
`FAILED` hiç görünmedi (`testdata/test_summary/nocapture_real_86c5602c.log`, öz-test A).

KURAL. Sonuç yalnız libtest'in satır başına bağlı bloklarından okunur:
    `Running <hedef>`          hedef başlangıcı
    `failures:` + 4 boşluklu ad düşen testlerin listesi (`--nocapture` olsa da olmasa da basılır)
    `test result: …`           hedef sonucu (geçti / düştü sayısı)
    `error: test failed, to rerun pass `…``  yeniden koşma bağımsız değişkenleri
Sonuç satırı olmadan biten hedef (çökme, zaman aşımı, sinyal) SONUÇSUZ yazılır, geçmiş sayılmaz.

KOL ETİKETİ. `--arm` iş matrisinden gelir (`KAMA_ASYM_ACK_FORMAT`). Her `ÜRÜN-SONUÇ` /
`TEŞHİS-KOLU` satırı kendi `yol=` alanına göre ayrıca etiketlenir: `yol=v2` → "V2 (ürün
varsayılanı)", `yol=bitmap` → "bitmap pilot". Satırın yolu `--arm` ile uyuşmazsa uyarı basılır.

Özet işin rengini DEĞİŞTİRMEZ (renk test adımınındır): çıkış 0; kullanım hatası 2.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ARMS = {"v2": "V2 (ürün varsayılanı)", "bitmap": "bitmap pilot"}

RUNNING = re.compile(r"^\s+Running (?:unittests )?(\S+)")
DOCTESTS = re.compile(r"^\s+Doc-tests (\S+)")
RESULT = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed")
FAIL_NAME = re.compile(r"^    ([A-Za-z0-9_:]+)$")
STREAM_FAIL = re.compile(r"^test (\S+) \.\.\. (?:.*\s)?FAILED$")
PANIC = re.compile(r"^thread '([^']+)'(?: \(\d+\))? panicked at (\S+?):?$")
RERUN = re.compile(r"^error: test failed, to rerun pass `([^`]+)`")
PRODUCT = re.compile(r"^(ÜRÜN-SONUÇ|TEŞHİS-KOLU) ")

# #240'ın (9bfd0fdc) özet adımındaki süzgeç; öz-test A kaybı bununla gösterir.
OLD_FILTER = re.compile(r"ÜRÜN-SONUÇ|TEŞHİS-KOLU|^test .* (ok|FAILED)$")


@dataclass
class Target:
    name: str
    result: tuple[str, int, int] | None = None
    failed: list[str] = field(default_factory=list)
    rerun: str | None = None


@dataclass
class Summary:
    targets: list[Target] = field(default_factory=list)
    panics: dict[str, str] = field(default_factory=dict)
    product: list[str] = field(default_factory=list)


def stem(path: str) -> str:
    p = Path(path)
    return p.stem if p.suffix == ".rs" else path


def parse(text: str) -> Summary:
    s = Summary()
    cur: Target | None = None
    in_failures = False
    lines = text.splitlines()
    for i, line in enumerate(lines):
        m = RUNNING.match(line) or DOCTESTS.match(line)
        if m:
            cur = Target(stem(m.group(1)) if RUNNING.match(line) else f"doc:{m.group(1)}")
            s.targets.append(cur)
            in_failures = False
            continue
        if PRODUCT.match(line):
            s.product.append(line)
            continue
        m = PANIC.match(line)
        if m:
            msg = lines[i + 1].strip() if i + 1 < len(lines) else ""
            s.panics.setdefault(m.group(1), msg)
            continue
        if cur is None:
            continue
        m = STREAM_FAIL.match(line)
        if m and m.group(1) not in cur.failed:
            cur.failed.append(m.group(1))
            continue
        if line == "failures:":
            in_failures = True
            continue
        m = RESULT.match(line)
        if m:
            cur.result = (m.group(1), int(m.group(2)), int(m.group(3)))
            in_failures = False
            continue
        m = FAIL_NAME.match(line)
        if in_failures and m and m.group(1) not in cur.failed:
            cur.failed.append(m.group(1))
            continue
        m = RERUN.match(line)
        if m:
            cur.rerun = m.group(1)
    return s


def path_label(line: str) -> str:
    m = re.search(r"yol=(\S+)", line)
    if not m:
        return "etiketsiz"
    return ARMS["v2"] if m.group(1).startswith("v2") else (
        ARMS["bitmap"] if m.group(1).startswith("bitmap") else f"yol={m.group(1)}"
    )


def verdict(s: Summary) -> str:
    if not s.targets or all(t.result is None for t in s.targets):
        return "SONUÇSUZ"
    if any(t.result is None for t in s.targets):
        return "SONUÇSUZ"
    if any(t.failed or t.result[0] == "FAILED" for t in s.targets):
        return "KIRMIZI"
    return "YEŞİL"


def render(s: Summary, arm: str, title: str, log_url: str | None) -> str:
    out = [f"### {title} · kol: {ARMS[arm]}"]
    out.append(f"Ham günlük: [artefakt]({log_url})" if log_url else "Ham günlük: bağlantı verilmedi")
    v = verdict(s)
    n_fail = sum(len(t.failed) for t in s.targets)
    n_pass = sum(t.result[1] for t in s.targets if t.result)
    out.append(f"**Sonuç: {v}** · geçti {n_pass} · düştü {n_fail}")
    if not s.targets:
        out.append("- hiç test hedefi koşmadı (`Running` satırı yok)")
    for t in s.targets:
        if t.result is None:
            out.append(f"- `{t.name}`: sonuç satırı yok (çökme / zaman aşımı / sinyal); geçmiş sayılmaz")
    failed = [(t, f) for t in s.targets for f in t.failed]
    if failed:
        out.append("")
        out.append("Düşen testler (tam ad):")
        for t, f in failed:
            row = f"- `{t.name}::{f}`"
            if f in s.panics and s.panics[f]:
                row += f" — {s.panics[f]}"
            if t.rerun:
                row += f" — yeniden koş: `cargo test {t.rerun} -- --exact {f}`"
            out.append(row)
    if s.product:
        out.append("")
        out.append("Ürün satırları (yol etiketiyle):")
        out.append("```")
        for line in s.product:
            out.append(f"[{path_label(line)}] {line}")
        out.append("```")
        bad = [l for l in s.product if path_label(l) not in (ARMS[arm], "etiketsiz")]
        if bad:
            out.append(f"**UYARI:** kol {ARMS[arm]}, ama {len(bad)} satırın yolu farklı.")
    return "\n".join(out) + "\n"


# ── Öz-test ───────────────────────────────────────────────────────────────────────────────

FIX = Path(__file__).resolve().parent / "testdata" / "test_summary"

SYNTH_CRASH = """\
     Running tests/a.rs (target/release/deps/a-1)

running 1 test
test a_ok ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/b.rs (target/release/deps/b-2)

running 2 tests
test b_one ... ok
error: test failed, to rerun pass `-p x --test b`

Caused by:
  process didn't exit successfully: `target/release/deps/b-2` (signal: 9, SIGKILL: kill)
"""

SYNTH_CAPTURED = """\
     Running tests/c.rs (target/debug/deps/c-3)

running 2 tests
test c_good ... ok
test c_bad ... FAILED

failures:

---- c_bad stdout ----

thread 'c_bad' panicked at tests/c.rs:9:5:
beklenen 1, gelen 2

failures:
    c_bad

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p y --test c`
"""

SYNTH_GREEN_V2 = """\
     Running tests/asym_product_outcome.rs (target/release/deps/asym-4)

running 1 test
test gate_real_6250_declared_12500 ... HÜCRE hücre=gerçek-6250
ÜRÜN-SONUÇ hücre=gerçek-6250 tohum=2 yol=v2 (ürün varsayılanı) ptt_biçim=x metin1=0.40(GEÇTİ) düşen_ölçüt=[]
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 61.00s
"""


def selftest() -> int:
    errors: list[str] = []

    def check(cond: bool, what: str) -> None:
        if not cond:
            errors.append(what)

    # A. Gerçek günlük (aday 86c5602c, release, tohum 1, 4 test, 1 kırmızı): eski süzgeç kırmızıyı
    #    kaybeder, bu betik tam adıyla gösterir.
    real = (FIX / "nocapture_real_86c5602c.log").read_text(encoding="utf-8")
    old = [l for l in real.splitlines() if OLD_FILTER.search(l)]
    check(not any("FAILED" in l for l in old), "A: eski süzgeç FAILED'ı görüyor (fikstür yanlış)")
    check(
        not any("text_receipt_ptt_and_file" in l for l in old),
        "A: eski süzgeç kırmızı testin adını görüyor (fikstür yanlış)",
    )
    s = parse(real)
    full = [f"{t.name}::{f}" for t in s.targets for f in t.failed]
    check(
        full == ["asym_product_outcome::text_receipt_ptt_and_file_all_progress_on_the_asymmetric_line"],
        f"A: düşen tam ad {full}",
    )
    check(s.targets[0].result == ("FAILED", 3, 1), f"A: sonuç {s.targets[0].result}")
    check(verdict(s) == "KIRMIZI", f"A: hüküm {verdict(s)}")
    md = render(s, "bitmap", "ürün hücreleri, tohum 1", "https://example.invalid/artefakt")
    check("PTT/bant/alice" in md, "A: düşüş nedeni özette yok")
    check("[bitmap pilot] ÜRÜN-SONUÇ" in md, "A: bitmap etiketi yok")
    check("Ham günlük: [artefakt](https://example.invalid/artefakt)" in md, "A: günlük bağlantısı yok")
    check(
        "yeniden koş: `cargo test -p kama-integration --test asym_product_outcome -- --exact "
        "text_receipt_ptt_and_file_all_progress_on_the_asymmetric_line`" in md,
        "A: yeniden koşma komutu yok",
    )
    check("UYARI" in render(s, "v2", "t", None), "A: kol ↔ yol uyuşmazlığı uyarısı yok")
    check("UYARI" not in md, "A: uyuşan kolda yersiz uyarı")

    # B. Çöken hedef sonuçsuz yazılır, geçmiş sayılmaz.
    s = parse(SYNTH_CRASH)
    check(verdict(s) == "SONUÇSUZ", f"B: hüküm {verdict(s)}")
    check("`b`: sonuç satırı yok" in render(s, "v2", "t", None), "B: çöken hedef özette yok")

    # C. Yakalanan çıktı biçimi (--nocapture yok): ad bir kez, neden panik satırından.
    s = parse(SYNTH_CAPTURED)
    check(s.targets[0].failed == ["c_bad"], f"C: düşen {s.targets[0].failed}")
    check("`c::c_bad` — beklenen 1, gelen 2" in render(s, "v2", "t", None), "C: neden yok")

    # D. Yeşil V2 kolu: etiket V2, düşen yok, uyarı yok.
    s = parse(SYNTH_GREEN_V2)
    md = render(s, "v2", "t", None)
    check(verdict(s) == "YEŞİL", f"D: hüküm {verdict(s)}")
    check("[V2 (ürün varsayılanı)] ÜRÜN-SONUÇ" in md and "UYARI" not in md, "D: V2 etiketi")
    check("Düşen testler" not in md, "D: yeşilde düşen listesi")

    # E. Boş günlük geçmiş sayılmaz.
    check(verdict(parse("")) == "SONUÇSUZ", "E: boş günlük sonuçsuz değil")

    for e in errors:
        print(f"ÖZ-TEST KALDI: {e}", file=sys.stderr)
    print(f"test_summary öz-test: {5 - len({e[0] for e in errors})}/5 bölüm geçti")
    return 1 if errors else 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("log", type=Path)
    ap.add_argument("--arm", required=True, choices=sorted(ARMS))
    ap.add_argument("--title", default="test sonuçları")
    ap.add_argument("--log-url")
    a = ap.parse_args(argv)
    try:
        text = a.log.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        print(f"günlük okunamadı: {e}", file=sys.stderr)
        text = ""
    sys.stdout.write(render(parse(text), a.arm, a.title, a.log_url))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
