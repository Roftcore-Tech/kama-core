//! AMP vektörleri ↔ `kama_app::codec` (B2.1 kabulü: `vectors/amp-*.json` birebir).
//!
//! * `amp-record-v1.json` (`encode`): yapısal girdiden kurulan mesajın kodlaması `expected_hex`
//!   ile BAYT-ÖZDEŞ ve `expected_hex`'in çözümü aynı mesaj.
//! * `amp-record-reject-v1.json` (`decode`): çözüm, vektörün `(result, reason)` çiftini AYNEN
//!   verir.
//!
//! Beklenen baytların TAMAMI vektör dosyalarından gelir (Python referansı `ref/amp/encode.py`);
//! bu dosyada gömülü beklenen bayt yoktur.
//!
//! # "pending zstd kararı" (Abkarada, 2026-10-08)
//!
//! ruzstd REDDEDİLDİ; B2.1 bu sprint `compression_id` = NONE ile çalışır (gönderici sıkıştırmaz).
//! Sıkıştırılmış kayıt (flags.bit0 = 1) taşıyan BÜTÜN vakalar — F07.4 ret vakası dahil —
//! [`PENDING_ZSTD`] listesindedir: B2.1 kabul sayımına GİRMEZLER, ama SESSİZCE ATLANMAZLAR. Her biri
//! bugünkü tipli davranışla koşulur ve ayrıca sayılır: açma adımına ULAŞANLAR ([`NEEDS_DECODER`])
//! `Drop(zstd-decoder-unavailable)` verir; başlık denetiminde (açmadan ÖNCE) karar verilenler
//! vektörün `(result, reason)` çiftini AYNEN verir. zstd kararı gelince liste boşaltılır.

use kama_app::codec::{
    decode, encode, AmpLimits, AppMessage, DecodeError, DropReason, FileAbortReason,
    FileReceiptStatus, PttCodec, ReceiptKind,
};
use kama_contracts::s1::{CompressionPolicy, TransportCapabilities};
use kama_vectors::{Case, Expected, Input, RejectResult, VectorFile};

const POSITIVE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vectors/amp-record-v1.json"
);
const REJECT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../vectors/amp-record-reject-v1.json"
);
const KIND_ENCODE: &str = "encode";
const KIND_DECODE: &str = "decode";
const POSITIVE_ID_PREFIX: &str = "amp-record-v1.encode.";
/// [K4G-09] 0x0B `FILE_RECEIPT` pozitif vakalarının (her `status` için bir) ortak soneki.
const FILE_RECEIPT_CASE_PREFIX: &str = "file-receipt-";

/// Vektörlerin üretildiği test parametresi — kaynak: `ref/amp/encode.py`
/// `TEST_MAX_PLAINTEXT_UNIT_BYTES` (MAX_BODY_BYTES = MAX_DECOMPRESSED_MESSAGE = 1019).
const VECTOR_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;

/// Sıkıştırılmış kayıt taşıyan vakaların TAMAMI — "pending zstd kararı" (Abkarada 2026-10-08).
const PENDING_ZSTD: [&str; 8] = [
    "amp-record-v1.encode.text-compressed",
    "amp-record-reject-v1.decode.uncompressed-len-exceeds-max",
    "amp-record-reject-v1.decode.expansion-ratio-exceeded",
    "amp-record-reject-v1.decode.truncated-zstd-stream",
    "amp-record-reject-v1.decode.invalid-zstd-frame",
    "amp-record-reject-v1.decode.unknown-compression-id",
    "amp-record-reject-v1.decode.media-bundle-compressed-expansion-ratio-exceeded",
    // [S8 2026-10-08] §5 kural 2 — tek Raw bloklu (deterministik) zstd karesi; açma adımına ulaşır.
    "amp-record-reject-v1.decode.decompressed-len-mismatch",
];

/// [`PENDING_ZSTD`]'den başlık denetimlerini geçip açma adımına ULAŞANLAR (gerçek zstd karesi ister).
const NEEDS_DECODER: [&str; 4] = [
    "amp-record-v1.encode.text-compressed",
    "amp-record-reject-v1.decode.truncated-zstd-stream",
    "amp-record-reject-v1.decode.invalid-zstd-frame",
    "amp-record-reject-v1.decode.decompressed-len-mismatch",
];

/// Koşulan vaka sayısı; `pending` = [`PENDING_ZSTD`]'den koşulanlar (onlar da `consumed`'a dahil).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Tally {
    consumed: usize,
    pending: usize,
}

fn suite(path: &str, kind: &str) -> VectorFile {
    let file = kama_vectors::load_file(path).expect("AMP vektör dosyası yüklenmeli");
    // Vakum kapısı: tanımadığımız bir `kind` varsa vakalar sessizce atlanmaz, test düşer.
    file.require_kinds(&[kind])
        .expect("AMP dosyasında bu tüketicinin tanımadığı bir kind var");
    assert!(file.case_count() > 0, "{path}: vaka yok (vakum)");
    file
}

fn limits() -> AmpLimits {
    let caps = TransportCapabilities {
        max_plaintext_unit_bytes: VECTOR_MAX_PLAINTEXT_UNIT_BYTES,
        preferred_plaintext_unit_bytes: VECTOR_MAX_PLAINTEXT_UNIT_BYTES,
        max_early_unit_bytes: VECTOR_MAX_PLAINTEXT_UNIT_BYTES,
        reliable: true,
    };
    AmpLimits::from_capabilities(&caps).expect("vektör parametresi geçerli limit vermeli")
}

fn is_pending(case: &Case) -> bool {
    PENDING_ZSTD.contains(&case.id.as_str())
}

/// Ret vakası: çözüm vektörün `(result, reason)` çiftini AYNEN vermeli.
fn assert_rejected(
    case: &Case,
    record: &[u8],
    limits: &AmpLimits,
    result: RejectResult,
    reason: &str,
) {
    let Err(got) = decode(record, limits) else {
        panic!("{}: ret vakası kabul edildi", case.id);
    };
    let class_matches = matches!(
        (result, &got),
        (RejectResult::Drop, DecodeError::Drop(_)) | (RejectResult::Reject, DecodeError::Reject(_))
    );
    assert!(
        class_matches,
        "{}: beklenen {}, gelen {got:?}",
        case.id,
        result.as_str()
    );
    assert_eq!(got.reason(), reason, "{}: gerekçe", case.id);
}

/// "pending zstd kararı" vakasının bugünkü davranışı: açma adımına ulaşan → tipli
/// `zstd-decoder-unavailable`; başlıkta karar verilen → vektörün kendi `(result, reason)`'ı.
fn assert_pending_zstd(
    case: &Case,
    record: &[u8],
    limits: &AmpLimits,
    expected: Option<(RejectResult, &str)>,
) {
    if NEEDS_DECODER.contains(&case.id.as_str()) {
        assert_eq!(
            decode(record, limits),
            Err(DecodeError::Drop(DropReason::CompressionUnavailable)),
            "{}: pending zstd kararı — bugünkü tipli davranış değişti",
            case.id
        );
    } else {
        let (result, reason) = expected.unwrap_or_else(|| {
            panic!("{}: başlıkta karar verilen vaka ret vakası olmalı", case.id)
        });
        assert_rejected(case, record, limits, result, reason);
    }
}

// ── Yapısal girdi okuyucuları ────────────────────────────────────────────────────────────────

fn uint(case: &Case, name: &str) -> u64 {
    let Input::Structured(map) = &case.input else {
        panic!("{}: yapısal girdi bekleniyordu", case.id);
    };
    map.get(name)
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| panic!("{}: '{name}' yok ya da işaretsiz tam sayı değil", case.id))
}

fn u32_field(case: &Case, name: &str) -> u32 {
    u32::try_from(uint(case, name)).unwrap_or_else(|_| panic!("{}: '{name}' u32 değil", case.id))
}

fn u8_field(case: &Case, name: &str) -> u8 {
    u8::try_from(uint(case, name)).unwrap_or_else(|_| panic!("{}: '{name}' u8 değil", case.id))
}

fn text_field(case: &Case, name: &str) -> String {
    let Input::Structured(map) = &case.input else {
        panic!("{}: yapısal girdi bekleniyordu", case.id);
    };
    map.get(name)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("{}: '{name}' yok ya da dizge değil", case.id))
}

fn hex_field(case: &Case, name: &str) -> Vec<u8> {
    kama_vectors::hex(&text_field(case, name))
        .unwrap_or_else(|e| panic!("{}: '{name}' hex değil: {e:?}", case.id))
}

/// Pozitif vakanın yapısal girdisinden mesaj. Tip, vaka kimliğinin sonekinden okunur; tanınmayan
/// sonek testi düşürür (yeni vaka sessizce atlanmaz).
fn message_from_input(case: &Case) -> AppMessage {
    let name = case
        .id
        .strip_prefix(POSITIVE_ID_PREFIX)
        .unwrap_or_else(|| panic!("{}: beklenmeyen kimlik öneki", case.id));
    match name {
        "text" | "text-compressed" => AppMessage::Text {
            msg_id: u32_field(case, "msg_id"),
            text: text_field(case, "text"),
        },
        "file-begin" => AppMessage::FileBegin {
            transfer_id: uint(case, "transfer_id"),
            file_size: uint(case, "file_size"),
            whole_file_hash: hex_field(case, "whole_file_hash_hex")
                .try_into()
                .unwrap_or_else(|_| panic!("{}: whole_file_hash 32 bayt değil", case.id)),
            chunk_count: u32_field(case, "chunk_count"),
            name: text_field(case, "name"),
        },
        "file-chunk" => AppMessage::FileChunk {
            transfer_id: uint(case, "transfer_id"),
            offset: uint(case, "offset"),
            payload: hex_field(case, "payload_hex"),
        },
        "file-end" => AppMessage::FileEnd {
            transfer_id: uint(case, "transfer_id"),
        },
        // [K4G-09] `file-abort-superseded`: reason 0x05 superseded.
        "file-abort" | "file-abort-superseded" => AppMessage::FileAbort {
            transfer_id: uint(case, "transfer_id"),
            reason: FileAbortReason::from_code(u8_field(case, "reason"))
                .unwrap_or_else(|| panic!("{}: bilinmeyen FILE_ABORT.reason", case.id)),
        },
        // [K4G-09] 0x0B `FILE_RECEIPT`, her `status` için bir vaka.
        receipt if receipt.starts_with(FILE_RECEIPT_CASE_PREFIX) => AppMessage::FileReceipt {
            transfer_id: uint(case, "transfer_id"),
            status: FileReceiptStatus::from_code(u8_field(case, "status"))
                .unwrap_or_else(|| panic!("{}: bilinmeyen FILE_RECEIPT.status", case.id)),
        },
        "ptt-open" => AppMessage::PttOpen {
            stream_id: u32_field(case, "stream_id"),
            codec: PttCodec::from_code(u8_field(case, "codec"))
                .unwrap_or_else(|| panic!("{}: bilinmeyen PTT codec", case.id)),
            bundle_ms: u8_field(case, "bundle_ms"),
        },
        "ptt-close" => AppMessage::PttClose {
            stream_id: u32_field(case, "stream_id"),
        },
        "receipt-delivered" => AppMessage::Receipt {
            ref_msg_id: u32_field(case, "ref_msg_id"),
            kind: ReceiptKind::from_code(u8_field(case, "kind"))
                .unwrap_or_else(|| panic!("{}: bilinmeyen RECEIPT.kind", case.id)),
        },
        "signaling-blob" => AppMessage::SignalingBlob {
            blob: hex_field(case, "blob_hex"),
        },
        "media-bundle" => AppMessage::MediaBundle {
            stream_id: u32_field(case, "stream_id"),
            codec: u8_field(case, "codec"),
            bundle_ms: u8_field(case, "bundle_ms"),
            payload: hex_field(case, "payload_hex"),
        },
        other => panic!("{}: tanınmayan pozitif vaka '{other}'", case.id),
    }
}

// ── Koşucular ────────────────────────────────────────────────────────────────────────────────

fn run_positive() -> Tally {
    let file = suite(POSITIVE_PATH, KIND_ENCODE);
    let limits = limits();
    let mut tally = Tally::default();
    for case in &file.cases {
        let Expected::Wire(expected) = &case.expected else {
            panic!("{}: pozitif vaka expected_hex taşımalı", case.id);
        };
        let message = message_from_input(case);
        let unit =
            encode(&message, &limits).unwrap_or_else(|e| panic!("{}: kodlanamadı: {e}", case.id));
        // Gönderici bugün hiçbir tipi sıkıştırmaz (zstd kararı bekliyor).
        assert_eq!(
            unit.compression_policy,
            CompressionPolicy::Never,
            "{}: gönderici sıkıştırmamalı",
            case.id
        );
        if is_pending(case) {
            // Sıkıştırılmış beklenen bayt üretilemez ve açılamaz: tipli davranış + kendi kaydımızın
            // gidiş-dönüşü.
            assert_pending_zstd(case, expected, &limits, None);
            assert_eq!(
                decode(&unit.bytes, &limits),
                Ok(message),
                "{}: sıkıştırmasız gidiş-dönüş",
                case.id
            );
            tally.pending += 1;
        } else {
            assert_eq!(
                &unit.bytes, expected,
                "{}: encode bayt-özdeş değil",
                case.id
            );
            assert_eq!(
                decode(expected, &limits),
                Ok(message),
                "{}: expected_hex çözümü girdi mesajı değil",
                case.id
            );
        }
        tally.consumed += 1;
    }
    tally
}

fn run_reject() -> Tally {
    let file = suite(REJECT_PATH, KIND_DECODE);
    let limits = limits();
    let mut tally = Tally::default();
    for case in &file.cases {
        let Input::Wire(record) = &case.input else {
            panic!("{}: ret vakası input_hex taşımalı", case.id);
        };
        let Expected::Rejected { result, reason } = &case.expected else {
            panic!("{}: ret vakası expected.result/reason taşımalı", case.id);
        };
        if is_pending(case) {
            assert_pending_zstd(case, record, &limits, Some((*result, reason.as_str())));
            tally.pending += 1;
        } else {
            assert_rejected(case, record, &limits, *result, reason.as_str());
        }
        tally.consumed += 1;
    }
    tally
}

#[test]
fn positive_vectors_encode_byte_identical_and_decode_back() {
    run_positive();
}

#[test]
fn reject_vectors_match_result_and_reason() {
    run_reject();
}

/// [K4G-09] Kart: "5 pozitif (her status)". `FILE_RECEIPT.status` registry'sinin HER değeri pozitif
/// dosyada tam bir vakayla temsil edilir; yeni bir status vektörsüz kalamaz.
///
/// `d3bc7407`'de UYARLANAMAZ: `FileReceiptStatus` ve 0x0B vakaları yoktu.
#[test]
fn every_file_receipt_status_has_one_positive_vector() {
    let file = suite(POSITIVE_PATH, KIND_ENCODE);
    let mut statuses: Vec<FileReceiptStatus> = file
        .cases
        .iter()
        .filter(|case| {
            case.id
                .strip_prefix(POSITIVE_ID_PREFIX)
                .is_some_and(|name| name.starts_with(FILE_RECEIPT_CASE_PREFIX))
        })
        .map(|case| match message_from_input(case) {
            AppMessage::FileReceipt { status, .. } => status,
            other => panic!("{}: FILE_RECEIPT bekleniyordu, gelen {other:?}", case.id),
        })
        .collect();
    statuses.sort_by_key(|status| status.code());
    assert_eq!(statuses, FileReceiptStatus::ALL, "her status tam bir kez");
}

/// Dosyalardaki HER vaka koşuldu ve zstd bekleyenlerin hepsi tam bir kez görüldü.
#[test]
fn every_case_is_consumed() {
    let positive = run_positive();
    let reject = run_reject();
    assert_eq!(
        positive.consumed,
        suite(POSITIVE_PATH, KIND_ENCODE).case_count(),
        "amp-record-v1: koşulan vaka sayısı dosyanın beyan ettiğinden farklı"
    );
    assert_eq!(
        reject.consumed,
        suite(REJECT_PATH, KIND_DECODE).case_count(),
        "amp-record-reject-v1: koşulan vaka sayısı dosyanın beyan ettiğinden farklı"
    );
    assert_eq!(
        positive.pending + reject.pending,
        PENDING_ZSTD.len(),
        "pending zstd kararı listesindeki her kimlik dosyalarda tam bir kez bulunmalı"
    );
    // Kabul sayımı: pending olmayanların hepsi koşuldu (B2.1 kabulü bu sayıya bağlıdır).
    assert_eq!(
        positive.consumed + reject.consumed - positive.pending - reject.pending,
        suite(POSITIVE_PATH, KIND_ENCODE).case_count()
            + suite(REJECT_PATH, KIND_DECODE).case_count()
            - PENDING_ZSTD.len(),
        "kabul sayımına giren vaka sayısı"
    );
}
