# DR-ARQ-PRODUCT-TAIL-1 — Fidelity & Pre-Run Freeze Record

> Single fidelity/freeze record (§8/§11) for the faithful two-netns/netem PRODUCT validation of the bounded
> tail-loss recovery mechanism. Everything below is frozen BEFORE any result-bearing row is executed. No
> adaptive stopping; no outcome-based cell change; no post-hoc performance threshold.

## 1. Source / binary / toolchain
```text
source_commit   : c46e8fa3bf7ffdf999dc2a5b3599bb62e0d6790c   (d1-sprint5c)
  wiring        : 21811df7 (§5.2 sender wiring) → 8c4b0f11 (§7 observation-only probe accounting) → c46e8fa3 (harness)
  CI green at   : 8c4b0f11 (quality/security/aead-differential/miri all success) + c46e8fa3 (this freeze)
binary(debug)   : target/debug/examples/dr_arqtail_product
  sha256        : a1113236e156fa60148e6154aa069c8702ca4e3b55f35c144aa22b3a6b33191f
  build note    : DEBUG build. The mechanism/correctness (OFF stuck, ON bounded probe, inv_fail=0) is codegen-
                  independent; only wall-clock timing differs from release. Correctness validation is the goal.
Cargo.lock sha  : 47893d24b4ea9b80cb1198f653931a09b8a15c8a4767a24375beb4337b046ad4
toolchain       : rustc 1.96.0 (ac68faa20 2026-05-25) (Arch)
```

## 2. Arms / product config (OFF = default product semantics; ON = frozen product config)
```text
OFF (PRODUCT-PTO-OFF): Sender::new() default → TailRecoveryConfig::default() (enabled=false). INV-CS-4 holds
                       byte-for-byte; the Pto core-path emits NO new retransmit. This IS default product behavior.
ON  (PRODUCT-PTO-ON) : set_tail_recovery_config(tail_on_config()) — FROZEN:
                         enabled=true, min_pto=10ms, max_pto=10s, initial_pto=1s (Karn; no-RTT),
                         max_backoff_pto=30s, max_ack_delay=25ms, max_tail_probes=3.
                       Enabled AFTER a warmup phase (fresh counters) — validates a CONVERGED flow.
lineage         : identical binary/config for OFF and ON of a given cell/rep; only the arm flag differs.
```

## 3. Workload / deadline-feasibility (FROZEN; formula UNCHANGED from TDD §3)
```text
workload        : 48 units × 101 B = 4848 B (matches frozen deadline-feasibility). pacer 0.80C, bottleneck 1 Mbit
                  (netem rate 1mbit), src_serialize ≈ 48.5 ms. Offered whole-then-paced (app→transport→pace).
1-tail LB       : serialize + 1.5·RTT   (FORMULA UNCHANGED; re-pinned on typed cells)
RTT cells       : low RTT=50  owd=25  op_deadline=199  lb=124  ext_horizon=699
                  med RTT=250 owd=125 op_deadline=599  lb=424  ext_horizon=2499
                  high RTT=1000 owd=500 op_deadline=2099 lb=1549 ext_horizon=9249
benchmark dl    : 3×LB = driver deadline3xlb (≈370/1270/4645 ms) — labeled a benchmark endpoint, NOT a product SLO.
```

## 4. Structures (core) and netem
```text
delay/RTT       : netem `delay {owd}ms rate 1mbit` on BOTH egress (v0 fwd + v1 rev) → RTT = 2·owd (faithful).
iid             : fwd += ` loss {L}%`  (L∈{2,5,10,20})           — statistical IID wire loss (netem).
burst           : fwd += ` loss gemodel {p}% 50%`, p=50L/(100-L)  — Gilbert-Elliott, steady bad-fraction=L%.
terminal-tail   : NO statistical loss; rx deterministically drops the FIRST arrival of the last {2:1,5:2,10:3,20:4}
                  source units (positional single tail-loss event; probe copy accepted). Matches R2A S1. loss-
                  label maps to a contiguous terminal drop count, NOT a wire loss-rate.
```

## 5. Grid / manifest / seed-rep domain
```text
core cells      : 3 struct × 4 loss × 3 RTT = 36 cells × 2 arms × N=30 = 2160 result-bearing rows.
adversarial     : noloss, reorder, delayedack, probeloss, jitter (10 reps ea.), pathpause, rttstepup,
                  rttstepdown (5 reps ea.) × 2 arms = 130 rows. Explicit netem/dynamic per manifest.
manifest        : scripts/netem/dr_arqtail_manifest.py (single typed source → manifest.csv), 2290 rows total.
  sha256        : bf849aba92158382d9b56b45d50258423e99e09f15329240b6d80c11b56ae1a1
rep domain      : rep index 0..N-1 per cell; OFF rep i paired with ON rep i (pairing unit = rep index). netem
                  loss is stochastic (no fixed PRNG seed exposed by tc); pairing is by rep index, N=30 averages.
NO adaptive stopping; NO outcome-based cell change.
```

## 6. Schema (driver emits one row/run; probe-due ≠ probe-sent ≠ cancelled ≠ token-starved)
```text
run_id,cell,arm,rep,struct,loss,rtt,w_units,unit_bytes,op_horizon_ms,lb_ms,ext_horizon_ms,complete,
t_complete_ms,deadline_hit,deadline3xlb_hit,probe_due,probe_sent,probe_cancelled,token_starved,probe_bytes,
spurious,timer_fires,arq_retx,wire_bytes,cap_exhausted,recovery_cause,inv_fail
  probe_sent  = emitted (cap-consuming). probe_due = eligible. probe_cancelled = ACK-race. token_starved =
  due−sent−cancelled. Metrics from Sender::tail_probe_diag_ext() (observation-only; never affects wire/decisions).
```

## 7. Analysis runner (frozen)
```text
scripts/netem/dr_arqtail_analyze.py  sha256=98d780e9a56206719f47f40ff151544ab6a4280be492a980b73355274393e8cf
  → per-cell paired OFF/ON (CELL-RESULTS.csv, PAIRWISE.csv, FINAL-REPORT.md). Latency CONDITIONAL on completion
    (never presented as unconditional). No silent pooling — cells reported separately; stratum aggregate secondary.
```

## 8. §10 GO / NO-GO correctness rules (frozen; NO-GO is ONLY for correctness/invariant/fidelity)
```text
GO requires ALL: OFF_never_probes · noloss_ON_zero_probe · noloss_ON_zero_added_recovery_bytes ·
  terminal_tail_bounded · terminal_tail_recovery_effect (ON completes ≥ OFF) · probe_loss_retry_cap ·
  no_invariant_failures (inv_fail=0 all rows) · no_unbounded_retransmission · faithful netns preflight PASS ·
  2160/2160 core rows conformant. Correctness passes but tuning/perf mixed ⇒ "GO — MECHANISM CORRECT, POLICY/
  TUNING PENDING". No post-hoc performance threshold invented after outcomes.
```

## 9. Faithful-path fidelity (§4 preflight — PASS, pre-registered gates)
```text
A two distinct netns · B veth placement (v0∈parent,v1∈child) · C connectivity · D netem on BOTH egress +
tc-dump expected config · E RTT+loss on-path (ping = capture-equivalent; 200.36ms/10%) · F PTO probe traverses
impaired path (ON recovers over delayed+lossy path, OFF stuck) · G no-impairment control completes 0-probe ·
H teardown isolation. Fail-closed: netem unverified ⇒ no run; per-row netem retried, else NETEM-SKIP logged
(never a silent drop). Rootless (unshare -rnm + child-netns veth-move); admin/docker NOT required.
```

## 10. Exact commands (frozen)
```text
manifest : python3 scripts/netem/dr_arqtail_manifest.py --out <GRID>/manifest.csv
run      : RUNNER_BIN=target/debug/examples/dr_arqtail_product MANIFEST=<GRID>/manifest.csv \
           RESULT_OUT=<GRID>/result.csv NSHARD=6 bash scripts/netem/dr_arqtail_launch.sh
analysis : python3 scripts/netem/dr_arqtail_analyze.py --in <GRID>/result.csv --outdir <OUT>
preflight: RUNNER_BIN=... PREFLIGHT_OUT=... bash scripts/netem/dr_arqtail_preflight.sh   (must PASS A–H first)
```
