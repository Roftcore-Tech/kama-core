//! Truncated-PN reconstruction + anti-replay window (§4 KİLİTLİ pseudocode + DR-3 per-width sabitler).
//!
//! İKİ-SABİT-SETİ (DR-3 — kapsam-ayrımı, çelişki-değil):
//! - legacy-seq16 → ccr-§35.8: WINDOW=1024 · far-future kapısı YOK ([D-GAP-1] `MAX_FUTURE_GAP` ve
//!   `MAX_REORDER_DISTANCE` kaldırıldı: AEAD-geçerli paket ne kadar ileride olursa olsun kabul edilir;
//!   geriye doğru tek sınır eski-paket kuralıdır)
//! - extended-pn32 → noise-wire-§4: WINDOW=2^14 · FUTURE_GAP_MAX=2^13 (D-GAP-1 kapsamı DIŞINDA; kapı durur)
//!
//! TAŞMA-KURALI (§4 + D-GAP-1): bu modüldeki HİÇBİR aritmetik u64 uç değerinde taşamaz. Far-future
//! reddi kalktığı için `candidate` meşru olarak `expected`ten çok uzakta olabilir; bu yüzden tüm
//! toplamalar checked/saturating'dir ve karşılaştırma formu `candidate + WINDOW < expected`'tir
//! (çıkarma yerine toplama). Kani ispatları: `tests/kani_pn_proofs.rs`.
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
    /// İleri-sapma tavanı (candidate > expected + bu → reject). `None` = far-future kapısı YOK
    /// (legacy-seq16; ccr §35.8 [D-GAP-1]).
    pub future_gap_max: Option<u64>,
}

/// Extended-pn32 sabitleri (noise-wire §4).
pub const EXTENDED_PN_PARAMS: PnParams = PnParams {
    low_width_bits: 32,
    window: 1 << 14,
    future_gap_max: Some(1 << 13),
};

/// Legacy-seq16 sabitleri (ccr §35.8 — mühürlü; DR-3).
///
/// [D-GAP-1] `MAX_FUTURE_GAP = 4096` ve `MAX_REORDER_DISTANCE = 8192` KALDIRILDI. İleri yönde kapı
/// yoktur; `MAX_REORDER_DISTANCE`'ın geri yarısı zaten `window` (1024 < 8192) ile kapsanıyordu.
pub const LEGACY_PN_PARAMS: PnParams = PnParams {
    low_width_bits: 16,
    window: 1024,
    future_gap_max: None,
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
        // İleri aday `cand + span`. u64'ün EN ÜST span bloğunda bu aday YOKTUR (2^64'ü aşar);
        // o durumda eldeki aday kalır. Eskiden `cand += span` burada taşıyordu (Kani K-1).
        if let Some(up) = cand.checked_add(span) {
            cand = up;
        }
    } else if cand > center.saturating_add(half) && cand >= span {
        cand -= span; // cand < span ise çıkarma-YOK (underflow-guard, §4)
    }
    cand
}

/// Tek dh_epoch'un receive-PN-state'i: expected_pn + sliding-bitmap.
/// `expected_pn` = en-yüksek-kabul+1 (§4); başlangıç 0 (oturum başında; epoch geçişinde SIFIRLANMAZ).
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
        // SAYAÇ UZAYININ SONU: `expected = candidate + 1` temsil edilemez. Meşru gönderen buraya
        // ulaşamaz (`SEND_PN_CAP = 2^62`); kabul edilseydi `commit` sonrası aynı PN'in tekrarı
        // duplicate olarak yakalanamazdı.
        if candidate == u64::MAX {
            return Err(PnReject::FutureGap);
        }
        // FUTURE-GAP: yalnız kapısı olan sette (extended-pn32). Legacy'de far-future kapısı YOKTUR
        // ([D-GAP-1]): sahte far-future paket AEAD'de düşer ve precheck PURE olduğu için state'i
        // ilerletemez; kapı yalnız meşru trafikte kalıcı oturum ölümü üretiyordu.
        if let Some(gap) = p.future_gap_max {
            if candidate > ew.saturating_add(gap) {
                return Err(PnReject::FutureGap);
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
            // `dist + 1` (atlanan PN sayısı) HESAPLANMAZ: candidate = u64::MAX, ew = 0 iken taşardı.
            // `dist + 1 >= window`  ⇔  `dist >= window − 1` (window ≥ 1).
            let dist = candidate - ew;
            if dist >= self.params.window.saturating_sub(1) {
                for w in &mut self.bits {
                    *w = 0;
                }
            } else {
                // dist < window − 1 ⇒ döngü en fazla window − 1 tur; `ew + off <= candidate`, taşmaz.
                for off in 0..=dist {
                    self.bit_clear(ew + off);
                }
            }
            self.bit_set(candidate);
            // precheck `candidate == u64::MAX`'ı reddeder; saturating yalnız sözleşme dışı çağrıda
            // (precheck atlanmışsa) paniği önler.
            self.expected_pn = candidate.saturating_add(1);
        } else {
            // Pencere-içi geç-paket: yalnız işaretle.
            self.bit_set(candidate);
        }
    }
}
