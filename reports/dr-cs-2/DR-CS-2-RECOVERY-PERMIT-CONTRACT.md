# DR-CS-2 — BOUNDED RECOVERY PERMIT CONTRACT

> Frozen BEFORE any code change and before any treatment outcome was seen.
> Preflight verdict: **Durum A** — recovery starvation is present in **P0 (shipped product baseline,
> tail recovery OFF)**, so the treatment targets the shipped cold-start path. P1 (tail-ON, the final
> FS-CAL ARQ baseline) is the regression panel.

## 1. Which recovery classes may use a permit?

**Exactly one: the §3.3-B geometric source copy (`DupRec` in `Sender::dup_queue`)**, and only while
`state == SenderState::PreConverge`.

Grounding: `sender.rs drain_new_data` — every DATA that reaches the wire while in `PreConverge` pushes a
`DupRec { due_ns = now + k_base×RTT_floor, unit, interval_ns }`. This is the ONLY recovery class that can
become due during a PreConverge stall: packet-threshold loss declaration is gated off
(`sender.rs` `on_ack_received`: `state != PreConverge || srtt.is_some()`), time-threshold PTO loss is gated
off (`on_timer`: `state != PreConverge`), so `pending_lost` stays empty and the ARQ planner has no work;
the tail probe is absent in the shipped tail-OFF default. Preflight confirms this at runtime:
P0 `dup_selected = 30,113,898`, `dup_token_denied = 30,113,896`, `dup_wire_emitted = 2`.

**NOT eligible:** new DATA · FEC/parity · duplicate-of-parity · CsProbe or any control packet ·
tail probe (it has its own bounded cap) · any post-PreConverge class.

## 2. Which authoritative event mints a unique permit?

The **first time a specific `DupRec` becomes due AND is refused by the normal pacer**. Each `DupRec` is
created exactly once per PreConverge DATA wire-emission and carries a monotonic `id` (generation).
The permit is minted per `DupRec.id`, never per scheduler tick and never per level-triggered poll.

## 3. What is the permit bound to?
`DupRec.id`. A permit minted for id *i* can only ever be consumed by the `DupRec` with id *i*.

## 4. When is it armed?
Inside `drain_due_duplicates`, only when ALL hold: `state == PreConverge` · the entry is due
(`due_ns <= now`) · `try_send_pre_parity` returned **false** · no permit is currently outstanding ·
this id has not previously been granted a permit · the session budget is not exhausted.

## 5. Where is normal pacer refusal verified?
At the existing `try_send_pre_parity(size, is_retx = true)` call site in `drain_due_duplicates`. The
permit is only reachable on the `false` branch — the normal pacer always gets first refusal.

## 6. When is it consumed?
Immediately, by emitting exactly **one** `TransmitKind::Duplicate` datagram for that `DupRec`, in the
same call. Arming and consumption are inseparable; a permit is never carried across events.

## 7. When does ACK / largest-acked progress cancel it?
Any ACK that removes an inflight unit (real ACK progress) clears the outstanding-permit slot and the
granted-id memory. This is the same progress signal the tail recovery already uses.

## 8. What if the work item becomes invalid?
If the `DupRec` is removed (its unit was ACKed → `dup_queue.retain`, or the recovery epoch changed), the
permit slot is cleared without emission. No permit survives its work item.

## 9. Why can a repeated level-triggered `due` state not mint a second permit?
Because the mint is keyed on `DupRec.id` and each id is recorded in a bounded granted-set. A `DupRec`
that stays due for millions of polls mints at most one permit. This is the explicit defence against the
`probe_due`-poll-inflation error class.

## 10. Byte / wire / congestion / pacing accounting
The credited datagram is emitted through the **normal** `track_sent` + `TransmitDecision` path, so wire
bytes, seq, inflight and retransmission accounting are unchanged and complete. The pacing cost is not
waived: the permit performs a **bounded pacing debit** — `TokenBucket::debit(size)` records the bytes as
debt, and every subsequent `refill` repays the debt **before** granting new tokens. The sender therefore
pays for the credited datagram out of future pacing allowance.

## 11. How is the added emission bounded?
Three independent bounds: (a) at most **one** permit outstanding at any instant; (b) at most one permit
per `DupRec.id`; (c) a session budget `MAX_RECOVERY_PERMITS` derived from the existing authoritative
recovery limit (`TailRecoveryConfig::default().max_tail_probes`), not a new invented number. Worst case
added wire = `MAX_RECOVERY_PERMITS × unit_size`, fully debited from future pacing.

## 12. How are DATA and parity structurally prevented from using it?
The permit is only reachable from inside `drain_due_duplicates`, which handles `dup_queue` entries only.
`drain_new_data` (DATA) and `plan_fec_parity` (parity) have no access to the permit state — there is no
call path from them to the mint. Pinned by `data_and_parity_can_never_use_a_recovery_permit`.

## Frozen predicates (unchanged): `STALL(65s)`, `ZERO_TX_STALL`; plus `MECHANISM-SUCCESS`,
`END-TO-END-RECOVERY-SUCCESS`, `SAFETY-PASS` per §7 of the directive.
