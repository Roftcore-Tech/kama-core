# 1b Budget Characterization — Micro-Sweep 1 (Commit-O) [KESİT-2-DEĞİLDİR; policy-değişikliği-YOK]

> Harness: `kama-engine/tests/micro_sweep_1b.rs` (deterministik-sim; model-beyanı-m1-harness'la-aynı;
> Controller-arm GERÇEK-fuzzy-yolu ENJEKSİYONSUZ). 10-hücre × 5-arm × 10-paired-seed (gap80: 30-seed).
> Veri: [data/micro-sweep-1b/](data/micro-sweep-1b/) — seed-level + blok-ledger(.gz) + interval(.gz) +
> summary + btlbw-offered + counterfactual. Yeniden-üretim: `MS1_OUT=<dir> cargo test --release -p
> kama-engine --test micro_sweep_1b micro_sweep_1b -- --ignored` (4s) + `scripts/ms1_analyze.py` +
> `scripts/ms1_counterfactual.py`. PRE-DECLARED equivalence/validity-kuralları harness+script-başlığında
> (koşudan-önce-yazıldı). Bootstrap-%95-CI deterministik (RNG-kütüphanesiz-xorshift).

## 1. ANA-BULGULAR (özet)

**B1 — Controller-ratio-arm 10/10-hücrede parity-talebi-SIFIR (NO-PARITY-DEMAND).** Gerçek-fuzzy,
gerçek-event-akışıyla her-hücrede ratio=0 üretti (enq=0; Controller≡ARQ-baseline byte-eşdeğer).
NEDEN (ölçülü): realized `loss_windowed_ppm` p50 — nominal(raw-5%): 10-55k · loss20(raw-20%): ~102-107k
— hepsi fixture-loss.LOW-plateau'sunun (≤200k) İÇİNDE → R1 ateşliyor → fec_zero. **Ham-kayıp-%20-bile
pencere-ölçümünde ~%10.5-ppm'e-düşüyor** (retx-başarıları+decay). Fixture-eşikleri (200k/600k) bu-
sweep'in-gerçekleşebilir-loss-zarfının ÜSTÜNDE — Kesit-P-realizability-map'in-ana-girdisi. (Eşik-
DEĞİŞTİRİLMEDİ; kalibrasyon-verisi.)

**B2 — Arm-collapse NİCEL-AÇIKLANDI: emitted-tavan-arz-sınırlı (talep-bağımsız).** rtt-d1000'de aynı-
seed'de r2/r4/r8 emit'leri BİREBİR-EŞİT (ör. seed-11: 10/10/10; seed-23: 13/13/13). Kuyruk-hiç-boşalmıyor
→ emit = leftover-throughput × koşu-süresi (talep-r'den-bağımsız). requested-r-artışı yalnız-kuyruk/
denial-sayısını-büyütüyor (d1000 denial: r2-1570 → r8-görülen-artış; first-parity-delay 37→45s).

**B3 — gap80-BİMODALLİK 30-seed'de-doğrulandı + mekanizma:** emit-dağılımı iki-keskin-küme:
{1-2}×7-seed vs {92-99}×23-seed (arada-HİÇ-değer-yok). Düşük-küme btlbw-kilidi-ile-korele (btlbw_p50:
0/18000 vs 24000-30000): erken-event-sırası BtlBw'yi-düşük-offered-kilidine-oturtursa rate→leftover→
parity tüm-koşu-boyunca-açlıkta. Seed-bimodalliği = kilit-değerinin-erken-kilitlenme-yarışı.

**B4 — Kilit-vs-starvation AYRIŞTI (talimat-faz-amacı-3):**
- *Kilitsiz-starvation:* offered-c1 (offered≈C): btlbw-SAĞLIKLI (64k ≥ C=60k) ama realization 0.01-0.02
  (emit=2) — saf-DATA-first-token-yarışı.
- *Kilit-kaynaklı:* gap80-düşük-kümesi (btlbw 0-18k ≪ C) — leftover-yapısal-yok.
- *Bileşke:* d1000/d2500 (btlbw 12k/4k ≪ C=60k KİLİT + retx-yükü) → tavan 10-27-emit.
- *Parity-trafiği-kilidi-KISMEN-KIRAR:* bw12-hücresinde ARQ-arm btlbw 6315 vs Manual-r4 13714 (≈0.95C)
  — parity-hacmi delivered-örneklerini-zenginleştirip-kilidi-yukarı-taşıyor (in-run-probe-etkisi).

**B5 — Token-denial = normal-bekleme-mekanizması, başına-metrik-DEĞİL.** nominal-r2: 132-denial ama
realization 1.00. Anlamlı-üçlü: (realization, first-parity-delay, denial) — tek-başına-denial-sayısı
starvation-hükmü-veremez (Commit-N-attribution'ının-1b-checklist-şartıyla-tutarlı).

## 2. VALIDITY-SINIFLARI (özet-tablo; tam: ms1-summary.csv)

| hücre | r2 | r4 | r8 | Controller |
|---|---|---|---|---|
| nominal | VALID | VALID | VALID | NO-DEMAND |
| gap80 | VALID+**BIMODAL** | VALID+**BIMODAL** | PARTIAL | NO-DEMAND |
| rtt-d1000 | PARTIAL(0.27) | PARTIAL(0.135) | PARTIAL(0.0675) | NO-DEMAND |
| rtt-d2500 | PARTIAL(0.655) | PARTIAL(0.3275) | PARTIAL(0.164) | NO-DEMAND |
| loss20 | VALID+BIMODAL | PARTIAL(0.5375) | PARTIAL(0.269) | NO-DEMAND |
| rev99 | PARTIAL(0.44) | PARTIAL(0.22) | PARTIAL(0.11) | NO-DEMAND |
| bw12 | VALID | VALID | VALID | NO-DEMAND |
| offered-c8 | VALID | VALID | PARTIAL(0.795) | NO-DEMAND |
| offered-c1 | PARTIAL(0.02) | PARTIAL(0.01) | PARTIAL(0.005) | NO-DEMAND |
| offered-1p5c | VALID | PARTIAL(0.795) | VALID(0.819) | NO-DEMAND |

Not: FULLY-COLLAPSED-eşiği (median-realization==0) hiçbir-hücrede-tetiklenmedi (en-düşük 0.005>0);
offered-c1/d1000 pratik-collapse-bölgesi. **Hiçbir-PARTIAL/collapse-hücresinden FEC-ratio-crossover-
hükmü-üretilmez** (pre-declared). completion=1.0-tüm-hücrelerde (bw12 600s-ufukta-tamamlandı; kesit-1-
censoring'i-300s-ufuk-artefaktıydı — burada-censoring-yalnız-ölçüldü, hiç-tetiklenmedi).

## 3. LEDGER-SEMANTİĞİ ve KİMLİK (test-gate)

Per-blok: `enqueued == emitted + canceled + pending_end` — **token-denied KATEGORİ-DEĞİL** (bekleme-
bağlamı; aynı-parity-iki-kategoriye-SAYILMAZ). Kimlik run-içinde-assert'li + global-çapraz
(Σledger == diag-sayaçları üçlüsü) + cancel-atıf-tamlığı ("çift-sayım-yok"-assert) + deterministik-
rerun-bit-özdeşliği: `ms1_ledger_consistency_smoke` (CI'da-koşan-hızlı-gate). Paired-seed: aynı-seed-
listesi-tüm-arm'larda (harness-yapısal).

## 4. BtlBw-OFFERED tablosu ve §8-soruları (tam: ms1-btlbw-offered.csv)

| hücre | C | offered | delivered-med | btlbw-p50 | rate-p50 | realization-r4 |
|---|---|---|---|---|---|---|
| nominal | 60000 | 30000 | 29869 | 60000 | 60000 | 0.995 |
| gap80 | 60000 | 15000 | 14944 | 26666 | 26666 | 0.902 |
| offered-c8 | 60000 | 7500 | 7489 | 24000 | 38858 | 0.917 |
| offered-c1 | 60000 | 60000 | 52864 | 64000 | 62414 | 0.010 |
| offered-1p5c | 60000 | 92307 | 18412 | 45000 | 45000 | 0.795 |
| rtt-d1000 | 60000 | 30000 | 9024 | 12000 | 12000 | 0.135 |
| rtt-d2500 | 60000 | 30000 | 5819 | 3922 | 7798 | 0.327 |
| rev99 | 60000 | 30000 | 9832 | 10535 | 24198 | 0.220 |
| bw12 | 14400 | 7228 | 1445 | 13714 | 13714 | 0.950 |

**§8-cevapları:**
1. *Kilit-ne-zaman?* offered ≤ C/4'te belirgin (gap80: 26.6k≈0.44C; c8: 24k=0.4C); offered=C/2-nominal'de
   kilit-YOK (burst-serialization-tam-C-ölçtürüyor). Uzun-RTT kilidi-şiddetlendiriyor (d1000: 0.2C;
   d2500: 0.065C) — pencere/EAI-etkileşimi.
2. *Clean-burst-kırması?* Bu-sweep'te-ayrı-clean-burst-enjekte-edilmedi; İN-RUN-kanıt: parity-hacmi-
   kendisi-kilidi-yukarı-taşıyor (bw12: 6315→13714; fuzzy_e2e-harness'ında-ayrıca: 3-ardışık-burst-
   round → ~1MB/s-örneği, W_BTLBW≈16.8s-boyunca-tutuyor).
3. *Seed-bimodalliği-event-sırası:* erken-faz-kilit-yarışı (B3): düşük-btlbw-kilitlenen-seed'ler
   (18k-vs-24k+) tüm-koşu-parity-açlığında; kilit-değeri-ilk-saniyelerin-ACK/loss-desenine-bağlı.
4. *Parity-token-üretimi-BtlBw'ye-bağlı-mı?* EVET-dolaylı-ve-güçlü: token-accrual = pacing-rate,
   pacing-rate-BtlBw-türevi → kilit-düşükse-leftover-yapısal-küçülür (d1000-zinciri: btlbw12k→rate12k→
   leftover≈%2-5→tavan-27).
5. *Requested-r↑-emitted-doygunluğu:* arz-sınırı (B2): kuyruk-boşalmıyor; emit = leftover×süre/UNIT —
   r'den-bağımsız (aynı-seed-birebir-eşit-emit-kanıtı).
6. *Collapse: policy-mi-measurement-mı?* **BİLEŞKE, ayrıştırılmış:** saf-policy-kolu offered-c1
   (ölçüm-sağlıklı, leftover=0); saf-ölçüm-kolu gap80-düşük-küme (offered-düşük, kilit-yapay-düşük);
   d1000/d2500 ikisi-birden. BtlBw-düzeltme-önerisi-bu-rapordan-ÇIKMAZ (ayrı-DR-gerektirir;
   DEĞİŞİKLİK-YASAĞI-korundu).

## 5. OFFLINE-COUNTERFACTUAL (talimat-§9; YALNIZ-MODELLEME — onay-değil)

Replay-varsayımları V1-V4 script-başlığında (kanal-feedback-sabit; 1s-bin-leftover). **Fidelity-kapısı
(baseline-replay-vs-ölçülen-emit):** nominal 0.02 · gap80 0.019 · bw12 0.029 · offered-c8 0.04 ·
offered-1p5c 0.18 → GÜVENİLİR-bölge; loss20 0.49 · rev99 3.5 · d1000 3.7 · d2500 2.2 · offered-c1 31.5 →
**REPLAY-UNRELIABLE** (1s-bin-leftover contended-rejimde-aşırı-iyimser; burst-cap-mikro-dinamiği-yıkanıyor).
Model-karşılaştırması-yalnız-güvenilir-bölgeden-hüküm-üretir; unreliable-hücrelerde-yalnız-yön-işareti.

| model | güvenilir-bölge-sonucu | DATA-riski | kuyruk | starve | değerlendirme |
|---|---|---|---|---|---|
| Baseline (leftover) | real 0.89-1.00; starve 0-0.02 (temiz) AMA d1000-yönünde 0.59-starve | 0 | orta | yüksek-contended | mevcut-davranış: temizde-yeterli, kilit/RTT'de-açlık |
| A bounded-share α=.25 | ≈baseline-temizde; d1000-yönü +0.24-real | 0.03-0.16-displacement | orta | orta | açlığı-azaltır ama DATA-yerinden-etme-payı-ölçülür-riskte |
| B micro-bucket β=.15/4U | real 0.32-0.52; delay 5-8s | 0 | EN-BÜYÜK | 0.5-0.8 | (bu-parametrelerle) RED — bucket-granülaritesi-parity'yi-boğuyor |
| C admission-cap-2 | real ~0.43-0.50 (yapısal-yarıya) | 0 | EN-KÜÇÜK | düşük | tek-başına-RED (korumayı-atıyor); pending-cap-TAMAMLAYICISI-olarak-değerli |
| D floor(1/blok≤2s)+cap(.30) | ≥baseline-her-hücrede; **starve=0.00 d1000/d2500-dahil**; displacement ≤0.04 | ~0.004-0.04 | orta | **0** | **EN-GÜÇLÜ-ADAY** — bounded-yerinden-etmeyle-starvation-eliminasyonu |
| E deadline-aware | DEĞERLENDİRİLMEDİ | — | — | — | deadline-semantiği-tanımsız (fuzzy-deadline-girdisi-yok; talimat-şartı) |

Diğer-boyutlar (tüm-modeller): determinism-korunur (sabit-aritmetik); memory-bound C+pending-cap-ile;
headroom-blindness A/B/D'de-sürer (hepsi-rate-türevi → BtlBw-kilit-duyarlılığı-AYNEN — kilit-düzeltmesi-
ayrı-DR); emergency-etkileşimi: planned=0-interlock-tüm-modellerde-üstte (latch-katmanı; 1b-değiştirmez);
1d-ilişkisi: dup↔parity-double-spend-D'nin-floor'una-1d-arbitrajı-gerekir (sınır-korundu).
**Hiçbir-model-sonucu-implementasyon-onayı-DEĞİLDİR** (DR-1b-v1-tasarım-girdisi).

## 6. Kesit-2'nin 1b-öncesi-geçerli-alt-kesitleri

Geçerli (VALID-SEPARATED + fidelity-uyumlu): nominal · bw12 · offered-c8 · gap80-yüksek-küme (r2/r4) ·
offered-1p5c(r2). Geçersiz-1b-öncesi (PARTIAL/collapse/kilit): d1000 · d2500 · rev99 · offered-c1 ·
loss20(r4/r8) · gap80-düşük-küme. Controller-ratio-arm'lı-her-kesit ayrıca-KALİBRASYON-blokajlı
(B1: fixture-eşikleri-realizable-zarfın-üstünde — Commit-P-realizability-map).
