# FH — FUZZY CALIBRATION EVIDENCE REVIEW [production-fuzzy-config-DEĞİŞMEDİ]

> **BAĞLAYICI-ÇERÇEVE (zorunlu-başlık):** Bu deney ARQ ile FEC'i KARŞILAŞTIRMAZ. ARQ bütün-kollarda
> AKTİFTİR (aynı-impl/ACK-NACK/retransmit-zamanlayıcı/retry-semantiği); değişen-tek-değişken ARQ-substrate'ı
> ÜZERİNE eklenen PROAKTİF-FEC-repair-miktarıdır (r0=ARQ+0, r1=ARQ+1, r2=ARQ+2, r4=ARQ+4). Ölçülen =
> **ARQ-substrate üzerine PROAKTİF-FEC'in MARJİNAL-FAYDASI** (marginal-FEC-utility-over-ARQ). FEC hiçbir-
> kolda ARQ'nun-yerine-geçmez; FEC'in-kapatamadığı-residual-deficit her-zaman-ARQ'yla-tamamlanır.


> Zincir: FA(graceful) → FB(DR-1b-freeze) → FC/FD(fuzzy-dataset+oracle) → FE/FF(analiz) → FG(shadow).
> 224-netem-run/0-fail; 231k-gerçek-fuzzy-event. PR#44-draft.

## Ana-soru cevabı
"ARQ-substrate-her-koşulda-aktifken hangi-durumda PROAKTİF-FEC-eklemek marjinal-outcome-faydası-üretir-ve-
fuzzy-doğru-seçiyor-mu?" → **Test-edilen-7-profilde proaktif-FEC (ARQ+1/2/4) ARQ-substrate-üzerine SIFIR-
MARJİNAL-fayda-gösterdi (NoMarginalFecUtility_thisCondition 7/7) çünkü ARQ-substrate deadline'ı-zaten-
karşıladı. Controller proaktif-FEC=0 çıkarıyor ve bu BU-KOŞULLARDA-DOĞRU (CorrectZeroProactiveFec 7/7).
Bu GLOBAL-"FEC-hiçbir-rejimde-faydalı-değil" HÜKMÜ-DEĞİLDİR — ARQ-limited/FEC-feasible-rejim (R-C2)
üretilmedi.**

## Kanıt-hiyerarşisi (§20; higher-layer-bağlayıcı)
1. Real-UDP/netem-outcome: fixed-r-oracle EquivalentSet-7/7 (r0≡r4). ← BAĞLAYICI
2. Real-Sender-fuzzy-trace: loss_windowed_ppm=0 → R1 → ratio=0 (231k-event).
3. Deterministic-model: (önceki-tur) FEC-fayda-gösterdi = MODEL-DIAGNOSTIC (higher-layer-ile-çelişti →
   higher-layer-kazanır; discrepancy-belgeli: sim-leftover-token-artefaktı).

## §14-kritik-cevaplar (özet; tam: fuzzy-calibration-evidence.md)
ratio=0-doğruluk=%100 · non-zero-FEC-faydalı-bölge-YOK · no-rule-useful-FEC-YOK (no_rule-frac=0) ·
emergency-gerçek-yolda-tetiklenmedi · loss.MED/HIGH-dead-in-traffic (ama-ulaşılmıyor-ve-o-seviyede-
faydasız → eşik-yanlış-değil) · Leftover-altında-fixed-r-emit-ediyor-ama-outcome-r0-özdeş.

## Candidate (§15/17)
**NO-CANDIDATE-WARRANTED** (FG-airtight): (a) hiçbir-threshold-adayı ratio>0-üretemez (loss-signal=0 →
loss.low=1-daima); (b) üretebilse-bile-oracle-FEC-faydasız → OverProtection. Baseline-zaten-%100-
CorrectZero → iyileştirilecek-alignment-yok. Live-prototype-aday-YOK.

## Açık-riskler / residual (dürüst)
(R-C1) **loss_windowed_ppm=0-under-30%-netem:** genuine-recovery-VEYA-harness-loss-signal-limitasyonu.
Fixed-r-oracle-BUNA-BAĞIMSIZ (FEC-faydasızlığını-doğrudan-ölçer) → verdict-değişmez; AMA doğru-müdahale-
noktası-VARSA = **loss-signal-fidelity (AYRI-DR; fuzzy-calibration-değil).** (R-C2) ARQ-RTT>>deadline ∧
forward-recovery-kazanır rejimi (FEC'in-teorik-fayda-bölgesi) test-edilen-profillerde-ÜRETİLMEDİ —
bulunamadı (kanıt-yokluğu; profil-tasarımı-ileride-genişletilebilir). (R-C3) oracle=directional
(run/profil-expected-utility; packet-fate-r-arm'ları-birebir-paired-değil).

## HÜKÜM-ÖNERİSİ
**FUZZY CALIBRATION EVIDENCE PASSED — CURRENT ZERO-BIASED CONTROLLER RETAINED (condition-scoped).**
Gerekçe: 7/7-profil-oracle-NoMarginalFecUtility_thisCondition → ARQ-substrate-üzerine-proaktif-FEC-marjinal-
faydası-BU-KOŞULLARDA-YOK → controller-proaktif-FEC=0 bu-koşullarda-doğru; hiçbir-config-adayı-alignment-
iyileştiremez (loss-signal=0 + koşulda-marjinal-fayda-yok). **HÜKÜM-GLOBAL-DEĞİL:** ARQ-limited/FEC-feasible-
rejim bulunursa-controller-orada-proaktif-FEC>0-üretmeli — o-rejim-bu-turda-üretilmedi (R-C2, açık-residual).
Production-fuzzy-config-DEĞİŞMEDİ. Residual: loss-signal-fidelity + ARQ-limited/FEC-feasible-rejim-arama =
ayrı-gelecek-kapılar.
