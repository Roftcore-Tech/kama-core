//! Tel biçimi — `KM-SEAM-TH-001` (S4 v1.3) §2'nin codec'i ve bayt birleştiricileri.
//!
//! **Geçici yer.** S4 §7'ye göre bu modülün içeriği `kama-contracts::s4`'ün işidir (A2.2): tipler,
//! fixed-width codec, TBS / cookie / tahsis bayt birleştiricileri — kripto yok, bağımlılık yok. O modül
//! `main`'e girene dek burada durur; bu dosya `std` dışında hiçbir şey kullanmaz ve olduğu gibi
//! taşınabilir. `hub-server` kalanı yalnız bu modülün genel API'sini çağırır.
//!
//! İki datagram sınıfı (S4 §2.1, hub yalnız ilk dört bayta bakar):
//! ```text
//! kontrol : [00 00 00 00][frame_type u8][gövde][16 × 00]      en az 21 B
//! veri    : [hub_handle u32 ≠ 0][opak yük]                    en az 5 B
//! ```
//! Çözme KATIDIR: sabit alanlı yükte fazla bayt, uzunluk alanının kalan baytla tutmaması, PoC'de sıfır
//! olması gereken alanın sıfır olmaması hatadır. Veri datagramının ilk dört baytı dışındaki hiçbir
//! bayta bu modülde bakılmaz.

use core::fmt;
use core::num::NonZeroU32;
use std::net::{IpAddr, SocketAddr};

/// `MAX_DATAGRAM` (S4 §6).
pub const MAX_DATAGRAM: usize = 1280;
/// Kimlik numarası boyu.
pub const DEST_HASH_LEN: usize = 32;
/// `device_cert_ref` = SHA-256(cert) boyu.
pub const CERT_REF_LEN: usize = 32;
/// `client_nonce` / `hub_nonce` boyu.
pub const NONCE_LEN: usize = 16;
/// Ed25519 imza / açık anahtar boyu.
pub const SIGNATURE_LEN: usize = 64;
pub const PUBLIC_KEY_LEN: usize = 32;
/// HEARTBEAT `echo_token` boyu (gövde boş da olabilir).
pub const ECHO_TOKEN_LEN: usize = 4;

pub type DestHash = [u8; DEST_HASH_LEN];
pub type CertRef = [u8; CERT_REF_LEN];
pub type Nonce = [u8; NONCE_LEN];
pub type Signature = [u8; SIGNATURE_LEN];

/// Kontrol datagramının ilk dört baytı.
pub const CONTROL_PREFIX: [u8; 4] = [0; 4];
/// PoC no-op `seal_hub`: çerçevenin sonuna 16 sıfır bayt.
pub const HUB_TRAILER: [u8; 16] = [0; 16];
/// En kısa kontrol datagramı: önek + `frame_type` + boş gövde + kuyruk.
pub const CONTROL_MIN_LEN: usize = CONTROL_PREFIX.len() + 1 + HUB_TRAILER.len();
/// En kısa veri datagramı: handle + en az bir bayt yük.
pub const DATA_MIN_LEN: usize = 5;
/// `AppRpcFrame` başlığı: `method_id u8 · flags u8 · request_id u32 · payload_len u32`.
pub const RPC_HEADER_LEN: usize = 10;
/// Bir RPC datagramının yük dışındaki baytları (31; `payload_len ≤ MAX_DATAGRAM − 31`).
pub const RPC_OVERHEAD: usize = CONTROL_MIN_LEN + RPC_HEADER_LEN;

/// HubChannel `frame_type` (engine-spec §1.1; `0x1D` S4 [YAMA]).
pub mod frame {
    pub const HEARTBEAT: u8 = 0x1B;
    pub const HEARTBEAT_ACK: u8 = 0x1C;
    pub const PLANE1_RPC: u8 = 0x1D;
}

/// `AppRpcFrame.method_id`. Bit 7: alışverişi hub başlatır (yanıtı yoktur).
pub mod method {
    pub const CLIENT_BIND: u8 = 0x01;
    pub const STORE_MESSAGE: u8 = 0x05;
    pub const BIND_PROOF: u8 = 0x0B;
    pub const DELIVER: u8 = 0x81;
    pub const ROUTE_MISS: u8 = 0x84;
    /// Bit 7: hub başlatır.
    pub const HUB_INITIATED: u8 = 0x80;
}

/// `AppRpcFrame.flags`.
pub mod flags {
    pub const REQUEST: u8 = 0x00;
    pub const RESPONSE: u8 = 0x01;
    pub const ERROR: u8 = 0x02;
    pub const RESPONSE_ERROR: u8 = RESPONSE | ERROR;
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

/// ROUTE_MISS `reason`.
pub const ROUTE_MISS_UNKNOWN_HANDLE: u8 = 0x01;
/// `RELAY_OPEN_MAX_ENVELOPE` (S4 §6).
pub const RELAY_OPEN_MAX_ENVELOPE: usize = 64;

/// Çözme / kodlama hatası; içerik taşımaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// Beklenen alan için bayt yok.
    Truncated,
    /// Sabit alanlı yükte fazla bayt.
    TrailingBytes,
    /// Kontrol öneki ya da sıfır kuyruğu yok.
    NotControl,
    /// Uzunluk alanı kalan baytla tutmuyor ya da sınırı aşıyor.
    Length,
    /// PoC'de sabit olan bir alanın değeri farklı.
    Field,
    /// Yük kodlanırken datagram sınırını aştı.
    TooLong,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Truncated => "truncated",
            Self::TrailingBytes => "trailing bytes",
            Self::NotControl => "not a hub control datagram",
            Self::Length => "length field mismatch",
            Self::Field => "fixed field has an unexpected value",
            Self::TooLong => "datagram too long",
        })
    }
}

impl std::error::Error for WireError {}

// ── Datagram sınıfı ────────────────────────────────────────────────────────────────────────────

/// Datagramın sınıfı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Datagram<'a> {
    /// HubChannel kontrol çerçevesi; `body` kuyruktan önceki gövdedir.
    Control { frame_type: u8, body: &'a [u8] },
    /// Veri datagramı; hub yalnız handle'ı okur.
    Data { handle: NonZeroU32 },
}

/// İlk dört bayta göre sınıflar. Kontrol için kuyruğu doğrular; veri için yüke BAKMAZ.
///
/// # Errors
/// [`WireError::Truncated`] — dört bayttan kısa; [`WireError::NotControl`] — kontrol öneki var ama
/// datagram 21 bayttan kısa ya da son 16 bayt sıfır değil.
pub fn classify(datagram: &[u8]) -> Result<Datagram<'_>, WireError> {
    let (head, rest) = datagram
        .split_first_chunk::<4>()
        .ok_or(WireError::Truncated)?;
    if let Some(handle) = NonZeroU32::new(u32::from_be_bytes(*head)) {
        return Ok(Datagram::Data { handle });
    }
    let (inner, trailer) = rest.split_last_chunk::<16>().ok_or(WireError::NotControl)?;
    if *trailer != HUB_TRAILER {
        return Err(WireError::NotControl);
    }
    let (frame_type, body) = inner.split_first().ok_or(WireError::NotControl)?;
    Ok(Datagram::Control {
        frame_type: *frame_type,
        body,
    })
}

/// HubChannel kontrol datagramı yazar: `[0;4] ‖ frame_type ‖ gövde ‖ [0;16]`. `out` önce boşaltılır.
pub fn control_datagram(out: &mut Vec<u8>, frame_type: u8, body: &[u8]) {
    out.clear();
    out.extend_from_slice(&CONTROL_PREFIX);
    out.push(frame_type);
    out.extend_from_slice(body);
    out.extend_from_slice(&HUB_TRAILER);
}

// ── AppRpcFrame ─────────────────────────────────────────────────────────────────────────────────

/// `AppRpcFrame` (app-rpc §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcFrame<'a> {
    pub method_id: u8,
    pub flags: u8,
    pub request_id: u32,
    pub payload: &'a [u8],
}

impl<'a> RpcFrame<'a> {
    /// `0x1D PLANE1_RPC` gövdesinden çözer. `payload_len` kalan baytla tam tutmalı ve
    /// `MAX_DATAGRAM − 31`'i aşmamalı.
    ///
    /// # Errors
    /// [`WireError::Truncated`] · [`WireError::Length`].
    pub fn decode(body: &'a [u8]) -> Result<Self, WireError> {
        let mut r = Reader(body);
        let method_id = r.u8()?;
        let flags = r.u8()?;
        let request_id = r.u32()?;
        let declared = usize::try_from(r.u32()?).map_err(|_| WireError::Length)?;
        if declared > MAX_DATAGRAM.saturating_sub(RPC_OVERHEAD) || declared != r.0.len() {
            return Err(WireError::Length);
        }
        Ok(Self {
            method_id,
            flags,
            request_id,
            payload: r.0,
        })
    }

    /// Tam kontrol datagramını (`0x1D` + çerçeve + kuyruk) `out`'a yazar.
    ///
    /// # Errors
    /// [`WireError::TooLong`] — datagram `MAX_DATAGRAM`'ı aşar; `out` boş kalır.
    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), WireError> {
        out.clear();
        let len = u32::try_from(self.payload.len()).map_err(|_| WireError::TooLong)?;
        if self.payload.len() > MAX_DATAGRAM.saturating_sub(RPC_OVERHEAD) {
            return Err(WireError::TooLong);
        }
        out.extend_from_slice(&CONTROL_PREFIX);
        out.push(frame::PLANE1_RPC);
        out.push(self.method_id);
        out.push(self.flags);
        out.extend_from_slice(&self.request_id.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(self.payload);
        out.extend_from_slice(&HUB_TRAILER);
        Ok(())
    }
}

// ── Yükler (S4 §2.4) ────────────────────────────────────────────────────────────────────────────

/// Bir yükün sabit yerleşimli kodlaması.
pub trait Payload<'a>: Sized {
    /// Yükü çözer (fazla bayt hata).
    ///
    /// # Errors
    /// [`WireError`].
    fn decode(payload: &'a [u8]) -> Result<Self, WireError>;
    /// Yükü `out`'un sonuna yazar.
    fn write(&self, out: &mut Vec<u8>);
    /// Yükü yeni bir vektöre yazar.
    fn to_vec(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write(&mut out);
        out
    }
}

/// CLIENT_BIND istek: `device_cert_mode u8 = 0 · device_cert_ref · presence_flags u8 = 0 · client_nonce`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientBindRequest {
    pub cert_ref: CertRef,
    pub client_nonce: Nonce,
}

impl Payload<'_> for ClientBindRequest {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        r.zero_u8()?;
        let cert_ref = r.array()?;
        r.zero_u8()?;
        let client_nonce = r.array()?;
        r.end()?;
        Ok(Self {
            cert_ref,
            client_nonce,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(0);
        out.extend_from_slice(&self.cert_ref);
        out.push(0);
        out.extend_from_slice(&self.client_nonce);
    }
}

/// CLIENT_BIND yanıt (meydan okuma): `bind_ok = 0 · mailbox_cursor u64 = 0 · revocation_seq u64 = 0 ·
/// error_code = CHALLENGE · hub_nonce`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientBindResponse {
    pub hub_nonce: Nonce,
}

impl Payload<'_> for ClientBindResponse {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        r.expect_u8(0)?;
        r.expect_u64(0)?;
        r.expect_u64(0)?;
        r.expect_u16(code::CHALLENGE)?;
        let hub_nonce = r.array()?;
        r.end()?;
        Ok(Self { hub_nonce })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(0);
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&code::CHALLENGE.to_be_bytes());
        out.extend_from_slice(&self.hub_nonce);
    }
}

/// BIND_PROOF istek: `device_cert_ref · client_nonce · hub_nonce · client_signature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindProofRequest {
    pub cert_ref: CertRef,
    pub client_nonce: Nonce,
    pub hub_nonce: Nonce,
    pub signature: Signature,
}

impl Payload<'_> for BindProofRequest {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        let request = Self {
            cert_ref: r.array()?,
            client_nonce: r.array()?,
            hub_nonce: r.array()?,
            signature: r.array()?,
        };
        r.end()?;
        Ok(request)
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.cert_ref);
        out.extend_from_slice(&self.client_nonce);
        out.extend_from_slice(&self.hub_nonce);
        out.extend_from_slice(&self.signature);
    }
}

/// BIND_PROOF yanıt (bağlandı): `bind_ok = 1 · 0 · 0 · error_code = OK · hub_signature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindProofResponse {
    pub hub_signature: Signature,
}

/// Başarılı BIND_PROOF yanıtındaki `bind_ok` değeri.
pub const BIND_OK: u8 = 1;

impl Payload<'_> for BindProofResponse {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        r.expect_u8(BIND_OK)?;
        r.expect_u64(0)?;
        r.expect_u64(0)?;
        r.expect_u16(code::OK)?;
        let hub_signature = r.array()?;
        r.end()?;
        Ok(Self { hub_signature })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(BIND_OK);
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&code::OK.to_be_bytes());
        out.extend_from_slice(&self.hub_signature);
    }
}

/// `ErrorPayload { error_code u16; detail_len u16 = 0 }` (app-rpc §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorPayload {
    pub code: u16,
}

impl Payload<'_> for ErrorPayload {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        let code = r.u16()?;
        r.expect_u16(0)?;
        r.end()?;
        Ok(Self { code })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.code.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
    }
}

/// STORE_MESSAGE istek: `hub_handle u32 · envelope_type u8 · recipient_dest_hash · envelope_len u32 ·
/// envelope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreRequest<'a> {
    pub hub_handle: u32,
    pub envelope_type: u8,
    pub recipient: DestHash,
    pub envelope: &'a [u8],
}

impl<'a> Payload<'a> for StoreRequest<'a> {
    fn decode(payload: &'a [u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        Ok(Self {
            hub_handle: r.u32()?,
            envelope_type: r.u8()?,
            recipient: r.array()?,
            envelope: r.tail_u32()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.push(self.envelope_type);
        out.extend_from_slice(&self.recipient);
        write_tail_u32(out, self.envelope);
    }
}

/// STORE_MESSAGE yanıt: `status u16 · assigned_handle u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreResponse {
    pub status: u16,
    pub assigned_handle: u32,
}

impl Payload<'_> for StoreResponse {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        let response = Self {
            status: r.u16()?,
            assigned_handle: r.u32()?,
        };
        r.end()?;
        Ok(response)
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.status.to_be_bytes());
        out.extend_from_slice(&self.assigned_handle.to_be_bytes());
    }
}

/// DELIVER: `hub_handle u32 · sender_dest_hash · envelope_type u8 · envelope_len u32 · envelope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deliver<'a> {
    pub hub_handle: u32,
    pub sender: DestHash,
    pub envelope_type: u8,
    pub envelope: &'a [u8],
}

impl<'a> Payload<'a> for Deliver<'a> {
    fn decode(payload: &'a [u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        Ok(Self {
            hub_handle: r.u32()?,
            sender: r.array()?,
            envelope_type: r.u8()?,
            envelope: r.tail_u32()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.extend_from_slice(&self.sender);
        out.push(self.envelope_type);
        write_tail_u32(out, self.envelope);
    }
}

/// ROUTE_MISS: `hub_handle u32 · reason u8`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteMiss {
    pub hub_handle: u32,
    pub reason: u8,
}

impl Payload<'_> for RouteMiss {
    fn decode(payload: &[u8]) -> Result<Self, WireError> {
        let mut r = Reader(payload);
        let miss = Self {
            hub_handle: r.u32()?,
            reason: r.u8()?,
        };
        r.end()?;
        Ok(miss)
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.push(self.reason);
    }
}

/// STORE_MESSAGE ile DELIVER'ın zarftan önceki baytları (ikisi aynı: 41).
pub const ENVELOPE_PAYLOAD_OVERHEAD: usize = 4 + 1 + DEST_HASH_LEN + 4;
/// Bir datagrama sığan en büyük zarf (1280 − 31 − 41 = 1208).
pub const MAX_ENVELOPE: usize = MAX_DATAGRAM - RPC_OVERHEAD - ENVELOPE_PAYLOAD_OVERHEAD;

// ── Bayt birleştiricileri (S4 §2.4, §2.6; kripto YOK) ──────────────────────────────────────────

/// `domain_sep` (ccr §13.2): ASCII ≤ 16 B + `0x00` dolgu.
pub const BIND_DOMAIN: [u8; 16] = *b"KAMA-HUB-BIND-V1";
pub const ACK_DOMAIN: [u8; 16] = *b"KAMA-HUB-ACK-V1\0";
/// Telefonun imzaladığı metin boyu (S4 §2.4(b)).
pub const BIND_PROOF_TBS_LEN: usize = 16 + PUBLIC_KEY_LEN + CERT_REF_LEN + NONCE_LEN + NONCE_LEN;
/// Hub'ın imzaladığı metin boyu.
pub const BIND_ACK_TBS_LEN: usize = 16 + CERT_REF_LEN + NONCE_LEN + NONCE_LEN + 1 + 2;

/// `domain_sep("KAMA-HUB-BIND-V1") ‖ hub_ed25519_pub ‖ device_cert_ref ‖ client_nonce ‖ hub_nonce` (112 B).
#[must_use]
pub fn bind_proof_tbs(
    hub_pub: &[u8; PUBLIC_KEY_LEN],
    cert_ref: &CertRef,
    client_nonce: &Nonce,
    hub_nonce: &Nonce,
) -> [u8; BIND_PROOF_TBS_LEN] {
    concat(&[&BIND_DOMAIN, hub_pub, cert_ref, client_nonce, hub_nonce])
}

/// `domain_sep("KAMA-HUB-ACK-V1") ‖ device_cert_ref ‖ client_nonce ‖ hub_nonce ‖ bind_ok ‖ error_code` (83 B).
#[must_use]
pub fn bind_ack_tbs(
    cert_ref: &CertRef,
    client_nonce: &Nonce,
    hub_nonce: &Nonce,
    bind_ok: u8,
    error_code: u16,
) -> [u8; BIND_ACK_TBS_LEN] {
    concat(&[
        &ACK_DOMAIN,
        cert_ref,
        client_nonce,
        hub_nonce,
        &[bind_ok],
        &error_code.to_be_bytes(),
    ])
}

/// Cookie girdisinin adres ailesi baytı (S4 v1.3.1 §2.4(a)).
pub const FAMILY_IPV4: u8 = 4;
pub const FAMILY_IPV6: u8 = 6;
/// Cookie sırrı boyu.
pub const SECRET_LEN: usize = 32;
/// Cookie girdisinin en uzun hâli (IPv6).
pub const COOKIE_INPUT_MAX: usize = SECRET_LEN + 1 + 16 + 2 + CERT_REF_LEN + NONCE_LEN + 4 + 4;

/// Cookie girdisi (S4 v1.3.1 §2.4(a); IPv4-eşlemeli IPv6 IPv4'e normalize): `cookie_secret ‖ fam u8 ‖ src_ip (4|16) ‖ src_port u16 ‖
/// device_cert_ref ‖ client_nonce ‖ minute u32 ‖ bind_epoch u32`. `hub_nonce` bunun BLAKE2s-256'sının
/// ilk 16 baytıdır. Dönüş: tampon + kullanılan uzunluk.
#[must_use]
pub fn cookie_input(
    secret: &[u8; SECRET_LEN],
    src: SocketAddr,
    cert_ref: &CertRef,
    client_nonce: &Nonce,
    minute: u32,
    bind_epoch: u32,
) -> ([u8; COOKIE_INPUT_MAX], usize) {
    let v4;
    let v6;
    // S4 v1.3.1 §2.4(a): `::ffff:a.b.c.d` özetlenmeden önce IPv4'e normalize edilir.
    let normalized = match src.ip() {
        IpAddr::V6(addr) => addr.to_ipv4_mapped().map_or(IpAddr::V6(addr), IpAddr::V4),
        v4 => v4,
    };
    let (family, ip): (u8, &[u8]) = match normalized {
        IpAddr::V4(addr) => {
            v4 = addr.octets();
            (FAMILY_IPV4, &v4)
        }
        IpAddr::V6(addr) => {
            v6 = addr.octets();
            (FAMILY_IPV6, &v6)
        }
    };
    let parts: [&[u8]; 8] = [
        secret,
        &[family],
        ip,
        &src.port().to_be_bytes(),
        cert_ref,
        client_nonce,
        &minute.to_be_bytes(),
        &bind_epoch.to_be_bytes(),
    ];
    let len = parts.iter().map(|part| part.len()).sum();
    (concat(&parts), len)
}

/// Handle adayı girdisi (S4 §2.6): `handle_secret ‖ alloc_counter u64`; aday = BLAKE2s-256'nın ilk 4
/// baytı (big-endian `u32`).
#[must_use]
pub fn handle_candidate_input(secret: &[u8; SECRET_LEN], counter: u64) -> [u8; SECRET_LEN + 8] {
    concat(&[secret, &counter.to_be_bytes()])
}

/// Tahsis anahtarındaki zarf özeti boyu (`BLAKE2s(envelope)[0..8]`).
pub const ENVELOPE_DIGEST_LEN: usize = 8;

/// `allocation_key` (S4 §2.4(c); KR-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationKey {
    pub sender: DestHash,
    pub request_id: u32,
    pub recipient: DestHash,
    pub envelope_type: u8,
    pub envelope_digest: [u8; ENVELOPE_DIGEST_LEN],
}

/// Parçaları `N` baytlık diziye dizer; toplam `N`'den kısaysa kalan sıfır, uzunsa fazlası atılır
/// (çağıranlar sabit boy verir; birim testleri kilitler). Panik yok.
fn concat<const N: usize>(parts: &[&[u8]]) -> [u8; N] {
    let mut out = [0u8; N];
    for (slot, byte) in out
        .iter_mut()
        .zip(parts.iter().flat_map(|part| part.iter()))
    {
        *slot = *byte;
    }
    out
}

// ── Okuyucu ─────────────────────────────────────────────────────────────────────────────────────

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let (head, rest) = self
            .0
            .split_first_chunk::<N>()
            .ok_or(WireError::Truncated)?;
        self.0 = rest;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(u8::from_be_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn zero_u8(&mut self) -> Result<(), WireError> {
        self.expect_u8(0)
    }

    fn expect_u8(&mut self, value: u8) -> Result<(), WireError> {
        (self.u8()? == value).then_some(()).ok_or(WireError::Field)
    }

    fn expect_u16(&mut self, value: u16) -> Result<(), WireError> {
        (self.u16()? == value).then_some(()).ok_or(WireError::Field)
    }

    fn expect_u64(&mut self, value: u64) -> Result<(), WireError> {
        (self.u64()? == value).then_some(()).ok_or(WireError::Field)
    }

    /// `len u32 · bytes[len]` — yükün SONUNA kadar; uzunluk kalanla tam tutmalı.
    fn tail_u32(&mut self) -> Result<&'a [u8], WireError> {
        let declared = usize::try_from(self.u32()?).map_err(|_| WireError::Length)?;
        if declared != self.0.len() {
            return Err(WireError::Length);
        }
        Ok(core::mem::take(&mut self.0))
    }

    fn end(self) -> Result<(), WireError> {
        self.0
            .is_empty()
            .then_some(())
            .ok_or(WireError::TrailingBytes)
    }
}

fn write_tail_u32(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn rpc(method_id: u8, flags: u8, request_id: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        RpcFrame {
            method_id,
            flags,
            request_id,
            payload,
        }
        .encode(&mut out)
        .expect("kodlanmalı");
        out
    }

    #[test]
    fn sizes_match_the_contract() {
        assert_eq!(CONTROL_MIN_LEN, 21);
        assert_eq!(RPC_OVERHEAD, 31);
        assert_eq!(MAX_ENVELOPE, 1208);
        assert_eq!(BIND_PROOF_TBS_LEN, 112);
        assert_eq!(BIND_ACK_TBS_LEN, 83);
        assert_eq!(
            ClientBindRequest {
                cert_ref: [0; 32],
                client_nonce: [0; 16]
            }
            .to_vec()
            .len(),
            50
        );
        assert_eq!(ClientBindResponse { hub_nonce: [0; 16] }.to_vec().len(), 35);
        assert_eq!(
            BindProofRequest {
                cert_ref: [0; 32],
                client_nonce: [0; 16],
                hub_nonce: [0; 16],
                signature: [0; 64]
            }
            .to_vec()
            .len(),
            128
        );
        assert_eq!(
            BindProofResponse {
                hub_signature: [0; 64]
            }
            .to_vec()
            .len(),
            83
        );
        assert_eq!(ErrorPayload { code: 1 }.to_vec().len(), 4);
        assert_eq!(
            StoreResponse {
                status: 0,
                assigned_handle: 1
            }
            .to_vec()
            .len(),
            6
        );
        assert_eq!(
            RouteMiss {
                hub_handle: 1,
                reason: 1
            }
            .to_vec()
            .len(),
            5
        );
    }

    #[test]
    fn classify_checks_prefix_trailer_and_reads_only_the_handle() {
        assert_eq!(classify(&[1, 2, 3]), Err(WireError::Truncated));
        assert_eq!(
            classify(&[0, 0, 0, 7, 0xff]),
            Ok(Datagram::Data {
                handle: NonZeroU32::new(7).expect("sıfır değil")
            })
        );
        let mut control = Vec::new();
        control_datagram(&mut control, frame::HEARTBEAT, &[9, 9, 9, 9]);
        assert_eq!(control.len(), 25);
        assert_eq!(
            classify(&control),
            Ok(Datagram::Control {
                frame_type: frame::HEARTBEAT,
                body: &[9, 9, 9, 9]
            })
        );
        let mut dirty = control.clone();
        if let Some(last) = dirty.last_mut() {
            *last = 1;
        }
        assert_eq!(classify(&dirty), Err(WireError::NotControl));
        assert_eq!(classify(&control[..20]), Err(WireError::NotControl));
        control_datagram(&mut control, frame::HEARTBEAT, &[]);
        assert_eq!(control.len(), CONTROL_MIN_LEN);
        assert!(classify(&control).is_ok());
    }

    #[test]
    fn rpc_frame_round_trips_and_rejects_bad_lengths() {
        let datagram = rpc(method::STORE_MESSAGE, flags::REQUEST, 0xdead_beef, b"yuk");
        let Ok(Datagram::Control { frame_type, body }) = classify(&datagram) else {
            panic!("kontrol bekleniyordu");
        };
        assert_eq!(frame_type, frame::PLANE1_RPC);
        let frame = RpcFrame::decode(body).expect("çözülmeli");
        assert_eq!(
            frame,
            RpcFrame {
                method_id: 0x05,
                flags: 0,
                request_id: 0xdead_beef,
                payload: b"yuk"
            }
        );
        let mut longer = body.to_vec();
        longer.push(0);
        assert_eq!(RpcFrame::decode(&longer), Err(WireError::Length));
        assert_eq!(
            RpcFrame::decode(&body[..body.len() - 1]),
            Err(WireError::Length)
        );
        let big = vec![0u8; MAX_DATAGRAM - RPC_OVERHEAD + 1];
        let mut out = vec![1];
        let frame = RpcFrame {
            method_id: 1,
            flags: 0,
            request_id: 0,
            payload: &big,
        };
        assert_eq!(frame.encode(&mut out), Err(WireError::TooLong));
        assert!(out.is_empty());
        assert!(RpcFrame {
            payload: &big[1..],
            ..frame
        }
        .encode(&mut out)
        .is_ok());
        assert_eq!(out.len(), MAX_DATAGRAM);
    }

    #[test]
    fn fixed_fields_are_checked() {
        let mut request = ClientBindRequest {
            cert_ref: [3; 32],
            client_nonce: [4; 16],
        }
        .to_vec();
        assert!(ClientBindRequest::decode(&request).is_ok());
        request[0] = 1;
        assert_eq!(ClientBindRequest::decode(&request), Err(WireError::Field));
        request[0] = 0;
        request[33] = 1;
        assert_eq!(ClientBindRequest::decode(&request), Err(WireError::Field));
        request[33] = 0;
        request.push(0);
        assert_eq!(
            ClientBindRequest::decode(&request),
            Err(WireError::TrailingBytes)
        );
        assert_eq!(ErrorPayload::decode(&[1, 6, 0, 1]), Err(WireError::Field));
    }

    #[test]
    fn envelope_length_must_match() {
        let store = StoreRequest {
            hub_handle: 0,
            envelope_type: 1,
            recipient: [7; 32],
            envelope: b"zarf",
        };
        let bytes = store.to_vec();
        assert_eq!(StoreRequest::decode(&bytes), Ok(store));
        assert_eq!(
            StoreRequest::decode(&bytes[..bytes.len() - 1]),
            Err(WireError::Length)
        );
        let mut longer = bytes;
        longer.push(0);
        assert_eq!(StoreRequest::decode(&longer), Err(WireError::Length));
    }

    #[test]
    fn signed_texts_and_cookie_input_have_the_documented_layout() {
        let proof = bind_proof_tbs(&[0xaa; 32], &[0xbb; 32], &[0x11; 16], &[0x22; 16]);
        let mut expected = b"KAMA-HUB-BIND-V1".to_vec();
        for (byte, n) in [(0xaa, 32), (0xbb, 32), (0x11, 16), (0x22, 16)] {
            expected.extend(std::iter::repeat_n(byte, n));
        }
        assert_eq!(proof.as_slice(), expected.as_slice());

        let ack = bind_ack_tbs(&[0xbb; 32], &[0x11; 16], &[0x22; 16], 1, 0);
        let mut expected = b"KAMA-HUB-ACK-V1\0".to_vec();
        for (byte, n) in [(0xbb, 32), (0x11, 16), (0x22, 16)] {
            expected.extend(std::iter::repeat_n(byte, n));
        }
        expected.extend_from_slice(&[1, 0, 0]);
        assert_eq!(ack.as_slice(), expected.as_slice());

        let v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7)), 0x9c41);
        let (input, len) = cookie_input(&[0x55; 32], v4, &[0xbb; 32], &[0x11; 16], 3, 7);
        let mut expected = vec![0x55; 32];
        expected.push(FAMILY_IPV4);
        expected.extend_from_slice(&[198, 51, 100, 7, 0x9c, 0x41]);
        expected.extend_from_slice(&[0xbb; 32]);
        expected.extend_from_slice(&[0x11; 16]);
        expected.extend_from_slice(&[0, 0, 0, 3, 0, 0, 0, 7]);
        assert_eq!(&input[..len], expected.as_slice());

        let v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 1);
        let (_, len6) = cookie_input(&[0x55; 32], v6, &[0xbb; 32], &[0x11; 16], 3, 7);
        assert_eq!(len6, COOKIE_INPUT_MAX);
        assert_eq!(len6, len + 12);

        let candidate = handle_candidate_input(&[9; 32], 0x0102);
        assert_eq!(&candidate[32..], &[0, 0, 0, 0, 0, 0, 1, 2]);
    }
}
