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
    ColdAdmitOutcome, ColdAdmitRequest, ColdReleaseReason, ColdRxState, CommittedTombstoneLease,
    GlobalColdLedger, TombstoneRegistry, SESSION_COUNT_CAP,
};
use kama_engine::{admit_and_decode, EngineError};
use kama_noise_wire::inner::InnerPlain;
use kama_noise_wire::{
    ControlDedupWindow, ControlIdIssueGate, DedupDecision, Direction, HandleRouter,
    KamaNoiseWireProtector, OpenedTransportShard, SealArgs, Session, SessionId, SessionLifecycle,
    SessionManager,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::UnsealedTransportPacket;
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::Zeroizing;

// ── F2 product-receiver sabitleri ────────────────────────────────────────────────────────────────

/// MANDATORY-BOOTSTRAP-PROFILE (§8-noise-wire B2; Alpha tek-code_suite). K=8/R=4/E=64.
const BOOTSTRAP_PROFILE_ID: u8 = 0x03;
/// Retained-cost modeli (freeze-r2 §2 SHARD_RETAINED_COST): E=64 → allocator-sınıf(80) + H_SHARD(256).
const SHARD_RETAINED_COST_E64: u64 = 336;

/// RX_COLD_ABANDON(profil) — F0-(viii) donmuş sınıf (receiver-monotonic ns; ZM-2 4-decade profil-türevi).
/// app-payload ≤300B → 75s sınıfı; >300B → 105s (1200B/S-B). E=64 bootstrap ≈53B app → 75s.
const RX_COLD_ABANDON_SMALL_NS: u64 = 75 * 1_000_000_000;
/// SESSION_IDLE_HOLD [CAL] konservatif profil-türevi (F2; ≥ en-büyük per-block deadline).
const SESSION_IDLE_HOLD_NS: u64 = 105 * 1_000_000_000;
/// TOMBSTONE_GRACE ≤ SESSION_IDLE_HOLD (§7.4 invariant; derleme-zamanı-pinli).
const TOMBSTONE_GRACE_NS: u64 = 30 * 1_000_000_000;
const _: () = assert!(TOMBSTONE_GRACE_NS <= SESSION_IDLE_HOLD_NS);

/// Ingest sonucu (product-gözlem; test/telemetri).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome {
    /// FEC source/repair shard cold-admit edildi (blok ilerledi).
    ShardAdmitted { block_id: u32, progress: u8 },
    /// Blok K-shard'a ulaştı → decode + app-delivery (payload-uzunluğu; içerik sızdırılmaz).
    BlockDelivered { block_id: u32, app_len: usize },
    /// Duplicate/role-conflict/cap-reject/param-mismatch → drop (typed neden).
    Dropped(&'static str),
    /// Stateless probe → echo üretildi (O(1)).
    ProbeEchoed,
    /// F3: yeni control (dedup-Fresh) → side-effect (F4-işleme yolu).
    ControlFresh { inner_kind: u8 },
    /// F3: görülmüş control (dedup-Duplicate/window-altı) → idempotent yalnız-ACK.
    ControlDuplicate,
}

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

/// Blok-toplama: authenticated shard'lar (byte'lar) + non-renewable deadline + K-decode eşiği.
struct BlockAssembly {
    /// P-1 non-renewable-deadline (first_authenticated_shard_time + RX_COLD_ABANDON; RESETLENMEZ).
    deadline_ns: u64,
    k_actual: u8,
    /// Authenticated shard'lar (open()-ürünü; INV-FEC-1 — decode'a yalnız bunlar girer).
    shards: Vec<OpenedTransportShard>,
}

/// Session-başına kompozisyon-yuvası: gerçek noise-wire `Session` (crypto-state) + session-local
/// cold-defter (muhasebe) + per-block assembly (byte-buffer) + son authenticated-DATA anchor (S-B).
pub struct SessionSlot {
    pub cold: ColdRxState,
    session: Session,
    blocks: BTreeMap<u32, BlockAssembly>,
    /// S-B session-idle-anchor: son authenticated-DATA-varış (probe/control BESLEMEZ).
    last_data_arrival_ns: u64,
    /// F3: fixed-window control dedup (session-scoped; probe/echo GİRMEZ). Closing'de düşer.
    dedup: ControlDedupWindow,
    /// F4: RECOVERED-emisyonu için control_id issuance-gate (F-CID-0; monoton + burst-enforce).
    issue_gate: ControlIdIssueGate,
    /// F4: bu batch'te RECOVERED-emisyon sayısı (SACK-return-supply-bütçesi payı; end_batch reset).
    recovered_emitted_in_batch: u32,
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
    /// Teslim-edilen app-payload'lar (product-gözlem; gerçek-üründe app-callback).
    delivered: Vec<Vec<u8>>,
    /// Stateless-echo sayacı (probe → echo; O(1) working-set).
    cs_echo_count: u64,
    #[allow(dead_code)] // F2-slice: SessionManager/HandleRouter tam-kablolama F3-F4 ingest'te
    session_manager: SessionManager,
    #[allow(dead_code)]
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
            delivered: Vec::new(),
            cs_echo_count: 0,
            session_manager: SessionManager::new(),
            router: HandleRouter::new(),
        }
    }

    /// Stateless-echo sayacı (probe → echo; test/telemetri).
    #[must_use]
    pub fn cs_echo_count(&self) -> u64 {
        self.cs_echo_count
    }

    /// Standart tombstone grace-deadline (now + TOMBSTONE_GRACE; §7.4 ≤ SESSION_IDLE_HOLD).
    /// close_session precompute-closure'ında cached-ACK için kullanılır.
    #[must_use]
    pub fn default_grace_deadline_ns(now_ns: u64) -> u64 {
        now_ns.saturating_add(TOMBSTONE_GRACE_NS)
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

    /// Test/host-bridge: gerçek noise-wire `Session`'lı aktif-yuva ekler (mock-key). Tek-register
    /// [`install_session_with_keys`]'te (çift-register YOK).
    pub fn insert_active_session_for_test(&mut self, h_ext: u32) -> Result<(), HostError> {
        self.install_session_with_keys(h_ext, &Zeroizing::new([0x11u8; 32]))
    }

    /// Gerçek-Session'lı aktif-yuva kurar (verilen simetrik-anahtarla; F2-ingest testleri paired-key
    /// verir). Product: handshake-Split()-key'leriyle. SESSION_COUNT_CAP enforce.
    pub fn install_session_with_keys(
        &mut self,
        h_ext: u32,
        key: &Zeroizing<[u8; 32]>,
    ) -> Result<(), HostError> {
        self.global_cold
            .register_session()
            .map_err(|_| HostError::SessionCountCapExceeded {
                cap: SESSION_COUNT_CAP,
            })?;
        let protector = KamaNoiseWireProtector::new(h_ext, key, key);
        let session = Session::new(SessionId(u64::from(h_ext)), Direction::Responder, protector);
        self.sessions.insert(
            h_ext,
            SessionSlot {
                cold: ColdRxState::new(),
                session,
                blocks: BTreeMap::new(),
                last_data_arrival_ns: 0,
                dedup: ControlDedupWindow::new(),
                issue_gate: ControlIdIssueGate::new(),
                recovered_emitted_in_batch: 0,
            },
        );
        Ok(())
    }

    /// Teslim-edilen app-payload sayısı (product-gözlem; F2-kabul).
    #[must_use]
    pub fn delivered_count(&self) -> usize {
        self.delivered.len()
    }

    /// GERÇEK INGEST YOLU (F2; v2-§1.3): h_ext-demux → gerçek AEAD-open (INV-FEC-1: authenticated-shard)
    /// → inner-demux → {stateless-echo | ParamState/profil-gate → iki-seviye cold-admission →
    /// block-assembly → K-decode-delivery}. Test-harness-mantığı DEĞİL; gerçek `Session` + `ColdRxState`
    /// + `admit_and_decode`. Saat dışarıdan (`now_ns`; receiver-monotonic, ZM-3/4).
    ///
    /// # Errors
    /// Yapısal host-hataları (cold-invariant → DUR-B). Normal-drop'lar `IngestOutcome::Dropped` (Err DEĞİL).
    pub fn on_datagram(
        &mut self,
        now_ns: u64,
        datagram: &[u8],
    ) -> Result<IngestOutcome, HostError> {
        // h_ext = §2.6 wire-layout ilk-4-byte (fixed-cost; alloc-öncesi demux).
        if datagram.len() < 4 {
            return Ok(IngestOutcome::Dropped("datagram too short"));
        }
        let h_ext = u32::from_be_bytes([datagram[0], datagram[1], datagram[2], datagram[3]]);
        let slot = match self.sessions.get_mut(&h_ext) {
            Some(s) => s,
            // Tombstone-grace: authenticated-CLOSE-retry burada cached-ACK üretir (F2.4; iskelet: bilinen
            // tombstone → echo-yolu). Bilinmeyen → drop (fixed-cost).
            None if self.tombstones.contains_key(&h_ext) => {
                return Ok(IngestOutcome::Dropped("tombstone grace (recv-only)"))
            }
            None => return Ok(IngestOutcome::Dropped("unknown handle")),
        };

        // GERÇEK AEAD-open (verify-before-admission §6; fail → drop, side-effect-yok).
        let opened = match slot.session.protector_mut().open(datagram) {
            Ok(o) => o,
            Err(_) => return Ok(IngestOutcome::Dropped("aead verify failed")),
        };

        // OpenedTransportShard sealed-tip (yeniden-kurulamaz) → tüketmeden inner()-ile oku, sonra
        // block-assembly'ye MOVE (decode'a authenticated-shard olarak girer; INV-FEC-1).
        let (profile_id, is_repair, block_id, k_actual) = match opened.inner() {
            InnerPlain::Control {
                inner_kind,
                control_id,
                ..
            } => {
                if *inner_kind == 0x19 || *inner_kind == 0x1A {
                    // Stateless probe/echo (A-2): ControlDedupWindow'a GİRMEZ; O(1); patience-BESLEMEZ.
                    self.cs_echo_count = self.cs_echo_count.saturating_add(1);
                    return Ok(IngestOutcome::ProbeEchoed);
                }
                // F3: fixed-window control dedup (session-scoped). Fresh → side-effect (F4-işleme);
                // DuplicateAckOnly → idempotent (yalnız-ACK). SESSION_CLOSE-retry'ı da bu yol kapsar.
                return Ok(match slot.dedup.check(*control_id) {
                    DedupDecision::Fresh => IngestOutcome::ControlFresh {
                        inner_kind: *inner_kind,
                    },
                    DedupDecision::DuplicateAckOnly => IngestOutcome::ControlDuplicate,
                });
            }
            InnerPlain::FecShard { header, .. } => (
                header.profile_id,
                matches!(header.kind, ShardKind::Repair),
                header.block_id,
                header.k_actual,
            ),
        };
        let esi = match opened.inner() {
            InnerPlain::FecShard { header, .. } => header.esi,
            InnerPlain::Control { .. } => unreachable!(),
        };

        // Profil-gate (MANDATORY-BOOTSTRAP; PARAM-bağı fail-closed).
        if profile_id != BOOTSTRAP_PROFILE_ID {
            return Ok(IngestOutcome::Dropped("profile mismatch"));
        }
        // İKİ-SEVİYE ATOMİK cold-admission (K-9).
        let req = ColdAdmitRequest {
            block_id,
            esi,
            is_repair,
            retained_cost_bytes: SHARD_RETAINED_COST_E64,
            pre_param: false, // bootstrap-confirmed
        };
        match slot
            .cold
            .admit_cold_shard(now_ns, req, &mut self.global_cold)
        {
            Ok(ColdAdmitOutcome::Admitted) => {}
            Ok(ColdAdmitOutcome::Duplicate) => {
                return Ok(IngestOutcome::Dropped("duplicate shard"))
            }
            Err(EngineError::ShardRoleConflict { .. }) => {
                return Ok(IngestOutcome::Dropped("shard role conflict"))
            }
            Err(EngineError::ColdCapExceeded { .. }) => {
                return Ok(IngestOutcome::Dropped("cold cap exceeded"))
            }
            Err(e) => return Err(HostError::from(e)),
        }

        // S-B: authenticated-DATA anchor'ı yenile (yalnız DATA; probe/control DEĞİL).
        slot.last_data_arrival_ns = now_ns;

        // Block-assembly: ilk-shard → non-renewable deadline (RESETLENMEZ; sonraki-shard değiştirmez).
        let asm = slot
            .blocks
            .entry(block_id)
            .or_insert_with(|| BlockAssembly {
                deadline_ns: now_ns.saturating_add(RX_COLD_ABANDON_SMALL_NS),
                k_actual,
                shards: Vec::new(),
            });
        asm.shards.push(opened); // authenticated-shard MOVE
        let progress = asm.shards.len() as u8;

        // K-decode eşiği: yeterli shard → admit_and_decode → app-delivery + block-release.
        if progress >= asm.k_actual {
            let asm = slot.blocks.remove(&block_id).expect("var");
            match admit_and_decode(asm.shards, BOOTSTRAP_PROFILE_ID, asm.k_actual) {
                Ok(app) => {
                    let app_len = app.len();
                    self.delivered.push(app);
                    slot.cold.release_block(
                        block_id,
                        ColdReleaseReason::Delivered,
                        &mut self.global_cold,
                    )?;
                    Ok(IngestOutcome::BlockDelivered { block_id, app_len })
                }
                // Decode-yetersiz: shard'lar drop (A-5 zeroize); blok deadline'da evict.
                Err(_) => Ok(IngestOutcome::ShardAdmitted { block_id, progress }),
            }
        } else {
            Ok(IngestOutcome::ShardAdmitted { block_id, progress })
        }
    }

    /// F4 RECEIVER RECOVERED-EMİSYONU (§10; §11 forward-recovery): blok-decode ile kurtarılan source-ESI'ler
    /// için sender'a RECOVERED-control (ARQ-cancel; logical-recovery — ACK/BtlBw-credit DEĞİL, §2.57).
    /// control_id F-CID-0 ControlIdIssueGate'ten (monoton + burst-enforce = SACK-return-supply bütçe-payı);
    /// gerçek-wire sealed datagram döner (sender'a gönderilecek). Active-session'da (send-canlı).
    ///
    /// # Errors
    /// [`HostError::UnknownSession`] · burst-aşımı [`HostError::PrecomputeFailed`] ("control-id burst") ·
    /// seal-hatası [`HostError::PrecomputeFailed`].
    pub fn emit_recovered(
        &mut self,
        h_ext: u32,
        block_id: u32,
        esi_ranges: Vec<(u16, u16)>,
        wire_pn: u64,
    ) -> Result<Vec<u8>, HostError> {
        let slot = self
            .sessions
            .get_mut(&h_ext)
            .ok_or(HostError::UnknownSession { h_ext })?;
        // F-CID-0: control_id monoton + burst-enforce (SACK-return-supply bütçe-payı).
        let id = slot
            .issue_gate
            .issue()
            .map_err(|_| HostError::PrecomputeFailed("control-id burst/renewal"))?;
        slot.recovered_emitted_in_batch = slot.recovered_emitted_in_batch.saturating_add(1);
        let ctrl = Control::Recovered {
            id,
            block_id,
            esi_ranges,
        };
        slot.session
            .seal(
                &UnsealedTransportPacket::Control(ctrl),
                SealArgs {
                    wire_pn,
                    dh_epoch: 0,
                    send_generation: 0,
                },
            )
            .map(|sealed| sealed.bytes)
            .map_err(|_| HostError::PrecomputeFailed("recovered seal"))
    }

    /// Emisyon-batch sınırı: RECOVERED-burst + control-id-gate burst sayaçlarını sıfırlar.
    pub fn end_emission_batch(&mut self, h_ext: u32) {
        if let Some(slot) = self.sessions.get_mut(&h_ext) {
            slot.issue_gate.end_batch();
            slot.recovered_emitted_in_batch = 0;
        }
    }

    /// PATIENCE-TICK (receiver-monotonic; non-renewable per-block deadline): deadline-geçmiş bloklar
    /// EVICT + zeroize (shard-drop) + cold-state release + typed-diag. S-B: canlı-blok-yokken
    /// authenticated-DATA-yoksa session-idle sonrası iade-zinciri (F2 diag; gerçek-teardown close_session).
    /// Döner: (evicted_block_count, idle_sessions).
    pub fn on_patience_tick(&mut self, now_ns: u64) -> (usize, usize) {
        let mut evicted = 0usize;
        let mut idle_sessions = 0usize;
        for slot in self.sessions.values_mut() {
            let expired: Vec<u32> = slot
                .blocks
                .iter()
                .filter(|(_, a)| now_ns >= a.deadline_ns)
                .map(|(b, _)| *b)
                .collect();
            for b in expired {
                slot.blocks.remove(&b); // shard-buffer Drop → A-5 zeroize
                let _ = slot.cold.release_block(
                    b,
                    ColdReleaseReason::DeadlineEvicted,
                    &mut self.global_cold,
                );
                evicted += 1;
            }
            // S-B session-idle: canlı-blok yok VE son-authenticated-DATA'dan bu yana IDLE_HOLD geçti.
            if slot.blocks.is_empty()
                && slot.last_data_arrival_ns > 0
                && now_ns.saturating_sub(slot.last_data_arrival_ns) >= SESSION_IDLE_HOLD_NS
            {
                idle_sessions += 1;
            }
        }
        (evicted, idle_sessions)
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
