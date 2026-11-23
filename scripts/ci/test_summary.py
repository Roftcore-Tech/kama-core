#!/usr/bin/env python3
"""cargo test günlüğünden iş özeti: düşen test tam adıyla, ham günlük bağlantısı, V2 / bitmap kolu.

    scripts/ci/test_summary.py LOG --arm {v2,bitmap} [--title BAŞLIK] [--log-url URL]
                                   [--rerun "<işin kendi komutu, env dahil, test adı hariç>"]
    scripts/ci/test_summary.py --selftest

NEDEN (ölçüldü, 2026-11-23, #240 7095575771). Ürün hücreleri `--nocapture` ile koşar. libtest önce
`test <ad> ... ` yazar, testin kendi çıktısı aynı satırdan devam eder, `ok` / `FAILED` ayrı bir
satıra düşer. #240'ın `grep -E '^test .* (ok|FAILED)$'` süzgeci bu yüzden yalnız çıktı basmayan
testlerin sonucunu görür. 86c5602c ürün kodu + #243 test dosyası (e6ff8b59) ile alınan 4 testlik bir
koşuda (3 geçti, 1 düştü) özet tek bir `ok` gösterdi, `FAILED` hiç görünmedi (öz-test A).

GİRDİ. `cargo test … 2>&1 | tee` dosyası ya da GitHub iş günlüğü. Satır başındaki iş günlüğü zaman
damgası ve ANSI renk kodları eşlemeden ÖNCE silinir (`CARGO_TERM_COLOR: always` altında `Running` ve
`error:` satırları ESC ile başlar; öz-test H gerçek hosted günlükle sınar). İş günlüğünde stdout ile
stderr ayrı borulardan birleştiği için satır sırası bozulabilir; özet `tee` dosyası içindir.

KURAL. Sonuç libtest'in satır başına bağlı bloklarından okunur:
    `Running <hedef>` / `Doc-tests <crate>`   hedef başlangıcı
    `test <ad> ... <durum>`                   açılan test; durum aynı satırda ya da tek başına satırda
    `failures:` (SON blok) + 4 boşluklu adlar  düşen testler; ilk blok stdout içerebilir, sayılmaz
    `test result: …`                          hedef sonucu: geçti / düştü / atlandı / süzüldü
    `error: test failed` · `process didn't exit successfully`  süreç başarısız çıktı
Hüküm:
    SONUÇSUZ  hedef yok, ya da bir hedef sonuç satırı olmadan bitti (çökme, asılma, zaman aşımı;
              düşen ve koşmakta olan testler adıyla yazılır), ya da sonuç `ok` ama süreç başarısız
    KIRMIZI   bir testin sonucu FAILED
    BOŞ       hiç test geçmedi (0 test koştu)
    EKSİK     geçenler var ama `#[ignore]` ile atlanan test var
    YEŞİL     yalnız bunların hiçbiri yoksa
Düşen sayısı `test result:` satırından okunur; ad listesiyle uyuşmazsa ayrıca yazılır.

KOL ETİKETİ. `--arm` iş matrisinin beyanıdır (`KAMA_ASYM_ACK_FORMAT`). `ÜRÜN-SONUÇ` / `TEŞHİS-KOLU`
satırları (satırın neresinde başlarsa başlasın) kendi `yol=` alanına göre ayrıca etiketlenir:
`yol=v2` → "V2 (ürün varsayılanı)", `yol=bitmap` → "bitmap pilot". Yolu beyanla uyuşmayan satır
ya da `yol=` taşıyan hiç satır yoksa UYARI basılır: kol yalnız beyandır, satırla doğrulanmamıştır.

YENİDEN KOŞMA. Betik komut uydurmaz. `--rerun` verilirse düşen her test için `<rerun> <ad>` basılır;
o metin işin kendi komutu olmalıdır (profil, tohum, kol, `--test-threads`, `--nocapture`, `--exact`).

Özet işin rengini DEĞİŞTİRMEZ (renk test adımınındır): çıkış 0; kullanım hatası 2.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ARMS = {"v2": "V2 (ürün varsayılanı)", "bitmap": "bitmap pilot"}

ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")
GH_STAMP = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?Z ")
RUNNING = re.compile(r"^\s+Running (unittests )?(\S+)(?: \(([^)]+)\))?")
DOCTESTS = re.compile(r"^\s+Doc-tests (\S+)")
TEST_LINE = re.compile(r"^test (.+?) \.\.\. ?(.*)$")
STATUS_ONLY = re.compile(r"^(ok|FAILED|ignored(?:, .*)?)$")
RESULT = re.compile(
    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
    r"(\d+) measured; (\d+) filtered out"
)
FAIL_NAME = re.compile(r"^    (\S.*)$")
PANIC = re.compile(r"^thread '([^']+)'(?: \(\d+\))? panicked at (\S+?):?$")
PROC_FAIL = re.compile(r"^error: test failed|process didn't exit successfully")
PRODUCT = re.compile(r"(ÜRÜN-SONUÇ|TEŞHİS-KOLU) ")

# #240'ın (9bfd0fdc) özet adımındaki süzgeç; öz-test A kaybı bununla gösterir.
OLD_FILTER = re.compile(r"ÜRÜN-SONUÇ|TEŞHİS-KOLU|^test .* (ok|FAILED)$")


def clean(line: str) -> str:
    return ANSI.sub("", GH_STAMP.sub("", line, count=1)).rstrip("\r")


@dataclass
class Target:
    name: str
    result: tuple[str, int, int, int, int] | None = None  # durum, geçti, düştü, atlandı, süzüldü
    streamed_failed: list[str] = field(default_factory=list)
    block: list[str] = field(default_factory=list)
    final_failed: list[str] | None = None
    open_test: str | None = None
    panics: dict[str, str] = field(default_factory=dict)
    proc_failed: bool = False

    def failed(self) -> list[str]:
        return self.final_failed if self.final_failed is not None else self.streamed_failed


@dataclass
class Summary:
    targets: list[Target] = field(default_factory=list)
    product: list[str] = field(default_factory=list)


def target_name(m: re.Match[str]) -> str:
    unit, path, binary = m.group(1), m.group(2), m.group(3)
    if unit:
        crate = Path(binary).name.rsplit("-", 1)[0] if binary else path
        return f"{crate}(lib)"
    p = Path(path)
    return p.stem if p.suffix == ".rs" else path


def parse(text: str) -> Summary:
    s = Summary()
    cur: Target | None = None
    in_failures = False
    lines = [clean(l) for l in text.splitlines()]
    for i, line in enumerate(lines):
        m = PRODUCT.search(line)
        if m:
            s.product.append(line[m.start():])
        m = RUNNING.match(line)
        d = DOCTESTS.match(line)
        if m or d:
            cur = Target(target_name(m) if m else f"doc:{d.group(1)}")
            s.targets.append(cur)
            in_failures = False
            continue
        if cur is None:
            continue
        m = PANIC.match(line)
        if m:
            nxt = lines[i + 1].strip() if i + 1 < len(lines) else ""
            cur.panics.setdefault(m.group(1), nxt)
            continue
        if PROC_FAIL.search(line):
            cur.proc_failed = True
            continue
        m = RESULT.match(line)
        if m:
            cur.result = (m.group(1), int(m.group(2)), int(m.group(3)), int(m.group(4)), int(m.group(6)))
            if in_failures or cur.block:
                cur.final_failed = list(dict.fromkeys(cur.block))
            cur.open_test = None
            in_failures = False
            continue
        if line == "failures:":
            in_failures = True
            cur.block = []  # son blok geçerli: ilk blok stdout taşıyabilir
            continue
        if in_failures:
            m = FAIL_NAME.match(line)
            if m:
                cur.block.append(m.group(1))
            continue
        m = TEST_LINE.match(line)
        if m:
            name, rest = m.group(1), m.group(2)
            st = STATUS_ONLY.match(rest)
            if st:
                if st.group(1) == "FAILED" and name not in cur.streamed_failed:
                    cur.streamed_failed.append(name)
                cur.open_test = None
            else:
                cur.open_test = name
            continue
        st = STATUS_ONLY.match(line)
        if st and cur.open_test:
            if st.group(1) == "FAILED" and cur.open_test not in cur.streamed_failed:
                cur.streamed_failed.append(cur.open_test)
            cur.open_test = None
    return s


def path_label(line: str) -> str:
    m = re.search(r"yol=(\S+)", line)
    if not m:
        return "etiketsiz"
    v = m.group(1)
    return ARMS["v2"] if v.startswith("v2") else ARMS["bitmap"] if v.startswith("bitmap") else f"yol={v}"


def verdict(s: Summary) -> str:
    if not s.targets or any(t.result is None for t in s.targets):
        return "SONUÇSUZ"
    if any(t.result[0] == "FAILED" or t.result[2] > 0 or t.failed() for t in s.targets):
        return "KIRMIZI"
    if any(t.proc_failed for t in s.targets):
        return "SONUÇSUZ"
    if sum(t.result[1] for t in s.targets) == 0:
        return "BOŞ"
    if any(t.result[3] > 0 for t in s.targets):
        return "EKSİK"
    return "YEŞİL"


def render(s: Summary, arm: str, title: str, log_url: str | None, rerun: str | None) -> str:
    kol = ARMS[arm]
    out = [f"### {title} · kol (iş matrisi beyanı): {kol}"]
    out.append(f"Ham günlük: [artefakt]({log_url})" if log_url else "Ham günlük: bağlantı verilmedi")
    res = [t.result for t in s.targets if t.result]
    passed, failed, ignored, filtered = (sum(r[k] for r in res) for k in (1, 2, 3, 4))
    out.append(
        f"**Sonuç: {verdict(s)}** · geçti {passed} · düştü {failed} · atlandı (ignore) {ignored}"
        f" · süzüldü {filtered}"
    )
    if not s.targets:
        out.append("- hiç test hedefi koşmadı (`Running` satırı yok)")
    for t in s.targets:
        if t.result is None:
            seen = ", ".join(f"`{f}`" for f in t.failed()) or "yok"
            now = f"`{t.open_test}`" if t.open_test else "yok"
            out.append(
                f"- `{t.name}`: sonuç satırı yok (çökme / asılma / zaman aşımı); geçmiş sayılmaz."
                f" Düşen: {seen}. Koşmakta olan: {now}."
            )
        elif t.proc_failed and t.result[0] == "ok":
            out.append(f"- `{t.name}`: sonuç `ok` ama süreç başarısız çıktı (sinyal / çıkış kodu)")
        if t.result and t.result[3] > 0:
            out.append(f"- **UYARI:** `{t.name}`: {t.result[3]} test `#[ignore]` ile atlandı")
        if t.result and t.result[2] != len(t.failed()):
            out.append(
                f"- `{t.name}`: sonuç satırı {t.result[2]} düşen diyor, ad listesinde {len(t.failed())}"
            )
    rows = [(t, f) for t in s.targets for f in t.failed()]
    if rows:
        out.append("")
        out.append(f"Düşen testler (tam ad, kol: {kol}):")
        for t, f in rows:
            row = f"- `{t.name}::{f}`"
            if t.panics.get(f):
                row += f" — {t.panics[f]}"
            if rerun:
                row += f" — yeniden koş: `{rerun} {f}`"
            out.append(row)
    if s.product:
        out.append("")
        out.append("Ürün satırları (yol etiketiyle):")
        out.append("```")
        out.extend(f"[{path_label(l)}] {l}" for l in s.product)
        out.append("```")
    labelled = [l for l in s.product if path_label(l) != "etiketsiz"]
    bad = [l for l in labelled if path_label(l) != kol]
    if bad:
        out.append(f"**UYARI:** kol {kol}, ama {len(bad)} satırın yolu farklı.")
    elif not labelled:
        out.append(f"**UYARI:** `yol=` taşıyan satır yok; kol ({kol}) yalnız beyan, doğrulanmadı.")
    return "\n".join(out) + "\n"


# ── Öz-test ───────────────────────────────────────────────────────────────────────────────

FIX = Path(__file__).resolve().parent / "testdata" / "test_summary"
FULL = "asym_product_outcome::text_receipt_ptt_and_file_all_progress_on_the_asymmetric_line"

SYNTH = {
    "B_hang": """\
     Running tests/h.rs (target/release/deps/h-1)

running 3 tests
test cell_one ... ok
test cell_two ... HÜCRE hücre=x
thread 'cell_two' (7) panicked at tests/h.rs:9:5:
düşen ürün ölçütleri: ["METİN/x"]
FAILED
test cell_three ... HÜCRE hücre=y
""",
    "C_captured": """\
     Running tests/c.rs (target/debug/deps/c-3)

running 2 tests
test c_good ... ok
test c_bad ... FAILED

failures:

---- c_bad stdout ----
    ipucu

thread 'c_bad' panicked at tests/c.rs:9:5:
beklenen 1, gelen 2

failures:
    c_bad

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p y --test c`
""",
    "D_green_v2": """\
     Running tests/asym_product_outcome.rs (target/release/deps/asym-4)

running 1 test
test gate_real_6250_declared_12500 ... ÜRÜN-SONUÇ hücre=gerçek-6250 tohum=2 yol=v2 (ürün varsayılanı) metin1=0.40(GEÇTİ) düşen_ölçüt=[]
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 61.00s
""",
    "F_empty_filtered": """\
     Running tests/a.rs (target/release/deps/a-1)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 0.00s
""",
    "F_ignored": """\
     Running tests/a.rs (target/release/deps/a-1)

running 3 tests
test a ... ok
test b ... ignored
test c ... ignored, gece

test result: ok. 1 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
""",
    "G_abort_after_ok": """\
     Running tests/a.rs (target/release/deps/a-1)

running 2 tests
test a ... ok
test b ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `-p x --test a`

Caused by:
  process didn't exit successfully: `target/release/deps/a-1` (signal: 6, SIGABRT: process abort signal)
""",
    "I_doctest": """\
   Doc-tests kama_x

running 2 tests
test crates/kama-x/src/lib.rs - f (line 21) ... ok
test crates/kama-x/src/lib.rs - g (line 40) ... FAILED

failures:

---- crates/kama-x/src/lib.rs - g (line 40) stdout ----
hata

failures:
    crates/kama-x/src/lib.rs - g (line 40)

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
""",
    "J_same_name": """\
     Running unittests src/lib.rs (target/debug/deps/kama_a-11)

running 1 test
test tests::must_panic ... FAILED

failures:

---- tests::must_panic stdout ----

thread 'tests::must_panic' (1) panicked at crates/a/src/lib.rs:3:5:
a nedeni

failures:
    tests::must_panic

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/kama_b-22)

running 1 test
test tests::must_panic ... FAILED

failures:

---- tests::must_panic stdout ----

thread 'tests::must_panic' (2) panicked at crates/b/src/lib.rs:3:5:
b nedeni

failures:
    tests::must_panic

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
""",
    "L_diag_only": """\
     Running tests/asym_product_outcome.rs (target/release/deps/asym-4)

running 1 test
test measure_rate_drop_during_file_transfer ... TEŞHİS-KOLU hücre=düşüş tohum=1 ölçütler=[]
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 70.00s
""",
}


def selftest() -> int:
    errors: list[str] = []

    def check(cond: bool, sec: str, what: str) -> None:
        if not cond:
            errors.append(f"{sec}: {what}")

    def names(s: Summary) -> list[str]:
        return [f"{t.name}::{f}" for t in s.targets for f in t.failed()]

    # A. Gerçek günlük (86c5602c + #243 test dosyası, release, tohum 1, 4 test, 1 kırmızı; renksiz,
    #    `2>&1 | tee`). Eski süzgeç kırmızıyı kaybeder; bu betik tam adıyla gösterir.
    for sec, fx in (("A", "nocapture_86c5602c_pr243_trimmed.log"), ("A2", "nocapture_86c5602c_pr243_full.log")):
        real = (FIX / fx).read_text(encoding="utf-8")
        old = [l for l in real.splitlines() if OLD_FILTER.search(l)]
        check(not any("FAILED" in l for l in old), sec, "eski süzgeç FAILED'ı görüyor (fikstür yanlış)")
        check(not any("text_receipt_ptt_and_file" in l for l in old), sec, "eski süzgeç adı görüyor")
        s = parse(real)
        check(names(s) == [FULL], sec, f"düşen tam ad {names(s)}")
        check(s.targets[0].result == ("FAILED", 3, 1, 0, 6), sec, f"sonuç {s.targets[0].result}")
        check(verdict(s) == "KIRMIZI", sec, f"hüküm {verdict(s)}")
    real = (FIX / "nocapture_86c5602c_pr243_trimmed.log").read_text(encoding="utf-8")
    s = parse(real)
    md = render(s, "bitmap", "ürün hücreleri, tohum 1", "https://example.invalid/artefakt", None)
    check("PTT/bant/alice" in md, "A", "düşüş nedeni özette yok")
    check("[bitmap pilot] ÜRÜN-SONUÇ" in md, "A", "bitmap etiketi yok")
    check("Ham günlük: [artefakt](https://example.invalid/artefakt)" in md, "A", "günlük bağlantısı yok")
    check("yeniden koş" not in md, "A", "--rerun yokken komut uyduruldu")
    cmd = "KAMA_ASYM_SEED=1 KAMA_ASYM_ACK_FORMAT=bitmap cargo test --release --locked -p kama-integration --test asym_product_outcome -- --test-threads=1 --nocapture --exact"
    check(f"yeniden koş: `{cmd} text_receipt_ptt" in render(s, "bitmap", "t", None, cmd), "A", "--rerun basılmadı")
    check("UYARI:** kol V2" in render(s, "v2", "t", None, None), "A", "kol ↔ yol uyuşmazlığı uyarısı yok")
    check("UYARI" not in md, "A", "uyuşan kolda yersiz uyarı")

    # H. Gerçek hosted iş günlüğü (iş 137559496250, `CARGO_TERM_COLOR: always`, zaman damgalı, renkli).
    hosted = (FIX / "hosted_ack_reference_137559496250.log").read_text(encoding="utf-8")
    check("\x1b[" in hosted, "H", "fikstür renk kodu taşımıyor")
    s = parse(hosted)
    check([t.name for t in s.targets] == ["ack_ref_vectors", "ack_ref_wire_size"], "H", f"hedefler {[t.name for t in s.targets]}")
    check(names(s) == ["ack_ref_wire_size::ack_ref_wire_size_link_profile"], "H", f"düşen {names(s)}")
    check(verdict(s) == "KIRMIZI", "H", f"hüküm {verdict(s)}")
    check("vakum: ACK mühürlenmedi" in render(s, "v2", "t", None, None), "H", "neden yok")

    # B. Sonuç satırı gelmeden asılan hedef: düşen ve koşmakta olan adıyla.
    md = render(parse(SYNTH["B_hang"]), "v2", "t", None, None)
    check("Düşen: `cell_two`. Koşmakta olan: `cell_three`." in md, "B", "asılan hedefte adlar yok")
    check("**Sonuç: SONUÇSUZ**" in md, "B", "asılan hedef sonuçsuz değil")

    # C. Yakalanan çıktı biçimi: ilk bloktaki 4 boşluklu stdout satırı düşen sayılmaz.
    s = parse(SYNTH["C_captured"])
    check(names(s) == ["c::c_bad"], "C", f"düşen {names(s)}")
    check("`c::c_bad` — beklenen 1, gelen 2" in render(s, "v2", "t", None, None), "C", "neden yok")

    # D. Yeşil V2 kolu; ürün satırı test satırının içinde başlıyor.
    s = parse(SYNTH["D_green_v2"])
    md = render(s, "v2", "t", None, None)
    check(verdict(s) == "YEŞİL", "D", f"hüküm {verdict(s)}")
    check("[V2 (ürün varsayılanı)] ÜRÜN-SONUÇ" in md and "UYARI" not in md, "D", "V2 etiketi")

    # E. Boş günlük.
    check(verdict(parse("")) == "SONUÇSUZ", "E", "boş günlük sonuçsuz değil")

    # F. 0 test ve #[ignore]: yeşil değil.
    check(verdict(parse(SYNTH["F_empty_filtered"])) == "BOŞ", "F", "0 test yeşil sayıldı")
    s = parse(SYNTH["F_ignored"])
    check(verdict(s) == "EKSİK", "F", f"atlanan testle hüküm {verdict(s)}")
    check("2 test `#[ignore]` ile atlandı" in render(s, "v2", "t", None, None), "F", "atlanan sayısı yok")

    # G. Sonuç ok, süreç SIGABRT ile çıktı.
    check(verdict(parse(SYNTH["G_abort_after_ok"])) == "SONUÇSUZ", "G", "çöken süreç yeşil sayıldı")

    # I. Doc-test adı boşluk içerir.
    s = parse(SYNTH["I_doctest"])
    check(names(s) == ["doc:kama_x::crates/kama-x/src/lib.rs - g (line 40)"], "I", f"düşen {names(s)}")

    # J. Aynı adlı iki birim testi: hedef crate'ten, neden kendi hedefinden.
    md = render(parse(SYNTH["J_same_name"]), "v2", "t", None, None)
    check("`kama_a(lib)::tests::must_panic` — a nedeni" in md, "J", "a hedefi/nedeni")
    check("`kama_b(lib)::tests::must_panic` — b nedeni" in md, "J", "b hedefi/nedeni")

    # L. yol= taşıyan satır yok: kol doğrulanmadı uyarısı.
    check("doğrulanmadı" in render(parse(SYNTH["L_diag_only"]), "v2", "t", None, None), "L", "uyarı yok")

    sections = ["A", "A2", "H", "B", "C", "D", "E", "F", "G", "I", "J", "L"]
    bad = {e.split(":", 1)[0] for e in errors}
    for e in errors:
        print(f"ÖZ-TEST KALDI: {e}", file=sys.stderr)
    print(f"test_summary öz-test: {len(sections) - len(bad)}/{len(sections)} bölüm geçti")
    return 1 if errors else 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("log", type=Path)
    ap.add_argument("--arm", required=True, choices=sorted(ARMS))
    ap.add_argument("--title", default="test sonuçları")
    ap.add_argument("--log-url")
    ap.add_argument("--rerun", help="işin kendi komutu (env dahil); düşen testin adı sonuna eklenir")
    a = ap.parse_args(argv)
    try:
        text = a.log.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        print(f"günlük okunamadı: {e}", file=sys.stderr)
        text = ""
    sys.stdout.write(render(parse(text), a.arm, a.title, a.log_url, a.rerun))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
