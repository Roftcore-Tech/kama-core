# FAZ 0 — Cleanup + baseline

## 0.1 Non-faithful code REMOVED
Removed the harness `CsProbe` carry + peer-echo path I wrote in the previous turn (feature
`dr-cs-2-ladder`): the `Logical::CsProbe` variant, the `TransmitKind::CsProbe` consume arm, the
dedicated CS forward loss-stream, the echo queue + `Event::CsProbeEcho` feed, the `cs_diag` /
`cs_probe_wire` / `cs_echo_delivered` fields and their record serialization, and the cargo feature.

**Kept** (all proven observation-only): the disaggregated DATA / ARQ / dup / tail / cold-start
scheduler counters (`diag-sched-telemetry`, default OFF), the read-only accessors
(`sched_diag`, `diag_pacer`, `diag_bw_accept`, `tail_probe_due_edge_count`, `burst_cap`), and the
event lineage needed for root cause.

### Factual correction to the previous turn's A1 report
A1 ground (5) said "there is no echo responder on the product receive path". That was **too strong**.
It is true for `adaptive-transport/src/receiver.rs` (0 `CsProbe` refs), but **false** for
`kama-integration/src/receiver_host.rs`, which self-describes as the *"ürün-alıcı COMPOSITION-ROOT"*
and handles the `0x19/0x1A` probe/echo **unconditionally** (`cs_echo_count`, `IngestOutcome::ProbeEchoed`).
The `LADDER-NOT-ISOLATABLE` verdict is **unchanged** — it stands on grounds (1)–(4), and ground (3)
(the feature makes the sender self-generate `Timer::Pto` via `BootstrapPto`, altering the ARQ firing
schedule) is disqualifying on its own. Ground (5) is withdrawn.

## 0.2 Zero-diff — **PASS**
```text
normal binary            vs FROZEN §4.2b records          120/120 scientific fields identical
diag feature build       vs normal binary                 232/232 identical, including
                                                          wire_digest · source_prefix_digest · state_digest
fs_cal2_pins                                              29/29 PASS
```

## 0.3 Contract
Deliberately NOT regenerated: `CONTRACT-STALE-PENDING-FINAL-REGROUND` (see that file). The authoritative
contract is produced once, in FAZ 6.
