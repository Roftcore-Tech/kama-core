#!/usr/bin/env bash
# ACK birlikte çalışma matrisinin ESKİ ucunu SABİT SHA'dan derler (scripts/ci/ack-interop/PIN).
#
#   scripts/ci/ack_interop_build_old.sh <çıktı_ikili_yolu>
#
# Eski uç aynı kaynak ağacında alternatif bir yol DEĞİLDİR: PIN'deki engine_sha ayrı bir dizine
# (git worktree) çıkarılır, kendi Cargo.lock'uyla (--locked) ve kendi hedef dizininde derlenir.
# Eş sürücüsü: engine_sha'da varsa onun kendi dosyası, yoksa PIN'le özeti doğrulanan donmuş kopya.
#
# Ortam: OLD_TARGET (isteğe bağlı) — eski derlemenin hedef dizini (varsayılan: geçici dizin).
# Çıkış: 0 başarı · 1 doğrulama/derleme hatası · 2 kullanım hatası.
set -euo pipefail

out="${1:-}"
[[ -n "$out" ]] || { echo "kullanım: $0 <çıktı_ikili_yolu>" >&2; exit 2; }
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PIN="$ROOT/scripts/ci/ack-interop/PIN"
FROZEN="$ROOT/scripts/ci/ack-interop/peer_pinned.rs"

engine_sha="$(sed -n 's/^engine_sha=//p' "$PIN")"
peer_sha256="$(sed -n 's/^peer_sha256=//p' "$PIN")"
[[ "$engine_sha" =~ ^[0-9a-f]{40}$ ]] || { echo "::error title=ACK_INTEROP::PIN engine_sha tam 40 haneli SHA değil" >&2; exit 1; }
[[ "$peer_sha256" =~ ^[0-9a-f]{64}$ ]] || { echo "::error title=ACK_INTEROP::PIN peer_sha256 geçersiz" >&2; exit 1; }

if ! git -C "$ROOT" cat-file -e "${engine_sha}^{commit}" 2>/dev/null; then
  git -C "$ROOT" fetch --no-tags --depth=1 origin "$engine_sha"
fi

tmp="$(mktemp -d)"
cleanup() { git -C "$ROOT" worktree remove --force "$tmp/old" >/dev/null 2>&1 || true; rm -rf "$tmp"; }
trap cleanup EXIT
git -C "$ROOT" worktree add --quiet --detach "$tmp/old" "$engine_sha" >/dev/null

peer="$tmp/old/crates/adaptive-transport/examples/ack_interop_peer.rs"
if [[ -f "$peer" ]]; then
  src="$peer"; origin="engine_sha'nın kendi dosyası"
else
  src="$FROZEN"; origin="donmuş kopya (scripts/ci/ack-interop/peer_pinned.rs)"
fi
got="$(sha256sum "$src" | cut -d' ' -f1)"
if [[ "$got" != "$peer_sha256" ]]; then
  echo "::error title=ACK_INTEROP::eski eş sürücüsünün özeti PIN ile uyuşmuyor ($origin: $got ≠ $peer_sha256)" >&2
  exit 1
fi
[[ "$src" == "$peer" ]] || cp "$src" "$peer"

target="${OLD_TARGET:-$tmp/target}"
echo "eski uç: engine_sha=$engine_sha sürücü=$origin özet=$got"
( cd "$tmp/old" && CARGO_TARGET_DIR="$target" cargo build --locked -p adaptive-transport --example ack_interop_peer )
cp "$target/debug/examples/ack_interop_peer" "$out"
echo "eski uç ikilisi: $out"
