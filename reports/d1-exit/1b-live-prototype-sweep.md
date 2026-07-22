# 1b Canlı-Prototip Micro-Sweep (Commit-X) [feature-gated; policy-default-SEÇİLMEDİ]

> Harness: `adaptive-transport/tests/live_budget_sweep.rs` (GERÇEK-Sender + GERÇEK-feedback-loop;
> MS1-modeli). PRE-DECLARED-daraltma-başlıkta (10-istatistiksel-hücre + fonksiyonel-senaryolar-W/kernel-
> sim'de; Controller-arm-YOK — kalibrasyonsuz=policy-evidence-değil). 10-paired-seed (gap80:30).
> Veri: [data/live-budget-sweep/](data/live-budget-sweep/) (seed-level 1040-satır + summary).
> Yeniden-üretim: `LBS_OUT=<dir> cargo test --release -p adaptive-transport --features
> experimental-fec-budget --test live_budget_sweep live_budget_sweep -- --ignored` (6s; deterministik;
> smoke=çift-koşu-bit-özdeşlik-CI'da) + `scripts/lbs_analyze.py`.

## ANA-SONUÇLAR (median; tam-tablo-CSV'de)

**1. Contended-realization (hedef-rejim; kabul-kriteri-10):** Leftover→P15/F15:
d1000: 0.27/0.14 → **0.52–1.00** (r2-P15=1.00) · d2500: 0.66/0.33 → 0.67–1.00 · rev99: 0.44/0.22 →
0.65–0.99 · offered-c1: 0.02/0.01 → 0.42–0.85 · manyblock: 0.02/0.01 → 0.48–0.95. **Anlamlı-yüksek ✓.**

**2. gap80-BİMODAL-KÜME-ELİMİNASYONU:** Leftover-r2 emit-dağılımı {1-2}×7-seed vs {92-99}×23 (MS1-
yapısı-aynen; GİZLENMEDİ) → **F15-r2: düşük-küme-YOK (min-77; {77-100}-tek-küme)** — entitlement,
BtlBw-erken-kilit-kurbanı-seed'leri-kurtarıyor. (BtlBw-kilidi-DEĞİŞMEDİ — başarı-yanlış-atfedilmiyor:
bw12'de FEC≡Leftover-realization; kilit-etkisi-ayrı-dosyalı.)

**3. Waste=0 TÜM-1040-koşuda** (kabul-kriteri-1 ✓; atomik-rezervasyon-canlı-kanıtı).

**4. Floor-ticket'lar:** on-time≈50/50-tipik; miss≈0 (yalnız gap80-F10: 2); first-parity-delay-p50
çarpıcı-iyileşme (Leftover-d1000: blokların-çoğu-HİÇ-parity-almıyor (emit-27/200) → F15: her-blok-ilk-
parity-ticket'lı). T_floor-sensitivity (m1/m2/m4): taranan-hücrelerde-FARK-YOK (deadline-hiç-binding-
değil — servis-anında; binding-yalnız-starved-rejimde (W-testi-PreConverge-70s)). [CAL]-duyarlılığı-düşük.

**5. Displacement (U9-ayrık; bounded):** new-DATA + retx AYRI-ölçüldü (ör. d1000-F15: 111k+22k ≈
%18-of-throughput ≈ entitlement-payı+credit — envelope-uyumlu). ARQ-displacement-GERÇEK (loss20-r4-P15:
retx-disp-113k) ama completion=1.0-korundu; dl5-etkisi ≤0.008; t95-etkisi ≤+16% (d1000/d2500 —
bounded-raporlu; unbounded-büyüme-YOK).

**6. Reliability:** completion=1.0-TÜM-hücrelerde (bu-zarf-tamamlama-doygun — MS1-bulgusuyla-tutarlı;
reliability-ayrışması-bu-horizonlarda-görünmez). Kriter-11 "VEYA-floor-miss-azalır"-koluyla-karşılanır
(per-blok-ilk-parity-kapsaması Leftover'da-yapısal-yok → F15'te-tam). Goodput-maliyeti = bilinen-
koruma-overhead'i (tüm-fixed-r-arm'larında-ortak); Leftover-vs-F15-delta-orta (d1000-r2: 0.867→0.759).

**7. Sensitivity (F10/F15/F20):** monoton-tradeoff: F10-düşük-displacement/düşük-realization ↔
F20-yüksek/yüksek (gap80: 0.70/0.84/0.87; offered-c1: 0.27/0.42/0.60) — [CAL]-seçimi-kalibrasyon-işi.

## Kabul/RED-kriterleri-değerlendirmesi (talimat-§22; bu-tur-default-SEÇMEZ)
1-waste=0 ✓ · 2-envelope-property (kernel+canlı-test) ✓ · 3-hardcap-ihlali-0 (yapısal-enforcement+assert) ✓ ·
4-cap-ihlali-0 (rejections-sayılı) ✓ · 5-çift-sayım-0 (kimlik+lifecycle) ✓ · 6-Leftover-non-regression
(pin+iki-mod-suite) ✓ · 7-DATA-wire-order ✓ (sprint5c-pin) · 8-ARQ-ayrı-ölçüm ✓ · 9-N8 ✓ (suite-iki-modda) ·
10-contended-realization ✓ · 11-floor-kapsaması ✓ (reliability-doygun-şerhiyle) · 12-displacement-bounded ✓ ·
13-tail-unbounded-YOK ✓ (≤+16%-raporlu) · 14-bimodal-gizlenmedi ✓ · 15-BtlBw-kilidi-yanlış-atfedilmedi ✓.
RED-taraması: parity↑&reliability-değişmez → doygun-zarf-şerhi (RED-değil; kalibrasyon-sorusu);
diğer-RED'ler-tetiklenmedi.
