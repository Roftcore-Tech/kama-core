# FC–FF — Fuzzy Calibration Evidence [production-fuzzy-config-DEĞİŞMEDİ; frozen-fixture-reference]

> Kanıt: GERÇEK-Sender + GERÇEK-UDP + GERÇEK-netem (rootless); 7-profil (nominal/loss_med/loss_high/ge_burst/blackout30/lowbw30k/offered_c) × 4-fixed-r-oracle-arm
> (r0/r1/r2/r4; LeftoverOnly) × 8-rep; **222 502 gerçek-fuzzy-event** (--emit-fuzzy; additive-diag-poll;
> decision-DEĞİŞMEDİ). Fuzzy-config=FROZEN-fixture (analytic-membership; anchor-validated).
> Veri: [data/fuzzy-evidence/](data/fuzzy-evidence/). Inference-birimi=run/profil; event-bağımsız-örnek-DEĞİL.

## FE — Gerçek-stack FusionInputs dağılımı (222k-event)
| input | min | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| **loss_windowed_ppm** | 0 | **0** | **0** | **0** | **0** |
| qd_ratio_permille | 0 | 0 | 0 | 0 | 0 |
| ack_confidence | 1000 | 1000 | 1000 | 1000 | 1000 |
| bandwidth_bps | 0 | 237k | 300k | 306k | 335k |
| jitter_ppm | 0 | 386 | 75.7k | 122k | 1.73M |

**KRİTİK-BULGU:** `loss_windowed_ppm` **222k-event'in-TAMAMINDA-0** — %30-netem-loss + GE-burst-altında-bile.
Sender'ın-loss-ledger'ı (ARQ-recovery + windowed-confirmed-loss + confirm_age-decay) kaybı-recover-
edilmeden-önce-saymıyor → controller-DÜŞÜK-LOSS-kanal-görüyor.

## FE — Membership-support (empirical: degree>0-event-oranı)
| değişken | LOW | MED | HIGH |
|---|---|---|---|
| **loss** | **1.0000** | **0.0000** | **0.0000** |
| bw | 0.022 | 0.158 | 0.977 |
loss.MED/HIGH memberships **gerçek-trafikte-HİÇ-realize-olmuyor** (dead-in-traffic). loss.LOW-her-event.

## FE — Traffic-weighted-coverage
`no_rule_frac=0.0000` (loss.low=1 → R1-her-event-ateşler) · `ratio_zero_frac=1.0000` (her-karar-ratio=0) ·
`mode=Normal-%100` (emergency-YOK). (Uniform-grid-coverage §rule-coverage-prepass'ta-AYRI; empirical≠grid.)

## FF — Fixed-r ORACLE vs current-fuzzy (ground-truth outcome)
| profil | dl_op r0 | r1 | r2 | r4 | best | Δ vs r0 | oracle | fuzzy-sınıf |
|---|---|---|---|---|---|---|---|---|
| nominal (5%) | 0.9737 | 0.9737 | 0.9737 | 0.9737 | r0 | 0.0000 | EquivalentSet | **CorrectZero** |
| loss_med (15%) | 0.9737 | " | " | " | r0 | 0.0000 | EquivalentSet | **CorrectZero** |
| loss_high (30%) | 0.9737 | " | " | " | r0 | 0.0000 | EquivalentSet | **CorrectZero** |
| ge_burst (12/55%) | 0.9737 | " | " | " | r0 | 0.0000 | EquivalentSet | **CorrectZero** |
| blackout30 | 0.9737 | " | " | " | r0 | 0.0000 | EquivalentSet | **CorrectZero** |
| lowbw30k | 1.0000 | " | " | " | r0 | 0.0000 | EquivalentSet | **CorrectZero** |
| offered_c | 1.0000 | " | " | " | r0 | 0.0000 | EquivalentSet | **CorrectZero** |

**7/7-PROFİLDE FEC (r1/r2/r4) r0-üzerinde-SIFIR-outcome-faydası** — ARQ/SACK-loop deadline-içinde-
her-bloğu-recover-ediyor. Controller'ın-ratio=0'ı OUTCOME-DOĞRU (CorrectZero), miscalibration-DEĞİL.
0.9737-tavan (1.0-değil) horizon-kenarı-blokları (r-bağımsız; FEC-düzeltemez).

## §14-kritik-sorular (cevaplar)
1. ratio=0-olaylarının-%'si-doğru? **%100 (CorrectZero; oracle-EquivalentSet-her-profil).**
2/3. no-rule/non-zero-FEC-faydalı-bölge? **BULUNAMADI (oracle-hiçbir-profilde-r>0>r0).**
4. Emergency-overlap-zararlı-mı? Gerçek-yolda-emergency-HİÇ-tetiklenmedi (mode=Normal-%100).
5. Loss-eşikleri-observed-support'la-uyumsuz-mu? loss.MED/HIGH-dead (support-0) AMA-observed-loss-0-
   olduğundan-ve-o-seviyede-FEC-faydasız-olduğundan eşikler-YANLIŞ-DEĞİL, sadece-ulaşılmıyor (doğru-olarak).
6. qd/jitter/ackconf-dead-bölgeler? qd=0-hep; ackconf=1000-hep → qd/ackconf-FEC-kurallarında-etkisiz.
9/10. Leftover-altında-parity-emit-ediliyor-mu / starvation? Fixed-r-arm'lar-emit-ediyor (ledger),
   starvation-yok, ama-outcome-r0'la-özdeş.

## DÜRÜST-CAVEAT'ler (residual)
(C1) loss_windowed_ppm=0-under-30%-netem: GENUINE-recovery-property-VEYA-harness-loss-detection-sınırı —
AMA fixed-r-oracle loss-signal'den-BAĞIMSIZ ve FEC-faydasızlığını-doğrudan-ölçüyor → ratio=0-outcome-
doğru-her-halükarda. (C2) Test-edilen-profiller ARQ-RTT>>deadline-∧-forward-recovery-kazanır rejimini-
ÜRETMEDİ (FEC'in-teorik-fayda-bölgesi); varsa-bulunamadı — named-residual. (C3) Oracle=run/profil-
expected-utility (packet-fate-r-arm'ları-arası-birebir-paired-değil) → directional-oracle.
