//! L-A5c — el sıkışma vektörleri ↔ Rust: `ccr-ssi1-v1`, `ccr-ssr1-v1`, `ccr-ed25519-neg-v1`,
//! `ccr-deprecated-neg-v1` (Python referansı `ref/crypto/gen_vectors.py`; Rust'tan BAĞIMSIZ).
//!
//! * `aad` / `seal`: AAD ve mühürlü wire baytları vektörle BAYT-ÖZDEŞ; ardından karşı tarafın
//!   açması aynı veriyi verir (iki yönün de tutarlılığı).
//! * `open`: ret sınıfı (`drop` / `reject`) ve gerekçe vektörle aynı. AEAD hatası tek tiptir
//!   ([`HandshakeError::AeadFail`]); vektör gerekçesi onu üreten maddeyi adlandırır
//!   (`v-ssi-3-aead-fail`, `v-ssi-4-aead-fail`, `v-ssr-3-aead-fail`) — eşleme `-aead-fail` sonekiyle.
//! * `verify` (Ed25519) ve `decode` (eski formatlar): ilk düşen kuralın gerekçesi aynı.
//!
//! Beklenen değerlerin TAMAMI vektör dosyalarından gelir; bu dosyada gömülü beklenen bayt yoktur.
//! Her dosyanın her vakası koşulur (`*_every_case_is_consumed`).

use kama_noise_wire::deprecated::{deprecated_format, DeprecatedFormat};
use kama_noise_wire::ed25519_strict::{verify_strict, Ed25519Reject};
use kama_noise_wire::handshake::{
    dh, open_ssi1, open_ssr1, parse_ssi1, seal_ssi1, seal_ssr1, session_context_v1, ssi1_aad,
    ssi1_cert_mode, ssr1_aad, CertMode, HandshakeError, Ssi1Seal, Ssr1Seal,
};
use kama_noise_wire::kdf::{self, InitRoot};
use kama_noise_wire::nonce96;
use kama_vectors::{Case, Expected, Input, RejectResult, VectorFile};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

fn path(suite: &str) -> String {
    format!(
        "{}/../../vectors/{suite}/{suite}.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn suite(name: &str, kinds: &[&str]) -> VectorFile {
    let file = kama_vectors::load_file(path(name)).expect("vektör dosyası yüklenmeli");
    file.require_kinds(kinds)
        .expect("dosyada bu tüketicinin tanımadığı bir kind var");
    file
}

fn cases<'a>(file: &'a VectorFile, kind: &str) -> Vec<&'a Case> {
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
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{}: '{name}' alanı yok", case.id));
    kama_vectors::hex(text).unwrap_or_else(|e| panic!("{}: '{name}' hex değil: {e:?}", case.id))
}

fn array<const N: usize>(case: &Case, name: &str) -> [u8; N] {
    field(case, name)
        .as_slice()
        .try_into()
        .unwrap_or_else(|_| panic!("{}: '{name}' {N} bayt olmalı", case.id))
}

fn be(bytes: &[u8]) -> u64 {
    assert!(bytes.len() <= 8);
    bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

fn secret(case: &Case, name: &str) -> StaticSecret {
    StaticSecret::from(array::<32>(case, name))
}

fn want_wire(case: &Case) -> &[u8] {
    let Expected::Wire(w) = &case.expected else {
        panic!("{}: pozitif vaka wire baytı beklemeli", case.id);
    };
    w
}

fn want_reject(case: &Case) -> (RejectResult, &str) {
    let Expected::Rejected { result, reason } = &case.expected else {
        panic!("{}: negatif vaka ret beklemeli", case.id);
    };
    (*result, reason.as_str())
}

/// Gözlenen ret ↔ vektör. `aead` bayrağı: gözlenen AEAD hatasıdır, gerekçe maddeyi adlandırır.
fn assert_reject(case: &Case, observed: (RejectResult, &str), aead: bool) {
    let (result, reason) = want_reject(case);
    assert_eq!(observed.0, result, "{}: ret sınıfı", case.id);
    if aead {
        assert!(
            reason.ends_with("-aead-fail"),
            "{}: AEAD hatası gözlendi, vektör '{reason}' bekliyor",
            case.id
        );
    } else {
        assert_eq!(observed.1, reason, "{}: gerekçe", case.id);
    }
}

/// `HandshakeError` → (sınıf, gerekçe, AEAD mi). Sözlükte karşılığı olmayan hata testi düşürür.
fn observed(case: &Case, e: HandshakeError) -> ((RejectResult, &'static str), bool) {
    let r = RejectResult::Reject;
    match e {
        HandshakeError::FlagsReservedNonzero => ((r, "ssi1-flags-reserved-nonzero"), false),
        HandshakeError::InitVariantReserved => ((r, "ssi1-init-variant-reserved"), false),
        HandshakeError::CertModeReserved => ((r, "ssi1-cert-mode-reserved"), false),
        HandshakeError::LengthMismatch => ((r, "v-ssi-5-length-mismatch"), false),
        HandshakeError::Ssr1FlagsNonzero => ((r, "ssr1-flags-nonzero"), false),
        HandshakeError::Ssr1TooShort => ((r, "ssr1-length-below-minimum"), false),
        HandshakeError::PublicKeyAllZero => ((r, "x25519-public-key-all-zero"), false),
        HandshakeError::SharedSecretAllZero => ((r, "x25519-shared-secret-all-zero"), false),
        HandshakeError::AeadFail => ((RejectResult::Drop, "aead-fail"), true),
        other => panic!(
            "{}: vektör sözlüğünde karşılığı olmayan hata: {other:?}",
            case.id
        ),
    }
}

// ── ccr-ssi1-v1 ──────────────────────────────────────────────────────────────────────────────

const SSI1: &str = "ccr-ssi1-v1";

/// Vakanın dünyası: alıcı statik anahtarı vektördeki genel anahtarla tutarlı olmalı.
fn ssi1_world(case: &Case) -> (StaticSecret, PublicKey, [u8; 32], StaticSecret, [u8; 32]) {
    let recipient = secret(case, "recipient_static_sk_hex");
    let recipient_pub = PublicKey::from(&recipient);
    assert_eq!(
        recipient_pub.as_bytes(),
        &array::<32>(case, "recipient_static_pub_hex"),
        "{}: alıcı genel anahtarı sırrıyla tutarsız",
        case.id
    );
    (
        recipient,
        recipient_pub,
        array::<32>(case, "recipient_dest_hash_hex"),
        secret(case, "sender_static_sk_hex"),
        array::<32>(case, "sender_cert_ref_hex"),
    )
}

/// Alıcı tarafından türetilen `session_id_internal` (AAD vakaları için; kök iki tarafta aynıdır).
fn ssi1_sid(case: &Case) -> [u8; 16] {
    let (_, recipient_pub, dest, sender_static, cert_ref) = ssi1_world(case);
    let eph = secret(case, "sender_ephemeral_sk_hex");
    let ss_e = dh(&eph, &recipient_pub).unwrap();
    let ss_s = dh(&sender_static, &recipient_pub).unwrap();
    let ctx = session_context_v1(&cert_ref, &dest, &recipient_pub);
    let root = kdf::d1_init_root(&ss_e, &ss_s, &ctx).unwrap();
    kdf::d3_session_id_internal(&root, &ctx).unwrap()
}

fn ssi1_aad_cases() -> usize {
    let file = suite(SSI1, &["aad", "seal", "open"]);
    let picked = cases(&file, "aad");
    for case in &picked {
        let flags = field(case, "flags_hex")[0];
        let eph = PublicKey::from(&secret(case, "sender_ephemeral_sk_hex"));
        let aad = ssi1_aad(
            flags,
            &array::<32>(case, "recipient_dest_hash_hex"),
            eph.as_bytes(),
            &field(case, "cert_field_hex"),
            be(&field(case, "init_counter_hex")),
            &ssi1_sid(case),
        );
        assert_eq!(aad.as_slice(), want_wire(case), "{}: AAD", case.id);
    }
    picked.len()
}

fn ssi1_seal_cases() -> usize {
    let file = suite(SSI1, &["aad", "seal", "open"]);
    let picked = cases(&file, "seal");
    for case in &picked {
        let (recipient, recipient_pub, dest, sender_static, cert_ref) = ssi1_world(case);
        let flags = field(case, "flags_hex")[0];
        let cert_mode = ssi1_cert_mode(flags).expect("pozitif vakanın bayrakları geçerli");
        assert_eq!(
            field(case, "nonce_hex"),
            nonce96(0),
            "{}: nonce96(0)",
            case.id
        );
        let eph = secret(case, "sender_ephemeral_sk_hex");
        let cert_field = field(case, "cert_field_hex");
        let early = field(case, "early_data_hex");
        let (wire, sealed) = seal_ssi1(&Ssi1Seal {
            cert_mode,
            recipient_dest_hash: &dest,
            recipient_static_pub: &recipient_pub,
            sender_static: &sender_static,
            sender_ephemeral: &eph,
            sender_cert_ref: &cert_ref,
            cert_field: &cert_field,
            init_counter: be(&field(case, "init_counter_hex")),
            early_data: &early,
        })
        .unwrap_or_else(|e| panic!("{}: seal reddedildi: {e:?}", case.id));
        assert_eq!(
            wire.as_slice(),
            want_wire(case),
            "{}: mühürlü wire",
            case.id
        );

        // Karşı yön: alıcı açar ve aynı sırlara varır. Bundle kipinde vektörün sertifika
        // baytları OPAK desenli dolgudur (ccr §20 yapısı değildir) → ayrıştırıcı reddeder.
        if cert_mode == CertMode::Bundle {
            assert_eq!(
                parse_ssi1(&wire).unwrap_err(),
                HandshakeError::BundleMalformed,
                "{}: opak bundle baytları ccr §20 yapısı değil",
                case.id
            );
            continue;
        }
        let sender_pub = PublicKey::from(&sender_static);
        let opened = open_ssi1(&wire, &recipient, &dest, |r| {
            assert_eq!(r, &cert_ref, "{}: gönderen kimliği", case.id);
            Some(sender_pub)
        })
        .unwrap_or_else(|e| panic!("{}: açılamadı: {e:?}", case.id));
        assert_eq!(opened.early_data, early, "{}: early_data", case.id);
        assert_eq!(
            opened.secrets.session_id_internal, sealed.session_id_internal,
            "{}: iki taraf aynı oturum kimliğine varmalı",
            case.id
        );
    }
    picked.len()
}

fn ssi1_open_cases() -> usize {
    let file = suite(SSI1, &["aad", "seal", "open"]);
    let picked = cases(&file, "open");
    for case in &picked {
        let (recipient, _, dest, sender_static, _) = ssi1_world(case);
        let sender_pub = PublicKey::from(&sender_static);
        let wire = field(case, "wire_hex");
        let err = open_ssi1(&wire, &recipient, &dest, |_| Some(sender_pub))
            .err()
            .unwrap_or_else(|| panic!("{}: negatif vaka açıldı", case.id));
        let (obs, aead) = observed(case, err);
        assert_reject(case, obs, aead);
    }
    picked.len()
}

#[test]
fn ssi1_aad_is_byte_identical() {
    ssi1_aad_cases();
}

#[test]
fn ssi1_seal_is_byte_identical_and_opens() {
    ssi1_seal_cases();
}

#[test]
fn ssi1_open_rejects_match() {
    ssi1_open_cases();
}

#[test]
fn ssi1_every_case_is_consumed() {
    let n = ssi1_aad_cases() + ssi1_seal_cases() + ssi1_open_cases();
    assert_eq!(n, suite(SSI1, &["aad", "seal", "open"]).case_count());
}

// ── ccr-ssr1-v1 ──────────────────────────────────────────────────────────────────────────────

const SSR1: &str = "ccr-ssr1-v1";

fn init_root(case: &Case) -> InitRoot {
    InitRoot::from_bytes(Zeroizing::new(array::<32>(case, "init_root_hex")))
}

fn ssr1_aad_cases() -> usize {
    let file = suite(SSR1, &["aad", "seal", "open"]);
    let picked = cases(&file, "aad");
    for case in &picked {
        let eph = PublicKey::from(&secret(case, "responder_ephemeral_sk_hex"));
        let aad = ssr1_aad(
            field(case, "flags_hex")[0],
            eph.as_bytes(),
            be(&field(case, "response_counter_hex")),
            &array::<16>(case, "session_id_internal_hex"),
        );
        assert_eq!(aad.as_slice(), want_wire(case), "{}: AAD", case.id);
    }
    picked.len()
}

fn ssr1_seal_cases() -> usize {
    let file = suite(SSR1, &["aad", "seal", "open"]);
    let picked = cases(&file, "seal");
    for case in &picked {
        assert_eq!(field(case, "flags_hex"), [0x00], "{}: SSR1 flags", case.id);
        assert_eq!(
            field(case, "nonce_hex"),
            nonce96(0),
            "{}: nonce96(0)",
            case.id
        );
        let init_eph = secret(case, "initiator_ephemeral_sk_hex");
        let init_static = secret(case, "initiator_static_sk_hex");
        let sid = array::<16>(case, "session_id_internal_hex");
        let data = field(case, "response_data_hex");
        let root = init_root(case);
        let (wire, _) = seal_ssr1(&Ssr1Seal {
            init_root: &root,
            session_id_internal: &sid,
            initiator_ephemeral: &PublicKey::from(&init_eph),
            initiator_static: &PublicKey::from(&init_static),
            responder_ephemeral: &secret(case, "responder_ephemeral_sk_hex"),
            response_counter: be(&field(case, "response_counter_hex")),
            response_data: &data,
        })
        .unwrap_or_else(|e| panic!("{}: seal reddedildi: {e:?}", case.id));
        assert_eq!(
            wire.as_slice(),
            want_wire(case),
            "{}: mühürlü wire",
            case.id
        );

        let opened = open_ssr1(&wire, &root, &sid, &init_eph, &init_static)
            .unwrap_or_else(|e| panic!("{}: başlatan açamadı: {e:?}", case.id));
        assert_eq!(opened.response_data, data, "{}: yanıt verisi", case.id);
    }
    picked.len()
}

fn ssr1_open_cases() -> usize {
    let file = suite(SSR1, &["aad", "seal", "open"]);
    let picked = cases(&file, "open");
    for case in &picked {
        let err = open_ssr1(
            &field(case, "wire_hex"),
            &init_root(case),
            &array::<16>(case, "session_id_internal_hex"),
            &secret(case, "initiator_ephemeral_sk_hex"),
            &secret(case, "initiator_static_sk_hex"),
        )
        .err()
        .unwrap_or_else(|| panic!("{}: negatif vaka açıldı", case.id));
        let (obs, aead) = observed(case, err);
        assert_reject(case, obs, aead);
    }
    picked.len()
}

#[test]
fn ssr1_aad_is_byte_identical() {
    ssr1_aad_cases();
}

#[test]
fn ssr1_seal_is_byte_identical_and_opens() {
    ssr1_seal_cases();
}

#[test]
fn ssr1_open_rejects_match() {
    ssr1_open_cases();
}

#[test]
fn ssr1_every_case_is_consumed() {
    let n = ssr1_aad_cases() + ssr1_seal_cases() + ssr1_open_cases();
    assert_eq!(n, suite(SSR1, &["aad", "seal", "open"]).case_count());
}

// ── ccr-ed25519-neg-v1 ───────────────────────────────────────────────────────────────────────

const ED25519_NEG: &str = "ccr-ed25519-neg-v1";

fn ed25519_reason(case: &Case, r: Ed25519Reject) -> &'static str {
    match r {
        Ed25519Reject::SNotBelowGroupOrder => "ed25519-s-not-below-group-order",
        Ed25519Reject::NonCanonicalPointEncoding => "ed25519-non-canonical-point-encoding",
        Ed25519Reject::SmallOrderPublicKey => "ed25519-small-order-public-key",
        Ed25519Reject::SmallOrderR => "ed25519-small-order-r",
        other => panic!(
            "{}: vektör sözlüğünde karşılığı olmayan ret: {other:?}",
            case.id
        ),
    }
}

#[test]
fn ed25519_negatives_reject_with_the_first_failing_rule() {
    let file = suite(ED25519_NEG, &["verify"]);
    let picked = cases(&file, "verify");
    for case in &picked {
        let r = verify_strict(
            &field(case, "public_key_hex"),
            &field(case, "message_hex"),
            &field(case, "signature_hex"),
        )
        .err()
        .unwrap_or_else(|| panic!("{}: negatif imza kabul edildi", case.id));
        assert_reject(case, (RejectResult::Reject, ed25519_reason(case, r)), false);
    }
    assert_eq!(picked.len(), file.case_count(), "her vaka koşulmalı");
}

// ── ccr-deprecated-neg-v1 ────────────────────────────────────────────────────────────────────

const DEPRECATED_NEG: &str = "ccr-deprecated-neg-v1";

fn deprecated_reason(f: DeprecatedFormat) -> &'static str {
    match f {
        DeprecatedFormat::AsyncInit => "deprecated-sai1",
        DeprecatedFormat::AsyncAccept => "deprecated-saa1",
        DeprecatedFormat::AsyncData => "deprecated-sad1",
        DeprecatedFormat::AsyncControl => "deprecated-sac1",
        DeprecatedFormat::AsyncMessage => "deprecated-sram",
        DeprecatedFormat::SignedNoisePrekey => "deprecated-reject-1-srpk",
        DeprecatedFormat::PrekeyBundle => "deprecated-reject-2-srpb",
    }
}

#[test]
fn deprecated_formats_are_rejected_by_magic() {
    let file = suite(DEPRECATED_NEG, &["decode"]);
    let picked = cases(&file, "decode");
    for case in &picked {
        let Input::Wire(input) = &case.input else {
            panic!("{}: ham girdi bekleniyordu", case.id);
        };
        let f = deprecated_format(input)
            .unwrap_or_else(|| panic!("{}: eski format tanınmadı", case.id));
        assert_reject(case, (RejectResult::Reject, deprecated_reason(f)), false);
        // Aktif SSI1 ayrıştırıcısı da kabul DÖNDÜREMEZ (ret gerekçesi orada bayraklardır).
        assert!(
            parse_ssi1(input).is_err(),
            "{}: SSI1 olarak ayrıştı",
            case.id
        );
    }
    assert_eq!(picked.len(), file.case_count(), "her vaka koşulmalı");
}
