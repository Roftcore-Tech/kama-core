//! S4 §2 tel biçiminin hub istemcisinin gerektirdiği alt kümesi — **yerel tip** (KK-H34).
//!
//! `kama_contracts::s4` (HA) `main`'e girene dek burada durur; girince geçiş ayrı commit'te yapılır
//! ve bu modül kaldırılır. Bayt farkı çıkarsa S4 kazanır, `vectors/hub` hakemdir.
//!
//! Tüm sayılar big-endian, tüm alanlar sabit genişlikte (ADR-05; varint yok). Çözme KATIDIR: sabit
//! alanlı yükte fazla bayt, uzunluk alanının kalan baytla tutmaması, PoC'de sabit olan alanın farklı
//! olması hatadır.
//!
//! Çerçeve sınırı: bu modül **açılmış** kontrol çerçevesini kodlar/çözer:
//! `[0;4] ‖ frame_type ‖ gövde` (16 B no-op kuyruk kriptonun `seal_hub`/`open_hub`'ındadır).
//! Kodlayıcılar sınırı kodlamadan ÖNCE denetler (bounds-before-alloc, SR-AUDIT-001).

use super::signer::{BIND_SIGNATURE_LEN, BIND_TBS_LEN};

/// `MAX_DATAGRAM` (S4 §6).
pub const MAX_DATAGRAM: usize = 1280;
/// HubChannel no-op mühür kuyruğu (S4 §2.1; kriptoda).
pub const HUB_TRAILER_LEN: usize = 16;
/// Açılmış kontrol çerçevesinin azami boyu (`MAX_DATAGRAM − 16`).
pub const MAX_OPENED_FRAME: usize = MAX_DATAGRAM - HUB_TRAILER_LEN;
/// Kontrol öneki.
pub const CONTROL_PREFIX: [u8; 4] = [0; 4];
/// `AppRpcFrame` başlığı: `method_id u8 · flags u8 · request_id u32 · payload_len u32`.
pub const RPC_HEADER_LEN: usize = 10;
/// Bir RPC datagramının yük dışı baytları: önek 4 + tip 1 + başlık 10 + kuyruk 16 = 31.
pub const RPC_OVERHEAD: usize = CONTROL_PREFIX.len() + 1 + RPC_HEADER_LEN + HUB_TRAILER_LEN;
/// STORE / DELIVER yükünün zarf dışı baytları: `handle 4 · tip 1 · dest_hash 32 · len 4`.
pub const ENVELOPE_PAYLOAD_OVERHEAD: usize = 4 + 1 + 32 + 4;
/// En uzun zarf (1208 B).
pub const MAX_ENVELOPE: usize = MAX_DATAGRAM - RPC_OVERHEAD - ENVELOPE_PAYLOAD_OVERHEAD;
/// HEARTBEAT `echo_token` boyu.
pub const ECHO_TOKEN_LEN: usize = 4;
/// `client_nonce` / `hub_nonce` boyu.
pub const NONCE_LEN: usize = 16;

/// BIND_PROOF TBS etki alanı ayracı (ccr §13.2: ASCII ≤ 16 B + `0x00` dolgu).
pub const BIND_DOMAIN: [u8; 16] = *b"KAMA-HUB-BIND-V1";
/// Bind ACK TBS etki alanı ayracı.
pub const ACK_DOMAIN: [u8; 16] = *b"KAMA-HUB-ACK-V1\0";
/// Bind ACK TBS boyu (16 + 32 + 16 + 16 + 1 + 2).
pub const BIND_ACK_TBS_LEN: usize = 83;
/// Başarılı BIND_PROOF yanıtındaki `bind_ok`.
pub const BIND_OK: u8 = 1;

/// HubChannel `frame_type` (engine-spec §1.1; `0x1D` S4 [YAMA]).
pub mod frame {
    pub const HEARTBEAT: u8 = 0x1B;
    pub const HEARTBEAT_ACK: u8 = 0x1C;
    pub const PLANE1_RPC: u8 = 0x1D;
}

/// `AppRpcFrame.method_id` (S4 §2.3). Bit 7: alışverişi hub başlatır.
pub mod method {
    pub const CLIENT_BIND: u8 = 0x01;
    pub const STORE_MESSAGE: u8 = 0x05;
    pub const BIND_PROOF: u8 = 0x0B;
    pub const DELIVER: u8 = 0x81;
    pub const ROUTE_MISS: u8 = 0x84;
}

/// `AppRpcFrame.flags`.
pub mod flags {
    pub const REQUEST: u8 = 0x00;
    pub const RESPONSE: u8 = 0x01;
    pub const RESPONSE_ERROR: u8 = 0x03;
}

/// `envelope_type` (S4 §2.4(c)).
pub mod envelope {
    pub const SESSION_INIT: u8 = 0x01;
    pub const SESSION_INIT_RESPONSE: u8 = 0x02;
    pub const RELAY_OPEN: u8 = 0x05;
}

/// Durum ve hata kodları (S4 §2.5).
pub mod code {
    pub const OK: u16 = 0x0000;
    pub const CHALLENGE: u16 = 0x0003;
    pub const POSSESSION_PROOF_FAIL: u16 = 0x0105;
    pub const BAD_COOKIE: u16 = 0x0106;
    pub const PEER_UNBOUND: u16 = 0x0301;
    pub const UNKNOWN_HANDLE: u16 = 0x0302;
    pub const HUB_CAPACITY: u16 = 0x0501;
    pub const RATE_LIMITED: u16 = 0x0502;
    pub const BAD_REQUEST: u16 = 0x0601;
}

/// ROUTE_MISS `reason` 0x01 UNKNOWN_HANDLE.
pub const ROUTE_MISS_UNKNOWN_HANDLE: u8 = 0x01;

/// Çözme / kodlama hatası; içerik taşımaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// Beklenen alan için bayt yok.
    Truncated,
    /// Sabit alanlı yükte fazla bayt.
    TrailingBytes,
    /// Kontrol öneki sıfır değil.
    NotControl,
    /// Uzunluk alanı kalan baytla tutmuyor ya da sınırı aşıyor.
    Length,
    /// PoC'de sabit olan bir alanın değeri farklı.
    Field,
    /// Kodlanan çerçeve + 16 B `MAX_DATAGRAM`'ı aşar.
    TooLong,
}

// ── Kodlayıcılar (tel → hub) ────────────────────────────────────────────────────────────────

/// RPC çerçevesi: `[0;4] ‖ 0x1D ‖ method ‖ flags ‖ request_id ‖ payload_len ‖ payload`.
fn rpc_frame(method_id: u8, request_id: u32, parts: &[&[u8]]) -> Result<Vec<u8>, CodecError> {
    let payload_len: usize = parts.iter().map(|p| p.len()).sum();
    // Sınır kodlamadan ÖNCE (bounds-before-alloc).
    if payload_len > MAX_DATAGRAM - RPC_OVERHEAD {
        return Err(CodecError::TooLong);
    }
    let len = u32::try_from(payload_len).map_err(|_| CodecError::TooLong)?;
    let mut out = Vec::with_capacity(CONTROL_PREFIX.len() + 1 + RPC_HEADER_LEN + payload_len);
    out.extend_from_slice(&CONTROL_PREFIX);
    out.push(frame::PLANE1_RPC);
    out.push(method_id);
    out.push(flags::REQUEST);
    out.extend_from_slice(&request_id.to_be_bytes());
    out.extend_from_slice(&len.to_be_bytes());
    for p in parts {
        out.extend_from_slice(p);
    }
    Ok(out)
}

/// CLIENT_BIND istek (§2.4(a)): `device_cert_mode u8 = 0 · device_cert_ref · presence_flags u8 = 0 ·
/// client_nonce`.
#[must_use]
pub fn encode_client_bind(
    request_id: u32,
    cert_ref: &[u8; 32],
    client_nonce: &[u8; NONCE_LEN],
) -> Vec<u8> {
    // 50 B yük: sınırın çok altında; hata yolu yok.
    rpc_frame(
        method::CLIENT_BIND,
        request_id,
        &[&[0], cert_ref, &[0], client_nonce],
    )
    .unwrap_or_default()
}

/// BIND_PROOF istek (§2.4(b)): `device_cert_ref · client_nonce · hub_nonce · client_signature`.
#[must_use]
pub fn encode_bind_proof(
    request_id: u32,
    cert_ref: &[u8; 32],
    client_nonce: &[u8; NONCE_LEN],
    hub_nonce: &[u8; NONCE_LEN],
    signature: &[u8; BIND_SIGNATURE_LEN],
) -> Vec<u8> {
    // 128 B yük; hata yolu yok.
    rpc_frame(
        method::BIND_PROOF,
        request_id,
        &[cert_ref, client_nonce, hub_nonce, signature],
    )
    .unwrap_or_default()
}

/// STORE_MESSAGE istek (§2.4(c)): `hub_handle · envelope_type · recipient_dest_hash · envelope_len ·
/// envelope`.
///
/// # Errors
/// [`CodecError::TooLong`] — zarf [`MAX_ENVELOPE`]'u aşar.
pub fn encode_store(
    request_id: u32,
    hub_handle: u32,
    envelope_type: u8,
    recipient: &[u8; 32],
    envelope: &[u8],
) -> Result<Vec<u8>, CodecError> {
    if envelope.len() > MAX_ENVELOPE {
        return Err(CodecError::TooLong);
    }
    let len = u32::try_from(envelope.len()).map_err(|_| CodecError::TooLong)?;
    rpc_frame(
        method::STORE_MESSAGE,
        request_id,
        &[
            &hub_handle.to_be_bytes(),
            &[envelope_type],
            recipient,
            &len.to_be_bytes(),
            envelope,
        ],
    )
}

/// HEARTBEAT (§2.2): `[0;4] ‖ 0x1B ‖ echo_token u32`.
#[must_use]
pub fn encode_heartbeat(token: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(CONTROL_PREFIX.len() + 1 + ECHO_TOKEN_LEN);
    out.extend_from_slice(&CONTROL_PREFIX);
    out.push(frame::HEARTBEAT);
    out.extend_from_slice(&token.to_be_bytes());
    out
}

/// BIND_PROOF TBS (112 B; §2.4(b)): `domain_sep("KAMA-HUB-BIND-V1") ‖ hub_ed25519_pub ‖
/// device_cert_ref ‖ client_nonce ‖ hub_nonce`.
#[must_use]
pub fn bind_proof_tbs(
    hub_pub: &[u8; 32],
    cert_ref: &[u8; 32],
    client_nonce: &[u8; NONCE_LEN],
    hub_nonce: &[u8; NONCE_LEN],
) -> [u8; BIND_TBS_LEN] {
    let mut tbs = [0u8; BIND_TBS_LEN];
    let parts: [&[u8]; 5] = [&BIND_DOMAIN, hub_pub, cert_ref, client_nonce, hub_nonce];
    fill(&mut tbs, &parts);
    tbs
}

/// Bind ACK TBS (83 B; §2.4(b)): `domain_sep("KAMA-HUB-ACK-V1") ‖ device_cert_ref ‖ client_nonce ‖
/// hub_nonce ‖ bind_ok ‖ error_code`.
#[must_use]
pub fn bind_ack_tbs(
    cert_ref: &[u8; 32],
    client_nonce: &[u8; NONCE_LEN],
    hub_nonce: &[u8; NONCE_LEN],
    bind_ok: u8,
    error_code: u16,
) -> [u8; BIND_ACK_TBS_LEN] {
    let mut tbs = [0u8; BIND_ACK_TBS_LEN];
    let parts: [&[u8]; 6] = [
        &ACK_DOMAIN,
        cert_ref,
        client_nonce,
        hub_nonce,
        &[bind_ok],
        &error_code.to_be_bytes(),
    ];
    fill(&mut tbs, &parts);
    tbs
}

fn fill(out: &mut [u8], parts: &[&[u8]]) {
    for (slot, byte) in out.iter_mut().zip(parts.iter().flat_map(|p| p.iter())) {
        *slot = *byte;
    }
}

// ── Çözücüler (hub → tel) ───────────────────────────────────────────────────────────────────

/// Açılmış çerçevenin sınıfı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inbound<'a> {
    /// `0x1C HEARTBEAT_ACK`; gövde boş ya da 4 B token.
    HeartbeatAck { token: Option<u32> },
    /// `0x1D PLANE1_RPC`.
    Rpc(RpcFrame<'a>),
    /// Başka `frame_type` (C8: sayaç).
    Unknown { frame_type: u8 },
}

/// `AppRpcFrame` (app-rpc §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcFrame<'a> {
    pub method_id: u8,
    pub flags: u8,
    pub request_id: u32,
    pub payload: &'a [u8],
}

/// Açılmış kontrol çerçevesini sınıflar: `[0;4] ‖ frame_type ‖ gövde`.
///
/// # Errors
/// [`CodecError`] — önek, uzunluk, katı alan.
pub fn decode_inbound(opened: &[u8]) -> Result<Inbound<'_>, CodecError> {
    if opened.len() > MAX_OPENED_FRAME {
        return Err(CodecError::TooLong);
    }
    let mut r = Reader(opened);
    if r.array::<4>()? != CONTROL_PREFIX {
        return Err(CodecError::NotControl);
    }
    match r.u8()? {
        frame::HEARTBEAT_ACK => match r.0.len() {
            0 => Ok(Inbound::HeartbeatAck { token: None }),
            ECHO_TOKEN_LEN => Ok(Inbound::HeartbeatAck {
                token: Some(r.u32()?),
            }),
            _ => Err(CodecError::Length),
        },
        frame::PLANE1_RPC => {
            let method_id = r.u8()?;
            let flags = r.u8()?;
            let request_id = r.u32()?;
            let payload = r.tail_u32()?;
            Ok(Inbound::Rpc(RpcFrame {
                method_id,
                flags,
                request_id,
                payload,
            }))
        }
        frame_type => Ok(Inbound::Unknown { frame_type }),
    }
}

/// CLIENT_BIND yanıt (meydan okuma): `bind_ok = 0 · 0u64 · 0u64 · CHALLENGE · hub_nonce` → `hub_nonce`.
///
/// # Errors
/// [`CodecError`].
pub fn decode_challenge(payload: &[u8]) -> Result<[u8; NONCE_LEN], CodecError> {
    let mut r = Reader(payload);
    r.expect_u8(0)?;
    r.expect_u64(0)?;
    r.expect_u64(0)?;
    r.expect_u16(code::CHALLENGE)?;
    let hub_nonce = r.array()?;
    r.end()?;
    Ok(hub_nonce)
}

/// BIND_PROOF yanıt (bağlandı): `bind_ok = 1 · 0u64 · 0u64 · OK · hub_signature` → `hub_signature`.
///
/// # Errors
/// [`CodecError`].
pub fn decode_bind_ack(payload: &[u8]) -> Result<[u8; BIND_SIGNATURE_LEN], CodecError> {
    let mut r = Reader(payload);
    r.expect_u8(BIND_OK)?;
    r.expect_u64(0)?;
    r.expect_u64(0)?;
    r.expect_u16(code::OK)?;
    let sig = r.array()?;
    r.end()?;
    Ok(sig)
}

/// `ErrorPayload { error_code u16; detail_len u16 = 0 }` → `error_code`.
///
/// # Errors
/// [`CodecError`].
pub fn decode_error(payload: &[u8]) -> Result<u16, CodecError> {
    let mut r = Reader(payload);
    let code = r.u16()?;
    r.expect_u16(0)?;
    r.end()?;
    Ok(code)
}

/// STORE_MESSAGE yanıt: `status u16 · assigned_handle u32`.
///
/// # Errors
/// [`CodecError`].
pub fn decode_store_response(payload: &[u8]) -> Result<(u16, u32), CodecError> {
    let mut r = Reader(payload);
    let status = r.u16()?;
    let handle = r.u32()?;
    r.end()?;
    Ok((status, handle))
}

/// DELIVER (§2.4(d)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deliver<'a> {
    pub hub_handle: u32,
    pub sender: [u8; 32],
    pub envelope_type: u8,
    pub envelope: &'a [u8],
}

/// DELIVER: `hub_handle · sender_dest_hash · envelope_type · envelope_len · envelope`.
///
/// # Errors
/// [`CodecError`].
pub fn decode_deliver(payload: &[u8]) -> Result<Deliver<'_>, CodecError> {
    let mut r = Reader(payload);
    Ok(Deliver {
        hub_handle: r.u32()?,
        sender: r.array()?,
        envelope_type: r.u8()?,
        envelope: r.tail_u32()?,
    })
}

/// ROUTE_MISS (§2.4(e)): `hub_handle · reason` → `(hub_handle, reason)`.
///
/// # Errors
/// [`CodecError`].
pub fn decode_route_miss(payload: &[u8]) -> Result<(u32, u8), CodecError> {
    let mut r = Reader(payload);
    let h = r.u32()?;
    let reason = r.u8()?;
    r.end()?;
    Ok((h, reason))
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let (head, rest) = self
            .0
            .split_first_chunk::<N>()
            .ok_or(CodecError::Truncated)?;
        self.0 = rest;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(u8::from_be_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn expect_u8(&mut self, v: u8) -> Result<(), CodecError> {
        (self.u8()? == v).then_some(()).ok_or(CodecError::Field)
    }

    fn expect_u16(&mut self, v: u16) -> Result<(), CodecError> {
        (self.u16()? == v).then_some(()).ok_or(CodecError::Field)
    }

    fn expect_u64(&mut self, v: u64) -> Result<(), CodecError> {
        (self.u64()? == v).then_some(()).ok_or(CodecError::Field)
    }

    /// `len u32 ‖ bytes[len]` — kalan baytla TAM tutmalı (önce uzunluk, sonra dilim; ayırma yok).
    fn tail_u32(&mut self) -> Result<&'a [u8], CodecError> {
        let declared = usize::try_from(self.u32()?).map_err(|_| CodecError::Length)?;
        if declared != self.0.len() {
            return Err(CodecError::Length);
        }
        Ok(core::mem::take(&mut self.0))
    }

    fn end(self) -> Result<(), CodecError> {
        self.0
            .is_empty()
            .then_some(())
            .ok_or(CodecError::TrailingBytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_s4() {
        assert_eq!(RPC_OVERHEAD, 31);
        assert_eq!(MAX_ENVELOPE, 1208);
        assert_eq!(BIND_DOMAIN.len() + 32 + 32 + 16 + 16, BIND_TBS_LEN);
        // CLIENT_BIND isteği 50 B yük → 81 B datagram (S4 H10: 66 ≤ 81).
        let f = encode_client_bind(7, &[1; 32], &[2; 16]);
        assert_eq!(f.len() + HUB_TRAILER_LEN, 81);
        // BIND_PROOF 128 B yük → 159 B datagram (S4 H10).
        let f = encode_bind_proof(7, &[1; 32], &[2; 16], &[3; 16], &[4; 64]);
        assert_eq!(f.len() + HUB_TRAILER_LEN, 159);
        assert_eq!(encode_heartbeat(9).len(), 9);
    }

    #[test]
    fn bind_proof_tbs_field_order_is_s4() {
        let tbs = bind_proof_tbs(&[0xAA; 32], &[0xBB; 32], &[0xCC; 16], &[0xDD; 16]);
        assert_eq!(&tbs[..16], b"KAMA-HUB-BIND-V1");
        assert_eq!(tbs[16..48], [0xAA; 32]); // hub_ed25519_pub
        assert_eq!(tbs[48..80], [0xBB; 32]); // device_cert_ref
        assert_eq!(tbs[80..96], [0xCC; 16]); // client_nonce
        assert_eq!(tbs[96..112], [0xDD; 16]); // hub_nonce
        let ack = bind_ack_tbs(&[0xBB; 32], &[0xCC; 16], &[0xDD; 16], 1, 0);
        assert_eq!(&ack[..16], b"KAMA-HUB-ACK-V1\0");
        assert_eq!(ack[16..48], [0xBB; 32]);
        assert_eq!(ack[48..64], [0xCC; 16]);
        assert_eq!(ack[64..80], [0xDD; 16]);
        assert_eq!(ack[80..], [1, 0, 0]);
    }

    #[test]
    fn store_layout_and_bound() {
        let f = encode_store(0x0102_0304, 0, envelope::SESSION_INIT, &[9; 32], &[7; 3]).unwrap();
        assert_eq!(f[..5], [0, 0, 0, 0, frame::PLANE1_RPC]);
        assert_eq!(f[5], method::STORE_MESSAGE);
        assert_eq!(f[6], flags::REQUEST);
        assert_eq!(f[7..11], [1, 2, 3, 4]);
        assert_eq!(f[11..15], 44u32.to_be_bytes());
        assert_eq!(f[15..19], [0; 4]);
        assert_eq!(f[19], envelope::SESSION_INIT);
        assert_eq!(f[20..52], [9; 32]);
        assert_eq!(f[52..56], 3u32.to_be_bytes());
        assert_eq!(f[56..], [7; 3]);
        let max = encode_store(1, 0, 1, &[0; 32], &[0; MAX_ENVELOPE]).unwrap();
        assert_eq!(max.len() + HUB_TRAILER_LEN, MAX_DATAGRAM);
        assert_eq!(
            encode_store(1, 0, 1, &[0; 32], &[0; MAX_ENVELOPE + 1]),
            Err(CodecError::TooLong)
        );
    }

    fn rpc(method_id: u8, fl: u8, rid: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 0, frame::PLANE1_RPC, method_id, fl];
        v.extend_from_slice(&rid.to_be_bytes());
        v.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn decoders_are_strict() {
        // Meydan okuma.
        let mut p = vec![0u8; 17];
        p.extend_from_slice(&code::CHALLENGE.to_be_bytes());
        p.extend_from_slice(&[5; 16]);
        assert_eq!(decode_challenge(&p), Ok([5; 16]));
        let mut longer = p.clone();
        longer.push(0);
        assert_eq!(decode_challenge(&longer), Err(CodecError::TrailingBytes));
        p[0] = 1;
        assert_eq!(decode_challenge(&p), Err(CodecError::Field));
        // Hata yükü.
        assert_eq!(decode_error(&[0x01, 0x06, 0, 0]), Ok(code::BAD_COOKIE));
        assert_eq!(decode_error(&[0x01, 0x06, 0, 1]), Err(CodecError::Field));
        // STORE yanıtı.
        assert_eq!(decode_store_response(&[0, 0, 0, 0, 0, 9]), Ok((0, 9)));
        assert_eq!(
            decode_store_response(&[0, 0, 0, 0, 9]),
            Err(CodecError::Truncated)
        );
        // ROUTE_MISS.
        assert_eq!(decode_route_miss(&[0, 0, 0, 5, 1]), Ok((5, 1)));
        // DELIVER uzunluk alanı tam tutmalı.
        let mut d = 7u32.to_be_bytes().to_vec();
        d.extend_from_slice(&[3; 32]);
        d.push(envelope::SESSION_INIT);
        d.extend_from_slice(&2u32.to_be_bytes());
        d.extend_from_slice(&[8, 8]);
        let got = decode_deliver(&d).unwrap();
        assert_eq!(
            (got.hub_handle, got.envelope_type, got.envelope),
            (7, 1, &[8u8, 8][..])
        );
        d.push(0);
        assert_eq!(decode_deliver(&d), Err(CodecError::Length));
    }

    #[test]
    fn inbound_classification() {
        let f = rpc(method::DELIVER, flags::REQUEST, 0, &[1, 2]);
        match decode_inbound(&f).unwrap() {
            Inbound::Rpc(r) => {
                assert_eq!(
                    (r.method_id, r.flags, r.request_id),
                    (method::DELIVER, 0, 0)
                );
                assert_eq!(r.payload, &[1, 2]);
            }
            other => panic!("{other:?}"),
        }
        let mut bad = f.clone();
        bad.pop();
        assert_eq!(decode_inbound(&bad), Err(CodecError::Length));
        assert_eq!(
            decode_inbound(&[0, 0, 0, 0, frame::HEARTBEAT_ACK, 0, 0, 0, 9]),
            Ok(Inbound::HeartbeatAck { token: Some(9) })
        );
        assert_eq!(
            decode_inbound(&[0, 0, 0, 0, frame::HEARTBEAT_ACK]),
            Ok(Inbound::HeartbeatAck { token: None })
        );
        assert_eq!(
            decode_inbound(&[0, 0, 0, 0, frame::HEARTBEAT_ACK, 1]),
            Err(CodecError::Length)
        );
        assert_eq!(
            decode_inbound(&[0, 0, 0, 0, 0x42]),
            Ok(Inbound::Unknown { frame_type: 0x42 })
        );
        assert_eq!(
            decode_inbound(&[0, 0, 0, 1, 0x1C]),
            Err(CodecError::NotControl)
        );
        assert_eq!(decode_inbound(&[0, 0]), Err(CodecError::Truncated));
        assert_eq!(
            decode_inbound(&vec![0u8; MAX_OPENED_FRAME + 1]),
            Err(CodecError::TooLong)
        );
    }
}
