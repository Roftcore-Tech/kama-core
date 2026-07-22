# FG — Shadow Candidate Değerlendirmesi: NO CANDIDATE WARRANTED [production-fuzzy-config-DEĞİŞMEDİ]

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
§17-acceptance: hiçbir-aday "oracle-aligned-decision-artışı" gösteremez (baseline-zaten-%100-CorrectZero).
**Live-prototype-review'e-taşınacak-aday-YOK.** Doğru-müdahale-noktası (varsa) = loss-signal-fidelity
(ayrı-DR; fuzzy-calibration-değil).
