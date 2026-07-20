//! Restart-politikası (§0.7 EARLY-DATA-REPLAY PATCH-15). Process-restart = YENİ-session (RESUME-YOK);
//! init_counter cihaz-başına KALICI+MONOTON; receiver persistent-anti-replay (high-water); replay-
//! persistence-yoksa production-early-data DEVRE-DIŞI. session-full-PN diske-persist-GÜVENİLMEZ.
//!
//! Deterministik; RNG-YOK. Bu-tipler persistence-SÖZLEŞMESİNİ modeller (gerçek-store engine/platform).

use std::collections::{HashMap, HashSet};

/// Cihaz-çifti anahtarı (sender_device_id, recipient_device_id).
pub type DevicePair = ([u8; 16], [u8; 16]);
/// Restart-snapshot: cihaz-çifti → high-water init_counter.
pub type ReplaySnapshot = Vec<(DevicePair, u64)>;

// ── Cihaz-monoton init_counter (KALICI) ──

/// Cihaz-başına monoton init_counter üretici. Restart'ta persisted-high-water'dan-DEVAM (asla-geri).
#[derive(Debug, Clone, Copy, Default)]
pub struct InitCounterStore {
    high_water: u64,
}

impl InitCounterStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restart: persisted-değerden geri-yükle (monoton-devam; asla-geri-sarmaz).
    #[must_use]
    pub fn restore(persisted_high_water: u64) -> Self {
        Self {
            high_water: persisted_high_water,
        }
    }

    /// Sonraki init_counter (monoton-artan). Çağıran bunu-PERSIST-etmeli (restart-öncesi).
    pub fn next_counter(&mut self) -> u64 {
        self.high_water = self.high_water.saturating_add(1);
        self.high_water
    }

    /// Persist-edilecek değer (restart-snapshot).
    #[must_use]
    pub fn snapshot(&self) -> u64 {
        self.high_water
    }
}

// ── Persistent anti-replay (high-water + recent-window; restart-survivable) ──

/// Bir replay-kontrolün sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayOutcome {
    /// İlk-görülüş: side-effect UYGULANIR.
    Fresh,
    /// Replay (görülmüş/eski): İKİNCİ-app-side-effect ÜRETİLMEZ (idempotent).
    ReplayNoSideEffect,
}

/// Cihaz-çifti başına persistent-anti-replay: high-water (en-yüksek-görülen init_counter) + recent-set
/// (high-water-altı reorder için, bounded). Restart'ta snapshot'tan-geri-yüklenir.
#[derive(Debug, Clone, Default)]
pub struct PersistentReplayGuard {
    /// (sender_device, recipient_device) → (high_water, recent-counters).
    per_pair: HashMap<([u8; 16], [u8; 16]), PairState>,
    /// recent-set üst-sınırı (TTL/bounded — DoS-guard).
    recent_cap: usize,
}

#[derive(Debug, Clone, Default)]
struct PairState {
    high_water: u64,
    recent: HashSet<u64>,
}

impl PersistentReplayGuard {
    #[must_use]
    pub fn new(recent_cap: usize) -> Self {
        Self {
            per_pair: HashMap::new(),
            recent_cap: recent_cap.max(1),
        }
    }

    /// init_counter admit (persistent, monoton-varsayımlı). counter > high_water → Fresh (side-effect);
    /// recent'te → ReplayNoSideEffect; high_water-altı-ve-recent-DEĞİL → fail-closed ReplayNoSideEffect
    /// (belirsiz→side-effect-uygulanmaz; early-data-güvenliği).
    pub fn admit(
        &mut self,
        sender: [u8; 16],
        recipient: [u8; 16],
        init_counter: u64,
    ) -> ReplayOutcome {
        let cap = self.recent_cap;
        let st = self.per_pair.entry((sender, recipient)).or_default();
        if init_counter > st.high_water {
            st.high_water = init_counter;
            if st.recent.len() >= cap {
                // Bounded: en-küçüğü-at (basit-eviction; TTL gerçek-store'da).
                if let Some(&min) = st.recent.iter().min() {
                    st.recent.remove(&min);
                }
            }
            st.recent.insert(init_counter);
            ReplayOutcome::Fresh
        } else if st.recent.contains(&init_counter) {
            ReplayOutcome::ReplayNoSideEffect
        } else {
            // high_water-altı + recent-DEĞİL (evict-edilmiş/eski) → fail-closed (side-effect-YOK).
            ReplayOutcome::ReplayNoSideEffect
        }
    }

    /// Restart-snapshot: (pair → high_water). Recent-set restart'ta-boşalır (high-water yeterli-taban;
    /// high-water-altı hepsi fail-closed-reject → early-data-güvenli).
    #[must_use]
    pub fn snapshot(&self) -> ReplaySnapshot {
        self.per_pair
            .iter()
            .map(|(k, v)| (*k, v.high_water))
            .collect()
    }

    /// Snapshot'tan geri-yükle (process-restart). Yalnız high-water taşınır (recent-boş → altı-reject).
    #[must_use]
    pub fn restore(snapshot: &[(DevicePair, u64)], recent_cap: usize) -> Self {
        let mut per_pair = HashMap::new();
        for (k, hw) in snapshot {
            per_pair.insert(
                *k,
                PairState {
                    high_water: *hw,
                    recent: HashSet::new(),
                },
            );
        }
        Self {
            per_pair,
            recent_cap: recent_cap.max(1),
        }
    }
}

// ── Early-data politikası (persistence-yoksa OFF) ──

/// Production early-data admission politikası (§0.7 PATCH-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlyDataPolicy {
    /// Replay-persistence mevcut → early-data-admission AÇILABİLİR (kod-doğrulandığında).
    Enabled,
    /// Replay-persistence YOK → production-early-data DEVRE-DIŞI (replay-riski).
    Disabled,
}

/// Persistence-durumuna göre early-data politikası. Persistence-yoksa DAİMA Disabled (fail-safe).
#[must_use]
pub fn early_data_policy(replay_persistence_available: bool) -> EarlyDataPolicy {
    if replay_persistence_available {
        EarlyDataPolicy::Enabled
    } else {
        EarlyDataPolicy::Disabled
    }
}

/// RESUME-politikası (§15): process-restart RESUME-EDİLMEZ. session-full-PN diske-persist-GÜVENİLMEZ →
/// restart-DAİMA yeni-session_id/handshake. Bu-fonksiyon sözleşmeyi-belgeler: eski-session-state resume-EDİLEMEZ.
#[must_use]
pub const fn resume_allowed_after_restart() -> bool {
    false
}
