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
