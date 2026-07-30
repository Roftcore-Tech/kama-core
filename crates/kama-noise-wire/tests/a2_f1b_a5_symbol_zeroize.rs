//! A2-F1-B / A-5 — SENSITIVE-SYMBOL sözleşme + zeroize KABUL-PİNLERİ (Abkarada A-5 + A-5.1).
//!
//! Yöntem: A-5 SÖZLEŞMESİ (parse→SensitiveSymbolBytes; borrow-only-&[u8]; Clone-yok; Debug-no-leak;
//! MOVE-tek-sahip) davranış-pinleriyle; zeroize'ın gerçekleştiği `zeroize::Zeroize` crate-sözleşmesi +
//! SensitiveSymbolBytes::Drop'un onu çağırması (kod-görünür) ile pinlenir. Serbest-bırakılmış allocator
//! belleğini SONRADAN okuyan UB-testi YAZILMAZ (Abkarada aynen).

#![cfg(feature = "experimental-cold-start")]

use kama_noise_wire::inner::{encode_fec_inner, parse_inner, InnerPlain};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::LogicalShardHeader;

fn hdr(kind: ShardKind) -> LogicalShardHeader {
    LogicalShardHeader {
        kind,
        profile_id: 0x03,
        block_id: 1,
        esi: 0,
        k_actual: 8,
    }
}

/// A-5: source shard parse → SensitiveSymbolBytes; borrow-yolu (&[u8]) payload'ı korur.
#[test]
fn a5_source_symbol_is_sensitive_and_borrow_preserves_payload() {
    let plain = encode_fec_inner(&hdr(ShardKind::Source), &[0xABu8; 64]);
    match parse_inner(&plain).unwrap() {
        InnerPlain::FecShard { symbol, .. } => {
            assert_eq!(
                symbol.as_ref(),
                &[0xABu8; 64][..],
                "AsRef payload'ı korumalı"
            );
            assert_eq!(&symbol[..], &[0xABu8; 64][..], "Deref payload'ı korumalı");
            assert_eq!(symbol.len(), 64);
        }
        InnerPlain::Control { .. } => panic!("FEC beklendi"),
    }
}

/// A-5.1: repair shard da SensitiveSymbolBytes (aynı sözleşme; source≠repair kimlik-yolu).
#[test]
fn a5_repair_symbol_also_sensitive() {
    let plain = encode_fec_inner(&hdr(ShardKind::Repair), &[0xCDu8; 64]);
    match parse_inner(&plain).unwrap() {
        InnerPlain::FecShard { header, symbol } => {
            assert_eq!(header.kind, ShardKind::Repair);
            assert_eq!(symbol.as_ref(), &[0xCDu8; 64][..]);
        }
        InnerPlain::Control { .. } => panic!("FEC beklendi"),
    }
}

/// A-5: ownership MOVE (Clone-yok → çift-sahip derleme-düzeyinde imkansız; tek-sahip → tek-zeroize).
#[test]
fn a5_ownership_move_single_owner() {
    let plain = encode_fec_inner(&hdr(ShardKind::Source), &[0x11u8; 64]);
    let sym = match parse_inner(&plain).unwrap() {
        InnerPlain::FecShard { symbol, .. } => symbol,
        InnerPlain::Control { .. } => panic!("FEC"),
    };
    let moved = sym; // MOVE (sym artık kullanılamaz — Clone yok)
    assert_eq!(moved.len(), 64);
    drop(moved); // tek-sahip drop → tek-zeroize (double-free yok)
}

/// A-5: Debug payload SIZDIRMAZ (yalnız len metadata).
#[test]
fn a5_debug_does_not_leak_payload() {
    let plain = encode_fec_inner(&hdr(ShardKind::Source), &[0xFFu8; 64]);
    let dbg = format!("{:?}", parse_inner(&plain).unwrap());
    assert!(dbg.contains("len"), "Debug len göstermeli");
    assert!(
        !dbg.contains("255, 255, 255"),
        "Debug payload sızdırdı: {dbg}"
    );
}

/// A-5: zeroize-crate SÖZLEŞMESİ (SensitiveSymbolBytes::Drop'un çağırdığı `Vec::zeroize` payload'ı
/// sıfırlar) — Drop-yolunun etkisi bu birim üzerinden pinlenir (UB-siz; aynı kod-yolu).
#[test]
fn a5_zeroize_contract_clears_payload() {
    use zeroize::Zeroize;
    let mut v = vec![0x5Au8; 64];
    v.zeroize();
    assert!(v.iter().all(|&b| b == 0), "zeroize payload'ı sıfırlamadı");
}
