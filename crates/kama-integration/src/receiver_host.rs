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

use kama_engine::cold_rx_state::{
    ColdRxState, CommittedTombstoneLease, GlobalColdLedger, TombstoneRegistry, SESSION_COUNT_CAP,
};
use kama_engine::EngineError;
use kama_noise_wire::{
    Direction, HandleRouter, KamaNoiseWireProtector, Session, SessionId, SessionLifecycle,
    SessionManager,
};
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::Zeroizing;

/// ReceiverHost typed-hataları (reachable todo!() YASAK).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostError {
    /// A2-fazı henüz uygulanmadı (typed-stub).
    #[error("feature not implemented: {0}")]
    FeatureNotImplemented(&'static str),
    /// SESSION_COUNT_CAP aşımı (typed-reject fail-closed; sessiz-eviction YOK).
    #[error("session count cap exceeded: cap={cap}")]
    SessionCountCapExceeded { cap: u64 },
    /// TOMBSTONE_REGISTRY_CAP dolu (PHASE-A: destructive-teardown-ÖNCESİ fail-closed; state DEĞİŞMEZ).
    #[error("tombstone registry full")]
    TombstoneRegistryFull,
    /// PHASE-A precompute (sealed CLOSE-ACK) başarısız → rezervasyon-rollback; destructive-teardown YOK.
    #[error("close-ack precompute failed: {0}")]
    PrecomputeFailed(&'static str),
    /// Bilinmeyen/çift-kapatılan session (idempotency sınırı).
    #[error("unknown or already-closing session: {h_ext:#010x}")]
    UnknownSession { h_ext: u32 },
    /// Grace-expiry bilinmeyen tombstone (çift-expiry).
    #[error("unknown tombstone at grace expiry: {h_ext:#010x}")]
    UnknownTombstone { h_ext: u32 },
    /// Engine-ledger tutarsızlığı (commit-sonrası underflow) → DUR-B tetikleyicisi.
    #[error("engine invariant: {0}")]
    EngineInvariant(&'static str),
}

impl From<EngineError> for HostError {
    fn from(e: EngineError) -> Self {
        match e {
            EngineError::ColdInvariantViolation(m) => HostError::EngineInvariant(m),
            _ => HostError::EngineInvariant("unexpected engine error"),
        }
    }
}

/// Yuva-yaşam-döngüsü (Abkarada iki-aşamalı-close; Closing ≠ Tombstone ≠ Active).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotLifecycle {
    Active,
    /// PHASE-B sonrası: yalnız grace-recv (cached CLOSE-ACK); ağır-state düşmüş.
    Tombstone,
}

/// Precompute edilmiş sealed CLOSE-ACK yer-tutucusu. GERÇEK cipher-payload (recv-cipher + ReplayWindow +
/// sealed-bytes) F1-B-ingest-kablolamasında opak noise-wire-tipiyle bağlanır (MUTLAK-SINIR §2.1-şerhi);
/// bu iskelet transaction-YAPISINI (reserve-before-destruct + rollback + grace) taşır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedCloseAck {
    pub grace_deadline_ns: u64,
}

/// Session-başına kompozisyon-yuvası: gerçek noise-wire `Session` (crypto-state) + session-local cold-defter.
pub struct SessionSlot {
    pub cold: ColdRxState,
    session: Session,
}

/// Tombstone-kaydı (grace-recv; ağır cold-state DÜŞMÜŞ). Minimal-state: send-retired `Session`
/// (recv-cipher + recv-ReplayWindow grace boyunca YAŞAR; send-yarısı A-4 ile retired) + cached CLOSE-ACK +
/// registry-slot KİRASI (Drop → slot otomatik-iade). Entry düşünce Session-drop (recv-cipher zeroize-on-drop)
/// + lease-drop (slot-iade) BİRLİKTE.
struct TombstoneEntry {
    ack: CachedCloseAck,
    _session: Session,
    _lease: CommittedTombstoneLease,
}

/// Ürün-alıcı composition-root (D-1: `kama-integration`; yeni-crate AÇILMAZ).
pub struct ReceiverHost {
    /// K-9: global-defter INSTANCE'ı — tek-sahip burası (single-writer domain).
    global_cold: GlobalColdLedger,
    /// K-9: TOMBSTONE_REGISTRY_CAP RAII-sayacı (Drop-auto-cancel/release; panik-yok).
    tombstone_registry: TombstoneRegistry,
    sessions: BTreeMap<u32, SessionSlot>,
    tombstones: BTreeMap<u32, TombstoneEntry>,
    session_manager: SessionManager,
    router: HandleRouter,
}

impl ReceiverHost {
    #[must_use]
    pub fn new() -> Self {
        Self {
            global_cold: GlobalColdLedger::new(),
            tombstone_registry: TombstoneRegistry::new(),
            sessions: BTreeMap::new(),
            tombstones: BTreeMap::new(),
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

    /// Aktif tombstone-kaydı sayısı (grace-penceresi; ≤ TOMBSTONE_REGISTRY_CAP).
    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.tombstones.len()
    }

    /// Registry-sayacı (reserve+commit − cancel/release). Diag/plateau-ölçümü.
    #[must_use]
    pub fn tombstone_registry_count(&self) -> u64 {
        self.tombstone_registry.count()
    }

    /// Tombstone grace-deadline (cached CLOSE-ACK metadata; grace-yönetimi/expiry-zamanlaması).
    #[must_use]
    pub fn tombstone_grace_deadline_ns(&self, h_ext: u32) -> Option<u64> {
        self.tombstones.get(&h_ext).map(|t| t.ack.grace_deadline_ns)
    }

    /// Test/host-bridge: gerçek noise-wire `Session`'lı aktif-yuva ekler (mock-key; F1-B-ingest gerçek-
    /// handshake-key'leriyle kuracak). SESSION_COUNT_CAP enforce.
    pub fn insert_active_session_for_test(&mut self, h_ext: u32) -> Result<(), HostError> {
        self.global_cold
            .register_session()
            .map_err(|_| HostError::SessionCountCapExceeded {
                cap: SESSION_COUNT_CAP,
            })?;
        let key = Zeroizing::new([0x11u8; 32]);
        let protector = KamaNoiseWireProtector::new(h_ext, &key, &key);
        let session = Session::new(SessionId(u64::from(h_ext)), Direction::Responder, protector);
        self.sessions.insert(
            h_ext,
            SessionSlot {
                cold: ColdRxState::new(),
                session,
            },
        );
        Ok(())
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

    /// İKİ-AŞAMALI CLOSE-TRANSACTION (Abkarada DUR-B düzeltmesi; reserve-BEFORE-destruct).
    ///
    /// PHASE-A (PREPARE — hiçbir destructive-mutasyon): (2) TOMBSTONE_REGISTRY_CAP altında slot
    /// RESERVE (dolu → `TombstoneRegistryFull`; Active/cold state DEĞİŞMEZ) · (3) send-canlıyken
    /// sealed CLOSE-ACK PRECOMPUTE (fail → rezervasyon-rollback + typed `PrecomputeFailed`; teardown YOK).
    /// PHASE-B (COMMIT — single-writer domain): (5-6) lifecycle→Closing + drop_send_state
    /// [gerçek-Session teardown F1-B-ingest'te; iskelet cold-tarafını yürütür] · (7) ColdRxState.release
    /// · (8) dedup/ağır-state düşer · (9-10) Active-entry rezerve-slot kullanılarak Tombstone'a çevrilir +
    /// token COMMIT. Hata-semantiği: reserve-fail→no-mutation; precompute-fail→rollback; commit-sonrası
    /// underflow→`EngineInvariant` (DUR-B).
    ///
    /// `precompute`: send-canlıyken sealed CLOSE-ACK üretir (F1-B-ingest gerçek-`Session::seal` bağlar).
    ///
    /// # Errors
    /// [`HostError::UnknownSession`] · [`HostError::TombstoneRegistryFull`] ·
    /// [`HostError::PrecomputeFailed`] · [`HostError::EngineInvariant`].
    pub fn close_session<F>(&mut self, h_ext: u32, precompute: F) -> Result<(), HostError>
    where
        F: FnOnce(&mut Session) -> Result<CachedCloseAck, HostError>,
    {
        // Idempotency/varlık: yalnız Active session kapatılır (tombstone/bilinmeyen → typed).
        let slot = match self.sessions.get_mut(&h_ext) {
            Some(s) => s,
            None => return Err(HostError::UnknownSession { h_ext }),
        };

        // ---- PHASE-A: PREPARE (destructive-teardown YOK; tüm fallible-işlem burada) --------------
        // (2) Kapasite GARANTİSİ önce: RAII token RESERVE (dolu → fail-closed, state değişmez).
        let token = self
            .tombstone_registry
            .reserve()
            .map_err(|_| HostError::TombstoneRegistryFull)?;

        // (3) send-canlıyken (henüz begin_close YOK) sealed CLOSE-ACK PRECOMPUTE. Fail → token
        // scope-sonu Drop-AUTO-CANCEL (explicit-cancel gerekmez) → destructive-teardown YOK.
        let ack = match precompute(&mut slot.session) {
            Ok(a) => a,
            Err(_e) => return Err(HostError::PrecomputeFailed("sealed close-ack precompute")),
        };

        // ---- PHASE-B: COMMIT (single-writer domain; NORMAL koşulda INFALLIBLE) -------------------
        // Buraya YALNIZ token+ack GARANTİLİ iken gelinir (reserve-BEFORE-destruct). Underflow gibi
        // defter-tutarsızlıkları = bug → EngineInvariant (DUR-B; sessiz-devam YOK).
        let mut slot = self
            .sessions
            .remove(&h_ext)
            .expect("PHASE-A get_mut sonrası slot mevcut (single-writer)");
        // (5-6) GERÇEK teardown: lifecycle→Closing + A-4 drop_send_state (send-cipher grace'te YAŞAMAZ).
        slot.session.begin_close();
        debug_assert_eq!(slot.session.lifecycle(), SessionLifecycle::Closing);
        // (7-8) ağır cold-state release (session+global rezerv iade + [F1.5 A-5 zeroize buffer-drop'ta]).
        slot.cold.release_session(&mut self.global_cold)?;
        // (9-10) Active→Tombstone ÇEVRİMİ: active-sayaçtan çıkar (unregister; HandleRouter kaydına
        // DOKUNMAZ — route grace-sonuna kadar kalır), rezerve-slot COMMIT (token→lease, tombstone sahiplenir).
        self.global_cold.unregister_session()?;
        let lease = token.commit();
        self.tombstones.insert(
            h_ext,
            TombstoneEntry {
                ack,
                _session: slot.session, // send-retired; recv grace boyunca yaşar
                _lease: lease,
            },
        );
        Ok(())
    }

    /// GRACE-EXPIRY cleanup (adım-11-13): tombstone-entry düşer → `Session`-Drop (recv-cipher/ReplayWindow
    /// zeroize-on-drop) + cached-ACK-Drop + `CommittedTombstoneLease`-Drop (registry-slot OTOMATİK-iade,
    /// panik-yok) BİRLİKTE + HandleRouter.unregister (route yalnız BURADA silinir).
    ///
    /// # Errors
    /// [`HostError::UnknownTombstone`] (çift-expiry; registry-underflow üretmez).
    pub fn expire_tombstone(&mut self, h_ext: u32) -> Result<(), HostError> {
        // (11+13) entry-remove → Session/ack/lease BİRLİKTE Drop (recv-zeroize + slot-auto-iade).
        if self.tombstones.remove(&h_ext).is_none() {
            return Err(HostError::UnknownTombstone { h_ext });
        }
        // (12) HandleRouter.unregister YALNIZ grace-expiry'de (F1-B-ingest gerçek-router-kaydını siler;
        // route grace boyunca korunmuştu — Abkarada §3 invariant).
        Ok(())
    }
}

impl Default for ReceiverHost {
    fn default() -> Self {
        Self::new()
    }
}
