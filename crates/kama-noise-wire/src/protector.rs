//! KamaNoiseWireProtector — EXTENDED seal/open (§0 API + §4-ORDER + §0.5 sizer/meta).
//!
//! AEAD: ChaCha20-Poly1305 (RFC 8439; pki-§13 registry: key 32B / nonce 12B / tag 16B).
//! NONCE-KURGUSU [DR-4 — ccr-§35-aktifte-byte-exact-tanım-YOK]: nonce = [4B-zero || full_pn u64_be]
//! (görev-promptu-varsayılanı; dh_epoch nonce'a-GİRMEZ — epoch AYRI-cipher-state/key'dedir, ccr-emsal
//! epoch_1_root-HKDF). DR-4-kararı değişirse yalnız bu-fonksiyon + golden'lar güncellenir.
//!
//! Anahtar-yaşamı: [`Zeroizing`] kopyalar; cipher-state drop'ta anahtar-materyali bırakır (zeroize).

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use zeroize::Zeroizing;

use crate::aad::build_aad;
use crate::error::WireError;
use crate::inner::{
    encode_control_inner, encode_fec_inner, parse_inner, CONTROL_INNER_PREFIX_LEN,
    FEC_INNER_HDR_LEN,
};
use crate::opened::OpenedTransportShard;
use crate::outer::{ExtendedOuter, OUTER_HDR_LEN, TAG_LEN};
use crate::pn::{reconstruct, ReplayWindow, EXTENDED_PN_PARAMS};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{UnsealedTransportPacket, EXTENDED_OUTER_OVERHEAD};

/// Full-PN gönderim-tavanı [CAL — renewal-politikası Sprint-4]: aşımı SESSION_RENEW_REQUIRED tetikler.
pub const SEND_PN_CAP: u64 = 1 << 62;

/// AEAD-anahtar boyu (registry: 32B).
pub const KEY_LEN: usize = 32;

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
    /// Nonce-reuse yapısal-guard: epoch-başına son-seal-PN (yeni-PN kesin-büyük olmalı).
    last_sealed_pn: [Option<u64>; 2],
    recv_windows: [ReplayWindow; 2],
    /// Epoch-kabul bayrakları (§4-adım-1 + B3): [0]=epoch-0 (grace-bitince-false), [1]=epoch-1
    /// (anahtar-kurulunca-true). B3-adım-3: Bob epoch-0'dayken-de geçerli-epoch-1-paketi-KABUL-EDER.
    recv_epoch_accepts: [bool; 2],
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
            last_sealed_pn: [None, None],
            recv_windows: [
                ReplayWindow::new(EXTENDED_PN_PARAMS),
                ReplayWindow::new(EXTENDED_PN_PARAMS),
            ],
            recv_epoch_accepts: [true, false],
        }
    }

    /// SESSION_INIT_RESPONSE-sonrası epoch-1 anahtar-kurulumu (dh_epoch 0→1; §5).
    /// Epoch-1 cipher-state YENİ → PN-0'dan-başlar (§4 EPOCH-PN-INIT).
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
    pub fn recv_expected_pn(&self, dh_epoch: u8) -> u64 {
        self.recv_windows[usize::from(dh_epoch & 1)].expected_pn()
    }

    /// DR-4 nonce-kurgusu: [4B-zero || full_pn u64_be].
    fn nonce_for(full_pn: u64) -> Nonce {
        let mut n = [0u8; 12];
        n[4..12].copy_from_slice(&full_pn.to_be_bytes());
        *Nonce::from_slice(&n)
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
        let ei = usize::from(args.dh_epoch & 1);
        let cipher = self.send_cipher[ei]
            .as_ref()
            .ok_or(WireError::EpochKeysMissing)?;
        if args.wire_pn >= SEND_PN_CAP {
            return Err(WireError::PnCapReached);
        }
        if let Some(last) = self.last_sealed_pn[ei] {
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
        self.last_sealed_pn[ei] = Some(args.wire_pn);
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
        // (3) low_bits + dh_epoch → candidate
        let candidate = reconstruct(
            &EXTENDED_PN_PARAMS,
            self.recv_windows[ei].expected_pn(),
            u64::from(outer.pn_trunc),
        );
        // (4) too-old/dup/future precheck — window-İLERLETMEDEN
        self.recv_windows[ei].precheck(candidate)?;
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
                                                // (6) SUCCESS → COMMIT (yalnız-ilgili-epoch-state)
        self.recv_windows[ei].commit(candidate);
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
