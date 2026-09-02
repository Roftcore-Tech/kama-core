# DR-CS-3 — IMPLEMENTATION REPORT

## RESULT: `DR-CS-3 IMPLEMENTED · SECURITY AND PRODUCT-PATH INTEGRATION PASS · DEFAULT PENDING PANEL VALIDATION`

## What was built

**Sender — shipping default build** (`adaptive-transport/src/sender.rs`)
`BootstrapRttConfig { enabled }` (**default OFF**) + `set_bootstrap_rtt` + `bootstrap_rtt_diag` +
`bootstrap_budget_remaining`. Un-gated the wire-facing surface `TransmitKind::CsProbe` and
`Event::CsProbeEcho` (both codepoints already ship in `kama-transport-schema`; no new codepoint).
State machine: arm at the first PreConverge DATA wire-emission with `srtt.is_none()`; ladder
`BS_FIRES_NS = [3,9,21,45] s` absolute from arm; **edge-only** step advance; one generation
(`probe_seq`) per step; single outstanding slot `(probe_seq, local_send_ns)`; budget
`BS_TX_HARD_BUDGET = 200 B` with `BS_PROBE_WIRE_EXACT = 44 B` (4 × 44 = 176 B worst case);
`drain_bootstrap_probes` runs **last** in `schedule_transmit`; `bs_on_echo` validates and produces the
RTT sample. No new constant was invented — all values are the shipping ones.

**Receiver — authenticated composition root** (`kama-integration/src/receiver_host.rs`)
Per-session **fixed-size** state (`bs_pending_echo: Option<u16>`,
`bs_echoed_generations: [Option<u16>; 4]`, `bs_echo_bytes`) — no growing set/map.
Ingest at `:331` (already **after** `protector.open()` at `:318`, inside the `h_ext` slot) decodes the
authenticated `0x19` via the shipping `decode_control` and records a **typed pending echo action**;
recognition semantics are untouched. `emit_cs_echo(h_ext, probe_seq, wire_pn)` mirrors `emit_recovered`:
burst-enforced monotonic `ControlId` → `Control::CsEcho` → `Session::seal` → sealed wire datagram.
No socket I/O in the parser; the bench `udp_runner` path is never called.

**Additive allowance.** `drain_bootstrap_probes` deliberately does **not** call
`try_send_pre_parity`/`pacer.try_send` and creates **no** pacer debt (the DR-CS-2 failure proved a debit
only reallocates the DATA budget). Bounded by three independent limits: one generation per step, four
steps, 200 B total. Receiver: ≤4 unique generations, ≤172 B (`4 × ECHO_WIRE_MAX`), echo 43 B < probe 44 B
⇒ **amplification ratio < 1 by construction**.

**Legacy supersession.** In an `experimental-cold-start` build the legacy `drain_cs_probes` ladder is a
second implementation of the same protocol. It is now skipped when `BootstrapRttConfig.enabled`, so no
build can ever run both ladders and emit duplicate generations.

## Gates

```text
§8 OFF zero-diff (config OFF vs FAZ-1A baseline)   192/192 identical  PASS
    incl. wire_digest · source_prefix_digest · state_digest · tx/ack lineage · epoch
adaptive-transport lib (default build)             145/145            PASS
adaptive-transport lib (+experimental-cold-start)  144/144            PASS
kama-integration (all tests, +cold-start)          all suites green   PASS
fs_cal2_pins (FAZ-1A rate-ramp + supersession)      33/33             PASS
scoped clippy -D warnings                          clean              PASS
```

## Security / amplification / budget (sender, 10 tests)
`bootstrap_off_is_inert` · `bootstrap_ladder_is_edge_only_and_bounded` ·
`bootstrap_sender_budget_never_exceeds_200_bytes` · `bootstrap_probe_is_additive_not_paced` ·
`bootstrap_unsolicited_echo_yields_no_rtt` · `bootstrap_wrong_generation_echo_yields_no_rtt` ·
`bootstrap_valid_echo_yields_rtt_from_local_timestamps_only` ·
`bootstrap_duplicate_echo_yields_at_most_one_sample` · `bootstrap_no_probe_after_success` ·
`bootstrap_does_not_relax_karn_or_touch_arq_timing`

## Product-path integration (6 tests) — **AUTHENTICATED TRANSPORT INTEGRATION WITH TEST-INJECTED SESSION KEYS**
`crates/kama-integration/tests/dr_cs3_bootstrap_rtt.rs`

`authenticated_bootstrap_rtt_unlocks_preconverge` runs §7 scenarios 1-9 on the real path:
```text
DATA forward-dropped, srtt = None, state = PreConverge
 → product sender emits 0x19 (probe_seq g)
 → real AEAD seal → ReceiverHost::on_datagram ⇒ ProbeEchoed  (verify-before-admission)
 → typed pending action, correlation == g
 → emit_cs_echo ⇒ sealed 0x1A, len ≤ probe len            (amplification asserted)
 → sender opens + decode_control ⇒ CsEcho{probe_seq: g, seen_count: 0, arrival_gap: 0}
 → Event::CsProbeEcho ingest ⇒ rtt_sample_ns == 1 s = (local arrival − local send)
 → srtt: None → Some                                       (PreConverge exit gate satisfied)
 → sender LEAVES PreConverge; FAZ-1A startup pacing takes over
 → ladder cancelled: no probe after success
```
Plus `lost_echo_advances_to_the_next_ladder_step`, `all_probes_lost_keeps_budget_and_gives_up`,
`receiver_rejects_unauthenticated_and_unknown_session` (corrupted ciphertext ⇒
`Dropped("aead verify failed")` **and no echo action**; unknown session ⇒ no action, emit refused),
`receiver_echo_is_bounded_by_generation_and_budget` (4 unique echoes; 5th refused; replay refused;
budget 172 B; each echo ≤ its probe), `receiver_recognition_semantics_are_unchanged`
(50 probes still counted; ≤4 responses ever prepared).

## Scope limits (binding, recorded — not worked around)
1. **Test-injected session keys.** `crates/crypto` is a 3-line skeleton; no Noise handshake exists.
   This is **not** a full product end-to-end, Noise-handshake, or key-establishment claim. Everything
   after key installation is the real product path. Key establishment stays Sprint-6 debt.
2. **`receiver_host` is itself `#[cfg(feature = "experimental-cold-start")]`** (`lib.rs:14-18`), so the
   receiver composition root is not in the shipped default build today; the **sender** emitter is.
   Un-gating `receiver_host` is a separate decision and was **not** taken.

## Product state
```text
BootstrapRttConfig      default OFF   ← pending locked/fresh panel validation (§9)
StartupPacingConfig     default ON    (FAZ-1A, untouched)
TailRecoveryConfig      default OFF   (untouched)
FEC policy              default OFF   (untouched)
product wire behaviour  UNCHANGED while the config is OFF (192/192 zero-diff)
```
No locked panel, no fresh P0/P1, no tail-v2, no estimator wiring, no re-grounding, no FS-CAL YETKİ-2.
