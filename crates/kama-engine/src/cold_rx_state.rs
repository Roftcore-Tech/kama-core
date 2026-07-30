//! A2-F1-A — ColdRxState: SESSION-LOCAL cold-admission/resource LEDGER (DR-CS-1b/A2).
//!
//! SAHİPLİK (K-8/K-9 — Abkarada-bağlayıcı):
//! - Bu modül YALNIZ session-local admission/resource state'i tanımlar. Saat TUTMAZ (`now_ns` enjekte).
//! - `GlobalColdLedger` burada SAF-TİP; INSTANCE/LIFECYCLE sahibi `kama-integration::ReceiverHost` (K-9).
//! - Notice-slot / PostTimeoutNotice / sender-cold_epoch state BURADA YOK (K-8: sender F4.2-dilimi).
//!
//! ATOMİKLİK (Abkarada F1-pini): admission'da TÜM kontroller SALT-OKUNUR yapılır; mutasyon yalnız
//! TEK-COMMIT-NOKTASINDA. Erken-dönüş/hata yollarında SIFIR mutasyon → yarım-rezervasyon YAPISAL
//! imkansız (rollback-guard'a gerek kalmaz; plan-sonra-commit, guard'dan daha güçlü garantidir).
//! Rezervasyon-alanları (`res`) diag-sayaçlarından (`diag`) AYRIŞTIRILDI: reject-yolunda `res`
//! byte-for-byte değişmez (test-pinli); C-8 diag'ları reject'i SAYMAK ZORUNDA olduğundan ayrı yaşar.
//!
//! CONCURRENCY-PİNİ (Abkarada F1-A-DUR-B; BAĞLAYICI): GlobalColdLedger + session-ledger mutasyonu
//! AYNI ReceiverHost EXCLUSIVE/SINGLE-WRITER domain'indedir (&mut-disiplinli tek-yazar; paylaşımlı/
//! çok-iş-parçacıklı mutasyona geçiş = THREAD-MODELİ-DEĞİŞİKLİĞİ = DUR-B).
//!
//! KONTROL-SIRASI (deterministik; talimat-F1.1): (0) (block_id,esi)-dedup → aynı-role=Duplicate /
//! farklı-role=ShardRoleConflict (İKİSİNDE-DE delta=0) ·
//! (1) SHARDS_PER_BLOCK ön-kapı · (2) PREPARAM alt-kapı (K-4 min) · (3) OPEN_BLOCK (yeni-blok) ·
//! (4) TOTAL_SHARD (defense-in-depth: mevcut-sabitlerle şekil-kapısı önce dolar — allowlist
//! genişlerse bağlayıcı olur) · (5) session-RETAINED_BYTE · (6) global-BYTE. İlk-dolan-reddeder.
//!
//! [CAL]-SEALED sabitler (freeze-r2, sha256=5b298244…): TUNING/GEVŞETME YASAK.
//! CONTROL_DEDUP_WINDOW BURADA YOK (CANDIDATE; CAL-10/F-CID-0; noise-wire).

use crate::error::EngineError;
use std::collections::BTreeMap;

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

/// Cold-admission isteği (kalıcı-retention-ÖNCESİ rezervasyon).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColdAdmitRequest {
    pub block_id: u32,
    pub esi: u16,
    /// Role-biti: KİMLİĞİN-PARÇASI-DEĞİL (Abkarada F1-A-düzeltmesi) — kimlik (block_id,esi);
    /// is_repair kimliğe-ait-DOĞRULANAN-özelliktir. Aynı-kimlik+farklı-role → ShardRoleConflict
    /// (pre-PARAM role-flip'le ikinci-rezervasyon yapısal-engelli).
    pub is_repair: bool,
    /// Retained-memory modeli maliyeti (yalnız-payload DEĞİL; freeze-r2 §2).
    pub retained_cost_bytes: u64,
    /// PARAM-öncesi mi (PREPARAM alt-kapısına tabi)?
    pub pre_param: bool,
}

/// Admission sonucu (typed; sessiz-eviction YOK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColdAdmitOutcome {
    /// Rezervasyon yapıldı (iki-seviye commit).
    Admitted,
    /// Aynı (block_id,esi)+AYNI-role zaten defterde — retransmit/replay; rezervasyon TÜKETİLMEDİ.
    Duplicate,
}

/// Blok-iade nedeni (F1-A mekanik-iade; zeroize F1-B'de bu yola bağlanır).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColdReleaseReason {
    Delivered,
    DeadlineEvicted,
    SessionClose,
}

/// Blok-iadesinin serbest bıraktığı kaynaklar (global-defter düşümüyle aynı değerler).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleasedBlock {
    pub bytes: u64,
    pub shards: u64,
    pub preparam_shards: u64,
}

// ---- C-8 zorunlu-diagnostik sayaçları (Abkarada aynen; REZERVASYONDAN AYRI) --------------------

/// Global diag'lar — reject-yolunda artar; `res` değişmez (atomiklik-pini bunun üstünden test edilir).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GlobalColdDiag {
    pub global_cold_budget_exceeded: u64,
    pub cold_admission_rejected_by_global_budget: u64,
    /// Global-tükenme ANINDAKİ aktif-session sayısı (son-tükenme snapshot'ı).
    pub active_sessions_at_global_exhaustion: u64,
    /// Host-yazar (engine peer-kimliği bilmez; ReceiverHost reject-outcome'unda doldurur).
    pub distinct_authenticated_peers_at_global_exhaustion: u64,
}

/// Session diag'ı (tüm session-eksen reject'leri tek-sayaçta — C-8 adlandırması).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionColdDiag {
    pub session_cold_budget_exceeded: u64,
}

// ---- Rezervasyon alt-struct'ları (byte-for-byte atomiklik-pini bu tipler üstünde) --------------

/// Session-rezervasyon durumu (reject-yolunda DEĞİŞMEZ — test-pinli).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionColdReservation {
    pub retained_bytes: u64,
    pub total_shards: u64,
    pub preparam_shards: u64,
    pub open_blocks: u64,
}

/// Global-rezervasyon durumu (reject-yolunda DEĞİŞMEZ — test-pinli).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GlobalColdReservation {
    pub retained_bytes: u64,
    pub session_count: u64,
    pub tombstone_count: u64,
}

/// Blok-defter girdisi (K-2-alt-küme: bunlar RxColdLedgerBlock'lardır; BootstrapColdBlock-kimliği
/// sender-tarafıdır). `first_arrival_ns` = P-1 non-renewable-deadline çapası (F2 tüketir).
#[derive(Debug, Clone, PartialEq, Eq)]
struct BlockEntry {
    first_arrival_ns: u64,
    retained_bytes: u64,
    preparam_shards: u64,
    /// esi → is_repair (kimlik=(block_id,esi); role=doğrulanan-özellik — F1-A-düzeltmesi).
    admitted: BTreeMap<u16, bool>,
}

// ---- Ledger tipleri ----------------------------------------------------------------------------

/// PROCESS-WIDE defter — SAF-TİP (K-9: instance + lifecycle `ReceiverHost`'ta).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GlobalColdLedger {
    pub res: GlobalColdReservation,
    pub diag: GlobalColdDiag,
}

impl GlobalColdLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// SESSION_COUNT_CAP ekseni (host session-kabulünde çağırır; K-1/K-9).
    ///
    /// # Errors
    /// Cap-doluysa [`EngineError::ColdCapExceeded`] (Global/SessionCount); `res` değişmez.
    pub fn register_session(&mut self) -> Result<(), EngineError> {
        if self.res.session_count + 1 > SESSION_COUNT_CAP {
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Global,
                kind: CapKind::SessionCount,
                cap: SESSION_COUNT_CAP,
                requested: self.res.session_count + 1,
            });
        }
        self.res.session_count += 1;
        Ok(())
    }

    /// Session-kaydı düşümü (iade-zinciri adımı).
    ///
    /// # Errors
    /// Sayaç-altında çağrı = defter-tutarsızlığı: [`EngineError::ColdInvariantViolation`].
    pub fn unregister_session(&mut self) -> Result<(), EngineError> {
        self.res.session_count =
            self.res
                .session_count
                .checked_sub(1)
                .ok_or(EngineError::ColdInvariantViolation(
                    "session_count_underflow",
                ))?;
        Ok(())
    }

    /// TOMBSTONE_REGISTRY_CAP ekseni (K-1: registry-dolu → fail-closed typed-diag).
    ///
    /// # Errors
    /// Cap-doluysa [`EngineError::ColdCapExceeded`] (Global/TombstoneRegistry).
    pub fn reserve_tombstone(&mut self) -> Result<(), EngineError> {
        if self.res.tombstone_count + 1 > TOMBSTONE_REGISTRY_CAP {
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Global,
                kind: CapKind::TombstoneRegistry,
                cap: TOMBSTONE_REGISTRY_CAP,
                requested: self.res.tombstone_count + 1,
            });
        }
        self.res.tombstone_count += 1;
        Ok(())
    }

    /// Tombstone-iadesi (grace-sonu).
    ///
    /// # Errors
    /// Sayaç-altında çağrı: [`EngineError::ColdInvariantViolation`].
    pub fn release_tombstone(&mut self) -> Result<(), EngineError> {
        self.res.tombstone_count =
            self.res
                .tombstone_count
                .checked_sub(1)
                .ok_or(EngineError::ColdInvariantViolation(
                    "tombstone_count_underflow",
                ))?;
        Ok(())
    }
}

/// SESSION-LOCAL cold-ledger (K-8/K-9 sınırlarıyla; saat tutmaz — `now_ns` enjekte).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ColdRxState {
    pub res: SessionColdReservation,
    pub diag: SessionColdDiag,
    blocks: BTreeMap<u32, BlockEntry>,
}

impl ColdRxState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Açık-blok first-arrival çapası (P-1 non-renewable-deadline girdisi; F2 tüketir).
    #[must_use]
    pub fn block_first_arrival_ns(&self, block_id: u32) -> Option<u64> {
        self.blocks.get(&block_id).map(|b| b.first_arrival_ns)
    }

    /// İKİ-SEVİYE ATOMİK cold-admission (K-9). Kontrol-sırası modül-başlığında; TÜM kontroller
    /// salt-okunur, mutasyon yalnız tek-commit-noktasında (yarım-rezervasyon yapısal-imkansız).
    ///
    /// # Errors
    /// İlk-dolan-eksen [`EngineError::ColdCapExceeded`] üretir; reject-yolunda her iki `res`
    /// byte-for-byte değişmez (yalnız C-8 diag-sayaçları artar).
    pub fn admit_cold_shard(
        &mut self,
        now_ns: u64,
        req: ColdAdmitRequest,
        global: &mut GlobalColdLedger,
    ) -> Result<ColdAdmitOutcome, EngineError> {
        // ---- SALT-OKUNUR kontrol fazı -----------------------------------------------------------
        let entry = self.blocks.get(&req.block_id);
        // (0) dedup — kimlik (block_id,esi); role-biti kimlik-DEĞİL (F1-A-düzeltmesi):
        //     aynı-role → Duplicate; farklı-role → typed ShardRoleConflict; İKİSİNDE-DE delta=0
        //     (mevcut-kayıt değişmez; role-flip'le ikinci-rezervasyon yapısal-imkansız).
        if let Some(e) = entry {
            if let Some(&role) = e.admitted.get(&req.esi) {
                if role == req.is_repair {
                    return Ok(ColdAdmitOutcome::Duplicate);
                }
                return Err(EngineError::ShardRoleConflict {
                    block_id: req.block_id,
                    esi: req.esi,
                });
            }
        }
        let is_new_block = entry.is_none();
        let block_shards = entry.map_or(0, |e| e.admitted.len() as u64);

        // (1) SHARDS_PER_BLOCK ön-kapı.
        if block_shards + 1 > RX_COLD_SHARDS_PER_BLOCK_CAP {
            self.diag.session_cold_budget_exceeded += 1;
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Session,
                kind: CapKind::ShardsPerBlock,
                cap: RX_COLD_SHARDS_PER_BLOCK_CAP,
                requested: block_shards + 1,
            });
        }
        // (2) PREPARAM alt-kapı (K-4: hold-sessiz-drop'a ulaşım yapısal-kapalı).
        if req.pre_param && self.res.preparam_shards + 1 > RX_PREPARAM_SHARD_CAP_PER_SESSION {
            self.diag.session_cold_budget_exceeded += 1;
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Session,
                kind: CapKind::PreparamShard,
                cap: RX_PREPARAM_SHARD_CAP_PER_SESSION,
                requested: self.res.preparam_shards + 1,
            });
        }
        // (3) OPEN_BLOCK (yalnız yeni-blok slot-talebi; mevcut-blok cap'e takılmaz — f4_di10).
        if is_new_block && self.res.open_blocks + 1 > RX_COLD_OPEN_BLOCK_CAP_PER_SESSION {
            self.diag.session_cold_budget_exceeded += 1;
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Session,
                kind: CapKind::OpenBlock,
                cap: RX_COLD_OPEN_BLOCK_CAP_PER_SESSION,
                requested: self.res.open_blocks + 1,
            });
        }
        // (4) TOTAL_SHARD — defense-in-depth (mevcut sabitlerle şekil-kapıları önce dolar;
        // allowlist genişlerse bağlayıcı olur — freeze-r2 #6 notu).
        if self.res.total_shards + 1 > RX_COLD_TOTAL_SHARD_CAP_PER_SESSION {
            self.diag.session_cold_budget_exceeded += 1;
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Session,
                kind: CapKind::TotalShard,
                cap: RX_COLD_TOTAL_SHARD_CAP_PER_SESSION,
                requested: self.res.total_shards + 1,
            });
        }
        // (5) session-RETAINED_BYTE.
        let new_session_bytes = self.res.retained_bytes + req.retained_cost_bytes;
        if new_session_bytes > RX_COLD_RETAINED_BYTE_CAP_PER_SESSION {
            self.diag.session_cold_budget_exceeded += 1;
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Session,
                kind: CapKind::RetainedByte,
                cap: RX_COLD_RETAINED_BYTE_CAP_PER_SESSION,
                requested: new_session_bytes,
            });
        }
        // (6) global-BYTE (C-8: bağlayıcı-eksen; diag-snapshot son-tükenme anını yazar).
        let new_global_bytes = global.res.retained_bytes + req.retained_cost_bytes;
        if new_global_bytes > GLOBAL_COLD_BYTE_CAP {
            global.diag.global_cold_budget_exceeded += 1;
            global.diag.cold_admission_rejected_by_global_budget += 1;
            global.diag.active_sessions_at_global_exhaustion = global.res.session_count;
            return Err(EngineError::ColdCapExceeded {
                scope: CapScope::Global,
                kind: CapKind::RetainedByte,
                cap: GLOBAL_COLD_BYTE_CAP,
                requested: new_global_bytes,
            });
        }

        // ---- TEK-COMMIT-NOKTASI (infallible; iki-seviye birlikte) -------------------------------
        let e = self
            .blocks
            .entry(req.block_id)
            .or_insert_with(|| BlockEntry {
                first_arrival_ns: now_ns,
                retained_bytes: 0,
                preparam_shards: 0,
                admitted: BTreeMap::new(),
            });
        e.admitted.insert(req.esi, req.is_repair);
        e.retained_bytes += req.retained_cost_bytes;
        if req.pre_param {
            e.preparam_shards += 1;
            self.res.preparam_shards += 1;
        }
        if is_new_block {
            self.res.open_blocks += 1;
        }
        self.res.total_shards += 1;
        self.res.retained_bytes = new_session_bytes;
        global.res.retained_bytes = new_global_bytes;
        Ok(ColdAdmitOutcome::Admitted)
    }

    /// Blok-iadesi (Delivered/DeadlineEvicted/SessionClose): session+global rezervleri düşer.
    /// Zeroize F1-B'de bu yola bağlanır (iade-değerleri byte-kanıtı olarak döner).
    ///
    /// # Errors
    /// Bilinmeyen-blok veya ÇİFT-iade: [`EngineError::ColdInvariantViolation`]
    /// (`"release_unknown_or_double"`); defter-tutarsızlığı (underflow) da typed.
    pub fn release_block(
        &mut self,
        block_id: u32,
        _reason: ColdReleaseReason,
        global: &mut GlobalColdLedger,
    ) -> Result<ReleasedBlock, EngineError> {
        let e = self
            .blocks
            .remove(&block_id)
            .ok_or(EngineError::ColdInvariantViolation(
                "release_unknown_or_double",
            ))?;
        let freed = ReleasedBlock {
            bytes: e.retained_bytes,
            shards: e.admitted.len() as u64,
            preparam_shards: e.preparam_shards,
        };
        self.res.retained_bytes = self.res.retained_bytes.checked_sub(freed.bytes).ok_or(
            EngineError::ColdInvariantViolation("session_bytes_underflow"),
        )?;
        self.res.total_shards = self.res.total_shards.checked_sub(freed.shards).ok_or(
            EngineError::ColdInvariantViolation("session_shards_underflow"),
        )?;
        self.res.preparam_shards = self
            .res
            .preparam_shards
            .checked_sub(freed.preparam_shards)
            .ok_or(EngineError::ColdInvariantViolation("preparam_underflow"))?;
        self.res.open_blocks = self
            .res
            .open_blocks
            .checked_sub(1)
            .ok_or(EngineError::ColdInvariantViolation("open_blocks_underflow"))?;
        global.res.retained_bytes = global.res.retained_bytes.checked_sub(freed.bytes).ok_or(
            EngineError::ColdInvariantViolation("global_bytes_underflow"),
        )?;
        Ok(freed)
    }

    /// Session-iadesi: TÜM bloklar + rezervler global-deftere döner. İDEMPOTENT (ikinci çağrı
    /// no-op; iade-zinciri tekrar-çağrılara dayanıklı — belgeli davranış, çift-release-blok'tan farklı).
    ///
    /// # Errors
    /// Defter-tutarsızlığı (global-underflow): [`EngineError::ColdInvariantViolation`].
    pub fn release_session(&mut self, global: &mut GlobalColdLedger) -> Result<(), EngineError> {
        let freed_bytes = self.res.retained_bytes;
        global.res.retained_bytes = global.res.retained_bytes.checked_sub(freed_bytes).ok_or(
            EngineError::ColdInvariantViolation("global_bytes_underflow"),
        )?;
        self.blocks.clear();
        self.res = SessionColdReservation::default();
        Ok(())
    }
}
