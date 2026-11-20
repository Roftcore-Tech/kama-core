//! Kanal kalibrasyon probe'u — sıra numaralı, sabit hızlı UDP. Yalnız `std`.
//!
//! Amaç: hiçbir protokol kolu koşmadan ÖNCE emüle kanalın AYARLANAN değerleri gerçekten
//! üretip üretmediğini ölçmek (gerçekleşen ileri/dönüş kaybı, RTT medyan/p90).
//!
//!   udp_probe rx --bind A:P --out FILE     her paketi AYNEN geri yollar (echo) ve alınan sayıyı yazar
//!   udp_probe tx --peer A:P --count N --pps R --size B --drain-ms D --out FILE
//!
//! Paket: `u64 seq ‖ u64 tx_ns ‖ dolgu`. İleri kayıp = 1 − rx_alınan/N (rx dosyasından);
//! dönüş kaybı = 1 − echo_alınan/rx_alınan; RTT = echo varışı − tx_ns (yalnız echo'su dönenler).

use std::io::Write;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

fn arg(a: &[String], k: &str) -> Option<String> {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
}
fn arg_u64(a: &[String], k: &str, d: u64) -> u64 {
    arg(a, k).and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn die(m: &str) -> ! {
    eprintln!("udp_probe: {m}");
    std::process::exit(2);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("rx") => rx(&a),
        Some("tx") => tx(&a),
        _ => die("kullanım: udp_probe rx|tx …"),
    }
}

fn rx(a: &[String]) {
    let bind: SocketAddr = arg(a, "--bind")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| die("--bind"));
    let out = arg(a, "--out").unwrap_or_else(|| die("--out"));
    let idle = Duration::from_millis(arg_u64(a, "--idle-ms", 4000));
    let s = UdpSocket::bind(bind).unwrap_or_else(|e| die(&format!("bind: {e}")));
    s.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let mut buf = [0u8; 2048];
    let (mut got, mut uniq_max, mut last) = (0u64, 0u64, None::<Instant>);
    let mut seen = std::collections::BTreeSet::new();
    loop {
        match s.recv_from(&mut buf) {
            Ok((n, from)) if n >= 16 => {
                let seq = u64::from_be_bytes(buf[..8].try_into().unwrap_or([0; 8]));
                if seen.insert(seq) {
                    got += 1;
                    uniq_max = uniq_max.max(seq);
                }
                let _ = s.send_to(&buf[..n], from);
                last = Some(Instant::now());
            }
            _ => {
                if last.is_some_and(|l| l.elapsed() > idle) {
                    break;
                }
            }
        }
    }
    let mut f = std::fs::File::create(&out).unwrap_or_else(|e| die(&format!("out: {e}")));
    let _ = writeln!(f, "{{\"rx_unique\":{got},\"rx_max_seq\":{uniq_max}}}");
}

fn tx(a: &[String]) {
    let peer: SocketAddr = arg(a, "--peer")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| die("--peer"));
    let count = arg_u64(a, "--count", 2000);
    let pps = arg_u64(a, "--pps", 250).max(1);
    let size = arg_u64(a, "--size", 200).clamp(16, 1400) as usize;
    let drain = Duration::from_millis(arg_u64(a, "--drain-ms", 3000));
    let out = arg(a, "--out").unwrap_or_else(|| die("--out"));
    let s = UdpSocket::bind("0.0.0.0:0").unwrap_or_else(|e| die(&format!("bind: {e}")));
    s.connect(peer)
        .unwrap_or_else(|e| die(&format!("connect: {e}")));
    s.set_nonblocking(true).ok();
    let gap = Duration::from_nanos(1_000_000_000 / pps);
    let start = Instant::now();
    let mut rtts: Vec<u64> = Vec::with_capacity(count as usize);
    let mut echoed = std::collections::BTreeSet::new();
    let mut pkt = vec![0u8; size];
    let mut buf = [0u8; 2048];
    let mut send_err = 0u64;
    let mut drain_rx = |rtts: &mut Vec<u64>, echoed: &mut std::collections::BTreeSet<u64>| {
        while let Ok(n) = s.recv(&mut buf) {
            if n >= 16 {
                let seq = u64::from_be_bytes(buf[..8].try_into().unwrap_or([0; 8]));
                let t = u64::from_be_bytes(buf[8..16].try_into().unwrap_or([0; 8]));
                let now = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
                if echoed.insert(seq) {
                    rtts.push(now.saturating_sub(t));
                }
            }
        }
    };
    for seq in 0..count {
        let due = start + gap.saturating_mul(seq as u32);
        while Instant::now() < due {
            drain_rx(&mut rtts, &mut echoed);
            std::thread::sleep(Duration::from_micros(200));
        }
        pkt[..8].copy_from_slice(&seq.to_be_bytes());
        let now = u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
        pkt[8..16].copy_from_slice(&now.to_be_bytes());
        if s.send(&pkt).is_err() {
            send_err += 1;
        }
    }
    let t_end = Instant::now() + drain;
    while Instant::now() < t_end {
        drain_rx(&mut rtts, &mut echoed);
        std::thread::sleep(Duration::from_millis(1));
    }
    rtts.sort_unstable();
    let q = |p: usize| -> u64 {
        if rtts.is_empty() {
            0
        } else {
            rtts[(rtts.len() - 1) * p / 100]
        }
    };
    let mut f = std::fs::File::create(&out).unwrap_or_else(|e| die(&format!("out: {e}")));
    let _ = writeln!(
        f,
        "{{\"sent\":{count},\"send_err\":{send_err},\"echoed\":{},\"rtt_p50_ns\":{},\"rtt_p90_ns\":{},\"rtt_min_ns\":{},\"size\":{size},\"pps\":{pps}}}",
        rtts.len(), q(50), q(90), rtts.first().copied().unwrap_or(0)
    );
}
