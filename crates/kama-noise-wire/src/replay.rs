//! Yön başına TEK replay penceresi (identity §15.5 `replay_precheck` / `replay_commit`; noise-wire
//! §2.5 + §4 [2026-10-06 — PN SÜREKLİ]; ccr §35.8 replay kimliği = `(session_id_internal,
//! direction, full_counter)`). Sayaç epoch'lar boyunca sürekli olduğundan pencere epoch'a
//! BÖLÜNMEZ; `dh_epoch` yalnız anahtar seçimine ve kabul kapısına girer (çağıranda).
//!
//! **Aritmetik bu modülde DEĞİLDİR.** `reconstruct` ve `ReplayWindow::{precheck, commit}`
//! `pn.rs`'te, kani ile ispatlanmış formlarıyla durur (`tests/kani_pn_proofs.rs`); burası onları
//! değiştirmeden çağırır. Bu modül yalnız iki sözleşmeyi tipe gömer:
//!
//! 1. `precheck` SAF'tır (`&self`) ve sonucu bir [`PrecheckedPn`] belgesidir. `commit` YALNIZ bu
//!    belgeyle çağrılabilir — precheck'i atlayan bir commit derlenmez.
//! 2. `commit` TEK mutasyondur; çağıran onu YALNIZ AEAD başarısından sonra çağırır (P0-6:
//!    doğrulanmamış paket pencereyi ilerletemez).
//!
//! Sabitler genişlik başınadır (DR-3): legacy-seq16 → [`crate::pn::LEGACY_PN_PARAMS`] (pencere
//! 1024, far-future kapısı YOK — ccr §35.8 [D-GAP-1]); extended-pn32 →
//! [`crate::pn::EXTENDED_PN_PARAMS`] (pencere 2^14, `FUTURE_GAP_MAX = 2^13` — noise-wire §4).
//! Bir oturumun tek zarf profili olduğundan (oturum başına sabit profil) pencerenin parametresi
//! kurulumda bir kez seçilir.

use crate::error::PnReject;
use crate::pn::{reconstruct, PnParams, ReplayWindow};

/// Precheck'ten geçmiş aday sayaç. Yalnız [`DirectionalReplay::precheck`] üretir; yalnız
/// [`DirectionalReplay::commit`] tüketir (kopyalanamaz — bir belge, bir commit).
#[derive(Debug)]
#[must_use = "precheck belgesi AEAD başarısından sonra commit edilir"]
pub struct PrecheckedPn(u64);

impl PrecheckedPn {
    /// Geri kazanılmış tam sayaç (AEAD nonce'unun girdisi).
    #[must_use]
    pub fn full_pn(&self) -> u64 {
        self.0
    }
}

/// Bir oturumun TEK alım yönü için replay durumu: `expected_pn` + kayan bitmap.
#[derive(Debug, Clone)]
pub struct DirectionalReplay {
    window: ReplayWindow,
}

impl DirectionalReplay {
    /// Boş pencere (`expected_pn = 0`; oturum başında, epoch geçişinde SIFIRLANMAZ).
    #[must_use]
    pub fn new(params: PnParams) -> Self {
        Self {
            window: ReplayWindow::new(params),
        }
    }

    #[must_use]
    pub fn params(&self) -> &PnParams {
        self.window.params()
    }

    /// En yüksek kabul + 1 (hiç kabul yoksa 0).
    #[must_use]
    pub fn expected_pn(&self) -> u64 {
        self.window.expected_pn()
    }

    /// Wire'daki düşük bitlerden tam sayaç adayı (ccr §35.8 / noise-wire §4 `reconstruct`; tek
    /// aday, beraberlik kuralı [C-9]). State'e dokunmaz.
    #[must_use]
    pub fn reconstruct(&self, low_bits: u64) -> u64 {
        reconstruct(self.window.params(), self.window.expected_pn(), low_bits)
    }

    /// too-old / duplicate / `u64::MAX` / (yalnız extended) future-gap kontrolü — SAF.
    ///
    /// # Errors
    /// [`PnReject`] — hangi kural düştüyse; pencere İLERLEMEZ.
    pub fn precheck(&self, candidate: u64) -> Result<PrecheckedPn, PnReject> {
        self.window.precheck(candidate)?;
        Ok(PrecheckedPn(candidate))
    }

    /// TEK mutasyon: `expected_pn` ilerletme + bitmap işaretleme. YALNIZ AEAD başarısından sonra.
    pub fn commit(&mut self, admitted: PrecheckedPn) {
        self.window.commit(admitted.0);
    }

    /// Pencerenin tam durumu (`expected_pn` + bitmap) — H-NO-MUTATE karşılaştırmaları için.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn fingerprint(&self) -> String {
        format!("{:?}", self.window)
    }
}
