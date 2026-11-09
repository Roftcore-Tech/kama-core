# [KTP2-F0 / Ajan B] B hücrelerinin ortak gövdesi (cells/b/*.cell bunu source eder).
#
# Hücre dosyası önce geometriyi yazar (MSGS PAY IVAL_MS DEADLINE_S FWD REV), sonra:
#   source "<f0>/b/hucre_b.sh"; b_hucre_kur
# b_hucre_kur:
#   1. Kama kolları: ../arms_kama.sh → kama_arms (taban, aday).
#   2. Rakip kolları: ../arms_rakip.sh (Ajan D) varsa ve `rakip_arms` işlevini sağlıyorsa o; yoksa
#      GEÇİCİ tanım (aşağıda, `env F0_RAKIP_TANIM=gecici-B` ile cmd.txt'de işaretli).
#   3. Her kolun SRV/CLI komutu cap_wrap.sh ile sarılır (uç yakalama; komut anlamı değişmez).
#      F0_BCAP=0 ortamıyla kapatılabilir.

B_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
F0_DIR="$(cd "$B_DIR/.." && pwd)"

b_rakip_gecici() {
  # [GEÇİCİ — Ajan D'nin arms_rakip.sh'si gelene kadar] KATALOG §1: quinn = `--transport quinn
  # --cc bbr` (mesaj başına bağımsız bi-stream, quic.rs:536), tcp = `--transport tcp` (tek bağlantı).
  local mark="env F0_RAKIP_TANIM=gecici-B"
  local common="--msgs $MSGS --payload $PAY --submit-interval-ms $IVAL_MS --max-deadline-s $DEADLINE_S \
--seed @REP@ --repeat @REP@ --fwd-impair '$FWD' --rev-impair '$REV' --manifest @D@/manifest.json --out @D@/out.jsonl"
  ARM_quinn_SRV="$mark @BIN_TABAN@ serve --bind @SRV@ --transport quinn --cc bbr"
  ARM_quinn_CLI="$mark @BIN_TABAN@ client --server @SRV@ --transport quinn --cc bbr $common"
  ARM_tcp_SRV="$mark @BIN_TABAN@ serve --bind @SRV@ --transport tcp"
  ARM_tcp_CLI="$mark @BIN_TABAN@ client --server @SRV@ --transport tcp $common"
}

b_rakip_arms() {
  if [ -f "$F0_DIR/arms_rakip.sh" ]; then
    # shellcheck disable=SC1091
    source "$F0_DIR/arms_rakip.sh"
  fi
  if declare -F rakip_arms >/dev/null 2>&1; then
    rakip_arms
    B_RAKIP_KAYNAK="arms_rakip.sh"
  else
    b_rakip_gecici
    B_RAKIP_KAYNAK="gecici-B"
  fi
}

b_cap_wrap() {
  [ "${F0_BCAP:-1}" = 1 ] || return 0
  local a v
  for a in $ARMS; do
    v="ARM_${a}_SRV"; printf -v "$v" '%s' "bash $B_DIR/cap_wrap.sh srv @D@ -- ${!v}"
    v="ARM_${a}_CLI"; printf -v "$v" '%s' "bash $B_DIR/cap_wrap.sh cli @D@ -- ${!v}"
  done
}

b_hucre_kur() {
  # shellcheck disable=SC1091
  source "$F0_DIR/arms_kama.sh"
  kama_arms
  b_rakip_arms
  b_cap_wrap
}
