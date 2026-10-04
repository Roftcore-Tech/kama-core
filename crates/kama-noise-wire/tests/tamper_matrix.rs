//! TC-15 — Tahrifat matrisi (SR-STR-CORE-001 §5.3, BUL-09).
//!
//! **Kapsanan bulgu:** BUL-09. [KOD] Bugün byte bozma TEK POZİSYONDA test edilmektedir
//! (`sprint1_core::aead_fail_does_not_advance_window`, son byte). AAD alanlarının wire üzerinde
//! mutasyonu ve gerçek bir sealed datagram'ın kırpılması test edilmemiştir.
//!
//! **Gerekçe — sessiz AAD kaybı:** `aad.rs` alan sırasının DEĞİŞMEZ olduğunu normatif olarak
//! belirtir. Ancak bir alan yanlışlıkla AAD'den düşerse, mevcut golden vektör DEĞİŞMEZ — çünkü
//! seal ve open AYNI hatalı AAD'yi üretir ve roundtrip çalışmaya devam eder. Hiçbir mevcut test
//! bunu yakalayamaz. Yakalayan tek şey, o alanı WIRE ÜZERİNDE değiştirip AEAD'in düşmesini
//! beklemektir: alan AAD'de değilse tag hâlâ doğrulanır ve datagram AÇILIR.
//!
//! **Gerekçe — ciphertext-length:** §2.8'e göre ct-length AAD'ye BİLİNÇLİ olarak dahil
//! edilmemiştir (UDP-length üzerinden implicit). Bu sadeleştirmenin güvenli olduğu kırpma
//! testiyle kanıtlanmalıdır.
//!
//! **Oracle:** Tüm mutasyonlar reddedilir; hiçbiri `Ok(_)` değildir. Ayrıca reddetme DOĞRU
//! KATMANDA olmalıdır — yanlış katmanda reddetmek gözlemciye bilgi sızdırır (BUL-10 hattı).

use kama_noise_wire::error::WireError;
use kama_noise_wire::outer::{OUTER_HDR_LEN, TAG_LEN};
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0102_0304;
const PN: u64 = 5;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn packet() -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: (0..64u8).collect(),
    }
}

/// `golden_wire.rs` ile AYNI parametreler → aynı PINNED datagram.
fn golden_datagram() -> Vec<u8> {
    let mut a = KamaNoiseWireProtector::new(H_EXT, &key(0x11), &key(0x22));
    a.seal(
        &packet(),
        SealArgs {
            wire_pn: PN,
            dh_epoch: 0,
            send_generation: 0,
        },
    )
    .expect("seal")
    .bytes
}

/// Golden'ı açabilen taze bir alıcı (her mutasyon için sıfır state).
fn fresh_receiver() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H_EXT, &key(0x22), &key(0x11))
}

#[test]
fn baseline_golden_opens() {
    // Matrisin anlamlı olması için taban vakanın AÇILDIĞINDAN emin ol.
    // Bu olmadan "her mutasyon reddedildi" iddiası vakumdur.
    let d = golden_datagram();
    assert_eq!(d.len(), 101, "sealed FEC @E=64 = 26+11+64");
    assert!(fresh_receiver().open(&d).is_ok(), "taban golden açılmıyor");
}

// ── (a) Ciphertext: her byte pozisyonunda tek-bit flip ──────────────────────

#[test]
fn single_bit_flip_anywhere_in_ciphertext_fails_aead() {
    // Ciphertext bölgesi = [OUTER_HDR_LEN, len-TAG_LEN). Her pozisyonda 8 bitin hepsi.
    // Poly1305 tag'in bit-düzeyinde bütünlük sağladığının tam kanıtı.
    let d = golden_datagram();
    let ct_range = OUTER_HDR_LEN..(d.len() - TAG_LEN);
    let mut checked = 0usize;

    for pos in ct_range {
        for bit in 0..8u8 {
            let mut bad = d.clone();
            bad[pos] ^= 1u8 << bit;
            match fresh_receiver().open(&bad) {
                Err(WireError::AeadFail) => checked += 1,
                other => panic!("ct pos={pos} bit={bit}: AeadFail beklendi, alınan {other:?}"),
            }
        }
    }
    assert_eq!(
        checked,
        (101 - OUTER_HDR_LEN - TAG_LEN) * 8,
        "75 byte × 8 bit"
    );
}

// ── (b) Tag: her byte pozisyonunda flip ─────────────────────────────────────

#[test]
fn single_bit_flip_anywhere_in_tag_fails_aead() {
    let d = golden_datagram();
    let tag_start = d.len() - TAG_LEN;
    for pos in tag_start..d.len() {
        for bit in 0..8u8 {
            let mut bad = d.clone();
            bad[pos] ^= 1u8 << bit;
            match fresh_receiver().open(&bad) {
                Err(WireError::AeadFail) => {}
                other => panic!("tag pos={pos} bit={bit}: AeadFail beklendi, alınan {other:?}"),
            }
        }
    }
}

// ── (c) AAD alanları: wire üzerinde mutasyon ────────────────────────────────
//
// **Bu grup dosyanın çekirdeğidir.** Her AAD alanı wire üzerinde değiştirildiğinde AEAD
// DÜŞMELİDİR. Düşmezse o alan AAD'den çıkmış demektir — ve bu sessiz bir regresyondur
// (golden vektör değişmez, roundtrip çalışır).

#[test]
fn mutating_pn_trunc_on_the_wire_is_rejected() {
    // pn_trunc AAD'nin parçasıdır (§2.8, offset 22..26). Wire'da değiştirilirse:
    //   - Ya PN-precheck düşer (candidate pencere dışına çıkar)
    //   - Ya da AEAD düşer (nonce + AAD ikisi de değişti)
    // Her iki durumda da reddedilir; ASLA açılmaz.
    let d = golden_datagram();
    for pos in 6..10usize {
        for bit in 0..8u8 {
            let mut bad = d.clone();
            bad[pos] ^= 1u8 << bit;
            match fresh_receiver().open(&bad) {
                Err(WireError::AeadFail) | Err(WireError::PnReject(_)) => {}
                other => panic!("pn_trunc pos={pos} bit={bit}: {other:?}"),
            }
        }
    }
}

#[test]
fn mutating_dh_epoch_flag_on_the_wire_is_rejected() {
    // wire_flags.bit0 = dh_epoch, AAD'nin parçası. Flip edilince datagram epoch-1'e işaret eder.
    // Epoch-1 anahtarı kurulu DEĞİLSE EpochNotAccepted; kuruluysa AEAD düşer (farklı key + AAD).
    let d = golden_datagram();
    let mut bad = d.clone();
    bad[5] ^= 0b0000_0001;

    // (1) epoch-1 kurulmamış alıcı
    match fresh_receiver().open(&bad) {
        Err(WireError::EpochNotAccepted(1)) => {}
        other => panic!("epoch-1 kurulmamışken: {other:?}"),
    }
    // (2) epoch-1 kurulu alıcı → AEAD düşmeli (epoch-1 cipher-state farklı)
    let mut b = fresh_receiver();
    b.install_epoch1_keys(&key(0x33), &key(0x44));
    match b.open(&bad) {
        Err(WireError::AeadFail) => {}
        other => panic!("epoch-1 kuruluyken AeadFail beklendi: {other:?}"),
    }
}

#[test]
fn mutating_wire_format_is_rejected_at_the_format_gate() {
    // wire_format AAD'nin parçasıdır AMA outer-parse onu AEAD'den ÖNCE reddeder.
    // Bu sıra kasıtlıdır (fixed-cost parse). Reddetmenin format kapısında olduğunu pinliyoruz.
    let d = golden_datagram();
    for v in [0x00u8, 0x02, 0x7F, 0xFF] {
        let mut bad = d.clone();
        bad[4] = v;
        assert_eq!(
            fresh_receiver().open(&bad).unwrap_err(),
            WireError::UnknownWireFormat(v)
        );
    }
}

#[test]
fn mutating_reserved_wire_flag_bits_is_rejected_at_the_flag_gate() {
    // bits1..7 reserved → REJECT (§2.5). Forward-compat DEĞİL: bilinmeyen flag sessizce
    // yok sayılırsa, gelecekte flag ekleyen bir gönderen eski alıcıda yanlış yorumlanır.
    let d = golden_datagram();
    for bit in 1..8u8 {
        let mut bad = d.clone();
        bad[5] |= 1u8 << bit;
        assert_eq!(
            fresh_receiver().open(&bad).unwrap_err(),
            WireError::ReservedWireFlags(bad[5]),
            "reserved bit {bit}"
        );
    }
}

#[test]
fn mutating_h_ext_is_rejected_before_aead_not_as_unknown_handle() {
    // **Katman doğruluğu (BUL-10 hattı).** h_ext mutasyonu bu katmanda `h_ext mismatch`
    // (InnerParse) vermeli — `UnknownHandle` DEĞİL. Ayrım önemlidir: `UnknownHandle` demux
    // katmanının (§2.55 aktif-session tablosu) kelimesidir; protector seviyesinde onu
    // döndürmek, gözlemciye "bu handle hiçbir oturumda yok" ile "bu handle başka bir
    // protector'a ait" arasındaki farkı verir.
    let d = golden_datagram();
    for pos in 0..4usize {
        let mut bad = d.clone();
        bad[pos] ^= 0xFF;
        assert_eq!(
            fresh_receiver().open(&bad).unwrap_err(),
            WireError::InnerParse("h_ext mismatch"),
            "h_ext pos={pos}"
        );
    }
}

// ── (d) Kırpma: ciphertext-length AAD'de yok — güvenli mi? ──────────────────

#[test]
fn truncating_from_the_end_is_always_rejected() {
    // §2.8: ct-length AAD'ye BİLİNÇLİ olarak dahil edilmedi (UDP-length üzerinden implicit).
    // Bu sadeleştirmenin güvenli olduğunun kanıtı: sondan 1..N byte kırpmanın HİÇBİRİ açılmamalı.
    // Kırpma tag'i bozar (son 16 byte) veya ct'yi kısaltır → Poly1305 uyuşmaz.
    let d = golden_datagram();
    for cut in 1..d.len() {
        let short = &d[..d.len() - cut];
        match fresh_receiver().open(short) {
            Err(WireError::AeadFail) | Err(WireError::DatagramTooShort { .. }) => {}
            other => panic!("cut={cut} (len={}): {other:?}", short.len()),
        }
    }
}

#[test]
fn appending_bytes_at_the_end_is_always_rejected() {
    // Uzatma: ct-length implicit olduğu için eklenen byte'lar ct'nin parçası sayılır → AEAD düşer.
    // Bu, ct-length'in AAD'de olmamasının diğer yarısıdır.
    let d = golden_datagram();
    for extra in 1..=16usize {
        let mut long = d.clone();
        long.extend(std::iter::repeat_n(0xFFu8, extra));
        match fresh_receiver().open(&long) {
            Err(WireError::AeadFail) => {}
            other => panic!("extra={extra}: AeadFail beklendi, alınan {other:?}"),
        }
    }
}

#[test]
fn truncating_below_the_outer_overhead_is_a_length_error_not_a_panic() {
    // 0..25 byte: outer parse minimumunun altı. Tipli hata, panik değil, ve tag dilimi
    // `try_into()` ile taşmamalı.
    let d = golden_datagram();
    for len in 0..26usize {
        assert_eq!(
            fresh_receiver().open(&d[..len]).unwrap_err(),
            WireError::DatagramTooShort { got: len, min: 26 },
            "len={len}"
        );
    }
    // Sınır: 26B uzunluk kapısını geçer (boş ct + tag) ve AEAD'e düşer.
    match fresh_receiver().open(&d[..26]) {
        Err(WireError::AeadFail) => {}
        other => panic!("26B uzunluk kapısını geçmeli: {other:?}"),
    }
}

// ── (e) Bütünsel hüküm ──────────────────────────────────────────────────────

#[test]
fn no_mutation_of_the_golden_datagram_ever_opens() {
    // Matrisin tek cümlelik hükmü: PINNED datagram üzerindeki HİÇBİR tek-byte mutasyonu
    // `Ok(_)` üretmez. Bu, yukarıdaki alan-bazlı testlerin üstünde bir bütünlük ağıdır —
    // yeni bir alan eklenirse ve ona özel test yazılmazsa burada yakalanır.
    let d = golden_datagram();
    let mut mutations = 0usize;
    for pos in 0..d.len() {
        for delta in [0x01u8, 0x02, 0x40, 0x80, 0xFF] {
            let mut bad = d.clone();
            bad[pos] ^= delta;
            if bad == d {
                continue;
            }
            assert!(
                fresh_receiver().open(&bad).is_err(),
                "pos={pos} delta={delta:#04x}: MUTASYON AÇILDI"
            );
            mutations += 1;
        }
    }
    assert_eq!(mutations, d.len() * 5, "101 pozisyon × 5 delta");
}

#[test]
fn rejected_mutations_leave_no_receiver_state() {
    // Paylaşılan bir alıcıya tüm mutasyon selini gönder; pencere ilerlememeli ve meşru
    // datagram sonrasında hâlâ açılabilmeli. (Yukarıdaki testler taze alıcı kullanıyor;
    // bu, state birikimini ölçer.)
    let d = golden_datagram();
    let mut b = fresh_receiver();
    let before = b.recv_expected_pn();

    for pos in 0..d.len() {
        let mut bad = d.clone();
        bad[pos] ^= 0xFF;
        let _ = b.open(&bad);
    }
    assert_eq!(
        b.recv_expected_pn(),
        before,
        "mutasyon seli recv-penceresini ilerletti"
    );
    assert!(b.open(&d).is_ok(), "selden sonra meşru datagram açılamadı");
}
