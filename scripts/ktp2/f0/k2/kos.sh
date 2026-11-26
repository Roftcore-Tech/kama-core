#!/usr/bin/env bash
# [K2 · #262] Kovalı kapalı döngü dar karşılaştırmasının koşu betiği (benzetim; hat koşusu DEĞİL).
# Gerçek `Sender` × ürün `Receiver` × kovalı darboğaz; iki vaka × deney açık / kapalı.
# Kullanım: kos.sh <çıktı dosyası>     (depo kökünden; CARGO değişkeni verilirse o komut kullanılır)
# Çıktının ilk satırı K2BASLIK (#175 7175939123 biçimi): kaynak SHA, ağaç temiz mi, test, komut,
# çalışan test ikilisinin sha256'sı, rustc, iki koşunun satırları bayt-aynı mı.
# Ağaç temiz değilse betik durur (kayıt SHA'ya bağlanamaz).
set -euo pipefail
out="${1:?çıktı dosyası}"
cargo_cmd="${CARGO:-cargo}"
[ -z "${KAMA_PAIR_PROBE_DATAGRAMS:-}" ] || { echo "kos.sh: KAMA_PAIR_PROBE_DATAGRAMS ortamda kurulu; deney kapalı kol bozulur" >&2; exit 2; }
[ -z "$(git status --porcelain)" ] || { echo "kos.sh: çalışma ağacı temiz değil; kayıt SHA'ya bağlanamaz" >&2; git status --short >&2; exit 2; }
sha="$(git rev-parse HEAD)"
t="adaptive-transport --test starved_line_sim k2_"
komut="cargo test --locked --release -p $t -- --nocapture --test-threads=1"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
# İkilinin yolu derleme iletisinden okunur (yeniden derlemeden).
bin="$($cargo_cmd test --locked --release -p adaptive-transport --test starved_line_sim --no-run --message-format=json 2>/dev/null \
  | python3 -c 'import json,sys
for l in sys.stdin:
    try: d=json.loads(l)
    except Exception: continue
    if d.get("reason")=="compiler-artifact" and d.get("executable") and d.get("target",{}).get("name")=="starved_line_sim": print(d["executable"])' | tail -1)"
[ -x "$bin" ] || { echo "kos.sh: test ikilisi bulunamadı" >&2; exit 2; }
for i in 1 2; do
  "$bin" k2_ --nocapture --test-threads=1 > "$tmp/ham$i.txt" 2>&1 || { echo "kos.sh: test koşusu $i başarısız" >&2; tail -20 "$tmp/ham$i.txt" >&2; exit 1; }
  # Test çalıştırıcısı satır sonu olmadan "test … ... " basar; kayıt satırı aynı satıra yapışabilir.
  grep -o -E 'K2(NOT|ARAC|ROW|OZET|BUTCE) .*' "$tmp/ham$i.txt" > "$tmp/satir$i.txt"
done
ayni=false; cmp -s "$tmp/satir1.txt" "$tmp/satir2.txt" && ayni=true
{
  printf 'K2BASLIK {"sha":"%s","agac_temiz":true,"test":"crates/adaptive-transport/tests/starved_line_sim.rs · k2_ (k2_bucket_model_matches_the_measured_gaps, k2_gate_refusal_counter_sees_a_binding_gate, k2_bucket_two_counterexamples_report)","komut":"%s","ikili_sha256":"%s","rustc":"%s","iki_kosu_bayt_ayni":%s,"satirlar_sha256":"%s","k2row":%s,"k2ozet":%s,"k2butce":%s}\n' \
    "$sha" "$komut" "$(sha256sum "$bin" | cut -d' ' -f1)" "$(rustc -V)" "$ayni" "$(sha256sum "$tmp/satir1.txt" | cut -d' ' -f1)" \
    "$(grep -c '^K2ROW ' "$tmp/satir1.txt")" "$(grep -c '^K2OZET ' "$tmp/satir1.txt")" "$(grep -c '^K2BUTCE ' "$tmp/satir1.txt")"
  cat "$tmp/satir1.txt"
} > "$out"
echo "kos.sh: $out yazıldı (sha $sha, iki koşu bayt-aynı: $ayni)"
