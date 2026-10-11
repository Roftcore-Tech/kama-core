//! PROVISIONING-FORMAT-v0 tipleri (serde) ve doğrulayan yükleyici
//! (`docs/provisioning/PROVISIONING-FORMAT-v0.md` §3, §5).
//!
//! K4G-11 bileşim kökü cihaz dosyasını [`load_device_config`] ile okur ve
//! [`DeviceConfig::local_identity`] + [`DeviceConfig::peer_directory`] ile `kama-noise-wire`'ın ürün
//! kurulumuna (`ControlConfig`) bağlar. Kimlik, anahtarlar ve eş dizini bellekte kalır (D8).
//!
//! * Gizli anahtar alanı yalnız [`Secret32`]'dir: Debug'ta maskeli, `Clone` değil, öbekte durur ve
//!   Drop'ta sıfırlanır; yalnız ödünç alınan kaynaktan kurulur.
//! * [`DeviceConfig`]'te kök gizli anahtarı için alan YOKTUR (D9) — yalnız `root_pub`.
//! * Bütün nesneler bilinmeyen alanı reddeder; JSON hataları girdiden içerik taşımaz (yalnız kategori
//!   ve satır/sütun).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;

use ed25519_dalek::SigningKey;
use kama_noise_wire::handshake::{cert_ref_of, COMPACT_DEVICE_CERT_LEN};
use kama_noise_wire::peer_dir::{DirectoryError, LocalIdentity, PeerDirectory, PeerEntry};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::cert::{self, split_compact_device_cert, KEY_LEN};
use crate::encoding::{
    base64_decode, base64_encode, hex_decode, hex_decode_into, hex_encode, DecodeError,
};

/// `format_version` (v0; göç yok).
pub const FORMAT_VERSION: u32 = 0;
/// Kök dosyası — YALNIZ provisioning otoritesinde kalır (D9).
pub const ROOT_FILE_NAME: &str = "root.json";
/// Çıktı dizinine ilk yazılan dosya (§2).
pub const GITIGNORE_FILE_NAME: &str = ".gitignore";
/// `.gitignore` içeriği: dizindeki her şey (kendisi dahil) Git dışında.
pub const GITIGNORE_CONTENT: &str = "*\n";
/// Cihaz sayısı alt sınırı (§7): tek cihazın eşi olmaz.
pub const MIN_DEVICES: usize = 2;
/// Cihaz sayısı üst sınırı (§7; uygulama sınırı, protokol sınırı değil).
pub const MAX_DEVICES: usize = 64;
/// Cihaz adının en çok uzunluğu (§7).
pub const MAX_NAME_LEN: usize = 32;
/// Yükleyicinin kabul ettiği en büyük dosya (§7): 64 cihazlık en büyük geçerli v0 dosyası < 48 KiB.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

// ── Değer tipleri ────────────────────────────────────────────────────────────────────────────

/// Cihaz adı geçersiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("device name is empty")]
    Empty,
    #[error("device name is longer than 32 characters")]
    TooLong,
    #[error("device name must match [a-z0-9][a-z0-9_-]*")]
    Charset,
}

/// Cihaz adı `[a-z0-9][a-z0-9_-]{0,31}` (§2, §7): dosya adına güvenle girer (yol ayırıcı, nokta,
/// büyük harf yok). `device_id`, `peers[].name` ve `direct_addrs` anahtarı bu tiptedir.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeviceName(String);

impl DeviceName {
    /// # Errors
    /// [`NameError`].
    pub fn new(name: &str) -> Result<Self, NameError> {
        let bytes = name.as_bytes();
        let Some((&first, rest)) = bytes.split_first() else {
            return Err(NameError::Empty);
        };
        if bytes.len() > MAX_NAME_LEN {
            return Err(NameError::TooLong);
        }
        let first_ok = first.is_ascii_lowercase() || first.is_ascii_digit();
        let rest_ok = rest
            .iter()
            .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
        if !(first_ok && rest_ok) {
            return Err(NameError::Charset);
        }
        Ok(Self(name.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `device-<ad>.json` (§3).
    #[must_use]
    pub fn file_name(&self) -> String {
        let name = &self.0;
        format!("device-{name}.json")
    }
}

impl TryFrom<String> for DeviceName {
    type Error = NameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<DeviceName> for String {
    fn from(name: DeviceName) -> Self {
        name.0
    }
}

impl fmt::Display for DeviceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 32 B açık değer (açık anahtar, `dest_hash`) — JSON'da 64 karakter küçük harf hex. Debug değeri
/// basmaz: kimlik özetleri log'a düşmez (`PeerEntry` / `PeerRef` ile aynı kural).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Public32([u8; KEY_LEN]);

impl Public32 {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        hex_encode(&self.0)
    }
}

impl TryFrom<String> for Public32 {
    type Error = DecodeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        hex_decode::<KEY_LEN>(&value).map(Self)
    }
}

impl From<Public32> for String {
    fn from(value: Public32) -> Self {
        value.to_hex()
    }
}

impl fmt::Debug for Public32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Public32(..)")
    }
}

/// 32 B gizli anahtar (X25519 skaler baytları ya da Ed25519 tohumu) — JSON'da 64 karakter hex.
/// Debug'ta MASKELİDİR, `Clone` DEĞİLDİR; baytlar ÖBEKTE durur (değerin taşınması yalnız işaretçiyi
/// kopyalar) ve Drop'ta sıfırlanır. Yalnız ödünç alınan kaynaktan kurulur ([`Secret32::copy_from`];
/// JSON'dan doğrudan öbek tamponuna çözülür); erişim yalnız ödünçtür ([`Secret32::expose`]).
///
/// **Kalan kopyalar (önlenemez, belgelidir):** `x25519_dalek::StaticSecret::from` ve
/// `LocalIdentity::new` diziyi DEĞERLE ister; dalek tipleri (`StaticSecret`, `SigningKey`) yığında
/// taşınır ve Rust'ta taşınan değerin eski yeri sıfırlanmaz. Bu kopyalar `zeroize`'ın da belgelediği
/// dil sınırıdır; gizli anahtarlar zaten düz JSON'da diskte durduğu için (kabul edilmiş PoC riski, D9)
/// bellek kopyaları riski büyütmez.
pub struct Secret32(Box<[u8; KEY_LEN]>);

impl Secret32 {
    /// Ödünç alınan gizli baytları DOĞRUDAN öbekteki tampona kopyalar (çağıranda ek dizi kopyası yok).
    #[must_use]
    pub fn copy_from(secret: &[u8; KEY_LEN]) -> Self {
        let mut boxed = Self::zeroed();
        boxed.0.copy_from_slice(secret);
        boxed
    }

    fn zeroed() -> Self {
        Self(Box::new([0u8; KEY_LEN]))
    }

    /// Gizli baytlar — yalnız anahtar kurmak için; log'a, Debug'a, standart çıktıya verilmez.
    #[must_use]
    pub fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl Drop for Secret32 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Secret32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret32(<redacted>)")
    }
}

impl Serialize for Secret32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text = Zeroizing::new(hex_encode(self.expose()));
        serializer.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for Secret32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = Zeroizing::new(String::deserialize(deserializer)?);
        // Hex doğrudan öbek tamponuna çözülür; hata yolunda tampon Drop'ta sıfırlanır.
        let mut secret = Self::zeroed();
        match hex_decode_into(&text, &mut secret.0[..]) {
            Ok(()) => Ok(secret),
            Err(error) => Err(serde::de::Error::custom(error)),
        }
    }
}

/// CompactDeviceCertV1 (152 B; ccr §13.1) — JSON'da base64 (§4.1). Debug içerik basmaz.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CertBytes([u8; COMPACT_DEVICE_CERT_LEN]);

impl CertBytes {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; COMPACT_DEVICE_CERT_LEN]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8; COMPACT_DEVICE_CERT_LEN] {
        &self.0
    }

    /// `cert_ref` = SHA-256(cert) (ccr §35.2 `sender_cert_ref`; INIT `cert_mode = ref`, D9).
    #[must_use]
    pub fn cert_ref(&self) -> [u8; KEY_LEN] {
        cert_ref_of(&self.0)
    }
}

impl TryFrom<String> for CertBytes {
    type Error = DecodeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        base64_decode::<COMPACT_DEVICE_CERT_LEN>(&value).map(Self)
    }
}

impl From<CertBytes> for String {
    fn from(value: CertBytes) -> Self {
        base64_encode(&value.0)
    }
}

impl fmt::Debug for CertBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CompactDeviceCert(152 B)")
    }
}

// ── Dosya tipleri ────────────────────────────────────────────────────────────────────────────

/// `{ "priv": hex, "pub": hex }`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPair {
    #[serde(rename = "priv")]
    pub secret: Secret32,
    #[serde(rename = "pub")]
    pub public: Public32,
}

/// `root.json` (§3.1) — YALNIZ provisioning otoritesinin eseri; cihaza aktarılmaz (D9).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootFile {
    pub format_version: u32,
    /// Unix saniyesi (UTC); cihaz sertifikalarının `not_after`'ı buradan türer (§4.3).
    pub created_at_unix: u64,
    pub ed25519: KeyPair,
}

/// `device-<ad>.json` (§3.2). Kök gizli anahtarı için alan YOKTUR (D9).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceConfig {
    pub format_version: u32,
    pub device_id: DeviceName,
    /// Kimlik Ed25519 çifti — cihaz anahtarlarından AYRI (plan K4G-12 Ek (3)); `priv` PoC'de çalışma
    /// zamanında kullanılmaz.
    pub identity_ed25519: KeyPair,
    /// `SHA-256(identity_ed25519.pub)` (ccr §13.3 / §13.5).
    pub identity_hash: Public32,
    /// `SHA-256(identity_hash)` (ccr §13.5).
    pub dest_hash: Public32,
    pub x25519_static: KeyPair,
    pub ed25519: KeyPair,
    pub cert: CertBytes,
    pub root_pub: Public32,
    pub peers: Vec<PeerConfig>,
    pub hub: Option<HubConfig>,
    pub direct_addrs: BTreeMap<DeviceName, SocketAddr>,
}

/// `peers[]` kaydı (§3.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerConfig {
    pub name: DeviceName,
    pub identity_hash: Public32,
    pub dest_hash: Public32,
    pub x25519_pub: Public32,
    pub cert: CertBytes,
}

/// `hub` (§4.7). v0'da `x25519_pub` daima `null` yazılır; ham hub anahtarı otorite DEĞİLDİR (CLAUDE.md
/// §4 cert-bound) ve v0 tüketicileri bu alanı kullanmaz.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HubConfig {
    pub addr: String,
    pub x25519_pub: Option<Public32>,
}

// ── Yükleyici ve doğrulama ───────────────────────────────────────────────────────────────────

/// Yükleme / doğrulama hatası. Girdiden içerik taşımaz.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("provisioning file exceeds the size limit")]
    TooLarge,
    #[error("json {category} error at line {line}, column {column}")]
    Json {
        category: &'static str,
        line: usize,
        column: usize,
    },
    #[error("unsupported format_version {0}")]
    UnsupportedVersion(u32),
    #[error("invalid provisioning file: {0}")]
    Invalid(&'static str),
}

impl FormatError {
    /// `serde_json` hatasının mesajı girdiden değer alıntılayabilir; yalnız yeri ve sınıfı tutulur.
    fn json(error: serde_json::Error) -> Self {
        let category = match error.classify() {
            serde_json::error::Category::Io => "i/o",
            serde_json::error::Category::Syntax => "syntax",
            serde_json::error::Category::Data => "data",
            serde_json::error::Category::Eof => "eof",
        };
        Self::Json {
            category,
            line: error.line(),
            column: error.column(),
        }
    }
}

/// Cihaz dosyasını okur ve doğrular (§5). K4G-11'in tek giriş noktası.
///
/// # Errors
/// [`FormatError`].
pub fn load_device_config(path: &Path) -> Result<DeviceConfig, FormatError> {
    let bytes = read_bounded(path)?;
    let config: DeviceConfig = serde_json::from_slice(&bytes).map_err(FormatError::json)?;
    config.validate()?;
    Ok(config)
}

/// Önce boyut, sonra okuma (SR-AUDIT-001 bounds-before-alloc). Tampon Drop'ta sıfırlanır.
fn read_bounded(path: &Path) -> Result<Zeroizing<Vec<u8>>, FormatError> {
    let file = File::open(path)?;
    let declared = file.metadata()?.len();
    if declared > MAX_FILE_BYTES {
        return Err(FormatError::TooLarge);
    }
    let capacity = usize::try_from(declared).map_err(|_| FormatError::TooLarge)?;
    let mut buffer = Zeroizing::new(Vec::with_capacity(capacity));
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut buffer)?;
    if u64::try_from(buffer.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(FormatError::TooLarge);
    }
    Ok(buffer)
}

/// `host:port` (§2): `host` boş değil ve boşluk içermez, `port` 1–65535.
#[must_use]
pub fn is_host_port(text: &str) -> bool {
    let Some((host, port)) = text.rsplit_once(':') else {
        return false;
    };
    !host.is_empty()
        && !host.chars().any(char::is_whitespace)
        && port.parse::<u16>().is_ok_and(|number| number != 0)
}

/// `hub` yoksa ya da adresi `host:port` ise doğru.
fn hub_addr_ok(hub: Option<&HubConfig>) -> bool {
    hub.is_none_or(|hub| is_host_port(&hub.addr))
}

fn x25519_public_of(secret: &Secret32) -> Public32 {
    let scalar = StaticSecret::from(*secret.expose());
    Public32(*PublicKey::from(&scalar).as_bytes())
}

fn ed25519_public_of(secret: &Secret32) -> Public32 {
    let signing = SigningKey::from_bytes(secret.expose());
    Public32(signing.verifying_key().to_bytes())
}

impl DeviceConfig {
    /// Yapısal tutarlılık (§5): anahtar çiftleri, kimlik ≠ cihaz anahtarı, `identity_hash` /
    /// `dest_hash` türetmesi (kendisi + eşler), sertifikanın taşıdığı anahtarlar, tekillik. İmza,
    /// zincir ve iptal DENETLENMEZ (ccr §35.4 KAPALI-SİSTEM KİPİ: eş listesi bant dışında doğrulanmıştır).
    ///
    /// # Errors
    /// [`FormatError::UnsupportedVersion`] · [`FormatError::Invalid`].
    pub fn validate(&self) -> Result<(), FormatError> {
        if self.format_version != FORMAT_VERSION {
            return Err(FormatError::UnsupportedVersion(self.format_version));
        }
        if x25519_public_of(&self.x25519_static.secret) != self.x25519_static.public {
            return Err(FormatError::Invalid(
                "x25519_static.pub does not match its priv",
            ));
        }
        if ed25519_public_of(&self.ed25519.secret) != self.ed25519.public {
            return Err(FormatError::Invalid("ed25519.pub does not match its priv"));
        }
        if ed25519_public_of(&self.identity_ed25519.secret) != self.identity_ed25519.public {
            return Err(FormatError::Invalid(
                "identity_ed25519.pub does not match its priv",
            ));
        }
        // ccr §13.3: kimlik cihaz anahtarından türetilmez — aynı anahtar iki rolde kullanılamaz.
        if self.identity_ed25519.public == self.ed25519.public {
            return Err(FormatError::Invalid(
                "identity key must differ from device key",
            ));
        }
        let identity_hash = cert::identity_hash(self.identity_ed25519.public.bytes());
        if identity_hash != *self.identity_hash.bytes() {
            return Err(FormatError::Invalid(
                "identity_hash is not SHA-256(identity pub)",
            ));
        }
        if cert::dest_hash(&identity_hash) != *self.dest_hash.bytes() {
            return Err(FormatError::Invalid(
                "dest_hash is not SHA-256(identity_hash)",
            ));
        }
        let (own, _) = split_compact_device_cert(self.cert.bytes());
        if own.device_ed25519_pub != *self.ed25519.public.bytes()
            || own.device_x25519_pub != *self.x25519_static.public.bytes()
        {
            return Err(FormatError::Invalid(
                "cert does not carry this device's keys",
            ));
        }
        if self.peers.is_empty() || self.peers.len() >= MAX_DEVICES {
            return Err(FormatError::Invalid("peer count out of range"));
        }
        let mut names = BTreeSet::from([&self.device_id]);
        let mut dest_hashes = BTreeSet::from([self.dest_hash]);
        let mut cert_refs = BTreeSet::from([self.cert.cert_ref()]);
        for peer in &self.peers {
            if !names.insert(&peer.name) {
                return Err(FormatError::Invalid("duplicate device name"));
            }
            if !dest_hashes.insert(peer.dest_hash) {
                return Err(FormatError::Invalid("duplicate dest_hash"));
            }
            if !cert_refs.insert(peer.cert.cert_ref()) {
                return Err(FormatError::Invalid("duplicate cert"));
            }
            if *peer.x25519_pub.bytes() == [0u8; KEY_LEN] {
                return Err(FormatError::Invalid("all-zero peer x25519_pub"));
            }
            if cert::dest_hash(peer.identity_hash.bytes()) != *peer.dest_hash.bytes() {
                return Err(FormatError::Invalid(
                    "peer dest_hash is not SHA-256(identity_hash)",
                ));
            }
            let (fields, _) = split_compact_device_cert(peer.cert.bytes());
            if fields.device_x25519_pub != *peer.x25519_pub.bytes() {
                return Err(FormatError::Invalid(
                    "peer cert does not carry peer x25519_pub",
                ));
            }
        }
        for (name, addr) in &self.direct_addrs {
            if !names.contains(name) {
                return Err(FormatError::Invalid("direct_addrs names an unknown device"));
            }
            if addr.port() == 0 {
                return Err(FormatError::Invalid("direct_addrs port 0"));
            }
        }
        if !hub_addr_ok(self.hub.as_ref()) {
            return Err(FormatError::Invalid("hub.addr is not host:port"));
        }
        Ok(())
    }

    /// `kama-noise-wire` yerel kimliği (§5): statik X25519 + `dest_hash` + `cert_ref`.
    #[must_use]
    pub fn local_identity(&self) -> LocalIdentity {
        LocalIdentity::new(
            StaticSecret::from(*self.x25519_static.secret.expose()),
            *self.dest_hash.bytes(),
            self.cert.cert_ref(),
        )
    }

    /// ccr §35.4 KAPALI-SİSTEM KİPİ eş dizini (§5) — `ControlConfig.directory`.
    ///
    /// # Errors
    /// [`DirectoryError`] — yinelenen `dest_hash` / `cert_ref` ya da tüm-sıfır statik anahtar.
    pub fn peer_directory(&self) -> Result<PeerDirectory, DirectoryError> {
        PeerDirectory::new(self.peers.iter().map(|peer| PeerEntry {
            dest_hash: *peer.dest_hash.bytes(),
            static_x25519_pub: *peer.x25519_pub.bytes(),
            cert_ref: peer.cert.cert_ref(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_are_file_name_safe() {
        for good in ["alice", "a", "bob-2", "node_01", "0x"] {
            assert!(DeviceName::new(good).is_ok(), "{good}");
        }
        assert_eq!(DeviceName::new(""), Err(NameError::Empty));
        assert_eq!(DeviceName::new(&"a".repeat(33)), Err(NameError::TooLong));
        for bad in ["Alice", "-a", "_a", "a.b", "a/b", "../x", "a b", "çağrı"] {
            assert_eq!(DeviceName::new(bad), Err(NameError::Charset), "{bad}");
        }
        let name = DeviceName::new("alice").expect("geçerli");
        assert_eq!(name.file_name(), "device-alice.json");
    }

    #[test]
    fn secrets_never_appear_in_debug_output() {
        let pair = KeyPair {
            secret: Secret32::copy_from(&[0xAB; KEY_LEN]),
            public: Public32::from_bytes([0xCD; KEY_LEN]),
        };
        let shown = format!("{pair:?}");
        assert!(shown.contains("<redacted>"), "{shown}");
        // Gizli: ne hex ne ondalık bayt dizisi. Açık kimlik baytları da basılmaz (PeerEntry kuralı).
        for needle in ["abababab", "171, 171", "cdcdcdcd", "205, 205"] {
            assert!(
                !shown.contains(needle),
                "Debug çıktısında {needle}: {shown}"
            );
        }
    }

    #[test]
    fn secret_round_trips_through_json_and_rejects_bad_hex() {
        let secret = Secret32::copy_from(&[0x5A; KEY_LEN]);
        let json = serde_json::to_string(&secret).expect("serileşir");
        assert_eq!(json, format!("\"{}\"", "5a".repeat(KEY_LEN)));
        let back: Secret32 = serde_json::from_str(&json).expect("çözülür");
        assert_eq!(back.expose(), &[0x5A; KEY_LEN]);
        let upper = format!("\"{}\"", "5A".repeat(KEY_LEN));
        assert!(
            serde_json::from_str::<Secret32>(&upper).is_err(),
            "büyük harf hex"
        );
        let short = format!("\"{}\"", "5a".repeat(KEY_LEN - 1));
        assert!(
            serde_json::from_str::<Secret32>(&short).is_err(),
            "kısa hex"
        );
    }

    #[test]
    fn host_port_rule() {
        for good in ["hub.example:7000", "192.0.2.1:1", "[::1]:65535"] {
            assert!(is_host_port(good), "{good}");
        }
        for bad in [
            "",
            "hub",
            ":7000",
            "hub:0",
            "hub:65536",
            "hub:x",
            "h b:1",
            "hub:",
        ] {
            assert!(!is_host_port(bad), "{bad}");
        }
    }

    #[test]
    fn root_file_is_not_a_device_config() {
        let root = RootFile {
            format_version: FORMAT_VERSION,
            created_at_unix: 1,
            ed25519: KeyPair {
                secret: Secret32::copy_from(&[1; KEY_LEN]),
                public: Public32::from_bytes([2; KEY_LEN]),
            },
        };
        let json = serde_json::to_vec(&root).expect("serileşir");
        assert!(serde_json::from_slice::<DeviceConfig>(&json).is_err());
    }
}
