#!/usr/bin/env bash
# KAMA TRANSPORT — SINGLE VERIFICATION GATE (kapanış planı v2.1 §2)
#
# MUST be run in a FRESH `git worktree` containing only committed content, with its own
# CARGO_TARGET_DIR, BEFORE any merge. Every step's counts are printed; the exit code is the verdict.
#
#   scripts/gate.sh [target-dir]
#
# Machine safety: -j2, one cargo process at a time (never parallelise the steps).
#
# GATE CONTRACT — 13 checks (was 12; the fmt check is new):
#     0            cargo fmt --all -- --check          <-- added; see below
#     1            compile closure (--workspace --all-targets --all-features)
#     2..6         tests: default | fec-budget | diag-telemetry | cold-start | all-features
#     7  (x5)      clippy -D warnings, once per feature configuration
#     8            worktree clean after the tests
#
# Why step 0 exists: CLAUDE.md section 7 makes `cargo fmt --check` a PR gate, but the hosted
# workflow only fires on push:[main] and pull_request. Branch `transport-baseline-closure` has no
# PR and no hosted runs, so no commit on it has ever been fmt-checked -- the drift measured at HEAD
# (POST-CI-FORMAT-DRIFT, 136 hunks, absent at the last hosted-green SHA 4aa11a93) accumulated
# entirely unobserved. This gate is the only place it can be caught locally.
#
# A RED here on an unformatted HEAD is the gate working, not a product or test failure. Report it
# as GATE RED -- POST-CI-FORMAT-DRIFT.
set -uo pipefail

JOBS="${GATE_JOBS:--j2}"
export CARGO_TARGET_DIR="${1:-${CARGO_TARGET_DIR:-$PWD/.gate-target}}"

pass=0
fail=0
declare -a RESULTS=()

record() { # name, ok(0/1), detail
  if [ "$2" -eq 0 ]; then pass=$((pass + 1)); RESULTS+=("PASS  $1  $3")
  else fail=$((fail + 1)); RESULTS+=("FAIL  $1  $3"); fi
}

# Sum "test result: ok. N passed" across all targets; report failures explicitly.
run_tests() { # name, extra-args...
  local name="$1"; shift
  local out
  out="$(cargo test $JOBS --workspace "$@" 2>&1)"
  local rc=$?
  local passed failed errs
  passed="$(grep -oP '^test result: \w+\. \K[0-9]+' <<<"$out" | paste -sd+ - | sed 's/^$/0/' | python3 -c 'import sys;print(eval(sys.stdin.read() or "0"))')"
  failed="$(grep -cE '^test [^ ]+ \.\.\. FAILED' <<<"$out")"
  errs="$(grep -cE '^error(\[|:)' <<<"$out")"
  record "$name" "$rc" "passed=$passed failed=$failed errors=$errs"
  if [ "$rc" -ne 0 ]; then grep -E '^test [^ ]+ \.\.\. FAILED|^error(\[|:)' <<<"$out" | head -10; fi
}

echo "== gate.sh :: $(git rev-parse --short HEAD) :: CARGO_TARGET_DIR=$CARGO_TARGET_DIR =="

# 0 — formatting, byte-identical to the hosted `quality` job's step.
# The repo pins rustfmt via rust-toolchain.toml (1.96.0), so the bare invocation IS the pinned one.
# This step is NOT skippable and NOT warning-only: the branch has never been checked by hosted CI
# (workflow triggers are push:[main] + pull_request; this branch has no PR and no runs), so the
# only place the fmt gate can run at all is here.
out="$(cargo fmt --all -- --check 2>&1)"; rc=$?
record "0 fmt-check" "$rc" "hunks=$(grep -c '^Diff in ' <<<"$out")"
[ "$rc" -ne 0 ] && grep '^Diff in ' <<<"$out" | sed 's/:.*//' | sort -u | head -25

# 1 — compile closure (catches files referenced but never tracked)
out="$(cargo build $JOBS --workspace --all-targets --all-features 2>&1)"; rc=$?
record "1 compile-closure" "$rc" "errors=$(grep -cE '^error(\[|:)' <<<"$out")"
[ "$rc" -ne 0 ] && grep -E '^error(\[|:)' <<<"$out" | head -10

# 2..6 — semantic feature matrix, each configuration on its own
run_tests "2 test default"
run_tests "3 test fec-budget"       --features experimental-fec-budget
run_tests "4 test diag-telemetry"   --features diag-sched-telemetry
run_tests "5 test cold-start"       --features experimental-cold-start
run_tests "6 test all-features"     --all-features

# 7 — clippy per configuration
for cfg in "" "--features experimental-fec-budget" "--features diag-sched-telemetry" \
           "--features experimental-cold-start" "--all-features"; do
  out="$(cargo clippy $JOBS --workspace --all-targets $cfg -- -D warnings 2>&1)"; rc=$?
  record "7 clippy ${cfg:---default}" "$rc" "errors=$(grep -cE '^error(\[|:)' <<<"$out")"
  [ "$rc" -ne 0 ] && grep -E '^error' <<<"$out" | head -5
done

# 8 — the worktree must be pristine AFTER the tests (no test may write into the repo tree)
dirty="$(git status --porcelain)"
if [ -z "$dirty" ]; then record "8 worktree-clean" 0 "git status empty"
else record "8 worktree-clean" 1 "$(wc -l <<<"$dirty") dirty path(s)"; echo "$dirty" | head -10; fi

echo
printf '%s\n' "${RESULTS[@]}"
echo "== gate.sh: $pass passed, $fail failed =="
[ "$fail" -eq 0 ] || exit 1
