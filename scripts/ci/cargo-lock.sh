#!/usr/bin/env bash
# Kama yerel ajan cargo sarmalayıcısı (EK-1 D-34). Yerel ajanlar cargo'yu YALNIZ bununla çağırır:
#
#   KAMA_AGENT=M2 scripts/ci/cargo-lock.sh test -p adaptive-transport --lib
#
#   * Tek cargo süreci: bütün yerel oturumlar aynı kilit dosyasını paylaşır
#     (KAMA_CARGO_LOCK, varsayılan ~/.cache/kama-cargo.lock); kilit alınamazsa bekler.
#     scripts/ci/local-gate.sh aynı kilidi tutar.
#   * CARGO_BUILD_JOBS=2 (makine kuralı; çift donma).
#   * Bütçe: ajan başına günlük 45 dk cargo süresi (kilit alındıktan sonra ölçülür; bekleme sayılmaz).
#     Her çağrı defterine yazılır (KAMA_CARGO_LEDGER, varsayılan ~/.cache/kama-cargo-ledger.tsv):
#     tarih · ajan · başlangıç · süre_s · çıkış kodu · argümanlar.
#     Bütçe aşılınca çağrı ENGELLENMEZ; stderr'e "BÜTÇE AŞIMI" yazılır ve orkestratör günlük raporda
#     okur (KARAR-KAYDI: gösterim penceresinde derlemeyi durdurmak dondurmayı riske atar).
#   * Bulut oturumları bu sarmalayıcıyı kullanmaz (D-34 dışı).
set -u
AGENT=${KAMA_AGENT:?KAMA_AGENT ayarlı değil (ör. KAMA_AGENT=M1)}
LOCK=${KAMA_CARGO_LOCK:-$HOME/.cache/kama-cargo.lock}
LEDGER=${KAMA_CARGO_LEDGER:-$HOME/.cache/kama-cargo-ledger.tsv}
BUDGET_S=${KAMA_CARGO_BUDGET_S:-2700}
mkdir -p "$(dirname "$LOCK")" "$(dirname "$LEDGER")"

today=$(date +%F)
used=$(awk -F'\t' -v d="$today" -v a="$AGENT" '$1==d && $2==a {s+=$4} END {print s+0}' "$LEDGER" 2>/dev/null || echo 0)
if [ "$used" -ge "$BUDGET_S" ]; then
  echo "BÜTÇE AŞIMI: $AGENT bugün ${used}s cargo kullandı (bütçe ${BUDGET_S}s); çağrı yine koşuyor, raporlanacak" >&2
fi

exec 9>"$LOCK"
flock 9
export CARGO_BUILD_JOBS=2
start=$(date +%H:%M:%S); t0=$SECONDS
cargo "$@"; rc=$?
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$today" "$AGENT" "$start" "$((SECONDS-t0))" "$rc" "$*" >> "$LEDGER"
exit "$rc"
