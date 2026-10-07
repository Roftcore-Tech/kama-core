//! A5.5 el sıkışma kodeği — SESSION_INIT (SSI1; ccr §35.3 / §35.3.1) ve SESSION_INIT_RESPONSE
//! (SSR1; ccr §35.5 / §35.5.1): wire düzeni, ayrıştırma ret kuralları, AAD, mühür / açma ve X25519
//! girdi kontrolleri (ccr §5.1).
//!
//! Anahtar türetimi [`crate::kdf`]'tedir (D1–D7); bu modül DH yapar, `session_context_v1`'i kurar
//! ve türetim çıktılarını AEAD'ye bağlar. Oturum durum makinesi (bekleyen INIT'ler, önbellek,
//! supersession) bu modülün işi DEĞİLDİR.
//!
//! * **Sıra (ret önceliği):** ayrıştırma (bayraklar → uzunluk) → genel anahtar tüm-sıfır → her DH
//!   çıktısı tüm-sıfır (`was_contributory`, sabit zamanlı) → türetim → AEAD. AEAD'ye ulaşmadan
//!   verilen her ret yapısaldır; AEAD hatası tek tiptir ([`HandshakeError::AeadFail`]).
//! * **Nonce:** her iki zarf `nonce96(0)` ile mühürlenir; güvenliği her kökün TAM BİR zarf
//!   mühürlemesine dayanır (INV-INIT-KEY-1 / INV-RESP-KEY-1). Yeniden gönderim önbellekli
//!   byte-özdeş datagramdır — bu modül aynı kökle ikinci bir zarf üreten bir yol SUNMAZ: her mühür
//!   çağrısı kendi DH'sinden yeni kök türetir.
//! * **Gizlilik etiketi:** INIT'in `early_data`'sı `ss_e = DH(sender_eph, recipient_static)` ile
//!   korunur — bu tam forward secrecy DEĞİLDİR ("per-INIT ephemeral confidentiality"); tam FS
//!   epoch-1'de, RESPONSE'un yanıtlayan ephemeral'ı katıldıktan sonradır.
//! * **Sertifika baytları OPAKTIR:** `cert_field` wire'da ne varsa AAD'ye aynen girer. Gönderen
//!   kimliği `sender_cert_ref`'tir (ref kipinde alanın kendisi; inline ve bundle kipinde
//!   `SHA-256(compact_device_cert)`); doğrulaması çağıranın eş dizinindedir (ccr §35.4 KAPALI-SİSTEM
//!   KİPİ, G1).

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::kdf::{
    self, Epoch1Root, InitRoot, KdfError, TrafficKeys, DH_LEN, HASH_LEN, SESSION_CONTEXT_V1_LEN,
    SESSION_ID_INTERNAL_LEN,
};
use crate::outer::TAG_LEN;
use crate::protector::nonce96;

/// `protocol_version` (ccr §35.2 `session_context_v1`; §35.3.1 / §35.5.1 AAD).
pub const PROTOCOL_VERSION_V1: u8 = 0x01;
/// `profile_id` = SESSION_PROFILE_V3_DEFAULT (ccr §35.2; Alpha sabit).
pub const PROFILE_ID_V3_DEFAULT: u8 = 0x01;
/// `cipher_suite_id` (ccr §35.2; Alpha sabit — pki §13 registry).
pub const CIPHER_SUITE_ID_V1: u16 = 0x0001;

/// SSI1 AAD öneki (wire'da YOK; ccr §35.3.1).
pub const SSI1_AAD_MAGIC: [u8; 4] = *b"SSI1";
/// SSR1 AAD öneki (wire'da YOK; ccr §35.5.1).
pub const SSR1_AAD_MAGIC: [u8; 4] = *b"SSR1";

/// X25519 genel anahtarı / DH çıktısı.
pub const X25519_LEN: usize = DH_LEN;
/// `recipient_dest_hash` (ccr §35.3).
pub const DEST_HASH_LEN: usize = HASH_LEN;
/// `sender_cert_ref` = SHA-256(canonical_compact_device_cert) (ccr §35.2).
pub const CERT_REF_LEN: usize = HASH_LEN;
/// CompactDeviceCertV1 (ccr §13; sabit, uzunluk öneki YOK).
pub const COMPACT_DEVICE_CERT_LEN: usize = 152;
/// DeviceCertBundle toplam üst sınırı (ccr §6.7).
pub const DEVICE_CERT_BUNDLE_MAX: usize = 16_384;
/// DeviceCertBundle magic'i (ccr §6.1 / §20).
pub const DEVICE_CERT_BUNDLE_MAGIC: [u8; 4] = *b"SRDB";
/// DeviceCertBundle sürümü (ccr §20).
pub const DEVICE_CERT_BUNDLE_VERSION: u8 = 1;

const INIT_COUNTER_LEN: usize = 8;
const RESPONSE_COUNTER_LEN: usize = 8;
/// flags(1) + recipient_dest_hash(32) + sender_ephemeral(32) — `cert_field`'den önceki sabit önek.
const SSI1_PREFIX_LEN: usize = 1 + DEST_HASH_LEN + X25519_LEN;
/// flags(1) + responder_ephemeral(32) + response_counter(8) — ciphertext'ten önceki sabit önek.
const SSR1_PREFIX_LEN: usize = 1 + X25519_LEN + RESPONSE_COUNTER_LEN;

/// SSI1 REF asgari boyu: 1 + 32 + 32 + 32 + 8 + tag 16 = 121 B (ccr §35.3 byte tablosu).
pub const SSI1_REF_MIN_LEN: usize = SSI1_PREFIX_LEN + CERT_REF_LEN + INIT_COUNTER_LEN + TAG_LEN;
/// SSI1 INLINE asgari boyu: 1 + 32 + 32 + 152 + 8 + tag 16 = 241 B.
pub const SSI1_INLINE_MIN_LEN: usize =
    SSI1_PREFIX_LEN + COMPACT_DEVICE_CERT_LEN + INIT_COUNTER_LEN + TAG_LEN;
/// SSR1 asgari boyu: 1 + 32 + 8 + tag 16 = 57 B (ccr §35.5).
pub const SSR1_MIN_LEN: usize = SSR1_PREFIX_LEN + TAG_LEN;
/// SSR1 AAD boyu: magic 4 + pv 1 + suite 2 + flags 1 + eph 32 + counter 8 + sid 16.
pub const SSR1_AAD_LEN: usize =
    4 + 1 + 2 + 1 + X25519_LEN + RESPONSE_COUNTER_LEN + SESSION_ID_INTERNAL_LEN;

/// `flags` bit 0..1 = cert_mode.
const FLAGS_CERT_MODE: u8 = 0b0000_0011;
/// `flags` bit 2..3 = init_variant (yalnız 00 = SR_SM3_AUTHENTICATED_ONE_WAY_DEFAULT).
const FLAGS_INIT_VARIANT: u8 = 0b0000_1100;
/// `flags` bit 4..7 = reserved (0 olmalı).
const FLAGS_RESERVED: u8 = 0b1111_0000;
/// SSR1 `flags`: [C-4] bit0 = 0 zorunlu, bit1..7 reserved → bütün bayt 0 olmalı.
pub const SSR1_FLAGS_V1: u8 = 0x00;

/// SSI1 `cert_mode` (ccr §35.3 flags bit 0..1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertMode {
    /// 00 — `sender_cert_ref` (32 B).
    Ref,
    /// 01 — CompactDeviceCertV1 (152 B).
    InlineCompact,
    /// 10 — DeviceCertBundle (ccr §20; ≤ [`DEVICE_CERT_BUNDLE_MAX`]).
    Bundle,
}

impl CertMode {
    /// `flags` baytındaki kodu.
    #[must_use]
    pub const fn flags_bits(self) -> u8 {
        match self {
            Self::Ref => 0b00,
            Self::InlineCompact => 0b01,
            Self::Bundle => 0b10,
        }
    }
}

/// El sıkışma reddi. Oturum katmanı bunları DIŞARIYA ayırt edilemez biçimde (`Failed`) yansıtır
/// (S3 §3); ayrım yalnız iç gözlem ve vektör sözlüğü içindir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HandshakeError {
    #[error("ssi1 reserved flag bits set")]
    FlagsReservedNonzero,
    #[error("ssi1 init_variant reserved")]
    InitVariantReserved,
    #[error("ssi1 cert_mode reserved")]
    CertModeReserved,
    #[error("ssi1 length does not fit cert_mode")]
    LengthMismatch,
    #[error("ssi1 device cert bundle malformed")]
    BundleMalformed,
    #[error("ssi1 addressed to another recipient")]
    WrongRecipient,
    #[error("ssi1 sender not in peer directory")]
    UnknownSender,
    #[error("ssr1 flags non-zero")]
    Ssr1FlagsNonzero,
    #[error("ssr1 below minimum length")]
    Ssr1TooShort,
    #[error("x25519 public key all-zero")]
    PublicKeyAllZero,
    #[error("x25519 shared secret all-zero")]
    SharedSecretAllZero,
    #[error("aead authentication failed")]
    AeadFail,
    #[error("key derivation failed")]
    Kdf,
}

impl From<KdfError> for HandshakeError {
    fn from(_: KdfError) -> Self {
        Self::Kdf
    }
}

// ── X25519 (ccr §5.1) ────────────────────────────────────────────────────────────────────────

/// Wire'dan gelen X25519 genel anahtarı: tüm-sıfır → ret (parse/validation; ccr §5.1).
///
/// # Errors
/// [`HandshakeError::PublicKeyAllZero`].
pub fn x25519_public(bytes: [u8; X25519_LEN]) -> Result<PublicKey, HandshakeError> {
    if bytes == [0u8; X25519_LEN] {
        return Err(HandshakeError::PublicKeyAllZero);
    }
    Ok(PublicKey::from(bytes))
}

/// DH + tüm-sıfır kontrolü (ccr §5.1: HER DH çıktısı). Düşük mertebeli ama sıfır olmayan genel
/// anahtar burada yakalanır (çıktı tüm-sıfır olur); kontrol sabit zamanlıdır.
///
/// # Errors
/// [`HandshakeError::SharedSecretAllZero`].
pub fn dh(
    secret: &StaticSecret,
    public: &PublicKey,
) -> Result<Zeroizing<[u8; X25519_LEN]>, HandshakeError> {
    let shared = secret.diffie_hellman(public);
    if !shared.was_contributory() {
        return Err(HandshakeError::SharedSecretAllZero);
    }
    Ok(Zeroizing::new(shared.to_bytes()))
}

// ── session_context_v1 (ccr §35.2) ───────────────────────────────────────────────────────────

/// `pv ‖ profile ‖ suite ‖ sender_cert_ref ‖ recipient_dest_hash ‖ SHA-256(recipient_static_pub)`
/// (100 B; wire'da TAŞINMAZ).
#[must_use]
pub fn session_context_v1(
    sender_cert_ref: &[u8; CERT_REF_LEN],
    recipient_dest_hash: &[u8; DEST_HASH_LEN],
    recipient_static_pub: &PublicKey,
) -> [u8; SESSION_CONTEXT_V1_LEN] {
    let mut ctx = [0u8; SESSION_CONTEXT_V1_LEN];
    ctx[0] = PROTOCOL_VERSION_V1;
    ctx[1] = PROFILE_ID_V3_DEFAULT;
    ctx[2..4].copy_from_slice(&CIPHER_SUITE_ID_V1.to_be_bytes());
    ctx[4..36].copy_from_slice(sender_cert_ref);
    ctx[36..68].copy_from_slice(recipient_dest_hash);
    ctx[68..100].copy_from_slice(&Sha256::digest(recipient_static_pub.as_bytes()));
    ctx
}

/// `sender_cert_ref` = SHA-256(canonical_compact_device_cert) (ccr §35.2).
#[must_use]
pub fn cert_ref_of(compact_device_cert: &[u8]) -> [u8; CERT_REF_LEN] {
    Sha256::digest(compact_device_cert).into()
}

// ── AAD ──────────────────────────────────────────────────────────────────────────────────────

/// SSI1 AAD (ccr §35.3.1): `"SSI1" ‖ pv ‖ suite ‖ flags ‖ recipient_dest_hash ‖ sender_ephemeral
/// ‖ cert_field ‖ init_counter ‖ session_id_internal`. `cert_field` wire'daki baytlar AYNEN.
#[must_use]
pub fn ssi1_aad(
    flags: u8,
    recipient_dest_hash: &[u8; DEST_HASH_LEN],
    sender_ephemeral: &[u8; X25519_LEN],
    cert_field: &[u8],
    init_counter: u64,
    session_id_internal: &[u8; SESSION_ID_INTERNAL_LEN],
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(
        4 + 1 + 2 + SSI1_PREFIX_LEN + cert_field.len() + INIT_COUNTER_LEN + SESSION_ID_INTERNAL_LEN,
    );
    aad.extend_from_slice(&SSI1_AAD_MAGIC);
    aad.push(PROTOCOL_VERSION_V1);
    aad.extend_from_slice(&CIPHER_SUITE_ID_V1.to_be_bytes());
    aad.push(flags);
    aad.extend_from_slice(recipient_dest_hash);
    aad.extend_from_slice(sender_ephemeral);
    aad.extend_from_slice(cert_field);
    aad.extend_from_slice(&init_counter.to_be_bytes());
    aad.extend_from_slice(session_id_internal);
    aad
}

/// SSR1 AAD (ccr §35.5.1): `"SSR1" ‖ pv ‖ suite ‖ flags ‖ responder_ephemeral ‖ response_counter
/// ‖ session_id_internal` (epoch-1'de `session_id_internal` DEĞİŞMEZ).
#[must_use]
pub fn ssr1_aad(
    flags: u8,
    responder_ephemeral: &[u8; X25519_LEN],
    response_counter: u64,
    session_id_internal: &[u8; SESSION_ID_INTERNAL_LEN],
) -> [u8; SSR1_AAD_LEN] {
    let mut aad = [0u8; SSR1_AAD_LEN];
    aad[..4].copy_from_slice(&SSR1_AAD_MAGIC);
    aad[4] = PROTOCOL_VERSION_V1;
    aad[5..7].copy_from_slice(&CIPHER_SUITE_ID_V1.to_be_bytes());
    aad[7] = flags;
    aad[8..40].copy_from_slice(responder_ephemeral);
    aad[40..48].copy_from_slice(&response_counter.to_be_bytes());
    aad[48..].copy_from_slice(session_id_internal);
    aad
}

fn aead_seal(
    key: &Zeroizing<[u8; crate::protector::KEY_LEN]>,
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, HandshakeError> {
    ChaCha20Poly1305::new(Key::from_slice(key.as_ref()))
        .encrypt(
            Nonce::from_slice(&nonce96(0)),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| HandshakeError::AeadFail)
}

fn aead_open(
    key: &Zeroizing<[u8; crate::protector::KEY_LEN]>,
    ct_tag: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, HandshakeError> {
    ChaCha20Poly1305::new(Key::from_slice(key.as_ref()))
        .decrypt(Nonce::from_slice(&nonce96(0)), Payload { msg: ct_tag, aad })
        .map_err(|_| HandshakeError::AeadFail)
}

// ── SSI1 ayrıştırma (ccr §35.3 parser reject kuralları) ──────────────────────────────────────

/// Ayrıştırılmış SESSION_INIT (AEAD ÖNCESİ; içerik doğrulanmamıştır).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ssi1Wire<'a> {
    pub flags: u8,
    pub cert_mode: CertMode,
    pub recipient_dest_hash: [u8; DEST_HASH_LEN],
    pub sender_ephemeral: [u8; X25519_LEN],
    /// Wire'daki sertifika alanı (kipine göre 32 B / 152 B / bundle) — AAD'ye AYNEN girer.
    pub cert_field: &'a [u8],
    pub init_counter: u64,
    /// `ciphertext ‖ tag` (≥ 16 B).
    pub ct_tag: &'a [u8],
}

impl Ssi1Wire<'_> {
    /// Gönderenin kimliği: ref kipinde alanın kendisi, inline kipte SHA-256(alan), bundle kipinde
    /// SHA-256(içindeki compact cert) (ccr §35.2: "ref/inline AYNI").
    #[must_use]
    pub fn sender_cert_ref(&self) -> [u8; CERT_REF_LEN] {
        match self.cert_mode {
            CertMode::Ref => {
                let mut r = [0u8; CERT_REF_LEN];
                r.copy_from_slice(self.cert_field);
                r
            }
            CertMode::InlineCompact => cert_ref_of(self.cert_field),
            CertMode::Bundle => cert_ref_of(
                &self.cert_field
                    [BUNDLE_COMPACT_OFFSET..BUNDLE_COMPACT_OFFSET + COMPACT_DEVICE_CERT_LEN],
            ),
        }
    }
}

/// SSI1 `flags` baytı → `cert_mode` (ret sırası: reserved → init_variant → cert_mode).
///
/// # Errors
/// [`HandshakeError::FlagsReservedNonzero`] · [`HandshakeError::InitVariantReserved`] ·
/// [`HandshakeError::CertModeReserved`].
pub fn ssi1_cert_mode(flags: u8) -> Result<CertMode, HandshakeError> {
    if flags & FLAGS_RESERVED != 0 {
        return Err(HandshakeError::FlagsReservedNonzero);
    }
    if flags & FLAGS_INIT_VARIANT != 0 {
        return Err(HandshakeError::InitVariantReserved);
    }
    match flags & FLAGS_CERT_MODE {
        0b00 => Ok(CertMode::Ref),
        0b01 => Ok(CertMode::InlineCompact),
        0b10 => Ok(CertMode::Bundle),
        _ => Err(HandshakeError::CertModeReserved),
    }
}

/// DeviceCertBundle içinde compact cert'in konumu: magic(4) + version(1).
const BUNDLE_COMPACT_OFFSET: usize = 4 + 1;
/// Bundle'ın uzunluk alanlarına kadar sabit kısmı: magic + version + compact.
const BUNDLE_FIXED_HEAD: usize = BUNDLE_COMPACT_OFFSET + COMPACT_DEVICE_CERT_LEN;
const BUNDLE_LEN_FIELD: usize = 4;

/// DeviceCertBundle'ın toplam boyu (ccr §20): `magic ‖ version ‖ compact(152) ‖ id_len u32 ‖ id ‖
/// int_len u32 ‖ int ‖ device_cert_hash(32)`. Sınır önce: uzunluklar okunur, toplam ≤ 16384 ve
/// girdiye sığıyor olmalı; tahsis YOK. `device_cert_hash` = SHA-256(compact) olmalı.
///
/// # Errors
/// [`HandshakeError::BundleMalformed`].
pub fn device_cert_bundle_len(input: &[u8]) -> Result<usize, HandshakeError> {
    let bad = HandshakeError::BundleMalformed;
    let head = input.get(..BUNDLE_FIXED_HEAD).ok_or(bad)?;
    if head[..4] != DEVICE_CERT_BUNDLE_MAGIC || head[4] != DEVICE_CERT_BUNDLE_VERSION {
        return Err(bad);
    }
    let read_len = |at: usize| -> Result<usize, HandshakeError> {
        let field = input.get(at..at + BUNDLE_LEN_FIELD).ok_or(bad)?;
        let v = u32::from_be_bytes(field.try_into().map_err(|_| bad)?);
        usize::try_from(v).map_err(|_| bad)
    };
    let id_len = read_len(BUNDLE_FIXED_HEAD)?;
    if id_len > DEVICE_CERT_BUNDLE_MAX {
        return Err(bad);
    }
    let int_at = BUNDLE_FIXED_HEAD + BUNDLE_LEN_FIELD + id_len;
    let int_len = read_len(int_at)?;
    if int_len > DEVICE_CERT_BUNDLE_MAX {
        return Err(bad);
    }
    let hash_at = int_at + BUNDLE_LEN_FIELD + int_len;
    let total = hash_at + HASH_LEN;
    if total > DEVICE_CERT_BUNDLE_MAX || total > input.len() {
        return Err(bad);
    }
    let compact = &input[BUNDLE_COMPACT_OFFSET..BUNDLE_FIXED_HEAD];
    if input[hash_at..total] != cert_ref_of(compact) {
        return Err(bad);
    }
    Ok(total)
}

/// SESSION_INIT'i ayrıştırır — AEAD'ye ULAŞMADAN verilen yapısal retler (ccr §35.3 / V-SSI-5).
///
/// # Errors
/// Bayrak retleri, [`HandshakeError::LengthMismatch`], [`HandshakeError::BundleMalformed`].
pub fn parse_ssi1(wire: &[u8]) -> Result<Ssi1Wire<'_>, HandshakeError> {
    let flags = *wire.first().ok_or(HandshakeError::LengthMismatch)?;
    let cert_mode = ssi1_cert_mode(flags)?;
    let rest = wire
        .get(SSI1_PREFIX_LEN..)
        .ok_or(HandshakeError::LengthMismatch)?;
    let cert_len = match cert_mode {
        CertMode::Ref => CERT_REF_LEN,
        CertMode::InlineCompact => COMPACT_DEVICE_CERT_LEN,
        CertMode::Bundle => device_cert_bundle_len(rest)?,
    };
    let min = SSI1_PREFIX_LEN + cert_len + INIT_COUNTER_LEN + TAG_LEN;
    if wire.len() < min {
        return Err(HandshakeError::LengthMismatch);
    }
    let mut recipient_dest_hash = [0u8; DEST_HASH_LEN];
    recipient_dest_hash.copy_from_slice(&wire[1..1 + DEST_HASH_LEN]);
    let mut sender_ephemeral = [0u8; X25519_LEN];
    sender_ephemeral.copy_from_slice(&wire[1 + DEST_HASH_LEN..SSI1_PREFIX_LEN]);
    let cert_end = SSI1_PREFIX_LEN + cert_len;
    let mut counter = [0u8; INIT_COUNTER_LEN];
    counter.copy_from_slice(&wire[cert_end..cert_end + INIT_COUNTER_LEN]);
    Ok(Ssi1Wire {
        flags,
        cert_mode,
        recipient_dest_hash,
        sender_ephemeral,
        cert_field: &wire[SSI1_PREFIX_LEN..cert_end],
        init_counter: u64::from_be_bytes(counter),
        ct_tag: &wire[cert_end + INIT_COUNTER_LEN..],
    })
}

// ── SSI1 mühür / açma ────────────────────────────────────────────────────────────────────────

/// INIT'in iki tarafta da aynı türetilen sırları (ccr §35.2.1 D1 / D3).
pub struct InitSecrets {
    pub init_root: InitRoot,
    pub session_id_internal: [u8; SESSION_ID_INTERNAL_LEN],
    /// Epoch-0 trafik anahtarları (D2; rol eşlemesi çağıranın).
    pub traffic_e0: TrafficKeys,
}

impl core::fmt::Debug for InitSecrets {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Sır ve oturum kimliği Debug'a GİRMEZ.
        f.debug_struct("InitSecrets").finish_non_exhaustive()
    }
}

fn init_secrets(
    ss_e: &[u8; DH_LEN],
    ss_s: &[u8; DH_LEN],
    ctx: &[u8; SESSION_CONTEXT_V1_LEN],
) -> Result<(InitSecrets, Zeroizing<[u8; crate::protector::KEY_LEN]>), HandshakeError> {
    let init_root = kdf::d1_init_root(ss_e, ss_s, ctx)?;
    let session_id_internal = kdf::d3_session_id_internal(&init_root, ctx)?;
    let traffic_e0 = kdf::d2_traffic_e0(&init_root)?;
    let k_init = kdf::d6_k_init(&init_root)?;
    Ok((
        InitSecrets {
            init_root,
            session_id_internal,
            traffic_e0,
        },
        k_init,
    ))
}

/// Başlatanın (Alice) SSI1 girdileri.
pub struct Ssi1Seal<'a> {
    pub cert_mode: CertMode,
    pub recipient_dest_hash: &'a [u8; DEST_HASH_LEN],
    pub recipient_static_pub: &'a PublicKey,
    pub sender_static: &'a StaticSecret,
    /// Bu INIT'e özgü ephemeral (her `Initiate`'te yenisi — INV-INIT-KEY-1).
    pub sender_ephemeral: &'a StaticSecret,
    /// `session_context_v1`'e giren kimlik (inline/bundle kipinde SHA-256(compact)).
    pub sender_cert_ref: &'a [u8; CERT_REF_LEN],
    /// Wire'daki sertifika alanı (kipine göre; AAD'ye aynen girer).
    pub cert_field: &'a [u8],
    pub init_counter: u64,
    /// OPAK `early_data` (ccr §35.3 AMENDMENT-6; Crypto yorumlamaz).
    pub early_data: &'a [u8],
}

/// SSI1'i mühürler: `ss_e = DH(eph, B_static)`, `ss_s = DH(A_static, B_static)` → D1 → D3 / D6 →
/// AEAD(`k_init`, `nonce96(0)`). Döner: wire + başlatanın tuttuğu sırlar.
///
/// # Errors
/// DH çıktısı tüm-sıfır · türetim · AEAD (ChaCha20 sınırı).
pub fn seal_ssi1(p: &Ssi1Seal<'_>) -> Result<(Vec<u8>, InitSecrets), HandshakeError> {
    let eph_pub = PublicKey::from(p.sender_ephemeral);
    let ss_e = dh(p.sender_ephemeral, p.recipient_static_pub)?;
    let ss_s = dh(p.sender_static, p.recipient_static_pub)?;
    let ctx = session_context_v1(
        p.sender_cert_ref,
        p.recipient_dest_hash,
        p.recipient_static_pub,
    );
    let (secrets, k_init) = init_secrets(&ss_e, &ss_s, &ctx)?;
    let flags = p.cert_mode.flags_bits();
    let aad = ssi1_aad(
        flags,
        p.recipient_dest_hash,
        eph_pub.as_bytes(),
        p.cert_field,
        p.init_counter,
        &secrets.session_id_internal,
    );
    let ct_tag = aead_seal(&k_init, p.early_data, &aad)?;
    let mut wire =
        Vec::with_capacity(SSI1_PREFIX_LEN + p.cert_field.len() + INIT_COUNTER_LEN + ct_tag.len());
    wire.push(flags);
    wire.extend_from_slice(p.recipient_dest_hash);
    wire.extend_from_slice(eph_pub.as_bytes());
    wire.extend_from_slice(p.cert_field);
    wire.extend_from_slice(&p.init_counter.to_be_bytes());
    wire.extend_from_slice(&ct_tag);
    Ok((wire, secrets))
}

/// Açılmış SSI1 (yanıtlayan tarafı).
pub struct Ssi1Opened {
    pub init_counter: u64,
    pub sender_cert_ref: [u8; CERT_REF_LEN],
    pub sender_ephemeral: PublicKey,
    pub sender_static: PublicKey,
    pub secrets: InitSecrets,
    /// OPAK `early_data`.
    pub early_data: Vec<u8>,
}

impl core::fmt::Debug for Ssi1Opened {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ssi1Opened")
            .field("init_counter", &self.init_counter)
            .finish_non_exhaustive()
    }
}

/// Yanıtlayanın (Bob) SSI1 açması. Sıra: ayrıştırma → alıcı = biz → `sender_ephemeral` tüm-sıfır
/// → gönderen dizinde (`resolve_sender(cert_ref)`; yoksa DH YAPILMAZ) → `ss_e`, `ss_s` → D1 →
/// AEAD. `resolve_sender` ccr §35.4 KAPALI-SİSTEM KİPİ'nin eş dizinidir (G1).
///
/// # Errors
/// Ayrıştırma retleri · [`HandshakeError::WrongRecipient`] · [`HandshakeError::PublicKeyAllZero`]
/// · [`HandshakeError::UnknownSender`] · [`HandshakeError::SharedSecretAllZero`] ·
/// [`HandshakeError::AeadFail`].
pub fn open_ssi1(
    wire: &[u8],
    own_static: &StaticSecret,
    own_dest_hash: &[u8; DEST_HASH_LEN],
    resolve_sender: impl FnOnce(&[u8; CERT_REF_LEN]) -> Option<PublicKey>,
) -> Result<Ssi1Opened, HandshakeError> {
    let parsed = parse_ssi1(wire)?;
    if parsed.recipient_dest_hash != *own_dest_hash {
        return Err(HandshakeError::WrongRecipient);
    }
    let sender_ephemeral = x25519_public(parsed.sender_ephemeral)?;
    let sender_cert_ref = parsed.sender_cert_ref();
    let sender_static = resolve_sender(&sender_cert_ref).ok_or(HandshakeError::UnknownSender)?;
    let ss_e = dh(own_static, &sender_ephemeral)?;
    let ss_s = dh(own_static, &sender_static)?;
    let own_pub = PublicKey::from(own_static);
    let ctx = session_context_v1(&sender_cert_ref, own_dest_hash, &own_pub);
    let (secrets, k_init) = init_secrets(&ss_e, &ss_s, &ctx)?;
    let aad = ssi1_aad(
        parsed.flags,
        &parsed.recipient_dest_hash,
        &parsed.sender_ephemeral,
        parsed.cert_field,
        parsed.init_counter,
        &secrets.session_id_internal,
    );
    let early_data = aead_open(&k_init, parsed.ct_tag, &aad)?;
    Ok(Ssi1Opened {
        init_counter: parsed.init_counter,
        sender_cert_ref,
        sender_ephemeral,
        sender_static,
        secrets,
        early_data,
    })
}

// ── SSR1 (ccr §35.5 / §35.5.1) ───────────────────────────────────────────────────────────────

/// Epoch-1 sırları (ccr §35.2.1 D4 / D5).
pub struct Epoch1Secrets {
    pub epoch_1_root: Epoch1Root,
    pub traffic_e1: TrafficKeys,
}

impl core::fmt::Debug for Epoch1Secrets {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Epoch1Secrets").finish_non_exhaustive()
    }
}

fn epoch1_secrets(
    init_root: &InitRoot,
    dh_ee: &[u8; DH_LEN],
    dh_se: &[u8; DH_LEN],
) -> Result<(Epoch1Secrets, Zeroizing<[u8; crate::protector::KEY_LEN]>), HandshakeError> {
    let epoch_1_root = kdf::d4_epoch_1_root(init_root, dh_ee, dh_se)?;
    let traffic_e1 = kdf::d5_traffic_e1(&epoch_1_root)?;
    let k_resp = kdf::d7_k_resp(&epoch_1_root)?;
    Ok((
        Epoch1Secrets {
            epoch_1_root,
            traffic_e1,
        },
        k_resp,
    ))
}

/// Yanıtlayanın (Bob) SSR1 girdileri.
pub struct Ssr1Seal<'a> {
    pub init_root: &'a InitRoot,
    pub session_id_internal: &'a [u8; SESSION_ID_INTERNAL_LEN],
    pub initiator_ephemeral: &'a PublicKey,
    pub initiator_static: &'a PublicKey,
    /// Bu RESPONSE'a özgü ephemeral (iki DH'de kullanılır, ardından silinir).
    pub responder_ephemeral: &'a StaticSecret,
    pub response_counter: u64,
    /// OPAK yanıt verisi.
    pub response_data: &'a [u8],
}

/// SSR1'i mühürler: `ss_e2 = DH(resp_eph, init_eph) ‖ DH(resp_eph, init_static)` → D4 → D5 / D7 →
/// AEAD(`k_resp`, `nonce96(0)`). Döner: wire + epoch-1 sırları.
///
/// # Errors
/// DH çıktısı tüm-sıfır · türetim · AEAD.
pub fn seal_ssr1(p: &Ssr1Seal<'_>) -> Result<(Vec<u8>, Epoch1Secrets), HandshakeError> {
    let eph_pub = PublicKey::from(p.responder_ephemeral);
    let dh_ee = dh(p.responder_ephemeral, p.initiator_ephemeral)?;
    let dh_se = dh(p.responder_ephemeral, p.initiator_static)?;
    let (secrets, k_resp) = epoch1_secrets(p.init_root, &dh_ee, &dh_se)?;
    let aad = ssr1_aad(
        SSR1_FLAGS_V1,
        eph_pub.as_bytes(),
        p.response_counter,
        p.session_id_internal,
    );
    let ct_tag = aead_seal(&k_resp, p.response_data, &aad)?;
    let mut wire = Vec::with_capacity(SSR1_PREFIX_LEN + ct_tag.len());
    wire.push(SSR1_FLAGS_V1);
    wire.extend_from_slice(eph_pub.as_bytes());
    wire.extend_from_slice(&p.response_counter.to_be_bytes());
    wire.extend_from_slice(&ct_tag);
    Ok((wire, secrets))
}

/// Açılmış SSR1 (başlatan tarafı).
pub struct Ssr1Opened {
    pub response_counter: u64,
    pub secrets: Epoch1Secrets,
    /// OPAK yanıt verisi.
    pub response_data: Vec<u8>,
}

impl core::fmt::Debug for Ssr1Opened {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ssr1Opened")
            .field("response_counter", &self.response_counter)
            .finish_non_exhaustive()
    }
}

/// Başlatanın (Alice) bir aday bekleyen INIT'e karşı SSR1 açması. Sıra: uzunluk → `flags` = 0
/// ([C-4]) → `responder_ephemeral` tüm-sıfır → `dh_ee`, `dh_se` tüm-sıfır → D4 → AEAD.
/// Yanlış aday ile bozuk paket AYNI hatayı verir (AEAD) — ayırt edici yok (S3 K7).
///
/// # Errors
/// [`HandshakeError::Ssr1TooShort`] · [`HandshakeError::Ssr1FlagsNonzero`] ·
/// [`HandshakeError::PublicKeyAllZero`] · [`HandshakeError::SharedSecretAllZero`] ·
/// [`HandshakeError::AeadFail`].
pub fn open_ssr1(
    wire: &[u8],
    init_root: &InitRoot,
    session_id_internal: &[u8; SESSION_ID_INTERNAL_LEN],
    own_ephemeral: &StaticSecret,
    own_static: &StaticSecret,
) -> Result<Ssr1Opened, HandshakeError> {
    if wire.len() < SSR1_MIN_LEN {
        return Err(HandshakeError::Ssr1TooShort);
    }
    if wire[0] != SSR1_FLAGS_V1 {
        return Err(HandshakeError::Ssr1FlagsNonzero);
    }
    let mut eph = [0u8; X25519_LEN];
    eph.copy_from_slice(&wire[1..1 + X25519_LEN]);
    let responder_ephemeral = x25519_public(eph)?;
    let mut counter = [0u8; RESPONSE_COUNTER_LEN];
    counter.copy_from_slice(&wire[1 + X25519_LEN..SSR1_PREFIX_LEN]);
    let response_counter = u64::from_be_bytes(counter);
    let dh_ee = dh(own_ephemeral, &responder_ephemeral)?;
    let dh_se = dh(own_static, &responder_ephemeral)?;
    let (secrets, k_resp) = epoch1_secrets(init_root, &dh_ee, &dh_se)?;
    let aad = ssr1_aad(wire[0], &eph, response_counter, session_id_internal);
    let response_data = aead_open(&k_resp, &wire[SSR1_PREFIX_LEN..], &aad)?;
    Ok(Ssr1Opened {
        response_counter,
        secrets,
        response_data,
    })
}
