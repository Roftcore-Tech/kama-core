//! KATMAN-1 (K-WIRE-NONCE96-1): RFC-8439 resmî AEAD test-vektörü → ChaCha20-Poly1305 PRIMITIVE
//! doğruluğu (RustCrypto backend, DR-5). **HARİCİ-ORACLE:** RFC-8439 §2.8.2 standardın-kendisidir;
//! self-golden-DEĞİL. Bu katman full_pn→nonce96-MAPPING'ini test-ETMEZ (o KATMAN-2); yalnız primitifi.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// RFC 8439 §2.8.2 "Example and Test Vector for AEAD_CHACHA20_POLY1305".
#[test]
fn rfc8439_aead_test_vector() {
    let key = hex("80 81 82 83 84 85 86 87 88 89 8a 8b 8c 8d 8e 8f
         90 91 92 93 94 95 96 97 98 99 9a 9b 9c 9d 9e 9f");
    let nonce = hex("07 00 00 00 40 41 42 43 44 45 46 47");
    let aad = hex("50 51 52 53 c0 c1 c2 c3 c4 c5 c6 c7");
    let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    // RFC-8439 beklenen ciphertext (114B) + tag (16B).
    let expected_ct = hex("d3 1a 8d 34 64 8e 60 db 7b 86 af bc 53 ef 7e c2
         a4 ad ed 51 29 6e 08 fe a9 e2 b5 a7 36 ee 62 d6
         3d be a4 5e 8c a9 67 12 82 fa fb 69 da 92 72 8b
         1a 71 de 0a 9e 06 0b 29 05 d6 a5 b6 7e cd 3b 36
         92 dd bd 7f 2d 77 8b 8c 98 03 ae e3 28 09 1b 58
         fa b3 24 e4 fa d6 75 94 55 85 80 8b 48 31 d7 bc
         3f f4 de f0 8e 4b 7a 9d e5 76 d2 65 86 ce c6 4b
         61 16");
    let expected_tag = hex("1a e1 0b 59 4f 09 e2 6a 7e 90 2e cb d0 60 06 91");

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let out = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .expect("rfc8439 encrypt");
    // RustCrypto ct||tag birleşik döner.
    let (ct, tag) = out.split_at(out.len() - 16);
    assert_eq!(ct, &expected_ct[..], "RFC-8439 ciphertext eşleşmeli");
    assert_eq!(tag, &expected_tag[..], "RFC-8439 tag eşleşmeli");

    // Ters yön: decrypt geri-plaintext.
    let dec = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &out,
                aad: &aad,
            },
        )
        .expect("rfc8439 decrypt");
    assert_eq!(dec, plaintext);
}

// Tip-assert: production-type AÇIKÇA ChaCha20Poly1305 (XChaCha20Poly1305 DEĞİL — DR-5 kısıtı).
// XChaCha 24-byte nonce alır; ChaCha20Poly1305 12-byte → Nonce boyu 12 olduğunu derleme+runtime doğrular.
#[test]
fn nonce_is_96_bit_not_xchacha() {
    let key = [0u8; 32];
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let nonce12 = [0u8; 12];
    // 12B nonce ile çalışır (IETF ChaCha20-Poly1305). XChaCha olsaydı 24B isterdi.
    assert!(cipher
        .encrypt(
            Nonce::from_slice(&nonce12),
            Payload {
                msg: b"x",
                aad: b""
            }
        )
        .is_ok());
}
