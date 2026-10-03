#!/usr/bin/env bash
# Zamanlanmış workflow'lar için "koşmaya değer mi" kapısı.
#
#   scripts/ci/schedule-guard.sh <workflow-dosyası> <yol>...
#
# NEDEN VAR (ölçüldü, 2026-10-04): main 24 Eylül–3 Ekim arasında DEĞİŞMEDİ (443d1555) ama gecelik
# iş dokuz gece üst üste aynı commit'i yeniden koşturdu. Değişmeyen kodu her gece yeniden derleyip
# test etmek runner-dakikası yakar ve hiçbir yeni bilgi üretmez.
#
# KURAL: son YEŞİL koşunun commit'inden bu yana verilen yollar değişmediyse run=false.
#   · elle tetikleme (workflow_dispatch)        → her zaman run=true
#   · hiç yeşil koşu yoksa / commit bulunamazsa → run=true (şüphede koş)
#   · kırmızı koşu "son yeşil" SAYILMAZ         → kırmızı kalan değişiklik yeşile dönene dek her gece yeniden koşar
#
# Zaman penceresi (ör. "son 24 saat") KULLANILMAZ: GitHub cron'u saatlerce kayabiliyor ve pencere
# dışında kalan bir değişiklik sessizce atlanırdı. Karşılaştırma commit'tendir, saatten değil.
set -euo pipefail

wf="${1:?workflow dosyası gerekli}"
shift
[ "$#" -ge 1 ] || { echo "en az bir yol gerekli" >&2; exit 2; }

emit() {
  echo "run=$1" >> "${GITHUB_OUTPUT:-/dev/stdout}"
  echo "schedule-guard: run=$1 — $2"
}

if [ "${GITHUB_EVENT_NAME:-}" = "workflow_dispatch" ]; then
  emit true "elle tetiklendi"
  exit 0
fi

last="$(gh run list --workflow "$wf" --branch main --status success --limit 1 \
          --json headSha -q '.[0].headSha' 2>/dev/null || true)"

if [ -z "$last" ]; then
  emit true "önceki yeşil koşu yok"
elif ! git cat-file -e "${last}^{commit}" 2>/dev/null; then
  emit true "son yeşil commit ($last) yerelde bulunamadı"
elif git diff --quiet "$last" HEAD -- "$@"; then
  emit false "son yeşilden (${last:0:8}) beri izlenen yollarda değişiklik yok"
else
  emit true "son yeşilden (${last:0:8}) beri değişen: $(git diff --name-only "$last" HEAD -- "$@" | head -5 | tr '\n' ' ')"
fi
