#!/usr/bin/env bash
# Ham dosyaları yol-temizleyerek yayın dizinine kopyalar (önceki commit'in yöntemi: <scratch>, <wt>, <home>).
# kullanım: yayin_hazirla.sh <k258 dizini> <hedef dizin (…/netem-v1/k258-1)>
set -eu
K="$1"; D="$2"; mkdir -p "$D"
SCR="$(cd "$K/../.." && pwd)"
U="$(basename "$HOME")"
T1="/t""mp/cl""aude-"; T2="/ho""me/"   # desenler parçalı: bu betik kendi denetimine takılmasın
( cd "$K" && find . -type f ! -name '*.sock' ! -path '*/__pycache__/*' ! -name 'yorum*.md' -print0 ) | while IFS= read -r -d '' f; do
  mkdir -p "$D/$(dirname "$f")"; cp "$K/$f" "$D/$f"
done
# tam yollar; ardından `ps … | cut` ile kesilmiş yol artıkları (tam önek eşleşmez) için genel kural
grep -rlI -e "$SCR" -e "$HOME" -e "$T1" "$D" | while IFS= read -r f; do
  sed -i -e "s#$SCR#<scratch>#g" -e "s#$HOME/kama-wt#<wt>#g" -e "s#$HOME#<home>#g" \
         -e "s#${T1}[0-9]*/[^ ]*#<scratch-kesik>#g" "$f"
done
if grep -rIl -e "$U" -e "$T1" -e "$T2" "$D"; then echo "YOL KALDI (yukarıdaki dosyalar)"; exit 1; fi
echo "yol denetimi temiz: $(find "$D" -type f | wc -l) dosya"
