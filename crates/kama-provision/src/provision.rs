//! `kama-provision init` — anahtar üretimi, öz-denetim ve dosya yazımı (PROVISIONING-FORMAT-v0 §2–§4).
//!
//! Sıra: girdi denetimi → bellekte üretim (kök + cihaz anahtarları, sertifikalar) → öz-denetim
//! (yükleyicinin denetimleri + sertifika köke karşı) → çıktı dizini → `.gitignore` → cihaz dosyaları →
//! `root.json`. Girdi, entropi ve öz-denetim hatalarında dosya sistemine dokunulmaz; yazım sırasında
//! hata olursa bu koşunun oluşturduğu her dosya (ve oluşturduysa dizin) silinir. Dosyalar `O_EXCL` ile
//! YENİ oluşturulur ve Unix'te `0600`'dür (dizin `0700`).
//!
//! Gizli anahtarlar yalnız dalek tiplerinde (`StaticSecret`, `SigningKey`; Drop'ta sıfırlanır) ve
//! sıfırlanan tamponlarda (`Zeroizing`) tutulur; hiçbiri log'a ya da standart çıktıya yazılmaz.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use kama_noise_wire::entropy::{EntropyError, EntropySource};
use kama_noise_wire::handshake::COMPACT_DEVICE_CERT_LEN;
use serde::Serialize;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::cert::{self, CertError, CertFields, ISSUER_KEY_ID_LEN, KEY_LEN};
use crate::format::{
    is_host_port, CertBytes, DeviceConfig, DeviceName, FormatError, HubConfig, KeyPair, PeerConfig,
    Public32, RootFile, Secret32, FORMAT_VERSION, GITIGNORE_CONTENT, GITIGNORE_FILE_NAME,
    MAX_DEVICES, MIN_DEVICES, ROOT_FILE_NAME,
};

/// Çıktı dizini izni (Unix).
#[cfg(unix)]
const PRIVATE_DIR_MODE: u32 = 0o700;
/// Çıktı dosyası izni (Unix; D9).
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

/// `init` girdisi (CLI ya da test).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRequest {
    /// Çıktı dizini: yoksa oluşturulur, varsa boş olmalı (§2).
    pub out_dir: PathBuf,
    /// 2–64 benzersiz cihaz; her cihazın `peers` sırası bu sıradır.
    pub devices: Vec<DeviceName>,
    /// `host:port` (§2, §4.7).
    pub hub: Option<String>,
    /// Direct-mode adresleri (§4.6).
    pub direct: BTreeMap<DeviceName, SocketAddr>,
}

/// `init` sonucu.
#[derive(Debug)]
pub struct InitReport {
    /// Yazılan dosyalar, yazım sırasıyla (`.gitignore`, cihazlar, `root.json`).
    pub written: Vec<PathBuf>,
    /// Unix izinleri (`0700` dizin, `0600` dosya) uygulandı mı; Unix dışında `false` (§2).
    pub permissions_enforced: bool,
}

/// Girdi hatası — dosya sistemine dokunulmadan reddedilir (CLI çıkış kodu 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("device count must be between 2 and 64")]
    DeviceCount,
    #[error("a device name is listed twice")]
    DuplicateDevice,
    #[error("--direct names a device that is not in --devices")]
    UnknownDirectDevice,
    #[error("--direct port must be 1-65535")]
    DirectPort,
    #[error("--hub must be host:port with port 1-65535")]
    HubAddr,
}

/// `init` hatası. Gizli değer taşımaz.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("output path exists and is not an empty directory")]
    OutDirNotEmpty,
    #[error("system clock out of range")]
    Clock,
    #[error("entropy source failed: {0}")]
    Entropy(#[from] EntropyError),
    #[error("entropy source returned an all-zero key")]
    ZeroKey,
    #[error("certificate: {0}")]
    Cert(#[from] CertError),
    #[error("self-check failed: {0}")]
    SelfCheck(#[from] FormatError),
    #[error("json encoding failed")]
    Json,
    #[error("i/o error at {path}: {source}")]
    Io { path: String, source: io::Error },
}

impl From<serde_json::Error> for InitError {
    // `serde_json` mesajı değer alıntılayabilir; yalnız sınıf tutulur.
    fn from(_: serde_json::Error) -> Self {
        Self::Json
    }
}

/// Girdi kuralları (§2). Dosya sistemine dokunmaz.
///
/// # Errors
/// [`RequestError`].
pub fn validate_request(request: &InitRequest) -> Result<(), RequestError> {
    let count = request.devices.len();
    if !(MIN_DEVICES..=MAX_DEVICES).contains(&count) {
        return Err(RequestError::DeviceCount);
    }
    let unique: BTreeSet<&DeviceName> = request.devices.iter().collect();
    if unique.len() != count {
        return Err(RequestError::DuplicateDevice);
    }
    for (name, addr) in &request.direct {
        if !unique.contains(name) {
            return Err(RequestError::UnknownDirectDevice);
        }
        if addr.port() == 0 {
            return Err(RequestError::DirectPort);
        }
    }
    if !request.hub.as_deref().is_none_or(is_host_port) {
        return Err(RequestError::HubAddr);
    }
    Ok(())
}

/// Kök ve cihaz anahtarlarını üretir, sertifikaları test köküyle imzalar, öz-denetler ve dosyaları
/// yazar. `created_at_unix` = `root.json` oluşturma zamanı (Unix saniyesi); sertifikaların
/// `not_after`'ı buradan türer (§4.3).
///
/// # Errors
/// [`InitError`]. Girdi, entropi, sertifika ve öz-denetim hatalarında dosya sistemine dokunulmaz;
/// yazım hatasında bu koşunun oluşturduğu her şey silinir.
pub fn init(
    request: &InitRequest,
    entropy: &mut dyn EntropySource,
    created_at_unix: u64,
) -> Result<InitReport, InitError> {
    validate_request(request)?;
    let not_after = created_at_unix
        .checked_add(cert::DEVICE_CERT_VALIDITY_SECS)
        .ok_or(InitError::Clock)?;

    // Tohum `Zeroizing`'de kalır; anahtar tipine yalnız ödünç verilir (deref).
    let root_seed = random_key(entropy)?;
    let root = SigningKey::from_bytes(&root_seed);
    let root_pub = root.verifying_key().to_bytes();
    let issuer_key_id = cert::issuer_key_id(&root_pub);
    let mut devices = Vec::with_capacity(request.devices.len());
    for name in &request.devices {
        let device = generate_device(name, entropy, &root, issuer_key_id, not_after)?;
        devices.push(device);
    }

    let mut files = Vec::with_capacity(devices.len() + 1);
    for device in &devices {
        let config = device_config(device, &devices, root_pub, request);
        // Öz-denetim: yükleyicinin denetimleri + sertifika köke karşı (katı Ed25519).
        config.validate()?;
        cert::verify_compact_device_cert(&device.cert, &root_pub)?;
        files.push((device.name.file_name(), to_json(&config)?));
    }
    let root_file = RootFile {
        format_version: FORMAT_VERSION,
        created_at_unix,
        ed25519: KeyPair {
            secret: Secret32::from_bytes(root.to_bytes()),
            public: Public32::from_bytes(root_pub),
        },
    };
    files.push((ROOT_FILE_NAME.to_owned(), to_json(&root_file)?));

    let mut output = Output::create(&request.out_dir)?;
    output.write(GITIGNORE_FILE_NAME, GITIGNORE_CONTENT.as_bytes())?;
    for (name, contents) in &files {
        output.write(name, contents)?;
    }
    Ok(InitReport {
        written: output.commit(),
        permissions_enforced: cfg!(unix),
    })
}

/// Bir cihazın bellekteki anahtarları ve sertifikası (gizli alanlar Drop'ta sıfırlanır).
struct DeviceMaterial {
    name: DeviceName,
    /// Kimlik Ed25519 çifti — cihaz anahtarlarından AYRI (plan K4G-12 Ek (3); ccr §13.3).
    identity: SigningKey,
    identity_pub: [u8; KEY_LEN],
    identity_hash: [u8; KEY_LEN],
    x25519: StaticSecret,
    x25519_pub: [u8; KEY_LEN],
    ed25519: SigningKey,
    ed25519_pub: [u8; KEY_LEN],
    dest_hash: [u8; KEY_LEN],
    cert: [u8; COMPACT_DEVICE_CERT_LEN],
}

/// OS entropisinden 32 B. Tüm-sıfır sonuç entropi arızası sayılır (§4.2).
fn random_key(entropy: &mut dyn EntropySource) -> Result<Zeroizing<[u8; KEY_LEN]>, InitError> {
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    entropy.fill(&mut key[..])?;
    if *key == [0u8; KEY_LEN] {
        return Err(InitError::ZeroKey);
    }
    Ok(key)
}

fn generate_device(
    name: &DeviceName,
    entropy: &mut dyn EntropySource,
    root: &SigningKey,
    issuer_key_id: [u8; ISSUER_KEY_ID_LEN],
    not_after: u64,
) -> Result<DeviceMaterial, InitError> {
    // Tohumlar `Zeroizing`'de kalır. `StaticSecret::from` diziyi değerle ister: tek kopya doğrudan
    // Drop'ta sıfırlanan `StaticSecret`'e gider; `SigningKey`'e yalnız ödünç verilir (deref).
    let x25519_seed = random_key(entropy)?;
    let x25519 = StaticSecret::from(*x25519_seed);
    let x25519_pub = *PublicKey::from(&x25519).as_bytes();
    let ed25519_seed = random_key(entropy)?;
    let ed25519 = SigningKey::from_bytes(&ed25519_seed);
    let ed25519_pub = ed25519.verifying_key().to_bytes();
    // dest_hash cihaz anahtarından DEĞİL, ayrı kimlik anahtarından türer (ccr §13.3 / §13.5).
    let identity_seed = random_key(entropy)?;
    let identity = SigningKey::from_bytes(&identity_seed);
    let identity_pub = identity.verifying_key().to_bytes();
    let identity_hash = cert::identity_hash(&identity_pub);
    let fields = CertFields {
        device_ed25519_pub: ed25519_pub,
        device_x25519_pub: x25519_pub,
        issuer_key_id,
        not_after,
    };
    let compact_cert = cert::sign_compact_device_cert(&fields, root)?;
    Ok(DeviceMaterial {
        name: name.clone(),
        identity,
        identity_pub,
        identity_hash,
        x25519,
        x25519_pub,
        ed25519,
        ed25519_pub,
        dest_hash: cert::dest_hash(&identity_hash),
        cert: compact_cert,
    })
}

/// `device-<ad>.json` içeriği: kendi kimlik ve cihaz anahtarları + diğer bütün cihazlar `peers`'ta
/// (§3.2). Kökün yalnız açık anahtarı girer (D9).
fn device_config(
    device: &DeviceMaterial,
    all: &[DeviceMaterial],
    root_pub: [u8; KEY_LEN],
    request: &InitRequest,
) -> DeviceConfig {
    let peers = all
        .iter()
        .filter(|peer| peer.name != device.name)
        .map(|peer| PeerConfig {
            name: peer.name.clone(),
            identity_hash: Public32::from_bytes(peer.identity_hash),
            dest_hash: Public32::from_bytes(peer.dest_hash),
            x25519_pub: Public32::from_bytes(peer.x25519_pub),
            cert: CertBytes::from_bytes(peer.cert),
        })
        .collect();
    let hub = request.hub.as_ref().map(|addr| HubConfig {
        addr: addr.clone(),
        x25519_pub: None,
    });
    DeviceConfig {
        format_version: FORMAT_VERSION,
        device_id: device.name.clone(),
        identity_ed25519: KeyPair {
            secret: Secret32::from_bytes(device.identity.to_bytes()),
            public: Public32::from_bytes(device.identity_pub),
        },
        identity_hash: Public32::from_bytes(device.identity_hash),
        dest_hash: Public32::from_bytes(device.dest_hash),
        x25519_static: KeyPair {
            secret: Secret32::from_bytes(device.x25519.to_bytes()),
            public: Public32::from_bytes(device.x25519_pub),
        },
        ed25519: KeyPair {
            secret: Secret32::from_bytes(device.ed25519.to_bytes()),
            public: Public32::from_bytes(device.ed25519_pub),
        },
        cert: CertBytes::from_bytes(device.cert),
        root_pub: Public32::from_bytes(root_pub),
        peers,
        hub,
        direct_addrs: request.direct.clone(),
    }
}

/// Girintili JSON + son satır sonu. Tampon Drop'ta sıfırlanır (gizli anahtar içerir).
fn to_json<T: Serialize>(value: &T) -> Result<Zeroizing<Vec<u8>>, InitError> {
    let mut bytes = Zeroizing::new(serde_json::to_vec_pretty(value)?);
    bytes.push(b'\n');
    Ok(bytes)
}

/// Bu koşunun oluşturduğu dosyalar. [`Output::commit`] edilmeden düşerse (hata yolu) oluşturduğu her
/// dosyayı ve oluşturduysa dizini siler. Drop panik etmez; silme hataları yok sayılır.
struct Output {
    dir: PathBuf,
    created_dir: bool,
    written: Vec<PathBuf>,
    committed: bool,
}

impl Output {
    /// Dizin yoksa oluşturur (Unix `0700`); varsa BOŞ bir dizin olmalı. Sembolik bağ kabul edilmez.
    fn create(dir: &Path) -> Result<Self, InitError> {
        let created_dir = match fs::symlink_metadata(dir) {
            Ok(meta) if meta.is_dir() && is_empty_dir(dir)? => false,
            Ok(_) => return Err(InitError::OutDirNotEmpty),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let created = create_private_dir(dir);
                created.map_err(|error| io_error(dir, error))?;
                true
            }
            Err(error) => return Err(io_error(dir, error)),
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            created_dir,
            written: Vec::new(),
            committed: false,
        })
    }

    /// `name`'i `O_EXCL` + `0600` ile YENİ oluşturur ve yazar.
    fn write(&mut self, name: &str, contents: &[u8]) -> Result<(), InitError> {
        let path = self.dir.join(name);
        let result = self.write_new(&path, contents);
        result.map_err(|error| io_error(&path, error))
    }

    fn write_new(&mut self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let mut file = create_private_file(path)?;
        // Dosya artık bu koşunundur: sonraki her hata yolunda silinir.
        self.written.push(path.to_path_buf());
        restrict_permissions(&file)?;
        file.write_all(contents)?;
        file.sync_all()
    }

    /// Yazımı tamamlar: dosyalar kalır, yolları yazım sırasıyla döner.
    fn commit(mut self) -> Vec<PathBuf> {
        self.committed = true;
        std::mem::take(&mut self.written)
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for path in self.written.iter().rev() {
            let _ = fs::remove_file(path);
        }
        if self.created_dir {
            let _ = fs::remove_dir(&self.dir);
        }
    }
}

fn is_empty_dir(dir: &Path) -> Result<bool, InitError> {
    let mut entries = fs::read_dir(dir).map_err(|error| io_error(dir, error))?;
    Ok(entries.next().is_none())
}

fn io_error(path: &Path, source: io::Error) -> InitError {
    InitError::Io {
        path: path.display().to_string(),
        source,
    }
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    fs::DirBuilder::new().mode(PRIVATE_DIR_MODE).create(dir)?;
    // umask yalnız bit kaldırır; yine de tam `0700` açıkça yazılır.
    fs::set_permissions(dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir(dir)
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// umask'tan bağımsız tam `0600`.
#[cfg(unix)]
fn restrict_permissions(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
}

/// Unix dışında izin uygulanmaz; CLI uyarı basar (§2).
#[cfg(not(unix))]
fn restrict_permissions(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// Deterministik test entropisi: `SHA-256(sayaç)` blokları. YALNIZ test.
    struct CounterEntropy(u64);

    impl EntropySource for CounterEntropy {
        fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
            for chunk in dst.chunks_mut(KEY_LEN) {
                self.0 += 1;
                let block = Sha256::digest(self.0.to_be_bytes());
                chunk.copy_from_slice(&block[..chunk.len()]);
            }
            Ok(())
        }
    }

    struct FailingEntropy;

    impl EntropySource for FailingEntropy {
        fn fill(&mut self, _dst: &mut [u8]) -> Result<(), EntropyError> {
            Err(EntropyError)
        }
    }

    struct ZeroEntropy;

    impl EntropySource for ZeroEntropy {
        fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
            dst.fill(0);
            Ok(())
        }
    }

    fn name(text: &str) -> DeviceName {
        DeviceName::new(text).expect("geçerli ad")
    }

    /// `std::env::temp_dir()` altında var OLMAYAN, bu teste özgü yol.
    fn scratch_path(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let pid = std::process::id();
        let leaf = format!("kama-provision-unit-{label}-{pid}-{nanos}");
        std::env::temp_dir().join(leaf)
    }

    fn request(out_dir: PathBuf, names: &[&str]) -> InitRequest {
        InitRequest {
            out_dir,
            devices: names.iter().copied().map(name).collect(),
            hub: None,
            direct: BTreeMap::new(),
        }
    }

    #[test]
    fn request_rules_reject_before_any_file_is_touched() {
        let out = scratch_path("rules");
        let one = request(out.clone(), &["alice"]);
        let twice = request(out.clone(), &["alice", "alice"]);
        let mut unknown = request(out.clone(), &["alice", "bob"]);
        let addr: SocketAddr = "127.0.0.1:1".parse().expect("adres");
        unknown.direct.insert(name("carol"), addr);
        let mut port_zero = request(out.clone(), &["alice", "bob"]);
        let zero: SocketAddr = "127.0.0.1:0".parse().expect("adres");
        port_zero.direct.insert(name("alice"), zero);
        let mut bad_hub = request(out.clone(), &["alice", "bob"]);
        bad_hub.hub = Some("hub-without-port".to_owned());
        let cases = [
            (one, RequestError::DeviceCount),
            (twice, RequestError::DuplicateDevice),
            (unknown, RequestError::UnknownDirectDevice),
            (port_zero, RequestError::DirectPort),
            (bad_hub, RequestError::HubAddr),
        ];
        for (req, expected) in cases {
            match init(&req, &mut CounterEntropy(0), 1) {
                Err(InitError::Request(error)) => assert_eq!(error, expected),
                other => panic!("{expected:?} bekleniyordu: {other:?}"),
            }
            assert!(!out.exists(), "girdi hatasında dizin oluşmamalı");
        }
    }

    #[test]
    fn entropy_failure_and_all_zero_keys_write_nothing() {
        let out = scratch_path("entropy");
        let req = request(out.clone(), &["alice", "bob"]);
        let failed = init(&req, &mut FailingEntropy, 1);
        assert!(matches!(failed, Err(InitError::Entropy(_))), "{failed:?}");
        let zero = init(&req, &mut ZeroEntropy, 1);
        assert!(matches!(zero, Err(InitError::ZeroKey)), "{zero:?}");
        assert!(!out.exists());
    }

    #[test]
    fn unfinished_output_removes_what_it_created() {
        let out = scratch_path("guard");
        {
            let mut output = Output::create(&out).expect("dizin");
            output.write("a.json", b"{}").expect("yazım");
            output.write("b.json", b"{}").expect("yazım");
            assert!(out.join("b.json").exists());
            // `commit` yok: hata yolundaki gibi düşer.
        }
        assert!(!out.exists(), "oluşturulan dosyalar ve dizin silinmeli");

        let mut output = Output::create(&out).expect("dizin");
        output.write("a.json", b"{}").expect("yazım");
        let collision = output.write("a.json", b"{}");
        assert!(matches!(collision, Err(InitError::Io { .. })), "O_EXCL");
        let written = output.commit();
        assert_eq!(written, vec![out.join("a.json")]);
        assert!(out.join("a.json").exists(), "commit sonrası dosya kalır");
        let busy = Output::create(&out);
        assert!(matches!(busy, Err(InitError::OutDirNotEmpty)));
        let _ = fs::remove_dir_all(&out);
    }
}
