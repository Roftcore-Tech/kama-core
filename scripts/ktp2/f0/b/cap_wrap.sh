#!/usr/bin/env bash
# [KTP2-F0 / Ajan B] Uç yakalama sarmalayıcısı — bir kol komutunu (sunucu ya da istemci) kendi
# ad alanındaki uç arayüzün yakalamasıyla birlikte koşturur. Komutun anlamı DEĞİŞMEZ: argümanlar
# aynen geçer, çıkış kodu aynen döner, SIGTERM komuta iletilir.
#
# Kullanım (hücre dosyasında, f0_cell.sh yer tutucularıyla):
#   bash <b>/cap_wrap.sh srv @D@ -- <sunucu komutu…>   # ns-B içinde: vb0 (i = ileri varış, o = dönüş enjekte)
#   bash <b>/cap_wrap.sh cli @D@ -- <istemci komutu…>  # ns-A içinde: va0 (o = ileri enjekte, i = dönüş varış)
# Topoloji: scripts/netem/kapi3_harness.sh (ns-A va0 ↔ ma0 · MID netem · mb0 ↔ vb0 ns-B). netem
# yalnız MID'de; uç arayüzler kayıpsız veth → vb0'a giren = darboğazdan geçen ileri trafik,
# va0'a giren = darboğazdan geçen dönüş trafiği.
# Çıktı: <D>/capb_<srv|cli>.csv.gz (+ .err). Yakalama başlamazsa komut YİNE koşar (ölçüm eksik
# kalır, koşu düşürülmez) ve <D>/capb_<yan>.FAIL yazılır.
set -u
side="${1:?srv|cli}"; d="${2:?koşu dizini}"; shift 2
[ "${1:-}" = "--" ] && shift
case "$side" in
  srv) ifc=vb0 ;;
  cli) ifc=va0 ;;
  *) echo "cap_wrap: bilinmeyen yan: $side" >&2; exit 2 ;;
esac
here="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
out="$d/capb_${side}.csv"
python3 -I "$here/cap_b.py" "$ifc" "$out" > "$d/capb_${side}.ready" 2> "$d/capb_${side}.err" < /dev/null &
cap=$!
ok=0
for _ in $(seq 1 100); do
  if grep -q READY "$d/capb_${side}.ready" 2>/dev/null; then ok=1; break; fi
  kill -0 "$cap" 2>/dev/null || break
  sleep 0.01
done
[ "$ok" = 1 ] || echo "capture başlamadı ($ifc)" > "$d/capb_${side}.FAIL"

"$@" &
child=$!
trap 'kill -TERM "$child" 2>/dev/null' TERM INT
rc=0
wait "$child"; rc=$?
# TERM ile bölünen wait'ten sonra komutun kendi kapanışını (ör. KP-SERVER satırı) bekle.
while kill -0 "$child" 2>/dev/null; do wait "$child"; rc=$?; done
trap - TERM INT
kill -TERM "$cap" 2>/dev/null; wait "$cap" 2>/dev/null
[ -s "$out" ] && gzip -f -1 "$out"
exit "$rc"
