//! **1b Budget Characterization — Micro-Sweep 1** (Commit-O; KESİT-2-DEĞİLDİR).
//! Amaç: MEVCUT leftover-token-politikasının davranışını block/event-düzeyinde ÖLÇMEK — alternatif-
//! politika UYGULANMAZ; production-policy-değişikliği YOK (harness/data/report-only).
//!
//! Model-beyanı m1_reverse_nis.rs ile AYNI (paket-düzeyi deterministik-sim; kapasiteli-FIFO;
//! akışlı-app; kümülatif-SACK+RECOVERED-piggyback; MDS-sayım-decode; gerçek-Sender-motoru).
//! FARKLAR: (a) Controller-ratio-arm GERÇEK-fuzzy-yolunu-kullanır (`controller_decision_snapshot()`
//! ENJEKSİYONSUZ → otorite-bağlı-latch; ratio/snapshot-doğrudan-yazılmaz), (b) Manual-arm'lar
//! otorite-bağlı `close_fec_block_manual` (staging-esi 0..r repair-uzayı → receiver-modelinde K+esi'ye
//! haritalanır; codec `is_repair`-uzay-ayrımının sim-karşılığı), (c) PER-BLOK-LEDGER (harness-tarafı;
//! sender'a-unbounded-ledger-EKLENMEDİ), (d) token-denial-attribution-polling (Commit-N-diag),
//! (e) 1s-interval-serisi (BtlBw/offered/rate/parity — counterfactual-replay-girdisi).
//!
//! PRE-DECLARED practical-equivalence (KOŞUDAN-ÖNCE-yazıldı): iki-arm-pratikte-eşdeğer ⇔
//! |Δmedian-completion| ≤ 0.01 ∧ |Δmedian-goodput|/max(baseline,ε) ≤ 0.02 ∧ bootstrap-%95-CI'ler-örtüşür.
//!
//! LEDGER-TUTARLILIK-KİMLİĞİ (test-gate): her-blok için `enqueued == emitted + canceled + pending_end`
//! (token-denied = KATEGORİ-DEĞİL, bağlam — bekleme-durumudur; hiçbir-parity-iki-kategoriye-sayılmaz);
//! global-çapraz: Σenqueued == diag.enqueued_r_total ∧ Σemitted == diag.emitted_r_total ∧
//! Σcanceled == diag.canceled_r_total.

use adaptive_transport::sender::{
    AckInfo, ControllerDecisionSnapshot, Event, FecActuationMode, OpaqueUnit, RecoveredInfo,
    RecoveredUnit, Sender, SrLogicalRef, Timer, TransmitKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::time::Duration;

const MS: u64 = 1_000_000;
const UNIT: u64 = 1200;
const OVERHEAD: u64 = 37;
const K: u64 = 8;
const TICK_MS: u64 = 5;
const PTO_MS: u64 = 50;
const D_MS: u64 = 50;
const DATA_ID_BASE: u64 = 1_000_000;
const CAP_PPS: u64 = 50;
const APP_GAP_MS: u64 = 40;
const R_CODE_MAX: u16 = 4; // profil-0x03 gerçek-clamp'i (Controller-arm bunu-kullanır)

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pass(&mut self, p_loss: f64) -> bool {
        (self.next() % 10_000) as f64 >= p_loss * 10_000.0
    }
}

type SackInFlight = (u64, Vec<u64>, Vec<(u32, u16, u64)>);

#[derive(Clone, Copy)]
enum ChanModel {
    Iid(f64),
}

struct FwdChan {
    model: ChanModel,
    attempts: u64,
    drops: u64,
}
impl FwdChan {
    fn new(model: ChanModel) -> Self {
        Self {
            model,
            attempts: 0,
            drops: 0,
        }
    }
    fn pass(&mut self, rng: &mut Rng) -> bool {
        self.attempts += 1;
        let ChanModel::Iid(p) = self.model;
        let ok = rng.pass(p);
        if !ok {
            self.drops += 1;
        }
        ok
    }
}

/// Arm-tanımı: Manual-fixed-r (otorite-bağlı-close) / Controller (GERÇEK-fuzzy) / ARQ-baseline.
#[derive(Clone, Copy, PartialEq)]
enum ArmKind {
    Arq,
    ManualR(u16),
    Controller,
}
impl ArmKind {
    fn name(self) -> String {
        match self {
            ArmKind::Arq => "ARQ-baseline".into(),
            ArmKind::ManualR(r) => format!("Manual-r{r}"),
            ArmKind::Controller => "Controller-ratio".into(),
        }
    }
    fn fec_decode(self) -> bool {
        !matches!(self, ArmKind::Arq)
    }
}

#[derive(Clone, Copy)]
struct Scenario {
    p_fwd: f64,
    p_rev: f64,
    d_ms: u64,
    cap_pps: u64,
    app_gap_ms: u64,
    w_units: u64,
    t_max_ms: u64,
}
impl Default for Scenario {
    fn default() -> Self {
        Scenario {
            p_fwd: 0.05,
            p_rev: 0.0,
            d_ms: D_MS,
            cap_pps: CAP_PPS,
            app_gap_ms: APP_GAP_MS,
            w_units: 400,
            t_max_ms: 300_000,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Arrival {
    t_ms: u64,
    unit_id: u64,
    is_parity: bool,
    block: u32,
    esi: u16,
    seq: u64,
}

/// PER-BLOK-LEDGER (harness-tarafı; §6 talimat-zinciri).
#[derive(Debug, Default, Clone)]
struct BlockLedger {
    generation: Option<u64>,
    mode: &'static str,
    ratio_permille: u32,
    requested_r: u16,
    planned_r: u16,
    enqueued_r: u64,
    emitted_r: u64,
    canceled_r: u64,
    denials: u64,
    denial_pending_max: u64,
    open_ms: Option<u64>,  // bloğun-ilk-DATA-emisyonu
    close_ms: Option<u64>, // otorite-tüketimi (latch/manual-close) anı
    first_par_ms: Option<u64>,
    last_par_ms: Option<u64>,
}

#[derive(Debug, Default, Clone)]
struct RunOut {
    delivered_units: usize,
    total_units: usize,
    delivered_bytes: u64,
    sealed_bytes: u64,
    t_p50_ms: u64,
    t_p95_ms: u64,
    dl_hits: [usize; 3],
    retx_dup_emitted: u64,
    parity_enqueued: u64,
    parity_emitted: u64,
    parity_canceled: u64,
    denial_events: u64,
    max_queue_ms: u64,
    censored: bool,
    end_t_ms: u64,
    fwd_attempts: u64,
    fwd_drops: u64,
    fi_btlbw: [u64; 4],
    fi_loss_ppm: [u64; 4],
    fi_qd: [u64; 4],
    fi_ackconf: [u64; 4],
    pacing_rate: [u64; 4],
    first_parity_delay_ms: Vec<u64>, // blok-başına close→ilk-emisyon
    parity_span_ms: Vec<u64>,        // blok-başına ilk→son-emisyon
    ledger: BTreeMap<u32, BlockLedger>,
    interval_csv: String, // 1s-serisi (counterfactual-girdisi)
    controller_ratio_seen: Vec<u32>,
}

#[allow(clippy::too_many_lines)]
fn run_arm(sc: Scenario, arm: ArmKind, seed: u64, cell: &str) -> RunOut {
    let ser_ms: u64 = 1000 / sc.cap_pps;
    let mut fwd_chan = FwdChan::new(ChanModel::Iid(sc.p_fwd));
    let mut s = Sender::new();
    match arm {
        ArmKind::Arq => {}
        ArmKind::ManualR(_) => s
            .set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
            .unwrap(),
        ArmKind::Controller => s
            .set_fec_actuation_mode(FecActuationMode::Controller)
            .unwrap(),
    }
    let mut rng = Rng::new(seed);

    let mut seq_unit: BTreeMap<u64, u64> = BTreeMap::new();
    let mut unit_lref: BTreeMap<u64, (u32, u16)> = BTreeMap::new();
    let mut emitted_in_block: BTreeMap<u32, u64> = BTreeMap::new();
    let mut unit_arrived: BTreeSet<u64> = BTreeSet::new();
    let mut deliver_t: BTreeMap<u64, u64> = BTreeMap::new();
    let mut sealed_bytes: u64 = 0;
    let mut rx_acked: BTreeSet<u64> = BTreeSet::new();
    let mut block_have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
    let mut block_decoded: BTreeSet<u32> = BTreeSet::new();
    let mut pending_recovered: Vec<(u32, u16, u64)> = Vec::new();
    let mut tx_acked_seen: BTreeSet<u64> = BTreeSet::new();
    let mut arrivals: Vec<Arrival> = Vec::new();
    let mut sacks: Vec<SackInFlight> = Vec::new();
    let mut link_free_ms: u64 = 0;
    let mut retx_dup_emitted = 0u64;
    let mut max_queue_ms = 0u64;
    let mut ledger: BTreeMap<u32, BlockLedger> = BTreeMap::new();
    let mut last_denial_count = 0u64;
    let mut controller_ratio_seen: Vec<u32> = Vec::new();
    let mut fi_btlbw_s = Vec::new();
    let mut fi_loss_s = Vec::new();
    let mut fi_qd_s = Vec::new();
    let mut fi_ack_s = Vec::new();
    let mut rate_s = Vec::new();
    let mut interval_csv = String::new();

    for i in 0..sc.w_units {
        unit_lref.insert(DATA_ID_BASE + i, ((i / K) as u32, (i % K) as u16));
    }

    // Emisyon-işleme + DENIAL-POLLING her-on_event-sonrası (Commit-N-attribution).
    macro_rules! process {
        ($out:expr, $t_ms:expr) => {{
            let out = $out;
            for d in &out {
                sealed_bytes += d.size + OVERHEAD;
                let (is_parity, block, esi) = if d.kind == TransmitKind::FecParity {
                    let h = d.send_record.expect("parity send_record");
                    let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                    let e = ledger.entry(block_id).or_default();
                    e.emitted_r += 1;
                    if e.first_par_ms.is_none() {
                        e.first_par_ms = Some($t_ms);
                    }
                    e.last_par_ms = Some($t_ms);
                    // Repair-uzayı → receiver-modeli esi-haritası (K+esi; is_repair-ayrımının sim-karşılığı).
                    (true, block_id, K as u16 + esi)
                } else {
                    let (b, e) = *unit_lref.get(&d.unit_id).unwrap_or(&(u32::MAX, u16::MAX));
                    (false, b, e)
                };
                if !is_parity && block != u32::MAX {
                    s.register_sent_logical(
                        d.seq,
                        SrLogicalRef::Fec {
                            block_id: block,
                            esi,
                        },
                    );
                    if d.kind == TransmitKind::Data {
                        *emitted_in_block.entry(block).or_insert(0) += 1;
                        let e = ledger.entry(block).or_default();
                        if e.open_ms.is_none() {
                            e.open_ms = Some($t_ms);
                        }
                    }
                }
                if matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate) {
                    retx_dup_emitted += 1;
                }
                seq_unit.insert(d.seq, d.unit_id);
                let depart = link_free_ms.max($t_ms);
                link_free_ms = depart + ser_ms;
                max_queue_ms = max_queue_ms.max(depart.saturating_sub($t_ms));
                if fwd_chan.pass(&mut rng) {
                    arrivals.push(Arrival {
                        t_ms: depart + ser_ms + sc.d_ms,
                        unit_id: d.unit_id,
                        is_parity,
                        block,
                        esi,
                        seq: d.seq,
                    });
                }
            }
            // Commit-N denial-polling: sayaç-artışı → snapshot-bloğuna-atfet.
            let (snap, cnt) = s.diag_token_denial();
            if cnt > last_denial_count {
                if let Some(dsnap) = snap {
                    let e = ledger.entry(dsnap.block_id).or_default();
                    e.denials += cnt - last_denial_count;
                    e.denial_pending_max = e.denial_pending_max.max(dsnap.pending_parity_total);
                }
                last_denial_count = cnt;
            }
        }};
    }

    let mut app_sent: u64 = 0;
    let mut t_ms: u64 = 0;
    while t_ms < sc.t_max_ms {
        t_ms += TICK_MS;
        let now = t_ms * MS;

        if t_ms.is_multiple_of(sc.app_gap_ms) && app_sent < sc.w_units {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: DATA_ID_BASE + app_sent,
                    size: UNIT,
                }),
                now,
            );
            app_sent += 1;
            process!(out, t_ms);
        }
        if t_ms.is_multiple_of(PTO_MS) {
            let out = s.on_event(Event::TimerFired(Timer::Pto), now);
            process!(out, t_ms);
        }

        // Blok-tamam → OTORİTE-BAĞLI-close (arm'a-göre).
        if arm.fec_decode() {
            let ready: Vec<u32> = emitted_in_block
                .iter()
                .filter(|&(_, &c)| c >= K)
                .map(|(&b, _)| b)
                .collect();
            for b in ready {
                emitted_in_block.remove(&b);
                let auth = s.open_fec_block_authority();
                let gen = auth.generation();
                match arm {
                    ArmKind::ManualR(r) => {
                        let got = s.close_fec_block_manual(auth, b, r, UNIT).expect("manual");
                        let e = ledger.entry(b).or_default();
                        e.generation = Some(gen);
                        e.mode = "Manual";
                        e.requested_r = r;
                        e.planned_r = got;
                        e.enqueued_r += u64::from(got);
                        e.close_ms = Some(t_ms);
                    }
                    ArmKind::Controller => {
                        // GERÇEK-fuzzy: snapshot-ENJEKSİYONSUZ (ratio-motor-üretimi).
                        let snap: Option<ControllerDecisionSnapshot> =
                            s.controller_decision_snapshot();
                        let l = s.fec_controller_latch(auth, snap, K as u8, R_CODE_MAX, b, UNIT);
                        controller_ratio_seen.push(l.requested_ratio_permille);
                        let e = ledger.entry(b).or_default();
                        e.generation = Some(gen);
                        e.mode = "Controller";
                        e.ratio_permille = l.requested_ratio_permille;
                        e.requested_r = l.computed_r;
                        e.planned_r = l.planned_r;
                        e.enqueued_r += u64::from(l.planned_r);
                        e.close_ms = Some(t_ms);
                    }
                    ArmKind::Arq => unreachable!(),
                }
            }
        }

        // 1s-interval-serisi (counterfactual/BtlBw-offered analizi girdisi).
        if t_ms.is_multiple_of(1000) {
            let (_, btlbw, _, rate, _) = s.diag_rate();
            let d = s.diag_fec_actuation();
            let (_, denials) = s.diag_token_denial();
            interval_csv.push_str(&format!(
                "{cell},{arm_n},{seed},{t_ms},{btlbw},{rate},{sealed_bytes},{enq},{emit},{canc},{denials}\n",
                arm_n = arm.name(),
                enq = d.enqueued_r_total,
                emit = d.emitted_r_total,
                canc = d.canceled_r_total,
            ));
            if let Some(fi) = s.diag_fusion_input_snapshot() {
                fi_btlbw_s.push(fi.bandwidth_bps);
                fi_loss_s.push(fi.loss_windowed_ppm);
                fi_qd_s.push(fi.qd_ratio_permille);
                fi_ack_s.push(fi.ack_confidence_permille);
            }
            rate_s.push(rate);
        }

        // Arrivals → receiver-modeli.
        let (due, rest): (Vec<Arrival>, Vec<Arrival>) =
            arrivals.drain(..).partition(|a| a.t_ms <= t_ms);
        arrivals = rest;
        for a in due {
            rx_acked.insert(a.seq);
            if a.block != u32::MAX {
                block_have.entry(a.block).or_default().insert(a.esi);
            }
            if !a.is_parity && a.unit_id >= DATA_ID_BASE {
                unit_arrived.insert(a.unit_id);
                deliver_t.entry(a.unit_id).or_insert(t_ms);
            }
            if arm.fec_decode() && a.block != u32::MAX && !block_decoded.contains(&a.block) {
                let have = &block_have[&a.block];
                let missing: Vec<u16> = (0..K as u16).filter(|e| !have.contains(e)).collect();
                if have.len() as u64 >= K && !missing.is_empty() {
                    block_decoded.insert(a.block);
                    for e in missing {
                        let unit = DATA_ID_BASE + u64::from(a.block) * K + u64::from(e);
                        if unit < DATA_ID_BASE + sc.w_units {
                            deliver_t.entry(unit).or_insert(t_ms);
                            pending_recovered.push((a.block, e, unit));
                        }
                    }
                }
            }
        }

        // SACK-kadansı + RECOVERED-piggyback.
        if t_ms.is_multiple_of(50)
            && (!rx_acked.is_empty() || !pending_recovered.is_empty())
            && rng.pass(sc.p_rev)
        {
            let snapshot: Vec<u64> = rx_acked.iter().copied().collect();
            sacks.push((t_ms + sc.d_ms, snapshot, pending_recovered.clone()));
        }
        let (due_s, rest_s): (Vec<_>, Vec<_>) = sacks.drain(..).partition(|x| x.0 <= t_ms);
        sacks = rest_s;
        for (_, snapshot, rec_batch) in due_s {
            let new_acked: Vec<u64> = snapshot
                .iter()
                .copied()
                .filter(|sq| !tx_acked_seen.contains(sq))
                .collect();
            for sq in &new_acked {
                tx_acked_seen.insert(*sq);
            }
            if !new_acked.is_empty() {
                let out = s.on_event(
                    Event::AckReceived(AckInfo {
                        acked_seqs: new_acked,
                        ack_delay: Duration::ZERO,
                    }),
                    now,
                );
                process!(out, t_ms);
            }
            if arm.fec_decode() && !rec_batch.is_empty() {
                let canc_before = s.diag_fec_actuation().canceled_r_total;
                let units: Vec<RecoveredUnit> = rec_batch
                    .iter()
                    .map(|&(b, e, unit)| RecoveredUnit {
                        unit_id: unit,
                        block_id: b,
                        esi: e,
                    })
                    .collect();
                let blocks_in_batch: BTreeSet<u32> = rec_batch.iter().map(|x| x.0).collect();
                s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
                let canc_delta = s.diag_fec_actuation().canceled_r_total - canc_before;
                if canc_delta > 0 {
                    // Blok-bazlı-cancel-atıfı: batch'teki-bloklara enqueued−emitted-payıyla dağıt
                    // (cancel = o-bloğun-kuyrukta-kalan-TÜM-item'ları; kimlik-tutarlılığı-test-gate'te).
                    let mut remaining = canc_delta;
                    for b in blocks_in_batch {
                        let e = ledger.entry(b).or_default();
                        let pend = e.enqueued_r - e.emitted_r - e.canceled_r;
                        let take = pend.min(remaining);
                        e.canceled_r += take;
                        remaining -= take;
                        if remaining == 0 {
                            break;
                        }
                    }
                    assert_eq!(remaining, 0, "cancel-atıfı-tam (çift-sayım-yok)");
                }
            }
        }

        if deliver_t.len() as u64 >= sc.w_units {
            break;
        }
    }

    let mut times: Vec<u64> = deliver_t.values().copied().collect();
    times.sort_unstable();
    let pct = |v: &Vec<u64>, p: f64| -> u64 {
        if v.is_empty() {
            u64::MAX
        } else {
            v[((v.len() as f64 - 1.0) * p) as usize]
        }
    };
    let d = s.diag_fec_actuation();
    let (_, denial_events) = s.diag_token_denial();
    // LEDGER-TUTARLILIK (kimlik): blok-düzeyi + global-çapraz (test-gate'in-özü).
    let mut sum_enq = 0u64;
    let mut sum_emit = 0u64;
    let mut sum_canc = 0u64;
    let mut first_delay = Vec::new();
    let mut spans = Vec::new();
    for (b, e) in &ledger {
        assert!(
            e.enqueued_r >= e.emitted_r + e.canceled_r,
            "blok-{b}: kategori-taşması"
        );
        sum_enq += e.enqueued_r;
        sum_emit += e.emitted_r;
        sum_canc += e.canceled_r;
        if let (Some(c), Some(f)) = (e.close_ms, e.first_par_ms) {
            first_delay.push(f.saturating_sub(c));
        }
        if let (Some(f), Some(l)) = (e.first_par_ms, e.last_par_ms) {
            spans.push(l - f);
        }
    }
    assert_eq!(sum_enq, d.enqueued_r_total, "Σenqueued==diag");
    assert_eq!(sum_emit, d.emitted_r_total, "Σemitted==diag");
    assert_eq!(sum_canc, d.canceled_r_total, "Σcanceled==diag");
    let quart = |v: &mut Vec<u64>| -> [u64; 4] {
        if v.is_empty() {
            return [0; 4];
        }
        v.sort_unstable();
        let n = v.len();
        [v[0], v[n / 2], v[(n * 9 / 10).min(n - 1)], v[n - 1]]
    };
    let dl_hits =
        [2_000u64, 5_000, 10_000].map(|dl| deliver_t.values().filter(|&&t| t <= dl).count());
    RunOut {
        delivered_units: deliver_t.len(),
        total_units: sc.w_units as usize,
        delivered_bytes: deliver_t.len() as u64 * UNIT,
        sealed_bytes,
        t_p50_ms: pct(&times, 0.50),
        t_p95_ms: pct(&times, 0.95),
        dl_hits,
        retx_dup_emitted,
        parity_enqueued: d.enqueued_r_total,
        parity_emitted: d.emitted_r_total,
        parity_canceled: d.canceled_r_total,
        denial_events,
        max_queue_ms,
        censored: (deliver_t.len() as u64) < sc.w_units,
        end_t_ms: t_ms,
        fwd_attempts: fwd_chan.attempts,
        fwd_drops: fwd_chan.drops,
        fi_btlbw: quart(&mut fi_btlbw_s),
        fi_loss_ppm: quart(&mut fi_loss_s),
        fi_qd: quart(&mut fi_qd_s),
        fi_ackconf: quart(&mut fi_ack_s),
        pacing_rate: quart(&mut rate_s),
        first_parity_delay_ms: first_delay,
        parity_span_ms: spans,
        ledger,
        interval_csv,
        controller_ratio_seen,
    }
}

fn cells() -> Vec<(&'static str, Scenario)> {
    let base = Scenario::default();
    vec![
        ("nominal", base),
        // gap80 (offered≈C/4) — kesit-1 bimodal-hücresi (30-seed).
        (
            "gap80-bimodal",
            Scenario {
                app_gap_ms: 80,
                ..base
            },
        ),
        (
            "rtt-d1000",
            Scenario {
                d_ms: 1000,
                t_max_ms: 600_000,
                ..base
            },
        ),
        (
            "rtt-d2500",
            Scenario {
                d_ms: 2500,
                t_max_ms: 600_000,
                ..base
            },
        ),
        (
            "loss20",
            Scenario {
                p_fwd: 0.20,
                ..base
            },
        ),
        (
            "rev99",
            Scenario {
                p_rev: 0.99,
                ..base
            },
        ),
        // düşük-bw sansür-sınırı (kesit-1 bw12: 3/3-censored-sınıfı).
        (
            "bw12-censored",
            Scenario {
                cap_pps: 12,
                app_gap_ms: 166,
                t_max_ms: 600_000,
                ..base
            },
        ),
        // offered-taraması: derin-underutilized / ≈C / kontrollü-overload (C=50pps=60kB/s).
        (
            "offered-c8",
            Scenario {
                app_gap_ms: 160,
                ..base
            },
        ),
        (
            "offered-c1",
            Scenario {
                app_gap_ms: 20,
                ..base
            },
        ),
        (
            "offered-1p5c",
            Scenario {
                app_gap_ms: 13,
                t_max_ms: 600_000,
                ..base
            },
        ),
    ]
}

fn arms() -> Vec<ArmKind> {
    vec![
        ArmKind::Arq,
        ArmKind::ManualR(2),
        ArmKind::ManualR(4),
        ArmKind::ManualR(8),
        ArmKind::Controller,
    ]
}

const SEEDS10: [u64; 10] = [11, 23, 37, 41, 53, 67, 79, 83, 97, 101];
const SEEDS30_EXTRA: [u64; 20] = [
    103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193, 197,
    199,
];

/// TEST-GATE (hızlı; her-CI): ledger-tutarlılık-kimliği + cancel-çift-sayım-yok + deterministik-rerun.
#[test]
fn ms1_ledger_consistency_smoke() {
    let sc = Scenario {
        w_units: 80,
        t_max_ms: 60_000,
        ..Scenario::default()
    };
    let a = run_arm(sc, ArmKind::ManualR(4), 11, "smoke");
    // Kimlik run_arm-içinde-assert'li (Σ==diag); burada-ek: deterministik-rerun bit-özdeş.
    let b = run_arm(sc, ArmKind::ManualR(4), 11, "smoke");
    assert_eq!(a.delivered_units, b.delivered_units, "deterministik-rerun");
    assert_eq!(a.sealed_bytes, b.sealed_bytes);
    assert_eq!(a.parity_emitted, b.parity_emitted);
    assert_eq!(a.denial_events, b.denial_events);
    // Controller-arm-smoke: gerçek-fuzzy-yol-çalışır (ratio-değerleri-motor-üretimi; panik-yok).
    let c = run_arm(sc, ArmKind::Controller, 11, "smoke");
    assert!(
        !c.controller_ratio_seen.is_empty()
            || c.ledger.is_empty()
            || c.censored
            || c.delivered_units > 0
    );
}

/// MICRO-SWEEP-1 (ignored; açık-komutla): MS1_OUT env-dizinine seed-level + ledger + interval CSV yazar.
#[test]
#[ignore]
fn micro_sweep_1b() {
    let out_dir = std::env::var("MS1_OUT").expect("MS1_OUT dizini (çıktı-CSV'leri)");
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut seed_f = std::fs::File::create(format!("{out_dir}/ms1-seed-level.csv")).unwrap();
    let mut ledg_f = std::fs::File::create(format!("{out_dir}/ms1-block-ledger.csv")).unwrap();
    let mut intv_f = std::fs::File::create(format!("{out_dir}/ms1-interval.csv")).unwrap();
    writeln!(seed_f, "cell,arm,seed,completion,goodput,t_p50,t_p95,dl2,dl5,dl10,retx_dup,par_enq,par_emit,par_canc,denials,realization,max_q_ms,censored,end_t_ms,fwd_att,fwd_drop,btlbw_min,btlbw_p50,btlbw_p90,btlbw_max,loss_ppm_p50,qd_p50,ackconf_p50,rate_p50,rate_max,first_par_delay_p50,par_span_p50,ratio_p50,offered_Bps,cap_Bps").unwrap();
    writeln!(ledg_f, "cell,arm,seed,block,gen,mode,ratio,requested_r,planned_r,enqueued,emitted,canceled,pending_end,denials,denial_pend_max,open_ms,close_ms,first_par_ms,last_par_ms").unwrap();
    writeln!(
        intv_f,
        "cell,arm,seed,t_ms,btlbw,rate,sealed_bytes,enq,emit,canc,denials"
    )
    .unwrap();
    for (cell, sc) in cells() {
        let seeds: Vec<u64> = if cell == "gap80-bimodal" {
            SEEDS10
                .iter()
                .chain(SEEDS30_EXTRA.iter())
                .copied()
                .collect()
        } else {
            SEEDS10.to_vec()
        };
        for arm in arms() {
            for &seed in &seeds {
                let m = run_arm(sc, arm, seed, cell);
                let tot = m.total_units as f64;
                let offered = UNIT * 1000 / sc.app_gap_ms;
                let median_u = |v: &[u64]| -> u64 {
                    if v.is_empty() {
                        0
                    } else {
                        let mut w = v.to_vec();
                        w.sort_unstable();
                        w[w.len() / 2]
                    }
                };
                let ratio_p50 = median_u(
                    &m.controller_ratio_seen
                        .iter()
                        .map(|&x| u64::from(x))
                        .collect::<Vec<_>>(),
                );
                writeln!(seed_f,
                    "{cell},{},{seed},{:.4},{:.4},{},{},{:.4},{:.4},{:.4},{},{},{},{},{},{:.4},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                    arm.name(),
                    m.delivered_units as f64 / tot,
                    m.delivered_bytes as f64 / m.sealed_bytes.max(1) as f64,
                    m.t_p50_ms, m.t_p95_ms,
                    m.dl_hits[0] as f64 / tot, m.dl_hits[1] as f64 / tot, m.dl_hits[2] as f64 / tot,
                    m.retx_dup_emitted, m.parity_enqueued, m.parity_emitted, m.parity_canceled,
                    m.denial_events,
                    m.parity_emitted as f64 / m.parity_enqueued.max(1) as f64,
                    m.max_queue_ms, u8::from(m.censored), m.end_t_ms,
                    m.fwd_attempts, m.fwd_drops,
                    m.fi_btlbw[0], m.fi_btlbw[1], m.fi_btlbw[2], m.fi_btlbw[3],
                    m.fi_loss_ppm[1], m.fi_qd[1], m.fi_ackconf[1],
                    m.pacing_rate[1], m.pacing_rate[3],
                    median_u(&m.first_parity_delay_ms), median_u(&m.parity_span_ms),
                    ratio_p50, offered, sc.cap_pps * UNIT
                ).unwrap();
                for (b, e) in &m.ledger {
                    let pend = e.enqueued_r - e.emitted_r - e.canceled_r;
                    writeln!(
                        ledg_f,
                        "{cell},{},{seed},{b},{},{},{},{},{},{},{},{},{pend},{},{},{},{},{},{}",
                        arm.name(),
                        e.generation.map_or(-1i64, |g| g as i64),
                        e.mode,
                        e.ratio_permille,
                        e.requested_r,
                        e.planned_r,
                        e.enqueued_r,
                        e.emitted_r,
                        e.canceled_r,
                        e.denials,
                        e.denial_pending_max,
                        e.open_ms.map_or(-1, |x| x as i64),
                        e.close_ms.map_or(-1, |x| x as i64),
                        e.first_par_ms.map_or(-1, |x| x as i64),
                        e.last_par_ms.map_or(-1, |x| x as i64),
                    )
                    .unwrap();
                }
                intv_f.write_all(m.interval_csv.as_bytes()).unwrap();
            }
        }
        eprintln!("[ms1] hücre-tamam: {cell}");
    }
}
