//! DR-ARQ-PRODUCT-TAIL-1 — faithful two-netns PRODUCT tail-recovery validation driver (tx/rx roles).
//!
//! Drives the PRODUCT `adaptive_transport::sender::Sender` (the real transport state machine) over a REAL
//! UDP socket path. Two roles:
//!   * `tx` — constructs `Sender::new()`, arms the bounded tail probe OFF (default product semantics) or ON
//!     (frozen product config), offers a fixed workload, fires `Timer::Pto` each tick, emits every
//!     `TransmitDecision` as a datagram, ingests SACKs as `Event::AckReceived`, and writes ONE §7 result row.
//!   * `rx` — a faithful SACK-generating endpoint: acks received data seqs (product ACK semantics). For the
//!     `terminal-tail` structure it deterministically drops the FIRST transmission of the terminal source
//!     unit (single positional tail-loss event) and accepts the probe copy — matching R2A S1. IID/burst loss
//!     is applied faithfully by netem on the wire (rx drops nothing there).
//!
//! The probe traverses the SAME impaired UDP path as data (faithful). No FEC is engaged (ARQ/tail only).
//! Wire framing (self-contained, NOT ccr — this is a harness):
//!   DATA [0xD0][seq:u64 BE][unit:u64 BE][pad → size]   SACK [0xA0][cnt:u16 BE][seq:u64 BE × cnt]   FIN [0xFF]
//!
//! Usage:
//!   --role tx --bind A --peer B --arm off|on --w-units N --unit-bytes N --op-horizon-ms N --lb-ms N
//!            --ext-horizon-ms N --tick-ms N --run-id ID --cell C --struct S --loss L --rtt R --rep N
//!            --result-out FILE
//!   --role rx --bind A --w-units N --drop-terminal N --ext-horizon-ms N

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

const BASE: u64 = 1_000_000; // workload unit-id base (both roles agree)
const MS: u64 = 1_000_000;

/// FROZEN product ON config (RFC-9002-flavored bounded backoff). Locked for the validation grid.
fn tail_on_config() -> TailRecoveryConfig {
    TailRecoveryConfig {
        enabled: true,
        min_pto_ns: 10 * MS,             // 10 ms floor
        max_pto_ns: 10_000 * MS,         // 10 s base ceiling
        initial_pto_ns: 1_000 * MS,      // 1 s when no RTT sample (Karn)
        max_backoff_pto_ns: 30_000 * MS, // 30 s absolute backoff ceiling
        max_ack_delay_ns: 25 * MS,       // 25 ms
        max_tail_probes: 3,
    }
}

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

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let code = match arg(&a, "--role", "").as_str() {
        "tx" => run_tx(&a),
        "rx" => run_rx(&a),
        _ => {
            eprintln!("--role tx|rx zorunlu");
            3
        }
    };
    std::process::exit(code);
}

fn enc_data(seq: u64, unit: u64, size: u64) -> Vec<u8> {
    let n = (size as usize).max(17);
    let mut v = vec![0u8; n];
    v[0] = 0xD0;
    v[1..9].copy_from_slice(&seq.to_be_bytes());
    v[9..17].copy_from_slice(&unit.to_be_bytes());
    v
}

/// Emit every decision as a datagram; returns (wire_bytes, retx_decisions). Records seq→unit.
fn emit_all(
    sock: &UdpSocket,
    out: &[TransmitDecision],
    seq_unit: &mut BTreeMap<u64, u64>,
    wire_bytes: &mut u64,
    retx_decisions: &mut u64,
) {
    for d in out {
        match d.kind {
            TransmitKind::Data | TransmitKind::Retransmit | TransmitKind::Duplicate => {
                seq_unit.insert(d.seq, d.unit_id);
                let dg = enc_data(d.seq, d.unit_id, d.size);
                *wire_bytes += dg.len() as u64;
                if d.kind == TransmitKind::Retransmit {
                    *retx_decisions += 1;
                }
                let _ = sock.send(&dg);
            }
            _ => {} // FecParity / CsProbe not engaged in this ARQ-tail harness
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run_tx(a: &[String]) -> i32 {
    let bind = arg(a, "--bind", "10.9.0.1:47401");
    let peer = arg(a, "--peer", "10.9.0.2:47402");
    let arm = arg(a, "--arm", "off");
    let w_units = argu(a, "--w-units", 24);
    let unit_bytes = argu(a, "--unit-bytes", 1200);
    let op_horizon_ms = argu(a, "--op-horizon-ms", 2000);
    let lb_ms = argu(a, "--lb-ms", 600);
    let ext_horizon_ms = argu(a, "--ext-horizon-ms", 30000);
    let tick_ms = argu(a, "--tick-ms", 10);
    let run_id = arg(a, "--run-id", "r0");
    let cell = arg(a, "--cell", "cell");
    let structure = arg(a, "--struct", "iid");
    let loss = arg(a, "--loss", "0");
    let rtt = arg(a, "--rtt", "low");
    let rep = argu(a, "--rep", 0);
    let result_out = arg(a, "--result-out", "");
    let verbose = a.iter().any(|x| x == "--verbose");

    let sock = match UdpSocket::bind(&bind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("tx bind {bind}: {e}");
            return 2;
        }
    };
    if sock.connect(&peer).is_err() {
        eprintln!("tx connect {peer}");
        return 2;
    }
    sock.set_read_timeout(Some(Duration::from_millis(1))).ok();

    let mut s = Sender::new(); // tail probe stays OFF through warmup; enabled fresh below (if arm==on)

    let start = Instant::now();
    let now_ns = |st: &Instant| st.elapsed().as_nanos() as u64;
    let mut seq_unit: BTreeMap<u64, u64> = BTreeMap::new();
    let mut delivered: BTreeSet<u64> = BTreeSet::new();
    let mut acked_seen: BTreeSet<u64> = BTreeSet::new();
    let mut wire_bytes: u64 = 0;
    let mut retx_decisions: u64 = 0;
    let mut offered: u64 = 0;
    let mut last_tick = Instant::now();
    let mut t_complete_ms: Option<u64> = None;
    let mut recovered_via_probe = false;
    let mut buf = [0u8; 4096];
    let terminal_unit = BASE + w_units - 1;

    // ── WARMUP (NOT counted in the measured row): bring the flow out of PreConverge and drain over the
    //    (impaired) path using normal ARQ only. The tail-recovery mechanism targets a CONVERGED flow;
    //    cold-start bootstrap loss is a separate concern (DR-CS-1), not what this validates. Warmup ids live
    //    in a disjoint space (900_000+) that rx never terminal-drops. Bounded by warm_deadline.
    let warm_base = 900_000u64;
    let warm_n = 8u64;
    let mut warm_off = 0u64;
    let warm_deadline_ms = (ext_horizon_ms / 2).max(2000);
    loop {
        let el = start.elapsed().as_millis() as u64;
        let (_st, _bw, srtt_us, _rate, inflight) = s.diag_rate();
        let converged = srtt_us > 0;
        if converged && inflight == 0 && warm_off >= warm_n {
            break;
        }
        if el >= warm_deadline_ms {
            break;
        }
        // steady trickle so lost warmup units get packet-threshold-recovered (a later packet always follows)
        if warm_off < warm_n || (!converged && warm_off < warm_n + el / 200) {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: warm_base + warm_off,
                    size: unit_bytes,
                }),
                now_ns(&start),
            );
            warm_off += 1;
            emit_all(
                &sock,
                &out,
                &mut seq_unit,
                &mut wire_bytes,
                &mut retx_decisions,
            );
        }
        if last_tick.elapsed() >= Duration::from_millis(tick_ms) {
            let out = s.on_event(Event::TimerFired(Timer::Pto), now_ns(&start));
            emit_all(
                &sock,
                &out,
                &mut seq_unit,
                &mut wire_bytes,
                &mut retx_decisions,
            );
            last_tick = Instant::now();
        }
        while let Ok(n) = sock.recv(&mut buf) {
            if n >= 3 && buf[0] == 0xA0 {
                let cnt = u16::from_be_bytes([buf[1], buf[2]]) as usize;
                let mut fresh = Vec::new();
                for i in 0..cnt.min((n - 3) / 8) {
                    let off = 3 + i * 8;
                    let q = u64::from_be_bytes(buf[off..off + 8].try_into().unwrap());
                    if acked_seen.insert(q) {
                        fresh.push(q);
                    }
                }
                if !fresh.is_empty() {
                    let out = s.on_event(
                        Event::AckReceived(AckInfo {
                            acked_seqs: fresh,
                            ack_delay: Duration::ZERO,
                        }),
                        now_ns(&start),
                    );
                    emit_all(
                        &sock,
                        &out,
                        &mut seq_unit,
                        &mut wire_bytes,
                        &mut retx_decisions,
                    );
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // Discard warmup from the MEASURED metrics; enable the tail probe fresh for the measured phase.
    wire_bytes = 0;
    retx_decisions = 0;
    delivered.clear();
    let measured_start_ms = start.elapsed().as_millis() as u64;
    if arm == "on" {
        s.set_tail_recovery_config(tail_on_config());
    }

    loop {
        let el_ms = (start.elapsed().as_millis() as u64).saturating_sub(measured_start_ms);
        if el_ms >= ext_horizon_ms {
            break;
        }
        // offer the WHOLE workload up front (app hands the transport its data); the sender's pacer serializes
        // it at ~0.8·BtlBw — matching the frozen deadline-feasibility serialize (4848 B ≈ 48.5 ms @ 800 kbps).
        while offered < w_units {
            let ev = Event::AppData(OpaqueUnit {
                id: BASE + offered,
                size: unit_bytes,
            });
            offered += 1;
            let out = s.on_event(ev, now_ns(&start));
            emit_all(
                &sock,
                &out,
                &mut seq_unit,
                &mut wire_bytes,
                &mut retx_decisions,
            );
        }
        // tick: fire PTO (drives loss-detection + bounded tail probe)
        if last_tick.elapsed() >= Duration::from_millis(tick_ms) {
            let out = s.on_event(Event::TimerFired(Timer::Pto), now_ns(&start));
            emit_all(
                &sock,
                &out,
                &mut seq_unit,
                &mut wire_bytes,
                &mut retx_decisions,
            );
            last_tick = Instant::now();
            if verbose && el_ms.is_multiple_of(250) {
                let (st, btlbw, srtt_us, rate, inflight) = s.diag_rate();
                let (due, sent, ..) = s.tail_probe_diag_ext();
                eprintln!(
                    "  t={el_ms}ms st={st:?} btlbw={btlbw} srtt_us={srtt_us} rate={rate} inflight={inflight} offered={offered} deliv={} wire={wire_bytes} due={due} sent={sent}",
                    delivered.len()
                );
            }
        }
        // ingest SACKs
        while let Ok(n) = sock.recv(&mut buf) {
            if n >= 3 && buf[0] == 0xA0 {
                let cnt = u16::from_be_bytes([buf[1], buf[2]]) as usize;
                let mut fresh = Vec::new();
                for i in 0..cnt.min((n - 3) / 8) {
                    let off = 3 + i * 8;
                    let q = u64::from_be_bytes(buf[off..off + 8].try_into().unwrap());
                    if acked_seen.insert(q) {
                        fresh.push(q);
                    }
                }
                if !fresh.is_empty() {
                    // map acked seq → unit → delivered
                    for &q in &fresh {
                        if let Some(&u) = seq_unit.get(&q) {
                            if delivered.insert(u) && u == terminal_unit {
                                // terminal delivered: probe-caused iff at least one probe was emitted
                                let (_due, sent, ..) = s.tail_probe_diag_ext();
                                if sent > 0 {
                                    recovered_via_probe = true;
                                }
                            }
                        }
                    }
                    let out = s.on_event(
                        Event::AckReceived(AckInfo {
                            acked_seqs: fresh,
                            ack_delay: Duration::ZERO,
                        }),
                        now_ns(&start),
                    );
                    emit_all(
                        &sock,
                        &out,
                        &mut seq_unit,
                        &mut wire_bytes,
                        &mut retx_decisions,
                    );
                }
            }
        }
        // completion
        if t_complete_ms.is_none() && delivered.len() as u64 == w_units {
            t_complete_ms = Some(el_ms);
        }
        // post-completion settle = a few RTT for the final ACK flush only (NOT the scientific horizon;
        // censoring still uses ext_horizon_ms). Keeps completing rows short without adaptive stopping.
        if let Some(tc) = t_complete_ms {
            let srtt_ms = s.diag_rate().2 / 1000; // srtt_us → ms
            let settle = (2 * srtt_ms).clamp(100, 1000);
            if el_ms >= tc + settle {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // FIN (best-effort ×3 for reverse-loss)
    for _ in 0..3 {
        let _ = sock.send(&[0xFF]);
    }

    let (due, sent, cancelled, probe_bytes, spurious, timer_fires) = s.tail_probe_diag_ext();
    let complete = t_complete_ms.is_some();
    let tc = t_complete_ms.unwrap_or(0);
    let deadline_hit = complete && tc <= op_horizon_ms;
    let deadline3xlb_hit = complete && tc <= 3 * lb_ms;
    let arq_retx = retx_decisions.saturating_sub(sent); // probe emissions are also Retransmit-kind
    let token_starved = due.saturating_sub(sent).saturating_sub(cancelled);
    let cap_exhausted = u64::from(sent >= u64::from(tail_on_config().max_tail_probes) && !complete);
    let cause = if !complete {
        "none"
    } else if recovered_via_probe {
        "probe"
    } else if arq_retx > 0 {
        "arq"
    } else {
        "direct"
    };
    // invariant checks (raw; analysis aggregates)
    let mut inv_fail = 0u64;
    if arm == "off" && (sent > 0 || due > 0 || probe_bytes > 0) {
        inv_fail += 1; // OFF must never probe
    }
    if sent > u64::from(tail_on_config().max_tail_probes).saturating_mul(w_units.max(1)) {
        inv_fail += 1; // grossly unbounded (sanity; per-window cap is max_tail_probes)
    }
    if sent > due {
        inv_fail += 1; // sent can never exceed due
    }

    let header = "run_id,cell,arm,rep,struct,loss,rtt,w_units,unit_bytes,op_horizon_ms,lb_ms,ext_horizon_ms,complete,t_complete_ms,deadline_hit,deadline3xlb_hit,probe_due,probe_sent,probe_cancelled,token_starved,probe_bytes,spurious,timer_fires,arq_retx,wire_bytes,cap_exhausted,recovery_cause,inv_fail";
    let row = format!(
        "{run_id},{cell},{arm},{rep},{structure},{loss},{rtt},{w_units},{unit_bytes},{op_horizon_ms},{lb_ms},{ext_horizon_ms},{},{},{},{},{due},{sent},{cancelled},{token_starved},{probe_bytes},{spurious},{timer_fires},{arq_retx},{wire_bytes},{cap_exhausted},{cause},{inv_fail}",
        u8::from(complete),
        t_complete_ms.map_or("null".to_string(), |v| v.to_string()),
        u8::from(deadline_hit),
        u8::from(deadline3xlb_hit),
    );
    eprintln!("{run_id} {arm} {structure}/{loss}/{rtt}: complete={complete} tc={tc}ms due={due} sent={sent} cancel={cancelled} arq_retx={arq_retx} wire={wire_bytes} cause={cause} inv_fail={inv_fail}");
    if !result_out.is_empty() {
        if let Some(dir) = std::path::Path::new(&result_out).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let write_header = !std::path::Path::new(&result_out).exists();
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&result_out)
        {
            if write_header {
                let _ = writeln!(f, "{header}");
            }
            let _ = writeln!(f, "{row}");
        }
    }
    0
}

fn run_rx(a: &[String]) -> i32 {
    let bind = arg(a, "--bind", "10.9.0.2:47402");
    let w_units = argu(a, "--w-units", 24);
    let drop_terminal = argu(a, "--drop-terminal", 0);
    let ext_horizon_ms = argu(a, "--ext-horizon-ms", 30000);
    let sack_ms = argu(a, "--sack-ms", 5);

    let sock = match UdpSocket::bind(&bind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rx bind {bind}: {e}");
            return 2;
        }
    };
    sock.set_read_timeout(Some(Duration::from_millis(5))).ok();

    let terminal_lo = BASE + w_units - drop_terminal; // units in [terminal_lo, BASE+w_units) are terminal
    let mut dropped_once: BTreeSet<u64> = BTreeSet::new(); // terminal unit ids already dropped one arrival
    let mut recv_ring: Vec<u64> = Vec::new(); // received seqs (SACK payload; recent window, redundant)
    let mut peer: Option<std::net::SocketAddr> = None;
    let mut last_sack = Instant::now();
    let start = Instant::now();
    let mut buf = [0u8; 4096];
    let mut fin = false;

    while start.elapsed() < Duration::from_millis(ext_horizon_ms) && !fin {
        if let Ok((n, src)) = sock.recv_from(&mut buf) {
            peer.get_or_insert(src);
            if n >= 1 && buf[0] == 0xFF {
                fin = true;
            } else if n >= 17 && buf[0] == 0xD0 {
                let seq = u64::from_be_bytes(buf[1..9].try_into().unwrap());
                let unit = u64::from_be_bytes(buf[9..17].try_into().unwrap());
                // terminal-tail: drop the FIRST arrival of a terminal unit (positional single-loss),
                // accept the probe copy (2nd arrival). Non-terminal / non-tail structures: accept all.
                if drop_terminal > 0 && unit >= terminal_lo && dropped_once.insert(unit) {
                    // dropped this original transmission; do NOT ack.
                } else {
                    recv_ring.push(seq);
                    if recv_ring.len() > 256 {
                        let cut = recv_ring.len() - 256;
                        recv_ring.drain(0..cut);
                    }
                }
            }
        }
        // periodic SACK: redundant recent-received window (survives reverse-loss)
        if last_sack.elapsed() >= Duration::from_millis(sack_ms) && !recv_ring.is_empty() {
            if let Some(p) = peer {
                let cnt = recv_ring.len().min(200);
                let slice = &recv_ring[recv_ring.len() - cnt..];
                let mut dg = Vec::with_capacity(3 + cnt * 8);
                dg.push(0xA0);
                dg.extend_from_slice(&(cnt as u16).to_be_bytes());
                for &q in slice {
                    dg.extend_from_slice(&q.to_be_bytes());
                }
                let _ = sock.send_to(&dg, p);
            }
            last_sack = Instant::now();
        }
    }
    // final SACK burst so the last delivery is acked
    if let Some(p) = peer {
        if !recv_ring.is_empty() {
            let cnt = recv_ring.len().min(200);
            let slice = &recv_ring[recv_ring.len() - cnt..];
            let mut dg = Vec::with_capacity(3 + cnt * 8);
            dg.push(0xA0);
            dg.extend_from_slice(&(cnt as u16).to_be_bytes());
            for &q in slice {
                dg.extend_from_slice(&q.to_be_bytes());
            }
            for _ in 0..3 {
                let _ = sock.send_to(&dg, p);
            }
        }
    }
    eprintln!(
        "rx done: received_window={} dropped_terminal={}",
        recv_ring.len(),
        dropped_once.len()
    );
    0
}
