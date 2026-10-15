#!/usr/bin/env bash
# INT-04 — uygulama yolunun uçtan uca gece koşusu, netem altında.
# ROOTLESS: `unshare -r -n` ile kendi kullanıcı + ağ ad alanı (CAP_NET_ADMIN-in-userns); root,
# docker ya da self-hosted netem runner'ı GEREKMEZ. Ad alanı özeldir: `tc` yalnız bu koşuyu etkiler.
#
# NETEM NEREYE. Testin iki `KamaNode`'u AYNI SÜREÇTEDİR (zarf rölesi süreç-içidir, DUR-K4G-11 → B):
# tek süreç tek ağ ad alanındadır, uçlar ayrı ad alanlarına BÖLÜNEMEZ. İki yerel adres arasındaki
# trafiği çekirdek yerel olarak teslim eder — veth'e konan netem ATLANIR. Bu yüzden bozulma `lo`
# üzerine uygulanır; veri datagramları (UDP, 127.0.0.1) oradan geçer. El sıkışma zarfları süreç-içi
# röleden geçer ve netem'e UĞRAMAZ (testin modül belgesinde beyanlı boşluk; INT-02 ile aynı).
#
# Kullanım:  scripts/netem/int04_netns.sh [test-ikilisi]
#   İkili verilmezse `cargo test --no-run` ile üretilir (derleme ad alanının DIŞINDA yapılır:
#   içeride ağ yoktur).
# Ortam:  NETEM_PROFILE (vars. "delay 50ms loss 5%") · BLACKOUT_S (vars. 60)
set -euo pipefail

PROFILE=${NETEM_PROFILE:-"delay 50ms loss 5%"}
BLACKOUT_S=${BLACKOUT_S:-60}
TEST_NAME=app_path_survives_netem_session_change_and_a_blackout

command -v unshare >/dev/null || { echo "::error title=INFRA_FAILURE::unshare yok"; exit 1; }
command -v tc >/dev/null || { echo "::error title=INFRA_FAILURE::tc (iproute2) yok"; exit 1; }

BIN=${1:-}
if [[ -z "$BIN" ]]; then
  echo "== test ikilisi derleniyor (ad alanının DIŞINDA: içeride ağ yok) =="
  BIN=$(cargo test -p kama-integration --test int04_app --no-run --message-format=json 2>/dev/null \
    | python3 -c '
import json,sys
for line in sys.stdin:
    try: rec = json.loads(line)
    except ValueError: continue
    if rec.get("target", {}).get("name") == "int04_app" and rec.get("executable"):
        print(rec["executable"])
')
fi
[[ -n "$BIN" && -x "$BIN" ]] || { echo "::error title=INFRA_FAILURE::test ikilisi bulunamadı: ${BIN:-<boş>}"; exit 1; }
echo "== ikili: $BIN"

export BIN PROFILE BLACKOUT_S TEST_NAME
unshare -r -n bash <<'INNER'
set -euo pipefail
ip link set lo up

echo "== netem: dev lo root netem $PROFILE"
tc qdisc replace dev lo root netem $PROFILE
tc qdisc show dev lo

# KANIT (varsayma, ölç): profil gerçekten uygulandı mı? `delay <N>ms` istendiyse gidiş-dönüş en az
# 2×N olmalıdır (netem gecikmeyi YÖN BAŞINA uygular; lo'da paket iki kez kuyruğa girer).
# Sonda BAŞARISIZ OLABİLİR ve bu koşuyu DÜŞÜRMEZ: profilde kayıp varken ping'in bütün paketleri
# düşebilir (ölçüldü) ya da ping kurulu olmayabilir. Düşüren tek şey ÖLÇÜLMÜŞ ve DÜŞÜK bir RTT'dir
# — o zaman netem gerçekten uygulanmamıştır. Yapılandırmanın kendisi yukarıdaki `tc qdisc show`da.
want_ms=$(sed -n 's/.*delay \([0-9]\+\)ms.*/\1/p' <<<"$PROFILE")
if [[ -n "$want_ms" ]] && command -v ping >/dev/null; then
  rtt=$(ping -n -c 5 -i 0.3 -W 5 127.0.0.1 2>/dev/null | sed -n 's|.*= [0-9.]*/\([0-9.]*\)/.*|\1|p' || true)
  echo "INT04_NETEM_PROBE rtt_ms=${rtt:-olculemedi} want_ms=$want_ms"
  if [[ -n "$rtt" ]]; then
    python3 -c "import sys; sys.exit(0 if float('$rtt') >= 2*float('$want_ms')*0.8 else 1)" \
      || { echo "::error title=INFRA_FAILURE::netem uygulanmamis (rtt=$rtt ms, beklenen >= $((want_ms*2)) ms)"; exit 1; }
  fi
fi

export KAMA_INT04_NETEM_DEV=lo
export KAMA_INT04_NETEM_BASE="$PROFILE"
export KAMA_INT04_BLACKOUT_S="$BLACKOUT_S"
export RUST_BACKTRACE=1

echo "== INT-04 koşuyor (kesinti ${BLACKOUT_S}s) =="
"$BIN" --ignored --exact --nocapture "$TEST_NAME"
INNER
