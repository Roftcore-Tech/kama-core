# SUPERSESSION (K4G-05b DUR-44) — the K3 ARQ-share reserve removes the startup-pacing-OFF spurious retransmit

Written and hashed **BEFORE** the test edit. It supersedes only the "startup pacing **OFF** → `> 0` → passes"
row and Discipline item 2 of `SPURIOUS-RETRANSMIT-SUPERSESSION.md` (sha256 `7718b94f…`).
That artifact stays byte-identical. It is a dated measurement record, and its hash is cited by
`fs_f0.rs` and `fs_fec_arms.rs`. Owner decision pending: K4G-05b design note, Ek B.1d, DUR-44.

## Measured

Measurements by the orchestrator, logs in `~/kama-k4g-work/k4g05b/kod-run/r10/`.

**Bisect** of `kama-integration/tests/fs_fec_arms.rs::f0_cost_split_intervention_vs_total_outcome`:

- passes at `35a02fcf` and `9e0ea164`;
- fails on `assert!(r.spurious_retransmits > 0)` from `bc913d49` onward (K4G-05b rev6: B′ rights + K3 ARQ-share
  reserve + DUR-40), through every later commit up to `20a92477`. The DUR-41 chain is unrelated.

**Raw values.** These come from a temporary diagnostic test that was not committed:
`run_f0_paced(2, &[2, 5], 1_000_000, 20 ms, Some(false))`.

| code | retx | completion | t_arq_wire | t_arq_arrival | t_repair_usable | spurious |
|---|---|---|---|---|---|---|
| `9e0ea164` | 1 | 136 ms | 124 ms | 144.808 ms | 136.0 ms | 1 |
| `bc913d49` = `20a92477` | 1 | 136 ms | **115 ms** | **135.808 ms** | 136.0 ms | **0** |

Values common to both rows:

- `parity_emitted` = 2;
- `realized_sealed_wire_bytes` = 202;
- `arq_retransmit_bytes` = 101;
- `fec_decoded_sources` = 2.

Under the product default (startup pacing ON), both codes give retx 0 and completion 67 ms. That path is
unchanged.

## Mechanism

Status: [code reading]. The confirming mutation is pending.

The mechanism is the K3 ARQ-share reserve (CEVAP PAKETİ 15 v3 §2.3), in
`sender.rs::plan_arq_retransmits(…, share = true)`.

1. At an ARQ-share turn, the ARQ head may not reach its tokens. If no reserve exists, it takes the reserve.
2. While the reserve stands, DATA cannot take tokens below it.
3. The retransmit therefore gets the next 101 B slot, ahead of the last original DATA unit.

In this fixture the two emissions swap slots: the retransmit moves 124 → 115 ms and the last original
moves 115 → 124 ms. At 136 ms the receiver decodes with 5 originals, the retransmit and 2 parity. The codec
recovers the other dropped source and the still-delayed original, so `fec_decoded_sources` stays 2.

The retransmit is now one of the K symbols used for decoding. It is useful, not spurious, so the fixture's
classifier (`t_repair_usable ≤ t_arq_arrival`) correctly returns 0.

Before the first ARQ emission, B′ rights and DUR-40 are inactive. No copy or ARQ emission precedes it, so no
right exists, and no copy record is due: §3.3-B copies are due after ≈ 8 s.

## Why this is a supersession and not a regression

Counts, bytes and completion time are identical, and the retransmit costs the same 101 B. Only its role
changed: it went from wasted to used.

The old line `spurious_retransmits > 0` documented the *presence of a cost channel*, not a safety property
(see the superseded artifact). The channel still exists under an ordering where DATA wins the slot, but this
fixture no longer realizes it.

## Discipline

1. This artifact is hashed before the test edit. The old artifact is untouched.
2. The test change is DUR-44 option (a), and the owner decision is pending.
   - `spurious_retransmits > 0` is no longer asserted.
   - Kept unchanged: `fec_decoded_sources == 2`, `realized_sealed_wire_bytes == 2 × fs_sealed_len(64)`,
     `arq_retransmit_bytes == retransmits × sealed` and `spurious ≤ retransmits`.
   - Added: `retransmits ≥ 1` (the ARQ arm is exercised), `delivered == 8` and completion present.
   - The spurious count and the sign of `t_arq_arrival − t_repair_usable` are printed, not asserted. Old
     ordering: +8.808 ms, spurious 1. K3 ordering: −0.192 ms, spurious 0.
3. The companion product-default test `f0_cost_split_under_product_default` is unchanged.

## Result

`K3-ARQ-SHARE-RESERVE-REMOVES-PACING-OFF-SPURIOUS-LEAK · COST-SPLIT-ASSERTIONS-PRESERVED · SPURIOUS-PRINTED-NOT-ASSERTED (DUR-44 (a), owner decision pending)`
