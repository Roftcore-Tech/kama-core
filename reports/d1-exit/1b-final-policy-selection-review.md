# AI — NİHAİ POLICY-SELECTION REVIEW (netem-EXECUTED) [production-default-DEĞİŞMEDİ]

> Zincir: AA(f1683b6)…AG(0c2a214-uncontended-netem) AH(rx-outcome+contended). Netem ROOTLESS-KOŞULDU
> (admin-gerekmedi; kullanıcı-talimatı). PR#44-DRAFT; merge-önerilmedi.

## Kanıt-katmanları-özeti
1. **Deterministik-sim (Z1-Z3):** F15 belirgin-outcome-üstünlüğü (rev99 dl_op +0.100-cluster-CI'lı;
   decode/avoided-retx/spend-matched hepsi-F15-lehine). GÜÇLÜ ama sim-model-bağımlı.
2. **Uncontended-netem (AG, 100-run):** F15-mekanizma REAL-stack+impairment-altında REGRESYONSUZ
   (waste=0, ticket-on-time, displacement-bounded). Mekanizma-güvenli.
3. **Contended-netem + receiver-outcome (AH, 80-run):** **outcome POLİTİKADAN-BAĞIMSIZ ÖZDEŞ**
   (blackout-30s dl_op-0.974-hepsi; lowbw-1.000-hepsi). Deterministik-ayrışma REAL-STACK'TE
   REPRODUCE-OLMADI → büyük-ölçüde-sim-leftover-token-artefaktı.

## §14-uygulaması → **LEFTOVER RETAINED**
F15-outcome-faydası real-stack-netem'de-DOĞRULANMADI; experimental-policy parity+displacement-overhead-
ekliyor-ama-reliability-outcome-ÖZDEŞ → talimat-§14 "Leftover-korunur"-şartı KARŞILANDI. F15 HARM-de-
üretmiyor (blackout-harm-reproduce-olmadı; regresyon-yok) → feature-flag-arkası GELECEK-için-KORUNUR.

## Önerilen-statü
- **Production-default: LeftoverOnly-KORUNUR** (değişiklik-YOK; NO-GO-onaylı).
- **F15/P15: experimental-feature-arkası-KALIR** (regresyonsuz; mekanizma+kernel+lifecycle-tam-test-
  edilmiş; gelecekte-real-stack-parity-starvation-gösteren-rejim/ölçek-bulunursa-yeniden-değerlendirilir).
- Deterministik-bulgu-arşivde (sim-modelinin-fayda-gösterdiği-koşul-kaydı; real-stack-reprodüksiyon-
  ön-koşulu).

## Açık-riskler
Test-edilen-ölçekler real-stack-parity-starvation-üretmedi (F15-fayda-rejimi-bulunamadı — kanıt-yokluğu,
fayda-yokluğu-kanıtı-DEĞİL-tam; ama mevcut-kanıtla-default-değişimi-gerekçesiz). rx-SIGTERM-race
(offered_c-outcome-eksik; küçük-harness-borç). netem=sim-harness (§4.4-nightly-tam-matris-ayrı).

## HÜKÜM-ÖNERİSİ
**DR-1b FINAL POLICY SELECTION PASSED — LEFTOVER RETAINED** (netem-EXECUTED; experimental-policy'ler
real-stack'te-outcome-faydası-göstermedi → default-değişimi-gerekçesiz; F15-feature-arkası-regresyonsuz-
korunur, gelecek-rejim-için).
