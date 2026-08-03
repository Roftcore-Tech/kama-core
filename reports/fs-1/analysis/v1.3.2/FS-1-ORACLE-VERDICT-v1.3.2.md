# FS-1-ORACLE-VERDICT-v1.3.2 (frozen oracle-ordering + C-P4 fairness; per-cell; screening)

> O0 no-redundancy · O1 redundancy-value · O2 FEC-specific · O3 insufficient. FEC>A0 alone ≠ FEC-specific; FEC-specific requires FEC>A1 at matched realized wire.

| cell | A1>A0 | F0>A0 | F3>A0 | F0>A1 | F3>A1 | F0-vs-A1 fairness | oracle |
|---|---|---|---|---|---|---|---|
| rtt1000-exactd1 | true | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt1000-exactd2 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt1000-exactd4 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt1000-iid20 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt250-exactd1 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt250-exactd2 | true | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt250-exactd4 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt250-iid20 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt50-exactd1 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt50-exactd2 | true | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt50-exactd4 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |
| rtt50-iid20 | false | true | true | true | true | NON_DOMINATED | O2_FEC_SPECIFIC |

**Oracle class distribution (12 cells):** O0=0 O1=0 O2=12 O3=0

Note: F3 vs A1 is BUDGET_MISMATCH (2424B vs 1212B) → coding-causal attribution PROHIBITED (C-P4); any F3>A1 is not FEC-specific coding gain. F3 vs F0 reported in pairwise CSV.
