//! Sealed OPENED-tip (§6 TYPE-LEVEL-ENFORCEMENT, K-WIRE-SEALED-TYPE-1):
//! alanlar PRIVATE + `_proof: PrivateAdmissionProof` (public-constructor-YOK) + YALNIZ
//! `KamaNoiseWireProtector::open()` üretir + test-constructor ayrı (`test-util` feature / cfg(test)).
//! Dış-crate üretim-denemesi DERLENMEZ (trybuild-testi).

use crate::inner::InnerPlain;

/// Admission-kanıtı: bu tipi yalnız bu crate üretebilir (alan private, constructor yok).
#[derive(Debug)]
pub struct PrivateAdmissionProof {
    _priv: (),
}

impl PrivateAdmissionProof {
    pub(crate) fn new_crate_only() -> Self {
        Self { _priv: () }
    }
}

/// AEAD-doğrulanmış + replay-geçmiş + inner-parse-edilmiş paket (§4-ORDER adım-8 handoff).
/// FEC-decoder'a YALNIZ bu tip girer (verify-before-admission sahibi bu-katman, §6).
#[derive(Debug)]
pub struct OpenedTransportShard {
    inner: InnerPlain,
    full_pn: u64,
    dh_epoch: u8,
    h_ext: u32,
    _proof: PrivateAdmissionProof,
}

impl OpenedTransportShard {
    pub(crate) fn new_authenticated(
        inner: InnerPlain,
        full_pn: u64,
        dh_epoch: u8,
        h_ext: u32,
    ) -> Self {
        Self {
            inner,
            full_pn,
            dh_epoch,
            h_ext,
            _proof: PrivateAdmissionProof::new_crate_only(),
        }
    }

    /// Test-constructor (K-WIRE-SEALED-TYPE-1: ayrı, production-yolunda-DEĞİL).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn test_new(inner: InnerPlain, full_pn: u64, dh_epoch: u8, h_ext: u32) -> Self {
        Self::new_authenticated(inner, full_pn, dh_epoch, h_ext)
    }

    #[must_use]
    pub fn inner(&self) -> &InnerPlain {
        &self.inner
    }

    #[must_use]
    pub fn into_inner(self) -> InnerPlain {
        self.inner
    }

    #[must_use]
    pub fn full_pn(&self) -> u64 {
        self.full_pn
    }

    #[must_use]
    pub fn dh_epoch(&self) -> u8 {
        self.dh_epoch
    }

    #[must_use]
    pub fn h_ext(&self) -> u32 {
        self.h_ext
    }
}
