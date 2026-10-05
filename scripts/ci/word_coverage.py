#!/usr/bin/env python3
"""Silme kapısı — spec/karar belgelerinde "üstü çiz, silme" kuralının ölçümü.

    scripts/ci/word_coverage.py [--base REF] [--root DİZİN] [DOSYA...]
    scripts/ci/word_coverage.py --selftest

KURAL. Donmuş spec'lerde ve karar belgelerinde eski metin SİLİNMEZ: yerine yenisi yazılacaksa
eskisinin üstü çizilir (`~~…~~`) ve yanına yenisi eklenir. Bu betik kuralı İKİ ölçümle denetler;
ikisinden biri sıfır değilse kapı kırmızıdır. Üstü çizili metin sayılır (`~~` işaretleri atılır,
içerik kalır).

  1. DOSYA GENELİ KELİME ÇOKKÜMESİ: kapsamdaki her dosya için TABAN sürümündeki her kelimenin geçiş
     sayısı, yeni sürümde EN AZ o kadar olmalıdır ("kaybolan kelime = 0").
  2. YERİNDE KORUNMA [2026-10-05]: taban sürümden çıkarılan ya da değiştirilen her satır öbeği
     (satır-düzeyi fark) için, öbekteki eski satırların her kelimesi AYNI öbeğin yeni satırlarında
     ya (a) üstü çizilmemiş metinde, eski SIRASI korunarak (araya yeni kelime girebilir), ya da
     (b) üstü çizili bir yerde (`~~…~~`; satır içinde yeri değişmiş olabilir) bulunmalıdır. Yeni
     dosyada bütün hâlinde duran satır (yeri değişmiş olsa da) taşınmış sayılır ve muaftır.
     Yani: eski metin ya olduğu gibi duruyordur, ya da üstü çizilmiştir.

NEDEN İKİNCİ ÖLÇÜM. Birincisi tek başına şunu göremez: eski cümle üstü çizilmeden silinir ya da
yeniden yazılır, ama kelimeleri dosyanın başka bir yerinde — aynı değişikliğin YAMA KAYDI satırında
ya da yerine yazılan cümlede — yeniden geçer; çokküme dengede kalır. Ölçüldü (2026-10-05):
  · #91'in iki spec dosyasında üstü çizilen metin düpedüz silinseydi birinci ölçüm 0 kayıp verirdi;
    ikinci ölçüm ikisini de yakalar (birleştiği hâliyle ikisi de temiz).
  · Depo geçmişi (main'de `310deea1`'den geriye 60 commit; birinci ölçümden temiz geçen 59 kapsam dosyası
    değişikliği): ikinci ölçüm 4'ünü işaretler ve dördü de gerçek, üstü çizilmemiş yerinde
    değişikliktir (üç tablo hücresi `| Abkarada |` → `| — |`; bir `0x19..0xFF` → `0x1B..0xFF`).
    Yanlış alarm yok.

NE ÖLÇMEZ. Bir cümlenin anlamının araya eklenenlerle değişmesini (eski kelimeler sırasıyla duruyorsa
ekleme serbesttir); silinen kelimenin aynı öbekte, sırayı bozmayacak bir yerde yeniden geçmesini;
EKLENEN dosyaları (taban sürümleri yoktur — başka bir daldan taşınan belge özgününe karşı ELLE
karşılaştırılır). Çok büyük öbeklerde (kelime çarpımı > CELL_LIMIT) ikinci ölçüm sırayı aramaz,
yalnız kelimelerin öbekte bulunmasına bakar. Bu bir silme kapısıdır, inceleme yerine geçmez.

KAPSAM (SCOPE): specs/, docs/seams/, docs/decisions/, docs/sprint_plan/ altındaki .md dosyaları ile
adıyla sayılan tekil belgeler. Kapsam dışındaki .md dosyaları (raporlar, README'ler) serbesttir.

TABAN. `--base` verilmezse `origin/main` ile HEAD'in ortak atası. `--base REF` verilirse REF'in UCU
kullanılır (ortak ata DEĞİL): diskteki ağaç o uçla birleştirilmemişse taban dalda sonradan eklenen
kelimeler "kayıp" görünür. Bu yüzden CI `--base HEAD^1` verir (checkout edilen PR merge commit'inin
ilk ebeveyni = birleştirilen taban); `origin/<dal>` vermek, main ilerledikten sonra yeniden koşturulan
eski bir işte sahte kayıp üretirdi. Karşılaştırılan yeni sürüm DİSKTEKİ dosyadır (yerelde commit'ten
ÖNCE koşulabilir).

SİLİNEN / TAŞINAN DOSYA. Kapsamdaki bir dosya silinirse bütün kelimeleri kaybolmuş sayılır (kırmızı).
Git'in yeniden-adlandırma olarak tanıdığı taşımada eski yol yeni yolla karşılaştırılır.

Çıkış: 0 = kayıp yok · 1 = kelime kaybı · 2 = ortam/kullanım hatası.
"""

from __future__ import annotations

import difflib
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


def _norm(line: str) -> str:
    """Satır kimliği: `~~` işaretleri ve boşluk farkları atılmış hâli."""
    return " ".join(line.replace("~~", " ").split())


# Üstü çizili parça (satır sonlarını aşabilir).
STRUCK = re.compile(r"~~(.+?)~~", re.S)
# Sıra araması |eski kelimeler| × |yeni kelimeler| hücrelik bir tablo kurar; bunun üstündeki öbeklerde
# yalnız kelimelerin öbekte bulunmasına bakılır (bkz. belge dizgisi, NE ÖLÇMEZ).
CELL_LIMIT = 2_000_000


def unplaced_words(old_words: list[str], new_text: str) -> Counter:
    """Eski kelime dizisinden yeni metinde YERİ BULUNAMAYANLAR.

    Bir eski kelimenin yeri vardır, eğer ya üstü çizilmemiş yeni metinde eski sırası korunarak
    eşleşiyorsa ya da üstü çizili bir parçanın içindeyse. Eşleşme ağırlıklı en uzun ortak alt dizidir:
    üstü çizili karşılığı OLMAYAN kelimeler önce yerleştirilir (onların başka çıkışı yoktur).
    """
    struck = Counter(w for span in STRUCK.findall(new_text) for w in WORD.findall(span))
    plain = WORD.findall(STRUCK.sub(" ", new_text).replace("~~", " "))
    n, m = len(old_words), len(plain)
    if n == 0:
        return Counter()
    if n * m > CELL_LIMIT:
        have = Counter(plain) + struck
        need = Counter(old_words)
        return Counter({w: c - have[w] for w, c in need.items() if c > have[w]})
    heavy = n + 1
    weight = [1 if struck[w] > 0 else heavy for w in old_words]
    # best[i][j] = old_words[i:] ile plain[j:] arasında eşleşebilen en büyük toplam ağırlık
    best = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n - 1, -1, -1):
        row, below, word, w = best[i], best[i + 1], old_words[i], weight[i]
        for j in range(m - 1, -1, -1):
            v = below[j] if below[j] >= row[j + 1] else row[j + 1]
            if plain[j] == word and below[j + 1] + w > v:
                v = below[j + 1] + w
            row[j] = v
    left: Counter = Counter()
    i = j = 0
    while i < n:
        if j < m and plain[j] == old_words[i] and best[i][j] == best[i + 1][j + 1] + weight[i]:
            i += 1
            j += 1
        elif j < m and best[i][j] == best[i][j + 1]:
            j += 1
        else:
            left[old_words[i]] += 1
            i += 1
    return Counter({w: c - struck[w] for w, c in left.items() if c > struck[w]})


def displaced_lines(old: str, new: str) -> list[tuple[int, Counter, str]]:
    """YERİNDE KORUNMAYAN eski satır öbekleri: (taban satır no, yeri bulunamayan kelimeler, ilk satırın başı).

    Satır-düzeyi farkın her `replace`/`delete` öbeğinde eski satırların kelimeleri aynı öbeğin yeni
    satırlarında aranır (`unplaced_words`). Yeni dosyada bütün hâlinde (herhangi bir yerde) duran eski
    satır taşınmış sayılır. Net kelime kaybını `lost_words` ölçer; bu fonksiyon YERİ ölçer.
    """
    o, n = old.splitlines(), new.splitlines()
    whole = {_norm(line) for line in n}
    out: list[tuple[int, Counter, str]] = []
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(a=o, b=n, autojunk=False).get_opcodes():
        if tag not in ("replace", "delete"):
            continue
        gone = [line for line in o[i1:i2] if _norm(line) and _norm(line) not in whole]
        if not gone:
            continue
        missing = unplaced_words(WORD.findall("\n".join(gone).replace("~~", " ")), "\n".join(n[j1:j2]))
        if missing:
            out.append((i1 + 1, missing, gone[0].strip()[:90]))
    return out


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
        # "A" (yeni dosya): taban sürümü yok, kaybolacak kelime de yok — `added()` ayrıca listeler.
    return pairs


def added(root: Path, base: str) -> list[str]:
    """Taban sürümde OLMAYAN kapsam dosyaları (denetlenemez; görünür olsun diye listelenir)."""
    out = git(root, "diff", "--name-status", "-M", base, "--")
    return [parts[1] for parts in (line.split("\t") for line in out.splitlines())
            if parts[0] == "A" and in_scope(parts[1])]


def run(root: Path, base: str, only: list[str]) -> int:
    pairs = changed(root, base)
    fresh = added(root, base)
    if only:
        wanted = {str(Path(p)) for p in only}
        pairs = [(o, n) for o, n in pairs if o in wanted or (n and n in wanted)]
        fresh = [p for p in fresh if p in wanted]
    total = 0
    displaced_total = 0
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
        # Dosya silindiyse ikinci ölçümün söyleyeceği yeni bir şey yoktur (her satır kayıptır).
        moved = displaced_lines(old, new) if new_path is not None else []
        displaced_total += len(moved)
        if n:
            sample = ", ".join(f"{w}×{c}" if c > 1 else w for w, c in lost.most_common(12))
            print(f"::error title=WORD-COVERAGE::{label}: {n} kelime kayboldu ({sample})")
            print(f"KIRMIZI: {label}: kaybolan kelime {n} — {sample}")
        for line_no, gone, head in moved:
            sample = ", ".join(f"{w}×{c}" if c > 1 else w for w, c in gone.most_common(8))
            print(f"::error title=WORD-COVERAGE::{label}: taban satır ~{line_no} yerinde korunmamış ({sample})")
            print(f"KIRMIZI: {label}: taban satır ~{line_no} yerinde korunmamış — eksilen: {sample} | {head}")
        if not n and not moved:
            print(f"  {label}: kaybolan kelime 0 · yerinde korunmayan satır 0")
    for path in fresh:
        print(f"  {path}: YENİ dosya — taban sürümü yok, denetlenmedi")
    if total or displaced_total:
        print(f"\nWORD-COVERAGE: KIRMIZI (kaybolan kelime {total}; yerinde korunmayan satır öbeği "
              f"{displaced_total}; taban {base[:12]}) — eski metni silmeyin, YERİNDE üstünü çizin (~~…~~) "
              "ve yenisini yanına yazın")
        return 1
    print(f"WORD-COVERAGE: YEŞİL ({len(pairs)} kapsam dosyası değişmiş; kaybolan kelime 0; "
          f"yerinde korunmayan satır 0; taban {base[:12]})")
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
    # İkinci ölçüm (yerinde korunma). Her vakada birinci ölçümün ne dediği de sabitlenir: bu ölçümün
    # varlık nedeni, birincinin 0 verdiği silmeleri yakalamaktır.
    yama = "> YAMA KAYDI: replay identity (session, direction, epoch, counter) → (session, direction, counter)\n"
    eski = "replay identity = (session, direction, epoch, counter).\n"
    placed = [
        ("eski satır silindi, kelimeleri YAMA KAYDI'nda yeniden geçiyor",
         "başlık\n" + eski + "son\n",
         yama + "başlık\nreplay identity = (session, direction, counter).\nson\n", 0, 1),
        ("aynı değişiklik yerinde üstü çizilerek yapıldı",
         "başlık\n" + eski + "son\n",
         yama + "başlık\n~~" + eski.strip() + "~~ replay identity = (session, direction, counter).\nson\n", 0, 0),
        ("cümle üstü çizilmeden yeniden yazıldı; silinen kelime aynı öbekte başka yerde geçiyor",
         "başlık\n" + eski + "son\n",
         "başlık\nreplay identity = (session, direction, counter). epoch kimlikten çıktı.\nson\n", 0, 1),
        ("tablo hücresi yerinde değişti, kelime başka yerde geçiyor",
         "başlık\n| KS-8 | değer | Abkarada |\ndipnot\n",
         "> YAMA KAYDI — Abkarada kararı\nbaşlık\n| ~~KS-8~~ | ~~değer~~ KAPANDI | — |\ndipnot\n", 0, 1),
        ("aynı hücre üstü çizilerek değişti",
         "başlık\n| KS-8 | değer | Abkarada |\ndipnot\n",
         "> YAMA KAYDI — Abkarada kararı\nbaşlık\n| ~~KS-8~~ | ~~değer~~ KAPANDI | ~~Abkarada~~ — |\ndipnot\n", 0, 0),
        ("sayı yerinde değiştirildi, eskisi dosyada başka yerde geçiyor",
         "0x19 atandı\naralık: 0x19..0xFF reserved\nson\n",
         "0x19 atandı\naralık: 0x1B..0xFF reserved\nson\n0x19 notu\n", 0, 1),
        ("üstü çizili parça satır içinde yer değiştirdi (başlık)",
         "## 8. N-Mesaj ACK Penceresi + Idempotency + Dedup\n",
         "## 8. Mailbox Kotası + Idempotency + Dedup (~~N-Mesaj ACK Penceresi~~ — üçe ayrıldı)\n", 0, 0),
        ("aynı kelimenin bir geçişi üstü çizili, diğerleri düz",
         "+ duplicate valid reject · too-old reject · future-gap reject · seq16 wrap\n",
         "+ duplicate valid reject · too-old reject · ~~future-gap reject~~ far-future ACCEPT · seq16 wrap\n", 0, 0),
        ("araya kelime eklendi (eski sıra duruyor)", "alfa beta gama\n", "alfa yeni beta başka gama\n", 0, 0),
        ("iki kelime üstü çizilmeden yer değiştirdi", "alfa beta gama\n", "alfa gama beta\n", 0, 1),
        ("bütün satır başka yere taşındı", "A satırı\nB satırı\nC satırı\n", "A satırı\nC satırı\nek\nB satırı\n", 0, 0),
        ("satır aynı öbekte ikiye bölündü", "bir iki üç dört beş\nson\n", "bir iki üç\ndört beş\nson\n", 0, 0),
        ("satır bütünüyle üstü çizildi, altına yenisi yazıldı", "eski hüküm burada\n", "~~eski hüküm burada~~\nyeni hüküm\n", 0, 0),
        ("üstü çizili parça iki satıra yayılıyor", "bir iki\nüç dört\nson\n", "~~bir iki\nüç dört~~ yeni metin\nson\n", 0, 0),
        ("yalnız ekleme (araya satır)", "A\nB\n", "A\nyeni\nB\n", 0, 0),
        ("kelime düpedüz silindi (iki ölçüm de görür)", "alfa beta gama\n", "alfa gama\n", 1, 1),
        ("SINIR (yakalanmaz): silinen kelime aynı öbekte, sırayı bozmayan yerde yeniden geçiyor",
         "| KS-8 | değer | Abkarada |\ndipnot\n",
         "> not — Abkarada kararı\n| ~~KS-8~~ | ~~değer~~ KAPANDI | — |\ndipnot\n", 0, 0),
    ]
    for name, old, new, want_lost, want_displaced in placed:
        got_lost = sum(lost_words(old, new).values())
        got_displaced = len(displaced_lines(old, new))
        ok = (got_lost, got_displaced) == (want_lost, want_displaced)
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  yerinde: {name}: beklenen (kayıp {want_lost}, öbek {want_displaced}), "
              f"gelen ({got_lost}, {got_displaced})")
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
