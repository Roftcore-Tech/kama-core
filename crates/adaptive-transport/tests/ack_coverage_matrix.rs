//! [İŞ-32 · kenar kuralı · sahip kararı 2026-11-21] Dolu ACK çerçevesinde bildirilmeyen seq'ler
//! için alıcı (eski / yeni) × gönderici (eski / yeni) matrisi.
//!
//! * eski alıcı: spec §2.1 kuralı, çerçeve taşarsa en yeni bloklar (`Receiver` varsayılanı);
//! * yeni alıcı: kırpılan blokları yeniden raporlayan seçim (`Receiver::set_ack_rereport(true)`);
//! * eski gönderici: çerçeve tam küme sayılır (bugünkü yol; `set_ack_coverage` çağrılmaz);
//! * yeni gönderici: ürün çalışma zamanıyla aynı kapı: çerçeve dolu (`MAX_ACK_RANGES` aralık) ise
//!   delik uçları `Sender::set_ack_coverage` ile verilir (`loss::ack_hole_edges`).
//!
//! Düzenek (ayrık olay, deterministik; gerçek `Sender` + gerçek `Receiver`): `line_guard_sim.rs` ile
//! aynı geri bildirimi aç profil kurulumu (profil 12 500 / 62 B/s, `FeedbackStarvedRule`, PTT benzeri
//! kısa birimlerin ACK'siz kopyaları) ve aynı hatlar: ileri hat FIFO (64 paket, 300 ms, serileşmeden
//! sonra %20 tohumlu kayıp), dönüş hattı 62 B/s, 300 ms. Dosya 41 × 1 000 B ve her 8 birimde iki FEC
//! onarım sembolü (manuel kip).
//!
//! İleri hat sırayı korur (FIFO, yeniden sıralama yok). Bu yüzden alıcının bir seq'i aldığı ANDAN
//! SONRA o seq'in kayıp ilanı kesin olarak yanlıştır: bu testin "alınmış iken ilan" sayacı budur.

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::ack::MAX_ACK_RANGES;
use adaptive_transport::loss::{ack_hole_edges, CoverageDiag};
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, EmitCause, Event, FecActuationMode, FeedbackStarvedRule, ForwardCopyRule, OpaqueUnit,
    Sender, Timer, TransmitDecision, TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const ACK_SEAL: u64 = 22;
const ONE_WAY_NS: u64 = 300 * MS;
const QUEUE_PKTS: usize = 64;
const LOSS_PERMILLE: u64 = 200;
const FILE_UNITS: u64 = 41;
const FILE_UNIT: u64 = 1000;
const FILE_BASE_ID: u64 = 1_000_000;
const FEC_BLOCK: u64 = 8;
const FEC_PARITY: u16 = 2;
const VOICE_UNIT: u64 = 86;
const VOICE_PERIOD_NS: u64 = 200 * MS;
const VOICE_START_NS: u64 = 500 * MS;
/// Bu süreden eski onarım sembolü koşu sonunda hâlâ uçuştaysa sızıntıdır (teyitli kopyanın
/// bırakma saati `K_CONFIRMED_RELEASE × T_loss` bu hatta ≈ 10–20 s).
const PARITY_STALE_NS: u64 = 40 * SEC;
/// Teslim edilmemiş sayılmak için kısa birimin koşu sonundan en az bu kadar önce sunulmuş olması
/// (ileri yol 0,3 s + kuyruk payı).
const SETTLE_NS: u64 = 3 * SEC;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Engine {
    /// Ürünün geri bildirimi aç profil kurulumu (port motoru + `FeedbackStarvedRule`).
    PortProfile,
    /// Port motoru, profil kuralı yok (profil kapalı yol).
    PortNoRule,
    /// Eski motor (`Sender::new()`).
    Legacy,
}

#[derive(Clone, Copy, Debug)]
struct Arm {
    engine: Engine,
    /// Yeni alıcı (yeniden raporlama açık).
    rereport: bool,
    /// Yeni gönderici (dolu çerçevede kapsam bilgisi verilir).
    coverage: bool,
    /// Kapsam bilgisi dolu olmayan çerçevede de verilir (yalnız kuralın kapalı yolda bilgiyi yok
    /// saydığını sınayan test; ürün çalışma zamanı bunu yapmaz).
    coverage_every_ack: bool,
    real_up: u64,
    seed: u64,
    t_max_ns: u64,
}

struct Bottleneck {
    rate_bps: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64, TransmitKind)>,
    lcg: u64,
}

impl Bottleneck {
    fn new(rate_bps: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
        }
    }

    /// Döner: datagram düştü mü.
    fn send(&mut self, now: u64, d: &TransmitDecision) -> bool {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= QUEUE_PKTS {
            return true;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + (d.size + OVH) * SEC / self.rate_bps;
        self.busy.push_back(done);
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        if (self.lcg >> 33) % 1000 < LOSS_PERMILLE {
            return true;
        }
        self.wire
            .push_back((done + ONE_WAY_NS, d.seq, d.unit_id, d.kind));
        false
    }
}

struct ReturnLine {
    free_at: u64,
    wire: VecDeque<(u64, u64, AckInfo, Option<Vec<u64>>)>,
}

impl ReturnLine {
    fn send(&mut self, now: u64, udp: u64, ack: AckInfo, coverage: Option<Vec<u64>>) {
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        self.wire
            .push_back((self.free_at + ONE_WAY_NS, udp, ack, coverage));
    }
}

#[derive(Debug, Default)]
struct Outcome {
    /// Göndericinin bütün kararları (an, karar) — bayt-aynılık kıyası için.
    trace: Vec<(u64, TransmitDecision)>,
    /// Kayıp ilanları (an, seq).
    declared: Vec<(u64, u64)>,
    /// Paket eşiği / zaman eşiği ilan sayısı.
    declared_pkt: usize,
    declared_time: usize,
    /// Alıcının ilan anından ÖNCE aldığı seq'in ilanı (FIFO hatta kesin yanlış).
    declared_received: usize,
    /// İlan edilen ama hatta düşmemiş seq (alınmış ya da yolda).
    declared_not_dropped: usize,
    /// ARQ yeniden iletimi: toplam / birim alıcıda teslim edilmişken (gereksiz).
    retx_total: usize,
    retx_unneeded: usize,
    /// PTO probu: toplam / gereksiz.
    pto_total: usize,
    pto_unneeded: usize,
    file_done_ns: Option<u64>,
    voice_offered: u64,
    voice_undelivered: usize,
    /// Koşu sonundan en az [`SETTLE_NS`] önce sunulmuş ama teslim edilmemiş kısa birim (koşu sonu
    /// kesintisi hariç).
    voice_undelivered_settled: usize,
    /// Alıcının ürettiği dolu (`MAX_ACK_RANGES` aralıklı) çerçeve sayısı (kollardan bağımsız).
    full_frames: usize,
    /// Göndericiye verilen kapsam bilgisi sayısı (`set_ack_coverage` çağrısı).
    hints_given: usize,
    /// Dosya birimleri: ilk iletimi düşen birimin (ilk iletim → teslim) süresi.
    file_recovery_ns: Vec<u64>,
    parity_stale: usize,
    coverage: CoverageDiag,
    reorder_start: u32,
    reorder_end: u32,
    diag_loss: (u64, u64, u64),
    diag_ledger: (u64, u64, u64),
}

/// Göndericinin emisyonları: hatta verir ve gerçek kaderini kaydeder.
#[derive(Default)]
struct Track {
    tx_datagrams: u64,
    sent_at: HashMap<u64, (u64, TransmitKind)>,
    dropped: HashSet<u64>,
    /// Dosya birimi → ilk iletiminin (anı, düştü mü).
    file_first: HashMap<u64, (u64, bool)>,
}

impl Track {
    fn emit(
        &mut self,
        decs: Vec<TransmitDecision>,
        now: u64,
        fwd: &mut Bottleneck,
        o: &mut Outcome,
    ) {
        for d in decs {
            o.trace.push((now, d));
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            self.tx_datagrams += 1;
            self.sent_at.insert(d.seq, (now, d.kind));
            let lost = fwd.send(now, &d);
            if lost {
                self.dropped.insert(d.seq);
            }
            if d.kind == TransmitKind::Data && d.unit_id >= FILE_BASE_ID {
                self.file_first.entry(d.unit_id).or_insert((now, lost));
            }
        }
    }
}

fn sender(engine: Engine) -> Sender {
    let mut s = match engine {
        Engine::Legacy => Sender::new(),
        Engine::PortProfile | Engine::PortNoRule => Sender::with_cc_port(),
    };
    if engine == Engine::PortProfile {
        let t_ack = adaptive_transport::receiver::budgeted_ack_interval_ns(PROFILE_DOWN);
        s.set_cc_initial_window(PROFILE_UP * t_ack / SEC);
        let cap = PROFILE_UP * MTU / (MTU + OVH);
        s.set_cc_profile_pacing_cap(Some(cap));
        s.set_cc_feedback_starved(Some(FeedbackStarvedRule {
            pacing_cap_bps: cap,
            feedback_ns: feedback_horizon_ns(PROFILE_DOWN),
            line: LineModel {
                rate_bps: PROFILE_UP,
                datagram_overhead: OVH,
            },
            return_line: LineModel {
                rate_bps: PROFILE_DOWN,
                datagram_overhead: OVH,
            },
        }));
        s.set_forward_copy_rule(Some(ForwardCopyRule {
            below_id: FILE_BASE_ID,
            priority_below_id: FILE_BASE_ID,
            copies: 4,
            spacing_ns: 40 * MS,
        }));
    }
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .expect("manuel FEC kipi");
    s.set_diag_emit_log(true);
    s
}

#[allow(clippy::too_many_lines, deprecated)] // `enqueue_fec_parity`: cc_port_sim ile aynı düzenek yolu
fn run(arm: Arm) -> Outcome {
    let mut s = sender(arm.engine);
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    r.set_ack_rereport(arm.rereport);
    let mut fwd = Bottleneck::new(arm.real_up, arm.seed);
    let mut ret = ReturnLine {
        free_at: 0,
        wire: VecDeque::new(),
    };
    let mut o = Outcome {
        reorder_start: s.diag_ack_coverage().1,
        ..Outcome::default()
    };
    let mut tk = Track::default();
    let mut rx_at: HashMap<u64, u64> = HashMap::new();
    let mut delivered_at: HashMap<u64, u64> = HashMap::new();
    for i in 0..FILE_UNITS {
        let id = FILE_BASE_ID + i;
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id,
                size: FILE_UNIT,
            }),
            0,
        );
        tk.emit(decs, 0, &mut fwd, &mut o);
        if (i + 1).is_multiple_of(FEC_BLOCK) {
            let block_id = u32::try_from(i / FEC_BLOCK).expect("blok");
            for esi in 0..FEC_PARITY {
                s.enqueue_fec_parity(block_id, 8 + esi, FILE_UNIT);
            }
        }
    }
    let mut file_left = FILE_UNITS;
    let mut next_voice = 0u64;
    let mut voice_offer_at: Vec<u64> = Vec::new();
    let mut now = 0u64;
    while now < arm.t_max_ns {
        let voice_at = VOICE_START_NS + next_voice * VOICE_PERIOD_NS;
        if o.file_done_ns.is_none() && now >= voice_at {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: VOICE_UNIT,
                }),
                now,
            );
            tk.emit(decs, now, &mut fwd, &mut o);
            voice_offer_at.push(now);
            next_voice += 1;
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        tk.emit(decs, now, &mut fwd, &mut o);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            tk.emit(decs, now, &mut fwd, &mut o);
        }
        r.set_uplink_yield(ret.free_at > now);
        let mut frames = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, kind) = fwd.wire.pop_front().expect("var");
            rx_at.entry(seq).or_insert(now);
            let rx_kind = if kind == TransmitKind::FecParity {
                RxKind::FecShard {
                    block_id: 0,
                    esi: 0,
                    k_actual: 8,
                    is_repair: true,
                }
            } else {
                RxKind::Data
            };
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: rx_kind,
                },
                now,
            );
            for u in out.delivered {
                if delivered_at.insert(u, now).is_none() && u >= FILE_BASE_ID {
                    file_left -= 1;
                    if file_left == 0 {
                        o.file_done_ns = Some(now);
                    }
                }
            }
            frames.extend(out.ack);
        }
        frames.extend(r.on_ack_timer(now));
        for frame in frames {
            let expected = r.largest_seq().unwrap_or(0) + 1;
            let udp = frame.encode().len() as u64 + ACK_SEAL;
            let pns = frame.acked_full(expected);
            o.full_frames += usize::from(frame.ranges.len() == MAX_ACK_RANGES);
            // Ürün çalışma zamanının kapısı: yalnız dolu çerçeve (bu düzenekte PN = seq).
            let coverage = (arm.coverage
                && (arm.coverage_every_ack || frame.ranges.len() == MAX_ACK_RANGES))
                .then(|| ack_hole_edges(&pns));
            ret.send(
                now,
                udp,
                AckInfo {
                    acked_seqs: pns,
                    ack_delay: frame.ack_delay_duration(),
                },
                coverage,
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, udp, ack, coverage) = ret.wire.pop_front().expect("var");
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: tk.tx_datagrams,
                },
                now,
            );
            if let Some(missing) = coverage {
                o.hints_given += 1;
                s.set_ack_coverage(missing);
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            tk.emit(decs, now, &mut fwd, &mut o);
        }
        for e in s.take_diag_emit_log() {
            match e.cause {
                EmitCause::Retransmit => {
                    o.retx_total += 1;
                    o.retx_unneeded += usize::from(delivered_at.contains_key(&e.unit_id));
                }
                EmitCause::PtoProbe => {
                    o.pto_total += 1;
                    o.pto_unneeded += usize::from(delivered_at.contains_key(&e.unit_id));
                }
                EmitCause::LossPacketThreshold => o.declared_pkt += 1,
                EmitCause::LossTimeThreshold => o.declared_time += 1,
                _ => {}
            }
            if matches!(
                e.cause,
                EmitCause::LossPacketThreshold | EmitCause::LossTimeThreshold
            ) {
                o.declared.push((e.at_ns, e.seq));
                if rx_at.get(&e.seq).is_some_and(|&t| t <= e.at_ns) {
                    o.declared_received += 1;
                }
                if !tk.dropped.contains(&e.seq) {
                    o.declared_not_dropped += 1;
                }
            }
        }
        let voice_left = (0..next_voice).any(|v| !delivered_at.contains_key(&v));
        if o.file_done_ns.is_some() && !voice_left {
            break;
        }
        now += MS;
    }
    o.voice_offered = next_voice;
    o.voice_undelivered = (0..next_voice)
        .filter(|v| !delivered_at.contains_key(v))
        .count();
    o.voice_undelivered_settled = voice_offer_at
        .iter()
        .enumerate()
        .filter(|&(v, &t)| t + SETTLE_NS <= now && !delivered_at.contains_key(&(v as u64)))
        .count();
    for (u, &(t0, lost)) in &tk.file_first {
        if lost {
            if let Some(&t1) = delivered_at.get(u) {
                o.file_recovery_ns.push(t1 - t0);
            }
        }
    }
    o.file_recovery_ns.sort_unstable();
    o.parity_stale = s
        .diag_inflight_seqs()
        .iter()
        .filter(|&&(seq, _, _, kind)| {
            kind == TransmitKind::FecParity
                && tk
                    .sent_at
                    .get(&seq)
                    .is_some_and(|&(t, _)| now.saturating_sub(t) >= PARITY_STALE_NS)
        })
        .count();
    let (cov, reorder) = s.diag_ack_coverage();
    o.coverage = cov;
    o.reorder_end = reorder;
    o.diag_loss = s.diag_loss();
    o.diag_ledger = s.diag_ledger();
    o
}

fn secs(ns: u64) -> String {
    format!("{:.2}", ns as f64 / SEC as f64)
}

fn label(rereport: bool, coverage: bool) -> &'static str {
    match (rereport, coverage) {
        (false, false) => "eski alıcı × eski gönderici",
        (false, true) => "eski alıcı × yeni gönderici",
        (true, false) => "yeni alıcı × eski gönderici",
        (true, true) => "yeni alıcı × yeni gönderici",
    }
}

/// Dört birleşim, iki gerçek hat (dürüst 12 500 ve iyimser profil altında 6 250 B/s), iki tohum.
///
/// * her kolda alıcı dolu çerçeve üretir (düzenek kuralı gerçekten sınar);
/// * eski gönderici (iki alıcıyla): dolu çerçevenin bildirmediği, alıcının ALMIŞ olduğu seq'leri
///   kayıp ilan eder — yalnız GÖZLEM olarak yazdırılır, iddia edilmez (bugünkü kusur başarı şartı
///   yapılmaz);
/// * yeni gönderici (iki alıcıyla): alınmış seq'i hiç ilan etmez, ilan ettiği her seq hatta
///   gerçekten düşmüştür; onarım sembolü uçuşta birikmez; dolu çerçeve gerçekten işlenmiştir;
///   reorder toleransı şişmez;
/// * yeni alıcı × yeni gönderici: dosya ve bütün kısa birimler teslim edilir.
///
/// CANLILIK İDDİA EDİLMEYEN birleşim: eski alıcı × yeni gönderici. Ölçüldü (2026-11-21, bu
/// düzenek, 150 s): iki hatta ve iki tohumda dosya bitmez; koşu sonundan ≥ 3 s önce sunulmuş
/// 4–33 kısa birim teslim edilmez (koşu sonu kesintisi dahil 10–42). Kök:
/// §2.1 alıcısı bir çerçeveye sığmayan bloğu bir daha bildirmez; kenar kuralı o seq'leri kayıp
/// saymaz; teslim edilmiş ama bildirilmemiş birim yalnız PTO probuyla teyit alabilir (ACK aralığında
/// ≤ 2 prob, en küçük kimlik önce), teyitsiz kayıtlar uçuşu doldurur (6 250, tohum 1, koşu sonu:
/// 781 kayıt uçuşta, 224'ü teslim edilmiş kısa birim). Bugünkü gönderici aynı seq'leri kayıp ilan
/// edip yeniden gönderir; yeni seq'ler sonraki çerçevede bildirilir. Bu birleşim yalnız gözlem
/// olarak yazdırılır; kenar kuralı bu yüzden ürün adayı DEĞİLDİR (sahibin önceden kayıtlı kuralı).
///
/// Gerçek kayıp kurtarma süresi (dosya birimi, ilk iletimi düşen) her birleşim için yazdırılır.
#[test]
fn receiver_sender_matrix_on_the_feedback_starved_line() {
    let mut failures: Vec<String> = Vec::new();
    for real_up in [6_250u64, PROFILE_UP] {
        for rereport in [false, true] {
            for coverage in [false, true] {
                let mut rec: Vec<u64> = Vec::new();
                for seed in 1..=2u64 {
                    let o = run(Arm {
                        engine: Engine::PortProfile,
                        rereport,
                        coverage,
                        coverage_every_ack: false,
                        real_up,
                        seed,
                        t_max_ns: 150 * SEC,
                    });
                    let tag = label(rereport, coverage);
                    println!(
                        "KAPSAM-MATRİS hat={real_up} tohum={seed} [{tag}]: ilan={} (paket {} zaman {}) \
                         alınmışken_ilan={} düşmemişken_ilan={} gereksiz_arq={}/{} gereksiz_pto={}/{} dosya={} \
                         kısa_birim_teslimsiz={}/{} (koşu sonu kesintisi hariç {}) kurtarma_max={} \
                         kurtarma_p50={} n={} bayat_parite={} dolu_çerçeve={} kapsam_bilgisi={} \
                         kapsam={:?} recent_max_reorder={}→{} (k={}→{}) diag_loss={:?} diag_ledger={:?}",
                        o.declared.len(),
                        o.declared_pkt,
                        o.declared_time,
                        o.declared_received,
                        o.declared_not_dropped,
                        o.retx_unneeded,
                        o.retx_total,
                        o.pto_unneeded,
                        o.pto_total,
                        o.file_done_ns.map_or("YOK".to_string(), secs),
                        o.voice_undelivered,
                        o.voice_offered,
                        o.voice_undelivered_settled,
                        o.file_recovery_ns.last().copied().map_or("-".into(), secs),
                        o.file_recovery_ns
                            .get(o.file_recovery_ns.len() / 2)
                            .copied()
                            .map_or("-".into(), secs),
                        o.file_recovery_ns.len(),
                        o.parity_stale,
                        o.full_frames,
                        o.hints_given,
                        o.coverage,
                        o.reorder_start,
                        o.reorder_end,
                        packet_k(o.reorder_start),
                        packet_k(o.reorder_end),
                        o.diag_loss,
                        o.diag_ledger,
                    );
                    rec.extend(&o.file_recovery_ns);
                    // Düzenek kuralı gerçekten sınıyor: her kolda alıcı dolu çerçeve üretti.
                    if o.full_frames == 0 {
                        failures.push(format!(
                            "{tag} hat={real_up} tohum={seed}: dolu çerçeve yok"
                        ));
                    }
                    if coverage {
                        let ctx = format!("{tag} hat={real_up} tohum={seed}");
                        let checks = [
                            (o.coverage.partial_frames > 0, "dolu çerçeve hiç işlenmedi"),
                            (o.declared_received == 0, "alınmış seq kayıp ilan edildi"),
                            (
                                o.declared_not_dropped == 0,
                                "düşmemiş seq kayıp ilan edildi",
                            ),
                            // Canlılık yalnız yeniden raporlamalı alıcıyla iddia edilir (bkz. doc).
                            (!rereport || o.file_done_ns.is_some(), "dosya bitmedi"),
                            (
                                !rereport || o.voice_undelivered == 0,
                                "teslimsiz kısa birim",
                            ),
                            (o.parity_stale == 0, "onarım sembolü uçuşta birikti"),
                            (o.reorder_end == o.reorder_start, "reorder toleransı şişti"),
                        ];
                        for (ok, what) in checks {
                            if !ok {
                                failures.push(format!("{ctx}: {what}"));
                            }
                        }
                    }
                }
                rec.sort_unstable();
                println!(
                    "KAPSAM-MATRİS-KURTARMA hat={real_up} [{}] (iki tohum birlikte): n={} p50={} max={}",
                    label(rereport, coverage),
                    rec.len(),
                    rec.get(rec.len() / 2).copied().map_or("-".into(), secs),
                    rec.last().copied().map_or("-".into(), secs),
                );
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Göndericinin paket eşiği `k = max(3, 2 × recent_max_reorder)` (`RetxConfig::fixture`, ürün
/// göndericisinin yapılandırması).
fn packet_k(recent_max_reorder: u32) -> u32 {
    3u32.max(recent_max_reorder.saturating_mul(2))
}

/// Profil kapalı yol (port motoru, kural yok) ve eski motor kapsam bilgisini YOK SAYAR: aynı
/// senaryoda `set_ack_coverage` her ACK'ten önce verilse de hiç verilmese de bütün kararlar, kayıp
/// ilanları ve kayıp sayaçları aynıdır. (Taban koda karşı bayt-aynılık CC sim kapısındadır.)
#[test]
fn profile_off_and_legacy_engines_ignore_coverage() {
    for engine in [Engine::PortNoRule, Engine::Legacy] {
        for rereport in [false, true] {
            let base = Arm {
                engine,
                rereport,
                coverage: false,
                // Bilgi her ACK'te verilir (dolu çerçeve beklenmez): kapalı yol onu her durumda
                // yok saymalı. Eski motor bu düzenekte dolu çerçeve üretmiyor (ölçüldü).
                coverage_every_ack: true,
                real_up: PROFILE_UP,
                seed: 1,
                t_max_ns: 40 * SEC,
            };
            let a = run(base);
            let b = run(Arm {
                coverage: true,
                ..base
            });
            assert!(!a.trace.is_empty(), "{engine:?}: karar yok");
            // Test boş değil: kapsam bilgisi gerçekten verildi.
            assert!(
                b.hints_given > 0,
                "{engine:?} rereport={rereport}: kapsam bilgisi verilmedi"
            );
            assert_eq!(
                b.coverage,
                CoverageDiag::default(),
                "{engine:?}: kural uygulandı"
            );
            assert_eq!(
                a.trace, b.trace,
                "{engine:?} rereport={rereport}: kararlar ayrıştı"
            );
            assert_eq!(a.declared, b.declared, "{engine:?}: ilanlar ayrıştı");
            assert_eq!(a.diag_loss, b.diag_loss, "{engine:?}: diag_loss ayrıştı");
            assert_eq!(
                a.diag_ledger, b.diag_ledger,
                "{engine:?}: diag_ledger ayrıştı"
            );
        }
    }
}
