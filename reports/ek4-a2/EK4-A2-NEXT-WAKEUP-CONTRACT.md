# EK4-A2 — `next_wakeup_ns()` CONTRACT AND TIMER INVENTORY

Derived from code **before** any code change. Start HEAD `4aa11a93af44b2b9e884461e8db3d1ed576c8b4f`.

---

## §1.2 Inventory

### Definition as it stands

```rust
#[cfg(feature = "experimental-cold-start")]          // ← NOT in the default build
pub fn next_wakeup_ns(&self) -> Option<u64> {
    self.cs_wakeup.iter().map(|&(d, _)| d).min()      // ← ONE source only
}
```

`sender.rs:116` states its scope explicitly: *"exposed-timer türleri (`next_wakeup_ns` sözleşmesine
bağlı küme). Scope: CS-probe/CS-giveup/(d)-PTO … + TEST-ONLY hook"*.

**It is a DR-CS-1 cold-start–scoped accessor, not a general event-loop deadline API**, and it does not
exist at all in the default build.

### Call sites

| Site | Kind |
|---|---|
| `sender.rs:1706` | `debug_assert!` C2 post-condition (no due exposed timer at batch return) |
| `udp_runner.rs:862` | bench runner (`experimental-fec-budget`), not a product path |
| `dr_cs1_acceptance.rs` ×9 (`:60, 80, 154, 170, 307, 347, 402, 449, 529`) | tests, incl. the failing EK4-A2 one at `:347` |

No product runtime consumer exists yet (that is AŞAMA 7).

### Authoritative deadline / progress sources in `Sender`

| # | Source | State | Deadline | Gated | In `next_wakeup_ns()` today |
|---|---|---|---|---|---|
| 1 | CS probe ladder · CS giveup · `BootstrapPto` | `cs_wakeup: Vec<(u64, CsTimerKind)>` | stored | `experimental-cold-start` | **yes (only one)** |
| 2 | ARQ time-threshold loss | `loss.inflight`, `largest_acked`, `have_acked` | `min{send_ns : seq < largest_acked} + max(scale(srtt, time_num/den), max(min_delay, cadence_floor))` (`loss.rs:280-297`) | **ungated** | no |
| 3 | §3.3-B geometric copies | `dup_queue: Vec<DupRec>` | `due_ns` | **ungated** | no |
| 4 | DR-CS-3 bootstrap ladder | `bs_armed_ns`, `bs_next_step` | `bs_armed_ns + BS_FIRES_NS[bs_next_step]` | ungated (config default OFF) | no |
| 5 | Tail-recovery PTO | `tail_recovery.armed_at_ns` | `armed_at_ns + current_pto_ns(srtt, rttvar)` (`tail_recovery.rs:127`) | ungated (config default OFF) | no |
| 6 | **Pacer token refill** | `pacer.tokens/rate/burst_cap/refill_rem` | time until enough tokens exist for the head-of-line pending unit | **ungated** | no |

### Ordering

* **Event ingest** — `cs_type_priority`: `AckReceived` = 0, `CsProbeEcho` = 1, everything else = 2 (stable).
* **Output drain** — `schedule_transmit` (`sender.rs:2806-2852`): ARQ share tick → `drain_new_data`
  → recovery (`plan_arq_retransmits` / `plan_forward_duplication`) → `emit_tail_probe` →
  `drain_due_duplicates` → `plan_fec_parity` → legacy CS probes → `drain_bootstrap_probes`.
* **PTO / retransmission rearm** — each batch re-derives the `(d)`-scope deadline from live loss state
  and re-pushes `CsTimerKind::BootstrapPto`, but **only** when `cs_resolved.is_some()` **and**
  `rtt.srtt().is_some()` **and** `loss.next_time_loss_deadline_ns(..)` is `Some` (`sender.rs:1817-1826`,
  mirrored at `:1872-1891`).

### The exact `None`-producing branch in EK4-A2

`next_wakeup_ns()` = `min(cs_wakeup)`. In the failing run `cs_wakeup` is empty because the only
candidate entry, `BootstrapPto`, is not pushed: `loss.next_time_loss_deadline_ns(..)` returns `None`.

It returns `None` because it maps over `inflight` filtered by `seq < largest_acked`, and the unit in
question has **already been declared lost** — it has moved out of `inflight` into `pending_lost`.

**So the outstanding work at that instant is not a future deadline at all — it is due-now work
(`pending_lost`) that could not be emitted because the pacer had no tokens.** Its next progress
instant is governed by source **#6**, which nothing exposes.

`cs_has_pending_work()` (`sender.rs:1736`, feature-gated) already names exactly this set:
`tx_queue | pending_lost | dup_queue | pending_fec`.

---

## §1.1 Binding contract (to be enforced)

`next_wakeup_ns()` MUST NOT return `None` when, after the documented immediate-output drain, either
timer-driven outstanding work or an internal future deadline exists.

`None` is valid **only** when all of: terminal/closed · genuinely idle · no immediate output ·
no internal deadline · progress depends solely on external network input.

When work is already due, the sender must signal immediate readiness as `Some(now)` — never a hidden
"poll me again" `None`.

**No busy loop may be constructible:** `Some(now)` is only returned while due work actually exists,
and it must be cleared once that work has been consumed, so a driver that drains then re-queries
converges instead of spinning.

### Minimal-fix constraints (§1.4)

* Expose the **earliest** deadline across sources #1–#6 above — all of which already exist.
* Do **not** invent a new independent timer.
* Do **not** create a second source of truth for timers/scheduling.
* Do **not** change PTO backoff, retry counts, the startup pacing formula, or FEC/tail/bootstrap
  behaviour.
* Clear the readiness signal after due-now work is consumed.

### Scope change required

`next_wakeup_ns()` must be available in the **default build** (the AŞAMA 9 runtime sleeps on it), so
its `#[cfg(feature = "experimental-cold-start")]` gate is removed. Source #1 remains feature-gated
internally and simply contributes nothing when the feature is off.
