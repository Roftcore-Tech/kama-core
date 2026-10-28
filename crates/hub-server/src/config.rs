//! `hub.json` (KM-HUB-TDD-001 §6; kama-provision v1 üretir) ve S4 §6 sabitleri ([`Limits`]).
//!
//! ```json
//! { "format_version": 1,
//!   "listen": "0.0.0.0:47000",
//!   "hub_ed25519": { "priv": "<64 hex>", "pub": "<64 hex>" },
//!   "devices": [ { "name": "alice", "dest_hash": "<64 hex>", "cert": "<204 base64>" } ] }
//! ```
//!
//! Okuma KATIDIR — bilinmeyen ya da eksik alan, `format_version ≠ 1`, `listen` soket adresi değil, hex
//! tam 64 küçük harf değil, `pub` `priv`'den türeyen açık anahtar değil, `cert` kanonik base64 152 B
//! değil, sertifikadaki cihaz Ed25519 anahtarı katı kipte kullanılamaz, aynı `dest_hash` ya da aynı
//! `cert` iki kez → hub BAŞLAMAZ (T-C1). Dosya okunamazsa [`ConfigError::Unreadable`] (T-C3).
//!
//! Dosyadaki TEK gizli alan `hub_ed25519.priv`'dir (T-C2; S4 §1 madde 10): şemada telefonlara ait gizli
//! alan yoktur ve bilinmeyen alan reddedildiği için böyle bir alan taşıyan dosya okunmaz. [`HubSecret`]
//! `Debug`'da baytlarını basmaz; hata metinleri dosyadan içerik taşımaz.
//!
//! Türetilenler (açılışta): `cert_ref = SHA-256(cert)` (`kama_noise_wire::handshake::cert_ref_of`),
//! `device_ed25519_pub = cert[0..32]`.

use core::fmt;
use core::time::Duration;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use kama_noise_wire::ed25519_strict::{verify_strict, Ed25519Reject};
use kama_noise_wire::handshake::{cert_ref_of, COMPACT_DEVICE_CERT_LEN};
use serde::Deserialize;

use crate::rate::Rate;
use crate::wire::{CertRef, DestHash, DEST_HASH_LEN, MAX_DATAGRAM, PUBLIC_KEY_LEN, SIGNATURE_LEN};

/// Bu sürümün okuduğu `format_version`.
pub const FORMAT_VERSION: u32 = 1;

/// S4 §6 sabitleri. Üretim [`Limits::S4`] ile koşar; başka değerler yalnız testler içindir (`hub.json`
/// sınır taşımaz).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// `MAX_DATAGRAM`.
    pub max_datagram: usize,
    /// `MAX_LINKS` (küresel).
    pub max_links: usize,
    /// `MAX_LINKS_PER_ENDPOINT` (her hat iki uçta sayılır).
    pub max_links_per_endpoint: usize,
    /// `MAX_TOMBSTONES`.
    pub max_tombstones: usize,
    /// `HANDLE_TOMBSTONE`.
    pub handle_tombstone: Duration,
    /// `COOKIE_ROTATE`.
    pub cookie_rotate: Duration,
    /// `VERIFY_BUDGET` (küresel).
    pub verify_budget: Rate,
    /// `BIND_FAIL_MAX` / `BIND_FAIL_HOLD` / `BIND_FAIL_ENTRIES`.
    pub bind_fail_max: u32,
    pub bind_fail_hold: Duration,
    pub bind_fail_entries: usize,
    /// `STORE_RATE` (uç başına).
    pub store_rate: Rate,
    /// `LINK_ALLOC_RATE` (uç başına).
    pub link_alloc_rate: Rate,
    /// `ROUTE_MISS_RATE` (uç başına).
    pub route_miss_rate: Rate,
    /// `STATS_PERIOD`.
    pub stats_period: Duration,
    /// Süresi dolan tombstone / kilit girdilerinin temizlik aralığı (TDD §3 `tick(every 60 s)`).
    pub sweep_period: Duration,
}

impl Limits {
    /// S4 v1.3 §6.
    pub const S4: Self = Self {
        max_datagram: MAX_DATAGRAM,
        max_links: 8192,
        max_links_per_endpoint: 64,
        max_tombstones: 65_536,
        handle_tombstone: Duration::from_secs(7 * 24 * 3600),
        cookie_rotate: Duration::from_secs(3600),
        verify_budget: Rate::new(2000, 2000),
        bind_fail_max: 10,
        bind_fail_hold: Duration::from_secs(60),
        bind_fail_entries: 4096,
        store_rate: Rate::new(10, 20),
        link_alloc_rate: Rate::new(1, 10),
        // S4 §6 patlama vermiyor; 1/s = en çok bir jeton.
        route_miss_rate: Rate::new(1, 1),
        stats_period: Duration::from_secs(10),
        sweep_period: Duration::from_secs(60),
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::S4
    }
}

/// Hub'ın Ed25519 gizli anahtarı (32 baytlık tohum). `Debug` baytları basmaz; Drop'ta sıfırlanır.
#[derive(Clone)]
pub struct HubSecret([u8; 32]);

impl HubSecret {
    /// İmza anahtarı (`ed25519-dalek` kopyayı Drop'ta sıfırlar).
    #[must_use]
    pub fn signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.0)
    }
}

impl fmt::Debug for HubSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HubSecret(<redacted>)")
    }
}

impl Drop for HubSecret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

/// Dizindeki bir cihaz. Gizli alanı YOKTUR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub dest_hash: DestHash,
    pub cert_ref: CertRef,
    pub ed25519_pub: [u8; PUBLIC_KEY_LEN],
}

/// Doğrulanmış yapılandırma.
#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub hub_pub: [u8; PUBLIC_KEY_LEN],
    pub hub_secret: HubSecret,
    pub devices: Vec<Device>,
}

/// Yapılandırma hatası; dosya içeriği taşımaz.
#[derive(Debug)]
pub enum ConfigError {
    /// Dosya okunamadı (yok, izin yok, UTF-8 değil).
    Unreadable(PathBuf),
    /// JSON değil, eksik ya da bilinmeyen alan; yalnız kategori ve konum.
    Json {
        category: &'static str,
        line: usize,
        column: usize,
    },
    /// Alan değeri geçersiz.
    Invalid(&'static str),
    /// `devices[index]` geçersiz.
    InvalidDevice { index: usize, what: &'static str },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(path) => write!(f, "config unreadable: {}", path.display()),
            Self::Json {
                category,
                line,
                column,
            } => {
                write!(
                    f,
                    "config invalid: {category} error at line {line} column {column}"
                )
            }
            Self::Invalid(what) => write!(f, "config invalid: {what}"),
            Self::InvalidDevice { index, what } => {
                write!(f, "config invalid: devices[{index}]: {what}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    format_version: u32,
    listen: String,
    hub_ed25519: RawKeyPair,
    devices: Vec<RawDevice>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKeyPair {
    #[serde(rename = "priv")]
    secret: String,
    #[serde(rename = "pub")]
    public: String,
}

impl Drop for RawKeyPair {
    fn drop(&mut self) {
        // En iyi çaba: gizli anahtarın hex metni ayrıştırma bitince sıfırlanır.
        let mut secret = core::mem::take(&mut self.secret).into_bytes();
        secret.fill(0);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDevice {
    name: String,
    dest_hash: String,
    cert: String,
}

/// `path`'teki `hub.json`'ı okur ve doğrular.
///
/// # Errors
/// [`ConfigError::Unreadable`] — dosya okunamadı; diğerleri modül belgesindeki kurallar.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    // Yalnız G/Ç hatası "unreadable"dır (Deploy T-DEP-6b bu satırı izin sorunu kanıtı sayar); okunmuş
    // ama UTF-8 olmayan dosya biçim hatasıdır.
    let mut bytes = std::fs::read(path).map_err(|_| ConfigError::Unreadable(path.to_path_buf()))?;
    let parsed = match core::str::from_utf8(&bytes) {
        Ok(text) => parse(text),
        Err(_) => Err(ConfigError::Invalid("not UTF-8")),
    };
    bytes.fill(0);
    parsed
}

/// `hub.json` metnini çözer ve doğrular.
///
/// # Errors
/// [`ConfigError`] — modül belgesindeki kurallardan biri düştü.
pub fn parse(text: &str) -> Result<Config, ConfigError> {
    let raw: RawConfig = serde_json::from_str(text).map_err(|error| ConfigError::Json {
        category: match error.classify() {
            serde_json::error::Category::Io => "io",
            serde_json::error::Category::Syntax => "syntax",
            serde_json::error::Category::Data => "data",
            serde_json::error::Category::Eof => "eof",
        },
        line: error.line(),
        column: error.column(),
    })?;
    if raw.format_version != FORMAT_VERSION {
        return Err(ConfigError::Invalid("format_version must be 1"));
    }
    let listen: SocketAddr = raw
        .listen
        .parse()
        .map_err(|_| ConfigError::Invalid("listen must be ip:port"))?;
    let hub_secret = HubSecret(
        hex_decode(&raw.hub_ed25519.secret).ok_or(ConfigError::Invalid(
            "hub_ed25519.priv must be 64 lowercase hex",
        ))?,
    );
    let hub_pub: [u8; PUBLIC_KEY_LEN] = hex_decode(&raw.hub_ed25519.public).ok_or(
        ConfigError::Invalid("hub_ed25519.pub must be 64 lowercase hex"),
    )?;
    if hub_secret.signing_key().verifying_key().to_bytes() != hub_pub {
        return Err(ConfigError::Invalid(
            "hub_ed25519.pub is not the public key of hub_ed25519.priv",
        ));
    }

    let mut devices = Vec::with_capacity(raw.devices.len());
    let mut seen_dest = HashSet::with_capacity(raw.devices.len());
    let mut seen_cert = HashSet::with_capacity(raw.devices.len());
    for (index, device) in raw.devices.iter().enumerate() {
        let invalid = |what| ConfigError::InvalidDevice { index, what };
        if device.name.is_empty() {
            return Err(invalid("name must not be empty"));
        }
        let dest_hash: DestHash = hex_decode::<DEST_HASH_LEN>(&device.dest_hash)
            .ok_or_else(|| invalid("dest_hash must be 64 lowercase hex"))?;
        let cert: [u8; COMPACT_DEVICE_CERT_LEN] = base64_decode(&device.cert)
            .ok_or_else(|| invalid("cert must be canonical base64 of 152 bytes"))?;
        let mut ed25519_pub = [0u8; PUBLIC_KEY_LEN];
        for (slot, byte) in ed25519_pub.iter_mut().zip(&cert) {
            *slot = *byte;
        }
        if !usable_public_key(&ed25519_pub) {
            return Err(invalid("cert does not carry a usable device Ed25519 key"));
        }
        let cert_ref = cert_ref_of(&cert);
        if !seen_dest.insert(dest_hash) {
            return Err(invalid("duplicate dest_hash"));
        }
        if !seen_cert.insert(cert_ref) {
            return Err(invalid("duplicate cert"));
        }
        devices.push(Device {
            name: device.name.clone(),
            dest_hash,
            cert_ref,
            ed25519_pub,
        });
    }
    Ok(Config {
        listen,
        hub_pub,
        hub_secret,
        devices,
    })
}

/// Açık anahtar katı Ed25519 kipinde kullanılabilir mi. Kurallar kopyalanmaz: çalışma zamanındaki TEK
/// doğrulama yoluna (`verify_strict`) sıfır imzayla sorulur. Sıfır imzanın `R`'si kanonik ama küçük
/// mertebelidir; anahtar sağlamsa ret gerekçesi anahtarla ilgili OLMAZ.
fn usable_public_key(public_key: &[u8; PUBLIC_KEY_LEN]) -> bool {
    !matches!(
        verify_strict(public_key, &[], &[0u8; SIGNATURE_LEN]),
        Err(Ed25519Reject::Length
            | Ed25519Reject::NonCanonicalPointEncoding
            | Ed25519Reject::PointDecoding
            | Ed25519Reject::SmallOrderPublicKey)
    )
}

/// Tam `2 * N` küçük harf hex (kama-provision kuralı). Hata içerik taşımaz.
pub(crate) fn hex_decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != N.checked_mul(2)? {
        return None;
    }
    let mut out = [0u8; N];
    for (slot, pair) in out.iter_mut().zip(digits.chunks_exact(2)) {
        let [high, low] = pair else { return None };
        *slot = (hex_value(*high)? << 4) | hex_value(*low)?;
    }
    Some(out)
}

fn hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => digit.checked_sub(b'0'),
        b'a'..=b'f' => digit.checked_sub(b'a')?.checked_add(10),
        _ => None,
    }
}

/// RFC 4648 §4 kanonik base64 (standart alfabe, `=` dolgulu, kullanılmayan bitler sıfır), tam `N` bayt.
pub(crate) fn base64_decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    let chars = text.as_bytes();
    if chars.len() != N.div_ceil(3).checked_mul(4)? {
        return None;
    }
    let mut out = [0u8; N];
    let mut written = 0usize;
    let groups = chars.len() / 4;
    for (index, group) in chars.chunks_exact(4).enumerate() {
        // Dolgu yalnız SON grupta olabilir.
        if index + 1 < groups && group.contains(&b'=') {
            return None;
        }
        let mut bits: u32 = 0;
        let mut data_chars = 0usize;
        for (position, &c) in group.iter().enumerate() {
            let value = match c {
                b'A'..=b'Z' => c - b'A',
                b'a'..=b'z' => c - b'a' + 26,
                b'0'..=b'9' => c - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => {
                    // Dolgu yalnız grubun son bir-iki karakteri olabilir.
                    if position < 2 {
                        return None;
                    }
                    bits <<= 6;
                    continue;
                }
                _ => return None,
            };
            if data_chars != position {
                return None; // dolgudan sonra veri
            }
            data_chars += 1;
            bits = (bits << 6) | u32::from(value);
        }
        let group_bytes = data_chars.checked_sub(1)?;
        let bytes = bits.to_be_bytes();
        for &byte in bytes.get(1..1 + group_bytes)? {
            *out.get_mut(written)? = byte;
            written += 1;
        }
        // Kanoniklik: kullanılmayan bitler sıfır.
        let unused = bytes.get(1 + group_bytes..).unwrap_or(&[]);
        if unused.iter().any(|&byte| byte != 0) {
            return None;
        }
    }
    (written == N).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let mut padded = [0u8; 3];
            padded[..chunk.len()].copy_from_slice(chunk);
            let n = u32::from_be_bytes([0, padded[0], padded[1], padded[2]]);
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn base64_is_canonical_and_exact() {
        for n in [1usize, 2, 3, 4, 5, 152] {
            let bytes: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            let text = b64(&bytes);
            match n {
                1 => assert_eq!(base64_decode::<1>(&text).map(|a| a.to_vec()), Some(bytes)),
                2 => assert_eq!(base64_decode::<2>(&text).map(|a| a.to_vec()), Some(bytes)),
                3 => assert_eq!(base64_decode::<3>(&text).map(|a| a.to_vec()), Some(bytes)),
                4 => assert_eq!(base64_decode::<4>(&text).map(|a| a.to_vec()), Some(bytes)),
                5 => assert_eq!(base64_decode::<5>(&text).map(|a| a.to_vec()), Some(bytes)),
                _ => assert_eq!(base64_decode::<152>(&text).map(|a| a.to_vec()), Some(bytes)),
            }
        }
        assert_eq!(base64_decode::<1>("AQ=="), Some([1]));
        assert_eq!(
            base64_decode::<1>("AR=="),
            None,
            "kullanılmayan bit sıfır değil"
        );
        assert_eq!(base64_decode::<1>("AQ="), None, "boy");
        assert_eq!(base64_decode::<2>("A=Q="), None, "dolgudan sonra veri");
        assert_eq!(base64_decode::<1>("AQ-="), None, "alfabe dışı");
        assert_eq!(base64_decode::<3>("AQID"), Some([1, 2, 3]));
        assert_eq!(base64_decode::<2>("AQID"), None, "uzunluk N'yi tutmuyor");
        assert_eq!(base64_decode::<4>("AQ==AQID"), None, "dolgu ortada");
    }

    #[test]
    fn hex_is_lowercase_exact_length_only() {
        assert_eq!(hex_decode::<2>("00ff"), Some([0x00, 0xff]));
        assert_eq!(hex_decode::<2>("00FF"), None);
        assert_eq!(hex_decode::<2>("00f"), None);
        assert_eq!(hex_decode::<2>("0xff"), None);
    }

    #[test]
    fn public_key_check_follows_the_strict_verification_rules() {
        let good = SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes();
        assert!(usable_public_key(&good));
        assert_eq!(
            verify_strict(&good, &[], &[0u8; SIGNATURE_LEN]),
            Err(Ed25519Reject::SmallOrderR),
            "yöntemin dayanağı: sağlam anahtarda ret R'den gelir"
        );
        assert!(!usable_public_key(&[0xff; 32]), "kanonik değil");
        let mut identity = [0u8; 32];
        identity[0] = 1;
        assert!(!usable_public_key(&identity), "küçük mertebeli");
        let mut off_curve = [0u8; 32];
        off_curve[0] = 2;
        assert!(!usable_public_key(&off_curve), "eğri dışı");
    }

    #[test]
    fn secret_debug_is_redacted() {
        let shown = format!("{:?}", HubSecret([0xab; 32]));
        assert!(!shown.contains("ab") && !shown.contains("171"), "{shown}");
    }
}
