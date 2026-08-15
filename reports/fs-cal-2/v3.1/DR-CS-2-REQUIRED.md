# DR-CS-2 REQUIRED — product cold-start liveness (raised by FS-CAL-2 §4.2b diagnosis)

> Raised by: `reports/fs-cal-2/v3.1/FS-CAL-2-V3.2-DECISION-CHECKPOINT.md` §5.
> Evidence namespace: `FSFC2-DIAG-CS-V32` (`fit_use = FORBIDDEN`, `result_bearing = false`).
> **This is a STOP marker, not an authorization to implement.** DR-CS-2 scope/design is Abkarada's call.

## Why this is raised

The FS-CAL-2 §4.2b failure diagnosis found a **confirmed `PRODUCT-COLD-START-LIVENESS` component**:
**5 runs never left `SenderState::PreConverge` within 65 s** — beyond the product's own authoritative
cold-start give-up bound (`CS_GIVEUP_NS = 60 s`) — while the harness drove every product path.

`HARNESS-DRIVER` is **refuted**: `app_enqueued = 384` on every stalled run (the harness offered every unit).

## Mechanism (exact, code-grounded)

```text
1. PRE_CONVERGE_FLOOR_BW = 31 B/s                       sender.rs:726   ("~0.25 kbps")
   returned unconditionally while in PreConverge        sender.rs:2306-2308 (compute_send_rate)
   ⇒ one 101 B sealed unit costs ~3.26 s of token accrual.

2. PreConverge is left ONLY on the first valid SACK     sender.rs:56-58, 2066-2073  [K-KEYSTONE]
   PTO / time-threshold loss declaration is FROZEN      sender.rs:1847, :1808
   ⇒ with no ACK there is no exit and no ARQ.

3. The cold-start probe ladder and give-up bound are COMPILED OUT of the default build:
       CS_PROBE_FIRES_NS = [3, 9, 21, 45] s             sender.rs:192   #[cfg(feature="experimental-cold-start")]
       CS_GIVEUP_NS      = 60 s                         sender.rs:198   #[cfg(feature="experimental-cold-start")]
   ⇒ no escape hatch exists in the shipped configuration.

4. DATA-first scheduling                                sender.rs:2356 before :2364 / :2365
   ⇒ fresh DATA consumes the floor tokens BEFORE §3.3-B geometric copies and the bounded tail probe,
     starving BOTH cold-start recovery mechanisms.
```

## Measured evidence

| budget | predicted `tx_data` = budget × 31 / 101 | **observed** |
|---|---|---|
| 18.498 s | 5.68 | **6** (all 8 replayed stall seeds) |
| 65.0 s | 19.95 | **20** (all 5 stalled runs) |

Across all 5 stalled runs:

```text
state_changes    = [[0, "PreConverge"]]        one state for the entire 65 s
send_rate        = 31 B/s                      constant, never ramps
srtt             = 0                           never any sample
inflight         = 101 → 2020                  monotonic, never drains
app_enqueued     = 384                         harness offered everything
ack_batches      = 0
tx_duplicate     = 0                           §3.3-B geometric copies NEVER fired
tail_probe_due   = 6,400,000                   probe decided due 6.4 M times ...
tail_probe_sent  = 0                           ... and NEVER emitted (token-starved)
```

Corroborates the existing DR-ARQ-PRODUCT-TAIL-1 record: *"high-loss cold-start fragility — probe
token-starved in PreConverge floor pacing."* Also matches the code-derived predictor
`stall ⟺ leading_drop_run ≥ 6`, which reproduces 8/8 stalls and 72/72 non-stalls with zero error.

## Scope honesty

**Workload-conditional.** Token starvation of §3.3-B requires a permanently backlogged `tx_queue`. An
app-limited flow would drain it, `front_size` would become `DEFAULT_MTU = 1200` (`sender.rs:1249`), and
copies/probes could fire. So this is **"cold-start liveness under a sustained bulk workload"**, not
unconditional. Frequency in the fresh diagnostic cell (GE-L10, p=20%, RTT=1000 ms): **2/40 = 5%** hard
stalls, plus 4/40 that converge only after 18.5 s.

## Second, separate pathology (do not conflate)

**Post-convergence rate-ramp lock.** `D2 rep2` left PreConverge at 1.1 s, then locked at **288 B/s** in
`Startup`→`RecoverArq` and never ramped — 63 packets in 65 s, versus 27,525 B/s in healthy runs (95× gap);
`probe_due = 388,427`, `probe_sent = 0`. `rep31` shows the same shape. This is **not** cold start and is
**not** addressed by any warm-up, window or timeout change. Decide whether it belongs in DR-CS-2 or its own DR.

## Open questions for the DR (not decided here)

1. Is `experimental-cold-start` (the `CS_PROBE`/`CS_GIVEUP` ladder) the intended remedy — i.e. should it be
   promoted toward default — or is the **PreConverge floor rate** / **DATA-first scheduling priority** the
   thing to revisit?
2. Should the floor be workload-relative (e.g. guarantee at least one recovery emission per PTO interval)
   rather than an absolute 31 B/s that cannot carry a single 101 B unit inside an RTT?
3. Should the bounded tail probe get token priority over fresh DATA while `srtt` is absent?
4. Is a 31 B/s worst-corner prior still the right choice for the BLOS/long-haul regime this product targets?

## Status

```text
DR-CS-2            REQUIRED — raised, NOT started, NOT scoped, NOT authorized.
FS-CAL-2 v3.2      BLOCKED behind it (PATH-B) unless PATH-A is chosen instead.
YETKİ-2            NOT AUTHORIZED.
Product code       UNCHANGED this turn. No safety parameter modified.
```
