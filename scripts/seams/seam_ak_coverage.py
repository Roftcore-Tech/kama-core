#!/usr/bin/env python3
"""KM-SEAM-AK-001 kapsama denetleyicisi — K4G-15 NE 2: "`ui-api v0.3`'teki her çağrı ve alan için tek satır";
eksik satır = kart DONE değil.

    scripts/seams/seam_ak_coverage.py --ui-api <ui-api-v0.md v0.3 yolu> [--seam YOL] [--root DİZİN] [--report]
    scripts/seams/seam_ak_coverage.py --selftest

GİRDİ. `ui-api v0.3` (kama-engine `docs/ui-api-v0.md`, tag `ui-api-v0.3` = `5de9be16`) bu depoda DEĞİLDİR; yolu
argümanla verilir ve sha256'sı pinlidir (`UI_API_SHA256`): başka bir sürüm verilirse ortam hatası (2). Seam belgesi
varsayılan olarak `docs/seams/KM-SEAM-AK-001.md`, Rust yüzeyi `crates/kama-mobile/src/{lib,event,error}.rs`
(`--root` deposundan).

YÖNTEM (tanımlayıcı düzeyi; v0.1'in yalnız kod aralığı tarayan, yazım düzeyindeki yöntemi `VOICE`'u ve
`DeviceRenewalState.Ok`'u kaçırmıştı — K4G-15 incelemesi):
  1. Çıkarım. v0.3'ün üç kaynağı taranır: satır içi kod aralıkları (`…`), çitli kod blokları ve DÜZYAZI. Aday =
     Unicode-bilinçli tanımlayıcı (noktalı yol dahil), yalnız ASCII. Kod aralığı/çitteki her aday alınır;
     düzyazıdan yalnız KOD BİÇİMLİ adaylar alınır: BÜYÜK_HARF (≥ 2 karakter; `VOICE`), iç büyük harfli
     (`camelCase` / `PascalCase`: küçük harfi büyük harf izler), alt çizgili ya da noktalı. Düzyazının Başı-büyük tek
     sözcükleri (`Motor`) ve küçük harfli sözcükleri Türkçe/İngilizce metindir: YALNIZ düzyazıda geçenlerin sayısı
     pinlidir (`PROSE_TITLE_ONLY`, `PROSE_LOWER_ONLY`) ve `--report` hepsini listeler (elle gözden geçirildi;
     hiçbiri API adı değil). SINIR: yalnız düzyazıda, yalnız Başı-büyük ya da küçük harfle yazılmış bir API adı
     bu yöntemle görülmez — pinli sayı, v0.3 değişmeden (sha256) o listenin değişmediğini garanti eder.
  2. Sınıflama. Her aday ya ÖĞE'dir ya da `NON_ITEMS`'taki bir sınıftadır (kategori adıyla). Öğe = aday − öğe-dışı;
     her öğenin §4'te satırı olmalıdır. Öğe-dışı sınıflanan bir ad §4'ün ilk sütununda geçerse KIRMIZI (sınıflama
     çelişkisi). `NON_ITEMS`'ta olup v0.3'te aday olmayan ad da KIRMIZI (bayat tablo).
  3. Kimlik. Aynı yazımın farklı sahipli kullanımları (`POLYSEMY`: `Ok` = ConnectionState / DeviceRenewalState …)
     ayrı kimliktir: her sahip, §4'te adı ve sahibi İLK SÜTUNDA birlikte geçen (sahip parantezde ya da noktalı ad
     olarak) bir satırla kapsanır. Her çokanlamlı ad için v0.3 kanıt satırları tablodadır; denetleyici adın o
     satırlarda geçtiğini sınar.
  4. Satırlar. §4 tablolarının her satırı: (i) ilk sütundaki her ad bir v0.3 adayıdır (uydurma satır yok);
     (ii) eşleme üç biçimden biridir — "Rust: `KamaMobile.<çağrı>`" ya da "Rust: `EngineEvent.<olay>`" ile
     başlayan, "PoC'de etkisiz gövde (K6)", "Kotlin tarafı (Hasan)"; (iii) belgenin HER yerindeki
     `KamaMobile.x` / `EngineEvent.X` / `NodeException.X` atfı Rust yüzeyinde vardır (Kotlin adlarıyla:
     `send_text` → `sendText`). §3 (K6 listesi): `K6_ROWS` satır, eşlemeler geçerli, K6 anahtar adları
     (`K6_KEYS`) ilk sütunlarda.

Çıkış: 0 = kapsandı · 1 = eksik / yabancı / geçersiz / kırık atıf · 2 = ortam/kullanım hatası.
"""

from __future__ import annotations

import hashlib
import re
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[2]
SEAM_PATH = "docs/seams/KM-SEAM-AK-001.md"
RUST_FILES = {
    "lib": "crates/kama-mobile/src/lib.rs",
    "event": "crates/kama-mobile/src/event.rs",
    "error": "crates/kama-mobile/src/error.rs",
}
# ui-api v0.3 = kama-engine tag `ui-api-v0.3` (5de9be16) `docs/ui-api-v0.md`.
UI_API_SHA256 = "7d409bdb15cc279612d5ac3ef09c7857569a4474a47745673701c01e8b1c2d14"
# Yalnız düzyazıda geçen Başı-büyük (tek harfli `A` dahil) / küçük harfli ASCII sözcük sayısı (v0.3'te;
# `--report` listeler).
PROSE_TITLE_ONLY = 155
PROSE_LOWER_ONLY = 738

IDENT = re.compile(r"(?<![\w\-/.])([^\W\d]\w*(?:\.[^\W\d]\w*)*)")
FENCE = re.compile(r"^\s*```")
CODE_SPAN = re.compile(r"`([^`]*)`")
MAPPING_FIXED = ("PoC'de etkisiz gövde (K6)", "Kotlin tarafı (Hasan)")
RUST_MAPPING = re.compile(r"Rust: `(KamaMobile|EngineEvent)\.([A-Za-z_]\w*)")
SURFACE_REF = re.compile(r"\b(KamaMobile|EngineEvent|NodeException)\.([A-Za-z_]\w*)")
ITEM_HEADER = "`ui-api v0.3` öğesi"
K6_HEADER = "K6 maddesi"

# ── v0.3'e özgü sınıflama (sha256 pinli belge için) ──────────────────────────────────────────────
NON_ITEMS: dict[str, frozenset[str]] = {
    "kaynak etiketi, ekran/kod/karar kimliği, şekil ya da alıntı etiketi": frozenset({
        "A1", "A3", "A6", "AB", "ASSUMPTIONS", "B1", "B12", "B2", "B2C", "B3", "B4", "B5", "B6", "B7", "B8",
        "BETA", "CANCELLED", "DONDURULMADI", "E", "E8", "EKRAN", "ENGINE", "FAKE", "FAKE_PLACEHOLDER", "FIGMA",
        "Gate", "ID", "INV", "K2", "KAPSAM", "KM", "P4", "S3", "SOLO", "SORUN", "SR", "SUPERSEDED", "T2", "T3",
        "TAM", "TASARIM", "TOPARLANIYOR", "UA", "UI", "UNIFORM", "VAR", "VE", "YALNIZCA", "YAZ", "YOK", "YOKTUR",
        "backend", "sahip", "v0",
    }),
    "kişi ya da kurum adı": frozenset({"Abkarada", "Roftcore"}),
    "belge, dosya, depo, dal, crate, paket ya da URI şeması adı": frozenset({
        "CLAUDE.md", "FakeSeed.kt", "UiApiV0.kt", "android.resource", "api_surface_v0.rs", "com.roftcore.kama",
        "com.roftcore.kama.engine", "docs", "engine", "kama", "ktlint_official", "main", "onboarding", "res",
        "signaling", "tag", "ui",
    }),
    "değişmez değer, birim ya da fake girdisi/tetiği": frozenset({
        "MB", "Test", "b29d36fd", "bad", "baran.y", "bayt", "expired", "false", "n", "null", "offline", "onur.k",
        "pinfail", "reject", "size", "true", "xxx",
    }),
    "kama-core / keşif / RA / medya iç adı (ui-api yüzeyi değil)": frozenset({
        "ADR", "CodecMetadataExt", "DiscoveryRecord", "FFI", "RA", "RTP", "SHA", "VOPRF", "admin", "bindings",
        "ccr", "client", "crypto", "dest_hash", "discovery", "fetch_mailbox", "hub", "identity_hash", "k",
        "lookup_key", "phone", "ra", "rate", "record_sequence", "routing_key", "send_message", "start_session",
    }),
    "Kotlin/Android platform adı ya da genel teknoloji kısaltması": frozenset({
        "API", "APK", "FCM", "IP", "OS", "PoC", "QR", "RFC", "SIM", "SMS", "TTL", "URI", "UX", "UniFFI",
        "android", "applicationContext", "interface", "internal", "process", "when",
    }),
    "v0.3'te kaldırılmış ad (SUPERSEDED)": frozenset({"AwaitingFederationChoice", "NotStarted", "chooseIndividual"}),
    "v0.3'ün \"API'de YOK\" dediği alan, metot ya da özellik": frozenset({"PTT", "muted", "readAt", "revoke"}),
    "Kotlin invariant test adı": frozenset({
        "test_contacts_api_has_no_query_method", "test_drain_blocked_never_exits_trust_failure",
        "test_drain_result_carries_no_fields", "test_engine_never_returns_phone",
        "test_hidden_mode_notification_carries_no_content", "test_hidden_unread_has_no_fields",
        "test_no_core_types_in_ui_api", "test_pending_count_only_in_diagnostics", "test_rejected_has_no_fields",
        "test_row_indicator_priority", "test_send_text_cannot_fail", "test_trust_failure_is_separate_state",
        "test_trust_retry_only_on_connection_api", "test_verification_not_in_row_types",
    }),
    "kod aralığı ya da çit içindeki düzyazı sözcüğü (Türkçe/İngilizce; şekil metni)": frozenset({
        "a", "ad", "bir", "cihaz", "da", "davet", "durum", "eksen", "expiry", "eylemiyle", "facade", "geldi",
        "gerekmez", "geri", "gitti", "herhangi", "kod", "koptu", "medya", "opak", "paketi", "parse", "payload",
        "resend", "tekrar", "tipi", "tipleri", "uniform", "uygulama", "yeni", "yerel", "yok",
    }),
}

# Aynı yazım, farklı sahip: {ad: [(sahip, v0.3 kanıt satırları)]}. Sahip, seam satırının İLK SÜTUNUNDA adla birlikte
# geçmelidir (parantezde ya da `Sahip.ad` biçiminde).
POLYSEMY: dict[str, tuple[tuple[str, tuple[int, ...]], ...]] = {
    "Ok": (("ConnectionState", (97, 166, 222)), ("DeviceRenewalState", (266,))),
    "Offline": (("EnrollmentState", (136, 150)), ("DrainResult", (283,))),
    "Failed": (("DownloadState", (200, 277)), ("DeviceRenewalState", (266, 365))),
    "Unavailable": (("UsernameLookupResult", (261, 335)), ("DevicesApi.refresh", (266, 358))),
    "None": (("RowIndicator", (220,)), ("MessageAlerts", (281,))),
    "Background": (("PlatformSignal", (282,)), ("dosya grubu", (8,))),
    "username": (("SelfIdentity", (288,)), ("PeerIdentity", (73,))),
    "code": (("InviteUnreadable", (130, 156)), ("Offline", (136, 156)), ("TrustValidationFailed", (145, 156)),
             ("TrustFailure", (328,)), ("DeviceRenewalState.Failed", (365,))),
    "id": (("markRead", (367,)), ("attachments.open", (285, 373))),
    "status": (("UserMessage", (270,)), ("Outbound", (220,))),
    "uri": (("AttachmentSource.File", (258,)), ("AttachmentSource.Voice", (344,))),
    "maxBytes": (("AttachmentLimits", (258,)), ("TooLarge", (345,))),
}
K6_ROWS = 12
K6_KEYS = ("Enrollment", "provisioning içe aktar", "Ready", "Contacts = provisioning listesi", "syncDirectory",
           "lookupUsername", "Devices tek satır", "Push", "retryAfterTrustFailure", "Gate-3 bildirimi",
           "background.drain()", "drain_background()", "notifications.state", "attachments.open",
           "onPlatformSignal", "CallsApi")


@dataclass
class Candidates:
    """v0.3 adayları: {ad: [satır]} — kod aralığı/çitten ve düzyazının kod biçimli adlarından."""
    code: dict[str, list[int]] = field(default_factory=dict)
    prose: dict[str, list[int]] = field(default_factory=dict)
    prose_title_only: list[str] = field(default_factory=list)
    prose_lower_only: list[str] = field(default_factory=list)

    def all(self) -> dict[str, list[int]]:
        merged: dict[str, list[int]] = defaultdict(list)
        for source in (self.code, self.prose):
            for name, lines in source.items():
                merged[name] += lines
        return dict(merged)


def code_shaped(name: str) -> bool:
    """Düzyazıda kod biçimli ad: BÜYÜK_HARF (≥ 2), iç büyük harfli, alt çizgili ya da noktalı."""
    return ("." in name or "_" in name or re.fullmatch(r"[A-Z][A-Z0-9_]+", name) is not None
            or re.search(r"[a-z][A-Z]", name) is not None)


def extract(doc: str) -> Candidates:
    code: dict[str, list[int]] = defaultdict(list)
    prose_all: dict[str, list[int]] = defaultdict(list)
    in_fence = False
    for number, line in enumerate(doc.split("\n"), 1):
        if FENCE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            spans, rest = [line], ""
        else:
            spans = CODE_SPAN.findall(line)
            rest = CODE_SPAN.sub(" ", line)
        for span in spans:
            for m in IDENT.finditer(span):
                if m.group(1).isascii():
                    code[m.group(1)].append(number)
        for m in IDENT.finditer(rest):
            if m.group(1).isascii():
                prose_all[m.group(1)].append(number)
    prose = {n: ls for n, ls in prose_all.items() if code_shaped(n) and n not in code}
    only = [n for n in prose_all if n not in code and not code_shaped(n)]
    return Candidates(
        code=dict(code), prose=prose,
        prose_title_only=sorted(n for n in only if re.fullmatch(r"[A-Z][a-z0-9]*", n)),
        prose_lower_only=sorted(n for n in only if not re.fullmatch(r"[A-Z][a-z0-9]*", n)),
    )


@dataclass
class Row:
    section: str
    line: int
    first: str
    names: list[str]
    mapping: str


def split_cells(line: str) -> list[str]:
    return [c.strip() for c in re.split(r"(?<!\\)\|", line.strip())[1:-1]]


def section_rows(seam: str, number: int, header: str) -> list[Row]:
    """`## <number>.` bölümünün tablo satırları (başlık ve ayraç satırları hariç)."""
    lines = seam.split("\n")
    rows: list[Row] = []
    inside = False
    for index, line in enumerate(lines, 1):
        if line.startswith("## "):
            inside = line.startswith(f"## {number}.")
            continue
        if not inside or not line.startswith("|") or re.match(r"^\|\s*-", line):
            continue
        cells = split_cells(line)
        if len(cells) < 2 or cells[0] == header:
            continue
        rows.append(Row(section=str(number), line=index, first=cells[0],
                        names=CODE_SPAN.findall(cells[0]), mapping=cells[1]))
    return rows


def snake_to_camel(name: str) -> str:
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


def block(src: str, opener: str) -> str:
    """`opener` satırından başlayan ve sütun 0'daki `}` ile biten blok (yoksa boş)."""
    start = src.find(opener)
    if start < 0:
        return ""
    end = src.find("\n}", start)
    return src[start:] if end < 0 else src[start:end]


def rust_surface(lib: str, event: str, error: str) -> dict[str, set[str]]:
    """Kotlin adlarıyla Rust yüzeyi: {`KamaMobile`: çağrılar, `EngineEvent`: olaylar, `NodeException`: hatalar}."""
    exported = block(lib, "#[uniffi::export]\nimpl KamaMobile {")
    methods = {snake_to_camel(m) for m in re.findall(r"^\s*pub fn (\w+)", exported, re.M)}
    events = set(re.findall(r"^    ([A-Z]\w*)\b", block(event, "pub enum EngineEvent {"), re.M))
    errors = set(re.findall(r"^    ([A-Z]\w*)\b", block(error, "pub enum NodeError {"), re.M))
    return {"KamaMobile": methods, "EngineEvent": events, "NodeException": errors}


@dataclass
class Result:
    findings: list[str]
    stats: dict[str, int]


def check(doc: str, seam: str, surface: dict[str, set[str]], *, non_items: dict[str, frozenset[str]],
          polysemy: dict[str, tuple[tuple[str, tuple[int, ...]], ...]], k6_rows: int,
          k6_keys: tuple[str, ...]) -> Result:
    """Saf fonksiyon: v0.3 metni + seam metni + Rust yüzeyi + sınıflama tabloları → bulgular ve sayılar."""
    cands = extract(doc)
    candidates = cands.all()
    findings: list[str] = []
    excluded = {name: category for category, names in non_items.items() for name in names}
    for name in sorted(set(excluded) - set(candidates)):
        findings.append(f"BAYAT: `{name}` öğe-dışı tablosunda ama v0.3'te aday değil ({excluded[name]})")
    items = sorted(set(candidates) - set(excluded))
    rows = section_rows(seam, 4, ITEM_HEADER)
    k6 = section_rows(seam, 3, K6_HEADER)
    in_rows: dict[str, list[Row]] = defaultdict(list)
    for row in rows:
        for name in row.names:
            in_rows[name].append(row)
    # (öğe) her öğenin satırı
    for name in items:
        if name not in in_rows:
            where = "düzyazı" if name in cands.prose else "kod"
            findings.append(f"EKSİK: `{name}` (v0.3 satır {candidates[name][:4]}, {where}) §4'te satır yok")
    # (çelişki) öğe-dışı ad satırda
    for name in sorted(set(in_rows) & set(excluded)):
        findings.append(f"ÇELİŞKİ: `{name}` öğe-dışı sınıflanmış ({excluded[name]}) ama §4 satır "
                        f"{in_rows[name][0].line}'de geçiyor")
    # (kimlik) çokanlamlı adların her sahibi
    doc_lines = doc.split("\n")
    identities = 0
    for name, owners in sorted(polysemy.items()):
        for owner, evidence in owners:
            identities += 1
            for number in evidence:
                text = doc_lines[number - 1] if 0 < number <= len(doc_lines) else ""
                # Noktalı biçim de kanıttır (`ConnectionState.Ok`).
                if not re.search(rf"(?<!\w){re.escape(name)}(?!\w)", text):
                    findings.append(f"KANIT: `{name}` ({owner}) v0.3 satır {number}'de geçmiyor")
            dotted = f"{owner}.{name}"
            if not (dotted in in_rows or any(owner in row.first for row in in_rows.get(name, []))):
                findings.append(f"KİMLİK: `{name}` ({owner}) — §4'te adı ve sahibi ilk sütunda birlikte geçen satır yok")
    # (i) uydurma ad, (ii) eşleme biçimi
    for row in rows + k6:
        if row.section == "4":
            for name in row.names:
                if name not in candidates:
                    findings.append(f"YABANCI: `{name}` (seam satır {row.line}) v0.3 adayı değil")
        if row.mapping not in MAPPING_FIXED and not RUST_MAPPING.match(row.mapping):
            findings.append(f"EŞLEME: seam satır {row.line} `{row.mapping[:60]}` üç biçimden biri değil")
    # (iii) Rust yüzeyi atıfları — belgenin her yerinde (kod aralıkları)
    for number, line in enumerate(seam.split("\n"), 1):
        for span in CODE_SPAN.findall(line):
            for owner, member in SURFACE_REF.findall(span):
                if member not in surface.get(owner, set()):
                    findings.append(f"ATIF: `{owner}.{member}` (seam satır {number}) Rust yüzeyinde yok")
    # §3 K6
    if len(k6) != k6_rows:
        findings.append(f"K6: §3'te {len(k6)} satır (beklenen {k6_rows})")
    k6_text = " ".join(row.first for row in k6)
    for key in k6_keys:
        if key not in k6_text:
            findings.append(f"K6: `{key}` §3'ün ilk sütunlarında yok")
    stats = {"aday": len(candidates), "kod adayı": len(cands.code), "düzyazı adayı": len(cands.prose),
             "öğe-dışı": len(excluded), "öğe": len(items), "kimlik": identities, "§4 satır": len(rows),
             "§3 satır": len(k6), "düzyazı Başı-büyük (yalnız)": len(cands.prose_title_only),
             "düzyazı küçük harf (yalnız)": len(cands.prose_lower_only)}
    return Result(findings=findings, stats=stats)


def read(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise EnvironmentError(f"okunamadı: {path} ({error.strerror})") from None


def run(argv: list[str]) -> int:
    def value(flag: str) -> str | None:
        if flag not in argv:
            return None
        i = argv.index(flag)
        if i + 1 >= len(argv):
            raise EnvironmentError(f"{flag} bir değer ister")
        return argv[i + 1]
    ui_api = value("--ui-api")
    if ui_api is None:
        raise EnvironmentError("--ui-api <ui-api v0.3 yolu> gerekli (belge bu depoda değil)")
    root = Path(value("--root") or DEFAULT_ROOT)
    seam_path = Path(value("--seam") or root / SEAM_PATH)
    raw = Path(ui_api).read_bytes() if Path(ui_api).is_file() else None
    if raw is None:
        raise EnvironmentError(f"ui-api belgesi yok: {ui_api}")
    digest = hashlib.sha256(raw).hexdigest()
    if digest != UI_API_SHA256:
        raise EnvironmentError(f"ui-api sha256 {digest} ≠ pinli v0.3 {UI_API_SHA256}")
    doc = raw.decode("utf-8")
    surface = rust_surface(*(read(root / RUST_FILES[k]) for k in ("lib", "event", "error")))
    if not all(surface.values()):
        raise EnvironmentError(f"Rust yüzeyi okunamadı (boş küme): { {k: len(v) for k, v in surface.items()} }")
    result = check(doc, read(seam_path), surface, non_items=NON_ITEMS, polysemy=POLYSEMY, k6_rows=K6_ROWS,
                   k6_keys=K6_KEYS)
    cands = extract(doc)
    findings = list(result.findings)
    for label, got, want in (("Başı-büyük", len(cands.prose_title_only), PROSE_TITLE_ONLY),
                             ("küçük harf", len(cands.prose_lower_only), PROSE_LOWER_ONLY)):
        if got != want:
            findings.append(f"PİN: yalnız düzyazıdaki {label} sözcük sayısı {got} (pinli {want}) — çıkarım değişti, "
                            f"liste yeniden gözden geçirilmeli (--report)")
    if "--report" in argv:
        excluded = {n: c for c, ns in NON_ITEMS.items() for n in ns}
        for name, lines in sorted(cands.all().items()):
            print(f"  {name}\t{excluded.get(name, 'ÖĞE')}\t{lines[:6]}")
        print(f"  yalnız düzyazı Başı-büyük ({len(cands.prose_title_only)}): {' '.join(cands.prose_title_only)}")
        print(f"  yalnız düzyazı küçük harf ({len(cands.prose_lower_only)}): {' '.join(cands.prose_lower_only)}")
    for finding in findings:
        print(f"KIRMIZI: {finding}")
    stats = ", ".join(f"{k} {v}" for k, v in result.stats.items())
    print(f"SEAM-AK KAPSAMA: {'YEŞİL' if not findings else f'KIRMIZI ({len(findings)})'} — {stats}")
    return 1 if findings else 0


# ── öz-test ──────────────────────────────────────────────────────────────────────────────────────
SELF_DOC = """# Örnek v0.3
| Alan | `KamaEngine.self` |
Motor `sendText` çağrısı ve `Ok` durumu (bağlantı). `TASARIM` etiketi.
Cihaz satırı: `Ok` (yenileme).
Ek türleri (FILE, VOICE) düzyazıda.
```
Initializing ──▶ Ready
```
"""
SELF_NON_ITEMS = {"etiket": frozenset({"TASARIM"}), "düzyazı": frozenset({"FILE"})}
SELF_POLYSEMY = {"Ok": (("ConnectionState", (3,)), ("DeviceRenewalState", (4,)))}
SELF_K6_KEYS = ("Ready",)
SELF_SEAM = """# seam
## 3. K6
| K6 maddesi | Eşleme | Not |
|---|---|---|
| Enrollment → `Ready` | Rust: `KamaMobile.start` | x |
## 4. Satırlar
| `ui-api v0.3` öğesi | Eşleme | Not |
|---|---|---|
| `KamaEngine.self` | Kotlin tarafı (Hasan) | x |
| `sendText` | Rust: `KamaMobile.sendText` | `EngineEvent.Receipt`, `NodeException.Stopped` |
| `Ok` (ConnectionState) | Kotlin tarafı (Hasan) | x |
| `Ok` (DeviceRenewalState) | Kotlin tarafı (Hasan) | x |
| `VOICE` | Kotlin tarafı (Hasan) | x |
| `Initializing` · `Ready` | PoC'de etkisiz gövde (K6) | x |
## 5. Yöntem
"""
SELF_SURFACE = {"KamaMobile": {"start", "sendText"}, "EngineEvent": {"Receipt"}, "NodeException": {"Stopped"}}


def selftest() -> int:
    def run_case(doc=SELF_DOC, seam=SELF_SEAM, surface=None, non_items=None, polysemy=None, keys=SELF_K6_KEYS):
        return check(doc, seam, surface or SELF_SURFACE, non_items=non_items or SELF_NON_ITEMS,
                     polysemy=SELF_POLYSEMY if polysemy is None else polysemy, k6_rows=1, k6_keys=keys)
    cases = [
        ("temiz", run_case(), []),
        ("satır silindi (kod öğesi)", run_case(seam=SELF_SEAM.replace("| `sendText` | Rust: `KamaMobile.sendText` | "
                                                                       "`EngineEvent.Receipt`, `NodeException.Stopped` |\n", "")),
         ["EKSİK"]),
        ("düzyazı öğesi (VOICE) satırı yok", run_case(seam=SELF_SEAM.replace("| `VOICE` | Kotlin tarafı (Hasan) | x |\n", "")),
         ["EKSİK"]),
        ("sahip satırı yok (Ok DeviceRenewalState)",
         run_case(seam=SELF_SEAM.replace("| `Ok` (DeviceRenewalState) | Kotlin tarafı (Hasan) | x |\n", "")), ["KİMLİK"]),
        ("sahip noktalı adla kapsanır (`DeviceRenewalState.Ok`)",
         run_case(doc=SELF_DOC + "Değer `DeviceRenewalState.Ok`.\n",
                  seam=SELF_SEAM.replace("| `Ok` (DeviceRenewalState) |", "| `DeviceRenewalState.Ok` |")), []),
        ("çokanlamlı adın kanıtı yanlış satırda", run_case(polysemy={"Ok": (("ConnectionState", (5,)),)}), ["KANIT"]),
        ("geçersiz eşleme", run_case(seam=SELF_SEAM.replace("| `VOICE` | Kotlin tarafı (Hasan)", "| `VOICE` | Kotlin")),
         ["EŞLEME"]),
        ("Rust eşlemesi KamaMobile/EngineEvent ile başlamıyor",
         run_case(seam=SELF_SEAM.replace("| `VOICE` | Kotlin tarafı (Hasan)", "| `VOICE` | Rust: `NodeMetrics`")),
         ["EŞLEME"]),
        ("uydurma ad", run_case(seam=SELF_SEAM.replace("| `VOICE` |", "| `VOICE` · `Uydurma` |")), ["YABANCI"]),
        ("yok olan Rust çağrısı", run_case(surface={**SELF_SURFACE, "KamaMobile": {"start"}}), ["ATIF"]),
        ("yok olan olay atfı (Not sütununda)", run_case(surface={**SELF_SURFACE, "EngineEvent": set()}), ["ATIF"]),
        ("öğe-dışı ad satırda", run_case(non_items={**SELF_NON_ITEMS, "x": frozenset({"VOICE"})}), ["ÇELİŞKİ"]),
        ("bayat öğe-dışı tablosu", run_case(non_items={**SELF_NON_ITEMS, "x": frozenset({"Hayalet"})}), ["BAYAT"]),
        ("düzyazı adayı sınıfsız kalırsa öğe olur (satırı yoksa EKSİK)",
         run_case(doc=SELF_DOC + "Yeni AT_SERVER değeri.\n"), ["EKSİK"]),
        ("K6 anahtarı yok", run_case(keys=("Ready", "CallsApi")), ["K6"]),
    ]
    bad = 0
    for name, result, want in cases:
        got = sorted({f.split(":")[0] for f in result.findings})
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
        if not ok:
            for finding in result.findings:
                print(f"        {finding}")
    # Çıkarım: kod aralığı, çit ve düzyazının kod biçimli adı alınır; düzyazının düz sözcüğü alınmaz.
    cands = extract(SELF_DOC)
    for name, present in (("sendText", True), ("Initializing", True), ("VOICE", True), ("FILE", True),
                          ("Motor", False), ("durumu", False)):
        ok = (name in cands.all()) == present
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  çıkarım: `{name}` {'aday' if present else 'aday değil'}")
    # Rust yüzeyi ayrıştırması: dışa açılan impl bloğu, olay ve hata varyantları.
    lib = ("#[uniffi::export]\nimpl KamaMobile {\n    pub fn send_text(&self) {}\n    fn hidden(&self) {}\n}\n"
           "impl KamaMobile {\n    pub fn with_node(&self) {}\n}\n")
    event = "pub enum EngineEvent {\n    /// x\n    Receipt {\n        peer_id: String,\n    },\n}\n"
    error = "pub enum NodeError {\n    #[error(\"x\")]\n    Stopped,\n}\n"
    surface = rust_surface(lib, event, error)
    want = {"KamaMobile": {"sendText"}, "EngineEvent": {"Receipt"}, "NodeException": {"Stopped"}}
    ok = surface == want
    bad += not ok
    print(f"  {'PASS' if ok else 'FAIL'}  Rust yüzeyi: beklenen {want}, gelen {surface}")
    print(f"seam_ak_coverage öz-test: {'YEŞİL' if not bad else f'{bad} BAŞARISIZ'}")
    return 1 if bad else 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()
    try:
        return run(argv)
    except EnvironmentError as error:
        print(f"ORTAM HATASI: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
