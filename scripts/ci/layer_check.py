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
      [2026-10-08 — Abkarada kararı] TEK DAR İSTİSNA: `blake2`, ADIYLA ve YALNIZ normal bağımlılık
      olarak; kapsamı YALNIZ `FILE_BEGIN.whole_file_hash` (BLAKE2s-256, AMP §3 0x02;
      `crates/kama-app/src/file_transfer.rs` `whole_file_hash`). dev/build bağımlılığı olarak ya da
      başka herhangi bir primitif / kripto katmanı crate'i için istisna YOKTUR.
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
  C5  `kama-app`, `kama-contracts`'ın `test-util` özelliğini AÇAMAZ — ne bağımlılık satırında (hiçbir
      bağımlılık türünde) ne de kendi `[features]` tablosundan ileterek (`x = ["kama-contracts/test-util"]`;
      bağımlılık yeniden adlandırılmışsa yerel adıyla: `x = ["contracts/test-util"]`), ne de `test-util`'i
      dolaylı açan bir sözleşme özelliği üzerinden (`mocks = ["test-util"]`, `default = ["test-util"]`)
      — ve kaynağında sahteler için açılan ham erişimleri (`*_for_mock`) ÇAĞIRAMAZ.
      SINIR: `kama-app`'in grafiğindeki BAŞKA bir crate'in aynı özelliği açması (özellik birleşimi)
      buradan görülmez; o yol için C2 katman crate'lerini zaten yasaklar.

  [2026-10-06 F07.2]
  C6  `kama-app` kaynağında (yorum ve dizge/char sabiti DIŞI) `as_bytes` adı GEÇEMEZ (sembol kuralı,
      `\bas_bytes\b`): `PeerRef::as_bytes` (ham `dest_hash`) Application'da ÇAĞRILAMAZ. Tip bilgisi
      olmadan alıcı ayırt edilemez; kural MUHAFAZAKÂRDIR — `PeerRef::as_bytes`'ın her çağrı biçimi
      (yöntem, UFCS, fonksiyon işaretçisi) bu adı taşır; bedeli: kama-app'te `str`/`String`
      baytları için de `as_bytes` kullanılamaz → `AsRef<[u8]>::as_ref`, `str::bytes`,
      `String::into_bytes`. Başka adlar (`as_bytes_len`, `my_as_bytes2`) kurala girmez.

  [2026-10-11 K4G-06 — ALTYAPI sınıfı; plan D8 v1.1]
  C7  Altyapı crate'ine (`INFRA`: `kama-store`) workspace'te YALNIZ bileşim kökü (`COMPOSITION_ROOTS`)
      bağımlı olabilir — normal, dev, build fark etmez. Bileşim kökü listesi: `kama-node`
      ([2026-10-12 K4G-11]; K4G-06'da boştu). Doğrudan kenar denetlenir; `A → B → kama-store`
      zincirinde doğrudan bağımlı `B` yakalanır. Altyapı crate'inin KENDİ bağımlılıkları (trait sahipleri `kama-app`,
      `kama-noise-wire`) bu kuralın konusu değildir; o yön diğer kurallarla sınırlıdır.

  [2026-10-12 K4G-11 — BİLEŞİM KÖKÜ sınıfı; INV-NEIGHBOR-1'in TEK istisnası, plan K4G-11 NE 3]
  C8  Bileşim kökü (`ROOT_WIRING`: `kama-node`) her katmana bağlanabilir ama YALNIZ kurar, protokol
      mantığı taşımaz. "Basit grep kuralı": Crypto katmanının Rust adı (`\bkama_noise_wire\b`) kökün
      ÜRÜN kaynağında (`src/**/*.rs`; yorum ve dizge/char sabiti dışı) yalnız KURMA MODÜLÜNDE
      (`src/wiring.rs`) geçebilir — `use` bildirimi dahil. Çalışma zamanında Crypto'yu Transport
      sürer; kök yalnız Application motorlarını sürer. Testler (`tests/`) ürün kaynağı değildir,
      kapsam dışıdır.
      SINIR: ad düzeyidir — kurma modülünün döndürdüğü bir değer üzerinde yöntem çağrısı (adsız)
      görülmez; o yol inceleme işidir.

  [2026-10-16 K4G-15 — FFI SARMALAYICI sınıfı; plan D10. `kama-app` / `kama-contracts` kenarları:
   DUR-K4G-15-A CEVAP (CEVAP PAKETİ 5, Abkarada 2026-10-16): "kalsın … koşul: C9 katman kuralı bu
   bağımlılığı yalnız tip dönüşümüne sınırlar (davranış çağrısı yok …), layer_check'te denetlenir"]
  C9  FFI sarmalayıcısı (`FFI_WRAPPER`: `kama-mobile`, UniFFI yüzeyi) bileşim kökünün API'sini
      birebir sarar, protokol mantığı taşımaz. Üç denetim:
      (a) Workspace içi DOĞRUDAN bağımlılıkları YALNIZ `FFI_ALLOWED`'dakiler ve YALNIZ izinli türde —
          `kama-node` (sarılan API; normal), `kama-app` ve `kama-contracts` (YALNIZ tip dönüşümü:
          `KamaNode` imzalarındaki tiplerin FFI aynasına eşlenmesi; normal), `kama-provision` (yalnız
          dev: smoke testi cihaz dosyalarını CLI ile üretir). Transport, Crypto ve altyapı crate'leri
          (client-core, kama-noise-wire, kama-store …) hiçbir türde olamaz; build-bağımlılığı olamaz.
          Workspace içi bağımlılık = manifest'te kaynağı (`source`) olmayan, yol bağımlılığı.
          Harici crate'ler (`uniffi`, `thiserror`) bu denetimin konusu değildir (D11: cargo deny +
          inceleme).
      (b) Normal ve build bağımlılıkları (workspace ya da harici) `test-util` özelliğini AÇAMAZ: ne
          bağımlılık satırında, ne `test-util`'i dolaylı açan bir özellik ya da varsayılan özellik
          üzerinden, ne de crate'in kendi `[features]` tablosundan iletilerek
          (`x = ["kama-app/test-util"]`, zayıf biçim `kama-app?/test-util`, yeniden adlandırılmış
          bağımlılığın yerel adıyla). Dev bağımlılıkları serbesttir (ürün `.so`'suna girmez).
      (c) Kaynakta (`crates/kama-mobile/**/*.rs`; yorum ve dizge/char sabiti DIŞI) `kama_app::` /
          `kama_contracts::` yolları YALNIZ TİP adı taşır: kökten sonra modül segmentleri, sonra
          YALNIZ CamelCase tip/varyant adları (`kama_app::ptt::PttError::Transport`). Tip adı
          olmayan yol (fonksiyon, sabit, makro, modül: `kama_app::ptt::max_bundle_payload_bytes`),
          tip üzerinde küçük harfli ya da BÜYÜK_HARFLİ segment (ilişkili fonksiyon/sabit:
          `PttCodec::from_byte`), turbofish arkasındaki aynısı ve nitelikli yol kuyruğu
          (`<kama_app::X as T>::f`) YASAK. `use` içinde de yalnız tip: crate/modül içe aktarımı
          (`use kama_app as app;`, `use kama_app::codec;`), glob (`::*`), `self` ve tip olmayan ad
          YASAK; `extern crate` YASAK. `use … as Takma` ile içe aktarılan tipin takma adı da aynı
          kurala tabidir (`Takma::from_byte` YASAK, `Takma::Varyant` serbest) — takma ad dosya
          kapsamında izlenir.
      SINIR: doğrudan kenar denetlenir; `kama-node` her katmana bağlandığı için geçişli kapanış
      (bileşim kökünün kapanışı) kuralın konusu değildir. (c) ad düzeyidir: değer üzerinde yöntem
      çağrısı (`id.get()`, `x.to_string()`), `Self::` üzerinden yabancı tipin ilişkili öğesi ve trait
      uygulamaları görülmez — o yol inceleme işidir.

  GEÇİŞLİ KAPANIŞ NASIL HESAPLANIR: `cargo metadata`'nın çözümlenmiş grafiği üzerinden. Kök crate'in
  KENDİ dev-bağımlılıkları dahildir (testleri de katman kuralına tabidir); yol üzerindeki diğer
  crate'lerin dev-bağımlılıkları dahil DEĞİLDİR (onlar kökün derleme grafiğine girmez).

VAKUM KORUMASI: denetlenen crate'lerden biri (`kama-app`, `kama-contracts`, `kama-noise-wire`,
`client-core`) metadata'da yoksa ya da `kama-app` için kaynak dosyası bulunamazsa kapı YEŞİL dönmez,
ortam hatası (2) verir — "denetlenecek şey yok" sessizce "ihlal yok" sayılmaz.
[2026-10-11 K4G-06] Altyapı crate'i (`kama-store`, C7) metadata'da yoksa da ortam hatası (2): crate
yeniden adlandırılırsa kural sessizce boşa düşmez.
[2026-10-12 K4G-11] Bileşim kökü (`kama-node`, C8) metadata'da yoksa, ürün kaynağı bulunamazsa ya da
kurma modülü (`src/wiring.rs`) yoksa da ortam hatası (2).
[2026-10-16 K4G-15] FFI sarmalayıcısı (`kama-mobile`, C9) metadata'da yoksa ya da taranacak kaynağı
yoksa da ortam hatası (2).

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
# L1 DAR İSTİSNA [2026-10-08 — Abkarada: "blake2 → kama-app DAR istisna ONAYLANDI (yalnız
# FILE_BEGIN.whole_file_hash; layer_check L1'de adıyla)"]. {ad: izinli TEK bağımlılık türü}. Kapsam:
# `crates/kama-app/src/file_transfer.rs` `whole_file_hash` (BLAKE2s-256, AMP §3 0x02). RAW_CRYPTO'nun
# geri kalanı ve CRYPTO_LAYER L1'de yasak kalır; istisna dev/build türüne uzanmaz.
APP_RAW_CRYPTO_EXCEPTION = {"blake2": "normal"}
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
RAW_PEER_BYTES = re.compile(r"\bas_bytes\b")      # C6 — `PeerRef::as_bytes` (muhafazakâr: ad düzeyi)
# C6 — char sabiti: `'x'`, `'\n'`, `'\x41'`, `'\u{1F600}'`, `'\''`. Eşleşmeyen `'` ömür/etikettir (`'a`).
CHAR_LITERAL = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F]{1,6}\}|x[0-9a-fA-F]{2}|.)|[^\\'\n])'")
# C7 [2026-10-11 K4G-06] — altyapı sınıfı: bu crate'lere workspace'te YALNIZ bileşim kökleri bağımlı
# olabilir. [2026-10-12 K4G-11] Bileşim kökü: `kama-node` (K4G-06'da liste boştu).
INFRA = ("kama-store",)
# C8 [2026-10-12 K4G-11] — bileşim kökü sınıfı: {kök crate: kurma modülü (crate-göreli yol)}. Crypto
# katmanının Rust adı kökün ürün kaynağında yalnız kurma modülünde geçebilir. TEK KAYNAK: C7'nin kök
# listesi buradan türer (iki ayrı liste ayrışamaz).
ROOT_WIRING = {"kama-node": "src/wiring.rs"}
COMPOSITION_ROOTS: tuple[str, ...] = tuple(sorted(ROOT_WIRING))
ROOT_CRYPTO_NAME = re.compile(r"\bkama_noise_wire\b")
# C9 [2026-10-16 K4G-15] — FFI sarmalayıcı sınıfı: {workspace crate'i: izinli bağımlılık türleri}.
# `kama-app` / `kama-contracts`: DUR-K4G-15-A CEVAP (CEVAP PAKETİ 5) — YALNIZ tip dönüşümü (C9 (c)).
FFI_WRAPPER = "kama-mobile"
FFI_ALLOWED: dict[str, frozenset[str]] = {
    "kama-node": frozenset({"normal"}),       # sarılan API (`KamaNode`)
    "kama-app": frozenset({"normal"}),        # YALNIZ tip dönüşümü (`KamaNode` imzalarındaki tipler)
    "kama-contracts": frozenset({"normal"}),  # YALNIZ tip dönüşümü (`KamaNode` olaylarındaki tipler)
    "kama-provision": frozenset({"dev"}),     # smoke testi: cihaz dosyaları CLI ile
}
# C9 (c) — kaynakta yolları YALNIZ tip/varyant adı taşıyabilen crate'ler (Rust adlarıyla).
FFI_TYPE_ONLY = ("kama_app", "kama_contracts")
TYPE_NAME = re.compile(r"[A-Z][A-Za-z0-9]*")          # + en az bir küçük harf (`is_type_name`)
RUST_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
USE_DECL = re.compile(r"(?<![A-Za-z0-9_])use\s+([^;]*);")
USE_TOKEN = re.compile(r"::|[{},*]|[A-Za-z_][A-Za-z0-9_]*")
EXTERN_CRATE = re.compile(r"(?<![A-Za-z0-9_])extern\s+crate\s+([A-Za-z_][A-Za-z0-9_]*)")
EXTERN_CRATE_DECL = re.compile(r"(?<![A-Za-z0-9_])extern\s+crate\s+[^;]*;")
FFI_ROOT = re.compile(rf"(?<![A-Za-z0-9_])({'|'.join(FFI_TYPE_ONLY)})(?![A-Za-z0-9_])")
# Nitelikli yol kuyruğu: yolun hemen ardından `>::öğe` ya da ` as Trait>::öğe` (`<T as Trait>::f`).
QUALIFIED_TAIL = re.compile(r"(?:\s+as\s+[^;{}()]*?)?\s*>\s*::\s*([A-Za-z_][A-Za-z0-9_]*)")


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


def _is_ident(ch: str) -> bool:
    return ch.isalnum() or ch == "_"


def strip_comments_and_literals(src: str) -> str:
    """C6 için: yorumları atar, dizge ve char sabitlerinin İÇİNİ boşlukla karartır (yaklaşık).

    `strip_comments`'ten farkı: dizgeleri KORUMAZ, karartır; ham dizgeleri (`r"…"`, `r#"…"#`,
    `br#"…"#`, `cr"…"`), char sabitlerini (`'"'` dahil — dizge başlangıcı sanılmaz) ve iç içe
    blok yorumları (`/* /* */ */`) tanır. Satır sonları korunur (bulgu satırı kaymaz). Ömürler
    (`'a`) ve etiketler kod olarak geçer.
    """
    out: list[str] = []
    i, n = 0, len(src)

    def blank(seg: str) -> str:
        return "".join(ch if ch == "\n" else " " for ch in seg)

    while i < n:
        c = src[i]
        prev = src[i - 1] if i > 0 else ""
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
        elif src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif src.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(blank(src[i:j]))
            i = j
        elif c in "bcr" and not _is_ident(prev) and (m := re.match(r"(?:b|c)?r(#*)\"", src[i:])):
            # ham dizge: kapanış `"` + aynı sayıda `#`; içinde kaçış yok
            close = '"' + m.group(1)
            k = src.find(close, i + m.end())
            j = n if k < 0 else k + len(close)
            out.append(src[i:i + m.end()] + blank(src[i + m.end():j]))
            i = j
        elif c == '"':
            j = i + 1
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append('"' + blank(src[i + 1:j]) + ('"' if j < n else ""))
            i = j + 1
        elif c == "'" and (m := CHAR_LITERAL.match(src, i)):
            out.append("'" + blank(src[i + 1:m.end() - 1]) + "'")
            i = m.end()
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

    # L1 — doğrudan kenar (tek DAR istisna: APP_RAW_CRYPTO_EXCEPTION, adıyla ve türüyle)
    banned = set(CRYPTO_LAYER) | set(RAW_CRYPTO)
    for dep in app["dependencies"]:
        if dep["name"] in banned:
            kind = dep.get("kind") or "normal"
            if APP_RAW_CRYPTO_EXCEPTION.get(dep["name"]) == kind:
                continue
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
    # `test-util`'i DOLAYLI açan sözleşme özellikleri de sayılır: sözleşme crate'inde `mocks = ["test-util"]`
    # gibi bir takma ad ya da `default = ["test-util"]` tanımlanırsa onu açmak da `test-util`'i açar.
    opens = {TEST_UTIL}
    contract_features = pkgs[CONTRACTS].get("features") or {}
    grew = True
    while grew:
        grew = False
        for feature, enables in contract_features.items():
            if feature not in opens and opens & set(enables):
                opens.add(feature)
                grew = True
    for dep in app["dependencies"]:
        if dep["name"] != CONTRACTS:
            continue
        kind = dep.get("kind") or "normal"
        enabled = set(dep.get("features") or [])
        if dep.get("uses_default_features", True):
            enabled.add("default")
        for feature in sorted(enabled & opens):
            via = "" if feature == TEST_UTIL else f" (`{feature}` üzerinden)"
            findings.append(f"C5 {APP} → {CONTRACTS} ({kind} bağımlılık) `{TEST_UTIL}` özelliğini açıyor{via}: "
                            f"Application sahte erişimlerini göremez")
    # … ya da kendi [features] tablosundan iletiyor: bağımlılık satırında görünmez. Özellik tablosu
    # bağımlılığı manifest'teki YEREL adıyla anar (`contracts = { package = "kama-contracts" }` ise
    # `contracts/test-util`), o yüzden yeniden adlandırmalar da sayılır.
    local_names = {CONTRACTS} | {dep.get("rename") or dep["name"]
                                 for dep in app["dependencies"] if dep["name"] == CONTRACTS}
    forwards = {f"{name}{weak}/{feature}" for name in local_names for weak in ("", "?") for feature in opens}
    for feature, enables in sorted((app.get("features") or {}).items()):
        for item in enables:
            if item in forwards:
                findings.append(f"C5 {APP} [features] `{feature}` → `{item}`: Application `{TEST_UTIL}` "
                                f"özelliğini kendi özellik tablosundan da açamaz")
    for path, src in sorted(sources.items()):
        code = strip_comments(src)
        for m in MOCK_ACCESSOR.finditer(code):
            line = code.count("\n", 0, m.start()) + 1
            findings.append(f"C5 {path}: sahte erişimi `{m.group(0)}` çağrılıyor (yorum-dışı, ~satır {line})")

    # C6 — Application kaynağında `as_bytes` adı geçemez (yorum/dizge/char dışı; muhafazakâr)
    for path, src in sorted(sources.items()):
        code = strip_comments_and_literals(src)
        for m in RAW_PEER_BYTES.finditer(code):
            line = code.count("\n", 0, m.start()) + 1
            findings.append(f"C6 {path}: `as_bytes` adı geçiyor (yorum/dizge-dışı, ~satır {line}) — "
                            f"`PeerRef::as_bytes` Application'da çağrılamaz; kural ad düzeyinde "
                            f"muhafazakârdır (`str`/`String` için `AsRef<[u8]>::as_ref`, `str::bytes`, "
                            f"`String::into_bytes`)")

    # C7 [2026-10-11 K4G-06] — altyapı crate'ine yalnız bileşim kökü bağımlı olabilir
    findings += infra_findings(metadata)
    return findings


def infra_findings(metadata: dict, roots: tuple[str, ...] = COMPOSITION_ROOTS) -> list[str]:
    """C7 — altyapı crate'ine (`INFRA`) yalnız bileşim kökleri (`roots`) bağımlı olabilir.

    Saf fonksiyon: `check()` onu gerçek kök listesiyle çağırır; öz-test istisna yolunu (bugün kök
    yok) sentetik bir kökle doğrudan sınar. Kenarlar çözümlenmiş grafikten, türü ne olursa olsun
    (normal / dev / build) okunur. Altyapı crate'i metadata'da yoksa ortam hatası (vakum koruması).
    """
    by_id = {p["id"]: p["name"] for p in metadata["packages"]}
    for infra in INFRA:
        if infra not in by_id.values():
            raise EnvironmentError(f"`{infra}` paketi metadata'da yok — altyapı kuralı (C7) denetlenemiyor")
    findings: list[str] = []
    for crate, deps in sorted(edge_map(metadata, by_id).items()):
        if crate in roots:
            continue
        for dep, dev_only in deps:
            if dep in INFRA:
                kind = "dev" if dev_only else "normal/build"
                findings.append(f"C7 {crate} → {dep} ({kind} bağımlılık): altyapı crate'ine yalnız bileşim "
                                f"kökü bağımlı olabilir (bileşim kökleri: {', '.join(roots) or 'yok'})")
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


def root_findings(metadata: dict, root_sources: dict[str, dict[str, str]],
                  wiring: dict[str, str] | None = None) -> list[str]:
    """C8 [2026-10-12 K4G-11] — bileşim kökünün ürün kaynağında Crypto adı yalnız kurma modülünde.

    `root_sources` = {kök crate: {crate-göreli yol (`src/…`): kaynak}}; `wiring` = {kök: kurma
    modülü} (varsayılan `ROOT_WIRING`). Saf fonksiyon: `main()` onu gerçek girdiyle, öz-test sentetik
    girdiyle çağırır. Yorumlar ve dizge/char sabitleri taranmaz (`strip_comments_and_literals`;
    yasağı ADLANDIRMAK serbest). Vakum koruması: kök crate metadata'da yoksa, ürün kaynağı yoksa ya
    da kurma modülü yoksa ortam hatası — kural sessizce boşa düşmez.
    """
    wiring = ROOT_WIRING if wiring is None else wiring
    names = {p["name"] for p in metadata["packages"]}
    findings: list[str] = []
    for root, module in sorted(wiring.items()):
        if root not in names:
            raise EnvironmentError(f"`{root}` paketi metadata'da yok — bileşim kökü kuralı (C8) denetlenemiyor")
        sources = root_sources.get(root) or {}
        if not sources:
            raise EnvironmentError(f"`{root}` için taranacak ürün kaynağı yok (C8)")
        if module not in sources:
            raise EnvironmentError(f"`{root}` kurma modülü `{module}` bulunamadı (C8)")
        for path, src in sorted(sources.items()):
            if path == module:
                continue
            code = strip_comments_and_literals(src)
            for m in ROOT_CRYPTO_NAME.finditer(code):
                line = code.count("\n", 0, m.start()) + 1
                findings.append(f"C8 {root}/{path}: Crypto adı `kama_noise_wire` kurma modülü (`{module}`) "
                                f"dışında (yorum/dizge-dışı, ~satır {line}) — bileşim kökü yalnız kurar")
    return findings


def ffi_findings(metadata: dict, sources: dict[str, str]) -> list[str]:
    """C9 [2026-10-16 K4G-15; CEVAP PAKETİ 5] — FFI sarmalayıcısı: (a) workspace içi doğrudan
    bağımlılıklar, (b) normal/build bağımlılıklarında `test-util`, (c) kaynakta `kama_app` /
    `kama_contracts` yolları yalnız tip adı.

    Saf fonksiyon: `main()` onu gerçek metadata ve kaynakla (`ffi_inputs`), öz-test sentetik girdiyle
    çağırır; `sources` = {crate-göreli yol: kaynak}. Workspace içi bağımlılık = manifest
    bağımlılığında `source` yok (yol bağımlılığı). Vakum koruması: FFI crate'i metadata'da yoksa ya da
    taranacak kaynağı yoksa ortam hatası.
    """
    pkg = next((p for p in metadata["packages"] if p["name"] == FFI_WRAPPER), None)
    if pkg is None:
        raise EnvironmentError(f"`{FFI_WRAPPER}` paketi metadata'da yok — FFI kuralı (C9) denetlenemiyor")
    if not sources:
        raise EnvironmentError(f"`{FFI_WRAPPER}` için taranacak kaynak yok (C9)")
    findings: list[str] = []
    # (a) workspace içi doğrudan bağımlılıklar: yalnız izinli crate, yalnız izinli tür
    for dep in pkg["dependencies"]:
        if dep.get("source") is not None:
            continue
        kind = dep.get("kind") or "normal"
        allowed = FFI_ALLOWED.get(dep["name"], frozenset())
        if kind in allowed:
            continue
        rule = (f"yalnız {'/'.join(sorted(allowed))} türünde bağlanabilir" if allowed
                else "hiçbir türde bağlanamaz")
        findings.append(f"C9 {FFI_WRAPPER} → {dep['name']} ({kind} bağımlılık): FFI sarmalayıcısı bu crate'e "
                        f"{rule} (izinli: {', '.join(sorted(FFI_ALLOWED))})")
    # (b) normal/build bağımlılıkları `test-util` açamaz
    findings += ffi_feature_findings(pkg, metadata["packages"])
    # (c) kaynak: `kama_app` / `kama_contracts` yolları yalnız tip adı
    for path, src in sorted(sources.items()):
        findings += ffi_source_findings(f"{FFI_WRAPPER}/{path}", src)
    return findings


def feature_opens(package: dict | None) -> set[str]:
    """`package`'ın `test-util`'i açan özellikleri (kendisi dahil): `mocks = ["test-util"]` gibi takma
    adlar ve `default = ["test-util"]` da sayılır (C5 ile aynı kapanış). Paket metadata'da yoksa yalnız
    `test-util`."""
    opens = {TEST_UTIL}
    features = (package or {}).get("features") or {}
    grew = True
    while grew:
        grew = False
        for feature, enables in features.items():
            if feature not in opens and opens & set(enables):
                opens.add(feature)
                grew = True
    return opens


def ffi_feature_findings(pkg: dict, packages: list[dict]) -> list[str]:
    """C9 (b) — FFI crate'inin normal/build bağımlılıkları `test-util` açamaz: bağımlılık satırında,
    dolaylı açan bir özellikle, varsayılan özellikle ya da crate'in kendi `[features]` tablosundan
    (bağımlılığın manifest'teki yerel adıyla, zayıf `dep?/feat` biçimi dahil). Dev bağımlılıkları
    serbesttir."""
    by_name = {p["name"]: p for p in packages}
    findings: list[str] = []
    local: dict[str, str] = {}  # normal/build bağımlılığın yerel adı → paket adı
    for dep in pkg["dependencies"]:
        kind = dep.get("kind") or "normal"
        if kind == "dev":
            continue
        name = dep["name"]
        local[dep.get("rename") or name] = name
        enabled = set(dep.get("features") or [])
        if dep.get("uses_default_features", True):
            enabled.add("default")
        for feature in sorted(enabled & feature_opens(by_name.get(name))):
            via = "" if feature == TEST_UTIL else f" (`{feature}` üzerinden)"
            findings.append(f"C9 {FFI_WRAPPER} → {name} ({kind} bağımlılık) `{TEST_UTIL}` özelliğini açıyor{via}: "
                            f"FFI sarmalayıcısının normal/build bağımlılıkları test erişimlerini açamaz")
    for feature, enables in sorted((pkg.get("features") or {}).items()):
        for item in enables:
            m = re.fullmatch(r"([^/?]+)\??/(.+)", item)
            if m is None or m.group(1) not in local:
                continue
            if m.group(2) in feature_opens(by_name.get(local[m.group(1)])):
                findings.append(f"C9 {FFI_WRAPPER} [features] `{feature}` → `{item}`: FFI sarmalayıcısı "
                                f"`{TEST_UTIL}`'i kendi özellik tablosundan da açamaz")
    return findings


def is_type_name(segment: str) -> bool:
    """C9 (c) — CamelCase tip/varyant adı: büyük harfle başlar, alt çizgi taşımaz, en az bir küçük harf
    içerir. `snake_case` (fonksiyon, modül, makro) ve `BÜYÜK_HARF` (sabit) tip adı DEĞİLDİR."""
    return TYPE_NAME.fullmatch(segment) is not None and any(ch.islower() for ch in segment)


def path_problem(segments: list[str], *, after_type: bool) -> str | None:
    """C9 (c) — yol kökünden (ya da `after_type`: içe aktarılmış bir tipin adından) sonraki
    segmentlerin sorunu; yoksa None. Kökten sonra önce modüller (küçük harf), sonra YALNIZ
    tip/varyant adları gelir; tip adı olmayan yol da sorundur."""
    first = 0 if after_type else next((i for i, s in enumerate(segments) if is_type_name(s)), None)
    if first is None:
        return "tip adı yok (fonksiyon, sabit, makro ya da modül)"
    bad = next((s for s in segments[first:] if not is_type_name(s)), None)
    if bad is not None:
        return f"tip üzerinde ilişkili öğe `{bad}` (fonksiyon/sabit)"
    return None


def use_leaves(tree: str) -> list[tuple[list[str], str | None]]:
    """`use` ağacını yapraklarına açar: [(yol segmentleri, `as` adı ya da None)]. Gruplar (`{…}`) açılır;
    glob `*` ve `self` yaprağın son segmenti olarak kalır; baştaki `::` atılır."""
    tokens = USE_TOKEN.findall(tree)
    leaves: list[tuple[list[str], str | None]] = []
    pos = 0

    def subtree(prefix: list[str]) -> None:
        nonlocal pos
        path = list(prefix)
        while pos < len(tokens):
            tok = tokens[pos]
            if tok == "::":
                pos += 1
            elif tok == "{":
                pos += 1
                while pos < len(tokens) and tokens[pos] != "}":
                    if tokens[pos] == ",":
                        pos += 1
                    else:
                        subtree(path)
                pos += 1
                return
            elif tok in (",", "}"):
                return
            else:
                path.append(tok)
                pos += 1
                if tok == "*":
                    break
                if pos < len(tokens) and tokens[pos] == "as":
                    alias = tokens[pos + 1] if pos + 1 < len(tokens) else None
                    pos += 2
                    leaves.append((path, alias))
                    return
                if pos < len(tokens) and tokens[pos] == "::":
                    continue
                break
        leaves.append((path, None))

    subtree([])
    return leaves


def skip_space(code: str, i: int) -> int:
    while i < len(code) and code[i].isspace():
        i += 1
    return i


def skip_generics(code: str, i: int) -> int:
    """`code[i] == "<"`: dengeli `<…>`'in sonrasına (`->` içindeki `>` sayılmaz)."""
    depth = 0
    while i < len(code):
        ch = code[i]
        if ch == "<":
            depth += 1
        elif ch == ">" and code[i - 1] != "-":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return i


def path_segments(code: str, i: int) -> tuple[list[str], int]:
    """`code[i:]`'deki `::seg::seg…` kuyruğu → (segmentler, kuyruğun sonu). Boşluk ve satır sonu ile
    turbofish (`::<…>`) atlanır: arkasındaki segment de sayılır."""
    segments: list[str] = []
    while True:
        j = skip_space(code, i)
        if not code.startswith("::", j):
            return segments, i
        j = skip_space(code, j + 2)
        if code.startswith("<", j):
            i = skip_generics(code, j)
            continue
        m = RUST_IDENT.match(code, j)
        if m is None:
            return segments, i
        segments.append(m.group(0))
        i = m.end()


def qualified_problem(code: str, end: int) -> str | None:
    """Yolun hemen ardından nitelikli yol kuyruğu (`<T>::f`, `<T as Trait>::f`) tip olmayan bir öğeye
    iniyorsa sorunu; yoksa None."""
    m = QUALIFIED_TAIL.match(code, end)
    if m is not None and not is_type_name(m.group(1)):
        return f"nitelikli yol üzerinden ilişkili öğe `{m.group(1)}`"
    return None


def blank_matches(code: str, pattern: re.Pattern[str]) -> str:
    """`pattern` eşleşmelerini boşlukla karartır (satır sonları korunur: bulgu satırı kaymaz)."""
    return pattern.sub(lambda m: "".join(ch if ch == "\n" else " " for ch in m.group(0)), code)


def ffi_source_findings(path: str, src: str) -> list[str]:
    """C9 (c) — bir kaynak dosyasında `kama_app` / `kama_contracts` yolları, `use` ve `extern crate`
    bildirimleri ve içe aktarılmış tiplerin takma adları (dosya kapsamında). Yorumlar ve dizge/char
    sabitleri taranmaz (`strip_comments_and_literals`)."""
    code = strip_comments_and_literals(src)
    findings: list[str] = []
    aliases: dict[str, str] = {}  # içe aktarılan tipin yerel adı → tam yolu

    def report(pos: int, shown: str, problem: str) -> None:
        line = code.count("\n", 0, pos) + 1
        findings.append(f"C9 {path}: `{shown}` (yorum/dizge-dışı, ~satır {line}) — {problem}; "
                        f"kama-app/kama-contracts YALNIZ tip dönüşümü için anılır (DUR-K4G-15-A)")

    for m in EXTERN_CRATE.finditer(code):
        if m.group(1) in FFI_TYPE_ONLY:
            report(m.start(), f"extern crate {m.group(1)}", "`extern crate` YASAK")
    for m in USE_DECL.finditer(code):
        for segments, alias in use_leaves(m.group(1)):
            if not segments or segments[0] not in FFI_TYPE_ONLY:
                continue
            rest = segments[1:]
            if not rest:
                problem = "crate içe aktarımı"
            elif rest[-1] == "*":
                problem = "glob içe aktarımı"
            elif rest[-1] == "self":
                problem = "modül içe aktarımı (`self`)"
            else:
                problem = path_problem(rest, after_type=False)
            if problem is not None:
                shown = "use " + "::".join(segments) + (f" as {alias}" if alias else "")
                report(m.start(), shown, f"`use` içinde yalnız tip — {problem}")
            elif alias != "_":
                aliases[alias or rest[-1]] = "::".join(segments)
    # `use` ve `extern crate` bildirimleri yukarıda denetlendi; yol taramasına girmez.
    body = blank_matches(blank_matches(code, USE_DECL), EXTERN_CRATE_DECL)
    for m in FFI_ROOT.finditer(body):
        segments, end = path_segments(body, m.end())
        problem = path_problem(segments, after_type=False) or qualified_problem(body, end)
        if problem is not None:
            report(m.start(), "::".join([m.group(1), *segments]), problem)
    for alias, target in sorted(aliases.items()):
        for m in re.finditer(rf"(?<![A-Za-z0-9_:.]){re.escape(alias)}(?![A-Za-z0-9_])", body):
            segments, end = path_segments(body, m.end())
            problem = path_problem(segments, after_type=True) or qualified_problem(body, end)
            if problem is not None:
                report(m.start(), "::".join([alias, *segments]), f"{problem} (`{alias}` = `{target}`)")
    return findings


def all_findings(md: dict, app_sources: dict[str, str], root_sources: dict[str, dict[str, str]],
                 ffi_sources: dict[str, str]) -> list[str]:
    """Bütün kurallar: L1–L3 + C1–C7 (`check`), C8 (`root_findings`), C9 (`ffi_findings`). `main()` ve
    öz-testin uçtan uca vakaları AYNI yolu çağırır: bir kuralın çağrısı düşerse öz-test kırmızıdır."""
    return check(md, app_sources) + root_findings(md, root_sources) + ffi_findings(md, ffi_sources)


def root_inputs(md: dict) -> dict[str, dict[str, str]]:
    """C8 girdisi: her bileşim kökünün `src/**/*.rs` dosyaları, crate-göreli yolla. Kök metadata'da
    yoksa listeye girmez (`root_findings` ortam hatası verir)."""
    out: dict[str, dict[str, str]] = {}
    for root in ROOT_WIRING:
        pkg = next((p for p in md["packages"] if p["name"] == root), None)
        if pkg is None:
            continue
        crate_dir = Path(pkg["manifest_path"]).parent
        out[root] = {f.relative_to(crate_dir).as_posix(): f.read_text(encoding="utf-8")
                     for f in sorted((crate_dir / "src").rglob("*.rs"))}
    return out


def ffi_inputs(md: dict) -> dict[str, str]:
    """C9 (c) girdisi: FFI crate'inin BÜTÜN `.rs` dosyaları (`src/`, `tests/` …; `target/` hariç),
    crate-göreli yolla. Crate metadata'da yoksa boş (`ffi_findings` ortam hatası verir)."""
    pkg = next((p for p in md["packages"] if p["name"] == FFI_WRAPPER), None)
    if pkg is None:
        return {}
    crate_dir = Path(pkg["manifest_path"]).parent
    out: dict[str, str] = {}
    for f in sorted(crate_dir.rglob("*.rs")):
        rel = f.relative_to(crate_dir)
        if "target" in rel.parts:
            continue
        out[rel.as_posix()] = f.read_text(encoding="utf-8")
    return out


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
def _md(deps_of_app, edges, *, dev_edges=None, contract_deps=(), app_contract_features=None,
        app_contract_rename=None, app_features=None, contract_features=None, app_contract_defaults=True):
    """Sentetik metadata. `edges`: normal kenarlar; `dev_edges`: {crate: [dev-bağımlılıklar]}.

    Denetlenen dört crate her zaman vardır (vakum koruması gerçek koşuda ayrıca sınanır).
    """
    dev_edges = dev_edges or {}
    base = {APP, CONTRACTS, NOISE, TRANSPORT}
    base |= set(INFRA)  # [2026-10-11 K4G-06] C7'nin altyapı crate'i de her zaman vardır
    names = sorted(base | set(edges) | set(dev_edges)
                   | {d for ds in edges.values() for d in ds}
                   | {d for ds in dev_edges.values() for d in ds}
                   | {d[0] for d in deps_of_app} | set(contract_deps))
    def manifest_deps(n):
        if n == APP:
            out = [{"name": d, "kind": k} for d, k in deps_of_app]
            if app_contract_features is not None:
                kind, feats = app_contract_features
                out.append({"name": CONTRACTS, "kind": kind, "features": list(feats), "rename": app_contract_rename,
                            "uses_default_features": app_contract_defaults})
            return out
        if n == CONTRACTS:
            return [{"name": d, "kind": None} for d in contract_deps]
        return []
    def node(n):
        deps = [{"pkg": f"id:{d}", "dep_kinds": [{"kind": None}]} for d in edges.get(n, [])]
        deps += [{"pkg": f"id:{d}", "dep_kinds": [{"kind": "dev"}]} for d in dev_edges.get(n, [])]
        return {"id": f"id:{n}", "deps": deps,
                "dependencies": [f"id:{d}" for d in [*edges.get(n, []), *dev_edges.get(n, [])]]}
    def package(n):
        pkg = {"name": n, "id": f"id:{n}", "dependencies": manifest_deps(n)}
        if n == APP and app_features is not None:
            pkg["features"] = app_features
        if n == CONTRACTS and contract_features is not None:
            pkg["features"] = contract_features
        return pkg
    return {
        "packages": [package(n) for n in names],
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
        # L1 DAR İSTİSNA [2026-10-08]: blake2 yalnız adıyla ve yalnız normal bağımlılık olarak geçer
        ("L1 DEĞİL: blake2 normal bağımlılık (DAR istisna)",
         _md([(CONTRACTS, None), ("blake2", None)], {APP: [CONTRACTS, "blake2"]}), clean_src, []),
        ("L1 blake2 dev-bağımlılık (istisna yalnız normal tür)",
         _md([("blake2", "dev")], {}, dev_edges={APP: ["blake2"]}), clean_src, ["L1"]),
        ("L1 blake2 build-bağımlılık (istisna yalnız normal tür)",
         _md([("blake2", "build")], {APP: ["blake2"]}), clean_src, ["L1"]),
        ("L1 istisna genişlemez: blake2 yanında sha2",
         _md([("blake2", None), ("sha2", None)], {APP: ["blake2", "sha2"]}), clean_src, ["L1"]),
        ("L1 istisna genişlemez: blake2 yanında chacha20poly1305",
         _md([("blake2", None), ("chacha20poly1305", None)], {APP: ["blake2", "chacha20poly1305"]}),
         clean_src, ["L1"]),
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
        ("C5 app test-util'i kendi [features] tablosundan iletiyor",
         _md([], ok_graph, app_features={"mocks": ["kama-contracts/test-util"]}), clean_src, ["C5"]),
        ("C5 app test-util'i zayıf bağımlılık biçimiyle iletiyor (dep?/feat)",
         _md([], ok_graph, app_features={"default": [], "mocks": ["kama-contracts?/test-util"]}), clean_src, ["C5"]),
        ("C5 app test-util'i YENİDEN ADLANDIRILMIŞ bağımlılık üzerinden iletiyor",
         _md([], ok_graph, app_contract_features=(None, []), app_contract_rename="contracts",
             app_features={"mocks": ["contracts/test-util"]}), clean_src, ["C5"]),
        ("C5 yeniden adlandırılmış bağımlılık, zayıf biçim (dep?/feat)",
         _md([], ok_graph, app_contract_features=("dev", []), app_contract_rename="contracts",
             app_features={"mocks": ["contracts?/test-util"]}), clean_src, ["C5"]),
        ("C5 DEĞİL: yeniden adlandırılmış bağımlılık, özellik iletilmiyor",
         _md([], ok_graph, app_contract_features=(None, []), app_contract_rename="contracts",
             app_features={"mocks": ["contracts/other", "baska/test-util"]}), clean_src, []),
        ("C5 app, test-util'i açan bir sözleşme özelliğini açıyor (mocks → test-util)",
         _md([], ok_graph, app_contract_features=(None, ["mocks"]), contract_features={"test-util": [], "mocks": ["test-util"]}),
         clean_src, ["C5"]),
        ("C5 aynı takma ad app'in [features] tablosundan iletiliyor",
         _md([], ok_graph, app_contract_features=(None, []), app_features={"x": ["kama-contracts/mocks"]},
             contract_features={"test-util": [], "mocks": ["araç"], "araç": ["test-util"]}), clean_src, ["C5"]),
        ("C5 sözleşmenin varsayılan özelliği test-util'i açıyor",
         _md([], ok_graph, app_contract_features=(None, []), contract_features={"test-util": [], "default": ["test-util"]}),
         clean_src, ["C5"]),
        ("C5 DEĞİL: varsayılan test-util'i açıyor ama app default-features = false",
         _md([], ok_graph, app_contract_features=(None, []), app_contract_defaults=False,
             contract_features={"test-util": [], "default": ["test-util"]}), clean_src, []),
        ("C5 DEĞİL: sözleşmenin test-util'e bağlı olmayan özelliği açılıyor",
         _md([], ok_graph, app_contract_features=(None, ["std"]), contract_features={"test-util": [], "std": []}),
         clean_src, []),
        ("C5 DEĞİL: app'in özellik tablosu başka şeyler iletiyor",
         _md([], ok_graph, app_features={"default": ["std"], "std": [], "x": ["other/test-util", "kama-contracts/other"]}),
         clean_src, []),
        ("C5 app kaynağında sahte erişimi", _md([], ok_graph), {"a.rs": "fn f(h: H) -> u64 { h.to_raw_for_mock() }\n"}, ["C5"]),
        ("C5 DEĞİL: sahte erişimi yalnız yorumda", _md([], ok_graph), {"a.rs": "// to_raw_for_mock YASAK\nfn f() {}\n"}, []),
        # C6 [2026-10-06 F07.2]
        ("C6 yöntem çağrısı peer.as_bytes()", _md([], ok_graph),
         {"a.rs": "fn f(peer: PeerRef) -> u8 { peer.as_bytes()[0] }\n"}, ["C6"]),
        ("C6 UFCS PeerRef::as_bytes(&p)", _md([], ok_graph),
         {"a.rs": "fn f(p: PeerRef) { let _ = PeerRef::as_bytes(&p); }\n"}, ["C6"]),
        ("C6 fonksiyon işaretçisi let f = PeerRef::as_bytes;", _md([], ok_graph),
         {"a.rs": "fn g() { let f = PeerRef::as_bytes; }\n"}, ["C6"]),
        ("C6 str üzerinde s.as_bytes() (muhafazakâr)", _md([], ok_graph),
         {"a.rs": "fn f(s: &str) -> usize { s.as_bytes().len() }\n"}, ["C6"]),
        ("C6 char sabiti '\"' dizge sanılıp sonraki çağrıyı yutmaz", _md([], ok_graph),
         {"a.rs": "fn f(p: PeerRef) -> bool { let q = '\"'; p.as_bytes()[0] == q as u8 }\n"}, ["C6"]),
        ("C6 ömür ('a) char sanılmaz", _md([], ok_graph),
         {"a.rs": "fn f<'a>(p: &'a PeerRef) -> &'a [u8; 32] { p.as_bytes() }\n"}, ["C6"]),
        ("C6 ham dizgeden SONRAKİ çağrı yakalanır", _md([], ok_graph),
         {"a.rs": 'fn f(p: PeerRef) { let s = r#"a " b"#; p.as_bytes(); let t = "z"; }\n'}, ["C6"]),
        ("C6 DEĞİL: yalnız yorumda (satır, blok, iç içe blok, doc)", _md([], ok_graph),
         {"a.rs": "// peer.as_bytes() YASAK\n/* PeerRef::as_bytes /* iç */ as_bytes */ fn f() {}\n/// as_bytes\n"}, []),
        ("C6 DEĞİL: dizgede as_bytes", _md([], ok_graph),
         {"a.rs": 'fn f() -> &\'static str { "dizgede as_bytes" }\n'}, []),
        ("C6 DEĞİL: ham/bayt dizgede as_bytes", _md([], ok_graph),
         {"a.rs": 'fn f() { let a = r#"x "as_bytes" y"#; let b = b"as_bytes"; let c = br"as_bytes"; }\n'}, []),
        ("C6 DEĞİL: başka ad (as_bytes_len, my_as_bytes2)", _md([], ok_graph),
         {"a.rs": "fn f() { as_bytes_len(); my_as_bytes2(); }\n"}, []),
        # C7 [2026-10-11 K4G-06] — altyapı crate'ine yalnız bileşim kökü bağımlı olabilir
        ("C7 client-core → kama-store (normal)",
         _md([], {**ok_graph, TRANSPORT: [CONTRACTS, NOISE, "kama-store"]}), clean_src, ["C7"]),
        ("C7 kama-app → kama-store (dev-bağımlılık)",
         _md([], ok_graph, dev_edges={APP: ["kama-store"]}), clean_src, ["C7"]),
        ("C7 zincir kama-integration → ara → kama-store (doğrudan bağımlı ara yakalanır)",
         _md([], {**ok_graph, "kama-integration": ["ara"], "ara": ["kama-store"]}), clean_src, ["C7"]),
        ("C7 DEĞİL: kama-store → kama-app + kama-noise-wire (meşru yön)",
         _md([], {**ok_graph, "kama-store": [APP, NOISE, CONTRACTS]}), clean_src, []),
    ]
    bad = 0
    for name, md, src, want in cases:
        got = sorted({f.split()[0] for f in check(md, src)})
        ok = got == sorted(want)
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    # C7 istisna yolu [2026-10-11 K4G-06; 2026-10-12 K4G-11: COMPOSITION_ROOTS = (`kama-node`,)]: yol
    # açık kök listesiyle de doğrudan sınanır — aynı kenar, kök listede değilken kırmızı, listedeyken
    # temiz; varsayılan liste de temiz.
    root_md = _md([], {**ok_graph, "kama-node": ["kama-store", APP, NOISE]})
    for name, roots, want in (("C7 kök listede değil: kama-node → kama-store", (), ["C7"]),
                              ("C7 DEĞİL: bileşim kökü kama-node → kama-store", ("kama-node",), []),
                              ("C7 DEĞİL: varsayılan kök listesi (K4G-11: kama-node)", COMPOSITION_ROOTS, [])):
        got = sorted({f.split()[0] for f in infra_findings(root_md, roots)})
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    # C8 [2026-10-12 K4G-11] — bileşim kökünün ürün kaynağında Crypto adı yalnız kurma modülünde.
    wiring_src = "use kama_noise_wire::clock::SystemClock;\npub(crate) fn build() -> SystemClock { todo() }\n"
    root_cases = [
        ("C8 DEĞİL: Crypto adı yalnız kurma modülünde (ve yorumda)",
         {"src/wiring.rs": wiring_src, "src/lib.rs": "mod wiring;\n// kama_noise_wire yalnız wiring.rs'te\n"}, []),
        ("C8 use bildirimi kurma modülü dışında",
         {"src/wiring.rs": wiring_src, "src/node.rs": "use kama_noise_wire::protector::NoiseWireProtection;\n"},
         ["C8"]),
        ("C8 çalışma-zamanı çağrısı kurma modülü dışında",
         {"src/wiring.rs": wiring_src, "src/node.rs": "fn f(p: &P) -> Option<u8> { kama_noise_wire::epoch(p) }\n"},
         ["C8"]),
        ("C8 alt modülde de (src/a/b.rs)",
         {"src/wiring.rs": wiring_src, "src/a/b.rs": "type C = kama_noise_wire::protector::NoiseWireProtection;\n"},
         ["C8"]),
        ("C8 DEĞİL: dizgede",
         {"src/wiring.rs": wiring_src, "src/event.rs": 'fn f() -> &\'static str { "kama_noise_wire" }\n'}, []),
        # İç içe ayıklama gerçekten sınanır: ad İÇ yorumun kapanışından SONRA, dış yorumun içinde. İç
        # içeliği bilmeyen bir ayıklayıcı ilk `*/`'da yorumu bitirip adı kod sanardı (kırmızı).
        ("C8 DEĞİL: iç içe blok yorumda, iç kapanıştan SONRA",
         {"src/wiring.rs": wiring_src, "src/event.rs": "/* dış /* iç */ kama_noise_wire */ fn f() {}\n"}, []),
        ("C8 iç içe blok yorum kapandıktan SONRA kod",
         {"src/wiring.rs": wiring_src,
          "src/event.rs": "/* dış /* iç */ yorum */ type C = kama_noise_wire::protector::NoiseWireProtection;\n"},
         ["C8"]),
        ("C8 DEĞİL: başka ad (kama_noise_wire_x, my_kama_noise_wire)",
         {"src/wiring.rs": wiring_src, "src/a.rs": "fn f() { kama_noise_wire_x(); my_kama_noise_wire(); }\n"}, []),
    ]
    for name, srcs, want in root_cases:
        got = sorted({f.split()[0] for f in root_findings(root_md, {"kama-node": srcs})})
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    # C7 ↔ C8 tek kaynak [2026-10-12 K4G-11]: kök listesi kurma modülü tablosundan türer.
    same = set(COMPOSITION_ROOTS) == set(ROOT_WIRING)
    bad += not same
    print(f"  {'PASS' if same else 'FAIL'}  C7/C8 kök listeleri aynı (COMPOSITION_ROOTS = ROOT_WIRING anahtarları)")
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
    vacuum += [(f"vakum: {n} yok (C7)", without(full, n), clean_src) for n in INFRA]  # [2026-10-11 K4G-06]
    for name, md, src in vacuum:
        try:
            check(md, src)
            print(f"  FAIL  {name}: ortam hatası bekleniyordu, yeşil döndü")
            bad += 1
        except EnvironmentError:
            print(f"  PASS  {name}: ortam hatası (yeşil DEĞİL)")
    # C8 vakum [2026-10-12 K4G-11]: kök yok / ürün kaynağı yok / kurma modülü yok → ortam hatası.
    root_vacuum = [("vakum: kama-node yok (C8)", without(root_md, "kama-node"), {"kama-node": {"src/wiring.rs": wiring_src}}),
                   ("vakum: kama-node ürün kaynağı yok (C8)", root_md, {"kama-node": {}}),
                   ("vakum: kurma modülü yok (C8)", root_md, {"kama-node": {"src/lib.rs": "fn f() {}\n"}})]
    for name, md, srcs in root_vacuum:
        try:
            root_findings(md, srcs)
            print(f"  FAIL  {name}: ortam hatası bekleniyordu, yeşil döndü")
            bad += 1
        except EnvironmentError:
            print(f"  PASS  {name}: ortam hatası (yeşil DEĞİL)")
    # C9 [2026-10-16 K4G-15; CEVAP PAKETİ 5] — FFI sarmalayıcısı: (a) bağımlılık kenarları, (b) test-util,
    # (c) kaynakta kama_app / kama_contracts yalnız tip adı.
    def ffi_md(deps, *, features=None, packages=None):
        """Sentetik metadata: FFI crate'i + `packages` ({ad: özellik tablosu}). `deps` öğesi
        (ad, tür, workspace_mı) ya da sözlük (`name`, `kind`, `ws`, `features`, `defaults`, `rename`)."""
        registry = "registry+https://github.com/rust-lang/crates.io-index"
        manifest = []
        for dep in deps:
            if isinstance(dep, tuple):
                dep = {"name": dep[0], "kind": dep[1], "ws": dep[2]}
            manifest.append({"name": dep["name"], "kind": dep.get("kind"),
                             "source": None if dep.get("ws", True) else registry,
                             "features": list(dep.get("features", ())), "rename": dep.get("rename"),
                             "uses_default_features": dep.get("defaults", True)})
        pkgs = [{"name": FFI_WRAPPER, "id": f"id:{FFI_WRAPPER}", "dependencies": manifest,
                 "features": features or {}}]
        pkgs += [{"name": n, "id": f"id:{n}", "dependencies": [], "features": f} for n, f in (packages or {}).items()]
        return {"packages": pkgs, "resolve": {"nodes": []}}
    clean_ffi = {"src/lib.rs": "pub fn f() {}\n"}
    ffi_ok = [("kama-node", None, True), ("kama-app", None, True), ("kama-contracts", None, True),
              ("kama-provision", "dev", True), ("uniffi", None, False), ("thiserror", None, False)]
    contracts_mocks = {"kama-contracts": {"test-util": [], "mocks": ["test-util"]}}
    node_default = {"kama-node": {"test-util": [], "default": ["test-util"]}}
    ffi_cases = [
        # (a) bağımlılık kenarları
        ("C9 DEĞİL: kama-node + tip crate'leri (normal), kama-provision (dev), harici crate'ler", ffi_md(ffi_ok), []),
        ("C9 kama-mobile → client-core (normal)", ffi_md(ffi_ok + [("client-core", None, True)]), ["C9"]),
        ("C9 kama-mobile → kama-noise-wire (dev)", ffi_md(ffi_ok + [("kama-noise-wire", "dev", True)]), ["C9"]),
        ("C9 kama-mobile → kama-store (normal)", ffi_md(ffi_ok + [("kama-store", None, True)]), ["C9"]),
        ("C9 kama-provision normal bağımlılık olamaz (yalnız dev)", ffi_md([("kama-provision", None, True)]), ["C9"]),
        ("C9 kama-node build-bağımlılığı olamaz", ffi_md([("kama-node", "build", True)]), ["C9"]),
        # izinli ama YALNIZ normal crate'lerin dev türü
        ("C9 kama-node dev-bağımlılığı olamaz (yalnız normal)", ffi_md([("kama-node", "dev", True)]), ["C9"]),
        ("C9 kama-app dev-bağımlılığı olamaz (yalnız normal)", ffi_md([("kama-app", "dev", True)]), ["C9"]),
        ("C9 kama-contracts dev-bağımlılığı olamaz (yalnız normal)", ffi_md([("kama-contracts", "dev", True)]),
         ["C9"]),
        # (b) normal/build bağımlılıkları test-util açamaz
        ("C9 kama-app test-util açıyor (normal)",
         ffi_md(ffi_ok + [{"name": "kama-app", "features": ["test-util"]}]), ["C9"]),
        ("C9 build bağımlılığı test-util açıyor",
         ffi_md(ffi_ok + [{"name": "uniffi", "kind": "build", "ws": False, "features": ["test-util"]}]), ["C9"]),
        ("C9 kama-contracts test-util'i açan özelliği açıyor (mocks → test-util)",
         ffi_md(ffi_ok + [{"name": "kama-contracts", "features": ["mocks"]}], packages=contracts_mocks), ["C9"]),
        ("C9 kama-node'un varsayılan özelliği test-util'i açıyor", ffi_md(ffi_ok, packages=node_default), ["C9"]),
        ("C9 DEĞİL: varsayılan test-util'i açıyor ama default-features = false",
         ffi_md([{"name": "kama-node", "defaults": False}], packages=node_default), []),
        ("C9 harici normal bağımlılık test-util açıyor",
         ffi_md(ffi_ok + [{"name": "uniffi", "ws": False, "features": ["test-util"]}]), ["C9"]),
        ("C9 DEĞİL: dev bağımlılığı test-util açıyor (ürün .so'suna girmez)",
         ffi_md(ffi_ok + [{"name": "kama-provision", "kind": "dev", "features": ["test-util"]}]), []),
        ("C9 kendi [features] tablosundan iletiyor (kama-app/test-util)",
         ffi_md(ffi_ok, features={"x": ["kama-app/test-util"]}), ["C9"]),
        ("C9 zayıf biçimle iletiyor (kama-contracts?/test-util)",
         ffi_md(ffi_ok, features={"x": ["kama-contracts?/test-util"]}), ["C9"]),
        ("C9 yeniden adlandırılmış bağımlılığın yerel adıyla iletiyor (app/test-util)",
         ffi_md([{"name": "kama-app", "rename": "app"}], features={"x": ["app/test-util"]}), ["C9"]),
        ("C9 test-util'i açan özelliği iletiyor (kama-contracts/mocks)",
         ffi_md(ffi_ok, features={"x": ["kama-contracts/mocks"]}, packages=contracts_mocks), ["C9"]),
        ("C9 DEĞİL: özellik tablosu başka şeyler iletiyor",
         ffi_md(ffi_ok, features={"bindgen": ["uniffi/bindgen"], "x": ["kama-app/std", "baska/test-util"]}), []),
    ]
    for name, md, want in ffi_cases:
        got = sorted({f.split()[0] for f in ffi_findings(md, clean_ffi)})
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    # (c) kaynak: kama_app / kama_contracts yolları yalnız tip adı (yorum/dizge dışı)
    mirror = ("impl From<kama_app::codec::PttCodec> for PttCodec {\n"
              "    fn from(codec: kama_app::codec::PttCodec) -> Self {\n"
              "        use kama_app::codec::PttCodec as Source;\n"
              "        match codec {\n            Source::Opus => Self::Opus,\n            Source::Lyra => Self::Lyra,\n"
              "        }\n    }\n}\n")
    source_cases = [
        ("C9 DEĞİL: ayna dönüşümü (tam yol + işlev içi tip takma adı)", mirror, []),
        ("C9 DEĞİL: tuple varyant deseni ve varyant değeri",
         "fn f(e: kama_app::ptt::PttError) -> bool {\n    matches!(e, kama_app::ptt::PttError::Transport(\n"
         "        kama_contracts::s1::TransportError::Backpressure))\n}\n", []),
        ("C9 DEĞİL: yorum, doc ve dizgede yasak yollar",
         "// kama_app::ptt::max_bundle_payload_bytes(l)\n/// use kama_app as app;\n"
         "fn f() -> &'static str { \"kama_app::ptt::max_bundle_payload_bytes()\" }\n", []),
        ("C9 DEĞİL: gruplu tip içe aktarımı ve takma adın düz kullanımı",
         "use kama_app::codec::{FileAbortReason, PttCodec as Codec};\n"
         "fn f(c: Codec) -> Option<FileAbortReason> { let _ = c; None }\n", []),
        ("C9 DEĞİL: varyant içe aktarımı",
         "use kama_contracts::s1::ConnectPhase::{Measuring, Suspended};\n", []),
        ("C9 DEĞİL: nitelikli yolun kuyruğu tip adı (ilişkili tip)",
         "type T = <kama_app::codec::PttCodec as Tr>::Output;\n", []),
        ("C9 DEĞİL: başka crate'ler ve benzer adlar (kama_node, my_kama_app, kama_app_x)",
         "fn f() { kama_node::KamaNode::start(a); my_kama_app::helper(); kama_app_x::helper(); }\n", []),
        ("C9 use: crate yeniden adlandırma (use kama_app as app;)", "use kama_app as app;\n", ["C9"]),
        ("C9 use: modül içe aktarımı (use kama_contracts::s1;)", "use kama_contracts::s1;\n", ["C9"]),
        ("C9 use: glob (use kama_app::codec::*;)", "use kama_app::codec::*;\n", ["C9"]),
        ("C9 use: self (use kama_app::codec::{self, PttCodec};)", "use kama_app::codec::{self, PttCodec};\n", ["C9"]),
        ("C9 use: fonksiyon içe aktarımı", "use kama_app::ptt::max_bundle_payload_bytes;\n", ["C9"]),
        ("C9 use: sabit içe aktarımı", "use kama_app::codec::MAX_BODY_BYTES;\n", ["C9"]),
        ("C9 use: iç içe grupta fonksiyon",
         "use kama_app::{codec::PttCodec, ptt::max_bundle_payload_bytes};\n", ["C9"]),
        ("C9 extern crate", "extern crate kama_app;\n", ["C9"]),
        ("C9 serbest fonksiyon çağrısı",
         "fn f(l: &L) -> usize { kama_app::ptt::max_bundle_payload_bytes(l) }\n", ["C9"]),
        ("C9 ilişkili fonksiyon çağrısı", "fn f() -> P { kama_app::codec::PttCodec::from_byte(1) }\n", ["C9"]),
        ("C9 ilişkili sabit (BÜYÜK_HARF)", "fn f() -> usize { kama_app::codec::Limits::MAX }\n", ["C9"]),
        ("C9 modül sabiti", "const M: usize = kama_contracts::s1::MAX_UNIT_BYTES;\n", ["C9"]),
        ("C9 makro", "fn f() { kama_app::trace!(\"x\"); }\n", ["C9"]),
        ("C9 takma ad üzerinden ilişkili fonksiyon",
         "fn f() { use kama_app::codec::PttCodec as Source; let _ = Source::from_byte(1); }\n", ["C9"]),
        ("C9 gruplu takma ad üzerinden ilişkili fonksiyon",
         "use kama_app::codec::{PttCodec as Codec};\nfn f() -> Codec { Codec::from_byte(1) }\n", ["C9"]),
        ("C9 turbofish arkasında ilişkili fonksiyon",
         "fn f() { let _ = kama_app::codec::Table::<u8>::new(); }\n", ["C9"]),
        ("C9 nitelikli yol kuyruğunda fonksiyon",
         "fn f(c: C) -> u8 { <kama_app::codec::PttCodec as Into<u8>>::into(c) }\n", ["C9"]),
        ("C9 satırlara bölünmüş yol",
         "fn f(l: &L) -> usize {\n    kama_app::ptt\n        ::max_bundle_payload_bytes(l)\n}\n", ["C9"]),
    ]
    for name, src, want in source_cases:
        got = sorted({f.split()[0] for f in ffi_findings(ffi_md(ffi_ok), {"src/x.rs": src})})
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
    # `use` bulgusu nedenini adlandırır (crate / glob / `self` ayrı ayrı; yalnız "tip adı yok" değil).
    for src, needle in (("use kama_app as app;\n", "crate içe aktarımı"), ("use kama_app::codec::*;\n", "glob"),
                        ("use kama_app::codec::{self, PttCodec};\n", "`self`")):
        found = ffi_findings(ffi_md(ffi_ok), {"src/x.rs": src})
        ok = len(found) == 1 and needle in found[0]
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  C9 use bulgusu nedeni `{needle}`: {src.strip()}")
    # Vakum: dolu sentetik metadata'dan yalnız FFI crate'i çıkarılır; kaynak yoksa da ortam hatası.
    for name, md, srcs in (("vakum: kama-mobile yok (C9)", without(ffi_md(ffi_ok, packages=node_default), FFI_WRAPPER),
                            clean_ffi),
                           ("vakum: kama-mobile kaynağı yok (C9)", ffi_md(ffi_ok), {})):
        try:
            ffi_findings(md, srcs)
            print(f"  FAIL  {name}: ortam hatası bekleniyordu, yeşil döndü")
            bad += 1
        except EnvironmentError:
            print(f"  PASS  {name}: ortam hatası (yeşil DEĞİL)")
    # Uçtan uca [2026-10-16 K4G-15]: `main()`'in yolu (`all_findings`) — C8 ve C9 çağrıları düşerse kırmızı.
    e2e_md = _md([], {**ok_graph, "kama-node": ["kama-store", APP, NOISE], FFI_WRAPPER: ["kama-node"]})
    e2e_root = {"kama-node": {"src/wiring.rs": wiring_src, "src/lib.rs": "mod wiring;\n"}}
    e2e_bad_root = {"kama-node": {"src/wiring.rs": wiring_src,
                                  "src/node.rs": "use kama_noise_wire::protector::NoiseWireProtection;\n"}}
    e2e_bad_ffi = {"src/lib.rs": "fn f(l: &L) -> usize { kama_app::ptt::max_bundle_payload_bytes(l) }\n"}
    for name, roots, ffi_srcs, want in (("uçtan uca DEĞİL: bütün kurallar temiz", e2e_root, clean_ffi, []),
                                        ("uçtan uca C8 (main'in yolu)", e2e_bad_root, clean_ffi, ["C8"]),
                                        ("uçtan uca C9 (main'in yolu)", e2e_root, e2e_bad_ffi, ["C9"])):
        got = sorted({f.split()[0] for f in all_findings(e2e_md, clean_src, roots, ffi_srcs)})
        ok = got == want
        bad += not ok
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: beklenen {want or 'temiz'}, gelen {got or 'temiz'}")
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
        ffi_sources = ffi_inputs(md)  # C9 (c) [2026-10-16 K4G-15]
        # C8 [2026-10-12 K4G-11] ve C9 [2026-10-16 K4G-15] dahil — öz-testin uçtan uca vakalarıyla aynı yol.
        findings = all_findings(md, sources, root_inputs(md), ffi_sources)
    except EnvironmentError as e:
        print(f"ORTAM HATASI: {e}", file=sys.stderr)
        return 2
    for f in findings:
        print(f"::error title=LAYER-VIOLATION::{f}")
        print(f"KIRMIZI: {f}")
    if findings:
        print(f"\nLAYER-CHECK: KIRMIZI ({len(findings)} ihlal) — INV-NEIGHBOR-1")
        return 1
    print(f"LAYER-CHECK: YEŞİL ({APP}: {len(sources)} kaynak dosyası; {FFI_WRAPPER}: {len(ffi_sources)} kaynak "
          f"dosyası; L1/L2/L3 + C1–C9 temiz; kök={root})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
