//! CLOSURE-2 — A2-RX NETEM RUNNER (DR-CS-1b/A2 FINAL VALIDATION; gerçek RSS/allocator + netem).
//!
//! GERÇEK ürün-yolu: `rx` rolü gerçek `ReceiverHost` (product path: on_datagram → AEAD-open →
//! iki-seviye cold-admission → block-assembly → patience-tick) + `/proc/self/statm` RSS örneklemesi.
//! `tx` rolü sealed-flood (INCOMPLETE source-shard + probe + control) UDP üzerinden; netem araya girer.
//! Anahtar simetrik (CLOSURE-2 KAYNAK-ölçümü; key-semantiği CLOSURE-1B'de doğrulandı).
//!
//! Kullanım: --role tx|rx --bind A --peer B --duration-ms N --sessions N --label PROF --run-id ID --out FILE
//! Freeze: reports/decision-requests/A2-CLOSURE-2-pre-registration.md v2 (sha256=d7b9aea0).

#[cfg(feature = "experimental-cold-start")]
fn main() {
    std::process::exit(imp::run());
}

#[cfg(not(feature = "experimental-cold-start"))]
fn main() {
    eprintln!("experimental-cold-start feature'ı gerekli");
    std::process::exit(3);
}

#[cfg(feature = "experimental-cold-start")]
mod imp {
    use kama_integration::{IngestOutcome, ReceiverHost};
    use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
    use kama_transport_schema::control::Control;
    use kama_transport_schema::inner_kind::ShardKind;
    use kama_transport_schema::prefix::SourcePrefix;
    use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
    use std::io::Write;
    use std::net::UdpSocket;
    use std::time::{Duration, Instant};
    use zeroize::Zeroizing;

    const KEY: [u8; 32] = [0x5A; 32];
    const K_ACTUAL: u8 = 8; // decode-eşiği; INCOMPLETE-flood esi 0..6 (7<8) → decode-etmez
    const PROFILE: u8 = 0x03; // MANDATORY-BOOTSTRAP (E=64)
    const BLOCKS_PER_SESSION: u32 = 16; // >OPEN_BLOCK_CAP(8) → cap-reject görünür
    const ESI_PER_BLOCK: u16 = 7; // K-1 (INCOMPLETE)
    const CLEANUP_NS: u64 = 300 * 1_000_000_000; // >RX_COLD_ABANDON(75s) → evict-all

    fn key() -> Zeroizing<[u8; 32]> {
        Zeroizing::new(KEY)
    }

    fn arg(args: &[String], k: &str, d: &str) -> String {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| d.to_string())
    }

    /// `/proc/self/statm` alan-2 (resident pages) × page_size → bytes (freeze §4).
    fn rss_bytes() -> u64 {
        let s = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
        let resident: u64 = s
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        resident * 4096
    }

    fn seal_source(ep: &mut KamaNoiseWireProtector, block: u32, esi: u16, pn: u64) -> Vec<u8> {
        let pfx = SourcePrefix {
            message_id: 1,
            chunk_offset: 0,
            actual_payload_len: 10,
            source_flags: 0,
        };
        let mut sym = vec![0u8; 64];
        pfx.serialize_symbol(&[0xCD; 10], &mut sym).unwrap();
        ep.seal(
            &UnsealedTransportPacket::FecShard {
                header: LogicalShardHeader {
                    kind: ShardKind::Source,
                    profile_id: PROFILE,
                    block_id: block,
                    esi,
                    k_actual: K_ACTUAL,
                },
                symbol: sym,
            },
            SealArgs {
                wire_pn: pn,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap()
        .bytes
    }

    fn seal_probe(ep: &mut KamaNoiseWireProtector, seq: u16, pn: u64) -> Vec<u8> {
        ep.seal(
            &UnsealedTransportPacket::Control(Control::CsProbe {
                id: 0,
                subtype: 0x01,
                probe_seq: seq,
            }),
            SealArgs {
                wire_pn: pn,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap()
        .bytes
    }

    fn seal_control(ep: &mut KamaNoiseWireProtector, id: u32, pn: u64) -> Vec<u8> {
        ep.seal(
            &UnsealedTransportPacket::Control(Control::SessionClose { id, reason_code: 0 }),
            SealArgs {
                wire_pn: pn,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .unwrap()
        .bytes
    }

    /// h_ext (ilk-4-byte) datagram önüne yazılır mı? seal zaten h_ext'i wire-layout'a koyar; rx demux eder.
    pub fn run() -> i32 {
        let args: Vec<String> = std::env::args().collect();
        let role = arg(&args, "--role", "");
        let bind = arg(&args, "--bind", "127.0.0.1:47101");
        let peer = arg(&args, "--peer", "127.0.0.1:47102");
        let duration_ms: u64 = arg(&args, "--duration-ms", "5000").parse().unwrap_or(5000);
        let sessions: u32 = arg(&args, "--sessions", "64").parse().unwrap_or(64);
        match role.as_str() {
            "tx" => run_tx(&bind, &peer, duration_ms, sessions),
            "rx" => run_rx(
                &bind,
                duration_ms,
                sessions,
                &arg(&args, "--label", "prof"),
                &arg(&args, "--run-id", "r0"),
                &arg(&args, "--out", "/tmp/a2-netem/summary.csv"),
            ),
            _ => {
                eprintln!("--role tx|rx zorunlu");
                3
            }
        }
    }

    fn run_tx(bind: &str, peer: &str, duration_ms: u64, sessions: u32) -> i32 {
        let sock = match UdpSocket::bind(bind) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("tx bind: {e}");
                return 2;
            }
        };
        // Session-başına tx-endpoint (send=KEY) + monoton wire_pn.
        let mut eps: Vec<KamaNoiseWireProtector> = (0..sessions)
            .map(|h| KamaNoiseWireProtector::new(h, &key(), &key()))
            .collect();
        let mut pn = vec![0u64; sessions as usize];
        let mut probe_seq: u16 = 0;
        let mut ctl_id: u32 = 0;
        let start = Instant::now();
        let mut i: u64 = 0;
        while start.elapsed() < Duration::from_millis(duration_ms) {
            // DISTINCT-BLOCK round-robin (freeze §3): ardışık-shard YENİ-bloğa → OPEN_BLOCK_CAP-reject
            // HIZLI görünür (9. distinct-blok → reject); esi yavaş-eksen (retained plateau'ya dolar).
            let block = (i % u64::from(BLOCKS_PER_SESSION)) as u32;
            let esi = ((i / u64::from(BLOCKS_PER_SESSION)) % u64::from(ESI_PER_BLOCK)) as u16;
            for s in 0..sessions as usize {
                let dg = seal_source(&mut eps[s], block, esi, pn[s]);
                pn[s] += 1;
                let _ = sock.send_to(&dg, peer);
            }
            if i.is_multiple_of(3) {
                for s in 0..sessions as usize {
                    let dg = seal_probe(&mut eps[s], probe_seq, pn[s]);
                    pn[s] += 1;
                    let _ = sock.send_to(&dg, peer);
                }
                probe_seq = probe_seq.wrapping_add(1);
            }
            if i.is_multiple_of(5) {
                for s in 0..sessions as usize {
                    let dg = seal_control(&mut eps[s], ctl_id, pn[s]);
                    pn[s] += 1;
                    let _ = sock.send_to(&dg, peer);
                }
                ctl_id = ctl_id.wrapping_add(1);
            }
            i += 1;
            std::thread::sleep(Duration::from_millis(30)); // ~2000 pkt/s pacing (freeze §3)
        }
        0
    }

    #[allow(clippy::too_many_lines)]
    fn run_rx(
        bind: &str,
        duration_ms: u64,
        sessions: u32,
        label: &str,
        run_id: &str,
        out: &str,
    ) -> i32 {
        let sock = match UdpSocket::bind(bind) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("rx bind: {e}");
                return 2;
            }
        };
        sock.set_read_timeout(Some(Duration::from_millis(50))).ok();

        let mut host = ReceiverHost::new();
        for h in 0..sessions {
            if host.install_session_with_keys(h, &key()).is_err() {
                break; // SESSION_COUNT_CAP
            }
        }
        // BASELINE (flood-öncesi; retained==0 anı).
        let baseline_rss = rss_bytes();
        let baseline_retained = host.global_cold().res.retained_bytes;

        let start = Instant::now();
        let mut buf = [0u8; 2048];
        let mut peak_rss = baseline_rss;
        let mut max_retained = baseline_retained;
        let mut peak_sessions = host.session_count();
        let mut peak_tombstones = host.tombstone_count();
        let mut rejects: u64 = 0;
        let mut last_tick = Instant::now();
        // retained örnek-halkası (100ms; son-1s plateau için ~10 örnek).
        let mut retained_ring: Vec<u64> = Vec::new();

        while start.elapsed() < Duration::from_millis(duration_ms) {
            let now_ns = start.elapsed().as_nanos() as u64;
            match sock.recv_from(&mut buf) {
                Ok((n, _)) => {
                    if let Ok(outcome) = host.on_datagram(now_ns, &buf[..n]) {
                        if matches!(outcome, IngestOutcome::Dropped("cold cap exceeded")) {
                            rejects += 1;
                        }
                    }
                }
                Err(_) => { /* read-timeout: loop devam (tick + sample) */ }
            }
            if last_tick.elapsed() >= Duration::from_millis(100) {
                let _ = host.on_patience_tick(now_ns);
                let r = host.global_cold().res.retained_bytes;
                let rss = rss_bytes();
                peak_rss = peak_rss.max(rss);
                max_retained = max_retained.max(r);
                peak_sessions = peak_sessions.max(host.session_count());
                peak_tombstones = peak_tombstones.max(host.tombstone_count());
                retained_ring.push(r);
                last_tick = Instant::now();
            }
        }

        let retained_last = retained_ring.last().copied().unwrap_or(0);
        // ~1s önce (~10 örnek geriden).
        let retained_prev1s = if retained_ring.len() >= 11 {
            retained_ring[retained_ring.len() - 11]
        } else {
            retained_ring.first().copied().unwrap_or(0)
        };
        let cs_echo = host.cs_echo_count();
        let delivered = host.delivered_count();

        // CLEANUP: evict-all (deadline aşımı) → retained baseline'a dönmeli (freeze §7).
        let _ = host.on_patience_tick(CLEANUP_NS);
        let post_clean_retained = host.global_cold().res.retained_bytes;
        let post_clean_rss = rss_bytes();

        // Özet-satır (freeze §4 kolonları).
        let line = format!(
            "{label},{run_id},{baseline_rss},{peak_rss},{max_retained},{retained_last},{retained_prev1s},{post_clean_rss},{post_clean_retained},{rejects},{peak_sessions},{peak_tombstones},{cs_echo},{delivered},{}\n",
            retained_ring.len()
        );
        if let Some(dir) = std::path::Path::new(out).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(out)
        {
            Ok(mut f) => {
                let _ = f.write_all(line.as_bytes());
            }
            Err(e) => {
                eprintln!("out yaz: {e}");
                return 2;
            }
        }
        eprintln!("{label}/{run_id}: baseline_rss={baseline_rss} peak_rss={peak_rss} max_retained={max_retained} post_clean_retained={post_clean_retained} rejects={rejects} sessions={peak_sessions} tombstones={peak_tombstones} cs_echo={cs_echo}");
        0
    }
}
