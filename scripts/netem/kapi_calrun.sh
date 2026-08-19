#!/usr/bin/env bash
# [KAPI-1] Kampanya koşu sürücüsü — manifest(JSONL) × kol → taze 3-netns'te uçtan uca yürütme.
#
# Akış (rep-kol başına, SERİ; hücreler arası paralellik YOK — CPU/timing izolasyonu):
#   unshare -rnm → kapi3_setup → [A2] netem uygula → capture başlat → server (ns-B) →
#   client (ns-A, --gate ile) → client .ready yazınca: netem AKTİF doğrula (barrier) +
#   realized-log → .go dokun (ölçüm-t0) → client biter → realized-log → teardown → toplama.
#
# Kollar manifest .arms'tan; sıra .kama_first ile (dengeli randomizasyon, r5 I).
# ÇIKTI: <out>/<cell>/<rep>/<kol>/{out.jsonl, client.log, server.log, realized_*.json, cap/*}
#
# --smoke: ARAÇ-DOĞRULAMA modu (result-bearing DEĞİL): msgs=8, D=8s, warmup=3, paced-interval=200ms
#          — uçtan uca tesisatı dakikalar içinde sınar; çıktı dizini "-SMOKE" damgalı.
#
# Kullanım: kapi_calrun.sh <manifest.jsonl> <out_dir> [--limit N] [--only CELL_REGEX] [--smoke]
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$DIR/../.." && pwd)"
BENCH="$REPO/target/release/bench"

# ── iç mod: tek rep-kol koşusu (unshare -rnm içinde) ─────────────────────────────────────────
if [ "${1:-}" = "--inner-run" ]; then
  ROW_FILE="$2"; ARM="$3"; OUT="$4"; SMOKE="${5:-0}"
  source "$DIR/kapi3_harness.sh"
  mkdir -p "$OUT"
  trap 'kill $(jobs -p) 2>/dev/null; kapi3_teardown' EXIT

  jqr() { jq -r "$1" "$ROW_FILE"; }
  TRANSPORT=$(jqr ".arms[\"$ARM\"].transport")
  KAMA_ARM=$(jqr ".arms[\"$ARM\"].kama_arm")
  MSGS=$(jqr .msgs); PAYLOAD=$(jqr .payload); DEADLINE=$(jqr .deadline_s)
  INTERVAL=$(jqr .submit_interval_ms); WARMUP=$(jqr .warmup_msgs); SEED=$(jqr .seed)
  REP=$(jqr .rep); PROFILE=$(jqr .profile.profile_id); QUOTA=$(jqr .quota_r)
  NET_FWD=$(jqr .profile.netem_fwd); NET_REV=$(jqr .profile.netem_rev)
  WORKLOAD=$(jqr .workload); PLANE=$(jqr .plane)
  TRACE_WINDOWS=$(jq -r '.profile.trace_windows // empty' "$ROW_FILE")
  # [inceleme #6] impairment uygulayıcı: statik profil → kapi3_netem; trace profil → manifest'e
  # gömülü pencere planı kapi3_trace ile (AYNI plan her kola = trace-replay, r5 J). "trace"
  # literalinin tc'ye sızması YASAK.
  apply_impairment() {
    if [ -n "$TRACE_WINDOWS" ]; then
      printf '%s' "$TRACE_WINDOWS" > "$OUT/trace_plan.txt"
      kapi3_trace "$OUT/trace_plan.txt"
    else
      kapi3_netem "$NET_FWD" "$NET_REV"
    fi
  }
  # Barrier doğrulaması: statikte İKİ yön qdisc; trace'te uygulayıcı-PID canlı. [inceleme #5/#14]
  verify_impairment() {
    if [ -n "$TRACE_WINDOWS" ]; then
      kill -0 "${KAPI3_TRACE_PID:-0}" 2>/dev/null && return 0
      echo "BARRIER-FAIL: trace uygulayıcısı ölü" >> "$OUT/driver.log"; return 1
    fi
    local ok=0
    if [ "$NET_FWD" != "none" ] && ! kapi3_mid tc qdisc show dev mb0 | grep -q netem; then
      echo "BARRIER-FAIL: fwd netem kurulmamış" >> "$OUT/driver.log"; ok=1
    fi
    if [ "$NET_REV" != "none" ] && ! kapi3_mid tc qdisc show dev ma0 | grep -q netem; then
      echo "BARRIER-FAIL: rev netem kurulmamış" >> "$OUT/driver.log"; ok=1
    fi
    return $ok
  }
  if [ "$SMOKE" = 1 ]; then
    MSGS=8; PAYLOAD=64; DEADLINE=8; WARMUP=3
    [ "$INTERVAL" != 0 ] && INTERVAL=200
  fi

  # Bütçe CLI argları (fwd=client, rev=server; r5 D yön-başına).
  budget_args() { # $1=fwd|rev
    local side="$1"
    if [ "$WORKLOAD" = burst ]; then
      echo "--budget-burst-bytes $(jqr .budget.$side.burst_bytes)"
    else
      echo "--budget-refill-bytes-per-s $(jqr .budget.$side.refill_bytes_per_s) \
            --budget-bucket-bytes $(jqr .budget.$side.bucket_bytes) \
            --budget-ceiling-bytes $(jqr .budget.$side.ceiling_bytes)"
    fi
  }
  # SMOKE'ta bütçe kapatılır (tesisat sınaması; bütçe pinleri bench testlerinde).
  FWD_BUDGET=$([ "$SMOKE" = 1 ] && echo "" || budget_args fwd)
  REV_BUDGET=$([ "$SMOKE" = 1 ] && echo "" || budget_args rev)

  kapi3_setup
  # [inceleme #29] Altyapı sağlığı: topoloji gerçekten forward ediyor mu (temiz ağda hızlı echo)?
  nsenter -t "$CHLB" -n python3 "$DIR/udp_echo.py" server 10.10.1.1 9990 > "$OUT/echo.ready" 2>/dev/null < /dev/null & ECHO=$!
  for _ in $(seq 1 40); do grep -q READY "$OUT/echo.ready" 2>/dev/null && break; sleep 0.05; done
  HC=$(kapi3_a python3 "$DIR/udp_echo.py" client 10.10.1.1 9990 3 1.0 | jq .received)
  kill "$ECHO" 2>/dev/null
  if [ "${HC:-0}" -lt 3 ]; then
    echo "INFRA-FAIL: temiz-ağ echo $HC/3 — topoloji bozuk" >> "$OUT/driver.log"
    touch "$OUT/INVALID"; exit 1
  fi
  # DÜZLEM-A: kuruluş TEMİZ ağda (netem HENÜZ yok; 14-alan tablosu session_establishment_network).
  kapi3_capture_start "$OUT/cap"
  kapi3_realized "$OUT" pre

  GATE_E="$OUT/gate_e"; GATE_M="$OUT/gate_m"
  # Kalıntı-durum temizliği: başarısız önceki denemeden kalan gate dosyaları barrier'ı ATLATIRDI
  # (bayat .go → client beklemeden geçer, server erken arm eder). Her koşu taze gate'le başlar.
  rm -f "$GATE_E.ready" "$GATE_E.go" "$GATE_M.ready" "$GATE_M.go"
  # --gate-watch: reverse-bütçe arm'ı client'ın ölçüm-t0'ına senkron (warmup-ACK'leri muaf, r5 D.2).
  # DİKKAT: fonksiyon-sarmalı ARKA-PLANA ALINMAZ (alt-kabuk fork → kill hedefe ulaşmaz, sızıntı).
  nsenter -t "$CHLB" -n "$BENCH" serve --bind 10.10.1.1:4433 --transport "$TRANSPORT" --quota-r "$QUOTA" \
    $REV_BUDGET --gate-watch "$GATE_M" > "$OUT/server.log" 2>&1 < /dev/null & SRV=$!
  sleep 1
  if ! kill -0 "$SRV" 2>/dev/null; then
    echo "INFRA-FAIL: server 1 s içinde öldü (bkz. server.log)" >> "$OUT/driver.log"
    touch "$OUT/INVALID"; exit 1
  fi

  # DİKKAT: kapi3_a sarmalısını arka-plana alma (alt-kabuk → kill CLI bench'e ulaşmaz);
  # A = mevcut netns, doğrudan çalıştır. timeout TERM'i çocuğa iletir.
  # shellcheck disable=SC2086
  timeout $(( DEADLINE + WARMUP * (INTERVAL>0 ? INTERVAL : 1000) / 1000 + (MSGS-1) * INTERVAL / 1000 + 90 )) \
    "$BENCH" client --server 10.10.1.1:4433 --transport "$TRANSPORT" --kama-arm "$KAMA_ARM" \
    --msgs "$MSGS" --payload "$PAYLOAD" --max-deadline-s "$DEADLINE" \
    --submit-interval-ms "$INTERVAL" --warmup-msgs "$WARMUP" \
    --establish-gate "$GATE_E" --gate "$GATE_M" \
    --seed "$SEED" --repeat "$REP" --profile "$PROFILE" $FWD_BUDGET \
    --out "$OUT/out.jsonl" > "$OUT/client.log" 2>&1 < /dev/null & CLI=$!

  # SENKRON-1 (kuruluş): temiz ağda bağlantı kuruldu → A2'de netem BURADA uygulanır
  # (warmup impaired koşar); A1'de burada uygulanMAZ (warmup temiz — converge).
  for _ in $(seq 1 6000); do [ -e "$GATE_E.ready" ] && break; kill -0 "$CLI" 2>/dev/null || break; sleep 0.05; done
  if [ ! -e "$GATE_E.ready" ]; then
    echo "INFRA-FAIL: client kuruluş-ready yazmadan öldü (temiz ağda kuruluş başarısız olmamalı)" >> "$OUT/driver.log"
    touch "$OUT/INVALID"; kill "$CLI" "$SRV" 2>/dev/null; exit 1
  fi
  if [ "$PLANE" = "A2" ]; then apply_impairment; fi
  kapi3_realized "$OUT" establish
  touch "$GATE_E.go"

  # SENKRON-2 (barrier, r5 B.2): warmup+quiescence bitti → A1'de netem BURADA uygulanır;
  # her iki düzlemde de aktivasyon DOĞRULANIR; .go = ölçüm-t0.
  for _ in $(seq 1 6000); do [ -e "$GATE_M.ready" ] && break; kill -0 "$CLI" 2>/dev/null || break; sleep 0.05; done
  if [ ! -e "$GATE_M.ready" ]; then
    echo "INFRA-FAIL: client ölçüm-ready yazmadan öldü" >> "$OUT/driver.log"
    touch "$OUT/INVALID"; kill "$CLI" "$SRV" 2>/dev/null; exit 1
  fi
  if [ "$PLANE" = "A1" ]; then apply_impairment; fi
  # [inceleme #5/#15/#25] Barrier ÖLÜMCÜL: doğrulanamayan impairment ile koşu ASLA ölçülmez —
  # .go yazılmaz, client öldürülür, hücre INVALID işaretlenir (aggregation'a giremez).
  if ! verify_impairment; then
    touch "$OUT/INVALID"; kill "$CLI" "$SRV" 2>/dev/null; exit 1
  fi
  kapi3_realized "$OUT" barrier
  touch "$GATE_M.go"   # ölçüm-t0

  wait "$CLI"; CLI_RC=$?
  echo "client rc=$CLI_RC" >> "$OUT/driver.log"
  kapi3_realized "$OUT" post
  kill "$SRV" 2>/dev/null
  kapi3_capture_stop
  exit 0
fi

# ── dış mod: manifest döngüsü ────────────────────────────────────────────────────────────────
MANIFEST="${1:?manifest.jsonl}"; OUTDIR="${2:?out_dir}"
LIMIT=0; ONLY=""; SMOKE=0
shift 2
while [ $# -gt 0 ]; do
  case "$1" in
    --limit) LIMIT="$2"; shift 2;;
    --only)  ONLY="$2"; shift 2;;
    --smoke) SMOKE=1; OUTDIR="${OUTDIR%-SMOKE}-SMOKE"; shift;;
    *) echo "bilinmeyen arg: $1"; exit 2;;
  esac
done
[ -x "$BENCH" ] || { echo "HATA: $BENCH yok — önce: cargo build -j2 --release -p bench"; exit 2; }
mkdir -p "$OUTDIR"
sha256sum "$BENCH" "$MANIFEST" > "$OUTDIR/provenance.txt"
git -C "$REPO" rev-parse HEAD >> "$OUTDIR/provenance.txt" 2>/dev/null || true
[ "$SMOKE" = 1 ] && echo "MODE: SMOKE (araç-doğrulama; result-bearing DEĞİL)" >> "$OUTDIR/provenance.txt"

COUNT=0
while IFS= read -r row; do
  CELL=$(echo "$row" | jq -r .cell_id | tr ':' '_')
  REP=$(echo "$row" | jq -r .rep)
  [ -n "$ONLY" ] && ! echo "$CELL" | grep -qE "$ONLY" && continue
  KF=$(echo "$row" | jq -r .kama_first)
  if [ "$KF" = true ]; then ORDER="kama-arq kama-tail-v1 q-stream q-dgram-arq"
  else ORDER="q-stream q-dgram-arq kama-arq kama-tail-v1"; fi
  for ARM in $ORDER; do
    OUT="$OUTDIR/$CELL/rep$REP/$ARM"
    [ -e "$OUT/out.jsonl" ] && { echo "SKIP (var): $OUT"; continue; }
    mkdir -p "$OUT"
    rm -f "$OUT/INVALID"
    echo "$row" > "$OUT/row.json"
    echo "RUN: $CELL rep$REP $ARM"
    unshare -rnm bash "$0" --inner-run "$OUT/row.json" "$ARM" "$OUT" "$SMOKE" \
      > "$OUT/unshare.log" 2>&1 < /dev/null
    if [ -e "$OUT/out.jsonl" ]; then echo "  OK: out.jsonl"
    elif [ -e "$OUT/INVALID" ]; then echo "  INVALID: altyapı/barrier hatası (bkz. $OUT/driver.log) — ölçüm sayılamaz"
    else echo "  FAIL: out.jsonl YOK (bkz. $OUT/*.log)"; fi
  done
  COUNT=$((COUNT + 1))
  [ "$LIMIT" -gt 0 ] && [ "$COUNT" -ge "$LIMIT" ] && break
done < "$MANIFEST"
echo "BITTI: $COUNT manifest satırı işlendi → $OUTDIR"
