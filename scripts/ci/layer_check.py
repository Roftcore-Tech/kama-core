#!/usr/bin/env python3
"""Katman kuralı kapısı (C1.2) — INV-NEIGHBOR-1'in bağımlılık grafiğindeki karşılığı.

    scripts/ci/layer_check.py              # cargo metadata + kaynak taraması
    scripts/ci/layer_check.py --root DİZİN # başka bir çalışma ağacını denetle (varsayılan: bu script'in deposu)
    scripts/ci/layer_check.py --selftest   # kuralların HER BİRİNİN gerçekten ateşlediğini kanıtlar

KURALLAR (transport-layer-tdd §0 INV-NEIGHBOR-1; KM-SEAM-AT-001 §6 H-NO-CRYPTO-EDGE / H-NO-PLANE1;
KM-SEAM-TC-001 §6 H-SPLIT):

  L1  Application → Crypto DOĞRUDAN kenarı YASAK.
      `kama-app`'in Cargo.toml'unda (normal/build/dev) şunlar olamaz: kripto katmanı crate'leri
      (kama-noise-wire, crypto, ccr-codec) ve ham kripto primitifleri.
  L2  Crypto → Application kenarı YASAK (doğrudan YA DA geçişli).
      Kripto katmanı crate'lerinin bağımlılık kapanışında `kama-app` olamaz.
  L3  Application kaynağında Plane 1 sembolü YASAK: `AppRpcFrame`, `method_id`.
      Yorumlar ve doc-comment'ler taranmaz (yasağı ADLANDIRMAK serbest).

  [B1 2026-10-05 — sözleşme crate'i `kama-contracts` ile gelen kurallar]
  C1  `kama-contracts`'ın bağımlılığı YOKTUR: ne workspace crate'i ne harici crate; normal, dev,
      build fark etmez. Sözleşme hiçbir katmana (ve hiçbir kütüphaneye) yaslanmaz.
  C2  `kama-app` için YASAK (doğrudan VE geçişli): kama-noise-wire, crypto, ccr-codec, client-core,
      kama-engine, adaptive-transport, kama-fec-codec, kama-transport-schema.
      (B1'den önce "App → Transport → Crypto geçişli yolu meşrudur" deniyordu; artık Application
      Transport'un crate'ine DEĞİL sözleşmeye bağlanır, geçişli grafiğinde hiçbir alt katman yoktur.)
  C3  `kama-noise-wire` için YASAK (doğrudan VE geçişli): kama-app, client-core.
  C4  `client-core` için YASAK (doğrudan VE geçişli): kama-app.
  C5  `kama-app`, `kama-contracts`'ın `test-util` özelliğini AÇAMAZ (hiçbir bağımlılık türünde) ve
      kaynağında sahteler için açılan ham erişimleri (`*_for_mock`) ÇAĞIRAMAZ.

  GEÇİŞLİ KAPANIŞ NASIL HESAPLANIR: `cargo metadata`'nın çözümlenmiş grafiği üzerinden. Kök crate'in
  KENDİ dev-bağımlılıkları dahildir (testleri de katman kuralına tabidir); yol üzerindeki diğer
  crate'lerin dev-bağımlılıkları dahil DEĞİLDİR (onlar kökün derleme grafiğine girmez).

VAKUM KORUMASI: denetlenen crate'lerden biri (`kama-app`, `kama-contracts`, `kama-noise-wire`,
`client-core`) metadata'da yoksa ya da `kama-app` için kaynak dosyası bulunamazsa kapı YEŞİL dönmez,
ortam hatası (2) verir — "denetlenecek şey yok" sessizce "ihlal yok" sayılmaz.

Çıkış: 0 = temiz · 1 = katman ihlali · 2 = ortam/kullanım hatası.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

# Varsayılan kök script'in kendi deposudur; `--root` ile başka bir çalışma ağacı (ör. bir worktree)
# denetlenir. Kök script konumuna SABİT olduğunda başka ağacı ölçtüğünü sanan koşu sessizce kendi
# ağacını ölçüyordu (2026-10-05'te feature/app envanterinde yaşandı).
DEFAULT_ROOT = Path(__file__).resolve().parents[2]

APP = "kama-app"
CRYPTO_LAYER = ("kama-noise-wire", "crypto", "ccr-codec")
RAW_CRYPTO = (
    "chacha20poly1305", "chacha20", "poly1305", "aead", "blake2", "hkdf", "hmac", "sha2",
    "x25519-dalek", "ed25519-dalek", "curve25519-dalek", "sodiumoxide", "libsodium-sys", "ring",
)
FORBIDDEN_SYMBOLS = ("AppRpcFrame", "method_id")

CONTRACTS = "kama-contracts"
NOISE = "kama-noise-wire"
TRANSPORT = "client-core"
# C2 — Application'ın grafiğine (doğrudan ya da geçişli) giremeyecek crate'ler.
APP_FORBIDDEN = (
    "kama-noise-wire", "crypto", "ccr-codec", "client-core", "kama-engine", "adaptive-transport",
    "kama-fec-codec", "kama-transport-schema",
)
NOISE_FORBIDDEN = (APP, "client-core")      # C3
TRANSPORT_FORBIDDEN = (APP,)                # C4
TEST_UTIL = "test-util"
MOCK_ACCESSOR = re.compile(r"\b\w+_for_mock\b")   # C5 — sahteler için açılan ham erişimler


def strip_comments(src: str) -> str:
    """Rust satır/blok yorumlarını ve doc-comment'leri atar (dizge içlerine dokunmadan, yaklaşık)."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        if c == '"':  # dizge: kapanışa kadar aynen geçir
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i:j + 1])
            i = j + 1
        elif src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            i = n if j < 0 else j + 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def check(metadata: dict, sources: dict[str, str]) -> list[str]:
    """Saf fonksiyon: metadata + {yol: kaynak} → bulgu listesi. Öz-test bunu doğrudan çağırır."""
    findings: list[str] = []
    pkgs = {p["name"]: p for p in metadata["packages"]}
    by_id = {p["id"]: p["name"] for p in metadata["packages"]}

    app = pkgs.get(APP)
    if app is None:
        raise EnvironmentError(f"`{APP}` paketi metadata'da yok — denetlenecek Application crate'i bulunamadı")
    for required in (CONTRACTS, NOISE, TRANSPORT):
        if required not in pkgs:
            raise EnvironmentError(f"`{required}` paketi metadata'da yok — katman kuralı denetlenemiyor")

    # L1 — doğrudan kenar
    banned = set(CRYPTO_LAYER) | set(RAW_CRYPTO)
    for dep in app["dependencies"]:
        if dep["name"] in banned:
            kind = dep.get("kind") or "normal"
            findings.append(f"L1 {APP} → {dep['name']} ({kind} bağımlılık): Application kripto katmanına DOĞRUDAN bağlanamaz")

    # L2 — kripto katmanının kapanışında kama-app
    nodes = {by_id[n["id"]]: [by_id[d] for d in n["dependencies"]] for n in metadata["resolve"]["nodes"]}
    for crate in CRYPTO_LAYER:
        if crate not in nodes:
            continue
        seen, stack, parent = {crate}, [crate], {}
        while stack:
            cur = stack.pop()
            for nxt in nodes.get(cur, []):
                if nxt in seen:
                    continue
                seen.add(nxt)
                parent[nxt] = cur
                stack.append(nxt)
        if APP in seen:
            path, cur = [APP], APP
            while cur != crate:
                cur = parent[cur]
                path.append(cur)
            findings.append(f"L2 {' → '.join(reversed(path))}: kripto katmanı Application'a bağlanamaz")

    # L3 — yasak semboller
    if not sources:
        raise EnvironmentError(f"`{APP}` için taranacak kaynak dosyası yok")
    for path, src in sorted(sources.items()):
        code = strip_comments(src)
        for sym in FORBIDDEN_SYMBOLS:
            for m in re.finditer(rf"\b{re.escape(sym)}\b", code):
                line = code.count("\n", 0, m.start()) + 1
                findings.append(f"L3 {path}: yasak Plane 1 sembolü `{sym}` (yorum-dışı, ~satır {line})")

    # ── B1 kuralları ─────────────────────────────────────────────────────────────────────────
    # C1 — sözleşme crate'inin bağımlılığı yoktur
    for dep in pkgs[CONTRACTS]["dependencies"]:
        kind = dep.get("kind") or "normal"
        findings.append(f"C1 {CONTRACTS} → {dep['name']} ({kind} bağımlılık): sözleşme crate'inin "
                        f"bağımlılığı olamaz (ne workspace ne harici)")

    # C2 / C3 / C4 — geçişli yasaklar
    edges = edge_map(metadata, by_id)
    for rule, root, forbidden in (("C2", APP, APP_FORBIDDEN),
                                  ("C3", NOISE, NOISE_FORBIDDEN),
                                  ("C4", TRANSPORT, TRANSPORT_FORBIDDEN)):
        parent = closure(edges, root)
        for bad in forbidden:
            if bad in parent:
                path, cur = [bad], bad
                while cur != root:
                    cur = parent[cur]
                    path.append(cur)
                how = "doğrudan" if len(path) == 2 else "geçişli"
                findings.append(f"{rule} {' → '.join(reversed(path))}: `{root}` için yasak kenar ({how})")

    # C5 — Application test-util açamaz ve sahte erişimlerini çağıramaz
    for dep in app["dependencies"]:
        if dep["name"] == CONTRACTS and TEST_UTIL in (dep.get("features") or []):
            kind = dep.get("kind") or "normal"
            findings.append(f"C5 {APP} → {CONTRACTS} ({kind} bağımlılık) `{TEST_UTIL}` özelliğini açıyor: "
                            f"Application sahte erişimlerini göremez")
    for path, src in sorted(sources.items()):
        code = strip_comments(src)
        for m in MOCK_ACCESSOR.finditer(code):
            line = code.count("\n", 0, m.start()) + 1
            findings.append(f"C5 {path}: sahte erişimi `{m.group(0)}` çağrılıyor (yorum-dışı, ~satır {line})")
    return findings


def edge_map(metadata: dict, by_id: dict[str, str]) -> dict[str, list[tuple[str, bool]]]:
    """{crate: [(bağımlılık, yalnız_dev_mi), …]} — çözümlenmiş grafikten, kenar türüyle."""
    out: dict[str, list[tuple[str, bool]]] = {}
    for n in metadata["resolve"]["nodes"]:
        name = by_id[n["id"]]
        lst: list[tuple[str, bool]] = []
        if "deps" in n:
            for d in n["deps"]:
                kinds = [k.get("kind") for k in d.get("dep_kinds", [])] or [None]
                lst.append((by_id[d["pkg"]], all(k == "dev" for k in kinds)))
        else:  # `deps` yoksa (eski cargo / öz-test) tür bilinmez: hepsi normal sayılır
            lst = [(by_id[d], False) for d in n["dependencies"]]
        out[name] = lst
    return out


def closure(edges: dict[str, list[tuple[str, bool]]], root: str) -> dict[str, str]:
    """Kökten erişilen crate'ler → {crate: ebeveyn}. Kökün dev-kenarları DAHİL, diğerlerininki DEĞİL."""
    parent: dict[str, str] = {}
    stack = [root]
    while stack:
        cur = stack.pop()
        for nxt, dev_only in edges.get(cur, []):
            if dev_only and cur != root:
                continue
            if nxt == root or nxt in parent:
                continue
            parent[nxt] = cur
            stack.append(nxt)
    return parent


def real_inputs(root: Path) -> tuple[dict, dict[str, str]]:
    if not (root / "Cargo.toml").is_file():
        raise EnvironmentError(f"kök bir Cargo çalışma ağacı değil: {root}")
    r = subprocess.run(["cargo", "metadata", "--format-version", "1", "--locked"],
                       cwd=root, capture_output=True, text=True)
    if r.returncode != 0:
        raise EnvironmentError(f"cargo metadata başarısız: {r.stderr.strip()[:400]}")
    md = json.loads(r.stdout)
    app = next((p for p in md["packages"] if p["name"] == APP), None)
    sources: dict[str, str] = {}
    if app is not None:
        crate_dir = Path(app["manifest_path"]).parent
        for f in sorted(crate_dir.rglob("*.rs")):
            if "target" in f.parts:
                continue
            sources[str(f.relative_to(root))] = f.read_text(encoding="utf-8")
    return md, sources


# ── öz-test ──────────────────────────────────────────────────────────────────────────────────
def _md(deps_of_app, edges, *, dev_edges=None, contract_deps=(), app_contract_features=None):
    """Sentetik metadata. `edges`: normal kenarlar; `dev_edges`: {crate: [dev-bağımlılıklar]}.

    Denetlenen dört crate her zaman vardır (vakum koruması gerçek koşuda ayrıca sınanır).
    """
    dev_edges = dev_edges or {}
    base = {APP, CONTRACTS, NOISE, TRANSPORT}
    names = sorted(base | set(edges) | set(dev_edges)
                   | {d for ds in edges.values() for d in ds}
                   | {d for ds in dev_edges.values() for d in ds}
                   | {d[0] for d in deps_of_app} | set(contract_deps))
    def manifest_deps(n):
        if n == APP:
            out = [{"name": d, "kind": k} for d, k in deps_of_app]
            if app_contract_features is not None:
                kind, feats = app_contract_features
                out.append({"name": CONTRACTS, "kind": kind, "features": list(feats)})
            return out
        if n == CONTRACTS:
            return [{"name": d, "kind": None} for d in contract_deps]
        return []
    def node(n):
        deps = [{"pkg": f"id:{d}", "dep_kinds": [{"kind": None}]} for d in edges.get(n, [])]
        deps += [{"pkg": f"id:{d}", "dep_kinds": [{"kind": "dev"}]} for d in dev_edges.get(n, [])]
        return {"id": f"id:{n}", "deps": deps,
                "dependencies": [f"id:{d}" for d in [*edges.get(n, []), *dev_edges.get(n, [])]]}
    return {
        "packages": [{"name": n, "id": f"id:{n}", "dependencies": manifest_deps(n)} for n in names],
        "resolve": {"nodes": [node(n) for n in names]},
    }


def selftest() -> int:
    clean_src = {"crates/kama-app/src/lib.rs": "pub fn f() {}\n// AppRpcFrame ve method_id YASAK\n/// method_id\n"}
    ok_graph = {APP: [CONTRACTS], TRANSPORT: [CONTRACTS, NOISE], NOISE: [CONTRACTS]}
    cases = [
        ("temiz ağaç (hedef grafik)", _md([(CONTRACTS, None)], ok_graph), clean_src, []),
        ("L1 doğrudan kama-noise-wire", _md([("kama-noise-wire", None)], {APP: ["kama-noise-wire"]}), clean_src, ["C2", "L1"]),
        ("L1 dev-bağımlılık ccr-codec", _md([("ccr-codec", "dev")], {}, dev_edges={APP: ["ccr-codec"]}), clean_src, ["C2", "L1"]),
        ("L1 ham primitif (hkdf)", _md([("hkdf", None)], {APP: ["hkdf"]}), clean_src, ["L1"]),
        ("L2 doğrudan kama-noise-wire → kama-app", _md([], {"kama-noise-wire": [APP]}), clean_src, ["C3", "L2"]),
        ("L2 geçişli ccr-codec → x → kama-app", _md([], {"ccr-codec": ["x"], "x": [APP]}), clean_src, ["L2"]),
        ("L3 AppRpcFrame kodda", _md([], {}), {"a.rs": "struct AppRpcFrame;\n"}, ["L3"]),
        ("L3 method_id kodda", _md([], {}), {"a.rs": "fn f(method_id: u8) {}\n"}, ["L3"]),
        ("L3 DEĞİL: yalnız yorumda", _md([], {}), {"a.rs": "/* method_id */ fn f() {}\n"}, []),
        # C1
        ("C1 sözleşme → harici crate", _md([], ok_graph, contract_deps=("thiserror",)), clean_src, ["C1"]),
        ("C1 sözleşme → workspace crate'i", _md([], ok_graph, contract_deps=(TRANSPORT,)), clean_src, ["C1"]),
        # C2
        ("C2 doğrudan app → client-core", _md([("client-core", None)], {APP: ["client-core"]}), clean_src, ["C2"]),
        ("C2 geçişli app → y → kama-engine", _md([("y", None)], {APP: ["y"], "y": ["kama-engine"]}), clean_src, ["C2"]),
        ("C2 app'in dev-bağımlılığı üzerinden adaptive-transport",
         _md([("z", "dev")], {"z": ["adaptive-transport"]}, dev_edges={APP: ["z"]}), clean_src, ["C2"]),
        ("C2 DEĞİL: yoldaki crate'in dev-bağımlılığı (app'in grafiğine girmez)",
         _md([(CONTRACTS, None)], {APP: [CONTRACTS, "w"]}, dev_edges={"w": ["kama-fec-codec"]}), clean_src, []),
        # C3
        ("C3 kama-noise-wire → client-core (dev)", _md([], {}, dev_edges={NOISE: [TRANSPORT]}), clean_src, ["C3"]),
        ("C3 geçişli kama-noise-wire → q → client-core", _md([], {NOISE: ["q"], "q": [TRANSPORT]}), clean_src, ["C3"]),
        ("C3 DEĞİL: client-core → kama-noise-wire (meşru yön)", _md([], {TRANSPORT: [NOISE]}), clean_src, []),
        # C4
        ("C4 client-core → kama-app", _md([], {TRANSPORT: [APP]}), clean_src, ["C4"]),
        ("C4 geçişli client-core → r → kama-app", _md([], {TRANSPORT: ["r"], "r": [APP]}), clean_src, ["C4"]),
        # C5
        ("C5 app test-util açıyor (normal)", _md([], ok_graph, app_contract_features=(None, ["test-util"])), clean_src, ["C5"]),
        ("C5 app test-util açıyor (dev)", _md([], ok_graph, app_contract_features=("dev", ["test-util"])), clean_src, ["C5"]),
        ("C5 DEĞİL: app sözleşmeye özelliksiz bağlı", _md([], ok_graph, app_contract_features=(None, [])), clean_src, []),
        ("C5 app kaynağında sahte erişimi", _md([], ok_graph), {"a.rs": "fn f(h: H) -> u64 { h.to_raw_for_mock() }\n"}, ["C5"]),
        ("C5 DEĞİL: sahte erişimi yalnız yorumda", _md([], ok_graph), {"a.rs": "// to_raw_for_mock YASAK\nfn f() {}\n"}, []),
    ]
    bad = 0
    for name, md, src, want in cases:
        got = sorted({f.split()[0] for f in check(md, src)})
        ok = got == sorted(want)
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    def without(md, name):
        gone = f"id:{name}"
        nodes = [{"id": n["id"],
                  "deps": [d for d in n["deps"] if d["pkg"] != gone],
                  "dependencies": [d for d in n["dependencies"] if d != gone]}
                 for n in md["resolve"]["nodes"] if n["id"] != gone]
        return {"packages": [p for p in md["packages"] if p["name"] != name], "resolve": {"nodes": nodes}}
    full = _md([], ok_graph)
    vacuum = [("vakum: kama-app yok", {"packages": [], "resolve": {"nodes": []}}, clean_src),
              ("vakum: kaynak yok", full, {})]
    vacuum += [(f"vakum: {n} yok", without(full, n), clean_src) for n in (CONTRACTS, NOISE, TRANSPORT)]
    for name, md, src in vacuum:
        try:
            check(md, src)
            print(f"  FAIL  {name}: ortam hatası bekleniyordu, yeşil döndü")
            bad += 1
        except EnvironmentError:
            print(f"  PASS  {name}: ortam hatası (yeşil DEĞİL)")
    print(f"layer_check öz-test: {'YEŞİL' if not bad else f'{bad} BAŞARISIZ'}")
    return 1 if bad else 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()
    root = DEFAULT_ROOT
    if "--root" in argv:
        i = argv.index("--root")
        if i + 1 >= len(argv):
            print("KULLANIM HATASI: --root bir dizin ister", file=sys.stderr)
            return 2
        root = Path(argv[i + 1]).resolve()
    try:
        md, sources = real_inputs(root)
        findings = check(md, sources)
    except EnvironmentError as e:
        print(f"ORTAM HATASI: {e}", file=sys.stderr)
        return 2
    for f in findings:
        print(f"::error title=LAYER-VIOLATION::{f}")
        print(f"KIRMIZI: {f}")
    if findings:
        print(f"\nLAYER-CHECK: KIRMIZI ({len(findings)} ihlal) — INV-NEIGHBOR-1")
        return 1
    print(f"LAYER-CHECK: YEŞİL ({APP}: {len(sources)} kaynak dosyası; L1/L2/L3 + C1–C5 temiz; kök={root})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
