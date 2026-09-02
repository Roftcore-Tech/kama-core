# DR-ARQ-TAIL-1 R1 — DIAGNOSTIC MECHANISM REPLAY

> Label: **DIAGNOSTIC MECHANISM REPLAY** (uses previously-seen FS-1 terminal-tail configs; NOT confirmatory).
> Experiment-only deterministic tail-recovery deconfounder — NOT a product ARQ, NOT RFC 9002, NOT kernel/netem.

## Terminal-tail rows (ExactDeficit; realized erasure includes terminal source-id 47)

- terminal-tail erasure rows: **5**
- of those, A0-FROZEN censors: **5**
- **A0-TAIL closes (completes): 5 / 5**
- F0 completes the same rows: **2 / 5**
- A0-TAIL added ARQ retransmit bytes (total over the recovered set): 1515

## Per-row (traceable)

- rtt50-exactd4 rep1: A0=censored A0-TAIL=COMPLETE F0=censored +retx_bytes=404
- rtt250-exactd1 rep4: A0=censored A0-TAIL=COMPLETE F0=COMPLETE +retx_bytes=101
- rtt250-exactd2 rep4: A0=censored A0-TAIL=COMPLETE F0=COMPLETE +retx_bytes=202
- rtt250-exactd4 rep1: A0=censored A0-TAIL=COMPLETE F0=censored +retx_bytes=404
- rtt1000-exactd4 rep1: A0=censored A0-TAIL=COMPLETE F0=censored +retx_bytes=404

## Interpretation bounds (§6)
A0-TAIL success does NOT mean 'Kama product ARQ fixed'. It shows the bounded probe recovers terminal-tail erasures the frozen ACK-driven ARQ cannot. F0 vs A0-TAIL cost-fairness is assessed in R2 (separate preregistered grid); this replay is diagnostic only.
