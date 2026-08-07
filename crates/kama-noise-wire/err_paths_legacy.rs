//! TC-02 — `legacy.rs` hata yolları (SR-STR-CORE-001 §5.1, BUL-05).
//!
//! **Kapsanan bulgu:** BUL-05. STR §3.4 `legacy.rs`'te üç sıfır-sayımlı construct-site raporlar:
//! `SealInput`, `DatagramTooShort`, `UnknownHandle`.
//!
//! **Gerekçe:** Legacy yol ccr §35.6/§35.9 uyumluluğu için canlı tutulacaksa aynı fail-closed
//! disiplinine tabidir. **Interop yolları tarihsel olarak en az test edilen ve en çok istismar
//! edilen yüzeydir**: ana yol sıkılaştırılırken uyumluluk yolu gevşek bırakılır ve saldırgan
//! oraya yönelir. Bu envelope MÜHÜRLÜ (§0.9) — yeniden tanımlanamıyor, o hâlde en azından
//! sınırlarının kanıtlanması gerekir.
//!
//! **Oracle:** Kısa datagram, bilinmeyen handle ve geçersiz seal girdisi tipli hatayla reddedilir;
//! reddedilen hiçbir çağrı state ilerletmez.

use kama_noise_wire::error::WireError;
use kama_noise_wire::{LegacyDirection, LegacySessionProtector, LEGACY_OVERHEAD};
use zeroize::Zeroizing;

const HUB_HANDLE: u32 = 0x0A0B_0C0D;
const SESSION_ID: [u8; 16] = [0x5A; 16];

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn legacy_pair() -> (LegacySessionProtector, LegacySessionProtector) {
    (
        LegacySessionProtector::new(
            HUB_HANDLE,
            &key(0xA1),
            &key(0xB2),
            1,
            0x0001,
            7,
            SESSION_ID,
            LegacyDirection::AToB,
        ),
        LegacySessionProtector::new(
            HUB_HANDLE,
            &key(0xB2),
            &key(0xA1),
            1,
            0x0001,
            7,
            SESSION_ID,
            LegacyDirection::BToA,
        ),
    )
}

// ── SITE 1: DatagramTooShort ────────────────────────────────────────────────

#[test]
fn open_below_legacy_overhead_is_datagram_too_short() {
    // §35.6: 22B = hub_handle 4 + seq16 2 + tag 16. Altındaki her uzunluk reddedilmeli.
    // 0..21 tam taraması — off-by-one bir gün 21B'yi geçirirse tag okuması taşar.
    let (_, mut b) = legacy_pair();
    for len in 0..LEGACY_OVERHEAD {
        let datagram = vec![0u8; len];
        assert_eq!(
            b.open(&datagram),
            Err(WireError::DatagramTooShort {
                got: len,
                min: LEGACY_OVERHEAD
            }),
            "len={len} yanlış sınıfta reddedildi"
        );
    }
}

#[test]
fn open_at_exactly_overhead_passes_length_gate() {
    // Sınır: tam 22B (boş plaintext + tag) uzunluk kapısını GEÇER ve bir sonraki kapıya düşer.
    // Sıfır-datagram'ın hub_handle'ı 0 olduğu için handle guard'ı yakalar — bu, uzunluk
    // kapısının 22'de KAPANMADIĞININ kanıtıdır.
    let (_, mut b) = legacy_pair();
    assert_eq!(
        b.open(&[0u8; LEGACY_OVERHEAD]),
        Err(WireError::UnknownHandle(0)),
        "22B sıfır-datagram handle guard'ına ulaşmalı"
    );
}

// ── SITE 2: UnknownHandle ───────────────────────────────────────────────────

#[test]
fn open_with_foreign_hub_handle_is_unknown_handle() {
    // Cross-session reddi: başka bir oturumun handle'ını taşıyan datagram AEAD'e HİÇ ULAŞMAMALI.
    // Ulaşırsa saldırgan, hedef oturumun cipher-state'ini ucuz bir handle taramasıyla yoklayabilir.
    let (mut a, mut b) = legacy_pair();
    let mut datagram = a.seal(b"merhaba", 1).expect("seal");

    for foreign in [0u32, 1, HUB_HANDLE ^ 0x0000_0001, 0xFFFF_FFFF] {
        datagram[0..4].copy_from_slice(&foreign.to_be_bytes());
        assert_eq!(
            b.open(&datagram),
            Err(WireError::UnknownHandle(foreign)),
            "handle={foreign:#010x}"
        );
    }
    // Doğru handle geri konunca açılır — guard'ın yalnız handle'a baktığının kanıtı.
    datagram[0..4].copy_from_slice(&HUB_HANDLE.to_be_bytes());
    assert_eq!(b.open(&datagram).expect("open"), b"merhaba");
}

#[test]
fn unknown_handle_does_not_advance_recv_window() {
    // Reddedilen datagram state İZ BIRAKMAMALI: yabancı handle'lı bir sel, alıcının PN
    // penceresini ilerletip meşru paketleri too-old'a düşürebilmemeli (DoS yüzeyi).
    let (mut a, mut b) = legacy_pair();
    let before = b.recv_expected_pn();

    let mut bad = a.seal(b"x", 1).expect("seal");
    bad[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
    for _ in 0..64 {
        assert!(matches!(b.open(&bad), Err(WireError::UnknownHandle(_))));
    }
    assert_eq!(
        b.recv_expected_pn(),
        before,
        "unknown-handle reddi pencereyi ilerletti"
    );

    // Meşru paket hâlâ açılabilmeli.
    let good = a.seal(b"y", 2).expect("seal");
    assert_eq!(b.open(&good).expect("open"), b"y");
}

// ── SITE 3: SealInput (non-monotonic full_pn) ───────────────────────────────

#[test]
fn seal_with_non_monotonic_pn_is_seal_input() {
    // Nonce = LE64(full_pn) (§4.5). Aynı anahtarla nonce tekrarı ChaCha20-Poly1305'te
    // KATASTROFİKTİR: keystream tekrarı + Poly1305 anahtar sızıntısı. Bu guard son yapısal
    // bariyerdir ve `<=` olmalıdır — `<` olsaydı eşit PN geçerdi.
    let (mut a, _) = legacy_pair();
    a.seal(b"first", 10).expect("ilk seal");

    for pn in [0u64, 1, 9, 10] {
        assert_eq!(
            a.seal(b"again", pn),
            Err(WireError::SealInput("legacy non-monotonic full_pn")),
            "pn={pn} kabul edildi (nonce-reuse)"
        );
    }
    // Sınır: 11 geçer (kesin-büyük).
    assert!(a.seal(b"next", 11).is_ok());
}

#[test]
fn rejected_seal_produces_no_bytes_and_does_not_move_the_pn_floor() {
    // Fail-closed: reddedilen seal ne byte üretir ne de last_sealed_pn'i değiştirir.
    // İkincisi önemli: reddedilen çağrı tabanı yükseltseydi, sonraki meşru PN de reddedilirdi.
    let (mut a, _) = legacy_pair();
    a.seal(b"first", 100).expect("seal");
    assert!(a.seal(b"dup", 100).is_err());
    assert!(a.seal(b"dup", 50).is_err());
    // Taban hâlâ 100 → 101 geçmeli.
    assert!(
        a.seal(b"ok", 101).is_ok(),
        "taban reddedilen çağrıyla kaydı"
    );
}

// ── AAD alan disiplini (§35.9) ──────────────────────────────────────────────

#[test]
fn tampering_seq16_on_the_wire_fails_aead_not_a_lower_gate() {
    // seq16 AAD'nin parçasıdır. Wire üzerinde değiştirilirse AEAD düşmelidir — daha ALT bir
    // katmanda (handle/uzunluk) reddedilmemeli. Yanlış katmanda reddetmek gözlemciye
    // "hangi alan bozuldu" bilgisini verir (BUL-10 hattı).
    let (mut a, mut b) = legacy_pair();
    let mut datagram = a.seal(b"payload", 5).expect("seal");
    datagram[4] ^= 0x01; // seq16 üst byte

    match b.open(&datagram) {
        Err(WireError::AeadFail) | Err(WireError::PnReject(_)) => {}
        other => panic!("seq16 tahrifi AEAD/PN sınıfında reddedilmeli, alınan: {other:?}"),
    }
}

#[test]
fn truncated_ciphertext_fails_aead_not_a_panic() {
    // Sondan kırpma: ct/tag sınırı bozulur. AAD'de ciphertext-length YOK, dolayısıyla
    // kırpmanın güvenli olduğu AEAD tarafından kanıtlanmalıdır (panik değil, tipli hata).
    let (mut a, mut b) = legacy_pair();
    let datagram = a.seal(b"uzunca bir payload", 5).expect("seal");
    for cut in 1..=(datagram.len() - LEGACY_OVERHEAD) {
        let short = &datagram[..datagram.len() - cut];
        match b.open(short) {
            Err(WireError::AeadFail) | Err(WireError::DatagramTooShort { .. }) => {}
            other => panic!("cut={cut}: beklenmeyen sonuç {other:?}"),
        }
    }
}