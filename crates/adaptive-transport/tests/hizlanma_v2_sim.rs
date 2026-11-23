//! [BULUT-B · HIZLANMA] Düşüşten sonra hızlanma ve süreli yoklama hükmü, V2 (0x14) ürün ACK yolunda:
//! gerçek toparlanma ile sahte toparlanma aynı düzenekte yan yana (ölçüm aracı; kabul kapısı değil).
//!
//! Düzenek (ayrık olay, 1 ms adım, deterministik): gerçek `Sender::with_cc_port` + gerçek
//! `Receiver::with_cc_port` (V2 çerçevesi; bitmap yok, ölçüm aralığı ve `pair_rate` yok — ürün
//! varsayılanı). Gönderici kurulumu `line_guard_sim`'inki (profil 12 500 / 62 B/s, profil pacer
//! tavanı, `FeedbackStarvedRule`); PTT kuralı yalın PTT'yi bilmeyen eşin ürün kuralı
//! (`client_core::runtime::set_ptt_legacy_copies`: 7 iletim, 200/7 ms aralık, hatla ölçekte en az
//! 6).
//!
//! * İleri hat: hız çizelgesi (serileşmenin başladığı andaki hız), 64 paket FIFO, 300 ms, serileşmeden
//!   sonra anahtarlı %20 kayıp (`splitmix64(tohum, sınıf, birim, o birimin kaçıncı iletimi)`;
//!   `starved_line_sim` `LossModel::Keyed` karmasıyla aynı), datagram başına 28 B zarf.
//! * İsteğe bağlı aşağı akış aşaması (darboğazdan sonra): durma penceresinde varışları tutar, sonra
//!   verilen hızlarla boşaltır (sahte toparlanma karşı örneği).
//! * Dönüş: 62 B/s, 300 ms; alıcı dönüş hattı doluyken bütçeli ACK çıkarmaz (`set_uplink_yield`).
//! * Yük: 0,5 s'den 200 ms'de bir 86 B PTT; 5 s'de bir 60 B metin; dosya ya hep hazır (gönderilmemiş
//!   en az 6 birim × 1 000 B) ya da verilen andan sonra sabit sunum hızında.
//!
//! Satırlar (`--nocapture`): `HZ-ACK` (her ACK: varış, korumanın tavanı önce/sonra, ölçülen hız,
//! hüküm), `HZ-SN` (1 s bölme: gerçek hat, darboğazda serileşen tel baytı sınıf başına, kuyruk bayt
//! ve süre, ms örnekli tavanın en küçüğü/en büyüğü, gönderilmemiş birim), `HZ-OZET`.

use std::collections::{HashMap, VecDeque};

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, Event, FeedbackStarvedRule, ForwardCopyLineScale, ForwardCopyRule, OpaqueUnit, Sender,
    Timer, TransmitDecision, TransmitKind,
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
const TEXT_ID: u64 = 500_000;
const FILE_BASE_ID: u64 = 1_000_000;
const FILE_UNIT: u64 = 1000;
const VOICE_UNIT: u64 = 86;
const TEXT_UNIT: u64 = 60;
const VOICE_PERIOD_NS: u64 = 200 * MS;
const VOICE_START_NS: u64 = 500 * MS;
const TEXT_PERIOD_NS: u64 = 5 * SEC;
/// Ürün kuralı: 7 iletim (6 kopya), 200/7 ms; hat ölçeğinde en az 6 iletim.
const PTT_COPIES: u8 = 6;
const PTT_MIN_COPIES: u8 = 5;
const PTT_SPACING_NS: u64 = 200 * MS / 7;
/// Kabul: toparlanmadan sonraki 3. ACK'e dek tavan bunun üstünde; dürüst düşük hatta bunun üstünde
/// kalıcı tavan yok.
const RAISED_MIN: u64 = 11_875;
const FALSE_MAX: u64 = 6_875;

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn keyed_permille(seed: u64, class: u8, id: u64, ordinal: u32) -> u64 {
    let mut h = splitmix64(seed);
    h = splitmix64(h ^ (1u64 << 8 | u64::from(class)));
    h = splitmix64(h ^ id);
    h = splitmix64(h ^ u64::from(ordinal));
    h % 1000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Class {
    Data,
    Arq,
    Ptt,
    Text,
    Fec,
    Other,
}

const CLASSES: [Class; 6] = [
    Class::Data,
    Class::Arq,
    Class::Ptt,
    Class::Text,
    Class::Fec,
    Class::Other,
];

fn class_of(d: &TransmitDecision) -> Class {
    if d.kind == TransmitKind::FecParity {
        Class::Fec
    } else if d.unit_id < TEXT_ID {
        Class::Ptt
    } else if d.unit_id < FILE_BASE_ID {
        Class::Text
    } else if d.kind == TransmitKind::Data {
        Class::Data
    } else if matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate) {
        Class::Arq
    } else {
        Class::Other
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // `RateAfter`: uygulama sınırlı kol (sonraki tur).
enum FileLoad {
    /// Gönderilmemiş en az 6 dosya birimi (veri hep hazır).
    Saturate,
    /// `from_ns`'ten sonra sabit sunum hızı (B/s, gövde); öncesinde hep hazır.
    RateAfter { from_ns: u64, bps: u64 },
}

/// Aşağı akış aşaması: (durma başı, durma sonu, [(andan, B/s)] boşalma hızları).
type Downstream = (u64, u64, Vec<(u64, u64)>);

#[derive(Debug, Clone)]
struct Plan {
    tag: &'static str,
    /// İleri hat hız çizelgesi (an, B/s tel); sıralı.
    fwd: Vec<(u64, u64)>,
    /// Aşağı akış: (durma başı, durma sonu, [(andan, B/s)] boşalma hızları).
    downstream: Option<Downstream>,
    file: FileLoad,
    t_end: u64,
    /// Toparlanma anı (yoksa sahte kol: "toparlanma" sayılan referans an).
    t_rec: u64,
    seed: u64,
    /// Kayıp bu andan sonra uygulanır (yerleşme evresi kayıpsız: gönderici 6 250'ye yerleşmiş başlar).
    loss_from: u64,
}

fn rate_at(plan: &[(u64, u64)], t: u64) -> u64 {
    plan.iter()
        .rev()
        .find(|&&(at, _)| at <= t)
        .map_or(plan[0].1, |&(_, r)| r)
}

struct Fwd {
    busy: VecDeque<u64>,
    /// (alıcıya varış, seq, birim).
    wire: VecDeque<(u64, u64, u64)>,
    ds_free: u64,
    /// (serileşme başı, bitişi, tel baytı, sınıf).
    ser: Vec<(u64, u64, u64, Class)>,
    /// (gönderim anı, kuyruk beklemesi).
    waits: Vec<(u64, u64)>,
    queue_drops: Vec<u64>,
    ordinals: HashMap<u64, u32>,
}

impl Fwd {
    fn send(&mut self, now: u64, d: &TransmitDecision, plan: &Plan) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        let ord = self.ordinals.entry(d.unit_id).or_insert(0);
        let ordinal = *ord;
        *ord += 1;
        if self.busy.len() >= QUEUE_PKTS {
            self.queue_drops.push(now);
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let wire = d.size + OVH;
        let done = start + wire * SEC / rate_at(&plan.fwd, start);
        self.busy.push_back(done);
        self.waits.push((now, start - now));
        let class = class_of(d);
        self.ser.push((start, done, wire, class));
        let key_class = u8::from(class == Class::Fec);
        if start >= plan.loss_from
            && keyed_permille(plan.seed, key_class, d.unit_id, ordinal) < LOSS_PERMILLE
        {
            return;
        }
        let out = match &plan.downstream {
            Some((from, to, drain)) => {
                let t0 = if done >= *from && done < *to {
                    *to
                } else {
                    done
                };
                let start = t0.max(self.ds_free);
                let r = drain
                    .iter()
                    .rev()
                    .find(|&&(at, _)| at <= start)
                    .map_or(u64::MAX / SEC, |&(_, r)| r);
                let o = if self.ds_free > t0 || t0 == *to {
                    start + wire * SEC / r
                } else {
                    t0
                };
                self.ds_free = o;
                o
            }
            None => done,
        };
        self.wire.push_back((out + ONE_WAY_NS, d.seq, d.unit_id));
    }
}

struct Ret {
    free_at: u64,
    wire: VecDeque<(u64, u64, AckInfo)>,
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
        below_id: TEXT_ID,
        priority_below_id: FILE_BASE_ID,
        copies: PTT_COPIES,
        spacing_ns: PTT_SPACING_NS,
    }));
    s.set_forward_copy_line_scale(Some(ForwardCopyLineScale {
        declared_bps: PROFILE_UP,
        min_copies: PTT_MIN_COPIES,
    }));
    s.set_diag_line_guard_trace(true);
    s
}

#[derive(Debug, Clone, Copy)]
struct AckRow {
    at: u64,
    pace_before: u64,
    pace_after: u64,
    rate_after: Option<u64>,
    raised: bool,
    failed: bool,
}

struct Outcome {
    acks: Vec<AckRow>,
    /// ms örnekli tavan (an, tavan).
    pace_ms: Vec<(u64, u64)>,
    fwd: Fwd,
    /// (an, gönderilmemiş birim) saniyede bir.
    unsent: Vec<(u64, usize)>,
    ptt_late: Vec<(u64, u64)>,
}

#[allow(clippy::too_many_lines)]
fn run(plan: &Plan) -> Outcome {
    let mut s = sender();
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    let mut fwd = Fwd {
        busy: VecDeque::new(),
        wire: VecDeque::new(),
        ds_free: 0,
        ser: Vec::new(),
        waits: Vec::new(),
        queue_drops: Vec::new(),
        ordinals: HashMap::new(),
    };
    let mut ret = Ret {
        free_at: 0,
        wire: VecDeque::new(),
    };
    let mut tx_datagrams = 0u64;
    let mut pace_ms = Vec::new();
    let mut unsent = Vec::new();
    let mut offered: HashMap<u64, u64> = HashMap::new();
    let mut ptt_late = Vec::new();
    let mut next_file = 0u64;
    let mut file_credit: u64 = 0;
    let mut next_voice = 0u64;
    let mut next_text = 0u64;
    let mut now = 0u64;
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Fwd, tx: &mut u64| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            *tx += 1;
            fwd.send(now, &d, plan);
        }
    };
    while now < plan.t_end {
        // Dosya sunumu.
        let saturate = match plan.file {
            FileLoad::Saturate => true,
            FileLoad::RateAfter { from_ns, .. } => now < from_ns,
        };
        if saturate {
            while s.unsent_units() < 6 {
                let id = FILE_BASE_ID + next_file;
                next_file += 1;
                let decs = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id,
                        size: FILE_UNIT,
                    }),
                    now,
                );
                emit(decs, now, &mut fwd, &mut tx_datagrams);
            }
        } else if let FileLoad::RateAfter { bps, .. } = plan.file {
            file_credit += bps; // B·ms birimi: her ms'de bps/1000 B
            while file_credit >= FILE_UNIT * 1000 {
                file_credit -= FILE_UNIT * 1000;
                let id = FILE_BASE_ID + next_file;
                next_file += 1;
                let decs = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id,
                        size: FILE_UNIT,
                    }),
                    now,
                );
                emit(decs, now, &mut fwd, &mut tx_datagrams);
            }
        }
        if now >= VOICE_START_NS + next_voice * VOICE_PERIOD_NS {
            offered.insert(next_voice, now);
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: VOICE_UNIT,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx_datagrams);
            next_voice += 1;
        }
        if now >= (next_text + 1) * TEXT_PERIOD_NS {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: TEXT_ID + next_text,
                    size: TEXT_UNIT,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx_datagrams);
            next_text += 1;
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut tx_datagrams);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut tx_datagrams);
        }
        r.set_uplink_yield(ret.free_at > now);
        let mut acks = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _)| t <= now) {
            let (_, seq, uid) = fwd.wire.pop_front().expect("var");
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: RxKind::Data,
                },
                now,
            );
            for u in out.delivered {
                if let Some(t0) = offered.remove(&u) {
                    ptt_late.push((t0, now - t0));
                }
            }
            acks.extend(out.ack);
        }
        acks.extend(r.on_ack_timer(now));
        for frame in acks {
            let expected = r.largest_seq().unwrap_or(0) + 1;
            let udp = frame.encode().len() as u64 + ACK_SEAL;
            let start = ret.free_at.max(now);
            ret.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
            ret.wire.push_back((
                ret.free_at + ONE_WAY_NS,
                udp,
                AckInfo {
                    acked_seqs: frame.acked_full(expected),
                    ack_delay: frame.ack_delay_duration(),
                },
            ));
        }
        while ret.wire.front().is_some_and(|&(t, _, _)| t <= now) {
            let (_, udp, ack) = ret.wire.pop_front().expect("var");
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: tx_datagrams,
                },
                now,
            );
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut tx_datagrams);
        }
        if let Some((p, _)) = s.diag_line_guard_pace() {
            pace_ms.push((now, p));
        }
        if now.is_multiple_of(SEC) {
            unsent.push((now, s.unsent_units()));
        }
        now += MS;
    }
    let acks = s
        .diag_line_guard_trace()
        .map(|t| AckRow {
            at: t.at_ns,
            pace_before: t.pace_before,
            pace_after: t.pace_after,
            rate_after: t.rate_after,
            raised: t.raised,
            failed: t.probe_failed,
        })
        .collect();
    Outcome {
        acks,
        pace_ms,
        fwd,
        unsent,
        ptt_late,
    }
}

fn secs(ns: u64) -> String {
    format!("{:.3}", ns as f64 / SEC as f64)
}

/// `[from, to)` içinde darboğazda serileşen tel baytı (sınıf başına; kesişen kısım orantılı).
fn wire_in(o: &Outcome, from: u64, to: u64) -> HashMap<Class, u64> {
    let mut m = HashMap::new();
    for &(a, b, w, c) in &o.fwd.ser {
        let lo = a.max(from);
        let hi = b.min(to);
        if hi > lo && b > a {
            *m.entry(c).or_insert(0) +=
                u64::try_from(u128::from(w) * u128::from(hi - lo) / u128::from(b - a))
                    .expect("sığar");
        }
    }
    m
}

/// `[from, to)` içinde gerçek hattın taşıyabileceği tel baytı.
fn capacity_in(plan: &Plan, from: u64, to: u64) -> u64 {
    let mut cap = 0u128;
    let mut t = from;
    while t < to {
        cap += u128::from(rate_at(&plan.fwd, t)) * u128::from(MS);
        t += MS;
    }
    u64::try_from(cap / u128::from(SEC)).expect("sığar")
}

/// `t` anında darboğaz kuyruğu: (bayt, boşalma süresi ns). Gönderim anı ≤ `t` olup serileşmesi
/// `t`'den sonra biten datagramlar (`ser` ve `waits` aynı sırada itilir).
fn queue_at(o: &Outcome, t: u64) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut last_done = t;
    for (k, &(a, b, w, _)) in o.fwd.ser.iter().enumerate() {
        let sent = o.fwd.waits[k].0;
        if sent <= t && b > t {
            let rem = if a >= t {
                w
            } else {
                u64::try_from(u128::from(w) * u128::from(b - t) / u128::from(b - a)).expect("sığar")
            };
            bytes += rem;
            last_done = last_done.max(b);
        }
    }
    (bytes, last_done - t)
}

struct Verdict {
    ack_rows: Vec<String>,
    /// Toparlanmadan sonraki k. ACK'te (1'den) tavan ≥ RAISED_MIN ilk kez.
    first_raised_ack: Option<usize>,
    /// ms örnekli tavanın RAISED_MIN altına son kez indiği an (t_rec sonrası); kalıcı açılma bundan
    /// sonra.
    permanent_from: Option<u64>,
    /// t_rec sonrası tavanın FALSE_MAX üstünde kesintisiz kaldığı en uzun süre (ns).
    longest_above_false: u64,
    final_pace: u64,
}

fn verdict(plan: &Plan, o: &Outcome) -> Verdict {
    let mut rows = Vec::new();
    let mut first_raised_ack = None;
    let mut k = 0usize;
    for a in &o.acks {
        let after = a.at >= plan.t_rec;
        if after {
            k += 1;
        }
        let probe_start = a.pace_after > a.pace_before && !a.raised;
        let hukum = if a.raised {
            "gecti"
        } else if a.failed {
            "basarisiz"
        } else if probe_start {
            "yoklama-basi"
        } else {
            "-"
        };
        if after && first_raised_ack.is_none() && a.pace_after >= RAISED_MIN && !probe_start {
            first_raised_ack = Some(k);
        }
        rows.push(format!(
            "HZ-ACK {} tohum={} t={} k={} tavan={}->{} hiz={} hukum={}",
            plan.tag,
            plan.seed,
            secs(a.at),
            if after { k.to_string() } else { "-".into() },
            a.pace_before,
            a.pace_after,
            a.rate_after.map_or("-".into(), |r| r.to_string()),
            hukum
        ));
    }
    let after: Vec<&(u64, u64)> = o
        .pace_ms
        .iter()
        .filter(|&&(t, _)| t >= plan.t_rec)
        .collect();
    let permanent_from = {
        let last_below = after.iter().rev().find(|&&&(_, p)| p < RAISED_MIN);
        match last_below {
            None => Some(plan.t_rec),
            Some(&&(t, _)) if t + MS < plan.t_end => Some(t + MS),
            Some(_) => None,
        }
    };
    let mut longest = 0u64;
    let mut run_from: Option<u64> = None;
    for &&(t, p) in &after {
        if p > FALSE_MAX {
            let f = *run_from.get_or_insert(t);
            longest = longest.max(t - f + MS);
        } else {
            run_from = None;
        }
    }
    Verdict {
        ack_rows: rows,
        first_raised_ack,
        permanent_from,
        longest_above_false: longest,
        final_pace: o.pace_ms.last().map_or(0, |&(_, p)| p),
    }
}

fn report(plan: &Plan, o: &Outcome) -> Verdict {
    let v = verdict(plan, o);
    for r in &v.ack_rows {
        println!("{r}");
    }
    let mut t = 0u64;
    while t + SEC <= plan.t_end {
        let w = wire_in(o, t, t + SEC);
        let tot: u64 = w.values().sum();
        let cap = capacity_in(plan, t, t + SEC);
        let (qb, qns) = queue_at(o, t + SEC);
        let win: Vec<u64> = o
            .pace_ms
            .iter()
            .filter(|&&(x, _)| x >= t && x < t + SEC)
            .map(|&(_, p)| p)
            .collect();
        let unsent = o
            .unsent
            .iter()
            .find(|&&(x, _)| x == t)
            .map_or(0, |&(_, u)| u);
        let cls: Vec<String> = CLASSES
            .iter()
            .map(|c| format!("{c:?}={}", w.get(c).copied().unwrap_or(0)))
            .collect();
        println!(
            "HZ-SN {} tohum={} t={} hat={} tel={} kullanim={} {} kuyruk_B={} kuyruk_ms={} tavan_min={} tavan_max={} bekleyen={}",
            plan.tag,
            plan.seed,
            t / SEC,
            rate_at(&plan.fwd, t),
            tot,
            tot * 1000 / cap.max(1),
            cls.join(" "),
            qb,
            qns / MS,
            win.iter().min().copied().unwrap_or(0),
            win.iter().max().copied().unwrap_or(0),
            unsent
        );
        t += SEC;
    }
    let drops_after = o
        .fwd
        .queue_drops
        .iter()
        .filter(|&&x| x >= plan.t_rec)
        .count();
    let max_wait_after = o
        .fwd
        .waits
        .iter()
        .filter(|&&(x, _)| x >= plan.t_rec)
        .map(|&(_, w)| w)
        .max()
        .unwrap_or(0);
    let in_eval = |t0: u64| t0 >= plan.t_rec && t0 + SEC < plan.t_end;
    let ptt_ok = o
        .ptt_late
        .iter()
        .filter(|&&(t0, l)| in_eval(t0) && l <= SEC)
        .count();
    let ptt_all = (0..)
        .map(|k| VOICE_START_NS + k * VOICE_PERIOD_NS)
        .take_while(|&t| t < plan.t_end)
        .filter(|&t| in_eval(t))
        .count();
    // Kararlı pencere: kalıcı açılmadan 1 s sonra, son 1 s hariç.
    let util = v.permanent_from.map(|p| {
        let from = p + SEC;
        let to = plan.t_end - SEC;
        if to <= from {
            return (0, 0);
        }
        let w: u64 = wire_in(o, from, to).values().sum();
        (w * 1000 / capacity_in(plan, from, to).max(1), to - from)
    });
    println!(
        "HZ-OZET {} tohum={} t_rec={} ilk_ack_ge_{RAISED_MIN}={} kalici_from={} kalici_sure_s={} uzun_ust_{FALSE_MAX}_s={} son_tavan={} kararli_kullanim_binde={} pencere_s={} kuyruk_dusen_sonra={} en_buyuk_bekleme_ms_sonra={} ptt_1s={}/{}",
        plan.tag,
        plan.seed,
        secs(plan.t_rec),
        v.first_raised_ack.map_or("yok".into(), |k| k.to_string()),
        v.permanent_from.map_or("yok".into(), secs),
        v.permanent_from.map_or("-".into(), |t| secs(plan.t_end - t)),
        secs(v.longest_above_false),
        v.final_pace,
        util.map_or("-".into(), |(u, _)| u.to_string()),
        util.map_or("-".into(), |(_, w)| secs(w)),
        drops_after,
        max_wait_after / MS,
        ptt_ok,
        ptt_all
    );
    v
}

/// Gönderici 6 250 B/s'ye yerleşmiş başlar: gerçek hat baştan 6 250, koruma onu başlangıç bütçesi
/// ve ilk ölçümlerle bulur; `T_R`'de (yerleşme doğrulanır: `HZ-YERLESIK`) çizelge ayrılır.
const T_R: u64 = 40 * SEC;
const T_E: u64 = 80 * SEC;
const LOW: u64 = 6_250;
/// Yerleşme evresi (kayıpsız) sonu; `T_R`'den önce en az iki ACK aralığı (≈ 8,6 s) kayıplı.
const LOSS_FROM: u64 = 20 * SEC;

/// Gerçek toparlanma: hat `T_R`'de 12 500'e döner.
fn real_recovery(seed: u64) -> Plan {
    Plan {
        tag: "gercek",
        fwd: vec![(0, LOW), (T_R, 12_500)],
        downstream: None,
        file: FileLoad::Saturate,
        t_end: T_E,
        t_rec: T_R,
        seed,
        loss_from: LOSS_FROM,
    }
}

/// Sahte toparlanma: hat sabit 6 250; `T_R`'de 3 s aşağı akış durması, 4 s 7 000 B/s, sonra
/// 11 000 B/s boşalma (yerel #175 7091092423 (iv) kolunun biçimi).
fn downstream_drain(seed: u64) -> Plan {
    Plan {
        tag: "asagi_akis",
        fwd: vec![(0, LOW)],
        downstream: Some((
            T_R,
            T_R + 3 * SEC,
            vec![(0, 11_000), (T_R + 3 * SEC, 7_000), (T_R + 7 * SEC, 11_000)],
        )),
        file: FileLoad::Saturate,
        t_end: T_E,
        t_rec: T_R,
        seed,
        loss_from: LOSS_FROM,
    }
}

/// Dürüst sabit 6 250 (aşağı akış olayı yok): sürekli yoklamanın sahte kalıcı yükseliş tabanı.
fn honest_low(seed: u64) -> Plan {
    Plan {
        tag: "durust6250",
        fwd: vec![(0, LOW)],
        downstream: None,
        file: FileLoad::Saturate,
        t_end: T_E,
        t_rec: T_R,
        seed,
        loss_from: LOSS_FROM,
    }
}

/// Tanı raporu (yalnız sayı basar; kabul kapısı değil): üç kol × tohum 1–3.
#[test]
#[ignore = "rapor: --ignored --nocapture ile koşulur"]
fn hizlanma_v2_report() {
    for seed in 1..=3 {
        for plan in [
            real_recovery(seed),
            downstream_drain(seed),
            honest_low(seed),
        ] {
            let o = run(&plan);
            let settled = o
                .pace_ms
                .iter()
                .filter(|&&(t, _)| t + 5 * SEC >= T_R && t < T_R)
                .map(|&(_, p)| p)
                .max()
                .unwrap_or(0);
            println!(
                "HZ-YERLESIK {} tohum={seed} tavan_max_son5s={settled} hiz={:?}",
                plan.tag,
                o.acks
                    .iter()
                    .rev()
                    .find(|a| a.at < T_R)
                    .and_then(|a| a.rate_after)
            );
            let _ = report(&plan, &o);
        }
    }
}
