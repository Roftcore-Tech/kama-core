//! EK4-A2 — EVENT-LOOP LIVENESS regressions.
//!
//! Contract: `reports/ek4-a2/EK4-A2-NEXT-WAKEUP-CONTRACT.md`
//! sha256 `d59212b2a40ebb1281b16c5eaa35d10541f2a83885e15f205c941719ae909f01`.
//!
//! `next_wakeup_ns()` must never return `None` while timer-driven outstanding work or an internal
//! future deadline exists, must never report a stale instant forever, and must let a purely
//! wakeup-driven driver (NO periodic tick) make progress.

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, SenderState, Timer, TransmitDecision, TransmitKind,
};
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const UNIT: u64 = 1200;

fn ack(seqs: Vec<u64>) -> Event {
    Event::AckReceived(AckInfo {
        acked_seqs: seqs,
        ack_delay: Duration::from_millis(1),
    })
}

/// Builds a flow with an established RTT and one genuinely outstanding unit below `largest_acked`,
/// using ONLY external events (no periodic tick).
fn flow_with_outstanding_gap() -> (Sender, u64) {
    let mut s = Sender::new();
    // Three units; the first will be left un-acked so it sits below largest_acked.
    let mut sent: Vec<u64> = Vec::new();
    let mut now = 0u64;
    for i in 0..3u64 {
        now = i * 10 * MS;
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1 + i,
                size: UNIT,
            }),
            now,
        ) {
            if d.kind == TransmitKind::Data {
                sent.push(d.seq);
            }
        }
    }
    // Drive emissions out with wakeup-driven steps only.
    for _ in 0..200 {
        let Some(w) = s.next_wakeup_ns() else { break };
        now = w.max(now).saturating_add(MS);
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            if d.kind == TransmitKind::Data {
                sent.push(d.seq);
            }
        }
        if sent.len() >= 3 {
            break;
        }
    }
    // ACK everything EXCEPT the first → a real gap below largest_acked.
    if sent.len() >= 2 {
        now += 100 * MS;
        let tail: Vec<u64> = sent[1..].to_vec();
        s.on_event(ack(tail), now);
    }
    (s, now)
}

/// A purely wakeup-driven driver: sleeps to `next_wakeup_ns()` and fires the timer there.
/// **No periodic tick, no new network event.** Returns `(fired_retransmit, iterations, wakeups)`.
fn drive_by_wakeup(s: &mut Sender, start: u64, budget: usize) -> (bool, usize, Vec<u64>) {
    let mut now = start;
    let mut wakeups = Vec::new();
    for i in 0..budget {
        let Some(w) = s.next_wakeup_ns() else {
            return (false, i, wakeups);
        };
        wakeups.push(w);
        now = w.max(now).saturating_add(MS);
        let out: Vec<TransmitDecision> = s.on_event(Event::TimerFired(Timer::Pto), now);
        if out
            .iter()
            .any(|d| matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate))
        {
            return (true, i + 1, wakeups);
        }
    }
    (false, budget, wakeups)
}

/// **(5) + (6)** A wakeup-driven driver with NO periodic tick produces recovery emission, and the
/// reported wakeup never sticks at one stale value.
#[test]
fn wakeup_driven_driver_makes_progress_without_periodic_tick() {
    let (mut s, t) = flow_with_outstanding_gap();
    let (fired, iters, wakeups) = drive_by_wakeup(&mut s, t, 256);
    assert!(
        fired,
        "recovery emission must occur under a purely wakeup-driven driver (no periodic tick); \
         {iters} wakeups, last few = {:?}",
        &wakeups[wakeups.len().saturating_sub(5)..]
    );
    // No busy-loop: the driver converged well inside the budget.
    assert!(iters < 256, "driver must converge, took {iters} wakeups");
}

/// **(2)** An overdue deadline is either consumed or advanced — the SAME instant is never reported
/// forever while the driver keeps waking on it.
#[test]
fn reported_wakeup_is_never_a_permanently_stale_instant() {
    let (mut s, t) = flow_with_outstanding_gap();
    let (_, _, wakeups) = drive_by_wakeup(&mut s, t, 64);
    // A stale-forever bug shows up as one value repeated for the whole budget.
    if wakeups.len() >= 8 {
        let first = wakeups[0];
        assert!(
            !wakeups.iter().all(|&w| w == first),
            "the same wakeup instant {first} was reported for all {} iterations \
             without progress (stale-deadline busy-loop)",
            wakeups.len()
        );
    }
}

/// **(6)** `None` is correct only for a genuinely idle sender, and never while work is outstanding.
#[test]
fn none_is_only_reported_when_genuinely_idle() {
    // Fresh sender, nothing enqueued → idle ⇒ None is correct.
    let s = Sender::new();
    assert_eq!(s.next_wakeup_ns(), None, "idle sender must report None");

    // With work enqueued, a wakeup must exist.
    let mut s2 = Sender::new();
    s2.on_event(Event::AppData(OpaqueUnit { id: 1, size: UNIT }), 0);
    // Enqueue more than one burst so something certainly remains pending.
    for i in 2..6u64 {
        s2.on_event(Event::AppData(OpaqueUnit { id: i, size: UNIT }), 0);
    }
    assert!(
        s2.next_wakeup_ns().is_some(),
        "outstanding queued work must expose a wakeup, never None"
    );
}

/// **(7-partial)** The accessor is observation-only: querying it repeatedly changes nothing
/// (no hidden state advance, so it cannot perturb pacing/ordering invariants).
#[test]
fn next_wakeup_is_observation_only() {
    let (mut s, t) = flow_with_outstanding_gap();
    let a = s.next_wakeup_ns();
    for _ in 0..50 {
        let _ = s.next_wakeup_ns();
    }
    let b = s.next_wakeup_ns();
    assert_eq!(a, b, "querying the wakeup must not mutate sender state");
    // And it still drives progress afterwards.
    let (fired, _, _) = drive_by_wakeup(&mut s, t, 256);
    assert!(fired, "progress must still occur after repeated queries");
}

// ── batch-API semantics (cold-start build only) ────────────────────────────────────────────────

/// **(1) + (3)** At a due time-loss deadline an EXTERNAL `Timer::Pto` really drives loss declaration,
/// and an empty batch is NOT semantically equivalent to a real PTO batch in that state.
#[cfg(feature = "experimental-cold-start")]
#[test]
fn external_pto_drives_loss_declaration_and_differs_from_empty_batch() {
    fn build() -> (Sender, u64, u64) {
        let mut s = Sender::new();
        let mut sent = Vec::new();
        let mut now = 0u64;
        for i in 0..3u64 {
            now = i * 10 * MS;
            for d in s.on_event_batch(
                now,
                vec![Event::AppData(OpaqueUnit {
                    id: 1 + i,
                    size: UNIT,
                })],
            ) {
                if d.kind == TransmitKind::Data {
                    sent.push(d.seq);
                }
            }
        }
        for _ in 0..200 {
            let Some(w) = s.next_wakeup_ns() else { break };
            now = w.max(now).saturating_add(MS);
            for d in s.on_event_batch(now, vec![Event::TimerFired(Timer::Pto)]) {
                if d.kind == TransmitKind::Data {
                    sent.push(d.seq);
                }
            }
            if sent.len() >= 3 {
                break;
            }
        }
        now += 100 * MS;
        let tail: Vec<u64> = sent[1..].to_vec();
        s.on_event_batch(now, vec![ack(tail)]);
        // Advance to a point where the time-threshold is genuinely due.
        let due = s
            .next_wakeup_ns()
            .expect("outstanding gap ⇒ a wakeup exists");
        (s, now, due)
    }

    // Real PTO at the due instant: loss must be declared (pending_lost grows or recovery emits).
    let (mut a, _, due_a) = build();
    let t_a = due_a.saturating_add(MS);
    let before_a = a.diag_loss();
    let out_a = a.on_event_batch(t_a, vec![Event::TimerFired(Timer::Pto)]);
    let after_a = a.diag_loss();
    let progressed = after_a.1 > before_a.1
        || after_a.2 > before_a.2
        || out_a
            .iter()
            .any(|d| matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate));
    assert!(
        progressed,
        "an external Timer::Pto at a due time-loss deadline must drive loss declaration \
         (before={before_a:?} after={after_a:?})"
    );

    // Same state, empty batch: must NOT be equivalent to the real PTO batch.
    let (mut b, _, due_b) = build();
    let t_b = due_b.saturating_add(MS);
    let before_b = b.diag_loss();
    let out_b = b.on_event_batch(t_b, Vec::new());
    let after_b = b.diag_loss();
    let empty_did_declare = after_b.1 > before_b.1;
    assert!(
        !(empty_did_declare && out_b.len() == out_a.len() && after_b == after_a),
        "an empty batch must not be semantically identical to a real PTO batch in a due-loss state"
    );
}

/// **(4)** An internal cold-start PTO and an external `Timer::Pto` in the SAME batch must not
/// double-process: loss declaration is idempotent within the batch.
#[cfg(feature = "experimental-cold-start")]
#[test]
fn internal_and_external_pto_in_one_batch_do_not_double_process() {
    let mut s = Sender::new();
    let mut sent = Vec::new();
    let mut now = 0u64;
    for i in 0..3u64 {
        now = i * 10 * MS;
        for d in s.on_event_batch(
            now,
            vec![Event::AppData(OpaqueUnit {
                id: 1 + i,
                size: UNIT,
            })],
        ) {
            if d.kind == TransmitKind::Data {
                sent.push(d.seq);
            }
        }
    }
    for _ in 0..200 {
        let Some(w) = s.next_wakeup_ns() else { break };
        now = w.max(now).saturating_add(MS);
        for d in s.on_event_batch(now, vec![Event::TimerFired(Timer::Pto)]) {
            if d.kind == TransmitKind::Data {
                sent.push(d.seq);
            }
        }
        if sent.len() >= 3 {
            break;
        }
    }
    now += 100 * MS;
    s.on_event_batch(now, vec![ack(sent[1..].to_vec())]);

    // Drive to a due instant, then deliver an external PTO while internal CS timers may also fire.
    let due = s.next_wakeup_ns().expect("gap ⇒ wakeup");
    let t = due.saturating_add(MS);
    let out = s.on_event_batch(t, vec![Event::TimerFired(Timer::Pto)]);

    // No unit may be retransmitted twice in one batch.
    let mut ids: Vec<u64> = out
        .iter()
        .filter(|d| matches!(d.kind, TransmitKind::Retransmit | TransmitKind::Duplicate))
        .map(|d| d.unit_id)
        .collect();
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        n,
        ids.len(),
        "duplicate recovery emission for the same unit in one batch"
    );

    // Seq numbers within the batch are unique (no nonce reuse / double plan).
    let mut seqs: Vec<u64> = out.iter().map(|d| d.seq).collect();
    let m = seqs.len();
    seqs.sort_unstable();
    seqs.dedup();
    assert_eq!(m, seqs.len(), "duplicate seq emitted within one batch");
}

/// PreConverge freeze is untouched: no time-threshold loss declaration there (§3.3-C).
#[test]
fn preconverge_still_freezes_time_threshold_declaration() {
    let mut s = Sender::new();
    s.on_event(Event::AppData(OpaqueUnit { id: 1, size: UNIT }), 0);
    assert_eq!(s.state(), SenderState::PreConverge);
    for t in 1..=20u64 {
        s.on_event(Event::TimerFired(Timer::Pto), t * SEC);
    }
    assert_eq!(
        s.state(),
        SenderState::PreConverge,
        "no ACK ⇒ PreConverge holds"
    );
    assert_eq!(
        s.diag_loss().1,
        0,
        "time-threshold declaration is frozen in PreConverge"
    );
}

/// [E1 (e-dar), ISEMRI-YOL3-M2 KK-M2-16] `e1_probe_keep` açıkken bekleyen kayıp ProbeUp'ı kesmez;
/// `pending_work_ready_ns` "bekleyen kayıp her turda planlanır" varsayımına dayanır (sender.rs
/// `pending_work_ready_ns` yorumu) — (e-dar) retx koşulu (`!pending_lost.is_empty() ∧ state ≠
/// RecoverFwd`) bu varsayımı ProbeUp'ta da korur. Kapalı döngüyle ProbeUp'a gelinir, ProbeUp'ta
/// gönderilen ilk DATA düşürülür; bekleyen kayıp ProbeUp'ta görülünce periyodik tick ve ACK kesilir,
/// yalnız `next_wakeup_ns()` güdümlü sürücü: uyanış raporlanır ve retx ProbeUp içinde tele çıkar.
#[test]
fn e1_probe_keep_pending_in_probe_up_reports_wakeup_and_plans_retx() {
    use adaptive_transport::rate::E1Flags;
    let mut s = Sender::new();
    s.set_e1_flags(E1Flags {
        e1_probe_keep: true,
        ..E1Flags::default()
    });
    let owd = 150 * MS;
    let rate_bps = 12_500u64;
    let mut link_free = 0u64;
    let mut acks: Vec<(u64, u64)> = Vec::new();
    let mut dropped = false;
    for i in 0..400u64 {
        let _ = s.on_event(
            Event::AppData(OpaqueUnit {
                id: 1_000 + i,
                size: UNIT,
            }),
            0,
        );
    }
    let mut send = |s: &Sender,
                    out: Vec<TransmitDecision>,
                    now: u64,
                    acks: &mut Vec<(u64, u64)>,
                    dropped: &mut bool| {
        for d in out {
            if !*dropped && d.kind == TransmitKind::Data && s.state() == SenderState::ProbeUp {
                *dropped = true;
                continue;
            }
            let start = now.max(link_free);
            link_free = start + d.size * SEC / rate_bps;
            let at = link_free + 2 * owd;
            let pos = acks.partition_point(|&(t, _)| t <= at);
            acks.insert(pos, (at, d.seq));
        }
    };
    let mut found = None;
    'outer: for t_ms in 0..60_000u64 {
        let now = t_ms * MS;
        while let Some(&(t, seq)) = acks.first() {
            if t > now {
                break;
            }
            acks.remove(0);
            let out = s.on_event(ack(vec![seq]), t);
            send(&s, out, t, &mut acks, &mut dropped);
            if s.state() == SenderState::ProbeUp && s.diag_loss().2 > 0 {
                found = Some(t);
                break 'outer;
            }
        }
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        send(&s, out, now, &mut acks, &mut dropped);
        if s.state() == SenderState::ProbeUp && s.diag_loss().2 > 0 {
            found = Some(now);
            break;
        }
        if t_ms % 200 == 0 {
            let out = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            send(&s, out, now, &mut acks, &mut dropped);
        }
    }
    let start = found.expect("kurulum: ProbeUp'ta bekleyen kayıp görüldü (e-dar açık)");
    assert!(dropped, "kurulum: ProbeUp'ta DATA düşürüldü");
    // Yalnız uyanış güdümlü sürücü (periyodik tick ve ACK yok).
    let mut now = start;
    let mut retx_in_probe = false;
    for _ in 0..200 {
        let w = s
            .next_wakeup_ns()
            .expect("ProbeUp'ta bekleyen kayıp varken uyanış raporlanır");
        now = w.max(now).saturating_add(MS);
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        if out.iter().any(|d| d.kind == TransmitKind::Retransmit) {
            retx_in_probe = s.state() == SenderState::ProbeUp;
            break;
        }
    }
    assert!(retx_in_probe, "retx ProbeUp içinde planlanır ve tele çıkar");
}

/// [E2a m1 · ISEMRI-YOL3-M1 KK-M1-12] `tt_rfc` açıkken zaman-kayıp uyanışı YENİ eşikte gelir
/// (`send + max(9/8·max(srtt, latest_rtt), g)`) ve yalnız uyanışla sürülen sürücü (periyodik tik
/// YOK) kaybı ilan edip kurtarma emisyonu üretir. Kapalı kolda uyanış eski eşikte (cadence tabanı
/// 1,5 × EAI ≥ 3 s) kalır — aynı akış, tek fark bayrak.
#[test]
fn e2a_m1_wakeup_at_rfc_threshold_and_wakeup_driven_progress() {
    use adaptive_transport::loss::E2aFlags;
    let flow = |tt_rfc: bool| {
        let mut s = Sender::new();
        if tt_rfc {
            s.set_e2a_flags(E2aFlags::new(true, MS, false, false, 1, false).expect("geçerli"));
        }
        let zero = |seqs: Vec<u64>| {
            Event::AckReceived(AckInfo {
                acked_seqs: seqs,
                ack_delay: Duration::ZERO,
            })
        };
        // Isınma: seq 0 @0, ACK @10 ms → örnek 10 ms.
        s.on_event(
            Event::AppData(OpaqueUnit {
                id: 200,
                size: UNIT,
            }),
            0,
        );
        s.on_event(zero(vec![0]), 10 * MS);
        // seq 1, 2 @20 ms; yalnız seq 2 ACK'lenir @30 ms (örnek 10 ms) → seq 1 boşlukta.
        let mut sent = Vec::new();
        for id in [1u64, 2] {
            for d in s.on_event(Event::AppData(OpaqueUnit { id, size: 100 }), 20 * MS) {
                if d.kind == TransmitKind::Data {
                    sent.push(d.seq);
                }
            }
        }
        assert_eq!(sent, vec![1, 2], "iki orijinal @20 ms");
        s.on_event(zero(vec![2]), 30 * MS);
        s
    };

    let mut on = flow(true);
    let srtt = on.path_timing_snapshot().srtt_ns.expect("srtt");
    let latest = 10 * MS; // son örnek (raw 10 ms, ack_delay 0)
    let expected = 20 * MS + (srtt.max(latest) * 9 / 8).max(MS);
    assert_eq!(
        on.next_wakeup_ns(),
        Some(expected),
        "açık: uyanış = send + max(9/8·max(srtt, latest_rtt), g)"
    );
    let off = flow(false);
    let w_off = off.next_wakeup_ns().expect("kapalı kolda da uyanış var");
    assert!(
        w_off > 20 * MS + 3 * SEC,
        "kapalı: uyanış eski eşikte (cadence tabanı); {w_off}"
    );

    // Yalnız uyanışla sür: ilk uyanışta seq 1 zaman eşiğiyle ilan edilir, kurtarma çıkar.
    let (fired, iters, wakeups) = drive_by_wakeup(&mut on, 30 * MS, 64);
    assert_eq!(on.diag_loss().1, 1, "açık: zaman eşiği ilanı (seq 1)");
    assert!(
        fired,
        "açık: uyanış güdümlü sürücü kurtarma emisyonu üretir; {iters} uyanış {wakeups:?}"
    );
    assert!(iters < 64, "yakınsar ({iters})");
}
