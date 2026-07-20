//! Sprint-4e: restart-politikası (§0.7 PATCH-15). init_counter-monoton-persist + persistent-anti-replay +
//! early-data-OFF-flag + resume-YOK.

use kama_noise_wire::restart::early_data_policy;
use kama_noise_wire::{
    resume_allowed_after_restart, EarlyDataPolicy, InitCounterStore, PersistentReplayGuard,
    ReplayOutcome,
};

fn dev(x: u8) -> [u8; 16] {
    [x; 16]
}

// ── init_counter cihaz-monoton + restart-devam (asla-geri) ──
#[test]
fn init_counter_monotonic_across_restart() {
    let mut store = InitCounterStore::new();
    assert_eq!(store.next_counter(), 1);
    assert_eq!(store.next_counter(), 2);
    assert_eq!(store.next_counter(), 3);
    let persisted = store.snapshot();
    assert_eq!(persisted, 3);

    // Process-restart: persisted-high-water'dan devam (asla-geri-sarmaz → nonce/replay-güvenli).
    let mut restarted = InitCounterStore::restore(persisted);
    assert_eq!(restarted.next_counter(), 4, "restart-sonrası monoton-devam");
    assert!(restarted.next_counter() > persisted);
}

// ── persistent-anti-replay: aynı (device,counter) restart-sonrası da ReplayNoSideEffect ──
#[test]
fn persistent_replay_across_restart() {
    let mut guard = PersistentReplayGuard::new(64);
    assert_eq!(guard.admit(dev(1), dev(2), 10), ReplayOutcome::Fresh);
    assert_eq!(guard.admit(dev(1), dev(2), 11), ReplayOutcome::Fresh);
    // Aynı-counter tekrar → replay (side-effect-YOK).
    assert_eq!(
        guard.admit(dev(1), dev(2), 11),
        ReplayOutcome::ReplayNoSideEffect
    );

    // Process-restart: snapshot'tan geri-yükle (yalnız-high-water).
    let snap = guard.snapshot();
    let mut restarted = PersistentReplayGuard::restore(&snap, 64);
    // Restart-sonrası eski-counter (≤ high_water=11) → fail-closed ReplayNoSideEffect (early-data-güvenli).
    assert_eq!(
        restarted.admit(dev(1), dev(2), 11),
        ReplayOutcome::ReplayNoSideEffect
    );
    assert_eq!(
        restarted.admit(dev(1), dev(2), 5),
        ReplayOutcome::ReplayNoSideEffect
    );
    // Yeni-counter (> high_water) → Fresh.
    assert_eq!(restarted.admit(dev(1), dev(2), 12), ReplayOutcome::Fresh);
}

// ── recent-set bounded (DoS-guard): cap aşınca eski-evict, yine monoton-fresh çalışır ──
#[test]
fn replay_guard_bounded() {
    let mut guard = PersistentReplayGuard::new(4);
    for c in 1..=10 {
        assert_eq!(guard.admit(dev(1), dev(2), c), ReplayOutcome::Fresh);
    }
    // En-yüksek 4 recent'te; en-yeni tekrar → replay.
    assert_eq!(
        guard.admit(dev(1), dev(2), 10),
        ReplayOutcome::ReplayNoSideEffect
    );
    // Farklı-cihaz-çifti bağımsız.
    assert_eq!(guard.admit(dev(3), dev(4), 1), ReplayOutcome::Fresh);
}

// ── early-data: persistence-YOK → DEVRE-DIŞI (fail-safe) ──
#[test]
fn early_data_off_without_persistence() {
    assert_eq!(early_data_policy(true), EarlyDataPolicy::Enabled);
    assert_eq!(early_data_policy(false), EarlyDataPolicy::Disabled);
}

// ── RESUME-YOK: restart-sonrası eski-session resume-EDİLEMEZ (§15) ──
#[test]
fn no_resume_after_restart() {
    assert!(
        !resume_allowed_after_restart(),
        "restart = yeni-session/handshake ZORUNLU (resume-YOK)"
    );
}
