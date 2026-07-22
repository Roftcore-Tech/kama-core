# AH/AI — CONTENDED-Netem + Receiver-Outcome-Ledger: NİHAİ-BULGU [production-default-DEĞİŞMEDİ]

> Rootless-userns (admin'siz); runner-rx RECEIVER-OUTCOME-ledger (decode/dl_op/completion; R2-kapatma) +
> contended-scale (büyük-w_units + saturating-profil; R1-kapatma). 80-run/0-fail; 56-rx-outcome-ledger
> (rx-SIGTERM-race → idle-exit-eden-runlar; blackout/lowbw-hepsi-yakalandı, offered_c-race'te-kayıp).
> Veri: [data/netem-contended/](data/netem-contended/).

## NİHAİ-BULGU (dürüst; deterministik-hükmü-DEĞİŞTİRİR)
Real-stack (GERÇEK-Sender-pacer + GERÇEK-UDP + GERÇEK-netem) altında **outcome-metrikleri POLİTİKADAN-
BAĞIMSIZ ÖZDEŞ**:
| profil | metrik | Leftover | P15 | F15 | F10 |
|---|---|---|---|---|---|
| blackout-30s | dl_op / complete | 0.974 | 0.974 | 0.974 | 0.974 |
| lowbw-30k | dl_op / complete | 1.000 | 1.000 | 1.000 | 1.000 |
| offered≈C | emit (parity) | 197 | 197 | 197 | — |

F15-mekanizması-ÇALIŞIYOR (blackout: ticket 37/37-on-time, displacement 1800B; offered_c: 50/50-ticket)
AMA **net-reliability-farkı-ÜRETMİYOR — ne-deterministik-fayda-ne-blackout-harm reproduce-oldu.**

## NEDEN (kök-neden)
Deterministik-sweep'in-dramatik-F15-ayrışması BÜYÜK-ÖLÇÜDE leftover-token-SİM-MODELİNİN-artefaktıydı.
Real-stack'te Sender'ın-KENDİ-pacing+ARQ'su parity-starvation'ı-bu-ölçeklerde-ÜRETMİYOR (rate-limit
DATA'yı-orantılı-throttle-ediyor → leftover-parity'ye-yetiyor); budget-policy outcome'u-değiştirmiyor.
Yani deterministik-outcome-yönü real-stack'te BAĞIMSIZ-DOĞRULANMADI; tersine ÖZDEŞLİK-bulundu.

## §14-KARAR-KURALI (yeniden-uygulama)
- F15-seçilebilir-şartı ("netem'de-≥2-ailede-aynı-recovery-yönü"): **KARŞILANMADI** (yön-yok, özdeş).
- **Leftover-retained-şartı** ("netem'de-outcome-faydası-doğrulanmaz; experimental-yalnız-parity-
  overhead-üretir"): **KARŞILANDI** — F15/P15 parity+displacement-ekliyor, outcome-ÖZDEŞ.
Netem-ters-yön-YOK (harm-de-yok); DUR-koşulu-yok. Deterministik-mekanizma-güvenli (regresyon-yok).

## Kalan-sınırlar (dürüst)
Test-edilen-ölçekler real-stack'te-parity-starvation-üretmedi → F15'in-fayda-gösterebileceği-rejim
(çok-yüksek-app-limited-saturation, farklı-pacing-regime) bu-turda-bulunamadı. F15 feature-flag-arkası
GELECEK-bağlamlar-için-KORUNUR (regresyonsuz; mekanizma-hazır). rx-SIGTERM-race küçük-harness-borç
(offered_c-outcome-eksik; idle-exit-runlarında-sorunsuz).
