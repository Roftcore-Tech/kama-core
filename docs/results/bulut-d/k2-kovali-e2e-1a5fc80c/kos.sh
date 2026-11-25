#!/usr/bin/env bash
# K2′ bağımsız düzeneğin koşusu (kart 7177073946): temiz ağaçta derler, test ikilisini İKİ KEZ
# koşar (araç doğrulaması dahil: --include-ignored), K2ROW/K2OZET satırlarının bayt-aynı olduğunu
# doğrular, ham kaydı K2BASLIK satırıyla yazar. Kirli ağaçta ya da deney ortam değişkeni kuruluyken
# durur (#175 7176932903: Yerel `scripts/ktp2/f0/k2/kos.sh` davranışına hizalı).
# Kullanım (depo kökünden): bash docs/results/bulut-d/k2-kovali-e2e-1a5fc80c/kos.sh <çıktı dosyası>
set -euo pipefail
out="${1:?çıktı dosyası}"
[ -z "${KAMA_PAIR_PROBE_DATAGRAMS:-}" ] || { echo "kos.sh: KAMA_PAIR_PROBE_DATAGRAMS kurulu" >&2; exit 2; }
[ -z "${K2_ONLY:-}" ] || { echo "kos.sh: K2_ONLY kurulu (tam koşu değil)" >&2; exit 2; }
[ -z "$(git status --porcelain)" ] || { echo "kos.sh: çalışma ağacı temiz değil" >&2; git status --short >&2; exit 2; }
sha=$(git rev-parse HEAD)
bin=$(cargo test --locked --release -p adaptive-transport --test k2_kovali_e2e --no-run \
  --message-format=json 2>/dev/null \
  | python3 -I -c 'import sys,json
for l in sys.stdin:
    try: m=json.loads(l)
    except Exception: continue
    if m.get("reason")=="compiler-artifact" and m.get("target",{}).get("name")=="k2_kovali_e2e" and m.get("executable"):
        print(m["executable"])' | tail -1)
[ -x "$bin" ] || { echo "kos.sh: test ikilisi bulunamadı" >&2; exit 2; }
komut="cargo test --locked --release -p adaptive-transport --test k2_kovali_e2e -- --include-ignored --nocapture --test-threads=1"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
for k in 1 2; do
  "$bin" --include-ignored --nocapture --test-threads=1 > "$tmp/ham$k.txt" 2>&1 \
    || { echo "kos.sh: koşu $k başarısız" >&2; tail -20 "$tmp/ham$k.txt" >&2; exit 1; }
  grep -o -E 'K2(ROW|OZET) .*' "$tmp/ham$k.txt" > "$tmp/satir$k.txt"
done
ayni=false; cmp -s "$tmp/satir1.txt" "$tmp/satir2.txt" && ayni=true
python3 -I - "$sha" "$(sha256sum "$bin" | cut -d' ' -f1)" "$(rustc -V)" "$komut" "$ayni" \
  "$(sha256sum "$tmp/satir1.txt" | cut -d' ' -f1)" \
  "$(grep -c '^K2ROW ' "$tmp/satir1.txt")" "$(grep -c '^K2OZET ' "$tmp/satir1.txt")" > "$out" <<'PY'
import json, sys
sha, ikili, rustc_v, komut, ayni, satir, nrow, noz = sys.argv[1:9]
print("K2BASLIK " + json.dumps({
    "sha": sha,
    "agac_temiz": True,
    "test": "crates/adaptive-transport/tests/k2_kovali_e2e.rs (kova_iki_tam_datagram_araligi, k2_kovali_e2e_rapor)",
    "komut": komut,
    "ikili_sha256": ikili,
    "rustc": rustc_v,
    "iki_kosu_bayt_ayni": ayni == "true",
    "satirlar_sha256": satir,
    "satirlar_kapsami": "K2ROW + K2OZET",
    "k2row": int(nrow),
    "k2ozet": int(noz),
}, ensure_ascii=False, separators=(",", ":")))
PY
cat "$tmp/satir1.txt" >> "$out"
echo "kos.sh: $out (sha $sha, iki koşu bayt-aynı: $ayni)"
