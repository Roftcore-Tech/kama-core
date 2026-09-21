#!/usr/bin/env python3
"""Kama test-vektör doğrulayıcısı — vectors/SCHEMA.json kapısı (T-00.1).

Üç araç aynı byte beklentisini okur: Python üretici, Rust testleri, bu doğrulayıcı.
Bu betik CI kolunun tamamıdır: `vectors/SCHEMA.json`'u okur, verilen dosyaları ona
karşı doğrular ve şemayla ifade edilemeyen üç repo kuralını ayrıca zorlar
(derin hex disiplini, id tekilliği, kontrol karakteri yasağı).

Kullanım:
    python3 scripts/vectors/validate_vectors.py vectors/
    python3 scripts/vectors/validate_vectors.py a.json b.json --cross-check

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
import re
import sys
from pathlib import Path

SUPPORTED_SCHEMA_VERSION = 1

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

HEX_RE = re.compile(r"^([0-9a-f]{2})*$")
CTRL_RE = re.compile(r"[\x00-\x1f\x7f]")


class SchemaUnsupported(Exception):
    """SCHEMA.json bu değerlendiricinin tanımadığı bir anahtar sözcük içeriyor."""


class Finding:
    __slots__ = ("path", "pointer", "code", "message")

    def __init__(self, path: str, pointer: str, code: str, message: str) -> None:
        self.path = path
        self.pointer = pointer or "/"
        self.code = code
        self.message = message

    def __str__(self) -> str:
        return f"{self.path}{self.pointer}: [{self.code}] {self.message}"


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

        if isinstance(instance, dict):
            for key in schema.get("required", []):
                if key not in instance:
                    out.append((pointer, f"zorunlu alan yok: {key!r}"))
            props = schema.get("properties", {})
            for key, value in instance.items():
                child = f"{pointer}/{key}"
                if key in props:
                    out += self.errors(value, props[key], child)
                elif schema.get("additionalProperties", True) is False:
                    out.append((child, "bilinmeyen alan (şema fail-closed)"))

        if isinstance(instance, list):
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


def _control_char_findings(path: str, doc) -> list[Finding]:
    out: list[Finding] = []
    for pointer, value in _walk_strings(doc, ""):
        if CTRL_RE.search(value):
            out.append(Finding(path, pointer, SV_CTRL, "dizgede kontrol karakteri var"))
    return out


# ── Dosya başına doğrulama ────────────────────────────────────────────────────


def validate_file(path: Path, validator: SubsetValidator, ids: dict[str, str]) -> list[Finding]:
    display = str(path)
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

    # 4) id repo genelinde tekil
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


def cross_check(files: list[Path], schema: dict) -> int:
    """Üçüncü parti Draft 2020-12 uygulamasıyla çapraz kontrol (opsiyonel)."""
    try:
        import jsonschema  # type: ignore
    except ImportError:
        print("cross-check: `jsonschema` kurulu değil — atlandı", file=sys.stderr)
        return 0
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
    args = parser.parse_args(argv)

    try:
        schema = json.loads(Path(args.schema).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"[{SV_SCHEMA_FILE}] şema okunamadı ({args.schema}): {exc}", file=sys.stderr)
        return 2

    validator = SubsetValidator(schema)
    files = collect(args.paths)
    missing = [f for f in files if not f.is_file()]
    if missing:
        for f in missing:
            print(f"[{SV_PARSE}] dosya yok: {f}", file=sys.stderr)
        return 2

    ids: dict[str, str] = {}
    findings: list[Finding] = []
    try:
        for f in files:
            findings += validate_file(f, validator, ids)
    except SchemaUnsupported as exc:
        # Fail-closed: şema, doğrulayıcının zorlayamadığı bir kural içeriyor.
        print(f"[{SV_SCHEMA_FILE}] {exc}", file=sys.stderr)
        print(
            "Doğrulayıcı bu kuralı ZORLAYAMAZ; sessizce geçmek yerine durdu. "
            "SubsetValidator'a anahtar sözcüğü ekleyin.",
            file=sys.stderr,
        )
        return 2

    for finding in findings:
        print(str(finding), file=sys.stderr)

    rc = 1 if findings else 0
    if args.cross_check and cross_check(files, schema):
        rc = 1

    print(f"{len(files)} dosya · {len(ids)} vaka · {len(findings)} bulgu")
    return rc


if __name__ == "__main__":
    sys.exit(main())
