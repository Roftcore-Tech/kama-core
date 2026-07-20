//! Golden-vector v0 (kama-transport-schema): §2.6 prefix + §8 PARAM-canonical byte-eşitliği +
//! §2.56 inner_kind + metadata-recoverability. İki-bağımsız-yol aynı-byte; crate-update drift-gate.

use kama_transport_schema::inner_kind::{classify, InnerKind, ShardKind};
use kama_transport_schema::param::{ParamFields, PARAM_DS};
use kama_transport_schema::prefix::{SourcePrefix, SOURCE_FLAG_FINAL};
use kama_transport_schema::{
    SchemaError, EXTENDED_OUTER_OVERHEAD, LEGACY_SESSION_DATA_OVERHEAD, SOURCE_PREFIX_LEN,
};

// ── §2.6 prefix: canonical byte-layout GOLDEN (elle-hesaplı big-endian) ──
#[test]
fn golden_source_prefix_bytes() {
    let e = 64usize;
    let payload = b"kama"; // 4B
    let p = SourcePrefix {
        message_id: 0x0102_0304,
        chunk_offset: 0x0000_0040,
        actual_payload_len: 4,
        source_flags: SOURCE_FLAG_FINAL,
    };
    let mut out = vec![0xAAu8; e];
    p.serialize_symbol(payload, &mut out).unwrap();
    // GOLDEN prefix (11B): msg 01 02 03 04 · off 00 00 00 40 · len 00 04 · flags 01
    assert_eq!(
        &out[0..11],
        &[0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x40, 0x00, 0x04, 0x01]
    );
    assert_eq!(&out[11..15], b"kama"); // payload
    assert!(out[15..].iter().all(|&b| b == 0), "canonical zero-pad"); // §2.6 pad
    assert_eq!(out.len(), e, "symbol_bytes == E (symbol_len KİLİDİ)");
}

// ── metadata-recoverability (K-FEC-META-1): prefix reconstruct'ta geri gelir ──
#[test]
fn metadata_survives_roundtrip() {
    let e = 32usize;
    let payload = b"payload-11B";
    let orig = SourcePrefix {
        message_id: 0xDEAD_BEEF,
        chunk_offset: 0x0000_1000,
        actual_payload_len: payload.len() as u16,
        source_flags: 0,
    };
    let mut sym = vec![0u8; e];
    orig.serialize_symbol(payload, &mut sym).unwrap();
    // "kayıp-source RS-reconstruct" → symbol-byte'lar döndü varsay; prefix'ten metadata geri:
    let rec = SourcePrefix::parse(&sym).unwrap();
    assert_eq!(rec, orig, "message_id/offset/len/flags recover");
    assert_eq!(rec.payload_slice(&sym), payload);
}

#[test]
fn reserved_flags_rejected() {
    let p = SourcePrefix {
        message_id: 1,
        chunk_offset: 0,
        actual_payload_len: 0,
        source_flags: 0x80,
    };
    let mut out = vec![0u8; 16];
    assert!(matches!(
        p.serialize_symbol(&[], &mut out),
        Err(SchemaError::ReservedFlagsSet(0x80))
    ));
}

#[test]
fn payload_too_long_fail_closed() {
    let e = 16usize; // cap = 5
    let p = SourcePrefix {
        message_id: 0,
        chunk_offset: 0,
        actual_payload_len: 6,
        source_flags: 0,
    };
    let mut out = vec![0u8; e];
    assert!(matches!(
        p.serialize_symbol(&[9u8; 6], &mut out),
        Err(SchemaError::PayloadTooLong { .. })
    ));
}

// ── §8 PARAM canonical-hash-input GOLDEN + drift-gate ──
#[test]
fn golden_param_canonical_input() {
    let f = ParamFields {
        protocol_version: 0x01,
        fec_version: 0x01,
        code_suite_id: 0x01,
        profile_id: 0x02,
        k_shape: 8,
        r_code_max: 4,
        e: 64,
        partial_policy: 0x00,
        packet_scheduling_policy: 0x00,
    };
    let got = f.canonical_hash_input().unwrap();
    // GOLDEN: DS || 01 01 01 02 | 00 08 | 00 04 | 00 40 | 00 00
    let mut want = Vec::new();
    want.extend_from_slice(PARAM_DS);
    want.extend_from_slice(&[
        0x01, 0x01, 0x01, 0x02, 0x00, 0x08, 0x00, 0x04, 0x00, 0x40, 0x00, 0x00,
    ]);
    assert_eq!(got, want, "§8 canonical field-serializer drift-gate");
    assert_eq!(&got[..PARAM_DS.len()], b"KAMA-PARAM-V1");
}

// PARAM-hash byte'ları: BLAKE2s-256(canonical-input) — hash binding-katmanında; golden dev-dep ile.
#[test]
fn golden_param_blake2s_hash() {
    use blake2::{Blake2s256, Digest};
    let f = ParamFields {
        protocol_version: 1,
        fec_version: 1,
        code_suite_id: 1,
        profile_id: 2,
        k_shape: 8,
        r_code_max: 4,
        e: 64,
        partial_policy: 0,
        packet_scheduling_policy: 0,
    };
    let input = f.canonical_hash_input().unwrap();
    let mut h = Blake2s256::new();
    h.update(&input);
    let digest: [u8; 32] = h.finalize().into();
    // Determinizm-golden: aynı-input → aynı-32B; iki-bağımsız-üretim aynı (crate-update drift-gate).
    let mut h2 = Blake2s256::new();
    h2.update(f.canonical_hash_input().unwrap());
    assert_eq!(
        digest,
        <[u8; 32]>::from(h2.finalize()),
        "PARAM-hash deterministik"
    );
    assert_eq!(digest.len(), 32, "BLAKE2s-256 = 32B");
}

#[test]
fn b4_invariant_fail_closed() {
    let bad_k = ParamFields {
        protocol_version: 1,
        fec_version: 1,
        code_suite_id: 1,
        profile_id: 1,
        k_shape: 256,
        r_code_max: 4,
        e: 64,
        partial_policy: 0,
        packet_scheduling_policy: 0,
    };
    assert!(matches!(
        bad_k.validate(),
        Err(SchemaError::ProfileBound(_))
    ));
    let bad_e = ParamFields {
        e: 10,
        k_shape: 8,
        ..bad_k
    };
    assert!(matches!(
        bad_e.validate(),
        Err(SchemaError::ProfileBound(_))
    ));
}

// ── §2.56 inner_kind registry: classify + fail-closed reserved ──
#[test]
fn inner_kind_registry() {
    assert_eq!(classify(0x01).unwrap(), InnerKind::Fec(ShardKind::Source));
    assert_eq!(classify(0x02).unwrap(), InnerKind::Fec(ShardKind::Repair));
    assert_eq!(classify(0x10).unwrap(), InnerKind::Control(0x10));
    assert_eq!(classify(0x18).unwrap(), InnerKind::Control(0x18));
    // fail-closed reserved codepoint'ler (§2.56)
    for cp in [0x00u8, 0x03, 0x0F, 0x19, 0xFF] {
        assert!(
            matches!(classify(cp), Err(SchemaError::UnknownInnerKind(_))),
            "cp {cp:#04x} reject"
        );
    }
}

// ── overhead sabitleri (§2.6 D0-B gate sealed_len) ──
#[test]
fn wire_overhead_constants() {
    assert_eq!(EXTENDED_OUTER_OVERHEAD, 26); // h_ext4+wf1+flags1+pn4+tag16
    assert_eq!(LEGACY_SESSION_DATA_OVERHEAD, 22); // hub_handle4+seq2+tag16
    assert_eq!(SOURCE_PREFIX_LEN, 11); // msg4+off4+len2+flags1
}
