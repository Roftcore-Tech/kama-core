# KESİT-2 DENEY-TASARIMI v1 [TASLAK — KOŞULMAZ; Abkarada-§15]

## ÖN-KOŞUL-CHECKLİSTİ (tümü-yeşil-olmadan-KOŞULMAZ)

| # | Ön-koşul | Durum |
|---|---|---|
| 1 | Kesit-1 validity-audit tamam | ✅ [validity-audit](tarama-kesit-1-validity-audit.md) |
| 2 | Gap80-r2-anomalisi açıklandı | ✅ kök-neden: BtlBw-offered-kilitlenmesi×token-artığı (bimodal) |
| 3 | requested/planned/emitted-parity harness'ta-görünür | ✅ (parity_enq/emit + blocks; DR-1a-telemetry-impl'le-tam) |
| 4 | Actual-FusionInputs-telemetri | ◐ KISMİ (btlbw/qd/no_rule); loss_ppm/jitter/ackconf-diag 1e-turu-additive → **kesit-2-blocker: bu-üçü-eklenmeden ackconf/loss-üyelik-eksenleri koşulmaz** |
| 5 | Censoring-işaretleniyor | ✅ (Metrics.censored) |
| 6 | Seed-level-sonuçlar-korunuyor | ✅ (audit_seed_level-CSV) |
| 7 | Objective/Pareto-raporlama-tanımlı | ✅ (audit-§6; objective-sırası-politika-kararı-AÇIK) |
| 8 | Arm-separation-validity-gate | ✅ tanım (VALID-SEPARATED/PART/FULL-COLLAPSED/CENSORED/ANOMALOUS); kesit-2'de-hücre-başına-otomatik |

## İSTATİSTİK-ŞARTLARI (audit-§7)
Paired-seed tüm-armlarda · ≥10-paired-seed (küçük-fark→30'a-genişletme-planı) · medyan+bootstrap-CI ·
practical-equivalence-band DENEY-ÖNCESİ-yöntemle (1/W-çözünürlük + seed-std'den; keyfî-değil) · 0.005-türü-fark
tek-başına-winner-ilan-etmez.

## ÖNCELİKLER

**A. Focused-interaction-search:** loss×RTT ve rev×RTT — FULL-FACTORIAL-DEĞİL: kesit-1 VALID-SEPARATED-
bölgelerinden odak-grid (örn. loss{5,15}×d{50,500}; collapsed-hücreler-crossover-aramasında-KULLANILMAZ).
Collapse-mekanizması (token-artığı) sabit-kaldıkça yüksek-RTT-hücreleri önce-ayrıştırma-gerektirir
(1b-öncesi: emitted-doğrulamalı-hücre-seçimi).

**B. Reverse-blackout (epizot):** sürekli-rev-yerine {kısa 2s, orta 10s, uzun 60s, recovery, tekrarlı}-karanlık;
actual-ackconf + silence-duration-kaydı (ön-koşul-4-diag'ları-şart).

**C. Matched-loss-burst:** aynı-REALIZED-mean-loss (kalibre-edilmiş-GE-parametreleri; kesit-1-profilleri-
eşleşmiyordu) × farklı-burst-length; sonra-K-varyasyonu.

**D. Headroom/overload:** utilization {0.5, 0.8, 0.95, 1.0, 1.1, 1.25}; finite-queue + drop-policy +
capacity-step-down tanımlı; horizon/censoring-raporu-zorunlu. (Production/white-paper-iddiası-değil.)

**E. E-size (shard-düzeyi-sim):** yalnız profile-negotiation/block-boundary/serialization/deadline-accounting
hazırsa; controller'a-BAĞLANMAZ (tasarım-verisi).

**F. Adaptif-arm'lar:** controller-ratio-arm YALNIZ-1a-impl-sonrası; synchronized-FEC+ARQ-arm YALNIZ-1b/1d-sonrası.
