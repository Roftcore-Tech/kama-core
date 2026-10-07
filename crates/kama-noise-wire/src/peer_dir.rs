//! Doğrulanmış eş dizini — ccr §35.4 KAPALI-SİSTEM KİPİ (sahip kararı G1, 2026-10-08).
//!
//! Crypto'ya ÖNCEDEN doğrulanmış bir eş dizini enjekte edilir: eş başına `dest_hash`, statik X25519
//! (`device_x25519_pub`) ve `sender_cert_ref`. Dizine giren her kayıt ccr §35.4'ün 1–4. adımlarını
//! dizin kurulurken (bant dışı) geçmiş sayılır; çalışma zamanında SESSION_INIT'in gönderen kimliği
//! (`sender_cert_ref`) dizinle eşleşmeli, eşleşmeyenin INIT'i opak `Failed`'dır. Zincir
//! (`verify_chain_to_root`) ve iptal doğrulaması bu kipte ÇALIŞMA ZAMANINDA YAPILMAZ — tipli sınır
//! olarak kapsam dışıdır (BL KATMAN 6 madde 7). Hub-untrusted iddiası dizinin bütünlüğüne dayanır.
//!
//! Ürün kurulumu dizinsiz yapılamaz: oturum katmanının kurucusu [`PeerDirectory`] ister.

use std::collections::BTreeMap;

use x25519_dalek::{PublicKey, StaticSecret};

use crate::handshake::{CERT_REF_LEN, DEST_HASH_LEN, X25519_LEN};

/// Dizindeki bir eş.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PeerEntry {
    pub dest_hash: [u8; DEST_HASH_LEN],
    pub static_x25519_pub: [u8; X25519_LEN],
    pub cert_ref: [u8; CERT_REF_LEN],
}

impl core::fmt::Debug for PeerEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Kimlik özetleri log'a düşmez (PeerRef ile aynı kural).
        f.debug_struct("PeerEntry").finish_non_exhaustive()
    }
}

/// Dizin kurulamadı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DirectoryError {
    #[error("duplicate dest_hash in peer directory")]
    DuplicateDestHash,
    #[error("duplicate cert_ref in peer directory")]
    DuplicateCertRef,
    #[error("all-zero static x25519 key in peer directory")]
    AllZeroStaticKey,
}

/// Doğrulanmış eş dizini — kurulumda bir kez verilir, çalışma zamanında BÜYÜMEZ (boyu çağıranın
/// verdiği kayıt sayısıdır).
#[derive(Clone, Default)]
pub struct PeerDirectory {
    by_dest: BTreeMap<[u8; DEST_HASH_LEN], PeerEntry>,
    by_cert: BTreeMap<[u8; CERT_REF_LEN], [u8; DEST_HASH_LEN]>,
}

impl core::fmt::Debug for PeerDirectory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PeerDirectory")
            .field("entries", &self.by_dest.len())
            .finish()
    }
}

impl PeerDirectory {
    /// # Errors
    /// Aynı `dest_hash` ya da aynı `cert_ref` iki kez · tüm-sıfır statik anahtar (ccr §5.1).
    pub fn new(entries: impl IntoIterator<Item = PeerEntry>) -> Result<Self, DirectoryError> {
        let mut dir = Self::default();
        for e in entries {
            if e.static_x25519_pub == [0u8; X25519_LEN] {
                return Err(DirectoryError::AllZeroStaticKey);
            }
            if dir.by_dest.contains_key(&e.dest_hash) {
                return Err(DirectoryError::DuplicateDestHash);
            }
            if dir.by_cert.contains_key(&e.cert_ref) {
                return Err(DirectoryError::DuplicateCertRef);
            }
            dir.by_cert.insert(e.cert_ref, e.dest_hash);
            dir.by_dest.insert(e.dest_hash, e);
        }
        Ok(dir)
    }

    #[must_use]
    pub fn by_dest_hash(&self, dest_hash: &[u8; DEST_HASH_LEN]) -> Option<&PeerEntry> {
        self.by_dest.get(dest_hash)
    }

    #[must_use]
    pub fn by_cert_ref(&self, cert_ref: &[u8; CERT_REF_LEN]) -> Option<&PeerEntry> {
        self.by_cert.get(cert_ref).and_then(|d| self.by_dest.get(d))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_dest.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_dest.is_empty()
    }
}

/// Bu ucun kimliği: statik X25519 sırrı + yayınlanan `dest_hash` ve `cert_ref`.
pub struct LocalIdentity {
    static_secret: StaticSecret,
    static_public: PublicKey,
    pub dest_hash: [u8; DEST_HASH_LEN],
    pub cert_ref: [u8; CERT_REF_LEN],
}

impl core::fmt::Debug for LocalIdentity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Sır ve kimlik özetleri Debug'a GİRMEZ.
        f.debug_struct("LocalIdentity").finish_non_exhaustive()
    }
}

impl LocalIdentity {
    #[must_use]
    pub fn new(
        static_secret: StaticSecret,
        dest_hash: [u8; DEST_HASH_LEN],
        cert_ref: [u8; CERT_REF_LEN],
    ) -> Self {
        let static_public = PublicKey::from(&static_secret);
        Self {
            static_secret,
            static_public,
            dest_hash,
            cert_ref,
        }
    }

    #[must_use]
    pub fn static_secret(&self) -> &StaticSecret {
        &self.static_secret
    }

    #[must_use]
    pub fn static_public(&self) -> &PublicKey {
        &self.static_public
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: u8) -> PeerEntry {
        PeerEntry {
            dest_hash: [n; DEST_HASH_LEN],
            static_x25519_pub: [n.wrapping_add(1); X25519_LEN],
            cert_ref: [n.wrapping_add(2); CERT_REF_LEN],
        }
    }

    #[test]
    fn lookups_by_dest_and_cert_agree() {
        let dir = PeerDirectory::new([entry(1), entry(9)]).unwrap();
        assert_eq!(dir.len(), 2);
        assert_eq!(dir.by_dest_hash(&[1; 32]), Some(&entry(1)));
        assert_eq!(dir.by_cert_ref(&[11; 32]), Some(&entry(9)));
        assert_eq!(dir.by_cert_ref(&[0xEE; 32]), None);
    }

    #[test]
    fn duplicates_and_zero_keys_are_refused() {
        assert_eq!(
            PeerDirectory::new([entry(1), entry(1)]).unwrap_err(),
            DirectoryError::DuplicateDestHash
        );
        let mut clash = entry(5);
        clash.cert_ref = entry(1).cert_ref;
        assert_eq!(
            PeerDirectory::new([entry(1), clash]).unwrap_err(),
            DirectoryError::DuplicateCertRef
        );
        let mut zero = entry(3);
        zero.static_x25519_pub = [0; 32];
        assert_eq!(
            PeerDirectory::new([zero]).unwrap_err(),
            DirectoryError::AllZeroStaticKey
        );
    }
}
