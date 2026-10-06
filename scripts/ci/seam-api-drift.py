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
  G. 'static' durumu YALNIZ statik satirlar icindir ve kaniti adlandirilmis olmalidir
     (`proof: layer_check` + bos olmayan `proof_rules`); davranis satiri 'static' beyan EDILEMEZ,
     statik satir da 'mock_only'/'implemented' beyan EDILEMEZ (kosumu yoktur).
     Bir satirin statik olup olmadigi beyandaki `kind` alanindan DEGIL spec §6 metninden okunur
     ("Kanitladigi" hucresinde "statik" yazan satirlar); `kind` o metinle ayni olmak zorundadir.
     `proof_rules` girdileri scripts/ci/layer_check.py'de gecen kural kimlikleri olmalidir. Bu,
     "boyle bir kural VAR" denetimidir; kuralin o satiri gercekten kanitladigini OLCMEZ (inceleme isi).
  H. BICIM [2026-10-05; 2026-10-06'da sessiz atlamalar kapatildi]: §1'de bildirilen her tipin TURU
     (enum / alanli struct / tuple struct / birim struct), her enum'un VARIANT kumesi ve her variant'in
     bicimi (birim / tuple eleman sayisi / struct alan adlari), alanli her struct'in ALAN ADLARI ve her
     trait metodunun IMZASI (niteleyiciler `async`/`unsafe`/`const`/`extern` + `fn ad` ile `;` ya da
     govde arasindaki her sey: jenerikler, parametreler, donus tipi, where; bosluklar atilarak) kaynaktakiyle
     aynidir. Ayrica: tipin BASLIGI (jenerik parametreler / where), metodun zorunlu mu varsayilan govdeli
     mi oldugu, trait'in iliskili ogeleri (`type X;`, `const N: T;`) ve kaynakta spec'te olmayan trait.
     Bildirilmis ama okunamayan enum / struct / imza KIRMIZIdir (atlanmaz).
     KARSILASTIRILMAYANLAR: variant ve struct ALAN TIPLERI (spec `Bytes` der, kaynak `Vec<u8>`), tuple
     struct'larin ic tipi, trait BASLIGI (ust-trait sinirlari — bugun kaynakta S3 trait'leri
     `: Send + 'static` tasir, spec §1 bloklarinda sinir yazmaz), gorunurluk, oznitelikler
     (`#[non_exhaustive]`, derive'lar), varsayilan metot govdelerinin icerigi.
     BILINEN KIRILGANLIK (A ve C, H'den once de vardi; H degistirmez): ikisi de ham metne bakar —
     S1'de trait basligina sinir eklenmesi (`pub trait KamaTransport: Send {`) A'da "trait yok" diye,
     variant satirinin sonundaki yorum (`NotConnected, // …`) C'de "yalniz spec'te" diye kirmizi verir
     (ikisi de olculdu; kirmizi dogru, neden yanlis).
     [2026-10-06] yorum/oznitelik ayiklama tek gecisli tarayicidir: dizge icindeki `//`, `/*`, `]`
     yorum/oznitelik sinirini KAPATMAZ (olculdu: `#[deprecated(note = "… https://…")]` yanlis kirmizi
     veriyordu — eski iki gecisli hal dizgedeki `//`'i yorum sanip dosya sonuna kadar yutuyordu).
     [2026-10-06b] Spec §1 bloklarinda ya da kaynakta dosya sonuna kadar ACIK kalan blok yorum / dizge /
     ham dizge / oznitelik KIRMIZIdir ("kapanmamis … — bicim denetimi kesildi"): acik yapidan sonraki
     bildirimler ayiklamada yutulur, karsilastirma sessizce daralirdi (olculdu: spec fence'ine kapanmamis
     `/*` + kaynakta variant ayrismasi → cikis 0). Sessiz atlama iddiasi bu kapiya dayanir.
  Ek olarak beyan dosyasinin kendi tutarliligi (tekrarsiz kimlik, gecerli durum, ratchet tabani).

--seam s3 (KM-SEAM-TC-001) — DENETLENEN ESITLIKLER:
  A. §1'deki HER `pub trait` (PacketProtection, SessionControl) kaynakta var ve metot kumesi ayni
  B. §1'de bildirilen tipler            ⊆ kaynaktaki tipler
  D. §6 tablosundaki her H-* satirinin noise-seam-coverage.json'da en az bir satiri var
     (`H-X` ya da alt satir `H-X/…`) ve beyandaki her satir §6'daki bir satira ait
  H. BICIM: --seam s1'deki H ile ayni (enum variant'lari, struct alan adlari, metot imzalari)

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
LAYER_CHECK = ROOT / "scripts/ci/layer_check.py"

VALID_STATUS = {"implemented", "mock_only", "static", "not_implemented"}
VALID_KIND = {"behavioral", "static"}
# Statik satirin kabul edilen kanit kapilari (runner bunlari kosturur).
STATIC_PROOFS = {"layer_check"}

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
    methods = set(re.findall(r"\bfn\s+(\w+)\s*[<(]", trait_body))
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


def spec_static_rows(spec_text: str) -> set[str]:
    """§6 tablosunda, kimlik hucresinden SONRAKI metninde `statik` kelimesi gecen satirlar: kosum degil
    kapi-seviyesi denetim. (Kelime eslesmesidir: bir davranis satirinin metnine `statik` yazilirsa o satir
    da statik sayilir ve beyandaki `kind` ile ayrisma kirmizi olur — gorunur, sessiz degil.)"""
    body = section(spec_text, "6.")
    out = set()
    for line in body.splitlines():
        m = re.match(r"\|\s*(H-[A-Z0-9-]+)\s*\|(.*)", line)
        if m and re.search(r"\bstatik\b", m.group(2), re.I):
            out.add(m.group(1))
    return out


def layer_rule_ids() -> set[str]:
    """layer_check.py'nin kaynaginda gecen kural kimlikleri (L1.. / C1..)."""
    if not LAYER_CHECK.is_file():
        die(f"dosya yok: {LAYER_CHECK}")
    return set(re.findall(r"\b([LC][1-9])\b", LAYER_CHECK.read_text(encoding="utf-8")))


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
    methods = set(re.findall(r"^\s*fn\s+(\w+)\s*[<(]", trait_body, re.M))
    types = set(re.findall(r"^pub\s+(?:struct|enum)\s+(\w+)", mirror_text, re.M))
    err_body = block_of(mirror_text, r"pub\s+enum\s+TransportError\s*\{") or ""
    variants = set(re.findall(r"^\s{4}([A-Z]\w*)\s*,\s*$", err_body, re.M))
    return trait_name, methods, types, variants


# ── bicim karsilastirmasi (variant / alan / imza) ────────────────────────────────────────────


def _is_ident_char(ch: str) -> bool:
    return ch.isalnum() or ch == "_"


def _blank(seg: str) -> str:
    """Atilan metnin yerine ayni uzunlukta bosluk; satir sonlari korunur (satir/sutun yapisi degismez)."""
    return "".join("\n" if ch == "\n" else " " for ch in seg)


def strip_code_comments(code: str) -> str:
    """Yorumlari (`// …` satir sonuna; `/* … */` ic ice) ve oznitelikleri (`#[…]` / `#![…]`) bosluga
    cevirir; dizge ve karakter sabitlerini OLDUGU GIBI birakir. Spec bloklarinda ustu cizili eski metin
    yorum icinde durur (`/* ~~ConnectFailed(...)~~ */`, `// ~~...~~`); o yuzden yorumlar bicime GIRMEZ.

    TEK gecisli SOZCUK tarayicisi: metin soldan saga bir kez okunur ve hangi yapi once basliyorsa o
    kazanir — dizge icindeki `//`, `/*`, `]` yorum/oznitelik sonu DEGILDIR; yorum icindeki `"`, `#[`,
    `]` dizge/oznitelik DEGILDIR. [2026-10-06] Onceki hal IKI gecisti (once yorum regex'i, sonra
    oznitelik tarayicisi): `#[deprecated(note = "bkz. https://example.org/k4")]` icindeki `//` yorum
    sanilip dizgenin kapanisi (`")]`) siliniyor, ardindan oznitelik tarayicisi kapanmamis dizgeyle
    dosya sonuna kadar HER SEYI (handles.rs dahil) yutuyordu → 10 sahte kirmizi.

    Taninan sabitler: `"…"` (kacis `\\`), ham `r"…"` / `r#"…"#` (b/c onekli dahil; kapanis `"` + ayni
    sayida `#`), karakter `'x'` / `'\\n'` / `'\\u{…}'` / `'"'` — yasam suresi (`'a`, `'static`) sabit
    DEGILDIR (ucuncu karakter `'` degilse). Oznitelik birkac satira yayilabilir ve dizgesinde `]`
    tasiyabilir (`#[doc = "bkz. [K4]"]`): koseli ayrac derinligi dizge/yorum DISINDA sayilir.
    Kapanmayan dizge / yorum / oznitelik dosya sonuna kadar surer; bu fonksiyon onu BILDIRMEZ
    (ayiklanmis metni dondurur). Bildiren hali `strip_code_comments_eof` — check_shapes onu kullanir
    ve acik kalan yapiyi KIRMIZI yapar; aksi halde yapidan sonraki bildirimler karsilastirmadan sessizce
    duser ve kaynaktaki gercek ayrisma YESIL gecerdi (olculdu, 2026-10-06b: spec §1 fence'ine kapanmamis
    `/*` + ayna variant mutasyonu → cikis 0)."""
    return strip_code_comments_eof(code)[0]


def strip_code_comments_eof(code: str) -> tuple[str, str | None]:
    """`strip_code_comments`'in bildiren hali: (ayiklanmis metin, dosya sonunda acik kalan yapi | None).
    Acik kalan yapi: `blok yorum` / `ham dizge` / `dizge` / `oznitelik` + basladigi satir — verilen
    metin icinde (check_shapes birlesik §1 fence metnini / birlesik ayna metnini verir; dosya satiri
    DEGILDIR). Satir sonuna kadar suren `//` yorumu acik yapi DEGILDIR."""
    out: list[str] = []
    i, n = 0, len(code)
    attr_depth = 0  # > 0: `#[` … `]` icindeyiz; sabitler dahil her sey bosluga gider
    attr_start = 0  # son acilan `#[`'in konumu (acik kalirsa satiri raporlanir)
    open_at_eof: str | None = None

    def emit(seg: str) -> None:
        out.append(_blank(seg) if attr_depth else seg)

    def line_of(pos: int) -> int:
        return code.count("\n", 0, pos) + 1

    while i < n:
        ch = code[i]
        nxt = code[i + 1] if i + 1 < n else ""
        # 1. yorumlar — her modda (oznitelik icindeki yorumun `]`'i oznitelik sonu degildir)
        if ch == "/" and nxt == "/":
            j = code.find("\n", i)
            j = n if j < 0 else j
            out.append(_blank(code[i:j]))
            i = j
            continue
        if ch == "/" and nxt == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if code.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif code.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            if depth:
                open_at_eof = f"blok yorum (birlesik metinde satir {line_of(i)})"
            out.append(_blank(code[i:j]))
            i = j
            continue
        # 2. ham dizge r"…" / r#"…"# (`br`/`cr` onekli dahil); `r` sozcuk basinda olmali (`bar"` degil)
        if ch == "r" and (
            i == 0
            or not _is_ident_char(code[i - 1])
            or (code[i - 1] in "bc" and (i < 2 or not _is_ident_char(code[i - 2])))
        ):
            j = i + 1
            while j < n and code[j] == "#":
                j += 1
            if j < n and code[j] == '"':
                close = '"' + "#" * (j - (i + 1))
                k = code.find(close, j + 1)
                if k < 0:
                    open_at_eof = f"ham dizge (birlesik metinde satir {line_of(i)})"
                k = n if k < 0 else k + len(close)
                emit(code[i:k])
                i = k
                continue
        # 3. dizge "…" (kacis `\`); icindeki `//`, `/*`, `]` yapisal DEGILDIR
        if ch == '"':
            j = i + 1
            while j < n and code[j] != '"':
                j += 2 if code[j] == "\\" else 1
            if j >= n:
                open_at_eof = f"dizge (birlesik metinde satir {line_of(i)})"
            j = min(j + 1, n)
            emit(code[i:j])
            i = j
            continue
        # 4. karakter sabiti 'x' / '\n' / '\x41' / '\u{…}' / '"' — yasam suresi ('a, 'static) DEGIL
        if ch == "'":
            end = None
            if nxt == "\\":
                k = code.find("'", i + 3, i + 14)  # kacisli govde ≤ 10 karakter (`\u{10FFFF}`)
                end = k + 1 if k >= 0 else None
            elif i + 2 < n and nxt not in "\n'" and code[i + 2] == "'":
                end = i + 3
            if end is not None:
                emit(code[i:end])
                i = end
                continue
        # 5. oznitelik baslangici `#[` / `#![` — derinlik sayimi; icindeki sabit/yorum 1–4'te islenir
        if attr_depth == 0 and ch == "#" and (nxt == "[" or code.startswith("![", i + 1)):
            j = code.index("[", i) + 1
            out.append(_blank(code[i:j]))
            attr_depth, attr_start = 1, i
            i = j
            continue
        if attr_depth:
            if ch == "[":
                attr_depth += 1
            elif ch == "]":
                attr_depth -= 1
            out.append("\n" if ch == "\n" else " ")
            i += 1
            continue
        out.append(ch)
        i += 1
    if attr_depth:
        open_at_eof = f"oznitelik (birlesik metinde satir {line_of(attr_start)})"
    return "".join(out), open_at_eof


def split_top(s: str) -> list[str]:
    """En dis duzeydeki virgullerden boler (parantez / suslu / koseli / acili ayrac ici bolunmez)."""
    out, depth, cur = [], 0, []
    for ch in s:
        if ch in "({[<":
            depth += 1
        elif ch in ")}]>":
            depth -= 1
        if ch == "," and depth == 0:
            out.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    out.append("".join(cur))
    return [x.strip() for x in out if x.strip()]


def brace_body(code: str, open_at: int) -> str | None:
    """`open_at` bir `{` karakterinin indeksi; esi olan `}`'a kadarki ic metin (yoksa None)."""
    depth = 0
    for i in range(open_at, len(code)):
        if code[i] == "{":
            depth += 1
        elif code[i] == "}":
            depth -= 1
            if depth == 0:
                return code[open_at + 1:i]
    return None


def declared_kinds(code: str) -> dict[str, tuple[str, str]]:
    """{tip adi: (tur, baslik)} — tur: "enum" | "struct{}" (alanli) | "struct()" (tuple) | "struct;" (birim);
    baslik: adla acilis ayraci arasindaki bosluksuz metin (jenerik parametreler, where)."""
    kinds: dict[str, tuple[str, str]] = {}
    for m in re.finditer(r"\bpub\s+(enum|struct)\s+(\w+)\b([^{;(]*)([{;(])", code):
        kind = "enum" if m.group(1) == "enum" else "struct" + {"{": "{}", "(": "()", ";": ";"}[m.group(4)]
        kinds[m.group(2)] = (kind, re.sub(r"\s+", "", m.group(3)))
    return kinds


def parse_enums(code: str) -> dict[str, dict[str, tuple]]:
    """{enum adi: {variant adi: bicim}} — bicim: ("unit",) | ("tuple", n) | ("struct", alan adlari)."""
    enums: dict[str, dict[str, tuple]] = {}
    for m in re.finditer(r"\bpub\s+enum\s+(\w+)\b[^{;(]*\{", code):
        body = brace_body(code, m.end() - 1)
        if body is None:
            continue
        variants: dict[str, tuple] = {}
        for item in split_top(body):
            vm = re.match(r"([A-Za-z_]\w*)\s*(.*)$", item, re.S)
            if not vm:
                variants[item] = ("?",)
                continue
            name, rest = vm.group(1), vm.group(2).strip()
            if rest.startswith("(") and ")" in rest:
                variants[name] = ("tuple", len(split_top(rest[1:rest.rindex(")")])))
            elif rest.startswith("{") and "}" in rest:
                fields = [f.split(":")[0].strip() for f in split_top(rest[1:rest.rindex("}")])]
                variants[name] = ("struct", tuple(sorted(fields)))
            elif not rest:
                variants[name] = ("unit",)
            else:
                variants[name] = ("?", rest)
        enums[m.group(1)] = variants
    return enums


def parse_structs(code: str) -> dict[str, tuple]:
    """{struct adi: alan adlari} — yalniz suslu ayracli (alanli) struct'lar."""
    structs: dict[str, tuple] = {}
    for m in re.finditer(r"\bpub\s+struct\s+(\w+)\b[^{;(]*\{", code):
        body = brace_body(code, m.end() - 1)
        if body is None:
            continue
        fields = []
        for item in split_top(body):
            fm = re.match(r"(?:pub(?:\([^)]*\))?\s+)?([A-Za-z_]\w*)\s*:", item)
            if fm:
                fields.append(fm.group(1))
        structs[m.group(1)] = tuple(sorted(fields))
    return structs


UNREADABLE = "?"
ITEMS = "<ogeler>"       # parse_signatures: trait'in metot DISI ogeleri (iliskili tip / sabit) bu anahtarda


def parse_signatures(code: str) -> dict[str, dict[str, str]]:
    """{trait adi: {metot adi: bosluksuz imza}} — imza = niteleyiciler (`async`/`unsafe`/`const`/`extern`)
    + `fn ad` ile `;` ya da govdenin `{`'i arasindaki her sey (jenerikler, parametreler, donus tipi, where)
    + govdesi varsa `{…}` isareti (zorunlu metot ↔ varsayilan govdeli metot ayrimi; govdenin ICERIGI
    karsilastirilmaz). Sonu bulunamayan imza UNREADABLE doner (cagiran KIRMIZI yapar).
    ITEMS anahtari: metot disi ogeler (`type X;`, `const N: T;`) sirali ve bosluksuz."""
    traits: dict[str, dict[str, str]] = {}
    fn_re = re.compile(r"((?:\b(?:async|unsafe|const|extern(?:\s*\"[^\"]*\")?)\s+)*)\bfn\s+(\w+)")
    for m in re.finditer(r"\bpub\s+trait\s+(\w+)[^{;]*\{", code):
        body = brace_body(code, m.end() - 1)
        if body is None:
            traits[m.group(1)] = {UNREADABLE: UNREADABLE}
            continue
        sigs: dict[str, str] = {}
        rest: list[str] = []
        pos = 0
        while True:
            fm = fn_re.search(body, pos)
            if fm is None:
                rest.append(body[pos:])
                break
            rest.append(body[pos:fm.start()])
            depth, end = 0, None
            for i in range(fm.end(), len(body)):
                ch = body[i]
                if ch in "([":
                    depth += 1
                elif ch in ")]":
                    depth -= 1
                elif ch in ";{" and depth == 0:
                    end = i
                    break
            if end is None:
                sigs[fm.group(2)] = UNREADABLE
                break
            sig = re.sub(r"\s+", "", fm.group(1) + "|" + body[fm.end():end]).replace(",)", ")")
            sig = sig[1:] if sig.startswith("|") else sig
            pos = end + 1
            if body[end] == "{":
                inner = brace_body(body, end)
                if inner is None:
                    sigs[fm.group(2)] = UNREADABLE
                    break
                pos = end + len(inner) + 2
                sig += "{…}"
            sigs[fm.group(2)] = sig
        items = sorted(re.sub(r"\s+", "", item) for item in re.findall(r"\b(?:type|const)\s+\w+[^;]*;", "".join(rest)))
        if items:
            sigs[ITEMS] = " ".join(items)
        traits[m.group(1)] = sigs
    return traits


def check_shapes(spec_text: str, mirror_text: str, say) -> None:
    """H — §1 bloklarindaki enum / struct / metot BICIMLERI kaynaktakiyle ayni mi."""
    spec_code, spec_open = strip_code_comments_eof("\n".join(all_fences(section(spec_text, "1."))))
    src_code, src_open = strip_code_comments_eof(mirror_text)
    # [2026-10-06b] Dosya sonunda acik kalan yorum / dizge / oznitelik: ondan sonraki her bildirim
    # ayiklamada yutulur ve karsilastirma sessizce daralirdi (spec'te kapanmamis `/*` + kaynakta gercek
    # variant ayrismasi → YESIL; olculdu). Iki tarafta da KIRMIZI: bicim denetimi o noktada kesilmistir.
    for side, opened in (("spec", spec_open), ("kaynak", src_open)):
        if opened:
            bad(f"§1 {side} tarafinda kapanmamis {opened} dosya sonuna kadar surdu — bicim denetimi kesildi")
    s_kinds, m_kinds = declared_kinds(spec_code), declared_kinds(src_code)
    s_enums, m_enums = parse_enums(spec_code), parse_enums(src_code)
    s_structs, m_structs = parse_structs(spec_code), parse_structs(src_code)
    s_sigs, m_sigs = parse_signatures(spec_code), parse_signatures(src_code)
    say(f"  bicim : {len(s_enums)} enum · {len(s_structs)} alanli struct · "
        f"{sum(len(v) - (ITEMS in v) for v in s_sigs.values())} metot imzasi karsilastirildi")
    if not s_enums or not s_sigs:
        bad("§1 bloklarindan enum ya da metot imzasi okunamadi — bicim denetimi vakumda")
    # Tur: tipin hic olmamasi B'de raporlanir; burada ayni adin BASKA turde bildirilmesi yakalanir
    # (alanli struct → tuple struct, enum → struct, …). Ayni turdeyse ayrintisi asagida karsilastirilir.
    same_kind: set[str] = set()
    for name, (kind, header) in sorted(s_kinds.items()):
        if name not in m_kinds:
            # Tipin hic olmamasi B'de raporlanir. B ham metne bakar: bildirim yalniz bir YORUMUN icinde
            # duruyorsa (sutun 0'da) B onu "var" sayar — burada yakalanir.
            if re.search(rf"^pub\s+(?:struct|enum)\s+{re.escape(name)}\b", mirror_text, re.M):
                bad(f"§1 {name} kaynakta yalniz yorum icinde bildirilmis (yorum disinda struct/enum olarak yok)")
            continue
        other, other_header = m_kinds[name]
        if other != kind:
            bad(f"§1 {name} turu ayrisiyor: spec `{kind}`, kaynak `{other}`")
            continue
        same_kind.add(name)
        if other_header != header:
            bad(f"§1 {name} basligi (jenerik parametreler / where) ayrisiyor: spec `{header}`, kaynak `{other_header}`")
    for name in sorted(same_kind):
        parsed = {"enum": (s_enums, m_enums), "struct{}": (s_structs, m_structs)}.get(s_kinds[name][0])
        if parsed is None:
            continue
        for side, table in zip(("spec", "kaynak"), parsed):
            if name not in table:
                bad(f"§1 {name} ({s_kinds[name][0]}) {side} tarafinda okunamadi — bicimi karsilastirilamadi")
    for name, s_var in sorted(s_enums.items()):
        m_var = m_enums.get(name)
        if m_var is None:
            continue  # yok / baska turde / okunamadi — yukarida ya da B'de raporlandi
        only_spec, only_src = sorted(set(s_var) - set(m_var)), sorted(set(m_var) - set(s_var))
        differ = sorted(v for v in set(s_var) & set(m_var) if s_var[v] != m_var[v])
        if only_spec or only_src or differ:
            detail = "; ".join(f"{v}: spec {s_var[v]} / kaynak {m_var[v]}" for v in differ)
            bad(f"§1 enum {name} bicimi ayrisiyor: yalniz spec'te {only_spec}, yalniz kaynakta {only_src}"
                + (f", bicimi farkli [{detail}]" if differ else ""))
    for name, s_fields in sorted(s_structs.items()):
        m_fields = m_structs.get(name)
        if m_fields is not None and m_fields != s_fields:
            bad(f"§1 struct {name} alanlari ayrisiyor: spec {list(s_fields)}, kaynak {list(m_fields)}")
    extra_traits = sorted(set(m_sigs) - set(s_sigs))
    if extra_traits:
        bad(f"kaynakta spec §1'de olmayan trait(ler) var: {extra_traits}")
    for trait, sigs in sorted(s_sigs.items()):
        m_trait = m_sigs.get(trait)
        if m_trait is None:
            continue  # trait'in hic olmamasi A'da raporlanir
        for method in sorted(set(sigs) | set(m_trait)):
            sig, other = sigs.get(method), m_trait.get(method)
            if method == ITEMS:
                if sig != other:
                    bad(f"§1 {trait} iliskili ogeleri (type / const) ayrisiyor: spec `{sig or ''}`, kaynak `{other or ''}`")
            elif UNREADABLE in (sig, other):
                bad(f"§1 {trait}::{method} imzasi okunamadi (spec `{sig}`, kaynak `{other}`)")
            elif sig is None or other is None:
                bad(f"§1 {trait}::{method} imzasi karsilastirilamadi: "
                    f"{'spec' if sig is None else 'kaynak'} tarafinda boyle bir metot yok")
            elif other != sig:
                bad(f"§1 {trait}::{method} imzasi ayrisiyor: spec `{sig}`, kaynak `{other}`")


# ── S3 (KM-SEAM-TC-001) ──────────────────────────────────────────────────────────────────────


def spec_traits(spec_text: str) -> tuple[dict[str, set[str]], set[str]]:
    """§1'deki TUM fenced bloklardan {trait adi: metot kumesi} ve bildirilen tipler."""
    traits: dict[str, set[str]] = {}
    types: set[str] = set()
    for block in all_fences(section(spec_text, "1.")):
        for tm in re.finditer(r"pub\s+trait\s+(\w+)[^{\n]*\{(.*?)\n\}", block, re.S):
            traits[tm.group(1)] = set(re.findall(r"\bfn\s+(\w+)\s*[<(]", tm.group(2)))
        types |= set(re.findall(r"pub\s+(?:struct|enum)\s+(\w+)", block))
    if not traits:
        die("spec §1 bloklarinda hic `pub trait … { … }` bulunamadi")
    return traits, types


def mirror_traits(mirror_text: str) -> tuple[dict[str, set[str]], set[str]]:
    traits: dict[str, set[str]] = {}
    for tm in re.finditer(r"^pub\s+trait\s+(\w+)[^{\n]*\{", mirror_text, re.M):
        body = block_of(mirror_text[tm.start():], r"pub\s+trait\s+\w+[^{\n]*\{") or ""
        traits[tm.group(1)] = set(re.findall(r"^\s*fn\s+(\w+)\s*[<(]", body, re.M))
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

    # H — bicim
    check_shapes(spec_text, mirror_text, say)

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

    # H — bicim
    check_shapes(spec_text, mirror_text, say)

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
    implemented, mock_only, static = 0, 0, 0
    spec_static = spec_static_rows(spec_text)
    known_rules = layer_rule_ids()
    for r in rows:
        rid, status, kind = r.get("id"), r.get("status"), r.get("kind")
        if rid in seen:
            bad(f"seam-coverage.json: '{rid}' iki kez beyan edildi")
        seen.add(rid)
        if status not in VALID_STATUS:
            bad(f"seam-coverage.json: '{rid}' gecersiz durum '{status}' (gecerli: {sorted(VALID_STATUS)})")
        if kind not in VALID_KIND:
            bad(f"seam-coverage.json: '{rid}' gecersiz tur '{kind}' (gecerli: {sorted(VALID_KIND)})")
        if status == "implemented":
            implemented += 1
        elif status == "mock_only":
            mock_only += 1
        elif status == "static":
            static += 1

        # G — tur SPEC'ten okunur; beyandaki `kind` onunla ayni olmali
        want_kind = "static" if rid in spec_static else "behavioral"
        if kind in VALID_KIND and kind != want_kind:
            bad(f"seam-coverage.json: '{rid}' kind='{kind}' ama spec §6 metnine gore satir {want_kind} "
                f"(statik satirlar: {sorted(spec_static)})")
        if status == "static":
            if rid not in spec_static:
                bad(f"seam-coverage.json: '{rid}' davranis satiri 'static' beyan edilmis — statik durum "
                    f"yalniz spec §6'da statik isaretli satirlar icindir (davranis satiri KOSARAK kanitlanir)")
            if r.get("proof") not in STATIC_PROOFS:
                bad(f"seam-coverage.json: '{rid}' statik ama kaniti adlandirilmamis "
                    f"(proof ∈ {sorted(STATIC_PROOFS)} olmali, gelen {r.get('proof')!r})")
            rules = r.get("proof_rules")
            if not (isinstance(rules, list) and rules and all(isinstance(x, str) and x for x in rules)):
                bad(f"seam-coverage.json: '{rid}' statik ama `proof_rules` bos/eksik — hangi kural kanitliyor?")
            else:
                unknown = sorted(set(rules) - known_rules)
                if unknown:
                    bad(f"seam-coverage.json: '{rid}' proof_rules icinde layer_check.py'de olmayan kural: "
                        f"{unknown} (bilinen: {sorted(known_rules)})")
        elif rid in spec_static and status in ("implemented", "mock_only"):
            bad(f"seam-coverage.json: '{rid}' statik satir '{status}' beyan edilmis — statik satirin kosumu "
                f"yoktur; dogru durum 'static' ya da 'not_implemented'")

    for key, actual in (("implemented_baseline", implemented), ("mock_only_baseline", mock_only),
                        ("static_baseline", static)):
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
