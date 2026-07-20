//! INIT-flight-machine (§0.7-B PATCH-8 + EARLY-DATA-REPLAY MADDE-5 + DR-2).
//!
//! **DR-2: AYRI-KATMAN.** PreConverge (CC-cold-start, data-path) ile KARIŞTIRILMAZ. Bu, mühürlü-N-türevi-
//! handshake'in *paket-teslim* katmanı (pattern-DEĞİL). Gönderim: **bounded-idempotent-SESSION_INIT-
//! retransmission**. KEYSTONE-Noise-AEAD-probe bu-machine'in-parçası.
//!
//! MADDE-5 CACHED-BYTE-FOR-BYTE: SESSION_INIT bir-kez-seal → retry'larda cached-datagram-byte'ları AYNEN.
//! **Aynı-init_counter yeniden-serialization/padding/encryption YASAK** (nonce-reuse). Payload-değişecekse:
//! YENİ init_counter + YENİ ephemeral + YENİ session-attempt. [Bootstrap-istisnası: §3 yeni-PN/nonce-
//! kuralından AYRI.] Anti-amplification: unauthenticated-input sınırsız-response-ÜRETMEZ.

use std::collections::HashMap;

// ── GÖNDERİM-tarafı: bounded-idempotent INIT-flight ──

/// Retry-sabitleri (§0.7-B: count + toplam-süre + backoff SABİTLERLE-bounded). [CAL — D0/D1-ölç-ve-kilitle].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitRetryConfig {
    /// Maksimum retransmit sayısı (ilk-gönderim hariç).
    pub max_retries: u32,
    /// İlk backoff (ms).
    pub backoff_base_ms: u64,
    /// Backoff çarpanı (exponential; ×mult her-adım).
    pub backoff_mult: u64,
    /// Toplam-retry-süresi üst-sınırı (ms).
    pub max_total_ms: u64,
}

impl InitRetryConfig {
    /// Alpha-varsayılan [CAL — ölçümle-kilitlenir]: 5 retry, 200ms base, ×2, ≤10s.
    #[must_use]
    pub const fn alpha_default() -> Self {
        Self {
            max_retries: 5,
            backoff_base_ms: 200,
            backoff_mult: 2,
            max_total_ms: 10_000,
        }
    }
}

/// INIT-flight durumu (initiator). Cached-datagram bir-kez-seal-edilir; retry'lar AYNEN-tekrar.
#[derive(Debug, Clone)]
pub struct InitFlight {
    /// bir-kez-seal-edilmiş SESSION_INIT datagram'ı (byte-for-byte cached).
    cached_datagram: Vec<u8>,
    /// bu-flight'ın init_counter'ı (replay-identity; değişmez).
    init_counter: u64,
    config: InitRetryConfig,
    retries_done: u32,
    elapsed_ms: u64,
    confirmed: bool,
}

/// Bir retransmit-adımının sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetransmitOutcome<'a> {
    /// Cached-byte'lar AYNEN gönderilir (byte-for-byte); sonraki-deadline `next_delay_ms`.
    Send {
        datagram: &'a [u8],
        next_delay_ms: u64,
    },
    /// Bounded-tükendi (count veya toplam-süre) — flight-durur.
    Exhausted,
    /// SENDER_CONFIRMED (§0.7-B-B) — retry-durur.
    Confirmed,
}

impl InitFlight {
    /// SESSION_INIT'i bir-kez seal edip cache'ler (çağıran seal-eder; bu-tip anahtarı-TUTMAZ → re-seal-imkansız).
    #[must_use]
    pub fn new(cached_datagram: Vec<u8>, init_counter: u64, config: InitRetryConfig) -> Self {
        Self {
            cached_datagram,
            init_counter,
            config,
            retries_done: 0,
            elapsed_ms: 0,
            confirmed: false,
        }
    }

    #[must_use]
    pub fn init_counter(&self) -> u64 {
        self.init_counter
    }
    #[must_use]
    pub fn cached_datagram(&self) -> &[u8] {
        &self.cached_datagram
    }

    /// SENDER_CONFIRMED: hayatta-kalan-ilk-ACK/RESPONSE geldi → retry-durur.
    pub fn on_confirmation(&mut self) {
        self.confirmed = true;
    }

    /// Bir sonraki retransmit'i üret. **AYNEN cached-byte** (re-seal YOK; nonce-reuse-guard yapısal).
    /// Bounded: retries_done < max_retries VE elapsed + next_delay ≤ max_total.
    pub fn retransmit(&mut self) -> RetransmitOutcome<'_> {
        if self.confirmed {
            return RetransmitOutcome::Confirmed;
        }
        if self.retries_done >= self.config.max_retries {
            return RetransmitOutcome::Exhausted;
        }
        // Exponential backoff: base × mult^retries_done.
        let delay = self
            .config
            .backoff_base_ms
            .saturating_mul(self.config.backoff_mult.saturating_pow(self.retries_done));
        if self.elapsed_ms.saturating_add(delay) > self.config.max_total_ms {
            return RetransmitOutcome::Exhausted;
        }
        self.retries_done += 1;
        self.elapsed_ms = self.elapsed_ms.saturating_add(delay);
        RetransmitOutcome::Send {
            datagram: &self.cached_datagram,
            next_delay_ms: delay,
        }
    }

    #[must_use]
    pub fn retries_done(&self) -> u32 {
        self.retries_done
    }
}

// ── ALIM-tarafı: INIT-admission (replay-identity + dedup + cert-mismatch) ──

/// INIT-replay-identity (MADDE-5, TEK-KİMLİK): (sender_device_id, recipient_device_id, init_counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InitReplayIdentity {
    pub sender_device_id: [u8; 16],
    pub recipient_device_id: [u8; 16],
    pub init_counter: u64,
}

/// INIT-admission sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitAdmission {
    /// İlk-görülüş: session-kurulur, early-data-kabul (RECEIVER_ESTABLISHED).
    Fresh,
    /// Duplicate-INIT (aynı-identity+aynı-cert): yeni-session-YOK, app-2.-teslim-YOK; confirmation-YENİDEN-gönder.
    DuplicateResendConfirmation,
    /// Aynı-init_counter FARKLI-sender-cert → REJECT (MADDE-5).
    RejectCertMismatch,
}

/// INIT-alım tarafı: replay-identity-dedup + cert-mismatch. Deterministik.
#[derive(Debug, Default)]
pub struct InitReceiver {
    /// identity → sender-cert-fingerprint (cert-mismatch tespiti).
    seen: HashMap<InitReplayIdentity, [u8; 32]>,
}

impl InitReceiver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// INIT admit (MADDE-5). İlk-görülüş→Fresh; duplicate(aynı-cert)→DuplicateResendConfirmation;
    /// aynı-identity-FARKLI-cert→RejectCertMismatch.
    pub fn admit(
        &mut self,
        identity: InitReplayIdentity,
        sender_cert_fp: [u8; 32],
    ) -> InitAdmission {
        match self.seen.get(&identity) {
            None => {
                self.seen.insert(identity, sender_cert_fp);
                InitAdmission::Fresh
            }
            Some(known) if *known == sender_cert_fp => InitAdmission::DuplicateResendConfirmation,
            Some(_) => InitAdmission::RejectCertMismatch,
        }
    }

    #[must_use]
    pub fn established_count(&self) -> usize {
        self.seen.len()
    }
}

// ── Anti-amplification (§0.7-B): unauthenticated-input sınırsız-response-ÜRETMEZ ──

/// Anti-amplification guard: doğrulanmamış-girdiye üretilen-response bounded (≤ factor × request).
/// cookie/return-routability YALNIZ gerçek-amplification-yüzeyi-varsa (bu-guard onu-modelleyip-sınırlar).
#[derive(Debug, Clone, Copy)]
pub struct AntiAmplification {
    /// response ≤ factor × request_bytes (klasik anti-amplification, örn 3×).
    pub factor: u64,
}

impl AntiAmplification {
    #[must_use]
    pub const fn new(factor: u64) -> Self {
        Self { factor }
    }

    /// Doğrulanmamış `request_bytes`e izin-verilen-max-response-byte. Aşan-response GÖNDERİLMEZ.
    #[must_use]
    pub fn max_response_bytes(&self, request_bytes: u64) -> u64 {
        request_bytes.saturating_mul(self.factor)
    }

    /// `response_bytes` doğrulanmamış-request için izinli-mi (amplification-sınırı).
    #[must_use]
    pub fn allows(&self, request_bytes: u64, response_bytes: u64) -> bool {
        response_bytes <= self.max_response_bytes(request_bytes)
    }
}
