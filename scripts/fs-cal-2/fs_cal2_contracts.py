#!/usr/bin/env python3
"""FS-CAL-2 v3 §4.0/§4.0b/§4.0c/§4.1 — product/joint-action/cost/FEC_R2 contract generator.

Every row below was verified by read-only code audit at HEAD 4aa11a93 (branch d1-sprint5c);
file sha256 values are computed at generation time from the working tree. No field is guessed:
each carries an exact Rust symbol + file:line evidence. Output: FS-CAL-2-PRODUCT-CONTRACT-v3.json
(+ companion ARQ / COST / FEC_R2 grounding blocks in the same file, per §4.0 output artifact rule)
with a .sha256 sidecar. Deterministic serialization (sorted keys, LF).
"""
import hashlib, json, os, subprocess, sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT_DIR = os.path.join(REPO, "reports", "fs-cal-2", "v3", "grounding")
DESIGN_SHA = "115fa266a40f744013d209d36711a698096a28509ede11dd417f23aa5cdb3393"


def sha(path):
    h = hashlib.sha256()
    with open(os.path.join(REPO, path), "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    files = {
        "sender": "crates/adaptive-transport/src/sender.rs",
        "fusion": "crates/adaptive-transport/src/fusion.rs",
        "fusion_policy": "crates/adaptive-transport/src/fusion_policy.rs",
        "arq": "crates/adaptive-transport/src/arq.rs",
        "loss": "crates/adaptive-transport/src/loss.rs",
        "rtt": "crates/adaptive-transport/src/rtt.rs",
        "tail_recovery": "crates/adaptive-transport/src/tail_recovery.rs",
        "fate_estimator": "crates/adaptive-transport/src/fate_estimator.rs",
        "rate": "crates/adaptive-transport/src/rate.rs",
        "budget": "crates/adaptive-transport/src/budget.rs",
        "fec_codec": "crates/kama-fec-codec/src/codec.rs",
        "fec_profile": "crates/kama-fec-codec/src/profile.rs",
        "dr_arqtail_product": "crates/adaptive-transport/examples/dr_arqtail_product.rs",
        "fs_cal2_spine": "crates/kama-integration/tests/common/fs_cal2_spine.rs",
        "fs_cal2_policy": "crates/kama-integration/tests/common/fs_cal2_policy.rs",
        "fs_cal2_gen": "crates/kama-integration/tests/common/fs_cal2_gen.rs",
        "fs_cal2_schema": "crates/kama-integration/tests/common/fs_cal2_schema.rs",
        "fs_spine_fs1": "crates/kama-integration/tests/common/fs_spine.rs",
    }
    fsha = {k: sha(v) for k, v in files.items()}
    head = subprocess.run(
        ["git", "-C", REPO, "rev-parse", "HEAD"], capture_output=True, text=True
    ).stdout.strip()

    def inp(symbol, key, producer, role, value, fallback):
        return {
            "input_symbol": symbol,
            "source_file": files[key],
            "source_file_sha256": fsha[key],
            "runtime_producer": producer,
            "role": role,
            "design_factor_or_value": value,
            "fallback_if_unavailable": fallback,
        }

    def outp(symbol, key, actuator, state, arm, phase, proof):
        return {
            "output_symbol": symbol,
            "source_file": files[key],
            "source_file_sha256": fsha[key],
            "actuator_symbol": actuator,
            "actuator_state": state,
            "treatment_arm": arm,
            "phase": phase,
            "wire_proof_test": proof,
        }

    contract = {
        "contract_id": "FS-CAL-2-PRODUCT-CONTRACT-v3",
        "design_sha256": DESIGN_SHA,
        "source_commit_40hex": head,
        "compile_configuration": "DEFAULT cargo features (no experimental-cold-start / experimental-fec-budget) — identical to the DR-ARQ-PRODUCT-TAIL-1 product-validation compile path",
        # ── §4.0 INPUT TABLE ──
        "inputs": [
            inp("FusionInputs.loss_ppm (fusion.rs:381)", "fusion",
                "ThresholdLossDetector::loss_windowed_ppm (loss.rs:240-251) via Sender::fuzzy_evaluate (sender.rs:2004)",
                "FIXED", "engine-internal; NOT an FS-CAL-2 policy input (the §7.3 policy consumes the FATE-estimator measured_loss_ppm instead)",
                "n/a (engine-internal)"),
            inp("FusionInputs.bandwidth_bps (fusion.rs:383)", "fusion",
                "BtlBwFilter::btlbw (bw.rs:221) via sender.rs:2005",
                "SAFETY_ONLY", "hard budget guard dimension (design §7.3: bandwidth is a FusionSafetyConfig guard, not a membership)",
                "R0 / SAFETY_CAP"),
            inp("FusionInputs.jitter_ppm (fusion.rs:385)", "fusion",
                "rttvar*1e6/srtt (sender.rs:1990-1993)",
                "UNSUPPORTED", "UNCALIBRATED-v1 (design §7.3): not an FEC-activation membership in this version",
                "R0 (not consulted)"),
            inp("FusionInputs.qd_ratio_permille (fusion.rs:388)", "fusion",
                "queue_delay*1000/rtprop (sender.rs:1997-2002, 2238-2246)",
                "UNSUPPORTED", "UNCALIBRATED-v1", "R0 (not consulted)"),
            inp("FusionInputs.ack_confidence_permille (fusion.rs:390)", "fusion",
                "silence-based ack_confidence_permille (sender.rs:3600-3631)",
                "UNSUPPORTED", "UNCALIBRATED-v1", "R0 (not consulted)"),
            inp("measured_loss_ppm (fate_estimator::WindowFeatures)", "fate_estimator",
                "FateEstimator window over receiver-ACK/recovery-lineage fates (§3.2/§3.3; fs_cal2_spine evidence wiring)",
                "VARIED", "runtime policy feature (§7.3 allowlist)", "R0 / ESTIMATOR_INELIGIBLE"),
            inp("burst_delta_ppm (fate_estimator::WindowFeatures)", "fate_estimator",
                "Laplace p11-p01 over the 256-fate window (§3.3)",
                "VARIED", "runtime policy feature (§7.3 allowlist)", "R0 / ESTIMATOR_INELIGIBLE"),
            inp("estimator_eligible (fate_estimator::WindowFeatures::eligible)", "fate_estimator",
                "N==256 && n10+n11>=8 && n00+n01>=32 && warmup_complete && !stale && converged (§3.4 pinned)",
                "VARIED", "policy gate", "R0 / ESTIMATOR_INELIGIBLE"),
            inp("support_eligible (fs_cal2_policy::support_bins + bitmap)", "fs_cal2_policy",
                "frozen §3.4b bitmap membership of (loss_bin, burst_bin)",
                "VARIED", "policy gate", "R0 / OUT_OF_SUPPORT"),
            inp("workload_supported (WORKLOAD_PROFILE_ID interlock)", "fs_cal2_spine",
                "workload_profile_id == frozen FSCAL2-BULK-W384-K8-U101-v1 (§2.2 mechanical interlock)",
                "VARIED", "policy gate", "R0 / UNSUPPORTED_WORKLOAD"),
        ],
        # ── §4.0 OUTPUT TABLE ──
        "outputs": [
            outp("FusionDecision.fec_parity_ratio_permille (fusion.rs:438)", "fusion",
                 "external fec_controller_latch chain only (sender.rs:2508-2619); default fec_mode Manual{enabled:false} (sender.rs:1026)",
                 "VARIABLE", "NONE — FS-CAL-2 R2 actuates via close_fec_block_manual, NOT via the fuzzy ratio (conditional latch path unused; default Manual{false})",
                 "B", "r2_parity_wire_proof_exact (fs_cal2_pins)"),
            outp("FusionDecision.arq_aggressiveness_permille (fusion.rs:440)", "fusion",
                 "arq.plan ==0 gate only (arq.rs:105-109); fixture min 200 ⇒ structurally unreachable (fusion.rs:513-514)",
                 "DEGENERATE", "NONE — inert on the product path (proof cited)",
                 "C", "n/a (degenerate; audit-proven)"),
            outp("FusionDecision.send_rate_gain_permille (fusion.rs:442)", "fusion",
                 "compute_send_rate → pacer.set_rate (sender.rs:2303-2333, 1244-1245; rate.rs:164-184)",
                 "VARIABLE", "NONE — send-rate is loss-decoupled (INV-RATE-1) and out of FS-CAL-2 scope (phase D); identical across arms by construction (same engine, same inputs)",
                 "D", "r0_determinism_wire_digest_exact (pacing identity within arm pairing)"),
            outp("FusionDecision.compress (fusion.rs:444)", "fusion",
                 "NO consumer anywhere (whole-crate grep)", "ABSENT", "NONE", "D", "n/a"),
            outp("FusionDecision.mode / EmergencyForwardRedundancy (fusion.rs:446)", "fusion",
                 "update_state → RecoverFwd arm flip (sender.rs:2052-2080, 2357-2361) + FEC latch suppression (sender.rs:2578-2587)",
                 "VARIABLE", "NONE — emergency machinery kept AS-IS in every arm (fixed baseline); not a treatment axis",
                 "C", "covered by arm-identical engine (pairing prefix equality)"),
            outp("FusionDecision.no_rule (fusion.rs:450)", "fusion",
                 "diag counter + FEC-latch fail-closed zero (sender.rs:2013-2015, 2571-2573)",
                 "DEGENERATE", "NONE (diagnostic)", "B", "n/a"),
            outp("close_fec_block_manual staged parity (sender.rs:2464-2505)", "sender",
                 "plan_fec_parity → TransmitKind::FecParity (sender.rs:2827-2912); paced via pacer.try_send; seq via track_sent",
                 "VARIABLE", "STATIC_R2 / CALIBRATED_POLICY(R2)", "A",
                 "r2_parity_wire_proof_exact + fec_decode_success_recovers_tail + fec_decode_failure_arq_liveness (fs_cal2_pins)"),
            outp("TailRecovery bounded probe (tail_recovery.rs; sender.rs:3372-3403)", "tail_recovery",
                 "emit_tail_probe → TransmitKind::Retransmit (token-gated, cap=3)",
                 "VARIABLE", "ALL ARMS (part of the frozen ARQ base, not a treatment)", "A",
                 "terminal_tail_r0_probe_recovers (fs_cal2_pins)"),
        ],
        # ── §4.0b JOINT-ACTION CONTRACT ──
        "joint_action_contract": {
            "arq_contract_id": "FSCAL2-ARQ-DEFAULTFEAT-TAILV1-v1",
            "controller_accessible_executable_arq_settings_count": 0,
            "evidence": [
                "fusion arq output inert: arq.rs:105-109 ==0 gate; 0 unproducible (fusion.rs:513-514; min consequent arq_light=200 fusion.rs:367; no_rule fallback also 200 fusion.rs:519-520)",
                "RetxConfig fixture-frozen at construction (arq.rs:68-80: cap=3, reorder_k=3, rho=2/1, HandToForward); NO runtime setter on Sender (complete pub fn set_ enumeration: sender.rs 1146/2386/2415/2919)",
                "set_tail_recovery_config is an operator/harness API outside the decision loop (sender.rs:1144-1150); no policy path reaches it",
            ],
            "resolution": "count == 1 executable ARQ configuration ⇒ joint action = {frozen ARQ} × {R0, R2} (design §4.0b first branch; row budget 39,300 stands)",
            "arq_setting_symbol": "Sender::new() + set_tail_recovery_config(tail_on_config())",
            "arq_setting_value": {
                "tail": {"enabled": True, "min_pto_ns": 10000000, "max_pto_ns": 10000000000,
                         "initial_pto_ns": 1000000000, "max_backoff_pto_ns": 30000000000,
                         "max_ack_delay_ns": 25000000, "max_tail_probes": 3},
                "retx_fixture": {"arq_retx_cap": 3, "reorder_k": 3, "reorder_rho": "2/1",
                                  "cap_exceed": "HandToForward"},
                "fuzzy_config": "FuzzyConfig::fixture() (uncalibrated engine default — unchanged)",
                "compile_features": "default (no experimental-*)",
            },
            "arq_source_sha256": {"sender.rs": fsha["sender"], "arq.rs": fsha["arq"],
                                    "tail_recovery.rs": fsha["tail_recovery"],
                                    "dr_arqtail_product.rs": fsha["dr_arqtail_product"]},
            "invalid_if_arq_setting_changes": True,
        },
        # ── §4.0c UTILITY/COST CONTRACT ──
        "cost_contract": {
            "max_fec_bytes_per_block": {
                "value": 4096,
                "source_symbol": "FusionSafetyConfig::default().hard_fec_byte_budget (fusion_policy.rs:126-127, 140)",
                "source_sha256": fsha["fusion_policy"],
                "grounded": True,
                "r2_realized": 202,
            },
            "max_combined_recovery_overhead_bytes": {
                "value": 8192,
                "source_symbol": "FusionSafetyConfig::default().hard_combined_byte_cap (fusion_policy.rs:128-129, 141)",
                "source_sha256": fsha["fusion_policy"],
                "grounded": True,
            },
            "max_fec_wire_ratio_ppm": {
                "value": 800000,
                "source_symbol": "FusionSafetyConfig::default().hard_max_redundancy_permille=800 (fusion_policy.rs:124-125, 139) — permille→ppm ×1000",
                "source_sha256": fsha["fusion_policy"],
                "grounded": True,
                "r2_realized_ppm": 250000,
                "note": "R2 = 202/808 B = 250,000 ppm of decision-block source bytes ≤ 800,000",
            },
            "minimum_completion_gain_pp": {
                "value": 15,
                "source_symbol": "design §4.0c/§6.3/§7.2 frozen minimum effect",
                "grounded": False,
                "label": "EXPERIMENTAL_BOUND — design-frozen scientific bound; NOT a product SLO; carries no product claim",
            },
            "primary_horizon_definition": {
                "value": "frozen per-RTT run horizon {50→699, 250→2499, 1000→9249} ms applied FROM the decision epoch",
                "source_symbol": "DR-ARQ-PRODUCT-TAIL-1 / R2A frozen horizons (fs_cal2_spine::horizon_ms; reports/dr-arq-product-tail-1/FIDELITY-AND-FREEZE.md identity)",
                "grounded": True,
            },
            "verdict": "cost bounds GROUNDED (three product symbols + one explicit EXPERIMENTAL_BOUND + grounded horizon) — §4.0c training-blocker NOT triggered",
        },
        # ── §4.1 FEC_R2 GROUNDING ──
        "fec_r2_grounding": {
            "authoritative_source_symbol": "FS-1/R2A SpineArm::F0{r:2} coding identity (fs_spine.rs:186-291 audit) EMITTED through the product Sender authority (fs_cal2_spine.rs)",
            "codec_profile_identity": {"profile_id": "0x03", "api": "kama_fec_codec::by_id(0x03)",
                                        "K": 8, "E": 64, "code_suite": "CODE_SUITE_RS_SIMD_V1"},
            "k_and_r_runtime_assertion": "fs_cal2_pins::r2_parity_wire_proof_exact (parity_bytes == 2×101; staged==2 else inv_fail)",
            "serializer_fields": "FS-CAL-2-RAW-SCHEMA-v3 (emitted from fs_cal2_schema.rs; parity accounting fields parity_bytes/fec_decoded_sources/fec_decode_success)",
            "parity_emission_path": "open_fec_block_authority (sender.rs:2400-2408) → close_fec_block_manual (sender.rs:2464-2505) → pending_fec → plan_fec_parity (sender.rs:2827-2869) → emit_parity_at/track_sent → TransmitKind::FecParity; pacer.try_send token-gated; integration proof: fs_cal2_pins::r2_parity_wire_proof_exact",
            "decode_reassembly_path": "FS-1/R2A counting-MDS rule (distinct<K && repairs>=1 && distinct+repairs>=K) + RsSimdCodec.reconstruct (codec.rs:310+) + byte-equality vs canonical + Event::RecoveredControl feedback (sender.rs:2721-2759 pure-logical); golden: fs_cal2_pins::fec_decode_success_recovers_tail",
            "max_fec_bytes": "FusionSafetyConfig.hard_fec_byte_budget = 4096 (see cost_contract)",
            "combined_overhead_cap": "FusionSafetyConfig.hard_combined_byte_cap = 8192",
            "parity_sealed_len": 101,
            "r1_r4": "ABSENT from this design (per §4.1)",
            "executable_in_product_path": True,
            "codec_source_sha256": {"codec.rs": fsha["fec_codec"], "profile.rs": fsha["fec_profile"]},
            "note": "fs_spine (FS-1/R2A) emitted parity harness-side; FS-CAL-2 keeps the IDENTICAL coding identity (K/E/profile/RsSimdCodec/r/sealed-len/decode rule) but routes emission through the product Sender FEC authority — no fake harness arm (§4.1 rule).",
        },
        "harness_source_sha256": {
            "fs_cal2_spine.rs": fsha["fs_cal2_spine"],
            "fs_cal2_policy.rs": fsha["fs_cal2_policy"],
            "fs_cal2_gen.rs": fsha["fs_cal2_gen"],
            "fs_cal2_schema.rs": fsha["fs_cal2_schema"],
            "fs_spine.rs (FS-1 reference)": fsha["fs_spine_fs1"],
            "fate_estimator.rs": fsha["fate_estimator"],
        },
    }

    os.makedirs(OUT_DIR, exist_ok=True)
    out_path = os.path.join(OUT_DIR, "FS-CAL-2-PRODUCT-CONTRACT-v3.json")
    payload = json.dumps(contract, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    with open(out_path, "w") as f:
        f.write(payload)
    digest = hashlib.sha256(payload.encode()).hexdigest()
    with open(out_path + ".sha256", "w") as f:
        f.write(f"{digest}  FS-CAL-2-PRODUCT-CONTRACT-v3.json\n")
    print(f"contract_sha256={digest}")
    print(f"out={out_path}")
    print(f"inputs={len(contract['inputs'])} outputs={len(contract['outputs'])}")
    print(f"joint_action: executable ARQ settings = {contract['joint_action_contract']['controller_accessible_executable_arq_settings_count']} → {{frozen ARQ}} × {{R0,R2}}")


if __name__ == "__main__":
    main()
