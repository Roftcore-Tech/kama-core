# [KTP2-F0] Kama ürün kolu şablonları: `taban` ve `aday` (sahibi: Ajan B; bütün şeritler source edebilir).
#
# Hücre dosyası geometriyi kurar, bu dosyayı `source` eder ve `kama_arms`'ı çağırır:
#   MSGS=<mesaj> PAY=<yük B> IVAL_MS=<sunum aralığı ms> DEADLINE_S=<son tarih s>
#   FWD="<netem>" REV="<netem>"            (manifest'e AYNEN yazılır: --fwd-impair/--rev-impair)
#   KAMA_SRV_EXTRA / KAMA_CLI_EXTRA        (isteğe bağlı; iki kola AYNI eklenir, varsayılan boş)
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
  _KSRV="$bin serve --bind @SRV@ --transport kama-product --payload $PAY ${KAMA_SRV_EXTRA:-}"
  _KCLI="$bin client --server @SRV@ --transport kama-product --msgs $MSGS --payload $PAY \
--submit-interval-ms $IVAL_MS --max-deadline-s $DEADLINE_S --seed @REP@ --repeat @REP@ \
--fwd-impair '$FWD' --rev-impair '$REV' --manifest @D@/manifest.json --out @D@/out.jsonl ${KAMA_CLI_EXTRA:-}"
}

kama_arms() {
  kama_arm_cmds @BIN_TABAN@
  ARM_taban_SRV="$_KSRV"; ARM_taban_CLI="$_KCLI"
  kama_arm_cmds @BIN_ADAY@
  ARM_aday_SRV="$_KSRV"; ARM_aday_CLI="$_KCLI"
  unset _KSRV _KCLI
}
