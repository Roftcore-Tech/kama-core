# ACK-PATH-ROOTCAUSE (FAZ 1)

> Deterministic replay of the known stall seeds. **No product behaviour was changed by this diagnostic.**
> `fit_use = FORBIDDEN` · `calibration_use = FORBIDDEN` · `result_bearing = false`

## Question
Why did the six DATA emissions in the GE-L10 / p=20% / RTT=1000 ms stalled runs produce **zero** ACKs?

## Method
Per-DATA-emission lineage (`AckChainRec`), recorded observation-only for every wire emission:
```text
sender source id -> sender wire emission -> FORWARD impairment decision -> receiver arrival
-> receiver parse/accept -> ACK eligibility -> ACK generation -> REVERSE impairment
-> sender socket arrival -> sender parse -> ACK accept/drop -> ACK progress / state transition
```
Cold-start / tail / dup / ARQ counters are kept **separate** (feature `diag-sched-telemetry`).

## Result — **`ACK-PATH-FORWARD-LOSS`** (unambiguous)

| seed | emissions | forward-dropped | receiver arrivals | ACKs generated | ACKs ingested |
|---|---|---|---|---|---|
| rep9 | 6 | **6** | 0 | 0 | 0 |
| rep14 | 6 | **6** | 0 | 0 | 0 |
| rep17 | 6 | **6** | 0 | 0 | 0 |
| **total** | **18** | **18 (100%)** | **0** | **0** | **0** |

Every emission was dropped at the **first** hop. Nothing ever reached the receiver, so **no receiver
defect and no sender-ingest defect can be implicated** — those hops were never exercised.

Emission instants: `0.0, 3258.1, 6516.1, 9774.2, 13032.3, 16290.3 ms`. The spacing is exactly
**3258.1 ms = 101 B / 31 B/s** — the PreConverge floor quantum, per emission, confirmed to the tick.

## Classification
```text
ACK-PATH-FORWARD-LOSS      CONFIRMED (18/18)
ACK-PATH-RECEIVER          REFUTED — hop never reached
ACK-PATH-SENDER-INGEST     REFUTED — hop never reached
ACK-PATH-REVERSE-LOSS      NOT REACHED (see limitation)
ACK-PATH-HARNESS           REFUTED — the harness offered all 384 units and drove Timer::Pto every tick;
                           the forward drop is the DESIGN-NORMATIVE GE-L10 impairment, not a harness fault
```

## Honest limitation
The harness models **no reverse-path ACK loss**: a delivered packet always generates an ACK one OWD
later (`reverse_drop` is structurally `false`). `ACK-PATH-REVERSE-LOSS` is therefore **unobservable** in
this harness. It does not affect this verdict — the reverse path was never reached — but a future
reverse-loss study needs a harness change.

## Routing consequence
Forward loss ⇒ **no receiver or ingest patch is warranted.** The cold-start remedy belongs to FAZ 3.
The six emissions are not an ACK-path bug: they are the arithmetic consequence of the PreConverge floor
(31 B/s) meeting a GE-L10 opening burst.
