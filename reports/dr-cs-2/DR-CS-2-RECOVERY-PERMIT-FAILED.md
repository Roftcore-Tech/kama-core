# DR-CS-2 — DUPLICATE RECOVERY PERMIT: FAILED (formally closed)

> This artifact **supersedes** `DR-CS-2-RECOVERY-PERMIT-CONTRACT.md`
> (sha256 `1ebb23a63a8316acf915cf0225abcd0496353a577fd913f5133ab2ed45289aea`).
> The superseded contract and every diagnostic record are **preserved unchanged**.

## Verdict
The bounded duplicate recovery permit is **structurally incapable** of resolving the cold-start stall.
It is withdrawn from the product tree. This is not a tuning failure and must not be retried by adjusting
permit counts, debit size, or budget.

## Failure classes

**`CREDIT-DELIVERED-AND-ACKED-BUT-KARN-EXCLUDED`** — decisive, structural.
rep9 (P0) with credit ON: 2 permits armed, 2 credited duplicates emitted (202 B), fully debited; the
copies **reached the receiver** (`first_delivery_ns = 8.533 s`) and **were acknowledged**
(`first_ack_ns = 9.033 s`, `ack_batches = 2`). Yet `max srtt_us = 0` for the whole run and
`state_changes = [[0, "PreConverge"]]`. Cause: a credited emission is `TransmitKind::Duplicate`, and RTT
sampling is gated at `sender.rs:2060` on `p.origin == TransmitKind::Data` (Karn); the PreConverge exit
requires an `srtt`. A duplicate's ACK therefore **can never open the cold-start gate**.

**`CREDIT-IS-NET-NEUTRAL-UNDER-DEBIT`.** `tx_data` 20 → **18**, `tx_duplicate` 0 → 2. Under total token
starvation a pacing *debit* cannot add transmissions; it only reallocates them — two fresh DATA slots were
replaced by duplicates of units the same burst had already destroyed.

**`NEW-STALL-PRODUCED`.** The 202 B debit is 202/31 = **6.52 s** of PreConverge allowance. Every
converging seed was delayed by exactly that (27,065→33,581 · 23,807→30,323 · 20,549→27,065 ms), and
**rep39** was pushed from converging at 62.9 s to not converging within the 65 s budget.

## Locked regression outcome (identical in P0 tail-OFF and P1 tail-ON)
```text
baseline stalls (rep9, rep14, rep17)            3
rescued by the treatment                        0        <- 0/3
new stalls created by the treatment             1        (rep39)
permits per run / credited per run / debit      2 / 2 / 202 B
```

## Why the mechanism being CORRECT does not help
`MECHANISM-SUCCESS` and `SAFETY-PASS` both passed: permits were minted once per `DupRec.id` (≈5 M
duplicate mints denied), never per poll; the session budget (2) was never exceeded; no DATA, parity or
accounting violation occurred; nothing was credited with the config OFF; every credited byte was debited.
The mechanism did exactly what its contract specified. The contract itself targeted the wrong quantity:
**cold start is unlocked by an RTT sample, not by a delivered byte.**

## Binding consequence for any successor treatment
A cold-start remedy must (a) produce an **RTT-sampleable transmission** — or change what may establish
`srtt` — and (b) **add** send allowance rather than reallocate it. Duplicate-based credit satisfies neither.

## Disposition
`BoundedRecoveryCreditConfig`, the permit mint/consume state, the duplicate-credit emission path and the
permit-specific product tests are **removed** from the product tree (FAZ 0.2). The failure artifacts,
the superseded contract and all diagnostic records are retained.
