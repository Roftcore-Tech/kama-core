#!/usr/bin/env python3
"""FS-CAL-2 v3.2 — FSFC2-DIAG-CS-V32 immutable diagnostic manifest.

Cold-start liveness diagnostic (D1 deterministic stall replay + D2 fresh disjoint-seed N=40).
These runs are LIVENESS/ROOT-CAUSE ONLY: fit_use = FORBIDDEN. They are namespace-disjoint from
every calibration dataset and must never enter training / support / holdout / fit.

The extended warm-up timeout is DERIVED FROM CODE (never estimated):

    extended_timeout >= max( authoritative ColdStartTimeout + guard,
                             last cold-start probe step + RTT + guard,
                             60 s )

with the guard justified from the sender's own timer resolution, the RTT cell, and the
PreConverge floor-pacing emission quantum.
"""
import hashlib, json, os, subprocess, sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "reports", "fs-cal-2", "v3.1", "diag-cs", "FSFC2-DIAG-CS-V32-MANIFEST.json")

# ── grounded product constants (exact file:line, verified by grep at build time) ──────────
GROUNDED = {
    "CS_GIVEUP_NS": {
        "value": 60_000_000_000,
        "symbol": "sender.rs::CS_GIVEUP_NS",
        "file_line": "crates/adaptive-transport/src/sender.rs:198",
        "literal": 'const CS_GIVEUP_NS: u64 = 60_000_000_000; // T_COLD_GIVEUP=60s (son-probe-45s + RTT_MAX-5s + marj)',
        "cfg_gate": '#[cfg(feature = "experimental-cold-start")]',
        "compiled_in_fs_cal_2": False,
        "note": "AUTHORITATIVE product cold-start give-up bound. NOT compiled under FS-CAL-2's DEFAULT features, "
                "so it does not act as a runtime escape hatch here; it is used as the authoritative timeout REFERENCE.",
    },
    "CS_PROBE_FIRES_NS": {
        "value": [3_000_000_000, 9_000_000_000, 21_000_000_000, 45_000_000_000],
        "symbol": "sender.rs::CS_PROBE_FIRES_NS",
        "file_line": "crates/adaptive-transport/src/sender.rs:192",
        "cfg_gate": '#[cfg(feature = "experimental-cold-start")]',
        "compiled_in_fs_cal_2": False,
        "last_step_ns": 45_000_000_000,
    },
    "PRE_CONVERGE_FLOOR_BW": {
        "value": 31,
        "units": "bytes/second",
        "symbol": "sender.rs::PRE_CONVERGE_FLOOR_BW",
        "file_line": "crates/adaptive-transport/src/sender.rs:726",
        "literal": "const PRE_CONVERGE_FLOOR_BW: u64 = 31;  // §3.3-A worst-corner prior pacing floor (B/s) ~0.25 kbps",
        "applied_at": "crates/adaptive-transport/src/sender.rs:2306 (compute_send_rate: PreConverge ⇒ return PRE_CONVERGE_FLOOR_BW)",
        "compiled_in_fs_cal_2": True,
    },
    "PC_COPY_MULTIPLIER": {
        "value": 2,
        "file_line": "crates/adaptive-transport/src/sender.rs:794",
        "compiled_in_fs_cal_2": True,
        "note": "§3.3-B geometric copy reschedule multiplier (interval ×= 2 on each fire).",
    },
    "PC_KBASE": {
        "value": "3/2",
        "file_line": "crates/adaptive-transport/src/sender.rs:792-793",
        "compiled_in_fs_cal_2": True,
    },
    "PreConverge_exit_rule": {
        "file_line": "crates/adaptive-transport/src/sender.rs:56-58, 2066-2072",
        "literal": "§3.3 cold-start: ... loss-ilanı/PTO/emergency/ack_confidence DONDURULUR. İlk geçerli SACK'te → Startup.",
        "operative": "PreConverge is left ONLY on the first valid SACK. PTO/time-threshold loss declaration is FROZEN "
                     "while in PreConverge (sender.rs:1847 guards it).",
    },
}

SEALED = 101          # workload sealed unit bytes (fs_cal2_spine::SEALED)
DT_NS = 10_000        # harness tick (fs_cal2_spine::DT_NS)
RTT_MS = 1000
FROZEN_WARMUP_TIMEOUT_NS = 2 * 9249 * 1_000_000   # 2 × frozen horizon(1000ms) = 18.498 s


def derive_timeout():
    """Return (extended_timeout_ns, derivation dict). Pure arithmetic from grounded constants."""
    rtt_ns = RTT_MS * 1_000_000
    floor_bw = GROUNDED["PRE_CONVERGE_FLOOR_BW"]["value"]
    # time to accrue enough tokens for ONE sealed unit at the PreConverge floor rate
    pc_quantum_ns = -(-SEALED * 1_000_000_000 // floor_bw)   # ceil division
    guard_ns = rtt_ns + pc_quantum_ns + DT_NS
    # round the guard UP to a whole second so the budget is not knife-edge
    guard_ns = -(-guard_ns // 1_000_000_000) * 1_000_000_000
    a = GROUNDED["CS_GIVEUP_NS"]["value"] + guard_ns
    b = GROUNDED["CS_PROBE_FIRES_NS"]["last_step_ns"] + rtt_ns + guard_ns
    c = 60_000_000_000
    ext = max(a, b, c)
    return ext, {
        "rtt_ns": rtt_ns,
        "preconverge_floor_bw_bytes_per_s": floor_bw,
        "sealed_unit_bytes": SEALED,
        "preconverge_emission_quantum_ns": pc_quantum_ns,
        "preconverge_emission_quantum_s": round(pc_quantum_ns / 1e9, 4),
        "harness_tick_dt_ns": DT_NS,
        "guard_ns": guard_ns,
        "guard_s": guard_ns / 1e9,
        "guard_justification": (
            "guard = RTT (one full round trip for the first SACK to return) "
            "+ PreConverge emission quantum (ceil(SEALED / PRE_CONVERGE_FLOOR_BW) = time to accrue tokens for ONE "
            "101 B unit at the 31 B/s floor) + harness timer resolution dt (10 µs), rounded UP to a whole second. "
            "Both terms are required: a sender that has just emitted its first unit needs a further RTT before any "
            "SACK can possibly arrive, and it cannot emit at all faster than the floor quantum."
        ),
        "candidate_a_coldstart_giveup_plus_guard_ns": a,
        "candidate_b_last_probe_step_plus_rtt_plus_guard_ns": b,
        "candidate_c_floor_60s_ns": c,
        "extended_timeout_ns": ext,
        "extended_timeout_s": ext / 1e9,
        "frozen_warmup_timeout_ns": FROZEN_WARMUP_TIMEOUT_NS,
        "frozen_warmup_timeout_s": FROZEN_WARMUP_TIMEOUT_NS / 1e9,
        "ratio_extended_over_frozen": round(ext / FROZEN_WARMUP_TIMEOUT_NS, 4),
    }


def sha_file(p):
    try:
        return hashlib.sha256(open(os.path.join(REPO, p), "rb").read()).hexdigest()
    except OSError:
        return None


def main():
    ext, deriv = derive_timeout()
    srcs = [
        "crates/adaptive-transport/src/sender.rs",
        "crates/adaptive-transport/src/tail_recovery.rs",
        "crates/adaptive-transport/src/fate_estimator.rs",
        "crates/adaptive-transport/src/fusion_policy.rs",
        "crates/kama-integration/tests/common/fs_cal2_spine.rs",
        "crates/kama-integration/examples/fs_cal2_runner.rs",
    ]
    try:
        commit = subprocess.check_output(["git", "-C", REPO, "rev-parse", "HEAD"], text=True).strip()
    except Exception:
        commit = "unknown"

    man = {
        "manifest": "FSFC2-DIAG-CS-V32",
        "namespace": "FSFC2-DIAG-CS-V32",
        "purpose": "liveness/root-cause only",
        "fit_use": "FORBIDDEN",
        "result_bearing": False,
        "calibration_rows_contributed": 0,
        "isolation": (
            "Namespace-disjoint from every calibration dataset. These records live under "
            "reports/fs-cal-2/v3.1/diag-cs/ and carry experiment_phase=DIAG_CS_V32 + fit_use=FORBIDDEN. "
            "They MUST NOT enter any training / support-bitmap / holdout / parameter-fit dataset."
        ),
        "condition": {
            "structure": "GE-L10",
            "p_ppm": 200_000,
            "burst_length_l": 10,
            "rtt_ms": RTT_MS,
            "arm": "STATIC_R0",
            "workload_sealed_bytes": SEALED,
            "warmup_units": 384,
            "estimator_window": 256,
            "note": "Identical to the FAILED §4.2b P2 cell (P2-GE20L10-RTT1000).",
        },
        "arq_contract": {
            "arq_contract_id": "FSCAL2-ARQ-DEFAULTFEAT-TAILV1-v1",
            "tail_probe": "ON (verified §1)",
            "setting_symbol": "Sender::new() + set_tail_recovery_config(tail_on_config())",
            "tail_on_config": {
                "enabled": True, "min_pto_ns": 10_000_000, "max_pto_ns": 10_000_000_000,
                "initial_pto_ns": 1_000_000_000, "max_backoff_pto_ns": 30_000_000_000,
                "max_ack_delay_ns": 25_000_000, "max_tail_probes": 3,
            },
            "compile_features": "DEFAULT (no experimental-cold-start, no experimental-fec-budget)",
            "applies_identically_to_both_arms": True,
        },
        "extended_timeout_derivation": deriv,
        "layers": {
            "D1": {
                "name": "deterministic stall replay",
                "seed_domain": "FSFC2-DIAG42B",
                "cell_id": "P2-GE20L10-RTT1000",
                "repetitions": [5, 9, 14, 16, 17, 25, 38, 39],
                "n": 8,
                "rationale": "The EXACT 8 runs that produced zero-transmission stalls in the failed §4.2b P2 cell "
                             "(same seed_domain + cell_id + repetition ⇒ byte-identical loss streams).",
                "warmup_timeout_ns": ext,
                "purpose": "debugging only — produces NO rate and NO scientific effect estimate.",
            },
            "D2": {
                "name": "fresh diagnostic",
                "seed_domain": "FSFC2-DIAG-CS-V32",
                "cell_id": "P2-GE20L10-RTT1000",
                "repetitions": list(range(40)),
                "n": 40,
                "rationale": "Disjoint seed domain — NOT a re-read of the §4.2b draws.",
                "warmup_timeout_ns": ext,
            },
            "INVARIANCE_CONTROL": {
                "name": "science-invariance control",
                "seed_domain": "FSFC2-DIAG42B",
                "repetitions": [5, 9, 14, 16, 17, 25, 38, 39],
                "warmup_timeout_ns": FROZEN_WARMUP_TIMEOUT_NS,
                "purpose": "Re-run the stall seeds at the FROZEN timeout with the instrumented binary and prove "
                           "every estimator field matches the frozen §4.2b records exactly (KALICI-KURAL-3: "
                           "executable conformance check, not inspection).",
            },
        },
        "grounded_constants": GROUNDED,
        "provenance": {
            "source_commit_40hex": commit,
            "source_sha256": {p: sha_file(p) for p in srcs},
        },
        "prohibitions": [
            "No calibration fit, no policy freeze, no support-bitmap derivation from these rows.",
            "max_telemetry_age_ns MUST NOT be changed by this diagnostic.",
            "Eligibility thresholds (N=256, n10+n11>=8, n00+n01>=32) MUST NOT be changed.",
            "The 0.90 §4.2b bar MUST NOT be lowered.",
            "RTT=1000 MUST NOT be dropped from scope merely because it fails.",
        ],
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    body = json.dumps(man, indent=2, sort_keys=True)
    with open(OUT, "w") as f:
        f.write(body + "\n")
    with open(OUT + ".sha256", "w") as f:
        f.write(hashlib.sha256((body + "\n").encode()).hexdigest() + "  FSFC2-DIAG-CS-V32-MANIFEST.json\n")
    print(body)
    print("\nextended_timeout_ns =", ext, f"({ext/1e9:.3f} s)")
    print("written:", OUT)
    return 0


if __name__ == "__main__":
    sys.exit(main())
