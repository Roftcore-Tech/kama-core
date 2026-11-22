//! [BOŞ-HAT YENİDEN İLETİMİ · #175 7069515224] Geri bildirimi aç profilde hat boşken teyitsiz
//! birimin önleyici yeniden iletimi.
//!
//! Düzenek: ürünün geri bildirimi aç profil kurulumu (12 500 / 62 B/s, `FeedbackStarvedRule`,
//! önlemsel kopya kuralı PTT kimliklerinde). On dosya birimi ve bir PTT birimi gönderilir; ACK
//! gelmez (dar dönüşün ≈ 7 s'lik bütçe aralığı). Ölçülen: birim başına boş hat yeniden iletimi.

use std::collections::BTreeMap;

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::feedback_horizon_ns;
use adaptive_transport::sender::{
    AckInfo, EmitCause, EmitDiag, Event, FeedbackStarvedRule, ForwardCopyRule, OpaqueUnit, Sender,
    Timer,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const FILE_BASE_ID: u64 = 1_000_000;
const FILE_UNIT: u64 = 1046;
const N: u64 = 10;

fn sender(starved: bool) -> Sender {
    let mut s = Sender::with_cc_port();
    let t_ack = adaptive_transport::receiver::budgeted_ack_interval_ns(PROFILE_DOWN);
    s.set_cc_initial_window(PROFILE_UP * t_ack / SEC);
    let cap = PROFILE_UP * MTU / (MTU + OVH);
    s.set_cc_profile_pacing_cap(Some(cap));
    if starved {
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
    }
    s.set_forward_copy_rule(Some(ForwardCopyRule {
        below_id: FILE_BASE_ID,
        priority_below_id: FILE_BASE_ID,
        copies: 4,
        spacing_ns: 40 * MS,
    }));
    s.set_establish_probe(false);
    s
}

fn run(s: &mut Sender, from: u64, to: u64) {
    let mut now = from;
    while now < to {
        let _ = s.on_event(Event::TimerFired(Timer::Pto), now);
        now += MS;
    }
}

/// Boş hat iletimlerinin (sayaç farkına denk gelen `Copy` kayıtları) birim → anlar listesi.
fn idle_copies(log: &[EmitDiag], from_ns: u64) -> BTreeMap<u64, Vec<u64>> {
    let mut m: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for e in log
        .iter()
        .filter(|e| e.cause == EmitCause::Copy && e.at_ns >= from_ns)
    {
        m.entry(e.unit_id).or_default().push(e.at_ns);
    }
    m
}

#[test]
fn idle_line_resends_each_unconfirmed_unit_once_per_ack_interval() {
    let mut s = sender(true);
    s.set_diag_emit_log(true);
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 0, size: 86 }), 0);
    for k in 0..N {
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: FILE_BASE_ID + k,
                size: FILE_UNIT,
            }),
            0,
        );
    }
    run(&mut s, MS, 6 * SEC);
    let log = s.take_diag_emit_log();
    let data: BTreeMap<u64, (u64, u64)> = log
        .iter()
        .filter(|e| e.cause == EmitCause::Data)
        .map(|e| (e.unit_id, (e.seq, e.at_ns)))
        .collect();
    assert_eq!(data.len() as u64, N + 1, "her birim bir kez gönderildi");
    let idle = idle_copies(&log, 0);
    assert_eq!(
        s.diag_idle_retransmits(),
        N,
        "boş hatta her dosya birimi bir kez"
    );
    assert!(
        !idle.contains_key(&0),
        "PTT birimi kopya kuralının kapsamında: {idle:?}"
    );
    let mut prev = 0u64;
    for k in 0..N {
        let id = FILE_BASE_ID + k;
        let t = idle.get(&id).map(Vec::as_slice).unwrap_or_default();
        assert_eq!(t.len(), 1, "birim {k}: ACK aralığında bir kez ({t:?})");
        assert!(
            t[0] >= data[&id].1 + SEC,
            "birim {k}: son iletimden 1 s sonra"
        );
        assert!(t[0] >= prev, "en eski teyitsiz önce");
        // [YARI-HAT] Ardışık kör kopyalar arası en az iki serileşme süresi (hattın yarısı).
        if k > 0 {
            let ser = (FILE_UNIT + OVH) * SEC / PROFILE_UP;
            assert!(
                t[0] - prev >= 2 * ser,
                "birim {k}: kopya aralığı {} ns < {} ns",
                t[0] - prev,
                2 * ser
            );
        }
        prev = t[0];
    }
    // 6 s'de ACK: ilk altı dosya birimi ve PTT teyitli. Teyitliler hiç gönderilmez. [KANIT-YOK-
    // TEKRAR-YOK] Kalan dört birimin boş hat kopyası bu ACK'in teyit ettiği en büyük iletimden
    // yenidir: ACK onların akıbetini göremez, kopya kanıtsız ikinci kez gitmez (eskiden bu ACK
    // süpürmeyi sıfırlıyor, dört birim yeniden gönderiliyordu).
    let acked: Vec<u64> = std::iter::once(data[&0].0)
        .chain((0..6).map(|k| data[&(FILE_BASE_ID + k)].0))
        .collect();
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: acked,
            ack_delay: std::time::Duration::ZERO,
        }),
        6 * SEC,
    );
    run(&mut s, 6 * SEC + MS, 8 * SEC);
    let log = s.take_diag_emit_log();
    let idle = idle_copies(&log, 6 * SEC);
    assert!(
        idle.is_empty(),
        "akıbeti görülmemiş kopya yeniden gitmez: {idle:?}"
    );
    assert_eq!(s.diag_idle_retransmits(), N);
    // Kanıt: 8 s'de yeni bir PTT birimi, 9 s'de ACK'i (boş hat kopyalarından yeni iletim teyitli,
    // dört birimin kopyası teyitsiz). Dört birim yeniden gönderilir (ARQ ya da boş hat), teyitliler
    // gönderilmez.
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 1, size: 86 }), 8 * SEC);
    let log = s.take_diag_emit_log();
    let ptt2 = log
        .iter()
        .find(|e| e.cause == EmitCause::Data && e.unit_id == 1)
        .map(|e| e.seq)
        .expect("PTT birimi 2 gönderildi");
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![ptt2],
            ack_delay: std::time::Duration::ZERO,
        }),
        9 * SEC,
    );
    run(&mut s, 9 * SEC + MS, 12 * SEC);
    let log = s.take_diag_emit_log();
    for k in 0..N {
        let n = log
            .iter()
            .filter(|e| {
                e.unit_id == FILE_BASE_ID + k
                    && matches!(e.cause, EmitCause::Copy | EmitCause::Retransmit)
            })
            .count();
        assert_eq!(n > 0, k >= 6, "kanıttan sonra birim {k}: {n} iletim");
    }
}

/// [KANIT-YOK-TEKRAR-YOK] Kanıt geri bildirim ufkunda gelmezse kural kalkar: ufuktan sonra gelen
/// ACK yine akıbeti göstermese de (kopyalardan eski iletimi teyit eder) süpürme yeniden açılır.
#[test]
fn idle_copy_repeats_once_the_feedback_horizon_has_passed() {
    let mut s = sender(true);
    s.set_diag_emit_log(true);
    let _ = s.on_event(Event::AppData(OpaqueUnit { id: 0, size: 86 }), 0);
    for k in 0..N {
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: FILE_BASE_ID + k,
                size: FILE_UNIT,
            }),
            0,
        );
    }
    let horizon = feedback_horizon_ns(PROFILE_DOWN);
    run(&mut s, MS, horizon + 3 * SEC);
    assert_eq!(
        s.diag_idle_retransmits(),
        N,
        "ACK yokken birim başına bir kez"
    );
    let log = s.take_diag_emit_log();
    let ptt = log
        .iter()
        .find(|e| e.cause == EmitCause::Data && e.unit_id == 0)
        .map(|e| e.seq)
        .expect("PTT birimi gönderildi");
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: vec![ptt],
            ack_delay: std::time::Duration::ZERO,
        }),
        horizon + 3 * SEC,
    );
    run(&mut s, horizon + 3 * SEC + MS, horizon + 9 * SEC);
    assert!(
        s.diag_idle_retransmits() > N,
        "ufuktan sonra yeniden: {}",
        s.diag_idle_retransmits()
    );
}

/// Profil kapalıyken (kural yok) boş hat yeniden iletimi yoktur: bugünkü yol.
#[test]
fn no_idle_retransmit_without_the_feedback_starved_rule() {
    let mut s = sender(false);
    for k in 0..N {
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: FILE_BASE_ID + k,
                size: FILE_UNIT,
            }),
            0,
        );
    }
    run(&mut s, MS, 6 * SEC);
    assert_eq!(s.diag_idle_retransmits(), 0);
}
