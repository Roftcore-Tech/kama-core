//! ACK REFERANS DÜZENEĞİ · (a) sabit kodlama vektörleri — `adaptive_transport::ack` ↔
//! `vectors/transport/transport-ack-frame-v1.json`.
//!
//! Beklenen baytların tamamı vektör dosyasından gelir; dosya Rust'tan bağımsız üretilir
//! (`ref/transport/gen_ack_vectors.py`). Bu dosya hiçbir beklenen bayt yazmaz. BAYT eşitliği
//! YALNIZ burada, bugünkü sabit kodlama için iddia edilir; anlamsal özdeşlik (decode(encode(x)) == x)
//! `crates/adaptive-transport/tests/ack_ref_codec.rs`'tedir.
//!
//! Vaka türleri (sözlük üreticinin başlığında):
//! * `from-received`: alınan küme → `AckFrame::from_received` → baytlar == `expected_hex`; çözülen
//!   çerçevenin teyit kümesi (`acked_full`) == girdideki `acked` (bilgi kümesi).
//! * `encode-fields`: alanlardan kurulan çerçeve → baytlar == `expected_hex`; çöz → aynı alanlar.
//! * `decode`: pozitifte çöz → yeniden kodla == `expected_hex`; negatifte tipli hata, kararlı adı
//!   `expected.reason` ile tam-dizge aynı.
//!
//! Her girdi alanı tüketilmek zorundadır (kullanılmayan alan = sessiz kayma → kırmızı).

use std::path::PathBuf;

use adaptive_transport::ack::{AckFrame, AckRange};
use adaptive_transport::error::TransportError;
use kama_vectors::{Case, Expected, Input, RejectResult};

const KINDS: &[&str] = &["from-received", "encode-fields", "decode"];

fn reason(e: &TransportError) -> &'static str {
    match e {
        TransportError::TooManyAckRanges { .. } => "too-many-ack-ranges",
        TransportError::Truncated { .. } => "truncated",
        TransportError::TrailingBytes { .. } => "trailing-bytes",
        TransportError::UnknownFrameType(_) => "unknown-frame-type",
        _ => "unexpected-error",
    }
}

/// Girdi alanını bir kez alır (tüketilmeyen alan sonda kırmızı).
macro_rules! take {
    ($m:expr, $id:expr, $k:literal) => {
        $m.remove($k)
            .unwrap_or_else(|| panic!("{}: girdi alanı yok: {}", $id, $k))
    };
}
macro_rules! take_u64 {
    ($m:expr, $id:expr, $k:literal) => {
        take!($m, $id, $k)
            .as_u64()
            .unwrap_or_else(|| panic!("{}: {} sayı değil", $id, $k))
    };
}
macro_rules! take_u64s {
    ($m:expr, $id:expr, $k:literal) => {
        take!($m, $id, $k)
            .as_array()
            .unwrap_or_else(|| panic!("{}: {} dizi değil", $id, $k))
            .iter()
            .map(|x| x.as_u64().expect("sayı"))
            .collect::<Vec<u64>>()
    };
}

fn want_wire(c: &Case) -> &[u8] {
    match &c.expected {
        Expected::Wire(b) => b,
        Expected::Rejected { .. } => panic!("{}: pozitif vaka bekleniyordu", c.id),
    }
}

fn run_case(c: &Case) {
    match c.kind.as_str() {
        "from-received" => {
            let Input::Structured(m) = &c.input else {
                panic!("{}: yapısal girdi bekleniyordu", c.id)
            };
            let mut m = m.clone();
            let id = &c.id;
            let recv = take_u64s!(m, id, "received");
            let acked = take_u64s!(m, id, "acked");
            let frame = AckFrame::from_received(
                &recv,
                take_u64!(m, id, "ack_epoch") as u8,
                take_u64!(m, id, "ack_delay") as u16,
                take_u64!(m, id, "seen_count") as u32,
                take_u64!(m, id, "arrival_rate") as u16,
            )
            .expect("boş olmayan küme");
            assert!(m.is_empty(), "{id}: tüketilmeyen alan {:?}", m.keys());
            assert_eq!(frame.encode(), want_wire(c), "{}: bayt", c.id);
            let g = AckFrame::decode(want_wire(c)).expect("çöz");
            let largest = *recv.iter().max().expect("boş değil");
            let mut got = g.acked_full(largest + 1);
            got.sort_unstable();
            assert_eq!(got, acked, "{}: teyit kümesi (bilgi kümesi)", c.id);
        }
        "encode-fields" => {
            let Input::Structured(m) = &c.input else {
                panic!("{}: yapısal girdi bekleniyordu", c.id)
            };
            let mut m = m.clone();
            let id = &c.id;
            let ft = take_u64!(m, id, "frame_type");
            let ranges: Vec<AckRange> = take!(m, id, "ranges")
                .as_array()
                .expect("dizi")
                .iter()
                .map(|p| {
                    let p = p.as_array().expect("[gap, len]");
                    AckRange {
                        gap: p[0].as_u64().expect("gap") as u16,
                        len: p[1].as_u64().expect("len") as u16,
                    }
                })
                .collect();
            let frame = AckFrame {
                ack_epoch: take_u64!(m, id, "ack_epoch") as u8,
                largest_seq: take_u64!(m, id, "largest_seq") as u16,
                ack_delay: take_u64!(m, id, "ack_delay") as u16,
                block0_len: take_u64!(m, id, "block0_len") as u16,
                ranges,
                v2: ft == 0x14,
                seen_count: take_u64!(m, id, "seen_count") as u32,
                arrival_rate: take_u64!(m, id, "arrival_rate") as u16,
                bitmap: None,
            };
            assert!(m.is_empty(), "{id}: tüketilmeyen alan {:?}", m.keys());
            assert!(ft == 0x14 || ft == 0x10, "{}: tür", c.id);
            assert_eq!(frame.encode(), want_wire(c), "{}: bayt", c.id);
            assert_eq!(
                AckFrame::decode(want_wire(c)).expect("çöz"),
                frame,
                "{}: çöz → aynı alanlar",
                c.id
            );
        }
        "decode" => {
            let Input::Wire(b) = &c.input else {
                panic!("{}: ham girdi bekleniyordu", c.id)
            };
            match &c.expected {
                Expected::Wire(want) => {
                    let g = AckFrame::decode(b)
                        .unwrap_or_else(|e| panic!("{}: çözülmeliydi: {e:?}", c.id));
                    assert_eq!(&g.encode(), want, "{}: yeniden kodlama", c.id);
                }
                Expected::Rejected { result, reason: r } => {
                    assert_eq!(*result, RejectResult::Reject, "{}", c.id);
                    match AckFrame::decode(b) {
                        Err(e) => assert_eq!(reason(&e), r, "{}: gerekçe ({e:?})", c.id),
                        Ok(g) => panic!("{}: reddedilmeliydi, çözüldü: {g:?}", c.id),
                    }
                }
            }
        }
        k => panic!("{}: bilinmeyen tür {k}", c.id),
    }
}

#[test]
fn ack_ref_fixed_vectors() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vectors/transport/transport-ack-frame-v1.json");
    let file = kama_vectors::load_file(&path).expect("vektör dosyası yüklenmeli");
    file.require_kinds(KINDS).expect("bilinmeyen tür yok");
    let mut ran = 0usize;
    for c in &file.cases {
        run_case(c);
        ran += 1;
    }
    assert_eq!(ran, file.case_count(), "vakum: koşulan vaka sayısı");
    assert!(ran >= 20, "vakum: {ran}");
    println!("ACK-REF vektör: {ran} vaka koştu");
}
