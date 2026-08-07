//! TC-07 — `control.rs` kalan hata yolları (SR-STR-CORE-001 §5.1, BUL-05).
//!
//! **Kapsanan bulgu:** BUL-05. STR §3.4 `control.rs`'te iki sıfır-sayımlı `ControlDecode` site'ı
//! raporlar. Control decode yüzeyinin geri kalanı `sprint2_control.rs`'te iyi test edilmiştir
//! (cap-aşımı, invalid-range, trailing, kısa-body); bu dosya BÜTÜNLÜK içindir ve mevcut
//! testlerle çakışmaz.
//!
//! Kapanan iki site:
//!   1. `"non-control codepoint"` — `decode_control_body`'nin `_` kolu. FEC codepoint'leri veya
//!      registry dışı bir byte control-decoder'a verilirse. Bu, katman karışması guard'ıdır:
//!      `decode_control` normalde yalnız `InnerPlain::Control` için çağrılır, ama fonksiyon
//!      public'tir ve doğrudan çağrılabilir.
//!   2. `"recovered esi start>end"` — RECOVERED'ın range-doğrulaması. `sprint2_control.rs`
//!      `invalid_range_rejected` yalnız ACK kolunu sürer; RECOVERED ayrı bir site'tır.
//!
//! **Oracle:** Tipli `ControlDecode`; panik yok; kısmi `Control` üretilmez.

use kama_noise_wire::error::WireError;
use kama_noise_wire::inner::InnerPlain;
use kama_noise_wire::{decode_control_body, encode_control_body};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind;

// ── SITE 1: non-control codepoint ───────────────────────────────────────────

#[test]
fn fec_codepoints_are_rejected_by_the_control_decoder() {
    // Katman karışması guard'ı: FEC bir control DEĞİLDİR. Bu kol düşmezse, bir FEC gövdesi
    // control gibi parse edilmeye çalışılır ve `Reader` saldırgan-kontrollü byte'ları
    // control alanları olarak yorumlar.
    for cp in [inner_kind::FEC_SOURCE, inner_kind::FEC_REPAIR] {
        assert_eq!(
            decode_control_body(cp, 1, &[0u8; 16]),
            Err(WireError::ControlDecode("non-control codepoint")),
            "cp={cp:#04x}"
        );
    }
}

#[test]
fn every_codepoint_outside_the_control_registry_is_rejected() {
    // §2.57 registry: 0x10..0x18. Tüm 256 codepoint taranır; registry dışı olan her biri
    // aynı sınıfta reddedilmeli. Bu tarama, registry'ye sessizce bir codepoint eklenirse
    // (ve §2.57 güncellenmezse) kırmızı yanar.
    let control_cps = [
        inner_kind::ACK,
        inner_kind::SACK,
        inner_kind::RECOVERED,
        inner_kind::BLOCK_COMPLETE,
        inner_kind::PARAM_SET,
        inner_kind::PARAM_ACK,
        inner_kind::BLOCK_ABORT,
        inner_kind::SESSION_CLOSE,
        inner_kind::SESSION_RENEW_REQUIRED,
    ];
    let body = [0u8; 64];
    let mut rejected = 0usize;
    for cp in 0..=255u8 {
        if control_cps.contains(&cp) {
            continue;
        }
        assert_eq!(
            decode_control_body(cp, 1, &body),
            Err(WireError::ControlDecode("non-control codepoint")),
            "codepoint={cp:#04x} control decoder'a sızdı"
        );
        rejected += 1;
    }
    assert_eq!(
        rejected,
        256 - control_cps.len(),
        "control registry boyutu değişmiş — §2.57 güncellenmiş olabilir"
    );
}

#[test]
fn empty_body_does_not_bypass_the_codepoint_guard() {
    // Boş gövde ile de codepoint kapısı ÖNCE düşmeli — `Reader` hiç çalışmadan.
    // Aksi hâlde reddetme nedeni "body too short" olur ve gözlemciye farklı bilgi verir.
    assert_eq!(
        decode_control_body(0x99, 1, &[]),
        Err(WireError::ControlDecode("non-control codepoint"))
    );
}

// ── SITE 2: recovered esi start>end ─────────────────────────────────────────

/// RECOVERED gövdesi: block_id u32 + count u16 + count × (esi_start u16, esi_end u16).
fn recovered_body(block_id: u32, ranges: &[(u16, u16)]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&block_id.to_be_bytes());
    b.extend_from_slice(&(ranges.len() as u16).to_be_bytes());
    for (s, e) in ranges {
        b.extend_from_slice(&s.to_be_bytes());
        b.extend_from_slice(&e.to_be_bytes());
    }
    b
}

#[test]
fn recovered_with_inverted_esi_range_is_rejected() {
    // `sprint2_control.rs::invalid_range_rejected` ACK kolunu kapatır; bu RECOVERED kolu.
    // Ters aralık, alıcı tarafında `for esi in s..=e` gibi bir döngüde ya boş küme ya da
    // (imzasız aritmetikte) taşma üretir — wire'da reddedilmeli.
    let body = recovered_body(7, &[(10, 5)]);
    assert_eq!(
        decode_control_body(inner_kind::RECOVERED, 1, &body),
        Err(WireError::ControlDecode("recovered esi start>end"))
    );
}

#[test]
fn recovered_rejects_inversion_anywhere_in_the_range_list() {
    // Doğrulama İLK aralıkta değil, HER aralıkta çalışmalı. Bir saldırgan geçerli aralıkların
    // arkasına bozuk bir tane gizleyebilir.
    let body = recovered_body(7, &[(0, 3), (4, 9), (100, 50)]);
    assert_eq!(
        decode_control_body(inner_kind::RECOVERED, 1, &body),
        Err(WireError::ControlDecode("recovered esi start>end"))
    );
}

#[test]
fn recovered_accepts_degenerate_and_full_width_ranges() {
    // Sınır: s == e (tek ESI) geçerlidir; 0..u16::MAX de geçerlidir. Guard `>` olmalı, `>=` değil.
    for ranges in [
        vec![(5u16, 5u16)],
        vec![(0, 0)],
        vec![(0, u16::MAX)],
        vec![(u16::MAX, u16::MAX)],
    ] {
        let body = recovered_body(7, &ranges);
        match decode_control_body(inner_kind::RECOVERED, 1, &body) {
            Ok(Control::Recovered { esi_ranges, .. }) => assert_eq!(esi_ranges, ranges),
            other => panic!("ranges={ranges:?} reddedildi: {other:?}"),
        }
    }
}

#[test]
fn recovered_range_check_precedes_trailing_byte_check() {
    // Sıra sözleşmesi: ters aralık, gövdenin sonunda fazladan byte olsa bile önce yakalanmalı.
    // Reddetme nedeninin hangi kapıdan geldiği tutarlı olmalı (BUL-10 hattı).
    let mut body = recovered_body(7, &[(10, 5)]);
    body.push(0xFF); // trailing
    assert_eq!(
        decode_control_body(inner_kind::RECOVERED, 1, &body),
        Err(WireError::ControlDecode("recovered esi start>end"))
    );
}

// ── decode_control() yönlendirmesi ──────────────────────────────────────────

#[test]
fn decode_control_returns_none_for_fec_inner() {
    // §4-ORDER-8: FEC-shard control decoder'a HİÇ girmemeli. `None` dönmezse yukarıdaki
    // codepoint guard'ı ikinci savunma hattı olarak devreye girer — ama ilk hat burasıdır.
    let inner = InnerPlain::FecShard {
        header: kama_transport_schema::LogicalShardHeader {
            kind: kama_transport_schema::inner_kind::ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: vec![0xAA; 64],
    };
    assert!(kama_noise_wire::control::decode_control(&inner).is_none());
}

#[test]
fn decode_control_propagates_body_errors_for_control_inner() {
    // Control-inner için hata YUTULMAMALI: `Some(Err(..))` dönmeli, `None` değil.
    // `None` dönseydi bozuk bir control sessizce "control değil" sayılır ve düşürülürdü.
    let inner = InnerPlain::Control {
        inner_kind: inner_kind::RECOVERED,
        control_id: 1,
        body: recovered_body(7, &[(10, 5)]),
    };
    match kama_noise_wire::control::decode_control(&inner) {
        Some(Err(WireError::ControlDecode("recovered esi start>end"))) => {}
        other => panic!("hata yutuldu veya sınıf değişti: {other:?}"),
    }
}

// ── Canonical encoding simetrisi ────────────────────────────────────────────

#[test]
fn accepted_recovered_body_is_byte_identical_to_the_encoder_output() {
    // Malleability kapısı: kabul edilen gövde, encoder'ın ürettiğiyle byte-eş olmalıdır.
    // Aksi hâlde aynı anlamın iki wire temsili olur ve dedup/replay muhasebesi kayabilir.
    let ranges = vec![(1u16, 4u16), (9, 9), (20, 30)];
    let body = recovered_body(0xDEAD_BEEF, &ranges);
    let decoded = decode_control_body(inner_kind::RECOVERED, 0x42, &body).expect("decode");
    assert_eq!(
        encode_control_body(&decoded),
        body,
        "kabul edilen gövde canonical değil"
    );
}
