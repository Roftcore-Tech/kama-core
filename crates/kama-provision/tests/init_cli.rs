//! K4G-12 DONE kanıtı (kart madde 3) — gerçek `kama-provision` ikilisi, geçici dizinde.
//!
//! `init` 3 cihaz üretir; her cihazın `peers`'ı diğer ikisini içerir; `dest_hash` cihazdan AYRI kimlik
//! anahtarından türetme fonksiyonuyla (ve bağımsız SHA-256 hesabıyla) doğrulanır (plan K4G-12 Ek (3));
//! `cert` katı Ed25519 (`verify_strict`) ile test köküne karşı geçer — TBS etiketi Python altın
//! vektöründen (`ccr-compact-device-cert-v1`) okunur; iki koşu farklı anahtarlar üretir; hiçbir
//! `device-*.json` kök gizli anahtarını içermez (bayt araması); `.gitignore` ve `0600` mevcut.
//! Çıktılar yalnız `std::env::temp_dir()` altındaki benzersiz alt dizinlere yazılır ve test sonunda
//! silinir — repoya hiçbir `root.json` / `device-*.json` yazılmaz.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use kama_noise_wire::ed25519_strict::verify_strict;
use kama_noise_wire::handshake::cert_ref_of;
use kama_provision::cert;
use kama_provision::encoding::{base64_encode, hex_encode};
use kama_provision::format::{load_device_config, DeviceConfig, RootFile};
use kama_vectors::Expected;
use sha2::{Digest, Sha256};

const BIN: &str = env!("CARGO_BIN_EXE_kama-provision");
const CDC_SUITE: &str = "ccr-compact-device-cert-v1";
const THREE: [&str; 3] = ["alice", "bob", "carol"];
/// ccr §13.1: 90 gün.
const NINETY_DAYS_SECS: u64 = 90 * 24 * 60 * 60;

/// `std::env::temp_dir()` altında bu teste özgü dizin; Drop'ta silinir (panik etmez).
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let pid = std::process::id();
        loop {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let leaf = format!("kama-provision-{label}-{pid}-{nanos}-{n}");
            let path = std::env::temp_dir().join(leaf);
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("geçici dizin {}: {error}", path.display()),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `domain_sep("SR_CDC_V1")` — Python referansının altın vektöründen (ccr §13.2 YAMA 2026-10-11);
/// Rust sabitinden DEĞİL.
fn reference_domain_sep() -> Vec<u8> {
    let path = format!(
        "{}/../../vectors/{CDC_SUITE}/{CDC_SUITE}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let file = kama_vectors::load_file(path).expect("vektör dosyası yüklenmeli");
    let case = file
        .cases
        .iter()
        .find(|case| case.kind == "domain-sep")
        .expect("domain-sep vakası");
    let Expected::Wire(wire) = &case.expected else {
        panic!("{}: pozitif vaka bekleniyordu", case.id);
    };
    wire.clone()
}

fn run_init(out: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("init")
        .arg("--out")
        .arg(out)
        .args(args)
        .output()
        .expect("kama-provision çalıştırılamadı")
}

/// Okunan çıktı: kök dosyası + cihaz yapılandırmaları (yükleyiciden geçmiş) ve ham baytları.
struct Provisioned {
    root: RootFile,
    root_raw: Vec<u8>,
    devices: BTreeMap<String, (DeviceConfig, Vec<u8>)>,
}

fn read_output(out: &Path, names: &[&str]) -> Provisioned {
    let root_raw = fs::read(out.join("root.json")).expect("root.json okunamadı");
    let root: RootFile = serde_json::from_slice(&root_raw).expect("root.json biçimi");
    let mut devices = BTreeMap::new();
    for name in names {
        let path = out.join(format!("device-{name}.json"));
        let raw = fs::read(&path).expect("cihaz dosyası okunamadı");
        let config = load_device_config(&path).expect("cihaz dosyası yükleyiciden geçmeli");
        devices.insert((*name).to_owned(), (config, raw));
    }
    Provisioned {
        root,
        root_raw,
        devices,
    }
}

/// Bir gizli anahtarın bir dosyada görünebileceği kodlamalar: ham bayt, küçük ve büyük harf hex,
/// base64 (dolgulu ve dolgusuz).
fn secret_encodings(secret: &[u8; 32]) -> Vec<Vec<u8>> {
    let hex = hex_encode(secret);
    let b64 = base64_encode(secret);
    vec![
        secret.to_vec(),
        hex.as_bytes().to_vec(),
        hex.to_uppercase().into_bytes(),
        b64.as_bytes().to_vec(),
        b64.trim_end_matches('=').as_bytes().to_vec(),
    ]
}

fn contains_any(haystack: &[u8], needles: &[Vec<u8>]) -> bool {
    needles.iter().any(|needle| {
        haystack
            .windows(needle.len())
            .any(|window| window == needle.as_slice())
    })
}

#[cfg(unix)]
fn assert_private_permissions(dir: &Path, files: &BTreeSet<String>) {
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(path).expect("metadata");
        meta.permissions().mode() & 0o777
    }
    assert_eq!(mode(dir), 0o700, "çıktı dizini 0700 olmalı");
    for name in files {
        assert_eq!(mode(&dir.join(name)), 0o600, "{name} 0600 olmalı");
    }
}

#[cfg(not(unix))]
fn assert_private_permissions(_dir: &Path, _files: &BTreeSet<String>) {}

#[test]
fn init_writes_three_mutually_listed_devices_with_root_signed_certs() {
    let tmp = TempDir::new("three");
    let out = tmp.path().join("out");
    let result = run_init(
        &out,
        &[
            "--devices",
            "alice,bob,carol",
            "--hub",
            "192.0.2.10:7000",
            "--direct",
            "alice=127.0.0.1:47001,bob=127.0.0.1:47002,carol=[::1]:47003",
        ],
    );
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(result.status.success(), "init başarısız: {stderr}");

    // Dosya kümesi tam olarak bu; `.gitignore` = `*`; dosyalar 0600, dizin 0700.
    let listed: BTreeSet<String> = fs::read_dir(&out)
        .expect("çıktı dizini")
        .map(|entry| entry.expect("dizin girdisi").file_name())
        .map(|file| file.to_string_lossy().into_owned())
        .collect();
    let expected: BTreeSet<String> = [
        ".gitignore",
        "device-alice.json",
        "device-bob.json",
        "device-carol.json",
        "root.json",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(listed, expected);
    let gitignore = fs::read_to_string(out.join(".gitignore")).expect(".gitignore");
    assert_eq!(gitignore, "*\n");
    assert_private_permissions(&out, &expected);

    let provisioned = read_output(&out, &THREE);
    let root_pub = *provisioned.root.ed25519.public.bytes();
    let root_secret = *provisioned.root.ed25519.secret.expose();
    let mut issuer = Sha256::new();
    issuer.update([0x01u8]);
    issuer.update(root_pub);
    let issuer_key_id = issuer.finalize();
    let not_after = provisioned.root.created_at_unix + NINETY_DAYS_SECS;
    let domain_sep = reference_domain_sep();

    for (name, (config, _)) in &provisioned.devices {
        assert_eq!(config.device_id.as_str(), name);
        assert_eq!(*config.root_pub.bytes(), root_pub, "{name}: root_pub");

        // `peers` = diğer iki cihaz, değerleri onların kendi dosyalarıyla bayt-özdeş.
        let peer_names: BTreeSet<&str> = config.peers.iter().map(|p| p.name.as_str()).collect();
        let others: BTreeSet<&str> = THREE.into_iter().filter(|n| n != name).collect();
        assert_eq!(peer_names, others, "{name}: peers diğer iki cihaz olmalı");
        assert_eq!(config.peers.len(), 2);
        for peer in &config.peers {
            let (other, _) = &provisioned.devices[peer.name.as_str()];
            assert_eq!(peer.identity_hash, other.identity_hash);
            assert_eq!(peer.dest_hash, other.dest_hash);
            assert_eq!(peer.x25519_pub, other.x25519_static.public);
            assert_eq!(peer.cert, other.cert);
        }

        // `identity_hash` / `dest_hash`: AYRI kimlik anahtarından (ccr §13.3 / §13.5) — türetme
        // fonksiyonu + bağımsız SHA-256; reddedilen cihaz-anahtarı formülüyle ÇAKIŞMAZ.
        let ed_pub = *config.ed25519.public.bytes();
        let identity_pub = *config.identity_ed25519.public.bytes();
        assert_ne!(
            identity_pub, ed_pub,
            "{name}: kimlik anahtarı cihaz anahtarından ayrı olmalı"
        );
        let identity_hash = cert::identity_hash(&identity_pub);
        assert_eq!(
            *config.identity_hash.bytes(),
            identity_hash,
            "{name}: identity_hash"
        );
        assert_eq!(*config.dest_hash.bytes(), cert::dest_hash(&identity_hash));
        let independent: [u8; 32] = Sha256::digest(Sha256::digest(identity_pub)).into();
        assert_eq!(*config.dest_hash.bytes(), independent, "{name}: dest_hash");
        let rejected: [u8; 32] = Sha256::digest(Sha256::digest(ed_pub)).into();
        assert_ne!(
            *config.dest_hash.bytes(),
            rejected,
            "{name}: cihaz anahtarından türemiş"
        );

        // `cert`: ccr §13.1 düzeni bağımsız ofsetlerle; imza köke karşı katı kipte.
        let cert_bytes = config.cert.bytes();
        assert_eq!(cert_bytes[0..32], ed_pub, "device_ed25519_pub");
        assert_eq!(
            cert_bytes[32..64],
            *config.x25519_static.public.bytes(),
            "device_x25519_pub"
        );
        assert_eq!(cert_bytes[64..80], issuer_key_id[..16], "issuer_key_id");
        assert_eq!(cert_bytes[80..88], not_after.to_be_bytes(), "not_after");
        let mut tbs = domain_sep.clone();
        tbs.extend_from_slice(&cert_bytes[..88]);
        let signature = &cert_bytes[88..];
        assert_eq!(verify_strict(&root_pub, &tbs, signature), Ok(()), "{name}");
        assert_eq!(
            cert::verify_compact_device_cert(cert_bytes, &root_pub),
            Ok(())
        );
        // Boş geçmediğinin kanıtı: yanlış imzalayan ve bozulmuş bayt reddedilir.
        assert!(verify_strict(&ed_pub, &tbs, signature).is_err());
        let mut tampered = tbs;
        tampered[20] ^= 0x01;
        assert!(verify_strict(&root_pub, &tampered, signature).is_err());

        // `hub` ve `direct_addrs`.
        let hub = config.hub.as_ref().expect("--hub verildi");
        assert_eq!(hub.addr, "192.0.2.10:7000");
        assert!(hub.x25519_pub.is_none(), "v0'da hub anahtarı yazılmaz");
        let addrs: BTreeMap<&str, String> = config
            .direct_addrs
            .iter()
            .map(|(device, addr)| (device.as_str(), addr.to_string()))
            .collect();
        let expected_addrs = BTreeMap::from([
            ("alice", "127.0.0.1:47001".to_owned()),
            ("bob", "127.0.0.1:47002".to_owned()),
            ("carol", "[::1]:47003".to_owned()),
        ]);
        assert_eq!(addrs, expected_addrs);

        // K4G-11 tüketicisi: kama-noise-wire kimliği ve eş dizini kurulur.
        let identity = config.local_identity();
        assert_eq!(
            identity.static_public().as_bytes(),
            config.x25519_static.public.bytes()
        );
        assert_eq!(identity.dest_hash, *config.dest_hash.bytes());
        assert_eq!(identity.cert_ref, cert_ref_of(cert_bytes));
        let directory = config.peer_directory().expect("eş dizini kurulmalı");
        assert_eq!(directory.len(), 2);
        for peer in &config.peers {
            let entry = directory
                .by_dest_hash(peer.dest_hash.bytes())
                .expect("eş dizinde");
            assert_eq!(entry.static_x25519_pub, *peer.x25519_pub.bytes());
            assert_eq!(entry.cert_ref, cert_ref_of(peer.cert.bytes()));
        }
    }

    // Kök gizli anahtarı hiçbir `device-*.json`'da yok (bayt araması). Yöntem boş geçmiyor: aynı
    // arama `root.json`'da kök gizlisini bulur.
    let root_needles = secret_encodings(&root_secret);
    assert!(
        contains_any(&provisioned.root_raw, &root_needles),
        "bayt araması root.json'daki kök gizlisini bulmalı"
    );
    for (name, (_, raw)) in &provisioned.devices {
        assert!(
            !contains_any(raw, &root_needles),
            "device-{name}.json kök gizli anahtarını içeriyor"
        );
    }
    assert!(
        load_device_config(&out.join("root.json")).is_err(),
        "root.json cihaz yapılandırması olarak yüklenmemeli"
    );

    // Standart çıktı ve hata: hiçbir gizli anahtar (kök + cihazlar) yok.
    let mut all_needles = root_needles;
    for (config, _) in provisioned.devices.values() {
        all_needles.extend(secret_encodings(config.x25519_static.secret.expose()));
        all_needles.extend(secret_encodings(config.ed25519.secret.expose()));
        all_needles.extend(secret_encodings(config.identity_ed25519.secret.expose()));
    }
    assert!(
        !contains_any(&result.stdout, &all_needles),
        "stdout'ta gizli anahtar"
    );
    assert!(
        !contains_any(&result.stderr, &all_needles),
        "stderr'de gizli anahtar"
    );
}

#[test]
fn every_run_generates_fresh_keys_and_optional_fields_stay_empty() {
    let tmp = TempDir::new("fresh");
    let first = tmp.path().join("first");
    let second = tmp.path().join("second");
    for out in [&first, &second] {
        let result = run_init(out, &["--devices", "alice,bob"]);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "init başarısız: {stderr}");
    }
    let a = read_output(&first, &["alice", "bob"]);
    let b = read_output(&second, &["alice", "bob"]);
    assert_ne!(a.root.ed25519.public, b.root.ed25519.public);
    assert!(
        a.root.ed25519.secret.expose() != b.root.ed25519.secret.expose(),
        "kök gizli anahtarı iki koşuda aynı"
    );
    for name in ["alice", "bob"] {
        let (x, _) = &a.devices[name];
        let (y, _) = &b.devices[name];
        assert_ne!(x.dest_hash, y.dest_hash, "{name}");
        assert_ne!(
            x.identity_ed25519.public, y.identity_ed25519.public,
            "{name}"
        );
        assert_ne!(x.x25519_static.public, y.x25519_static.public, "{name}");
        assert_ne!(x.ed25519.public, y.ed25519.public, "{name}");
        assert_ne!(x.cert, y.cert, "{name}");
        assert!(
            x.x25519_static.secret.expose() != y.x25519_static.secret.expose()
                && x.ed25519.secret.expose() != y.ed25519.secret.expose()
                && x.identity_ed25519.secret.expose() != y.identity_ed25519.secret.expose(),
            "{name}: gizli anahtar iki koşuda aynı"
        );
        assert_ne!(x.ed25519.public, a.root.ed25519.public, "cihaz ≠ kök");
        assert!(x.hub.is_none(), "--hub yokken hub null");
        assert!(x.direct_addrs.is_empty(), "--direct yokken boş");
    }
}

#[test]
fn non_empty_output_directory_is_refused_and_left_untouched() {
    let tmp = TempDir::new("busy");
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("dizin");
    fs::write(out.join("keep.txt"), b"x").expect("dosya");
    let result = run_init(&out, &["--devices", "alice,bob"]);
    assert_eq!(result.status.code(), Some(1));
    let left = fs::read_dir(&out).expect("dizin").count();
    assert_eq!(left, 1, "yalnız önceden var olan dosya kalmalı");
}

#[test]
fn usage_errors_exit_2_and_create_nothing() {
    let tmp = TempDir::new("usage");
    let out = tmp.path().join("out");
    let cases: [&[&str]; 7] = [
        &["--devices", "alice"],
        &["--devices", "alice,alice"],
        &["--devices", "Alice,bob"],
        &["--devices", "alice,bob", "--direct", "carol=127.0.0.1:1"],
        &["--devices", "alice,bob", "--direct", "alice=localhost:1"],
        &["--devices", "alice,bob", "--hub", "hub-without-port"],
        &["--devices", "alice,bob", "--bogus", "x"],
    ];
    for args in cases {
        let result = run_init(&out, args);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
        assert!(
            !out.exists(),
            "{args:?}: kullanım hatasında dizin oluşmamalı"
        );
    }
    let missing_out = Command::new(BIN)
        .args(["init", "--devices", "alice,bob"])
        .output()
        .expect("kama-provision çalıştırılamadı");
    assert_eq!(missing_out.status.code(), Some(2));
}
