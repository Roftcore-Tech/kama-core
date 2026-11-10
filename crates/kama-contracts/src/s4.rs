//! S4 — Transport ↔ Hub sözleşmesinin tel biçimi (`docs/seams/KM-SEAM-TH-001.md` v1.3.1 §2).
//!
//! Bu modül S4 §7'nin ilk maddesidir: §2 tipleri, sabit-genişlikli big-endian codec ve TBS / cookie /
//! tahsis-anahtarı **bayt birleştiricileri**. Kripto yoktur, özet yoktur, bağımlılık yoktur (C1):
//! imzalar, BLAKE2s ve SHA-256 çağıranın işidir. Baytların tek otoritesi S4'tür; bu dosya S4'ten
//! yazılmıştır ve `vectors/hub/` altın vektörlerine (Python, `ref/hub/gen_vectors.py`) karşı sınanır
//! (`crates/kama-integration/tests/s4_vectors.rs`).
//!
//! İki datagram sınıfı (S4 §2.1; hub yalnız ilk dört bayta bakar):
//! ```text
//! kontrol : [00 00 00 00][frame_type u8][gövde][16 × 00]      en az 21 B
//! veri    : [hub_handle u32 ≠ 0][opak yük]                    en az 5 B
//! ```
//! Sondaki 16 sıfır bayt PoC'nin no-op `seal_hub`'ıdır (S3 `ProtectionContext::HubChannel`); onu
//! **seam ekler ve çıkarır**, bu modül değil. [`hub_frame_encode`] / [`hub_frame_decode`] kuyruksuz
//! ("açılmış") çerçeveyle çalışır.
//!
//! Çözme **fail-closed**'dur: kısa gövde, fazla bayt, kalan baytla tutmayan uzunluk alanı, PoC'de
//! sabit olan alanın farklı değeri, bilinmeyen tip/method/kod → [`S4Error`]. Bellek ayırma yalnız
//! denetlenmiş uzunlukla yapılır (SR-AUDIT-001: bounds-before-alloc).

use core::fmt;
use std::net::{IpAddr, SocketAddr};

// ── Sabitler (S4 §2.1–§2.6, §6) ─────────────────────────────────────────────────────────────────

/// HubChannel kontrol datagramının ilk dört baytı = tutamaç 0 (S4 §2.1). Hub bu değeri hiçbir
/// hatta vermez.
pub const HUB_CHANNEL_HANDLE: u32 = 0;
/// `frame_type` HEARTBEAT (S4 §2.2).
pub const FRAME_HEARTBEAT: u8 = 0x1B;
/// `frame_type` HEARTBEAT_ACK (S4 §2.2).
pub const FRAME_HEARTBEAT_ACK: u8 = 0x1C;
/// `frame_type` PLANE1_RPC (S4 §2.2; [YAMA engine §1.1]).
pub const FRAME_PLANE1_RPC: u8 = 0x1D;

/// `method_id` CLIENT_BIND (S4 §2.3, §2.4(a)).
pub const METHOD_CLIENT_BIND: u8 = 0x01;
/// `method_id` STORE_MESSAGE (S4 §2.3, §2.4(c)).
pub const METHOD_STORE_MESSAGE: u8 = 0x05;
/// `method_id` BIND_PROOF (S4 §2.3, §2.4(b)).
pub const METHOD_BIND_PROOF: u8 = 0x0B;
/// `method_id` DELIVER (S4 §2.3, §2.4(d)); bit 7 = hub başlatır.
pub const METHOD_DELIVER: u8 = 0x81;
/// `method_id` ROUTE_MISS (S4 §2.3, §2.4(e)); bit 7 = hub başlatır.
pub const METHOD_ROUTE_MISS: u8 = 0x84;
/// `method_id` bit 7: alışverişi BAŞLATAN taraf hub'dır (yanıtı yoktur) (S4 §2.3).
pub const METHOD_HUB_INITIATED: u8 = 0x80;

/// `AppRpcFrame.flags` bit 0: yanıt (S4 §2.3).
pub const FLAG_RESPONSE: u8 = 0x01;
/// `AppRpcFrame.flags` bit 1: hata (S4 §2.3). Yalnız [`FLAG_RESPONSE`] ile birlikte geçerlidir.
pub const FLAG_ERROR: u8 = 0x02;

/// `envelope_type` SESSION_INIT (S4 §2.4(c)).
pub const ENVELOPE_SESSION_INIT: u8 = 0x01;
/// `envelope_type` SESSION_INIT_RESPONSE (S4 §2.4(c)).
pub const ENVELOPE_SESSION_INIT_RESPONSE: u8 = 0x02;
/// `envelope_type` RELAY_OPEN (S4 §2.4(c), §2.4(f); [YAMA registry]).
pub const ENVELOPE_RELAY_OPEN: u8 = 0x05;

/// Durum kodu OK (S4 §2.5; yanıt alanı).
pub const STATUS_OK: u16 = 0x0000;
/// Durum kodu CHALLENGE (S4 §2.5; CLIENT_BIND yanıtının `error_code` alanı).
pub const STATUS_CHALLENGE: u16 = 0x0003;
/// Durum kodu PEER_UNBOUND (S4 §2.5; STORE yanıtının `status` alanı).
pub const STATUS_PEER_UNBOUND: u16 = 0x0301;
/// Durum kodu UNKNOWN_HANDLE (S4 §2.5; STORE yanıtının `status` alanı — iki anlamı
/// [`StoreMessageResp`] belgesinde).
pub const STATUS_UNKNOWN_HANDLE: u16 = 0x0302;
/// Hata kodu POSSESSION_PROOF_FAIL (S4 §2.5; [`ErrorPayload`]).
pub const ERROR_POSSESSION_PROOF_FAIL: u16 = 0x0105;
/// Hata kodu BAD_COOKIE (S4 §2.5; [`ErrorPayload`]).
pub const ERROR_BAD_COOKIE: u16 = 0x0106;
/// Hata kodu HUB_CAPACITY (S4 §2.5; [`ErrorPayload`]).
pub const ERROR_HUB_CAPACITY: u16 = 0x0501;
/// Hata kodu RATE_LIMITED (S4 §2.5; [`ErrorPayload`]).
pub const ERROR_RATE_LIMITED: u16 = 0x0502;
/// Hata kodu BAD_REQUEST (S4 §2.5; [`ErrorPayload`]).
pub const ERROR_BAD_REQUEST: u16 = 0x0601;
/// ROUTE_MISS `reason` UNKNOWN_HANDLE (S4 §2.4(e)).
pub const ROUTE_MISS_UNKNOWN_HANDLE: u8 = 0x01;

/// `MAX_DATAGRAM` (S4 §6).
pub const MAX_DATAGRAM: usize = 1280;
/// `RELAY_OPEN_MAX_ENVELOPE` (S4 §6, §2.4(c) kurallar).
pub const RELAY_OPEN_MAX_ENVELOPE: usize = 64;
/// Kontrol datagramının sıfır öneki (S4 §2.1).
pub const CONTROL_PREFIX_LEN: usize = 4;
/// PoC no-op `seal_hub` kuyruğu: 16 sıfır bayt (S4 §2.1). Seam ekler; bu modül eklemez.
pub const HUB_TRAILER_LEN: usize = 16;
/// En kısa kontrol datagramı: önek + `frame_type` + boş gövde + kuyruk = 21 B (S4 §2.1).
pub const CONTROL_MIN_LEN: usize = CONTROL_PREFIX_LEN + 1 + HUB_TRAILER_LEN;
/// En kısa veri datagramı = 5 B (S4 §2.1).
pub const DATA_MIN_LEN: usize = 5;
/// Kuyruksuz kontrol çerçevesinin en büyük boyu (`MAX_DATAGRAM − 16`).
pub const MAX_OPENED_FRAME: usize = MAX_DATAGRAM - HUB_TRAILER_LEN;
/// `AppRpcFrame` başlığı: `method_id u8 · flags u8 · request_id u32 · payload_len u32` (S4 §2.3).
pub const RPC_HEADER_LEN: usize = 10;
/// Bir RPC datagramının yük dışındaki baytları: 4 + 1 + 10 + 16 = 31 (S4 §2.3).
pub const RPC_OVERHEAD: usize = CONTROL_MIN_LEN + RPC_HEADER_LEN;
/// `payload_len ≤ MAX_DATAGRAM − 31` (S4 §2.3).
pub const MAX_RPC_PAYLOAD: usize = MAX_DATAGRAM - RPC_OVERHEAD;
/// STORE_MESSAGE isteğinin zarf dışı baytları: `hub_handle 4 · envelope_type 1 · recipient 32 ·
/// envelope_len 4` (S4 §2.4(c)).
pub const STORE_REQ_FIXED_LEN: usize = 4 + 1 + 32 + 4;
/// DELIVER yükünün zarf dışı baytları: `hub_handle 4 · sender 32 · envelope_type 1 ·
/// envelope_len 4` (S4 §2.4(d)).
pub const DELIVER_FIXED_LEN: usize = 4 + 32 + 1 + 4;
/// HEARTBEAT `echo_token` boyu (S4 §2.2).
pub const ECHO_TOKEN_LEN: usize = 4;

/// `dest_hash` boyu (32 B).
pub const DEST_HASH_LEN: usize = 32;
/// `device_cert_ref` = SHA-256(cert) boyu (32 B).
pub const CERT_REF_LEN: usize = 32;
/// `client_nonce` / `hub_nonce` / `open_nonce` boyu (16 B).
pub const NONCE_LEN: usize = 16;
/// Ed25519 imza boyu (64 B).
pub const SIGNATURE_LEN: usize = 64;
/// Ed25519 açık anahtar boyu (32 B).
pub const PUBLIC_KEY_LEN: usize = 32;
/// `client_signature` TBS boyu (S4 §2.4(b)): 16 + 32 + 32 + 16 + 16.
pub const BIND_TBS_LEN: usize = 112;
/// `hub_signature` TBS boyu (S4 §2.4(b)): 16 + 32 + 16 + 16 + 1 + 2.
pub const ACK_TBS_LEN: usize = 83;
/// [`AllocationKey`] kodlaması: 32 + 4 + 32 + 1 + 8 (S4 §2.4(c)).
pub const ALLOCATION_KEY_LEN: usize = 77;
/// `domain_sep` çıktısı (ccr §13.2 geleneği): 16 B.
pub const DOMAIN_SEP_LEN: usize = 16;

/// `client_signature` etiketi (S4 §2.4(b)).
pub const LABEL_HUB_BIND: &[u8] = b"KAMA-HUB-BIND-V1";
/// `hub_signature` etiketi (S4 §2.4(b)).
pub const LABEL_HUB_ACK: &[u8] = b"KAMA-HUB-ACK-V1";

/// Kimlik numarası (`dest_hash`).
pub type DestHash = [u8; DEST_HASH_LEN];
/// `device_cert_ref` = SHA-256(CompactDeviceCertV1).
pub type CertRef = [u8; CERT_REF_LEN];
/// 16 B nonce (`client_nonce`, `hub_nonce`, `open_nonce`).
pub type Nonce = [u8; NONCE_LEN];
/// Ed25519 imzası.
pub type Signature = [u8; SIGNATURE_LEN];
/// Ed25519 açık anahtarı.
pub type PublicKey = [u8; PUBLIC_KEY_LEN];

// ── Hata ────────────────────────────────────────────────────────────────────────────────────────

/// Çözme / kodlama / doğrulama hatası. İçerik taşımaz (girdi baytı loglanmaz, S4 H12).
///
/// [`S4Error::as_str`] kararlı bir addır: `vectors/hub/` negatiflerinin `expected.reason` alanı
/// bununla tam-dizge karşılaştırılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum S4Error {
    /// Beklenen alan için bayt yok.
    Truncated,
    /// Sabit boylu yükte fazla bayt.
    TrailingBytes,
    /// Uzunluk alanı (`payload_len`, `envelope_len`, `detail_len`) kalan baytla tutmuyor.
    LengthMismatch,
    /// S4 §2.3 / §6 sınırı aşıldı (`MAX_DATAGRAM`, `payload_len ≤ MAX_DATAGRAM − 31`).
    TooLong,
    /// Gövde izinli boylardan biri değil (HEARTBEAT: 0 ya da 4 B; etiket ≤ 16 B).
    BadLength,
    /// İlk dört bayt sıfır değil: kontrol çerçevesi değil (S4 §2.1).
    NotControl,
    /// Bilinmeyen `frame_type` (S4 §2.2: sessiz drop).
    UnknownFrameType,
    /// Bilinmeyen `method_id` (S4 §2.3).
    UnknownMethod,
    /// `flags` S4 §2.3 tablosunda olmayan bir değer.
    BadFlags,
    /// Hub başlatımlı mesajda `request_id ≠ 0` (S4 §2.3).
    BadRequestId,
    /// PoC'de sabit olan bir alanın değeri farklı (S4 §2.4).
    BadField,
    /// Kod yanlış sütunda ya da bilinmiyor (S4 §2.5).
    BadCode,
    /// Hub'ın hiç vermediği `hub_handle = 0` (S4 §2.1).
    ZeroHandle,
    /// Hub'ın teslim etmediği `envelope_type` (S4 §2.4(c)/(d)).
    UnknownEnvelopeType,
    /// STORE_MESSAGE çözüldü ama geçerli birleşim değil (S4 §2.4(c) → `BAD_REQUEST`).
    BadRequest,
}

impl S4Error {
    /// Kararlı ad (vektör `reason` sözlüğü; sayaç adı olarak da kullanılabilir).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Truncated => "truncated",
            Self::TrailingBytes => "trailing-bytes",
            Self::LengthMismatch => "length-mismatch",
            Self::TooLong => "too-long",
            Self::BadLength => "bad-length",
            Self::NotControl => "not-control",
            Self::UnknownFrameType => "unknown-frame-type",
            Self::UnknownMethod => "unknown-method",
            Self::BadFlags => "bad-flags",
            Self::BadRequestId => "bad-request-id",
            Self::BadField => "bad-field",
            Self::BadCode => "bad-code",
            Self::ZeroHandle => "zero-handle",
            Self::UnknownEnvelopeType => "unknown-envelope-type",
            Self::BadRequest => "bad-request",
        }
    }
}

impl fmt::Display for S4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for S4Error {}

// ── Okuyucu / yazıcı (sabit genişlik, big-endian; varint yok — ADR-05) ───────────────────────────

struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], S4Error> {
        if self.buf.len() < n {
            return Err(S4Error::Truncated);
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], S4Error> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, S4Error> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, S4Error> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, S4Error> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, S4Error> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    const fn remaining(&self) -> usize {
        self.buf.len()
    }

    /// Uzunluk önekli alan: önek kalan baytların TAMAMI olmalı (alan yükün sonundadır), ve
    /// `max`'ı aşmamalı. Ayırma yalnız bu denetimlerden sonra.
    fn tail_with_len(&mut self, len: usize, max: usize) -> Result<Vec<u8>, S4Error> {
        if len > max {
            return Err(S4Error::TooLong);
        }
        if len != self.remaining() {
            return Err(S4Error::LengthMismatch);
        }
        Ok(self.take(len)?.to_vec())
    }

    fn finish(&self) -> Result<(), S4Error> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(S4Error::TrailingBytes)
        }
    }
}

fn len_u32(len: usize) -> Result<u32, S4Error> {
    u32::try_from(len).map_err(|_| S4Error::TooLong)
}

fn len_u16(len: usize) -> Result<u16, S4Error> {
    u16::try_from(len).map_err(|_| S4Error::TooLong)
}

// ── HubChannel çerçevesi (S4 §2.1, §2.2) ────────────────────────────────────────────────────────

/// Kuyruksuz kontrol çerçevesi kurar: `[0;4] ‖ frame_type ‖ body` (S4 §2.1). 16 sıfır kuyruğu
/// **seam ekler**, burada eklenmez.
///
/// # Errors
/// [`S4Error::TooLong`]: kuyrukla birlikte `MAX_DATAGRAM`'ı aşar.
pub fn hub_frame_encode(frame_type: u8, body: &[u8]) -> Result<Vec<u8>, S4Error> {
    let len = CONTROL_PREFIX_LEN + 1 + body.len();
    if len > MAX_OPENED_FRAME {
        return Err(S4Error::TooLong);
    }
    let mut out = Vec::with_capacity(len);
    out.extend_from_slice(&HUB_CHANNEL_HANDLE.to_be_bytes());
    out.push(frame_type);
    out.extend_from_slice(body);
    Ok(out)
}

/// Kuyruğu seam tarafından çıkarılmış ("açılmış") kontrol çerçevesini böler: `(frame_type, body)`.
/// `frame_type` burada yorumlanmaz; tipli çözme [`HubFrame::decode`].
///
/// # Errors
/// [`S4Error::Truncated`] (< 5 B) · [`S4Error::TooLong`] (> `MAX_DATAGRAM − 16`) ·
/// [`S4Error::NotControl`] (ilk dört bayt 0 değil).
pub fn hub_frame_decode(opened_frame: &[u8]) -> Result<(u8, &[u8]), S4Error> {
    if opened_frame.len() > MAX_OPENED_FRAME {
        return Err(S4Error::TooLong);
    }
    let mut r = Reader::new(opened_frame);
    if r.u32()? != HUB_CHANNEL_HANDLE {
        return Err(S4Error::NotControl);
    }
    let frame_type = r.u8()?;
    Ok((frame_type, r.buf))
}

/// HEARTBEAT / HEARTBEAT_ACK gövdesi (S4 §2.2): boş ya da `echo_token u32`. ACK gelenle aynı
/// uzunlukta, token aynen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Heartbeat {
    /// `echo_token` (gövde 4 B) ya da yok (gövde boş).
    pub token: Option<u32>,
}

impl Heartbeat {
    /// Gövde baytları (0 ya da 4 B).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.token
            .map_or_else(Vec::new, |t| t.to_be_bytes().to_vec())
    }

    /// # Errors
    /// [`S4Error::BadLength`]: gövde 0 ya da 4 B değil (S4 §2.2 "başka uzunluk → reject").
    pub fn decode(body: &[u8]) -> Result<Self, S4Error> {
        match body.len() {
            0 => Ok(Self { token: None }),
            ECHO_TOKEN_LEN => Ok(Self {
                token: Some(Reader::new(body).u32()?),
            }),
            _ => Err(S4Error::BadLength),
        }
    }
}

/// Tipli HubChannel çerçevesi (S4 §2.2). Başka `frame_type` hub'a gelmez; gelirse sessiz drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubFrame<'a> {
    /// `0x1B` HEARTBEAT (tel → hub).
    Heartbeat(Heartbeat),
    /// `0x1C` HEARTBEAT_ACK (hub → tel).
    HeartbeatAck(Heartbeat),
    /// `0x1D` PLANE1_RPC: gövde bir [`AppRpcFrame`]'dir (çözülmeden verilir).
    Plane1Rpc(&'a [u8]),
}

impl<'a> HubFrame<'a> {
    /// Kuyruksuz kontrol çerçevesinin baytları.
    ///
    /// # Errors
    /// [`hub_frame_encode`] ile aynı.
    pub fn encode(&self) -> Result<Vec<u8>, S4Error> {
        match self {
            Self::Heartbeat(h) => hub_frame_encode(FRAME_HEARTBEAT, &h.encode()),
            Self::HeartbeatAck(h) => hub_frame_encode(FRAME_HEARTBEAT_ACK, &h.encode()),
            Self::Plane1Rpc(body) => hub_frame_encode(FRAME_PLANE1_RPC, body),
        }
    }

    /// # Errors
    /// [`hub_frame_decode`]'unkiler + [`S4Error::UnknownFrameType`] + [`Heartbeat::decode`]'unki.
    pub fn decode(opened_frame: &'a [u8]) -> Result<Self, S4Error> {
        let (frame_type, body) = hub_frame_decode(opened_frame)?;
        match frame_type {
            FRAME_HEARTBEAT => Ok(Self::Heartbeat(Heartbeat::decode(body)?)),
            FRAME_HEARTBEAT_ACK => Ok(Self::HeartbeatAck(Heartbeat::decode(body)?)),
            FRAME_PLANE1_RPC => Ok(Self::Plane1Rpc(body)),
            _ => Err(S4Error::UnknownFrameType),
        }
    }
}

// ── AppRpcFrame (S4 §2.3) ───────────────────────────────────────────────────────────────────────

/// `AppRpcFrame` (S4 §2.3, app-rpc §3 birebir): `method_id u8 · flags u8 · request_id u32 ·
/// payload_len u32 · payload`.
///
/// `method_id` bit 7 alışverişi başlatan tarafı söyler: 0 = telefon (istek VE yanıtı aynı
/// `method_id`), 1 = hub (DELIVER, ROUTE_MISS; yanıtı yoktur). `flags` bit 0 = yanıt, bit 1 = hata;
/// diğer bitler 0. Bu tip yalnız çerçeve yapısını denetler; anlam denetimi [`RpcMessage`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppRpcFrame {
    /// `method_id`.
    pub method_id: u8,
    /// `flags`.
    pub flags: u8,
    /// `request_id` (telefon başlatıyorsa telefonun sayacı; hub başlatıyorsa 0).
    pub request_id: u32,
    /// `payload` (`payload_len ≤ MAX_DATAGRAM − 31`).
    pub payload: Vec<u8>,
}

impl AppRpcFrame {
    /// # Errors
    /// [`S4Error::TooLong`]: `payload` > [`MAX_RPC_PAYLOAD`].
    pub fn encode(&self) -> Result<Vec<u8>, S4Error> {
        if self.payload.len() > MAX_RPC_PAYLOAD {
            return Err(S4Error::TooLong);
        }
        let mut out = Vec::with_capacity(RPC_HEADER_LEN + self.payload.len());
        out.push(self.method_id);
        out.push(self.flags);
        out.extend_from_slice(&self.request_id.to_be_bytes());
        out.extend_from_slice(&len_u32(self.payload.len())?.to_be_bytes());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    /// # Errors
    /// [`S4Error::Truncated`] (başlık < 10 B) · [`S4Error::TooLong`] (`payload_len` sınırı aşar) ·
    /// [`S4Error::LengthMismatch`] (`payload_len` kalan baytla tutmuyor).
    pub fn decode(bytes: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(bytes);
        let method_id = r.u8()?;
        let flags = r.u8()?;
        let request_id = r.u32()?;
        let payload_len = usize::try_from(r.u32()?).map_err(|_| S4Error::TooLong)?;
        let payload = r.tail_with_len(payload_len, MAX_RPC_PAYLOAD)?;
        Ok(Self {
            method_id,
            flags,
            request_id,
            payload,
        })
    }
}

// ── Yükler (S4 §2.4) ────────────────────────────────────────────────────────────────────────────

/// CLIENT_BIND isteği, 1. tur (S4 §2.4(a)): `device_cert_mode u8 = 0x00 · device_cert_ref bytes32 ·
/// presence_flags u8 (PoC: 0) · client_nonce bytes16` = 50 B. PoC'de sabit iki alan tipte tutulmaz:
/// kodlamada 0 yazılır, çözmede 0 değilse [`S4Error::BadField`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientBindReq {
    /// `device_cert_ref` = SHA-256(cert).
    pub device_cert_ref: CertRef,
    /// `client_nonce` (her bind denemesinde yeni, telefonun CSPRNG'si).
    pub client_nonce: Nonce,
}

impl ClientBindReq {
    /// Yük boyu.
    pub const LEN: usize = 1 + CERT_REF_LEN + 1 + NONCE_LEN;

    /// Yük baytları.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(0x00);
        out.extend_from_slice(&self.device_cert_ref);
        out.push(0x00);
        out.extend_from_slice(&self.client_nonce);
        out
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`] · [`S4Error::BadField`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let mode = r.u8()?;
        let device_cert_ref = r.array()?;
        let presence = r.u8()?;
        let client_nonce = r.array()?;
        r.finish()?;
        if mode != 0x00 || presence != 0x00 {
            return Err(S4Error::BadField);
        }
        Ok(Self {
            device_cert_ref,
            client_nonce,
        })
    }
}

/// CLIENT_BIND yanıtı = meydan okuma (S4 §2.4(a)): `bind_ok u8 = 0 · mailbox_cursor u64 = 0 ·
/// revocation_seq u64 = 0 · error_code u16 = 0x0003 CHALLENGE · hub_nonce bytes16` = 35 B.
/// `hub_nonce` cookie'dir (girdisi [`cookie_input`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientBindResp {
    /// `hub_nonce` (durumsuz cookie).
    pub hub_nonce: Nonce,
}

impl ClientBindResp {
    /// Yük boyu.
    pub const LEN: usize = 1 + 8 + 8 + 2 + NONCE_LEN;

    /// Yük baytları.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(0);
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&STATUS_CHALLENGE.to_be_bytes());
        out.extend_from_slice(&self.hub_nonce);
        out
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`] · [`S4Error::BadField`] (sabit alanlardan
    /// biri farklı).
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let bind_ok = r.u8()?;
        let cursor = r.u64()?;
        let revocation = r.u64()?;
        let code = r.u16()?;
        let hub_nonce = r.array()?;
        r.finish()?;
        if bind_ok != 0 || cursor != 0 || revocation != 0 || code != STATUS_CHALLENGE {
            return Err(S4Error::BadField);
        }
        Ok(Self { hub_nonce })
    }
}

/// BIND_PROOF isteği, 2. tur (S4 §2.4(b)): `device_cert_ref bytes32 · client_nonce bytes16 ·
/// hub_nonce bytes16 · client_signature bytes64` = 128 B. İmza [`bind_tbs`] üzerinde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindProofReq {
    /// `device_cert_ref`.
    pub device_cert_ref: CertRef,
    /// 1. turdaki `client_nonce`.
    pub client_nonce: Nonce,
    /// 1. turda alınan `hub_nonce`.
    pub hub_nonce: Nonce,
    /// `Ed25519_Sign(device_ed25519_sk, bind_tbs(..))`.
    pub client_signature: Signature,
}

impl BindProofReq {
    /// Yük boyu.
    pub const LEN: usize = CERT_REF_LEN + NONCE_LEN + NONCE_LEN + SIGNATURE_LEN;

    /// Yük baytları.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.extend_from_slice(&self.device_cert_ref);
        out.extend_from_slice(&self.client_nonce);
        out.extend_from_slice(&self.hub_nonce);
        out.extend_from_slice(&self.client_signature);
        out
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let out = Self {
            device_cert_ref: r.array()?,
            client_nonce: r.array()?,
            hub_nonce: r.array()?,
            client_signature: r.array()?,
        };
        r.finish()?;
        Ok(out)
    }
}

/// BIND_PROOF yanıtı = bağlandı (S4 §2.4(b)): `bind_ok u8 = 1 · mailbox_cursor u64 = 0 ·
/// revocation_seq u64 = 0 · error_code u16 = 0 · hub_signature bytes64` = 83 B. İmza [`ack_tbs`]
/// üzerinde (`bind_ok = 1`, `error_code = 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindProofResp {
    /// `Ed25519_Sign(hub_ed25519_sk, ack_tbs(..))`.
    pub hub_signature: Signature,
}

impl BindProofResp {
    /// Yük boyu.
    pub const LEN: usize = 1 + 8 + 8 + 2 + SIGNATURE_LEN;

    /// Yük baytları.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.push(1);
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&0u64.to_be_bytes());
        out.extend_from_slice(&STATUS_OK.to_be_bytes());
        out.extend_from_slice(&self.hub_signature);
        out
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`] · [`S4Error::BadField`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let bind_ok = r.u8()?;
        let cursor = r.u64()?;
        let revocation = r.u64()?;
        let code = r.u16()?;
        let hub_signature = r.array()?;
        r.finish()?;
        if bind_ok != 1 || cursor != 0 || revocation != 0 || code != STATUS_OK {
            return Err(S4Error::BadField);
        }
        Ok(Self { hub_signature })
    }
}

/// S4 §2.4(c) geçerli `(hub_handle, envelope_type)` birleşimleri:
///
/// | `hub_handle` | `envelope_type` | anlam |
/// |---|---|---|
/// | 0 | `0x01` SESSION_INIT | yeni oturum |
/// | 0 | `0x05` RELAY_OPEN | yeni medya hattı |
/// | h ≠ 0 | `0x02` SESSION_INIT_RESPONSE | h hattındaki INIT'e cevap |
///
/// Başka her birleşim (`0·0x02`, `h·0x01`, `h·0x05`, `0x03`/`0x04`/diğer) geçersizdir → hub
/// `BAD_REQUEST` döner.
#[must_use]
pub const fn valid_combo(hub_handle: u32, envelope_type: u8) -> bool {
    matches!(
        (hub_handle == 0, envelope_type),
        (true, ENVELOPE_SESSION_INIT | ENVELOPE_RELAY_OPEN)
            | (false, ENVELOPE_SESSION_INIT_RESPONSE)
    )
}

/// STORE_MESSAGE isteği (S4 §2.4(c)): `hub_handle u32 (0 = yeni hat) · envelope_type u8 ·
/// recipient_dest_hash bytes32 · envelope_len u32 · envelope`.
///
/// [`StoreMessageReq::decode`] yalnız yapıyı denetler (hub `BAD_REQUEST`'i aynı `request_id` ile
/// cevaplayabilsin diye geçersiz birleşim de çözülür); kurallar [`StoreMessageReq::validate`]'te.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StoreMessageReq {
    /// 0 = yeni hat; ≠ 0 = mevcut hat.
    pub hub_handle: u32,
    /// `envelope_type`.
    pub envelope_type: u8,
    /// Alıcı.
    pub recipient_dest_hash: DestHash,
    /// Opak zarf (hub açmaz).
    pub envelope: Vec<u8>,
}

impl StoreMessageReq {
    /// En büyük zarf: `MAX_RPC_PAYLOAD − 41` = 1208 B.
    pub const MAX_ENVELOPE: usize = MAX_RPC_PAYLOAD - STORE_REQ_FIXED_LEN;

    /// # Errors
    /// [`S4Error::TooLong`]: zarf > [`Self::MAX_ENVELOPE`].
    pub fn encode(&self) -> Result<Vec<u8>, S4Error> {
        if self.envelope.len() > Self::MAX_ENVELOPE {
            return Err(S4Error::TooLong);
        }
        let mut out = Vec::with_capacity(STORE_REQ_FIXED_LEN + self.envelope.len());
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.push(self.envelope_type);
        out.extend_from_slice(&self.recipient_dest_hash);
        out.extend_from_slice(&len_u32(self.envelope.len())?.to_be_bytes());
        out.extend_from_slice(&self.envelope);
        Ok(out)
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TooLong`] · [`S4Error::LengthMismatch`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let hub_handle = r.u32()?;
        let envelope_type = r.u8()?;
        let recipient_dest_hash = r.array()?;
        let len = usize::try_from(r.u32()?).map_err(|_| S4Error::TooLong)?;
        let envelope = r.tail_with_len(len, Self::MAX_ENVELOPE)?;
        Ok(Self {
            hub_handle,
            envelope_type,
            recipient_dest_hash,
            envelope,
        })
    }

    /// S4 §2.4(c) tel kuralları: geçerli birleşim ([`valid_combo`]) ve `RELAY_OPEN` zarfı ≤ 64 B.
    /// Dizin kuralları (alıcı dizinde, gönderenden farklı, hattın karşı ucu) hub'ın durumuna
    /// bağlıdır ve burada denetlenmez.
    ///
    /// # Errors
    /// [`S4Error::BadRequest`].
    pub fn validate(&self) -> Result<(), S4Error> {
        if !valid_combo(self.hub_handle, self.envelope_type) {
            return Err(S4Error::BadRequest);
        }
        if self.envelope_type == ENVELOPE_RELAY_OPEN
            && self.envelope.len() > RELAY_OPEN_MAX_ENVELOPE
        {
            return Err(S4Error::BadRequest);
        }
        Ok(())
    }
}

/// STORE_MESSAGE yanıtı (S4 §2.4(c), app-rpc §6.1 birebir): `status u16 · assigned_handle u32`.
///
/// `status ∈ {OK, PEER_UNBOUND, UNKNOWN_HANDLE}`; `OK` ise `assigned_handle ≠ 0`.
///
/// **`UNKNOWN_HANDLE`'ın iki anlamı** (S4 §2.3, KR-19): (1) istekte `hub_handle ≠ 0` ve o hat yok;
/// (2) istekte `hub_handle = 0` ve yeni-hat tahsis anahtarı ([`AllocationKey`]) tombstone'da —
/// emekli anahtar: `{UNKNOWN_HANDLE, assigned_handle = 0}`, tahsis yok, DELIVER yok. İstemci (2)'de
/// eski denemeyi sonlandırır; niyet sürüyorsa yeni kriptografik INIT + yeni `request_id` (§4 C3).
/// `OK` hub'ın isteği kabul edip handle tahsis ettiğini söyler; karşı ucun INIT'i aldığını
/// söylemez (KR-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreMessageResp {
    /// `status` (durum kodu sütunu, S4 §2.5).
    pub status: u16,
    /// `assigned_handle`.
    pub assigned_handle: u32,
}

impl StoreMessageResp {
    /// Yük boyu.
    pub const LEN: usize = 2 + 4;

    /// Yük baytları.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.extend_from_slice(&self.status.to_be_bytes());
        out.extend_from_slice(&self.assigned_handle.to_be_bytes());
        out
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`] · [`S4Error::BadCode`] ·
    /// [`S4Error::ZeroHandle`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let status = r.u16()?;
        let assigned_handle = r.u32()?;
        r.finish()?;
        match status {
            STATUS_OK if assigned_handle == 0 => Err(S4Error::ZeroHandle),
            STATUS_OK | STATUS_PEER_UNBOUND | STATUS_UNKNOWN_HANDLE => Ok(Self {
                status,
                assigned_handle,
            }),
            _ => Err(S4Error::BadCode),
        }
    }
}

/// DELIVER (hub → tel, S4 §2.4(d)): `hub_handle u32 · sender_dest_hash bytes32 · envelope_type u8 ·
/// envelope_len u32 · envelope`.
///
/// `sender_dest_hash` hub'ın bağlı kaynaktan **tanıklık ettiği** gönderendir; hub kanalı imzasız
/// olduğu için sahtelenebilir, hiçbir güvenlik kararı ona dayanmaz (beyanlı sınır 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Deliver {
    /// Hattın tutamacı (≠ 0).
    pub hub_handle: u32,
    /// Hub'ın tanıklık ettiği gönderen.
    pub sender_dest_hash: DestHash,
    /// `0x01` / `0x02` / `0x05`.
    pub envelope_type: u8,
    /// Opak zarf.
    pub envelope: Vec<u8>,
}

impl Deliver {
    /// En büyük zarf: `MAX_RPC_PAYLOAD − 41` = 1208 B.
    pub const MAX_ENVELOPE: usize = MAX_RPC_PAYLOAD - DELIVER_FIXED_LEN;

    /// # Errors
    /// [`S4Error::TooLong`]: zarf > [`Self::MAX_ENVELOPE`].
    pub fn encode(&self) -> Result<Vec<u8>, S4Error> {
        if self.envelope.len() > Self::MAX_ENVELOPE {
            return Err(S4Error::TooLong);
        }
        let mut out = Vec::with_capacity(DELIVER_FIXED_LEN + self.envelope.len());
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.extend_from_slice(&self.sender_dest_hash);
        out.push(self.envelope_type);
        out.extend_from_slice(&len_u32(self.envelope.len())?.to_be_bytes());
        out.extend_from_slice(&self.envelope);
        Ok(out)
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TooLong`] · [`S4Error::LengthMismatch`] ·
    /// [`S4Error::ZeroHandle`] · [`S4Error::UnknownEnvelopeType`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let hub_handle = r.u32()?;
        let sender_dest_hash = r.array()?;
        let envelope_type = r.u8()?;
        let len = usize::try_from(r.u32()?).map_err(|_| S4Error::TooLong)?;
        let envelope = r.tail_with_len(len, Self::MAX_ENVELOPE)?;
        if hub_handle == 0 {
            return Err(S4Error::ZeroHandle);
        }
        if !matches!(
            envelope_type,
            ENVELOPE_SESSION_INIT | ENVELOPE_SESSION_INIT_RESPONSE | ENVELOPE_RELAY_OPEN
        ) {
            return Err(S4Error::UnknownEnvelopeType);
        }
        Ok(Self {
            hub_handle,
            sender_dest_hash,
            envelope_type,
            envelope,
        })
    }
}

/// ROUTE_MISS (hub → tel, S4 §2.4(e)): `hub_handle u32 · reason u8 (0x01 UNKNOWN_HANDLE)` = 5 B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteMiss {
    /// Bilinmeyen/emekli hattın tutamacı (≠ 0: veri datagramının ilk dört baytı).
    pub hub_handle: u32,
    /// [`ROUTE_MISS_UNKNOWN_HANDLE`].
    pub reason: u8,
}

impl RouteMiss {
    /// Yük boyu.
    pub const LEN: usize = 4 + 1;

    /// Yük baytları.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::LEN);
        out.extend_from_slice(&self.hub_handle.to_be_bytes());
        out.push(self.reason);
        out
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`] · [`S4Error::ZeroHandle`] ·
    /// [`S4Error::BadField`] (`reason ≠ 0x01`).
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let hub_handle = r.u32()?;
        let reason = r.u8()?;
        r.finish()?;
        if hub_handle == 0 {
            return Err(S4Error::ZeroHandle);
        }
        if reason != ROUTE_MISS_UNKNOWN_HANDLE {
            return Err(S4Error::BadField);
        }
        Ok(Self { hub_handle, reason })
    }
}

/// Hata bayraklı (`flags = 0x03`) her yanıtın gövdesi (S4 §2.3; app-rpc §11 aynen):
/// `error_code u16 · detail_len u16 · detail`. PoC'de hub `detail`'i boş gönderir.
///
/// S4 §2.5 iki sütunludur: **durum kodları** (OK, CHALLENGE, PEER_UNBOUND, UNKNOWN_HANDLE) yanıt
/// gövdesinin kendi alanındadır; **hata kodları** (POSSESSION_PROOF_FAIL, BAD_COOKIE, HUB_CAPACITY,
/// RATE_LIMITED, BAD_REQUEST) burada. Durum kodu taşıyan `ErrorPayload` [`S4Error::BadCode`]'dur.
/// İmzasızdır: yoldaki saldırgan üretebilir, yalnız yeniden deneme ipucudur (S4 §4 C8).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ErrorPayload {
    /// Hata kodu (S4 §2.5 hata sütunu).
    pub error_code: u16,
    /// `detail` (PoC: boş).
    pub detail: Vec<u8>,
}

impl ErrorPayload {
    /// `detail` dahil en büyük gövde [`MAX_RPC_PAYLOAD`]'dır.
    pub const MAX_DETAIL: usize = MAX_RPC_PAYLOAD - 4;

    /// S4 §2.5 hata sütunundaki kodlar.
    #[must_use]
    pub const fn is_error_code(code: u16) -> bool {
        matches!(
            code,
            ERROR_POSSESSION_PROOF_FAIL
                | ERROR_BAD_COOKIE
                | ERROR_HUB_CAPACITY
                | ERROR_RATE_LIMITED
                | ERROR_BAD_REQUEST
        )
    }

    /// # Errors
    /// [`S4Error::TooLong`]: `detail` > [`Self::MAX_DETAIL`].
    pub fn encode(&self) -> Result<Vec<u8>, S4Error> {
        if self.detail.len() > Self::MAX_DETAIL {
            return Err(S4Error::TooLong);
        }
        let mut out = Vec::with_capacity(4 + self.detail.len());
        out.extend_from_slice(&self.error_code.to_be_bytes());
        out.extend_from_slice(&len_u16(self.detail.len())?.to_be_bytes());
        out.extend_from_slice(&self.detail);
        Ok(out)
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TooLong`] · [`S4Error::LengthMismatch`] ·
    /// [`S4Error::BadCode`].
    pub fn decode(payload: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(payload);
        let error_code = r.u16()?;
        let len = usize::from(r.u16()?);
        let detail = r.tail_with_len(len, Self::MAX_DETAIL)?;
        if !Self::is_error_code(error_code) {
            return Err(S4Error::BadCode);
        }
        Ok(Self { error_code, detail })
    }
}

/// RELAY_OPEN zarfı (`envelope_type 0x05`'in gövdesi, S4 §2.4(f)): `open_nonce bytes16` = 16 B.
/// İçinde `call_id` **yoktur** (telde açık taşınmaz); hub için opak, alıcı için bilgi (KR-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RelayOpenEnvelope {
    /// Telefonun CSPRNG'sinden, her açılışta yeni.
    pub open_nonce: Nonce,
}

impl RelayOpenEnvelope {
    /// Zarf boyu.
    pub const LEN: usize = NONCE_LEN;

    /// Zarf baytları.
    #[must_use]
    pub const fn encode(&self) -> [u8; Self::LEN] {
        self.open_nonce
    }

    /// # Errors
    /// [`S4Error::Truncated`] · [`S4Error::TrailingBytes`].
    pub fn decode(envelope: &[u8]) -> Result<Self, S4Error> {
        let mut r = Reader::new(envelope);
        let open_nonce = r.array()?;
        r.finish()?;
        Ok(Self { open_nonce })
    }
}

// ── Mesaj tablosu (S4 §2.3) ─────────────────────────────────────────────────────────────────────

/// S4 §2.3 mesaj tablosunun bir satırı (gövde). `method_id` ve `flags` gövdenin türünden çıkar.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RpcBody {
    /// `0x01 · 00` CLIENT_BIND istek.
    ClientBindReq(ClientBindReq),
    /// `0x01 · 01` CLIENT_BIND yanıt (meydan okuma).
    ClientBindResp(ClientBindResp),
    /// `0x0B · 00` BIND_PROOF istek.
    BindProofReq(BindProofReq),
    /// `0x0B · 01` BIND_PROOF yanıt (bağlandı).
    BindProofResp(BindProofResp),
    /// `0x05 · 00` STORE_MESSAGE istek.
    StoreMessageReq(StoreMessageReq),
    /// `0x05 · 01` STORE_MESSAGE yanıt.
    StoreMessageResp(StoreMessageResp),
    /// `<method> · 03` hata yanıtı (`method_id` ∈ {0x01, 0x05, 0x0B}).
    Error {
        /// Hatanın ait olduğu telefon-başlatımlı method.
        method_id: u8,
        /// Gövde.
        payload: ErrorPayload,
    },
    /// `0x81 · 00` DELIVER (`request_id = 0`).
    Deliver(Deliver),
    /// `0x84 · 00` ROUTE_MISS (`request_id = 0`).
    RouteMiss(RouteMiss),
}

/// Çözülmüş ve anlamı denetlenmiş `AppRpcFrame` (S4 §2.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RpcMessage {
    /// `request_id` (hub başlatımlıda 0).
    pub request_id: u32,
    /// Satır.
    pub body: RpcBody,
}

const fn is_phone_method(method_id: u8) -> bool {
    matches!(
        method_id,
        METHOD_CLIENT_BIND | METHOD_STORE_MESSAGE | METHOD_BIND_PROOF
    )
}

impl RpcMessage {
    /// `(method_id, flags, payload)` üçlüsü.
    ///
    /// # Errors
    /// [`S4Error::UnknownMethod`] (hata satırında telefon-başlatımlı olmayan method) ·
    /// [`S4Error::BadRequestId`] (hub başlatımlıda `request_id ≠ 0`) · yük kodlayıcılarının
    /// [`S4Error::TooLong`]'u.
    pub fn to_frame(&self) -> Result<AppRpcFrame, S4Error> {
        const REQ: u8 = 0;
        const RESP: u8 = FLAG_RESPONSE;
        const ERR: u8 = FLAG_RESPONSE | FLAG_ERROR;
        let (method_id, flags, payload) = match &self.body {
            RpcBody::ClientBindReq(m) => (METHOD_CLIENT_BIND, REQ, m.encode()),
            RpcBody::ClientBindResp(m) => (METHOD_CLIENT_BIND, RESP, m.encode()),
            RpcBody::BindProofReq(m) => (METHOD_BIND_PROOF, REQ, m.encode()),
            RpcBody::BindProofResp(m) => (METHOD_BIND_PROOF, RESP, m.encode()),
            RpcBody::StoreMessageReq(m) => (METHOD_STORE_MESSAGE, REQ, m.encode()?),
            RpcBody::StoreMessageResp(m) => (METHOD_STORE_MESSAGE, RESP, m.encode()),
            RpcBody::Error { method_id, payload } => {
                if !is_phone_method(*method_id) {
                    return Err(S4Error::UnknownMethod);
                }
                (*method_id, ERR, payload.encode()?)
            }
            RpcBody::Deliver(m) => (METHOD_DELIVER, REQ, m.encode()?),
            RpcBody::RouteMiss(m) => (METHOD_ROUTE_MISS, REQ, m.encode()),
        };
        if method_id & METHOD_HUB_INITIATED != 0 && self.request_id != 0 {
            return Err(S4Error::BadRequestId);
        }
        Ok(AppRpcFrame {
            method_id,
            flags,
            request_id: self.request_id,
            payload,
        })
    }

    /// `AppRpcFrame` baytları (PLANE1_RPC gövdesi).
    ///
    /// # Errors
    /// [`Self::to_frame`] ve [`AppRpcFrame::encode`] ile aynı.
    pub fn encode(&self) -> Result<Vec<u8>, S4Error> {
        self.to_frame()?.encode()
    }

    /// Çerçeveyi S4 §2.3 tablosuna göre yorumlar. Sıra: method → flags → `request_id` → yük.
    ///
    /// # Errors
    /// [`S4Error::UnknownMethod`] · [`S4Error::BadFlags`] (00/01/03 dışı; hub başlatımlıda ≠ 00) ·
    /// [`S4Error::BadRequestId`] · yük çözücülerinin hataları.
    pub fn from_frame(frame: &AppRpcFrame) -> Result<Self, S4Error> {
        let m = frame.method_id;
        let p = frame.payload.as_slice();
        if !is_phone_method(m) && m != METHOD_DELIVER && m != METHOD_ROUTE_MISS {
            return Err(S4Error::UnknownMethod);
        }
        if m & METHOD_HUB_INITIATED != 0 {
            if frame.flags != 0 {
                return Err(S4Error::BadFlags);
            }
            if frame.request_id != 0 {
                return Err(S4Error::BadRequestId);
            }
        }
        let body = match (m, frame.flags) {
            (METHOD_CLIENT_BIND, 0x00) => RpcBody::ClientBindReq(ClientBindReq::decode(p)?),
            (METHOD_CLIENT_BIND, 0x01) => RpcBody::ClientBindResp(ClientBindResp::decode(p)?),
            (METHOD_BIND_PROOF, 0x00) => RpcBody::BindProofReq(BindProofReq::decode(p)?),
            (METHOD_BIND_PROOF, 0x01) => RpcBody::BindProofResp(BindProofResp::decode(p)?),
            (METHOD_STORE_MESSAGE, 0x00) => RpcBody::StoreMessageReq(StoreMessageReq::decode(p)?),
            (METHOD_STORE_MESSAGE, 0x01) => RpcBody::StoreMessageResp(StoreMessageResp::decode(p)?),
            (_, 0x03) if is_phone_method(m) => RpcBody::Error {
                method_id: m,
                payload: ErrorPayload::decode(p)?,
            },
            (METHOD_DELIVER, _) => RpcBody::Deliver(Deliver::decode(p)?),
            (METHOD_ROUTE_MISS, _) => RpcBody::RouteMiss(RouteMiss::decode(p)?),
            _ => return Err(S4Error::BadFlags),
        };
        Ok(Self {
            request_id: frame.request_id,
            body,
        })
    }

    /// `AppRpcFrame` baytlarını çözer ve yorumlar.
    ///
    /// # Errors
    /// [`AppRpcFrame::decode`] ve [`Self::from_frame`] ile aynı.
    pub fn decode(bytes: &[u8]) -> Result<Self, S4Error> {
        Self::from_frame(&AppRpcFrame::decode(bytes)?)
    }
}

// ── Bayt birleştiriciler — kripto YOK (S4 §2.4(a)/(b)/(c)) ──────────────────────────────────────

/// `domain_sep(label)` (ccr §13.2 geleneği; S4 §2.4(b)): ASCII etiket (≤ 16 B) + `0x00` dolgu.
///
/// # Errors
/// [`S4Error::BadLength`]: etiket 16 B'tan uzun ya da ASCII değil.
pub const fn domain_sep(label: &[u8]) -> Result<[u8; DOMAIN_SEP_LEN], S4Error> {
    if label.len() > DOMAIN_SEP_LEN || !label.is_ascii() {
        return Err(S4Error::BadLength);
    }
    let mut out = [0u8; DOMAIN_SEP_LEN];
    let mut i = 0;
    while i < label.len() {
        out[i] = label[i];
        i += 1;
    }
    Ok(out)
}

/// `domain_sep("KAMA-HUB-BIND-V1")`.
pub const DOMAIN_SEP_HUB_BIND: [u8; DOMAIN_SEP_LEN] = match domain_sep(LABEL_HUB_BIND) {
    Ok(v) => v,
    Err(_) => panic!("LABEL_HUB_BIND > 16 B"),
};
/// `domain_sep("KAMA-HUB-ACK-V1")`.
pub const DOMAIN_SEP_HUB_ACK: [u8; DOMAIN_SEP_LEN] = match domain_sep(LABEL_HUB_ACK) {
    Ok(v) => v,
    Err(_) => panic!("LABEL_HUB_ACK > 16 B"),
};

/// `client_signature`'ın imzaladığı 112 B (S4 §2.4(b)):
/// `domain_sep("KAMA-HUB-BIND-V1") ‖ hub_ed25519_pub ‖ device_cert_ref ‖ client_nonce ‖ hub_nonce`.
#[must_use]
pub fn bind_tbs(
    hub_ed25519_pub: &PublicKey,
    device_cert_ref: &CertRef,
    client_nonce: &Nonce,
    hub_nonce: &Nonce,
) -> [u8; BIND_TBS_LEN] {
    let mut out = [0u8; BIND_TBS_LEN];
    let parts: [&[u8]; 5] = [
        &DOMAIN_SEP_HUB_BIND,
        hub_ed25519_pub,
        device_cert_ref,
        client_nonce,
        hub_nonce,
    ];
    concat_into(&mut out, &parts);
    out
}

/// `hub_signature`'ın imzaladığı 83 B (S4 §2.4(b)):
/// `domain_sep("KAMA-HUB-ACK-V1") ‖ device_cert_ref ‖ client_nonce ‖ hub_nonce ‖ bind_ok u8 ‖
/// error_code u16`.
#[must_use]
pub fn ack_tbs(
    device_cert_ref: &CertRef,
    client_nonce: &Nonce,
    hub_nonce: &Nonce,
    bind_ok: u8,
    error_code: u16,
) -> [u8; ACK_TBS_LEN] {
    let mut out = [0u8; ACK_TBS_LEN];
    let code = error_code.to_be_bytes();
    let parts: [&[u8]; 6] = [
        &DOMAIN_SEP_HUB_ACK,
        device_cert_ref,
        client_nonce,
        hub_nonce,
        &[bind_ok],
        &code,
    ];
    concat_into(&mut out, &parts);
    out
}

fn concat_into(out: &mut [u8], parts: &[&[u8]]) {
    let mut at = 0;
    for p in parts {
        out[at..at + p.len()].copy_from_slice(p);
        at += p.len();
    }
    debug_assert_eq!(at, out.len());
}

/// Cookie'nin kaynak adresi (S4 §2.4(a)): `fam = 0x04` + 4 B ya da `fam = 0x06` + 16 B, ve port.
/// IPv4-eşlemeli IPv6 (`::ffff:a.b.c.d`) kurulurken IPv4'e **normalize edilir**: aynı kaynak her
/// zaman aynı baytları verir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CookieSrc {
    /// `fam = 0x04`.
    V4 {
        /// IPv4 adresi.
        ip: [u8; 4],
        /// Kaynak port.
        port: u16,
    },
    /// `fam = 0x06` (IPv4-eşlemeli değil).
    V6 {
        /// IPv6 adresi.
        ip: [u8; 16],
        /// Kaynak port.
        port: u16,
    },
}

impl CookieSrc {
    /// IPv4 kaynağı.
    #[must_use]
    pub const fn v4(ip: [u8; 4], port: u16) -> Self {
        Self::V4 { ip, port }
    }

    /// IPv6 kaynağı; `::ffff:a.b.c.d` ise IPv4'e normalize edilir.
    #[must_use]
    pub const fn v6(ip: [u8; 16], port: u16) -> Self {
        let mapped = matches!(ip, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, _, _, _, _]);
        if mapped {
            Self::V4 {
                ip: [ip[12], ip[13], ip[14], ip[15]],
                port,
            }
        } else {
            Self::V6 { ip, port }
        }
    }

    /// `SocketAddr`'dan (normalizasyonla).
    #[must_use]
    pub const fn from_socket_addr(addr: SocketAddr) -> Self {
        match addr.ip() {
            IpAddr::V4(ip) => Self::v4(ip.octets(), addr.port()),
            IpAddr::V6(ip) => Self::v6(ip.octets(), addr.port()),
        }
    }

    /// `fam` baytı.
    #[must_use]
    pub const fn fam(&self) -> u8 {
        match self {
            Self::V4 { .. } => 0x04,
            Self::V6 { .. } => 0x06,
        }
    }
}

/// Cookie özetinin `cookie_secret`'tan **sonraki** girdisi (S4 §2.4(a)):
/// `fam u8 ‖ src_ip (4|16 B) ‖ src_port u16 ‖ device_cert_ref ‖ client_nonce ‖ minute u32 ‖
/// bind_epoch u32` (63 ya da 75 B).
///
/// `hub_nonce = BLAKE2s-256(cookie_secret ‖ cookie_input(..))[0..16]` — özeti ve gizli anahtarı
/// çağıran (hub) tutar; bu modül özet almaz.
#[must_use]
pub fn cookie_input(
    src: &CookieSrc,
    device_cert_ref: &CertRef,
    client_nonce: &Nonce,
    minute: u32,
    bind_epoch: u32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 16 + 2 + CERT_REF_LEN + NONCE_LEN + 4 + 4);
    out.push(src.fam());
    let port = match src {
        CookieSrc::V4 { ip, port } => {
            out.extend_from_slice(ip);
            *port
        }
        CookieSrc::V6 { ip, port } => {
            out.extend_from_slice(ip);
            *port
        }
    };
    out.extend_from_slice(&port.to_be_bytes());
    out.extend_from_slice(device_cert_ref);
    out.extend_from_slice(client_nonce);
    out.extend_from_slice(&minute.to_be_bytes());
    out.extend_from_slice(&bind_epoch.to_be_bytes());
    out
}

/// Yeni hat isteğinin tahsis anahtarı (S4 §2.4(c), KR-19; S4 §7 "idempotency-key"):
/// `(gönderen dest_hash, request_id, recipient_dest_hash, envelope_type, BLAKE2s(envelope)[0..8])`.
/// Her aktif hat kendisini açan anahtarı taşır; emekli olunca anahtar tombstone'a taşınır.
/// `envelope_hash8`'i **çağıran hesaplar**; bu modül özet almaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AllocationKey {
    /// Gönderen (hub'ın bağlı kaynaktan bildiği uç).
    pub sender_dest_hash: DestHash,
    /// İsteğin `request_id`'si.
    pub request_id: u32,
    /// Alıcı.
    pub recipient_dest_hash: DestHash,
    /// `envelope_type`.
    pub envelope_type: u8,
    /// `BLAKE2s-256(envelope)[0..8]`.
    pub envelope_hash8: [u8; 8],
}

impl AllocationKey {
    /// Sabit 77 B: `sender 32 ‖ request_id u32 ‖ recipient 32 ‖ envelope_type u8 ‖ hash8 8`.
    #[must_use]
    pub fn encode(&self) -> [u8; ALLOCATION_KEY_LEN] {
        let mut out = [0u8; ALLOCATION_KEY_LEN];
        let rid = self.request_id.to_be_bytes();
        let parts: [&[u8]; 5] = [
            &self.sender_dest_hash,
            &rid,
            &self.recipient_dest_hash,
            &[self.envelope_type],
            &self.envelope_hash8,
        ];
        concat_into(&mut out, &parts);
        out
    }
}

// ── Birim testleri: YALNIZ yapısal (round-trip, sınırlar). Beklenen baytlar `vectors/hub/`'dan. ──

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(msg: &RpcMessage) {
        let bytes = msg.encode().expect("encode");
        assert_eq!(&RpcMessage::decode(&bytes).expect("decode"), msg);
    }

    #[test]
    fn rpc_round_trips_every_row() {
        let env = vec![7u8; 33];
        let rows = [
            RpcBody::ClientBindReq(ClientBindReq {
                device_cert_ref: [1; 32],
                client_nonce: [2; 16],
            }),
            RpcBody::ClientBindResp(ClientBindResp { hub_nonce: [3; 16] }),
            RpcBody::BindProofReq(BindProofReq {
                device_cert_ref: [1; 32],
                client_nonce: [2; 16],
                hub_nonce: [3; 16],
                client_signature: [4; 64],
            }),
            RpcBody::BindProofResp(BindProofResp {
                hub_signature: [5; 64],
            }),
            RpcBody::StoreMessageReq(StoreMessageReq {
                hub_handle: 0,
                envelope_type: ENVELOPE_SESSION_INIT,
                recipient_dest_hash: [6; 32],
                envelope: env.clone(),
            }),
            RpcBody::StoreMessageResp(StoreMessageResp {
                status: STATUS_OK,
                assigned_handle: 9,
            }),
            RpcBody::Error {
                method_id: METHOD_STORE_MESSAGE,
                payload: ErrorPayload {
                    error_code: ERROR_BAD_REQUEST,
                    detail: Vec::new(),
                },
            },
        ];
        for body in rows {
            rt(&RpcMessage {
                request_id: 0xDEAD_BEEF,
                body,
            });
        }
        rt(&RpcMessage {
            request_id: 0,
            body: RpcBody::Deliver(Deliver {
                hub_handle: 9,
                sender_dest_hash: [8; 32],
                envelope_type: ENVELOPE_RELAY_OPEN,
                envelope: env,
            }),
        });
        rt(&RpcMessage {
            request_id: 0,
            body: RpcBody::RouteMiss(RouteMiss {
                hub_handle: 9,
                reason: ROUTE_MISS_UNKNOWN_HANDLE,
            }),
        });
    }

    #[test]
    fn fixed_lengths_match_s4() {
        // S4 §3 H10 datagram boyları: 81, 66, 159, 35, 36 (RPC_OVERHEAD = 31).
        assert_eq!(RPC_OVERHEAD + ClientBindReq::LEN, 81);
        assert_eq!(RPC_OVERHEAD + ClientBindResp::LEN, 66);
        assert_eq!(RPC_OVERHEAD + BindProofReq::LEN, 159);
        assert_eq!(RPC_OVERHEAD + 4, 35);
        assert_eq!(RPC_OVERHEAD + RouteMiss::LEN, 36);
        assert_eq!(MAX_RPC_PAYLOAD, 1249);
        assert_eq!(StoreMessageReq::MAX_ENVELOPE, 1208);
        assert_eq!(
            cookie_input(&CookieSrc::v4([0; 4], 0), &[0; 32], &[0; 16], 0, 0).len(),
            63
        );
        assert_eq!(
            cookie_input(&CookieSrc::v6([1; 16], 0), &[0; 32], &[0; 16], 0, 0).len(),
            75
        );
    }

    #[test]
    fn payload_bounds() {
        let at_max = StoreMessageReq {
            hub_handle: 0,
            envelope_type: ENVELOPE_SESSION_INIT,
            recipient_dest_hash: [0; 32],
            envelope: vec![0; StoreMessageReq::MAX_ENVELOPE],
        };
        let frame = RpcMessage {
            request_id: 1,
            body: RpcBody::StoreMessageReq(at_max.clone()),
        }
        .encode()
        .expect("sınırda kodlanır");
        let datagram = hub_frame_encode(FRAME_PLANE1_RPC, &frame).expect("sınır");
        assert_eq!(datagram.len() + HUB_TRAILER_LEN, MAX_DATAGRAM);
        let mut over = at_max;
        over.envelope.push(0);
        assert_eq!(over.encode(), Err(S4Error::TooLong));
        assert_eq!(
            hub_frame_encode(FRAME_PLANE1_RPC, &[0; MAX_OPENED_FRAME - 4]),
            Err(S4Error::TooLong)
        );
        // payload_len kalan baytı aşan büyük bir değer ayırma yapmadan reddedilir.
        let mut huge = vec![METHOD_STORE_MESSAGE, 0, 0, 0, 0, 1];
        huge.extend_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(AppRpcFrame::decode(&huge), Err(S4Error::TooLong));
    }

    #[test]
    fn hub_frame_and_heartbeat() {
        for hb in [Heartbeat { token: None }, Heartbeat { token: Some(42) }] {
            let f = HubFrame::Heartbeat(hb).encode().expect("encode");
            assert_eq!(HubFrame::decode(&f), Ok(HubFrame::Heartbeat(hb)));
        }
        assert_eq!(hub_frame_decode(&[0, 0, 0, 0]), Err(S4Error::Truncated));
        assert_eq!(Heartbeat::decode(&[0; 3]), Err(S4Error::BadLength));
    }

    #[test]
    fn combos_and_validate() {
        assert!(valid_combo(0, ENVELOPE_SESSION_INIT));
        assert!(valid_combo(0, ENVELOPE_RELAY_OPEN));
        assert!(valid_combo(7, ENVELOPE_SESSION_INIT_RESPONSE));
        for (h, t) in [
            (0, 0x02),
            (7, 0x01),
            (7, 0x05),
            (0, 0x03),
            (0, 0x04),
            (7, 0x03),
        ] {
            assert!(!valid_combo(h, t), "{h} {t}");
        }
        let mut req = StoreMessageReq {
            hub_handle: 0,
            envelope_type: ENVELOPE_RELAY_OPEN,
            recipient_dest_hash: [0; 32],
            envelope: vec![0; RELAY_OPEN_MAX_ENVELOPE],
        };
        assert_eq!(req.validate(), Ok(()));
        req.envelope.push(0);
        assert_eq!(req.validate(), Err(S4Error::BadRequest));
    }

    #[test]
    fn hub_initiated_rules() {
        let rm = RpcMessage {
            request_id: 1,
            body: RpcBody::RouteMiss(RouteMiss {
                hub_handle: 1,
                reason: ROUTE_MISS_UNKNOWN_HANDLE,
            }),
        };
        assert_eq!(rm.encode(), Err(S4Error::BadRequestId));
        let err = RpcMessage {
            request_id: 1,
            body: RpcBody::Error {
                method_id: METHOD_DELIVER,
                payload: ErrorPayload {
                    error_code: ERROR_BAD_REQUEST,
                    detail: Vec::new(),
                },
            },
        };
        assert_eq!(err.encode(), Err(S4Error::UnknownMethod));
    }

    #[test]
    fn cookie_src_normalizes_mapped() {
        let mapped = CookieSrc::v6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 10, 0, 0, 1], 9);
        assert_eq!(mapped, CookieSrc::v4([10, 0, 0, 1], 9));
        let sa: SocketAddr = "[::ffff:10.0.0.1]:9".parse().expect("addr");
        assert_eq!(CookieSrc::from_socket_addr(sa), mapped);
        assert_eq!(CookieSrc::v6([0x20; 16], 9).fam(), 0x06);
    }

    #[test]
    fn domain_sep_rejects_long_label() {
        assert_eq!(domain_sep(&[b'A'; 17]), Err(S4Error::BadLength));
        assert_eq!(DOMAIN_SEP_HUB_ACK[15], 0);
    }

    #[test]
    fn error_codes_columns() {
        for c in [
            STATUS_OK,
            STATUS_CHALLENGE,
            STATUS_PEER_UNBOUND,
            STATUS_UNKNOWN_HANDLE,
        ] {
            assert!(!ErrorPayload::is_error_code(c));
        }
        assert_eq!(S4Error::BadRequest.to_string(), "bad-request");
    }
}
