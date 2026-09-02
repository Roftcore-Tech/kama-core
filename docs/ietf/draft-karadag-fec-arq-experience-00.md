---
title: "Experimental Evidence on Adding Proactive FEC to an Always-On ARQ Transport"
abbrev: "FEC-over-ARQ Experience"
docname: draft-karadag-fec-arq-experience-00
category: info
submissiontype: independent
ipr: trust200902
area: "Web and Internet Transport"
keyword:
  - FEC
  - ARQ
  - QUIC
  - loss recovery
  - erasure coding
author:
  - name: Abdurrahman Karadag
    organization: Roftcore
    email: abdurrahmankaradag19@gmail.com
normative:
informative:
  RFC5052:
  RFC6363:
  RFC8406:
  RFC9000:
  RFC9002:

--- abstract

This document reports experimental results from the Kama transport
project on the marginal utility of adding proactive Forward Erasure
Correction (FEC) on top of an always-on retransmission (ARQ)
substrate in a recovery-oriented UDP transport.  The experiments were
designed to avoid a framing error the authors consider common in this
space: FEC and ARQ were never compared as rival mechanisms; every
experimental arm runs ARQ, and the measured quantity is the marginal
effect of proactive redundancy added on top of it.

Three results are believed to be directly useful to ongoing efforts
to add FEC to reliable transports such as QUIC: (1) the apparent FEC
advantage on trailing-loss ("tail") patterns largely disappears once
the ARQ substrate is given a bounded probe-timeout tail-recovery
mechanism, while (2) the FEC advantage on body-spread (IID) loss is
genuine, unique, and cheap, and (3) an FEC activation policy keyed on
observed loss RATE alone is not derivable from the data -- at the same
20% loss rate, IID-structured loss wants coding and burst-structured
loss does not, so activation requires a loss-STRUCTURE observable.
The document also reports a measured emission-model effect: with
parity emitted at block close, the wall-clock benefit of proactive
FEC is confined to low-RTT regimes even though combinatorial
decodability is RTT-independent.  All retractions and corrections
binding these numbers are reproduced alongside them.

--- middle

# Introduction

Several proposals exist for adding FEC to QUIC and other reliable
transports (see {{RFC8406}} for coding-in-transport terminology, and
{{RFC5052}}/{{RFC6363}} for FEC framework precedents).  A recurring
difficulty in evaluating such proposals is that "FEC versus ARQ"
comparisons are usually ill-posed: any deployable reliable transport
retransmits, so the deployable question is not which mechanism wins
but what marginal value proactive redundancy adds on top of
retransmission, and under which loss structures.

The Kama project (a recovery-oriented UDP transport for hostile
networks: high loss, low bandwidth, jamming) ran a series of
pre-registered, frozen-protocol experiments between July and August
2026 to answer exactly that marginal question.  This document
publishes the results, including every correction and retraction that
binds them, in the belief that they are useful input to QUIC+FEC
design discussions.

This document makes no claims about any transport other than the one
measured, and no product or interoperability claims.  It is an
experiment report.

## Binding framing rule {#framing}

All experiments below obey one rule, stated here because violating it
invalidates the analysis and because the authors violated it once
themselves and had to retract a verdict (see {{withdrawn}}):

> ARQ is active in every arm.  The measured quantity is the marginal
> utility of proactive redundancy `r` on top of always-on ARQ --
> never a contest between two rival mechanisms.

Two orthogonal axes describe every arm:

* ARQ-capability axis: "tail-blind" (a purely ACK-driven ARQ with no
  retransmission timer; a trailing loss with no subsequent ACK is
  unrecoverable) versus "tail-capable" (the same ARQ plus a bounded
  probe-timeout tail-recovery mechanism, inspired by, but not
  claiming conformance to, the PTO of {{RFC9002}}).
* Redundancy axis: `r` proactive repair symbols per block
  (Reed-Solomon, block size K=8 application units, symbol size E=64
  octets in these experiments), with r=0 meaning ARQ-only.

Valid single-axis contrasts are r0-to-r2 and r2-to-r4 at fixed ARQ
capability, and tail-blind-to-tail-capable at fixed r.  Contrasts that
move both axes at once are diagnostic only.

# Experimental setups

Five campaigns contribute data.  Each was pre-registered and frozen
before outcomes were observed; arms, cell grids, seeds, horizons and
analysis rules could not be changed after results were seen.

## FS-1: in-process screening (240 rows)

A deterministic in-process bottleneck-channel model (explicitly NOT
kernel networking, netem, or a real NIC).  12 cells: RTT {50, 250,
1000} ms x loss condition {exact-deficit d in {1,2,4}, IID p=0.20};
n=5 per cell (screening only; no asymptotic p-values).  Arms: A0
(tail-blind ARQ, r0), A1-EARLY (blind duplication of r=2 source
symbols, 1212 configured intervention bytes), F0 (coding, r=2, 1212
configured bytes), F3 (coding, r=4, 2424 bytes).  Workload is bulk
only: 8 x 300-byte units = 48 source symbols, multi-block.

## DR-ARQ-TAIL-1 R2A: the deconfounder (8400 rows)

Same in-process channel, disjoint seed domain.  4 arms {A0-FROZEN,
A0-TAIL, F0, A1-EARLY} x 21 cells x N=100.  A0-TAIL adds an
experiment-only bounded tail probe (PTO = 2*RTT, at most 2 probes,
armed only while data is outstanding, cancelled on ACK progress).
Strata separate terminal-tail loss (S1), same-budget non-tail loss
(S2), and IID 20% loss (S3).  Burst loss (S4) and probe-loss (S5)
strata were split out pre-outcome and never executed (see
{{limitations}}).

## DR-ARQ-PRODUCT-TAIL-1: real-wire validation (2290 rows)

The product sender over a faithful two-network-namespace kernel netem
path (netem applied and verified on both egress directions; RTT and
loss acceptance-tested on-path before any measured run).  Arms: OFF
(product default, tail-recovery disabled) versus ON (bounded
tail-recovery, max 3 probes, Karn-style initial PTO).  3 loss
structures (IID, Gilbert-Elliott burst, deterministic terminal-tail)
x loss {2,5,10,20}% x RTT {100, 500, 2000} ms (as 2*one-way-delay of
{50,250,1000} ms) x N=30.  This campaign contains NO FEC arm; both
arms are r0.  It answers only the ARQ-capability-axis question on a
real kernel path.

## Wall-clock FEC utility (two-netns netem, 48 runs)

The hybrid FEC+ARQ path over the same faithful two-netns netem
harness, measured RTTs 250.09 / 1000.18 / 2500.33 ms
(configured-exact).  Arms r0..r4 on top of always-on ARQ.  Measures
actual arrival times: T_fec (parity arrival relative to block first
packet) versus T_arq (retransmitted-repair arrival).

## FS-FUSION-CELL-1: the fusion cell (in-process, N=12)

Adds the one cell absent from all other data: proactive redundancy on
top of TAIL-CAPABLE ARQ.  Arms: tail-only (tail-capable, r0), fusion
(tail-capable, r2), coding-only (tail-blind, r2), exactly paired per
seed.  N=12 per cell (the erasure generator's structural maximum, not
a power calculation; no p-values claimed).

# Results

Numbers are reproduced verbatim from frozen analysis artifacts.
Where a correction or retraction binds a number, it is stated
immediately; omitting them would misrepresent the evidence.

## Coding beats blind duplication at equal configured budget {#coding-vs-dup}

The cleanest pro-FEC result in the corpus.  In R2A, over 2100 paired
rows of F0 (coding, r=2) versus A1-EARLY (blind duplication, r=2) at
equal CONFIGURED intervention budget (1212 bytes both):

* F0-only completions: 868.  A1-only completions: 0.
  (Example cell: terminal-tail RTT50 d1 -- F0 100/100 versus
  A1-EARLY 0/100 at equal 6060 realized bytes.)

Correction that binds this number: the original report phrase
"matched realized-wire budget" was RETRACTED.  What was matched is
the configured intervention budget; realized total wire was equal in
3/21 cells and LOWER for F0 in 18/21, never higher.  The correction
strengthens rather than weakens the result: coding completed 868 rows
duplication could not, at equal-or-lower realized cost.

FS-1 independently produced the same signal in its own seed domain:
oracle class O2_FEC_SPECIFIC (F0 better than both no-redundancy AND
duplication) in 12 of 12 cells, with blind duplication beating
no-redundancy in only 3 of 12 -- duplication mostly buys nothing that
coding does not buy better.

Design note for QUIC+FEC: within a block, a Reed-Solomon repair
symbol protects ANY r losses, while a duplicate protects one specific
packet; at equal budget this combinatorial advantage was decisive in
every measured cell.  This result is NOT generalized to burst loss
(never measured with a coding arm; see {{limitations}}).

## Tail loss is a retransmission-timer problem, not a coding problem {#tail}

FS-1's terminal-tail cells initially looked like a strong FEC
use-case.  The deconfounding campaign showed most of that advantage
was an artifact of the frozen ARQ's tail blindness:

* R1 diagnostic: of 5 terminal-tail configurations the tail-blind
  ARQ censors, the bounded tail probe closes 5/5; coding (F0) closes
  only 2/5.
* R2A C1 (valid single-axis contrast): adding the tail probe took
  terminal-tail cells from 0/100 to 100/100 (RTT50 d1, +101 realized
  bytes -- one retransmit) and was never harmful in any cell.
* FS-FUSION-CELL-1 (valid single-axis contrast): with tail-capable
  ARQ, all nine exact-deficit tail cells complete 12/12 with r=0;
  adding r=2 keeps 12/12 and adds roughly +1111 bytes per run --
  coding adds nothing on tail loss once the timer exists.
* Real wire (product campaign): terminal-tail horizon completions
  went from 8/360 (2.2%) with tail-recovery OFF to 329/360 (91.4%)
  with it ON, at +1..4 retransmitted units per run, probe count
  bounded at its cap of 3, zero invariant failures in 2290 rows, and
  zero spurious retransmissions across all adversarial rows
  (reordering, delayed ACKs, jitter, path pause, RTT step).

A caution recorded in the corpus itself: the R2A C2 contrast
(F0 versus A0-TAIL) that first suggested this conclusion moves both
axes at once and is under a formal framing objection; the conclusion
is therefore cited here from the valid single-axis contrasts above,
which reach it independently.

Design note for QUIC+FEC: QUIC already has PTO {{RFC9002}}, i.e. it
is already "tail-capable".  Evaluations of FEC extensions against a
deliberately tail-blind baseline will overstate FEC benefit on
trailing-loss patterns.  The measured failure mode of a FIXED probe
cap is also worth recording: with cap=3, a 3-loss tail at 10% loss /
RTT 500 ms exhausted probes before packet-threshold detection engaged
(0/30 recovery in that one cell), while the 4-loss variant recovered
-- probe budgets should adapt to outstanding-tail length rather than
being constant.

## Body-spread loss is where coding uniquely pays {#iid}

The complement of {{tail}}, from the same valid contrasts:

* FS-FUSION-CELL-1 IID cells (p=0.20): tail-capable ARQ alone
  completes 0/12 in all three cells; adding r=2 completes 9, 5 and 9
  of 12.  A tail probe has nothing to aim at when loss is spread
  through the body of the block.
* R2A S3 (IID 20%): adding r=2 over the (tail-blind) ARQ base took
  completion from 12/100 to 66/100, 12/100 to 62/100, 14/100 to
  59/100 -- and the coding arm was the CHEAPEST intervention
  (+772..842 realized bytes, versus +1017..1212 on tail cells).
* Marginal-utility summary over all 21 R2A cells: adding proactive
  redundancy NEVER reduced completion ("base_only = 0 in 21/21");
  verdicts were 15 POSITIVE, 5 NO_COMPLETION_CHANGE, 1 INDETERMINATE.
  Where flows already complete (83-96% in S2), redundancy is merely
  cost -- a cost-efficiency boundary, not a harm boundary.

The summary sentence the project adopted: tail loss and body loss are
different problems; the timer and the code are not competitors -- each
is nearly useless where the other works.

## Loss RATE alone cannot gate FEC activation {#structure}

Two independent lines of evidence:

* A pooled re-analysis (10930 rows across the three domains, never
  pooled across domains; group-disjoint holdout by RTT stratum)
  attempted to derive an activation boundary from observed loss rate.
  The boundary degenerated (199999 -> 200000 ppm), because at the
  same 200000 ppm the data contains both coding-preferred (IID) and
  ARQ-preferred (burst, real-wire) outcomes.  The holdout produced a
  harmful false positive precisely on a burst cell.  Verbatim
  conclusion: "Loss rate alone is not a separating feature."
* A shipped fuzzy controller calibrated on rate alone planned ZERO
  parity at exactly 20% loss -- the operating point where the
  measurements above say coding is the only mechanism that completes
  anything (IID) -- and 250 permille at 25%.  The knee and the need
  sit at the same value on opposite sides.  The recorded fix
  direction is not "move the knee" but "give the gate a
  loss-structure input" (e.g. a burst-run-length observable).

Design note for QUIC+FEC: an adaptive FEC extension whose activation
predicate is a loss-rate threshold is unsupported by this data; some
structure statistic (run-length / gap distribution, e.g. a
Gilbert-Elliott p11-p01 discriminator) appears necessary.  Note that
no runtime telemetry was stored alongside outcomes in these
campaigns, so no runtime-feature-to-action map could be fitted at
all; this is stated as a limitation, not as evidence of
impossibility.

## The wall-clock benefit of FEC is gated by the emission model {#emission}

On the real netem path with parity emitted AT BLOCK CLOSE (after all
K source units dispatched, source dispatch ACK-clocked):

* At RTT 250 ms a frozen pre-declared deadline interval exists:
  p99(T_fec) = 293..296 ms is strictly below p01(T_arq) = 315..328
  ms across d1/d2/d4.  Marginal completion-latency gains measured at
  -46 / -52 / -77 ms for d1/d2/d4; the FEC path completed 30/30
  blocks in all three cells.
* At RTT 1000 and 2500 ms the interval disappears (e.g. RTT1000 d2:
  T_fec p99 = 2266 ms versus T_arq p01 = 1038 ms) and FEC-path usage
  collapses (15/30 -> 6/30 -> 0/30 as r rises): block close is
  delayed by roughly one RTT, so parity arrives level with or after
  the retransmitted repair.
* Combinatorial decodability (r >= d) is RTT-INDEPENDENT (verified in
  an isolated anchor at all three RTTs).  Only the wall-clock latency
  advantage is RTT-gated, and the report marks this
  "emission-model-bound": parity sent in-flight alongside source
  data could win at high RTT as well.  The project could not measure
  that variant because its sender seals a data-before-parity
  emission ordering (interleaved parity was dropped pre-campaign as
  a sealed-ordering conflict).

Corrections binding this section: the historical claim "r >= d meets
the 305 ms deadline at ~100% for deficits 1/2/4" was PARTIALLY
RETRACTED after exact recomputation from the raw receiver ledger:
d1 = 30/30 (100%), d2 = 28/30 (93.3%), d4 = 4/30 (13.3%) -- at
deficit 4 even the parity arm misses the deadline, so
"FEC-essential" holds only for deficit <= 2.  Also, true zero-parity
baselines at d2/d4 were never run at RTT250 (the r<d comparators are
one-short arms); this data hole is recorded, not hidden.

Design note for QUIC+FEC: proposals that emit repair symbols
opportunistically in-flight (rather than at block boundaries) target
exactly the regime where this data shows block-close emission losing
its advantage; this experiment supplies a measured baseline for that
argument, not a refutation of in-flight designs.

## Deadline endpoint honesty {#deadline}

Every "completion" number above is completion within a pre-registered
OBSERVATION HORIZON (699/2499/9249 ms per RTT tier), not a deadline
hit.  Against the frozen tight deadlines (199/599/2099 ms), ALL
in-process arms scored ~0 (A0-TAIL: 46/2100; every other arm 0/2100),
and the real-wire campaign scored 0.0 in every terminal-tail cell in
both arms.  None of these mechanisms is a real-time success under
those deadlines; the contrasts in this document are about recovery,
not deadline attainment.  Conditional completion-latency medians are
computed over completed rows only, whose populations differ per arm,
and are not evidence of unconditional latency superiority.

# A withdrawn verdict, reported for methodological value {#withdrawn}

An earlier calibration campaign in this project concluded
"CALIBRATION-NO-GO" from cells labelled ARQ_DOMINANT/FEC_DOMINANT.
That verdict was WITHDRAWN AS INVALID: its decisive contrast compared
(tail-capable ARQ, r0) against (tail-blind ARQ, r2) -- moving both
axes at once -- and drew a harmful false positive from a dataset that
contains no FEC arm at all.  The authors publish this mistake because
it is exactly the error the "FEC versus ARQ" framing invites, and any
QUIC+FEC evaluation matrix can fall into it: keep the
retransmission-capability axis and the redundancy axis orthogonal,
and never read a two-axis contrast as a mechanism verdict.

# Limitations {#limitations}

In the spirit of {{framing}}, the following are explicit non-claims.

1. Burst loss was never measured with a coding arm.  The burst
   stratum (S4) was split out pre-outcome and not executed; the only
   burst measurements (real wire, Gilbert-Elliott) contain no FEC
   arm.  No result here says what happens when a burst run-length
   exceeds r within a block.  For jamming-motivated deployments this
   is the largest open gap.
2. All coding-arm results are from a deterministic in-process channel
   model.  Only the ARQ-capability axis (tail probe) and the r0..r4
   wall-clock campaign crossed a real kernel path.  No
   identical-numeric replication between the two environments is
   claimed.
3. Congestion-induced loss is not modelled anywhere; nothing here
   measures parity bytes competing with a congestion controller,
   which is a central objection to FEC in reliable transports.
4. Sample sizes: FS-1 is n=5 screening (no p-values);
   FS-FUSION-CELL-1 is N=12 (generator maximum).  The N=100 and N=30
   campaigns carry pre-registered Wilson-interval precision targets.
5. Bulk workload only (48 symbols, multi-block).  Emission-timing
   results must not be generalized to single-message/single-block
   traffic; a pre-registered single-block stratum is required before
   any product-wide verdict (recorded as a binding boundary).
6. The (tail-capable ARQ, r>0) fusion cell has only N=12 screening
   evidence; the redundancy optimum (r=4 on that axis) is unmeasured.
7. The tail-recovery mechanism here is a bounded probe inspired by
   PTO; no conformance to {{RFC9002}} is claimed.
8. No comparison against QUIC/TCP implementations appears in this
   document; those campaigns are reported separately and none of
   their numbers are used here.

# Applicability to QUIC FEC extensions

Condensed, with the caveats above:

* Evaluate FEC extensions against a PTO-capable baseline; a
  tail-blind baseline overstates FEC benefit ({{tail}}).
* Expect the genuine, unique FEC win on body-spread loss, and expect
  it to be cheap relative to duplication or extra retransmissions
  ({{iid}}, {{coding-vs-dup}}).
* Do not gate FEC on loss rate; carry a loss-structure observable
  ({{structure}}).
* Emission scheduling is first-order: block-close emission confines
  wall-clock gains to low RTT even when decodability is guaranteed
  ({{emission}}).
* Keep redundancy-never-harms in perspective: completion was never
  reduced by adding r, but where completion headroom is absent,
  redundancy is pure byte cost ({{iid}}).

# Security Considerations

The experiments concern loss-recovery scheduling only.  In the
measured transport, FEC encoding operates on plaintext blocks and
each shard is independently AEAD-protected afterwards
(code-then-encrypt; receivers authenticate before decoding), and the
transport treats secured payloads as opaque.  None of the results
here alter the security properties of a host transport; a transport
adding FEC should ensure repair symbols receive the same
confidentiality and integrity protection as source symbols, and
should account for the amplification potential of proactive
redundancy toward unauthenticated peers.

# IANA Considerations

This document has no IANA actions.

--- back

# Acknowledgments
{:numbered="false"}

The experimental discipline reported here (pre-registration, frozen
analysis, adversarial verification, mandatory corrigenda) was
enforced by the project owner; errors of presentation in this
document are the author's.

# Data availability
{:numbered="false"}

Frozen datasets, pre-registrations, analysis artifacts and the
corrigenda quoted here are archived in the project repository under
reports/fs-1/analysis/v1.3.2/, reports/dr-arq-tail-1/v1/,
reports/dr-arq-product-tail-1/, reports/d1-exit/ and
reports/fs-fusion-cell-1/, each with content hashes recorded at
freeze time.
