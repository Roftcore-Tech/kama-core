#!/usr/bin/env bash
# Kampanya yürütücüsü sarmalayıcısı: rootless `unshare -rnm` içinde kvt_exec.py'yi koşturur.
# Kullanım: kvt_run.sh MANIFEST OUTDIR [RUN_ID_REGEX]
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
exec unshare -rnm python3 "$HERE/kvt_exec.py" "$@"
