# AA — Seed-Cluster-İstatistiği + Spend-Matched + Blackout-Kök-Neden [inference-birimi=SEED-CLUSTER]

> Bloklar-bağımsız-örnek-SAYILMADI; cluster-bootstrap-seed-resample-1000 (deterministik). rev99
> ÖN-BEYANLI-30-seed'e-genişletildi (10-seed-dl_op-CI-hassasiyet-yetersizliği — §2-kuralı; sonuç-
> görülüp-seed-seçme-YAPILMADI: sabit-ek-20-seed-listesi). Veri: aa-*.csv (data/policy-outcome/).

## 1. Cluster-CI (rev99-30-seed; %95; sıfır-dışlama-işaretli)
| karşılaştırma | metrik | medyan-Δ | CI | seeds(+/−/0) | sıfır-dışlar |
|---|---|---|---|---|---|
| F15−L | dl_op_rate | **+0.100** | [0.040, 0.170] | 24/2/4 | **✓** |
| F15−L | decodes | +25 | [22, 27.5] | 30/0/0 | ✓ |
| F15−L | avoided_retx | +16 | [10, 18.5] | 30/0/0 | ✓ |
| F15−L | worst-decile-comp | **−4825ms** | [−6920, −2900] | 2/28/0 | ✓ |
| F15−P15 | dl_op_rate | 0.000 | [−0.01, +0.02] | 13/10/7 | ✗ (EŞDEĞER) |
| F15−P15 | decodes | **+15.5** | [9.5, 19] | 28/2/0 | ✓ |
| F15−P15 | avoided_retx | +9 | [7, 13.5] | 28/1/1 | ✓ |
| F15−P15 | **parity_bytes** | **−48 000** | [−64.2k, −33.6k] | 1/29/0 | ✓ (F15-DAHA-AZ-harcar) |
| F15−P15 | worst-decile-comp | −1675ms | [−2550, −350] | 7/22/1 | ✓ |
Diğer-hücreler-aynı-yön (blackout-30s/manyblock/d1000-ge/ge-long decode-CI'ları-sıfır-dışlar; tam-CSV).

## 2. rev99 seed-level rescued−harmed (F15-vs-P15; blok-toplamı-bağımsız-deneme-sayılmadı)
Dağılım (30-seed): {−6..−1}×10-seed · 0×7 · {+1..+10}×13-seed; toplam-net≈+24-blok — tek/iki-seed-
taşıması-YOK; dl_op-düzeyinde-eşdeğerlik-bandında (yukarıdaki-CI).

## 3. SPEND-MATCHED denetimi — fairness≠fazla-harcama
F15−P15 parity-spend NEGATİF-CI'lı (rev99 −48k; blackout-30s −61.8k; d1000-ge −3.6k; ge-long −4.8k) →
**F15 P15'ten-DAHA-AZ-parity-byte-harcayıp-DAHA-ÇOK-decode-üretir** (decode_delta/parity_delta-oranı-
negatif-paydalı → verimlilik-üstünlüğü; "yalnız-daha-fazla-token"-açıklaması-ELENDİ). P15-fazlası
moot/konsantre-repair'lere-gidiyor (ledger-moot-parity-alanı).

## 4. Blackout-30s KÖK-NEDEN (10-harmed-blok-timeline; tam-CSV)
FAZ: 10/10 karanlık-BAŞLANGICINDA-açılan-bloklar (~60-63s; 9-during_blackout + 1-recovery-sınırı).
MEKANİZMA: L-aynı-bloğu-620ms-2.2s'de-teslim-ediyor (forward-yol-AÇIK; kanal-izin-veriyor) — F15'te
karanlıkta-biriken-ÖNCEKİ-blok-parity-backlog'unun-drenajı yeni-DATA'yı-2-13s-itiyor (displacement-
kaynaklı-gerçek-politika-maliyeti; gürültü-DEĞİL). miss-margin 270ms-5.4s. Cevaplar: (1)-yeni-DATA-
emisyonu-gecikti (rdisp/ndisp-kayıtlı); (2)-marj-CSV'de; (3)-blok-kendisi-8/10-decode-etti-ama-source-
tesliminden-geç; (4)-EVET-parity-önceki-bloklara-fairness-akışıydı; (5)-**F10 10-harmed'in-6'sını-
kurtarıyor** (aynı-paired-trace); (6)-P15-üstünlüğü-DÜŞÜK-displacement'tan (concentration'dan-değil —
P15-decode-aynı-bloklarda-daha-az); (7)-horizon-aşırı-sert-DEĞİL (L-tamamlıyor); (8)-EVET-faz-duyarlı
(yalnız-dark-start-açılışları).

## 5. Recovery-normalized (tamamlayıcı-perspektif; operational-SONUÇ-DEĞİŞTİRİLMEDİ)
T_recov = ilk-karanlık-bitişi + 10s-allowance (ÖN-BEYANLI): 10/10-harmed-blok HEM-F15-HEM-P15'te
recovery-normalized-İÇİNDE-tamamlanıyor → harm-yalnız-service-deadline-katmanında; kalıcı-kayıp-yok.
