//! Golden-vector (K-FEC-WIRE-1, §14): canonical-source → PINNED expected-repair-byte + reconstructed-
//! METADATA. İkinci-bağımsız-referans = pinned-byte-dizisi → reed-solomon-simd sürüm/backend değişince
//! DRIFT yakalanır (iki-bağımsız-impl-emsali). §6: recompute_repair(esi) == encode()[esi] == golden.
//!
//! Profil 0x01 (K=2, R=2, E=64). Source = canonical-source-symbol (prefix+payload+zero-pad, §2.6/§6.5).

use kama_fec_codec::{by_id, FecCodec, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use kama_transport_schema::prefix::SourcePrefix;

/// 64B golden. Hex çözümü `kama_vectors::hex` (vectors/SCHEMA.json sözleşmesi: küçük harf,
/// çift uzunluk, 0x yok); buradaki tek ek kural pinlenmiş uzunluktur.
fn hex(s: &str) -> Vec<u8> {
    assert_eq!(s.len(), 128, "golden = 64B = 128 hex");
    kama_vectors::hex(s).expect("golden hex")
}

fn canonical_sym(mid: u32, off: u32, payload: &[u8], fin: bool) -> Vec<u8> {
    let p = SourcePrefix {
        message_id: mid,
        chunk_offset: off,
        actual_payload_len: payload.len() as u16,
        source_flags: u8::from(fin),
    };
    let mut s = vec![0u8; 64];
    p.serialize_symbol(payload, &mut s).unwrap();
    s
}

// PINNED golden (reed-solomon-simd 3.1.0, CODE_SUITE_RS_SIMD_V1). Değişirse = kasıtlı-onaylı-güncelleme.
const GOLDEN_REPAIR0: &str = "010203040000001c001102676f6c64656e2d5a6d59e96a609f5969d7e10000000000000000000000000000000000000000000000000000000000000000000000";
const GOLDEN_REPAIR1: &str = "0102030400000029001303676f6c64656e2d466c49b66f6cdc4260a58e0000000000000000000000000000000000000000000000000000000000000000000000";

fn golden_block() -> (Vec<u8>, Vec<u8>) {
    let s0 = canonical_sym(0x0102_0304, 0, b"golden-source-zero", false);
    let s1 = canonical_sym(0x0102_0304, 53, b"golden-one-final", true);
    (s0, s1)
}

#[test]
fn golden_repair_bytes_pinned() {
    let c = RsSimdCodec;
    let p = by_id(0x01).unwrap();
    let (s0, s1) = golden_block();
    let src = vec![
        SourceSymbol { esi: 0, bytes: &s0 },
        SourceSymbol { esi: 1, bytes: &s1 },
    ];
    let reps = c.encode(&p, &src).unwrap();
    assert_eq!(
        reps[0].bytes,
        hex(GOLDEN_REPAIR0),
        "repair0 golden-drift-gate"
    );
    assert_eq!(
        reps[1].bytes,
        hex(GOLDEN_REPAIR1),
        "repair1 golden-drift-gate"
    );
}

#[test]
fn golden_recompute_equals_full_encode_and_golden() {
    // §6 on-demand-recompute: recompute_repair(esi) == encode()[esi] == golden.
    let c = RsSimdCodec;
    let p = by_id(0x01).unwrap();
    let (s0, s1) = golden_block();
    let src = vec![
        SourceSymbol { esi: 0, bytes: &s0 },
        SourceSymbol { esi: 1, bytes: &s1 },
    ];
    let full = c.encode(&p, &src).unwrap();
    for esi in 0..2u16 {
        let one = c.recompute_repair(&p, &src, esi).unwrap();
        assert_eq!(
            one.bytes,
            full[usize::from(esi)].bytes,
            "recompute == full-encode[esi]"
        );
    }
    assert_eq!(
        c.recompute_repair(&p, &src, 1).unwrap().bytes,
        hex(GOLDEN_REPAIR1),
        "recompute == golden"
    );
    // Range dışı ESI (R=2) → fail-closed.
    assert!(c.recompute_repair(&p, &src, 2).is_err());
}

#[test]
fn golden_metadata_recovery() {
    // K-FEC-WIRE-1: golden-vector reconstructed-METADATA'yı da kapsar (§6.5).
    let c = RsSimdCodec;
    let p = by_id(0x01).unwrap();
    let (s0, s1) = golden_block();
    let src = vec![
        SourceSymbol { esi: 0, bytes: &s0 },
        SourceSymbol { esi: 1, bytes: &s1 },
    ];
    let reps = c.encode(&p, &src).unwrap();
    // source0 düş; source1 + repair0 → reconstruct source0 (byte + metadata).
    let recv = vec![
        ReceivedSymbol {
            esi: 1,
            is_repair: false,
            bytes: &s1,
        },
        ReceivedSymbol {
            esi: 0,
            is_repair: true,
            bytes: &reps[0].bytes,
        },
    ];
    let rec = c.reconstruct(&p, &recv, 0).unwrap();
    assert_eq!(rec.len(), 1);
    assert_eq!(
        rec[0].bytes, s0,
        "recovered source0 == golden SRC0 (byte-byte)"
    );
    let pfx = SourcePrefix::parse(&rec[0].bytes).unwrap();
    assert_eq!(pfx.message_id, 0x0102_0304);
    assert_eq!(pfx.chunk_offset, 0);
    assert_eq!(pfx.source_flags, 0);
    assert_eq!(pfx.payload_slice(&rec[0].bytes), b"golden-source-zero");
}
