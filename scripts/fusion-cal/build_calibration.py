#!/usr/bin/env python3
"""FS-FUSION-CAL-1 Phase E — emit the typed, versioned calibration artifact from the decision regions.

Calibrates ONLY what the authoritative data identifies. Dimensions the data does not vary
(bandwidth, jitter, queue_delay, ack_confidence) are RETAINED AT FIXTURE VALUES and explicitly
declared UNCALIBRATED/UNSUPPORTED — never silently "tuned". Consequent levels are retained.

Grounding (from reports/fs-fusion-cal-1/decision-regions.csv):
  * ARQ_TAIL Pareto-dominates sparse terminal tails: drop 1..4 of 48 units => 20_833..83_333 ppm.
    (equal completion vs FEC_R2, ~1111 B cheaper)
  * FEC_R2 Pareto-dominates IID 20 % loss = 200_000 ppm at all three RTTs
    (59-66/100 vs ARQ_TAIL 12-14/100, AND lower realized wire)
=> the LOW/HIGH loss boundary must lie strictly between 83_333 and 200_000 ppm.
   Chosen from the evidence gap, not from an outcome-tuned search: LOW shoulder ends 90_000,
   fully HIGH at 200_000, with the transition band spanning the unobserved interval.
"""
import csv, json, os, hashlib

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "reports", "fs-fusion-cal-1")


def sha256_file(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def log2_fp(x):
    """Mirror of fusion.rs log2_fp: 1024*log2(x), integer, matching the Rust const fn."""
    if x == 0:
        return 0
    hi = x.bit_length() - 1
    frac = 0
    rem = x
    # replicate the Rust fixed-point routine's semantics closely enough for the retained values
    import math
    return int(1024 * math.log2(x))


def main():
    regions_p = os.path.join(OUT, "decision-regions.csv")
    canon_p = os.path.join(OUT, "canonical-calibration-data.csv")
    method_p = os.path.join(OUT, "analysis-method.json")
    regions = list(csv.DictReader(open(regions_p)))

    ev_arq = [r["cell_id"] for r in regions if r["preferred_action"] == "ARQ_TAIL"]
    ev_fec = [r["cell_id"] for r in regions if r["preferred_action"] == "FEC_R2"]

    art = {
        "calibration_id": "FS-FUSION-CAL-1",
        "calibration_version": "calibration-v1",
        "feature_schema_version": "FusionInputs-v1(loss_ppm,bandwidth_bps,jitter_ppm,qd_ratio_permille,ack_confidence_permille)",
        "created_from_source_commit": "1e8fe0ba (FS-FUSION-CAL-1 Phases B-D)",
        "source_dataset_hashes": {
            "canonical-calibration-data.csv": sha256_file(canon_p),
            "decision-regions.csv": sha256_file(regions_p),
            "analysis-method.json": sha256_file(method_p),
        },
        "authoritative_sources": {
            "FS1_IN_PROCESS": "reports/fs-1/raw/v1.3.2 (240)",
            "R2A_IN_PROCESS": "reports/dr-arq-tail-1/v1/raw (8400)",
            "PRODUCT_NETNS": "reports/dr-arq-product-tail-1/v1/result.csv (2290)",
        },
        "rollback_target": "FIXTURE_UNCALIBRATED (FuzzyConfig::fixture(); the current production default)",

        "membership": {
            "loss": {
                "status": "CALIBRATED",
                "unit": "ppm",
                "low": {"a": 0, "b": 0, "c": 90000, "d": 140000},
                "med": {"a": 90000, "b": 140000, "c": 160000, "d": 200000},
                "high": {"a": 160000, "b": 200000, "c": "U64_MAX", "d": "U64_MAX"},
                "fixture_was": {
                    "low": [0, 0, 200000, 300000],
                    "med": [200000, 300000, 500000, 600000],
                    "high": [500000, 600000, "U64_MAX", "U64_MAX"],
                },
                "defect_corrected": "The fixture makes mu_LOW = 1.0 at 200_000 ppm, i.e. 20 % loss is classified fully LOW, so rule R1 (loss LOW -> fec_zero) fires and FEC is never recommended in exactly the region where the authoritative data shows FEC_R2 Pareto-dominates.",
                "evidence_arq_side": {"cells": ev_arq, "loss_ppm_range": [20833, 83333]},
                "evidence_fec_side": {"cells": ev_fec, "loss_ppm": 200000},
            },
            "bandwidth": {
                "status": "UNCALIBRATED_RETAINED",
                "reason": "The authoritative data has NO bandwidth variation (every cell ran a 1 Mbit bottleneck with a 0.80C = 800 kbps pacer). Calibrating this dimension would be unfounded.",
                "retained_from": "FuzzyConfig::fixture() (log2_fp domain: 50k/100k/150k/200k B/s)",
            },
            "jitter": {
                "status": "UNCALIBRATED_RETAINED",
                "reason": "No jitter sweep in the in-process authoritative data; the product grid has a single jitter adversarial slice (n=10), insufficient to calibrate three membership bands.",
                "runtime_hazard": "rtt.rs bootstrap sets rttvar = srtt/2 on the FIRST sample => jitter_ppm = 500_000, which lands in the fixture jitter HIGH shoulder (400k-500k). Any jitter-conditioned rule is therefore untrustworthy until >=2 RTT samples exist. Encoded below as a low-confidence precondition.",
                "retained_from": "FuzzyConfig::fixture()",
            },
            "queue_delay": {"status": "UNCALIBRATED_RETAINED", "reason": "no queue-delay variation in the authoritative data", "retained_from": "FuzzyConfig::fixture()"},
            "ack_confidence": {"status": "UNCALIBRATED_RETAINED", "reason": "not varied as an independent factor", "retained_from": "FuzzyConfig::fixture()"},
        },

        "rule_table": {
            "status": "STRUCTURALLY_UNCHANGED",
            "rules": [
                "R1: loss LOW -> (fec_zero, arq_light)",
                "R2: loss HIGH & bw HIGH & jitter LOW -> (fec_high, arq_moderate)",
                "R3: loss MED & bw MED -> (fec_med, arq_med)",
                "R4: loss LOW & jitter HIGH -> (fec_zero, arq_light)",
                "SR1: bw LOW & qd HIGH -> sr_low, complement -> sr_high",
                "EMERGENCY: loss HIGH & bw LOW & ack_conf LOW",
            ],
            "consequents_retained": {"fec_zero": 0, "fec_med": 500, "fec_high": 800,
                                     "arq_light": 200, "arq_med": 500, "arq_moderate": 600,
                                     "sr_low": 500, "sr_med": 750, "sr_high": 1000},
            "STRUCTURAL_LIMITATION": "R2 (the only rule that can raise FEC to fec_high) is conjoined with 'bw HIGH', a dimension the authoritative data never varied and which the fixture places at 150-200 kB/s. The tested regime (~125 kB/s bottleneck) does not reach bw HIGH, so even with the corrected loss membership the calibrated policy CANNOT reach fec_high in the supported region. Correcting the loss membership is a NECESSARY but not SUFFICIENT condition for FEC activation. Changing R2's antecedent is a rule-structure change that this calibration deliberately does NOT make without evidence over bandwidth; it is recorded as backlog.",
        },

        "supported_region": {
            "loss_ppm": {"min": 0, "max": 200000, "note": "calibrated boundary evidence at 20_833-83_333 (ARQ_TAIL) and 200_000 (FEC_R2)"},
            "bandwidth_bps_bytes_per_sec": {"approx_min": 100000, "approx_max": 125000, "note": "the single tested regime"},
            "flow_state": "CONVERGED only (>=2 RTT samples; srtt present)",
            "rtt_ms": [50, 250, 1000],
            "workload_bytes": 4848,
        },
        "unsupported_regions": [
            {"region": "loss STRUCTURE (IID vs burst vs terminal-tail)", "reason": "burstiness / run-length / gap statistics are NOT computed anywhere in adaptive-transport; loss structure is not runtime-observable", "action": "ArqOnly"},
            {"region": "loss_ppm > 200000", "reason": "no authoritative evidence above the IID 20 % point", "action": "ArqOnly"},
            {"region": "bandwidth outside ~100-125 kB/s", "reason": "never varied in the authoritative data", "action": "ArqOnly"},
            {"region": "cold-start / PreConverge (no srtt, <2 RTT samples)", "reason": "jitter bootstrap artifact (rttvar=srtt/2 -> 500_000 ppm) + pacer floor token starvation", "action": "ArqOnly"},
            {"region": "simultaneous ARQ+FEC fusion", "reason": "NO authoritative arm combines tail-capable ARQ with proactive FEC; FUSION_DOMINANT is unreachable from existing data", "action": "ArqOnly"},
        ],
        "safety_requirements": {
            "note": "Hard caps live in an INDEPENDENT safety configuration; this calibration artifact can never raise them.",
            "required": [
                "version mismatch -> FallbackArq",
                "unsupported region -> FallbackArq",
                "low confidence / stale telemetry -> FallbackArq",
                "cold-start without valid observability -> FallbackArq",
                "invalid input (NaN/Inf/out-of-domain/unknown enum) -> FallbackArq",
                "FEC bytes <= hard_fec_budget; combined overhead <= hard_combined_cap",
                "policy never mutates ARQ/ACK/loss/RTT/PTO state",
            ],
        },
        "actuation_state": {
            "default": "OFF — production keeps FuzzyConfig::fixture(); enabling is explicit and versioned",
            "product_fec_actuation": "POLICY-ONLY: no product send-path FEC encoder exists (a decode path exists but is default-OFF and unreachable from any product sender). A calibrated FEC recommendation therefore CANNOT be actuated in product today.",
            "consequence": "verdict class is capped at CALIBRATION-GO — EXPERIMENTAL/LIMITED",
        },
    }

    p = os.path.join(OUT, "calibration-v1.json")
    tmp = p + ".tmp"
    with open(tmp, "w") as f:
        json.dump(art, f, indent=2, sort_keys=True)
        f.flush(); os.fsync(f.fileno())
    os.replace(tmp, p)
    d = sha256_file(p)
    open(p + ".sha256", "w").write(f"{d}  calibration-v1.json\n")
    print(f"calibration-v1.json sha256={d}")
    print(f"ARQ_TAIL-preferred cells: {len(ev_arq)} | FEC_R2-preferred cells: {len(ev_fec)}")


if __name__ == "__main__":
    main()
