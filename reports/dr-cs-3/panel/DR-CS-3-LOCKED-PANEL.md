# DR-CS-3 — §4 LOCKED REGRESSION PANEL

**Verdict: `DR-CS-3 LOCKED PANEL FAIL` — efficacy gate 2/3, every safety gate PASS.**

Scope label: `DEFAULT-BUILD AUTHENTICATED TRANSPORT COMPONENT PANEL WITH TEST-INJECTED SESSION KEYS`.
Does not verify the Noise handshake, a product binary, or a deployment network path.

Runner `examples/dr_cs3_panel_runner locked` · release binary sha256
`bd4c8169441d3395d6c121ca9b26e3ee3757a669294e0af6337febdbf7c9174e` · 32 runs
(2 panels × 8 records × {baseline, treatment}) · every record `fit_use=FORBIDDEN`,
`result_bearing=false`.

---

## §2 harness conformance

`fs_cal2_spine.rs` was **not** modified and **not** converted into a product-path harness. A separate
harness (`tests/common/dr_cs3_panel.rs`) drives: real product `Sender` · real default-build
`ReceiverControlCore` · real `kama-transport-schema` control codec · real AEAD seal/open with
test-injected **directional** keys · deterministic forward/reverse loss · real probe/echo ingest ·
real RTT estimator · real PreConverge exit · real startup-pacing handoff.

Seed and loss semantics are reproduced from the frozen records verbatim: `seed_domain`
`FSFC2-DIAG42B`, `cell_id` `P2-GE20L10-RTT1000`, reps `{5,9,14,16,17,25,38,39}`, RTT 1000 ms,
GE `p=200000 ppm, L=10`, warm-up 384 × 101 B, timeout 65 s, and the FS-CAL-2 §5.3 generator
(`stream_key` + `loss_bits`) re-used unchanged. The bootstrap path draws from two **additional,
disjoint** stream keys (`DRCS3ProbeForward`, `DRCS3EchoReverse`) so the three existing stream
classes are bit-for-bit untouched. **No new seeds were selected.**

### Harness validity: all 16 baselines reproduce the frozen records EXACTLY

| record | frozen `preconverge_exit_ns` | panel baseline | tx_data | ack_batches | delivered | `source_prefix_digest` |
|---|---|---|---|---|---|---|
| P0/rep5 | 27065330000 | 27065330000 | 384/384 | 381/381 | 384/384 | identical |
| P0/rep9 | null | null | 20/20 | 0/0 | 0/0 | identical |
| P0/rep14 | null | null | 20/20 | 0/0 | 0/0 | identical |
| P0/rep16 | 23807270000 | 23807270000 | 384/384 | 382/382 | 384/384 | identical |
| P0/rep17 | null | null | 20/20 | 0/0 | 0/0 | identical |
| P0/rep25 | 20549200000 | 20549200000 | 384/384 | 366/366 | 366/366 | identical |
| P0/rep38 | 23807270000 | 23807270000 | 384/384 | 368/368 | 384/384 | identical |
| P0/rep39 | 62904040000 | 62904040000 | 106/106 | 54/54 | 78/78 | identical |
| P1/rep5…rep39 | — | — | — | — | — | all 8 identical |

**16/16 exact, including `source_prefix_digest`.** The separate harness is a faithful reproduction of
the locked loss schedule and seed semantics.

---

## Outcome definition (stated before the results)

* **`cold_start_stall`** := `preconverge_exit_ns is None` — the sender never left `PreConverge`
  within the 65 s observation window. This is the DR-CS-3 target class.
* `warmup_complete` (all 384 drained) is a **separate, harder** criterion that the bootstrap RTT
  probe does not claim to fix; it is reported but is not the gate.

Under this definition the frozen classification is confirmed: cold-start stalls = **rep9, rep14,
rep17**; the remaining 5 (rep5, rep16, rep25, rep38, rep39) left `PreConverge` and are the
"baseline success" set. rep25 and rep39 are TIMEOUT-but-not-stalled (they escaped PreConverge at
20.5 s and 62.9 s respectively and did not fully drain).

---

## Results — P0 (tail OFF) and P1 (tail ON) are identical

| rep | baseline | treatment | outcome |
|---|---|---|---|
| 5 | 27 065 ms | **4 000 ms** | improved 6.8× |
| 9 | **STALL** | **4 000 ms** | **RECOVERED** |
| 14 | **STALL** | **STALL** | **NOT RECOVERED** |
| 16 | 23 807 ms | 23 807 ms | unchanged (all probes forward-dropped) |
| 17 | **STALL** | **4 000 ms** | **RECOVERED** |
| 25 | 20 549 ms | 20 549 ms | unchanged (flow self-recovered before an echo landed) |
| 38 | 23 807 ms | **4 000 ms** | improved 6.0× |
| 39 | 62 904 ms | **4 000 ms** | improved 15.7× |

## §4 acceptance

| Gate | Required | P0 | P1 | Result |
|---|---|---|---|---|
| Baseline stalls recovered | **3/3** | **2/3** | **2/3** | **FAIL** |
| Baseline success → no new stall | 5/5 | 5/5 | 5/5 | PASS |
| rep39 not stalled under treatment | yes | not stalled (62.9 s → 4.0 s) | same | PASS |
| Sender budget violation | 0 | 0 (max 176 B ≤ 200 B) | 0 | PASS |
| Receiver budget violation | 0 | 0 (max 172 B = budget, never exceeded) | 0 | PASS |
| Authentication/security failure | 0 | 0 | 0 | PASS |
| DATA displacement | 0 | 0 | 0 | PASS |
| ARQ timer diff (pre-unlock) | 0 | 0 | 0 | PASS |
| Duplicate RTT sample | 0 | 0 | 0 | PASS |
| New stall | 0 | 0 | 0 | PASS |
| OFF arm emits nothing | 0 | 0 probes, 0 echo bytes | 0 | PASS |

**Exactly one gate fails, and it is the primary efficacy gate.**

---

## EXACT FAILURE LINEAGE — rep14

The mechanism executed **completely correctly**; the reverse channel defeated it.

```
t=0             bootstrap_edge armed
t=3.000 s       probe_emission seq=0
t=3.500360 s    authenticated_receiver_arrival seq=0
t=3.500360 s    echo_generation seq=0 wire=43B      → reverse drop, never emitted
t=9.000 s       probe_emission seq=1
t=9.500360 s    authenticated_receiver_arrival seq=1
t=9.500360 s    echo_generation seq=1 wire=43B      → reverse drop, never emitted
t=21.000 s      probe_emission seq=2
t=21.500360 s   authenticated_receiver_arrival seq=2
t=21.500360 s   echo_generation seq=2 wire=43B      → reverse drop, never emitted
t=45.000 s      probe_emission seq=3
t=45.500360 s   authenticated_receiver_arrival seq=3
t=45.500360 s   echo_generation seq=3 wire=43B      → reverse drop, never emitted
                ladder exhausted (4/4) → no RTT sample → srtt stays None → PreConverge never exits
```

Counters: `probes_emitted=4` · `probe_wire_bytes=176` (≤200) · `probes_authenticated=4` ·
`echoes_generated=4` · `echo_wire_bytes=172` (= the exact 172 B budget) · `echoes_arrived=0` ·
`echo_match_valid=0` · `rtt_sample_ns=0` · all violation counters **0**.

**Failing step: `echo_generation → sealed_echo_emission`.** The deterministic reverse stream
`FSFC2-DIAG42B|P2-GE20L10-RTT1000|rep14|DRCS3EchoReverse` yields drop bits `[D, D, D, D]`: the GE
chain starts in the Bad state and its mean bad-run length (L=10) exceeds the 4-step ladder, so every
generation of the ladder is consumed inside one burst.

For contrast, the same computation over all eight records:

| rep | probe forward drops | echo reverse drops | consequence |
|---|---|---|---|
| 5, 9, 17, 38, 39 | `. . . .` | `. . . .` | first round trip succeeds at t≈4 s |
| 14 | `. . . .` | **`D D D D`** | **ladder exhausted** |
| 16 | `D D D D` | `. . . .` | no probe ever arrives (baseline was not a stall) |
| 25 | `. . . .` | `D D . .` | flow self-recovered at 20.5 s first |

---

## Recorded harness limitations (NOT used to overturn the verdict)

Both were fixed by design **before** the panel ran and are **not** being changed now — §0 forbids
altering the model after seeing the outcome. They are recorded so the decision can be taken
pre-outcome, by Abkarada, before any re-run.

1. **Consecutive-draw correlation for temporally distant probes.** The four echoes draw from four
   *consecutive indices* of one GE chain, which forces maximal burst correlation between events that
   are actually 6 s, 12 s and 24 s apart in virtual time. GE bad-run length is defined in *packets*
   (L=10), so consecutive-index draws are the worst-case correlation assumption for a ladder spread
   over 45 s.
2. **Reverse-path asymmetry.** The DATA path's reverse (ACK) direction is modelled as **lossless** —
   inherited verbatim from the frozen FS-CAL-2 channel so the baselines reproduce exactly — while the
   bootstrap echo's reverse direction is modelled as **GE-lossy**. The treatment therefore faces a
   reverse-path impairment the baseline never faces.

Neither limitation changes the fact recorded above: under the pre-registered model, one of the three
frozen stall records is not recovered.

---

## Consequence

Per §4 (*"Locked panel düşerse fresh panel çalıştırma. İkinci treatment eklemeden DUR."*) and §8 FAIL:

* the §5 fresh P0/P1 panels were **not run**,
* the §6 control panels were **not run**,
* the §7 targeted two-netns validation was **not run**,
* `BootstrapRttConfig::default()` stays **OFF**,
* no ladder timing, budget, retry, timer, PreConverge floor, seed or threshold was changed,
* no second treatment was added.
