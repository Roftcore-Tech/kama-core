//! KamaNoiseWireProtector — EXTENDED seal/open (§0 API + §4-ORDER + §0.5 sizer/meta).
//!
//! AEAD: ChaCha20-Poly1305 (RFC 8439; pki-§13 registry: key 32B / nonce 12B / tag 16B).
//! NONCE96 [DR-4-CLOSED — KOŞUL-A/LE64; noise-wire §4.5 KİLİTLİ]: nonce = [4B-zero || LE64(full_pn)]
//! (Noise-Framework-ChaChaPoly-kanonik-mapping; dh_epoch nonce'a-GİRMEZ — epoch AYRI-cipher-state/key'dedir).
//! wire'daki pn32/seq16 BIG-ENDIAN (§2.6); nonce96 CIPHER-PRIMITIVE-girdisi LE64 — iki-ayrı-serialization.
//!
//! Anahtar-yaşamı: [`Zeroizing`] kopyalar; cipher-state drop'ta anahtar-materyali bırakır (zeroize).

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use core::fmt;
use std::collections::{BTreeMap, VecDeque};
use zeroize::Zeroizing;

use crate::aad::build_aad;
use crate::envelope::{
    open_extended, open_legacy, parse_extended, parse_legacy, seal_extended, seal_legacy,
    wire_handle, EnvelopeProfile, LegacyAadFields, HUB_NOOP_LIMITS, MAX_UDP_PAYLOAD,
    SESSION_CONSERVATIVE_LIMITS,
};
use crate::error::WireError;
use crate::inner::{
    encode_control_inner, encode_fec_inner, parse_inner, CONTROL_INNER_PREFIX_LEN,
    FEC_INNER_HDR_LEN,
};
use crate::opened::OpenedTransportShard;
use crate::outer::{ExtendedOuter, FLAG_DH_EPOCH, OUTER_HDR_LEN, TAG_LEN};
use crate::pn::{reconstruct, ReplayWindow, EXTENDED_PN_PARAMS};
use crate::replay::{DirectionalReplay, PrecheckedPn};
use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{
    self, Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits,
    SealError, SealedUnit, SessionEvent,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{UnsealedTransportPacket, EXTENDED_OUTER_OVERHEAD};

/// Full-PN gönderim-tavanı = ccr §35.8 `REKEY_THRESHOLD` (2^31; transport-layer-tdd §6 "REKEY_THRESHOLD <= 2^31").
/// Tavana ulaşan oturum yeni mühür ÜRETMEZ ve `RenewRequired` yayınlar; sayaç ASLA sarmaz.
/// [2026-10-08 — sahip kararı S6] önceki değer `1 << 62` ([CAL]) spec'le çelişiyordu; spec kazanır.
pub const SEND_PN_CAP: u64 = 1 << 31;

/// AEAD-anahtar boyu (registry: 32B).
pub const KEY_LEN: usize = 32;

/// NONCE96 (§4.5 NORMATİF, DR-4-CLOSED KOŞUL-A/LE64): `nonce = [0x00000000 ‖ LE64(full_pn)]`.
/// Deterministik-public-mapping (sır-DEĞİL; sır yalnız key). Noise-Framework-ChaChaPoly-kanonik.
/// wire'daki pn32/seq16 big-endian (§2.6) — nonce96 cipher-primitive-girdisi LE64 (iki-ayrı-serialization).
#[must_use]
pub fn nonce96(full_pn: u64) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[4..12].copy_from_slice(&full_pn.to_le_bytes());
    n
}

/// Paket-sınıfı (SealedPacketMeta; §0.5 P0-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketClass {
    Source,
    Repair,
    Control,
}

/// wire-PN → logical eşleme referansı (§0.5: ACK/BtlBw/ARQ-semantiği bunsuz-imkansız).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalPacketRef {
    FecShard { block_id: u32, esi: u16 },
    Control { inner_kind: u8, control_id: u32 },
}

/// §0.5 SealedPacketMeta — engine/pacer send-record'u bundan kurar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedPacketMeta {
    pub full_wire_pn: u64,
    pub dh_epoch: u8,
    pub sealed_len: usize,
    pub logical_ref: LogicalPacketRef,
    pub packet_class: PacketClass,
    pub send_generation: u32,
}

/// Seal çıktısı (§0.5): wire-byte + meta.
#[derive(Debug, Clone)]
pub struct SealedPacket {
    pub bytes: Vec<u8>,
    pub meta: SealedPacketMeta,
}

/// Boyut-hatası (§0.5 sizer-sözleşmesi).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WireSizeError {
    /// Symbol boş (E ≥ 11 invariant'ının wire-yüzü).
    #[error("empty symbol")]
    EmptySymbol,
    /// Control-count DR-1 tavanını aşıyor → önce `split_for_send` gerekir (boyutlanamaz).
    #[error("control count exceeds cap (split_for_send required): kind {kind:#04x}")]
    ControlCountExceedsCap { kind: u8 },
}

/// §0.5 P0-3: pacer-admission seal-ÖNCESİ kesin-boyut. NORMATİF: `seal(p).len() == sealed_len(p)`.
pub trait KamaNoiseWireSizer {
    /// # Errors
    /// [`WireSizeError`] — boyutlanamayan paket.
    fn sealed_len(&self, p: &UnsealedTransportPacket) -> Result<usize, WireSizeError>;
}

/// Seal-parametreleri: PN + epoch + retransmission-generation (engine-verir; §0.5).
#[derive(Debug, Clone, Copy)]
pub struct SealArgs {
    pub wire_pn: u64,
    pub dh_epoch: u8,
    pub send_generation: u32,
}

/// Yön-başına EXTENDED protector: seal (gönderim) + open (alım) + epoch-state (§4/§5).
/// Handshake/key-türetimi AYRI-katman (Sprint-4); bu-tip kurulu-anahtarlarla çalışır.
pub struct KamaNoiseWireProtector {
    h_ext: u32,
    send_cipher: [Option<ChaCha20Poly1305>; 2],
    recv_cipher: [Option<ChaCha20Poly1305>; 2],
    /// Nonce-reuse yapısal-guard: son-seal-PN (yeni-PN kesin-büyük olmalı). TEK sayaç: PN uzayı
    /// (session, direction) başınadır ve epoch'lar boyunca SÜREKLİDİR — dh_epoch yalnız anahtar seçer
    /// (noise-wire §2.5 + §4 EPOCH-PN-INIT, 2026-10-06).
    last_sealed_pn: Option<u64>,
    /// TEK replay penceresi (identity §15.5): sayaç küresel sıralı olduğundan epoch'a bölünmez.
    recv_window: ReplayWindow,
    /// Epoch-kabul bayrakları (§4-adım-1 + B3): [0]=epoch-0 (grace-bitince-false), [1]=epoch-1
    /// (anahtar-kurulunca-true). B3-adım-3: Bob epoch-0'dayken-de geçerli-epoch-1-paketi-KABUL-EDER.
    recv_epoch_accepts: [bool; 2],
    /// A-4 (DR-CS-1b/A2; spec-§0.9-amendmanı): send-state RETIRED yapısal-bayrağı — TEK-YÖN
    /// (true'ya-yazan-tek-yol drop_send_state; temizleyen API YOK → sessiz-reinit YAPISAL-imkansız;
    /// seal + install-sonrası-seal bu-bayrakla kapılı → renewal-üzerinden-reactivation İMKANSIZ).
    #[cfg(feature = "experimental-cold-start")]
    send_retired: bool,
}

impl KamaNoiseWireProtector {
    /// Epoch-0 anahtarlarıyla kurar (Split()-sonrası k_send/k_recv; direction-local).
    #[must_use]
    pub fn new(
        h_ext: u32,
        send_key_e0: &Zeroizing<[u8; KEY_LEN]>,
        recv_key_e0: &Zeroizing<[u8; KEY_LEN]>,
    ) -> Self {
        Self {
            h_ext,
            send_cipher: [
                Some(ChaCha20Poly1305::new(Key::from_slice(send_key_e0.as_ref()))),
                None,
            ],
            recv_cipher: [
                Some(ChaCha20Poly1305::new(Key::from_slice(recv_key_e0.as_ref()))),
                None,
            ],
            last_sealed_pn: None,
            recv_window: ReplayWindow::new(EXTENDED_PN_PARAMS),
            recv_epoch_accepts: [true, false],
            #[cfg(feature = "experimental-cold-start")]
            send_retired: false,
        }
    }

    /// A-4 `drop_send_state()` — send-yarısı-emekliliği (7-postcondition; spec-§0.9-A-4 aynen):
    /// yalnız lifecycle=Closing altında çağrılır (yapısal: `pub(crate)` — tek-üretim-yolu
    /// `Session::begin_close`) · TÜM send-secret/cipher zeroize (cipher drop = anahtar-yaşamı-notu) ·
    /// send-PN/nonce-allocation irreversibly-retired · recv-cipher + recv-ReplayWindow'a SIFIR-dokunuş ·
    /// sonraki-seal → typed `SendStateRetired` · sessiz-reinit YASAK (bayrak tek-yön) ·
    /// tekrar-çağrı İDEMPOTENT.
    #[cfg(feature = "experimental-cold-start")]
    pub(crate) fn drop_send_state(&mut self) {
        self.send_cipher = [None, None];
        self.last_sealed_pn = None;
        self.send_retired = true;
    }

    /// SESSION_INIT_RESPONSE-sonrası epoch-1 anahtar-kurulumu (dh_epoch 0→1; §5).
    /// Yalnız ANAHTAR kurulur; PN sayacı ve replay penceresi SIFIRLANMAZ — epoch-1'in ilk PN'i
    /// epoch-0'ın son PN'inden +1 ile devam eder (§4 EPOCH-PN-INIT, 2026-10-06).
    pub fn install_epoch1_keys(
        &mut self,
        send_key_e1: &Zeroizing<[u8; KEY_LEN]>,
        recv_key_e1: &Zeroizing<[u8; KEY_LEN]>,
    ) {
        self.send_cipher[1] = Some(ChaCha20Poly1305::new(Key::from_slice(send_key_e1.as_ref())));
        self.recv_cipher[1] = Some(ChaCha20Poly1305::new(Key::from_slice(recv_key_e1.as_ref())));
        self.recv_epoch_accepts[1] = true;
    }

    /// Grace-bitişi: epoch-0 recv-state discard (§4 B3-adım-5; Sprint-4 machine çağırır).
    pub fn discard_epoch0_recv(&mut self) {
        self.recv_epoch_accepts[0] = false;
        self.recv_cipher[0] = None;
    }

    #[must_use]
    pub fn h_ext(&self) -> u32 {
        self.h_ext
    }

    /// Alım-penceresi görünümü (teşhis/test; state-mutasyonu-YOK).
    #[must_use]
    pub fn recv_expected_pn(&self) -> u64 {
        self.recv_window.expected_pn()
    }

    fn nonce_for(full_pn: u64) -> Nonce {
        *Nonce::from_slice(&nonce96(full_pn))
    }

    fn encode_inner_plain(
        p: &UnsealedTransportPacket,
    ) -> Result<(Vec<u8>, LogicalPacketRef, PacketClass), WireError> {
        match p {
            UnsealedTransportPacket::FecShard { header, symbol } => {
                if symbol.is_empty() {
                    return Err(WireError::SealInput("empty symbol"));
                }
                let class = match header.kind {
                    ShardKind::Source => PacketClass::Source,
                    ShardKind::Repair => PacketClass::Repair,
                };
                Ok((
                    encode_fec_inner(header, symbol),
                    LogicalPacketRef::FecShard {
                        block_id: header.block_id,
                        esi: header.esi,
                    },
                    class,
                ))
            }
            UnsealedTransportPacket::Control(c) => {
                // §2.57 typed-body-codec (Sprint-2). control_id inner-header'da; body ayrı.
                crate::control::control_body_len(c)?; // tavan-aşımı → çağıran split_for_send yapmalıydı
                let body = crate::control::encode_control_body(c);
                Ok((
                    encode_control_inner(c.codepoint(), c.control_id(), &body),
                    LogicalPacketRef::Control {
                        inner_kind: c.codepoint(),
                        control_id: c.control_id(),
                    },
                    PacketClass::Control,
                ))
            }
        }
    }

    /// EXTENDED seal (§0 API + §4-nonce + §0.5 meta). `seal(p).len() == sealed_len(p)` debug-assert'li.
    ///
    /// # Errors
    /// Epoch-anahtarı-yok / non-monotonic-PN (nonce-reuse-guard) / PN-cap / seal-input.
    pub fn seal(
        &mut self,
        p: &UnsealedTransportPacket,
        args: SealArgs,
    ) -> Result<SealedPacket, WireError> {
        // A-4: retired-send-state → typed-reject (yapısal tek-yön bayrak; reactivation-imkansız).
        #[cfg(feature = "experimental-cold-start")]
        if self.send_retired {
            return Err(WireError::SendStateRetired(self.h_ext));
        }
        let ei = usize::from(args.dh_epoch & 1);
        let cipher = self.send_cipher[ei]
            .as_ref()
            .ok_or(WireError::EpochKeysMissing)?;
        if args.wire_pn >= SEND_PN_CAP {
            return Err(WireError::PnCapReached);
        }
        if let Some(last) = self.last_sealed_pn {
            if args.wire_pn <= last {
                return Err(WireError::SealInput(
                    "non-monotonic wire_pn (nonce-reuse guard)",
                ));
            }
        }
        let (inner_plain, logical_ref, packet_class) = Self::encode_inner_plain(p)?;
        let outer = ExtendedOuter {
            h_ext: self.h_ext,
            dh_epoch: args.dh_epoch & 1,
            pn_trunc: (args.wire_pn & 0xFFFF_FFFF) as u32,
        };
        let aad = build_aad(&outer);
        let ct_and_tag = cipher
            .encrypt(
                &Self::nonce_for(args.wire_pn),
                Payload {
                    msg: &inner_plain,
                    aad: &aad,
                },
            )
            .map_err(|_| WireError::AeadFail)?;
        let mut bytes = Vec::with_capacity(OUTER_HDR_LEN + ct_and_tag.len());
        bytes.extend_from_slice(&outer.serialize_hdr());
        bytes.extend_from_slice(&ct_and_tag);

        let sealed_len = bytes.len();
        debug_assert_eq!(
            sealed_len,
            EXTENDED_OUTER_OVERHEAD + inner_plain.len(),
            "seal-len != 26 + inner-len (P0-3 ihlali)"
        );
        self.last_sealed_pn = Some(args.wire_pn);
        Ok(SealedPacket {
            bytes,
            meta: SealedPacketMeta {
                full_wire_pn: args.wire_pn,
                dh_epoch: args.dh_epoch & 1,
                sealed_len,
                logical_ref,
                packet_class,
                send_generation: args.send_generation,
            },
        })
    }

    /// EXTENDED open — §4-ORDER birebir: (1)outer-parse (2)handle (3)reconstruct (4)precheck-iz-
    /// bırakmadan (5)AEAD (6)COMMIT (7)inner-parse (8)Opened. **AEAD-fail window-İLERLETMEZ.**
    ///
    /// # Errors
    /// Outer/format/flags/handle/epoch/PN/AEAD/inner hataları — hepsi fail-closed, state-iz-yok.
    pub fn open(&mut self, datagram: &[u8]) -> Result<OpenedTransportShard, WireError> {
        // (1) fixed-cost outer parse
        let (outer, _ct, _tag) = ExtendedOuter::parse(datagram)?;
        // (2) handle → bu-protector'ın-layout'u (demux üstte; burada eşitlik-guard'ı)
        if outer.h_ext != self.h_ext {
            return Err(WireError::InnerParse("h_ext mismatch"));
        }
        let ei = usize::from(outer.dh_epoch & 1);
        if !self.recv_epoch_accepts[ei] {
            return Err(WireError::EpochNotAccepted(outer.dh_epoch));
        }
        let cipher = self.recv_cipher[ei]
            .as_ref()
            .ok_or(WireError::EpochKeysMissing)?;
        // (3) low_bits → candidate (tek pencere; dh_epoch yalnız anahtarı seçer)
        let candidate = reconstruct(
            &EXTENDED_PN_PARAMS,
            self.recv_window.expected_pn(),
            u64::from(outer.pn_trunc),
        );
        // (4) too-old/dup/future precheck — window-İLERLETMEDEN
        self.recv_window.precheck(candidate)?;
        // (5) dh_epoch-cipher-state AEAD-attempt (AAD = alınan-outer-byte'ların-kendisi)
        let mut aad = [0u8; crate::aad::AAD_LEN];
        aad[0..16].copy_from_slice(crate::aad::AAD_DS);
        aad[16..26].copy_from_slice(&datagram[0..OUTER_HDR_LEN]);
        let ct_with_tag = &datagram[OUTER_HDR_LEN..];
        let plain = cipher
            .decrypt(
                &Self::nonce_for(candidate),
                Payload {
                    msg: ct_with_tag,
                    aad: &aad,
                },
            )
            .map_err(|_| WireError::AeadFail)?; // window-İLERLEMEZ (commit-çağrılmadı)
                                                // (6) SUCCESS → COMMIT (tek pencere)
        self.recv_window.commit(candidate);
        // (7) inner-parse + bounds
        let inner = parse_inner(&plain)?;
        // (8) OpenedPacket handoff
        Ok(OpenedTransportShard::new_authenticated(
            inner,
            candidate,
            outer.dh_epoch,
            outer.h_ext,
        ))
    }
}

impl KamaNoiseWireSizer for KamaNoiseWireProtector {
    fn sealed_len(&self, p: &UnsealedTransportPacket) -> Result<usize, WireSizeError> {
        match p {
            UnsealedTransportPacket::FecShard { symbol, .. } => {
                if symbol.is_empty() {
                    return Err(WireSizeError::EmptySymbol);
                }
                // 26 + (11 + E)  (§2.6; N7: 101B @ E=64)
                Ok(EXTENDED_OUTER_OVERHEAD + FEC_INNER_HDR_LEN + symbol.len())
            }
            UnsealedTransportPacket::Control(c) => {
                // 26 (outer) + 6 (inner_version+inner_kind+control_id) + body (§2.57).
                let body = crate::control::control_body_len(c).map_err(|e| match e {
                    WireError::ControlCountExceedsCap { kind, .. } => {
                        WireSizeError::ControlCountExceedsCap { kind }
                    }
                    _ => WireSizeError::ControlCountExceedsCap {
                        kind: c.codepoint(),
                    },
                })?;
                let _ = TAG_LEN;
                Ok(EXTENDED_OUTER_OVERHEAD + CONTROL_INNER_PREFIX_LEN + body)
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════════════════════
// S3 GERÇEK `PacketProtection` — `NoiseWireProtection` (L-A5b: A5.2 seal/open + sayaç + nonce/AAD
// · A5.3 replay). Yukarıdaki `KamaNoiseWireProtector` DEĞİŞMEDİ (mevcut çağıranları var).
// ═════════════════════════════════════════════════════════════════════════════════════════════

/// `dh_epoch` değerleri (noise-wire §5: Alpha'da yalnız 0 ve 1).
const DH_EPOCH_0: u8 = 0;
const DH_EPOCH_1: u8 = 1;
/// No-op hub kanalı PN'i wire'a yazmaz; alımda bilinemez (`IdentitySeam` paritesi).
const HUB_NOOP_RECV_PN: u64 = 0;

/// `dh_epoch` → anahtar dizisi indeksi (0 ya da 1; dizi sınırı yapısal olarak aşılamaz).
fn idx(dh_epoch: u8) -> usize {
    usize::from(dh_epoch & FLAG_DH_EPOCH)
}

/// Gönderim sayacı tahsisi — SAF. `next_send_pn < SEND_PN_CAP` ise `(pn, pn + 1)`, aksi `None`:
/// tavanda yeni mühür YOK, sayaç TÜKETİLMEZ ve ASLA sarmaz (S3 `CapReached`; noise-wire §4 WRAP).
/// `pn + 1` taşamaz çünkü `SEND_PN_CAP < u64::MAX`.
#[must_use]
pub fn allocate_send_pn(next_send_pn: u64) -> Option<(u64, u64)> {
    if next_send_pn >= SEND_PN_CAP {
        return None;
    }
    Some((next_send_pn, next_send_pn.checked_add(1)?))
}

/// Bir oturumun kurulum girdisi: sabit zarf profili + epoch-0 trafik anahtarları (ccr §35.2.1 D2;
/// rol eşlemesi çağıranındır: initiator `send = k_A2B_e0`, `recv = k_B2A_e0`, responder tersi).
/// Anahtarlar [`Zeroizing`] içinde gelir; cipher kurulduktan sonra bu kopyalar silinir.
pub struct SessionKeyMaterial {
    pub profile: EnvelopeProfile,
    pub send_e0: Zeroizing<[u8; KEY_LEN]>,
    pub recv_e0: Zeroizing<[u8; KEY_LEN]>,
}

impl fmt::Debug for SessionKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Anahtar baytı Debug'a GİRMEZ.
        f.debug_struct("SessionKeyMaterial")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

/// Oturumun gönderim yarısının durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    /// `seal` + `open`.
    Active,
    /// Gönderim yarısı emekli (send anahtarları silindi): `seal` → `UnknownSession`, `open` sürer
    /// (supersession drain'i — kural L-A5c'nin).
    ReceiveOnly,
}

/// Bir oturumun kripto durumu.
struct SessionCrypto {
    profile: EnvelopeProfile,
    send: [Option<ChaCha20Poly1305>; 2],
    recv: [Option<ChaCha20Poly1305>; 2],
    /// Gönderimde kullanılan epoch (yalnız ileri: 0 → 1).
    send_epoch: u8,
    /// Alım kabul kapısı (noise-wire §4 `accept` adım 1; grace). `[0]` grace kapanınca `false`
    /// olur ve onu `true`'ya çeviren yol YOKTUR.
    recv_accepts: [bool; 2],
    /// TEK gönderim sayacı: epoch'lar boyunca SÜREKLİ, Crypto atar (S3 K2; noise-wire §2.5 + §4
    /// EPOCH-PN-INIT). Yalnız başarılı mühürde ilerler.
    next_send_pn: u64,
    /// TEK alım penceresi (yön başına; epoch'a bölünmez — identity §15.5).
    replay: DirectionalReplay,
    /// `RenewRequired` bu oturum için kuyruğa kondu mu (tek sefer).
    renew_signalled: bool,
    lifecycle: Lifecycle,
}

/// Saf açma aşamasının çıktısı (commit ÖNCESİ).
struct OpenOk {
    pn: PrecheckedPn,
    epoch: u8,
    frame: Vec<u8>,
}

fn cipher_from(key: &Zeroizing<[u8; KEY_LEN]>) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(Key::from_slice(key.as_ref()))
}

impl SessionCrypto {
    fn new(material: SessionKeyMaterial) -> Self {
        let replay = DirectionalReplay::new(material.profile.pn_params());
        Self {
            send: [Some(cipher_from(&material.send_e0)), None],
            recv: [Some(cipher_from(&material.recv_e0)), None],
            send_epoch: DH_EPOCH_0,
            recv_accepts: [true, false],
            next_send_pn: 0,
            replay,
            renew_signalled: false,
            lifecycle: Lifecycle::Active,
            profile: material.profile,
        }
    }

    /// Zarfı mühürler (state'e DOKUNMAZ; sayaç çağıranda ilerler).
    fn seal_envelope(&self, pn: u64, frame: &[u8]) -> Result<Vec<u8>, SealError> {
        let cipher = self.send[idx(self.send_epoch)]
            .as_ref()
            .ok_or(SealError::UnknownSession)?;
        let sealed = match &self.profile {
            EnvelopeProfile::Legacy {
                hub_handle,
                protocol_version,
                cipher_suite_id,
                session_id_internal,
                send_direction,
            } => {
                let aad = LegacyAadFields {
                    protocol_version: *protocol_version,
                    cipher_suite_id: *cipher_suite_id,
                    direction_code: send_direction.code(),
                    epoch: u16::from(self.send_epoch),
                    session_id_internal,
                };
                seal_legacy(cipher, hub_handle.raw(), &aad, pn, frame)
            }
            EnvelopeProfile::Extended { h_ext } => {
                seal_extended(cipher, h_ext.raw(), self.send_epoch, pn, frame)
            }
        };
        // ChaCha20 şifrelemesi yalnız plaintext sınırını (≈ 256 GiB) aşan girdide düşer; çerçeve
        // `max_plaintext_bytes` ile önceden sınırlandığından erişilemez — panik yerine yapısal ret.
        sealed.map_err(|_| SealError::FrameTooLarge)
    }

    /// Legacy alımda TEK anahtar (L-A5b): 22 B zarfta epoch alanı YOKTUR → kurulu ve kabul edilen
    /// EN YENİ alım epoch'u. Grace içindeki ≤2 anahtar denemesi (ccr §35.8) L-A5c'nindir.
    fn legacy_recv_epoch(&self) -> Option<u8> {
        [DH_EPOCH_1, DH_EPOCH_0]
            .into_iter()
            .find(|&e| self.recv_accepts[idx(e)] && self.recv[idx(e)].is_some())
    }

    /// SAF açma aşaması (noise-wire §4-ORDER 1–5; identity §15.5 P0-8): parse → reconstruct →
    /// precheck → anahtar → AEAD. `&self`: hiçbir başarısızlık state'e dokunamaz (H-NO-MUTATE).
    fn try_open(&self, wire: &[u8]) -> Result<OpenOk, WireError> {
        match &self.profile {
            EnvelopeProfile::Legacy {
                hub_handle,
                protocol_version,
                cipher_suite_id,
                session_id_internal,
                send_direction,
            } => {
                let header = parse_legacy(wire)?;
                if header.hub_handle != hub_handle.raw() {
                    return Err(WireError::UnknownHandle(header.hub_handle));
                }
                let candidate = self.replay.reconstruct(u64::from(header.seq16));
                let pn = self.replay.precheck(candidate)?;
                let epoch = self
                    .legacy_recv_epoch()
                    .ok_or(WireError::EpochKeysMissing)?;
                let cipher = self.recv[idx(epoch)]
                    .as_ref()
                    .ok_or(WireError::EpochKeysMissing)?;
                let aad = LegacyAadFields {
                    protocol_version: *protocol_version,
                    cipher_suite_id: *cipher_suite_id,
                    // Alınan paket eşin yönündedir (ccr §35.9 cross-direction ayrımı).
                    direction_code: send_direction.opposite().code(),
                    epoch: u16::from(epoch),
                    session_id_internal,
                };
                let frame = open_legacy(cipher, &aad, candidate, wire)?;
                Ok(OpenOk { pn, epoch, frame })
            }
            EnvelopeProfile::Extended { h_ext } => {
                let outer = parse_extended(wire)?;
                if outer.h_ext != h_ext.raw() {
                    return Err(WireError::UnknownHandle(outer.h_ext));
                }
                // `dh_epoch` wire'dadır → TEK anahtar (ccr §35.8 "anahtar denemesi" notu).
                let epoch = outer.dh_epoch & FLAG_DH_EPOCH;
                if !self.recv_accepts[idx(epoch)] {
                    return Err(WireError::EpochNotAccepted(epoch));
                }
                let cipher = self.recv[idx(epoch)]
                    .as_ref()
                    .ok_or(WireError::EpochKeysMissing)?;
                let candidate = self.replay.reconstruct(u64::from(outer.pn_trunc));
                let pn = self.replay.precheck(candidate)?;
                let frame = open_extended(cipher, candidate, wire)?;
                Ok(OpenOk { pn, epoch, frame })
            }
        }
    }

    /// Salt-durum özeti — anahtar BAYTI içermez (yalnız varlık bayrakları).
    #[cfg(feature = "test-util")]
    fn fingerprint(&self) -> String {
        format!(
            "{:?}|pn={}|send_epoch={}|accepts={:?}|send_keys={:?}|recv_keys={:?}|lifecycle={:?}|\
             renew={}|replay={}",
            self.profile,
            self.next_send_pn,
            self.send_epoch,
            self.recv_accepts,
            self.send.each_ref().map(Option::is_some),
            self.recv.each_ref().map(Option::is_some),
            self.lifecycle,
            self.renew_signalled,
            self.replay.fingerprint()
        )
    }
}

/// GERÇEK S3 `PacketProtection` (KM-SEAM-TC-001 §1.1) — endpoint başına TEK nesne. L-A5c aynı
/// tipe `SessionControl`'ü ekler (iki trait tek nesnede; olay kuyruğu ortak).
///
/// * **Sayaç (S3 K2):** oturum başına TEK gönderim sayacı, epoch'lar boyunca SÜREKLİ; Crypto
///   atar. [`SEND_PN_CAP`]'e ulaşınca `seal` → `CapReached` (sayaç tüketilmez) ve İLK seferde tek
///   `RenewRequired(h)` olayı kuyruğa girer.
/// * **Zarf:** oturum başına SABİT [`EnvelopeProfile`] (legacy 22 B / extended 26 B); `limits(ctx)`
///   o profili raporlar.
/// * **Alım (S3 K5):** demux (ilk dört bayt) → parse → reconstruct → precheck (saf) → AEAD →
///   commit (TEK mutasyon). Her başarısızlık `None` ve state bayt-özdeş (H-NO-MUTATE); commit'ten
///   sonra başarısızlık yolu YOKTUR. Çerçeve OPAK — iç parse yapılmaz (S3 K4).
/// * **HubChannel:** PoC no-op, `client-core` `IdentitySeam` ile BAYT/ALAN ÖZDEŞ (`frame ‖ 0×16`,
///   ayrı sayaç, `dh_epoch = 0`). PoC no-op hub kanalı kimlik doğrulamaz (IdentitySeam
///   paritesi); oturum trafiği hiçbir zaman HubChannel'a atfedilmez (demux önce): `open` önce
///   ilk dört baytı kayıtlı oturum tutamaçlarıyla eşler; tutamaç kayıtlıysa sonuç ya o oturumdur
///   ya `None`. Yalnız tutamacı bilinmeyen VE son 16 baytı sıfır olan datagram `HubChannel`'a
///   atfedilir. Gerçek hop anahtarı KAPSAM DIŞI (M4).
/// * **Oturum tablosu:** oturum YALNIZ `pub(crate)` kurulum API'siyle eklenir; ürün yolunda onu
///   yalnız L-A5c'nin `SessionControl`'ü, kendi kabul kapılarının (HALF_OPEN_CAP / pending
///   tavanı) arkasında çağırır. Tablo bu kapılarla sınırlıdır; ayrı tavan sabiti yoktur.
/// * **Olay kuyruğu:** bu katmanın tek üreticisi `RenewRequired`'dır (oturum başına en çok bir
///   kez; `close` o oturumun bekleyen `RenewRequired`'ını düşürür) → kuyruk ≤ canlı oturum sayısı.
#[derive(Default)]
pub struct NoiseWireProtection {
    sessions: BTreeMap<SessionHandle, SessionCrypto>,
    /// Wire tutamacı (`hub_handle` / `h_ext`) → oturum (decrypt ÖNCESİ demux; noise-wire §2.5).
    by_wire_handle: BTreeMap<u32, SessionHandle>,
    /// Sonraki basılacak oturum tutamacının ham değeri (tutamaç tekrar kullanılmaz).
    next_session_raw: u64,
    /// HubChannel no-op'un AYRI PN sayacı (S3 K1).
    hub_next_pn: u64,
    /// Crypto → Transport olayları (L-A5c'nin `SessionControl::poll_event`'i boşaltır).
    events: VecDeque<SessionEvent>,
    /// `seal()`'e en son verilen bağlam (S3 K1 kayıt kancası; YALNIZ test).
    #[cfg(feature = "test-util")]
    last_seal_ctx: Option<ProtectionContext>,
}

impl fmt::Debug for NoiseWireProtection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Anahtar ve oturum kimliği Debug'a GİRMEZ; yalnız sayımlar.
        f.debug_struct("NoiseWireProtection")
            .field("sessions", &self.sessions.len())
            .field("hub_next_pn", &self.hub_next_pn)
            .field("pending_events", &self.events.len())
            .finish_non_exhaustive()
    }
}

impl NoiseWireProtection {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ── Kurulum API'si (pub(crate): L-A5c'nin SessionControl'ü çağırır) ──────────────────────

    /// Oturum kurar ve tutamacını basar (S3 K8). Wire tutamacı başka bir oturumda kayıtlıysa ya
    /// da tutamaç uzayı tükendiyse `None`; state DEĞİŞMEZ.
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    #[must_use]
    pub(crate) fn install_session(
        &mut self,
        material: SessionKeyMaterial,
    ) -> Option<SessionHandle> {
        let wire = material.profile.wire_handle();
        if self.by_wire_handle.contains_key(&wire) {
            return None;
        }
        let raw = self.next_session_raw;
        let after = raw.checked_add(1)?;
        let h = SessionHandle::from_raw(raw);
        self.next_session_raw = after;
        self.by_wire_handle.insert(wire, h);
        self.sessions.insert(h, SessionCrypto::new(material));
        Some(h)
    }

    /// Epoch-1 trafik anahtarlarını kurar (ccr §35.2.1 D5); kurulunca epoch-1 alımı kabul edilir
    /// (noise-wire §4 B3 adım 3). Sayaç ve pencere SIFIRLANMAZ (EPOCH-PN-INIT). Oturum yoksa,
    /// aktif değilse ya da epoch-1 zaten kuruluysa `false` (anahtar DEĞİŞTİRİLMEZ).
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    #[must_use]
    pub(crate) fn install_epoch1(
        &mut self,
        h: SessionHandle,
        send_e1: &Zeroizing<[u8; KEY_LEN]>,
        recv_e1: &Zeroizing<[u8; KEY_LEN]>,
    ) -> bool {
        let Some(s) = self.sessions.get_mut(&h) else {
            return false;
        };
        let e1 = idx(DH_EPOCH_1);
        if s.lifecycle != Lifecycle::Active || s.send[e1].is_some() || s.recv[e1].is_some() {
            return false;
        }
        s.send[e1] = Some(cipher_from(send_e1));
        s.recv[e1] = Some(cipher_from(recv_e1));
        s.recv_accepts[e1] = true;
        true
    }

    /// Gönderim epoch'unu İLERİ alır (yalnız 0 → 1; anahtarı kurulu olmalı). Epoch-1'e geçişte
    /// epoch-0 gönderim anahtarı silinir. Sayaç DEVAM eder: epoch-1'in ilk PN'i son epoch-0 PN'i
    /// + 1, epoch-0'da hiç mühürlenmediyse 0 (noise-wire §4 EPOCH-PN-INIT).
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    #[must_use]
    pub(crate) fn set_send_epoch(&mut self, h: SessionHandle, epoch: u8) -> bool {
        let Some(s) = self.sessions.get_mut(&h) else {
            return false;
        };
        if s.lifecycle != Lifecycle::Active
            || epoch > DH_EPOCH_1
            || epoch < s.send_epoch
            || s.send[idx(epoch)].is_none()
        {
            return false;
        }
        if epoch == DH_EPOCH_1 {
            s.send[idx(DH_EPOCH_0)] = None;
        }
        s.send_epoch = epoch;
        true
    }

    /// Grace kapanışı (noise-wire §4 KS-GRACE-1): epoch-0 alım state'i atılır ve bir daha
    /// AÇILMAZ (kabul bayrağını geri çeviren yol yoktur). Kuralın kendisi (`N_g` / `T_g`)
    /// L-A5c'nindir. Epoch-1 alımı kurulmadan çağrılırsa `false` (oturum sağırlaşırdı).
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    #[must_use]
    pub(crate) fn discard_epoch0_recv(&mut self, h: SessionHandle) -> bool {
        let Some(s) = self.sessions.get_mut(&h) else {
            return false;
        };
        if s.recv[idx(DH_EPOCH_1)].is_none() {
            return false;
        }
        s.recv_accepts[idx(DH_EPOCH_0)] = false;
        s.recv[idx(DH_EPOCH_0)] = None;
        true
    }

    /// Gönderim yarısını emekliye ayırır: send anahtarları silinir, `seal` → `UnknownSession`;
    /// alım sürer. Geri alınamaz.
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    #[must_use]
    pub(crate) fn retire_send(&mut self, h: SessionHandle) -> bool {
        let Some(s) = self.sessions.get_mut(&h) else {
            return false;
        };
        s.lifecycle = Lifecycle::ReceiveOnly;
        s.send = [None, None];
        true
    }

    /// Oturumu siler (anahtarlar drop'ta silinir), wire tutamacını demux tablosundan çıkarır ve
    /// bekleyen `RenewRequired(h)`'yi düşürür. Olay ÜRETMEZ (`Closed` L-A5c'nin
    /// `SessionControl`'ündedir).
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    #[must_use]
    pub(crate) fn close(&mut self, h: SessionHandle) -> bool {
        let Some(s) = self.sessions.remove(&h) else {
            return false;
        };
        self.by_wire_handle.remove(&s.profile.wire_handle());
        self.events
            .retain(|ev| *ev != SessionEvent::RenewRequired(h));
        true
    }

    /// Sıradaki bekleyen olay (L-A5c `SessionControl::poll_event`).
    #[cfg_attr(not(feature = "test-util"), allow(dead_code))]
    pub(crate) fn take_event(&mut self) -> Option<SessionEvent> {
        self.events.pop_front()
    }

    fn push_event(&mut self, ev: SessionEvent) {
        self.events.push_back(ev);
    }

    // ── Veri yolu (PacketProtection'ın gövdeleri) ───────────────────────────────────────────

    /// Oturum bağlamının seal kapısı — `seal` ve `sealed_len` AYNI sırayla: çerçeve tavanı
    /// (oturum yoksa sınıfın güvenli minimumu) → oturum var ve gönderebilir.
    fn check_session_seal(
        &self,
        h: SessionHandle,
        plaintext_len: usize,
    ) -> Result<&SessionCrypto, SealError> {
        let session = self.sessions.get(&h);
        let limits = session.map_or(SESSION_CONSERVATIVE_LIMITS, |s| s.profile.limits());
        if plaintext_len > limits.max_plaintext_bytes as usize {
            return Err(SealError::FrameTooLarge);
        }
        match session {
            Some(s) if s.lifecycle == Lifecycle::Active => Ok(s),
            _ => Err(SealError::UnknownSession),
        }
    }

    /// Sıra: `FrameTooLarge` → `UnknownSession` → `CapReached` (+ ilk seferde `RenewRequired`)
    /// → mühür → BAŞARIDA sayaç +1. Hata yollarında sayaç/pencere/anahtar DEĞİŞMEZ.
    fn seal_session(&mut self, h: SessionHandle, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.check_session_seal(h, frame.len())?;
        let s = self.sessions.get_mut(&h).ok_or(SealError::UnknownSession)?;
        let Some((pn, next)) = allocate_send_pn(s.next_send_pn) else {
            let first = !s.renew_signalled;
            s.renew_signalled = true;
            if first {
                self.push_event(SessionEvent::RenewRequired(h));
            }
            return Err(SealError::CapReached);
        };
        let bytes = s.seal_envelope(pn, frame)?;
        let sealed_len = u32::try_from(bytes.len()).map_err(|_| SealError::FrameTooLarge)?;
        debug_assert_eq!(
            bytes.len(),
            frame.len() + s.profile.overhead(),
            "seal(ctx, f).len() != sealed_len(ctx, f.len())"
        );
        s.next_send_pn = next;
        Ok(SealedUnit {
            bytes,
            meta: s3::SealedPacketMeta {
                full_wire_pn: pn,
                dh_epoch: s.send_epoch,
                sealed_len,
            },
        })
    }

    /// `IdentitySeam::seal` ile bayt/alan özdeş: `frame ‖ 0×16`, ayrı sayaç, `dh_epoch = 0`.
    fn seal_hub(&mut self, frame: &[u8]) -> Result<SealedUnit, SealError> {
        if frame.len() > HUB_NOOP_LIMITS.max_plaintext_bytes as usize {
            return Err(SealError::FrameTooLarge);
        }
        let pn = self.hub_next_pn;
        // Sayaç SARMAZ (IdentitySeam ile aynı kural).
        let next = pn.checked_add(1).ok_or(SealError::CapReached)?;
        let mut bytes = Vec::with_capacity(frame.len() + TAG_LEN);
        bytes.extend_from_slice(frame);
        bytes.resize(frame.len() + TAG_LEN, 0);
        let sealed_len = u32::try_from(bytes.len()).map_err(|_| SealError::FrameTooLarge)?;
        self.hub_next_pn = next;
        Ok(SealedUnit {
            bytes,
            meta: s3::SealedPacketMeta {
                full_wire_pn: pn,
                dh_epoch: DH_EPOCH_0,
                sealed_len,
            },
        })
    }

    /// Saf aşama → (başarıda) TEK commit → `Opened`. Commit'ten sonra başarısızlık yolu YOK.
    fn open_session(&mut self, h: SessionHandle, wire: &[u8]) -> Option<Opened> {
        let s = self.sessions.get_mut(&h)?;
        let ok = s.try_open(wire).ok()?;
        let sealed_len = u32::try_from(wire.len()).ok()?;
        let full_wire_pn = ok.pn.full_pn();
        // noise-wire §4-ORDER (6): AEAD BAŞARISINDAN SONRA commit — TEK mutasyon.
        s.replay.commit(ok.pn);
        Some(Opened {
            ctx: ProtectionContext::Session(h),
            frame: ok.frame,
            meta: s3::SealedPacketMeta {
                full_wire_pn,
                dh_epoch: ok.epoch,
                sealed_len,
            },
        })
    }

    /// `IdentitySeam::open` paritesi; yalnız tutamacı kayıtlı OLMAYAN datagram buraya gelir ve
    /// son 16 baytı sıfır olmalıdır. State'e dokunmaz.
    fn open_hub(wire: &[u8]) -> Option<Opened> {
        if wire.len() <= TAG_LEN {
            return None;
        }
        let (frame, tag) = wire.split_at(wire.len() - TAG_LEN);
        if tag.iter().any(|&b| b != 0) {
            return None;
        }
        let sealed_len = u32::try_from(wire.len()).ok()?;
        Some(Opened {
            ctx: ProtectionContext::HubChannel,
            frame: frame.to_vec(),
            meta: s3::SealedPacketMeta {
                full_wire_pn: HUB_NOOP_RECV_PN,
                dh_epoch: DH_EPOCH_0,
                sealed_len,
            },
        })
    }

    // ── YALNIZ test (`test-util`): kurulum sarmalayıcıları ve gözlem kancaları ─────────────

    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn install_session_for_test(
        &mut self,
        material: SessionKeyMaterial,
    ) -> Option<SessionHandle> {
        self.install_session(material)
    }

    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn install_epoch1_for_test(
        &mut self,
        h: SessionHandle,
        send_e1: &Zeroizing<[u8; KEY_LEN]>,
        recv_e1: &Zeroizing<[u8; KEY_LEN]>,
    ) -> bool {
        self.install_epoch1(h, send_e1, recv_e1)
    }

    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn set_send_epoch_for_test(&mut self, h: SessionHandle, epoch: u8) -> bool {
        self.set_send_epoch(h, epoch)
    }

    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn discard_epoch0_recv_for_test(&mut self, h: SessionHandle) -> bool {
        self.discard_epoch0_recv(h)
    }

    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn retire_send_for_test(&mut self, h: SessionHandle) -> bool {
        self.retire_send(h)
    }

    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn close_for_test(&mut self, h: SessionHandle) -> bool {
        self.close(h)
    }

    #[cfg(feature = "test-util")]
    pub fn take_event_for_test(&mut self) -> Option<SessionEvent> {
        self.take_event()
    }

    /// Gönderim sayacını İLERİ taşır (vektör sayaçları ve `SEND_PN_CAP` sınırı için). Geri alma
    /// YOK — aynı anahtar altında nonce tekrarı testte de üretilemez. `pn > SEND_PN_CAP`
    /// reddedilir.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn set_next_send_pn_for_test(&mut self, h: SessionHandle, pn: u64) -> bool {
        match self.sessions.get_mut(&h) {
            Some(s) if pn >= s.next_send_pn && pn <= SEND_PN_CAP => {
                s.next_send_pn = pn;
                true
            }
            _ => false,
        }
    }

    /// Bir oturumun alım penceresinin `expected_pn`'i (en yüksek kabul + 1).
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn recv_expected_pn_for_test(&self, h: SessionHandle) -> Option<u64> {
        self.sessions.get(&h).map(|s| s.replay.expected_pn())
    }

    /// `open`'ın oturum yolunun AYNI saf aşaması, commit'siz ve TYPED: `None`'ın gerekçesini
    /// gözlemlemek içindir (vektör sözlüğü). Döner: `(full_pn, dh_epoch, frame)`.
    /// `MAX_UDP_PAYLOAD` üstü datagram ve HubChannel yolu burada sınanmaz.
    ///
    /// # Errors
    /// Saf aşamanın [`WireError`]'ı; tutamaç kayıtlı değilse [`WireError::UnknownHandle`].
    #[cfg(feature = "test-util")]
    pub fn inspect_open_for_test(&self, wire: &[u8]) -> Result<(u64, u8, Vec<u8>), WireError> {
        let handle = wire_handle(wire).ok_or(WireError::DatagramTooShort {
            got: wire.len(),
            min: crate::envelope::WIRE_HANDLE_LEN,
        })?;
        let session = self
            .by_wire_handle
            .get(&handle)
            .and_then(|h| self.sessions.get(h))
            .ok_or(WireError::UnknownHandle(handle))?;
        let ok = session.try_open(wire)?;
        Ok((ok.pn.full_pn(), ok.epoch, ok.frame))
    }

    /// Oturum (sayaç, gönderim epoch'u, kabul bayrakları, anahtar VARLIK bayrakları, pencere
    /// `expected` + bitler, yaşam döngüsü, renew bayrağı), demux tablosu, tutamaç sayacı, hub
    /// sayacı ve olay kuyruğu. Anahtar baytı ASLA; salt-gözlem alanı (`last_seal_ctx`) dışarıda.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn state_fingerprint(&self) -> String {
        let sessions: Vec<String> = self
            .sessions
            .iter()
            .map(|(h, s)| format!("{h:?}={}", s.fingerprint()))
            .collect();
        format!(
            "sessions={sessions:?}|wire={:?}|next_raw={}|hub_pn={}|events={:?}",
            self.by_wire_handle, self.next_session_raw, self.hub_next_pn, self.events
        )
    }

    /// `seal()`'e en son verilen bağlam (S3 K1: bağlam çağırandan gelir, seam'e DEĞİŞMEDEN ulaşır).
    #[cfg(feature = "test-util")]
    #[must_use]
    pub fn last_seal_ctx(&self) -> Option<ProtectionContext> {
        self.last_seal_ctx
    }
}

impl PacketProtection for NoiseWireProtection {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        let unit = match ctx {
            ProtectionContext::Session(h) => self.seal_session(h, frame)?,
            ProtectionContext::HubChannel => self.seal_hub(frame)?,
        };
        #[cfg(feature = "test-util")]
        {
            self.last_seal_ctx = Some(ctx);
        }
        Ok(unit)
    }

    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        // bounds-before-alloc: UDP'nin taşıyamayacağı datagram hiçbir yola girmez.
        if wire.len() > MAX_UDP_PAYLOAD {
            return None;
        }
        // Demux ÖNCE: kayıtlı tutamaç → yalnız o oturum (asla HubChannel'a düşmez).
        let session = wire_handle(wire).and_then(|w| self.by_wire_handle.get(&w).copied());
        match session {
            Some(h) => self.open_session(h, wire),
            None => Self::open_hub(wire),
        }
    }

    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        match ctx {
            ProtectionContext::Session(h) => {
                let s = self.check_session_seal(h, plaintext_len)?;
                Ok(plaintext_len + s.profile.overhead())
            }
            ProtectionContext::HubChannel => {
                if plaintext_len > HUB_NOOP_LIMITS.max_plaintext_bytes as usize {
                    return Err(SealError::FrameTooLarge);
                }
                Ok(plaintext_len + TAG_LEN)
            }
        }
    }

    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        match class {
            ProtectionClass::Session => SESSION_CONSERVATIVE_LIMITS,
            ProtectionClass::HubChannel => HUB_NOOP_LIMITS,
        }
    }

    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        match ctx {
            ProtectionContext::Session(h) => self
                .sessions
                .get(&h)
                .map_or(SESSION_CONSERVATIVE_LIMITS, |s| s.profile.limits()),
            ProtectionContext::HubChannel => HUB_NOOP_LIMITS,
        }
    }
}
