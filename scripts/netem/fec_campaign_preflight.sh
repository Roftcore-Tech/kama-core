#!/usr/bin/env bash
# [FEC-KAMPANYA ÖN-UÇUŞ KAPISI] — fail-closed.
#
# NEDEN VAR: bu repodaki her FEC sonucu ya geri çekildi ya taşımadı, ve her seferinde sebep
# koşudan ÖNCE mekanik olarak yakalanabilecek bir şeydi. Adversaryal tasarım incelemesi 25 saldırı
# üretti; aşağıdakiler assertion'a çevrilebilenler. Bir ÖNKOŞUL PROGRAM DEĞİLSE CÜMLEDİR, ve bu
# reponun defteri cümlelerin tutmadığını gösteriyor.
#
# Kullanım: fec_campaign_preflight.sh OUTDIR
# Çıktı: OUTDIR/preflight.txt (ön-kayıtla BİRLİKTE commit edilir, hash'i freeze kaydına girer).
set -u
DIR="$(cd "$(dirname "$0")" && pwd)"; REPO="$(cd "$DIR/../.." && pwd)"
OUT="${1:?kullanım: fec_campaign_preflight.sh OUTDIR}"
mkdir -p "$OUT"; LOG="$OUT/preflight.txt"
BENCH="$REPO/target/release/bench"
FAIL=0
say() { printf '%s\n' "$*" | tee -a "$LOG"; }
ok()  { say "  PASS  $*"; }
bad() { say "  FAIL  $*"; FAIL=1; }

: > "$LOG"
say "=== FEC KAMPANYA ÖN-UÇUŞ KAPISI  $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
say "repo=$(git -C "$REPO" rev-parse HEAD)  dirty=$(git -C "$REPO" status --porcelain | wc -l)"

# ── G1: binary var ve kampanya kolları TEMSİL EDİLEBİLİR ───────────────────────────────────────
say ""; say "[G1] kol temsil edilebilirliği (kill-shot: statik r=0 ulaşılamazdı)"
[ -x "$BENCH" ] || { bad "release binary yok: $BENCH"; say "SONUC=FAIL"; exit 1; }
if "$BENCH" client --help 2>&1 | grep -q -- "--fec-mode"; then
  ok "--fec-mode seçicisi var (r'den ortogonal)"
else
  bad "--fec-mode YOK → statik r=0 kontrol kolu temsil edilemez"
fi

# ── G2: KOD FİZİBİLİTESİ — kayıp oranı MDS sınırının içinde mi ────────────────────────────────
# K=8 blokta ≤r silme çözülebilir. P(blok çözülebilir) = sum_{i=0..r} C(K+r,i) p^i (1-p)^(K+r-i).
# Bu kapı olmadan kampanya, kodun ÇÖZEMEYECEĞİ bir bölgede parity maliyeti ölçer (geçmişte oldu).
say ""; say "[G2] MDS fizibilitesi (P(blok çözülebilir) ≥ eşik)"
MIN_DECODE="${MIN_DECODE:-0.25}"
python3 - "$MIN_DECODE" "$OUT" <<'PY' | tee -a "$LOG"
import sys, math
floor=float(sys.argv[1])
K=8
cells=[("iid15",0.15),("iid20",0.20),("iid30",0.30),("iid50",0.50)]
def pdec(p,K,r):
    n=K+r
    return sum(math.comb(n,i)*p**i*(1-p)**(n-i) for i in range(r+1))
worst_ok=True
for name,p in cells:
    row=[]
    for r in (2,4):
        d=pdec(p,K,r); row.append(f"r={r}:{d:6.1%}")
        if p in (0.15,0.20,0.30) and d<floor: worst_ok=False
    print(f"  {name:8s} p={p:4.0%}  " + "  ".join(row))
print(f"  eşik={floor:.0%} — aday hücreler (iid15/20/30) eşiği geçmeli")
print("  PASS  MDS fizibilitesi" if worst_ok else "  FAIL  aday hücre eşiğin altında")
PY
grep -q "FAIL  aday hücre" "$LOG" && FAIL=1

# ── G3: BLOK HİZASI — kısa blok flush edilmiyor, son mesaj korumasız kalmasın ──────────────────
say ""; say "[G3] blok hizası (kısa blok FLUSH EDİLMİYOR → artık birimler korumasız)"
python3 - "${N_MSGS:-96}" "${PAYLOAD:-256}" "${CHUNK_CAP:-53}" <<'PY' | tee -a "$LOG"
import sys, math
n=int(sys.argv[1]); pay=int(sys.argv[2]); cap=int(sys.argv[3]); K=8
per=math.ceil(pay/cap); units=n*per; orphan=units%K
print(f"  N={n} payload={pay}B chunk_cap={cap}B → {per} chunk/mesaj, {units} birim, K={K}")
print(f"  artık birim = {orphan}")
print("  PASS  blok hizası tam (son mesaj korumasız kalmıyor)" if orphan==0
      else f"  FAIL  {orphan} artık birim → SON mesaj parity ALMIYOR; N'i K'ya hizala")
PY
grep -q "FAIL  .* artık birim" "$LOG" && FAIL=1

# ── G4: DEADLINE SEMANTİĞİ — d_msg ≤ d_run_grace ──────────────────────────────────────────────
say ""; say "[G4] deadline semantiği (d_msg skorlanan, d_run_grace koşu ufku)"
python3 - "${D_MSG_MAX_MS:-3200}" "${D_RUN_GRACE_MS:-20000}" <<'PY' | tee -a "$LOG"
import sys
d_msg=int(sys.argv[1]); d_run=int(sys.argv[2])
print(f"  max(d_msg)={d_msg} ms   d_run_grace={d_run} ms")
print("  PASS  d_msg ≤ d_run_grace (skorlama ufkun içinde)" if d_msg<=d_run
      else "  FAIL  d_msg > d_run_grace → hiçbir mesaj o eşikte skorlanamaz (sahte sıfır)")
PY
grep -q "FAIL  d_msg" "$LOG" && FAIL=1

# ── G5: HARNESS SADAKATİ — netem gerçekten uyguluyor mu (ölç, VARSAYMA) ────────────────────────
# Geçmişte tek-namespace harness ne gecikme ne kayıp uyguluyordu ve "FEC gereksiz" sonucu
# BU YÜZDEN çıkmıştı. Ölçülmemiş bir kanal, ölçüm değildir.
say ""; say "[G5] harness sadakati (ölçülen RTT + ölçülen drop)"
if [ -z "${PF_INNER:-}" ]; then
  PF_INNER=1 unshare -rnm bash "$0" "$OUT" 2>&1 | grep -E "^\[G[56]\]|  (PASS|FAIL)  " | tee -a "$LOG"
else
  source "$DIR/kapi3_harness.sh"
  trap 'kapi3_teardown' EXIT
  kapi3_setup
  kapi3_netem "delay ${OWD_MS:-100}ms loss ${LOSS_PCT:-20}%" "delay ${OWD_MS:-100}ms loss ${REV_LOSS_PCT:-2}%"
  PING=$(ping -c 20 -i 0.05 -q 10.10.1.1 2>/dev/null | tail -2)
  RTT=$(echo "$PING" | grep -oE 'rtt min/avg[^=]*= [0-9.]+/[0-9.]+' | grep -oE '[0-9.]+$')
  DROP=$(echo "$PING" | grep -oE '[0-9.]+% packet loss' | grep -oE '^[0-9.]+')
  echo "  ölçülen RTT=${RTT:-?} ms   ölçülen ping-drop=${DROP:-?}%   (konfigüre: 2×${OWD_MS:-100}ms, fwd ${LOSS_PCT:-20}%)"
  python3 - "${RTT:-0}" "${OWD_MS:-100}" "${DROP:-0}" "${LOSS_PCT:-20}" <<'PY'
import sys
rtt=float(sys.argv[1]); owd=float(sys.argv[2]); drop=float(sys.argv[3]); loss=float(sys.argv[4])
exp=2*owd
ok_rtt = exp*0.85 <= rtt <= exp*1.25
# ping çift yönlü: yaklaşık 1-(1-fwd)(1-rev); kaba alt sınır olarak fwd'nin yarısını isteriz
ok_drop = drop >= loss*0.5
print(f"  PASS  ölçülen RTT beklenen {exp:.0f} ms ile uyumlu" if ok_rtt else
      f"  FAIL  ölçülen RTT {rtt} ms, beklenen {exp:.0f} ms — netem gecikmeyi UYGULAMIYOR")
print(f"  PASS  netem kaybı gerçekten uyguluyor" if ok_drop else
      f"  FAIL  ölçülen drop {drop}% ≪ konfigüre {loss}% — netem kaybı UYGULAMIYOR")
PY
fi
grep -q "FAIL  ölçülen RTT\|FAIL  netem" "$LOG" && FAIL=1

# ── G6: PİLOT — kollar GERÇEKTEN koşuyor ve FARKLILAŞIYOR mu (bayrak varlığı YETMEZ) ──────────
# Bunu ilk sürümde atladım ve bedelini ödedim: G1 `--fec-mode`'un VARLIĞINI doğruladı, ama
# `validate_fec` `manual_r = 0`'ı REDDEDİYORDU → r0 kontrol kolunun HER rep'i FAILED_rc=1 oldu ve
# ilk analizcimde kol çıktıdan sessizce kayboldu. Bayrak temsil edilebilirliği ≠ kol çalışırlığı.
say ""; say "[G6] pilot: kollar koşuyor ve farklılaşıyor mu"
if [ -z "${PF_PILOT_DONE:-}" ] && [ -n "${PF_INNER:-}" ]; then
  PILOT="$OUT/pilot"; rm -rf "$PILOT"; mkdir -p "$PILOT"
  kapi3_netem "delay ${OWD_MS:-100}ms loss ${LOSS_PCT:-20}% rate 1mbit" \
              "delay ${OWD_MS:-100}ms loss ${REV_LOSS_PCT:-2}% rate 1mbit"
  PP=5900
  for r in 0 2; do
    PP=$((PP+1)); d="$PILOT/r$r"; mkdir -p "$d"
    nsenter -t "$CHLB" -n "$BENCH" serve --bind "10.10.1.1:$PP" --transport kama-product \
      --fec-mode static --fec-profile 3 --fec-r "$r" --payload 256 2>"$d/s.err" >/dev/null &
    sp=$!; sleep 1
    timeout 40 "$BENCH" client --server "10.10.1.1:$PP" --transport kama-product \
      --fec-mode static --fec-profile 3 --fec-r "$r" --msgs 96 --payload 256 \
      --submit-interval-ms 20 --max-deadline-s 20 --out "$d/out.jsonl" 2>"$d/c.err" >/dev/null
    echo "rc=$?" > "$d/rc"; kill "$sp" 2>/dev/null; wait "$sp" 2>/dev/null || true
  done
  for r in 0 2; do
    d="$PILOT/r$r"; rc=$(cat "$d/rc" 2>/dev/null)
    tx=$(grep -oE 'fec_repairs_tx=[0-9]+' "$d/c.err" 2>/dev/null | grep -oE '[0-9]+' | head -1)
    if [ "$rc" != "rc=0" ] || [ ! -s "$d/out.jsonl" ]; then
      bad "r=$r kolu KOŞMADI ($rc) — $(head -c 120 "$d/c.err" 2>/dev/null)"
    elif [ "$r" = "0" ] && [ "${tx:-0}" != "0" ]; then
      bad "r=0 kolu parity BASTI (tx=$tx) — kol farklılaşması ihlali"
    elif [ "$r" = "2" ] && [ "${tx:-0}" = "0" ]; then
      bad "r=2 kolu parity BASMADI — vakum kol"
    else
      ok "r=$r kolu koştu ve beklendiği gibi farklılaştı (fec_repairs_tx=${tx:-0})"
    fi
  done
fi

say ""
if [ "$FAIL" -eq 0 ]; then say "SONUC=PASS — kampanya dondurulabilir"; else say "SONUC=FAIL — DONDURMA YOK"; fi
say "sha256(preflight)=$(sha256sum "$LOG" | cut -d' ' -f1)"
exit "$FAIL"
