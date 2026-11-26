#!/usr/bin/env bash
# K2′ bağımsız düzeneğin koşusu: temiz ağaçta derler, test ikilisini İKİ KEZ koşar, K2ROW/K2OZET
# satırlarının bayt-aynı olduğunu doğrular ve ham kaydı K2BASLIK satırıyla yazar.
# Kullanım (depo kökünden): bash docs/results/bulut-d/k2-kovali-e2e-ee168d3d/kos.sh <çıktı dosyası>
set -euo pipefail
out="${1:?çıktı dosyası}"
unset K2_ONLY
sha=$(git rev-parse HEAD)
if [ -z "$(git status --porcelain)" ]; then temiz=true; else temiz=false; fi
bin=$(cargo test --locked --release -p adaptive-transport --test k2_kovali_e2e --no-run \
  --message-format=json 2>/dev/null \
  | python3 -I -c 'import sys,json
for l in sys.stdin:
    m=json.loads(l)
    if m.get("reason")=="compiler-artifact" and m.get("target",{}).get("name")=="k2_kovali_e2e" and m.get("executable"):
        print(m["executable"])' | tail -1)
ikili=$(sha256sum "$bin" | cut -d' ' -f1)
rustc_v=$(rustc -V)
komut="cargo test --locked --release -p adaptive-transport --test k2_kovali_e2e -- --ignored --nocapture --test-threads=1"
tmp=$(mktemp -d)
for k in 1 2; do
  "$bin" --ignored --nocapture --test-threads=1 2>&1 | grep -E '^K2(ROW|OZET) ' > "$tmp/kos$k.txt"
done
if cmp -s "$tmp/kos1.txt" "$tmp/kos2.txt"; then ayni=true; else ayni=false; fi
python3 -I - "$sha" "$temiz" "$ikili" "$rustc_v" "$komut" "$ayni" "$(sha256sum "$tmp/kos1.txt" | cut -d' ' -f1)" > "$out" <<'PY'
import json, sys
sha, temiz, ikili, rustc_v, komut, ayni, satir = sys.argv[1:8]
print("K2BASLIK " + json.dumps({
    "sha": sha,
    "agac_temiz": temiz == "true",
    "test": "crates/adaptive-transport/tests/k2_kovali_e2e.rs::k2_kovali_e2e_rapor",
    "komut": komut,
    "ikili_sha256": ikili,
    "rustc": rustc_v,
    "olcülen_kod": "ee168d3d77920632359e537f3e64678c8c9c852d",
    "ortam_degiskeni": "K2_ONLY yok (tam koşu); başka ortam, duvar saati ya da iş parçacığı bağımlılığı yok",
    "iki_kosu_bayt_ayni": ayni == "true",
    "satirlar_sha256": satir,
}, ensure_ascii=False))
PY
cat "$tmp/kos1.txt" >> "$out"
echo "ikili=$bin ayni=$ayni"
