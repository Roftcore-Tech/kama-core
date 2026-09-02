# DR-CS-2 — PRODUCT REPORT (FAZ 2)

## RESULT: **FAIL** — bounded recovery permit implemented and mechanically correct, but it does **not**
## resolve the cold-start stall, and it produced a new one. **Credit default stays OFF. STOPPED.**

## Preflight (§2) — **Durum A**

8 frozen stall seeds, GE-L10/p20/RTT1000, R0, startup pacing ON (product default).

| field | P0 tail-OFF (shipped) | P1 tail-ON (final FS-CAL baseline) |
|---|---:|---:|
| always-on ARQ retx selected / denied / emitted | 1,456,660 / **1,456,336** / 324 | 1,456,660 / 1,456,336 / 324 |
| geometric copy (§3.3-B) selected / denied / emitted | 30,113,898 / **30,113,896** / **2** | identical |
| tail probe selected / denied / emitted | 0 / 0 / 0 | 34,513,309 / 34,513,306 / 3 |
| tail probe due **EDGE** (unique demand) | 0 | **11** (vs 34.5 M polls) |
| DATA selected / denied / emitted | 37,479,734 / 37,478,032 / 1,702 | identical |
| PreConverge exits · ACK batches | 5/8 · 1,551 | 5/8 · 1,551 |

Starvation is present in **P0, the shipped tail-OFF default** ⇒ Durum A. The **§3.3-B geometric source
copy** is the only recovery class that can become due during a PreConverge stall (packet-threshold loss
declaration and time-threshold PTO are both gated off in PreConverge, so `pending_lost` stays empty).

## Implementation (contract frozen first)

```text
DR-CS-2-RECOVERY-PERMIT-CONTRACT.md    1ebb23a63a8316acf915cf0225abcd0496353a577fd913f5133ab2ed45289aea
DR-CS-2-RECOVERY-PERMIT-CONTRACT.json  5e221666ceb947a1ef5b8e31d9706dbf85c49d43fb21cf588e47c8c3433979e3
```

`BoundedRecoveryCreditConfig` (runtime, **default OFF**) · permit minted per `DupRec.id` (a unique work
item), never per poll · reachable only on the `try_send_pre_parity == false` branch inside
`drain_due_duplicates` · consumed immediately by exactly one `TransmitKind::Duplicate` · cancelled on ACK
progress · session budget `MAX_RECOVERY_PERMITS = 2`, derived from
`TailRecoveryConfig::default().max_tail_probes` (not invented) · pacing cost paid as a **bounded debit**
(`TokenBucket::debit`, repaid before any new token is granted). No new packet type, no wire codepoint, no
receiver change, DATA-before-PARITY untouched.

## §8.2 Locked regression — 8 frozen stall seeds, credit axis only

Identical in **both** panels (P0 and P1):

| rep | pc_exit OFF | pc_exit ON | armed | credited | debit B |
|---|---|---|---|---|---|
| 5 | 27,065 | 33,581 | 2 | 2 | 202 |
| **9** | **None** | **None** | 2 | 2 | 202 |
| **14** | **None** | **None** | 2 | 2 | 202 |
| 16 | 23,807 | 30,323 | 2 | 2 | 202 |
| **17** | **None** | **None** | 2 | 2 | 202 |
| 25 | 20,549 | 27,065 | 2 | 2 | 202 |
| 38 | 23,807 | 30,323 | 2 | 2 | 202 |
| **39** | **62,904** | **None** ← NEW STALL | 2 | 2 | 202 |

```text
MECHANISM-SUCCESS          PASS   (see below)
END-TO-END-RECOVERY-SUCCESS FAIL  0/3 baseline stalls rescued
SAFETY-PASS                PASS   0 data/parity/accounting violations; max 2 credited per run (= budget);
                                  0 credited emissions with the config OFF; ~5 M duplicate mints DENIED
```

## Why it fails — exact lineage (rep9, P0)

```text
CREDIT-ON: 2 permits armed, 2 credited duplicates emitted (202 B), fully debited
           first_delivery_ns = 8.533 s      <- a credited copy DID reach the receiver
           first_ack_ns      = 9.033 s      <- and it WAS acknowledged
           ack_batches       = 2
           max srtt_us       = 0            <- yet NO RTT sample was ever established
           state_changes     = [[0,"PreConverge"]]   <- never left PreConverge
```

**Failure class 1 — `CREDIT-DELIVERED-AND-ACKED-BUT-KARN-EXCLUDED` (structural).**
The credited emission is a `TransmitKind::Duplicate`. RTT sampling is gated at `sender.rs:2060` on
`p.origin == TransmitKind::Data` (Karn), so a duplicate's ACK **cannot** establish `srtt`. The
PreConverge→Startup gate requires the first *valid* SACK, i.e. an `srtt`. Therefore a credited **copy can
never open the cold-start gate**, no matter how many are granted or whether they are delivered.

**Failure class 2 — `CREDIT-IS-NET-NEUTRAL-UNDER-DEBIT`.**
`tx_data` 20 → **18** with `tx_duplicate` 0 → 2. Under total token starvation a pacing *debit* cannot add
transmissions; it only **reallocates** them — two fresh DATA slots were replaced by two duplicates of
units the same burst had already destroyed.

**Failure class 3 — new stall produced.** The 202 B debit is 202/31 = **6.52 s** of PreConverge pacing
allowance. That is exactly the observed convergence delay (27,065→33,581; 23,807→30,323; 20,549→27,065),
and it pushed **rep39** from converging at 62.9 s to not converging inside the 65 s budget.

## What was NOT done (per §9 FAIL discipline)
No timer added, no PreConverge floor change, no scheduler-priority change, no retry-count tuning, no
CsProbe/echo, no tail-cap change, no FEC change. No second treatment was stacked to rescue the result.

## Product state
```text
BoundedRecoveryCreditConfig::default()  = enabled: FALSE  (mechanism inert; shipped behaviour unchanged)
StartupPacingConfig::default()          = enabled: TRUE   (FAZ 1A, unchanged by this phase)
TailRecoveryConfig::default()           = enabled: FALSE  (unchanged)
adaptive-transport lib   142/142 PASS      fs_cal2_pins 33/33 PASS      clippy -D clean x3
```

## Implication for the next design step (NOT decided here)
Any cold-start remedy must produce an **RTT-sampleable original transmission** (or change what may
establish `srtt`), and must **add** allowance rather than reallocate it. A duplicate-based credit is
structurally incapable of both. This is a design decision, not a tuning knob.
