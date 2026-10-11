//! DUR-K4G-12 (c) — ccr §13 altın vektörleri ↔ Rust: `ccr-dest-hash-v1` ve
//! `ccr-compact-device-cert-v1` (Python referansı `ref/crypto/gen_vectors.py`; Rust'tan BAĞIMSIZ).
//!
//! * `identity-hash` / `dest-hash`: kimlik anahtarından türetme (ccr §13.3 / §13.5).
//! * `domain-sep`: ccr §13.2 YAMA KAYDI 2026-10-11 (ASCII etiket + `0x00` dolgu).
//! * `issuer-key-id`, `tbs`, `encode`: CompactDeviceCertV1 (ccr §8.1 / §13.1 / §13.2); `encode` tam
//!   imzalı 152 B — Ed25519 belirlenimli olduğundan Rust aynı tohumla bayt-özdeş üretir; ardından
//!   ayrıştırma ve katı kip doğrulaması.
//!
//! Beklenen değerlerin TAMAMI vektör dosyalarından gelir; bu dosyada gömülü beklenen bayt yoktur.
//! Her dosyanın her vakası koşulur (`case_count` vakum kapısı).

use ed25519_dalek::SigningKey;
use kama_provision::cert::{self, CertFields, DOMAIN_SEP_CDC_V1};
use kama_vectors::{Case, Expected, Input, VectorFile};

fn suite(name: &str, kinds: &[&str]) -> VectorFile {
    let path = format!(
        "{}/../../vectors/{name}/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let file = kama_vectors::load_file(path).expect("vektör dosyası yüklenmeli");
    file.require_kinds(kinds)
        .expect("dosyada bu tüketicinin tanımadığı bir kind var");
    file
}

fn of_kind<'a>(file: &'a VectorFile, kind: &str) -> Vec<&'a Case> {
    let picked: Vec<&Case> = file.cases.iter().filter(|c| c.kind == kind).collect();
    assert!(!picked.is_empty(), "'{kind}' türünde vaka yok (vakum)");
    picked
}

fn field(case: &Case, name: &str) -> Vec<u8> {
    let Input::Structured(map) = &case.input else {
        panic!("{}: yapısal girdi bekleniyordu", case.id);
    };
    let text = map
        .get(name)
        .and_then(|value| value.as_str())
        .unwrap_or_else(|| panic!("{}: '{name}' alanı yok", case.id));
    kama_vectors::hex(text).unwrap_or_else(|e| panic!("{}: '{name}' hex değil: {e:?}", case.id))
}

fn array<const N: usize>(case: &Case, name: &str) -> [u8; N] {
    field(case, name)
        .as_slice()
        .try_into()
        .unwrap_or_else(|_| panic!("{}: '{name}' {N} bayt olmalı", case.id))
}

fn expected(case: &Case) -> &[u8] {
    let Expected::Wire(wire) = &case.expected else {
        panic!("{}: pozitif vaka bekleniyordu", case.id);
    };
    wire
}

fn fields_of(case: &Case) -> CertFields {
    CertFields {
        device_ed25519_pub: array(case, "device_ed25519_pub_hex"),
        device_x25519_pub: array(case, "device_x25519_pub_hex"),
        issuer_key_id: array(case, "issuer_key_id_hex"),
        not_after: u64::from_be_bytes(array(case, "not_after_hex")),
    }
}

#[test]
fn identity_and_dest_hash_match_the_reference() {
    let file = suite("ccr-dest-hash-v1", &["identity-hash", "dest-hash"]);
    let mut consumed = 0usize;
    for case in of_kind(&file, "identity-hash") {
        let identity_hash = cert::identity_hash(&array(case, "identity_ed25519_pub_hex"));
        assert_eq!(identity_hash.as_slice(), expected(case), "{}", case.id);
        consumed += 1;
    }
    for case in of_kind(&file, "dest-hash") {
        let identity_hash = cert::identity_hash(&array(case, "identity_ed25519_pub_hex"));
        let dest_hash = cert::dest_hash(&identity_hash);
        assert_eq!(dest_hash.as_slice(), expected(case), "{}", case.id);
        consumed += 1;
    }
    assert_eq!(consumed, file.case_count(), "her vaka koşulmalı");
}

#[test]
fn compact_device_cert_matches_the_reference() {
    let file = suite(
        "ccr-compact-device-cert-v1",
        &["domain-sep", "issuer-key-id", "tbs", "encode"],
    );
    let mut consumed = 0usize;
    for case in of_kind(&file, "domain-sep") {
        let label = field(case, "label_hex");
        assert_eq!(DOMAIN_SEP_CDC_V1.as_slice(), expected(case), "{}", case.id);
        // ccr §13.2 YAMA 2026-10-11 kuralının kendisi: etiket, ardından `0x00` dolgu.
        assert_eq!(&DOMAIN_SEP_CDC_V1[..label.len()], label.as_slice());
        assert!(DOMAIN_SEP_CDC_V1[label.len()..].iter().all(|&b| b == 0));
        consumed += 1;
    }
    for case in of_kind(&file, "issuer-key-id") {
        let key_id = cert::issuer_key_id(&array(case, "issuer_ed25519_pub_hex"));
        assert_eq!(key_id.as_slice(), expected(case), "{}", case.id);
        consumed += 1;
    }
    for case in of_kind(&file, "tbs") {
        let tbs = fields_of(case).tbs();
        assert_eq!(tbs.as_slice(), expected(case), "{}", case.id);
        consumed += 1;
    }
    for case in of_kind(&file, "encode") {
        let fields = fields_of(case);
        let issuer = SigningKey::from_bytes(&array(case, "issuer_ed25519_seed_hex"));
        let compact = cert::sign_compact_device_cert(&fields, &issuer).expect("imza");
        assert_eq!(compact.as_slice(), expected(case), "{}", case.id);
        let (parsed, _) = cert::split_compact_device_cert(&compact);
        assert_eq!(parsed, fields, "{}", case.id);
        let issuer_pub = issuer.verifying_key().to_bytes();
        let verdict = cert::verify_compact_device_cert(&compact, &issuer_pub);
        assert_eq!(verdict, Ok(()), "{}", case.id);
        consumed += 1;
    }
    assert_eq!(consumed, file.case_count(), "her vaka koşulmalı");
}
