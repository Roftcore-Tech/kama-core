//! A2-F0 İSKELETİ — ColdRxState: SESSION-LOCAL cold-admission/resource state (DR-CS-1b/A2).
//!
//! SAHİPLİK (K-8/K-9 — Abkarada-bağlayıcı):
//! - Bu modül YALNIZ session-local admission/resource state'i tanımlar. Saat TUTMAZ (`now_ns` enjekte).
//! - `GlobalColdLedger` burada SAF-TİP olarak tanımlanır; INSTANCE/LIFECYCLE sahibi
//!   `kama-integration::ReceiverHost`'tur (K-9: engine global-bütçenin lifecycle-sahibi OLAMAZ).
//! - Notice-slot / PostTimeoutNotice / sender-cold_epoch state BURADA YOK (K-8: sender-tarafı;
//!   adaptive-transport::Sender F4.2-diliminde doğar).
//!
//! [CAL]-SEALED sabitler (freeze-r2, sha256=5b298244…, SEALED—SCOPE-BOUNDED): TUNING YASAK;
//! koşu-sonucu görülerek GEVŞETME YASAK (yalnız model-kapısı sıkılaştırabilir → o da yeni-DUR-B).
//! CONTROL_DEDUP_WINDOW BURADA YOK (CANDIDATE; CAL-10-mühründen sonra, F-CID-0-kapısı, noise-wire).
//!
//! Bu dosya A2-F0 iskeletidir: tipler + SEALED-sabitler + typed-stub imzalar. Gerçek rezervasyon
//! (iki-seviye ATOMİK admission) A2-F1'de doldurulur.

use crate::error::EngineError;

// ---- [CAL]-SEALED sabitler (freeze-r2 §3 birebir) ----------------------------------------------

/// #4 — RxColdLedgerBlock eşzamanlılık-sınırı (4 × headroom-2).
pub const RX_COLD_OPEN_BLOCK_CAP_PER_SESSION: u64 = 8;
/// #5 — allowlist max(K_shape + R_code_max) = 16+8 (profil-katalog-türevi).
pub const RX_COLD_SHARDS_PER_BLOCK_CAP: u64 = 24;
/// #5b — REACHABLE-üst-sınır: açık-blok-modeli-dışı in-ledger shard-sınıfı YOK (freeze-r2 §2-beyanı).
pub const MAX_SHARDS_BY_BLOCK_SHAPE: u64 =
    RX_COLD_OPEN_BLOCK_CAP_PER_SESSION * RX_COLD_SHARDS_PER_BLOCK_CAP;
/// #6 — min(bellek-kolu-316, blok-şekli-192) = 192 (bağlayıcı-eksen = BLOK-ŞEKLİ).
pub const RX_COLD_TOTAL_SHARD_CAP_PER_SESSION: u64 = 192;
/// #7 — K-4 alt-kapı: min(TOTAL=192, MAX_PREPARAM_SHARDS=256) = 192.
pub const RX_PREPARAM_SHARD_CAP_PER_SESSION: u64 = 192;
/// #2 — 15 MiB / 32 (eşit-pay; N≤32 zarfı — C-8).
pub const RX_COLD_RETAINED_BYTE_CAP_PER_SESSION: u64 = 480 * 1024;
/// #1 — HARD cold-retained cap (16 MiB envelope − 1 MiB side-state-rezervi).
pub const GLOBAL_COLD_RETAINED_MEMORY_BUDGET: u64 = 15 * 1024 * 1024;
/// #3 — min(budget, 32 × per-session) = 15 MiB.
pub const GLOBAL_COLD_BYTE_CAP: u64 = 15 * 1024 * 1024;
/// #8 — 32 × headroom-2; ötesi typed-reject fail-closed.
pub const SESSION_COUNT_CAP: u64 = 64;
/// #9 — = SESSION_COUNT_CAP (worst-case: tümü aynı-anda closing).
pub const TOMBSTONE_REGISTRY_CAP: u64 = 64;

// Türetim-tutarlılık pinleri (derleme-zamanı; freeze-r2 C-1/C-4/C-5 aynaları).
const _: () = assert!(MAX_SHARDS_BY_BLOCK_SHAPE == 192);
const _: () = assert!(RX_COLD_TOTAL_SHARD_CAP_PER_SESSION == MAX_SHARDS_BY_BLOCK_SHAPE);
const _: () =
    assert!(RX_PREPARAM_SHARD_CAP_PER_SESSION <= crate::preparam_hold::MAX_PREPARAM_SHARDS as u64);
const _: () = assert!(32 * RX_COLD_RETAINED_BYTE_CAP_PER_SESSION == GLOBAL_COLD_BYTE_CAP);

// ---- Typed eksen/kimlik tipleri ----------------------------------------------------------------

/// Cap-aşımı kapsamı (iki-seviye admission; K-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapScope {
    Session,
    Global,
}

/// Cap-ekseni (freeze-r2 dört-eksen + alt-kapılar + host-eksenleri).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapKind {
    OpenBlock,
    RetainedByte,
    TotalShard,
    ShardsPerBlock,
    PreparamShard,
    SessionCount,
    TombstoneRegistry,
}

/// Cold-admission isteği (F1'de rezervasyon-defterine girer; kalıcı-retention-ÖNCESİ).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColdAdmitRequest {
    pub block_id: u32,
    pub esi: u16,
    /// Retained-memory modeli maliyeti (yalnız-payload DEĞİL; freeze-r2 §2).
    pub retained_cost_bytes: u64,
    /// PARAM-öncesi mi (PREPARAM alt-kapısına tabi)?
    pub pre_param: bool,
}

/// Admission sonucu (typed; sessiz-eviction YOK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColdAdmitOutcome {
    Admitted,
}

// ---- C-8 zorunlu-diagnostik sayaçları (Abkarada aynen; freeze-r2 §4-C-8) -----------------------

/// Global-defter diag'ları — INSTANCE ReceiverHost'ta yaşar (K-9).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GlobalColdDiag {
    pub global_cold_budget_exceeded: u64,
    pub cold_admission_rejected_by_global_budget: u64,
    pub active_sessions_at_global_exhaustion: u64,
    pub distinct_authenticated_peers_at_global_exhaustion: u64,
}

/// Session-defter diag'ı.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionColdDiag {
    pub session_cold_budget_exceeded: u64,
}

// ---- Ledger tipleri ----------------------------------------------------------------------------

/// PROCESS-WIDE defter — SAF-TİP (K-9: instance + lifecycle `ReceiverHost`'ta; engine sahiplenmez).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GlobalColdLedger {
    pub retained_bytes: u64,
    pub session_count: u64,
    pub tombstone_count: u64,
    pub diag: GlobalColdDiag,
}

impl GlobalColdLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// SESSION-LOCAL cold-state (K-8/K-9 sınırlarıyla; saat tutmaz — `now_ns` enjekte edilir).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ColdRxState {
    pub retained_bytes: u64,
    pub total_shards: u64,
    pub preparam_shards: u64,
    pub open_blocks: u64,
    pub diag: SessionColdDiag,
}

impl ColdRxState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// İKİ-SEVİYE ATOMİK cold-admission (K-9: global-reserve + session-reserve → ikisi-de-başarılıysa
    /// commit; biri-reddederse HİÇBİR ledger değişmez). A2-F1'de doldurulur.
    ///
    /// # Errors
    /// F0-iskelet: her çağrı [`EngineError::FeatureNotImplemented`] (`"a2_f1_admit"`).
    pub fn admit_cold_shard(
        &mut self,
        _now_ns: u64,
        _req: ColdAdmitRequest,
        _global: &mut GlobalColdLedger,
    ) -> Result<ColdAdmitOutcome, EngineError> {
        Err(EngineError::FeatureNotImplemented("a2_f1_admit"))
    }

    /// Session-iade: tüm session-local rezervler global-deftere iade edilir (zeroize F1.5'te,
    /// iade-zinciri v2-§4). A2-F1'de doldurulur.
    ///
    /// # Errors
    /// F0-iskelet: her çağrı [`EngineError::FeatureNotImplemented`] (`"a2_f1_release"`).
    pub fn release_session(&mut self, _global: &mut GlobalColdLedger) -> Result<(), EngineError> {
        Err(EngineError::FeatureNotImplemented("a2_f1_release"))
    }
}
