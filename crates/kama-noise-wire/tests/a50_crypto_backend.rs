//! A5.0 — kripto bağımlılıklarının DERLEME ve DOĞRULUK teyidi (çekirdek A5.1+ bunların üstüne yazılır).
//!
//! Buradaki beklenen değerler Rust'tan ÜRETİLMEDİ: bağımsız Python referansından (`ref/crypto/kdf.py`,
//! A2.1) alındı. Kalıcı yeri `vectors/` (A2.2 — V-KDF-1..7); bu dosya yalnız "seçilen backend doğru
//! fonksiyonu mu hesaplıyor" sorusunu A5.0 anında kapatır.
//!
//! İKİ TUZAK (ccr §35.2.1, INV-KDF-3):
//!  1. `hkdf::Hkdf<Blake2s256>` DERLENMEZ — BLAKE2'nin blok API'si lazy'dir ve `Hkdf`'in varsayılan
//!     HMAC sarmalayıcısının istediği eager-buffer bound'unu sağlamaz. Doğrusu `SimpleHkdf<Blake2s256>`.
//!  2. `blake2::Blake2sMac` BLAKE2s'in KEYED modudur, HMAC DEĞİLDİR. Derlenir ve "çalışır" ama farklı
//!     çıktı üretir → sessiz interop kırılması.

use blake2::digest::{KeyInit, Mac};
use blake2::{Blake2s256, Blake2sMac256};
use hkdf::SimpleHkdf;
use hmac::SimpleHmac;
use kama_vectors::hex as unhex;

/// ccr §35.2.1 kanonik sarmalayıcı: `Extract(salt = ASCII(label), IKM = secret)` → `Expand(info = context)`.
fn kdf(label: &[u8], secret: &[u8], context: &[u8], out: &mut [u8]) {
    SimpleHkdf::<Blake2s256>::new(Some(label), secret)
        .expand(context, out)
        .expect("L <= 255*HashLen");
}

#[test]
fn simple_hkdf_blake2s_matches_python_reference() {
    let ctx: Vec<u8> = (0u8..100).collect();
    let secret64: Vec<u8> = (0u8..64).collect();
    let secret32: Vec<u8> = (0u8..32).collect();

    let mut d1 = [0u8; 32];
    kdf(b"SR-SESSION-INIT-V3", &secret64, &ctx, &mut d1);
    assert_eq!(
        &d1[..],
        &h("2db81a3cee736cacb126a1183712c4bcec59eca9bfefeca712fe9867b49e8788")[..]
    );

    // L = 16 (D3 session_id_internal biçimi).
    let mut d3 = [0u8; 16];
    kdf(b"SR-SESSION-ID-V3", &secret32, &ctx, &mut d3);
    assert_eq!(&d3[..], &h("b779a18f9a343e81cbd0c181b95463dd")[..]);

    // 1 baytlık context (D2 yön kodu).
    let mut d2 = [0u8; 32];
    kdf(b"SR-SESSION-TRAFFIC-V3", &secret32, &[0x01], &mut d2);
    assert_eq!(
        &d2[..],
        &h("3ecefbd2ca69013df065f5722f4ae5ddfd38992320217843276e2fa1a1f54802")[..]
    );
}

/// V-KDF-6 negatifi (INV-KDF-3): HMAC-BLAKE2s-256 ≠ BLAKE2s-keyed. İkisi de derlenir; yalnız biri doğrudur.
#[test]
fn hmac_blake2s_is_not_blake2s_keyed_mode() {
    let key: Vec<u8> = (0u8..32).collect();
    let msg = b"kama-inv-kdf-3";

    let mut mac = <SimpleHmac<Blake2s256> as KeyInit>::new_from_slice(&key).expect("hmac key");
    mac.update(msg);
    let hmac_out = mac.finalize().into_bytes();

    let mut k = <Blake2sMac256 as KeyInit>::new_from_slice(&key).expect("blake2s key");
    k.update(msg);
    let keyed_out = k.finalize().into_bytes();

    assert_eq!(
        &hmac_out[..],
        &h("ea4a0276e3fc70883f818acb7d5881e0f917e5a4dc304beb71aff977a7c2dca8")[..],
        "HMAC-BLAKE2s Python referansıyla aynı olmalı"
    );
    assert_eq!(
        &keyed_out[..],
        &h("4ebb87ff2a036a59be4d4fdcc508f5154980f84b7f92b018d9592034131f6445")[..],
        "BLAKE2s-keyed de referansla aynı — ve HMAC'ten FARKLI"
    );
    assert_ne!(
        hmac_out, keyed_out,
        "INV-KDF-3: iki fonksiyon aynı çıktıyı veremez"
    );
}

/// X25519 (RFC 7748 §6.1) — `reusable_secrets` ile aynı ephemeral iki DH'de kullanılabilir (ccr D4 `ss_e2`).
#[test]
fn x25519_rfc7748_and_reusable_secret() {
    use x25519_dalek::{PublicKey, ReusableSecret, StaticSecret};

    let a = StaticSecret::from(unhex32(
        "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
    ));
    let b = StaticSecret::from(unhex32(
        "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb",
    ));
    let (pa, pb) = (PublicKey::from(&a), PublicKey::from(&b));
    assert_eq!(
        &pa.as_bytes()[..],
        &h("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")[..]
    );
    let ss = a.diffie_hellman(&pb);
    assert_eq!(
        &ss.as_bytes()[..],
        &h("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742")[..]
    );
    assert!(
        ss.was_contributory(),
        "geçerli DH katkılı olmalı (ccr §5.1)"
    );

    // Düşük-mertebeli nokta → tüm-sıfır paylaşılan sır → was_contributory() == false (REDDEDİLMELİ).
    let low = PublicKey::from([0u8; 32]);
    assert!(!a.diffie_hellman(&low).was_contributory());

    // Aynı ephemeral iki ayrı DH'de (move olmadan).
    let eph = ReusableSecret::random_from_rng(rand_core::OsRng);
    let (s1, s2) = (eph.diffie_hellman(&pa), eph.diffie_hellman(&pb));
    assert_ne!(s1.as_bytes(), s2.as_bytes());
}

/// Ed25519 (RFC 8032 §7.1 TEST 2) + katı doğrulama (ccr §5.2: S >= L reddi).
#[test]
fn ed25519_rfc8032_strict_verify() {
    use ed25519_dalek::{Signature, VerifyingKey};

    let pk = VerifyingKey::from_bytes(&unhex32(
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
    ))
    .expect("pk");
    let sig_bytes: [u8; 64] = h(
        "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da\
         085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    )
    .try_into()
    .expect("64 B");
    let sig = Signature::from_bytes(&sig_bytes);
    assert!(pk.verify_strict(&[0x72], &sig).is_ok());
    assert!(
        pk.verify_strict(&[0x73], &sig).is_err(),
        "mesaj değişti → RED"
    );

    // S + L: aynı denklemi sağlar ama kanonik değildir → katı kipte RED (malleability).
    let l: [u8; 32] = unhex32("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
    let mut mal = sig_bytes;
    let mut carry = 0u16;
    for i in 0..32 {
        let v = u16::from(mal[32 + i]) + u16::from(l[i]) + carry;
        mal[32 + i] = (v & 0xff) as u8;
        carry = v >> 8;
    }
    assert!(pk
        .verify_strict(&[0x72], &Signature::from_bytes(&mal))
        .is_err());
}

/// Beklenen hex → byte (boşluk/satır devamı temizlenir). Tek hex çözücü `kama_vectors::hex`'tir.
fn h(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    unhex(&s).expect("geçerli hex")
}

fn unhex32(s: &str) -> [u8; 32] {
    h(s).try_into().expect("32 B")
}
