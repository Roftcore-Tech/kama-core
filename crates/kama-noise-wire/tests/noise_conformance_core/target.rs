//! Hedef katmanı — S3 sözleşme süiti "hangi Crypto'ya karşı koşuyorum" sorusu.
//!
//! Süit yalnız S3 yüzeyini (`SessionControl` / `PacketProtection`) ve aşağıdaki FİKSTÜR kancalarını
//! kullanır. Kancalar sözleşmenin parçası değildir: SSR1'i üretecek karşı ucu ve gözlem noktalarını
//! (deneme sayacı, state parmak izi) hedefe özgü biçimde sağlarlar. Gerçek hedef aynı kancaları
//! gerçek el sıkışmayla doldurur; satır gövdeleri değişmez.

#![allow(dead_code)]

use client_core::seam::{PacketProtection, ProtectionContext, SessionControl, SessionHandle};
use client_core::seam_mock::{self, MockCrypto};

/// Gerçek kriptonun yokluğunun kimliği (koşucu ile paylaşılır).
pub const GAP_REAL_CRYPTO: &str = "GAP-S3-REAL-A5";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Mock,
    Real,
}

/// Hedef kurulamadı — typed; panik değil, `todo!()` değil (CLAUDE.md §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetUnavailable {
    NotImplemented { gap: &'static str },
}

/// Pending kümesi yeni INIT kabul etmedi (ccr §35.15: istemci pending INIT tavanı).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingRefused;

pub trait S3Target {
    type Crypto: SessionControl + PacketProtection;
    const ID: &'static str;
    const KIND: TargetKind;
    /// ccr §35.15 `PENDING_RESPONSE_TRIAL_CAP` — hedefin ADLANDIRILMIŞ sabitinden gelir.
    const TRIAL_CAP: usize;

    /// # Errors
    /// Hedef bu derlemede yoksa [`TargetUnavailable`] (beyan edilmiş boşluk).
    fn build() -> Result<Self::Crypto, TargetUnavailable>;

    /// Yanıt bekleyen bir INIT kaydeder; `tag` yalnız fikstürün o INIT'i sonradan anmasına yarar.
    ///
    /// # Errors
    /// Pending kümesi tavandaysa [`PendingRefused`].
    fn add_pending(c: &mut Self::Crypto, tag: u64) -> Result<(), PendingRefused>;

    /// `tag`'li INIT'e karşı ucun üreteceği GEÇERLİ RESPONSE zarfı.
    fn response_for(tag: u64) -> Vec<u8>;

    /// Geçerli bir RESPONSE zarfının doğrulama alanı (tag) bozulmuş kopyası.
    fn corrupt(valid: &[u8]) -> Vec<u8>;

    /// Son `AcceptResponse`'ta yapılan aday denemesi sayısı (K7 gözlem noktası).
    fn last_trials(c: &Self::Crypto) -> usize;

    fn pending_len(c: &Self::Crypto) -> usize;

    /// Pending kümesi + oturum/replay state'inin parmak izi (H-NO-MUTATE karşılaştırmaları).
    /// Deneme sayacı gibi salt-gözlem alanları da içindedir; karşılaştıran taraf bunu bilir.
    fn fingerprint(c: &Self::Crypto) -> String;

    // ── A4.4 kancaları ──

    /// Hedefte `Session` ve `HubChannel` ayrı anahtarla mı mühürlenir (H-CTX'in anahtar yarısı).
    const CONTEXTS_ARE_KEY_SEPARATED: bool;

    /// `seal()`'e en son verilen bağlam — seam'in kendi kaydı (K1 kayıt kancası).
    fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext>;

    /// Oturum/replay/pending/anahtar state'inin parmak izi; salt-gözlem alanları HARİÇ.
    fn state_fingerprint(c: &Self::Crypto) -> String;

    /// Oturumu epoch-1'e geçirir (gönderim tarafı). Hedef geçişi yapamıyorsa `false`.
    fn enter_epoch1(c: &mut Self::Crypto, session: SessionHandle) -> bool;
}

/// Kayıtlı hedefler — `missing_target_is_a_declared_gap` bu listeye karşı sınar.
pub const REGISTERED_TARGETS: &[(&str, TargetKind)] = &[(MockTarget::ID, TargetKind::Mock)];

// ── Sahte hedef ──────────────────────────────────────────────────────────────────────────────

pub struct MockTarget;

impl S3Target for MockTarget {
    type Crypto = MockCrypto;
    const ID: &'static str = "mock";
    const KIND: TargetKind = TargetKind::Mock;
    const TRIAL_CAP: usize = seam_mock::PENDING_RESPONSE_TRIAL_CAP;

    fn build() -> Result<Self::Crypto, TargetUnavailable> {
        Ok(MockCrypto::new())
    }

    fn add_pending(c: &mut Self::Crypto, tag: u64) -> Result<(), PendingRefused> {
        c.add_pending(tag).map_err(|_| PendingRefused)
    }

    fn response_for(tag: u64) -> Vec<u8> {
        MockCrypto::response_envelope(tag)
    }

    fn corrupt(valid: &[u8]) -> Vec<u8> {
        // Sahtede RESPONSE'un tamamı doğrulama alanıdır (ayrı tag yok). En yüksek bit çevrilir:
        // en düşük bit çevrilseydi sonuç KOMŞU bir pending'in geçerli RESPONSE'u olurdu ve
        // "bozuk paket" yerine "başka aday" sınanırdı (ilk yazımda ölçüldü).
        let mut w = valid.to_vec();
        if let Some(first) = w.first_mut() {
            *first ^= 0x80;
        }
        w
    }

    fn last_trials(c: &Self::Crypto) -> usize {
        c.last_response_trials
    }

    fn pending_len(c: &Self::Crypto) -> usize {
        c.pending_len()
    }

    fn fingerprint(c: &Self::Crypto) -> String {
        format!("{c:?}")
    }

    const CONTEXTS_ARE_KEY_SEPARATED: bool = true;

    fn last_seal_ctx(c: &Self::Crypto) -> Option<ProtectionContext> {
        c.last_seal_ctx()
    }

    fn state_fingerprint(c: &Self::Crypto) -> String {
        c.state_fingerprint()
    }

    fn enter_epoch1(c: &mut Self::Crypto, session: SessionHandle) -> bool {
        c.mock_set_epoch(session, 1)
    }
}

// ── Gerçek hedef (A5'e kadar beyan edilmiş boşluk) ───────────────────────────────────────────

/// Derlenir, koşmaz. `Crypto` tipi A5.5'te gerçek implementasyonla değiştirilir.
pub struct RealTarget;

impl S3Target for RealTarget {
    type Crypto = MockCrypto;
    const ID: &'static str = "real";
    const KIND: TargetKind = TargetKind::Real;
    const TRIAL_CAP: usize = seam_mock::PENDING_RESPONSE_TRIAL_CAP;

    fn build() -> Result<Self::Crypto, TargetUnavailable> {
        Err(TargetUnavailable::NotImplemented {
            gap: GAP_REAL_CRYPTO,
        })
    }

    fn add_pending(_: &mut Self::Crypto, _: u64) -> Result<(), PendingRefused> {
        Err(PendingRefused)
    }

    fn response_for(_: u64) -> Vec<u8> {
        Vec::new()
    }

    fn corrupt(valid: &[u8]) -> Vec<u8> {
        valid.to_vec()
    }

    fn last_trials(_: &Self::Crypto) -> usize {
        0
    }

    fn pending_len(_: &Self::Crypto) -> usize {
        0
    }

    fn fingerprint(_: &Self::Crypto) -> String {
        String::new()
    }

    const CONTEXTS_ARE_KEY_SEPARATED: bool = true;

    fn last_seal_ctx(_: &Self::Crypto) -> Option<ProtectionContext> {
        None
    }

    fn state_fingerprint(_: &Self::Crypto) -> String {
        String::new()
    }

    fn enter_epoch1(_: &mut Self::Crypto, _: SessionHandle) -> bool {
        false
    }
}
