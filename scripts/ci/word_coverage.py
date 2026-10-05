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
  2. YERİNDE KORUNMA [2026-10-05; 2026-10-06'da incelemeden sonra düzeltildi]: taban sürümden
     çıkarılan ya da değiştirilen her satır öbeği için, öbekteki eski satırların her kelimesi AYNI
     öbeğin yeni satırlarında ya (a) üstü çizilmemiş metinde, eski SIRASI korunarak (araya yeni kelime
     girebilir), ya da (b) üstü çizili bir yerde (`~~…~~`; yeri değişmiş olabilir) bulunmalıdır.
     Yani: eski metin ya olduğu gibi duruyordur, ya da üstü çizilmiştir.
       · Öbek = satır-düzeyi farkın değişen parçası; aralarında yalnız boş satır ya da dosyada birden
         çok kez geçen satır bulunan komşu parçalar TEK öbektir (fark algoritması böyle satırları
         yanlış eşleyip tek bir düzenlemeyi bölebilir).
       · Yeni dosyada kelime dizisi olarak bütün hâlinde duran satır (taşınmış ya da tümüyle üstü
         çizilmiş) muaftır — yeni dosyada o satırdan en az eskisi KADAR varsa. Muaf satırın öbek
         içindeki karşılığı başka satırlara örtü olmaz.
       · Üstü çizme durumu satırdan satıra taşınır, yeni blokta (boş satır, tablo satırı, başlık,
         liste maddesi) sıfırlanır.

NEDEN İKİNCİ ÖLÇÜM. Birincisi tek başına şunu göremez: eski cümle üstü çizilmeden silinir ya da
yeniden yazılır, ama kelimeleri dosyanın başka bir yerinde — aynı değişikliğin YAMA KAYDI satırında
ya da yerine yazılan cümlede — yeniden geçer; çokküme dengede kalır. Ölçüldü (2026-10-06):
  · #91'in (`5124acd1`) iki spec dosyasında o PR'ın üstünü çizdiği metin düpedüz silinseydi birinci
    ölçüm 0 kayıp verirdi; ikinci ölçüm ikisini de yakalar (birleştiği hâliyle ikisi de temiz).
  · Depo geçmişi (main'de `310deea1`'den geriye ilk-ebeveyn 60 commit; birinci ölçümden temiz geçen
    59 kapsam dosyası değişikliği): ikinci ölçüm 4'ünü işaretler ve dördü de gerçek, üstü çizilmemiş
    yerinde değişikliktir (üç tablo hücresi `| Abkarada |` → `| — |`; bir `0x19..0xFF` →
    `0x1B..0xFF`). Kalan 55'inde alarm yok.

SINIRLAR.
  YAKALAMAZ: bir cümlenin anlamının araya eklenenlerle değişmesini (eski kelimeler sırasıyla
  duruyorsa ekleme serbesttir); silinen kelimenin aynı öbekte, sırayı bozmayacak bir yerde yeniden
  geçmesini; tek kalmış bir `~~`'den sonra kendi bloğunun kalanında yapılan değişiklikleri (o kısım
  üstü çizili sayılır); EKLENEN ve kapsam dışından taşınan dosyaları (taban sürümleri yoktur —
  listelenir; başka bir daldan taşınan belge özgününe karşı ELLE karşılaştırılır). Çok büyük
  öbeklerde (kelime çarpımı > CELL_LIMIT) sıra aranmaz, yalnız kelimelerin öbekte bulunmasına bakılır.
  KURALA UYGUN OLDUĞU HÂLDE KIRMIZI VERİR: hem taşınan hem düzenlenen satır (beklenen: eskisi
  yerinde üstü çizilir, yenisi yeni yere yazılır). Kelime eşleştirmesi sezgiseldir: YEŞİL sonuç her
  zaman geçerli bir yerleştirmeye dayanır, ama var olan bir yerleştirme bulunamayabilir (aynı
  kelimenin öbekte hem düz hem üstü çizili yinelendiği girdiler).
  Bu bir silme kapısıdır, inceleme yerine geçmez.

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

Çıkış: 0 = kayıp yok · 1 = kelime kaybı ya da yerinde korunmayan satır öbeği · 2 = ortam/kullanım hatası.
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


# Satır başı: (alıntı işareti ve girintiden sonra) tablo satırı, başlık ya da liste maddesi.
BLOCK_START = re.compile(r"^[\s>]*(?:\||#{1,6}\s|[-*+]\s|\d+[.)]\s)")
TILDE_FENCE = re.compile(r"^[\s>]*~~~")

# Sıra araması |eski kelimeler| × |yeni kelimeler| hücrelik bir tablo kurar; bunun üstündeki öbeklerde
# yalnız kelimelerin öbekte bulunmasına bakılır (bkz. belge dizgisi, NE ÖLÇMEZ).
CELL_LIMIT = 2_000_000
# Eşleştirme sezgiseldir (bkz. `unplaced_words`); yeri bulunamayan kelimeler öne alınarak yeniden denenir.
PLACEMENT_ROUNDS = 4


def _key(line: str) -> tuple[str, ...]:
    """Satır kimliği: kelime dizisi. `~~`, boşluk, alıntı (`> `) ve tablo işaretleri kimliğe girmez."""
    return tuple(WORD.findall(line.replace("~~", " ")))


def _marked_words(lines: list[str]) -> list[list[tuple[str, bool]]]:
    """Her satır için [(kelime, üstü çizili mi)].

    `~~` açma/kapama durumu satırdan satıra TAŞINIR (parça birkaç satıra yayılabilir; depoda örnekleri
    var) ve yeni bir blokta sıfırlanır: boş satır, tablo satırı, başlık, liste maddesi (GFM'de üstü
    çizme blok sınırını aşmaz). Böylece tek kalmış bir `~~` dosyanın geri kalanını değil yalnız kendi
    bloğunun kalanını üstü çizili gösterir. `~~~` kod çiti üstü çizme işareti sayılmaz.
    """
    out: list[list[tuple[str, bool]]] = []
    struck = False
    for line in lines:
        if not line.strip():
            struck = False
            out.append([])
            continue
        if TILDE_FENCE.match(line):
            struck = False
            out.append([(w, False) for w in WORD.findall(line)])
            continue
        if BLOCK_START.match(line):
            struck = False
        marked: list[tuple[str, bool]] = []
        for k, segment in enumerate(line.split("~~")):
            if k:
                struck = not struck
            marked.extend((w, struck) for w in WORD.findall(segment))
        out.append(marked)
    return out


def _unmatched(old_words: list[str], plain: list[str], heavy: set[str]) -> Counter:
    """Ağırlıklı en uzun ortak alt dizi: `old_words`'ten `plain` içinde sırasıyla EŞLEŞMEYENLER.
    `heavy` kümesindeki kelimeler önce yerleştirilir."""
    n, m = len(old_words), len(plain)
    big = n + 1
    weight = [big if w in heavy else 1 for w in old_words]
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
    return left


def unplaced_words(old_words: list[str], plain: list[str], struck: Counter) -> Counter:
    """Eski kelime dizisinden yeni metinde YERİ BULUNAMAYANLAR.

    Bir eski kelimenin yeri vardır, eğer ya üstü çizilmemiş yeni metinde (`plain`) eski sırası korunarak
    eşleşiyorsa ya da üstü çizili kelimeler çokkümesinde (`struck`) karşılığı varsa.

    GÜVENCE: dönen her sonuç geçerli bir eşleştirmeden hesaplanır — boş sonuç, gerçekten bir yerleştirme
    bulunduğu demektir (yanlış YEŞİL üretmez). Tersi sezgiseldir: üstü çizili karşılığı olmayan kelimeler
    önce yerleştirilir, yeri bulunamayanlar öne alınıp yeniden denenir (`PLACEMENT_ROUNDS`). Aynı kelimenin
    hem düz hem üstü çizili tekrar ettiği kurgusal girdilerde var olan bir yerleştirme bulunamayabilir.
    """
    n, m = len(old_words), len(plain)
    if n == 0:
        return Counter()
    if n * m > CELL_LIMIT:
        have = Counter(plain) + struck
        need = Counter(old_words)
        return Counter({w: c - have[w] for w, c in need.items() if c > have[w]})
    heavy = {w for w in set(old_words) if struck[w] == 0}
    missing: Counter = Counter()
    for _ in range(PLACEMENT_ROUNDS):
        left = _unmatched(old_words, plain, heavy)
        missing = Counter({w: c - struck[w] for w, c in left.items() if c > struck[w]})
        if not missing or set(missing) <= heavy:
            break
        heavy |= set(missing)
    return missing


def displaced_lines(old: str, new: str) -> list[tuple[int, Counter, str]]:
    """YERİNDE KORUNMAYAN eski satır öbekleri: (taban satır no, yeri bulunamayan kelimeler, ilk satırın başı).

    Öbek = satır-düzeyi farkın değişen parçası. Aralarında yalnız ZAYIF çapa (boş satır ya da dosyada
    birden çok kez geçen satır) bulunan komşu parçalar TEK öbek sayılır: fark algoritması böyle
    satırları yanlış eşleyip tek bir düzenlemeyi ikiye bölebilir (ölçüldü: iki komşu paragraf yerinde
    düzenlenip üstlerine bir not paragrafı eklenince ilk paragraf yalnız notla karşılaştırılıyordu).

    Yeni dosyada kelime dizisi olarak bütün hâlinde duran eski satır (taşınmış ya da tümüyle üstü
    çizilmiş) muaftır — ama yalnız yeni dosyada o satırdan en az eskisi KADAR varsa; öbeğin içindeki
    karşılığı da başka satırlara örtü olmasın diye hesaptan düşülür.
    """
    o, n = old.splitlines(), new.splitlines()
    raw_old, raw_new = Counter(o), Counter(n)
    keys_old, keys_new = Counter(map(_key, o)), Counter(map(_key, n))
    marked = _marked_words(n)

    groups: list[list[int]] = []
    current: list[int] | None = None
    bridged = False
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(a=o, b=n, autojunk=False).get_opcodes():
        if tag == "equal":
            weak = all(not line.strip() or raw_old[line] > 1 or raw_new[line] > 1 for line in o[i1:i2])
            if current is not None and weak:
                bridged = True
            else:
                if current is not None:
                    groups.append(current)
                current, bridged = None, False
            continue
        if current is not None and bridged:
            current[1], current[3] = i2, j2
        else:
            if current is not None:
                groups.append(current)
            current = [i1, i2, j1, j2]
        bridged = False
    if current is not None:
        groups.append(current)

    out: list[tuple[int, Counter, str]] = []
    for i1, i2, j1, j2 in groups:
        home = [_key(line) for line in n[j1:j2]]
        used = [False] * (j2 - j1)
        gone: list[str] = []
        for line in o[i1:i2]:
            key = _key(line)
            if not key:
                continue
            if keys_new[key] >= keys_old[key]:
                for idx, candidate in enumerate(home):
                    if candidate == key and not used[idx]:
                        used[idx] = True
                        break
                continue
            gone.append(line)
        if not gone:
            continue
        plain: list[str] = []
        struck: Counter = Counter()
        for idx in range(j2 - j1):
            if used[idx]:
                continue
            for word, is_struck in marked[j1 + idx]:
                if is_struck:
                    struck[word] += 1
                else:
                    plain.append(word)
        missing = unplaced_words([w for line in gone for w in _key(line)], plain, struck)
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


def name_status(root: Path, base: str) -> list[tuple[str, str, str | None]]:
    """(durum harfi, yol, yeniden-adlandırmada yeni yol | None). `-z`: ASCII dışı yol adları tırnaklanmaz."""
    fields = git(root, "-c", "core.quotepath=false", "diff", "--name-status", "-M", "-z", base, "--").split("\0")
    out: list[tuple[str, str, str | None]] = []
    i = 0
    while i < len(fields) and fields[i]:
        status = fields[i][0]
        if status in ("R", "C"):
            out.append((status, fields[i + 1], fields[i + 2]))
            i += 3
        else:
            out.append((status, fields[i + 1], None))
            i += 2
    return out


def changed(root: Path, base: str) -> list[tuple[str, str | None]]:
    """(taban yolu, yeni yol | None) çiftleri — taban sürümde VAR olan, değişmiş kapsam dosyaları."""
    pairs: list[tuple[str, str | None]] = []
    for status, path, renamed_to in name_status(root, base):
        if not in_scope(path):
            continue
        if status == "R":
            pairs.append((path, renamed_to))
        elif status == "D":
            pairs.append((path, None))
        elif status in ("M", "T"):
            pairs.append((path, path))
        # "A" (yeni dosya): taban sürümü yok, kaybolacak kelime de yok — `unaudited()` ayrıca listeler.
    return pairs


def unaudited(root: Path, base: str) -> list[tuple[str, str]]:
    """Kapsama GİREN ama taban sürümü kapsamda olmayan dosyalar: (yol, neden). Denetlenemezler;
    görünür olsunlar diye listelenirler."""
    out: list[tuple[str, str]] = []
    for status, path, renamed_to in name_status(root, base):
        if status == "A" and in_scope(path):
            out.append((path, "YENİ dosya — taban sürümü yok"))
        elif status in ("R", "C") and renamed_to and in_scope(renamed_to) and not in_scope(path):
            out.append((renamed_to, f"kapsam dışından geldi ({path})"))
    return out


def run(root: Path, base: str, only: list[str]) -> int:
    pairs = changed(root, base)
    fresh = unaudited(root, base)
    if only:
        wanted = {str(Path(p)) for p in only}
        pairs = [(o, n) for o, n in pairs if o in wanted or (n and n in wanted)]
        fresh = [(p, why) for p, why in fresh if p in wanted]
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
            if not in_scope(new_path):
                label += " (KAPSAM DIŞINA taşındı)"
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
    for path, why in fresh:
        print(f"  {path}: {why} — denetlenmedi")
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
        # 2026-10-06 — #93 incelemesinin bulguları
        ("iki komşu paragraf yerinde düzenlendi, üstlerine not paragrafı eklendi (fark boş satırları yanlış eşler)",
         "## 4\n\nP1 eski bir.\n\nP2 eski iki.\n\n## 5\n",
         "## 4\n\n> YAMA: not.\n\nP1 ~~eski~~ yeni bir.\n\nP2 ~~eski~~ yeni iki.\n\n## 5\n", 0, 0),
        ("üç paragraf yerinde düzenlendi, araya yeni paragraf girdi",
         "P1 eski bir.\n\nP2 eski iki.\n\nP3 eski üç.\n",
         "P1 ~~eski~~ yeni bir.\n\nAraya giren.\n\nP2 ~~eski~~ yeni iki.\n\nP3 ~~eski~~ yeni üç.\n", 0, 0),
        ("yinelenen satırın bir kopyası silindi; öbür kopya muafiyet sağlamaz",
         "## A\nDurum: AÇIK bekliyor\n## B\nDurum: AÇIK bekliyor\nson\n",
         "> YAMA: B için Durum AÇIK bekliyor satırı kaldırıldı\n## A\nDurum: AÇIK bekliyor\n## B\nson\n", 0, 1),
        ("bütünüyle üstü çizilen komşu satır, başka satırdaki silmeyi örtmez",
         "başlık\nfoo bar\nfoo baz qux\nson\n",
         "> YAMA: foo\nbaşlık\n~~foo bar~~\nbaz qux\nson\n", 0, 1),
        ("komşu tablo satırı üstü çizildi; bu satırdan silinen kelime onunla örtülmez",
         "| KS-8 | connect kalıcı niyet |\n| KS-9 | connect sonrası idle |\nson\n",
         "> YAMA: connect\n| ~~KS-8~~ | ~~connect kalıcı niyet~~ |\n| KS-9 | sonrası idle |\nson\n", 0, 1),
        ("taşınan satır alıntıya alındı (kelime dizisi aynı)",
         "A\nB eski metin burada\nC\nD\n", "A\nC\nD\n\n> B eski metin burada\n", 0, 0),
        ("SINIR (yanlış kırmızı): satır hem taşındı hem düzenlendi — eskisi yerinde üstü çizilmeli",
         "A\nB eski metin\nC\nD\n", "A\nC\nD\nB ~~eski~~ yeni metin\n", 0, 1),
        ("aynı taşıma kurala uygun yapıldı (eskisi yerinde üstü çizili, yenisi yeni yerde)",
         "A\nB eski metin\nC\nD\n", "A\n~~B eski metin~~\nC\nD\nB yeni metin\n", 0, 0),
        ("tek kalmış ~~ yalnız kendi bloğunu etkiler; sonraki paragraftaki sıra değişikliği yakalanır",
         "bir iki\n\nalfa beta gama\nson\n", "bir ~~ iki\n\nalfa gama beta\nson\n", 0, 1),
        ("tek kalmış ~~ sonraki tablo satırına taşmaz",
         "| a | bir iki |\n| b | alfa beta gama |\nson\n", "| a | bir ~~ iki |\n| b | alfa gama beta |\nson\n", 0, 1),
        ("üstü çizili parça liste maddesinin devam satırına yayılıyor",
         "- bir iki\n  üç dört\n- son\n", "- ~~bir iki\n  üç dört~~ yeni\n- son\n", 0, 0),
        ("iç içe üstü çizme (~~…~~…~~~~) dengeyi bozmaz",
         "| A2 | eski hüküm |\nalfa beta gama\n", "| ~~A2~~ | KAPANDI. Önceki: ~~eski ~~hüküm~~~~ |\nalfa beta gama\n", 0, 0),
        ("~~~ kod çiti üstü çizme sayılmaz; çit içindeki sıra değişikliği yakalanır",
         "~~~\nbir iki üç\n~~~\nson\n", "~~~\nüç iki bir\n~~~\nson\n", 0, 1),
        ("eşleştirme eşitliği: üstü çizili parça + düz tekrar (yeniden deneme çözer)",
         "c b b\n", "~~c b~~ b c\n", 0, 0),
        ("paragraf yeniden sarıldı (kelime sırası aynı)",
         "bir iki üç dört\nbeş altı yedi\nsekiz dokuz\n", "bir iki üç\ndört beş altı\nyedi sekiz dokuz\n", 0, 0),
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
