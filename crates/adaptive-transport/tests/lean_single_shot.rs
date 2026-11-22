//! [YALIN-TEK-İLETİM · 2026-11-23] Yalın PTT demeti (tek iletim, son tarihte bırakma) geri
//! bildirimi aç profilde hiçbir yolla yeniden gönderilmez; güvenilir kayıt ve diğer birimler aynen.
//!
//! Gözlenen arıza (ürün testi, dürüst hücre 12 500 / 62 B/s, tohum 1, 8,4 s basış, yalın
//! anlaşılmış satır): Bob'a varmış bir yalın demet ilk iletiminden ≈ 0,5 s sonra PTO probuyla bir
//! kez daha gitti (hedef = en küçük kimlikli teyitsiz birim = PTT şeridi); 420 ms kolunda PTT bandı
//! 5 085 bit/s (sınır 5 000).
//!
//! Düzenek: ürünün profil kurulumu (port motoru, `FeedbackStarvedRule`, yalın PTT kuralı: PTT
//! şeridi kopyasız öncelikli). Her test aynı senaryoyu işaretsiz ve `mark_single_shot` ile
//! işaretli demetle koşar: işaretsiz kol yeniden gönderimi gösterir (ayırt edicilik), işaretli kolda
//! demet yeniden gitmez; aynı senaryodaki diğer birimin PTO probu / ARQ'su değişmez.

use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::feedback_horizon_ns;
use adaptive_transport::sender::{
    AckInfo, EmitCause, EmitDiag, Event, FeedbackStarvedRule, ForwardCopyRule, OpaqueUnit, Sender,
    Timer, TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
/// PTT şeridi (şerit 0): 0 = güvenilir açılış kaydı, 1 = yalın demet.
const PTT_OPEN: u64 = 0;
const BUNDLE: u64 = 1;
const LANE1: u64 = 1 << 62;
const TEXT: u64 = 2 << 62;
const FILE: u64 = 3 << 62;

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
    // client-core `ptt_forward_copy_rule(0, 0)`: kopyasız, PTT/SIG/TEXT öncelikli.
    s.set_forward_copy_rule(Some(ForwardCopyRule {
        below_id: LANE1,
        priority_below_id: FILE,
        copies: 0,
        spacing_ns: 0,
    }));
    s.set_diag_emit_log(true);
    s
}

fn run(s: &mut Sender, from: u64, to: u64) {
    let mut now = from;
    while now < to {
        let _ = s.on_event(Event::TimerFired(Timer::Pto), now);
        now += MS;
    }
}

fn ack(s: &mut Sender, seqs: Vec<u64>, now: u64) {
    let _ = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: seqs,
            ack_delay: std::time::Duration::ZERO,
        }),
        now,
    );
}

/// Birimin ilk iletimi dışındaki iletimleri: (an, neden).
fn resends(log: &[EmitDiag], unit: u64) -> Vec<(u64, EmitCause)> {
    log.iter()
        .filter(|e| {
            e.unit_id == unit
                && matches!(
                    e.cause,
                    EmitCause::Retransmit
                        | EmitCause::PtoProbe
                        | EmitCause::Copy
                        | EmitCause::CopyPriority
                        | EmitCause::CopyEstablish
                )
        })
        .map(|e| (e.at_ns, e.cause))
        .collect()
}

/// Isınma: metin birimi + ACK → RTT örneği (PreConverge biter, PTO kurulur).
fn warm(s: &mut Sender) {
    let out = s.on_event(Event::AppData(OpaqueUnit { id: TEXT, size: 80 }), 0);
    let seq = out
        .iter()
        .find(|t| t.unit_id == TEXT)
        .expect("metin çıktı")
        .seq;
    run(s, MS, 600 * MS);
    ack(s, vec![seq], 600 * MS);
}

/// Birimi sunar ve ilk iletimi tele çıkana dek (pacer, 1 ms adım) zamanı ilerletir; döner: (seq, an).
fn send(s: &mut Sender, id: u64, size: u64, now: u64) -> (u64, u64) {
    let mut out = s.on_event(Event::AppData(OpaqueUnit { id, size }), now);
    let mut t = now;
    loop {
        if let Some(d) = out
            .iter()
            .find(|d| d.unit_id == id && d.kind == TransmitKind::Data)
        {
            return (d.seq, t);
        }
        assert!(t < now + SEC, "birim 1 s içinde tele çıktı");
        t += MS;
        out = s.on_event(Event::TimerFired(Timer::Pto), t);
    }
}

/// ACK'siz 6 s: işaretsiz demete PTO probu gider (en küçük kimlik); işaretli demet yeniden gitmez,
/// PTO probu sıradaki teyitsiz birimi (metin) hedefler.
#[test]
fn single_shot_bundle_is_not_pto_probed_and_pto_still_serves_other_units() {
    let arm = |mark: bool| {
        let mut s = sender();
        warm(&mut s);
        let t = SEC;
        if mark {
            assert!(
                s.mark_single_shot(BUNDLE, t + SEC, t),
                "profil açık: işaretlendi"
            );
        }
        let (_, t1) = send(&mut s, BUNDLE, 234, t);
        let (_, t2) = send(&mut s, TEXT + 1, 80, t1 + MS);
        run(&mut s, t2 + MS, 7 * SEC);
        let log = s.take_diag_emit_log();
        (resends(&log, BUNDLE), resends(&log, TEXT + 1))
    };
    let (plain, _) = arm(false);
    let (marked, other) = arm(true);
    assert!(
        plain.iter().any(|r| r.1 == EmitCause::PtoProbe),
        "işaretsiz: demete PTO probu (ayırt edicilik) {plain:?}"
    );
    assert!(
        marked.is_empty(),
        "işaretli: demet yeniden gitmez {marked:?}"
    );
    assert!(
        other.iter().any(|r| r.1 == EmitCause::PtoProbe),
        "işaretli: PTO diğer teyitsiz birimi hedefler {other:?}"
    );
}

/// Kayıp kanıtı (paket eşiği: sonraki üç iletim teyitli): işaretsiz demet ARQ ile yeniden gider;
/// işaretli demet gitmez. Aynı ACK'te kaybı görünen güvenilir açılış kaydı iki kolda da ARQ alır.
#[test]
fn single_shot_bundle_loss_is_not_retransmitted_reliable_record_is() {
    let arm = |mark: bool| {
        let mut s = sender();
        warm(&mut s);
        let t = SEC;
        let (_, mut now) = send(&mut s, PTT_OPEN, 44, t);
        if mark {
            assert!(s.mark_single_shot(BUNDLE, t + SEC, now));
        }
        now = send(&mut s, BUNDLE, 234, now + MS).1;
        let mut later = Vec::new();
        for i in 0..4 {
            let (seq, at) = send(&mut s, TEXT + 1 + i, 80, now + MS);
            later.push(seq);
            now = at;
        }
        ack(&mut s, later, t + 400 * MS);
        run(&mut s, t + 401 * MS, t + 900 * MS);
        let log = s.take_diag_emit_log();
        (resends(&log, BUNDLE), resends(&log, PTT_OPEN))
    };
    let (plain, plain_open) = arm(false);
    let (marked, marked_open) = arm(true);
    assert!(
        !plain.is_empty(),
        "işaretsiz: kaybı görünen demet yeniden gider (ayırt edicilik) {plain:?}"
    );
    assert!(
        marked.is_empty(),
        "işaretli: kayıp demet yeniden gitmez {marked:?}"
    );
    assert!(
        !plain_open.is_empty(),
        "güvenilir kayıt ARQ alır {plain_open:?}"
    );
    assert_eq!(
        plain_open.len(),
        marked_open.len(),
        "güvenilir kaydın kurtarması değişmez {plain_open:?} {marked_open:?}"
    );
}

/// Profil yokken işaret kurulmaz ve yol işaretsizle aynıdır.
#[test]
fn single_shot_is_inert_without_the_feedback_starved_rule() {
    let arm = |mark: bool| {
        let mut s = Sender::with_cc_port();
        s.set_forward_copy_rule(Some(ForwardCopyRule {
            below_id: LANE1,
            priority_below_id: FILE,
            copies: 0,
            spacing_ns: 0,
        }));
        s.set_diag_emit_log(true);
        if mark {
            assert!(
                !s.mark_single_shot(BUNDLE, SEC, 0),
                "profil yok: işaret yok"
            );
        }
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: BUNDLE,
                size: 234,
            }),
            0,
        );
        run(&mut s, MS, 10 * SEC);
        s.take_diag_emit_log()
    };
    assert_eq!(arm(false), arm(true));
}
