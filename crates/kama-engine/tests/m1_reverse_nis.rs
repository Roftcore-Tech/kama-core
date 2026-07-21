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

/// Forward-kanal-modeli (M5): IID veya Gilbert-Elliott iki-durumlu burst.
#[derive(Clone, Copy)]
enum ChanModel {
    Iid(f64),
    /// GE: good/bad kayıp-olasılıkları + ortalama-durum-süreleri (geometrik-geçiş).
    Ge {
        p_good: f64,
        p_bad: f64,
        mean_good_ms: u64,
        mean_bad_ms: u64,
    },
}

/// Forward-kanal-durumu (GE-state; IID'de durumsuz — M1-determinizm korunur: IID tek-rng-çağrısı).
struct FwdChan {
    model: ChanModel,
    ge_bad: bool,
    last_t_ms: u64,
    // Validity-audit: realized-loss + burst-istatistikleri (pattern-ekseni ayrıştırması).
    attempts: u64,
    drops: u64,
    cur_run: u64, // ardışık-drop-koşusu
    burst_count: u64,
    burst_sum: u64,
    burst_max: u64,
}

impl FwdChan {
    fn new(model: ChanModel) -> Self {
        Self {
            model,
            ge_bad: false,
            last_t_ms: 0,
            attempts: 0,
            drops: 0,
            cur_run: 0,
            burst_count: 0,
            burst_sum: 0,
            burst_max: 0,
        }
    }
    fn note(&mut self, passed: bool) -> bool {
        self.attempts += 1;
        if passed {
            if self.cur_run > 0 {
                self.burst_count += 1;
                self.burst_sum += self.cur_run;
                self.burst_max = self.burst_max.max(self.cur_run);
                self.cur_run = 0;
            }
        } else {
            self.drops += 1;
            self.cur_run += 1;
        }
        passed
    }
    /// true = paket-GEÇTİ.
    fn pass(&mut self, rng: &mut Rng, t_ms: u64) -> bool {
        let passed = match self.model {
            ChanModel::Iid(p) => rng.pass(p),
            ChanModel::Ge {
                p_good,
                p_bad,
                mean_good_ms,
                mean_bad_ms,
            } => {
                // Zaman-tabanlı geometrik durum-geçişi (dt/mean olasılığıyla flip).
                let dt = t_ms.saturating_sub(self.last_t_ms).max(1);
                self.last_t_ms = t_ms;
                let mean = if self.ge_bad {
                    mean_bad_ms
                } else {
                    mean_good_ms
                };
                let flip_p = (dt as f64 / mean as f64).min(1.0);
                if !rng.pass(flip_p) {
                    self.ge_bad = !self.ge_bad;
                }
                rng.pass(if self.ge_bad { p_bad } else { p_good })
            }
        };
        self.note(passed)
    }
}

/// Senaryo-parametreleri (M1/M3-K1/M5 ortak-harness).
#[derive(Clone, Copy)]
struct Scenario {
    fwd: ChanModel,
    p_rev: f64,
    d_ms: u64,       // tek-yön gecikme
    cap_pps: u64,    // forward-link kapasitesi
    app_gap_ms: u64, // akışlı-yük aralığı
    fec_on: bool,
    w_units: u64,
    t_max_ms: u64,
    r_parity: u64, // blok-başına parity-adedi (sabit-FEC-arm-seviyesi; tarama-kesit-1)
    sack_ms: u64,  // SACK-kadansı (fixed-ARQ-yoğunluk-PROXY'si; tarama-kesit-1)
}

impl Scenario {
    fn m1(fec_on: bool, p_fwd: f64, p_rev: f64) -> Self {
        Scenario {
            fwd: ChanModel::Iid(p_fwd),
            p_rev,
            d_ms: D_MS,
            cap_pps: CAP_PPS,
            app_gap_ms: APP_GAP_MS,
            fec_on,
            w_units: 400,
            t_max_ms: 300_000,
            r_parity: R,
            sack_ms: SACK_MS,
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
    retx_dup_emitted: u64, // Retransmit+Duplicate decision-sayısı (K1-vergisi/M5)
    max_queue_ms: u64,     // FIFO-kuyruk gecikme-tepe (bufferbloat-gözlemi)
    /// Deadline-hit-rate ham-verisi (M2-metriği; NETLEŞTİRME-2): teslim-anı ≤ {2s,5s,10s}
    /// eşiklerindeki unit-sayısı (payda = total_units; ıskalayan+teslim-edilmeyen = miss).
    dl_hits: [usize; 3],
    // ── Validity-audit alanları (Kesit-1 denetimi; harness-only) ──
    app_sent: u64,
    data_units_emitted: usize, // distinct-DATA-unit emisyonu (kuyrukta-kalan = app_sent − bu)
    blocks_enqueue_completed: u64, // parity-enqueue-tetiklenen blok-sayısı
    parity_enqueued: u64,      // enqueue_fec_parity çağrı-adedi (= r × blocks_completed)
    censored: bool,            // horizon-censoring: t_max'a-çarpıp tam-teslim-olmadı
    fwd_attempts: u64,
    fwd_drops: u64,
    burst_count: u64,
    burst_mean_x100: u64,
    burst_max: u64,
    // FusionInput-KISMİ telemetri (mevcut-public-diag'lardan; loss_ppm/jitter/ackconf diag-YOK — plan-notu):
    fi_btlbw: [u64; 4], // min/p50/p90/max (500ms-örnekleme; Commit-D canonical-snapshot)
    fi_qd_ratio: [u64; 4], // qd_ratio_permille (snapshot)
    fi_no_rule: u64,    // diag_fuzzy_no_rule (kümülatif)
    fi_loss_ppm: [u64; 4],
    fi_jitter_ppm: [u64; 4],
    fi_ackconf: [u64; 4],
}

/// Tek hücre-arm koşusu. `fec_on` tek-fark (KARAR-3).
#[allow(clippy::too_many_lines)]
fn run_arm(sc: Scenario, seed: u64) -> Metrics {
    let Scenario {
        fwd,
        p_rev,
        d_ms,
        cap_pps,
        app_gap_ms,
        fec_on,
        w_units,
        t_max_ms,
        r_parity,
        sack_ms,
    } = sc;
    let ser_ms: u64 = 1000 / cap_pps;
    let mut fwd_chan = FwdChan::new(fwd);
    let mut s = Sender::new();
    s.set_manual_fec_enabled(fec_on).expect("Manual-mode");
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
    let mut retx_dup_emitted = 0u64;
    let mut max_queue_ms = 0u64;

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
                             parity_emitted: &mut u64,
                             fwd_chan: &mut FwdChan,
                             retx_dup: &mut u64,
                             max_q: &mut u64,
                             data_units: &mut BTreeSet<u64>| {
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
            if matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate) {
                *retx_dup += 1;
            }
            if d.kind == TransmitKind::Data {
                data_units.insert(d.unit_id);
            }
            seq_unit.insert(d.seq, d.unit_id);
            // Forward-link: KAPASİTELİ-FIFO (serialization+kuyruk) → sonra loss-zar.
            let depart = (*link_free_ms).max(t_ms);
            *link_free_ms = depart + ser_ms;
            *max_q = (*max_q).max(depart.saturating_sub(t_ms));
            if fwd_chan.pass(rng, t_ms) {
                arrivals.push(Arrival {
                    t_ms: depart + ser_ms + d_ms,
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
    let mut data_units_emitted: BTreeSet<u64> = BTreeSet::new();
    let mut blocks_enqueue_completed = 0u64;
    let mut parity_enqueued = 0u64;
    let mut fi_btlbw_s: Vec<u64> = Vec::new();
    let mut fi_qd_s: Vec<u64> = Vec::new();
    let mut fi_loss_s: Vec<u64> = Vec::new();
    let mut fi_jit_s: Vec<u64> = Vec::new();
    let mut fi_ack_s: Vec<u64> = Vec::new();
    let mut t_ms: u64 = 0;
    while t_ms < t_max_ms {
        t_ms += TICK_MS;
        let now = t_ms * MS;

        // 0) Akışlı-app: her APP_GAP_MS bir-unit (W'ye kadar) — gerçek-kullanım-modeli.
        if t_ms.is_multiple_of(app_gap_ms) && app_sent < w_units {
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
                &mut fwd_chan,
                &mut retx_dup_emitted,
                &mut max_queue_ms,
                &mut data_units_emitted,
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
                &mut fwd_chan,
                &mut retx_dup_emitted,
                &mut max_queue_ms,
                &mut data_units_emitted,
            );
        }

        // FusionInput-örnekleme (500ms): Commit-D CANONICAL-snapshot (controller'ın-gördüğü;
        // environment-knob-değil). 5-alan-tam → kesit-2-önkoşul-4 kapanışı.
        if t_ms.is_multiple_of(500) {
            if let Some(snap) = s.diag_fusion_input_snapshot() {
                fi_btlbw_s.push(snap.bandwidth_bps);
                fi_qd_s.push(snap.qd_ratio_permille);
                fi_loss_s.push(snap.loss_windowed_ppm);
                fi_jit_s.push(snap.jitter_ppm);
                fi_ack_s.push(snap.ack_confidence_permille);
            }
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
                blocks_enqueue_completed += 1;
                for r in 0..r_parity {
                    s.enqueue_fec_parity(b, (K + r) as u16, UNIT);
                    parity_enqueued += 1;
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
        if t_ms.is_multiple_of(sack_ms)
            && (!rx_acked.is_empty() || !pending_recovered.is_empty())
            && rng.pass(p_rev)
        {
            {
                let snapshot: Vec<u64> = rx_acked.iter().copied().collect();
                let rec_batch: Vec<(u32, u16, u64)> = pending_recovered.clone();
                sacks.push((t_ms + d_ms, snapshot, rec_batch));
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
                    &mut fwd_chan,
                    &mut retx_dup_emitted,
                    &mut max_queue_ms,
                    &mut data_units_emitted,
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
    let dl_hits =
        [2_000u64, 5_000, 10_000].map(|d| deliver_t.values().filter(|&&t| t <= d).count());
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
        retx_dup_emitted,
        max_queue_ms,
        dl_hits,
        app_sent,
        data_units_emitted: data_units_emitted.len(),
        blocks_enqueue_completed,
        parity_enqueued,
        censored: (deliver_t.len() as u64) < w_units,
        fwd_attempts: fwd_chan.attempts,
        fwd_drops: fwd_chan.drops,
        burst_count: fwd_chan.burst_count,
        burst_mean_x100: (fwd_chan.burst_sum * 100)
            .checked_div(fwd_chan.burst_count)
            .unwrap_or(0),
        burst_max: fwd_chan.burst_max,
        fi_btlbw: quartiles(&mut fi_btlbw_s),
        fi_qd_ratio: quartiles(&mut fi_qd_s),
        fi_no_rule: s.diag_fuzzy_no_rule(),
        fi_loss_ppm: quartiles(&mut fi_loss_s),
        fi_jitter_ppm: quartiles(&mut fi_jit_s),
        fi_ackconf: quartiles(&mut fi_ack_s),
    }
}

/// (min, p50, p90, max) — audit-özeti.
fn quartiles(v: &mut [u64]) -> [u64; 4] {
    if v.is_empty() {
        return [0; 4];
    }
    v.sort_unstable();
    let n = v.len();
    [v[0], v[n / 2], v[(n * 9 / 10).min(n - 1)], v[n - 1]]
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp); // NaN-güvenli (ARQ-arm suppress=NaN → sona-sıralanır)
    v[v.len() / 2]
}

/// Ağır-matris: 12-hücre × 2-arm × 5-seed. `--ignored --nocapture` ile koşulur; tablo stdout'a.
#[test]
#[ignore]
fn m1_matrix() {
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
                    let m = run_arm(Scenario::m1(arm, pf, pr), seed);
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
        let m = run_arm(
            Scenario {
                w_units: 64,
                t_max_ms: 60_000,
                ..Scenario::m1(arm, 0.05, 0.70)
            },
            7,
        );
        assert!(m.delivered_units > 0, "teslim-oldu (arm={arm})");
        assert_eq!(m.evicted, 0, "KARAR-2 guard");
        assert!(m.sealed_bytes > 0);
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════════
// M3-K1 — FEC-on TEMİZ-LİNK spurious-retransmit-vergisi (sim-ön-kontrol; gerçek-K1 netem-nightly)
// ══════════════════════════════════════════════════════════════════════════════════════════════
#[test]
fn m3_k1_clean_link_spurious_tax() {
    let base = Scenario {
        fwd: ChanModel::Iid(0.0),
        p_rev: 0.0,
        w_units: 200,
        t_max_ms: 120_000,
        ..Scenario::m1(false, 0.0, 0.0)
    };
    let arq = run_arm(
        Scenario {
            fec_on: false,
            ..base
        },
        5,
    );
    let fec = run_arm(
        Scenario {
            fec_on: true,
            ..base
        },
        5,
    );
    eprintln!(
        "[M3-K1] temiz-link: ARQ retx_dup={} sealed={}B | FEC retx_dup={} sealed={}B parity={} (vergi: retx_dup-farkı={}, byte-farkı={}B)",
        arq.retx_dup_emitted, arq.sealed_bytes, fec.retx_dup_emitted, fec.sealed_bytes,
        fec.parity_emitted,
        fec.retx_dup_emitted as i64 - arq.retx_dup_emitted as i64,
        fec.sealed_bytes as i64 - arq.sealed_bytes as i64
    );
    assert_eq!(arq.delivered_units, 200);
    assert_eq!(fec.delivered_units, 200);
    assert_eq!(fec.evicted, 0);
    // K1-sim-hükmü raporda: sim'de spurious-gap-vergisi retx_dup-farkı olarak ölçülür;
    // sim-SACK-kümülatif-idealize olduğundan sıfır-çıkması "netem'de-de-sıfır" DEMEK-DEĞİL (nightly-madde).
}

// ══════════════════════════════════════════════════════════════════════════════════════════════
// M5 — TRACE-SETİ + TRAFO-PROFİLLERİ (CC-katmanı testi; çoğu fec-off — "FEC-orada-sessiz").
// PMTU-shrink: transport-PMTU-mekanizması-YOK (DEFAULT_MTU-sabit; envanter) → known-limitation, taranamaz.
// ══════════════════════════════════════════════════════════════════════════════════════════════
#[test]
#[ignore]
fn m5_trace_table() {
    let traces: Vec<(&str, Scenario)> = vec![
        (
            "iid-1%",
            Scenario {
                fwd: ChanModel::Iid(0.01),
                p_rev: 0.0,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            "iid-10%",
            Scenario {
                fwd: ChanModel::Iid(0.10),
                p_rev: 0.0,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            "iid-30%",
            Scenario {
                fwd: ChanModel::Iid(0.30),
                p_rev: 0.0,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            "ge-burst",
            Scenario {
                fwd: ChanModel::Ge {
                    p_good: 0.001,
                    p_bad: 0.40,
                    mean_good_ms: 2000,
                    mean_bad_ms: 200,
                },
                p_rev: 0.0,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            "rtt-1s-iid5%",
            Scenario {
                fwd: ChanModel::Iid(0.05),
                p_rev: 0.0,
                d_ms: 500,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            "rtt-5s-iid5%",
            Scenario {
                fwd: ChanModel::Iid(0.05),
                p_rev: 0.0,
                d_ms: 2500,
                t_max_ms: 600_000,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            "rev-95-rtt1s",
            Scenario {
                fwd: ChanModel::Iid(0.05),
                p_rev: 0.95,
                d_ms: 500,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            // TRAFO-1 bufferbloat: loss-0, RTT-baz-64ms, offered≈C → FIFO-kuyruk şişer (RTT 64→saniyeler)
            "trafo-bufferbloat",
            Scenario {
                fwd: ChanModel::Iid(0.0),
                p_rev: 0.0,
                d_ms: 32,
                app_gap_ms: 20,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
        (
            // TRAFO-2 kırsal: ~30kbps (C=3pps@1237B), yüksek-RTT-600ms, hafif-loss
            "trafo-kirsal-30kbps",
            Scenario {
                fwd: ChanModel::Iid(0.02),
                p_rev: 0.0,
                d_ms: 300,
                cap_pps: 3,
                app_gap_ms: 350,
                w_units: 100,
                t_max_ms: 600_000,
                ..Scenario::m1(false, 0.0, 0.0)
            },
        ),
    ];
    println!("trace,completion,t_p50_ms,t_p95_ms,retx_dup,sealed_MB,max_queue_ms,evicted");
    for (name, sc) in traces {
        let mut comp = vec![];
        let mut p50 = vec![];
        let mut p95 = vec![];
        let mut rd = vec![];
        let mut sl = vec![];
        let mut mq = vec![];
        let mut ev = 0u64;
        for seed in [11u64, 23, 37] {
            let m = run_arm(sc, seed);
            comp.push(m.delivered_units as f64 / m.total_units as f64);
            p50.push(m.t_p50_ms as f64);
            p95.push(m.t_p95_ms as f64);
            rd.push(m.retx_dup_emitted as f64);
            sl.push(m.sealed_bytes as f64 / 1e6);
            mq.push(m.max_queue_ms as f64);
            ev = ev.max(m.evicted);
        }
        println!(
            "{name},{:.3},{:.0},{:.0},{:.0},{:.2},{:.0},{ev}",
            median(comp),
            median(p50),
            median(p95),
            median(rd),
            median(sl),
            median(mq)
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════════
// TARAMA-KESİT-1 — karar-yüzeyi tek-eksen-süpürmeleri (≤200-hücre; NETLEŞTİRME-1).
// 7-arm: ARQ-baseline(≡ARQ-orta, sack50) · FEC-düşük/orta/yüksek(r=2/4/8) · ARQ-hafif/orta/agresif
// (sack=100/50/25 — ARQ-yoğunluk-PROXY'si: feedback-frekansı; motor-içi-agresiflik-düğmesi mühürlü).
// Nominal (süpürülmeyen-eksenler): IID-5%, rev=0, RTT≈150ms(d=50), C=50pps, gap=40ms, W=400, T=300s.
// Deadline-ekseni: AYRI-KOŞU-DEĞİL — dl_hits post-hoc-eşikler (2s/5s/10s; NETLEŞTİRME-2-metriği).
// E-ekseni: bu-kesitte ANALİTİK (M3-verisi; sim-shard-düzeyi-refactor = ikinci-kesit).
// ══════════════════════════════════════════════════════════════════════════════════════════════
struct Arm {
    name: &'static str,
    fec_on: bool,
    r: u64,
    sack: u64,
}

/// Kesit-1 hücre-inşası (sweep + seed-level-audit ortak; 6-distinct-executed-arms / 7-logical-labels).
fn kesit1_cells() -> Vec<(String, Scenario)> {
    let arms = [
        Arm {
            name: "ARQ-baseline",
            fec_on: false,
            r: 0,
            sack: 50,
        },
        Arm {
            name: "FEC-dusuk-r2",
            fec_on: true,
            r: 2,
            sack: 50,
        },
        Arm {
            name: "FEC-orta-r4",
            fec_on: true,
            r: 4,
            sack: 50,
        },
        Arm {
            name: "FEC-yuksek-r8",
            fec_on: true,
            r: 8,
            sack: 50,
        },
        Arm {
            name: "ARQ-hafif-s100",
            fec_on: false,
            r: 0,
            sack: 100,
        },
        // ARQ-orta ≡ ARQ-baseline (sack50) — ayrı-koşu-yok, tabloda-not.
        Arm {
            name: "ARQ-agresif-s25",
            fec_on: false,
            r: 0,
            sack: 25,
        },
    ];
    let nominal = |fec_on: bool| Scenario::m1(fec_on, 0.05, 0.0);

    // Eksenler: (ad, senaryo-mutasyonu listesi)
    let mut cells: Vec<(String, Scenario)> = Vec::new();
    // 1) forward-loss
    for pf in [0.01, 0.03, 0.05, 0.10, 0.20, 0.30] {
        for a in &arms {
            let mut sc = nominal(a.fec_on);
            sc.fwd = ChanModel::Iid(pf);
            sc.r_parity = a.r;
            sc.sack_ms = a.sack;
            cells.push((format!("loss,{:.0}%,{}", pf * 100.0, a.name), sc));
        }
    }
    // 2) IID-vs-burst (aynı-kaba-oran-sınıfı; patern-farkı)
    let patterns: [(&str, ChanModel); 3] = [
        ("iid5", ChanModel::Iid(0.05)),
        (
            "ge-hafif",
            ChanModel::Ge {
                p_good: 0.001,
                p_bad: 0.20,
                mean_good_ms: 5000,
                mean_bad_ms: 500,
            },
        ),
        (
            "ge-agir",
            ChanModel::Ge {
                p_good: 0.001,
                p_bad: 0.40,
                mean_good_ms: 2000,
                mean_bad_ms: 200,
            },
        ),
    ];
    for (pn, pm) in patterns {
        for a in &arms {
            let mut sc = nominal(a.fec_on);
            sc.fwd = pm;
            sc.r_parity = a.r;
            sc.sack_ms = a.sack;
            cells.push((format!("pattern,{pn},{}", a.name), sc));
        }
    }
    // 3) bandwidth (headroom-sabit: gap = 2×ser → offered≈C/2)
    for cap in [12u64, 25, 50, 100] {
        for a in &arms {
            let mut sc = nominal(a.fec_on);
            sc.cap_pps = cap;
            sc.app_gap_ms = 2 * (1000 / cap);
            sc.r_parity = a.r;
            sc.sack_ms = a.sack;
            cells.push((format!("bw,{cap}pps,{}", a.name), sc));
        }
    }
    // 4) offered/headroom (C=50-sabit; gap süpürülür: ≈C/4, C/3, C/2, 3C/4)
    for gap in [80u64, 53, 40, 27] {
        for a in &arms {
            let mut sc = nominal(a.fec_on);
            sc.app_gap_ms = gap;
            sc.r_parity = a.r;
            sc.sack_ms = a.sack;
            cells.push((format!("headroom,gap{gap}ms,{}", a.name), sc));
        }
    }
    // 5) RTT
    for d in [50u64, 250, 1000, 2500] {
        for a in &arms {
            let mut sc = nominal(a.fec_on);
            sc.d_ms = d;
            if d >= 1000 {
                sc.t_max_ms = 600_000;
            }
            sc.r_parity = a.r;
            sc.sack_ms = a.sack;
            cells.push((format!("rtt,d{d}ms,{}", a.name), sc));
        }
    }
    // 6) reverse-silence
    for pr in [0.50, 0.90, 0.99] {
        for a in &arms {
            let mut sc = nominal(a.fec_on);
            sc.p_rev = pr;
            sc.r_parity = a.r;
            sc.sack_ms = a.sack;
            cells.push((format!("rev,{:.0}%,{}", pr * 100.0, a.name), sc));
        }
    }
    cells
}

#[test]
#[ignore]
fn sweep_kesit_1() {
    let cells = kesit1_cells();
    assert!(
        cells.len() <= 200,
        "NETLEŞTİRME-1: hücre={} ≤ 200",
        cells.len()
    );
    eprintln!("[kesit-1] hücre-sayısı: {}", cells.len());

    println!("eksen,nokta,arm,goodput_per_byte,completion,t_p50_ms,t_p95_ms,dl2s,dl5s,dl10s,retx_dup,parity,cancels,sealed_MB,max_q_ms,evicted");
    for (label, sc) in cells {
        let mut gp = vec![];
        let mut comp = vec![];
        let mut p50 = vec![];
        let mut p95 = vec![];
        let mut d2 = vec![];
        let mut d5 = vec![];
        let mut d10 = vec![];
        let mut rd = vec![];
        let mut par = vec![];
        let mut canc = vec![];
        let mut sl = vec![];
        let mut mq = vec![];
        let mut ev = 0u64;
        for seed in [11u64, 23, 37] {
            let m = run_arm(sc, seed);
            let tot = m.total_units as f64;
            gp.push(m.delivered_bytes as f64 / m.sealed_bytes.max(1) as f64);
            comp.push(m.delivered_units as f64 / tot);
            p50.push(m.t_p50_ms as f64);
            p95.push(m.t_p95_ms as f64);
            d2.push(m.dl_hits[0] as f64 / tot);
            d5.push(m.dl_hits[1] as f64 / tot);
            d10.push(m.dl_hits[2] as f64 / tot);
            rd.push(m.retx_dup_emitted as f64);
            par.push(m.parity_emitted as f64);
            canc.push(m.recovered_cancels as f64);
            sl.push(m.sealed_bytes as f64 / 1e6);
            mq.push(m.max_queue_ms as f64);
            ev = ev.max(m.evicted);
        }
        println!(
            "{label},{:.4},{:.3},{:.0},{:.0},{:.3},{:.3},{:.3},{:.0},{:.0},{:.0},{:.2},{:.0},{ev}",
            median(gp),
            median(comp),
            median(p50),
            median(p95),
            median(d2),
            median(d5),
            median(d10),
            median(rd),
            median(par),
            median(canc),
            median(sl),
            median(mq)
        );
        assert_eq!(ev, 0, "KARAR-2 evicted-guard: {label}");
    }
}

// ══════════════════════════════════════════════════════════════════════════════════════════════
// KESİT-1 VALIDITY-AUDIT — seed-level döküm (medyan-CSV-sınırlaması-giderimi) + audit-sütunları.
// ══════════════════════════════════════════════════════════════════════════════════════════════
#[test]
#[ignore]
fn audit_seed_level() {
    println!("eksen,nokta,arm,seed,goodput,completion,t_p50,t_p95,dl2,dl5,dl10,retx_dup,parity_enq,parity_emit,blocks_enq,cancels,sealed_B,max_q,evicted,censored,app_sent,data_emitted,fwd_att,fwd_drop,burst_n,burst_mean_x100,burst_max,btlbw_min,btlbw_p50,btlbw_p90,btlbw_max,qdr_p50,qdr_max,no_rule,loss_ppm_p50,jitter_ppm_p50,ackconf_p50");
    for (label, sc) in kesit1_cells() {
        for seed in [11u64, 23, 37] {
            let m = run_arm(sc, seed);
            let tot = m.total_units as f64;
            println!(
                "{label},{seed},{:.4},{:.4},{},{},{:.4},{:.4},{:.4},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                m.delivered_bytes as f64 / m.sealed_bytes.max(1) as f64,
                m.delivered_units as f64 / tot,
                m.t_p50_ms,
                m.t_p95_ms,
                m.dl_hits[0] as f64 / tot,
                m.dl_hits[1] as f64 / tot,
                m.dl_hits[2] as f64 / tot,
                m.retx_dup_emitted,
                m.parity_enqueued,
                m.parity_emitted,
                m.blocks_enqueue_completed,
                m.recovered_cancels,
                m.sealed_bytes,
                m.max_queue_ms,
                m.evicted,
                u8::from(m.censored),
                m.app_sent,
                m.data_units_emitted,
                m.fwd_attempts,
                m.fwd_drops,
                m.burst_count,
                m.burst_mean_x100,
                m.burst_max,
                m.fi_btlbw[0],
                m.fi_btlbw[1],
                m.fi_btlbw[2],
                m.fi_btlbw[3],
                m.fi_qd_ratio[1],
                m.fi_qd_ratio[3],
                m.fi_no_rule,
                m.fi_loss_ppm[1],
                m.fi_jitter_ppm[1],
                m.fi_ackconf[1]
            );
        }
    }
}
