//! Hub-SİMÜLASYONU (§2.55 — hub-tarafı D1'de SİMÜLE; gerçek-hub ayrı-repo). Sıralama-garantisini +
//! idempotency + bounded-retry'yi test-edilebilir modelller. **SIRALAMA-GARANTİSİ:** hub, h_ext'li
//! extended-paketi responder'a HANDLE_ASSIGN-teslim-edilmeden FORWARD-ETMEZ (extended-parser-bunsuz-çalışmaz).
//! responder-offline → mailbox [HANDLE_ASSIGN, INIT/extended]-sıralı.
//!
//! Deterministik (RNG-YOK, ADR-17): handle-allocator monoton; aynı-input→aynı-output.

use std::collections::{HashMap, HashSet};

use crate::error::WireError;
use crate::handle_assign::HandleAssign;

/// HANDLE_ASSIGN retry üst-sınırı (§2.55 bounded-retry; ACK/deadline'a-kadar). [CAL — ölç-ve-kilitle].
pub const HANDLE_ASSIGN_MAX_RETRIES: u32 = 5;

/// Responder'a hub-teslim öğesi (sıralı-mailbox).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponderDelivery {
    /// h_ext'i taşır — extended-paketlerden ÖNCE gelmeli.
    HandleAssign(HandleAssign),
    /// E2E extended-paket (FEC/control) — forward.
    Extended { h_ext: u32, bytes: Vec<u8> },
}

/// Deterministik hub-sim state.
#[derive(Debug, Default)]
pub struct HubSim {
    next_handle: u32,
    /// idempotency: (session_attempt_id, init_counter) → atanmış HandleAssign.
    assignments: HashMap<(u64, u64), HandleAssign>,
    /// responder mailbox (sıralı-teslim).
    responder_queue: Vec<ResponderDelivery>,
    /// hangi h_ext'ler için HANDLE_ASSIGN responder'a teslim-edildi (sıralama-guard).
    handle_delivered: HashSet<u32>,
    /// session_attempt_id → retry-sayısı.
    retries: HashMap<u64, u32>,
    /// ACK-gelen session_attempt_id'ler (retry-durur).
    acked: HashSet<u64>,
}

impl HubSim {
    /// `first_handle` monoton-allocator başlangıcı (deterministik). h_legacy=base, h_ext=base+1.
    #[must_use]
    pub fn new(first_handle: u32) -> Self {
        Self {
            next_handle: first_handle,
            ..Default::default()
        }
    }

    /// INIT admit → HANDLE_ASSIGN (idempotent: aynı-attempt-tekrarı AYNI-içerik). HANDLE_ASSIGN'ı
    /// responder-mailbox'a SIRALI enqueue eder (extended'lerden önce) + initiator'a döner.
    pub fn admit_init(&mut self, session_attempt_id: u64, init_counter: u64) -> HandleAssign {
        let key = (session_attempt_id, init_counter);
        if let Some(existing) = self.assignments.get(&key) {
            // idempotent: yeni-atama YOK, aynı-içerik.
            return *existing;
        }
        let h_legacy = self.next_handle;
        let h_ext = self.next_handle + 1;
        self.next_handle += 2;
        let assign = HandleAssign {
            session_attempt_id,
            init_counter,
            h_legacy,
            h_ext,
        };
        self.assignments.insert(key, assign);
        // Responder'a HANDLE_ASSIGN'ı ÖNCE teslim et (sıralama-garantisi).
        self.responder_queue
            .push(ResponderDelivery::HandleAssign(assign));
        self.handle_delivered.insert(h_ext);
        assign
    }

    /// h_ext'li extended-paketi responder'a forward. **SIRALAMA-GARANTİSİ:** HANDLE_ASSIGN önce-teslim
    /// edilmemişse FORWARD-ETMEZ (fail-closed — gerçek-hub admit_init'i önce-çağırır).
    ///
    /// # Errors
    /// [`WireError::InnerParse`] — h_ext için HANDLE_ASSIGN henüz-teslim-edilmemiş (sıralama-ihlali).
    pub fn forward_extended(&mut self, h_ext: u32, bytes: Vec<u8>) -> Result<(), WireError> {
        if !self.handle_delivered.contains(&h_ext) {
            return Err(WireError::InnerParse(
                "hub ordering: extended before HANDLE_ASSIGN",
            ));
        }
        self.responder_queue
            .push(ResponderDelivery::Extended { h_ext, bytes });
        Ok(())
    }

    /// HANDLE_ASSIGN_ACK geldi → retry-durur.
    pub fn record_ack(&mut self, session_attempt_id: u64) {
        self.acked.insert(session_attempt_id);
    }

    /// Bir retry gerekli mi (ACK-yok + bounded-cap-altında). Çağıran-her-deadline'da sorar.
    #[must_use]
    pub fn should_retry(&self, session_attempt_id: u64) -> bool {
        if self.acked.contains(&session_attempt_id) {
            return false;
        }
        self.retries.get(&session_attempt_id).copied().unwrap_or(0) < HANDLE_ASSIGN_MAX_RETRIES
    }

    /// Bir retry gerçekleştir (sayaç-artır). Döner: kalan-hakla retry-yapıldı-mı.
    pub fn do_retry(&mut self, session_attempt_id: u64) -> bool {
        if !self.should_retry(session_attempt_id) {
            return false;
        }
        *self.retries.entry(session_attempt_id).or_insert(0) += 1;
        true
    }

    /// Responder'a teslim-sırası (mailbox) — test sıralamayı doğrular.
    #[must_use]
    pub fn responder_stream(&self) -> &[ResponderDelivery] {
        &self.responder_queue
    }
}
