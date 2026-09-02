# Fuzzy Rule-Coverage Prepass (Commit-P) — COVERAGE-AUDIT [kalibrasyon-DEĞİL; eşik/kural-DEĞİŞMEDİ]

> İki-uzay-AYRI-tutulur (talimat-§11): **11.1 matematiksel-input-space-map** (doğrudan-FusionInputs-
> uzayı; network-realisability-İDDİASI-YOK) ve **11.2 event-driven-realizability-map** (gerçek-harness-
> event-akışıyla-ulaşılan-bölgeler). Araç: `adaptive-transport/tests/rule_coverage_map.rs` (analitik-
> harita public-fuzzify+kaynak-formül-kopyası; sürüklenme-riski 3-GERÇEK-YOL-ÇAPA-testiyle-sınırlı —
> `anchor_validation_against_real_path` CI'da; decide-pub-görünürlüğü ayrı-küçük-DR-adayı).
> Veri: [data/rule-coverage/](data/rule-coverage/). Yeniden-üretim: `RCM_OUT=<dir> cargo test -p
> adaptive-transport --test rule_coverage_map rule_coverage_map -- --ignored` (30ms; deterministik).

## 11.1 Matematiksel-map (77 112-nokta-grid: loss-51 × bw-21 × jitter-9 × ackconf-8)

| metrik | oran |
|---|---|
| R1-aktif (loss.LOW→fec0) | %29.4 |
| R2-aktif (loss.H∧bw.H∧jit.L→fec800) | %6.2 |
| R3-aktif (loss.M∧bw.M→fec500) | %12.4 |
| R4-aktif (loss.L∧jit.H→fec0) | %6.5 |
| ≥2-kural-örtüşmesi | %8.7 |
| **no_rule (4-kural-sıfır)** | **%54.7** |
| emergency-mode | %8.1 |
| **no_rule ∧ emergency** | **%8.0 (emergency'nin %99.13'ü)** |
| emergency ∧ nonzero-ratio | **%0.07 (ölçü-sıfır-şerit)** |
| ratio=0 | %81.5 |
| ratio=500-civarı | %9.6 |
| ratio=800 | %6.1 |

**Ana-audit-bulgusu-1:** örneklenen-uzayın YARIDAN-FAZLASI (%54.7) no_rule-deliği — kural-tabanı
uzayın-çoğunu-örtmüyor (fallback fec_zero+arq_light'a-düşüyor). En-büyük-delik: loss≥600k-plateau ∧
bw∉HIGH (R2-dışı) — fuzzy_e2e-4.3'ün-genelleşmiş-hali.

**Ana-audit-bulgusu-2 (EMERGENCY-ÖRTÜŞME-NİCEL):** fixture-geometrisinde emergency-bölgesi (loss.H∧
bw.LOW∧ackc.LOW) non-zero-FEC-kural-bölgelerinden-pratik-olarak-AYRIK: emergency-noktalarının
%99.13'ü aynı-anda-no_rule; emergency∧nonzero-ratio yalnız %0.07 (dar loss∈(550k,600k)×bw∈(50k,75k)-
şeridi — E2E'de-sürülemeyen-hassas-köşe). Sonuç (Commit-J/K-bulgusunun-global-teyidi): gerçek-
emergency-yolunda requested-ratio-tipik-SIFIR; `EmergencyPendingArbitration`-non-zero-dalı-pratikte-
yalnız-enjeksiyonlu-testte; **emergency-FEC-politikası-bugün-ampirik-anlamsız** — 1b/1d+kalibrasyon-
öncesi-ürün-davranışı-olarak-yorumlanmaz.

**Dead-membership-yüzeyleri (statik-kullanım-analizi; decide-formüllerinden):**
| değişken-bandı | FEC-kuralları | emergency | send-rate | durum |
|---|---|---|---|---|
| qd.low / qd.med | — | — | — | **ÖLÜ (hiçbir-çıktıda)** |
| qd.high | — | — | SR1 | yalnız-send-rate |
| ackc.med / ackc.high | — | — | — | **ÖLÜ** |
| ackc.low | — | ✓ | — | yalnız-emergency |
| jitter.med | — | — | — | **ÖLÜ** |
| bw.low | — | ✓ | SR1 | FEC-kurallarında-YOK |

## 11.2 Event-driven-realizability-map (MS1 + fuzzy_e2e ölçümleri; İKİ-UZAY-KARIŞTIRILMAZ)

| kaynak | realized-inputs (p50) | aktif-kural (gerçek-yol) | ratio |
|---|---|---|---|
| MS1-nominal (raw-%5) | loss 10-55k · bw ≈60k · ackconf 1000 | R1 | 0 |
| MS1-loss20 (raw-%20) | **loss ~102-107k** · bw ~50k | R1 | 0 |
| MS1-d1000/d2500 | loss düşük · bw 4-12k (kilit) | R1 | 0 |
| MS1-rev99 | loss düşük-orta · bw 7-10k · ackconf-değişken | R1 | 0 |
| MS1 (10-hücre×tümü) | — | **hep-R1** | **hep-0** |
| fuzzy_e2e-4.1 (uzun-pencere-pompa + clean-burst) | loss 592k · bw 800k · jit 1.3k | R2 | 800 |
| fuzzy_e2e-4.3 | loss 659k · bw 80k | no_rule | LITERAL-0 |
| fuzzy_e2e-4.4 (+11s-sessizlik) | loss 787k · bw 26k · ackconf 393 | no_rule∧emergency | 0-interlock |

**Realizability-ana-bulgusu:** ARQ-aktif-normal-dinamikte realized `loss_windowed_ppm` raw-%20-kayıpta-
bile ~105k'da-doyuyor (retx-başarıları + pencere-decay) → **fixture-loss-eşikleri (LOW≤200k / HIGH≥600k)
MS1-zarfının-tamamen-ÜSTÜNDE**. R2/R3/no-rule-bölgeleri yalnız fuzzy_e2e-tipi-aşırı-rejimlerle
(süreğen-yüksek-kayıp ≥ W_loss≈16.8s + btlbw-bakım-burst'leri) ulaşılabilir. no-rule-oranı/emergency-
oranı normal-hücrelerde ölçülen-yolda-0 (hiç-tetiklenmedi). Bu KALİBRASYON-GİRDİSİDİR (kesit-2-üyelik-
sınırı-tasarımına); bu-turda-eşik-DEĞİŞTİRİLMEDİ.

## DİL-DÜZELTMESİ (Commit-Q) + evaluator-drift-seçenekleri

**no_rule=%54.7 YALNIZ şudur:** örneklenen-UNIFORM-MATEMATİKSEL-grid'in %54.7'si. ŞUNLAR-DEĞİLDİR:
gerçek-ağ-zamanının-%54.7'si · production-state-olasılığı · trafik-ağırlıklı-olasılık. (Event-driven-
gerçeklik 11.2'de: ölçülen-yolda no-rule/emergency normal-hücrelerde-hiç-tetiklenmedi.) Aynı-uyarı
tüm-grid-oranları-için-geçerlidir (R1-%29.4 vb.).

**Kaynak-formül-kopyası drift-riski (1b-blocker-DEĞİL; kalibrasyon-fazı-öncesi-önerilir):** seçenekler —
(a) canonical-test-only-evaluator (decide()'ın pub(crate)+cfg(test)-görünür-sarmalayıcısı; küçük-DR),
(b) shared-rule-definition (kural-tablosu-tek-kaynak-const'a-çıkarılır), (c) generated-rule-map
(build-time-üretim), (d) daha-geniş-çapa-seti (mevcut-3-gerçek-yol-çapasına grid-köşe-noktaları-eklenir).
ÖNERİ: (a)+(d) — en-küçük-yüzey.
