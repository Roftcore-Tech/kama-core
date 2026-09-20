#!/usr/bin/env bash
# 3-netns topolojiyi (scripts/netem/kapi3_harness.sh) kurar; MID/CHLB pid'lerini yazar. `unshare -rnm` İÇİNDE çağrılır.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$HERE/../.." && pwd)"
source "$REPO/scripts/netem/kapi3_harness.sh"
case "${1:-}" in
  up)   kapi3_setup >/dev/null 2>&1; echo "MID=$MID CHLB=$CHLB" ;;
  down) MID="$2"; CHLB="$3"; kapi3_teardown >/dev/null 2>&1 ;;
  *) echo "kullanım: kvt_topo.sh up | down MID CHLB"; exit 2 ;;
esac
