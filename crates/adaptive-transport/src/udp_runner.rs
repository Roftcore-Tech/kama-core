//! Commit-AB — POLICY-AWARE UDP BENCH-RUNNER (feature-gated; ÜRETİM-YOLUNA-BAĞLI-DEĞİL; root'suz —
//! root-yalnız-dış-netns/tc-katmanında). GERÇEK-UdpSocket + GERÇEK-Sender-budget-yolu; default-policy=
//! LeftoverOnly; product/admin-activation-YOK. Çıktı: blok-ledger-CSV (pos-şema-alt-kümesi) + run-özeti
//! + canonical-config-dump (config-hash + build-sürümü). Exit: 0=başarı, 2=timeout, 3=arg-hatası.
#![allow(clippy::missing_errors_doc)]

use crate::budget_kernel::PolicyParams;
use crate::sender::{
    AckInfo, BudgetPolicy, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, SrLogicalRef,
    Timer, TransmitKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

pub const UNIT: usize = 1200;
const K: u64 = 8;

#[derive(Debug, Clone)]
pub struct RunnerCfg {
    pub policy: String, // leftover|p15|f15 (+param-alanları-CLI'dan)
    pub ent_share: u32,
    pub hc_share: u32,
    pub ent_cap: u64,
    pub hc_cap: u64,
    pub ticket_cap: u32, // bilgi-amaçlı (kernel-AdmissionCaps-sabit-8; dump'ta-görünür)
    pub t_floor_mult: u32,
    pub op_horizon_ms: u64,
    pub ext_horizon_ms: u64,
    pub run_id: String,
    pub seed: u64,
    pub out_dir: String,
    pub w_units: u64,
    pub peer: String,
    pub bind: String,
    pub fixed_r: u16,     // FD-oracle: sabit-r (0..=4; close_fec_block_manual-r)
    pub emit_fuzzy: bool, // FC: fuzzy-event-dataset yaz (additive; decision-değişmez)
}

impl RunnerCfg {
    #[must_use]
    pub fn canonical_dump(&self) -> String {
        format!(
            "policy={};ent_share={};hc_share={};ent_cap={};hc_cap={};ticket_cap={};t_floor_mult={};op={};ext={};seed={};w={}",
            self.policy, self.ent_share, self.hc_share, self.ent_cap, self.hc_cap,
            self.ticket_cap, self.t_floor_mult, self.op_horizon_ms, self.ext_horizon_ms,
            self.seed, self.w_units
        )
    }
    #[must_use]
    pub fn config_hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in self.canonical_dump().bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        h
    }
    fn to_policy(&self) -> Result<BudgetPolicy, String> {
        let mut p = PolicyParams::defaults();
        p.share_permille = self.ent_share;
        p.hardcap_share_permille = self.hc_share;
        p.entitlement_cap_bytes = self.ent_cap;
        p.hardcap_cap_bytes = self.hc_cap;
        p.floor_mult_permille = self.t_floor_mult;
        match self.policy.as_str() {
            "leftover" => Ok(BudgetPolicy::LeftoverOnly),
            "p15" => Ok(BudgetPolicy::ProtectedShareExperimental(p)),
            "f15" | "f10" | "f20" => Ok(BudgetPolicy::FloorCapExperimental(p)),
            other => Err(format!("bilinmeyen-policy: {other}")),
        }
    }
}

/// Datagram: [kind u8][seq u64][unit u64][block u32][esi u16] + dolgu (size-temsili).
fn enc(kind: u8, seq: u64, unit: u64, b: u32, e: u16, size: usize) -> Vec<u8> {
    let mut v = vec![0u8; size.max(23)];
    v[0] = kind;
    v[1..9].copy_from_slice(&seq.to_be_bytes());
    v[9..17].copy_from_slice(&unit.to_be_bytes());
    v[17..21].copy_from_slice(&b.to_be_bytes());
    v[21..23].copy_from_slice(&e.to_be_bytes());
    v
}

/// RX-tarafı: alınanları-say + kümülatif-SACK + MDS-decode → RECOVERED bildir. Root'suz.
pub fn run_rx(cfg: &RunnerCfg, deadline: Duration) -> Result<(), String> {
    let sock = UdpSocket::bind(&cfg.bind).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_millis(20))).ok();
    let start = Instant::now();
    let mut acked: BTreeSet<u64> = BTreeSet::new();
    let mut have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
    let mut recovered: Vec<(u32, u16)> = Vec::new();
    let mut peer: Option<std::net::SocketAddr> = None;
    let mut buf = [0u8; 2048];
    let mut last_sack = Instant::now();
    let mut last_rx = Instant::now(); // erken-çıkış: ilk-paketten-sonra-5s-sessizlik → bitir
    let mut seen_any = false;
    let mut rx_start: Option<Instant> = None;
    let mut block_complete: BTreeMap<u32, u64> = BTreeMap::new(); // block → complete-ms (K-source-hazır)
    let mut block_first: BTreeMap<u32, u64> = BTreeMap::new(); // block → ilk-görülme-ms (open-proxy)
    let mut src_have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new(); // yalnız-source-esi (<K)
    let mut finish_seen = 0u32; // FA: sender-FINISH-marker sayacı (temiz-bitiş)
    let mut ev_count = 0u64;
    while start.elapsed() < deadline {
        // FA: idle-erken-çıkış YALNIZ-FALLBACK (FINISH-marker gelmezse); normal-yol=FINISH-marker.
        if seen_any && last_rx.elapsed() > Duration::from_secs(8) {
            break;
        }
        if finish_seen >= 3 {
            break; // FINISH-marker ≥3-onaylı (UDP-kaybına-karşı) → temiz-drain-bitti
        }
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            if n >= 1 && buf[0] == 0xFE {
                // FA: sender-FINISH-marker (workload-tamam) → ACK-FINISH + drain-onayı.
                peer = Some(from);
                seen_any = true;
                finish_seen += 1;
                last_rx = Instant::now();
                let _ = sock.send_to(&[0xFEu8], from);
                continue;
            }
            if n >= 23 {
                peer = Some(from);
                seen_any = true;
                ev_count += 1;
                last_rx = Instant::now();
                let seq = u64::from_be_bytes(buf[1..9].try_into().unwrap());
                let b = u32::from_be_bytes(buf[17..21].try_into().unwrap());
                let e = u16::from_be_bytes(buf[21..23].try_into().unwrap());
                acked.insert(seq);
                let now_ms = rx_start
                    .get_or_insert_with(Instant::now)
                    .elapsed()
                    .as_millis() as u64;
                block_first.entry(b).or_insert(now_ms);
                // Source-completion: distinct-source-esi (<K) tam-K → blok-teslim (decode-veya-doğrudan).
                if e < K as u16 {
                    src_have.entry(b).or_default().insert(e);
                }
                let s = have.entry(b).or_default();
                let newly = s.insert(e);
                if s.len() as u64 >= K {
                    // MDS: distinct≥K → tüm-K-source-kurtarılabilir → blok-tamam.
                    block_complete.entry(b).or_insert(now_ms);
                } else if src_have.get(&b).is_some_and(|x| x.len() as u64 >= K) {
                    block_complete.entry(b).or_insert(now_ms);
                }
                // MDS-decode-modeli: distinct ≥ K ∧ eksik-source → recovered-bildir.
                if newly && s.len() as u64 >= K {
                    for miss in 0..K as u16 {
                        if !s.contains(&miss) {
                            recovered.push((b, miss));
                            s.insert(miss);
                        }
                    }
                }
            }
        }
        if last_sack.elapsed() >= Duration::from_millis(50) {
            if let Some(p) = peer {
                // SACK: [0xA0][count u16][seq...]; RECOVERED: [0xA1][count][block u32+esi u16...]
                let tail: Vec<u64> = acked.iter().rev().take(64).copied().collect();
                let mut v = vec![0xA0u8];
                v.extend((tail.len() as u16).to_be_bytes());
                for q in &tail {
                    v.extend(q.to_be_bytes());
                }
                let _ = sock.send_to(&v, p);
                if !recovered.is_empty() {
                    let mut r = vec![0xA1u8];
                    let take: Vec<_> = recovered.drain(..).take(100).collect();
                    r.extend((take.len() as u16).to_be_bytes());
                    for (b, e) in take {
                        r.extend(b.to_be_bytes());
                        r.extend(e.to_be_bytes());
                    }
                    let _ = sock.send_to(&r, p);
                }
            }
            last_sack = Instant::now();
        }
    }
    // FA: RECEIVER-GRACEFUL-FINALIZE — ledger + checksum + done-marker (SIGTERM-değil, temiz-yol).
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;
    let op = cfg.op_horizon_ms;
    let mut body = String::from("run_id,block,first_ms,complete_ms,rel_ms,dl_op\n");
    let (mut finalized, mut pending) = (0u64, 0u64);
    for (b, first) in &block_first {
        let (comp, rel, dlop) = match block_complete.get(b) {
            Some(&c) => {
                finalized += 1;
                (c as i64, (c - first) as i64, u8::from((c - first) <= op))
            }
            None => {
                pending += 1;
                (-1, -1, 0)
            }
        };
        body.push_str(&format!("{},{b},{first},{comp},{rel},{dlop}\n", cfg.run_id));
    }
    // FNV-1a checksum (ledger-integrity; deterministik).
    let mut ck = 0xcbf2_9ce4_8422_2325u64;
    for by in body.bytes() {
        ck ^= u64::from(by);
        ck = ck.wrapping_mul(0x0000_0100_0000_01B3);
    }
    let lpath = format!("{}/{}-rxledger.csv", cfg.out_dir, cfg.run_id);
    std::fs::write(&lpath, &body).map_err(|e| e.to_string())?;
    // FA: done-marker (orchestrator-BAŞARI-teyidi; marker-yoksa run≠başarılı).
    let clean = finish_seen >= 3; // FINISH-onaylı-mı (yoksa idle-fallback)
    let marker = format!(
        "run_id={}\npolicy={}\nreceiver_events={ev_count}\nfinalized_blocks={finalized}\npending_blocks={pending}\nledger_path={lpath}\nledger_checksum={ck:x}\nclean_shutdown={clean}\n",
        cfg.run_id, cfg.policy
    );
    std::fs::write(
        format!("{}/{}-rxdone.marker", cfg.out_dir, cfg.run_id),
        marker,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// TX-tarafı: GERÇEK-Sender (budget-path) + gerçek-zaman-event-döngüsü. Ledger+özet-CSV yazar.
#[allow(clippy::too_many_lines)]
pub fn run_tx(cfg: &RunnerCfg, deadline: Duration) -> Result<(), String> {
    let sock = UdpSocket::bind(&cfg.bind).map_err(|e| e.to_string())?;
    sock.connect(&cfg.peer).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_millis(1))).ok();
    let mut s = Sender::new();
    if cfg.policy != "off" {
        s.set_manual_fec_enabled(true).map_err(|e| e.to_string())?;
        s.set_budget_policy(cfg.to_policy()?)
            .map_err(|e| format!("{e:?}"))?;
    }
    let start = Instant::now();
    let now_ns = |st: &Instant| -> u64 { st.elapsed().as_nanos() as u64 };
    let mut app = 0u64;
    let mut emitted_in_block: BTreeMap<u32, u64> = BTreeMap::new();
    let mut open_ms: BTreeMap<u32, u64> = BTreeMap::new();
    let mut close_ms: BTreeMap<u32, u64> = BTreeMap::new();
    let mut par_emit: BTreeMap<u32, u64> = BTreeMap::new();
    let mut acked_seen: BTreeSet<u64> = BTreeSet::new();
    let mut buf = [0u8; 4096];
    let mut last_tick = Instant::now();
    let base = 1_000_000u64;
    // FC: canonical fuzzy-event dataset (additive; existing-diag-poll; decision-DEĞİŞMEZ).
    let mut fuzzy_rows = String::new();
    let mut last_ev_idx = u64::MAX;
    while start.elapsed() < deadline {
        let t_ms = start.elapsed().as_millis() as u64;
        // FC: fuzzy-event-yakalama (event_index-ilerlediğinde tek-satır; salt-gözlem).
        if cfg.emit_fuzzy {
            if let Some(fi) = s.diag_fusion_input_snapshot() {
                if fi.event_index != last_ev_idx {
                    last_ev_idx = fi.event_index;
                    let dec = s.last_decision();
                    let (_st, btlbw, srtt_us, rate, inflight) = s.diag_rate();
                    fuzzy_rows.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:?},{}\n",
                        cfg.run_id,
                        cfg.policy,
                        fi.event_index,
                        t_ms,
                        fi.loss_windowed_ppm,
                        fi.qd_ratio_permille,
                        fi.jitter_ppm,
                        fi.ack_confidence_permille,
                        fi.bandwidth_bps,
                        btlbw,
                        srtt_us,
                        rate,
                        inflight,
                        dec.fec_parity_ratio_permille,
                        dec.mode,
                        u8::from(fi.no_rule)
                    ));
                }
            }
        }
        // app + tick (inline):
        if app < cfg.w_units && t_ms >= app * 40 {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: base + app,
                    size: UNIT as u64,
                }),
                now_ns(&start),
            );
            app += 1;
            emit(
                &sock,
                &out,
                &mut emitted_in_block,
                &mut open_ms,
                &mut par_emit,
                t_ms,
                base,
            );
        }
        if last_tick.elapsed() >= Duration::from_millis(10) {
            let out = s.on_event(Event::TimerFired(Timer::Pto), now_ns(&start));
            emit(
                &sock,
                &out,
                &mut emitted_in_block,
                &mut open_ms,
                &mut par_emit,
                t_ms,
                base,
            );
            last_tick = Instant::now();
        }
        // Blok-tamam → otorite-bağlı-close (gerçek-budget-yolu).
        let ready: Vec<u32> = emitted_in_block
            .iter()
            .filter(|&(_, &c)| c >= K)
            .map(|(&b, _)| b)
            .collect();
        for b in ready {
            emitted_in_block.remove(&b);
            let a = s.open_fec_block_authority();
            let _ = s.close_fec_block_manual(a, b, 4, UNIT as u64);
            close_ms.insert(b, t_ms);
        }
        // Geri-yol: SACK/RECOVERED.
        while let Ok(n) = sock.recv(&mut buf) {
            if n < 3 {
                continue;
            }
            let cnt = u16::from_be_bytes(buf[1..3].try_into().unwrap()) as usize;
            if buf[0] == 0xA0 {
                let mut new = Vec::new();
                for i in 0..cnt.min((n - 3) / 8) {
                    let q = u64::from_be_bytes(buf[3 + i * 8..11 + i * 8].try_into().unwrap());
                    if acked_seen.insert(q) {
                        new.push(q);
                    }
                }
                if !new.is_empty() {
                    let out = s.on_event(
                        Event::AckReceived(AckInfo {
                            acked_seqs: new,
                            ack_delay: Duration::ZERO,
                        }),
                        now_ns(&start),
                    );
                    emit(
                        &sock,
                        &out,
                        &mut emitted_in_block,
                        &mut open_ms,
                        &mut par_emit,
                        t_ms,
                        base,
                    );
                }
            } else if buf[0] == 0xA1 {
                let mut units = Vec::new();
                for i in 0..cnt.min((n - 3) / 6) {
                    let b = u32::from_be_bytes(buf[3 + i * 6..7 + i * 6].try_into().unwrap());
                    let e = u16::from_be_bytes(buf[7 + i * 6..9 + i * 6].try_into().unwrap());
                    units.push(RecoveredUnit {
                        unit_id: base + u64::from(b) * K + u64::from(e),
                        block_id: b,
                        esi: e,
                    });
                }
                s.on_event(
                    Event::RecoveredControl(RecoveredInfo { units }),
                    now_ns(&start),
                );
            }
        }
        std::thread::sleep(Duration::from_millis(2));
        if app >= cfg.w_units && t_ms > cfg.op_horizon_ms {
            break;
        }
    }
    // FA: WORKLOAD-tamam → FINISH-marker (0xFE) tekrar-gönder + rx-ACK-bekle (SIGTERM-yerine-graceful).
    let fin_start = Instant::now();
    let mut fin_acked = false;
    while fin_start.elapsed() < Duration::from_secs(3) {
        let _ = sock.send(&[0xFEu8; 1]);
        if let Ok(n) = sock.recv(&mut buf) {
            if n >= 1 && buf[0] == 0xFE {
                fin_acked = true;
                // Birkaç-kez-daha-gönder (rx ≥3-FINISH-görsün) sonra-çık.
                for _ in 0..5 {
                    let _ = sock.send(&[0xFEu8; 1]);
                    std::thread::sleep(Duration::from_millis(20));
                }
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let _ = fin_acked; // best-effort; rx idle-fallback'i de temiz-finalize-eder
                       // Çıktılar.
    std::fs::create_dir_all(&cfg.out_dir).map_err(|e| e.to_string())?;
    if cfg.emit_fuzzy && !fuzzy_rows.is_empty() {
        let hdr = "run_id,policy,event_index,t_ms,loss_windowed_ppm,qd_ratio_permille,jitter_ppm,ack_confidence_permille,bandwidth_bps,btlbw,srtt_us,pacing_rate,inflight_bytes,fuzzy_ratio_permille,mode,no_rule\n";
        std::fs::write(
            format!("{}/{}-fuzzy.csv", cfg.out_dir, cfg.run_id),
            format!("{hdr}{fuzzy_rows}"),
        )
        .map_err(|e| e.to_string())?;
    }
    let d = s.diag_fec_actuation();
    let b = s.diag_budget();
    let mut lf = std::fs::File::create(format!("{}/{}-ledger.csv", cfg.out_dir, cfg.run_id))
        .map_err(|e| e.to_string())?;
    writeln!(lf, "run_id,block,open_ms,close_ms,par_emit").ok();
    for (blk, o) in &open_ms {
        writeln!(
            lf,
            "{},{blk},{o},{},{}",
            cfg.run_id,
            close_ms.get(blk).copied().unwrap_or(0),
            par_emit.get(blk).copied().unwrap_or(0)
        )
        .ok();
    }
    let mut sf = std::fs::File::create(format!("{}/{}-summary.csv", cfg.out_dir, cfg.run_id))
        .map_err(|e| e.to_string())?;
    writeln!(
        sf,
        "run_id,config_hash,policy,enq,emit,canc,waste,ndisp,rdisp,tk_adm,tk_on,tk_late,tk_miss"
    )
    .ok();
    writeln!(
        sf,
        "{},{:x},{},{},{},{},{},{},{},{},{},{},{}",
        cfg.run_id,
        cfg.config_hash(),
        cfg.policy,
        d.enqueued_r_total,
        d.emitted_r_total,
        d.canceled_r_total,
        b.reservation_waste_total,
        b.new_data_displaced,
        b.retx_displaced,
        b.tk_admitted,
        b.tk_on_time,
        b.tk_late,
        b.tk_missed_unique
    )
    .ok();
    std::fs::write(
        format!("{}/{}-config.txt", cfg.out_dir, cfg.run_id),
        format!("{}\nhash={:x}\n", cfg.canonical_dump(), cfg.config_hash()),
    )
    .ok();
    Ok(())
}

fn emit(
    sock: &UdpSocket,
    out: &[crate::sender::TransmitDecision],
    eib: &mut BTreeMap<u32, u64>,
    open_ms: &mut BTreeMap<u32, u64>,
    par_emit: &mut BTreeMap<u32, u64>,
    t_ms: u64,
    base: u64,
) {
    for d in out {
        let (kind, b, e, unit) = if d.kind == TransmitKind::FecParity {
            let h = d.send_record.expect("hint");
            let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
            *par_emit.entry(block_id).or_insert(0) += 1;
            (2u8, block_id, esi, d.unit_id)
        } else if d.unit_id >= base {
            let i = d.unit_id - base;
            let (b, e) = ((i / K) as u32, (i % K) as u16);
            if d.kind == TransmitKind::Data {
                *eib.entry(b).or_insert(0) += 1;
                open_ms.entry(b).or_insert(t_ms);
            }
            (u8::from(d.kind != TransmitKind::Data), b, e, d.unit_id)
        } else {
            (3u8, u32::MAX, u16::MAX, d.unit_id)
        };
        let _ = sock.send(&enc(kind, d.seq, unit, b, e, d.size as usize));
    }
}
