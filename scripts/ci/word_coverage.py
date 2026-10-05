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
  2. YERİNDE KORUNMA [2026-10-05; 2026-10-06'da iki incelemeden sonra yeniden yazıldı]: taban
     sürümden çıkarılan ya da değiştirilen her satır öbeği için, öbekteki eski satırların her kelimesi
     AYNI öbeğin yeni satırlarında
       (a) eski SIRASI korunarak — üstü çizili ya da değil; araya yeni kelime girebilir — ya da
       (b) KENDİ SATIRINDA üstü çizili olarak (üstü çizili parça satır içinde yer değiştirmiş olabilir)
     bulunmalıdır. Üstü çizilmemiş bir kelime yalnız sırasında yer bulur; BAŞKA bir satırın üstü
     çizili kelimesi karşılık olmaz. Yani: eski metin ya olduğu gibi duruyordur, ya da üstü çizilmiştir.
       · Öbek = satır-düzeyi farkın değişen parçası; aralarında yalnız boş satır ya da dosyada birden
         çok kez geçen satır bulunan komşu parçalar, aradaki satırlarla birlikte TEK öbektir (fark
         algoritması böyle satırları yanlış eşleyebilir).
       · Taşınan satır: kelimeleri öbeğinde yer bulamayan bir eski satır, kelime dizisi aynı olan ve
         hiçbir kelimesi başka satıra yer olmamış bir yeni satır varsa muaftır (bire bir).
       · Üstü çizme durumu satırdan satıra taşınır; yeni blokta (boş ya da yalnız `>` taşıyan satır,
         tablo satırı, başlık, liste maddesi, yatay çizgi, kod çiti) sıfırlanır.
       · Numaralı liste maddesinin sıra numarası (`3. `) bu ölçümde kelime sayılmaz: listeye madde
         eklenince sonrakilerin numarası kayar.

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
  YAKALAMAZ:
  · Bir cümlenin anlamının araya eklenenlerle değişmesini (eski kelimeler sırasıyla duruyorsa ekleme
    serbesttir).
  · Silinen kelimenin aynı öbekte, sırayı bozmayan bir yerde — düz ya da üstü çizili — yeniden
    geçmesini. Örnek: bir satırın SON kelimesi silinir; hemen sonraki satır aynı kelimeyle başlar ve
    orada `~~W~~ W …` yazılmıştır. Aynı SATIRIN içinde de: bir kelime silinir, aynı kelime satırın
    başka yerinde üstü çizilip yeniden yazılmıştır.
  · Yalnız işleç / noktalama değişikliğini — İKİ ÖLÇÜM DE görmez: kelime harf/rakam dizisidir;
    `≤` → `<`, `-3` → `3`, `10^-3` → `10^3`, `==` → `!=`, `→` → `←` kelime değiştirmez.
  · Numaralı liste maddesinin sıra numarasının değişmesini (bilerek; yukarıda).
  · Tek kalmış bir `~~`'den sonra kendi bloğunun kalanında yapılan değişiklikleri (o kısım üstü çizili
    sayılır). Aynı şey, dengeli bir üstü çizili parça bu betiğin blok saydığı bir satırı aşarsa da olur
    (ör. paragrafın sayı ve noktayla başlayan devam satırı). Başında `|` olmayan tablo satırı blok
    sayılmaz.
  · EKLENEN ve kapsam dışından taşınan dosyaları (taban sürümleri yoktur — listelenir; başka bir daldan
    taşınan belge özgününe karşı ELLE karşılaştırılır).
  · Çok büyük öbeklerde (kelime çarpımı > CELL_LIMIT) sırayı: yalnız kelimelerin öbekte bulunmasına
    bakılır ve çıktıya NOT düşülür.
  KURALA UYGUN OLDUĞU HÂLDE KIRMIZI VERİR:
  · Hem taşınan hem düzenlenen satır (beklenen: eskisi yerinde üstü çizilir, yenisi yeni yere yazılır).
    Hangi bloğun "taşındığına" fark algoritması karar verir: uzun bir blok, yerinde düzenlenmiş kısa
    bir bölümün üstüne taşınırsa kısa bölüm taşınmış sayılır.
  · Eski satırın bir PARÇASI ayrı bir satırda üstü çizilip kalanı başka satıra yazılırsa ve parça
    sırasında değilse (beklenen: parça kalan kelimelerle aynı satırda, ya da satır bütünüyle üstü çizili).
  · Numaralı bir TABLOYA araya satır eklenip sonraki satırların numarası değişirse.
  · Bileşik bir kelimenin (`epoch-0`, `2026-10-04`, `KS-8'in`) yalnız bir PARÇASI üstü çizilirse — birinci
    ölçümde de (beklenen: kelimenin bütünü çizilir).
  · Kelime eşleştirmesi sezgiseldir: "hepsi yerleşti" sonucu her zaman gerçek bir yerleştirmeye
    dayanır, ama var olan bir yerleştirme bulunamayabilir (aynı kelimenin öbekte hem düz hem üstü
    çizili yinelendiği girdiler).
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


# Yeni blok: (alıntı işareti ve girintiden sonra) tablo satırı, başlık, liste maddesi, yatay çizgi.
BLOCK_START = re.compile(r"^[\s>]*(?:\||#{1,6}\s|[-*+]\s|\d+[.)]\s|(?:[-*_]\s*){3,}$)")
BLANKISH = re.compile(r"^[\s>]*$")              # boş satır ya da yalnız alıntı işareti (`>`)
BACKTICK_FENCE = re.compile(r"^[\s>]*`{3,}[^`]*$")
TILDE_FENCE = re.compile(r"^[\s>]*~{3,}[^~]*$")
# Numaralı liste maddesinin sıra numarası (`3. `, `12) `). Listeye madde eklenince sonraki maddelerin
# numarası değişir; numara içerik değildir, ikinci ölçümde kelime sayılmaz (birinci ölçüm sayar).
LIST_ORDINAL = re.compile(r"^([\s>]*)\d+([.)]\s)")

# Sıra araması |eski kelimeler| × |yeni kelimeler| hücrelik bir tablo kurar; bunun üstündeki öbeklerde
# yalnız kelimelerin öbekte bulunmasına bakılır ve çıktıya not düşülür (bkz. belge dizgisi, SINIRLAR).
CELL_LIMIT = 2_000_000
# Eşleştirme sezgiseldir (bkz. `unplaced_words`); yeri bulunamayan kelimeler öne alınarak yeniden denenir.
PLACEMENT_ROUNDS = 4


def _body(line: str) -> str:
    return LIST_ORDINAL.sub(r"\1\2", line, count=1)


def _key(line: str) -> tuple[str, ...]:
    """Satır kimliği: kelime dizisi. `~~`, boşluk, alıntı (`> `), tablo işaretleri ve numaralı liste
    maddesinin sıra numarası kimliğe girmez."""
    return tuple(WORD.findall(_body(line).replace("~~", " ")))


def _marked_words(lines: list[str]) -> list[list[tuple[str, bool]]]:
    """Her satır için [(kelime, üstü çizili mi)].

    `~~` açma/kapama durumu satırdan satıra TAŞINIR (parça birkaç satıra yayılabilir; depoda örnekleri
    var) ve yeni bir blokta sıfırlanır: boş ya da yalnız `>` taşıyan satır, tablo satırı, başlık, liste
    maddesi, yatay çizgi, kod çiti satırı (GFM'de üstü çizme blok sınırını aşmaz). Böylece tek kalmış
    bir `~~` dosyanın geri kalanını değil yalnız kendi bloğunun kalanını üstü çizili gösterir.
    Kod çitinin İÇİNDE `~~` işaret sayılmaya devam eder: spec'lerdeki kod bloklarında eski satır
    yorum içinde üstü çizilerek korunur (`// ~~eski~~`).
    """
    out: list[list[tuple[str, bool]]] = []
    struck = False
    for line in lines:
        if BLANKISH.match(line):
            struck = False
            out.append([])
            continue
        if BACKTICK_FENCE.match(line) or TILDE_FENCE.match(line):
            struck = False
            out.append([(w, False) for w in WORD.findall(line)])
            continue
        if BLOCK_START.match(line):
            struck = False
        marked: list[tuple[str, bool]] = []
        for k, segment in enumerate(_body(line).split("~~")):
            if k:
                struck = not struck
            marked.extend((w, struck) for w in WORD.findall(segment))
        out.append(marked)
    return out


def _align(old_words: list[str], new_words: list[str], weight: list[int]) -> list[int | None]:
    """Ağırlıklı en uzun ortak alt dizi. Döner: her eski kelime için eşleştiği yeni kelimenin indeksi
    (eşleşmediyse None). Eşleşmeler sırayı korur."""
    n, m = len(old_words), len(new_words)
    # best[i][j] = old_words[i:] ile new_words[j:] arasında eşleşebilen en büyük toplam ağırlık
    best = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n - 1, -1, -1):
        row, below, word, w = best[i], best[i + 1], old_words[i], weight[i]
        for j in range(m - 1, -1, -1):
            v = below[j] if below[j] >= row[j + 1] else row[j + 1]
            if new_words[j] == word and below[j + 1] + w > v:
                v = below[j + 1] + w
            row[j] = v
    match: list[int | None] = [None] * n
    i = j = 0
    while i < n and j < m:
        if new_words[j] == old_words[i] and best[i][j] == best[i + 1][j + 1] + weight[i]:
            match[i] = j
            i += 1
            j += 1
        elif best[i][j] == best[i][j + 1]:
            j += 1
        else:
            i += 1
    return match


def _leftover(old: list[tuple[str, int]], new: list[tuple[str, bool, int]],
              match: list[int | None]) -> tuple[list[int], set[int]]:
    """Sırasıyla eşleşmeyen eski kelimelerden, AYNI SATIRDA üstü çizili bir karşılığı da bulunamayanların
    indeksleri; ve kullanılan yeni kelimelerin indeksleri.

    "Aynı satır": eski kelimenin satırından sırasıyla eşleşen kelimelerin bulunduğu yeni satırlar. Oradaki,
    sırasıyla eşleşmede kullanılmamış üstü çizili bir kelime karşılık sayılır (üstü çizili parça satır
    içinde yer değiştirmiş olabilir). Her üstü çizili kelime en çok BİR eski kelimeye karşılık olur
    (iki parçalı eşleme). Başka bir satırın üstü çizili kelimesi karşılık OLMAZ.
    """
    used = {j for j in match if j is not None}
    lines_of: dict[int, set[int]] = {}
    for (_, old_line), j in zip(old, match):
        if j is not None:
            lines_of.setdefault(old_line, set()).add(new[j][2])
    need: dict[str, list[int]] = {}
    for i, (word, _) in enumerate(old):
        if match[i] is None:
            need.setdefault(word, []).append(i)
    unplaced: list[int] = []
    for word, wanted in need.items():
        cands = [k for k, (w, is_struck, _) in enumerate(new) if is_struck and w == word and k not in used]
        owner: dict[int, int] = {}

        def place(i: int, seen: set[int]) -> bool:
            own_lines = lines_of.get(old[i][1], ())
            for k in cands:
                if k in seen or new[k][2] not in own_lines:
                    continue
                seen.add(k)
                if k not in owner or place(owner[k], seen):
                    owner[k] = i
                    return True
            return False

        for i in wanted:
            if not place(i, set()):
                unplaced.append(i)
        used |= set(owner)
    return sorted(unplaced), used


def too_large(old: list[tuple[str, int]], new: list[tuple[str, bool, int]]) -> bool:
    return len(old) * len(new) > CELL_LIMIT


def place_words(old: list[tuple[str, int]], new: list[tuple[str, bool, int]]) -> tuple[list[int], set[int]]:
    """Bir öbeğin eski kelimelerini yeni metne yerleştirir. Döner: YERİ BULUNAMAYAN eski kelimelerin
    indeksleri ve kullanılan yeni kelimelerin indeksleri.

    `old`: (kelime, eski satır no) — `new`: (kelime, üstü çizili mi, yeni satır no); ikisi de dosya
    sırasıyla. Bir eski kelimenin yeri vardır, eğer
      (a) yeni metinde — üstü çizili ya da değil — eski SIRASI korunarak eşleşiyorsa, ya da
      (b) kendi satırında, sıranın dışında kalmış üstü çizili bir karşılığı varsa (`_leftover`).
    Üstü çizilmemiş bir kelime yalnız (a) ile, yani sırasında yer bulur.

    GÜVENCE: "hepsi yerleşti" sonucu her zaman gerçekten bulunmuş bir yerleştirmeye dayanır. Tersi
    sezgiseldir: en çok kelimeyi sırasıyla eşleyen hizalama aranır; öbekte üstü çizili hiç geçmeyen
    kelimeler önceliklidir, yeri bulunamayanlar öne alınıp yeniden denenir (`PLACEMENT_ROUNDS`) ve en az
    eksik veren deneme döner. Var olan bir yerleştirme bulunamayabilir. İSTİSNA: öbek `CELL_LIMIT`'i
    aşıyorsa sıra ARANMAZ, yalnız kelimelerin öbekte bulunmasına bakılır (`too_large`; çağıran not düşer).
    """
    if not old:
        return [], set()
    old_words = [w for w, _ in old]
    new_words = [w for w, _, _ in new]
    if too_large(old, new):
        have = Counter(new_words)
        unplaced = []
        for i, word in enumerate(old_words):
            if have[word] > 0:
                have[word] -= 1
            else:
                unplaced.append(i)
        return unplaced, set(range(len(new)))
    n = len(old_words)
    struck_words = {w for w, is_struck, _ in new if is_struck}
    must = {w for w in old_words if w not in struck_words}
    promoted: set[str] = set()
    best: tuple[list[int], set[int]] | None = None
    for _ in range(PLACEMENT_ROUNDS):
        weight = [(n + 1) * (n + 1) if w in must else n + 1 if w in promoted else 1 for w in old_words]
        result = _leftover(old, new, _align(old_words, new_words, weight))
        if best is None or len(result[0]) < len(best[0]):
            best = result
        fresh = {old_words[i] for i in result[0]} - must - promoted
        if not result[0] or not fresh:
            break
        promoted |= fresh
    assert best is not None
    return best


def unplaced_words(old: list[tuple[str, int]], new: list[tuple[str, bool, int]]) -> Counter:
    """`place_words`'ün yeri bulunamayan kelimeleri (kelime → sayı)."""
    return Counter(old[i][0] for i in place_words(old, new)[0])


def displaced_lines(old: str, new: str, notes: list[str] | None = None) -> list[tuple[int, Counter, str]]:
    """YERİNDE KORUNMAYAN eski satır öbekleri: (taban satır no, yeri bulunamayan kelimeler, o satırın başı).

    ÖBEK = satır-düzeyi farkın değişen parçası. Aralarında yalnız ZAYIF çapa (boş satır ya da dosyada
    birden çok kez geçen satır) bulunan komşu parçalar, aradaki satırlarla BİRLİKTE tek öbek sayılır:
    fark algoritması böyle satırları yanlış eşleyip tek bir düzenlemeyi bölebilir ya da yinelenen bir
    satırın yanlış kopyasını "değişmedi" sayabilir (ikisi de ölçüldü).

    TAŞINAN SATIR. Kelimeleri kendi öbeğinde yer bulamayan bir eski satır, kelime dizisi aynı olan bir
    yeni satır (taşınmış ya da tümüyle üstü çizilmiş) bulursa muaftır — o yeni satırın hiçbir kelimesi
    başka bir eski satıra yer olmamışsa. Her yeni satır en çok BİR eski satırı muaf kılar ve muaf kıldıktan
    sonra kendi öbeğinde başka satırlara yer olmaz.
    """
    o, n = old.splitlines(), new.splitlines()
    raw_old, raw_new = Counter(o), Counter(n)
    marked = _marked_words(n)

    groups: list[tuple[list[int], list[int]]] = []    # (eski satır indeksleri, yeni satır indeksleri)
    current: tuple[list[int], list[int]] | None = None
    bridge: tuple[int, int, int, int] | None = None
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(a=o, b=n, autojunk=False).get_opcodes():
        if tag == "equal":
            weak = all(not line.strip() or raw_old[line] > 1 or raw_new[line] > 1 for line in o[i1:i2])
            if current is not None and weak:
                bridge = (i1, i2, j1, j2)
            else:
                if current is not None:
                    groups.append(current)
                current, bridge = None, None
            continue
        if current is None:
            current = ([], [])
        elif bridge is not None:
            current[0].extend(range(bridge[0], bridge[1]))
            current[1].extend(range(bridge[2], bridge[3]))
        bridge = None
        current[0].extend(range(i1, i2))
        current[1].extend(range(j1, j2))
    if current is not None:
        groups.append(current)

    def place_group(g: int, skip_old: set[int], skip_new: set[int]):
        old_lines, new_lines = groups[g]
        old_tokens = [(w, i) for i in old_lines if i not in skip_old for w in _key(o[i])]
        new_tokens = [(w, is_struck, j) for j in new_lines if j not in skip_new for w, is_struck in marked[j]]
        if notes is not None and too_large(old_tokens, new_tokens) and old_tokens:
            note = (f"taban satır ~{old_tokens[0][1] + 1}: öbek çok büyük ({len(old_tokens)} × {len(new_tokens)} "
                    "kelime) — sıra aranmadı, yalnız kelimelerin öbekte bulunmasına bakıldı")
            if note not in notes:
                notes.append(note)
        unplaced, used = place_words(old_tokens, new_tokens)
        return old_tokens, [old_tokens[i] for i in unplaced], {new_tokens[k][2] for k in used}

    # 1) Her öbek kendi içinde yerleştirilir.
    placed = [place_group(g, set(), set()) for g in range(len(groups))]

    # 2) Yer bulamayan satırlar için taşınmış karşılık aranır: aynı kelime dizisi, hiçbir kelimesi kullanılmamış.
    touched = {j for _, _, used_lines in placed for j in used_lines}
    group_of_new = {j: g for g, (_, new_lines) in enumerate(groups) for j in new_lines}
    homes: dict[tuple[str, ...], list[int]] = {}
    for j in sorted(group_of_new):
        if marked[j] and j not in touched:
            homes.setdefault(_key(n[j]), []).append(j)
    exempt: set[int] = set()
    consumed: set[int] = set()
    again: set[int] = set()
    for g, (_, unplaced, _) in enumerate(placed):
        for i in sorted({line for _, line in unplaced}):
            free = [j for j in homes.get(_key(o[i]), ()) if j not in consumed]
            if free:
                consumed.add(free[0])
                exempt.add(i)
                again.update((g, group_of_new[free[0]]))

    # 3) Muaf satırlar ve tüketilen karşılıklar çıkarılıp etkilenen öbekler yeniden yerleştirilir.
    for g in again:
        placed[g] = place_group(g, exempt, consumed)

    out: list[tuple[int, Counter, str]] = []
    for _, unplaced, _ in placed:
        if unplaced:
            first = min(line for _, line in unplaced)
            out.append((first + 1, Counter(w for w, _ in unplaced), o[first].strip()[:90]))
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
        notes: list[str] = []
        moved = displaced_lines(old, new, notes) if new_path is not None else []
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
        for note in notes:
            print(f"  NOT {label}: {note}")
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
        ("silinen kelime hemen üstteki not satırında geçiyor (satırın kalanı yerinde üstü çizili)",
         "| KS-8 | değer | Abkarada |\ndipnot\n",
         "> not — Abkarada kararı\n| ~~KS-8~~ | ~~değer~~ KAPANDI | — |\ndipnot\n", 0, 1),
        ("aynı düzen, hücre üstü çizilerek değişti (not satırı hemen üstte)",
         "| KS-8 | değer | Abkarada |\ndipnot\n",
         "> not — Abkarada kararı\n| ~~KS-8~~ | ~~değer~~ KAPANDI | ~~Abkarada~~ — |\ndipnot\n", 0, 0),
        ("SINIR (yakalanmaz): silinen kelime aynı öbekte, sırayı bozmayan yerde (hemen altta) yeniden geçiyor",
         "| KS-8 | değer | Abkarada |\ndipnot\n",
         "| ~~KS-8~~ | ~~değer~~ KAPANDI | — |\n> not — Abkarada kararı\ndipnot\n", 0, 0),
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
        # 2026-10-06 — düzeltme commit'inin incelemesi: komşu satırın üstü çizili kelimesi örtü olmaz
        ("komşu başlıkta üstü çizilip yeniden yazılan kelime, bu başlıktan silineni örtmez",
         "## 16. RA Identity / RA Noise CCR\n\n### 16.1 RA Identity CCR\n\nmetin\n",
         "## 16. Identity / RA Noise CCR\n\n### 16.1 ~~RA Identity~~ RA Kimlik CCR\n\nmetin\n", 0, 1),
        ("komşu paragrafta `~~W~~ W …` yazılmış; bu paragraftan W üstü çizilmeden çıkarılmış",
         "Karar sahibi: Abkarada.\n\nOnay: Abkarada.\n\nson\n",
         "Karar sahibi: Hasan.\n\nOnay: ~~Abkarada~~ Abkarada ve Hasan.\n\nson\n", 0, 1),
        ("komşu tablo satırında `~~W~~ W …`; bu satırın hücresi üstü çizilmeden değişmiş",
         "| KS-8 | connect kalıcı niyet | Abkarada |\n| KS-9 | idle teardown yok | Abkarada |\nson\n",
         "| KS-8 | connect kalıcı niyet | — |\n| KS-9 | idle teardown yok | ~~Abkarada~~ Abkarada + Hasan |\nson\n", 0, 1),
        ("aynı tablo, iki hücre de üstü çizilerek değişti",
         "| KS-8 | connect kalıcı niyet | Abkarada |\n| KS-9 | idle teardown yok | Abkarada |\nson\n",
         "| KS-8 | connect kalıcı niyet | ~~Abkarada~~ — |\n| KS-9 | idle teardown yok | ~~Abkarada~~ Abkarada + Hasan |\nson\n", 0, 0),
        ("bütünüyle üstü çizilen satır başka öbeğe taşınmış; oradaki silmeyi örtmez",
         "başlık\nfoo bar\nara metin tekil\nfoo baz qux\nson\n",
         "> YAMA: foo\nbaşlık\nara metin tekil\n~~foo bar~~\nbaz qux\nson\n", 0, 1),
        ("yinelenen satır yerinde duruyor; komşusunun üstü çizili karşılığını tüketmez",
         "Durum: AÇIK (Abkarada bekliyor)\nDurum: AÇIK\nNot: eski\nx\nDurum: AÇIK\n",
         "~~Durum: AÇIK (Abkarada bekliyor)~~\nDurum: KAPANDI (Abkarada bekliyor)\nDurum: AÇIK\nNot: ~~eski~~ yeni\nx\nDurum: AÇIK\n", 0, 0),
        ("SINIR (yanlış kırmızı): eski satırın bir PARÇASI ayrı bir satırda üstü çizilmiş",
         "Durum: AÇIK (Abkarada bekliyor)\nson\n",
         "~~AÇIK~~\nDurum: KAPANDI (Abkarada bekliyor)\nson\n", 0, 1),
        ("alıntıda yalnız `>` taşıyan satır paragrafı ayırır; tek kalmış ~~ sonraki paragrafa taşmaz",
         "> bir iki\n>\n> alfa beta gama\nson\n", "> bir ~~ iki\n>\n> alfa gama beta\nson\n", 0, 1),
        ("numaralı listeye madde eklendi, sonrakilerin numarası kaydı",
         "1. alfa\n2. beta\n3. gama\nson\n", "1. alfa\n2. yeni madde\n3. beta\n4. gama\nson\n", 0, 0),
        ("numaralı liste maddesinin metni üstü çizilmeden değişti (numara muafiyeti içeriği kapsamaz)",
         "1. alfa\n2. beta delta\n3. gama\nson\n", "> not: delta\n\n1. alfa\n2. beta\n3. gama\nson\n", 0, 1),
        ("yeniden sarma + üstü çizme: üstü çizili kelime yeni satırın başına düştü",
         "aaa bbb ccc\nddd eee\nson\n", "aaa bbb\n~~ccc~~ XXX ddd eee\nson\n", 0, 0),
        ("kod bloğunda eski satır yorum içinde üstü çizilerek korundu",
         "```rust\nfn cancel(&mut self, ticket: ConnectTicket);\n```\nson\n",
         "```rust\n// ~~fn cancel(&mut self, ticket: ConnectTicket);~~\nfn cancel(&mut self, ticket: ConnectTicket) -> bool;\n```\nson\n", 0, 0),
    ]
    for name, old, new, want_lost, want_displaced in placed:
        got_lost = sum(lost_words(old, new).values())
        got_displaced = len(displaced_lines(old, new))
        ok = (got_lost, got_displaced) == (want_lost, want_displaced)
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  yerinde: {name}: beklenen (kayıp {want_lost}, öbek {want_displaced}), "
              f"gelen ({got_lost}, {got_displaced})")
    reported = displaced_lines(
        "replay identity = (session, direction, epoch, counter).\n\nState per (session, direction, epoch) tutulur.\n\nson\n",
        "replay identity = (session, direction, counter).\n\nState ~~per (session, direction, epoch)~~ per (session, direction) "
        "tutulur; epoch yalnız anahtarı seçer.\n\nson\n")
    ok = len(reported) == 1 and reported[0][0] == 1 and dict(reported[0][1]) == {"epoch": 1}
    bad += not ok
    print(f"  {'PASS' if ok else 'FAIL'}  yerinde: raporlanan kelime çıkarılan kelimedir (epoch): gelen "
          f"{[(line, dict(words)) for line, words, _ in reported]}")
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
    except UnicodeDecodeError as e:
        print(f"ORTAM HATASI: kapsamdaki bir dosya UTF-8 değil ({e})", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
