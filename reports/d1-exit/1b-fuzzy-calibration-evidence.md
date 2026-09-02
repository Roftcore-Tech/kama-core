# FC–FF — Fuzzy Calibration Evidence [production-fuzzy-config-DEĞİŞMEDİ; frozen-fixture-reference]

> **BAĞLAYICI-ÇERÇEVE (zorunlu-başlık):** Bu deney ARQ ile FEC'i KARŞILAŞTIRMAZ. ARQ bütün-kollarda
> AKTİFTİR (aynı-impl/ACK-NACK/retransmit-zamanlayıcı/retry-semantiği); değişen-tek-değişken ARQ-substrate'ı
> ÜZERİNE eklenen PROAKTİF-FEC-repair-miktarıdır (r0=ARQ+0, r1=ARQ+1, r2=ARQ+2, r4=ARQ+4). Ölçülen =
> **ARQ-substrate üzerine PROAKTİF-FEC'in MARJİNAL-FAYDASI** (marginal-FEC-utility-over-ARQ). FEC hiçbir-
> kolda ARQ'nun-yerine-geçmez; FEC'in-kapatamadığı-residual-deficit her-zaman-ARQ'yla-tamamlanır.


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

## FF-ÇERÇEVE-DÜZELTMESİ (bağlayıcı)
r0=ARQ+0-proaktif-FEC · r1/r2/r4=ARQ+1/2/4-proaktif-FEC. Aşağıdaki-tablo "ARQ-vs-FEC" DEĞİL; **ARQ-
substrate-üzerine-proaktif-FEC'in-marjinal-faydasını** ölçer. "delta=0" = *bu-exact-workload/deadline/
impairment/capacity-koşulunda ARQ-substrate deadline'ı-zaten-karşıladığı-için proaktif-FEC-marjinal-
fayda-üretmedi* — **"FEC-faydasız/ARQ-kazandı" DEĞİL**, koşula-özgü-"NoMarginalFecUtility". Residual-
deficit-varsa-ARQ-tamamlar (hibrit-beklenen-davranış).

## FF — proaktif-FEC-marjinal-utility ORACLE (ARQ-her-kolda-aktif; ground-truth outcome)
| profil (ARQ-hep-aktif) | dl_op ARQ+0 | +1 | +2 | +4 | best | Δ-marjinal | oracle | fuzzy-sınıf |
|---|---|---|---|---|---|---|---|---|
| nominal (5%) | 0.9737 | 0.9737 | 0.9737 | 0.9737 | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |
| loss_med (15%) | 0.9737 | " | " | " | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |
| loss_high (30%) | 0.9737 | " | " | " | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |
| ge_burst (12/55%) | 0.9737 | " | " | " | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |
| blackout30 | 0.9737 | " | " | " | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |
| lowbw30k | 1.0000 | " | " | " | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |
| offered_c | 1.0000 | " | " | " | r0 | 0.0000 | NoMarginalFecUtility | **CorrectZeroProactiveFec** |

**7/7-PROFİLDE proaktif-FEC (r1/r2/r4) ARQ-substrate-üzerine-SIFIR-MARJİNAL-fayda** — ARQ-substrate
(retransmit+SACK) deadline-içinde-her-bloğu-zaten-tamamlıyor, dolayısıyla-proaktif-repair-eklemenin-
marjinal-outcome-katkısı-0. Controller'ın-proaktif-FEC=0-kararı BU-KOŞULLARDA-DOĞRU (CorrectZeroProactive
Fec_thisCondition) — **GLOBAL-"FEC-faydasız"-DEĞİL; test-edilen-koşullar ARQ-limited/FEC-feasible-deadline-
bölgesine-ULAŞMADI (R-C2).** miscalibration-değil.
0.9737-tavan (1.0-değil) horizon-kenarı-blokları (r-bağımsız; FEC-düzeltemez).

## §14-kritik-sorular (cevaplar)
1. proaktif-FEC=0-olaylarının-%'si-BU-KOŞULLARDA-doğru? **%100 (CorrectZeroProactiveFec_thisCondition; oracle-NoMarginalFecUtility-her-profil; GLOBAL-hüküm-DEĞİL).**
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
