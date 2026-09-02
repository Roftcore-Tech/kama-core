# DR-CS-2 SIZING — PREREGISTRATION v1

> Written and frozen BEFORE any sizing run. Diagnostic only.
> `fit_use = FORBIDDEN` · `calibration_use = FORBIDDEN` · `result_bearing = false`
> Namespace: `DR-CS-2-SIZING-V1` (disjoint from every calibration and every prior diagnostic namespace).

## B1 Question
Without changing any other ARQ/pacing/scheduling value, does enabling the EXISTING experimental
cold-start probe ladder resolve the PreConverge stalls at GE-L10 / p=20% / RTT=1000 ms, or does it
block on probe scheduler/token priority?

## B2 Frozen STALL predicate (frozen before outcomes)
```text
T_EVAL = 65 seconds

STALL(T_EVAL) := no PreConverge exit by T_EVAL
                 OR no valid ACK/SACK received by T_EVAL
A run that mechanically completes earlier AND left PreConverge on a valid ACK/SACK ⇒ STALL = false.

ZERO_TX_STALL := STALL(T_EVAL) = true
                 AND data_wire_emitted = 0
                 AND recovery/control wire emission = 0
"sent 20 packets but never left PreConverge" is NOT ZERO_TX_STALL, but IS STALL.
PRIMARY classification is on STALL.
```

## B3 Single-axis arms
```text
CS-BASELINE       current product Sender, experimental ladder OFF
CS-LADDER-ONLY    exact same Sender/config, ONLY the experimental ladder ON
```
No other config/timeout/pacing/floor/priority/ARQ/tail/FEC difference is permitted.

## B4 Condition and pairing
```text
condition : GE-L10, p = 20%, RTT = 1000 ms
arms      : {CS-BASELINE, CS-LADDER-ONLY}
N         : 40 paired seeds
timeout   : exactly 65 s
seed      : SHA256("DR-CS-2-SIZING-V1" || repetition || stream_class)
total     : 40 pairs x 2 arms = 80 diagnostic runs
```
Prior D2 records are context/reproduction evidence ONLY — never the control arm of this comparison.

## B6 Minimum baseline-event gate (frozen)
```text
fresh CS-BASELINE baseline_stall_count < 4
  -> CS-AMBIGUOUS-INSUFFICIENT-EVENTS
  -> no product scope class is chosen
  -> report the N required at the observed stall rate
  -> NO additional runs in the same turn
```

## B7 Frozen classification (chosen before outcomes)
`CS-SMALL` · `CS-MEDIUM` · `CS-SCHEDULING-ARCH` · `CS-NO-SIGNAL` · `CS-AMBIGUOUS` (definitions per the
task brief). If CS-LADDER-ONLY fails, NO second treatment (floor/priority) is produced in the same turn.

## A-gate precondition (blocking)
The sizing run executes ONLY if §A1 returns `LADDER-PRESENT-AND-ISOLATABLE`. If the ladder is absent,
would require a fake harness probe in place of the product Sender, or cannot be isolated to one axis,
the required output is `LADDER-NOT-ISOLATABLE` and the run is NOT executed.
