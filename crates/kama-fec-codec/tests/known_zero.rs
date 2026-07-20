//! Model-A known-zero-injection (fec-core §6, NORMATİF) + metadata-recoverability (§6.5) POC.
//! Zorunlu-POC-seti (§6): K_shape=8×{K_actual=1,7} · yalnız-repair · mixed · known-zero-injection ·
//! wrong-K_actual-fail-closed · final-block-padding-recovery. §6.5: reconstructed-METADATA testi.
//!
//! Model-A: sender [0,K_actual) real-source + [K_actual,K_shape) canonical-zero'yu ENCODE'a katar
//! (zero'ları wire'a GÖNDERMEZ); receiver known_zero_count=K_shape−K_actual canonical-zero'yu decoder'a
//! bilinen-source enjekte eder. Decode-koşulu: received_source + received_repair + known_zero ≥ K_shape.

use kama_fec_codec::{by_id, FecCodec, FecError, ReceivedSymbol, RsSimdCodec, SourceSymbol};
use kama_transport_schema::prefix::{SourcePrefix, SOURCE_FLAG_FINAL};

/// Bir real-source'un metadata + payload'ı (K_actual adet; kalan K_shape−K_actual = canonical-zero).
struct SrcMeta {
    message_id: u32,
    chunk_offset: u32,
    payload: Vec<u8>,
    final_flag: bool,
}

/// Model-A blok kur: [0,K_actual) canonical-source-symbol (prefix+payload+zero-pad), [K_actual,K_shape)
/// canonical-zero (all-zero E-byte). Dönen Vec = E-byte owned symbol'ler (encode girdisi).
fn build_block(k_shape: usize, e: usize, metas: &[SrcMeta]) -> Vec<Vec<u8>> {
    assert!(metas.len() <= k_shape);
    let mut block = Vec::with_capacity(k_shape);
    for m in metas {
        let flags = if m.final_flag { SOURCE_FLAG_FINAL } else { 0 };
        let prefix = SourcePrefix {
            message_id: m.message_id,
            chunk_offset: m.chunk_offset,
            actual_payload_len: m.payload.len() as u16,
            source_flags: flags,
        };
        let mut sym = vec![0u8; e];
        prefix.serialize_symbol(&m.payload, &mut sym).unwrap();
        block.push(sym);
    }
    while block.len() < k_shape {
        block.push(vec![0u8; e]); // canonical-zero-pad (wire'a gönderilmez)
    }
    block
}

fn src_syms(block: &[Vec<u8>]) -> Vec<SourceSymbol<'_>> {
    block
        .iter()
        .enumerate()
        .map(|(i, b)| SourceSymbol {
            esi: i as u16,
            bytes: b,
        })
        .collect()
}

fn payloads(metas: &[SrcMeta]) -> Vec<SrcMeta> {
    metas
        .iter()
        .map(|m| SrcMeta {
            message_id: m.message_id,
            chunk_offset: m.chunk_offset,
            payload: m.payload.clone(),
            final_flag: m.final_flag,
        })
        .collect()
}

// ── K_shape=8 × K_actual=1 (known_zero=7): tek real-source, kaybı 1 repair ile kurtarılır ──
#[test]
fn known_zero_k8_kactual1() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8, R=4, E=64
    let metas = [SrcMeta {
        message_id: 0xAABB_CCDD,
        chunk_offset: 0,
        payload: vec![9u8; 40],
        final_flag: true,
    }];
    let block = build_block(8, 64, &metas);
    let repairs = c.encode(&p, &src_syms(&block)).unwrap();

    // source0 DÜŞ; yalnız repair0 admit. known_zero=7. admitted 0+1, +7 = 8 ≥ 8.
    let recv = vec![ReceivedSymbol {
        esi: 0,
        is_repair: true,
        bytes: &repairs[0].bytes,
    }];
    let rec = c.reconstruct(&p, &recv, 7).unwrap();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].esi, 0);
    assert_eq!(rec[0].bytes, block[0], "recovered source0 byte-byte eş");
    // Metadata da geri geldi (§6.5).
    let pfx = SourcePrefix::parse(&rec[0].bytes).unwrap();
    assert_eq!(pfx.message_id, 0xAABB_CCDD);
    assert_eq!(pfx.actual_payload_len, 40);
    assert_eq!(pfx.source_flags, SOURCE_FLAG_FINAL);
    assert_eq!(pfx.payload_slice(&rec[0].bytes), &[9u8; 40]);
}

// ── K_shape=8 × K_actual=7 (known_zero=1): 1 real-source kaybı 1 repair ile ──
#[test]
fn known_zero_k8_kactual7() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap();
    let metas: Vec<SrcMeta> = (0..7)
        .map(|i| SrcMeta {
            message_id: 0x1000 + i as u32,
            chunk_offset: (i as u32) * 40,
            payload: vec![i as u8; 40],
            final_flag: i == 6,
        })
        .collect();
    let block = build_block(8, 64, &metas);
    let repairs = c.encode(&p, &src_syms(&block)).unwrap();

    // esi 3 DÜŞ; kalan 6 real-source + 1 repair. known_zero=1. 7 + 1 = 8 ≥ 8.
    let mut recv: Vec<ReceivedSymbol> = (0..7)
        .filter(|i| *i != 3)
        .map(|i| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: &block[i],
        })
        .collect();
    recv.push(ReceivedSymbol {
        esi: 0,
        is_repair: true,
        bytes: &repairs[0].bytes,
    });
    let rec = c.reconstruct(&p, &recv, 1).unwrap();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].esi, 3);
    assert_eq!(rec[0].bytes, block[3]);
    let pfx = SourcePrefix::parse(&rec[0].bytes).unwrap();
    assert_eq!(pfx.message_id, 0x1003);
    assert_eq!(pfx.chunk_offset, 120);
    assert_eq!(pfx.payload_slice(&rec[0].bytes), &[3u8; 40]);
}

// ── yalnız-repair: tüm real-source düş, R repair + known_zero ile kurtarma ──
#[test]
fn only_repair_with_known_zero() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8, R=4
    let metas: Vec<SrcMeta> = (0..2)
        .map(|i| SrcMeta {
            message_id: 0x50 + i as u32,
            chunk_offset: 0,
            payload: vec![(i + 1) as u8; 20],
            final_flag: false,
        })
        .collect();
    let block = build_block(8, 64, &metas); // K_actual=2, known_zero=6
    let repairs = c.encode(&p, &src_syms(&block)).unwrap();

    // İki real-source da düş; 2 repair admit. admitted 0+2, +6 = 8 ≥ 8.
    let recv: Vec<ReceivedSymbol> = repairs
        .iter()
        .take(2)
        .map(|r| ReceivedSymbol {
            esi: r.esi,
            is_repair: true,
            bytes: &r.bytes,
        })
        .collect();
    let rec = c.reconstruct(&p, &recv, 6).unwrap();
    assert_eq!(rec.len(), 2);
    for (i, r) in rec.iter().enumerate() {
        assert_eq!(r.esi, i as u16);
        assert_eq!(r.bytes, block[i]);
    }
}

// ── final-block-padding-recovery: K_actual=5/8, 2 source düş, 2 repair ile payload+metadata kurtar ──
#[test]
fn final_block_padding_recovery() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap();
    let metas: Vec<SrcMeta> = (0..5)
        .map(|i| SrcMeta {
            message_id: 0x7777,
            chunk_offset: (i as u32) * 53,
            payload: (0..53).map(|j| (i as u8).wrapping_add(j)).collect(),
            final_flag: i == 4, // son real-source = mesaj-sonu
        })
        .collect();
    let block = build_block(8, 64, &metas); // known_zero=3
    let repairs = c.encode(&p, &src_syms(&block)).unwrap();

    // esi 1 ve 4 (final-flag'li) düş; kalan 3 real + 2 repair. 5 + 3 = 8 ≥ 8.
    let mut recv: Vec<ReceivedSymbol> = [0usize, 2, 3]
        .iter()
        .map(|&i| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: &block[i],
        })
        .collect();
    recv.push(ReceivedSymbol {
        esi: 0,
        is_repair: true,
        bytes: &repairs[0].bytes,
    });
    recv.push(ReceivedSymbol {
        esi: 1,
        is_repair: true,
        bytes: &repairs[1].bytes,
    });
    let rec = c.reconstruct(&p, &recv, 3).unwrap();
    assert_eq!(rec.len(), 2);
    // final-flag'li kayıp-source'un metadata'sı geri geldi (mesaj-sonu tek-pakete-bağlı-DEĞİL, §6.5).
    let final_rec = rec.iter().find(|r| r.esi == 4).unwrap();
    assert_eq!(final_rec.bytes, block[4]);
    let pfx = SourcePrefix::parse(&final_rec.bytes).unwrap();
    assert_eq!(pfx.source_flags, SOURCE_FLAG_FINAL);
    assert_eq!(pfx.chunk_offset, 4 * 53);
    assert_eq!(pfx.payload_slice(&final_rec.bytes).len(), 53);
}

// ── §6.5: kaybolan-source'un message_id/chunk_offset/len/flags TAM geri gelir ──
#[test]
fn metadata_recovered_on_source_loss() {
    let c = RsSimdCodec;
    let p = by_id(0x02).unwrap(); // K=4, R=4, E=64
    let metas = vec![
        SrcMeta {
            message_id: 0xDEAD_BEEF,
            chunk_offset: 100,
            payload: b"first-chunk".to_vec(),
            final_flag: false,
        },
        SrcMeta {
            message_id: 0xDEAD_BEEF,
            chunk_offset: 200,
            payload: b"second-distinct".to_vec(),
            final_flag: false,
        },
        SrcMeta {
            message_id: 0xDEAD_BEEF,
            chunk_offset: 300,
            payload: b"third".to_vec(),
            final_flag: false,
        },
        SrcMeta {
            message_id: 0xDEAD_BEEF,
            chunk_offset: 400,
            payload: b"fourth-final".to_vec(),
            final_flag: true,
        },
    ];
    let block = build_block(4, 64, &metas); // K_actual=4, known_zero=0
    let repairs = c.encode(&p, &src_syms(&block)).unwrap();

    // esi 1 (distinct metadata) düş; 3 real + 1 repair. known_zero=0. 4 ≥ 4.
    let mut recv: Vec<ReceivedSymbol> = [0usize, 2, 3]
        .iter()
        .map(|&i| ReceivedSymbol {
            esi: i as u16,
            is_repair: false,
            bytes: &block[i],
        })
        .collect();
    recv.push(ReceivedSymbol {
        esi: 0,
        is_repair: true,
        bytes: &repairs[0].bytes,
    });
    let rec = c.reconstruct(&p, &recv, 0).unwrap();
    assert_eq!(rec.len(), 1);
    let pfx = SourcePrefix::parse(&rec[0].bytes).unwrap();
    assert_eq!(
        pfx.message_id, 0xDEAD_BEEF,
        "kayıp-source message_id recover"
    );
    assert_eq!(pfx.chunk_offset, 200, "kayıp-source chunk_offset recover");
    assert_eq!(pfx.actual_payload_len as usize, "second-distinct".len());
    assert_eq!(pfx.payload_slice(&rec[0].bytes), b"second-distinct");
    let expected = payloads(&metas);
    assert_eq!(pfx.payload_slice(&rec[0].bytes), &expected[1].payload[..]);
}

// ── fail-closed: known_zero_count == K_shape → K_actual < 1 ──
#[test]
fn known_zero_too_large_fail_closed() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8
    let recv: Vec<ReceivedSymbol> = Vec::new();
    assert!(matches!(
        c.reconstruct(&p, &recv, 8),
        Err(FecError::KnownZeroTooLarge {
            known_zero: 8,
            k_shape: 8
        })
    ));
}

// ── fail-closed: admitted-source ESI zero-pad-bölgesinde (K_actual-mismatch) ──
#[test]
fn source_in_zero_region_fail_closed() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8
    let sym = vec![0u8; 64];
    // known_zero=5 → k_actual=3. esi=5 real-source geldi (sender GÖNDERMEZDİ) → reject.
    let recv = vec![ReceivedSymbol {
        esi: 5,
        is_repair: false,
        bytes: &sym,
    }];
    assert!(matches!(
        c.reconstruct(&p, &recv, 5),
        Err(FecError::SourceInZeroRegion {
            esi: 5,
            k_actual: 3
        })
    ));
}

// ── fail-closed: known_zero ile bile decode-koşulu sağlanmıyor ──
#[test]
fn insufficient_with_known_zero_fail_closed() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8
    let sym = vec![0u8; 64];
    // K_actual=2, known_zero=6. Yalnız 1 repair. admitted 1, +6 = 7 < 8.
    let recv = vec![ReceivedSymbol {
        esi: 0,
        is_repair: true,
        bytes: &sym,
    }];
    assert!(matches!(
        c.reconstruct(&p, &recv, 6),
        Err(FecError::Insufficient {
            admitted: 1,
            known_zero: 6,
            k_shape: 8
        })
    ));
}
