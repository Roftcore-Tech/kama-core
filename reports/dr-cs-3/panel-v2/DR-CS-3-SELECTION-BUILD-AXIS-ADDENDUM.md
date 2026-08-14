# DR-CS-3 — SELECTION BUILD-AXIS ADDENDUM (OUTCOME-BLIND, FROZEN)

**Status:** FROZEN **before any selection data exists**. No selection run, no seed search, no panel
and no OFF-only selection had been executed when this file was written. Nothing in it was chosen
against a result, because there is no result to choose against.

**Scope:** it fixes the *build axis* of every DR-CS-3 selection/panel run. It changes no metric, no
denominator, no cell construction and no verdict rule.

---

## 1. The closed finding this addendum records

Under `--features experimental-cold-start` **the bootstrap-OFF arm is NOT a no-treatment control.**

The LEGACY experimental cold-start ladder is gated to fire **exactly when the bootstrap treatment is
disabled** — i.e. in the baseline/OFF arm:

```text
sender.rs:2996-2999
    #[cfg(feature = "experimental-cold-start")]
    if !self.bootstrap_rtt.enabled {
        self.drain_cs_probes(&mut out);
    }
```

`drain_cs_probes` (`sender.rs:2065`) calls `self.pacer.try_send(CS_PROBE_WIRE_MAX)`
(`sender.rs:2071`, `CS_PROBE_WIRE_MAX = 44`, `sender.rs:194`), which **DEBITS 44 tokens**.

So the legacy ladder engages at the SAME 3 s point as the shipping bootstrap treatment, on the SAME
pacer, in the arm that is supposed to be doing nothing.

**Measured (paired structural oracle, `cell_2`, `--features experimental-cold-start`):**

| quantity | value |
|---|---|
| `decision_entry_tokens` (OFF arm, pre-arbitration) | **93** |
| `post_turn_tokens` (OFF arm, post-arbitration residue) | **49** |
| debit | **44 B** |
| reconciliation | `93 = 49 + 0 + 44` (residual 44, closing exactly on the observed probe bytes) |
| attribution | `LEGACY-COLD-START-LADDER` — off-arm `bootstrap_rtt_diag().probe_emitted = 0`, legacy `cs_probe_emitted = 1` |

The paired "additive treatment vs nothing" premise **does not hold** in that configuration: the
control arm runs a competing PACED ladder against the same token bucket.

---

## 2. AUTHORITATIVE SELECTION/PANEL BUILD

```text
AUTHORITATIVE SELECTION/PANEL BUILD:
Cargo default feature set, experimental-cold-start NOT enabled.
```

In that build the legacy ladder is **not compiled at all**: there is no `drain_cs_probes` call site,
and `diag_cold_start()` (`sender.rs:2101`) does not exist. The OFF arm is therefore a genuine
no-treatment control by construction, not by hope.

### One binary, two runtime states

The **SAME binary** MUST serve the OFF selection and the later paired ON arm. The two arms may differ
**ONLY in runtime treatment state** — `BootstrapRttConfig { enabled: false }` vs
`{ enabled: true }` — and in nothing else. Specifically:

* no feature-flag difference between arms;
* no recompilation between arms;
* no profile, codegen or `cfg` difference between arms;
* the recorded binary hash MUST be byte-identical across the OFF selection and the paired ON arm.

A treatment/control pair built from two different binaries is not a paired comparison and is
inadmissible for this gate.

---

## 3. What every selection/panel run MUST assert AND report

Each run emits these fields into its own record. They are **assertions**, not merely logged notes:
a failure halts the run.

| # | field | requirement |
|---|---|---|
| 1 | `cfg!(feature = "experimental-cold-start")` | MUST be `false` |
| 2 | enabled-feature fingerprint | COMPLETE enumeration of every crate feature and its on/off state — not just the cold-start flag |
| 3 | binary hash | recorded; identical across the OFF selection and the paired ON arm |
| 4 | bootstrap treatment state | the runtime enabled/disabled state of the bootstrap treatment, per arm |
| 5 | OFF `bootstrap_rtt_diag().probe_emitted` delta | MUST be `0` over the whole run |
| 6 | OFF `cs_probe_emitted` delta | MUST be `0`. In the authoritative build the accessor does not exist and the path is not compiled: the run MUST record that fact explicitly (`legacy_ladder_compiled = false`) rather than silently reporting a zero it never read. Where the accessor does exist (diagnostic builds only), the delta MUST be read and asserted `== 0`. |

The feature fingerprint is required in full because keying only on `experimental-cold-start` would
print a build label that is false in every other non-default configuration.

---

## 4. HALT RULE — no silent repair

> **ANY shipping-bootstrap or legacy-CS probe observed in the OFF arm halts the run with
> `INVALID-BASELINE-CONFIG - ABORT SELECTION`.**

This applies to both origins, which are byte-indistinguishable on the wire (same
`TransmitKind::CsProbe`, same 44 B) and are separated only by their DISJOINT product counters:

* `bootstrap_rtt_diag().probe_emitted > 0` in the OFF arm ⇒ the SHIPPING treatment leaked past its
  own gate (`sender.rs:2436`) into the control arm. A product finding. Never excusable.
* `bootstrap_rtt_diag().probe_emitted == 0` **and** the legacy counter corroborating ⇒ the legacy
  ladder ran. A configuration error: the run was built off the authoritative axis.
* neither counter claiming it ⇒ an unattributed probe. Also abort — the excuse would rest on an
  unproven lineage.

**Forbidden responses**, in all three cases:

* silently excluding the offending seed;
* replacing the offending seed with another;
* re-rolling, re-running or extending the seed set to route around it;
* down-grading the event to a note while the selection continues.

The abort is loud and terminal for that run. The configuration is fixed, and the run starts over from
the beginning on the authoritative build.

---

## 5. Admissibility of `experimental-cold-start` results

> `experimental-cold-start` results are **diagnostic / configuration evidence ONLY**.

They can **never** enter:

* a no-treatment **efficacy** panel;
* a **prevalence** estimate;
* a **default-promotion** decision.

They may be used to characterise the legacy ladder itself, to prove the confound exists, and to
prove that the attribution machinery separates the two probe origins. That is their entire admissible
range. A number produced under that feature never becomes evidence about the shipping default, in
either direction.

---

## 6. Outcome-blindness statement

This addendum was written and frozen **before** any DR-CS-3 selection data existed. It constrains how
runs are built and how failures are handled; it makes no prediction about, and takes no position on,
what those runs will show.
