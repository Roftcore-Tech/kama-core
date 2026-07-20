//! Truncated-PN reconstruction + anti-replay window (§4 KİLİTLİ pseudocode + DR-3 per-width sabitler).
//!
//! İKİ-SABİT-SETİ (DR-3 — kapsam-ayrımı, çelişki-değil):
//! - legacy-seq16 → ccr-§35.8: WINDOW=1024 · FUTURE_GAP=4096 · REORDER=8192 (mühürlü; <2^15-invariant)
//! - extended-pn32 → noise-wire-§4: WINDOW=2^14 · FUTURE_GAP_MAX=2^13 (reorder-eki-YOK)
//!
//! UNDERFLOW-KURALI (§4): tüm −WINDOW/−span/−FUTURE_GAP işlemleri checked/saturating (u64-wrap-YASAK);
//! karşılaştırma-formu `candidate + WINDOW < expected` (çıkarma-yerine-toplama).
//! **Unauthenticated-paket window-İLERLETMEZ:** precheck (iz-bırakmaz) ile commit (yalnız-AEAD-success-
//! sonrası) AYRI fonksiyonlardır; çağıran §4-ORDER'ı uygular.

use crate::error::PnReject;

/// Bir encoding-genişliğinin PN sabit-seti (DR-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PnParams {
    /// Wire'daki düşük-bit genişliği (16 legacy | 32 extended).
    pub low_width_bits: u32,
    /// Anti-replay pencere boyu (bit sayısı).
    pub window: u64,
    /// İleri-sapma tavanı (candidate > expected + bu → reject).
    pub future_gap_max: u64,
    /// ccr-§35.8 MAX_REORDER_DISTANCE (yalnız legacy; |candidate−expected| tavanı).
    pub max_reorder: Option<u64>,
}

/// Extended-pn32 sabitleri (noise-wire §4).
pub const EXTENDED_PN_PARAMS: PnParams = PnParams {
    low_width_bits: 32,
    window: 1 << 14,
    future_gap_max: 1 << 13,
    max_reorder: None,
};

/// Legacy-seq16 sabitleri (ccr §35.8 — mühürlü; DR-3).
pub const LEGACY_PN_PARAMS: PnParams = PnParams {
    low_width_bits: 16,
    window: 1024,
    future_gap_max: 4096,
    max_reorder: Some(8192),
};

/// §4 reconstruct-pseudocode'unun birebir uygulaması (underflow-safe, en-yakın-aday).
#[must_use]
pub fn reconstruct(params: &PnParams, expected_pn: u64, low_bits: u64) -> u64 {
    let span: u64 = 1u64 << params.low_width_bits;
    let half = span / 2;
    let center = expected_pn;
    let base = center & !(span - 1);
    let mut cand = base | low_bits;
    if cand.checked_add(half).is_some_and(|v| v < center) {
        cand += span;
    } else if cand > center.saturating_add(half) && cand >= span {
        cand -= span; // cand < span ise çıkarma-YOK (underflow-guard, §4)
    }
    cand
}

/// Tek dh_epoch'un receive-PN-state'i: expected_pn + sliding-bitmap.
/// `expected_pn` = en-yüksek-kabul+1 (§4); başlangıç 0 (epoch-cipher-state-YENİ, PN-0'dan-başlar).
#[derive(Debug, Clone)]
pub struct ReplayWindow {
    params: PnParams,
    expected_pn: u64,
    bits: Vec<u64>,
}

impl ReplayWindow {
    #[must_use]
    pub fn new(params: PnParams) -> Self {
        let words = usize::try_from(params.window / 64).expect("window/64 usize sığar");
        Self {
            params,
            expected_pn: 0,
            bits: vec![0u64; words.max(1)],
        }
    }

    #[must_use]
    pub fn expected_pn(&self) -> u64 {
        self.expected_pn
    }

    #[must_use]
    pub fn params(&self) -> &PnParams {
        &self.params
    }

    fn bit_is_set(&self, pn: u64) -> bool {
        let idx = (pn % self.params.window) as usize;
        self.bits[idx / 64] & (1u64 << (idx % 64)) != 0
    }

    fn bit_set(&mut self, pn: u64) {
        let idx = (pn % self.params.window) as usize;
        self.bits[idx / 64] |= 1u64 << (idx % 64);
    }

    fn bit_clear(&mut self, pn: u64) {
        let idx = (pn % self.params.window) as usize;
        self.bits[idx / 64] &= !(1u64 << (idx % 64));
    }

    /// §4 accept-adım-4: too-old/dup/future precheck — **window-İLERLETMEZ, iz-BIRAKMAZ.**
    ///
    /// # Errors
    /// [`PnReject`] — hangi kural düştüyse.
    pub fn precheck(&self, candidate: u64) -> Result<(), PnReject> {
        let ew = self.expected_pn;
        let p = &self.params;
        // TOO-OLD: candidate + WINDOW < ew (checked-add; overflow → too-old-DEĞİL).
        if candidate.checked_add(p.window).is_some_and(|v| v < ew) {
            return Err(PnReject::TooOld);
        }
        // DUPLICATE: yalnız pencere-içi geçmiş için bitmap anlamlı.
        if candidate < ew && self.bit_is_set(candidate) {
            return Err(PnReject::Duplicate);
        }
        // FUTURE-GAP: candidate > ew + FUTURE_GAP_MAX (saturating; tavan-aşımı reject).
        if candidate > ew.saturating_add(p.future_gap_max) {
            return Err(PnReject::FutureGap);
        }
        // ccr-§35.8 MAX_REORDER_DISTANCE (yalnız legacy-seti; |cand−ew| tavanı, iki-yön).
        if let Some(mr) = p.max_reorder {
            if candidate.abs_diff(ew) > mr {
                return Err(if candidate >= ew {
                    PnReject::FutureGap
                } else {
                    PnReject::TooOld
                });
            }
        }
        Ok(())
    }

    /// §4 accept-adım-6 COMMIT — YALNIZ AEAD-success-sonrası çağrılır (çağıran-sözleşmesi).
    /// candidate precheck'ten geçmiş olmalı; expected-ilerletme + bitmap-işaretleme.
    pub fn commit(&mut self, candidate: u64) {
        let ew = self.expected_pn;
        if candidate >= ew {
            // İleri-atlama: ring'e girecek yeni-aralığın eski-bit'lerini temizle.
            let advance = candidate - ew + 1;
            if advance >= self.params.window {
                for w in &mut self.bits {
                    *w = 0;
                }
            } else {
                let mut pn = ew;
                while pn <= candidate {
                    self.bit_clear(pn);
                    pn += 1;
                }
            }
            self.bit_set(candidate);
            self.expected_pn = candidate + 1;
        } else {
            // Pencere-içi geç-paket: yalnız işaretle.
            self.bit_set(candidate);
        }
    }
}
