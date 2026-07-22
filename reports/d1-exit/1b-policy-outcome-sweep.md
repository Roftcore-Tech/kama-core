# Z2 — Reliability-Separating Deterministic Sweep [policy-seçimi-YAPMAZ]

> Harness: `policy_outcome_sweep.rs` (Z1: paired-trace-kanal + blok-causal-ledger + PRE-DECLARED
> T_op=max(10s,8×2×d_ms) + deficit-sınıfı-arm-invariant-pinli). 13-hücre × 7-ana-arm (+F10/F20+T_floor-
> sens) × 10/30-paired-seed = 1231-koşu / 64k-blok-satırı (8s-deterministik). Veri:
> [data/policy-outcome/](data/policy-outcome/) (ledger.gz + run-level + 4-analiz-CSV'si).
> Yeniden-üretim: `POS_OUT=<dir> cargo test --release -p adaptive-transport --features
> experimental-fec-budget --test policy_outcome_sweep policy_outcome_sweep -- --ignored` +
> `scripts/pos_analyze.py`. Utility-sıfır-bölme='NA'-explicit. Kernel/canlı-sweep-havuzlarıyla-KARIŞTIRILMAZ.

## Reliability-ayrışması SAĞLANDI
Operational-deadline (dl_op) doygunluğu-kırıldı: rev99 L=0.870 · blackout'lar 0.96-0.98 · diğerleri≈1.0
(extended-completion-hepsi≈1.0 — ayrım-operational-katmanda; censoring-raporlu).

## ANA-TABLO (r4-ailesi; dl_op / FEC-decode / avoided-retx / retx-eff)
| hücre | L | P15 | F15 |
|---|---|---|---|
| rev99 | 0.870 / 8 / 3 / 0.003 | 0.922 / 127 / 84 / 0.049 | **0.938 / 225 / 141 / 0.099** |
| blackout-30s | 0.980 / 0 / 0 / 0 | 0.980 / 50 / 14 / 0.007 | 0.960 / **202 / 119 / 0.090** |
| blackout-10s | 0.980 / 69 / 10 | 0.980 / 118 / 15 | 0.978 / **235 / 73** |
| d1000-ge | 1.0 / 0 | 0.998 / 17 | 1.0 / **113 / 35** |
| manyblock | 1.0 / 0 | 0.995 / 25 | 0.999 / **291 / 155** |
| ge-long | 1.0 / 115 | 1.0 / 146 | 1.0 / **180 / 92** |
| gap80 | 0.998 / 214 | 0.998 / 262 | 0.998 / **292 / 190** |

## Deficit-profil × decode (H1-vs-H2; rev99-örneği; tam-CSV)
deficit-1: L-6/164 · P15-71 · **F15-133** — deficit-2: 1 · 44 · **75** — deficit-3+: 1 · 12 · **17**.
**H1-fairness-DOĞRULANDI; H2-concentration-ÇÜRÜDÜ** (P15-hiçbir-deficit-sınıfında-decode'da-F15'i-geçmedi).

## Dürüst-karşı-sinyaller (gizlenmedi)
blackout-30s: paired F15_harmed=10 (P15_rescued_vs_F15=10; dl_op 0.96-vs-0.98) — karanlık-pencerede-
parity-displacement DATA/ARQ-tail'ini-10-blokta-deadline-dışına-itti (decode-üstünlüğüne-rağmen).
rev99: F15_harmed=3 (=P15_harmed=3; simetrik-gürültü-bandı). manyblock-P15: dl_op-0.995 (5-blok-geç).
Displacement (ndisp/rdisp) her-hücrede-raporlu-bounded; waste=0-tümü.
