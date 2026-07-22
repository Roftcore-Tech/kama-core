//! Commit-AB — policy-aware UDP-bench-runner CLI (root'suz; netem-dış-katmanda).
//! Kullanım: --role tx|rx --bind A --peer B --policy leftover|p15|f15 [--entitlement-share-permille N]
//! [--hardcap-share-permille N] [--entitlement-cap-bytes N] [--hardcap-cap-bytes N] [--ticket-cap N]
//! [--t-floor-multiplier N] [--operational-horizon MS] [--extended-horizon MS] --run-id ID --seed N
//! --output-dir DIR [--w-units N]. Exit: 0=ok, 2=timeout/çalışma-hatası, 3=arg-hatası.
#[cfg(feature = "experimental-fec-budget")]
fn main() {
    use adaptive_transport::udp_runner::{run_rx, run_tx, RunnerCfg};
    use std::time::Duration;
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str, d: &str| -> String {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| d.to_string())
    };
    let role = get("--role", "");
    let cfg = RunnerCfg {
        policy: get("--policy", "leftover"),
        ent_share: get("--entitlement-share-permille", "150")
            .parse()
            .unwrap_or(150),
        hc_share: get("--hardcap-share-permille", "250")
            .parse()
            .unwrap_or(250),
        ent_cap: get("--entitlement-cap-bytes", "4800")
            .parse()
            .unwrap_or(4800),
        hc_cap: get("--hardcap-cap-bytes", "9600").parse().unwrap_or(9600),
        ticket_cap: get("--ticket-cap", "8").parse().unwrap_or(8),
        t_floor_mult: get("--t-floor-multiplier", "2000").parse().unwrap_or(2000),
        op_horizon_ms: get("--operational-horizon", "10000")
            .parse()
            .unwrap_or(10_000),
        ext_horizon_ms: get("--extended-horizon", "60000").parse().unwrap_or(60_000),
        run_id: get("--run-id", "run0"),
        seed: get("--seed", "11").parse().unwrap_or(11),
        out_dir: get("--output-dir", "/tmp/udp-runner"),
        w_units: get("--w-units", "80").parse().unwrap_or(80),
        peer: get("--peer", "127.0.0.1:47002"),
        bind: get("--bind", "127.0.0.1:47001"),
    };
    let deadline = Duration::from_millis(cfg.ext_horizon_ms);
    let r = match role.as_str() {
        "tx" => run_tx(&cfg, deadline),
        "rx" => run_rx(&cfg, deadline),
        _ => {
            eprintln!("--role tx|rx zorunlu");
            std::process::exit(3);
        }
    };
    match r {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("runner-hata: {e}");
            std::process::exit(2);
        }
    }
}
#[cfg(not(feature = "experimental-fec-budget"))]
fn main() {
    eprintln!("experimental-fec-budget feature'ı gerekli");
    std::process::exit(3);
}
