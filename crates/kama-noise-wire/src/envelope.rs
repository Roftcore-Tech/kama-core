//! Zarf kodlayıcıları — SAF fonksiyonlar (state TUTMAZ) ve oturum başına sabit zarf profili
//! ([`EnvelopeProfile`]).
//!
//! * **Legacy 22 B** — ccr §35.6: `[hub_handle 4][seq16 2][ct][tag 16]`; AAD ccr §35.9 (28 B,
//!   `"SSD1" ‖ protocol_version ‖ cipher_suite_id ‖ direction ‖ epoch ‖ seq16 ‖
//!   session_id_internal`). `hub_handle` AAD'ye GİRMEZ. AAD'deki `epoch` = mühürleyen/açan
//!   anahtarın epoch'u (`u16_be(dh_epoch)`, ccr §35.9 notu).
//! * **Extended 26 B** — noise-wire §2.5/§2.6: `[h_ext 4][wire_format 1][wire_flags 1][pn32 4]
//!   [ct][tag 16]`; AAD noise-wire §2.8. Yön/epoch AAD'de DEĞİL, anahtardadır (ccr §35.9 B2 notu:
//!   asimetri kasıtlı).
//!
//! Çerçeve OPAKTIR (S3 K4): bu modül iç çerçeveyi parse ETMEZ.
//!
//! Sıra sözleşmesi (noise-wire §4-ORDER; identity §15.5 P0-8) ÇAĞIRANINDIR: parse → reconstruct
//! → precheck → AEAD (`open_*`) → commit. `open_*` yalnız AEAD'dir; replay penceresine dokunmaz.
//!
//! Nonce: `nonce96(full_pn) = 0x00×4 ‖ LE64(full_pn)` (noise-wire §4.5; ccr §35.6 [C-1]). Tag
//! doğrulaması YALNIZ `chacha20poly1305`'in `decrypt`'idir (sabit zamanlı karşılaştırma
//! kütüphanede); elle tag karşılaştırması YOKTUR.

use core::fmt;

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use kama_contracts::s3::{HubHandle, ProtectionLimits};
use kama_transport_schema::EXTENDED_OUTER_OVERHEAD;

use crate::aad::build_aad;
use crate::error::WireError;
use crate::legacy::{build_legacy_aad, LegacyDirection, LEGACY_AAD_LEN, LEGACY_OVERHEAD};
use crate::outer::{ExtendedOuter, FLAG_DH_EPOCH, OUTER_HDR_LEN, TAG_LEN};
use crate::pn::{PnParams, EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};
use crate::protector::nonce96;

// ── Boyutlar ─────────────────────────────────────────────────────────────────────────────────

/// En küçük IPv4 başlığı (seçeneksiz).
const IPV4_MIN_HEADER_LEN: usize = 20;
/// UDP başlığı.
const UDP_HEADER_LEN: usize = 8;

/// Bir UDP/IPv4 datagramının taşıyabileceği azami payload: 65 535 − 20 (IPv4) − 8 (UDP) = 65 507.
///
/// `client-core` `IdentitySeam` ve `seam_mock::MockCrypto` ile AYNI türetim (L-A5b K5); oturum ve
/// hub bağlamlarının `max_plaintext_bytes` değeri `MAX_UDP_PAYLOAD − wire_overhead`'tir.
pub const MAX_UDP_PAYLOAD: usize = u16::MAX as usize - IPV4_MIN_HEADER_LEN - UDP_HEADER_LEN;

/// Wire tutamacı (`hub_handle` / `h_ext`) boyu: `u32_be`, datagramın ilk dört baytı.
pub const WIRE_HANDLE_LEN: usize = core::mem::size_of::<u32>();
/// `seq16` boyu (`u16_be`).
const SEQ16_LEN: usize = core::mem::size_of::<u16>();
/// Legacy açık başlığı: `hub_handle` + `seq16`.
pub const LEGACY_HDR_LEN: usize = WIRE_HANDLE_LEN + SEQ16_LEN;

const _: () = assert!(LEGACY_HDR_LEN + TAG_LEN == LEGACY_OVERHEAD);
const _: () = assert!(OUTER_HDR_LEN + TAG_LEN == EXTENDED_OUTER_OVERHEAD);

const fn limits_for(wire_overhead: usize) -> ProtectionLimits {
    ProtectionLimits {
        wire_overhead_bytes: wire_overhead as u32,
        max_plaintext_bytes: (MAX_UDP_PAYLOAD - wire_overhead) as u32,
    }
}

/// İki profilin EN KISITLAYICISI: büyük ek yük, küçük plaintext tavanı.
const fn most_restrictive(a: ProtectionLimits, b: ProtectionLimits) -> ProtectionLimits {
    ProtectionLimits {
        wire_overhead_bytes: if a.wire_overhead_bytes > b.wire_overhead_bytes {
            a.wire_overhead_bytes
        } else {
            b.wire_overhead_bytes
        },
        max_plaintext_bytes: if a.max_plaintext_bytes < b.max_plaintext_bytes {
            a.max_plaintext_bytes
        } else {
            b.max_plaintext_bytes
        },
    }
}

/// Legacy oturum: 22 B ek yük (S3 §1.1 "legacy 18(+4 hub_handle)" — `hub_handle`'ı Crypto yazar).
pub(crate) const LEGACY_LIMITS: ProtectionLimits = limits_for(LEGACY_OVERHEAD);
/// Extended oturum: 26 B ek yük.
pub(crate) const EXTENDED_LIMITS: ProtectionLimits = limits_for(EXTENDED_OUTER_OVERHEAD);
/// `HubChannel` PoC no-op: +16 B sıfır "tag" (`IdentitySeam` paritesi; gerçek hop anahtarı M4).
pub(crate) const HUB_NOOP_LIMITS: ProtectionLimits = limits_for(TAG_LEN);
/// `Session` sınıfının TÜM profilleri için güvenli minimum (S3 `conservative_limits`).
pub(crate) const SESSION_CONSERVATIVE_LIMITS: ProtectionLimits =
    most_restrictive(LEGACY_LIMITS, EXTENDED_LIMITS);

// ── Oturum başına sabit zarf profili ─────────────────────────────────────────────────────────

/// Bir oturumun TEK zarf profili: kurulumda seçilir, oturum boyunca DEĞİŞMEZ. Tek gönderim
/// sayacı ve tek replay penceresi (yön başına) bu profilin PN parametreleriyle çalışır.
#[derive(Clone, PartialEq, Eq)]
pub enum EnvelopeProfile {
    /// 22 B SESSION_DATA (ccr §35.6 / §35.9).
    Legacy {
        /// Wire'daki cleartext `hub_handle` (ccr §35.7); AAD'ye GİRMEZ.
        hub_handle: HubHandle,
        protocol_version: u8,
        cipher_suite_id: u16,
        /// INTERNAL (wire'da YOK) — yalnız AAD'ye girer. `Debug`'a yazılmaz.
        session_id_internal: [u8; 16],
        /// Bu ucun GÖNDERDİĞİ yön; alımda AAD eşin yönünü (`opposite`) taşır.
        send_direction: LegacyDirection,
    },
    /// 26 B extended (noise-wire §2.5 / §2.6 / §2.8).
    Extended {
        /// Wire'daki cleartext `h_ext` (noise-wire §2.55 HANDLE_ASSIGN).
        h_ext: HubHandle,
    },
}

impl EnvelopeProfile {
    /// Mühürlemenin eklediği wire baytı (başlık + tag).
    #[must_use]
    pub fn overhead(&self) -> usize {
        match self {
            Self::Legacy { .. } => LEGACY_OVERHEAD,
            Self::Extended { .. } => EXTENDED_OUTER_OVERHEAD,
        }
    }

    /// Bu profilin S3 boyut sınırları.
    #[must_use]
    pub fn limits(&self) -> ProtectionLimits {
        match self {
            Self::Legacy { .. } => LEGACY_LIMITS,
            Self::Extended { .. } => EXTENDED_LIMITS,
        }
    }

    /// Replay/reconstruct sabit seti (DR-3: genişlik başına).
    #[must_use]
    pub fn pn_params(&self) -> PnParams {
        match self {
            Self::Legacy { .. } => LEGACY_PN_PARAMS,
            Self::Extended { .. } => EXTENDED_PN_PARAMS,
        }
    }

    /// Datagramın ilk dört baytında taşınan wire tutamacı (decrypt ÖNCESİ demux anahtarı).
    #[must_use]
    pub fn wire_handle(&self) -> u32 {
        match self {
            Self::Legacy { hub_handle, .. } => hub_handle.raw(),
            Self::Extended { h_ext } => h_ext.raw(),
        }
    }
}

impl fmt::Debug for EnvelopeProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // `session_id_internal` wire'da olmayan bir oturum kimliğidir — log'a düşmez.
            Self::Legacy {
                hub_handle,
                protocol_version,
                cipher_suite_id,
                send_direction,
                ..
            } => f
                .debug_struct("Legacy")
                .field("hub_handle", hub_handle)
                .field("protocol_version", protocol_version)
                .field("cipher_suite_id", cipher_suite_id)
                .field("send_direction", send_direction)
                .finish_non_exhaustive(),
            Self::Extended { h_ext } => f.debug_struct("Extended").field("h_ext", h_ext).finish(),
        }
    }
}

// ── Ortak yardımcılar ────────────────────────────────────────────────────────────────────────

/// Datagramın ilk dört baytındaki wire tutamacı (noise-wire §2.5 PN-WIDTH-DEMUX). Dört bayttan
/// kısa → `None`.
#[must_use]
pub fn wire_handle(datagram: &[u8]) -> Option<u32> {
    let bytes: [u8; WIRE_HANDLE_LEN] = datagram.get(..WIRE_HANDLE_LEN)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn nonce(full_pn: u64) -> Nonce {
    *Nonce::from_slice(&nonce96(full_pn))
}

fn too_short(got: usize, min: usize) -> WireError {
    WireError::DatagramTooShort { got, min }
}

// ── Legacy 22 B ──────────────────────────────────────────────────────────────────────────────

/// Legacy açık başlığı (cleartext; hub açmadan okur).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyHeader {
    pub hub_handle: u32,
    pub seq16: u16,
}

/// ccr §35.9 AAD'nin pakete göre değişmeyen alanları (`seq16` hariç).
#[derive(Clone, Copy)]
pub struct LegacyAadFields<'a> {
    pub protocol_version: u8,
    pub cipher_suite_id: u16,
    /// Bu paketin YÖNÜ (0x01 A→B / 0x02 B→A): gönderende kendi yönü, alıcıda eşin yönü.
    pub direction_code: u8,
    /// Mühürleyen/açan anahtarın epoch'u (`u16_be(dh_epoch)`).
    pub epoch: u16,
    pub session_id_internal: &'a [u8; 16],
}

impl LegacyAadFields<'_> {
    /// 28 B SSD1 AAD (ccr §35.9; kurucu `legacy::build_legacy_aad` — tek kaynak).
    #[must_use]
    pub fn aad(&self, seq16: u16) -> [u8; LEGACY_AAD_LEN] {
        build_legacy_aad(
            self.protocol_version,
            self.cipher_suite_id,
            self.direction_code,
            self.epoch,
            seq16,
            self.session_id_internal,
        )
    }
}

/// Tam sayacın wire'daki düşük 16 biti (ccr §35.8: `seq16` TEK BAŞINA nonce DEĞİLDİR).
#[must_use]
pub fn seq16_of(full_pn: u64) -> u16 {
    (full_pn & u64::from(u16::MAX)) as u16
}

/// Legacy mühür: `[hub_handle][seq16][ChaCha20-Poly1305(nonce96(full_pn), frame, AAD)]`.
///
/// # Errors
/// [`WireError::AeadFail`] — yalnız ChaCha20'nin plaintext sınırı aşılırsa (çağıran çerçeveyi
/// [`MAX_UDP_PAYLOAD`] ile önceden sınırlar).
pub fn seal_legacy(
    cipher: &ChaCha20Poly1305,
    hub_handle: u32,
    aad: &LegacyAadFields<'_>,
    full_pn: u64,
    frame: &[u8],
) -> Result<Vec<u8>, WireError> {
    let seq16 = seq16_of(full_pn);
    let aad_bytes = aad.aad(seq16);
    let ct_tag = cipher
        .encrypt(
            &nonce(full_pn),
            Payload {
                msg: frame,
                aad: &aad_bytes,
            },
        )
        .map_err(|_| WireError::AeadFail)?;
    let mut out = Vec::with_capacity(LEGACY_HDR_LEN + ct_tag.len());
    out.extend_from_slice(&hub_handle.to_be_bytes());
    out.extend_from_slice(&seq16.to_be_bytes());
    out.extend_from_slice(&ct_tag);
    Ok(out)
}

/// Legacy açık başlığını okur (sabit maliyet; decrypt ÖNCESİ).
///
/// # Errors
/// [`WireError::DatagramTooShort`] — 22 B'tan kısa (ccr §35.6 asgari UDP payload).
pub fn parse_legacy(datagram: &[u8]) -> Result<LegacyHeader, WireError> {
    if datagram.len() < LEGACY_OVERHEAD {
        return Err(too_short(datagram.len(), LEGACY_OVERHEAD));
    }
    let hub_handle =
        wire_handle(datagram).ok_or_else(|| too_short(datagram.len(), LEGACY_OVERHEAD))?;
    let seq: [u8; SEQ16_LEN] = datagram
        .get(WIRE_HANDLE_LEN..LEGACY_HDR_LEN)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| too_short(datagram.len(), LEGACY_OVERHEAD))?;
    Ok(LegacyHeader {
        hub_handle,
        seq16: u16::from_be_bytes(seq),
    })
}

/// Legacy AEAD açma — YALNIZ doğrulama + şifre çözme; replay penceresine DOKUNMAZ. `candidate`
/// çağıranın `reconstruct` + `precheck`'ten geçirdiği tam sayaçtır.
///
/// # Errors
/// [`WireError::DatagramTooShort`] / [`WireError::AeadFail`] (tag, anahtar, AAD ya da nonce
/// uyuşmazlığı — dışarıdan ayırt edilemez).
pub fn open_legacy(
    cipher: &ChaCha20Poly1305,
    aad: &LegacyAadFields<'_>,
    candidate: u64,
    datagram: &[u8],
) -> Result<Vec<u8>, WireError> {
    let header = parse_legacy(datagram)?;
    let ct_tag = datagram
        .get(LEGACY_HDR_LEN..)
        .ok_or_else(|| too_short(datagram.len(), LEGACY_OVERHEAD))?;
    let aad_bytes = aad.aad(header.seq16);
    cipher
        .decrypt(
            &nonce(candidate),
            Payload {
                msg: ct_tag,
                aad: &aad_bytes,
            },
        )
        .map_err(|_| WireError::AeadFail)
}

// ── Extended 26 B ────────────────────────────────────────────────────────────────────────────

/// Tam sayacın wire'daki düşük 32 biti (`pn_trunc`, noise-wire §2.5).
#[must_use]
pub fn pn_trunc_of(full_pn: u64) -> u32 {
    (full_pn & u64::from(u32::MAX)) as u32
}

/// Extended mühür: `[outer 10 B][ChaCha20-Poly1305(nonce96(full_pn), frame, AAD §2.8)]`.
/// `KamaNoiseWireProtector::seal` ile aynı inşa (iç çerçeveyi o tip kendisi kodlar; burada OPAK).
///
/// # Errors
/// [`WireError::AeadFail`] — yalnız ChaCha20'nin plaintext sınırı aşılırsa.
pub fn seal_extended(
    cipher: &ChaCha20Poly1305,
    h_ext: u32,
    dh_epoch: u8,
    full_pn: u64,
    frame: &[u8],
) -> Result<Vec<u8>, WireError> {
    let outer = ExtendedOuter {
        h_ext,
        dh_epoch: dh_epoch & FLAG_DH_EPOCH,
        pn_trunc: pn_trunc_of(full_pn),
    };
    let aad = build_aad(&outer);
    let ct_tag = cipher
        .encrypt(
            &nonce(full_pn),
            Payload {
                msg: frame,
                aad: &aad,
            },
        )
        .map_err(|_| WireError::AeadFail)?;
    let mut out = Vec::with_capacity(OUTER_HDR_LEN + ct_tag.len());
    out.extend_from_slice(&outer.serialize_hdr());
    out.extend_from_slice(&ct_tag);
    Ok(out)
}

/// Extended dış başlığı okur (noise-wire §4-ORDER adım 1: sabit maliyet).
///
/// # Errors
/// Kısa datagram / bilinmeyen `wire_format` / ayrılmış `wire_flags` bitleri → fail-closed.
pub fn parse_extended(datagram: &[u8]) -> Result<ExtendedOuter, WireError> {
    ExtendedOuter::parse(datagram).map(|(outer, _, _)| outer)
}

/// Extended AEAD açma — YALNIZ doğrulama + şifre çözme; replay penceresine DOKUNMAZ.
///
/// AAD alınan başlıktan kurulur: `parse` kanonik olmayan başlığı (format ≠ EXTENDED_V1, ayrılmış
/// bayrak) reddettiği için `build_aad(parse(d))` = `AAD_DS ‖ d[..10]` (alınan baytların kendisi).
///
/// # Errors
/// Dış başlık hataları / [`WireError::AeadFail`].
pub fn open_extended(
    cipher: &ChaCha20Poly1305,
    candidate: u64,
    datagram: &[u8],
) -> Result<Vec<u8>, WireError> {
    let outer = parse_extended(datagram)?;
    let aad = build_aad(&outer);
    let ct_tag = datagram
        .get(OUTER_HDR_LEN..)
        .ok_or_else(|| too_short(datagram.len(), EXTENDED_OUTER_OVERHEAD))?;
    cipher
        .decrypt(
            &nonce(candidate),
            Payload {
                msg: ct_tag,
                aad: &aad,
            },
        )
        .map_err(|_| WireError::AeadFail)
}
