//! DR-CS-3 — **PAIRED STRUCTURAL ORACLE** for the gate-5 displacement component.
//!
//! Classification: `STRUCTURAL REGRESSION INVARIANT — PACER/GRANT DISPLACEMENT EXPECTED 0 BY
//! ADDITIVE, UNPACED, DATA-FIRST DESIGN`; semantics `A — contended grant/token`.
//!
//! ## Grounded accessor semantics (from signatures, not tuple position)
//! * `diag_loss() = (diag_loss_pkt, diag_loss_time, pending_lost.len())` — `.0`/`.1` are CUMULATIVE
//!   loss-declared counters; `.2` is an INSTANTANEOUS depth. Loss is declared *and* drained inside
//!   one `on_event(AckReceived)`, so post-call depth is NOT an activation oracle.
//! * `diag_pacer() = (rate_bps, tokens, burst_cap)`; tokens are BYTES.
//! * `try_send(size)`: `tokens >= size` then `tokens -= size` ⇒ **grant cost = packet bytes**.
//! * **PACER debt: none.** `TokenBucket::try_send` (pacer.rs:59-66) is a plain compare-and-subtract
//!   over a single `tokens` field; the struct (pacer.rs:17-24) has `rate_bps` / `tokens` /
//!   `burst_cap` / `refill_rem` and NO debt field, so a denied packet leaves no obligation behind.
//!   This is a claim about the PACER only — the *FEC budget kernel* is a different subsystem and
//!   does model debt (`budget_kernel.rs:31-33` `BoundedDebtFloor`, `:48` `max_debt_bytes`,
//!   `:87` / `:403` `debt_bytes`). Nothing in this file reads or depends on that kernel's debt.
//!
//! ## THREE MEASUREMENT POINTS (never conflated)
//! ```text
//! call_entry_state    tokens observed BEFORE on_event is called (pre-refill)
//! decision_entry_state derived post-refill/pre-arbitration capacity — the ONLY authoritative
//!                     input to the gate-5 grant predicate
//! post_turn_residue   diag_pacer() AFTER on_event returned — an AUDIT input only; it is NEVER
//!                     used as an entry predicate anywhere in this file
//! ```
//! The previous revision read `diag_pacer()` **after** the target `on_event` returned. The pacer
//! refills INSIDE the same `on_event` that arbitrates, so that reading is the POST-ARBITRATION
//! RESIDUE, not the entry capacity — which is why three differently-named cells all collapsed to
//! `grants=0`. Entry capacity is now **derived pre-arbitration** from the frozen refill arithmetic
//! and is never read back out of the sender after the call. A post-turn reconstruction
//! (`tokens_after + Σ paced bytes`) is kept only as an independent audit of the derivation.
//!
//! ## MARK/PROVE INDEPENDENCE RULE (why criteria (2) and (4) are falsifiable)
//! ```text
//! A criterion that re-tests the SAME local that defined the thing it certifies is UNFALSIFIABLE:
//! no assignment of measured facts can make the flag true and the check false, so the check prints
//! `true` while verifying nothing. Every activation leg and every cap-lineage term below therefore
//! names TWO sources and keeps them disjoint:
//!
//!   leg       MARKING source (sets the flag)             PROVING source (independent evidence)
//!   ────────  ─────────────────────────────────────────  ────────────────────────────────────────
//!   probe     a CsProbe DECISION in the target turn      bootstrap_rtt_diag().probe_emitted DELTA
//!                                                        across that same on_event
//!   parity    parity_pending_at_entry > 0 (the exact     parity_competed_for_paced_grant:
//!             diag_fec_actuation() depth read BEFORE     diag_fec_actuation().emitted_r_total DELTA
//!             the call), or parity_actuated_in_target_   across the target on_event, or the
//!             turn (a FecParity DECISION in that turn)   diag_token_denial() event-count DELTA
//!                                                        across it (the parity queue front was
//!                                                        reached and refused by the pacer)
//!   recovery  a Retransmit DECISION in the target turn   diag_loss() ledger movement across that
//!                                                        same on_event: a positive CUMULATIVE
//!                                                        loss-declared delta (.0/.1) and/or a
//!                                                        strict DROP of the pending_lost depth (.2)
//!
//!   crit. (4) burst_cap  ← diag_pacer().2                front_size ← the harness's OWN enqueue
//!             window_bytes ← diag_pacer().0 * window     ledger (it knows exactly which unit it
//!                                                        pushed and its size; no shipping accessor
//!                                                        for the tx-queue front exists and none is
//!                                                        added), so
//!                 burst_cap == window_bytes.clamp(front_size, max(front_size, 2*MTU))
//!             is a cross-check between three INDEPENDENTLY obtained numbers. Breaking exactly one
//!             of them turns the criterion RED.
//!
//! Consequence: mutating ONLY a proving source must turn the criterion RED while the marking source
//! still reports the cell active. That is the mutation contract this file is verified against.
//! ```
//!
//! ## PARITY — THREE DISTINCT CONCEPTS, NEVER CONFLATED
//! ```text
//! parity_pending_at_entry          EXACT pending-parity depth read from diag_fec_actuation()
//!                                  BEFORE the target on_event (enqueued − emitted − cancelled).
//!                                  It says parity WORK EXISTS. It says NOTHING about what the
//!                                  target turn did with it.
//! parity_actuated_in_target_turn   a real TransmitKind::FecParity DECISION was emitted IN THE
//!                                  TARGET TURN (cross-checked against the emitted_r_total delta).
//!                                  This is the ONLY thing that means "parity actually ran here".
//! parity_competed_for_paced_grant  the scheduler REACHED the parity queue front in the target turn
//!                                  and the pacer answered it: emitted_r_total DELTA > 0 (parity
//!                                  took a grant) or diag_token_denial() event-count DELTA > 0
//!                                  (parity asked and was REFUSED). Parity TOUCHED THE PACER.
//!
//! `parity_pending_at_entry > 0` is NOT `parity_actuated_in_target_turn`, and NEITHER of them is
//! evidence of parity-vs-DATA contention. The product plans DATA/ARQ BEFORE `plan_fec_parity` in
//! the SAME turn (sender.rs `schedule_transmit`), so parity only ever sees the RESIDUE of a grant
//! that DATA has already been offered. A parity token denial is therefore DOWNSTREAM of the DATA
//! decision, not a competitor to it.
//! ```
//!
//! ## cell_7 CLAIM SCOPE (the decisive cell, correctly described)
//! ```text
//! cell_7 = PROBE-DUE x ONE-DATA-GRANT x PARITY-PENDING-AT-ENTRY
//!          PARITY NOT ACTUATED IN TARGET TURN
//! MEASURED (default build): parity_pending_at_entry = 3, actual DATA grants = 1,
//!          FecParity decisions in the target turn = 0, emitted_r_total delta = 0
//!          (token_denial delta = 1 — the parity queue front was reached and REFUSED, AFTER DATA
//!          had already taken the single grant).
//! IT IS     genuine Probe-vs-DATA contention evidence, and it REMAINS in the Gate-5 denominator.
//! IT IS NOT evidence of target-turn parity ACTUATION.
//! IT IS NOT evidence of parity-vs-DATA contention.
//! The earlier label "parity-active" asserted both of the last two and is WITHDRAWN. Nothing about
//! the measurement, the cell construction, the denominator or any assertion changed with it — only
//! what the cell is allowed to be called.
//! ```
//!
//! ## FROZEN ARITHMETIC LINEAGE (measurement-validated; not re-derived here)
//! ```text
//! PreConverge floor send_rate = 31 B/s      (identical for startup_pacing = false AND true)
//! DATA unit                   = 101 B       (= its exact pacer grant cost)
//! window_bytes = 31 * 40_000_000 / 1e9      = 1
//! front_size   = tx_queue.front().size      = 101      (holds only while DATA is queued)
//!   OBSERVED INDEPENDENTLY of the pacer: the harness keeps its own FIFO ledger of the units it
//!   enqueued (id + size, in order) and of the DATA decisions the product has emitted, so the front
//!   entry is read from that ledger — NEVER derived from `burst_cap`, which is the very number the
//!   clamp check below is supposed to test.
//! burst_hi     = max(front_size, 2*MTU)     = 2400
//! burst_cap    = clamp(1, 101, 2400)        = 101
//!   LINEAGE RULE: window_bytes < front_size ⇒ the clamp LOCKS to front_size ⇒ the bucket carries
//!   ~ONE front-packet grant. `MultipleDataGrants` (>= 202) is therefore impossible by THIS
//!   LINEAGE — not merely by the literal value of a constant. Both halves (window_bytes and
//!   front_size) are OBSERVED per cell below, never assumed.
//! entry_tokens(T) = min(burst_cap, call_entry_tokens + floor(rate*T_elapsed/1e9))
//!   T=2900ms -> 89     T=3258ms -> 100     T=3259ms -> 101
//! saturation_ns = ceil(101 * 1e9 / 31)      = 3_258_064_517 ns
//!   ⇒ at and after that instant entry_tokens is PINNED at 101: OneDataGrant is a STABLE
//!     saturated state, NOT a fragile single point.
//! Bootstrap ladder steps are ABSOLUTE from the arm instant: 3 s / 9 s / 21 s / 45 s.
//! ```
//!
//! ## FEATURE-CONFIGURATION AXIS (why the control arm is not always a control)
//! ```text
//! sender.rs:2996-2999
//!     #[cfg(feature = "experimental-cold-start")]
//!     if !self.bootstrap_rtt.enabled { self.drain_cs_probes(&mut out); }
//! ```
//! The LEGACY experimental CS ladder fires exactly when the bootstrap treatment is DISABLED — i.e.
//! in the BASELINE/OFF arm — and `drain_cs_probes` (sender.rs:2065) calls
//! `self.pacer.try_send(CS_PROBE_WIRE_MAX)` (sender.rs:2071), which DEBITS 44 tokens. Under
//! `--features experimental-cold-start` the OFF arm is therefore **not a no-treatment control**: it
//! runs a competing PACED ladder, and the paired "additive treatment vs nothing" premise does not
//! hold. Such cells are classified `CONFOUNDED-BASELINE-CONFIG`, excluded from the DATA-displacement
//! denominator, and named in the claim scope. They are never silently skipped and never allowed to
//! masquerade as a product finding.
//!
//! ## THE CONFOUND EXCUSE IS ATTRIBUTED, NOT INFERRED FROM THE KIND
//! A `CsProbe` in the control arm is NOT by itself evidence of the legacy ladder. The SHIPPING
//! bootstrap path emits an IDENTICAL decision — same `TransmitKind::CsProbe`, same wire size
//! (`CS_PROBE_WIRE_MAX` = 44, sender.rs:194 == `BS_PROBE_WIRE_EXACT` = 44, sender.rs:597) — so a
//! regression in the treatment's OWN gate (`if !self.bootstrap_rtt.enabled || self.bs_done { return; }`,
//! sender.rs:2436) leaking a shipping probe into the control arm would be byte-indistinguishable from
//! the legacy ladder and would otherwise be absorbed by the excuse. The two paths are separated by
//! their PRODUCT COUNTERS, which are disjoint:
//! ```text
//! shipping bootstrap  drain_bootstrap_probes  ⇒ bs_diag.probe_emitted++      (sender.rs:2466)
//!                                               read via bootstrap_rtt_diag() — ALWAYS COMPILED
//! legacy CS ladder    drain_cs_probes         ⇒ cs_probe_emitted++            (sender.rs:2083)
//!                                               read via diag_cold_start().0  — cfg-gated
//!                                               and NEVER touches bs_diag
//! ```
//! So the control arm's probe is attributed by [`BaselineProbeSource`]:
//! `bootstrap_rtt_diag().probe_emitted > 0` in the OFF arm ⇒ **PRODUCT FINDING in EVERY feature
//! configuration** (the shipping treatment leaked into the control arm), never `CONFOUNDED`;
//! `== 0` **and** the cold-start diag corroborating the legacy emission ⇒ `CONFOUNDED`; anything
//! else ⇒ an unattributed probe, also RED. The `!cfg!(experimental-cold-start)` corroboration only
//! ever covered the DEFAULT build, which left the hole exactly in the configs where the excuse is
//! applied (including `--all-features`); the counter read closes it cfg-independently.
//!
//! The excuse is also NARROWED TO THE LINEAGE IT PROVES: `pacer.try_send(44)` debits TOKENS and
//! nothing else, so only the pacer TOKEN component may be excused, and only when its MAGNITUDE
//! equals the observed baseline probe bytes. `burst_cap` and `rate` divergence between the paired
//! arms stay RED even inside a confounded cell.
//!
//! Shipping `Sender` has no `Clone` and none is added; no shipping accessor, token setter or any
//! state-mutating helper is added either. Both arms are built from identical frozen inputs and
//! asserted equal before the target event. Token accumulation uses a SINGLE event-free jump to the
//! exact derived timestamp — never a 100 ms poll loop, whose intervening events consume tokens and
//! corrupt the measurement.

use adaptive_transport::sender::{
    AckInfo, BootstrapRttConfig, Event, OpaqueUnit, Sender, SenderState, StartupPacingConfig,
    Timer, TransmitDecision, TransmitKind,
};
use std::collections::BTreeSet;
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;

/// DATA size, and therefore its exact grant cost in tokens.
const UNIT: u64 = 101;
/// `PRE_CONVERGE_FLOOR_BW` (sender.rs:900).
const FLOOR_BPS: u64 = 31;
/// `clamp(window_bytes=1, front_size=101, burst_hi=2400)` while DATA is queued.
const EXPECTED_BURST_CAP: u64 = 101;
/// `ceil(101 * 1e9 / 31)` — first instant at which `entry_tokens == 101`.
const SATURATION_NS: u64 = 3_258_064_517;
/// `BS_FIRES_NS` (sender.rs:595) — ABSOLUTE offsets from the ladder arm instant.
const PROBE_STEPS_NS: [u64; 4] = [3 * SEC, 9 * SEC, 21 * SEC, 45 * SEC];
/// `BURST_WINDOW_NS` (sender.rs:972) — the cap window used by the `set_burst_cap` clamp.
const BURST_WINDOW_NS: u64 = 40_000_000;
/// `rate::DEFAULT_MTU` — the `front_size` fallback used when the tx queue is EMPTY (sender.rs:1547).
const DEFAULT_MTU_BYTES: u64 = 1200;
/// `CS_PROBE_WIRE_MAX` (sender.rs:194) — the LEGACY ladder's paced debit per probe.
const LEGACY_CS_PROBE_WIRE_MAX: u64 = 44;

// ── plan / oracle input ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanEntry {
    kind: &'static str,
    seq: u64,
    unit_id: u64,
    size: u64,
}

fn kind_name(k: TransmitKind) -> &'static str {
    match k {
        TransmitKind::Data => "Data",
        TransmitKind::Retransmit => "Retransmit",
        TransmitKind::Duplicate => "Duplicate",
        TransmitKind::FecParity => "FecParity",
        TransmitKind::CsProbe => "CsProbe",
    }
}

/// Ordered NORMAL-packet plan; the (append-only) Probe suffix is excluded by construction.
fn normal_plan(out: &[TransmitDecision]) -> Vec<PlanEntry> {
    out.iter()
        .filter(|d| d.kind != TransmitKind::CsProbe)
        .map(|d| PlanEntry {
            kind: kind_name(d.kind),
            seq: d.seq,
            unit_id: d.unit_id,
            size: d.size,
        })
        .collect()
}

/// Bytes that debited the pacer in a turn **excluding every `CsProbe`**.
///
/// `CsProbe` cannot be attributed by kind alone: the SHIPPING bootstrap path
/// (`drain_bootstrap_probes`, sender.rs:2425-2434) deliberately never calls `try_send` /
/// `try_send_pre_parity` and is therefore UNPACED, while the LEGACY experimental path
/// (`drain_cs_probes`, sender.rs:2065) emits the SAME `TransmitKind::CsProbe` and DOES pace 44 B via
/// `pacer.try_send` (sender.rs:2071). The two are indistinguishable by kind AND by wire size, so
/// probe bytes are excluded here; the residual MAGNITUDE is reconciled against [`cs_probe_bytes`] at
/// the call site, and the probe's ORIGIN is attributed separately, from the two paths' DISJOINT
/// product counters, by [`classify_baseline_probe`]. A residual equal to the baseline arm's observed
/// probe bytes closes the arithmetic; anything else is `HARNESS-LIMITED`. Closing the arithmetic is
/// NECESSARY but NOT SUFFICIENT for the `CONFOUNDED-BASELINE-CONFIG` excuse — the counter
/// attribution decides that, because a leaked SHIPPING probe closes it identically.
fn paced_bytes_excluding_cs_probe(out: &[TransmitDecision]) -> u64 {
    out.iter()
        .filter(|d| d.kind != TransmitKind::CsProbe)
        .map(|d| d.size)
        .sum()
}

/// Observed `CsProbe` wire bytes in a turn — the quantity that decides whether an unclosed
/// reconciliation residual is a legacy-ladder debit or a genuine harness limitation.
fn cs_probe_bytes(out: &[TransmitDecision]) -> u64 {
    out.iter()
        .filter(|d| d.kind == TransmitKind::CsProbe)
        .map(|d| d.size)
        .sum()
}

/// The LEGACY ladder's own emission counter, where the accessor exists.
///
/// `diag_cold_start()` is `#[cfg(feature = "experimental-cold-start")]`; `.0` is `cs_probe_emitted`,
/// incremented ONLY by `drain_cs_probes` (sender.rs:2083). Returns `None` in builds where the legacy
/// ladder is not compiled at all — there is then no accessor AND no legacy path, so a control-arm
/// probe cannot be attributed to it.
fn legacy_cs_probe_emitted(s: &Sender) -> Option<u64> {
    #[cfg(feature = "experimental-cold-start")]
    {
        Some(s.diag_cold_start().0)
    }
    #[cfg(not(feature = "experimental-cold-start"))]
    {
        let _ = s;
        None
    }
}

/// Origin of a `CsProbe` observed in the CONTROL (bootstrap-OFF) arm.
///
/// The two emitting paths are byte-indistinguishable on the wire (both `TransmitKind::CsProbe`,
/// both 44 B) but write to DISJOINT product counters, so the origin is decided by the counters and
/// NEVER by the decision alone — and never by `cfg`, which only ever covered the default build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaselineProbeSource {
    /// No `CsProbe` in the control arm.
    None,
    /// `bootstrap_rtt_diag().probe_emitted > 0` in the bootstrap-OFF arm: the SHIPPING treatment
    /// fired in the CONTROL arm. A PRODUCT FINDING in EVERY feature configuration — the gate
    /// `if !self.bootstrap_rtt.enabled || self.bs_done { return; }` (sender.rs:2436) failed. This is
    /// NEVER excusable as a confound.
    ShippingBootstrapLeak,
    /// `bootstrap_rtt_diag().probe_emitted == 0` AND the legacy ladder's own counter corroborates
    /// the emission. The only source that earns the `CONFOUNDED-BASELINE-CONFIG` excuse.
    LegacyColdStartLadder,
    /// A `CsProbe` that NO product counter claims. Not excusable: the excuse would rest on an
    /// unproven lineage.
    Unattributed,
}

impl BaselineProbeSource {
    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ShippingBootstrapLeak => "SHIPPING-BOOTSTRAP-LEAK",
            Self::LegacyColdStartLadder => "LEGACY-COLD-START-LADDER",
            Self::Unattributed => "UNATTRIBUTED",
        }
    }
}

/// Attribute the control arm's probe from the two DISJOINT product counters.
///
/// `bs_probe_emitted` is `bootstrap_rtt_diag().probe_emitted` read on the OFF arm (always compiled);
/// `legacy_emitted` is `diag_cold_start().0` where that accessor exists.
fn classify_baseline_probe(
    baseline_probe_bytes: u64,
    bs_probe_emitted: u64,
    legacy_emitted: Option<u64>,
) -> BaselineProbeSource {
    if baseline_probe_bytes == 0 {
        return BaselineProbeSource::None;
    }
    if bs_probe_emitted > 0 {
        // Checked FIRST and unconditionally: a shipping leak must never be absorbed by the excuse.
        return BaselineProbeSource::ShippingBootstrapLeak;
    }
    match legacy_emitted {
        Some(n) if n > 0 => BaselineProbeSource::LegacyColdStartLadder,
        _ => BaselineProbeSource::Unattributed,
    }
}

/// Stable DATA identities actually emitted in a turn.
fn data_identities(out: &[TransmitDecision]) -> BTreeSet<u64> {
    out.iter()
        .filter(|d| d.kind == TransmitKind::Data)
        .map(|d| d.unit_id)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OracleInput {
    plan: Vec<PlanEntry>,
    tokens: u64,
    burst_cap: u64,
    rate: u64,
    inflight: u64,
    sent_logical_len: usize,
    pending_lost_depth: u64,
}

fn input(s: &Sender, out: &[TransmitDecision]) -> OracleInput {
    let (rate, tokens, burst_cap) = s.diag_pacer();
    let (_st, _bw, _srtt, _r, inflight) = s.diag_rate();
    let (sl, _ev, _m) = s.diag_sent_logical();
    OracleInput {
        plan: normal_plan(out),
        tokens,
        burst_cap,
        rate,
        inflight,
        sent_logical_len: sl,
        pending_lost_depth: s.diag_loss().2,
    }
}

// ── the ONE shared oracle ───────────────────────────────────────────────────────────────────────

/// Displacement verdict.
///
/// **NO AGGREGATE `total`.** The dimensions are REPORTED SEPARATELY AND NEVER SUMMED. They are not
/// independent — `displaced_data_ids` is a SUBSET of `missing_or_replaced_ids` by construction — so
/// summing them would double-count one physical event. One physical substitution shows up as exactly
/// ONE displaced DATA identity, plus whatever other diagnostic dimensions it also perturbs. A pure
/// ORDER divergence may raise `violation` but must NOT fabricate a displaced identity.
#[derive(Debug, Default, Clone)]
struct DisplacementReport {
    /// Stable DATA identities (unit ids) that OFF emitted and ON did not — DEDUPLICATED.
    displaced_data_ids: BTreeSet<u64>,
    /// `== displaced_data_ids.len()` by construction.
    direct_displacement_count: usize,
    /// Every OFF plan entry (any kind) absent from the ON plan. SUPERSET of `displaced_data_ids`.
    missing_or_replaced_ids: Vec<PlanEntry>,
    order_divergence: usize,
    length_divergence: usize,
    /// The pacer triple, kept as THREE NAMED COMPONENTS and never collapsed into one number.
    /// The legacy ladder's `pacer.try_send(44)` (sender.rs:2071) is a plain compare-and-subtract on
    /// `tokens`; it cannot move `burst_cap` or `rate`. Splitting the triple is what makes the
    /// confound excuse expressible over EXACTLY the dimension the lineage proves.
    pacer_tokens_divergence: usize,
    pacer_burst_cap_divergence: usize,
    pacer_rate_divergence: usize,
    inflight_state_divergence: usize,
    sent_logical_divergence: usize,
    /// true iff ANY dimension above is non-zero.
    violation: bool,
}

impl DisplacementReport {
    /// Every dimension EXCEPT the pacer TOKEN component. `burst_cap` and `rate` divergence are
    /// deliberately INSIDE this predicate: the legacy ladder debits tokens and nothing else, so a
    /// cap or rate divergence between the paired arms is outside the confound's proven lineage and
    /// stays RED even in a confounded cell.
    fn diverges_beyond_pacer_tokens(&self) -> bool {
        self.direct_displacement_count > 0
            || !self.missing_or_replaced_ids.is_empty()
            || self.order_divergence > 0
            || self.length_divergence > 0
            || self.pacer_burst_cap_divergence > 0
            || self.pacer_rate_divergence > 0
            || self.inflight_state_divergence > 0
            || self.sent_logical_divergence > 0
    }
}

/// The single comparator used by BOTH the shipping cells and the synthetic control.
/// Never `zip`-only: it sees length, unmatched entries, identity, kind and order.
fn evaluate_grant_displacement(off: &OracleInput, on: &OracleInput) -> DisplacementReport {
    let mut r = DisplacementReport {
        length_divergence: off.plan.len().abs_diff(on.plan.len()),
        pacer_tokens_divergence: usize::from(off.tokens != on.tokens),
        pacer_burst_cap_divergence: usize::from(off.burst_cap != on.burst_cap),
        pacer_rate_divergence: usize::from(off.rate != on.rate),
        inflight_state_divergence: usize::from(off.inflight != on.inflight)
            + usize::from(off.pending_lost_depth != on.pending_lost_depth),
        sent_logical_divergence: usize::from(off.sent_logical_len != on.sent_logical_len),
        ..DisplacementReport::default()
    };
    // Any OFF normal packet not present in the ON plan was replaced or dropped.
    for p in &off.plan {
        if !on.plan.contains(p) {
            if p.kind == "Data" {
                // ONE physical substitution ⇒ ONE stable identity. The set deduplicates.
                r.displaced_data_ids.insert(p.unit_id);
            }
            r.missing_or_replaced_ids.push(p.clone());
        }
    }
    r.direct_displacement_count = r.displaced_data_ids.len();
    // Order divergence over the common prefix — a DIAGNOSTIC dimension, never a displaced identity.
    for (a, b) in off.plan.iter().zip(on.plan.iter()) {
        if a != b {
            r.order_divergence += 1;
        }
    }
    r.violation = r.direct_displacement_count > 0
        || !r.missing_or_replaced_ids.is_empty()
        || r.order_divergence > 0
        || r.length_divergence > 0
        || r.pacer_tokens_divergence > 0
        || r.pacer_burst_cap_divergence > 0
        || r.pacer_rate_divergence > 0
        || r.inflight_state_divergence > 0
        || r.sent_logical_divergence > 0;
    r
}

// ── grant / token classification ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrantClass {
    ZeroDataGrant,
    OneDataGrant,
    MultipleDataGrants,
}

impl GrantClass {
    fn of(grants: u64) -> Self {
        match grants {
            0 => Self::ZeroDataGrant,
            1 => Self::OneDataGrant,
            _ => Self::MultipleDataGrants,
        }
    }
}

/// `LiteralZeroTokens` and `ZeroDataGrantCapacity` are DISTINCT concepts and are never conflated:
/// `tokens == 89` is NOT zero-token, it is only zero-grant-capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenPredicate {
    /// `tokens == 0`.
    LiteralZeroTokens,
    /// `0 < tokens < 101` — capacity for zero DATA grants, but tokens are NOT literally zero.
    ZeroDataGrantCapacityNonZeroTokens,
    /// `tokens == 101` (== burst_cap): saturated, exactly one DATA grant. STABLE, not fragile.
    OneDataGrantCapacitySaturated,
}

impl TokenPredicate {
    fn holds(self, tokens: u64) -> bool {
        match self {
            Self::LiteralZeroTokens => tokens == 0,
            Self::ZeroDataGrantCapacityNonZeroTokens => tokens > 0 && tokens < UNIT,
            Self::OneDataGrantCapacitySaturated => tokens == UNIT,
        }
    }

    /// The predicate an OBSERVED entry capacity actually satisfies. `None` = above `burst_cap`,
    /// which the frozen config cannot produce (and would itself be a finding).
    fn observed(tokens: u64) -> Option<Self> {
        match tokens {
            0 => Some(Self::LiteralZeroTokens),
            t if t < UNIT => Some(Self::ZeroDataGrantCapacityNonZeroTokens),
            t if t == UNIT => Some(Self::OneDataGrantCapacitySaturated),
            _ => None,
        }
    }
}

/// Per-cell constructibility verdict. Exactly ONE is assigned to every cell that is actually built
/// and run. Regimes that are never built at all carry
/// `UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT` and are recorded separately (cell_4a / cell_4b /
/// cell_5), because there is no turn to fingerprint for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellVerdict {
    /// The CONTROL (OFF) arm actually took a DATA grant and emitted a DATA identity, so a Probe has
    /// something real to displace. ONLY these cells count as DATA-displacement evidence.
    ContentionTesting,
    /// Probe activated in the treatment arm, but the control arm emitted no DATA — nothing exists to
    /// displace, so the cell is a Probe-ACTIVATION control, not displacement evidence.
    ActivatedButVacuousForDataDisplacement,
    /// Parity was ACTUATED in the target turn (a real `FecParity` decision), but the control arm
    /// emitted no DATA — so parity ran while nothing existed for it to displace. A parity-ACTUATION
    /// control, NOT displacement evidence, and NOT in the Gate-5 denominator.
    ParityActuatedButVacuousForDataDisplacement,
    /// Recovery work, or parity work merely PENDING AT ENTRY, is present but the control arm emitted
    /// no DATA — a state-INVARIANCE control, not displacement evidence. Distinct from the variant
    /// above: nothing was actuated in the target turn here.
    StateActiveButVacuousForDataDisplacement,
    /// The treatment is entirely INERT here (probe not due by design) and the control arm emits no
    /// DATA. A negative control for probe activation; not displacement evidence.
    InertTreatmentVacuousForDataDisplacement,
    /// The turn could not be reconciled by code lineage — the measurement, not the product, is the
    /// limit. Reported, never silently dropped.
    CellUnconstructibleHarnessLimitation,
    /// The control arm is NOT a no-treatment control in this feature configuration: it runs the
    /// LEGACY paced CS ladder. Excluded from the DATA-displacement denominator; the claim scope
    /// names the feature axis.
    ConfoundedBaselineConfig,
}

impl CellVerdict {
    fn label(self) -> &'static str {
        match self {
            Self::ContentionTesting => "contention-testing",
            Self::ActivatedButVacuousForDataDisplacement => {
                "ACTIVATED-BUT-VACUOUS-FOR-DATA-DISPLACEMENT"
            }
            Self::ParityActuatedButVacuousForDataDisplacement => {
                "PARITY-ACTUATED-BUT-VACUOUS-FOR-DATA-DISPLACEMENT"
            }
            Self::StateActiveButVacuousForDataDisplacement => {
                "STATE-ACTIVE-BUT-VACUOUS-FOR-DATA-DISPLACEMENT"
            }
            Self::InertTreatmentVacuousForDataDisplacement => {
                "INERT-TREATMENT-VACUOUS-FOR-DATA-DISPLACEMENT"
            }
            Self::CellUnconstructibleHarnessLimitation => "CELL-UNCONSTRUCTIBLE-HARNESS-LIMITATION",
            Self::ConfoundedBaselineConfig => "CONFOUNDED-BASELINE-CONFIG",
        }
    }
}

/// Kind label for a tx-queue front SIZE.
///
/// The size itself is NEVER derived from `burst_cap` — that is exactly the tautology closure
/// criterion (4) exists to break. It comes from [`Arm::observed_front`], the harness's own FIFO
/// ledger of enqueued units, which is independent of everything `diag_pacer()` reports. This
/// function only maps that independently observed size onto a human label, and criterion (4)
/// cross-checks the recorded label against the recorded size.
fn front_kind_of(front_size: u64) -> &'static str {
    match front_size {
        UNIT => "Data(OpaqueUnit)",
        DEFAULT_MTU_BYTES => "EMPTY-QUEUE(DEFAULT_MTU)",
        _ => "NON-DATA-FRONT",
    }
}

// ── arm construction + shadow pacer model (observation-only, no product mutation) ────────────────

/// One paired arm plus the harness-side shadow of the pacer's *hidden* sub-byte refill remainder.
///
/// `rate` / `tokens` / `burst_cap` are always the OBSERVED `diag_pacer()` values. Only
/// `refill_rem` is modelled, because `TokenBucket::refill_rem` has no accessor and adding one is
/// forbidden. The model mirrors exactly one line of frozen product arithmetic
/// (`numer = rate*dt + rem; rem = numer % 1e9`, pacer.rs) and is independently audited after every
/// target turn by `tokens_after + Σ paced bytes == derived entry_tokens`.
struct Arm {
    s: Sender,
    pre: Vec<TransmitDecision>,
    rate: u64,
    tokens: u64,
    burst_cap: u64,
    refill_rem: u128,
    t_last: u64,
    /// The harness's OWN FIFO ledger of what it pushed into the sender's tx queue, in order:
    /// `(unit id, size)`. This is the INDEPENDENT observation of the queue front that closure
    /// criterion (4) needs — `tx_queue` has no shipping accessor and none is added, and deriving the
    /// front from `burst_cap` would make the clamp check a value compared with itself.
    enqueued_units: Vec<(u64, u64)>,
}

impl Arm {
    fn resync(&mut self, t: u64) {
        let (rate, tokens, burst_cap) = self.s.diag_pacer();
        self.rate = rate;
        self.tokens = tokens;
        self.burst_cap = burst_cap;
        self.t_last = t;
    }

    /// PRE-ARBITRATION entry capacity at `t` (`decision_entry_state`). Mirrors `refill()` and then
    /// applies ONE clamp.
    ///
    /// **STATED ASSUMPTION:** the product refills against the PREVIOUS turn's `burst_cap` and only
    /// afterwards recomputes the cap (`set_burst_cap`, sender.rs:1547-1551), which re-clamps the
    /// bucket. Applying a single clamp with `self.burst_cap` is therefore exact **iff the old and
    /// new caps are equal**. Every cell asserts `cap_before == cap_after` explicitly, so this
    /// assumption breaks LOUDLY (a red) instead of silently, if a future change ever makes the cap
    /// move within the target turn. Computed BEFORE the target event is delivered; never read back
    /// after the call.
    fn derive_entry_tokens(&self, t: u64) -> u64 {
        let dt = t.saturating_sub(self.t_last);
        let numer = u128::from(self.rate) * u128::from(dt) + self.refill_rem;
        let add = u64::try_from(numer / 1_000_000_000).unwrap_or(u64::MAX);
        self.tokens.saturating_add(add).min(self.burst_cap)
    }

    /// Deliver one event, advancing the shadow remainder with the rate in force BEFORE the event
    /// (the product refills first, then calls `set_rate`).
    fn advance(&mut self, ev: Event, t: u64) -> Vec<TransmitDecision> {
        let dt = t.saturating_sub(self.t_last);
        let numer = u128::from(self.rate) * u128::from(dt) + self.refill_rem;
        self.refill_rem = numer % 1_000_000_000;
        let out = self.s.on_event(ev, t);
        self.resync(t);
        out
    }

    fn data_emitted(&self) -> u64 {
        self.pre
            .iter()
            .filter(|d| d.kind == TransmitKind::Data)
            .count() as u64
    }

    /// Exact remaining tx-queue depth. `self.pre` MUST be kept current — every turn's output has to
    /// be folded back in before this is called again, or the count over-reports.
    fn queued_data(&self) -> u64 {
        (self.enqueued_units.len() as u64) - self.data_emitted()
    }

    /// The tx-queue FRONT as `(size, kind)`, read from the harness's OWN enqueue ledger.
    ///
    /// `Sender::on_event` drains `tx_queue` strictly FIFO, so the front is the ledger entry at index
    /// `data_emitted()`. When every enqueued unit has been emitted the queue is EMPTY and the product
    /// falls back to `DEFAULT_MTU` (`sender.rs:1547`,
    /// `self.tx_queue.front().map_or(DEFAULT_MTU, |u| u.size)`) — mirrored here.
    ///
    /// This deliberately reads NOTHING from `diag_pacer()`. `front_size` obtained this way and
    /// `burst_cap` obtained from `diag_pacer()` are two independent numbers, which is what turns the
    /// criterion-(4) clamp identity into a real cross-check instead of `x == x`.
    fn observed_front(&self) -> (u64, &'static str) {
        let idx = usize::try_from(self.data_emitted()).unwrap_or(usize::MAX);
        match self.enqueued_units.get(idx) {
            // The LABEL comes from queue OCCUPANCY, the SIZE from the ledger entry — two different
            // facts, so criterion (4)'s `front_kind == front_kind_of(front_size)` compares them
            // against each other rather than re-deriving one from the other.
            Some(&(_id, size)) => (size, "Data(OpaqueUnit)"),
            // Ledger exhausted ⇒ the product's tx_queue is empty ⇒ the DEFAULT_MTU fallback.
            None => (DEFAULT_MTU_BYTES, "EMPTY-QUEUE(DEFAULT_MTU)"),
        }
    }

    /// EXACT pending-parity depth (no accessor exists): enqueued − emitted − cancelled.
    fn pending_fec(&self) -> u64 {
        let d = self.s.diag_fec_actuation();
        d.enqueued_r_total - d.emitted_r_total - d.canceled_r_total
    }
}

/// Build one arm from frozen inputs, up to (but not including) the target event.
///
/// All `AppData` are delivered at `t = 0`, so the hidden refill remainder is PROVABLY 0 at the end
/// of the build (`refill(0)`: `numer = rate*0 + 0 = 0`). The very first unit passes `try_send`
/// (pacer starts full at `DEFAULT_BURST_CAP`, then clamps to 101) and therefore ARMS the bootstrap
/// ladder at `t0 = 0` — which is why the ladder steps below are absolute 3/9/21/45 s.
#[allow(deprecated)] // legacy staging path; the shipping authority route is not under test here
fn build_arm(
    bootstrap_on: bool,
    startup_pacing: bool,
    units: u64,
    parity: Option<(u64, u16)>,
) -> Arm {
    let mut s = Sender::new();
    // MANDATORY: StartupPacingConfig::default() is enabled=true (sender.rs:684).
    s.set_startup_pacing_init(StartupPacingConfig {
        enabled: startup_pacing,
    });
    s.set_bootstrap_rtt(BootstrapRttConfig {
        enabled: bootstrap_on,
    });
    if parity.is_some() {
        s.set_manual_fec_enabled(true)
            .expect("manual FEC authority must be grantable from the Manual default");
    }
    let mut pre = Vec::new();
    // The enqueue LEDGER is written here, from the exact units handed to the product — this is the
    // independent tx-queue-front observation criterion (4) cross-checks `burst_cap` against.
    let mut enqueued_units: Vec<(u64, u64)> = Vec::new();
    for i in 0..units {
        let unit = OpaqueUnit {
            id: 1000 + i,
            size: UNIT,
        };
        enqueued_units.push((unit.id, unit.size));
        pre.extend(s.on_event(Event::AppData(unit), 0));
    }
    // Parity is staged IMMEDIATELY before the target turn, identically in both arms, with NO
    // intervening event (so the pacer clock does not move).
    if let Some((size, count)) = parity {
        for esi in 0..count {
            s.enqueue_fec_parity(9, esi, size);
        }
    }
    let mut arm = Arm {
        s,
        pre,
        rate: 0,
        tokens: 0,
        burst_cap: 0,
        refill_rem: 0,
        t_last: 0,
        enqueued_units,
    };
    arm.resync(0);
    arm
}

/// Observe the product's FIRST bootstrap ladder step from LIVE state, so the cell_4a
/// unconstructibility guard compares an OBSERVED product quantity against an OBSERVED product
/// quantity instead of two file-local literals.
///
/// Bounded bisection over FRESH arms (never a poll loop on one arm — intervening events would
/// consume tokens and shift the very quantity being measured). The predicate "a probe is emitted by
/// a single `Pto` at `t`" is monotone in `t`: once a ladder step is due it stays due.
fn observe_first_probe_step_ns() -> Option<u64> {
    let fires = |t: u64| -> bool {
        let mut a = build_arm(true, false, 8, None);
        a.advance(Event::TimerFired(Timer::Pto), t)
            .iter()
            .any(|d| d.kind == TransmitKind::CsProbe)
    };
    let hi_bound = 8 * SEC;
    if !fires(hi_bound) {
        return None; // ladder never fires within the search bound
    }
    if fires(0) {
        return Some(0);
    }
    let (mut lo, mut hi) = (0u64, hi_bound);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if fires(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// `ceil(cap * 1e9 / rate)` from OBSERVED cap and OBSERVED rate — the live counterpart of the
/// frozen `SATURATION_NS`.
fn observed_saturation_ns(cap: u64, rate: u64) -> Option<u64> {
    if rate == 0 {
        return None;
    }
    let n = u128::from(cap) * 1_000_000_000u128;
    let d = u128::from(rate);
    u64::try_from(n.div_ceil(d)).ok()
}

// ── constructibility table ──────────────────────────────────────────────────────────────────────

struct RegimeCell {
    id: &'static str,
    target_ns: u64,
    expected_probe_due: bool,
    expected_entry_tokens: u64,
    expected_token_predicate: TokenPredicate,
    expected_grant_class: GrantClass,
    required_activation: &'static str,
    units: u64,
    parity: Option<(u64, u16)>,
    /// FROZEN expectation for `parity_pending_or_actuated`
    /// (`parity_pending_at_entry > 0 || parity_actuated_in_target_turn`) — i.e. "this cell stages
    /// parity work at all". It is deliberately NOT an expectation of ACTUATION: whether the target
    /// turn actuates parity is a MEASURED outcome, reported per cell and carried by the verdict.
    expect_parity_pending_or_actuated: bool,
    /// The cell's own frozen claim-scope sentence about parity. PRINTED, never asserted over.
    parity_claim_scope: &'static str,
}

/// The regimes the frozen arithmetic actually admits. Every field left of the result is FROZEN
/// BEFORE the run.
const REGIME_CELLS: [RegimeCell; 5] = [
    RegimeCell {
        id: "cell_1 probe-NOT-due x ZeroDataGrant",
        target_ns: 2_900 * MS, // < 3.000 s ⇒ ladder step 0 not yet due
        expected_probe_due: false,
        expected_entry_tokens: 89, // floor(31 * 2.900)
        expected_token_predicate: TokenPredicate::ZeroDataGrantCapacityNonZeroTokens,
        expected_grant_class: GrantClass::ZeroDataGrant,
        required_activation: "queued DATA only (no probe, no recovery, no parity)",
        units: 8,
        parity: None,
        expect_parity_pending_or_actuated: false,
        parity_claim_scope: "NO parity staged: parity_pending_at_entry == 0 and no FecParity \
                             decision is possible ⇒ this cell says NOTHING about parity.",
    },
    RegimeCell {
        id: "cell_2 probe-DUE x ZeroDataGrant",
        target_ns: 3_000 * MS, // in [3.000, 3.258065) ⇒ step 0 due, still sub-grant
        expected_probe_due: true,
        expected_entry_tokens: 93, // floor(31 * 3.000)
        expected_token_predicate: TokenPredicate::ZeroDataGrantCapacityNonZeroTokens,
        expected_grant_class: GrantClass::ZeroDataGrant,
        required_activation: "probe emitted in treatment arm + queued DATA",
        units: 8,
        parity: None,
        expect_parity_pending_or_actuated: false,
        parity_claim_scope: "NO parity staged: parity_pending_at_entry == 0 and no FecParity \
                             decision is possible ⇒ this cell says NOTHING about parity.",
    },
    RegimeCell {
        id: "cell_3 probe-DUE x OneDataGrant",
        target_ns: SATURATION_NS, // >= 3.258065 s ⇒ tokens pinned at burst_cap: STABLE saturation
        expected_probe_due: true,
        expected_entry_tokens: 101,
        expected_token_predicate: TokenPredicate::OneDataGrantCapacitySaturated,
        expected_grant_class: GrantClass::OneDataGrant,
        required_activation:
            "probe emitted in treatment arm + exactly one DATA grant + queued DATA",
        units: 8,
        parity: None,
        expect_parity_pending_or_actuated: false,
        parity_claim_scope: "NO parity staged: parity_pending_at_entry == 0 and no FecParity \
                             decision is possible ⇒ this cell says NOTHING about parity.",
    },
    RegimeCell {
        id: "cell_6 probe-DUE x ZeroDataGrant x parity-PENDING-at-entry",
        target_ns: 3_000 * MS,
        expected_probe_due: true,
        expected_entry_tokens: 93,
        expected_token_predicate: TokenPredicate::ZeroDataGrantCapacityNonZeroTokens,
        expected_grant_class: GrantClass::ZeroDataGrant,
        required_activation: "probe emitted + parity PENDING AT ENTRY + queued DATA (whether the \
                              target turn ACTUATES parity is MEASURED, never required)",
        units: 8,
        parity: Some((40, 3)),
        expect_parity_pending_or_actuated: true,
        parity_claim_scope: "PARITY STAGED (pending at entry). Whether it is ACTUATED in \
                             the target turn is a MEASURED outcome carried by the verdict. The \
                             control arm takes NO DATA grant here, so whatever parity does is \
                             VACUOUS for DATA displacement and this cell is NOT in the Gate-5 \
                             denominator.",
    },
    // cell_7 — THE DECISIVE CELL for the Gate-5 Probe-vs-DATA claim. Strictly stronger than cell_6
    // (which is sub-grant and therefore vacuous for DATA displacement) and strictly stronger than
    // cell_3 (which stages no parity work at all). It is cell_6's construction (units 8, parity
    // 3x40 B) at cell_3's instant (SATURATION_NS), so the probe is DUE, parity work is PENDING AT
    // ENTRY, and the single grant is GENUINELY CONTENDED BY THE PROBE. Predicates below were frozen
    // BEFORE the first run.
    //
    // CLAIM SCOPE — PROBE-DUE x ONE-DATA-GRANT x PARITY-PENDING-AT-ENTRY, PARITY NOT ACTUATED IN
    // TARGET TURN. Measured in the default build: parity_pending_at_entry = 3, actual DATA grants
    // = 1, FecParity decisions in the target turn = 0. The cell REMAINS in the Gate-5 denominator
    // as genuine Probe-vs-DATA contention evidence, but it is NOT evidence of target-turn parity
    // ACTUATION and NOT evidence of parity-vs-DATA contention.
    //
    // If parity were to consume the single grant ahead of DATA, the control arm would emit no DATA
    // and the honest verdict is PARITY-ACTUATED-BUT-VACUOUS-FOR-DATA-DISPLACEMENT — the
    // scheduler-order lineage and the observed front kind/size are printed either way, and nothing
    // is forced to look like a pass. Product plan order (sender.rs schedule_transmit): DATA/ARQ →
    // tail probe → duplicates → plan_fec_parity → budget_settle → legacy CS ladder → bootstrap
    // probe. DATA is planned FIRST, which is exactly why a parity token denial in this turn is
    // downstream of the DATA decision rather than a competitor to it.
    RegimeCell {
        id: "cell_7 probe-DUE x OneDataGrant x parity-PENDING-at-entry",
        target_ns: SATURATION_NS,
        expected_probe_due: true,
        expected_entry_tokens: 101,
        expected_token_predicate: TokenPredicate::OneDataGrantCapacitySaturated,
        expected_grant_class: GrantClass::OneDataGrant,
        required_activation: "probe emitted + parity PENDING AT ENTRY + exactly one DATA \
                              grant CONTENDED BY THE PROBE (parity ACTUATION is MEASURED, never \
                              required)",
        units: 8,
        parity: Some((40, 3)),
        expect_parity_pending_or_actuated: true,
        parity_claim_scope: "PROBE-DUE x ONE-DATA-GRANT x PARITY-PENDING-AT-ENTRY, PARITY \
                             NOT ACTUATED IN TARGET TURN. Gate-5 evidence for PROBE-vs-DATA \
                             contention ONLY. It is NOT evidence of target-turn parity ACTUATION \
                             and NOT evidence of parity-vs-DATA contention — the measured \
                             actuation/competition facts are printed per run.",
    },
];

// ── activation evidence (SUBSTANTIVE facts, never a formatted string) ───────────────────────────

/// The concrete in-target-turn observations behind each activation claim, split into the source
/// that MARKS a leg active and the INDEPENDENT source that PROVES it.
///
/// Closure criterion (2) asserts over THESE, not over the non-emptiness of a `format!`ed string —
/// a string built from literal text can never be empty, so a guard on `is_empty()` is unreachable
/// and certifies nothing.
///
/// **The split is the point.** The previous revision recorded the two locals that *defined* the
/// parity flag and then re-tested that same disjunction, so no assignment of measured facts
/// could make the flag true and the check false: the criterion printed `true` while verifying
/// nothing (mutating `parity_pending_at_entry` did NOT turn it red). Every marking field below comes
/// from a different product surface than the `*_proof_*` fields it is checked against, so breaking
/// exactly one proving source turns criterion (2) RED while the marking source still says active.
#[derive(Debug, Clone, Copy)]
struct ActivationObservation {
    // ── PROBE ───────────────────────────────────────────────────────────────────────────────────
    /// MARK — a `TransmitKind::CsProbe` decision in the treatment arm's target-turn output.
    probe_mark_decision_in_turn: bool,
    /// PROOF — `bootstrap_rtt_diag().probe_emitted` DELTA across the target `on_event` on the
    /// TREATMENT arm: the shipping counter, written by `drain_bootstrap_probes` (sender.rs:2466),
    /// a surface INDEPENDENT of the decision list.
    probe_proof_bs_counter_delta: u64,

    // ── PARITY — three DISTINCT concepts, never conflated ───────────────────────────────────────
    /// MARK, CONCEPT 1 — `parity_pending_at_entry`: EXACT pending-parity depth at target-turn ENTRY,
    /// from `diag_fec_actuation()` (`enqueued_r_total − emitted_r_total − canceled_r_total`) read
    /// BEFORE the call. Parity WORK EXISTS. This is NOT actuation and NOT contention.
    parity_pending_at_entry: u64,
    /// MARK, CONCEPT 2 — `parity_actuated_in_target_turn`: a real `TransmitKind::FecParity` decision
    /// in the TARGET turn (the decision list). This is the ONLY thing that licenses the words
    /// "parity actuated"; a positive `parity_pending_at_entry` never does.
    parity_actuated_in_target_turn: bool,
    /// PROOF — `diag_fec_actuation().emitted_r_total` DELTA across the target `on_event`. Same
    /// physical event as `parity_actuated_in_target_turn` seen through a DIFFERENT surface (the
    /// product's own cumulative emission counter, sender.rs:3266), so the two can disagree.
    parity_proof_emitted_counter_delta: u64,
    /// PROOF — `diag_token_denial().1` (`token_denial_count_total`) DELTA across the target
    /// `on_event`. Written ONLY on the real token-denial path in `plan_fec_parity`
    /// (sender.rs:3472): the scheduler reached the parity queue front and the pacer refused it.
    /// This is the surface that proves parity CONTENDED in a turn where nothing was emitted.
    parity_proof_token_denial_delta: u64,
    /// PROOF context — `pending_parity_total` recorded in the last denial snapshot, when the denial
    /// delta above is positive. Diagnostic; the delta itself carries the proof.
    parity_proof_denied_pending_total: u64,

    // ── RECOVERY ────────────────────────────────────────────────────────────────────────────────
    /// MARK — a real `TransmitKind::Retransmit` decision in the target turn (the decision list).
    recovery_mark_retransmit_in_turn: bool,
    /// PROOF — a POSITIVE delta on the CUMULATIVE loss-declared counters `diag_loss().0/.1` across
    /// the target `on_event`.
    recovery_proof_loss_declared_delta: bool,
    /// PROOF — a STRICT DROP of the instantaneous `pending_lost` depth `diag_loss().2` across the
    /// target `on_event`: `plan_arq_retransmits` removes the record it retransmits
    /// (sender.rs:3919), so a falling depth is the loss ledger's own witness that the recovery
    /// machinery consumed work in THIS turn. Independent of the decision list.
    recovery_proof_pending_lost_drop: bool,
    /// `state()` is a recovery state after the target turn. Diagnostic only — never on its own an
    /// activation proof, since it can be carried in from a previous turn.
    recovery_state_after: bool,
}

impl ActivationObservation {
    /// PROOF: the SHIPPING bootstrap counter advanced in this turn. Independent of the CsProbe
    /// decision that marks the leg.
    fn probe_proved(self) -> bool {
        self.probe_proof_bs_counter_delta > 0
    }

    /// CONCEPT 3 — `parity_competed_for_paced_grant`: the parity scheduler was REACHED in this turn
    /// and the pacer answered it — parity either emitted (product emission counter advanced) or was
    /// REFUSED (token-denial event counter advanced).
    ///
    /// Deliberately reads NEITHER of the marking sources: not the entry depth, not the decision
    /// list. A cell can therefore carry `parity_pending_at_entry > 0` and still fail to show any
    /// competition, if the scheduler never reaches the parity queue in the target turn.
    ///
    /// **This is NOT parity-vs-DATA contention.** The product plans DATA/ARQ before
    /// `plan_fec_parity` in the same turn, so parity competes only for the RESIDUE of a grant DATA
    /// has already been offered; a denial here is downstream of the DATA decision.
    fn parity_competed_for_paced_grant(self) -> bool {
        self.parity_proof_emitted_counter_delta > 0 || self.parity_proof_token_denial_delta > 0
    }

    /// PROOF: the loss ledger moved across the same `on_event` — either loss was DECLARED
    /// (cumulative `.0`/`.1` advanced) or a pending-lost record was CONSUMED (`.2` fell).
    /// Deliberately reads no decision, so it can be false while a Retransmit was emitted.
    fn recovery_proved(self) -> bool {
        self.recovery_proof_loss_declared_delta || self.recovery_proof_pending_lost_drop
    }
}

// ── RULE-8 fingerprint ──────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Fingerprint {
    cell: &'static str,
    expected_grant_class: GrantClass,
    expected_token_predicate: TokenPredicate,
    /// Measurement point 1 — tokens BEFORE the call (pre-refill).
    call_entry_tokens: u64,
    /// Measurement point 2 — derived post-refill / pre-arbitration. AUTHORITATIVE for gate-5.
    decision_entry_tokens: u64,
    /// Measurement point 3 — post-turn residue. AUDIT ONLY; never an entry predicate.
    post_turn_tokens: u64,
    front_kind: &'static str,
    front_size: u64,
    burst_cap: u64,
    window_bytes: u64,
    observed_rate: u64,
    paced_debit_bytes: u64,
    baseline_probe_bytes: u64,
    available_data_grants: u64,
    actual_data_grants: u64,
    data_queued: u64,
    off_data_ids: BTreeSet<u64>,
    on_data_ids: BTreeSet<u64>,
    probe_due: bool,
    probe_emitted: bool,
    probe_in_baseline: bool,
    /// ATTRIBUTED origin of any control-arm probe (never inferred from `TransmitKind` or `cfg`).
    baseline_probe_source: BaselineProbeSource,
    recovery_active: bool,
    /// `parity_pending_at_entry > 0 || parity_actuated_in_target_turn` — "this cell carries parity
    /// work at all". It is the RULE-8 axis flag and is deliberately NOT called "parity active":
    /// pending-at-entry, actuated-in-turn and competed-for-a-paced-grant are three different facts,
    /// reported separately in [`ActivationObservation`] and never collapsed into this one bit.
    parity_pending_or_actuated: bool,
    /// The frozen per-cell parity claim-scope sentence. PRINT ONLY.
    parity_claim_scope: &'static str,
    /// The SUBSTANTIVE activation facts closure criterion (2) asserts over.
    activation: ActivationObservation,
    /// Human-readable rendering of `activation`. PRINT ONLY — never load-bearing.
    activation_proof: String,
    contention_non_vacuous: bool,
    verdict: CellVerdict,
    startup_pacing: bool,
}

impl Fingerprint {
    /// The INTENDED axes. Two cells sharing this tuple are one piece of evidence, not two.
    fn axes(&self) -> (bool, GrantClass, bool, bool) {
        (
            self.probe_due,
            GrantClass::of(self.available_data_grants),
            self.recovery_active,
            self.parity_pending_or_actuated,
        )
    }
}

/// RULE-8 fingerprint table — one row per cell, with the cap/front lineage OBSERVED PER CELL
/// (`burst_cap = 101` is never carried blindly into the parity/recovery cells).
fn print_fingerprint_table(rows: &[Fingerprint]) {
    eprintln!("\n RULE-8 FINGERPRINT TABLE (one row per cell; front kind/size and burst_cap OBSERVED per cell)");
    eprintln!(
        "{:<58} {:<20} {:>5} {:>5} {:>5} {:<26} {:>4} {:>6} {:<14} {:<14} {:<20} {:<36} {:>7} constructibility verdict",
        "CELL",
        "expected grant cls",
        "call",
        "dec",
        "post",
        "front kind/size",
        "cap",
        "debit",
        "OFF data ids",
        "ON data ids",
        "probe emission",
        "recov / parity(3 concepts)",
        "nonvac"
    );
    for f in rows {
        eprintln!(
            "{:<58} {:<20} {:>5} {:>5} {:>5} {:<26} {:>4} {:>6} {:<14} {:<14} {:<20} {:<36} {:>7} {}",
            f.cell,
            format!("{:?}", f.expected_grant_class),
            f.call_entry_tokens,
            f.decision_entry_tokens,
            f.post_turn_tokens,
            format!("{}/{}B", f.front_kind, f.front_size),
            f.burst_cap,
            f.paced_debit_bytes,
            format!("{:?}", f.off_data_ids),
            format!("{:?}", f.on_data_ids),
            format!("ON={} OFF={}", f.probe_emitted, f.probe_in_baseline),
            format!(
                "r={} p:pend={} act={} comp={}",
                f.recovery_active,
                f.activation.parity_pending_at_entry,
                f.activation.parity_actuated_in_target_turn,
                f.activation.parity_competed_for_paced_grant()
            ),
            f.contention_non_vacuous,
            f.verdict.label(),
        );
    }
    eprintln!(
        "  legend: call = call_entry_state (pre-refill) | dec = decision_entry_state (pre-arbitration, \
         AUTHORITATIVE) | post = post_turn_residue (AUDIT ONLY, never an entry predicate) | \
         debit = paced bytes excluding CsProbe"
    );
    eprintln!(
        "  parity legend: pend = parity_pending_at_entry (depth BEFORE the turn — work EXISTS) | \
         act = parity_actuated_in_target_turn (a real FecParity DECISION in the turn — the ONLY \
         licence for the word \"actuated\") | comp = parity_competed_for_paced_grant (the scheduler \
         reached the parity queue front and the pacer emitted or REFUSED). THREE DISTINCT FACTS; \
         `pend > 0` is NEVER reported as actuation, and none of them is parity-vs-DATA contention \
         (DATA/ARQ is planned BEFORE plan_fec_parity in the same turn)."
    );
    for f in rows {
        eprintln!(
            "  {:<58} startup_pacing={} rate={} window_bytes={} (< front_size={} ⇒ cap locks to the \
             front packet) data_queued_at_entry={} baseline_cs_probe_bytes={} \
             baseline_probe_source={} expected_token_predicate={:?} activation_proof: {}",
            f.cell,
            f.startup_pacing,
            f.observed_rate,
            f.window_bytes,
            f.front_size,
            f.data_queued,
            f.baseline_probe_bytes,
            f.baseline_probe_source.label(),
            f.expected_token_predicate,
            f.activation_proof
        );
        eprintln!(
            "  {:<58} activation MARKS  : probe_decision_in_turn={} | parity_pending_at_entry={} \
             parity_actuated_in_target_turn={} | retransmit_in_turn={} (recovery_state_after={} — \
             DIAGNOSTIC, never a proof)",
            "",
            f.activation.probe_mark_decision_in_turn,
            f.activation.parity_pending_at_entry,
            f.activation.parity_actuated_in_target_turn,
            f.activation.recovery_mark_retransmit_in_turn,
            f.activation.recovery_state_after,
        );
        eprintln!(
            "  {:<58} activation PROOFS : bs_probe_counter_delta={} | fec_emitted_counter_delta={} \
             token_denial_count_delta={} (denied_pending_total={}) | loss_declared_delta={} \
             pending_lost_drop={} ⇒ probe_proved={} parity_competed_for_paced_grant={} \
             recovery_proved={}",
            "",
            f.activation.probe_proof_bs_counter_delta,
            f.activation.parity_proof_emitted_counter_delta,
            f.activation.parity_proof_token_denial_delta,
            f.activation.parity_proof_denied_pending_total,
            f.activation.recovery_proof_loss_declared_delta,
            f.activation.recovery_proof_pending_lost_drop,
            f.activation.probe_proved(),
            f.activation.parity_competed_for_paced_grant(),
            f.activation.recovery_proved(),
        );
        eprintln!("  {:<58} CLAIM SCOPE       : {}", "", f.parity_claim_scope);
    }
}

// ── regime-cell runner ──────────────────────────────────────────────────────────────────────────

struct MatrixResult {
    fingerprints: Vec<Fingerprint>,
    reds: Vec<String>,
}

impl MatrixResult {
    fn count(&self, v: CellVerdict) -> usize {
        self.fingerprints.iter().filter(|f| f.verdict == v).count()
    }
}

#[allow(clippy::too_many_lines)]
fn run_regime_cells(startup_pacing: bool, label: &str) -> MatrixResult {
    let mut fingerprints = Vec::new();
    let mut reds: Vec<String> = Vec::new();

    eprintln!("\n===== {label} (startup_pacing={startup_pacing}) =====");

    for c in &REGIME_CELLS {
        let mut off = build_arm(false, startup_pacing, c.units, c.parity);
        let mut on = build_arm(true, startup_pacing, c.units, c.parity);

        // ── MEASUREMENT POINT 1 — call_entry_state (per cell; never carried between cells) ───────
        let call_entry_tokens = off.tokens;
        let call_entry_cap = off.burst_cap;
        let observed_rate = off.rate;

        // ── PRECONDITION 1: paired arms identical at the PRE-TREATMENT snapshot ──────────────────
        let off_pre = input(&off.s, &off.pre);
        let on_pre = input(&on.s, &on.pre);
        let arms_identical = off_pre == on_pre;
        if !arms_identical {
            reds.push(format!(
                "{}: arms NOT identical at pre-treatment snapshot: off={off_pre:?} on={on_pre:?}",
                c.id
            ));
        }

        // ── MEASUREMENT POINT 2 — decision_entry_state (derived; NEVER read post-call) ───────────
        let entry_off = off.derive_entry_tokens(c.target_ns);
        let entry_on = on.derive_entry_tokens(c.target_ns);
        if entry_off != entry_on {
            reds.push(format!(
                "{}: derived entry_tokens differ between arms (off={entry_off} on={entry_on})",
                c.id
            ));
        }
        let decision_entry_tokens = entry_off;
        let burst_cap = call_entry_cap;
        let data_grant_cost = UNIT;
        let available_data_grants = decision_entry_tokens / data_grant_cost;
        let observed_class = GrantClass::of(available_data_grants);
        let literal_zero_tokens = decision_entry_tokens == 0;
        let zero_data_grant_capacity = decision_entry_tokens < UNIT;

        // ── PER-CELL CAP LINEAGE — THREE INDEPENDENTLY OBTAINED NUMBERS ──────────────────────────
        // `burst_cap`   : diag_pacer().2                       (product pacer state)
        // `window_bytes`: diag_pacer().0 * BURST_WINDOW_NS/1e9 (product pacer RATE — a different field)
        // `front_size`  : the harness's OWN enqueue ledger     (independent of the pacer entirely)
        // The previous revision set `front_size = burst_cap`, which made criterion (4)'s clamp
        // identity a value compared with itself and detected nothing. It is now a real cross-check.
        let window_bytes = observed_rate.saturating_mul(BURST_WINDOW_NS) / 1_000_000_000;
        let (front_size, front_kind) = off.observed_front();

        eprintln!(
            "\n[{}] target_ns={} burst_cap={burst_cap} data_grant_cost={data_grant_cost}",
            c.id, c.target_ns
        );
        eprintln!("  required_activation      : {}", c.required_activation);
        eprintln!(
            "  cap lineage (OBSERVED)   : rate={observed_rate} window_bytes={window_bytes} \
             front_kind={front_kind} front_size={front_size} burst_cap={burst_cap} \
             burst_hi=max(front_size, 2400)"
        );
        eprintln!(
            "  expected_token_predicate : {:?} (expected entry_tokens={})",
            c.expected_token_predicate, c.expected_entry_tokens
        );
        let observed_predicate = TokenPredicate::observed(decision_entry_tokens);
        eprintln!(
            "  call_entry_tokens        : {call_entry_tokens}  (measurement point 1 — pre-refill)"
        );
        eprintln!(
            "  decision_entry_tokens    : {decision_entry_tokens}  LiteralZeroTokens={literal_zero_tokens}  ZeroDataGrantCapacity={zero_data_grant_capacity}  (measurement point 2 — AUTHORITATIVE)"
        );
        eprintln!(
            "  observed_token_predicate : {observed_predicate:?}  (LiteralZeroTokens vs \
             ZeroDataGrantCapacity are DISTINCT concepts; here tokens={decision_entry_tokens})"
        );
        if observed_predicate.is_none() {
            reds.push(format!(
                "{}: decision_entry_tokens={decision_entry_tokens} exceeds burst_cap — unclassifiable token predicate",
                c.id
            ));
        }
        eprintln!("  available_data_grants    : {available_data_grants}");
        eprintln!(
            "  {} expected={:?} observed={:?}",
            c.id, c.expected_grant_class, observed_class
        );

        if observed_rate != FLOOR_BPS {
            reds.push(format!(
                "{}: PreConverge floor rate expected={FLOOR_BPS} observed={observed_rate}",
                c.id
            ));
        }
        if burst_cap != EXPECTED_BURST_CAP {
            reds.push(format!(
                "{}: burst_cap expected={EXPECTED_BURST_CAP} observed={burst_cap}",
                c.id
            ));
        }
        if window_bytes >= front_size {
            reds.push(format!(
                "{}: cap lineage BROKEN — window_bytes={window_bytes} >= front_size={front_size}; \
                 the clamp no longer locks to the front packet and burst_cap can no longer be read \
                 as front_size",
                c.id
            ));
        } else if burst_cap != front_size {
            // A GENUINE cross-check now that `front_size` no longer comes from `burst_cap`:
            // `window_bytes < front_size` forces `clamp(window_bytes, front_size, burst_hi)` to
            // return `front_size`, so the PRODUCT's cap must equal the INDEPENDENTLY OBSERVED front.
            reds.push(format!(
                "{}: cap lineage BROKEN — window_bytes={window_bytes} < front_size={front_size} \
                 (observed from the harness's own enqueue ledger) forces the clamp to lock at \
                 front_size, but the product's burst_cap is {burst_cap}",
                c.id
            ));
        }
        if decision_entry_tokens != c.expected_entry_tokens {
            reds.push(format!(
                "{}: decision_entry_tokens expected={} observed={decision_entry_tokens}",
                c.id, c.expected_entry_tokens
            ));
        }
        if !c.expected_token_predicate.holds(decision_entry_tokens) {
            reds.push(format!(
                "{}: token predicate expected={:?} does NOT hold at decision_entry_tokens={decision_entry_tokens}",
                c.id, c.expected_token_predicate
            ));
        }
        if observed_class != c.expected_grant_class {
            reds.push(format!(
                "{}: expected={:?} observed={:?} (grant class)",
                c.id, c.expected_grant_class, observed_class
            ));
        }

        // ── PRECONDITION 2: DATA queued / eligible at target-turn ENTRY ─────────────────────────
        let data_queued = off.queued_data();
        if data_queued == 0 {
            reds.push(format!("{}: no DATA queued at target-turn entry", c.id));
        }
        if data_queued > 0 && front_kind != "Data(OpaqueUnit)" {
            reds.push(format!(
                "{}: the enqueue ledger reports {data_queued} DATA unit(s) still queued but its \
                 front entry is {front_kind}/{front_size}B — the ledger is internally inconsistent \
                 and cannot be used as the independent front observation",
                c.id
            ));
        }
        // ── PRECONDITION 3: parity pending at target-turn ENTRY (cumulative totals are NOT used)
        // CONCEPT 1 / MARKING source for the parity leg — `diag_fec_actuation()` depth read BEFORE
        // the call. "Parity work EXISTS"; this is NOT actuation and NOT contention.
        let parity_pending_at_entry = off.pending_fec();
        // PROVING sources for the parity leg, sampled BEFORE the call so only the target turn's
        // movement is counted. Both are surfaces the marking source does not touch.
        let fec_emitted_before = off.s.diag_fec_actuation().emitted_r_total;
        let token_denials_before = off.s.diag_token_denial().1;

        let loss_before = off.s.diag_loss();
        let state_before = off.s.state();
        // Shipping bootstrap counters BEFORE the target turn, on BOTH arms. The treatment arm's
        // delta is criterion (2)'s probe proof; the CONTROL arm's is the D1 discriminator that
        // separates a leaked SHIPPING probe from the legacy ladder.
        let on_bs_probe_before = on.s.bootstrap_rtt_diag().probe_emitted;
        let off_bs_probe_before = off.s.bootstrap_rtt_diag().probe_emitted;

        // ── THE TARGET TURN — one event-free jump, delivered identically to both arms ────────────
        let off_out = off.advance(Event::TimerFired(Timer::Pto), c.target_ns);
        let on_out = on.advance(Event::TimerFired(Timer::Pto), c.target_ns);

        // ── MEASUREMENT POINT 3 — post_turn_residue (AUDIT ONLY) ────────────────────────────────
        let post_turn_tokens = off.tokens;
        let cap_after = off.burst_cap;
        let paced_debit_bytes = paced_bytes_excluding_cs_probe(&off_out);
        let baseline_probe_bytes = cs_probe_bytes(&off_out);
        let probe_in_baseline = baseline_probe_bytes > 0;
        let on_bs_probe_delta =
            on.s.bootstrap_rtt_diag()
                .probe_emitted
                .saturating_sub(on_bs_probe_before);
        // THE D1 DISCRIMINATOR. `drain_bootstrap_probes` increments `bs_diag.probe_emitted`
        // (sender.rs:2466); `drain_cs_probes` NEVER touches `bs_diag` (sender.rs:2065-2095 — it
        // increments `cs_probe_emitted` instead). A non-zero value on the bootstrap-OFF arm can
        // therefore only be the SHIPPING path firing in the CONTROL arm.
        let off_bs_probe_delta = off
            .s
            .bootstrap_rtt_diag()
            .probe_emitted
            .saturating_sub(off_bs_probe_before);
        let off_bs_probe_total = off.s.bootstrap_rtt_diag().probe_emitted;
        let off_legacy_probe_emitted = legacy_cs_probe_emitted(&off.s);

        // `derive_entry_tokens` applies ONE clamp with the PREVIOUS turn's cap; that is exact only
        // while old cap == new cap. Assert it so the assumption breaks loudly if it ever changes.
        if cap_after != call_entry_cap {
            reds.push(format!(
                "{}: burst_cap MOVED inside the target turn ({call_entry_cap} -> {cap_after}); the \
                 single-clamp entry derivation is no longer exact and must be re-derived",
                c.id
            ));
        }

        // ── RECONCILIATION — closes every token mutation by CODE LINEAGE ─────────────────────────
        // decision_entry_tokens = post_turn_tokens + paced_debit + (legacy CsProbe debit, if any).
        let recon_excl_probe = post_turn_tokens + paced_debit_bytes;
        let residual = i128::from(decision_entry_tokens) - i128::from(recon_excl_probe);
        let legacy_debit_closes = residual == i128::from(baseline_probe_bytes);
        let mut harness_limited = false;
        if !legacy_debit_closes {
            harness_limited = true;
            reds.push(format!(
                "{}: HARNESS-LIMITED — reconciliation does NOT close: decision_entry={decision_entry_tokens} \
                 post_turn={post_turn_tokens} paced_excl_probe={paced_debit_bytes} \
                 baseline_cs_probe_bytes={baseline_probe_bytes} residual={residual}",
                c.id
            ));
        }

        // The TREATMENT arm must reconcile with residual EXACTLY 0 even though it emits probes —
        // that IS the DR-CS-3 claim: `drain_bootstrap_probes` is ADDITIVE and UNPACED
        // (sender.rs:2425-2434, no `try_send` / `try_send_pre_parity` call on that path).
        let on_recon = on.tokens + paced_bytes_excluding_cs_probe(&on_out);
        let on_probe_bytes = cs_probe_bytes(&on_out);
        if on_recon != decision_entry_tokens {
            reds.push(format!(
                "PRODUCT FINDING - the TREATMENT arm debited the pacer :: {}: decision_entry={decision_entry_tokens} \
                 post_turn={} paced_excl_probe={} probe_bytes={on_probe_bytes} — the bootstrap probe \
                 is supposed to be additive/unpaced",
                c.id,
                on.tokens,
                paced_bytes_excluding_cs_probe(&on_out)
            ));
        }

        // ── CONFOUND DETECTION — ATTRIBUTED from the two paths' DISJOINT PRODUCT COUNTERS ────────
        // The kind and the wire size are IDENTICAL for both emitters (CS_PROBE_WIRE_MAX = 44,
        // sender.rs:194 == BS_PROBE_WIRE_EXACT = 44, sender.rs:597), and the reconciliation residual
        // closes identically either way — so neither the decision nor the arithmetic can attribute
        // the probe. Only the counters can, and they are read regardless of `cfg`.
        let baseline_probe_source = classify_baseline_probe(
            baseline_probe_bytes,
            off_bs_probe_total,
            off_legacy_probe_emitted,
        );
        let confounded_baseline =
            baseline_probe_source == BaselineProbeSource::LegacyColdStartLadder;
        if probe_in_baseline {
            eprintln!(
                "  control-arm CsProbe      : {baseline_probe_bytes} B in the bootstrap-OFF arm; \
                 ATTRIBUTION={} (off-arm bootstrap_rtt_diag().probe_emitted total={off_bs_probe_total} \
                 delta_in_turn={off_bs_probe_delta}; legacy cold-start diag \
                 cs_probe_emitted={off_legacy_probe_emitted:?})",
                baseline_probe_source.label()
            );
        }
        match baseline_probe_source {
            BaselineProbeSource::ShippingBootstrapLeak => {
                // NEVER classified CONFOUNDED, in ANY feature configuration. This is the exact case
                // the old `probe_in_baseline`-only rule absorbed: a regression in the treatment's own
                // gate (`if !self.bootstrap_rtt.enabled || self.bs_done { return; }`, sender.rs:2436)
                // presents with the same kind, the same 44 B and a closing residual.
                reds.push(format!(
                    "PRODUCT FINDING - the SHIPPING bootstrap treatment fired in the CONTROL arm :: \
                     {} :: the bootstrap-OFF arm's bootstrap_rtt_diag().probe_emitted is \
                     {off_bs_probe_total} (delta {off_bs_probe_delta} in the target turn) — only \
                     drain_bootstrap_probes (sender.rs:2466) increments that counter; drain_cs_probes \
                     (sender.rs:2065-2095) never touches bs_diag. The treatment leaked past its own \
                     gate at sender.rs:2436 into the control arm ({baseline_probe_bytes} B of \
                     CsProbe). This is NOT a feature-configuration confound and is NEVER excused.",
                    c.id
                ));
            }
            BaselineProbeSource::Unattributed => {
                reds.push(format!(
                    "PRODUCT FINDING - an UNATTRIBUTED CsProbe was emitted in the bootstrap-OFF \
                     control arm :: {} :: {baseline_probe_bytes} B, but no product counter claims it \
                     (bootstrap_rtt_diag().probe_emitted={off_bs_probe_total}, legacy cold-start \
                     cs_probe_emitted={off_legacy_probe_emitted:?} — None means the legacy ladder is \
                     not compiled in this build). The CONFOUNDED-BASELINE-CONFIG excuse requires a \
                     PROVEN legacy lineage and is refused here.",
                    c.id
                ));
            }
            BaselineProbeSource::LegacyColdStartLadder => {
                eprintln!(
                    "  CONFOUNDED-BASELINE-CONFIG: the control (bootstrap-OFF) arm emitted \
                     {baseline_probe_bytes} B of CsProbe, ATTRIBUTED to the LEGACY ladder."
                );
                eprintln!(
                    "    lineage : sender.rs:2996-2999  #[cfg(feature = \"experimental-cold-start\")] \
                     if !self.bootstrap_rtt.enabled {{ self.drain_cs_probes(&mut out); }}"
                );
                eprintln!(
                    "            : the LEGACY ladder fires exactly when the bootstrap treatment is \
                     DISABLED, i.e. in the BASELINE arm; drain_cs_probes (sender.rs:2065, body +7) \
                     calls self.pacer.try_send(CS_PROBE_WIRE_MAX = {LEGACY_CS_PROBE_WIRE_MAX}) at \
                     sender.rs:2071 and DEBITS {LEGACY_CS_PROBE_WIRE_MAX} tokens."
                );
                eprintln!(
                    "    ATTRIB. : off-arm bootstrap_rtt_diag().probe_emitted={off_bs_probe_total} \
                     (== 0 ⇒ the SHIPPING path did NOT fire here) AND the legacy counter \
                     cs_probe_emitted={off_legacy_probe_emitted:?} (> 0 ⇒ the LEGACY path DID). The \
                     attribution is made from the counters, NOT from the decision kind, NOT from the \
                     44 B size and NOT from cfg — all three are identical for both emitters."
                );
                eprintln!(
                    "            : reconciliation residual={residual} closes EXACTLY with the observed \
                     baseline probe bytes ({baseline_probe_bytes}) ⇒ the token delta is the legacy \
                     ladder's paced debit, not a treatment effect."
                );
                eprintln!(
                    "    verdict : the paired 'additive treatment vs nothing' premise DOES NOT HOLD in \
                     this feature configuration — the control arm runs a competing PACED ladder. This \
                     cell is EXCLUDED from the DATA-displacement denominator and the claim scope names \
                     the feature axis. It is neither RED nor silently skipped."
                );
                if !baseline_probe_bytes.is_multiple_of(LEGACY_CS_PROBE_WIRE_MAX) {
                    reds.push(format!(
                        "{}: baseline probe bytes {baseline_probe_bytes} are NOT a multiple of the \
                         legacy ladder's CS_PROBE_WIRE_MAX={LEGACY_CS_PROBE_WIRE_MAX}; the confound \
                         lineage does not close",
                        c.id
                    ));
                }
            }
            BaselineProbeSource::None => {
                if cfg!(feature = "experimental-cold-start") {
                    eprintln!(
                        "  legacy-ladder note       : experimental-cold-start IS compiled, but the \
                         control arm emitted no CsProbe in this turn (token-denied or step-not-due) \
                         ⇒ not confounded."
                    );
                }
                if off_bs_probe_total > 0 {
                    reds.push(format!(
                        "PRODUCT FINDING - the SHIPPING bootstrap path advanced its counter in the \
                         CONTROL arm :: {} :: bootstrap-OFF arm bootstrap_rtt_diag().probe_emitted \
                         ={off_bs_probe_total} with no CsProbe decision to match",
                        c.id
                    ));
                }
            }
        }

        // ── ACTIVATION, proven from the TARGET turn only ────────────────────────────────────────
        let actual_data_grants = off_out
            .iter()
            .filter(|d| d.kind == TransmitKind::Data)
            .count() as u64;
        let off_data_ids = data_identities(&off_out);
        let on_data_ids = data_identities(&on_out);
        if actual_data_grants != available_data_grants {
            let data_bytes: u64 = off_out
                .iter()
                .filter(|d| d.kind == TransmitKind::Data)
                .map(|d| d.size)
                .sum();
            reds.push(format!(
                "{}: actual_data_grants={actual_data_grants} != available_data_grants={available_data_grants} \
                 (decision_entry={decision_entry_tokens}, paced_debit={paced_debit_bytes}, of which \
                 DATA={data_bytes}, non-DATA={}) — a token grant the control arm could have spent on \
                 DATA went elsewhere; scheduler-order lineage must be recorded before this is called \
                 a pass",
                c.id,
                paced_debit_bytes - data_bytes
            ));
        }

        let probe_emitted = on_out.iter().any(|d| d.kind == TransmitKind::CsProbe);
        if probe_emitted != c.expected_probe_due {
            reds.push(format!(
                "{}: probe expected={} observed={probe_emitted}",
                c.id, c.expected_probe_due
            ));
        }

        // ── RECOVERY — MARK from the decision list, PROVE from the loss ledger ──────────────────
        let loss_after = off.s.diag_loss();
        // PROOF #1: the CUMULATIVE loss-declared counters moved in this `on_event`.
        let loss_declared_delta = loss_after.0 > loss_before.0 || loss_after.1 > loss_before.1;
        // PROOF #2: a pending-lost record was CONSUMED in this `on_event` (`plan_arq_retransmits`
        // removes what it retransmits, sender.rs:3919), so the instantaneous depth strictly fell.
        let pending_lost_drop = loss_after.2 < loss_before.2;
        let recovery_state = matches!(
            off.s.state(),
            SenderState::RecoverArq | SenderState::RecoverFwd
        );
        // MARK: a real `TransmitKind::Retransmit` in the target turn — the DECISION LIST alone. The
        // previous marking `retx || (loss_delta && recovery_state)` shared `loss_delta` with the
        // proof, so a cell marked through that branch proved itself by construction.
        let retx_in_turn = off_out.iter().any(|d| d.kind == TransmitKind::Retransmit);
        let recovery_active = retx_in_turn;
        // Nothing that was previously folded into the marking expression is dropped: no cell in the
        // frozen matrix injects loss, so BOTH recovery surfaces must read silent. A lone loss
        // declaration or a carried-in recovery state used to be absorbed by the old disjunction; it
        // is now surfaced on its own.
        if loss_declared_delta || pending_lost_drop || recovery_state || loss_after.2 > 0 {
            reds.push(format!(
                "{}: the CONTROL arm shows recovery-ledger movement in a cell that injects no loss \
                 — loss_declared_delta={loss_declared_delta} pending_lost_drop={pending_lost_drop} \
                 recovery_state_after={recovery_state} pending_lost_depth_after={} \
                 (before={:?} after={loss_after:?})",
                c.id, loss_after.2, loss_before
            ));
        }

        // ── PARITY — MARK from entry depth / decision list, PROVE from the product counters ──────
        // CONCEPT 2 — a real FecParity DECISION in the TARGET turn. The ONLY licence for the words
        // "parity actuated". `parity_pending_at_entry > 0` is NEVER reported as actuation.
        let parity_actuated_in_target_turn =
            off_out.iter().any(|d| d.kind == TransmitKind::FecParity);
        // The RULE-8 AXIS flag: "this cell carries parity work at all" = concept 1 OR concept 2.
        // Deliberately NOT named `parity_active` — that name asserted actuation and contention it
        // never measured. A cumulative `enqueued_r_total > 0` alone is still not used.
        let parity_pending_or_actuated =
            parity_pending_at_entry > 0 || parity_actuated_in_target_turn;
        // CONCEPT 3 PROOF: movement of surfaces the marking sources do not read — the product's own
        // cumulative parity EMISSION counter, and the token-DENIAL event counter that only
        // `plan_fec_parity` writes when the pacer refuses the parity queue front. Together they say
        // "the scheduler actually reached the parity queue and the pacer answered in THIS turn"
        // (`parity_competed_for_paced_grant`), which neither marking source can establish on its
        // own — and which is still NOT parity-vs-DATA contention, since DATA/ARQ is planned first.
        let fec_emitted_delta = off
            .s
            .diag_fec_actuation()
            .emitted_r_total
            .saturating_sub(fec_emitted_before);
        let (last_denial, token_denials_after) = off.s.diag_token_denial();
        let token_denial_delta = token_denials_after.saturating_sub(token_denials_before);
        let denied_pending_total = if token_denial_delta > 0 {
            last_denial.map_or(0, |d| d.pending_parity_total)
        } else {
            0
        };
        if parity_pending_or_actuated != c.expect_parity_pending_or_actuated {
            reds.push(format!(
                "{}: parity pending-or-actuated expected={} observed={parity_pending_or_actuated} \
                 (parity_pending_at_entry={parity_pending_at_entry}, \
                 parity_actuated_in_target_turn={parity_actuated_in_target_turn})",
                c.id, c.expect_parity_pending_or_actuated
            ));
        }
        // The DECISION LIST and the product's cumulative emission counter are two views of the same
        // physical event; a disagreement means one of them is not measuring what it claims, so the
        // parity leg of criterion (2) would be resting on an unproven source.
        if parity_actuated_in_target_turn && fec_emitted_delta == 0 {
            reds.push(format!(
                "{}: FecParity decision(s) were emitted in the TARGET turn but \
                 diag_fec_actuation().emitted_r_total did NOT advance — the decision list and the \
                 product's parity emission counter disagree",
                c.id
            ));
        }
        if !parity_actuated_in_target_turn && fec_emitted_delta > 0 {
            reds.push(format!(
                "{}: diag_fec_actuation().emitted_r_total advanced by {fec_emitted_delta} in the \
                 TARGET turn but no FecParity decision was emitted — the counter and the decision \
                 list disagree",
                c.id
            ));
        }

        // Every ACTIVE cell must prove its activation IN THE TARGET TURN, not from a cumulative
        // counter and not from a prior turn. The proof is the STRUCTURED OBSERVATION below;
        // `activation_proof` is only its rendering. A guard on the rendering's non-emptiness would
        // be unreachable — `format!` over literal text can never produce an empty string — so the
        // criterion asserts over `ActivationObservation` instead (closure criterion (2)).
        let activation = ActivationObservation {
            probe_mark_decision_in_turn: probe_emitted,
            probe_proof_bs_counter_delta: on_bs_probe_delta,
            parity_pending_at_entry,
            parity_actuated_in_target_turn,
            parity_proof_emitted_counter_delta: fec_emitted_delta,
            parity_proof_token_denial_delta: token_denial_delta,
            parity_proof_denied_pending_total: denied_pending_total,
            recovery_mark_retransmit_in_turn: retx_in_turn,
            recovery_proof_loss_declared_delta: loss_declared_delta,
            recovery_proof_pending_lost_drop: pending_lost_drop,
            recovery_state_after: recovery_state,
        };
        let activation_proof = format!(
            "probe MARK decision_in_target_turn={probe_emitted} / PROOF \
             bootstrap_rtt_diag().probe_emitted delta={on_bs_probe_delta}; \
             parity MARK parity_pending_at_entry={parity_pending_at_entry} \
             parity_actuated_in_target_turn={parity_actuated_in_target_turn} \
             / PROOF emitted_r_total delta={fec_emitted_delta} token_denial_count delta={token_denial_delta}; \
             recovery MARK retx_in_target_turn={retx_in_turn} / PROOF \
             loss_declared_delta={loss_declared_delta} pending_lost_drop={pending_lost_drop} \
             (recovery_state_after={recovery_state} — diagnostic only)"
        );
        // The DECISION and the SHIPPING COUNTER are independent sources for the same event; if they
        // ever disagree the probe leg of criterion (2) is unproven, so the disagreement is RED.
        if probe_emitted && on_bs_probe_delta == 0 {
            reds.push(format!(
                "{}: a CsProbe decision was emitted in the treatment arm's TARGET turn but \
                 bootstrap_rtt_diag().probe_emitted did NOT advance (delta=0) — the emission is not \
                 attributable to the shipping bootstrap path and cannot prove activation",
                c.id
            ));
        }
        if !probe_emitted && on_bs_probe_delta > 0 {
            reds.push(format!(
                "{}: bootstrap_rtt_diag().probe_emitted advanced by {on_bs_probe_delta} in the \
                 treatment arm's TARGET turn but no CsProbe decision was emitted — the counter and \
                 the decision list disagree",
                c.id
            ));
        }

        // ── CONTENTION NON-VACUITY — a first-class closure criterion, computed on EVERY cell ─────
        // A cell is DATA-displacement evidence ONLY if the CONTROL arm actually took a DATA grant
        // AND emitted a DATA identity. Below 101 entry tokens the control arm can use no DATA
        // grant, so there is nothing for a Probe to displace.
        let contention_non_vacuous = actual_data_grants >= 1 && !off_data_ids.is_empty();

        eprintln!(
            "  actual_data_grants       : {actual_data_grants}   data_queued={data_queued}   state_before={state_before:?} state_after={:?}",
            off.s.state()
        );
        eprintln!("  OFF DATA identities      : {off_data_ids:?}");
        eprintln!("  ON  DATA identities      : {on_data_ids:?}");
        eprintln!(
            "  probe due/emitted        : {}/{probe_emitted}   in-baseline={probe_in_baseline}   bootstrap_diag={:?}",
            c.expected_probe_due,
            on.s.bootstrap_rtt_diag().probe_emitted
        );
        eprintln!(
            "  recovery active in turn  : {recovery_active} (MARK retx_in_turn={retx_in_turn} | \
             PROOF loss_declared_delta={loss_declared_delta} pending_lost_drop={pending_lost_drop} \
             | diagnostic recovery_state={recovery_state})"
        );
        eprintln!(
            "  parity (3 concepts)      : parity_pending_at_entry={parity_pending_at_entry} \
             parity_actuated_in_target_turn={parity_actuated_in_target_turn} \
             parity_competed_for_paced_grant={} \
             (axis parity_pending_or_actuated={parity_pending_or_actuated} | PROOF \
             emitted_r_total_delta={fec_emitted_delta} token_denial_delta={token_denial_delta} \
             denied_pending_total={denied_pending_total})",
            fec_emitted_delta > 0 || token_denial_delta > 0
        );
        eprintln!("  parity CLAIM SCOPE       : {}", c.parity_claim_scope);
        eprintln!(
            "  post_turn_residue        : {post_turn_tokens} (measurement point 3 — AUDIT ONLY; \
             NEVER an entry predicate)"
        );
        eprintln!(
            "  reconciliation           : {decision_entry_tokens} = {post_turn_tokens} + {paced_debit_bytes} + {baseline_probe_bytes} (residual={residual})"
        );
        eprintln!("  contention_non_vacuous   : {contention_non_vacuous}");

        // ── THE ORACLE — one shared comparator, verdict derived not hard-coded ───────────────────
        let off_oracle_in = input(&off.s, &off_out);
        let on_oracle_in = input(&on.s, &on_out);
        let rep = evaluate_grant_displacement(&off_oracle_in, &on_oracle_in);
        eprintln!(
            "  ORACLE                   : displaced_data_ids={:?} direct={} missing={} order={} length={} pacer_tokens={} pacer_burst_cap={} pacer_rate={} inflight={} sent_logical={} violation={}",
            rep.displaced_data_ids,
            rep.direct_displacement_count,
            rep.missing_or_replaced_ids.len(),
            rep.order_divergence,
            rep.length_divergence,
            rep.pacer_tokens_divergence,
            rep.pacer_burst_cap_divergence,
            rep.pacer_rate_divergence,
            rep.inflight_state_divergence,
            rep.sent_logical_divergence,
            rep.violation
        );
        if rep.violation {
            let non_vacuous = data_queued > 0 && (probe_emitted || parity_pending_or_actuated);
            let predicates_ok = decision_entry_tokens == c.expected_entry_tokens
                && observed_class == c.expected_grant_class
                && probe_emitted == c.expected_probe_due;
            // MISFIRE RECORD: this branch fired ONCE, wrongly, on `cell_2` under
            // `--features experimental-cold-start`. There the control arm runs the LEGACY paced CS
            // ladder (sender.rs:2996-2999 → drain_cs_probes → pacer.try_send(44)), so the observed
            // pacer_state_divergence was a KNOWN feature-configuration confound, not a shipped
            // product violation. The label "PRODUCT FINDING" must be reserved for a genuine shipped
            // violation, so the escalation now additionally requires that the cell shows no probe in
            // the baseline arm, is not harness-limited and is not confounded. The branch stays LIVE
            // and REACHABLE for a real violation.
            if startup_pacing
                && predicates_ok
                && non_vacuous
                && arms_identical
                && !probe_in_baseline
                && !harness_limited
                && !confounded_baseline
            {
                reds.push(format!(
                    "PRODUCT FINDING - structural pacer/grant invariant does not hold in the \
                     shipped configuration :: {} :: {rep:?}",
                    c.id
                ));
            } else if confounded_baseline {
                // The excuse is NARROWED to the lineage it proves. `pacer.try_send(44)`
                // (pacer.rs:59-66) is a compare-and-subtract on `tokens` ONLY, so:
                //   (a) burst_cap or rate divergence is OUTSIDE the lineage        -> RED
                //   (b) any non-pacer dimension is OUTSIDE the lineage             -> RED
                //   (c) the token divergence must MATCH THE MAGNITUDE of the debit -> else RED
                // (c) is what the old branch merely ASSERTED in its printed message without ever
                // checking it.
                let token_delta =
                    i128::from(on_oracle_in.tokens) - i128::from(off_oracle_in.tokens);
                let magnitude_matches = token_delta == i128::from(baseline_probe_bytes);
                if rep.diverges_beyond_pacer_tokens() {
                    reds.push(format!(
                        "{}: CONFOUNDED-BASELINE-CONFIG cell diverges BEYOND the pacer TOKEN \
                         component — the legacy-ladder debit moves `tokens` and nothing else, so a \
                         burst_cap / rate / plan / inflight / sent-logical divergence cannot be \
                         explained by it :: pacer_burst_cap={} pacer_rate={} :: {rep:?}",
                        c.id, rep.pacer_burst_cap_divergence, rep.pacer_rate_divergence
                    ));
                } else if !magnitude_matches {
                    reds.push(format!(
                        "{}: CONFOUNDED-BASELINE-CONFIG cell's TOKEN divergence MAGNITUDE does not \
                         equal the legacy ladder's debit — ON tokens {} - OFF tokens {} = \
                         {token_delta}, but the control arm's observed probe bytes are \
                         {baseline_probe_bytes}. The excuse claims that equality, so it must hold \
                         :: {rep:?}",
                        c.id, on_oracle_in.tokens, off_oracle_in.tokens
                    ));
                } else {
                    eprintln!(
                        "  violation CLASSIFIED as CONFOUNDED-BASELINE-CONFIG: the divergence is \
                         confined to the pacer TOKEN component (burst_cap divergence={}, rate \
                         divergence={}) and its MAGNITUDE is CHECKED, not merely asserted: ON \
                         tokens {} - OFF tokens {} = {token_delta} == the legacy ladder's \
                         {baseline_probe_bytes} B debit in the control arm. NOT RED, NOT SKIPPED, \
                         EXCLUDED from the DATA-displacement denominator.",
                        rep.pacer_burst_cap_divergence,
                        rep.pacer_rate_divergence,
                        on_oracle_in.tokens,
                        off_oracle_in.tokens
                    );
                }
            } else {
                reds.push(format!("{}: displacement violation :: {rep:?}", c.id));
            }
        }

        // Probe may only be an append-only suffix and must carry no send record.
        for p in on_out.iter().filter(|d| d.kind == TransmitKind::CsProbe) {
            if p.send_record.is_some() {
                reds.push(format!("{}: probe carries a send_record", c.id));
            }
        }
        let n = normal_plan(&on_out).len();
        if !on_out[..n].iter().all(|d| d.kind != TransmitKind::CsProbe) {
            reds.push(format!(
                "{}: probes must be appended AFTER all normal packets",
                c.id
            ));
        }

        // ── EXACTLY ONE constructibility verdict per cell ────────────────────────────────────────
        let verdict = if confounded_baseline {
            CellVerdict::ConfoundedBaselineConfig
        } else if harness_limited {
            CellVerdict::CellUnconstructibleHarnessLimitation
        } else if contention_non_vacuous {
            CellVerdict::ContentionTesting
        } else if parity_actuated_in_target_turn {
            // MEASURED, not frozen: parity really ran in this turn, but the control arm took no DATA
            // grant, so there was nothing for it to displace. Distinguished from the generic
            // state-active label so "parity actuated" is never claimed for a cell that only had
            // parity PENDING AT ENTRY. Still OUT of the Gate-5 denominator — only ContentionTesting
            // cells are in it, and that ordering is unchanged.
            CellVerdict::ParityActuatedButVacuousForDataDisplacement
        } else if recovery_active || parity_pending_or_actuated {
            CellVerdict::StateActiveButVacuousForDataDisplacement
        } else if probe_emitted {
            CellVerdict::ActivatedButVacuousForDataDisplacement
        } else {
            CellVerdict::InertTreatmentVacuousForDataDisplacement
        };
        eprintln!("  CONSTRUCTIBILITY VERDICT : {}", verdict.label());
        if verdict == CellVerdict::ParityActuatedButVacuousForDataDisplacement {
            eprintln!(
                "    parity was ACTUATED in the target turn (a real FecParity decision, \
                 emitted_r_total delta={fec_emitted_delta}) but the control arm took NO DATA grant, \
                 so nothing existed for it to displace. This is a parity-ACTUATION control. It is \
                 NOT in the Gate-5 denominator and it is NOT evidence of parity-vs-DATA contention."
            );
        }
        if verdict == CellVerdict::InertTreatmentVacuousForDataDisplacement {
            eprintln!(
                "    the treatment is ENTIRELY INERT in this cell (probe_emitted=false, by design: \
                 expected_probe_due={}) and the control arm emits no DATA — it is a negative \
                 control for probe activation, and contributes NOTHING to the DATA-displacement \
                 claim.",
                c.expected_probe_due
            );
        }
        if !contention_non_vacuous {
            eprintln!(
                "    VACUOUS FOR DATA DISPLACEMENT: decision_entry_tokens={decision_entry_tokens} < \
                 {UNIT} ⇒ available_data_grants={available_data_grants}; the control arm can use no \
                 DATA grant, so no DATA exists for a Probe to displace. Retained as an activation / \
                 state-invariance control; NOT counted in the Gate-5 DATA-displacement denominator."
            );
        }

        fingerprints.push(Fingerprint {
            cell: c.id,
            expected_grant_class: c.expected_grant_class,
            expected_token_predicate: c.expected_token_predicate,
            call_entry_tokens,
            decision_entry_tokens,
            post_turn_tokens,
            front_kind,
            front_size,
            burst_cap,
            window_bytes,
            observed_rate,
            paced_debit_bytes,
            baseline_probe_bytes,
            available_data_grants,
            actual_data_grants,
            data_queued,
            off_data_ids,
            on_data_ids,
            probe_due: c.expected_probe_due,
            probe_emitted,
            probe_in_baseline,
            baseline_probe_source,
            recovery_active,
            parity_pending_or_actuated,
            parity_claim_scope: c.parity_claim_scope,
            activation,
            activation_proof,
            contention_non_vacuous,
            verdict,
            startup_pacing,
        });
    }

    // ── RULE-8: semantic cell uniqueness on the INTENDED axes ───────────────────────────────────
    for i in 0..fingerprints.len() {
        for j in (i + 1)..fingerprints.len() {
            if fingerprints[i].axes() == fingerprints[j].axes() {
                reds.push(format!(
                    "RULE-8 SEMANTIC CELL UNIQUENESS FAILURE: {} and {} share the axis tuple {:?} \
                     — one piece of evidence, not two",
                    fingerprints[i].cell,
                    fingerprints[j].cell,
                    fingerprints[i].axes()
                ));
            }
        }
    }

    print_fingerprint_table(&fingerprints);

    MatrixResult { fingerprints, reds }
}

// ── UNCONSTRUCTIBLE records (classified, lineage-bearing; never silently skipped) ───────────────

/// The two regimes that are never built at all. Their guards compare LIVE product observations
/// (per-cell `burst_cap`, the bisected first ladder step, the saturation instant derived from the
/// observed cap and rate) — not two file-local literals — so they break if the product changes.
fn print_unconstructible_records(fingerprints: &[Fingerprint]) -> Vec<String> {
    let mut reds = Vec::new();

    eprintln!(
        "\n===== UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT (arithmetic, NOT time-swept) ====="
    );

    // Live product observations that the guards below are built from.
    let observed_first_step = observe_first_probe_step_ns();
    let obs_cap = fingerprints.iter().map(|f| f.burst_cap).max().unwrap_or(0);
    let obs_rate = fingerprints
        .iter()
        .map(|f| f.observed_rate)
        .max()
        .unwrap_or(0);
    let obs_sat = observed_saturation_ns(obs_cap, obs_rate);
    eprintln!(
        "  LIVE OBSERVATIONS used by the guards: first ladder step (bisected from product output) = \
         {observed_first_step:?} ns; max observed burst_cap = {obs_cap} B; max observed rate = \
         {obs_rate} B/s; saturation = ceil(cap*1e9/rate) = {obs_sat:?} ns"
    );

    // cell_4a — probe-NOT-due x OneDataGrant.
    eprintln!(
        "\n[cell_4a probe-NOT-due x OneDataGrant] UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT"
    );
    eprintln!("  expected_token_predicate : OneDataGrantCapacitySaturated (entry_tokens == cap)");
    eprintln!("  expected_grant_class     : OneDataGrant");
    eprintln!("  required_activation      : probe NOT due AND exactly one DATA grant, same turn");
    eprintln!(
        "  lineage                  : probe-not-due requires T < {observed_first_step:?} ns \
         (OBSERVED first ladder step);"
    );
    eprintln!(
        "                             OneDataGrant requires entry_tokens == burst_cap ⇒ \
         T >= {obs_sat:?} ns (OBSERVED cap and rate)."
    );
    eprintln!(
        "                             That interval is EMPTY ⇒ the conjunction is unsatisfiable. NOT time-swept."
    );
    match (observed_first_step, obs_sat) {
        (Some(step0), Some(sat)) => {
            if step0 > sat {
                reds.push(format!(
                    "cell_4a: lineage broken — the OBSERVED interval [{sat}, {step0}) is NOT empty; \
                     the cell would be constructible and must be built, not classified"
                ));
            }
            if step0 != PROBE_STEPS_NS[0] {
                reds.push(format!(
                    "cell_4a: the OBSERVED first ladder step {step0} ns differs from the frozen \
                     BS_FIRES_NS[0]={} ns used in this file's prints",
                    PROBE_STEPS_NS[0]
                ));
            }
        }
        (None, _) => reds.push(
            "cell_4a: the bootstrap ladder never fired within the search bound — the \
             unconstructibility lineage cannot be verified against live product state"
                .to_string(),
        ),
        (_, None) => reds.push(
            "cell_4a: the saturation instant could not be derived from observed cap/rate"
                .to_string(),
        ),
    }

    // cell_4b — any x MultipleDataGrants. Checked PER CELL, not against one carried value.
    eprintln!("\n[cell_4b any x MultipleDataGrants] UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT");
    eprintln!(
        "  expected_token_predicate : entry_tokens >= 2*{UNIT} = {}",
        2 * UNIT
    );
    eprintln!("  expected_grant_class     : MultipleDataGrants");
    eprintln!("  required_activation      : >= 2 DATA grants in one turn");
    eprintln!(
        "  lineage                  : entry_tokens <= burst_cap by TokenBucket::refill's min(); \
         and burst_cap = window_bytes.clamp(front_size, burst_hi) LOCKS to front_size whenever \
         window_bytes < front_size — i.e. the bucket carries ~ONE front packet. PER-CELL OBSERVED:"
    );
    for f in fingerprints {
        eprintln!(
            "                             {:<58} rate={} window_bytes={} front={}/{}B burst_cap={}",
            f.cell, f.observed_rate, f.window_bytes, f.front_kind, f.front_size, f.burst_cap
        );
        if f.burst_cap >= 2 * UNIT {
            reds.push(format!(
                "cell_4b: lineage broken — {} observed burst_cap={} >= {}; MultipleDataGrants is \
                 reachable and must be built, not classified",
                f.cell,
                f.burst_cap,
                2 * UNIT
            ));
        }
        if f.window_bytes >= f.front_size {
            reds.push(format!(
                "cell_4b: lineage broken — {} has window_bytes={} >= front_size={}; the cap no \
                 longer locks to the front packet, so `MultipleDataGrants is impossible` no longer \
                 follows from THIS lineage",
                f.cell, f.window_bytes, f.front_size
            ));
        }
    }
    eprintln!(
        "                             Time cannot lift a cap ⇒ ARITHMETICALLY IMPOSSIBLE BY THIS \
         LINEAGE (not merely by a literal constant). NOT time-swept."
    );

    reds
}

// ── CELL 5 — the four-condition recovery conjunction ────────────────────────────────────────────

#[derive(Debug)]
struct ConjunctionAttempt {
    route: &'static str,
    attempted_ns: u64,
    selected_probe_step_ns: u64,
    setup_event_order: String,
    pre_target_state: String,
    entry_tokens: u64,
    data_queued: u64,
    arms_identical: bool,
    recovery_active: bool,
    recovery_evidence: String,
    /// MARK — a real `TransmitKind::Retransmit` decision inside this attempt's event window.
    /// This is the ONLY place the recovery leg of closure criterion (2) is non-vacuous: the frozen
    /// regime matrix injects no loss, so route A2 (where ARQ actually plans a retransmit) is what
    /// keeps that leg live rather than merely well-formed.
    recovery_mark_retransmit: bool,
    /// PROOF — the loss ledger moved across the SAME window: a positive cumulative loss-declared
    /// delta (`diag_loss().0/.1`).
    recovery_proof_loss_declared_delta: bool,
    /// PROOF — a strict DROP of the instantaneous `pending_lost` depth (`diag_loss().2`) across the
    /// same window: `plan_arq_retransmits` removes the record it retransmits (sender.rs:3919).
    recovery_proof_pending_lost_drop: bool,
    probe_emitted_treatment: bool,
    /// `parity_pending_at_entry > 0 || parity_actuated_in_target_turn` for this attempt. Never
    /// called "parity active": no cell_5 route stages parity, so all three parity concepts are
    /// silent here and the field records only that fact.
    parity_pending_or_actuated: bool,
    breaking_transition: String,
}

impl ConjunctionAttempt {
    /// All four conjuncts of the cell-5 construction.
    fn all_four_hold(&self) -> bool {
        self.recovery_active
            && self.probe_emitted_treatment
            && self.data_queued > 0
            && self.arms_identical
    }

    /// The INDEPENDENT proof for this attempt's recovery MARK. Reads no decision list.
    fn recovery_proved(&self) -> bool {
        self.recovery_proof_loss_declared_delta || self.recovery_proof_pending_lost_drop
    }

    /// EVERY conjunct that did not hold, derived LIVE from the recorded observations. It is
    /// deliberately not a hard-coded label: when the route-A emission-ledger accounting was
    /// corrected, route A2's true queue depth fell to 0 and a frozen string would have kept
    /// asserting a single, now-incomplete reason.
    fn failed_conjuncts(&self) -> String {
        let mut v: Vec<&str> = Vec::new();
        if !self.recovery_active {
            v.push("recovery active in target turn");
        }
        if !self.probe_emitted_treatment {
            v.push("probe emitted in treatment arm");
        }
        if self.data_queued == 0 {
            v.push("DATA queued at target-turn entry");
        }
        if !self.arms_identical {
            v.push("arms identical pre-treatment");
        }
        if v.is_empty() {
            "none — all four held (see the red list)".to_string()
        } else {
            v.join(" + ")
        }
    }
}

fn print_attempt(a: &ConjunctionAttempt) {
    eprintln!("\n  --- attempt route={} ---", a.route);
    eprintln!("    attempted timestamp     : {} ns", a.attempted_ns);
    eprintln!(
        "    selected probe step     : {} ns (absolute)",
        a.selected_probe_step_ns
    );
    eprintln!("    setup-event order       : {}", a.setup_event_order);
    eprintln!("    pre-target state        : {}", a.pre_target_state);
    eprintln!("    entry_tokens (derived)  : {}", a.entry_tokens);
    eprintln!("    DATA queued at entry    : {}", a.data_queued);
    eprintln!("    arms identical pre      : {}", a.arms_identical);
    eprintln!(
        "    recovery active in turn : {} ({})",
        a.recovery_active, a.recovery_evidence
    );
    eprintln!(
        "    recovery MARK / PROOF   : MARK retransmit_decision={} | PROOF loss_declared_delta={} \
         pending_lost_drop={} ⇒ recovery_proved={}",
        a.recovery_mark_retransmit,
        a.recovery_proof_loss_declared_delta,
        a.recovery_proof_pending_lost_drop,
        a.recovery_proved()
    );
    eprintln!(
        "    probe emitted (treat.)  : {}",
        a.probe_emitted_treatment
    );
    eprintln!(
        "    parity pend-or-actuated : {} (no cell_5 route stages parity: parity_pending_at_entry=0, \
         parity_actuated_in_target_turn=false, parity_competed_for_paced_grant=false)",
        a.parity_pending_or_actuated
    );
    eprintln!("    FAILED CONJUNCT(S)      : {}", a.failed_conjuncts());
    eprintln!("    breaking transition     : {}", a.breaking_transition);
}

/// Build a cell-5 arm with the MINIMUM number of intervening events needed to put >= 3 DATA on the
/// wire, each placed exactly at a token-saturation instant so no grant is wasted and no extra probe
/// generation is minted. Returns the arm and the wire seqs of the emitted DATA.
fn build_recovery_arm(bootstrap_on: bool) -> (Arm, Vec<u64>, Vec<u64>) {
    let mut arm = build_arm(bootstrap_on, false, 8, None);
    let mut steps = Vec::new();
    // Saturation recurs every ceil(101e9/31) ns; the sub-byte remainder carries, so these are the
    // exact instants at which the next single grant becomes available.
    for k in 1..=3u64 {
        let t = SATURATION_NS * k;
        let out = arm.advance(Event::TimerFired(Timer::Pto), t);
        arm.pre.extend(out);
        steps.push(t);
    }
    let seqs: Vec<u64> = arm
        .pre
        .iter()
        .filter(|d| d.kind == TransmitKind::Data)
        .map(|d| d.seq)
        .collect();
    (arm, seqs, steps)
}

/// Returns `(reds, classification, attempts)`. The ATTEMPTS are handed back so closure criterion (2)
/// can assert over them: the frozen regime matrix injects no loss, so cell_5 route A2 is the only
/// place in this file where a recovery activation is actually claimed, and a criterion that never
/// sees it would leave the recovery leg unfalsifiable no matter how well it is written.
#[allow(clippy::too_many_lines)]
fn run_cell_5() -> (Vec<String>, &'static str, Vec<ConjunctionAttempt>) {
    let mut reds = Vec::new();
    let target_ns = PROBE_STEPS_NS[2]; // absolute ladder step 21 s
    let mut attempts: Vec<ConjunctionAttempt> = Vec::new();

    eprintln!("\n===== cell_5 probe-DUE x DATA x RECOVERY-in-target-turn =====");
    eprintln!("  expected_token_predicate : OneDataGrantCapacitySaturated (entry_tokens == 101)");
    eprintln!("  expected_grant_class     : OneDataGrant");
    eprintln!(
        "  required_activation      : recovery active in TARGET turn AND probe emitted in the"
    );
    eprintln!("                             treatment arm AND DATA queued AND arms identical pre.");
    eprintln!("  loss-declaration paths in the product (exhaustive, sender.rs):");
    eprintln!("    route A = packet-threshold, on_ack_received  (guard sender.rs:2337)");
    eprintln!("    route B = time-threshold,  on_timer(Timer::Pto) (guard sender.rs:2376)");

    // ── route A: ACK-driven packet-threshold at the absolute step ────────────────────────────────
    {
        let (mut off, seqs, steps) = build_recovery_arm(false);
        let (mut on, _s2, _t2) = build_recovery_arm(true);
        let arms_identical = input(&off.s, &off.pre) == input(&on.s, &on.pre);
        let entry_tokens = off.derive_entry_tokens(target_ns);
        let data_queued = off.queued_data();
        let state_before = format!("{:?}", off.s.state());
        let setup = format!(
            "8x AppData@0 (DATA#1 emitted, ladder armed t0=0); Pto@{steps:?} (one DATA grant each)"
        );
        if seqs.len() < 3 {
            reds.push(format!(
                "cell_5 route A: need >= 3 sent DATA for a real gap, got {}",
                seqs.len()
            ));
        }
        let gap: Vec<u64> = seqs[1..].to_vec(); // withhold the LOWEST ⇒ real packet-threshold gap
        let loss_before = off.s.diag_loss();
        let off_out = off.advance(
            Event::AckReceived(AckInfo {
                acked_seqs: gap.clone(),
                ack_delay: Duration::from_millis(1),
            }),
            target_ns,
        );
        let on_out = on.advance(
            Event::AckReceived(AckInfo {
                acked_seqs: gap,
                ack_delay: Duration::from_millis(1),
            }),
            target_ns,
        );
        let loss_after = off.s.diag_loss();
        let loss_delta = loss_after.0 > loss_before.0 || loss_after.1 > loss_before.1;
        let pending_lost_drop = loss_after.2 < loss_before.2;
        let retx = off_out.iter().any(|d| d.kind == TransmitKind::Retransmit);
        let recovery_state = matches!(
            off.s.state(),
            SenderState::RecoverArq | SenderState::RecoverFwd
        );
        let recovery_active = retx || (loss_delta && recovery_state);
        let probe = on_out.iter().any(|d| d.kind == TransmitKind::CsProbe);
        let (_, _, srtt_us, _, _) = off.s.diag_rate();
        // The target turn's own output MUST be folded into the emission ledger before `queued_data()`
        // is consulted again, or every later reading over-reports the queue by the DATA route A
        // emitted here.
        off.pre.extend(off_out);
        on.pre.extend(on_out);
        attempts.push(ConjunctionAttempt {
            route: "A (packet-threshold ACK at absolute step 21 s)",
            attempted_ns: target_ns,
            selected_probe_step_ns: PROBE_STEPS_NS[2],
            setup_event_order: setup,
            pre_target_state: state_before,
            entry_tokens,
            data_queued,
            arms_identical,
            recovery_active,
            recovery_evidence: format!(
                "retx_in_turn={retx}, loss_declared_delta={loss_delta} ({loss_before:?}->{loss_after:?}), state_after={:?}",
                off.s.state()
            ),
            recovery_mark_retransmit: retx,
            recovery_proof_loss_declared_delta: loss_delta,
            recovery_proof_pending_lost_drop: pending_lost_drop,
            probe_emitted_treatment: probe,
            parity_pending_or_actuated: false,
            breaking_transition: format!(
                "the SAME on_event produced the first RTT sample (srtt now {srtt_us} us), which \
                 (a) closes drain_bootstrap_probes' `rtt.srtt().is_none()` gate and (b) drives \
                 update_state's PreConverge arm to Startup (sender.rs:2683-2690) BEFORE \
                 schedule_transmit runs — state_after={:?}. Loss declaration therefore lands in \
                 pending_lost while the probe gate is already closed, permanently.",
                off.s.state()
            ),
        });

        // Route A2 — the follow-up turns, to show that the RECOVERY leg is achievable on its own
        // (a real `TransmitKind::Retransmit`) and that the probe leg is nonetheless gone forever.
        // Bounded, deterministic: at most 16 further Pto ticks at 4 s spacing, stopping at the first
        // real retransmit. This is a diagnostic sub-record of cell_5, not a cell of its own — but it
        // is guarded by the SAME four-conjunct RED as routes A and B.
        let mut retx2 = false;
        let mut probe2 = false;
        let mut ticks = 0u64;
        let mut t2 = target_ns;
        // Pre-arbitration entry facts of the FINAL turn of this loop (never read post-call).
        let mut entry2 = 0u64;
        let mut queued2 = 0u64;
        // INDEPENDENT recovery proof, accumulated over this attempt's OWN event window: the loss
        // ledger's movement, read from `diag_loss()` around each `on_event` and never from `o2`.
        let mut loss_delta2 = false;
        let mut pending_lost_drop2 = false;
        while ticks < 16 && !retx2 {
            ticks += 1;
            t2 = target_ns + ticks * 4 * SEC;
            entry2 = off.derive_entry_tokens(t2);
            queued2 = off.queued_data();
            let loss_pre2 = off.s.diag_loss();
            let o2 = off.advance(Event::TimerFired(Timer::Pto), t2);
            let n2 = on.advance(Event::TimerFired(Timer::Pto), t2);
            let loss_post2 = off.s.diag_loss();
            loss_delta2 |= loss_post2.0 > loss_pre2.0 || loss_post2.1 > loss_pre2.1;
            pending_lost_drop2 |= loss_post2.2 < loss_pre2.2;
            retx2 = o2.iter().any(|d| d.kind == TransmitKind::Retransmit);
            probe2 |= n2.iter().any(|d| d.kind == TransmitKind::CsProbe);
            // Keep the emission ledger current so `queued_data()` stays exact across the loop.
            off.pre.extend(o2);
            on.pre.extend(n2);
        }
        let recovery_state2 = matches!(
            off.s.state(),
            SenderState::RecoverArq | SenderState::RecoverFwd
        );
        attempts.push(ConjunctionAttempt {
            route: "A2 (follow-up turns, where ARQ actually plans a retransmit)",
            attempted_ns: t2,
            selected_probe_step_ns: PROBE_STEPS_NS[2],
            setup_event_order: format!("…route A, then {ticks} further Pto at 4 s spacing"),
            pre_target_state: "Startup/RecoverArq (post-ACK)".to_string(),
            entry_tokens: entry2,
            data_queued: queued2,
            arms_identical,
            recovery_active: retx2,
            recovery_evidence: format!(
                "real Retransmit in turn={retx2} after {ticks} tick(s), state_after={:?}, loss={:?}",
                off.s.state(),
                off.s.diag_loss()
            ),
            recovery_mark_retransmit: retx2,
            recovery_proof_loss_declared_delta: loss_delta2,
            recovery_proof_pending_lost_drop: pending_lost_drop2,
            probe_emitted_treatment: probe2,
            parity_pending_or_actuated: false,
            breaking_transition: format!(
                "the recovery leg IS reachable here (recovery_state={recovery_state2}), but srtt is \
                 now Some for good, so the bootstrap ladder can never re-open — the gate is \
                 `state == PreConverge && rtt.srtt().is_none()` (sender.rs:2440-2442). The two legs \
                 are never co-timed."
            ),
        });
    }

    // ── route B: PTO time-threshold at the same absolute step ───────────────────────────────────
    {
        let (mut off, _seqs, steps) = build_recovery_arm(false);
        let (mut on, _s2, _t2) = build_recovery_arm(true);
        let arms_identical = input(&off.s, &off.pre) == input(&on.s, &on.pre);
        let entry_tokens = off.derive_entry_tokens(target_ns);
        let data_queued = off.queued_data();
        let state_before = format!("{:?}", off.s.state());
        let loss_before = off.s.diag_loss();
        let off_out = off.advance(Event::TimerFired(Timer::Pto), target_ns);
        let on_out = on.advance(Event::TimerFired(Timer::Pto), target_ns);
        let loss_after = off.s.diag_loss();
        let loss_delta = loss_after.0 > loss_before.0 || loss_after.1 > loss_before.1;
        let pending_lost_drop = loss_after.2 < loss_before.2;
        let retx = off_out.iter().any(|d| d.kind == TransmitKind::Retransmit);
        let recovery_state = matches!(
            off.s.state(),
            SenderState::RecoverArq | SenderState::RecoverFwd
        );
        let recovery_active = retx || (loss_delta && recovery_state);
        let probe = on_out.iter().any(|d| d.kind == TransmitKind::CsProbe);
        attempts.push(ConjunctionAttempt {
            route: "B (time-threshold PTO at absolute step 21 s)",
            attempted_ns: target_ns,
            selected_probe_step_ns: PROBE_STEPS_NS[2],
            setup_event_order: format!(
                "8x AppData@0 (DATA#1 emitted, ladder armed t0=0); Pto@{steps:?}"
            ),
            pre_target_state: state_before,
            entry_tokens,
            data_queued,
            arms_identical,
            recovery_active,
            recovery_evidence: format!(
                "retx_in_turn={retx}, loss_declared_delta={loss_delta} ({loss_before:?}->{loss_after:?}), state_after={:?}",
                off.s.state()
            ),
            recovery_mark_retransmit: retx,
            recovery_proof_loss_declared_delta: loss_delta,
            recovery_proof_pending_lost_drop: pending_lost_drop,
            probe_emitted_treatment: probe,
            parity_pending_or_actuated: false,
            breaking_transition:
                "no transition broke it — `on_timer(Timer::Pto)` FREEZES time-threshold loss \
                 declaration while `state == PreConverge` (sender.rs:2374-2384, §3.3-C). The probe \
                 fires precisely because the sender is still in PreConverge, and that is the same \
                 condition that suppresses recovery."
                    .to_string(),
        });
    }

    for a in &attempts {
        print_attempt(a);
    }

    // ONE guard over EVERY route (A, A2 and B alike): if all four conjuncts ever hold, the cell is
    // constructible and classifying it would be dishonest — that is a RED, not a reclassification.
    for a in &attempts {
        if a.all_four_hold() {
            reds.push(format!(
                "cell_5 route {}: all four conjuncts held — the cell is CONSTRUCTIBLE and must be \
                 run through the paired oracle, not classified",
                a.route
            ));
        }
    }

    let any_full = attempts.iter().any(ConjunctionAttempt::all_four_hold);
    let classification = if any_full {
        "CONSTRUCTIBLE"
    } else {
        "UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT"
    };
    eprintln!("\n  cell_5 CLASSIFICATION: {classification}");
    eprintln!(
        "  lineage: probe emission requires `state == PreConverge && rtt.srtt().is_none()`. Both \
         product loss-declaration paths are mutually exclusive with that predicate — route A \
         supplies srtt (closing the gate and leaving PreConverge in the SAME on_event), route B is \
         frozen by the PreConverge guard. This is a PRODUCT GATING invariant, not a harness limit."
    );

    (reds, classification, attempts)
}

// ── scope summary ───────────────────────────────────────────────────────────────────────────────

/// The ACTUAL cargo feature configuration this binary was compiled with.
///
/// The previous revision keyed only on `experimental-cold-start`, so runs under
/// `--features experimental-fec-budget` and `--features diag-sched-telemetry` printed
/// "as measured in the default build" — a FALSE label in 3 of the 5 configurations this file is run
/// in. Every feature of the crate is enumerated here, so the sentence is true in all of them.
fn feature_configuration() -> String {
    let mut on: Vec<&str> = Vec::new();
    if cfg!(feature = "experimental-cold-start") {
        on.push("experimental-cold-start");
    }
    if cfg!(feature = "experimental-fec-budget") {
        on.push("experimental-fec-budget");
    }
    if cfg!(feature = "diag-sched-telemetry") {
        on.push("diag-sched-telemetry");
    }
    if on.is_empty() {
        "DEFAULT build of adaptive-transport (no cargo features enabled)".to_string()
    } else {
        format!(
            "build of adaptive-transport with cargo feature(s) [{}] ENABLED (every other feature of \
             the crate off)",
            on.join(", ")
        )
    }
}

#[allow(clippy::too_many_lines)]
fn print_scope_summary(
    m: &MatrixResult,
    cell5_class: &str,
    cell5_attempts: &[ConjunctionAttempt],
) -> Vec<String> {
    let mut reds = Vec::new();

    let n_contention = m.count(CellVerdict::ContentionTesting);
    let n_activated = m.count(CellVerdict::ActivatedButVacuousForDataDisplacement);
    let n_parity_actuated = m.count(CellVerdict::ParityActuatedButVacuousForDataDisplacement);
    let n_state = m.count(CellVerdict::StateActiveButVacuousForDataDisplacement);
    let n_inert = m.count(CellVerdict::InertTreatmentVacuousForDataDisplacement);
    let n_harness = m.count(CellVerdict::CellUnconstructibleHarnessLimitation);
    let n_conf = m.count(CellVerdict::ConfoundedBaselineConfig);
    // cell_4a + cell_4b are never built; cell_5 joins them when its conjunction is unsatisfiable.
    let unreachable = 2 + usize::from(cell5_class == "UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT");
    let regimes: BTreeSet<String> = m
        .fingerprints
        .iter()
        .map(|f| format!("{:?}", GrantClass::of(f.available_data_grants)))
        .collect();

    eprintln!("\n===== SCOPE SUMMARY (startup_pacing = false) =====");
    eprintln!("  contention-testing cells                        : {n_contention}");
    eprintln!("  activated-but-vacuous-for-DATA-displacement     : {n_activated}");
    eprintln!("  parity-ACTUATED-but-vacuous-for-DATA-displacement: {n_parity_actuated}");
    eprintln!("  state-active-but-vacuous-for-DATA-displacement  : {n_state}");
    eprintln!("  unreachable-by-frozen-config-invariant          : {unreachable}");
    eprintln!("  harness-limited cells                           : {n_harness}");
    eprintln!("  confounded-baseline-config cells                : {n_conf}");
    eprintln!(
        "  observed distinct token regimes                 : {}",
        regimes.len()
    );
    // Additional honesty lines (not part of the fixed block above).
    eprintln!(
        "  [extra] inert-treatment cells (probe not due by design, control emits no DATA) : {n_inert}"
    );
    eprintln!("  [extra] unreachable records                     : cell_4a, cell_4b, cell_5");
    eprintln!("  [extra] observed regimes                        : {regimes:?}");
    let accounted =
        n_contention + n_activated + n_parity_actuated + n_state + n_inert + n_harness + n_conf;
    eprintln!(
        "  [extra] per-cell accounting                     : {accounted} classified / {} built",
        m.fingerprints.len()
    );
    if accounted != m.fingerprints.len() {
        reds.push(format!(
            "SCOPE ACCOUNTING FAILURE: {accounted} classified cells != {} built cells",
            m.fingerprints.len()
        ));
    }

    let denominator: Vec<&str> = m
        .fingerprints
        .iter()
        .filter(|f| f.verdict == CellVerdict::ContentionTesting)
        .map(|f| f.cell)
        .collect();
    let excluded: Vec<String> = m
        .fingerprints
        .iter()
        .filter(|f| f.verdict != CellVerdict::ContentionTesting)
        .map(|f| format!("{} [{}]", f.cell, f.verdict.label()))
        .collect();
    eprintln!("  Gate-5 DATA-displacement DENOMINATOR            : {denominator:?}");
    eprintln!("  EXCLUDED from that denominator                  : {excluded:?}");

    let build_axis = feature_configuration();
    let cap = m
        .fingerprints
        .iter()
        .map(|f| f.burst_cap)
        .max()
        .unwrap_or(0);
    // The exclusion clause is driven by the LIVE per-cell classification and the LIVE `n_conf`
    // count. It NEVER says "the <build> is excluded": that would assert the claim FOR a build and
    // that the same build is excluded, in one sentence. What is excluded is a set of CELLS, which
    // may be empty — and the empty case must read correctly too.
    let confounded_cells: Vec<&str> = m
        .fingerprints
        .iter()
        .filter(|f| f.verdict == CellVerdict::ConfoundedBaselineConfig)
        .map(|f| f.cell)
        .collect();
    let exclusion_clause = if n_conf == 0 {
        format!(
            "no cell of this run is CONFOUNDED-BASELINE-CONFIG — the control arm's probe was \
             attributed to the LEGACY paced CS ladder (sender.rs:2996-2999 → drain_cs_probes → \
             pacer.try_send({LEGACY_CS_PROBE_WIRE_MAX})) in 0 of {} built cell(s), so no cell is \
             excluded on that ground",
            m.fingerprints.len()
        )
    } else {
        format!(
            "{n_conf} of {} built cell(s) — {confounded_cells:?} — are EXCLUDED as \
             CONFOUNDED-BASELINE-CONFIG because their CONTROL arm ran the LEGACY paced CS ladder \
             (sender.rs:2996-2999 → drain_cs_probes → pacer.try_send({LEGACY_CS_PROBE_WIRE_MAX})), \
             attributed from the legacy counter with the shipping counter \
             bootstrap_rtt_diag().probe_emitted == 0; the remaining {} built cell(s) of this run are \
             NOT excluded on that ground",
            m.fingerprints.len(),
            m.fingerprints.len() - n_conf
        )
    };
    // The parity clause is derived LIVE from the three measured concepts, so the claim can never
    // drift back into calling a pending-at-entry cell "parity-active".
    let cells_parity_pending: Vec<&str> = m
        .fingerprints
        .iter()
        .filter(|f| f.activation.parity_pending_at_entry > 0)
        .map(|f| f.cell)
        .collect();
    let cells_parity_actuated: Vec<&str> = m
        .fingerprints
        .iter()
        .filter(|f| f.activation.parity_actuated_in_target_turn)
        .map(|f| f.cell)
        .collect();
    let cells_parity_competed: Vec<&str> = m
        .fingerprints
        .iter()
        .filter(|f| f.activation.parity_competed_for_paced_grant())
        .map(|f| f.cell)
        .collect();
    let denom_pending_not_actuated: Vec<&str> = m
        .fingerprints
        .iter()
        .filter(|f| {
            f.verdict == CellVerdict::ContentionTesting
                && f.activation.parity_pending_at_entry > 0
                && !f.activation.parity_actuated_in_target_turn
        })
        .map(|f| f.cell)
        .collect();
    let parity_clause = format!(
        "PARITY CLAIM SCOPE (three DISTINCT concepts, never conflated): \
         parity_pending_at_entry > 0 in {cells_parity_pending:?}; \
         parity_actuated_in_target_turn in {cells_parity_actuated:?}; \
         parity_competed_for_paced_grant in {cells_parity_competed:?}. \
         The denominator cell(s) {denom_pending_not_actuated:?} carry parity work PENDING AT ENTRY \
         and did NOT actuate parity in the target turn: they are evidence of PROBE-vs-DATA \
         contention ONLY, and are NOT evidence of target-turn parity actuation and NOT evidence of \
         parity-vs-DATA contention. Nothing in this run licenses a parity-vs-DATA displacement \
         claim in either direction: the product plans DATA/ARQ BEFORE plan_fec_parity in the same \
         turn, so a parity token denial is downstream of the DATA decision, not a competitor to it"
    );
    eprintln!(
        "  CLAIM SCOPE: Paired runtime oracle confirms zero Probe-caused DATA grant displacement in \
         the contended single-grant regime (OneDataGrant: decision_entry_tokens == burst_cap == \
         {cap} B, with the control arm actually TAKING that grant and EMITTING a DATA identity), as \
         measured in the {build_axis}; {exclusion_clause}. The Gate-5 denominator for this run is \
         {denominator:?}. {parity_clause}. ZeroDataGrant cells are Probe-activation / \
         state-invariance / parity-actuation controls ONLY — they are VACUOUS for DATA displacement \
         (the control arm holds no DATA grant to displace) and are NOT in the Gate-5 denominator. \
         cell_4a / cell_4b / cell_5 are unreachable by frozen-config arithmetic, not time-swept. \
         This is NOT a completed matrix over all conceivable regimes."
    );

    // ── CLOSURE CRITERION ───────────────────────────────────────────────────────────────────────
    let c1 = n_contention >= 1;
    // (2) asserts the SUBSTANTIVE measured facts behind every activation this run reports — NOT the
    // non-emptiness of a formatted string, which `format!` over literal text can never fail, and NOT
    // the same locals that set the flag, which no assignment of measured facts could falsify. Each
    // leg's PROVING source is a product surface its MARKING source never reads:
    //   probe    MARK  a CsProbe decision in the target turn
    //            PROVE bootstrap_rtt_diag().probe_emitted ADVANCED across that on_event;
    //   parity   MARK  parity_pending_at_entry > 0, or parity_actuated_in_target_turn
    //            PROVE parity_competed_for_paced_grant — diag_fec_actuation().emitted_r_total
    //                  advanced, or the diag_token_denial() event counter advanced (the parity queue
    //                  front was reached and refused). NOTE: this proves parity TOUCHED THE PACER,
    //                  never that parity was ACTUATED and never that it contended WITH DATA;
    //   recovery MARK  a Retransmit decision in the target turn
    //            PROVE the diag_loss() ledger moved across that on_event — a positive cumulative
    //                  loss-declared delta and/or a strict pending_lost depth drop.
    let mut c2_failures: Vec<String> = m
        .fingerprints
        .iter()
        .filter_map(|f| {
            let mut missing: Vec<String> = Vec::new();
            if f.probe_emitted && !f.activation.probe_proved() {
                missing.push("probe (bootstrap_rtt_diag().probe_emitted delta == 0)".to_string());
            }
            if f.parity_pending_or_actuated && !f.activation.parity_competed_for_paced_grant() {
                missing.push(format!(
                    "parity (marked by parity_pending_at_entry={} / \
                     parity_actuated_in_target_turn={} but the product counters did not move, so \
                     parity_competed_for_paced_grant is UNPROVEN: emitted_r_total delta={}, \
                     token_denial delta={})",
                    f.activation.parity_pending_at_entry,
                    f.activation.parity_actuated_in_target_turn,
                    f.activation.parity_proof_emitted_counter_delta,
                    f.activation.parity_proof_token_denial_delta
                ));
            }
            if f.recovery_active && !f.activation.recovery_proved() {
                missing.push(
                    "recovery (a Retransmit decision was emitted but the diag_loss() ledger did \
                     not move: no cumulative loss-declared delta and no pending_lost depth drop)"
                        .to_string(),
                );
            }
            if f.probe_emitted != f.probe_due {
                missing.push("probe emission does not match the frozen probe_due axis".to_string());
            }
            if missing.is_empty() {
                None
            } else {
                Some(format!("{} [{}]", f.cell, missing.join(" + ")))
            }
        })
        .collect();
    // The frozen matrix injects no loss, so the recovery leg would be VACUOUS over the fingerprints
    // alone — a leg that never fires certifies nothing however carefully it is written. cell_5's
    // routes are therefore held to the SAME criterion: route A2 emits a real Retransmit, so its
    // independent loss-ledger proof has to hold.
    c2_failures.extend(cell5_attempts.iter().filter_map(|a| {
        if a.recovery_mark_retransmit && !a.recovery_proved() {
            Some(format!(
                "cell_5 route {} [recovery (a real Retransmit decision was emitted in this \
                 attempt's window but the diag_loss() ledger did not move: loss_declared_delta={}, \
                 pending_lost_drop={})]",
                a.route, a.recovery_proof_loss_declared_delta, a.recovery_proof_pending_lost_drop
            ))
        } else {
            None
        }
    }));
    let recovery_legs_exercised = m.fingerprints.iter().filter(|f| f.recovery_active).count()
        + cell5_attempts
            .iter()
            .filter(|a| a.recovery_mark_retransmit)
            .count();
    if recovery_legs_exercised == 0 {
        reds.push(
            "CLOSURE CRITERION (2) DEGENERATE: no cell and no cell_5 route claims a recovery \
             activation anywhere in this run, so the recovery leg is vacuous and its proving source \
             is never exercised"
                .to_string(),
        );
    }
    let c2 = c2_failures.is_empty();
    // Post-turn residue is never an entry predicate anywhere in this file. Evidence that the
    // distinction is MATERIAL (not merely asserted): at least one cell where the two differ.
    let c3_material = m
        .fingerprints
        .iter()
        .any(|f| f.post_turn_tokens != f.decision_entry_tokens);
    // (4) cross-checks the cap lineage across THREE INDEPENDENTLY OBTAINED numbers:
    //   burst_cap    <- diag_pacer().2                        (product pacer state)
    //   window_bytes <- diag_pacer().0 * BURST_WINDOW_NS/1e9  (product pacer RATE — a different field)
    //   front_size   <- the harness's OWN enqueue ledger      (Arm::observed_front; independent of
    //                                                          the pacer entirely)
    // The previous revision recorded `front_size = burst_cap` and then re-applied the same clamp, so
    // every sub-check compared a value with itself: MUT-4A/4B changed the facts without turning it
    // red, and the reds that appeared came from pre-existing inline guards. With the three sources
    // disjoint, `burst_cap == window_bytes.clamp(front_size, max(front_size, 2*MTU))` is a real
    // identity between measurements and breaking ANY ONE of them alone turns the criterion RED.
    let c4_failures: Vec<String> = m
        .fingerprints
        .iter()
        .filter_map(|f| {
            let burst_hi = f.front_size.max(2 * DEFAULT_MTU_BYTES);
            let window_from_rate =
                f.observed_rate.saturating_mul(BURST_WINDOW_NS) / 1_000_000_000;
            let clamped = f.window_bytes.clamp(f.front_size, burst_hi);
            let mut bad: Vec<String> = Vec::new();
            if f.burst_cap == 0 || f.front_size == 0 || f.observed_rate == 0 {
                bad.push(format!(
                    "cap lineage not recorded (burst_cap={} front_size={} rate={})",
                    f.burst_cap, f.front_size, f.observed_rate
                ));
            }
            if f.window_bytes != window_from_rate {
                bad.push(format!(
                    "recorded window_bytes={} != rate*{BURST_WINDOW_NS}/1e9={window_from_rate} for \
                     THIS cell's observed rate={}",
                    f.window_bytes, f.observed_rate
                ));
            }
            if f.front_kind != front_kind_of(f.front_size) {
                bad.push(format!(
                    "the enqueue ledger's OCCUPANCY label front_kind={} does not match the size of \
                     its front entry front_size={} (that size maps to {})",
                    f.front_kind,
                    f.front_size,
                    front_kind_of(f.front_size)
                ));
            }
            if f.window_bytes >= f.front_size {
                bad.push(format!(
                    "window_bytes={} >= front_size={} — the clamp no longer locks to the front packet",
                    f.window_bytes, f.front_size
                ));
            }
            if f.burst_cap != clamped {
                bad.push(format!(
                    "PRODUCT burst_cap={} != clamp(window_bytes={}, front_size={}, burst_hi={})={} \
                     — the pacer's cap, the pacer's rate and the INDEPENDENTLY OBSERVED tx-queue \
                     front no longer satisfy the set_burst_cap identity (sender.rs:1547-1551)",
                    f.burst_cap, f.window_bytes, f.front_size, burst_hi, clamped
                ));
            }
            if bad.is_empty() {
                None
            } else {
                Some(format!("{} [{}]", f.cell, bad.join(" + ")))
            }
        })
        .collect();
    let c4 = !m.fingerprints.is_empty() && c4_failures.is_empty();
    eprintln!("\n===== CLOSURE CRITERION =====");
    eprintln!("  (1) at least one contention-testing DATA cell PASS      : {c1} ({denominator:?})");
    eprintln!(
        "  (2) every reported active cell proves activation in turn: {c2} (each leg PROVED from a \
         product surface its MARKING source never reads — probe: MARK CsProbe decision / PROVE \
         bootstrap_rtt_diag().probe_emitted delta > 0; parity: MARK parity_pending_at_entry > 0 OR \
         parity_actuated_in_target_turn / PROVE parity_competed_for_paced_grant, i.e. \
         diag_fec_actuation().emitted_r_total delta > 0 OR diag_token_denial() event-count delta > 0 \
         — which proves parity TOUCHED THE PACER, never that it was ACTUATED and never that it \
         contended WITH DATA; recovery: MARK a Retransmit decision / PROVE a cumulative \
         loss-declared delta OR a pending_lost depth drop across that on_event) \
         recovery legs exercised (cells + cell_5 routes)={recovery_legs_exercised} \
         failures={c2_failures:?}"
    );
    eprintln!(
        "  (3) no post-turn residue used as an entry predicate      : true (structural; \
         decision_entry_state is the only grant predicate input) — materiality proven: {c3_material}"
    );
    eprintln!(
        "  (4) cell-specific cap lineage complete                  : {c4} (per cell, across THREE \
         INDEPENDENT sources — burst_cap from diag_pacer().2, window_bytes from diag_pacer().0, \
         front_size from the harness's own enqueue ledger: window_bytes re-derives from that cell's \
         observed rate, the ledger's occupancy label matches its front entry's size, and burst_cap \
         == clamp(window_bytes, front_size, max(front_size, 2*MTU))) over {} recorded cell(s) \
         failures={c4_failures:?}",
        m.fingerprints.len()
    );
    if !c1 {
        reds.push(
            "CLOSURE CRITERION (1) FAILED: no contention-testing DATA cell — every built cell is \
             vacuous for DATA displacement, so nothing supports the Gate-5 claim"
                .to_string(),
        );
    }
    if !c2 {
        reds.push(format!(
            "CLOSURE CRITERION (2) FAILED: a reported active cell does not carry the measured \
             in-target-turn fact its activation claim needs :: {c2_failures:?}"
        ));
    }
    if !c3_material {
        reds.push(
            "CLOSURE CRITERION (3) DEGENERATE: post_turn_residue never differs from \
             decision_entry_state in any cell, so the instrument cannot demonstrate the distinction"
                .to_string(),
        );
    }
    if !c4 {
        reds.push(format!(
            "CLOSURE CRITERION (4) FAILED: a cell's cap lineage was not recorded per cell or is not \
             internally consistent (window_bytes must re-derive from that cell's observed rate, \
             front_kind must match front_size, and burst_cap must equal clamp(window_bytes, \
             front_size, max(front_size, 2*MTU))) :: recorded_cells={} :: {c4_failures:?}",
            m.fingerprints.len()
        ));
    }

    reds
}

// ── tests ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn paired_structural_oracle_constructibility_matrix() {
    let mut reds = Vec::new();

    let m = run_regime_cells(false, "CONSTRUCTIBILITY MATRIX");
    reds.extend(m.reds.iter().cloned());

    reds.extend(print_unconstructible_records(&m.fingerprints));

    let (cell5_reds, cell5_class, cell5_attempts) = run_cell_5();
    reds.extend(cell5_reds);

    reds.extend(print_scope_summary(&m, cell5_class, &cell5_attempts));

    assert!(
        reds.is_empty(),
        "DR-CS-3 structural oracle RED:\n  - {}",
        reds.join("\n  - ")
    );
}

/// **(G) SHIPPING-DEFAULT PACING CONFIRMATION** — the matrix above is built under
/// `startup_pacing = false`; the product default is `StartupPacingConfig::default().enabled = true`.
/// Measurement says pacing ON yields the IDENTICAL rate=31 / burst_cap=101, so the same regimes are
/// reachable and no more. That is VERIFIED here, not assumed.
#[test]
fn shipping_default_pacing_confirmation() {
    let mut reds = Vec::new();

    let off_pacing = run_regime_cells(false, "REFERENCE (pacing OFF)");
    let on_pacing = run_regime_cells(true, "SHIPPING-DEFAULT PACING CONFIRMATION");
    // BOTH arms' reds are collected — dropping the reference arm's would hide a regression that
    // happens to affect only the pacing-OFF build.
    reds.extend(off_pacing.reds.iter().cloned());
    reds.extend(on_pacing.reds.iter().cloned());

    eprintln!("\n===== SHIPPING-DEFAULT PACING: OFF vs ON (per cell) =====");
    for (a, b) in off_pacing
        .fingerprints
        .iter()
        .zip(on_pacing.fingerprints.iter())
    {
        eprintln!(
            "  {:<58} rate OFF={} ON={} | cap OFF={} ON={} | front OFF={}/{}B ON={}/{}B | \
             call OFF={} ON={} | dec OFF={} ON={} | avail OFF={} ON={} | actual OFF={} ON={} | \
             verdict OFF={} ON={}",
            a.cell,
            a.observed_rate,
            b.observed_rate,
            a.burst_cap,
            b.burst_cap,
            a.front_kind,
            a.front_size,
            b.front_kind,
            b.front_size,
            a.call_entry_tokens,
            b.call_entry_tokens,
            a.decision_entry_tokens,
            b.decision_entry_tokens,
            a.available_data_grants,
            b.available_data_grants,
            a.actual_data_grants,
            b.actual_data_grants,
            a.verdict.label(),
            b.verdict.label()
        );
        if a.observed_rate != b.observed_rate {
            reds.push(format!(
                "{}: pacing ON changed the PreConverge rate: OFF={} ON={}",
                a.cell, a.observed_rate, b.observed_rate
            ));
        }
        if a.burst_cap != b.burst_cap {
            reds.push(format!(
                "{}: pacing ON changed the burst_cap: OFF={} ON={}",
                a.cell, a.burst_cap, b.burst_cap
            ));
        }
        if a.decision_entry_tokens != b.decision_entry_tokens
            || a.available_data_grants != b.available_data_grants
            || a.actual_data_grants != b.actual_data_grants
        {
            reds.push(format!(
                "{}: pacing ON changed the grant regime (dec-entry {}->{}, avail {}->{}, actual {}->{})",
                a.cell, a.decision_entry_tokens, b.decision_entry_tokens, a.available_data_grants,
                b.available_data_grants, a.actual_data_grants, b.actual_data_grants
            ));
        }
        // Did pacing ON open a MultipleDataGrants regime? If so it MUST be built, not classified.
        if b.burst_cap >= 2 * UNIT {
            reds.push(format!(
                "{}: pacing ON raised burst_cap to {} >= {}: MultipleDataGrants is now reachable \
                 and a full paired oracle must be run for it with predicates frozen before the \
                 result",
                a.cell,
                b.burst_cap,
                2 * UNIT
            ));
        }
    }
    eprintln!(
        "  MultipleDataGrants under pacing ON: UNCONSTRUCTIBLE-BY-FROZEN-CONFIG-INVARIANT \
         (every observed burst_cap < {})",
        2 * UNIT
    );
    eprintln!(
        "  OneDataGrant under pacing ON: CONSTRUCTIBLE and exercised by cell_3 / cell_7 above (full \
         paired oracle, predicates frozen before the result)."
    );

    assert!(
        reds.is_empty(),
        "DR-CS-3 shipping-default pacing RED:\n  - {}",
        reds.join("\n  - ")
    );
}

/// **SYNTHETIC POSITIVE CONTROL** — DR-CS-2 debiting, evaluated by the SAME helper.
///
/// ORACLE-SENSITIVITY EVIDENCE ONLY. A passing control proves the comparator can SEE a grant
/// substitution; it is NEVER evidence of shipping-product success.
#[test]
fn synthetic_debiting_is_caught_by_the_same_oracle() {
    let base = |plan: Vec<PlanEntry>| OracleInput {
        plan,
        tokens: 0,
        burst_cap: EXPECTED_BURST_CAP,
        rate: FLOOR_BPS,
        inflight: 0,
        sent_logical_len: 0,
        pending_lost_depth: 0,
    };
    // OFF: the single grant goes to DATA.
    let off = base(vec![PlanEntry {
        kind: "Data",
        seq: 0,
        unit_id: 1000,
        size: UNIT,
    }]);
    // ON: a treatment packet consumed that same grant instead — ONE physical substitution.
    let on = base(vec![PlanEntry {
        kind: "Duplicate",
        seq: 0,
        unit_id: 1000,
        size: UNIT,
    }]);

    let rep = evaluate_grant_displacement(&off, &on);
    eprintln!("\n===== SYNTHETIC POSITIVE CONTROL (oracle sensitivity ONLY) =====");
    eprintln!("  report = {rep:?}");

    // ONE physical substitution ⇒ EXACTLY one displaced identity. No dimension is ever summed.
    assert_eq!(
        rep.direct_displacement_count, 1,
        "one physical substitution must count exactly once"
    );
    assert_eq!(
        rep.displaced_data_ids.len(),
        1,
        "the displaced identity set must hold exactly the one replaced DATA unit"
    );
    assert!(rep.violation, "the oracle must flag the substitution");

    // The order divergence is a SEPARATE diagnostic dimension — it is reported alongside, never
    // summed into, the identity count.
    assert_eq!(
        rep.order_divergence, 1,
        "order divergence is reported separately"
    );
    // `displaced_data_ids` is a SUBSET of `missing_or_replaced_ids`: the substitution must appear
    // once in each, and the two must never be added together.
    assert_eq!(
        rep.missing_or_replaced_ids.len(),
        1,
        "the substituted DATA entry must appear exactly once in the superset dimension too — \
         `displaced_data_ids` is a subset of `missing_or_replaced_ids`, never a second event"
    );
}

/// A pure ORDER divergence (both DATA units still present, resequenced) must raise `violation`
/// WITHOUT fabricating any displaced identity.
#[test]
fn pure_order_divergence_raises_violation_without_fabricating_identity() {
    let e = |seq: u64, unit_id: u64| PlanEntry {
        kind: "Data",
        seq,
        unit_id,
        size: UNIT,
    };
    let base = |plan: Vec<PlanEntry>| OracleInput {
        plan,
        tokens: 0,
        burst_cap: EXPECTED_BURST_CAP,
        rate: FLOOR_BPS,
        inflight: 0,
        sent_logical_len: 0,
        pending_lost_depth: 0,
    };
    let off = base(vec![e(0, 1000), e(1, 1001)]);
    let on = base(vec![e(1, 1001), e(0, 1000)]);

    let rep = evaluate_grant_displacement(&off, &on);
    eprintln!("\n===== PURE ORDER DIVERGENCE CONTROL =====\n  report = {rep:?}");
    assert_eq!(
        rep.direct_displacement_count, 0,
        "no identity was displaced"
    );
    assert!(
        rep.displaced_data_ids.is_empty(),
        "no identity may be fabricated"
    );
    assert_eq!(rep.order_divergence, 2, "both positions diverge");
    assert!(
        rep.violation,
        "order divergence alone still raises violation"
    );
}
