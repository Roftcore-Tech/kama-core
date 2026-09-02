# FS-FUSION-CAL-1 — FRAMING CORRIGENDUM (binding; supersedes the earlier verdict)

> The earlier **CALIBRATION-NO-GO** in `FINAL-REPORT.md` is **WITHDRAWN AS INVALID**. It rested on a
> comparison the program's binding framing forbids and that is not a valid contrast in the data.
> Nothing in the authoritative datasets is invalidated. No experiment is re-run.

## 1. The violated constraint

The program's binding framing is stated in multiple committed documents:

- `docs/design/dr-cs-1b-a2-receiver-design-v2.md:20` — **"ARQ-vs-FEC" dili yasak** ("ARQ-vs-FEC language is forbidden").
- `reports/d1-exit/1b-fec-utility-boundary-observability.md:191` — *"Bu deney ARQ-vs-FEC değil; **ARQ bütün-kollarda-aktif**; ölçülen **marjinal-proaktif-FEC-faydası**."*
- `reports/d1-exit/1b-fuzzy-calibration-evidence.md:40` — *"r0 = ARQ+0-proaktif-FEC · r1/r2/r4 = ARQ+1/2/4-proaktif-FEC."*
- `reports/decision-requests/A2-CLOSURE-2-pre-registration.md:128`, `A2-F5-pre-registration.md:53`,
  `docs/tasks/dr-cs-1b-a2-icra-talimati-taslagi-v1.md:9` — same prohibition, repeatedly.

**ARQ is active in every arm.** The measured quantity is the **marginal utility of proactive redundancy
`r` on top of always-on ARQ** — never a contest between two rival mechanisms.

## 2. What the earlier analysis did wrong

`decision-regions.csv` labelled cells `ARQ_DOMINANT` / `FEC_DOMINANT` — the forbidden rival vocabulary —
and its decisive contrast was `A0-TAIL` vs `F0`, i.e.

    (tail-capable ARQ, r0)  vs  (frozen ARQ, r2)

which **changes both orthogonal axes at once** (ARQ capability *and* redundancy level). It is not a valid
contrast under any framing. The "harmful false positive" that drove the NO-GO (`burst/20/high`) came from
that invalid comparison — and, compounding it, was drawn from `PRODUCT_NETNS`, a dataset that **contains no
FEC arm at all** (both product arms are r0; they differ only in tail capability). A conclusion about FEC
could not legitimately be drawn from it.

## 3. The two orthogonal axes and actual coverage

| | r0 | r2 | r4 |
|---|---|---|---|
| **ARQ frozen (tail-blind)** | A0 / A0-FROZEN / product `off` | F0 | F3 (FS-1) |
| **ARQ tail-capable** | A0-TAIL / product `on` | **absent** | **absent** |

Valid single-axis contrasts only: `r0→r2`, `r2→r4` (at fixed ARQ capability), and `frozen→tail` (at fixed r).
The **(tail-capable, r>0) fusion cell is genuinely absent** from every authoritative dataset — the marginal
utility of proactive redundancy *on top of tail-capable ARQ* is **unmeasured**. That coverage gap is real and
was correctly identified before; it is the one finding that survives unchanged.

## 4. Corrected result — marginal utility of proactive FEC (r0 → r2), always-on ARQ

Artifacts: `marginal-utility-regions.csv`, `marginal-utility-summary.json` (same frozen rows, McNemar exact,
grouped bootstrap, Wilson CI; method otherwise unchanged).

**Adding proactive redundancy never reduced completion in any cell: `base_only = 0` in 21/21 R2A cells.**

| Stratum | Effect of r0 → r2 | Cost |
|---|---|---|
| S1 terminal-tail | **large positive** — e.g. 0/100→100/100, 32/100→97/100, 29/100→81/100 (p≈0) | +1017…+1212 B |
| S3 IID 20 % | **large positive** — 12/100→66/100, 12/100→62/100, 14/100→59/100 (p≈0) | **+772…+842 B (cheapest)** |
| S2 non-tail | small positive (+5…+8) where headroom existed; otherwise **no completion change** | +913…+1153 B |

Verdict counts (R2A, r0→r2): **15 POSITIVE / 5 NO_COMPLETION_CHANGE / 1 INDETERMINATE**.
Marginal utility of tail capability at r0: 17 POSITIVE (R2A), 18 POSITIVE (PRODUCT).
FS-1 (n=5) is underpowered as expected: no completion change detectable at r0→r2 or r2→r4.

**Reading:** proactive redundancy pays where there is completion *headroom* (deep tails, high IID loss) and
is merely cost where the flow already completes (~83–96 % in S2). That is a **cost-efficiency boundary, not a
harm boundary** — which is precisely the "marginal utility boundary" this program was designed to locate.

## 5. Status of the earlier artifacts

| Artifact | Status |
|---|---|
| `decision-regions.csv`, `decision-regions.v1-uncorrected.csv` | **SUPERSEDED — forbidden rival framing**; preserved, not deleted |
| `FINAL-REPORT.md` §2/§5 verdict (CALIBRATION-NO-GO) | **WITHDRAWN — invalid cross-axis contrast** |
| `calibration-v1.json` loss membership | **SUSPENDED** — derived from the rival framing; must be re-derived on the marginal-utility axis |
| `holdout-*.{csv,json}` | **VOID** — evaluated the withdrawn rival-framing policy |
| Canonical dataset, ingest ledger, `analysis-method.json` (endpoints/statistics) | **VALID — unchanged** |
| `fusion_policy.rs` safety architecture, default-OFF, ARQ zero-diff | **VALID — unchanged** (no calibration constant is baked into the sender) |
| Coverage gap: (tail-capable, r>0) unmeasured | **VALID — unchanged** |

**No verdict is asserted here.** A calibration verdict requires re-deriving the policy on the marginal-utility
axis and a fresh pre-registered holdout. Default remains **OFF**; product behaviour remains byte-unchanged.

## 6. Correct question for the next iteration

Not "ARQ or FEC", but: **at which runtime-observable operating point does adding redundancy `r` on top of
always-on ARQ buy enough completion to justify its bytes — and does that still hold once ARQ is
tail-capable (the unmeasured fusion cell)?**
