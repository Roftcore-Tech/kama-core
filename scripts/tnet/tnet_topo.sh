#!/usr/bin/env bash
# kama-target-net-v2 — 3-netns topolojiyi (scripts/netem/kapi3_harness.sh: IPv6 kapalı, dört arayüzde
# statik ARP) kurar ve MID/CHLB pid'lerini yazar. `unshare -rnm` İÇİNDE çağrılır (kama-vs-tcp-v1
# kvt_topo.sh deseni; harness main'deki güncel sürüm).
#   tnet_topo.sh up               → "MID=<pid> CHLB=<pid> IPV6=<durum>"
#   tnet_topo.sh down MID CHLB
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$HERE/../.." && pwd)"
# shellcheck source=../netem/kapi3_harness.sh
source "$REPO/scripts/netem/kapi3_harness.sh"
case "${1:-}" in
  up)   kapi3_setup >/dev/null 2>&1; echo "MID=$MID CHLB=$CHLB IPV6=${KAPI3_IPV6_STATE:-}" ;;
  down) MID="$2"; CHLB="$3"; kapi3_teardown >/dev/null 2>&1 ;;
  *) echo "kullanım: tnet_topo.sh up | down MID CHLB"; exit 2 ;;
esac
