# FH — FUZZY CALIBRATION EVIDENCE REVIEW [production-fuzzy-config-DEĞİŞMEDİ]

> Zincir: FA(graceful) → FB(DR-1b-freeze) → FC/FD(fuzzy-dataset+oracle) → FE/FF(analiz) → FG(shadow).
> 224-netem-run/0-fail; 231k-gerçek-fuzzy-event. PR#44-draft.

## Ana-soru cevabı
"Gerçek-Kama-stack'inde-hangi-gözlemlenebilir-durumlarda-FEC-recovery-faydası-üretir-ve-fuzzy-doğru-
ayırıyor-mu?" → **Test-edilen-7-profilde (nominal→30%-loss→GE-burst→blackout→lowbw→offered≈C) FEC
(fixed-r) r0-üzerinde-SIFIR-outcome-faydası-gösterdi (oracle-EquivalentSet-7/7). Fuzzy-controller-
ratio=0 çıkarıyor ve bu OUTCOME-DOĞRU (CorrectZero-7/7). Ayırt-etme-sorunu-YOK: ayrılacak-fayda-yok.**

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
**FUZZY CALIBRATION EVIDENCE PASSED — CURRENT ZERO-BIASED CONTROLLER RETAINED.**
Gerekçe: 7/7-profil-oracle-EquivalentSet → FEC-outcome-faydası-YOK → controller-ratio=0 outcome-doğru;
hiçbir-config-adayı-alignment-iyileştiremez. Production-fuzzy-config-DEĞİŞMEDİ. Residual: loss-signal-
fidelity + FEC-fayda-rejimi-arama = ayrı-gelecek-kapılar (fuzzy-calibration-DEĞİL).
