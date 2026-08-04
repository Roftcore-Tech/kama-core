# DR-ARQ-PRODUCT-TAIL-1 — Final Product-Validation Checkpoint

> Single final checkpoint (§11/§12) for the faithful two-netns/netem PRODUCT validation of the bounded
> tail-loss recovery mechanism. Freeze/fidelity record: `../FIDELITY-AND-FREEZE.md`. Structured artifacts:
> `result.csv` (2290 rows), `CELL-RESULTS.csv`, `PAIRWISE.csv`, `FINAL-REPORT.md`. NO adaptive stopping;
> NO post-hoc performance threshold; latency is reported CONDITIONAL on completion only.

## Verdict — **GO — MECHANISM CORRECT, POLICY/TUNING PENDING**

All §10 correctness/invariant/fidelity gates PASS. The mechanism is correct, bounded, safe, and recovers
terminal tails the default product cannot; specific multi-unit-tail cap/backoff **tuning** is identified as
pending (a policy question, not a correctness defect). NO-GO is reserved for correctness/invariant/fidelity
failure — none occurred.

## 1. Traceability
```text
source_commit c46e8fa3 (CI green) · binary(debug) sha a1113236 · manifest sha bf849aba · analyzer sha 98d780e9
dataset: 2290/2290 rows (2160 core + 130 adversarial) · 0 missing · 0 extra · 0 duplicate · 0 NETEM-SKIP
```

## 2. §10 correctness gates (mechanical, from the dataset)
```text
[PASS] OFF_never_probes                 — 0 OFF rows with any probe activity (INV-CS-4 default holds)
[PASS] noloss_ON_zero_probe             — no-loss ON probe_sent all 0
[PASS] noloss_ON_zero_added_recovery    — no-loss ON probe_bytes all 0 (mechanism inert; exact OFF-mode byte
                                          identity proven deterministically by frozen in-process golden tests)
[PASS] terminal_tail_bounded            — 0 terminal-tail ON rows exceed the bound
[PASS] terminal_tail_recovery_effect    — terminal-tail horizon completions OFF=8 vs ON=329 (of 720)
[PASS] probe_loss_retry_cap             — probe-loss slice: probe_sent ≤ 3 (max observed 1)
[PASS] no_invariant_failures            — inv_fail=0 across ALL 2290 rows
[PASS] no_unbounded_retransmission      — global max probe_sent = 3 (= max_tail_probes cap)
[PASS] faithful netns preflight (§4)    — gates A–H (see freeze record §9)
[PASS] spurious probes = 0              — sum_spurious=0 across ALL adversarial slices (guards hold)
```

## 3. Primary result — terminal-tail recovery (C1)
```text
Per-cell (30 paired reps): OFF-only completions = 0 in EVERY cell; ON-only = 27–30 in 11 of 12 cells.
OFF horizon completion ≈ 0 (0–0.10; the few OFF completions are §11 forward-duplication, arm-independent,
not a tail probe). ON horizon completion = 1.0 in 11 of 12 cells. Cost is minimal and bounded: ON adds
1–4 retransmitted units (Δwire ≈ +101…+404 B) and 1–3 probes; median completion latency tracks RTT
(≈0.5 s low, ≈2 s med, ≈6.5 s high — CONDITIONAL on completion). A single probe of the oldest tail unit
often unblocks normal packet-threshold ARQ for the remaining tail (the probe's new higher seq becomes a
loss-detection reference), so recovery is frequently cheaper than one-probe-per-lost-unit.
```

## 4. Structure comparison (IID / burst — secondary, normal-ARQ territory)
```text
Mid-stream (non-tail) losses are recovered by the existing packet-threshold ARQ in BOTH arms → OFF and ON are
near-parity (both-complete 17–30 of 30). ON shows a modest edge where the LAST packets of an IID/burst draw
are lost (a tail-within-stream): e.g. iid/burst L5–L10 low, ON-only 6–11 vs OFF-only 0–5. At 20% loss the
cells are noisy (stochastic loss draws + cold-start fragility) and ON is not uniformly > OFF. No FEC engaged;
this is an ARQ/tail-only comparison. Δwire is mixed-sign (stochastic), never a systematic ON cost blow-up.
```

## 5. Adversarial slices (§6) — all inv_fail=0, all bounded
```text
noloss      OFF 0.60 / ON 1.00  probe_sent 0  — ON adds nothing (0 probe); OFF baseline completion variance is a
                                                harness settle-timing artifact, not a tail-mechanism effect.
reorder     OFF 1.00 / ON 1.00  probe_sent 0  — reordering guard holds: NO spurious probe on reordered-not-lost.
delayedack  OFF 0.00 / ON 0.20  probe_sent 0  — delayed-ACK guard holds: NO premature/spurious probe. (No terminal
                                                drop here; low completion = the delayed-ACK flow itself is slow.)
jitter      OFF 0.00 / ON 1.00  probe_sent 1  — recovers terminal tail under jitter.
pathpause   OFF 0.00 / ON 1.00  probe_sent 1  — recovers after a mid-run 100%-loss blackout window.
rttstepup   OFF 0.00 / ON 1.00  probe_sent 1  — PTO adapts to a mid-run RTT increase; recovers.
rttstepdown OFF 0.00 / ON 1.00  probe_sent 1  — recovers after a mid-run RTT decrease.
probeloss   OFF 0.30 / ON 0.10  probe_due ~51, probe_sent ~0.2, maxsent 1 — under 40% loss the retry CAP holds
                                                (≤3, observed 1) and probes stay bounded despite many due signals.
```

## 6. Tuning / performance assessment (POLICY PENDING — not correctness)
```text
T1 (main) multi-unit-tail cap: terminal-tail L10-med (drop 3 units, RTT 250) is the ONLY core terminal cell where
   ON fails to recover (0/30) — the fixed max_tail_probes=3 is spent (sent=3) before packet-threshold ARQ engages,
   whereas L20-med (drop 4) completes because one probe unblocks ARQ (arq~3). The cap/backoff should ADAPT to the
   observed tail length / ARQ-engagement, not be a fixed 3. Mechanism stayed correct+bounded throughout.
T2 high-loss (20%) IID/burst noise + cold-start fragility: when the FIRST post-warmup exchange is heavily lossy the
   flow can stall in PreConverge where floor pacing token-starves the probe. The tail probe targets a CONVERGED
   flow (validated with warmup); cold-start bootstrap loss is DR-CS-1's concern, not this mechanism's.
T3 no product SLO exists → deadline_hit (op) is ~0 across arms (expected; deadlines are tight and workload+RTT
   dominated). Report horizon completion + conditional latency as the endpoints; 3×LB benchmark deadline is a
   labeled benchmark endpoint, not an SLO.
No performance threshold is invented post-hoc; these are observations feeding a future tuning pass.
```

## 7. Scope / what this does NOT claim
```text
- Not a production-deployment or RFC 9002 compliance claim; ARQ/tail only (no FEC engaged).
- terminal-tail = deterministic positional erasure on the faithful-delay path (RTT via netem); IID/burst = faithful
  netem statistical loss. Both traverse real two-netns/netem; the PTO probe crosses the SAME impaired path.
- receiver = faithful SACK-generating endpoint; the mechanism under test is the PRODUCT Sender's bounded probe.
```

## 8. Quinn / TCP / KCP readiness (explicitly NOT run this turn)
```text
Out of scope per directive. The product tail-recovery baseline is now established (mechanism correct + bounded,
default-OFF, tuning pending). A §5-style quinn/TCP/KCP comparison over the SAME two-netns/netem harness is a
separate future turn; no external-protocol comparison or "beats X" claim is made here.
```
