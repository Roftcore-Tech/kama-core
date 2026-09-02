# FS-1-TAIL-STRATA-v1.3.2

Tail = REAL_TAIL_ERASURE (terminal source-id 47 erased on initial transmission; ARQ-unrecoverable). Reported separately from non-tail. FEC completion under tail-loss demonstrates coverage of an erasure class the current ARQ (no RTO/TLP) may miss — NOT automatic product-wide/future-tail-ARQ/FEC-specific superiority.

| cell | arm | tail_rows | complete | censored |
|---|---|---|---|---|
| rtt1000-exactd4 | A0 | 1 | 4 | 1 |
| rtt1000-exactd4 | A1-EARLY | 1 | 4 | 1 |
| rtt1000-exactd4 | F0 | 1 | 4 | 1 |
| rtt1000-exactd4 | F3 | 1 | 4 | 1 |
| rtt250-exactd1 | A0 | 1 | 4 | 1 |
| rtt250-exactd1 | A1-EARLY | 1 | 4 | 1 |
| rtt250-exactd1 | F0 | 1 | 5 | 0 |
| rtt250-exactd1 | F3 | 1 | 5 | 0 |
| rtt250-exactd2 | A0 | 1 | 4 | 1 |
| rtt250-exactd2 | A1-EARLY | 1 | 4 | 1 |
| rtt250-exactd2 | F0 | 1 | 5 | 0 |
| rtt250-exactd2 | F3 | 1 | 5 | 0 |
| rtt250-exactd4 | A0 | 1 | 4 | 1 |
| rtt250-exactd4 | A1-EARLY | 1 | 4 | 1 |
| rtt250-exactd4 | F0 | 1 | 4 | 1 |
| rtt250-exactd4 | F3 | 1 | 4 | 1 |
| rtt50-exactd4 | A0 | 1 | 4 | 1 |
| rtt50-exactd4 | A1-EARLY | 1 | 4 | 1 |
| rtt50-exactd4 | F0 | 1 | 4 | 1 |
| rtt50-exactd4 | F3 | 1 | 4 | 1 |
