# FG — Shadow Candidate Değerlendirmesi: NO CANDIDATE WARRANTED [production-fuzzy-config-DEĞİŞMEDİ]

> **BAĞLAYICI-ÇERÇEVE (zorunlu-başlık):** Bu deney ARQ ile FEC'i KARŞILAŞTIRMAZ. ARQ bütün-kollarda
> AKTİFTİR (aynı-impl/ACK-NACK/retransmit-zamanlayıcı/retry-semantiği); değişen-tek-değişken ARQ-substrate'ı
> ÜZERİNE eklenen PROAKTİF-FEC-repair-miktarıdır (r0=ARQ+0, r1=ARQ+1, r2=ARQ+2, r4=ARQ+4). Ölçülen =
> **ARQ-substrate üzerine PROAKTİF-FEC'in MARJİNAL-FAYDASI** (marginal-FEC-utility-over-ARQ). FEC hiçbir-
> kolda ARQ'nun-yerine-geçmez; FEC'in-kapatamadığı-residual-deficit her-zaman-ARQ'yla-tamamlanır.


> Shadow-mode (actuation-YOK; salt-analiz; production-decision-etkilenmez). 231 390-gerçek-event.
> Baseline=FROZEN-fixture. Script: `scripts/netem/fuzzy_shadow_eval.py`.

## Aday-A (aggressive threshold-alignment) shadow-sonucu
loss.LOW-shoulder 200k/300k → 30k/60k; HIGH 500k/600k → 90k/150k (gözlem-zarfına-maksimum-çekme):
| metrik | değer |
|---|---|
| baseline_nonzero_frac | 0.0000 |
| **candidate_nonzero_frac** | **0.0000** |
| agree_frac | 1.0000 |
| overprotect_frac | 0.0000 |

## Airtight-bulgu: HİÇBİR-threshold-adayı ratio>0-üretemez
`loss_windowed_ppm` 231k-event'in-TAMAMINDA-**0** → `trap(0, 0,0,c,d)=1.0` (loss.LOW-her-eşikte-1.0) →
R1-ateşler → ratio=0. **Eşiği-1-ppm'e-bile-çekmek-değiştirmez** (loss=0 → loss.low=1 daima). Sorun
fuzzy-EŞİKLERİNDE-DEĞİL, loss-SİNYALİNDE (gerçek-stack'te-0).

## Sonuç: aday-config-ÜRETİLMEDİ (evidence-desteklemiyor)
- Candidate-A (threshold): ETKİSİZ (loss-signal-0 → ratio-değişmez). RED.
- Candidate-B (coverage-repair): gereksiz (no-rule-frac=0; delik-yok). RED.
- Candidate-C (outcome-aware): oracle-hiçbir-profilde-FEC-faydası-göstermedi → r>0-bölgesi-YOK. RED.
§17-acceptance: hiçbir-aday "oracle-aligned-decision-artışı" gösteremez (baseline BU-KOŞULLARDA-zaten-%100-
CorrectZeroProactiveFec). **YASAK-YORUM-DÜZELTMESİ:** bu, "FEC'in-faydalı-olduğu-hiçbir-rejim-yok" DEMEK-
DEĞİLDİR — test-edilen-koşullarda ARQ-substrate-deadline'ı-karşıladığı-için-proaktif-FEC-marjinal-fayda-
üretmedi; ARQ-limited/FEC-feasible-rejim (ARQ-RTT>>deadline ∧ forward-recovery-kazanır) bu-turda-üretilmedi.
**Live-prototype-review'e-taşınacak-aday-YOK.** Doğru-müdahale-noktası (varsa) = loss-signal-fidelity
(ayrı-DR; fuzzy-calibration-değil).
