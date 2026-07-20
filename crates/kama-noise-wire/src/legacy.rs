//! Legacy SESSION_DATA (22B) codec — ccr §35.6/§35.9 AKTİF-format (D2-DEĞİL). MÜHÜRLÜ-envelope OKUNUR,
//! yeniden-TANIMLANMAZ (§0.9): wire = `[hub_handle 4][seq16 2][ct][tag 16]` = 22B-overhead (byte-for-byte).
//!
//! **AAD (§35.9, 28B):** `"SSD1" ‖ protocol_version ‖ cipher_suite_id_be ‖ direction ‖ epoch_be ‖ seq16_be ‖
//! session_id_internal(16B)`. hub_handle AAD'ye GİRMEZ (hub-local-remap-serbest). session_id_internal wire'da-YOK.
//! **nonce96 = LE64(full_pn)** (§4.5-aynı-mapping). **seq16→full-counter: DR-3 LEGACY sabitleri** (WINDOW=1024/
//! FUTURE_GAP=4096/REORDER=8192, extended-2^14/2^13'ten AYRI — iki-yol-iki-sabit).
//!
//! Mock-key (Split/DH mühürlü-pattern; Sprint-6+). PN-namespace §2.5: legacy-seq16 + extended-pn32 AYNI
//! full-PN'in-düşük-bitleri → production tek-PN-allocator (bu-codec kendi-sayacıyla test-edilir).

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use zeroize::Zeroizing;

use crate::error::WireError;
use crate::pn::{reconstruct, ReplayWindow, LEGACY_PN_PARAMS};
use crate::protector::{nonce96, KEY_LEN};

/// SESSION_DATA domain-separator (§35.9; AAD-only, wire'da YOK). ASCII "SSD1".
pub const SESSION_DATA_MAGIC: [u8; 4] = *b"SSD1";
/// §35.9 AAD toplam uzunluğu: 4+1+2+1+2+2+16 = 28B.
pub const LEGACY_AAD_LEN: usize = 28;
/// §35.6 wire-overhead: hub_handle 4 + seq16 2 + tag 16 = 22B.
pub const LEGACY_OVERHEAD: usize = 22;
const HDR_LEN: usize = 6; // hub_handle 4 + seq16 2
const TAG_LEN: usize = 16;

/// Mesaj-yönü (§35.9: cross-direction-replay engeli).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyDirection {
    /// A→B.
    AToB,
    /// B→A.
    BToA,
}
impl LegacyDirection {
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            LegacyDirection::AToB => 0x01,
            LegacyDirection::BToA => 0x02,
        }
    }
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            LegacyDirection::AToB => LegacyDirection::BToA,
            LegacyDirection::BToA => LegacyDirection::AToB,
        }
    }
}

/// §35.9 SESSION_DATA_AAD üretir (28B, big-endian, sıra-sabit). seq16 + direction-code parametrik.
#[must_use]
pub fn build_legacy_aad(
    protocol_version: u8,
    cipher_suite_id: u16,
    direction_code: u8,
    epoch: u16,
    seq16: u16,
    session_id_internal: &[u8; 16],
) -> [u8; LEGACY_AAD_LEN] {
    let mut aad = [0u8; LEGACY_AAD_LEN];
    aad[0..4].copy_from_slice(&SESSION_DATA_MAGIC);
    aad[4] = protocol_version;
    aad[5..7].copy_from_slice(&cipher_suite_id.to_be_bytes());
    aad[7] = direction_code;
    aad[8..10].copy_from_slice(&epoch.to_be_bytes());
    aad[10..12].copy_from_slice(&seq16.to_be_bytes());
    aad[12..28].copy_from_slice(session_id_internal);
    aad
}

/// Legacy SESSION_DATA seal/open (tek-yön-çifti; mock-key). Session Model V3 §35.6/§35.9.
pub struct LegacySessionProtector {
    hub_handle: u32,
    send_cipher: ChaCha20Poly1305,
    recv_cipher: ChaCha20Poly1305,
    protocol_version: u8,
    cipher_suite_id: u16,
    epoch: u16,
    session_id_internal: [u8; 16],
    send_direction: LegacyDirection,
    last_sealed_pn: Option<u64>,
    recv_window: ReplayWindow,
}

impl LegacySessionProtector {
    /// `send_direction` = bu-endpoint'in-gönderdiği-yön; open PEER-yönünü (opposite) AAD'de kullanır.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hub_handle: u32,
        send_key: &Zeroizing<[u8; KEY_LEN]>,
        recv_key: &Zeroizing<[u8; KEY_LEN]>,
        protocol_version: u8,
        cipher_suite_id: u16,
        epoch: u16,
        session_id_internal: [u8; 16],
        send_direction: LegacyDirection,
    ) -> Self {
        Self {
            hub_handle,
            send_cipher: ChaCha20Poly1305::new(Key::from_slice(send_key.as_ref())),
            recv_cipher: ChaCha20Poly1305::new(Key::from_slice(recv_key.as_ref())),
            protocol_version,
            cipher_suite_id,
            epoch,
            session_id_internal,
            send_direction,
            last_sealed_pn: None,
            recv_window: ReplayWindow::new(LEGACY_PN_PARAMS),
        }
    }

    #[must_use]
    pub fn hub_handle(&self) -> u32 {
        self.hub_handle
    }
    #[must_use]
    pub fn recv_expected_pn(&self) -> u64 {
        self.recv_window.expected_pn()
    }

    /// SESSION_DATA seal (§35.6): `[hub_handle 4][seq16 2][ct][tag 16]`. nonce=LE64(full_pn); seq16=full_pn düşük-16.
    ///
    /// # Errors
    /// Non-monotonic-PN (nonce-reuse-guard) / AEAD.
    pub fn seal(&mut self, plaintext: &[u8], full_pn: u64) -> Result<Vec<u8>, WireError> {
        if let Some(last) = self.last_sealed_pn {
            if full_pn <= last {
                return Err(WireError::SealInput("legacy non-monotonic full_pn"));
            }
        }
        let seq16 = (full_pn & 0xFFFF) as u16;
        let aad = build_legacy_aad(
            self.protocol_version,
            self.cipher_suite_id,
            self.send_direction.code(),
            self.epoch,
            seq16,
            &self.session_id_internal,
        );
        let ct_tag = self
            .send_cipher
            .encrypt(
                &nonce(full_pn),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| WireError::AeadFail)?;
        let mut out = Vec::with_capacity(HDR_LEN + ct_tag.len());
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.extend_from_slice(&seq16.to_be_bytes());
        out.extend_from_slice(&ct_tag);
        self.last_sealed_pn = Some(full_pn);
        Ok(out)
    }

    /// SESSION_DATA open (§35.6 + §4-ORDER): parse → reconstruct(LEGACY) → precheck → AEAD → commit.
    /// AAD peer-yönünü (send_direction.opposite) kullanır.
    ///
    /// # Errors
    /// Kısa/handle-mismatch / PN-reject / AEAD.
    pub fn open(&mut self, datagram: &[u8]) -> Result<Vec<u8>, WireError> {
        if datagram.len() < LEGACY_OVERHEAD {
            return Err(WireError::DatagramTooShort {
                got: datagram.len(),
                min: LEGACY_OVERHEAD,
            });
        }
        let hub_handle = u32::from_be_bytes([datagram[0], datagram[1], datagram[2], datagram[3]]);
        if hub_handle != self.hub_handle {
            return Err(WireError::UnknownHandle(hub_handle));
        }
        let seq16 = u16::from_be_bytes([datagram[4], datagram[5]]);
        let candidate = reconstruct(
            &LEGACY_PN_PARAMS,
            self.recv_window.expected_pn(),
            u64::from(seq16),
        );
        self.recv_window.precheck(candidate)?;
        let recv_dir = self.send_direction.opposite().code();
        let aad = build_legacy_aad(
            self.protocol_version,
            self.cipher_suite_id,
            recv_dir,
            self.epoch,
            seq16,
            &self.session_id_internal,
        );
        let ct_tag = &datagram[HDR_LEN..];
        let plain = self
            .recv_cipher
            .decrypt(
                &nonce(candidate),
                Payload {
                    msg: ct_tag,
                    aad: &aad,
                },
            )
            .map_err(|_| WireError::AeadFail)?; // window-İLERLEMEZ
        self.recv_window.commit(candidate);
        let _ = TAG_LEN;
        Ok(plain)
    }
}

fn nonce(full_pn: u64) -> Nonce {
    *Nonce::from_slice(&nonce96(full_pn))
}
