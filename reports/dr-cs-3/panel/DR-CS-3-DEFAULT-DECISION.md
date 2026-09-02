# DR-CS-3 — §8 DEFAULT PROMOTION DECISION

**Decision: `NO PROMOTION` — `BootstrapRttConfig::default()` stays `enabled: false`.**

**Verdict: `DR-CS-3 PANEL PREFLIGHT PASS · LOCKED PANEL FAIL (EFFICACY 2/3) · BOOTSTRAP RTT
DEFAULT-OFF · PRODUCT RUNTIME INTEGRATION PENDING`**

---

## Gate ledger

| §8 PASS condition | Result |
|---|---|
| Preflight conformance PASS | **PASS** — `DR-CS-3 PANEL PREFLIGHT PASS`, `CONTROL-ID-REFLECTION-SAFE` |
| Locked 3/3 stalls recovered | **FAIL — 2/3** (rep9 ✓, rep17 ✓, rep14 ✗) |
| Baseline success → treatment stall: 0 | PASS (0/5 both panels) |
| Fresh applicable P0/P1 panel PASS | **NOT RUN** (gated on the locked panel) |
| No-loss zero-overhead PASS | **NOT RUN** |
| IID non-regression PASS | **NOT RUN** |
| Probe/Echo loss safety PASS | **NOT RUN** as a panel; the equivalent safety properties are pinned by §1 preflight tests |
| Two-netns component lineage PASS | **NOT RUN** |
| Budget/security/accounting violations: 0 | PASS (0 across all 32 locked runs) |
| DATA displacement: 0 | PASS |
| ARQ timer diff: 0 | PASS |
| Rate-ramp regression: 0 | **FAIL — one open regression found** (see below; NOT a bootstrap issue) |
| OFF zero-diff preserved | PASS (0 probes, 0 echo bytes on every OFF arm) |

Two conditions fail, so the PASS branch does not apply.

---

## §8 FAIL-branch compliance

| Requirement | Status |
|---|---|
| Bootstrap stays default-OFF | **held** — `BootstrapRttConfig::default() = { enabled: false }`, source unchanged |
| Ladder timing unchanged | **held** — `BS_FIRES_NS = [3, 9, 21, 45] s` |
| Budget not increased | **held** — sender 200 B, receiver 172 B |
| No new retry/timer | **held** |
| PreConverge floor unchanged | **held** — `PRE_CONVERGE_FLOOR_BW = 31 B/s` |
| Duplicate-credit not reintroduced | **held** |
| Not rescued with tail or FEC | **held** — tail is a panel axis, never a treatment axis; FEC R0 throughout |
| Seed/threshold not tuned | **held** — no new seeds, no threshold moved, the locked seed domain is verbatim |
| No second treatment added | **held** |
| Exact failure lineage recorded | **done** — `DR-CS-3-LOCKED-PANEL` §"EXACT FAILURE LINEAGE — rep14" |

`git` state: no commit, no push, no PR. Product sources byte-identical to the start-of-turn manifest
(`START-FILE-HASHES.txt`, all 7 files `OK`).

---

## What the panel did establish

The failure is **not** a mechanism defect. In rep14 the mechanism executed perfectly and was defeated
by the channel:

* 4/4 probes emitted within budget (176 B ≤ 200 B),
* 4/4 probes authenticated at the real `ReceiverControlCore`,
* 4/4 echoes generated at exactly the 172 B budget,
* 0 echoes reached the wire — the deterministic reverse stream is `[D, D, D, D]`,
* every violation counter 0.

Where a round trip completed at all, the effect is large and consistent:

| rep | baseline | treatment |
|---|---|---|
| 9 | STALL (never left PreConverge in 65 s) | 4 000 ms |
| 17 | STALL | 4 000 ms |
| 39 | 62 904 ms | 4 000 ms |
| 5 | 27 065 ms | 4 000 ms |
| 38 | 23 807 ms | 4 000 ms |

Zero new stalls, zero DATA displacement, zero budget/auth/accounting violations, and the OFF arm is
inert in all 32 runs. Notably rep39 — the record the *previous* failed duplicate-credit treatment
newly stalled — not only does not stall, it starts 59 s earlier.

So the honest statement is: **the mechanism is correct and effective when a bootstrap round trip
survives; the 4-step ladder is not long enough to survive a reverse-path burst that spans all four
generations.** Extending the ladder is forbidden this turn and is not proposed as a fix here — it is
a decision, and it must be taken before any re-run, not after seeing this result.

---

## Recorded limitations that bear on the verdict (not used to overturn it)

1. **Consecutive-draw correlation.** The four echoes draw four consecutive indices from one GE chain,
   forcing maximal burst correlation between events 6 s / 12 s / 24 s apart, while GE's bad-run
   length is defined in *packets* (L=10).
2. **Reverse-path asymmetry.** The DATA path's reverse (ACK) direction is lossless — inherited
   verbatim from the frozen FS-CAL-2 channel so the 16 baselines reproduce exactly — while the
   bootstrap echo's reverse direction is GE-lossy.

Both were fixed by design before the panel ran. Changing either now to obtain a different verdict is
precisely what §0 forbids. They are recorded so the choice can be made **pre-outcome**.

---

## Collateral finding (separate from this verdict)

`DR-CS-3-COLLATERAL-FINDING-RATE-RAMP-PIN`: the sealed Commit-W leftover-equivalence pin
(`budget_equivalence_pin`) had stopped **compiling** because DR-CS-3 un-gated `TransmitKind::CsProbe`
while the test's match arm was still feature-gated. Compilation was restored (test-only, one line);
the pin then **fails on behaviour**, and the cause is isolated to **startup pacing default-ON**
(DR-RATE-RAMP-2), not to the bootstrap probe. The earlier "rate-ramp validated, no regressions"
claim did not cover this pin because the target was not building. Left failing and visible; the fix
requires an explicit decision and was not taken here.

---

## Verdict

```
DR-CS-3 PANEL PREFLIGHT PASS
· LOCKED PANEL FAIL (EFFICACY 2/3)
· BOOTSTRAP RTT DEFAULT-OFF
· PRODUCT RUNTIME INTEGRATION PENDING
```

Still forbidden, unchanged by anything in this turn:
`SHIPPING RECEIVER COMPLETE` · `FULL PRODUCT END-TO-END PASS` · `DEPLOYABLE TRANSPORT` ·
`TRANSPORT PRODUCT READY` · `PRODUCT COLD-START FULLY FIXED`.

`DUR.`
