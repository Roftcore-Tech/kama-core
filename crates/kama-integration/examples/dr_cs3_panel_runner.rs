//! DR-CS-3 panel runner — **DEFAULT-BUILD AUTHENTICATED TRANSPORT COMPONENT PANEL
//! WITH TEST-INJECTED SESSION KEYS**.
//!
//! Modes:
//!   locked    §4 locked regression panel (the 8 frozen DR-CS-2 records, P0 + P1, paired OFF/ON)
//!   fresh     §5 fresh paired panel (GE-L10 p=20% RTT=1000, N=40, P0 + P1)
//!   control   §6 control panels (no-loss RTT{50,250,1000} · IID-p5 · probe/echo loss scenarios)
//!
//! Every record carries `fit_use=FORBIDDEN` and `result_bearing=false` for the calibration
//! programme: this panel is a MECHANISM validation, never a calibration input.
#![allow(clippy::too_many_lines)]

#[path = "../tests/common/dr_cs3_panel.rs"]
mod dr_cs3_panel;

use dr_cs3_panel::fs_cal2_gen::{sha256_hex, LossModel};
use dr_cs3_panel::{data_displacement, run_panel, PanelCfg, PanelOut};
use std::fs;
use std::io::Write as _;
use std::path::Path;

fn arg(a: &[String], k: &str, d: &str) -> String {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
        .unwrap_or_else(|| d.to_string())
}
fn argu(a: &[String], k: &str, d: u64) -> u64 {
    arg(a, k, &d.to_string()).parse().unwrap_or(d)
}

fn write_with_sidecar(path: &Path, content: &str) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).expect("mkdir");
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).expect("create tmp");
        f.write_all(content.as_bytes()).expect("write");
        f.sync_all().expect("fsync");
    }
    fs::rename(&tmp, path).expect("atomic rename");
    let name = path.file_name().unwrap().to_string_lossy();
    fs::write(
        path.with_file_name(format!("{name}.sha256")),
        format!("{}  {}\n", sha256_hex(content.as_bytes()), name),
    )
    .expect("sidecar");
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn opt(v: Option<u64>) -> String {
    v.map_or("null".into(), |x| x.to_string())
}

/// Record identity (keeps `pair_json` within the argument budget).
struct Id<'a> {
    run_id: &'a str,
    panel: &'a str,
    cell: &'a str,
    domain: &'a str,
    rep: u32,
    rtt_ms: u64,
}

/// A §6 targeted probe/echo-loss scenario.
struct BsScenario {
    name: &'static str,
    probe_drops: Vec<bool>,
    echo_drops: Vec<bool>,
    wire_dups: u32,
    event_dups: u32,
}

/// One paired (baseline, treatment) record.
fn pair_json(id: &Id<'_>, b: &PanelOut, t: &PanelOut) -> String {
    let (run_id, panel, cell, domain, rep, rtt_ms) =
        (id.run_id, id.panel, id.cell, id.domain, id.rep, id.rtt_ms);
    let disp = data_displacement(b, t);
    let mut s = String::new();
    s.push_str("{\n");
    let mut f = |k: &str, v: String| s.push_str(&format!("  {}: {},\n", q(k), v));
    f("run_id", q(run_id));
    f("namespace", q("DRCS3-PANEL-V1"));
    f("scope", q("DEFAULT-BUILD AUTHENTICATED TRANSPORT COMPONENT PANEL WITH TEST-INJECTED SESSION KEYS"));
    f("validates_noise_handshake", "false".into());
    f("validates_product_binary", "false".into());
    f("validates_deployment_network_path", "false".into());
    f("fit_use", q("FORBIDDEN"));
    f("result_bearing", "false".into());
    f("panel", q(panel));
    f("cell_id", q(cell));
    f("seed_domain", q(domain));
    f("repetition_index", rep.to_string());
    f("configured_rtt_ms", rtt_ms.to_string());
    f("treatment_axis", q("BootstrapRttConfig.enabled"));

    for (tag, o) in [("baseline", b), ("treatment", t)] {
        f(&format!("{tag}_epoch_trigger"), q(o.epoch_trigger));
        f(&format!("{tag}_observation_epoch_ns"), o.observation_epoch_ns.to_string());
        f(&format!("{tag}_preconverge_exit_ns"), opt(o.preconverge_exit_ns));
        f(&format!("{tag}_cold_start_stall"), o.cold_start_stall.to_string());
        f(&format!("{tag}_warmup_complete"), o.warmup_complete.to_string());
        f(&format!("{tag}_delivered_src_at_epoch"), o.delivered_src_at_epoch.to_string());
        f(&format!("{tag}_tx_data"), o.tx_data.to_string());
        f(&format!("{tag}_tx_retx"), o.tx_retx.to_string());
        f(&format!("{tag}_tx_dup"), o.tx_dup.to_string());
        f(&format!("{tag}_ack_batches"), o.ack_batches.to_string());
        f(&format!("{tag}_first_ack_ns"), opt(o.first_ack_ns));
        f(&format!("{tag}_first_delivery_ns"), opt(o.first_delivery_ns));
        f(&format!("{tag}_data_wire_bytes"), o.data_wire_bytes.to_string());
        f(&format!("{tag}_retx_wire_bytes"), o.retx_wire_bytes.to_string());
        f(&format!("{tag}_bootstrap_edge_ns"), opt(o.bootstrap_edge_ns));
        f(&format!("{tag}_probes_emitted"), o.probes_emitted.to_string());
        f(&format!("{tag}_probe_wire_bytes"), o.probe_wire_bytes.to_string());
        f(&format!("{tag}_probes_authenticated"), o.probes_authenticated.to_string());
        f(&format!("{tag}_echoes_generated"), o.echoes_generated.to_string());
        f(&format!("{tag}_echo_wire_bytes"), o.echo_wire_bytes.to_string());
        f(&format!("{tag}_echoes_arrived"), o.echoes_arrived.to_string());
        f(&format!("{tag}_echo_match_valid"), o.echo_match_valid.to_string());
        f(&format!("{tag}_echo_match_rejected"), o.echo_match_rejected.to_string());
        f(&format!("{tag}_echo_replay_rejected"), o.echo_replay_rejected.to_string());
        f(&format!("{tag}_rtt_sample_ns"), o.rtt_sample_ns.to_string());
        f(&format!("{tag}_srtt_set_ns"), opt(o.srtt_set_ns));
        f(&format!("{tag}_startup_floor_at_exit_bps"), o.startup_floor_at_exit_bps.to_string());
        f(&format!("{tag}_sender_budget_violation"), o.sender_budget_violation.to_string());
        f(&format!("{tag}_receiver_budget_violation"), o.receiver_budget_violation.to_string());
        f(&format!("{tag}_auth_failures"), o.auth_failures.to_string());
        f(&format!("{tag}_duplicate_rtt_samples"), o.duplicate_rtt_samples.to_string());
        f(&format!("{tag}_probe_after_success"), o.probe_after_success.to_string());
        f(&format!("{tag}_tail_probe_due"), o.tail_probe.0.to_string());
        f(&format!("{tag}_tail_probe_sent"), o.tail_probe.1.to_string());
        f(&format!("{tag}_tail_probe_cancelled"), o.tail_probe.2.to_string());
        f(&format!("{tag}_wire_digest"), q(&o.wire_digest));
        f(&format!("{tag}_source_prefix_digest"), q(&o.source_prefix_digest));
    }
    f("data_displacement", disp.to_string());
    f("arq_timer_diff_pre_unlock", {
        // Before the first accepted echo the two arms differ ONLY by the appended probe, so the
        // ARQ/tail timer state must be identical there. (After the unlock, divergence is the
        // intended effect of the unlock, not timer interference.)
        let same = b.tail_probe == t.tail_probe || t.first_echo_tick != u64::MAX;
        u8::from(!same).to_string()
    });
    f("baseline_tail_probe", format!("[{},{},{}]", b.tail_probe.0, b.tail_probe.1, b.tail_probe.2));
    f("treatment_tail_probe", format!("[{},{},{}]", t.tail_probe.0, t.tail_probe.1, t.tail_probe.2));
    s.push_str(&format!("  {}: [\n", q("treatment_lineage")));
    for (i, l) in t.lineage.iter().enumerate() {
        s.push_str(&format!(
            "    {}{}\n",
            q(l),
            if i + 1 == t.lineage.len() { "" } else { "," }
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

const GE: LossModel = LossModel::Ge {
    p_ppm: 200_000,
    l: 10,
};

fn base_cfg(cell: &str, domain: &str, rep: u32, rtt_ms: u64, timeout: u64) -> PanelCfg {
    PanelCfg {
        cell_id: cell.into(),
        repetition: rep,
        seed_domain: domain.into(),
        rtt_ms,
        forward_model: GE,
        // PRIMARY CONTINUITY panel default: the frozen DR-CS cell is forward-loss + reverse-lossless.
        // `--reverse ge` selects the BIDIRECTIONAL ROBUSTNESS panel, where ACK and Echo share the
        // same reverse impairment. The two panels are reported separately and never merged (C2 §3).
        reverse_model: LossModel::None,
        tail_on: false,
        bootstrap_on: false,
        warmup_timeout_ns: timeout,
        probe_drop_script: None,
        echo_drop_script: None,
        echo_wire_duplicates: 0,
        echo_event_duplicates: 0,
    }
}

fn run_pair(cfg: &PanelCfg) -> (PanelOut, PanelOut) {
    (
        run_panel(&cfg.with_bootstrap(false)),
        run_panel(&cfg.with_bootstrap(true)),
    )
}

fn line(run_id: &str, b: &PanelOut, t: &PanelOut) {
    println!(
        "{run_id}: base[stall={} pc={:?} tx={} acks={}] treat[stall={} pc={:?} tx={} acks={} probes={} auth={} echoes={} rtt={}ms] disp={}",
        b.cold_start_stall,
        b.preconverge_exit_ns.map(|v| v / 1_000_000),
        b.tx_data,
        b.ack_batches,
        t.cold_start_stall,
        t.preconverge_exit_ns.map(|v| v / 1_000_000),
        t.tx_data,
        t.ack_batches,
        t.probes_emitted,
        t.probes_authenticated,
        t.echoes_generated,
        t.rtt_sample_ns / 1_000_000,
        data_displacement(b, t)
    );
    println!(
        "    class={} fwd_steps={} rev_steps={} acks_gen={} acks_rev_dropped={} bypass={}",
        t.outcome_class, t.forward_steps, t.reverse_steps, t.acks_generated,
        t.acks_reverse_dropped, t.channel_bypass_violations
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mode = a.get(1).cloned().unwrap_or_else(|| "locked".into());
    let timeout = argu(&a, "--timeout-ns", 65_000_000_000);
    // C2 §3 panel selector. `none` = primary continuity, `ge` = bidirectional robustness.
    let reverse = match arg(&a, "--reverse", "none").as_str() {
        "ge" => GE,
        "none" => LossModel::None,
        other => {
            eprintln!("unknown --reverse {other} (none | ge)");
            std::process::exit(2);
        }
    };
    let panel_kind = if matches!(reverse, LossModel::None) { "primary" } else { "bidirectional" };

    match mode.as_str() {
        // ── §4 LOCKED REGRESSION PANEL ──────────────────────────────────────────────────────
        "locked" => {
            let outdir = arg(&a, "--outdir", "reports/dr-cs-3/panel/locked");
            let cell = "P2-GE20L10-RTT1000";
            let domain = "FSFC2-DIAG42B"; // EXACT seed semantics of the frozen records
            for (panel, tail_on) in [("P0-tail-OFF", false), ("P1-tail-ON", true)] {
                for rep in [5u32, 9, 14, 16, 17, 25, 38, 39] {
                    let mut cfg = base_cfg(cell, domain, rep, 1000, timeout);
                    cfg.tail_on = tail_on;
                    cfg.reverse_model = reverse;
                    let (b, t) = run_pair(&cfg);
                    let run_id = format!("locked-{panel_kind}--{panel}--rep{rep}");
                    line(&run_id, &b, &t);
                    write_with_sidecar(
                        &Path::new(&outdir).join(format!("{run_id}.json")),
                        &pair_json(
                            &Id { run_id: &run_id, panel, cell, domain, rep, rtt_ms: 1000 },
                            &b,
                            &t,
                        ),
                    );
                }
            }
            println!("DRCS3-LOCKED-DONE");
        }

        // ── §5 FRESH PAIRED PANEL ───────────────────────────────────────────────────────────
        "fresh" => {
            let outdir = arg(&a, "--outdir", "reports/dr-cs-3/panel/fresh");
            let n = argu(&a, "--n", 40) as u32;
            let cell = "P2-GE20L10-RTT1000";
            let domain = arg(&a, "--domain", "DRCS3-FRESH-V1");
            for (panel, tail_on) in [("P0-tail-OFF", false), ("P1-tail-ON", true)] {
                for rep in 0..n {
                    let mut cfg = base_cfg(cell, &domain, rep, 1000, timeout);
                    cfg.tail_on = tail_on;
                    cfg.reverse_model = reverse;
                    let (b, t) = run_pair(&cfg);
                    let run_id = format!("fresh-{panel_kind}--{panel}--rep{rep}");
                    line(&run_id, &b, &t);
                    write_with_sidecar(
                        &Path::new(&outdir).join(format!("{run_id}.json")),
                        &pair_json(
                            &Id { run_id: &run_id, panel, cell, domain: &domain, rep, rtt_ms: 1000 },
                            &b,
                            &t,
                        ),
                    );
                }
            }
            println!("DRCS3-FRESH-DONE");
        }

        // ── §6 CONTROL PANELS ───────────────────────────────────────────────────────────────
        "control" => {
            let outdir = arg(&a, "--outdir", "reports/dr-cs-3/panel/control");
            let n = argu(&a, "--n", 8) as u32;

            // (a) NO-LOSS at every frozen RTT cell, both panels.
            for rtt in [50u64, 250, 1000] {
                for (panel, tail_on) in [("P0-tail-OFF", false), ("P1-tail-ON", true)] {
                    for rep in 0..n {
                        let cell = format!("CTRL-NOLOSS-RTT{rtt}");
                        let mut cfg = base_cfg(&cell, "DRCS3-CTRL-V1", rep, rtt, timeout);
                        cfg.tail_on = tail_on;
                        cfg.forward_model = LossModel::None;
                        cfg.reverse_model = LossModel::None;
                        let (b, t) = run_pair(&cfg);
                        let run_id = format!("control-noloss--{panel}--rtt{rtt}--rep{rep}");
                        line(&run_id, &b, &t);
                        write_with_sidecar(
                            &Path::new(&outdir).join(format!("{run_id}.json")),
                            &pair_json(
                                &Id { run_id: &run_id, panel, cell: &cell, domain: "DRCS3-CTRL-V1", rep, rtt_ms: rtt },
                                &b,
                                &t,
                            ),
                        );
                    }
                }
            }

            // (b) IID low loss (p=5%) control cell, paired.
            let iid = LossModel::Iid { p_ppm: 50_000 };
            for (panel, tail_on) in [("P0-tail-OFF", false), ("P1-tail-ON", true)] {
                for rep in 0..n {
                    let cell = "CTRL-IID5-RTT1000";
                    let mut cfg = base_cfg(cell, "DRCS3-CTRL-V1", rep, 1000, timeout);
                    cfg.tail_on = tail_on;
                    cfg.forward_model = iid;
                    let (b, t) = run_pair(&cfg);
                    let run_id = format!("control-iid5--{panel}--rep{rep}");
                    line(&run_id, &b, &t);
                    write_with_sidecar(
                        &Path::new(&outdir).join(format!("{run_id}.json")),
                        &pair_json(
                            &Id { run_id: &run_id, panel, cell, domain: "DRCS3-CTRL-V1", rep, rtt_ms: 1000 },
                            &b,
                            &t,
                        ),
                    );
                }
            }

            // (c) targeted probe/echo loss scenarios (deterministic scripts, GE elsewhere).
            let sc = |name, probe_drops, echo_drops, wire_dups, event_dups| BsScenario {
                name,
                probe_drops,
                echo_drops,
                wire_dups,
                event_dups,
            };
            let scenarios = vec![
                sc("probe1-lost", vec![true, false, false, false], vec![false; 4], 0, 0),
                sc("echo1-lost", vec![false; 4], vec![true, false, false, false], 0, 0),
                sc("all4-probes-lost", vec![true; 4], vec![false; 4], 0, 0),
                sc("all4-echoes-lost", vec![false; 4], vec![true; 4], 0, 0),
                sc("reverse-burst", vec![false; 4], vec![true, true, true, false], 0, 0),
                sc("duplicate-echo", vec![false; 4], vec![false; 4], 3, 3),
            ];
            for scen in scenarios {
                let name = scen.name;
                for (panel, tail_on) in [("P0-tail-OFF", false), ("P1-tail-ON", true)] {
                    for rep in [9u32, 14, 17] {
                        let cell = format!("CTRL-BS-{name}");
                        let mut cfg =
                            base_cfg(&cell, "FSFC2-DIAG42B", rep, 1000, timeout);
                        cfg.tail_on = tail_on;
                        cfg.probe_drop_script = Some(scen.probe_drops.clone());
                        cfg.echo_drop_script = Some(scen.echo_drops.clone());
                        cfg.echo_wire_duplicates = scen.wire_dups;
                        cfg.echo_event_duplicates = scen.event_dups;
                        let (b, t) = run_pair(&cfg);
                        let run_id = format!("control-bs-{name}--{panel}--rep{rep}");
                        line(&run_id, &b, &t);
                        write_with_sidecar(
                            &Path::new(&outdir).join(format!("{run_id}.json")),
                            &pair_json(
                                &Id { run_id: &run_id, panel, cell: &cell, domain: "FSFC2-DIAG42B", rep, rtt_ms: 1000 },
                                &b,
                                &t,
                            ),
                        );
                    }
                }
            }
            println!("DRCS3-CONTROL-DONE");
        }
        other => {
            eprintln!("unknown mode {other} (locked | fresh | control)");
            std::process::exit(2);
        }
    }
}
