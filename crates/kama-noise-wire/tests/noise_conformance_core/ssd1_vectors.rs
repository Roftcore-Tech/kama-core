//! SSD1 vektörleri ↔ Rust mühür yolları — G-S3 kanıtı (iş emri 2026-10-05 §3) ve L-A5b kabulü.
//!
//! `vectors/ccr-ssd1-v1/ccr-ssd1-v1.json` Rust'tan BAĞIMSIZ Python referansından üretilir
//! (`ref/crypto/gen_vectors.py`; CI her PR'da yeniden üretip byte-byte karşılaştırır). Aynı vaka
//! gövdeleri iki mühür yoluna koşar ([`Ssd1Path`]; 22 B SESSION_DATA zarfı, ccr §35.6 / §35.8 /
//! §35.9):
//!
//! * [`LegacyPath`] — bugünkü `LegacySessionProtector` (#90'ın G-S3 kanıtı; `--target vectors`);
//! * [`RealPath`] — GERÇEK S3 yolu: `NoiseWireProtection` + legacy zarf profili (L-A5b; `--target
//!   real`). Mühür sayacı Crypto'nundur; vaka sayacına test kancası ileri taşır.
//!
//! İddialar:
//! * `seal` vakaları: AAD, nonce96 ve mühürlü wire byte'ları vektörle BAYT-ÖZDEŞ;
//! * `open` vakaları: kabul edilenlerin plaintext'i ve reddedilenlerin karar/gerekçesi vektörle aynı;
//!   reddedilen paket alım penceresini ilerletmez.
//!
//! Beklenen değerlerin TAMAMI vektör dosyasından gelir; bu dosyada gömülü beklenen byte yoktur
//! (`scripts/ci/no_inline_expected.py`).
//!
//! KAPSAM DIŞI: `ccr-kdf-v1` → A5.1 (`kdf.rs` testleri); `ccr-ssi1-v1` / `ccr-ssr1-v1` → A5.5.

use kama_contracts::handles::SessionHandle;
use kama_contracts::s3::{HubHandle, PacketProtection, ProtectionContext};
use kama_noise_wire::envelope::EnvelopeProfile;
use kama_noise_wire::protector::{nonce96, NoiseWireProtection, SessionKeyMaterial};
use kama_noise_wire::{
    build_legacy_aad, LegacyDirection, LegacySessionProtector, PnReject, WireError,
    LEGACY_PN_PARAMS,
};
use kama_vectors::{Case, Expected, Input, RejectResult, VectorFile};
use zeroize::Zeroizing;

const SUITE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vectors/ccr-ssd1-v1/ccr-ssd1-v1.json"
);
const KIND_SEAL: &str = "seal";
const KIND_OPEN: &str = "open";

/// seq16'nın tek-anlamlı kaldığı yapısal sınır: |full − expected| < 2^(genişlik−1) (ccr §35.8).
const STRUCTURAL_LIMIT: u64 = 1 << (LEGACY_PN_PARAMS.low_width_bits - 1);
/// Alıcıyı uzak bir sayaca taşırken atılan adım; yapısal sınırın yarısı (rahat pay).
const SEED_STEP: u64 = STRUCTURAL_LIMIT / 2;

fn suite() -> VectorFile {
    let file = kama_vectors::load_file(SUITE_PATH).expect("SSD1 vektör dosyası yüklenmeli");
    // Vakum kapısı: tanımadığımız bir `kind` varsa o vakalar sessizce atlanmaz, test düşer.
    file.require_kinds(&[KIND_SEAL, KIND_OPEN])
        .expect("SSD1 dosyasında bu tüketicinin tanımadığı bir kind var");
    file
}

fn cases<'a>(file: &'a VectorFile, kind: &str) -> Vec<&'a Case> {
    let picked: Vec<&Case> = file.cases.iter().filter(|c| c.kind == kind).collect();
    assert!(!picked.is_empty(), "'{kind}' türünde vaka yok (vakum)");
    picked
}

/// Yapısal girdiden hex alanı.
fn field(case: &Case, name: &str) -> Vec<u8> {
    let Input::Structured(map) = &case.input else {
        panic!("{}: yapısal girdi bekleniyordu", case.id);
    };
    let text = map
        .get(name)
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("{}: '{name}' alanı yok ya da dizge değil", case.id));
    kama_vectors::hex(text).unwrap_or_else(|e| panic!("{}: '{name}' hex değil: {e:?}", case.id))
}

/// Yapısal girdiden hex dizge listesi.
fn field_list(case: &Case, name: &str) -> Vec<Vec<u8>> {
    let Input::Structured(map) = &case.input else {
        panic!("{}: yapısal girdi bekleniyordu", case.id);
    };
    map.get(name)
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("{}: '{name}' alanı yok ya da liste değil", case.id))
        .iter()
        .map(|v| {
            let text = v
                .as_str()
                .unwrap_or_else(|| panic!("{}: '{name}' öğesi dizge değil", case.id));
            kama_vectors::hex(text)
                .unwrap_or_else(|e| panic!("{}: '{name}' hex değil: {e:?}", case.id))
        })
        .collect()
}

/// Big-endian tamsayı (≤ 8 bayt).
fn be(bytes: &[u8]) -> u64 {
    assert!(bytes.len() <= 8, "alan sekiz bayttan uzun");
    bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
}

fn array<const N: usize>(case: &Case, name: &str) -> [u8; N] {
    let v = field(case, name);
    v.as_slice()
        .try_into()
        .unwrap_or_else(|_| panic!("{}: '{name}' {N} bayt olmalı, {} geldi", case.id, v.len()))
}

fn direction(case: &Case) -> LegacyDirection {
    let code = be(&field(case, "direction_hex"));
    [LegacyDirection::AToB, LegacyDirection::BToA]
        .into_iter()
        .find(|d| u64::from(d.code()) == code)
        .unwrap_or_else(|| panic!("{}: bilinmeyen yön kodu {code}", case.id))
}

/// Vakaların koşturulduğu mühür yolu — aynı vaka gövdeleri, iki yol (R-7(c) kalıbı).
pub trait Ssd1Path {
    type Endpoint;
    /// Vakanın oturum parametreleriyle bir uç; `send` = bu ucun GÖNDERDİĞİ yön.
    fn endpoint(case: &Case, send: LegacyDirection) -> Self::Endpoint;
    /// `payload`'ı TAM `counter` sayacıyla mühürler.
    fn seal(case: &Case, e: &mut Self::Endpoint, payload: &[u8], counter: u64) -> Vec<u8>;
    /// Açar. Ret yolunda alım state'inin değişmediğini yol kendisi de iddia eder.
    ///
    /// # Errors
    /// Yolun typed reddi.
    fn open(case: &Case, e: &mut Self::Endpoint, wire: &[u8]) -> Result<Vec<u8>, WireError>;
    /// Alım penceresinin `expected_pn`'i (en yüksek kabul + 1).
    fn expected_pn(e: &Self::Endpoint) -> u64;
}

fn hub_handle(case: &Case) -> u32 {
    u32::try_from(be(&field(case, "hub_handle_hex"))).expect("hub_handle 4 bayt")
}

fn protocol_version(case: &Case) -> u8 {
    u8::try_from(be(&field(case, "protocol_version_hex"))).expect("protocol_version 1 bayt")
}

fn cipher_suite_id(case: &Case) -> u16 {
    u16::try_from(be(&field(case, "cipher_suite_id_hex"))).expect("cipher_suite_id 2 bayt")
}

fn epoch(case: &Case) -> u16 {
    u16::try_from(be(&field(case, "epoch_hex"))).expect("epoch 2 bayt")
}

/// Bugünkü legacy mühür fonksiyonu (`LegacySessionProtector`).
pub struct LegacyPath;

impl Ssd1Path for LegacyPath {
    type Endpoint = LegacySessionProtector;

    fn endpoint(case: &Case, send: LegacyDirection) -> Self::Endpoint {
        let key = Zeroizing::new(array::<32>(case, "key_hex"));
        LegacySessionProtector::new(
            hub_handle(case),
            &key,
            &key,
            protocol_version(case),
            cipher_suite_id(case),
            epoch(case),
            array::<16>(case, "session_id_internal_hex"),
            send,
        )
    }

    fn seal(case: &Case, e: &mut Self::Endpoint, payload: &[u8], counter: u64) -> Vec<u8> {
        e.seal(payload, counter)
            .unwrap_or_else(|err| panic!("{}: seal {counter} reddedildi: {err:?}", case.id))
    }

    fn open(_: &Case, e: &mut Self::Endpoint, wire: &[u8]) -> Result<Vec<u8>, WireError> {
        e.open(wire)
    }

    fn expected_pn(e: &Self::Endpoint) -> u64 {
        e.recv_expected_pn()
    }
}

/// GERÇEK S3 yolu: `NoiseWireProtection` + legacy zarf profili. Vakanın `epoch_hex`'i 1 ise
/// oturum epoch-1'e geçirilir (anahtar = vakanın anahtarı) ve epoch-0 alımı kapatılır: legacy
/// zarfta epoch alanı wire'da YOKTUR, alıcı TEK anahtar dener (≤2 deneme A5.4'ün).
pub struct RealPath;

impl Ssd1Path for RealPath {
    type Endpoint = (NoiseWireProtection, SessionHandle);

    fn endpoint(case: &Case, send: LegacyDirection) -> Self::Endpoint {
        let key = || Zeroizing::new(array::<32>(case, "key_hex"));
        let mut p = NoiseWireProtection::new();
        let h = p
            .install_session_for_test(SessionKeyMaterial {
                profile: EnvelopeProfile::Legacy {
                    hub_handle: HubHandle::from_raw(hub_handle(case)),
                    protocol_version: protocol_version(case),
                    cipher_suite_id: cipher_suite_id(case),
                    session_id_internal: array::<16>(case, "session_id_internal_hex"),
                    send_direction: send,
                },
                send_e0: key(),
                recv_e0: key(),
            })
            .unwrap_or_else(|| panic!("{}: oturum kurulamadı", case.id));
        match epoch(case) {
            0 => {}
            1 => {
                assert!(
                    p.install_epoch1_for_test(h, &key(), &key())
                        && p.set_send_epoch_for_test(h, 1)
                        && p.discard_epoch0_recv_for_test(h),
                    "{}: epoch-1'e geçilemedi",
                    case.id
                );
            }
            other => panic!("{}: desteklenmeyen epoch {other}", case.id),
        }
        (p, h)
    }

    fn seal(case: &Case, e: &mut Self::Endpoint, payload: &[u8], counter: u64) -> Vec<u8> {
        let (p, h) = e;
        let ctx = ProtectionContext::Session(*h);
        assert!(
            p.set_next_send_pn_for_test(*h, counter),
            "{}: sayaç {counter}'a taşınamadı",
            case.id
        );
        let unit = p
            .seal(ctx, payload)
            .unwrap_or_else(|err| panic!("{}: seal {counter} reddedildi: {err:?}", case.id));
        assert_eq!(
            unit.meta.full_wire_pn, counter,
            "{}: meta.full_wire_pn kullanılan sayaç değil",
            case.id
        );
        assert_eq!(
            usize::try_from(unit.meta.sealed_len).ok(),
            Some(unit.bytes.len()),
            "{}: meta.sealed_len",
            case.id
        );
        assert_eq!(
            p.sealed_len(ctx, payload.len()),
            Ok(unit.bytes.len()),
            "{}: sealed_len(ctx, n) ≠ seal(ctx, f).len()",
            case.id
        );
        unit.bytes
    }

    fn open(case: &Case, e: &mut Self::Endpoint, wire: &[u8]) -> Result<Vec<u8>, WireError> {
        let (p, h) = e;
        let before = p.state_fingerprint();
        // Typed gerekçe saf aşamadan (commit'siz); karar `open`'ın kendisinden.
        let typed = p.inspect_open_for_test(wire);
        match (typed, p.open(wire)) {
            (Ok((pn, _, frame)), Some(opened)) => {
                assert_eq!(
                    opened.ctx,
                    ProtectionContext::Session(*h),
                    "{}: bağlam",
                    case.id
                );
                assert_eq!(
                    opened.frame, frame,
                    "{}: saf aşama ↔ open çerçevesi",
                    case.id
                );
                assert_eq!(
                    opened.meta.full_wire_pn, pn,
                    "{}: meta.full_wire_pn",
                    case.id
                );
                Ok(opened.frame)
            }
            (Err(err), None) => {
                assert_eq!(
                    p.state_fingerprint(),
                    before,
                    "{}: ret state'i değiştirdi (H-NO-MUTATE)",
                    case.id
                );
                Err(err)
            }
            (typed, opened) => panic!(
                "{}: saf aşama ({typed:?}) ile open ({:?}) ayrıştı",
                case.id,
                opened.map(|o| o.ctx)
            ),
        }
    }

    fn expected_pn(e: &Self::Endpoint) -> u64 {
        e.0.recv_expected_pn_for_test(e.1)
            .expect("oturumun alım penceresi var")
    }
}

/// Uygulamanın reddini vektör sözlüğüne çevirir. Sözlükte karşılığı olmayan bir hata testi düşürür.
fn observed(case: &Case, e: &WireError) -> (RejectResult, &'static str) {
    match e {
        WireError::PnReject(PnReject::Duplicate) => (RejectResult::Drop, "ssd1-replay-duplicate"),
        WireError::PnReject(PnReject::TooOld) => (RejectResult::Drop, "ssd1-replay-too-old"),
        WireError::AeadFail => (RejectResult::Drop, "ssd1-aead-fail"),
        WireError::DatagramTooShort { .. } => (RejectResult::Reject, "ssd1-length-below-minimum"),
        other => panic!(
            "{}: vektör sözlüğünde karşılığı olmayan hata: {other:?}",
            case.id
        ),
    }
}

/// `seal` — AAD, nonce96 ve wire byte'ları vektörle bayt-özdeş.
pub fn seal_is_byte_identical<P: Ssd1Path>() -> usize {
    let file = suite();
    let picked = cases(&file, KIND_SEAL);
    for case in &picked {
        let Expected::Wire(want) = &case.expected else {
            panic!("{}: seal vakası wire byte'ı beklemeli", case.id);
        };
        let dir = direction(case);
        let counter = be(&field(case, "full_counter_hex"));
        let payload = field(case, "payload_hex");

        let aad = build_legacy_aad(
            protocol_version(case),
            cipher_suite_id(case),
            dir.code(),
            epoch(case),
            (counter & 0xFFFF) as u16,
            &array::<16>(case, "session_id_internal_hex"),
        );
        assert_eq!(aad.as_slice(), field(case, "aad_hex"), "{}: AAD", case.id);
        assert_eq!(
            nonce96(counter).as_slice(),
            field(case, "nonce_hex"),
            "{}: nonce96",
            case.id
        );

        let wire = P::seal(case, &mut P::endpoint(case, dir), &payload, counter);
        assert_eq!(
            &wire, want,
            "{}: mühürlü wire byte'ları vektörden farklı",
            case.id
        );
    }
    picked.len()
}

/// `open` — kabul/ret kararı, plaintext ve gerekçe vektörle aynı; ret pencereyi ilerletmez.
pub fn open_matches_expected_decisions<P: Ssd1Path>() -> usize {
    let file = suite();
    let picked = cases(&file, KIND_OPEN);
    for case in &picked {
        let dir = direction(case);
        let wire = field(case, "wire_hex");
        let expected_counter = be(&field(case, "expected_counter_hex"));
        let seen: Vec<u64> = field_list(case, "seen_counters_hex")
            .iter()
            .map(|b| be(b))
            .collect();

        // Alıcının state'i vektörün tarif ettiği gibi kurulur: aynı anahtarlı bir gönderen `seen`
        // sayaçlarını mühürler, alıcı açar.
        let mut sender = P::endpoint(case, dir);
        let mut receiver = P::endpoint(case, dir.opposite());
        for pair in seen.windows(2) {
            assert!(pair[0] < pair[1], "{}: seen_counters artan olmalı", case.id);
        }
        let mut seed = |receiver: &mut P::Endpoint, counter: u64| {
            let wire = P::seal(case, &mut sender, b"tohum", counter);
            P::open(case, receiver, &wire)
                .unwrap_or_else(|e| panic!("{}: tohum {counter} açılamadı: {e:?}", case.id));
        };
        // İlk `seen` sayacı yapısal sınırın ötesindeyse alıcı oraya TEK sıçramada varamaz (seq16
        // yanlış geri kazanılır). Gerçek bir oturumda oraya ara paketlerle gelinir; burada da öyle:
        // basamak taşları atılır. Taşlar son pencerenin DIŞINDA kalır (ilk `seen`'den en az pencere
        // boyu kadar geride), dolayısıyla "hiç görülmemiş çok eski sayaç"tan ayırt edilemezler ve
        // vektörün tarif ettiği state'i değiştirmezler.
        if let Some(first) = seen.first().copied() {
            let mut stone = SEED_STEP;
            while first - P::expected_pn(&receiver) >= STRUCTURAL_LIMIT {
                assert!(
                    stone + LEGACY_PN_PARAMS.window < first,
                    "{}: basamak taşı son pencerenin içine düşüyor",
                    case.id
                );
                seed(&mut receiver, stone);
                stone += SEED_STEP;
            }
        }
        for c in &seen {
            seed(&mut receiver, *c);
        }
        let state_reproduced = P::expected_pn(&receiver) == expected_counter;

        let before = P::expected_pn(&receiver);
        match (&case.expected, P::open(case, &mut receiver, &wire)) {
            (Expected::Wire(want), Ok(plain)) => {
                assert!(state_reproduced, "{}: alıcı state'i kurulamadı", case.id);
                assert_eq!(&plain, want, "{}: plaintext", case.id);
            }
            (Expected::Rejected { result, reason }, Err(e)) => {
                let (got_result, got_reason) = observed(case, &e);
                assert_eq!(got_reason, reason, "{}: ret gerekçesi", case.id);
                assert_eq!(got_result, *result, "{}: ret davranışı", case.id);
                // State'e bakmadan (uzunluk) verilen ret dışında, vektörün tarif ettiği state
                // kurulmuş olmalı — yoksa "doğru gerekçe" tesadüf olabilir.
                assert!(
                    state_reproduced || matches!(e, WireError::DatagramTooShort { .. }),
                    "{}: alıcı state'i kurulamadı (beklenen sayaç {expected_counter}, kurulan {before})",
                    case.id
                );
                assert_eq!(
                    P::expected_pn(&receiver),
                    before,
                    "{}: reddedilen paket pencereyi ilerletti",
                    case.id
                );
            }
            (want, got) => panic!("{}: beklenen {want:?}, gelen {got:?}", case.id),
        }
    }
    picked.len()
}

/// Dosyadaki HER vaka bu tüketici tarafından koşuldu (filtreleyip azını koşmak yeşil görünmesin).
pub fn every_case_is_consumed<P: Ssd1Path>() {
    let consumed = seal_is_byte_identical::<P>() + open_matches_expected_decisions::<P>();
    assert_eq!(
        consumed,
        suite().case_count(),
        "koşulan vaka sayısı dosyanın beyan ettiğinden farklı"
    );
}
