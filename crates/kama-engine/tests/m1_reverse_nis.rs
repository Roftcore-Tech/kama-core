//! M1 — REVERSE-NİŞ ÖLÇÜMÜ (bilimsel-deney; white-paper belkemiği). Production-graph-DIŞI harness.
//!
//! KARAR-1: bütçe-eşitliği = OVERHEAD-NORMALİZE — iki-arm SERBEST koşar; adalet metriğe-gömülü:
//!   goodput_per_sealed_byte = delivered_app_bytes / total_sealed_wire_bytes (parity VE retransmit paydada).
//! KARAR-2: her-hücre evicted==0 guard (aşımda hücre-DIŞLA+raporla); ack_miss SALT-TEŞHİS (işaretleme-dışı).
//! KARAR-3: iki-arm AYNI RTO/Pto/SACK-kadansı (tek-motor, tek-fark fec_enabled) — suppress FEC'ten, timer-farkından değil.
//!
//! Model-beyanı (raporda da): paket-düzeyi deterministik-sim (seed'li xorshift, tick=5ms, tek-yön 50ms);
//! forward-link KAPASİTELİ-FIFO (C pkt/s; serialization+kuyruk-gecikmesi — BtlBw gerçek-C'yi-ölçer,
//! zaman-maliyeti anlamlı); app-yükü AKIŞLI (arrival-aralıklı — gerçek-kullanım; bulk-modeli parity'yi
//! DATA-ÖNCE-sıralaması-gereği koşturamaz, ölçüm-aracı-doğrulamasında tespit-edildi). SACK kümülatif
//! (gerçek-SACK-semantiği), RECOVERED SACK'e piggyback (aynı-datagram → aynı-reverse-drop kararı);
//! RS-decode MDS-sayım-kuralı (ulaşan≥K → eksik-source recovered; codec-doğruluğu admission_5d2/pipeline
//! testlerinde ayrıca-kanıtlı). Gerçek Sender-motoru; iki-arm tek-fark fec_enabled.

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, SrLogicalRef, Timer,
    TransmitKind,
};
use kama_engine::RecoveredMap;
use kama_transport_schema::control::Control;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const MS: u64 = 1_000_000; // ns
const UNIT: u64 = 1200;
const OVERHEAD: u64 = 37; // 26B EXTENDED-outer + 11B FEC-inner-hdr (sealed-wire-byte payı)
const K: u64 = 8; // blok source-sayısı (profil-3 oranı)
const R: u64 = 4; // blok parity-sayısı
const TICK_MS: u64 = 5;
const PTO_MS: u64 = 50; // KARAR-3: iki-arm AYNI
const SACK_MS: u64 = 50; // KARAR-3: iki-arm AYNI
const D_MS: u64 = 50; // tek-yön gecikme
const DATA_ID_BASE: u64 = 1_000_000; // parity-unit_id(=esi) çakışması-yok
const CAP_PPS: u64 = 50; // forward-link kapasitesi (pkt/s) → serialization 20ms/paket
const SER_MS: u64 = 1000 / CAP_PPS; // 20ms
                                    // Akışlı-yük: offered = C/2 = 25 pkt/s — FEC-overhead (R/K=%50) + retransmit/dup-headroom.
                                    // Araç-doğrulama-bulgusu: offered≈C→parity tam-starving; offered=2C/3→retx/dup'la-yarışta yine-starving
                                    // (DATA-ÖNCE mühürlü-sıralamada parity token-ARTIĞINA akar — gerçek-motor-dinamiği; rapora-geçer).
const APP_GAP_MS: u64 = 40;

/// Deterministik xorshift64 (kanal-kararları; HashSet-order-bağımsız).
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
    /// true = kanal-GEÇTİ (kayıp-olasılığı p).
    fn pass(&mut self, p_loss: f64) -> bool {
        (self.next() % 10_000) as f64 >= p_loss * 10_000.0
    }
}

/// Uçuştaki SACK: (varış-t, kümülatif-ack-snapshot, recovered-batch).
type SackInFlight = (u64, Vec<u64>, Vec<(u32, u16, u64)>);

#[derive(Debug, Clone, Copy)]
struct Arrival {
    t_ms: u64,
    unit_id: u64,
    is_parity: bool,
    block: u32,
    esi: u16,
    seq: u64,
}

#[derive(Debug, Default, Clone)]
struct Metrics {
    delivered_units: usize,
    total_units: usize,
    delivered_bytes: u64,
    sealed_bytes: u64,
    t_p50_ms: u64,
    t_p95_ms: u64,
    recovered_cancels: u64,
    suppress_num: u64, // gerçekleşen-cancel toplamı
    suppress_den: u64, // potansiyel-cancel toplamı (dış-tanım)
    evicted: u64,
    ack_miss: u64,
    // Araç-doğrulama teşhisleri (parity-yolu-canlı-mı):
    parity_emitted: u64,
    blocks_decoded: u64,
    rec_batches_arrived: u64,
}

/// Tek hücre-arm koşusu. `fec_on` tek-fark (KARAR-3).
#[allow(clippy::too_many_lines)]
fn run_arm(
    fec_on: bool,
    p_fwd: f64,
    p_rev: f64,
    seed: u64,
    w_units: u64,
    t_max_ms: u64,
) -> Metrics {
    let mut s = Sender::new();
    s.set_fec_enabled(fec_on);
    let mut rng = Rng::new(seed);
    let mut rmap = RecoveredMap::new();

    // App: bulk-transfer (hepsi t=0'da kuyruğa; pacing motor-işi).
    // Harness-muhasebe:
    let mut seq_unit: BTreeMap<u64, u64> = BTreeMap::new(); // seq → unit_id
    let mut unit_lref: BTreeMap<u64, (u32, u16)> = BTreeMap::new(); // DATA unit → (block, esi)
    let mut emitted_in_block: BTreeMap<u32, u64> = BTreeMap::new(); // parity-enqueue tetikleyici
    let mut unit_arrived: BTreeSet<u64> = BTreeSet::new(); // forward-ulaşan DATA-unit
    let mut unit_retx_emitted: BTreeSet<u64> = BTreeSet::new(); // Retransmit-decision-görülen unit
    let mut deliver_t: BTreeMap<u64, u64> = BTreeMap::new(); // unit → teslim-anı-ms (arrival|decode)
    let mut sealed_bytes: u64 = 0;

    // Receiver-modeli:
    let mut rx_acked: BTreeSet<u64> = BTreeSet::new(); // kümülatif-acked seq'ler
    let mut block_have: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new(); // blok → distinct-esi (src+parity)
    let mut block_decoded: BTreeSet<u32> = BTreeSet::new();
    let mut pending_recovered: Vec<(u32, u16, u64)> = Vec::new(); // decode-anı kurtarılan (block, esi, unit)

    // Sender-tarafı görülen-acked (yeni-hesabı için):
    let mut tx_acked_seen: BTreeSet<u64> = BTreeSet::new();

    // Uçuştaki olaylar:
    let mut arrivals: Vec<Arrival> = Vec::new();
    let mut sacks: Vec<SackInFlight> = Vec::new();

    let mut cancels_before: u64;
    let mut suppress_num = 0u64;
    let mut suppress_den = 0u64;
    let mut link_free_ms: u64 = 0; // forward-FIFO (kapasiteli-link)
    let mut parity_emitted = 0u64;
    let mut rec_batches_arrived = 0u64;

    // DATA registrasyonu (DR-S5-6: tüm-emisyon-register'lı) + blok/esi ataması.
    for i in 0..w_units {
        let (b, e) = ((i / K) as u32, (i % K) as u16);
        unit_lref.insert(DATA_ID_BASE + i, (b, e));
        rmap.register(b, e, DATA_ID_BASE + i);
    }

    let process_decisions = |s: &mut Sender,
                             out: &[adaptive_transport::sender::TransmitDecision],
                             t_ms: u64,
                             rng: &mut Rng,
                             seq_unit: &mut BTreeMap<u64, u64>,
                             emitted_in_block: &mut BTreeMap<u32, u64>,
                             unit_retx_emitted: &mut BTreeSet<u64>,
                             arrivals: &mut Vec<Arrival>,
                             sealed_bytes: &mut u64,
                             unit_lref: &BTreeMap<u64, (u32, u16)>,
                             link_free_ms: &mut u64,
                             parity_emitted: &mut u64| {
        for d in out {
            *sealed_bytes += d.size + OVERHEAD;
            let (is_parity, block, esi) = if d.kind == TransmitKind::FecParity {
                let h = d.send_record.expect("parity send_record");
                let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                *parity_emitted += 1;
                (true, block_id, esi)
            } else {
                let (b, e) = *unit_lref.get(&d.unit_id).unwrap_or(&(u32::MAX, u16::MAX));
                (false, b, e)
            };
            if !is_parity && block != u32::MAX {
                // DR-S5-6: DATA-source engine-post-emit register (M1: tüm-emisyon-register'lı).
                s.register_sent_logical(
                    d.seq,
                    SrLogicalRef::Fec {
                        block_id: block,
                        esi,
                    },
                );
                if d.kind == TransmitKind::Data {
                    let c = emitted_in_block.entry(block).or_insert(0);
                    *c += 1;
                }
                if d.kind == TransmitKind::Retransmit {
                    unit_retx_emitted.insert(d.unit_id);
                }
            }
            seq_unit.insert(d.seq, d.unit_id);
            // Forward-link: KAPASİTELİ-FIFO (serialization+kuyruk) → sonra loss-zar.
            let depart = (*link_free_ms).max(t_ms);
            *link_free_ms = depart + SER_MS;
            if rng.pass(p_fwd) {
                arrivals.push(Arrival {
                    t_ms: depart + SER_MS + D_MS,
                    unit_id: d.unit_id,
                    is_parity,
                    block,
                    esi,
                    seq: d.seq,
                });
            }
        }
    };

    let mut app_sent: u64 = 0;
    let mut t_ms: u64 = 0;
    while t_ms < t_max_ms {
        t_ms += TICK_MS;
        let now = t_ms * MS;

        // 0) Akışlı-app: her APP_GAP_MS bir-unit (W'ye kadar) — gerçek-kullanım-modeli.
        if t_ms.is_multiple_of(APP_GAP_MS) && app_sent < w_units {
            let out = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: DATA_ID_BASE + app_sent,
                    size: UNIT,
                }),
                now,
            );
            app_sent += 1;
            process_decisions(
                &mut s,
                &out,
                t_ms,
                &mut rng,
                &mut seq_unit,
                &mut emitted_in_block,
                &mut unit_retx_emitted,
                &mut arrivals,
                &mut sealed_bytes,
                &unit_lref,
                &mut link_free_ms,
                &mut parity_emitted,
            );
        }

        // 1) Pto-tick (KARAR-3: iki-arm aynı-kadans).
        if t_ms.is_multiple_of(PTO_MS) {
            let out = s.on_event(Event::TimerFired(Timer::Pto), now);
            process_decisions(
                &mut s,
                &out,
                t_ms,
                &mut rng,
                &mut seq_unit,
                &mut emitted_in_block,
                &mut unit_retx_emitted,
                &mut arrivals,
                &mut sealed_bytes,
                &unit_lref,
                &mut link_free_ms,
                &mut parity_emitted,
            );
        }

        // 1b) Blok-tamamlanınca parity-enqueue (yalnız FEC-arm; engine-encode-anı-modeli).
        if fec_on {
            let ready: Vec<u32> = emitted_in_block
                .iter()
                .filter(|&(_, &c)| c >= K)
                .map(|(&b, _)| b)
                .collect();
            for b in ready {
                emitted_in_block.remove(&b);
                for r in 0..R {
                    s.enqueue_fec_parity(b, (K + r) as u16, UNIT);
                }
            }
        }

        // 2) Forward-arrival'lar → receiver-modeli.
        let (due, rest): (Vec<Arrival>, Vec<Arrival>) =
            arrivals.drain(..).partition(|a| a.t_ms <= t_ms);
        arrivals = rest;
        for a in due {
            rx_acked.insert(a.seq);
            if a.block != u32::MAX {
                let set = block_have.entry(a.block).or_default();
                set.insert(a.esi);
            }
            if !a.is_parity && a.unit_id >= DATA_ID_BASE {
                unit_arrived.insert(a.unit_id);
                deliver_t.entry(a.unit_id).or_insert(t_ms);
            }
            // FEC-decode (MDS-sayım): blokta distinct ≥ K ∧ EKSİK-source-var → recovery.
            // (Eksiksiz-blok "decode" SAYILMAZ — recovery-fırsatı-yok; araç-doğrulama-düzeltmesi.)
            if fec_on && a.block != u32::MAX && !block_decoded.contains(&a.block) {
                let have = &block_have[&a.block];
                let missing: Vec<u16> = (0..K as u16).filter(|e| !have.contains(e)).collect();
                if have.len() as u64 >= K && !missing.is_empty() {
                    block_decoded.insert(a.block); // yalnız GERÇEK-recovery'li blok sayılır
                    for e in missing {
                        let unit = DATA_ID_BASE + u64::from(a.block) * K + u64::from(e);
                        if unit < DATA_ID_BASE + w_units {
                            deliver_t.entry(unit).or_insert(t_ms); // logical-teslim = decode-anı
                            pending_recovered.push((a.block, e, unit));
                        }
                    }
                }
            }
        }

        // 3) SACK-kadansı (kümülatif; RECOVERED piggyback — aynı-datagram, aynı-drop).
        if t_ms.is_multiple_of(SACK_MS)
            && (!rx_acked.is_empty() || !pending_recovered.is_empty())
            && rng.pass(p_rev)
        {
            {
                let snapshot: Vec<u64> = rx_acked.iter().copied().collect();
                let rec_batch: Vec<(u32, u16, u64)> = pending_recovered.clone();
                sacks.push((t_ms + D_MS, snapshot, rec_batch));
            }
            // pending_recovered receiver-tarafında tutulur; ulaşana-kadar sonraki-SACK'lerde tekrar
            // (kümülatif-bildirim; ulaşınca sender-idempotent — cancel ikinci-kez no-op).
        }

        // 4) Sender'a ulaşan SACK+RECOVERED.
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
                process_decisions(
                    &mut s,
                    &out,
                    t_ms,
                    &mut rng,
                    &mut seq_unit,
                    &mut emitted_in_block,
                    &mut unit_retx_emitted,
                    &mut arrivals,
                    &mut sealed_bytes,
                    &unit_lref,
                    &mut link_free_ms,
                    &mut parity_emitted,
                );
            }
            if fec_on && !rec_batch.is_empty() {
                rec_batches_arrived += 1;
                // Suppress-payda (dış-tanım): kayıp ∧ ¬retx-emit ∧ ¬acked @RECOVERED-ulaşma-anı.
                let mut units: Vec<RecoveredUnit> = Vec::new();
                for &(b, e, unit) in &rec_batch {
                    if !unit_arrived.contains(&unit)
                        && !unit_retx_emitted.contains(&unit)
                        && !seq_unit
                            .iter()
                            .any(|(sq, u)| *u == unit && tx_acked_seen.contains(sq))
                    {
                        suppress_den += 1;
                    }
                    units.push(RecoveredUnit {
                        unit_id: unit,
                        block_id: b,
                        esi: e,
                    });
                }
                // Gerçek-yol: Control::Recovered → RecoveredMap.resolve → sender (n8_v2-zinciri).
                let ranges: Vec<(u16, u16)> = rec_batch.iter().map(|&(_, e, _)| (e, e)).collect();
                let _ = rmap.resolve(&Control::Recovered {
                    id: 1,
                    block_id: rec_batch[0].0,
                    esi_ranges: ranges,
                }); // (çok-blok-batch'te harness doğrudan-units kullanır; resolve tek-blok-örnek)
                cancels_before = s.diag_recovered_cancels();
                s.on_event(Event::RecoveredControl(RecoveredInfo { units }), now);
                suppress_num += s.diag_recovered_cancels() - cancels_before;
            }
        }

        // erken-çıkış: hepsi teslim.
        if deliver_t.len() as u64 >= w_units {
            break;
        }
    }

    // Metrikler.
    let mut times: Vec<u64> = deliver_t.values().copied().collect();
    times.sort_unstable();
    let pct = |v: &Vec<u64>, p: f64| -> u64 {
        if v.is_empty() {
            u64::MAX
        } else {
            v[((v.len() as f64 - 1.0) * p) as usize]
        }
    };
    let (_, evicted, ack_miss) = s.diag_sent_logical();
    Metrics {
        delivered_units: deliver_t.len(),
        total_units: w_units as usize,
        delivered_bytes: deliver_t.len() as u64 * UNIT,
        sealed_bytes,
        t_p50_ms: pct(&times, 0.50),
        t_p95_ms: pct(&times, 0.95),
        recovered_cancels: s.diag_recovered_cancels(),
        suppress_num,
        suppress_den,
        evicted,
        ack_miss,
        parity_emitted,
        blocks_decoded: block_decoded.len() as u64,
        rec_batches_arrived,
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp); // NaN-güvenli (ARQ-arm suppress=NaN → sona-sıralanır)
    v[v.len() / 2]
}

/// Ağır-matris: 12-hücre × 2-arm × 5-seed. `--ignored --nocapture` ile koşulur; tablo stdout'a.
#[test]
#[ignore]
fn m1_matrix() {
    const W: u64 = 400;
    const T_MAX: u64 = 300_000; // 300s-sim
    const SEEDS: [u64; 5] = [11, 23, 37, 53, 71];
    let revs = [0.50, 0.70, 0.90, 0.95];
    let fwds = [0.01, 0.05, 0.15];

    println!("rev%,fwd%,arm,goodput_per_byte(med),completion(med),t_p50_ms(med),t_p95_ms(med),cancels(med),suppress(med),evicted(max),ack_miss(med),sealed_MB(med),parity(med),decoded(med),recv_batch(med)");
    for &pr in &revs {
        for &pf in &fwds {
            for arm in [true, false] {
                let mut gp = vec![];
                let mut comp = vec![];
                let mut p50 = vec![];
                let mut p95 = vec![];
                let mut canc = vec![];
                let mut supp = vec![];
                let mut evmax = 0u64;
                let mut miss = vec![];
                let mut sealed = vec![];
                let mut par = vec![];
                let mut dec = vec![];
                let mut rcv = vec![];
                for &seed in &SEEDS {
                    let m = run_arm(arm, pf, pr, seed, W, T_MAX);
                    gp.push(m.delivered_bytes as f64 / m.sealed_bytes.max(1) as f64);
                    comp.push(m.delivered_units as f64 / m.total_units as f64);
                    p50.push(m.t_p50_ms as f64);
                    p95.push(m.t_p95_ms as f64);
                    canc.push(m.recovered_cancels as f64);
                    supp.push(if m.suppress_den > 0 {
                        m.suppress_num as f64 / m.suppress_den as f64
                    } else {
                        f64::NAN
                    });
                    evmax = evmax.max(m.evicted);
                    miss.push(m.ack_miss as f64);
                    sealed.push(m.sealed_bytes as f64 / 1e6);
                    par.push(m.parity_emitted as f64);
                    dec.push(m.blocks_decoded as f64);
                    rcv.push(m.rec_batches_arrived as f64);
                }
                println!(
                    "{:.0},{:.0},{},{:.4},{:.3},{:.0},{:.0},{:.0},{:.2},{},{:.0},{:.2},{:.0},{:.0},{:.0}",
                    pr * 100.0,
                    pf * 100.0,
                    if arm { "FEC" } else { "ARQ" },
                    median(gp),
                    median(comp),
                    median(p50),
                    median(p95),
                    median(canc),
                    median(supp.clone()),
                    evmax,
                    median(miss),
                    median(sealed),
                    median(par),
                    median(dec),
                    median(rcv)
                );
                assert_eq!(
                    evmax, 0,
                    "KARAR-2: evicted>0 → hücre-DIŞLA + CAP-artır + DUR-raporla"
                );
            }
        }
    }
}

/// Smoke (normal-CI): tek-mini-hücre iki-arm — harness panik-yok + teslim>0 + evicted==0.
#[test]
fn m1_smoke_single_cell() {
    for arm in [true, false] {
        let m = run_arm(arm, 0.05, 0.70, 7, 64, 60_000);
        assert!(m.delivered_units > 0, "teslim-oldu (arm={arm})");
        assert_eq!(m.evicted, 0, "KARAR-2 guard");
        assert!(m.sealed_bytes > 0);
    }
}
