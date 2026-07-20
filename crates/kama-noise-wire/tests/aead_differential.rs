//! KATMAN-3 (K-WIRE-NONCE96-1): RustCrypto == libsodium AEAD implementation-independence.
//!
//! **TEST-ONLY, feature-gate `libsodium-oracle`** (DEFAULT-KAPALI; production-graph-dışı; ayrı-CI-job:
//! `cargo test -p kama-noise-wire --features libsodium-oracle`). Sistem-libsodium'a linklenir.
//! Aynı {key, nonce96(LE64), AAD, plaintext} → RustCrypto-ct+tag == libsodium-ct+tag (byte-eş).
//! **Amaç:** self-golden'ı tek-oracle-yapmamak — iki-BAĞIMSIZ-implementation aynı-wire-byte'ı üretmeli.
//! Differential-failure = HARD-FAIL (assert). full_pn={0,1,2^32,2^63} nonce96-mapping'i de kapsanır.

#![cfg(feature = "libsodium-oracle")]

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use kama_noise_wire::nonce96;
use sodiumoxide::crypto::aead::chacha20poly1305_ietf as sodium;

fn rustcrypto_seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], pt: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .encrypt(Nonce::from_slice(nonce), Payload { msg: pt, aad })
        .expect("rustcrypto encrypt")
}

fn libsodium_seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], pt: &[u8]) -> Vec<u8> {
    let k = sodium::Key::from_slice(key).unwrap();
    let n = sodium::Nonce::from_slice(nonce).unwrap();
    sodium::seal(pt, Some(aad), &n, &k)
}

#[test]
fn rustcrypto_equals_libsodium_across_nonce_mapping() {
    sodiumoxide::init().expect("libsodium init");
    let key = [0x24u8; 32];
    let aad = b"KAMA-WIRE-EXT-V1\x01\x02\x03\x04\x01\x00\x00\x00\x00\x05";
    let plaintext = b"kama noise-wire differential oracle payload -- RustCrypto vs libsodium IETF ChaCha20-Poly1305";

    // nonce96-mapping'in kritik full_pn değerleri + rastgele-benzeri sabitler.
    for &full_pn in &[
        0u64,
        1,
        2,
        255,
        1 << 16,
        1u64 << 32,
        1u64 << 63,
        0x0102_0304_0506_0708,
    ] {
        let nonce = nonce96(full_pn);
        let a = rustcrypto_seal(&key, &nonce, aad, plaintext);
        let b = libsodium_seal(&key, &nonce, aad, plaintext);
        assert_eq!(
            a, b,
            "RustCrypto != libsodium @ full_pn={full_pn} (ct+tag byte-eş olmalı)"
        );
    }
}

// KATMAN-3 legacy (Sprint-5b): legacy §35.9 AAD (28B) + LE64-nonce'un da impl-bağımsız-eşitliği.
#[test]
fn rustcrypto_equals_libsodium_legacy_aad() {
    sodiumoxide::init().expect("libsodium init");
    let key = [0x77u8; 32];
    let pt = b"legacy session-data hot-path payload";
    for &full_pn in &[0u64, 1, 5, 1 << 16, 1u64 << 40] {
        let seq16 = (full_pn & 0xFFFF) as u16;
        let aad = kama_noise_wire::build_legacy_aad(0x01, 0x0001, 0x01, 1, seq16, &[0x33u8; 16]);
        let nonce = nonce96(full_pn);
        let a = rustcrypto_seal(&key, &nonce, &aad, pt);
        let b = libsodium_seal(&key, &nonce, &aad, pt);
        assert_eq!(a, b, "legacy differential mismatch @ full_pn={full_pn}");
    }
}

#[test]
fn rustcrypto_equals_libsodium_varied_inputs() {
    sodiumoxide::init().expect("libsodium init");
    for seed in 0u8..24 {
        let key = [seed.wrapping_mul(7); 32];
        let nonce = nonce96(u64::from(seed) * 0x1_0000_0001);
        let aad: Vec<u8> = (0..seed).map(|i| i.wrapping_mul(3)).collect();
        let pt: Vec<u8> = (0..(seed as usize * 5 + 1))
            .map(|i| (i as u8).wrapping_add(seed))
            .collect();
        let a = rustcrypto_seal(&key, &nonce, &aad, &pt);
        let b = libsodium_seal(&key, &nonce, &aad, &pt);
        assert_eq!(a, b, "differential mismatch @ seed={seed}");
    }
}
