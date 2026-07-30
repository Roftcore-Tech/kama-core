//! A2-F0 İSKELETİ — ReceiverHost: ürün-alıcı COMPOSITION-ROOT (DR-CS-1b/A2; Abkarada D-1/K-8/K-9).
//!
//! SAHİPLİK (bağlayıcı):
//! - **K-9 — PROCESS-WIDE kaynak-SAHİBİ:** `GlobalColdLedger`-INSTANCE'ı ve lifecycle'ı BURADA
//!   (GLOBAL_COLD_RETAINED_MEMORY_BUDGET · GLOBAL_COLD_BYTE_CAP · SESSION_COUNT_CAP ·
//!   TOMBSTONE_REGISTRY_CAP · global-reservation-ledger). İki-seviye admission ATOMİK:
//!   global-reserve + session-reserve → ikisi-de-başarılıysa commit; biri-reddederse hiçbir
//!   ledger değişmez (F1'de doldurulur).
//! - **K-8:** sender-application-notice state'i (PostTimeoutNotice/cold_epoch/1-1-1-pinleri)
//!   BURADA YOK — sender.rs F4.2-diliminde doğar. ReceiverHost yalnız RECOVERED (0x12)
//!   kontrol-emisyonunu yönetir (bütçesi SACK-return-supply-defterinden; F4.1).
//! - Saat-örneklemesi HOST'ta (ZM-3: receiver-monotonic; ColdRxState'e `now_ns` enjekte edilir).
//! - Tombstone-KAYIT-içeriği opak noise-wire-tipidir (tip session.rs-dönüşüm-kancasında doğar;
//!   ham anahtar/AEAD-primitifi bu crate'te TANIMLANAMAZ — MUTLAK-SINIR §2.1-şerhi).
//!
//! Bu dosya A2-F0 iskeletidir: sahiplik-alanları + typed-stub imzalar. Gerçek ingest-akışı
//! (open-demux → cold-gate → transport-Receiver) A2-F1'de kablolanır; adaptive-transport
//! prod-dep'i o dilimde eklenir.

use kama_engine::cold_rx_state::{ColdRxState, GlobalColdLedger, SESSION_COUNT_CAP};
use kama_noise_wire::{HandleRouter, SessionManager};
use std::collections::BTreeMap;
use thiserror::Error;

/// ReceiverHost typed-hataları (reachable todo!() YASAK).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostError {
    /// A2-fazı henüz uygulanmadı (typed-stub).
    #[error("feature not implemented: {0}")]
    FeatureNotImplemented(&'static str),
    /// SESSION_COUNT_CAP aşımı (typed-reject fail-closed; sessiz-eviction YOK).
    #[error("session count cap exceeded: cap={cap}")]
    SessionCountCapExceeded { cap: u64 },
}

/// Session-başına kompozisyon-yuvası (F1: + transport-Receiver + tombstone-durumu).
#[derive(Debug, Default)]
pub struct SessionSlot {
    pub cold: ColdRxState,
}

/// Ürün-alıcı composition-root (D-1: `kama-integration`; yeni-crate AÇILMAZ).
pub struct ReceiverHost {
    /// K-9: global-defter INSTANCE'ı — tek-sahip burası.
    global_cold: GlobalColdLedger,
    sessions: BTreeMap<u32, SessionSlot>,
    session_manager: SessionManager,
    router: HandleRouter,
}

impl ReceiverHost {
    #[must_use]
    pub fn new() -> Self {
        Self {
            global_cold: GlobalColdLedger::new(),
            sessions: BTreeMap::new(),
            session_manager: SessionManager::new(),
            router: HandleRouter::new(),
        }
    }

    /// K-9 global-defter salt-okunur görünümü (diag/plateau-ölçümü).
    #[must_use]
    pub fn global_cold(&self) -> &GlobalColdLedger {
        &self.global_cold
    }

    /// Aktif session-yuvası sayısı (SESSION_COUNT_CAP ≤ 64 pinli; F1'de enforce).
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Inbound-datagram girişi: HandleRouter.classify → SessionManager.open → demux →
    /// {stateless-echo | control-dedup | cold-gate → Receiver} (v2-§1.3 hedef-akışı).
    /// A2-F1'de kablolanır.
    ///
    /// # Errors
    /// F0-iskelet: her çağrı [`HostError::FeatureNotImplemented`] (`"a2_f1_ingest"`).
    pub fn on_datagram(&mut self, _now_ns: u64, _datagram: &[u8]) -> Result<(), HostError> {
        // Alanların F0'da ölü-koddan sayılmaması + sahiplik-imzasının derlenmesi için dokunuş:
        let _ = (
            &self.global_cold,
            self.sessions.len(),
            &self.session_manager,
            &self.router,
            SESSION_COUNT_CAP,
        );
        Err(HostError::FeatureNotImplemented("a2_f1_ingest"))
    }

    /// Session-kapanış iade-zinciri (v2-§4 hibrit-sıra; K-1 pinleri F2.4'te). A2-F1'de kablolanır.
    ///
    /// # Errors
    /// F0-iskelet: her çağrı [`HostError::FeatureNotImplemented`] (`"a2_f1_close_chain"`).
    pub fn close_session(&mut self, _h_ext: u32, _now_ns: u64) -> Result<(), HostError> {
        Err(HostError::FeatureNotImplemented("a2_f1_close_chain"))
    }
}

impl Default for ReceiverHost {
    fn default() -> Self {
        Self::new()
    }
}
