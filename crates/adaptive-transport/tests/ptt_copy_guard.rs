//! [KOPYA-KORUMASI · 2026-11-22] İleri kopyaları planlanmış güvenilir PTT kaydı, geri bildirimi aç
//! profilde kopya takvimi + bir geri bildirim ufku boyunca zaman güdümlü yeniden gönderilmez.
//!
//! Gözlenen arıza (ürün testi, dürüst hücre 12 500 / 62 B/s, tohum 1): açılış kaydı 1 + 2 kopyayla
//! Bob'a varmışken ilk ACK'i beklerken bir kez daha gidiyordu — basış 4 s'de PTO probu (hedef =
//! en küçük kimlikli teyitsiz birim = PTT şeridi), basış 0,5 s'de (RTT örneği yok) PreConverge
//! §3.3-B merdiven kopyası (kuruluş probu taşır). İkisi de kayıp kanıtı olmadan çıkar.
//!
//! Düzenek: ürünün profil kurulumu (port motoru, `FeedbackStarvedRule`, yalın PTT kuralı: PTT
//! şeridi kopyasız öncelikli). Her test aynı senaryoyu korumasız (`schedule_forward_copies`) ve
//! korumalı (`schedule_forward_copies_guarded`) koşar: korumasız kol fazlalığı gösterir (ayırt
//! edicilik), korumalı kolda fazlalık yoktur; koruma bitince PTO probu kaydı yine hedefler.

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
/// PTT şeridi kimliği (şerit 0) ve metin şeridi kimliği (öncelik sınırının altı, kopya kapsamı dışı).
const PTT_OPEN: u64 = 0;
const TEXT: u64 = 2 << 62;
const LANE1: u64 = 1 << 62;
const FILE: u64 = 3 << 62;
const COPIES: u8 = 2;
const SPACING: u64 = 20 * MS;

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

/// Kaydın ilk iletimi dışındaki iletimleri: (an, neden).
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

fn schedule(s: &mut Sender, guarded: bool, now: u64) -> bool {
    if guarded {
        s.schedule_forward_copies_guarded(PTT_OPEN, COPIES, SPACING, now)
    } else {
        s.schedule_forward_copies(PTT_OPEN, COPIES, SPACING, now)
    }
}

/// RTT örneği varken (basış anlaşmadan sonra): ACK'siz 6 s'de korumasız kayda PTO probu gider;
/// korumalı kayıt koruma boyunca yalnız iki kopyasını alır. Koruma bitince PTO yine hedefler.
#[test]
fn guarded_ptt_record_is_not_pto_probed_until_the_guard_ends() {
    let horizon = feedback_horizon_ns(PROFILE_DOWN);
    let arm = |guarded: bool, until: u64| {
        let mut s = sender();
        // Isınma: metin birimi + ACK → RTT örneği (PreConverge biter, PTO kurulur).
        let out = s.on_event(Event::AppData(OpaqueUnit { id: TEXT, size: 80 }), 0);
        let seq = out
            .iter()
            .find(|t| t.unit_id == TEXT)
            .expect("metin çıktı")
            .seq;
        run(&mut s, MS, 600 * MS);
        let _ = s.on_event(
            Event::AckReceived(AckInfo {
                acked_seqs: vec![seq],
                ack_delay: std::time::Duration::ZERO,
            }),
            600 * MS,
        );
        let t_press = SEC;
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id: PTT_OPEN,
                size: 44,
            }),
            t_press,
        );
        assert!(
            out.iter()
                .any(|t| t.unit_id == PTT_OPEN && t.kind == TransmitKind::Data),
            "açılış boş hatta hemen çıktı"
        );
        assert!(schedule(&mut s, guarded, t_press), "kopyalar planlandı");
        run(&mut s, t_press + MS, until);
        resends(&s.take_diag_emit_log(), PTT_OPEN)
    };
    // ACK aralığı ≈ 5–9 s: 6 s'ye dek ACK yok.
    let plain = arm(false, 7 * SEC);
    let guarded = arm(true, 7 * SEC);
    let probes = |v: &[(u64, EmitCause)]| v.iter().filter(|r| r.1 == EmitCause::PtoProbe).count();
    let copies = |v: &[(u64, EmitCause)]| v.iter().filter(|r| r.1 != EmitCause::PtoProbe).count();
    assert!(
        probes(&plain) >= 1,
        "korumasız: kopyalı kayda PTO probu (ayırt edicilik) {plain:?}"
    );
    assert_eq!(probes(&guarded), 0, "korumalı: PTO probu yok {guarded:?}");
    assert_eq!(
        copies(&guarded),
        usize::from(COPIES),
        "korumalı: tasarlanan iki kopya {guarded:?}"
    );
    // Koruma bitince (takvim + ufuk + RTT) kayıt teyitsizse PTO yine hedefler (canlılık).
    let late = arm(true, SEC + horizon + 20 * SEC);
    assert!(
        late.iter()
            .any(|r| r.1 == EmitCause::PtoProbe && r.0 > SEC + horizon),
        "koruma sonrası PTO hedefler {late:?}"
    );
}

/// RTT örneği yokken (basış oturum başında): korumasız kayda PreConverge merdiveni / kuruluş probu
/// ilk ACK'ten önce bir kopya daha gönderir; korumalı kayıt yalnız iki kopyasını alır.
#[test]
fn guarded_ptt_record_gets_no_ladder_copy_before_the_first_ack() {
    let arm = |guarded: bool| {
        let mut s = sender();
        let t_press = 500 * MS;
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: PTT_OPEN,
                size: 44,
            }),
            t_press,
        );
        assert!(schedule(&mut s, guarded, t_press), "kopyalar planlandı");
        // Dürüst hücrede ilk ACK ≈ +2 s: o ana dek.
        run(&mut s, t_press + MS, 3 * SEC);
        resends(&s.take_diag_emit_log(), PTT_OPEN)
    };
    let plain = arm(false);
    let guarded = arm(true);
    assert!(
        plain.len() > usize::from(COPIES),
        "korumasız: kopyaların üstüne zaman güdümlü iletim (ayırt edicilik) {plain:?}"
    );
    assert_eq!(
        guarded.len(),
        usize::from(COPIES),
        "korumalı: yalnız tasarlanan kopyalar {guarded:?}"
    );
}

/// Profil yokken (geri bildirimi aç kural kurulu değil) korumalı çağrı korumasızla aynıdır.
#[test]
fn guard_is_inert_without_the_feedback_starved_rule() {
    let arm = |guarded: bool| {
        let mut s = Sender::with_cc_port();
        s.set_forward_copy_rule(Some(ForwardCopyRule {
            below_id: LANE1,
            priority_below_id: FILE,
            copies: 0,
            spacing_ns: 0,
        }));
        s.set_diag_emit_log(true);
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: PTT_OPEN,
                size: 44,
            }),
            0,
        );
        assert!(schedule(&mut s, guarded, 0));
        run(&mut s, MS, 10 * SEC);
        s.take_diag_emit_log()
    };
    assert_eq!(arm(false), arm(true));
}
