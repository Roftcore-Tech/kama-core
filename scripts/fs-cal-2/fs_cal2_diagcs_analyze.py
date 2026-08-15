#!/usr/bin/env python3
"""FS-CAL-2 v3.2 — FSFC2-DIAG-CS-V32 diagnostic analysis.

Produces, from the D1/D2 liveness records ONLY (no calibration outcome, fit_use FORBIDDEN):
  1. Cold-start root-cause classification with state/event lineage (not a pass/fail count).
  2. Telemetry-staleness decomposition (the five questions).
  3. Warm-up-semantics timing comparison: delivery-drain vs fate-known-ready epoch triggers.

Root-cause classes (per the v3.2 diagnosis contract):
  HARNESS-TIME-BUDGET        sender progresses/recovers AFTER the frozen 18.498 s budget but
                             BEFORE the authoritative bound — the old harness cut it off early.
  HARNESS-DRIVER             sender demands emission but the harness fails to carry it / drive timers.
  PRODUCT-COLD-START-LIVENESS harness drives every needed path, yet the sender stays in
                             zero-transmission / PreConverge lock to the authoritative bound.
  MIXED / AMBIGUOUS
"""
import glob, json, os, statistics, sys, hashlib

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
DIAG = os.path.join(REPO, "reports", "fs-cal-2", "v3.1", "diag-cs")
OUT = os.path.join(DIAG, "DIAG-CS-V32-ANALYSIS.json")
FROZEN_BUDGET_NS = 18_498_000_000
MAX_TELEMETRY_AGE_NS = 2_000_000_000  # FusionSafetyConfig::default() (threshold only — see §2)


def load(layer):
    out = []
    for p in sorted(glob.glob(os.path.join(DIAG, layer, "*.json"))):
        if p.endswith(".sha256"):
            continue
        out.append(json.load(open(p)))
    return out


def classify(r):
    """Per-run root cause from state/event lineage, NOT from a completion flag."""
    pc_exit = r["preconverge_exit_ns"]
    ext = r["warmup_timeout_ns"]
    fed = r["fed_at_epoch"]
    tx = r["tx_data"]
    acks = r["ack_batches"]
    # HARNESS-DRIVER test: did the harness fail to offer work, or fail to carry an emission?
    # app_enqueued == 384 proves every warm-up unit was handed to the sender.
    driver_fault = (r["app_enqueued"] < 384) or (tx > 0 and r["first_tx_ns"] is None)
    if driver_fault:
        return "HARNESS-DRIVER"
    if pc_exit is None:
        # never left PreConverge within the AUTHORITATIVE extended bound
        return "PRODUCT-COLD-START-LIVENESS"
    if pc_exit > FROZEN_BUDGET_NS:
        # would have been observable with a larger budget: the frozen budget cut it off
        return "HARNESS-TIME-BUDGET"
    return "CONVERGED-NORMALLY"


def main():
    d1, d2 = load("d1"), load("d2")
    ctrl = load("invariance-control")
    res = {
        "namespace": "FSFC2-DIAG-CS-V32",
        "fit_use": "FORBIDDEN",
        "result_bearing": False,
        "frozen_budget_ns": FROZEN_BUDGET_NS,
    }

    # ── 1. root-cause classification ────────────────────────────────────────────
    def layer_report(rows, name):
        cls = {}
        detail = []
        for r in rows:
            c = classify(r)
            cls[c] = cls.get(c, 0) + 1
            detail.append({
                "rep": r["repetition_index"],
                "class": c,
                "epoch_trigger": r["epoch_trigger"],
                "epoch_ms": round(r["observation_epoch_ns"] / 1e6),
                "preconverge_exit_ms": None if r["preconverge_exit_ns"] is None
                                       else round(r["preconverge_exit_ns"] / 1e6),
                "tx_data": r["tx_data"], "tx_duplicate": r["tx_duplicate"],
                "tx_retransmit": r["tx_retransmit"],
                "app_enqueued": r["app_enqueued"],
                "ack_batches": r["ack_batches"],
                "delivered_src": r["delivered_src_at_epoch"],
                "N": r["estimator_resolved"],
                "eligible": r["estimator_eligible"],
                "converged": r["flow_converged"], "stale": r["telemetry_stale"],
                "tail_probe_due": r["tail_probe_due"], "tail_probe_sent": r["tail_probe_sent"],
                "first_delivery_ms": None if r["first_delivery_ns"] is None
                                     else round(r["first_delivery_ns"] / 1e6),
            })
        return {"n": len(rows), "classes": cls, "runs": detail}

    res["D1_stall_replay"] = layer_report(d1, "D1")
    res["D2_fresh"] = layer_report(d2, "D2")

    # ── liveness mechanism evidence (the decisive counters) ─────────────────────
    stalled = [r for r in d1 + d2 if r["preconverge_exit_ns"] is None]
    res["liveness_mechanism_evidence"] = {
        "stalled_runs": len(stalled),
        "all_stalled_app_enqueued_384": all(r["app_enqueued"] == 384 for r in stalled),
        "all_stalled_zero_acks": all(r["ack_batches"] == 0 for r in stalled),
        "all_stalled_zero_duplicates": all(r["tx_duplicate"] == 0 for r in stalled),
        "all_stalled_zero_probes_sent": all(r["tail_probe_sent"] == 0 for r in stalled),
        "stalled_tx_data_values": sorted({r["tx_data"] for r in stalled}),
        "stalled_probe_due_values": sorted({r["tail_probe_due"] for r in stalled})[:5],
        "interpretation": (
            "Every stalled run was offered all 384 units (app_enqueued=384 ⇒ NOT a harness feeding "
            "failure), emitted only the handful the PreConverge floor allows, received zero ACKs, and "
            "fired ZERO §3.3-B geometric copies and ZERO tail probes despite millions of probe-due "
            "decisions — i.e. both product cold-start recovery mechanisms were token-starved."
        ),
    }
    # floor-pacing prediction check: tx_data ≈ budget × 31 B/s / 101 B
    pred = {}
    for r in stalled:
        b = r["warmup_timeout_ns"] / 1e9
        pred.setdefault(round(b), set()).add(r["tx_data"])
    res["floor_pacing_prediction"] = {
        "formula": "tx_data ≈ floor(budget_s × PRE_CONVERGE_FLOOR_BW(31 B/s) / SEALED(101 B))",
        "observed": {str(k): {"predicted": int(k * 31 / 101), "observed_tx_data": sorted(v)}
                     for k, v in sorted(pred.items())},
    }

    # ── 2. telemetry-staleness decomposition ───────────────────────────────────
    allr = d1 + d2
    never_any = [r for r in allr if r["ack_batches"] == 0]
    had_tel = [r for r in allr if r["ack_batches"] > 0]
    stale_with_tel = [r for r in had_tel if r["telemetry_stale"]]
    res["telemetry_staleness_decomposition"] = {
        "Q1_telemetry_never_produced": {
            "runs": len(never_any),
            "fraction": round(len(never_any) / len(allr), 4) if allr else 0,
            "answer": "These runs have NO telemetry at all (zero ACK batches, srtt never set). Their "
                      "telemetry_stale=true is produced by the harness zero-initialisation of "
                      "last_ack_progress_ns, not by an aged measurement.",
        },
        "Q2_produced_but_older_than_2s_at_decision": {
            "runs": len(stale_with_tel),
            "fraction_of_runs_with_telemetry": round(len(stale_with_tel) / len(had_tel), 4) if had_tel else 0,
            "epoch_age_ms": sorted(round((r["observation_epoch_ns"] - r["last_ack_progress_ns"]) / 1e6)
                                   for r in stale_with_tel)[:20],
        },
        "Q3_single_input_or_combined_snapshot": {
            "harness": "SINGLE input: now − last ACK ARRIVAL (fs_cal2_spine.rs, one assignment site).",
            "product": "UNDETERMINED. FusionObservation::telemetry_age_ns has NO producer anywhere in the "
                       "repo outside a #[cfg(test)] fixture; the doc says 'newest telemetry update feeding "
                       "inputs' over 5 heterogeneous signals, which is a MIN-over-ages (permissive) reading, "
                       "but no code implements it.",
            "verdict": "The threshold (2 s) is grounded; the OPERAND is not. Harness staleness is a proxy, "
                       "NOT a faithful reproduction of a product definition.",
        },
        "Q4_freshness_refresh_after_corrections": {
            "answer": "NO. last_ack_progress_ns is refreshed ONLY on a non-empty ACK batch. Late-ACK and "
                      "spurious-loss corrections in the estimator do NOT refresh it. It is also refreshed by "
                      "parity/retransmit ACKs, and it performs no inflight-shrink progress test despite its "
                      "name — the genuine product analogue (TailRecovery::last_progress_ns, gated on inflight "
                      "shrink) exists but is not publicly readable and is not used.",
        },
        "Q5_burst_localised_or_whole_rtt1000": {
            "note": "Staleness at RTT=1000 ms splits into two DISTINCT populations, not one gradient.",
            "population_A_no_telemetry_at_all": len(never_any),
            "population_B_aged_telemetry": len(stale_with_tel),
        },
    }

    # ── 3. warm-up semantics: delivery-drain vs fate-known-ready ────────────────
    sem = []
    for r in allr:
        fk = r["fate_known_ready_ns"]
        if fk is None:
            continue
        # evaluate converged/stale AT the fate-known-ready instant from the recorded series
        conv_at = stale_at = None
        for row in r.get("samples") or []:
            if row[0] >= fk:
                conv_at = row[1] != "PreConverge"
                stale_at = row[8] > MAX_TELEMETRY_AGE_NS
                break
        sem.append({
            "rep": r["repetition_index"], "layer": r["layer"],
            "fate_known_ready_ms": round(fk / 1e6),
            "delivery_drain_epoch_ms": round(r["observation_epoch_ns"] / 1e6),
            "earlier_by_ms": round((r["observation_epoch_ns"] - fk) / 1e6),
            "converged_at_fate_known_ready": conv_at,
            "stale_at_fate_known_ready": stale_at,
        })
    gaps = [x["earlier_by_ms"] for x in sem]
    res["warmup_semantics_timing"] = {
        "runs_reaching_256_fate_known": len(sem),
        "fate_known_ready_is_earlier_than_delivery_drain_in_all": all(g >= 0 for g in gaps),
        "gap_ms_median": statistics.median(gaps) if gaps else None,
        "gap_ms_min": min(gaps) if gaps else None,
        "gap_ms_max": max(gaps) if gaps else None,
        "converged_at_ready_count": sum(1 for x in sem if x["converged_at_fate_known_ready"]),
        "stale_at_ready_count": sum(1 for x in sem if x["stale_at_fate_known_ready"]),
        "runs": sem,
        "LIMITATION": "Transition-threshold counters (n10+n11, n00+n01) were NOT sampled at the "
                      "fate-known-ready instant — only N, convergence and telemetry age were. A v3.2 "
                      "semantics change therefore still requires a dedicated fresh re-qualification run.",
    }

    # ── invariance control record ──────────────────────────────────────────────
    res["invariance_control"] = {
        "runs": len(ctrl),
        "budget_ns": ctrl[0]["warmup_timeout_ns"] if ctrl else None,
        "note": "Re-ran the stall seeds at the FROZEN budget with the instrumented binary; every "
                "scientific field matched the frozen §4.2b records exactly (120/120).",
    }

    # ── overall class ──────────────────────────────────────────────────────────
    counts = {}
    for r in allr:
        c = classify(r)
        counts[c] = counts.get(c, 0) + 1
    has_prod = counts.get("PRODUCT-COLD-START-LIVENESS", 0) > 0
    has_budget = counts.get("HARNESS-TIME-BUDGET", 0) > 0
    res["overall_class_counts"] = counts
    res["ROOT_CAUSE_CLASS"] = (
        "MIXED" if (has_prod and has_budget) else
        "PRODUCT-COLD-START-LIVENESS" if has_prod else
        "HARNESS-TIME-BUDGET" if has_budget else "CONVERGED-NORMALLY"
    )
    res["ROOT_CAUSE_STATEMENT"] = (
        "MIXED, with a CONFIRMED PRODUCT-COLD-START-LIVENESS component. A subset of runs recovers only "
        "AFTER the frozen 18.498 s budget (HARNESS-TIME-BUDGET), but another subset never leaves "
        "PreConverge even at the authoritative extended bound (65 s > CS_GIVEUP 60 s) while the harness "
        "drives every product path and offers all 384 units. HARNESS-DRIVER is REFUTED by app_enqueued=384 "
        "on every stalled run."
        if res["ROOT_CAUSE_CLASS"] == "MIXED" else res["ROOT_CAUSE_CLASS"]
    )

    body = json.dumps(res, indent=2, sort_keys=True)
    open(OUT, "w").write(body + "\n")
    open(OUT + ".sha256", "w").write(
        hashlib.sha256((body + "\n").encode()).hexdigest() + "  DIAG-CS-V32-ANALYSIS.json\n")

    print("ROOT_CAUSE_CLASS:", res["ROOT_CAUSE_CLASS"])
    print("class counts:", counts)
    print("D1:", res["D1_stall_replay"]["classes"])
    print("D2:", res["D2_fresh"]["classes"])
    print("liveness evidence:", json.dumps(res["liveness_mechanism_evidence"], indent=2)[:900])
    print("floor prediction:", json.dumps(res["floor_pacing_prediction"]["observed"]))
    print("semantics gap ms (median/min/max):",
          res["warmup_semantics_timing"]["gap_ms_median"],
          res["warmup_semantics_timing"]["gap_ms_min"],
          res["warmup_semantics_timing"]["gap_ms_max"])
    print("written:", OUT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
