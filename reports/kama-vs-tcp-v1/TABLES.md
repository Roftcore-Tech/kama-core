
## ANA TABLO — WARM, ürün (kama-v1-full) ↔ tcp-std, R1+R2 havuz

| kayıp | RTT | yük | kama-v1-full horizon x/N [Wilson95] | tcp-std horizon x/N [Wilson95] | kama-v1-full deadline | tcp-std deadline | kama-v1-full t_tam p50/p90 ms | tcp-std t_tam p50/p90 ms | kama-v1-full tel B p50 | tcp-std tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 10% | low | large | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 484.278 / 755.715 | 818.788 / 1655.801 | 140346.0 | 136213.0 | ayırt edilemedi |
| 10% | low | small | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 126/200 [0.56,0.69] | 141/200 [0.64,0.76] | 206.138 / 346.638 | 200.460 / 458.467 | 14346.5 | 12519.5 | ayırt edilemedi |
| 10% | med | large | 58/60 [0.89,0.99] | 60/60 [0.94,1.00] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 2511.862 / 4579.258 | 3824.758 / 6547.316 | 137231.0 | 138905.0 | ayırt edilemedi |
| 10% | med | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 27/60 [0.33,0.58] | 31/60 [0.39,0.64] | 1215.362 / 2119.872 | 1073.917 / 2043.250 | 14232.0 | 12752.5 | ayırt edilemedi |
| 10% | high | large | 55/60 [0.82,0.96] | 54/60 [0.80,0.95] | 0/60 [0.00,0.06] | 1/60 [0.00,0.09] | 8878.124 / 11899.291 | 10003.544 / 15961.348 | 136388.0 | 137178.0 | ayırt edilemedi |
| 10% | high | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 35/60 [0.46,0.70] | 41/60 [0.56,0.79] | 4120.827 / 6350.045 | 4001.601 / 6834.738 | 15830.5 | 13045.0 | ayırt edilemedi |
| 20% | low | large | 52/60 [0.76,0.93] | 59/60 [0.91,1.00] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 719.307 / 2883.537 | 2549.060 / 6409.807 | 159928.0 | 153447.5 | ayırt edilemedi |
| 20% | low | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 6/60 [0.05,0.20] | 12/60 [0.12,0.32] | 300.483 / 881.638 | 479.581 / 1069.864 | 21408.5 | 13907.5 | ayırt edilemedi |
| 20% | med | large | 42/60 [0.57,0.80] | 47/60 [0.66,0.87] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 3400.062 / 9151.303 | 8893.930 / 12486.875 | 153018.5 | 152877.5 | ayırt edilemedi |
| 20% | med | small | 60/60 [0.94,1.00] | 58/60 [0.89,0.99] | 5/60 [0.04,0.18] | 17/60 [0.19,0.41] | 1967.120 / 3802.741 | 1406.683 / 3573.762 | 15651.5 | 12673.0 | ayırt edilemedi |
| 20% | high | large | 46/60 [0.65,0.86] | 17/60 [0.19,0.41] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 12720.506 / 17640.824 | 15343.747 / 19764.348 | 151966.0 | 141579.0 | **kama-v1-full ÖNDE** |
| 20% | high | small | 59/60 [0.91,1.00] | 53/60 [0.78,0.94] | 8/60 [0.07,0.24] | 17/60 [0.19,0.41] | 6559.982 / 10200.272 | 6254.575 / 13326.234 | 17742.0 | 13615.5 | ayırt edilemedi |
| 30% | low | large | 49/60 [0.70,0.89] | 24/60 [0.29,0.53] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 1418.384 / 7749.176 | 6660.924 / 10495.180 | 184310.5 | 163587.5 | **kama-v1-full ÖNDE** |
| 30% | low | small | 59/60 [0.91,1.00] | 53/60 [0.78,0.94] | 2/60 [0.01,0.11] | 5/60 [0.04,0.18] | 659.346 / 3059.496 | 1319.053 / 4377.797 | 22519.5 | 14282.5 | ayırt edilemedi |
| 30% | med | large | 105/200 [0.46,0.59] | 15/200 [0.05,0.12] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 4980.592 / 9340.141 | 10368.172 / 11838.770 | 170486.5 | 109396.0 | **kama-v1-full ÖNDE** |
| 30% | med | small | 59/60 [0.91,1.00] | 51/60 [0.74,0.92] | 2/60 [0.01,0.11] | 2/60 [0.01,0.11] | 2677.684 / 6899.518 | 3135.176 / 8381.641 | 17997.5 | 13717.0 | ayırt edilemedi |
| 30% | high | large | 50/200 [0.20,0.31] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 17060.226 / 19518.338 |  /  | 135354.5 | 72204.0 | **kama-v1-full ÖNDE** |
| 30% | high | small | 52/60 [0.76,0.93] | 46/60 [0.65,0.86] | 2/60 [0.01,0.11] | 6/60 [0.05,0.20] | 8440.327 / 15846.310 | 8299.651 / 14851.237 | 20006.0 | 14459.5 | ayırt edilemedi |
| 40% | low | large | 41/60 [0.56,0.79] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 3361.449 / 8420.033 |  /  | 205310.5 | 67798.5 | **kama-v1-full ÖNDE** |
| 40% | low | small | 54/60 [0.80,0.95] | 37/60 [0.49,0.73] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 1280.244 / 4759.697 | 1574.921 / 8921.129 | 23788.0 | 13658.5 | **kama-v1-full ÖNDE** |
| 40% | med | large | 13/60 [0.13,0.34] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 9187.803 / 11540.681 |  /  | 106803.5 | 53450.0 | **kama-v1-full ÖNDE** |
| 40% | med | small | 47/60 [0.66,0.87] | 32/60 [0.41,0.65] | 0/60 [0.00,0.06] | 1/60 [0.00,0.09] | 5884.560 / 11020.213 | 6207.010 / 12095.877 | 20509.5 | 14922.5 | **kama-v1-full ÖNDE** |
| 40% | high | large | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] |  /  |  /  | 73277.0 | 40161.5 | ayırt edilemedi |
| 40% | high | small | 37/60 [0.49,0.73] | 21/60 [0.24,0.48] | 1/60 [0.00,0.09] | 0/60 [0.00,0.06] | 12472.058 / 17279.939 | 13151.791 / 17135.024 | 22809.5 | 12393.0 | **kama-v1-full ÖNDE** |
| 50% | low | large | 65/200 [0.26,0.39] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 0/200 [0.00,0.02] | 5784.680 / 9082.034 |  /  | 141389.5 | 39769.0 | **kama-v1-full ÖNDE** |
| 50% | low | small | 39/60 [0.52,0.76] | 11/60 [0.11,0.30] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 3080.663 / 7580.045 | 2341.226 / 3911.916 | 26079.0 | 12753.0 | **kama-v1-full ÖNDE** |
| 50% | med | large | 1/60 [0.00,0.09] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 7825.727 / 7825.727 |  /  | 44555.0 | 29580.5 | ayırt edilemedi |
| 50% | med | small | 29/60 [0.36,0.61] | 7/60 [0.06,0.22] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 8640.160 / 12047.540 | 8466.757 / 11066.063 | 23453.5 | 12631.5 | **kama-v1-full ÖNDE** |
| 50% | high | large | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] |  /  |  /  | 45432.0 | 21316.5 | ayırt edilemedi |
| 50% | high | small | 21/60 [0.24,0.48] | 4/60 [0.03,0.16] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 14780.681 / 19161.126 | 16704.027 / 18963.321 | 26361.0 | 10449.5 | **kama-v1-full ÖNDE** |

Özet: **kama-v1-full ÖNDE**: 13 · ayırt edilemedi: 17

### KAMA'nın GERİDE olduğu hücreler (horizon, aralıklar ayrışıyor)

- (yok)

### Tamamlanma SÜRESİNDE KAMA'nın geride olduğu hücreler (iki kol da ≥ %90 horizon-complete; p50 kıyas, aralık testi YOK — betimsel)

- control · kayıp 0% · RTT none · large: kama p50 66.1 ms  vs  tcp p50 1.0 ms
- control · kayıp 0% · RTT none · small: kama p50 16.8 ms  vs  tcp p50 0.5 ms
- main · kayıp 10% · RTT high · small: kama p50 4120.8 ms  vs  tcp p50 4001.6 ms
- main · kayıp 10% · RTT low · small: kama p50 206.1 ms  vs  tcp p50 200.5 ms
- main · kayıp 10% · RTT med · small: kama p50 1215.4 ms  vs  tcp p50 1073.9 ms
- main · kayıp 20% · RTT med · small: kama p50 1967.1 ms  vs  tcp p50 1406.7 ms

## KONTROL (bozulmasız) — her kol N/N tamamlamalı

| kayıp | RTT | yük | kama-v1-full horizon x/N [Wilson95] | tcp-std horizon x/N [Wilson95] | kama-v1-full deadline | tcp-std deadline | kama-v1-full t_tam p50/p90 ms | tcp-std t_tam p50/p90 ms | kama-v1-full tel B p50 | tcp-std tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0% | none | large | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 66.138 / 76.084 | 0.965 / 1.293 | 120349.0 | 127679.0 | ayırt edilemedi |
| 0% | none | small | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 16.848 / 19.184 | 0.512 / 0.619 | 11748.0 | 15834.0 | ayırt edilemedi |

Özet: ayırt edilemedi: 2

## COLD senaryo (kurulum pencerenin İÇİNDE) — manşet hücreler

| kayıp | RTT | yük | kama-v1-full horizon x/N [Wilson95] | tcp-std horizon x/N [Wilson95] | kama-v1-full deadline | tcp-std deadline | kama-v1-full t_tam p50/p90 ms | tcp-std t_tam p50/p90 ms | kama-v1-full tel B p50 | tcp-std tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 10% | low | small | 60/60 [0.94,1.00] | 60/60 [0.94,1.00] | 39/60 [0.52,0.76] | 18/60 [0.20,0.43] | 185.970 / 326.600 | 267.637 / 1238.921 | 11376.0 | 9835.5 | ayırt edilemedi |
| 30% | med | large | 42/60 [0.57,0.80] | 8/60 [0.07,0.24] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 6126.415 / 10235.689 | 10911.818 / 12266.118 | 157054.5 | 122036.5 | **kama-v1-full ÖNDE** |
| 30% | high | large | 19/60 [0.21,0.44] | 1/60 [0.00,0.09] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 18480.633 / 19653.218 | 15596.356 / 15596.356 | 160712.5 | 89670.5 | **kama-v1-full ÖNDE** |
| 50% | low | large | 18/60 [0.20,0.43] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 7546.105 / 10518.845 |  /  | 192938.5 | 33537.5 | **kama-v1-full ÖNDE** |

Özet: **kama-v1-full ÖNDE**: 3 · ayırt edilemedi: 1

## BİLEŞEN ABLASYONU — çıplak Sender sürücüsü, tail OFF ↔ ON (ürün DEĞİL; TCP ile kıyaslanmaz)

| kayıp | RTT | yük | abl-off horizon x/N [Wilson95] | abl-on horizon x/N [Wilson95] | abl-off deadline | abl-on deadline | abl-off t_tam p50/p90 ms | abl-on t_tam p50/p90 ms | abl-off tel B p50 | abl-on tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 10% | low | small | 56/60 [0.84,0.97] | 59/60 [0.91,1.00] | 35/60 [0.46,0.70] | 37/60 [0.49,0.73] | 204.000 / 3300.000 | 212.000 / 345.000 | 36523.0 | 33153.5 | ayırt edilemedi |
| 20% | low | small | 43/60 [0.59,0.81] | 57/60 [0.86,0.98] | 7/60 [0.06,0.22] | 10/60 [0.09,0.28] | 320.000 / 3484.000 | 363.000 / 811.000 | 223091.0 | 78034.0 | **abl-on ÖNDE** |
| 30% | low | small | 43/60 [0.59,0.81] | 54/60 [0.80,0.95] | 10/60 [0.09,0.28] | 9/60 [0.08,0.26] | 347.000 / 3437.000 | 395.000 / 1186.000 | 119691.0 | 88712.0 | ayırt edilemedi |
| 40% | low | small | 31/60 [0.39,0.64] | 47/60 [0.66,0.87] | 1/60 [0.00,0.09] | 2/60 [0.01,0.11] | 566.000 / 4637.000 | 633.000 / 1687.000 | 342094.0 | 108155.0 | **abl-on ÖNDE** |
| 50% | low | small | 30/60 [0.38,0.62] | 38/60 [0.51,0.74] | 0/60 [0.00,0.06] | 0/60 [0.00,0.06] | 834.000 / 4676.000 | 763.000 / 2814.000 | 272218.5 | 115366.0 | ayırt edilemedi |

Özet: **abl-on ÖNDE**: 2 · ayırt edilemedi: 3

## KONTROL (ablasyon kolları)

| kayıp | RTT | yük | abl-off horizon x/N [Wilson95] | abl-on horizon x/N [Wilson95] | abl-off deadline | abl-on deadline | abl-off t_tam p50/p90 ms | abl-on t_tam p50/p90 ms | abl-off tel B p50 | abl-on tel B p50 | hüküm (horizon) |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0% | none | small | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 200/200 [0.98,1.00] | 42.000 / 49.000 | 43.000 / 50.000 | 20256.0 | 20151.0 | ayırt edilemedi |

Özet: ayırt edilemedi: 1

## R1 ↔ R2 UYUMU (kol × hücre; horizon oranı)

| grup | kayıp | RTT | yük | mod | kol | R1 x/N | R2 x/N | aralıklar | 
|---|---|---|---|---|---|---|---|---|
| ablation | 10% | low | small | warm | abl-off | 28/30 | 28/30 | örtüşüyor |
| ablation | 10% | low | small | warm | abl-on | 30/30 | 29/30 | örtüşüyor |
| ablation | 20% | low | small | warm | abl-off | 25/30 | 18/30 | örtüşüyor |
| ablation | 20% | low | small | warm | abl-on | 30/30 | 27/30 | örtüşüyor |
| ablation | 30% | low | small | warm | abl-off | 21/30 | 22/30 | örtüşüyor |
| ablation | 30% | low | small | warm | abl-on | 28/30 | 26/30 | örtüşüyor |
| ablation | 40% | low | small | warm | abl-off | 17/30 | 14/30 | örtüşüyor |
| ablation | 40% | low | small | warm | abl-on | 24/30 | 23/30 | örtüşüyor |
| ablation | 50% | low | small | warm | abl-off | 11/30 | 19/30 | örtüşüyor |
| ablation | 50% | low | small | warm | abl-on | 17/30 | 21/30 | örtüşüyor |
| cold | 10% | low | small | cold | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| cold | 10% | low | small | cold | tcp-std | 30/30 | 30/30 | örtüşüyor |
| cold | 30% | high | large | cold | kama-v1-full | 11/30 | 8/30 | örtüşüyor |
| cold | 30% | high | large | cold | tcp-std | 0/30 | 1/30 | örtüşüyor |
| cold | 30% | med | large | cold | kama-v1-full | 22/30 | 20/30 | örtüşüyor |
| cold | 30% | med | large | cold | tcp-std | 6/30 | 2/30 | örtüşüyor |
| cold | 50% | low | large | cold | kama-v1-full | 10/30 | 8/30 | örtüşüyor |
| cold | 50% | low | large | cold | tcp-std | 0/30 | 0/30 | örtüşüyor |
| control | 0% | none | large | warm | kama-v1-full | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | large | warm | tcp-std | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | abl-off | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | abl-on | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | kama-v1-full | 100/100 | 100/100 | örtüşüyor |
| control | 0% | none | small | warm | tcp-std | 100/100 | 100/100 | örtüşüyor |
| main | 10% | high | large | warm | kama-v1-full | 27/30 | 28/30 | örtüşüyor |
| main | 10% | high | large | warm | tcp-std | 26/30 | 28/30 | örtüşüyor |
| main | 10% | high | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 10% | high | small | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 10% | low | large | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 10% | low | large | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 10% | low | small | warm | kama-v1-full | 100/100 | 100/100 | örtüşüyor |
| main | 10% | low | small | warm | tcp-std | 100/100 | 100/100 | örtüşüyor |
| main | 10% | med | large | warm | kama-v1-full | 29/30 | 29/30 | örtüşüyor |
| main | 10% | med | large | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 10% | med | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 10% | med | small | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 20% | high | large | warm | kama-v1-full | 25/30 | 21/30 | örtüşüyor |
| main | 20% | high | large | warm | tcp-std | 4/30 | 13/30 | örtüşüyor |
| main | 20% | high | small | warm | kama-v1-full | 29/30 | 30/30 | örtüşüyor |
| main | 20% | high | small | warm | tcp-std | 25/30 | 28/30 | örtüşüyor |
| main | 20% | low | large | warm | kama-v1-full | 25/30 | 27/30 | örtüşüyor |
| main | 20% | low | large | warm | tcp-std | 30/30 | 29/30 | örtüşüyor |
| main | 20% | low | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 20% | low | small | warm | tcp-std | 30/30 | 30/30 | örtüşüyor |
| main | 20% | med | large | warm | kama-v1-full | 19/30 | 23/30 | örtüşüyor |
| main | 20% | med | large | warm | tcp-std | 23/30 | 24/30 | örtüşüyor |
| main | 20% | med | small | warm | kama-v1-full | 30/30 | 30/30 | örtüşüyor |
| main | 20% | med | small | warm | tcp-std | 29/30 | 29/30 | örtüşüyor |
| main | 30% | high | large | warm | kama-v1-full | 27/100 | 23/100 | örtüşüyor |
| main | 30% | high | large | warm | tcp-std | 0/100 | 0/100 | örtüşüyor |
| main | 30% | high | small | warm | kama-v1-full | 28/30 | 24/30 | örtüşüyor |
| main | 30% | high | small | warm | tcp-std | 24/30 | 22/30 | örtüşüyor |
| main | 30% | low | large | warm | kama-v1-full | 25/30 | 24/30 | örtüşüyor |
| main | 30% | low | large | warm | tcp-std | 11/30 | 13/30 | örtüşüyor |
| main | 30% | low | small | warm | kama-v1-full | 30/30 | 29/30 | örtüşüyor |
| main | 30% | low | small | warm | tcp-std | 27/30 | 26/30 | örtüşüyor |
| main | 30% | med | large | warm | kama-v1-full | 52/100 | 53/100 | örtüşüyor |
| main | 30% | med | large | warm | tcp-std | 3/100 | 12/100 | örtüşüyor |
| main | 30% | med | small | warm | kama-v1-full | 29/30 | 30/30 | örtüşüyor |
| main | 30% | med | small | warm | tcp-std | 26/30 | 25/30 | örtüşüyor |
| main | 40% | high | large | warm | kama-v1-full | 0/30 | 0/30 | örtüşüyor |
| main | 40% | high | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 40% | high | small | warm | kama-v1-full | 18/30 | 19/30 | örtüşüyor |
| main | 40% | high | small | warm | tcp-std | 13/30 | 8/30 | örtüşüyor |
| main | 40% | low | large | warm | kama-v1-full | 18/30 | 23/30 | örtüşüyor |
| main | 40% | low | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 40% | low | small | warm | kama-v1-full | 26/30 | 28/30 | örtüşüyor |
| main | 40% | low | small | warm | tcp-std | 18/30 | 19/30 | örtüşüyor |
| main | 40% | med | large | warm | kama-v1-full | 7/30 | 6/30 | örtüşüyor |
| main | 40% | med | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 40% | med | small | warm | kama-v1-full | 24/30 | 23/30 | örtüşüyor |
| main | 40% | med | small | warm | tcp-std | 18/30 | 14/30 | örtüşüyor |
| main | 50% | high | large | warm | kama-v1-full | 0/30 | 0/30 | örtüşüyor |
| main | 50% | high | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 50% | high | small | warm | kama-v1-full | 12/30 | 9/30 | örtüşüyor |
| main | 50% | high | small | warm | tcp-std | 2/30 | 2/30 | örtüşüyor |
| main | 50% | low | large | warm | kama-v1-full | 35/100 | 30/100 | örtüşüyor |
| main | 50% | low | large | warm | tcp-std | 0/100 | 0/100 | örtüşüyor |
| main | 50% | low | small | warm | kama-v1-full | 20/30 | 19/30 | örtüşüyor |
| main | 50% | low | small | warm | tcp-std | 6/30 | 5/30 | örtüşüyor |
| main | 50% | med | large | warm | kama-v1-full | 1/30 | 0/30 | örtüşüyor |
| main | 50% | med | large | warm | tcp-std | 0/30 | 0/30 | örtüşüyor |
| main | 50% | med | small | warm | kama-v1-full | 14/30 | 15/30 | örtüşüyor |
| main | 50% | med | small | warm | tcp-std | 3/30 | 4/30 | örtüşüyor |

Özet: 84 kol×hücre çiftinin 0 tanesinde R1 ve R2 Wilson aralıkları ayrışıyor (beklenen yanlış-pozitif ≈ %5 altı).

## YÜK İŞARETİ DUYARLILIĞI (loadavg1 > 2.0 satırlar DIŞLANARAK ana tablo hükümleri)

İşaretli satır: 69/7000. İşaretliler dışlanınca hükmü DEĞİŞEN ana hücre: 0/30.
