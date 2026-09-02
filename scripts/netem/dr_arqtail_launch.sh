#!/usr/bin/env bash
# DR-ARQ-PRODUCT-TAIL-1 — parallel shard launcher. Spawns NSHARD grid-executor workers, each in its OWN
# rootless netns (disjoint namespaces → no veth/port conflict), each running a disjoint manifest shard.
# Serial WITHIN a worker (netns is shared per worker); parallel ACROSS workers. Low CPU (UDP+sleep) → safe.
# Concatenates the shard result CSVs into one RESULT_OUT (header once). Deterministic union of rows.
#
# Env: RUNNER_BIN, MANIFEST, RESULT_OUT, NSHARD (default 6), [SUBSET], [LIMIT].
set -uo pipefail
BIN=${RUNNER_BIN:?}; MANIFEST=${MANIFEST:?}; RESULT=${RESULT_OUT:?}; NSHARD=${NSHARD:-6}
SUBSET=${SUBSET:-}; LIMIT=${LIMIT:-0}
OUTDIR=$(dirname "$RESULT"); mkdir -p "$OUTDIR"
RUN="$(cd "$(dirname "$0")" && pwd)/dr_arqtail_run.sh"

echo "launching $NSHARD shard workers over $(($(wc -l < "$MANIFEST")-1)) manifest rows..."
pids=()
for i in $(seq 0 $((NSHARD-1))); do
  SHARD=$i NSHARD=$NSHARD RUNNER_BIN="$BIN" MANIFEST="$MANIFEST" SUBSET="$SUBSET" LIMIT="$LIMIT" \
    RESULT_OUT="$OUTDIR/shard$i.csv" bash "$RUN" > "$OUTDIR/shard$i.stdout" 2>&1 &
  pids+=($!)
done
echo "workers: ${pids[*]}"
fail=0
for p in "${pids[@]}"; do wait "$p" || fail=$((fail+1)); done

# concatenate: header from the first non-empty shard, then all data rows
: > "$RESULT"
hdr=""
for i in $(seq 0 $((NSHARD-1))); do
  f="$OUTDIR/shard$i.csv"
  [ -s "$f" ] || continue
  if [ -z "$hdr" ]; then head -1 "$f" > "$RESULT"; hdr=1; fi
  tail -n +2 "$f" >> "$RESULT"
done
rows=$(($(wc -l < "$RESULT")-1))
echo "DONE: $rows result rows in $RESULT (worker failures: $fail)"
for i in $(seq 0 $((NSHARD-1))); do
  echo "  shard$i: $(tail -1 "$OUTDIR/shard$i.csv.log" 2>/dev/null || echo '(no log)')"
done
