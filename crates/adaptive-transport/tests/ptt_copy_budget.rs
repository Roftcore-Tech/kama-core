//! [İŞ-33 C2] Öncelikli şeridin (PTT) kopyaları uzun bir hat kuyruğunda yığılmaz ama demet
//! kopyasız da kalmaz.
//!
//! Düzenek: ürünün geri bildirimi aç profil kurulumu (12 500 / 62 B/s, `FeedbackStarvedRule`,
//! dört ACK'siz kopya, 40 ms aralık). Tek bir öncelikli birim gönderilir; hat kuyruğu
//! (`Sender::cc_line_observe_backlog`, ürün çalışma zamanının paylaşılan hat gözlemi) verilen süre
//! boyunca dolu tutulur, sonra boşalır. Ölçülen: birimin `CopyPriority` iletim sayısı.
//!
//! Bugünkü kural (2026-11-20): vadesinden 1 s'ten fazla gecikmiş kopya iptal edilir. Uzun kuyrukta
//! dört kopyanın dördü de iptal oluyordu (ürün testi, gerçek 6 250 B/s, tohum 1: demet 20, ikinci
//! iletim 17,3 s sonra ARQ'yla). Yeni kural: birimin vadesi gelmiş son kopyası düşmez.

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::feedback_horizon_ns;
use adaptive_transport::sender::{
    EmitCause, Event, FeedbackStarvedRule, ForwardCopyRule, OpaqueUnit, Sender, Timer,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const VOICE_UNIT: u64 = 86;
const FILE_BASE_ID: u64 = 1_000_000;

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
    s
}

/// Tek öncelikli birim; hat kuyruğu `[1 ms, backlog_ns)` boyunca dolu (kalan süre × profil hattı
/// bayt). 4 s sonundaki `(orijinal, kopya)` iletim sayısı.
fn transmissions(backlog_ns: u64) -> (usize, usize) {
    let mut s = sender();
    s.set_diag_emit_log(true);
    let _ = s.on_event(
        Event::AppData(OpaqueUnit {
            id: 0,
            size: VOICE_UNIT,
        }),
        0,
    );
    let mut now = MS;
    while now < 4 * SEC {
        if now < backlog_ns {
            s.cc_line_observe_backlog(PROFILE_UP * (backlog_ns - now) / SEC);
        }
        let _ = s.on_event(Event::TimerFired(Timer::Pto), now);
        now += MS;
    }
    let log = s.take_diag_emit_log();
    let count = |c: EmitCause| {
        log.iter()
            .filter(|e| e.unit_id == 0 && e.cause == c)
            .count()
    };
    (count(EmitCause::Data), count(EmitCause::CopyPriority))
}

/// Hat kuyruğu yokken dört kopyanın dördü çıkar (bugünkü davranış).
#[test]
fn copies_go_out_when_the_line_is_free() {
    assert_eq!(transmissions(0), (1, 4));
}

/// Kopyaların vadesinden (≈ 50–170 ms) kısa süre sonra açılan hatta (0,5 s; hiçbir kopya bayat
/// değil) dört kopya da çıkar: kısa kuyrukta davranış değişmez.
#[test]
fn a_short_backlog_keeps_every_copy() {
    assert_eq!(transmissions(500 * MS), (1, 4));
}

/// Kopyaların hepsi vadesinden 1 s'ten fazla gecikince (kuyruk 3 s) bugünkü kural dördünü de iptal
/// ederdi; yeni kural tek kopya bırakır ve hat açılınca o kopya çıkar: kuyruk varken kopyalar
/// yığılmaz, demet kopyasız kalmaz.
#[test]
fn a_long_backlog_leaves_exactly_one_copy() {
    assert_eq!(transmissions(3 * SEC), (1, 1));
}
