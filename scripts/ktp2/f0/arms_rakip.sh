# shellcheck shell=bash
# [KTP2-F0 / Ajan D] RAKİP KOL ŞABLONLARI (quinn, tcp) — hücreler `source` eder, `rakip_arms` çağırır.
# Kama kolları arms_kama.sh'dedir (Ajan B, `kama_arms`). İkisi AYNI geometri değişkenlerini okur ve
# AYNI iş yükü argümanlarını üretir → bir hücrenin dört kolu aynı yükü koşar, `bench fairness`
# engelleyici fark bulmaz. Source etmek hiçbir şey kurmaz/doğrulamaz; her şey `rakip_arms` içinde.
#
# GİRDİ (hücre dosyası `rakip_arms`'tan ÖNCE tanımlar; arms_kama.sh ile aynı adlar):
#   MSGS PAY IVAL_MS DEADLINE_S    mesaj sayısı · yük B · sunum aralığı ms · mesaj başına pencere s
#   FWD REV                        netem dizeleri (manifeste AYNEN yazılır)
#   HS_TIMEOUT_S   (isteğe bağlı)  el sıkışma/bağlanma tavanı s; varsayılan = DEADLINE_S. Kurulum
#                                  teslim penceresinin DIŞINDA kalır, setup_ns olarak ayrı yazılır.
#   RTT_HINT_MS    (isteğe bağlı)  yalnız quinn_irtt kolu: quinn initial_rtt
#   RAKIP_RXLOG    (isteğe bağlı)  1 (varsayılan) = sunucular @D@/rx_log.jsonl alıcı damgası yazar
#                                  (şema ktp2-c-rx-v1; quinn istemcisi --payload-id); 0 = kapalı
# ÇIKTI: ARM_<kol>_SRV / ARM_<kol>_CLI (f0_cell.sh yer tutucularıyla) ve RAKIP_LOAD_ARGS (ortak iş
#   yükü argümanları; Kama kolu da kullanabilir: `... client --transport kama-product $RAKIP_LOAD_ARGS`).
#
# KOLLAR (ARMS'a yazılan adlar):
#   quinn        BİRİNCİL. quinn 0.11 (quinn-proto 0.11.18) BBR · mesaj başına bağımsız TEK-YÖNLÜ
#                stream · teslim = eşin taşıma ACK'i (SendStream::stopped; uygulama yanıtı yok) ·
#                idle 300 s · initial_rtt/IW quinn varsayılanı (333 ms / BBR 240 000 B) ·
#                el sıkışma tavanı HS_TIMEOUT_S · QUINN-STATS satırı c.err'de.
#   tcp          BİRİNCİL. TEK TCP bağlantısı · çekirdek CC (tcp_cc.txt'ye yazılır; bu makinede
#                cubic) · TCP_NODELAY · teslim = snd_una mesajın sonunu geçti (/proc tx_queue, 5 ms)
#                · sunum anı = plan · bağlanma tavanı HS_TIMEOUT_S · hepsi teslimde erken biter.
#   quinn_rpl    betimsel. Eski kol (KATALOG §1, önceki kampanyalar): bi-stream + 1 B uygulama
#                yanıtı, el sıkışma tavanı = --max-deadline-s.
#   tcp_rpl      betimsel. Eski kol: 8 B uygulama ACK'i, bağlanma tavanı 20 s, yazma-anı damgası;
#                --early-exit (İSTEK-D1: teslim olayları değişmez, yalnız boş bekleme kesilir).
#   quinn_cubic  betimsel. `quinn` ile aynı, CC = Cubic (kayba dayalı).
#   quinn_irtt   betimsel. `quinn` + initial_rtt = RTT_HINT_MS + IW 12 000 B (RFC 9002 §7.2, 1200 B
#                datagram; = quinn Cubic varsayılanı). Yalnız RTT_HINT_MS verilmişse tanımlanır.
#
# İstemciler scripts/ktp2/f0/d/rival_wrap.sh ile sarılır (koşu öncesi/sonrası istemci netns'inin
# /proc/net/snmp + netstat görüntüsü + TCP CC adı; komut ve rc aynen).

RAKIP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

rakip_arms() {
  : "${MSGS:?rakip_arms: MSGS yok}" "${PAY:?rakip_arms: PAY yok}" "${IVAL_MS:?rakip_arms: IVAL_MS yok}"
  : "${DEADLINE_S:?rakip_arms: DEADLINE_S yok}" "${FWD:?rakip_arms: FWD yok}" "${REV:?rakip_arms: REV yok}"
  local hs="${HS_TIMEOUT_S:-$DEADLINE_S}"
  local wrap="bash $RAKIP_DIR/d/rival_wrap.sh @D@"
  local srx="" qid=""
  if [ "${RAKIP_RXLOG:-1}" = 1 ]; then
    srx="--rx-log @D@/rx_log.jsonl"
    qid="--payload-id"
  fi
  # İş yükü + manifest argümanları: arms_kama.sh `kama_arm_cmds` ile aynı değerler ve sıra.
  RAKIP_LOAD_ARGS="--msgs $MSGS --payload $PAY --submit-interval-ms $IVAL_MS \
--max-deadline-s $DEADLINE_S --seed @REP@ --repeat @REP@ --fwd-impair '$FWD' --rev-impair '$REV' \
--manifest @D@/manifest.json --out @D@/out.jsonl"
  local tack="--rival-delivery transport-ack --handshake-timeout-s $hs"
  local B="@BIN_TABAN@"

  # ── BİRİNCİL ───────────────────────────────────────────────────────────────────────────────
  ARM_quinn_SRV="$B serve --bind @SRV@ --transport quinn --cc bbr --rival-delivery transport-ack $srx"
  ARM_quinn_CLI="$wrap $B client --server @SRV@ --transport quinn --cc bbr $tack --rival-stats $qid \
$RAKIP_LOAD_ARGS"
  ARM_tcp_SRV="$B serve --bind @SRV@ --transport tcp --rival-delivery transport-ack $srx"
  ARM_tcp_CLI="$wrap $B client --server @SRV@ --transport tcp $tack $RAKIP_LOAD_ARGS"

  # ── BETİMSEL ───────────────────────────────────────────────────────────────────────────────
  ARM_quinn_rpl_SRV="$B serve --bind @SRV@ --transport quinn --cc bbr $srx"
  ARM_quinn_rpl_CLI="$wrap $B client --server @SRV@ --transport quinn --cc bbr --rival-stats $qid \
$RAKIP_LOAD_ARGS"
  ARM_tcp_rpl_SRV="$B serve --bind @SRV@ --transport tcp $srx"
  ARM_tcp_rpl_CLI="$wrap $B client --server @SRV@ --transport tcp --early-exit $RAKIP_LOAD_ARGS"
  ARM_quinn_cubic_SRV="$B serve --bind @SRV@ --transport quinn --cc cubic --rival-delivery transport-ack $srx"
  ARM_quinn_cubic_CLI="$wrap $B client --server @SRV@ --transport quinn --cc cubic $tack --rival-stats \
$qid $RAKIP_LOAD_ARGS"
  if [ -n "${RTT_HINT_MS:-}" ]; then
    local irtt="--quinn-initial-rtt-ms $RTT_HINT_MS --quinn-iw-bytes 12000"
    ARM_quinn_irtt_SRV="$B serve --bind @SRV@ --transport quinn --cc bbr --rival-delivery transport-ack \
$irtt $srx"
    ARM_quinn_irtt_CLI="$wrap $B client --server @SRV@ --transport quinn --cc bbr $tack $irtt \
--rival-stats $qid $RAKIP_LOAD_ARGS"
  fi
}
