//! Commit-W — LEFTOVER-EŞDEĞERLİK-PİNİ (feature-GATE'SİZ: hem-OFF hem-ON-derlemesinde-AYNI-sabitler).
//! Kanıt-modeli: deterministik-mini-senaryo → karar-dizisi/sayaç-SABİTLERİ. OFF-koşusu=baseline;
//! ON-koşusu (`--features experimental-fec-budget`; policy-dokunulmaz=LeftoverOnly-default) AYNI-
//! sabitleri-geçmeli → "ON+LeftoverOnly ≡ OFF-baseline" karar-düzeyi-kanıtı. (Ek-kanıt: tüm-mevcut-
//! suite — N8/golden/sprint5c/M1-spot — iki-modda-da-yeşil; Commit-V-kapıları.)

//! DR-RATE-RAMP-2B SUPERSESSION (artifact `DR-RATE-RAMP-2B-ORDERING-SUPERSESSION`,
//! md `d14e35b6…`, json `7474123c…`): the ORIGINAL pin below is **preserved verbatim** — all six
//! assertions, including `last_data < first_par`, are untouched — and is **re-scoped** to the
//! startup-pacing-INACTIVE path it was captured from (`// PİN-SABİTLERİ (feature-OFF-baseline'dan-
//! ölçüldü …)`). Only the reading of `last_data < first_par` as a WHOLE-FLOW rule valid under any
//! pacing configuration is superseded; that constant is a measured trajectory, not a contract.
//! The invariants Commit-W actually sealed are re-sealed below by exact tests that assert on BOTH
//! sides of the pacing axis explicitly, so they no longer depend on whatever the default happens to
//! be. (DR-RATE-RAMP-2B §A4 subsequently reverted the startup-pacing default to OFF for an unrelated,
//! still-unresolved EK4-A2 deadline-exposure question — see `reports/dr-rate-ramp-2b/`.)

use adaptive_transport::sender::{
    AckInfo, Event, OpaqueUnit, Sender, StartupPacingConfig, Timer, TransmitDecision, TransmitKind,
};
use std::time::Duration;

const MS: u64 = 1_000_000;

#[test]
#[allow(deprecated)] // legacy-enqueue: baseline-yolun-parçası (pin-senaryosu)
fn leftover_equivalence_pin_constants() {
    let mut s = Sender::new();
    // DR-RATE-RAMP-2B: explicit re-scope to the startup-pacing-INACTIVE trajectory these constants
    // were measured from. NOT a weakening — every assertion below is unchanged.
    s.set_startup_pacing_init(StartupPacingConfig { enabled: false });
    s.set_manual_fec_enabled(true).unwrap();
    // Deterministik-mini-senaryo: 6-DATA (batch-ACK) + 3-parity-enqueue + PTO-drenajı.
    let mut decisions: Vec<(u8, u64, u64)> = Vec::new(); // (kind-tag, seq, size)
    let push = |out: &[adaptive_transport::sender::TransmitDecision],
                acc: &mut Vec<(u8, u64, u64)>| {
        for d in out {
            let k = match d.kind {
                TransmitKind::Data => 0u8,
                TransmitKind::Retransmit => 1,
                TransmitKind::Duplicate => 2,
                TransmitKind::FecParity => 3,
                // DR-CS-3: `CsProbe` is un-gated (the shipping bootstrap emitter lives in the
                // default build), so this arm must NOT be feature-gated or the match is
                // non-exhaustive under default features.
                TransmitKind::CsProbe => 4,
            };
            acc.push((k, d.seq, d.size));
        }
    };
    let mut seqs = Vec::new();
    for i in 0..6u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit {
                id: 100 + i,
                size: 1200,
            }),
            i * MS,
        );
        seqs.extend(out.iter().map(|d| d.seq));
        push(&out, &mut decisions);
    }
    let out = s.on_event(
        Event::AckReceived(AckInfo {
            acked_seqs: seqs,
            ack_delay: Duration::ZERO,
        }),
        7 * MS,
    );
    push(&out, &mut decisions);
    for esi in 0..3u16 {
        s.enqueue_fec_parity(9, esi, 400);
    }
    for t in 0..40u64 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), (10 + t * 10) * MS);
        push(&out, &mut decisions);
    }
    let d = s.diag_fec_actuation();
    // PİN-SABİTLERİ (feature-OFF-baseline'dan-ölçüldü; ON-koşusu-AYNI-değerleri-geçmeli):
    assert_eq!(d.enqueued_r_total, 3, "pin: enqueued");
    assert_eq!(d.emitted_r_total, 3, "pin: emitted (leftover-yolunda-3/3)");
    let parity_count = decisions.iter().filter(|x| x.0 == 3).count();
    let data_count = decisions.iter().filter(|x| x.0 == 0).count();
    assert_eq!((data_count, parity_count), (6, 3), "pin: karar-sayıları");
    // DATA-ÖNCE-sıra-pini: son-DATA-index < ilk-parity-index.
    let last_data = decisions.iter().rposition(|x| x.0 == 0).unwrap();
    let first_par = decisions.iter().position(|x| x.0 == 3).unwrap();
    assert!(last_data < first_par, "pin: DATA-önce-parity");
    // Karar-dizisi-özet-pini (toplam-byte + son-seq): rejim-drift'ini-yakalar.
    let total_bytes: u64 = decisions.iter().map(|x| x.2).sum();
    let max_seq = decisions.iter().map(|x| x.1).max().unwrap();
    assert_eq!(total_bytes, 6 * 1200 + 3 * 400, "pin: toplam-byte");
    assert_eq!(max_seq, 8, "pin: son-seq (6-DATA + 3-parity − 1)");
}

// ═══════════════════════════════════════════════════════════════════════════════════════════════
// DR-RATE-RAMP-2B — the invariants Commit-W actually sealed, re-sealed on BOTH sides of the axis
// ═══════════════════════════════════════════════════════════════════════════════════════════════

fn kind_tag(k: TransmitKind) -> u8 {
    match k {
        TransmitKind::Data => 0,
        TransmitKind::Retransmit => 1,
        TransmitKind::Duplicate => 2,
        TransmitKind::FecParity => 3,
        TransmitKind::CsProbe => 4,
    }
}

/// The Commit-W scenario, returning the decisions **grouped by scheduling turn** (the level at which
/// `DATA-before-PARITY` is actually defined — `sender.rs:2831`).
#[allow(deprecated)] // legacy-enqueue: part of the sealed pin scenario
fn commit_w_turns(startup_pacing: bool) -> Vec<Vec<(u8, u64, u64)>> {
    let mut s = Sender::new();
    s.set_startup_pacing_init(StartupPacingConfig {
        enabled: startup_pacing,
    });
    s.set_manual_fec_enabled(true).unwrap();
    let mut turns: Vec<Vec<(u8, u64, u64)>> = Vec::new();
    let rec = |out: &[TransmitDecision], t: &mut Vec<Vec<(u8, u64, u64)>>| {
        if !out.is_empty() {
            t.push(out.iter().map(|d| (kind_tag(d.kind), d.seq, d.size)).collect());
        }
    };
    let mut seqs = Vec::new();
    for i in 0..6u64 {
        let out = s.on_event(
            Event::AppData(OpaqueUnit { id: 100 + i, size: 1200 }),
            i * MS,
        );
        seqs.extend(out.iter().map(|d| d.seq));
        rec(&out, &mut turns);
    }
    let out = s.on_event(
        Event::AckReceived(AckInfo { acked_seqs: seqs, ack_delay: Duration::ZERO }),
        7 * MS,
    );
    rec(&out, &mut turns);
    for esi in 0..3u16 {
        s.enqueue_fec_parity(9, esi, 400);
    }
    for t in 0..40u64 {
        let out = s.on_event(Event::TimerFired(Timer::Pto), (10 + t * 10) * MS);
        rec(&out, &mut turns);
    }
    turns
}

/// **SEALED INVARIANT (scheduler-turn level).** Within any single `schedule_transmit` result, no
/// `Data` may follow a `FecParity` — `sender.rs:2831` "FEC-parity DATA/recovery SONRASI
/// (DATA-ÖNCE-sıralaması mühürlü)". This must hold on BOTH sides of the pacing axis.
#[test]
fn data_before_parity_holds_within_every_scheduler_turn() {
    for startup_pacing in [false, true] {
        for (i, turn) in commit_w_turns(startup_pacing).iter().enumerate() {
            if let Some(first_par) = turn.iter().position(|x| x.0 == 3) {
                assert!(
                    !turn[first_par..].iter().any(|x| x.0 == 0),
                    "turn {i} (startup_pacing={startup_pacing}) emits DATA after parity \
                     within one schedule_transmit: {turn:?}"
                );
            }
        }
    }
}

/// **SEALED INVARIANT (Commit-W's actual purpose).** `ON + LeftoverOnly ≡ OFF-baseline` at decision
/// level. This test pins the decision stream itself with startup pacing explicitly ON, so running it
/// in both feature builds proves the equivalence on the side the original constants did NOT cover.
#[test]
fn leftover_equivalence_decision_stream_is_feature_invariant() {
    let flat: Vec<(u8, u64, u64)> = commit_w_turns(true).into_iter().flatten().collect();
    assert_eq!(
        flat,
        vec![
            (0, 0, 1200),
            (0, 1, 1200),
            (0, 2, 1200),
            (0, 3, 1200),
            (3, 4, 400),
            (3, 5, 400),
            (0, 6, 1200),
            (0, 7, 1200),
            (3, 8, 400),
        ],
        "decision stream must be identical with and without `experimental-fec-budget`"
    );
}

/// **SEALED INVARIANT (budget).** Counts and serialized bytes are invariant to the pacing axis —
/// startup pacing changes WHEN units are granted, never HOW MANY or HOW BIG.
#[test]
fn counts_and_byte_budget_are_invariant_to_startup_pacing() {
    for startup_pacing in [false, true] {
        let flat: Vec<(u8, u64, u64)> = commit_w_turns(startup_pacing).into_iter().flatten().collect();
        let data = flat.iter().filter(|x| x.0 == 0).count();
        let parity = flat.iter().filter(|x| x.0 == 3).count();
        let bytes: u64 = flat.iter().map(|x| x.2).sum();
        let max_seq = flat.iter().map(|x| x.1).max().unwrap();
        assert_eq!((data, parity), (6, 3), "startup_pacing={startup_pacing}");
        assert_eq!(bytes, 6 * 1200 + 3 * 400, "startup_pacing={startup_pacing}");
        assert_eq!(max_seq, 8, "startup_pacing={startup_pacing}");
    }
}

/// **SEALED INVARIANT (FEC block level).** On the authoritative path, parity for a block is staged
/// only by `close_fec_block_manual`, so it can never precede that block's own source emissions —
/// under the product default. This is the property that would actually matter to a decoder.
#[test]
fn parity_never_precedes_its_own_block_source_on_the_authoritative_path() {
    const K: u64 = 8;
    const BLOCK: u32 = 3;
    let mut s = Sender::new();
    s.set_manual_fec_enabled(true).unwrap();

    let mut source_emitted = 0u64;
    let mut parity_seen = 0u64;
    let mut closed = false;
    let check = |out: &[TransmitDecision], source_emitted: &mut u64, parity_seen: &mut u64| {
        for d in out {
            match d.kind {
                TransmitKind::Data => *source_emitted += 1,
                TransmitKind::FecParity => {
                    assert!(
                        *source_emitted >= K,
                        "parity emitted after only {source_emitted}/{K} block sources"
                    );
                    *parity_seen += 1;
                }
                _ => {}
            }
        }
    };

    let mut unacked: Vec<u64> = Vec::new();
    let collect = |out: &[TransmitDecision], u: &mut Vec<u64>| {
        u.extend(out.iter().filter(|d| d.kind == TransmitKind::Data).map(|d| d.seq));
    };

    for i in 0..K {
        let out = s.on_event(
            Event::AppData(OpaqueUnit { id: 500 + i, size: 1200 }),
            i * MS,
        );
        check(&out, &mut source_emitted, &mut parity_seen);
        collect(&out, &mut unacked);
    }
    for t in 0..200u64 {
        let now = (10 + t * 10) * MS;
        // Keep the flow ACK-clocked so the block's sources can actually drain; without feedback the
        // pacer starves and the block would never close (that would test nothing).
        if !unacked.is_empty() {
            let acked = std::mem::take(&mut unacked);
            let out = s.on_event(
                Event::AckReceived(AckInfo { acked_seqs: acked, ack_delay: Duration::ZERO }),
                now,
            );
            check(&out, &mut source_emitted, &mut parity_seen);
            collect(&out, &mut unacked);
        }
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        check(&out, &mut source_emitted, &mut parity_seen);
        collect(&out, &mut unacked);
        if !closed && source_emitted >= K {
            closed = true;
            let auth = s.open_fec_block_authority();
            s.close_fec_block_manual(auth, BLOCK, 2, 400)
                .expect("authoritative close stages parity only after the block's sources");
        }
    }
    assert!(closed, "block must close once its K sources are on the wire");
    assert_eq!(parity_seen, 2, "both staged parity units must drain");
}
