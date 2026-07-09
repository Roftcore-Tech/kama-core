//! TV-ARQ (§9): selective retransmit execution.
//!
//! SACK gap → kayıp ilan edilen seq → **deadline-öncelikli** selective retransmit
//! (§9, §12). Retransmit'ler DATA pacing bütçesini paylaşır (§13, [`crate::pacer`])
//! — inflight cap'e tabi; retransmit fırtınası rate'i patlatamaz (INV-RATE-1). Fuzzy
//! `arq_aggressiveness=0` → baskıla (fec-dominant, §9). Spurious → reorder_thresh↑ (§9.3).
//!
//! **Deadline politikası (NORMATİF karar):** `deadline_ns` YALNIZ öncelik sırasıdır
//! (erken önce); DROP kriteri DEĞİLDİR. Deadline pace-beklerken geçse bile seq planda
//! KALIR ve token gelince gönderilir — "veri ULAŞSIN" (mesajlaşma semantiği; geç mesaj
//! > hiç mesaj). **Expired-drop YOK.**
//!
//! Clean-arch: [`ArqScheduler`] trait (port) + [`DeadlineArqScheduler`] impl. §9.4 rx
//! duplicate-suppress receiver-side'dır (bu modül sender execution).

/// Kayıp ilan edilmiş, retransmit bekleyen bir seq.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LostSeq {
    /// Kayıp seq (full counter).
    pub seq: u64,
    /// Deadline (ns) — YALNIZ öncelik sırası; drop kriteri DEĞİL.
    pub deadline_ns: u64,
    /// Bu seq için şu ana dek retransmit sayısı.
    pub retx_count: u8,
}

/// ARQ_RETX_CAP aşımında politika (§9.1 — spec iki seçeneği de sunar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapExceedPolicy {
    /// Forward-recovery'ye devret (recovery-obsesif default; "veri ulaşsın").
    HandToForward,
    /// Bağlantıyı fail-closed kapat.
    ConnectionFail,
}

/// Bir seq için retransmit kararı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetxDecision {
    /// Yeniden gönder.
    Retransmit {
        /// Gönderilecek seq.
        seq: u64,
    },
    /// ARQ_RETX_CAP aşıldı → politika uygula.
    CapExceeded {
        /// İlgili seq.
        seq: u64,
        /// Uygulanacak politika.
        policy: CapExceedPolicy,
    },
}

/// ARQ yapılandırması. **[CAL] fixture** (netem'e).
#[derive(Debug, Clone, Copy)]
pub struct RetxConfig {
    /// Bir seq için max retransmit (§9.1).
    pub arq_retx_cap: u8,
    /// Reorder tolerance tabanı K (§5, [STD:3]).
    pub reorder_k: u32,
    /// ρ pay — `reorder_thresh = max(K, ρ×recent_max_reorder)` [CAL].
    pub reorder_rho_num: u32,
    /// ρ payda [CAL].
    pub reorder_rho_den: u32,
    /// Cap aşımı politikası.
    pub cap_exceed: CapExceedPolicy,
}

impl RetxConfig {
    /// **KALİBRE DEĞİL** fixture.
    #[must_use]
    pub fn fixture() -> Self {
        Self {
            arq_retx_cap: 3,
            reorder_k: 3,
            reorder_rho_num: 2,
            reorder_rho_den: 1,
            cap_exceed: CapExceedPolicy::HandToForward,
        }
    }
}

/// Selective retransmit planlayıcı portu (clean-arch).
pub trait ArqScheduler {
    /// Kayıp seq'lerden retransmit planı üretir. `arq_aggressiveness_permille == 0` →
    /// baskıla (boş plan, fec-dominant). Aksi halde deadline-öncelikli (erken önce);
    /// expired deadline DROP EDİLMEZ.
    fn plan(&self, lost: &[LostSeq], arq_aggressiveness_permille: u32) -> Vec<RetxDecision>;
}

/// Deadline-öncelikli selective retransmit scheduler (§9).
#[derive(Debug, Clone, Copy)]
pub struct DeadlineArqScheduler {
    cfg: RetxConfig,
}

impl DeadlineArqScheduler {
    /// Verilen config ile.
    #[must_use]
    pub fn new(cfg: RetxConfig) -> Self {
        Self { cfg }
    }
}

impl ArqScheduler for DeadlineArqScheduler {
    fn plan(&self, lost: &[LostSeq], arq_aggressiveness_permille: u32) -> Vec<RetxDecision> {
        // Fuzzy arq_aggressiveness=0 → fec-dominant baskılama (§9: "retransmit BASKILA").
        if arq_aggressiveness_permille == 0 {
            return Vec::new();
        }
        // Deadline-öncelik: erken deadline önce. deadline YALNIZ SIRA — expired seq
        // DROP EDİLMEZ, planda kalır ("veri ulaşsın").
        let mut ordered = lost.to_vec();
        ordered.sort_by_key(|l| l.deadline_ns);
        ordered
            .into_iter()
            .map(|l| {
                if l.retx_count >= self.cfg.arq_retx_cap {
                    RetxDecision::CapExceeded {
                        seq: l.seq,
                        policy: self.cfg.cap_exceed,
                    }
                } else {
                    RetxDecision::Retransmit { seq: l.seq }
                }
            })
            .collect()
    }
}

/// Adaptif reorder tolerance + spurious-retransmit takibi (§9.3).
#[derive(Debug, Clone, Copy, Default)]
pub struct ReorderTracker {
    recent_max_reorder: u32,
}

impl ReorderTracker {
    /// Boş tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `reorder_thresh = max(K, ρ×recent_max_reorder)` (§5/§9.3).
    #[must_use]
    pub fn reorder_thresh(&self, cfg: &RetxConfig) -> u32 {
        let scaled = (u64::from(cfg.reorder_rho_num) * u64::from(self.recent_max_reorder)
            / u64::from(cfg.reorder_rho_den.max(1))) as u32;
        cfg.reorder_k.max(scaled)
    }

    /// Spurious retransmit teyidi (retransmit'ten SONRA orijinal ACK / DSACK geldi):
    /// `recent_max_reorder`'ı yükseltir → reorder_thresh↑ → gelecekte daha uzun bekle.
    /// Küçülmez (max korunur).
    pub fn note_spurious(&mut self, observed_reorder: u32) {
        self.recent_max_reorder = self.recent_max_reorder.max(observed_reorder);
    }

    /// Güncel `recent_max_reorder`.
    #[must_use]
    pub fn recent_max_reorder(&self) -> u32 {
        self.recent_max_reorder
    }
}
