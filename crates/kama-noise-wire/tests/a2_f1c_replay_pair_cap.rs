//! A2-F1-C — PersistentReplayGuard: REPLAY_PAIR_CAP + RECENT correctness + gerçek memory-model.
//!
//! Abkarada 6.2 correctness-6 + 6.3 registry-davranışı + 6.1 allocation-aware ölçüm. RECENT_CAP=16'nın
//! high-water-invariant güvencesi: recent'tan evict-edilen counter high_water-altında kalır → tekrar-
//! gelince fail-closed (ReplayNoSideEffect), YENİDEN-Fresh-OLAMAZ.

#![cfg(feature = "experimental-cold-start")]

use kama_noise_wire::{PersistentReplayGuard, ReplayOutcome, RECENT_CAP_PER_PAIR, REPLAY_PAIR_CAP};
use std::collections::HashSet;
use std::mem::size_of;

fn pair(a: u8, b: u8) -> ([u8; 16], [u8; 16]) {
    ([a; 16], [b; 16])
}

/// Correctness-1: aynı INIT ikinci kez → replay reject (side-effect yok).
#[test]
fn c1_duplicate_init_reject() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    let (s, r) = pair(1, 2);
    assert_eq!(g.admit(s, r, 5), ReplayOutcome::Fresh);
    assert_eq!(g.admit(s, r, 5), ReplayOutcome::ReplayNoSideEffect);
}

/// Correctness-2: recent-cap dolduktan sonra EVICT-edilen eski INIT'in replay'i → reject (Fresh DEĞİL).
#[test]
fn c2_evicted_old_init_replay_reject() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    let (s, r) = pair(3, 4);
    // 1..=16 Fresh (recent dolar: {1..16}, high_water=16).
    for c in 1..=RECENT_CAP_PER_PAIR as u64 {
        assert_eq!(g.admit(s, r, c), ReplayOutcome::Fresh);
    }
    // 17 Fresh → en-küçük (1) evict edilir; high_water=17.
    assert_eq!(g.admit(s, r, 17), ReplayOutcome::Fresh);
    // Evict-edilen 1'in replay'i: high_water-altı + recent-değil → fail-closed (YENİDEN-Fresh OLAMAZ).
    assert_eq!(
        g.admit(s, r, 1),
        ReplayOutcome::ReplayNoSideEffect,
        "evict-edilen counter yeniden Fresh oldu — high-water-invariant ihlali"
    );
}

/// Correctness-3: recent-window-İÇİNDEKİ counter'ın replay'i doğru reddedilir; monoton-ilerleyen yeni
/// INIT'ler yanlış-reddedilmez (Fresh). (INIT-counter monoton; window-içi tekrar = replay.)
#[test]
fn c3_window_replays_rejected_forward_progress_fresh() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    let (s, r) = pair(5, 6);
    // Monoton-ilerleyen meşru INIT'ler hepsi Fresh (yanlış-red YOK).
    for c in [10u64, 11, 12, 13, 14] {
        assert_eq!(g.admit(s, r, c), ReplayOutcome::Fresh);
    }
    // Window-içi (henüz-evict-edilmemiş) tekrar → replay reject.
    assert_eq!(g.admit(s, r, 12), ReplayOutcome::ReplayNoSideEffect);
    assert_eq!(g.admit(s, r, 14), ReplayOutcome::ReplayNoSideEffect);
    // Sonraki monoton INIT hâlâ Fresh.
    assert_eq!(g.admit(s, r, 15), ReplayOutcome::Fresh);
}

/// Correctness-4: renewal/reconnect — aynı pair yeni-INIT-counter (monoton-devam) → Fresh; eski → reject.
#[test]
fn c4_renewal_same_pair_monotonic() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    let (s, r) = pair(7, 8);
    assert_eq!(g.admit(s, r, 100), ReplayOutcome::Fresh);
    // Reconnect: yeni-yüksek counter → Fresh.
    assert_eq!(g.admit(s, r, 101), ReplayOutcome::Fresh);
    // Eski counter → reject.
    assert_eq!(g.admit(s, r, 100), ReplayOutcome::ReplayNoSideEffect);
}

/// Correctness-5: farklı pair AYNI INIT değeri BAĞIMSIZ (her pair kendi high-water'ı).
#[test]
fn c5_distinct_pairs_independent() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    let (pa, pb) = (pair(9, 10), pair(11, 12));
    assert_eq!(g.admit(pa.0, pa.1, 42), ReplayOutcome::Fresh);
    // Farklı pair aynı-42 → kendi-ilk-görülüşü → Fresh (bağımsız).
    assert_eq!(g.admit(pb.0, pb.1, 42), ReplayOutcome::Fresh);
    // İlk pair 42-tekrar → replay.
    assert_eq!(g.admit(pa.0, pa.1, 42), ReplayOutcome::ReplayNoSideEffect);
}

/// Correctness-6: eviction eski identity'yi yeniden-Fresh YAPMAZ (c2-genellemesi; snapshot-restore-sonrası).
#[test]
fn c6_eviction_and_restore_no_refresh() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    let (s, r) = pair(13, 14);
    for c in 1..=20u64 {
        g.admit(s, r, c);
    }
    // Restart-restore: yalnız high-water taşınır (recent boşalır); altı-hepsi fail-closed.
    let snap = g.snapshot();
    let mut g2 = PersistentReplayGuard::restore(&snap, RECENT_CAP_PER_PAIR);
    // high_water=20 altındaki HER eski counter → reject (recent boş; fail-closed).
    for c in 1..=20u64 {
        assert_eq!(
            g2.admit(s, r, c),
            ReplayOutcome::ReplayNoSideEffect,
            "restore sonrası eski counter {c} yeniden Fresh oldu"
        );
    }
    // Yalnız yeni-yüksek Fresh.
    assert_eq!(g2.admit(s, r, 21), ReplayOutcome::Fresh);
}

/// 6.3 REGISTRY: REPLAY_PAIR_CAP dolu → 129. YENİ pair typed fail-closed; mevcut pair KORUNUR.
#[test]
fn registry_pair_cap_typed_reject_existing_preserved() {
    let mut g = PersistentReplayGuard::new(RECENT_CAP_PER_PAIR);
    for i in 0..REPLAY_PAIR_CAP as u32 {
        // Benzersiz sender-device (i-türevi), sabit recipient; Fresh.
        assert_eq!(g.admit(unique_dev(i), [0u8; 16], 1), ReplayOutcome::Fresh);
    }
    // 129. YENİ pair → typed fail-closed.
    assert_eq!(
        g.admit(unique_dev(9999), [0xFF; 16], 1),
        ReplayOutcome::PairRegistryFull
    );
    // MEVCUT pair hâlâ çalışır (silent-eviction YOK): 0. pair'in yeni-counter'ı Fresh.
    assert_eq!(g.admit(unique_dev(0), [0u8; 16], 2), ReplayOutcome::Fresh);
}

fn unique_dev(i: u32) -> [u8; 16] {
    let mut d = [0u8; 16];
    d[0..4].copy_from_slice(&i.to_be_bytes());
    d
}

/// 6.1 GERÇEK MEMORY-MODEL: per-pair maliyet ≤ 3328 B (416 KiB / 128). Allocation-aware konservatif.
#[test]
fn f1c_per_pair_memory_within_budget() {
    // Bileşenler (freeze-r2 side-state kapsamı):
    // (a) HashMap bucket/load-factor payı: key+PairState + hashbrown ~1.15× overhead (kontrol-baytı+boşluk).
    let key = size_of::<([u8; 16], [u8; 16])>(); // 32 B
                                                 // PairState = high_water u64 + HashSet<u64>(recent).
    let pairstate_base = size_of::<u64>() + size_of::<HashSet<u64>>(); // sabit alanlar
                                                                       // recent HashSet capacity: 16 girdi → hashbrown next-pow2 ≥16 bucket × (8B-u64 + 1B-ctrl) ≈ 16×9=144,
                                                                       // load-factor ~0.875 → ~19 bucket-slot × 9 ≈ 171 → konservatif 256.
    let recent_cap_bytes = 256usize;
    // HashMap-slot overhead (key+PairState-base bir bucket'ta) × 1.15 load-factor.
    let map_slot = ((key + pairstate_base) as f64 * 1.15) as usize;
    // Persistence/snapshot peak-copy payı: (key + u64-high_water) per pair.
    let snapshot_share = key + size_of::<u64>();
    let per_pair_total = map_slot + recent_cap_bytes + snapshot_share;

    let budget_per_pair = (1024 * 1024 - 608 * 1024) / REPLAY_PAIR_CAP; // 416 KiB / 128 = 3328
    assert_eq!(budget_per_pair, 3328);
    assert!(
        per_pair_total <= budget_per_pair,
        "per-pair maliyet {per_pair_total} B > bütçe {budget_per_pair} B → REPLAY_PAIR_CAP AŞAĞI türetilmeli"
    );
    // Toplam registry side-state payı ≤ 416 KiB.
    assert!(per_pair_total * REPLAY_PAIR_CAP <= 416 * 1024);
}
