#!/usr/bin/env bash
# [DÜZENEK v1] Düğüm kayıtlarının kopyacısı (ürün kodu değil): f0_flows node-*/ dizinlerini koşu
# sonunda siler; başlangıç kaydı (transport-start.txt) ve pilot özeti (pilot-summary.txt) silinmeden
# <koşu dizini>/kayit/ altına alınır. Ayrıca iki f0_flows sürecinin PID + ağ ad alanı çekimi (en çok
# iki kez; kayit/pid_ns.txt). kullanım: kapi3_kopyaci.sh <koşu dizini>  (kapi3_harness.sh başlatır;
# MID ortamdan). SIGTERM ile kapanır.
rd="$1"; mkdir -p "$rd/kayit"; shot=0
trap 'exit 0' TERM
while :; do
  for f in "$rd"/node-*/transport-start.txt "$rd"/node-*/pilot-summary.txt; do
    [ -f "$f" ] && cp -f "$f" "$rd/kayit/$(basename "$(dirname "$f")")-$(basename "$f")" 2>/dev/null
  done
  if [ "$shot" -lt 2 ]; then
    A=""; B=""
    for p in $(pgrep -f "f0_flows[^ ]* --role"); do
      c=$(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null)
      case "$c" in *"$rd"*) case "$c" in *"--role alice"*) A=$p;; *"--role bob"*) B=$p;; esac;; esac
    done
    if [ -n "$A" ] && [ -n "$B" ]; then
      shot=$((shot+1))
      { echo "=== çekim $shot t=$(date -u +%FT%T.%NZ)"; ps -o pid,ppid,etimes,stat,args -p "$A","$B" | cut -c1-160
        echo "alice pid=$A net=$(readlink /proc/$A/ns/net)"; echo "bob pid=$B net=$(readlink /proc/$B/ns/net)"
        echo "orta pid=${MID:-} net=$(readlink "/proc/${MID:-}/ns/net")"; echo "kabuk net=$(readlink /proc/self/ns/net)"
      } >> "$rd/kayit/pid_ns.txt" 2>&1
    fi
  fi
  sleep 0.5
done
