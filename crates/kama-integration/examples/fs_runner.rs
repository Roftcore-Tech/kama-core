//! DR-FEC-FS-1 FS-HARNESS-A — fs_runner CLI. A0/A1 (adaptive-transport run_arm) + F0 (shared codec-loop).
//! Machine-readable KEY=VALUE çıktı: timing-ledger · redundancy-accounting · completion-cause · trace-hash ·
//! binary-provenance · arg-parity · path-timing · INVALID. Deterministik bottleneck-channel.
//!
//! Kullanım: --arm a0|a1|f0 [--r N] [--deficit e,e] [--seed N] [--link-bps N] [--owd-ms N]
//!
//! NOT: result-bearing-DEĞİL (harness-smoke/gösterim); FS-1 AYRI (v1.2-freeze-sonrası).

#[cfg(feature = "experimental-fs-harness")]
#[path = "../tests/common/fs_f0.rs"]
mod fs_f0;

#[cfg(not(feature = "experimental-fs-harness"))]
fn main() {
    eprintln!("experimental-fs-harness feature'ı gerekli");
    std::process::exit(3);
}

#[cfg(feature = "experimental-fs-harness")]
fn main() {
    use adaptive_transport::fs_harness::{run_arm, run_config_hash, Arm, RunCfg};
    use std::collections::BTreeSet;

    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: &str| -> String {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| d.to_string())
    };
    let arm_s = get("--arm", "a0");
    let r: u16 = get("--r", "2").parse().unwrap_or(2);
    let seed: u64 = get("--seed", "2026").parse().unwrap_or(2026);
    let link_bps: u64 = get("--link-bps", "1000000").parse().unwrap_or(1_000_000);
    let owd_ns: u64 = get("--owd-ms", "20").parse::<u64>().unwrap_or(20) * 1_000_000;
    let deficit: BTreeSet<u64> = get("--deficit", "")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let dup_seed = adaptive_transport::fs_harness::domain_seed(seed, "A1_DUP_TARGET");

    // arg-parity / binary-provenance: config-kimliği (arm hariç aynı → arg-parity kanıtı).
    let cfg = RunCfg {
        num_units: 8,
        unit_size: 64,
        link_bps,
        owd_ns,
        horizon_ns: 20_000 * 1_000_000,
        dt_ns: 1_000_000,
        drop_initial_units: deficit.clone(),
        dup_seed,
    };

    println!("# fs_runner (DR-FEC-FS-1 FS-HARNESS-A; result-bearing=NO)");
    println!("arm={arm_s}");
    println!(
        "cfg.k=8 cfg.unit_size=64 cfg.link_bps={link_bps} cfg.owd_ns={owd_ns} cfg.seed={seed}"
    );
    println!("cfg.deficit={:?}", deficit);
    match arm_s.as_str() {
        "a0" | "a1" => {
            let arm = if arm_s == "a1" {
                Arm::A1 { r }
            } else {
                Arm::A0
            };
            let res = run_arm(arm, &cfg);
            println!(
                "binary_provenance.config_hash={:#018x}",
                run_config_hash(arm, &cfg)
            );
            println!(
                "arg_parity.config_hash={:#018x}",
                res.instrument.config_hash
            );
            println!("trace_hash={:#018x}", res.instrument.trace_hash);
            println!("intervention={:?}", res.instrument.arm_intervention);
            println!(
                "faithful={} overlap={} admitted={}",
                res.instrument.faithful,
                res.instrument.overlap_ok,
                res.instrument.admitted()
            );
            println!(
                "configured_drops={} realized_drops={} receiver_events={}",
                res.instrument.configured_source_drops,
                res.instrument.realized_source_drops,
                res.instrument.receiver_events
            );
            println!(
                "delivered={}/{} completion_ns={:?}",
                res.delivered_units, res.num_units, res.completion_ns
            );
            println!(
                "data_emitted={} retransmits={} duplicates={}",
                res.data_emitted, res.retransmits_emitted, res.duplicates_emitted
            );
            println!(
                "a1_dup_hits={} a1_dup_misses={}",
                res.a1_dup_hits, res.a1_dup_misses
            );
            println!("cost.configured_r={} cost.realized_sealed_wire_bytes={} cost.payload={} cost.framing={} cost.packets={}", res.cost.configured_r, res.cost.realized_sealed_wire_bytes, res.cost.redundancy_payload_bytes, res.cost.redundancy_framing_bytes, res.cost.redundancy_packet_count);
            print_timing(&res.timing);
            println!("parity_delay_class={:?}", res.timing.parity_delay_class());
            print_path_timing(&res.path_timing);
            println!("completion_cause={}", completion_cause_a(&res));
            println!("INVALID={}", !res.instrument.admitted());
        }
        "f0" => {
            let res = fs_f0::run_f0(
                r,
                &deficit.iter().map(|&u| u as u16).collect::<Vec<_>>(),
                link_bps,
                owd_ns,
            );
            println!(
                "binary_provenance.config_hash={:#018x}",
                run_config_hash(Arm::A0, &cfg) ^ 0xF0
            );
            println!("trace_hash={:#018x}", res.instrument.trace_hash);
            println!("intervention={:?}", res.instrument.arm_intervention);
            println!(
                "faithful={} overlap={} admitted={}",
                res.instrument.faithful,
                res.instrument.overlap_ok,
                res.instrument.faithful && res.instrument.overlap_ok
            );
            println!(
                "delivered={}/8 completion_ns={:?}",
                res.delivered, res.completion_ns
            );
            println!(
                "parity_emitted={} retransmits={} fec_decoded_sources={}",
                res.parity_emitted, res.retransmits, res.fec_decoded_sources
            );
            println!(
                "reconstruct_byte_equal={} estimator_pure={} post_timeout_notice_none={}",
                res.reconstruct_byte_equal, res.estimator_pure, res.post_timeout_notice_none
            );
            println!("cost.configured_r={} cost.realized_sealed_wire_bytes={} cost.payload={} cost.framing={}", r, res.realized_sealed_wire_bytes, res.redundancy_payload_bytes, res.redundancy_framing_bytes);
            print_timing(&res.timing);
            println!("parity_delay_class={:?}", res.timing.parity_delay_class());
            print_path_timing(&res.path_timing);
            // Timing-honest cause: blok'u FEC mi ARQ mı TAMAMLADI (retransmit-SAYISI değil — parity post-DATA
            // geç-varınca ARQ SPURIOUS retransmit sızdırabilir AMA FEC yine erken-tamamlar → cause=fec).
            let fec_beats_arq = match (res.timing.t_repair_usable, res.timing.t_arq_arrival) {
                (Some(u), Some(a)) => u <= a,
                (Some(_), None) => true,
                _ => false,
            };
            let fec_completed = res.fec_decoded_sources > 0 && fec_beats_arq;
            let cause = if fec_completed {
                "fec"
            } else if res.retransmits > 0 {
                "arq"
            } else {
                "direct"
            };
            // FEC tamamladıysa emitilen retransmit'ler SPURIOUS (FEC-decode ARQ-emisyonundan geç-kaldı).
            let spurious = if fec_completed { res.retransmits } else { 0 };
            println!("completion_cause={cause} spurious_retransmits={spurious}");
            println!(
                "INVALID={}",
                !(res.instrument.faithful && res.instrument.overlap_ok)
            );
        }
        _ => {
            eprintln!("--arm a0|a1|f0 zorunlu");
            std::process::exit(3);
        }
    }
}

#[cfg(feature = "experimental-fs-harness")]
fn print_timing(t: &adaptive_transport::fs_harness::BlockTiming) {
    println!(
        "timing.t_source_loss={:?} t_parity_eligible={:?} t_parity_enqueued={:?} t_parity_wire={:?} t_parity_arrival={:?}",
        t.t_source_loss, t.t_parity_eligible, t.t_parity_enqueued, t.t_parity_wire, t.t_parity_arrival
    );
    println!(
        "timing.t_arq_eligible={:?} t_arq_wire={:?} t_arq_arrival={:?} t_block_complete={:?}",
        t.t_arq_eligible, t.t_arq_wire, t.t_arq_arrival, t.t_block_complete
    );
    println!(
        "timing.redundancy_kind={:?} t_redundancy_eligible={:?} t_redundancy_enqueued={:?} t_redundancy_wire={:?} t_redundancy_arrival={:?} t_repair_usable={:?}",
        t.redundancy_kind, t.t_redundancy_eligible, t.t_redundancy_enqueued, t.t_redundancy_wire, t.t_redundancy_arrival, t.t_repair_usable
    );
}

#[cfg(feature = "experimental-fs-harness")]
fn print_path_timing(p: &adaptive_transport::fs_harness::PathTimingLedger) {
    println!(
        "path_timing(LEDGER-ONLY).srtt_ns={:?} min_rtt_ns={:?} rttvar_ns={} queue_delay_ns={} sample_present={} rtprop_fresh={}",
        p.path_srtt_ns, p.path_min_rtt_ns, p.path_rttvar_ns, p.path_queue_delay_ns, p.path_sample_present, p.path_rtprop_fresh
    );
}

#[cfg(feature = "experimental-fs-harness")]
fn completion_cause_a(res: &adaptive_transport::fs_harness::RunResult) -> &'static str {
    if res.duplicates_emitted > 0 && res.retransmits_emitted == 0 && res.a1_dup_hits > 0 {
        "duplicate"
    } else if res.retransmits_emitted > 0 {
        "arq"
    } else {
        "direct"
    }
}
