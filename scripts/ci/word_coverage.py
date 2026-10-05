#!/usr/bin/env python3
"""Silme kapısı — spec/karar belgelerinde "üstü çiz, silme" kuralının ölçümü.

    scripts/ci/word_coverage.py [--base REF] [--root DİZİN] [DOSYA...]
    scripts/ci/word_coverage.py --selftest

KURAL. Donmuş spec'lerde ve karar belgelerinde eski metin SİLİNMEZ: yerine yenisi yazılacaksa
eskisinin üstü çizilir (`~~…~~`) ve yanına yenisi eklenir. Bu betik kuralı sayarak denetler:
kapsamdaki her dosya için TABAN sürümündeki her kelimenin geçiş sayısı, yeni sürümde EN AZ o kadar
olmalıdır. Üstü çizili metin sayılır (`~~` işaretleri atılır, içerik kalır). Kaybolan kelime = 0
değilse kapı kırmızıdır.

NE ÖLÇER, NE ÖLÇMEZ. Ölçülen şey kelime ÇOKKÜMESİDİR: bir kelimenin silinmesini yakalar; kelimelerin
yerinin değişmesini ya da bir cümlenin anlamının çevresine eklenenlerle değişmesini YAKALAMAZ. Bu
bir silme kapısıdır, inceleme yerine geçmez.

KAPSAM (SCOPE): specs/, docs/seams/, docs/decisions/, docs/sprint_plan/ altındaki .md dosyaları ile
adıyla sayılan tekil belgeler. Kapsam dışındaki .md dosyaları (raporlar, README'ler) serbesttir.

TABAN. `--base` verilmezse `origin/main` ile HEAD'in ortak atası. Karşılaştırılan yeni sürüm DİSKTEKİ
dosyadır (CI'da checkout = HEAD; yerelde commit'ten ÖNCE koşulabilir).

SİLİNEN / TAŞINAN DOSYA. Kapsamdaki bir dosya silinirse bütün kelimeleri kaybolmuş sayılır (kırmızı).
Git'in yeniden-adlandırma olarak tanıdığı taşımada eski yol yeni yolla karşılaştırılır.

Çıkış: 0 = kayıp yok · 1 = kelime kaybı · 2 = ortam/kullanım hatası.
"""

from __future__ import annotations

import re
import subprocess
import sys
from collections import Counter
from fnmatch import fnmatch
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[2]

SCOPE = (
    "specs/*.md",
    "specs/**/*.md",
    "docs/seams/*.md",
    "docs/seams/**/*.md",
    "docs/decisions/*.md",
    "docs/decisions/**/*.md",
    "docs/sprint_plan/*.md",
    "docs/sprint_plan/**/*.md",
    "docs/PARK-LISTESI.md",
    "docs/hayri-app.md",
)

# Kelime: harf/rakam/alt çizgi/§ dizisi; içinde kesme ya da tire olabilir, başında/sonunda olamaz.
# Noktalama ve Markdown işaretleri kelimeye DAHİL DEĞİLDİR — üstü çizilen bir ifadenin yanındaki
# nokta ya da parantez yer değiştirdiğinde yanlış alarm üretmemek için.
WORD = re.compile(r"[\w§]+(?:['’\-][\w§]+)*", re.UNICODE)


def words(text: str) -> Counter:
    return Counter(WORD.findall(text.replace("~~", " ")))


def lost_words(old: str, new: str) -> Counter:
    """Taban sürümde olup yeni sürümde eksilen kelimeler (kelime → eksik geçiş sayısı)."""
    o, n = words(old), words(new)
    return Counter({w: c - n[w] for w, c in o.items() if c > n[w]})


def in_scope(path: str) -> bool:
    return any(fnmatch(path, pat) for pat in SCOPE)


def git(root: Path, *args: str) -> str:
    r = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    if r.returncode != 0:
        raise EnvironmentError(f"git {' '.join(args)}: {r.stderr.strip()[:300]}")
    return r.stdout


def changed(root: Path, base: str) -> list[tuple[str, str | None]]:
    """(taban yolu, yeni yol | None) çiftleri — taban sürümde VAR olan, değişmiş kapsam dosyaları."""
    out = git(root, "diff", "--name-status", "-M", base, "--")
    pairs: list[tuple[str, str | None]] = []
    for line in out.splitlines():
        parts = line.split("\t")
        status = parts[0]
        if status.startswith("R"):
            old, new = parts[1], parts[2]
            if in_scope(old):
                pairs.append((old, new))
        elif status == "D":
            if in_scope(parts[1]):
                pairs.append((parts[1], None))
        elif status in ("M", "T"):
            if in_scope(parts[1]):
                pairs.append((parts[1], parts[1]))
        # "A" (yeni dosya): taban sürümü yok, kaybolacak kelime de yok.
    return pairs


def run(root: Path, base: str, only: list[str]) -> int:
    pairs = changed(root, base)
    if only:
        wanted = {str(Path(p)) for p in only}
        pairs = [(o, n) for o, n in pairs if o in wanted or (n and n in wanted)]
    total = 0
    for old_path, new_path in pairs:
        old = git(root, "show", f"{base}:{old_path}")
        if new_path is None:
            new = ""
            label = f"{old_path} (SİLİNDİ)"
        else:
            new = (root / new_path).read_text(encoding="utf-8")
            label = old_path if new_path == old_path else f"{old_path} → {new_path}"
        lost = lost_words(old, new)
        n = sum(lost.values())
        total += n
        if n:
            sample = ", ".join(f"{w}×{c}" if c > 1 else w for w, c in lost.most_common(12))
            print(f"::error title=WORD-COVERAGE::{label}: {n} kelime kayboldu ({sample})")
            print(f"KIRMIZI: {label}: kaybolan kelime {n} — {sample}")
        else:
            print(f"  {label}: kaybolan kelime 0")
    if total:
        print(f"\nWORD-COVERAGE: KIRMIZI (toplam {total} kelime kayboldu; taban {base[:12]}) — "
              "eski metni silmeyin, üstünü çizin (~~…~~)")
        return 1
    print(f"WORD-COVERAGE: YEŞİL ({len(pairs)} kapsam dosyası değişmiş; kaybolan kelime 0; taban {base[:12]})")
    return 0


def selftest() -> int:
    cases = [
        ("üstü çizilip yanına yenisi yazıldı", "N_g = [KALİBRASYON] paket.", "~~N_g = [KALİBRASYON]~~ N_g = 64 paket.", 0),
        ("kelime silindi", "sayaç epoch-1'de 0'dan başlar", "sayaç epoch-1'de başlar", 1),
        ("yalnız ekleme", "A B C", "A B C D E", 0),
        ("yinelenen kelimenin biri silindi", "PN PN PN", "PN PN", 1),
        ("satır taşındı (sıra değişti)", "birinci satır\nikinci satır", "ikinci satır\nbirinci satır", 0),
        ("noktalama yer değiştirdi", "(session, direction, dh_epoch). Gerekçe", "~~(session, direction, dh_epoch)~~ **yeni**. Gerekçe", 0),
        ("kelime değiştirildi (üstü çizilmeden)", "Closed(h, Renewed)", "Closed(h, Superseded)", 1),
        ("değişiklik üstü çizilerek yapıldı", "Closed(h, Renewed)", "Closed(h, ~~Renewed~~ Superseded)", 0),
        ("dosya silindi", "bir iki üç", "", 3),
        ("tireli bileşik bozuldu", "epoch-0 kabulü", "epoch 0 kabulü", 1),
    ]
    bad = 0
    for name, old, new, want in cases:
        got = sum(lost_words(old, new).values())
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen kayıp {want}, gelen {got}")
    for path, want in (("specs/a.md", True), ("specs/x/b.md", True), ("docs/seams/KM-SEAM-AT-001.md", True),
                       ("docs/sprint_plan/SPRINT-BACKLOG-FREEZE.md", True), ("docs/PARK-LISTESI.md", True),
                       ("docs/hayri-app.md", True), ("README.md", False), ("reports/r.md", False),
                       ("docs/BRANCHES.md", False), ("specs/a.txt", False)):
        ok = in_scope(path) == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  kapsam {path}: beklenen {want}")
    print(f"word_coverage öz-test: {'YEŞİL' if not bad else f'{bad} BAŞARISIZ'}")
    return 1 if bad else 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()
    root, base, files = DEFAULT_ROOT, None, []
    i = 0
    while i < len(argv):
        a = argv[i]
        if a in ("--root", "--base"):
            if i + 1 >= len(argv):
                print(f"KULLANIM HATASI: {a} bir değer ister", file=sys.stderr)
                return 2
            if a == "--root":
                root = Path(argv[i + 1]).resolve()
            else:
                base = argv[i + 1]
            i += 2
        elif a.startswith("--"):
            print(f"KULLANIM HATASI: bilinmeyen argüman {a}", file=sys.stderr)
            return 2
        else:
            files.append(a)
            i += 1
    try:
        if base is None:
            base = git(root, "merge-base", "origin/main", "HEAD").strip()
        else:
            base = git(root, "rev-parse", "--verify", f"{base}^{{commit}}").strip()
        return run(root, base, files)
    except EnvironmentError as e:
        print(f"ORTAM HATASI: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
