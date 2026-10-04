#!/usr/bin/env python3
"""Katman kuralı kapısı (C1.2) — INV-NEIGHBOR-1'in bağımlılık grafiğindeki karşılığı.

    scripts/ci/layer_check.py              # cargo metadata + kaynak taraması
    scripts/ci/layer_check.py --root DİZİN # başka bir çalışma ağacını denetle (varsayılan: bu script'in deposu)
    scripts/ci/layer_check.py --selftest   # kuralların HER BİRİNİN gerçekten ateşlediğini kanıtlar

KURALLAR (transport-layer-tdd §0 INV-NEIGHBOR-1; KM-SEAM-AT-001 §6 H-NO-CRYPTO-EDGE / H-NO-PLANE1):

  L1  Application → Crypto DOĞRUDAN kenarı YASAK.
      `kama-app`'in Cargo.toml'unda (normal/build/dev) şunlar olamaz: kripto katmanı crate'leri
      (kama-noise-wire, crypto, ccr-codec) ve ham kripto primitifleri.
      DOĞRUDAN bağımlılık denetlenir, geçişli değil: App → Transport → Crypto meşru yoldur ve
      Transport'a bağlanan Application'ın geçişli grafiğinde kripto ZATEN bulunur.
  L2  Crypto → Application kenarı YASAK (doğrudan YA DA geçişli).
      Kripto katmanı crate'lerinin bağımlılık kapanışında `kama-app` olamaz.
  L3  Application kaynağında Plane 1 sembolü YASAK: `AppRpcFrame`, `method_id`.
      Yorumlar ve doc-comment'ler taranmaz (yasağı ADLANDIRMAK serbest).

VAKUM KORUMASI: `kama-app` paketi metadata'da yoksa ya da kaynak dosyası bulunamazsa kapı YEŞİL
dönmez, ortam hatası (2) verir — "denetlenecek şey yok" sessizce "ihlal yok" sayılmaz.

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
    return findings


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
def _md(deps_of_app, edges):
    names = sorted({APP, *edges, *(d for ds in edges.values() for d in ds), *(d[0] for d in deps_of_app)})
    return {
        "packages": [{"name": n, "id": f"id:{n}",
                      "dependencies": [{"name": d, "kind": k} for d, k in deps_of_app] if n == APP else []}
                     for n in names],
        "resolve": {"nodes": [{"id": f"id:{n}", "dependencies": [f"id:{d}" for d in edges.get(n, [])]}
                              for n in names]},
    }


def selftest() -> int:
    clean_src = {"crates/kama-app/src/lib.rs": "pub fn f() {}\n// AppRpcFrame ve method_id YASAK\n/// method_id\n"}
    cases = [
        ("temiz ağaç", _md([("thiserror", None)], {APP: ["thiserror"]}), clean_src, []),
        ("L1 doğrudan kama-noise-wire", _md([("kama-noise-wire", None)], {APP: ["kama-noise-wire"]}), clean_src, ["L1"]),
        ("L1 dev-bağımlılık ccr-codec", _md([("ccr-codec", "dev")], {APP: ["ccr-codec"]}), clean_src, ["L1"]),
        ("L1 ham primitif (hkdf)", _md([("hkdf", None)], {APP: ["hkdf"]}), clean_src, ["L1"]),
        ("L1 DEĞİL: geçişli (app → client-core → kama-noise-wire)",
         _md([("client-core", None)], {APP: ["client-core"], "client-core": ["kama-noise-wire"]}), clean_src, []),
        ("L2 doğrudan kama-noise-wire → kama-app", _md([], {"kama-noise-wire": [APP]}), clean_src, ["L2"]),
        ("L2 geçişli ccr-codec → x → kama-app", _md([], {"ccr-codec": ["x"], "x": [APP]}), clean_src, ["L2"]),
        ("L3 AppRpcFrame kodda", _md([], {}), {"a.rs": "struct AppRpcFrame;\n"}, ["L3"]),
        ("L3 method_id kodda", _md([], {}), {"a.rs": "fn f(method_id: u8) {}\n"}, ["L3"]),
        ("L3 DEĞİL: yalnız yorumda/dizgede-olmayan", _md([], {}), {"a.rs": "/* method_id */ fn f() {}\n"}, []),
    ]
    bad = 0
    for name, md, src, want in cases:
        got = sorted({f.split()[0] for f in check(md, src)})
        ok = got == sorted(want)
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    for name, md, src in (("vakum: kama-app yok", {"packages": [], "resolve": {"nodes": []}}, clean_src),
                          ("vakum: kaynak yok", _md([], {}), {})):
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
    print(f"LAYER-CHECK: YEŞİL ({APP}: {len(sources)} kaynak dosyası; L1/L2/L3 temiz; kök={root})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
