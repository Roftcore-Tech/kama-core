# DR-CS-3 — V1 PANEL VERDICT, BOUNDED

**V1 locked result `2/3` is marked:**

`SUPERSEDED — INVALID FOR DEFAULT PROMOTION DUE TO TREATMENT-ONLY CHANNEL IMPAIRMENT`

The V1 records are preserved unchanged. This document bounds what may be concluded from them.

## What is structurally proven

1. The baseline's ACK and the treatment's Echo did **not** see the same reverse channel semantics —
   ACK reverse was lossless, Echo reverse was GE-lossy.
2. Per-class loss streams were used (`DRCS3ProbeForward`, `DRCS3EchoReverse`), so packet classes had
   independent channel state.
3. Therefore the V1 baseline/treatment comparison **was not comparable**, and cannot promote or
   block a default.

That is the entire structural claim. Nothing further.

## What is NOT claimed

The following are **not** established and must not be asserted:

* "C2 conclusively explains rep14."
* "The bootstrap mechanism works 3/3 on the locked panel."
* "rep14 will certainly recover under V2."

## Diagnostic expectation (recorded before any V2 result)

> V2 semantics are expected to change rep14's outcome. If rep14 still fails under V2, V1's
> invalidity is **not** reversed — it only means channel asymmetry was not the sole cause of the
> failure, and §4 (A0 state · A1 counter semantics · A2 observation coverage · A3 positive control)
> is applied to find the additional root cause.

This expectation is a diagnostic prior, **not** an acceptance gate. The binding gates are the nine
in `DR-CS-3-PANEL-CONTRACT-V2` (md `0abbb114…`, json `353311bf…`).
