#!/usr/bin/env bash
# Tüm gecelik/haftalık fuzz hedeflerini TEK job'da, tek derleme ağacıyla koşturur.
#
#   scripts/ci/fuzz-all.sh <hedef-başına-saniye>
#
# NEDEN TEK JOB: altı hedef altı ayrı job'dayken her biri bağımlılık ağacını baştan derliyor ve
# GitHub her job'ı ayrı ayrı yukarı yuvarlayarak faturalıyordu. Hedefler aynı crate'in ikilileri;
# derleme bir kez yapılır, hedefler sırayla koşar.
#
# NEDEN --target: `taiki-e/install-action`'ın indirdiği hazır `cargo-fuzz` ikilisi musl için
# derlenmiştir ve varsayılan hedefini KENDİ derlendiği üçlüden alır (x86_64-unknown-linux-musl).
# Sanitizer statik libc ile çalışmaz → "sanitizer is incompatible with statically linked libc" +
# "can't find crate for `core`". Bu yüzden fuzz işi KURULDUĞU GÜNDEN BERİ hiç koşmadı (ölçüldü:
# 4 Eylül–3 Ekim arası her koşuda ~1 dakikada kırmızı). Hedef üçlüsü açıkça host'a sabitlenir.
set -euo pipefail

secs="${1:?hedef başına saniye gerekli}"
host="$(rustc +nightly -vV | sed -n 's/^host: //p')"
[ -n "$host" ] || { echo "host üçlüsü okunamadı" >&2; exit 2; }

targets=(
  fuzz_inner_parse
  fuzz_open
  fuzz_control_decode
  fuzz_ack_decode
  fuzz_prefix
  fuzz_fec_decode
)

for t in "${targets[@]}"; do
  echo "::group::fuzz $t (${secs}s, hedef=$host)"
  cargo +nightly fuzz run "$t" --target "$host" -- \
    -max_total_time="$secs" -rss_limit_mb=2048 -print_final_stats=1
  echo "::endgroup::"
done

# Corpus küçültme yalnız HEPSİ temiz bittiyse (bir çökme varsa yukarıda çıkılmıştır).
for t in "${targets[@]}"; do
  echo "::group::cmin $t"
  cargo +nightly fuzz cmin "$t" --target "$host"
  echo "::endgroup::"
done
