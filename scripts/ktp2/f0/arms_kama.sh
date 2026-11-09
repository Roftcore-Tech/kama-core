# [KTP2-F0] Kama ürün kolu şablonları: `taban` ve `aday` (sahibi: Ajan B; bütün şeritler source edebilir).
#
# Hücre dosyası geometriyi kurar, bu dosyayı `source` eder ve `kama_arms`'ı çağırır:
#   MSGS=<mesaj> PAY=<yük B> IVAL_MS=<sunum aralığı ms> DEADLINE_S=<son tarih s>
#   FWD="<netem>" REV="<netem>"            (manifest'e AYNEN yazılır: --fwd-impair/--rev-impair)
#   KAMA_SRV_EXTRA / KAMA_CLI_EXTRA        (isteğe bağlı; iki kola AYNI eklenir, varsayılan boş)
#   KAMA_RXLOG (varsayılan 1)              1 = sunucu `--rx-log @D@/rx_log.jsonl` (alıcı teslim damgası,
#                                          şema ktp2-c-rx-v1; rakip kollarla aynı: arms_rakip.sh RAKIP_RXLOG)
#   KAMA_TS (varsayılan 1)                 1 = istemci `--ts-out @D@/ts.csv` (50 ms motor zaman serisi;
#                                          salt gözlem — pacer jetonu, ret sayaçları; Ajan C ile aynı)
# Çıktı: ARM_taban_SRV, ARM_taban_CLI, ARM_aday_SRV, ARM_aday_CLI (f0_cell.sh yer tutucularıyla).
#
# Kol tanımı (KATALOG §1): ikili ürün kolu `--transport kama-product`, ürün v1 profili (bench
# varsayılanları; fec 0x05 adaptive, tail validated_v1). İki kolda komut BAYT BAYT AYNIDIR; fark
# yalnız ikilidir (@BIN_TABAN@ = main a858f3fc + ölçüm kodu, @BIN_ADAY@ = 05b 49864510 + aynı ölçüm
# kodu). Türetildiği örnek: reports/ktp2/f0/05b-netem/arac/netem7_cell.sh:80-95.
#
# Not (yük boyu): ürün v1 profilinde birim tavanı 1013 B'dir (client-core runtime.rs:245) ve
# fragmenter kapalıdır (runtime.rs:266). 1013 B'yi aşan `--payload` bench'te fragmenter'ı açar ve
# koşu `kama-custom` kimliği alır (kama_product.rs:156, 256-263, 661). "1 KB" hücreleri bu yüzden 1000 B
# kullanır.

kama_arm_cmds() { # $1 = ikili yer tutucusu → _KSRV / _KCLI
  local bin="$1"
  : "${MSGS:?MSGS yok}" "${PAY:?PAY yok}" "${IVAL_MS:?IVAL_MS yok}" "${DEADLINE_S:?DEADLINE_S yok}"
  : "${FWD:?FWD yok}" "${REV:?REV yok}"
  local srx="" cts=""
  [ "${KAMA_RXLOG:-1}" = 1 ] && srx="--rx-log @D@/rx_log.jsonl"
  [ "${KAMA_TS:-1}" = 1 ] && cts="--ts-out @D@/ts.csv"
  _KSRV="$bin serve --bind @SRV@ --transport kama-product --payload $PAY $srx ${KAMA_SRV_EXTRA:-}"
  _KCLI="$bin client --server @SRV@ --transport kama-product --msgs $MSGS --payload $PAY \
--submit-interval-ms $IVAL_MS --max-deadline-s $DEADLINE_S --seed @REP@ --repeat @REP@ \
--fwd-impair '$FWD' --rev-impair '$REV' --manifest @D@/manifest.json --out @D@/out.jsonl $cts ${KAMA_CLI_EXTRA:-}"
}

kama_arms() {
  kama_arm_cmds @BIN_TABAN@
  ARM_taban_SRV="$_KSRV"; ARM_taban_CLI="$_KCLI"
  kama_arm_cmds @BIN_ADAY@
  ARM_aday_SRV="$_KSRV"; ARM_aday_CLI="$_KCLI"
  unset _KSRV _KCLI
}

# [YOL3 · Ö] EKLEMELİ (KK-Ö-06): E1 teşhis kolu — tek kol, tek ikili, kol bayrağı komut satırında.
#   kama_e1_arm <kol> <e1-arm> [ikili yer tutucusu = @BIN_TESIS_BENCH@]
#     <kol>    = ARMS'taki ad (f0_cell.sh `ARM_<kol>_SRV/CLI`; yalnız [A-Za-z0-9_])
#     <e1-arm> = bench `--e1-arm` değeri (K0|K0r|K1|K2|K2mc|K2me); İKİ uca AYNI verilir
#   Geometri ve isteğe bağlı ekler `kama_arm_cmds` ile aynı (KAMA_SRV_EXTRA / KAMA_CLI_EXTRA ile
#   ör. FEC kapalı kol: `--fec-profile 0`). Mevcut `kama_arms` (taban, aday) değişmez.
kama_e1_arm() {
  local arm="${1:?kama_e1_arm: kol adı yok}" e1="${2:?kama_e1_arm: --e1-arm değeri yok}"
  local bin="${3:-@BIN_TESIS_BENCH@}"
  case "$arm" in *[!A-Za-z0-9_]*) echo "kama_e1_arm: geçersiz kol adı: $arm" >&2; return 2;; esac
  kama_arm_cmds "$bin"
  printf -v "ARM_${arm}_SRV" '%s' "$_KSRV --e1-arm $e1"
  printf -v "ARM_${arm}_CLI" '%s' "$_KCLI --e1-arm $e1"
  unset _KSRV _KCLI
}
