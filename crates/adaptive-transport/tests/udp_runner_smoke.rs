//! Commit-AB — UDP-runner LOOPBACK-SMOKE (gerçek-UdpSocket; netem'siz; root'suz).
//! Kapı: Leftover/P15/F15 üçü-de-koşar; ledger/özet/config-dump-üretilir; waste=0; config-hash-
//! deterministik; F15-parity-emisyonu-canlı-UDP-yolunda-gerçekleşir.
#![cfg(feature = "experimental-fec-budget")]
use adaptive_transport::udp_runner::{run_rx, run_tx, RunnerCfg};
use std::time::Duration;

fn cfg(policy: &str, port_base: u16, out: &str) -> RunnerCfg {
    RunnerCfg {
        policy: policy.into(),
        ent_share: 150,
        hc_share: 250,
        ent_cap: 4800,
        hc_cap: 9600,
        ticket_cap: 8,
        t_floor_mult: 2000,
        op_horizon_ms: 4_000,
        ext_horizon_ms: 5_000,
        run_id: format!("smoke-{policy}"),
        seed: 11,
        out_dir: out.into(),
        w_units: 40,
        peer: format!("127.0.0.1:{}", port_base + 1),
        bind: format!("127.0.0.1:{port_base}"),
        fixed_r: 4,
        emit_fuzzy: false,
    }
}

#[test]
fn loopback_three_policies() {
    let out = std::env::temp_dir().join("udp-runner-smoke");
    let out = out.to_string_lossy().to_string();
    for (i, pol) in ["leftover", "p15", "f15"].iter().enumerate() {
        let base = 47010 + (i as u16) * 4;
        let tx = cfg(pol, base, &out);
        let rx = RunnerCfg {
            bind: tx.peer.clone(),
            peer: tx.bind.clone(),
            ..tx.clone()
        };
        let rxh = std::thread::spawn(move || run_rx(&rx, Duration::from_millis(5500)));
        std::thread::sleep(Duration::from_millis(50));
        run_tx(&tx, Duration::from_millis(5000)).expect("tx-ok");
        rxh.join().unwrap().expect("rx-ok");
        let summary =
            std::fs::read_to_string(format!("{out}/smoke-{pol}-summary.csv")).expect("özet-var");
        let row: Vec<&str> = summary.lines().nth(1).unwrap().split(',').collect();
        // kolonlar: run,confighash,policy,enq,emit,canc,waste,ndisp,rdisp,...
        assert_eq!(row[6], "0", "waste=0 ({pol})");
        if *pol != "leftover" {
            assert!(row[4].parse::<u64>().unwrap() > 0, "parity-emit>0 ({pol})");
        }
        assert!(
            std::fs::read_to_string(format!("{out}/smoke-{pol}-config.txt"))
                .unwrap()
                .contains("hash="),
            "config-dump"
        );
    }
    // Config-hash-determinizmi:
    assert_eq!(
        cfg("f15", 1, "x").config_hash(),
        cfg("f15", 2, "y").config_hash() // bind/out-hash'e-girmez; policy-alanları-girer
    );
}

/// FA — receiver GRACEFUL-shutdown lifecycle: FINISH-marker → done-marker(clean=true) + checksum-uyumu
/// + ledger-tam (pending=0 nominal). SIGTERM-YOK (thread-join = temiz-çıkış).
#[test]
fn fa_graceful_shutdown_marker_and_checksum() {
    let out = std::env::temp_dir().join("fa-shutdown-smoke");
    let out = out.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&out);
    let tx = cfg("f15", 47040, &out);
    let tx = RunnerCfg {
        run_id: "fa1".into(),
        w_units: 40,
        op_horizon_ms: 5_000,
        ext_horizon_ms: 12_000,
        ..tx
    };
    let rx = RunnerCfg {
        bind: tx.peer.clone(),
        peer: tx.bind.clone(),
        ..tx.clone()
    };
    let rxh = std::thread::spawn(move || run_rx(&rx, Duration::from_millis(15_000)));
    std::thread::sleep(Duration::from_millis(50));
    run_tx(&tx, Duration::from_millis(6_000)).expect("tx");
    rxh.join().unwrap().expect("rx-clean-exit"); // SIGTERM-YOK: FINISH-marker ile döner
    let marker = std::fs::read_to_string(format!("{out}/fa1-rxdone.marker")).expect("done-marker");
    assert!(
        marker.contains("clean_shutdown=true"),
        "FINISH-yolu-temiz: {marker}"
    );
    assert!(
        marker.contains("pending_blocks=0"),
        "nominal: tüm-blok-finalize"
    );
    // Checksum-integrity: marker'daki checksum ledger'ın-FNV-1a'sıyla-eşleşmeli.
    let body = std::fs::read_to_string(format!("{out}/fa1-rxledger.csv")).unwrap();
    let mut ck = 0xcbf2_9ce4_8422_2325u64;
    for by in body.bytes() {
        ck ^= u64::from(by);
        ck = ck.wrapping_mul(0x0000_0100_0000_01B3);
    }
    assert!(
        marker.contains(&format!("ledger_checksum={ck:x}")),
        "checksum-uyumu (LedgerIntegrity)"
    );
}

/// FA-MUTATION-pini: rx-thread-FINISH-görmeden-DÜŞÜRÜLÜRSE (deadline≈0 → loop-hiç-çalışmaz) done-marker
/// clean_shutdown=false VEYA hiç-yok. Bu, orchestrator'ın "marker-yoksa≠başarılı" kuralının-gerekçesi.
#[test]
fn fa_no_finish_marker_is_not_clean() {
    let out = std::env::temp_dir().join("fa-nofinish-smoke");
    let out = out.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&out);
    let rx = cfg("f15", 47060, &out);
    let rx = RunnerCfg {
        run_id: "fa2".into(),
        bind: "127.0.0.1:47061".into(),
        ..rx
    };
    // Hiç-tx-yok → rx-deadline-1ms → loop-anında-biter (FINISH-görmez) → clean_shutdown=false.
    run_rx(&rx, Duration::from_millis(1)).expect("rx-ok-ama-finish-yok");
    let marker =
        std::fs::read_to_string(format!("{out}/fa2-rxdone.marker")).expect("marker-yine-yazılır");
    assert!(
        marker.contains("clean_shutdown=false"),
        "FINISH-yok → clean=false (orchestrator-IncompleteReceiverLedger-tetiği): {marker}"
    );
}
