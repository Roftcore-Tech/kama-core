//! Transport'un Crypto dikişindeki (S3) YEREL parçaları: PoC stub'ı [`IdentitySeam`] ve entropi
//! enjeksiyonu ([`EntropySource`]).
//!
//! **Sözleşmenin kendisi burada DEĞİLDİR.** S3 trait'leri ve tipleri (`PacketProtection`,
//! `SessionControl`, `ProtectionContext`, …) `kama-contracts` crate'indedir
//! (`kama_contracts::s3`, ortak tutamaçlar `kama_contracts::handles`); bkz.
//! `docs/seams/KM-SEAM-TC-001.md` §1.1 + §1.2. Taşımanın nedeni bağımlılık döngüsüdür: gerçek Crypto
//! `kama-noise-wire`'da yazılır ve bu crate zaten ona bağımlıdır (R-7(b), 2026-10-05).
//!
//! INV-ENGINE-SOC'un çalışma-zamanı sınırı. Transport datagram'ı OPAK taşır: dışarı çıkan her
//! çerçeve `PacketProtection::seal`'den, içeri giren her datagram `PacketProtection::open`'dan
//! geçer. Kripto bu trait'in ARKASINDADIR — bugün [`IdentitySeam`] (stub, +16 B byte-eşdeğer sahte
//! tag); gerçek implementasyon aynı dikiş noktasına takılır.
//!
//! GÜVENLİK İDDİASI YOK: [`IdentitySeam`] kimlik/gizlilik/bütünlük SAĞLAMAZ; yalnız byte-eşdeğer
//! maliyet modeli taşır. `open` reddi sessiz-düşürmedir (verify-before-admission: doğrulanamayan
//! datagram transport durumuna DOKUNMADAN çöpe gider — INV-FEC-1'in alıcı yarısıyla aynı kural).

// ── Sözleşme tipleri: GEÇİCİ yeniden-ihraç (B1 commit 1) ─────────────────────────────────────
// Çağrı noktaları gövde değişikliği olmadan derlensin diye; bir sonraki commit bunu kaldırır ve
// `use` yollarını `kama_contracts`'a çevirir.
pub use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
pub use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedPacketMeta, SealedUnit, SessionCommand, SessionControl, SessionEvent, SessionResult,
};

// ── Entropi ──────────────────────────────────────────────────────────────────────────────────

/// Entropi kaynağı tükendi / kullanılamıyor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("entropy source unavailable")]
pub struct EntropyError;

/// Crypto'nun ephemeral anahtar üretimi için ENJEKTE EDİLEBİLİR entropi kaynağı.
///
/// Üretimde OS kaynağıdır (implementasyonu `kama-noise-wire`'da, A5). Testte deterministik kaynak
/// verilir: SSI1/SSR1 exact vektörleri (A2.3/A2.4) ancak ephemeral anahtar sabitlenebilirse
/// byte-for-byte doğrulanabilir.
pub trait EntropySource: Send + 'static {
    /// `dst`'yi TAMAMEN doldurur ya da hata döner (kısmi doldurma YOK).
    ///
    /// # Errors
    /// [`EntropyError`] — kaynak tükendi.
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError>;
}

/// Önceden verilmiş byte'ları sırayla döndüren deterministik kaynak — YALNIZ TEST.
///
/// Tükenince hata verir (sessizce sıfır ya da tekrar ÜRETMEZ: vektör testinde fazladan bir
/// ephemeral çekilirse bu görünür olmalı).
#[cfg(any(test, feature = "test-util"))]
#[derive(Debug, Clone)]
pub struct FixedEntropy {
    bytes: Vec<u8>,
    pos: usize,
}

#[cfg(any(test, feature = "test-util"))]
impl FixedEntropy {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Henüz tüketilmemiş bayt sayısı.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}

#[cfg(any(test, feature = "test-util"))]
impl EntropySource for FixedEntropy {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        if dst.len() > self.remaining() {
            return Err(EntropyError);
        }
        dst.copy_from_slice(&self.bytes[self.pos..self.pos + dst.len()]);
        self.pos += dst.len();
        Ok(())
    }
}

// ── IdentitySeam ─────────────────────────────────────────────────────────────────────────────

/// AEAD-tag SİMÜLASYONU: her datagram'a +16 B sıfır-tag ekler/soyar (byte-eşdeğer maliyet modeli;
/// kampanya TAG_STUB soyu). Güvenlik iddiası olmayan stub.
///
/// Tek örtük oturumu vardır ([`IdentitySeam::SESSION`]); `open` her datagram'ı ona atfeder (peer
/// ayrımı YOK). PN'i kendisi sayar (K2), bağlam sınıfı başına AYRI sayaçla; sayaç wire'a YAZILMAZ,
/// dolayısıyla v1'e göre wire byte'ları DEĞİŞMEDİ.
#[derive(Debug, Default)]
pub struct IdentitySeam {
    session_pn: u64,
    hub_pn: u64,
    /// `seal()`'e en son verilen bağlam (S3 K1 kayıt kancası; YALNIZ test).
    #[cfg(any(test, feature = "test-util"))]
    last_seal_ctx: Option<ProtectionContext>,
}

/// Sahte tag boyu (ChaCha20-Poly1305 tag'iyle byte-eşdeğer).
pub const IDENTITY_TAG: usize = 16;

/// Bir UDP/IPv4 datagramının taşıyabileceği azami payload (65 535 − 20 IP − 8 UDP).
const MAX_UDP_PAYLOAD: usize = 65_507;

impl IdentitySeam {
    /// Stub'ın tek örtük oturumu.
    pub const SESSION: SessionHandle = SessionHandle::from_raw(0);

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `seal()`'e en son verilen bağlam (K1: bağlam çağırandan gelir, seam'e değişmeden ulaşır).
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn last_seal_ctx(&self) -> Option<ProtectionContext> {
        self.last_seal_ctx
    }

    const LIMITS: ProtectionLimits = ProtectionLimits {
        wire_overhead_bytes: IDENTITY_TAG as u32,
        max_plaintext_bytes: (MAX_UDP_PAYLOAD - IDENTITY_TAG) as u32,
    };

    fn check(ctx: ProtectionContext, plaintext_len: usize) -> Result<(), SealError> {
        if let ProtectionContext::Session(h) = ctx {
            if h != Self::SESSION {
                return Err(SealError::UnknownSession);
            }
        }
        if plaintext_len > Self::LIMITS.max_plaintext_bytes as usize {
            return Err(SealError::FrameTooLarge);
        }
        Ok(())
    }
}

impl PacketProtection for IdentitySeam {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        Self::check(ctx, frame.len())?;
        #[cfg(any(test, feature = "test-util"))]
        {
            self.last_seal_ctx = Some(ctx);
        }
        let counter = match ctx {
            ProtectionContext::Session(_) => &mut self.session_pn,
            ProtectionContext::HubChannel => &mut self.hub_pn,
        };
        let pn = *counter;
        // Sayaç SARMAZ (K2: wrap = aynı anahtar altında nonce tekrarı).
        *counter = pn.checked_add(1).ok_or(SealError::CapReached)?;
        let mut bytes = Vec::with_capacity(frame.len() + IDENTITY_TAG);
        bytes.extend_from_slice(frame);
        bytes.resize(frame.len() + IDENTITY_TAG, 0);
        let sealed_len = bytes.len() as u32; // ≤ MAX_UDP_PAYLOAD (check)
        Ok(SealedUnit {
            bytes,
            meta: SealedPacketMeta {
                full_wire_pn: pn,
                dh_epoch: 0,
                sealed_len,
            },
        })
    }

    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        if wire.len() <= IDENTITY_TAG || wire.len() > MAX_UDP_PAYLOAD {
            return None;
        }
        Some(Opened {
            ctx: ProtectionContext::Session(Self::SESSION),
            frame: wire[..wire.len() - IDENTITY_TAG].to_vec(),
            meta: SealedPacketMeta {
                // Stub PN'i wire'da taşımaz; alım tarafında bilinemez.
                full_wire_pn: 0,
                dh_epoch: 0,
                sealed_len: wire.len() as u32,
            },
        })
    }

    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        Self::check(ctx, plaintext_len)?;
        Ok(plaintext_len + IDENTITY_TAG)
    }

    fn conservative_limits(&self, _class: ProtectionClass) -> ProtectionLimits {
        Self::LIMITS
    }

    fn limits(&self, _ctx: ProtectionContext) -> ProtectionLimits {
        Self::LIMITS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: ProtectionContext = ProtectionContext::Session(IdentitySeam::SESSION);

    #[test]
    fn sealed_len_is_exact_and_bytes_match_v1_wire() {
        let mut seam = IdentitySeam::new();
        let frame = [0xABu8; 37];
        let u = seam.seal(S, &frame).expect("seal");
        // v1 wire'ı: çerçeve + 16 sıfır bayt. Değişmedi.
        assert_eq!(&u.bytes[..37], &frame);
        assert_eq!(&u.bytes[37..], &[0u8; IDENTITY_TAG]);
        assert_eq!(u.bytes.len(), seam.sealed_len(S, frame.len()).expect("len"));
        assert_eq!(u.meta.sealed_len as usize, u.bytes.len());
        let o = seam.open(&u.bytes).expect("open");
        assert_eq!(o.frame, frame);
        assert_eq!(o.ctx, S);
    }

    #[test]
    fn pn_is_owned_by_the_seam_and_separate_per_context_class() {
        let mut seam = IdentitySeam::new();
        let pns: Vec<u64> = (0..3)
            .map(|_| seam.seal(S, b"x").expect("seal").meta.full_wire_pn)
            .collect();
        assert_eq!(pns, [0, 1, 2], "Session PN ardışık ve Crypto'da (K2)");
        let hub = seam
            .seal(ProtectionContext::HubChannel, b"x")
            .expect("seal")
            .meta
            .full_wire_pn;
        assert_eq!(hub, 0, "HubChannel AYRI PN uzayı (K1)");
        assert_eq!(seam.seal(S, b"x").expect("seal").meta.full_wire_pn, 3);
    }

    #[test]
    fn pn_never_wraps() {
        let mut seam = IdentitySeam {
            session_pn: u64::MAX,
            ..IdentitySeam::default()
        };
        assert_eq!(seam.seal(S, b"x").unwrap_err(), SealError::CapReached);
        assert_eq!(
            seam.session_pn,
            u64::MAX,
            "reddedilen seal sayacı ilerletmez"
        );
    }

    #[test]
    fn structural_rejects_are_typed_and_open_is_silent() {
        let mut seam = IdentitySeam::new();
        let other = ProtectionContext::Session(SessionHandle::from_raw(7));
        assert_eq!(
            seam.seal(other, b"x").unwrap_err(),
            SealError::UnknownSession
        );
        assert_eq!(
            seam.sealed_len(other, 1).unwrap_err(),
            SealError::UnknownSession
        );
        let big = vec![0u8; MAX_UDP_PAYLOAD];
        assert_eq!(seam.seal(S, &big).unwrap_err(), SealError::FrameTooLarge);
        assert!(
            seam.open(&[0u8; IDENTITY_TAG]).is_none(),
            "tag kadar kısa → sessiz drop"
        );
        assert!(seam.open(&[]).is_none());
    }

    #[test]
    fn fixed_entropy_is_exact_and_fails_closed_when_exhausted() {
        let mut e = FixedEntropy::new((0u8..40).collect());
        let mut a = [0u8; 32];
        e.fill(&mut a).expect("ilk 32");
        assert_eq!(a[0], 0);
        assert_eq!(a[31], 31);
        let mut b = [0u8; 32];
        assert_eq!(
            e.fill(&mut b),
            Err(EntropyError),
            "8 bayt kaldı; kısmi doldurma yok"
        );
        assert_eq!(b, [0u8; 32]);
        assert_eq!(e.remaining(), 8);
    }
}
