#!/usr/bin/env python3
"""Kama test-vektör doğrulayıcısı — vectors/SCHEMA.json kapısı (T-00.1 / T-00.2).

Üç araç aynı byte beklentisini okur: Python üretici, Rust yükleyici, bu doğrulayıcı.
Bu betik CI kolunun tamamıdır: `vectors/SCHEMA.json`'u okur, verilen dosyaları ona
karşı doğrular ve şemayla ifade edilemeyen üç repo kuralını ayrıca zorlar
(derin hex disiplini, id tekilliği, kontrol karakteri yasağı).

İKİZİ: `crates/kama-vectors` (Rust yükleyici) aynı kuralları zorlar ve aynı SV- kodlarını
üretir. İkisinin vaka listesi `scripts/vectors/testdata/EXPECTATIONS.json`'da TEK yerde
durur; ayrışma `crates/kama-vectors/tests/parity.rs` tarafından yakalanır.

Kullanım:
    python3 scripts/vectors/validate.py vectors/
    python3 scripts/vectors/validate.py a.json b.json --cross-check
    python3 scripts/vectors/validate.py vectors/ --min-files 2   # vakum kapısı

Çıkış kodu: 0 = temiz · 1 = doğrulama hatası · 2 = kullanım/ortam hatası.

TASARIM — FAIL-CLOSED. Buradaki JSON Schema değerlendiricisi tam bir Draft 2020-12
uygulaması DEĞİL, SCHEMA.json'un kullandığı alt kümedir. Tanımadığı bir şema anahtar
sözcüğüyle karşılaşırsa sessizce atlamaz, SV-009 ile durur. Yani şemaya yeni bir
anahtar sözcük eklenirse doğrulayıcı kırmızı olur — kuralın zorlanmadan geçtiği bir
ara durum yoktur. Üçüncü parti bir doğrulayıcıyla çapraz kontrol için: --cross-check
(`jsonschema` kuruluysa çalışır, kurulu değilse atlanır; bu betiğin bağımlılığı yok).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

SUPPORTED_SCHEMA_VERSION = 2

# Doğrulayıcı hata kodları — mesaj metni değişse de bu kodlar kararlıdır.
SV_PARSE = "SV-001"  # JSON olarak ayrıştırılamadı
SV_DUP_KEY = "SV-002"  # aynı nesnede yinelenen JSON anahtarı
SV_VERSION_SHAPE = "SV-003"  # schema_version yok / tamsayı değil
SV_VERSION_UNKNOWN = "SV-004"  # bilinmeyen schema_version — sessiz geçilmez
SV_SCHEMA = "SV-005"  # SCHEMA.json ihlali
SV_HEX = "SV-006"  # hex alanı kuralı (küçük harf, çift uzunluk, 0x yok)
SV_DUP_ID = "SV-007"  # id repo genelinde tekil değil
SV_CTRL = "SV-008"  # dizgede kontrol karakteri
SV_SCHEMA_FILE = "SV-009"  # SCHEMA.json okunamadı / desteklenmeyen anahtar sözcük
SV_VACUOUS = "SV-010"  # beklenenden az dosya doğrulandı (kapı vakumda)
SV_CANONICAL = "SV-011"  # kanonik serileştirme ihlali (üretici disiplini)
SV_SIZE = "SV-012"  # dosya boyutu sınırı (bounds-before-alloc, SR-AUDIT-001)
SV_COUNT = "SV-013"  # case_count ile cases uzunluğu uyuşmuyor
SV_FLAGS = "SV-014"  # flag/notes tutarsızlığı

HEX_RE = re.compile(r"^([0-9a-f]{2})*$")

# SR-AUDIT-001 (bounds-before-alloc): dosya BELLEĞE ALINMADAN ÖNCE boyutu sınanır.
# Ölçüldü: sınırsızken 16 MB'lık bir vektör dosyası python'da 158 MB, rust'ta 446 MB RSS
# tüketiyordu. 8 MiB en büyük makul vektör dosyasının çok üstünde (hex maxLength 262144
# = 128 KB/alan) ve hostile bir girdinin aracı düşürmesini engelliyor.
MAX_FILE_BYTES = 8 * 1024 * 1024
CTRL_RE = re.compile(r"[\x00-\x1f\x7f]")


class SchemaUnsupported(Exception):
    """SCHEMA.json bu değerlendiricinin güvenle yorumlayamayacağı bir şey içeriyor.

    İki durum: tanımadığı bir anahtar sözcük, ya da tanıdığı bir anahtar sözcüğün
    beklenmedik tipte bir değeri. İkisi de fail-closed — sessizce yorumlanmaz.
    """


class Finding:
    __slots__ = ("path", "pointer", "code", "message")

    def __init__(self, path: str, pointer: str, code: str, message: str) -> None:
        self.path = path
        self.pointer = pointer or "/"
        self.code = code
        self.message = message

    def __str__(self) -> str:
        return f"{self.path}{self.pointer}: [{self.code}] {self.message}"

    def annotation(self) -> str:
        """GitHub Actions iş-akışı komutu: bulguyu PR diff'inde DOSYAYA iliştirir.

        Satır numarası yok — JSON işaretçisinden satır türetmek güvenilmez; işaretçi
        mesaja konur, anotasyon dosya düzeyinde kalır (GitHub 1. satıra bağlar).
        """
        text = f"{self.pointer}: {self.message}".replace("%", "%25")
        text = text.replace("\r", "%0D").replace("\n", "%0A")
        return f"::error file={self.path},title={self.code}::{text}"


# ── JSON ayrıştırma (yinelenen anahtar = hata) ────────────────────────────────
# json.load yinelenen anahtarda sessizce SONUNCUYU tutar; iki araç aynı dosyadan
# farklı şey okuyabilir. Fail-closed: yinelenen anahtar hatadır.


class DuplicateKey(Exception):
    def __init__(self, key: str) -> None:
        super().__init__(key)
        self.key = key


def _no_duplicates(pairs):
    seen = {}
    for key, value in pairs:
        if key in seen:
            raise DuplicateKey(key)
        seen[key] = value
    return seen


# ── SCHEMA.json alt-küme değerlendiricisi ─────────────────────────────────────

_ANNOTATION_KEYWORDS = frozenset(
    {"$schema", "$id", "$comment", "title", "description", "examples", "default", "$defs"}
)
_ASSERTION_KEYWORDS = frozenset(
    {
        "type",
        "const",
        "enum",
        "required",
        "properties",
        "additionalProperties",
        "items",
        "minItems",
        "maxItems",
        "minLength",
        "maxLength",
        "pattern",
        "propertyNames",
        "uniqueItems",
        "minimum",
        "oneOf",
        "allOf",
        "anyOf",
        "not",
        "$ref",
    }
)


def _is_bool(value) -> bool:
    return isinstance(value, bool)


def _type_matches(value, want: str) -> bool:
    if want == "object":
        return isinstance(value, dict)
    if want == "array":
        return isinstance(value, list)
    if want == "string":
        return isinstance(value, str)
    if want == "boolean":
        return _is_bool(value)
    if want == "integer":
        return isinstance(value, int) and not _is_bool(value)
    if want == "number":
        return isinstance(value, (int, float)) and not _is_bool(value)
    if want == "null":
        return value is None
    raise SchemaUnsupported(f"bilinmeyen type: {want!r}")


def _json_equal(a, b) -> bool:
    """JSON eşitliği — Python'da True == 1 olduğu için tip-duyarlı."""
    if _is_bool(a) != _is_bool(b):
        return False
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_json_equal(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_json_equal(x, y) for x, y in zip(a, b))
    return a == b


class SubsetValidator:
    def __init__(self, schema: dict) -> None:
        self.root = schema

    def resolve(self, ref: str) -> dict:
        if ref == "#":
            return self.root
        if not ref.startswith("#/"):
            raise SchemaUnsupported(f"yalnız yerel $ref destekleniyor, gelen: {ref!r}")
        node = self.root
        for token in ref[2:].split("/"):
            token = token.replace("~1", "/").replace("~0", "~")
            if not isinstance(node, dict) or token not in node:
                raise SchemaUnsupported(f"$ref çözülemedi: {ref!r}")
            node = node[token]
        return node

    def errors(self, instance, schema, pointer: str) -> list[tuple[str, str]]:
        """(pointer, mesaj) listesi. Boş liste = geçerli."""
        if _is_bool(schema):
            return [] if schema else [(pointer, "bu konumda hiçbir değere izin yok")]
        if not isinstance(schema, dict):
            raise SchemaUnsupported(f"şema düğümü nesne ya da boolean olmalı: {pointer}")

        unknown = set(schema) - _ANNOTATION_KEYWORDS - _ASSERTION_KEYWORDS
        if unknown:
            raise SchemaUnsupported(
                f"desteklenmeyen şema anahtar sözcüğü {sorted(unknown)!r} ({pointer})"
            )

        out: list[tuple[str, str]] = []

        if "$ref" in schema:
            out += self.errors(instance, self.resolve(schema["$ref"]), pointer)

        if "type" in schema:
            want = schema["type"]
            wants = want if isinstance(want, list) else [want]
            if not any(_type_matches(instance, w) for w in wants):
                out.append((pointer, f"tip {'|'.join(wants)} olmalı, gelen {_kind_of(instance)}"))
                return out  # tip yanlışsa kalan iddialar anlamsız

        if "const" in schema and not _json_equal(instance, schema["const"]):
            out.append((pointer, f"değer {schema['const']!r} olmalı, gelen {instance!r}"))

        if "enum" in schema and not any(_json_equal(instance, c) for c in schema["enum"]):
            out.append((pointer, f"değer {schema['enum']!r} kümesinden olmalı, gelen {instance!r}"))

        if isinstance(instance, str):
            if "pattern" in schema and re.search(schema["pattern"], instance) is None:
                out.append((pointer, f"desene uymuyor {schema['pattern']!r}: {instance!r}"))
            if "minLength" in schema and len(instance) < schema["minLength"]:
                out.append((pointer, f"en az {schema['minLength']} karakter olmalı"))
            if "maxLength" in schema and len(instance) > schema["maxLength"]:
                out.append((pointer, f"en çok {schema['maxLength']} karakter olmalı"))

        if isinstance(instance, (int, float)) and not _is_bool(instance):
            if "minimum" in schema and instance < schema["minimum"]:
                out.append((pointer, f"en az {schema['minimum']} olmalı, gelen {instance}"))

        if isinstance(instance, dict):
            if "propertyNames" in schema:
                for key in instance:
                    out += [
                        (f"{pointer}/{key}", f"anahtar adı: {msg}")
                        for _, msg in self.errors(key, schema["propertyNames"], pointer)
                    ]
            for key in schema.get("required", []):
                if key not in instance:
                    out.append((pointer, f"zorunlu alan yok: {key!r}"))
            props = schema.get("properties", {})
            for key, value in instance.items():
                child = f"{pointer}/{key}"
                if key in props:
                    out += self.errors(value, props[key], child)
                else:
                    extra = schema.get("additionalProperties", True)
                    if extra is False:
                        out.append((child, "bilinmeyen alan (şema fail-closed)"))
                    elif isinstance(extra, dict):
                        out += self.errors(value, extra, child)

        if isinstance(instance, list):
            if schema.get("uniqueItems") is True:
                seen: list = []
                for item in instance:
                    if any(_json_equal(item, prev) for prev in seen):
                        out.append((pointer, f"yinelenen eleman: {item!r}"))
                        break
                    seen.append(item)
            if "minItems" in schema and len(instance) < schema["minItems"]:
                out.append((pointer, f"en az {schema['minItems']} eleman olmalı"))
            if "maxItems" in schema and len(instance) > schema["maxItems"]:
                out.append((pointer, f"en çok {schema['maxItems']} eleman olmalı"))
            if "items" in schema:
                for i, item in enumerate(instance):
                    out += self.errors(item, schema["items"], f"{pointer}/{i}")

        for sub in schema.get("allOf", []):
            out += self.errors(instance, sub, pointer)

        if "anyOf" in schema and not any(
            not self.errors(instance, sub, pointer) for sub in schema["anyOf"]
        ):
            out.append((pointer, "anyOf kollarının hiçbiri eşleşmedi"))

        if "oneOf" in schema:
            hit = [i for i, sub in enumerate(schema["oneOf"]) if not self.errors(instance, sub, pointer)]
            if len(hit) != 1:
                hint = schema.get("$comment", "")
                out.append(
                    (pointer, f"oneOf: tam olarak 1 kol eşleşmeli, {len(hit)} eşleşti"
                              + (f" — {hint}" if hint else ""))
                )

        if "not" in schema and not self.errors(instance, schema["not"], pointer):
            out.append((pointer, "'not' kolu eşleşmemeliydi"))

        return out


def _kind_of(value) -> str:
    if value is None:
        return "null"
    if _is_bool(value):
        return "boolean"
    if isinstance(value, dict):
        return "object"
    if isinstance(value, list):
        return "array"
    if isinstance(value, str):
        return "string"
    if isinstance(value, int):
        return "integer"
    return "number"


# ── Şema öz-denetimi ──────────────────────────────────────────────────────────
#
# NEDEN. Doğrulayıcı `SCHEMA.json`'a GÜVENİYOR. Şemadaki bir anahtar sözcüğün DEĞERİ yanlış
# tipteyse (ör. `"maxLength": "64"`) değerlendirici doğrulama ortasında Python istisnasıyla
# düşer: operatör teşhis yerine yığın izi görür ve hata veri hatası sanılır. Sektör pratiği
# nettir — şemanı da doğrula. Aşağıdaki denetim, herhangi bir dosyaya bakılmadan ÖNCE koşar.
#
# Ayrıca iki yapısal kural pinlenir (§3 ve §2 bulguları):
#   * `$id` ile `schema_version` const'u BİRBİRİNE BAĞLI: `$id` `/v<N>/SCHEMA.json` ile
#     bitmeli ve N == const olmalı. Böylece sürüm yükseltilip `$id` unutulamaz — iki farklı
#     şema aynı kimliği taşıyamaz.
#   * `additionalProperties: false` olan bir düğümün combinator kolları (`allOf`/`anyOf`/
#     `oneOf`/`$ref`) `properties` TANIMLAYAMAZ. Draft 2020-12'de `additionalProperties`
#     kardeş kolların tanıttığı alanları GÖRMEZ; böyle bir kol eklendiği gün geçerli belgeler
#     reddedilmeye başlar ve sebebi bariz olmaz. (Doğru cevap `unevaluatedProperties`'tir;
#     onu desteklemek annotation-izleme semantiği ister — bu değerlendiricinin kapsamı değil,
#     o yüzden tuzak KULLANIMDA yasaklanır.)

_SCHEMA_KEYWORD_SHAPE: dict[str, type | tuple[type, ...]] = {
    "type": (str, list),
    "required": list,
    "properties": dict,
    "additionalProperties": (bool, dict),
    "propertyNames": dict,
    "items": dict,
    "minItems": int,
    "maxItems": int,
    "minLength": int,
    "maxLength": int,
    "minimum": (int, float),
    "pattern": str,
    "uniqueItems": bool,
    "enum": list,
    "oneOf": list,
    "allOf": list,
    "anyOf": list,
    "not": dict,
    "$ref": str,
}
_KNOWN_TYPES = frozenset(
    {"object", "array", "string", "boolean", "integer", "number", "null"}
)


def check_schema_shape(schema: dict) -> None:
    """Şemayı, hiçbir örneğe bakmadan önce denetler.

    Raises:
        SchemaUnsupported: tanınmayan anahtar sözcük, yanlış tipte değer, bozuk `pattern`,
            çözülemeyen `$ref`, `$id`/`schema_version` uyumsuzluğu ya da combinator tuzağı.
    """
    _check_id_matches_version(schema)
    _walk_schema(schema, schema, "#")


def _check_id_matches_version(schema: dict) -> None:
    schema_id = schema.get("$id")
    const = schema.get("properties", {}).get("schema_version", {}).get("const")
    if not isinstance(schema_id, str):
        raise SchemaUnsupported("kökte `$id` yok ya da dizge değil")
    if not isinstance(const, int) or isinstance(const, bool):
        raise SchemaUnsupported("`properties.schema_version.const` bir tamsayı olmalı")
    want = f"/v{const}/SCHEMA.json"
    if not schema_id.endswith(want):
        raise SchemaUnsupported(
            f"`$id` ile sürüm ayrışmış: schema_version.const={const} iken $id={schema_id!r} — "
            f"{want!r} ile bitmeli. İki farklı şema aynı kimliği taşıyamaz."
        )


def _walk_schema(node, root: dict, path: str) -> None:
    if isinstance(node, bool):
        return
    if not isinstance(node, dict):
        raise SchemaUnsupported(f"{path}: şema düğümü nesne ya da boolean olmalı")

    unknown = set(node) - _ANNOTATION_KEYWORDS - _ASSERTION_KEYWORDS
    if unknown:
        raise SchemaUnsupported(f"desteklenmeyen şema anahtar sözcüğü {sorted(unknown)!r} ({path})")

    for key, value in node.items():
        want = _SCHEMA_KEYWORD_SHAPE.get(key)
        if want is not None and not isinstance(value, want):
            names = want.__name__ if isinstance(want, type) else "/".join(t.__name__ for t in want)
            raise SchemaUnsupported(
                f"{path}/{key}: {names} bekleniyordu, {type(value).__name__} geldi ({value!r})"
            )

    if "type" in node:
        for t in node["type"] if isinstance(node["type"], list) else [node["type"]]:
            if t not in _KNOWN_TYPES:
                raise SchemaUnsupported(f"{path}/type: bilinmeyen tip {t!r}")
    if "pattern" in node:
        try:
            re.compile(node["pattern"])
        except re.error as exc:
            raise SchemaUnsupported(f"{path}/pattern: derlenemeyen düzenli ifade: {exc}") from exc
    if "$ref" in node:
        ref = node["$ref"]
        target = root
        if ref != "#":
            if not ref.startswith("#/"):
                raise SchemaUnsupported(f"{path}/$ref: yalnız yerel $ref destekleniyor ({ref!r})")
            for token in ref[2:].split("/"):
                token = token.replace("~1", "/").replace("~0", "~")
                if not isinstance(target, dict) or token not in target:
                    raise SchemaUnsupported(f"{path}/$ref: çözülemedi ({ref!r})")
                target = target[token]

    _check_combinator_trap(node, path)

    for key in ("properties", "$defs"):
        for name, child in (node.get(key) or {}).items():
            _walk_schema(child, root, f"{path}/{key}/{name}")
    for key in ("items", "not", "propertyNames"):
        if key in node:
            _walk_schema(node[key], root, f"{path}/{key}")
    if isinstance(node.get("additionalProperties"), dict):
        _walk_schema(node["additionalProperties"], root, f"{path}/additionalProperties")
    for key in ("oneOf", "allOf", "anyOf"):
        branches = node.get(key)
        if branches is None:
            continue
        if not branches:
            raise SchemaUnsupported(f"{path}/{key}: boş combinator")
        for i, branch in enumerate(branches):
            _walk_schema(branch, root, f"{path}/{key}/{i}")


def _check_combinator_trap(node: dict, path: str) -> None:
    """`additionalProperties: false` + kardeş kolda `properties` = sessiz ret tuzağı."""
    if node.get("additionalProperties") is not False:
        return
    for key in ("allOf", "anyOf", "oneOf"):
        for i, branch in enumerate(node.get(key) or []):
            if isinstance(branch, dict) and "properties" in branch:
                raise SchemaUnsupported(
                    f"{path}/{key}/{i}: `additionalProperties: false` olan bir düğümün "
                    "combinator kolu `properties` tanımlayamaz — Draft 2020-12'de "
                    "`additionalProperties` kardeş kolların alanlarını GÖRMEZ, geçerli belgeler "
                    "sessizce reddedilir. Alanı düğümün kendi `properties`'ine taşıyın."
                )
    if "$ref" in node and node.get("properties") is not None:
        raise SchemaUnsupported(
            f"{path}: `additionalProperties: false` + kardeş `$ref` — aynı tuzak."
        )


# ── Şemayla ifade edilemeyen repo kuralları ───────────────────────────────────


def _walk_strings(node, pointer: str):
    if isinstance(node, dict):
        for key, value in node.items():
            yield from _walk_strings(value, f"{pointer}/{key}")
    elif isinstance(node, list):
        for i, item in enumerate(node):
            yield from _walk_strings(item, f"{pointer}/{i}")
    elif isinstance(node, str):
        yield pointer, node


def _nearest_key(pointer: str) -> str:
    """Dizin segmentlerini atlayarak en yakın nesne anahtarını döndürür.

    '/cases/3/input/cert_cache_hex/0' -> 'cert_cache_hex'
    """
    for segment in reversed(pointer.split("/")[1:]):
        if not segment.isdigit():
            return segment
    return ""


def _deep_hex_findings(path: str, doc) -> list[Finding]:
    """Her derinlikte '*_hex' / 'hex' anahtarı küçük harf, çift uzunluk, 0x'siz hex olmalı.

    Anahtar bir hex DİZGE ya da hex dizgelerden oluşan bir DİZİ tutabilir; dizi
    elemanları da denetlenir (dizin segmentleri atlanarak en yakın anahtar bulunur).

    Ayrıca `input` altındaki hiçbir dizge 0x ile başlayamaz: byte dizileri hex DİZGE
    olarak yazılır, 0x öneki YOK (şema kuralı, ccr ADR-05 fixed-width disiplininin uzantısı).
    """
    out: list[Finding] = []
    for pointer, value in _walk_strings(doc, ""):
        key = _nearest_key(pointer)
        if key == "hex" or key.endswith("_hex"):
            if HEX_RE.match(value) is None:
                out.append(
                    Finding(
                        path,
                        pointer,
                        SV_HEX,
                        "hex alanı: yalnız küçük harf [0-9a-f], çift uzunluk, 0x öneki YOK "
                        f"— gelen {value[:64]!r}",
                    )
                )
        elif pointer.startswith("/cases/") and "/input/" in pointer and value[:2].lower() == "0x":
            out.append(
                Finding(path, pointer, SV_HEX, f"'0x' öneki yasak — gelen {value[:64]!r}")
            )
    return out


def _count_findings(path: str, doc) -> list[Finding]:
    """`case_count` == len(cases). Kesilmiş dosyayı ve tüketici vakumunu yakalar."""
    declared = doc.get("case_count")
    cases = doc.get("cases")
    if not isinstance(declared, int) or _is_bool(declared) or not isinstance(cases, list):
        return []  # şekil hatası zaten SV-005'te
    if declared != len(cases):
        return [
            Finding(
                path,
                "/case_count",
                SV_COUNT,
                f"beyan {declared}, cases uzunluğu {len(cases)} — dosya kesilmiş olabilir",
            )
        ]
    return []


def _flag_findings(path: str, doc) -> list[Finding]:
    """Her kullanılan flag tanımlı; her tanımlı flag kullanılıyor.

    İlk yön yazım hatasını yakalar (`shrot-key` diye bir flag sessizce anlamsız kalmaz),
    ikinci yön ölü not birikmesini engeller (vaka silinir, açıklaması kalır).
    """
    notes = doc.get("notes") or {}
    cases = doc.get("cases")
    if not isinstance(notes, dict) or not isinstance(cases, list):
        return []
    out: list[Finding] = []
    used: set[str] = set()
    for i, case in enumerate(cases):
        if not isinstance(case, dict):
            continue
        for j, flag in enumerate(case.get("flags") or []):
            if not isinstance(flag, str):
                continue
            used.add(flag)
            if flag not in notes:
                out.append(
                    Finding(
                        path,
                        f"/cases/{i}/flags/{j}",
                        SV_FLAGS,
                        f"flag {flag!r} kökteki `notes` sözlüğünde tanımlı değil",
                    )
                )
    for flag in sorted(set(notes) - used):
        out.append(
            Finding(
                path,
                f"/notes/{flag}",
                SV_FLAGS,
                f"not {flag!r} hiçbir vakada kullanılmıyor — ölü kayıt",
            )
        )
    return out


def _control_char_findings(path: str, doc) -> list[Finding]:
    out: list[Finding] = []
    for pointer, value in _walk_strings(doc, ""):
        if CTRL_RE.search(value):
            out.append(Finding(path, pointer, SV_CTRL, "dizgede kontrol karakteri var"))
    return out


# ── Kanonik serileştirme ──────────────────────────────────────────────────────
#
# NEDEN. Vektör dosyalarını bir üretici yazacak. Üretici her koşuda aynı içeriği FARKLI
# byte'larla yazarsa (anahtar sırası, girinti, unicode kaçışı, son satırsonu) her yeniden-
# üretim git diff'ini gürültüyle doldurur ve golden incelemesi okunamaz hâle gelir —
# "bu byte kasıtlı mı değişti" sorusu cevaplanamaz. Kanonik biçim o soruyu geri getirir:
# diff'te görünen her satır gerçek bir içerik değişikliğidir.
#
# KURAL (NORMATİF):
#   * UTF-8, BOM YOK · satır sonu LF · dosya sonunda TEK satırsonu
#   * girinti 2 boşluk, ayraçlar `,` ve `: ` (json.dumps indent varsayılanı)
#   * ensure_ascii=True — ASCII olmayan her karakter \uXXXX olarak kaçırılır (editör/locale
#     bağımsız byte kararlılığı)
#   * ÜST DÜZEY ve VAKA anahtar sırası = SCHEMA.json'daki bildirim sırası (alfabetik DEĞİL:
#     şema sırası okunur bir sıra ve şema zaten tek otorite)
#   * `input` içindeki serbest haritalarda anahtarlar özyinelemeli LEKSİKOGRAFİK sıralı
#     (şemanın söyleyecek bir şeyi yok, o yüzden belirlenimci bir kural gerekiyor)
#
# KAPSAM. Bu kural `vectors/` ağacındaki GERÇEK vektör dosyaları içindir (üretici çıktısı).
# `scripts/vectors/testdata/` fixture'ları kapsam DIŞIDIR: bazıları kasten bozuk JSON, bazıları
# kasten yinelenen anahtar taşır — kanonikleştirilemezler ve kanonikleştirilmemelidirler.


def _schema_key_order(schema: dict) -> tuple[list[str], list[str]]:
    """Kanonik anahtar sırası ŞEMADAN okunur; ikinci bir sıra listesi tutulmaz."""
    root = list(schema.get("properties", {}))
    case = list(schema.get("$defs", {}).get("case", {}).get("properties", {}))
    return root, case


def _ordered(mapping: dict, order: list[str]) -> dict:
    """Bilinen anahtarlar şema sırasında; şemada olmayan varsa (olmamalı) sonda, sıralı."""
    out = {k: mapping[k] for k in order if k in mapping}
    out.update({k: mapping[k] for k in sorted(mapping) if k not in out})
    return out


def _sorted_deep(value):
    """Serbest `input` haritaları: özyinelemeli leksikografik anahtar sırası."""
    if isinstance(value, dict):
        return {k: _sorted_deep(value[k]) for k in sorted(value)}
    if isinstance(value, list):
        return [_sorted_deep(v) for v in value]
    return value


def canonical_text(doc: dict, schema: dict) -> str:
    """Belgeyi kanonik metne çevirir. Girdi ŞEMADAN GEÇMİŞ olmalıdır."""
    root_order, case_order = _schema_key_order(schema)
    out = _ordered(doc, root_order)
    if isinstance(out.get("cases"), list):
        cases = []
        for case in out["cases"]:
            if not isinstance(case, dict):
                cases.append(case)
                continue
            c = _ordered(case, case_order)
            if isinstance(c.get("input"), dict):
                c["input"] = _sorted_deep(c["input"])
            cases.append(c)
        out["cases"] = cases
    return json.dumps(out, indent=2, ensure_ascii=True) + "\n"


def _canonical_findings(path: str, raw: str, doc: dict, schema: dict) -> list[Finding]:
    want = canonical_text(doc, schema)
    if raw == want:
        return []
    detail = "dosya kanonik biçimde değil"
    if raw.replace("\r\n", "\n") == want:
        detail = "satır sonu CRLF — LF olmalı"
    elif raw.rstrip("\n") + "\n" == want:
        detail = "dosya sonunda tam olarak bir satırsonu olmalı"
    elif raw.startswith("\ufeff"):
        detail = "UTF-8 BOM var"
    elif json.loads(raw) == json.loads(want):
        detail = "içerik aynı, BİÇİM farklı (anahtar sırası / girinti / unicode kaçışı)"
    return [
        Finding(
            path,
            "",
            SV_CANONICAL,
            f"{detail}. Düzeltmek için: "
            f"python3 scripts/vectors/validate.py {path} --write-canonical",
        )
    ]


# ── Dosya başına doğrulama ────────────────────────────────────────────────────


def validate_file(
    path: Path,
    validator: SubsetValidator,
    ids: dict[str, str],
    canonical: bool = False,
    max_bytes: int = MAX_FILE_BYTES,
) -> list[Finding]:
    display = str(path)
    try:
        size = path.stat().st_size
    except OSError as exc:
        return [Finding(display, "", SV_PARSE, f"okunamadı: {exc}")]
    if size > max_bytes:
        return [
            Finding(
                display,
                "",
                SV_SIZE,
                f"{size} byte > {max_bytes} sınırı (SR-AUDIT-001: bounds-before-alloc)",
            )
        ]

    try:
        raw = path.read_text(encoding="utf-8")
    except OSError as exc:
        return [Finding(display, "", SV_PARSE, f"okunamadı: {exc}")]

    try:
        doc = json.loads(raw, object_pairs_hook=_no_duplicates)
    except DuplicateKey as exc:
        return [Finding(display, "", SV_DUP_KEY, f"yinelenen JSON anahtarı: {exc.key!r}")]
    except json.JSONDecodeError as exc:
        return [Finding(display, "", SV_PARSE, f"JSON ayrıştırılamadı: {exc}")]

    # 1) schema_version kapısı — yapısal doğrulamadan ÖNCE. Bilinmeyen sürümde
    #    araç HATA verir; şema ihlali yığını basıp asıl nedeni gömmez.
    if not isinstance(doc, dict):
        return [Finding(display, "", SV_SCHEMA, f"kök nesne olmalı, gelen {_kind_of(doc)}")]
    if "schema_version" not in doc:
        return [Finding(display, "/schema_version", SV_VERSION_SHAPE, "alan yok")]
    version = doc["schema_version"]
    if not isinstance(version, int) or _is_bool(version):
        return [
            Finding(
                display,
                "/schema_version",
                SV_VERSION_SHAPE,
                f"tamsayı olmalı, gelen {_kind_of(version)} ({version!r})",
            )
        ]
    if version != SUPPORTED_SCHEMA_VERSION:
        return [
            Finding(
                display,
                "/schema_version",
                SV_VERSION_UNKNOWN,
                f"desteklenmeyen sürüm {version}; bu araç yalnız "
                f"{SUPPORTED_SCHEMA_VERSION} biliyor — sessizce GEÇİLMEZ",
            )
        ]

    # 2) Yapı
    findings = [
        Finding(display, pointer, SV_SCHEMA, message)
        for pointer, message in validator.errors(doc, validator.root, "")
    ]

    # 3) Şemayla ifade edilemeyen kurallar
    findings += _deep_hex_findings(display, doc)
    findings += _control_char_findings(display, doc)
    findings += _count_findings(display, doc)
    findings += _flag_findings(display, doc)

    # 4) Kanonik biçim (üretici disiplini; yalnız istendiğinde)
    if canonical and not findings:
        findings += _canonical_findings(display, raw, doc, validator.root)

    # 5) id repo genelinde tekil
    for i, case in enumerate(doc.get("cases", []) if isinstance(doc.get("cases"), list) else []):
        if not isinstance(case, dict):
            continue
        case_id = case.get("id")
        if not isinstance(case_id, str):
            continue
        first = ids.get(case_id)
        if first is None:
            ids[case_id] = f"{display}/cases/{i}"
        else:
            findings.append(
                Finding(
                    display,
                    f"/cases/{i}/id",
                    SV_DUP_ID,
                    f"id {case_id!r} repo genelinde tekil değil; ilk görülme: {first}",
                )
            )

    return findings


# ── Giriş noktası ─────────────────────────────────────────────────────────────


def collect(paths: list[str]) -> list[Path]:
    out: list[Path] = []
    for raw in paths:
        p = Path(raw)
        if p.is_dir():
            out += [f for f in sorted(p.rglob("*.json")) if f.name != "SCHEMA.json"]
        else:
            out.append(p)
    return out


def cross_check(files: list[Path], schema: dict, required: bool = False) -> int | None:
    """Üçüncü parti Draft 2020-12 uygulamasıyla çapraz kontrol.

    Döner: bulgu sayısı; `None` = kütüphane yok ve `required` değil (atlandı).
    """
    try:
        import jsonschema  # type: ignore
    except ImportError:
        if required:
            print(
                "[SV-009] --require-cross-check verildi ama `jsonschema` kurulu değil — "
                "çapraz kontrol SESSİZCE ATLANMAZ",
                file=sys.stderr,
            )
            return -1
        print("cross-check: `jsonschema` kurulu değil — atlandı", file=sys.stderr)
        return None
    checker = jsonschema.Draft202012Validator(schema)
    bad = 0
    for f in files:
        try:
            doc = json.loads(f.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        for err in checker.iter_errors(doc):
            bad += 1
            pointer = "/" + "/".join(str(x) for x in err.absolute_path)
            print(f"cross-check {f}{pointer}: {err.message}", file=sys.stderr)
    from importlib.metadata import version as _pkg_version

    print(f"cross-check: {len(files)} dosya, {bad} bulgu (jsonschema {_pkg_version('jsonschema')})")
    return bad


def _write_canonical(files: list[Path], validator: SubsetValidator) -> int:
    """`--write-canonical`: geçerli dosyaları kanonik biçimde yeniden yazar.

    GEÇERSİZ dosyaya DOKUNULMAZ — kanonikleştirme bir düzeltme aracı değildir; bozuk bir
    dosyayı "düzeltip" ihlali gizlemesi istenmez.
    """
    changed = 0
    for path in files:
        raw = path.read_text(encoding="utf-8")
        ids: dict[str, str] = {}
        if validate_file(path, validator, ids):
            print(f"{path}: doğrulamadan geçmiyor — DOKUNULMADI", file=sys.stderr)
            continue
        want = canonical_text(json.loads(raw), validator.root)
        if raw != want:
            path.write_text(want, encoding="utf-8")
            print(f"{path}: kanonik biçime yazıldı")
            changed += 1
    print(f"{len(files)} dosya · {changed} yeniden yazıldı")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Kama test-vektör dosyalarını vectors/SCHEMA.json'a karşı doğrular."
    )
    parser.add_argument("paths", nargs="+", help="JSON dosyası ya da dizin (özyinelemeli)")
    parser.add_argument(
        "--schema",
        default=str(Path(__file__).resolve().parents[2] / "vectors" / "SCHEMA.json"),
        help="SCHEMA.json yolu",
    )
    parser.add_argument(
        "--cross-check",
        action="store_true",
        help="`jsonschema` kuruluysa üçüncü parti uygulamayla da doğrula",
    )
    parser.add_argument(
        "--require-cross-check",
        action="store_true",
        help=(
            "`jsonschema` YOKSA hata ver (rc=2). --cross-check tek başına kütüphane "
            "yoksa sessizce atlar ve yeşil döner — CI'da bu, çapraz kontrolün hiç "
            "koşmaması demektir (vakum). CI bu bayrakla koşar"
        ),
    )
    parser.add_argument(
        "--github",
        action="store_true",
        default=os.environ.get("GITHUB_ACTIONS") == "true",
        help=(
            "bulguları GitHub Actions anotasyonu olarak da bas (PR diff'inde dosyaya "
            "iliştirilir). GITHUB_ACTIONS=true iken kendiliğinden açılır"
        ),
    )
    parser.add_argument(
        "--canonical",
        action="store_true",
        help=(
            "kanonik serileştirmeyi de denetle (SV-011). ÜRETİCİ DİSİPLİNİ: vectors/ "
            "ağacındaki gerçek vektör dosyaları için; testdata fixture'ları kapsam dışı"
        ),
    )
    parser.add_argument(
        "--write-canonical",
        action="store_true",
        help="dosyaları kanonik biçimde YENİDEN YAZ (--canonical'ın düzeltme kipi)",
    )
    parser.add_argument(
        "--max-file-bytes",
        type=int,
        default=MAX_FILE_BYTES,
        metavar="N",
        help=f"dosya boyutu sınırı (varsayılan {MAX_FILE_BYTES}); aşılırsa SV-012",
    )
    parser.add_argument(
        "--min-files",
        type=int,
        default=0,
        metavar="N",
        help=(
            "en az N dosya doğrulanmalı; aksi hâlde SV-010. VAKUM KAPISI: yol yanlış "
            "yazıldığında ya da ağaç boşaldığında bu bayrak olmadan doğrulayıcı 0 dosya "
            "gezip YEŞİL döner (ci.yml'deki 'test result: ok. [1-9]' kapılarıyla aynı sınıf)"
        ),
    )
    args = parser.parse_args(argv)

    try:
        schema = json.loads(Path(args.schema).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"[{SV_SCHEMA_FILE}] şema okunamadı ({args.schema}): {exc}", file=sys.stderr)
        return 2

    # Şema, herhangi bir dosyaya bakılmadan ÖNCE denetlenir: bozuk bir şema veri hatası
    # (rc=1) değil ARAÇ hatasıdır (rc=2) ve teşhisle bildirilir, yığın iziyle değil.
    try:
        check_schema_shape(schema)
    except SchemaUnsupported as exc:
        print(f"[{SV_SCHEMA_FILE}] {args.schema}: {exc}", file=sys.stderr)
        return 2
    # Bedava ikinci merci: kuruluysa resmî meta-şema.
    try:
        import jsonschema as _js

        _js.validators.validator_for(schema).check_schema(schema)
    except ImportError:
        pass
    except Exception as exc:  # jsonschema.SchemaError ve türevleri
        print(f"[{SV_SCHEMA_FILE}] {args.schema}: meta-şema reddetti: {exc}", file=sys.stderr)
        return 2

    validator = SubsetValidator(schema)
    files = collect(args.paths)
    missing = [f for f in files if not f.is_file()]
    if missing:
        for f in missing:
            print(f"[{SV_PARSE}] dosya yok: {f}", file=sys.stderr)
        return 2

    if args.write_canonical:
        return _write_canonical(files, validator)

    ids: dict[str, str] = {}
    findings: list[Finding] = []
    try:
        for f in files:
            findings += validate_file(
                f, validator, ids, canonical=args.canonical, max_bytes=args.max_file_bytes
            )
    except SchemaUnsupported as exc:
        # Fail-closed: şema, doğrulayıcının zorlayamadığı bir kural içeriyor.
        print(f"[{SV_SCHEMA_FILE}] {exc}", file=sys.stderr)
        print(
            "Doğrulayıcı bu kuralı ZORLAYAMAZ; sessizce geçmek yerine durdu. "
            "SubsetValidator'a anahtar sözcüğü ekleyin.",
            file=sys.stderr,
        )
        return 2

    if len(files) < args.min_files:
        findings.append(
            Finding(
                ", ".join(args.paths),
                "",
                SV_VACUOUS,
                f"en az {args.min_files} dosya beklendi, {len(files)} bulundu — "
                "kapı vakumda çalışıyor (yol yanlış ya da ağaç boş)",
            )
        )

    for finding in findings:
        print(str(finding), file=sys.stderr)
        if args.github:
            print(finding.annotation())

    rc = 1 if findings else 0
    if args.cross_check or args.require_cross_check:
        bad = cross_check(files, schema, required=args.require_cross_check)
        if bad == -1:
            return 2
        if bad:
            rc = 1

    print(f"{len(files)} dosya · {len(ids)} vaka · {len(findings)} bulgu")
    return rc


if __name__ == "__main__":
    sys.exit(main())
