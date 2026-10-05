//! SSD1 vektörleri ↔ mevcut Rust mühür yolu — G-S3 kanıtı (iş emri 2026-10-05 §3).
//!
//! `vectors/ccr-ssd1-v1/ccr-ssd1-v1.json` Rust'tan BAĞIMSIZ Python referansından üretilir
//! (`ref/crypto/gen_vectors.py`; CI her PR'da yeniden üretip byte-byte karşılaştırır). Bu dosya o
//! vektörleri bugünkü ürün koduna — `LegacySessionProtector` (22 B SESSION_DATA zarfı, ccr §35.6 /
//! §35.8 / §35.9) — koşturur:
//!
//! * `seal` vakaları: AAD, nonce96 ve mühürlü wire byte'ları vektörle BAYT-ÖZDEŞ;
//! * `open` vakaları: kabul edilenlerin plaintext'i ve reddedilenlerin karar/gerekçesi vektörle aynı;
//!   reddedilen paket alım penceresini ilerletmez.
//!
//! Beklenen değerlerin TAMAMI vektör dosyasından gelir; bu dosyada gömülü beklenen byte yoktur
//! (`scripts/ci/no_inline_expected.py`).
//!
//! KAPSAM DIŞI (tüketicisi yok, gerçek kod yazılınca): `ccr-kdf-v1` → A5.1; `ccr-ssi1-v1` /
//! `ccr-ssr1-v1` → A5.5. Bu süit mock/real hedeflerinden bağımsızdır: sınadığı şey S3 trait'leri
//! değil, bugün var olan legacy mühür fonksiyonudur.

use kama_noise_wire::protector::nonce96;
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

/// Vakanın oturum parametreleriyle bir protector. `send` = bu ucun gönderdiği yön.
fn protector(case: &Case, send: LegacyDirection) -> LegacySessionProtector {
    let key = Zeroizing::new(array::<32>(case, "key_hex"));
    LegacySessionProtector::new(
        be(&field(case, "hub_handle_hex")) as u32,
        &key,
        &key,
        be(&field(case, "protocol_version_hex")) as u8,
        be(&field(case, "cipher_suite_id_hex")) as u16,
        be(&field(case, "epoch_hex")) as u16,
        array::<16>(case, "session_id_internal_hex"),
        send,
    )
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
pub fn seal_is_byte_identical() -> usize {
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
            be(&field(case, "protocol_version_hex")) as u8,
            be(&field(case, "cipher_suite_id_hex")) as u16,
            dir.code(),
            be(&field(case, "epoch_hex")) as u16,
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

        let wire = protector(case, dir)
            .seal(&payload, counter)
            .unwrap_or_else(|e| panic!("{}: seal reddedildi: {e:?}", case.id));
        assert_eq!(
            &wire, want,
            "{}: mühürlü wire byte'ları vektörden farklı",
            case.id
        );
    }
    picked.len()
}

/// `open` — kabul/ret kararı, plaintext ve gerekçe vektörle aynı; ret pencereyi ilerletmez.
pub fn open_matches_expected_decisions() -> usize {
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
        let mut sender = protector(case, dir);
        let mut receiver = protector(case, dir.opposite());
        for pair in seen.windows(2) {
            assert!(pair[0] < pair[1], "{}: seen_counters artan olmalı", case.id);
        }
        let mut seed = |receiver: &mut LegacySessionProtector, counter: u64| {
            let wire = sender
                .seal(b"tohum", counter)
                .unwrap_or_else(|e| panic!("{}: tohum {counter} mühürlenemedi: {e:?}", case.id));
            receiver
                .open(&wire)
                .unwrap_or_else(|e| panic!("{}: tohum {counter} açılamadı: {e:?}", case.id));
        };
        // İlk `seen` sayacı yapısal sınırın ötesindeyse alıcı oraya TEK sıçramada varamaz (seq16
        // yanlış geri kazanılır). Gerçek bir oturumda oraya ara paketlerle gelinir; burada da öyle:
        // basamak taşları atılır. Taşlar son pencerenin DIŞINDA kalır (ilk `seen`'den en az pencere
        // boyu kadar geride), dolayısıyla "hiç görülmemiş çok eski sayaç"tan ayırt edilemezler ve
        // vektörün tarif ettiği state'i değiştirmezler.
        if let Some(first) = seen.first().copied() {
            let mut stone = SEED_STEP;
            while first - receiver.recv_expected_pn() >= STRUCTURAL_LIMIT {
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
        let state_reproduced = receiver.recv_expected_pn() == expected_counter;

        let before = receiver.recv_expected_pn();
        match (&case.expected, receiver.open(&wire)) {
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
                    receiver.recv_expected_pn(),
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
pub fn every_case_is_consumed() {
    let consumed = seal_is_byte_identical() + open_matches_expected_decisions();
    assert_eq!(
        consumed,
        suite().case_count(),
        "koşulan vaka sayısı dosyanın beyan ettiğinden farklı"
    );
}
