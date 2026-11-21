//! [ACK-BITMAP · KALICI GERİLEME TESTİ] Dönüş yolunda ACK kaybı varken bitmap ACK'i (0x15) ile
//! geri bildirimi aç hatta canlılık: dosya biter, her kısa birim teslim edilir, uçuşta teyitsiz
//! kayıt birikmez.
//!
//! Korunan kusur (İŞ-32 / #225, 2026-11-21): bildirilmeyeni "bilinmiyor" sayan bir gönderici
//! kuralı, alıcı o seq'i bir daha bildirmezse teslim edilmiş birimi yalnız PTO probuyla teyit
//! ettirebilir; teyitsiz kayıtlar uçuş tavanını doldurur ve dosya ilerlemez (ölçüldü: 150 s'de
//! dosya bitmedi, 6 250 hattında 781 kayıt uçuşta, 224'ü teslim edilmiş kısa birim). Bitmap
//! alıcısı her seq'i ardışık iki ACK'te bildirir; ikisi de kaybolursa seq bir daha görünmez. Bu
//! test, o durumda göndericinin kapsam boşluğu kuralının (`loss::ThresholdLossDetector`,
//! `diag_ack_evidence_orphaned`) kaydı eşiğe açtığını ve oturumun ilerlediğini sınar.
//!
//! Düzenek (ayrık olay, deterministik; gerçek `Sender` + gerçek `Receiver`): `line_guard_sim.rs`
//! ve İŞ-32 kapsam matrisiyle aynı geri bildirimi aç profil kurulumu — profil 12 500 / 62 B/s,
//! `FeedbackStarvedRule`, PTT benzeri kısa birimlerin ACK'siz kopyaları; ileri hat FIFO (64 paket,
//! 300 ms, serileşmeden sonra %20 tohumlu kayıp), dönüş hattı 62 B/s, 300 ms. Dönüş hattında ACK
//! datagramı serileştikten sonra tohumlu olasılıkla düşer (`ack_loss_permille`). Dosya 41 × 1 000 B
//! ve her 8 birimde iki FEC onarım sembolü (manuel kip). Bu düzenekte PN = motor seq'i.

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::ack::{AckFrame, ACK_DATAGRAM_MAX_LINE_BYTES, FRAME_TYPE_ACK_BITMAP};
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
/// ACK datagramının çerçeve dışı zarfı OVH dışında kalan kısmı (handle 4 + seq 2 + etiket 16).
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
/// Teslim edilmemiş sayılmak için kısa birimin koşu sonundan en az bu kadar önce sunulmuş olması.
const SETTLE_NS: u64 = 3 * SEC;
/// Bu süreden eski onarım sembolü koşu sonunda hâlâ uçuştaysa sızıntıdır.
const PARITY_STALE_NS: u64 = 40 * SEC;
const T_MAX_NS: u64 = 150 * SEC;

fn lcg(x: &mut u64) -> u64 {
    *x = x
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *x >> 33
}

#[derive(Clone, Copy, Debug)]
struct Arm {
    bitmap: bool,
    real_up: u64,
    ack_loss: AckLoss,
    seed: u64,
}

/// Dönüş yolunda ACK kaybı deseni.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AckLoss {
    None,
    /// Her ACK tohumlu olasılıkla (binde) düşer.
    Random(u64),
    /// Dört ACK'ten ikinci ve üçüncüsü düşer: ardışık iki kayıp, her seq'i kapsayan iki ACK'in
    /// ikisini de götürür (bitmap alıcısının tekrarını kesin aşan desen).
    ConsecutivePairs,
}

impl std::fmt::Display for AckLoss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "yok"),
            Self::Random(p) => write!(f, "rastgele‰{p}"),
            Self::ConsecutivePairs => write!(f, "ardışık-çift"),
        }
    }
}

struct Bottleneck {
    rate_bps: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64, TransmitKind)>,
    lcg: u64,
}

impl Bottleneck {
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
        if lcg(&mut self.lcg) % 1000 < LOSS_PERMILLE {
            return true;
        }
        self.wire
            .push_back((done + ONE_WAY_NS, d.seq, d.unit_id, d.kind));
        false
    }
}

/// Dönüş hattında yoldaki ACK: (varış anı, UDP gövdesi, teyit, kanıt aralığı).
type InFlightAck = (u64, u64, AckInfo, Option<(u64, u64)>);

struct ReturnLine {
    free_at: u64,
    loss: AckLoss,
    lcg: u64,
    wire: VecDeque<InFlightAck>,
    sent: u64,
    lost: u64,
    wire_bytes: u64,
}

impl ReturnLine {
    fn send(&mut self, now: u64, udp: u64, ack: AckInfo, evidence: Option<(u64, u64)>) {
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        self.sent += 1;
        self.wire_bytes += udp + OVH;
        // Kayıp hattı kullandıktan sonra (serileşme bedeli ödenir).
        let index = self.sent - 1;
        let drop = match self.loss {
            AckLoss::None => false,
            AckLoss::Random(p) => lcg(&mut self.lcg) % 1000 < p,
            AckLoss::ConsecutivePairs => matches!(index % 4, 1 | 2),
        };
        if drop {
            self.lost += 1;
            return;
        }
        self.wire
            .push_back((self.free_at + ONE_WAY_NS, udp, ack, evidence));
    }
}

#[derive(Debug, Default)]
struct Outcome {
    file_done_ns: Option<u64>,
    voice_offered: u64,
    voice_undelivered_settled: usize,
    parity_stale: usize,
    retx_total: usize,
    retx_unneeded: usize,
    pto_total: usize,
    pto_unneeded: usize,
    orphaned: u64,
    acks_sent: u64,
    acks_lost: u64,
    ack_wire_max: u64,
    ack_wire_bytes: u64,
    bitmap_frames: u64,
    end_ns: u64,
    inflight_end: usize,
    file_recovery_max_ns: u64,
}

fn sender() -> Sender {
    let mut s = Sender::with_cc_port();
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
    s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
        .expect("manuel FEC kipi");
    s.set_diag_emit_log(true);
    s
}

#[allow(clippy::too_many_lines, deprecated)] // `enqueue_fec_parity`: cc_port_sim ile aynı düzenek yolu
fn run(arm: Arm) -> Outcome {
    let mut s = sender();
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    r.set_ack_bitmap(arm.bitmap);
    let mut fwd = Bottleneck {
        rate_bps: arm.real_up,
        busy: VecDeque::new(),
        wire: VecDeque::new(),
        lcg: arm.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5151,
    };
    let mut ret = ReturnLine {
        free_at: 0,
        loss: arm.ack_loss,
        lcg: arm.seed.wrapping_mul(0xD1B5_4A32_D192_ED03) ^ 0xA5A5,
        wire: VecDeque::new(),
        sent: 0,
        lost: 0,
        wire_bytes: 0,
    };
    let mut o = Outcome::default();
    let mut sent_at: HashMap<u64, u64> = HashMap::new();
    let mut dropped: HashSet<u64> = HashSet::new();
    let mut file_first: HashMap<u64, (u64, bool)> = HashMap::new();
    let mut delivered_at: HashMap<u64, u64> = HashMap::new();
    let emit = |decs: Vec<TransmitDecision>,
                now: u64,
                fwd: &mut Bottleneck,
                sent_at: &mut HashMap<u64, u64>,
                dropped: &mut HashSet<u64>,
                file_first: &mut HashMap<u64, (u64, bool)>| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            sent_at.insert(d.seq, now);
            let lost = fwd.send(now, &d);
            if lost {
                dropped.insert(d.seq);
            }
            if d.kind == TransmitKind::Data && d.unit_id >= FILE_BASE_ID {
                file_first.entry(d.unit_id).or_insert((now, lost));
            }
        }
    };
    for i in 0..FILE_UNITS {
        let id = FILE_BASE_ID + i;
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id,
                size: FILE_UNIT,
            }),
            0,
        );
        emit(
            decs,
            0,
            &mut fwd,
            &mut sent_at,
            &mut dropped,
            &mut file_first,
        );
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
    let mut tx_datagrams = 0u64;
    let mut now = 0u64;
    while now < T_MAX_NS {
        let mut decs = Vec::new();
        let voice_at = VOICE_START_NS + next_voice * VOICE_PERIOD_NS;
        if o.file_done_ns.is_none() && now >= voice_at {
            decs.extend(s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: VOICE_UNIT,
                }),
                now,
            ));
            voice_offer_at.push(now);
            next_voice += 1;
        }
        decs.extend(s.on_event(Event::TimerFired(Timer::Pto), now));
        if now.is_multiple_of(200 * MS) {
            decs.extend(s.on_event(Event::TimerFired(Timer::RtpropProbe), now));
        }
        tx_datagrams += decs
            .iter()
            .filter(|d| d.kind != TransmitKind::CsProbe)
            .count() as u64;
        emit(
            decs,
            now,
            &mut fwd,
            &mut sent_at,
            &mut dropped,
            &mut file_first,
        );
        r.set_uplink_yield(ret.free_at > now);
        let mut frames = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, kind) = fwd.wire.pop_front().expect("var");
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
            let bytes = frame.encode();
            let udp = bytes.len() as u64 + ACK_SEAL;
            o.ack_wire_max = o.ack_wire_max.max(udp + OVH);
            // Gönderici tarafı: teldeki baytları çözer (bitmap biçimi açık uç).
            let got = AckFrame::decode_with_bitmap(&bytes).expect("kendi çerçevesi çözülür");
            o.bitmap_frames += u64::from(bytes[0] == FRAME_TYPE_ACK_BITMAP);
            let expected = r.largest_seq().unwrap_or(0) + 1;
            ret.send(
                now,
                udp,
                AckInfo {
                    acked_seqs: got.acked_full(expected),
                    ack_delay: got.ack_delay_duration(),
                },
                got.evidence_span_full(expected),
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, udp, ack, evidence) = ret.wire.pop_front().expect("var");
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: evidence.is_none(),
                    sent_datagrams: tx_datagrams,
                },
                now,
            );
            if evidence.is_some() {
                s.set_next_ack_evidence(evidence);
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(
                decs,
                now,
                &mut fwd,
                &mut sent_at,
                &mut dropped,
                &mut file_first,
            );
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
                _ => {}
            }
        }
        let voice_left = (0..next_voice).any(|v| !delivered_at.contains_key(&v));
        if o.file_done_ns.is_some() && !voice_left {
            break;
        }
        now += MS;
    }
    o.end_ns = now;
    o.voice_offered = next_voice;
    o.voice_undelivered_settled = voice_offer_at
        .iter()
        .enumerate()
        .filter(|&(v, &t)| t + SETTLE_NS <= now && !delivered_at.contains_key(&(v as u64)))
        .count();
    let inflight = s.diag_inflight_seqs();
    o.inflight_end = inflight.len();
    o.parity_stale = inflight
        .iter()
        .filter(|&&(seq, _, _, kind)| {
            kind == TransmitKind::FecParity
                && sent_at
                    .get(&seq)
                    .is_some_and(|&t| now.saturating_sub(t) >= PARITY_STALE_NS)
        })
        .count();
    o.file_recovery_max_ns = file_first
        .iter()
        .filter(|(_, &(_, lost))| lost)
        .filter_map(|(u, &(t0, _))| delivered_at.get(u).map(|&t1| t1 - t0))
        .max()
        .unwrap_or(0);
    o.orphaned = s.diag_ack_evidence_orphaned();
    o.acks_sent = ret.sent;
    o.acks_lost = ret.lost;
    o.ack_wire_bytes = ret.wire_bytes;
    o
}

fn secs(ns: u64) -> String {
    format!("{:.2}", ns as f64 / SEC as f64)
}

/// Bitmap biçimi, iki gerçek hat (dürüst 12 500; iyimser profil altında 6 250), dönüşte ACK kaybı
/// yok / tohumlu %30 / ardışık çiftler (dört ACK'ten ikisi arka arkaya), iki tohum. Her kolda:
/// * dosya biter ve koşu sonundan ≥ 3 s önce sunulmuş her kısa birim teslim edilir (canlılık);
/// * onarım sembolü uçuşta birikmez;
/// * her ACK datagramı bugünkü tel tavanını (129 B) aşmaz ve dönüş hattının ACK baytı bütçeyi
///   (Y = %25) aşmaz;
/// * ACK kaybı yokken kapsam boşluğu oluşmaz (kural hiçbir kaydı açmaz).
///
/// V2 (0x14, spec §2.1 alıcısı) aynı senaryolarda yalnız GÖZLEM olarak yazdırılır.
#[test]
fn bitmap_stays_live_under_return_ack_loss() {
    let mut failures: Vec<String> = Vec::new();
    let mut orphaned_under_loss = 0u64;
    for real_up in [6_250u64, PROFILE_UP] {
        for ack_loss in [
            AckLoss::None,
            AckLoss::Random(300),
            AckLoss::ConsecutivePairs,
        ] {
            for seed in 1..=2u64 {
                for bitmap in [false, true] {
                    let arm = Arm {
                        bitmap,
                        real_up,
                        ack_loss,
                        seed,
                    };
                    let o = run(arm);
                    let tag = format!(
                        "{} hat={real_up} ack_kaybı={ack_loss} tohum={seed}",
                        if bitmap { "bitmap" } else { "v2" }
                    );
                    println!(
                        "BITMAP-DÖNÜŞ-KAYBI {tag}: dosya={} kısa_birim_teslimsiz={}/{} \
                         uçuş_sonu={} bayat_parite={} gereksiz_arq={}/{} gereksiz_pto={}/{} \
                         boşluk_açılan={} ack={} (kayıp {}) ack_tel_max={} B bitmap_çerçeve={} \
                         dosya_kurtarma_max={} son={}",
                        o.file_done_ns.map_or("YOK".into(), secs),
                        o.voice_undelivered_settled,
                        o.voice_offered,
                        o.inflight_end,
                        o.parity_stale,
                        o.retx_unneeded,
                        o.retx_total,
                        o.pto_unneeded,
                        o.pto_total,
                        o.orphaned,
                        o.acks_sent,
                        o.acks_lost,
                        o.ack_wire_max,
                        o.bitmap_frames,
                        secs(o.file_recovery_max_ns),
                        secs(o.end_ns),
                    );
                    if !bitmap {
                        continue;
                    }
                    if ack_loss == AckLoss::ConsecutivePairs {
                        orphaned_under_loss += o.orphaned;
                        if o.orphaned == 0 {
                            failures
                                .push(format!("{tag}: ardışık ACK kaybı kapsam boşluğu üretmedi"));
                        }
                    }
                    // Dönüş bütçesi: Y = 250‰ × 62 B/s; kova tavanı (iki tam ACK) kadar pay.
                    let budget = PROFILE_DOWN * 250 / 1000 * o.end_ns / SEC
                        + 2 * ACK_DATAGRAM_MAX_LINE_BYTES as u64;
                    let checks = [
                        (o.bitmap_frames > 0, "bitmap çerçevesi üretilmedi"),
                        (o.file_done_ns.is_some(), "dosya bitmedi"),
                        (o.voice_undelivered_settled == 0, "teslimsiz kısa birim"),
                        (o.parity_stale == 0, "onarım sembolü uçuşta birikti"),
                        (
                            o.ack_wire_max <= ACK_DATAGRAM_MAX_LINE_BYTES as u64,
                            "ACK datagramı tel tavanını aştı",
                        ),
                        (o.ack_wire_bytes <= budget, "ACK baytı dönüş bütçesini aştı"),
                        (
                            ack_loss != AckLoss::None || o.orphaned == 0,
                            "ACK kaybı yokken kapsam boşluğu",
                        ),
                    ];
                    for (ok, what) in checks {
                        if !ok {
                            failures.push(format!("{tag}: {what}"));
                        }
                    }
                }
            }
        }
    }
    // Test boş değil: ACK kaybı en az bir kolda kapsam boşluğu üretti ve kural onu açtı.
    assert!(
        orphaned_under_loss > 0,
        "ACK kaybı hiç kapsam boşluğu üretmedi: kural sınanmadı"
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
