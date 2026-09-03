//! TC-01 — `inner.rs` inner-parse hata yolları (SR-STR-CORE-001 §5.1, BUL-05).
//!
//! **Kapsanan bulgu:** BUL-05. STR §3.4 `inner.rs`'te YEDİ sıfır-sayımlı `InnerParse` site'ı
//! raporlar — dosyadaki tüm hata yolları. Bu, tek bir dosyadaki en büyük kanıt boşluğudur.
//!
//! **Gerekçe:** Bu katman AEAD'in İÇİNDEDİR, yani buraya ulaşan byte'lar authenticate edilmiştir —
//! ancak **authenticate edilmiş olmak iyi biçimli olmak demek değildir.** Bozuk bir gönderen,
//! uyumsuz bir sürüm veya gelecekteki bir format değişikliği bu yolları tetikler; gönderen
//! anahtarı paylaşan, kimliği doğrulanmış bir eş de olabilir (compromised-peer).
//!
//! Ayrıca BUL-04 gereği bu katman `fuzz_open` ile **erişilemezdir** (AEAD reddi inner-parse'tan
//! önce gelir; geçerli tag üretme olasılığı 2⁻¹²⁸). Kapsamın iki yolu vardır: doğrudan birim testi
//! (bu dosya) ve AEAD-baypaslı fuzz hedefi (`fuzz/fuzz_targets/fuzz_inner_parse.rs`).
//!
//! **Oracle:** Her vaka tipli `InnerParse` hatası döndürür; panik yok; kısmi/bozuk `InnerPlain`
//! üretilmez. Ayrıca hata sınıfı `InnerParse` DIŞINA çıkmaz (katman sınırı).

use kama_noise_wire::error::WireError;
use kama_noise_wire::inner::{
    encode_control_inner, encode_fec_inner, parse_inner, InnerPlain, CONTROL_INNER_PREFIX_LEN,
    FEC_INNER_HDR_LEN,
};
use kama_transport_schema::inner_kind::{self, ShardKind};
use kama_transport_schema::{LogicalShardHeader, INNER_V1};

/// Geçerli bir FEC-inner plaintext üretir (E byte symbol ile).
fn valid_fec(e: usize) -> Vec<u8> {
    encode_fec_inner(
        &LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        &vec![0xA5u8; e],
    )
}

fn expect_inner_parse(plain: &[u8], case: &str) -> &'static str {
    match parse_inner(plain) {
        Err(WireError::InnerParse(msg)) => msg,
        other => panic!("{case}: InnerParse beklendi, alınan {other:?}"),
    }
}

// ── SITE 1: "inner too short" ───────────────────────────────────────────────

#[test]
fn plaintext_below_two_bytes_is_rejected() {
    // inner_version + inner_kind okunamadan reddedilmeli. Boş plaintext dahil.
    for len in 0..2usize {
        let plain = vec![INNER_V1; len];
        assert_eq!(
            expect_inner_parse(&plain, "kısa inner"),
            "inner too short",
            "len={len}"
        );
    }
    // Sınır: 2 byte version+kind okumaya yeter (sonraki kapıya düşer, "too short" DEĞİL).
    let two = [INNER_V1, inner_kind::FEC_SOURCE];
    assert_eq!(
        expect_inner_parse(&two, "2B"),
        "fec inner too short",
        "2 byte artık version+kind kapısını geçmeli"
    );
}

// ── SITE 2: "unknown inner_version" ─────────────────────────────────────────

#[test]
fn unknown_inner_version_is_rejected() {
    // §2.56: INNER_V1 dışındaki her version fail-closed. Forward-compat DEĞİL: bilinmeyen
    // sürüm sessizce v1 gibi yorumlanırsa, gelecekteki bir alan değişikliği yanlış parse edilir.
    let mut plain = valid_fec(64);
    for v in [0x00u8, 0x02, 0x7F, 0x80, 0xFF] {
        plain[0] = v;
        assert_eq!(
            expect_inner_parse(&plain, "version"),
            "unknown inner_version",
            "version={v:#04x} kabul edildi"
        );
    }
    // Sınır: INNER_V1 geçer.
    plain[0] = INNER_V1;
    assert!(parse_inner(&plain).is_ok());
}

// ── SITE 3: "unknown inner_kind" ────────────────────────────────────────────

#[test]
fn unknown_inner_kind_codepoint_is_rejected() {
    // §2.56 registry: 0x01/0x02 (FEC) + 0x10..0x1A (control). Kalan tüm codepoint'ler reserved.
    // [2026-09-02] 0x19/0x1A (CTRL_CS_PROBE/ECHO) DR-CS-3 ile registry'ye meşru girdi —
    // liste güncel inner_kind'a eşitlendi (err_paths_control.rs taramasıyla aynı olay).
    let known: Vec<u8> = vec![
        inner_kind::FEC_SOURCE,
        inner_kind::FEC_REPAIR,
        inner_kind::ACK,
        inner_kind::SACK,
        inner_kind::RECOVERED,
        inner_kind::BLOCK_COMPLETE,
        inner_kind::PARAM_SET,
        inner_kind::PARAM_ACK,
        inner_kind::BLOCK_ABORT,
        inner_kind::SESSION_CLOSE,
        inner_kind::SESSION_RENEW_REQUIRED,
        inner_kind::CTRL_CS_PROBE,
        inner_kind::CTRL_CS_ECHO,
    ];
    // Tüm 256 codepoint'i tara: registry dışındaki her biri reddedilmeli.
    let mut plain = valid_fec(64);
    let mut rejected = 0usize;
    for cp in 0..=255u8 {
        plain[1] = cp;
        if known.contains(&cp) {
            continue;
        }
        assert_eq!(
            expect_inner_parse(&plain, "kind"),
            "unknown inner_kind",
            "codepoint={cp:#04x} reddedilmedi"
        );
        rejected += 1;
    }
    assert_eq!(
        rejected,
        256 - known.len(),
        "registry dışı codepoint sayısı değişmiş — §2.56 güncellenmiş olabilir"
    );
}

// ── SITE 4: "fec inner too short" ───────────────────────────────────────────

#[test]
fn fec_inner_below_header_len_is_rejected() {
    // FEC header 11B sabit. 2..10 arası her uzunluk reddedilmeli (bounds-before-read).
    let full = valid_fec(64);
    for len in 2..FEC_INNER_HDR_LEN {
        let plain = &full[..len];
        assert_eq!(
            expect_inner_parse(plain, "fec kısa"),
            "fec inner too short",
            "len={len}"
        );
    }
}

#[test]
fn fec_repair_kind_follows_same_bounds() {
    // Registry'de iki FEC codepoint'i var; ikisi de aynı bounds-check'e tabi olmalı.
    let mut full = valid_fec(64);
    full[1] = inner_kind::FEC_REPAIR;
    assert!(matches!(
        parse_inner(&full),
        Ok(InnerPlain::FecShard { .. })
    ));
    assert_eq!(
        expect_inner_parse(&full[..7], "repair kısa"),
        "fec inner too short"
    );
}

// ── SITE 5: "fec inner reserved flags" ──────────────────────────────────────

#[test]
fn fec_reserved_flags_nonzero_is_rejected() {
    // §2.6: flags byte'ı (offset 2) reserved, HEP 0 yazılır. Nonzero → reject.
    // Bu, ileride flag eklenirse eski alıcıların sessizce yok saymamasını garantiler.
    let mut plain = valid_fec(64);
    for bit in 0..8u8 {
        plain[2] = 1u8 << bit;
        assert_eq!(
            expect_inner_parse(&plain, "flags"),
            "fec inner reserved flags",
            "flags bit {bit} kabul edildi"
        );
    }
    plain[2] = 0xFF;
    assert_eq!(
        expect_inner_parse(&plain, "flags 0xFF"),
        "fec inner reserved flags"
    );
}

// ── SITE 6: "k_actual zero (B4)" ────────────────────────────────────────────

#[test]
fn k_actual_zero_is_rejected() {
    // B4-invariant: 1 ≤ K_actual ≤ K_shape. k_actual=0 decoder'da sıfır-bölme veya
    // boş-blok semantiği üretir; wire'da kabul edilmemeli.
    let mut plain = valid_fec(64);
    plain[10] = 0;
    assert_eq!(expect_inner_parse(&plain, "k_actual"), "k_actual zero (B4)");
    // Sınır: k_actual=1 geçerli (tek gerçek source + zero-pad).
    plain[10] = 1;
    assert!(parse_inner(&plain).is_ok());
}

// ── SITE 7: "empty symbol" ──────────────────────────────────────────────────

#[test]
fn fec_with_zero_length_symbol_is_rejected() {
    // Tam 11B header, symbol yok. Header geçerli ama taşınan veri yok → E ≥ 11 invariant'ının
    // wire-yüzü. Bu, `sealed_len`'in EmptySymbol kapısıyla simetriktir (bkz. err_paths_protector).
    let plain = valid_fec(0);
    assert_eq!(plain.len(), FEC_INNER_HDR_LEN);
    assert_eq!(expect_inner_parse(&plain, "boş symbol"), "empty symbol");
    // Sınır: 1 byte symbol kabul edilir (parser E'yi bilmez — profil kontrolü üst katmanda).
    assert!(parse_inner(&valid_fec(1)).is_ok());
}

// ── SITE 8: "control inner too short" ───────────────────────────────────────

#[test]
fn control_inner_below_prefix_len_is_rejected() {
    // Control prefix 6B: version 1 + kind 1 + control_id 4.
    let full = encode_control_inner(inner_kind::ACK, 0x1122_3344, &[0x00, 0x00, 0x00]);
    for len in 2..CONTROL_INNER_PREFIX_LEN {
        assert_eq!(
            expect_inner_parse(&full[..len], "control kısa"),
            "control inner too short",
            "len={len}"
        );
    }
    // Sınır: tam 6B (boş body) kabul edilir — body-doğrulama control.rs'in işi, inner'ın değil.
    let empty_body = encode_control_inner(inner_kind::ACK, 0x1122_3344, &[]);
    assert_eq!(empty_body.len(), CONTROL_INNER_PREFIX_LEN);
    assert!(matches!(
        parse_inner(&empty_body),
        Ok(InnerPlain::Control { .. })
    ));
}

#[test]
fn all_control_codepoints_share_the_same_prefix_bound() {
    // Dokuz control türünün hepsi aynı 6B prefix kapısına tabi olmalı; birinin gevşek olması
    // bir DoS/parse-confusion yüzeyi açar.
    for cp in [
        inner_kind::ACK,
        inner_kind::SACK,
        inner_kind::RECOVERED,
        inner_kind::BLOCK_COMPLETE,
        inner_kind::PARAM_SET,
        inner_kind::PARAM_ACK,
        inner_kind::BLOCK_ABORT,
        inner_kind::SESSION_CLOSE,
        inner_kind::SESSION_RENEW_REQUIRED,
    ] {
        let full = encode_control_inner(cp, 1, &[0xAA]);
        assert_eq!(
            expect_inner_parse(&full[..5], "control 5B"),
            "control inner too short",
            "cp={cp:#04x}"
        );
    }
}

// ── Kabul yolunun yapısal tutarlılığı ───────────────────────────────────────

#[test]
fn accepted_fec_roundtrips_all_header_fields() {
    // Kabul edilen bir plaintext'in TÜM alanları geri okunabilmeli; sessiz alan kaybı olmamalı.
    let header = LogicalShardHeader {
        kind: ShardKind::Repair,
        profile_id: 0x05,
        block_id: 0xDEAD_BEEF,
        esi: 0xC0DE,
        k_actual: 255,
    };
    let symbol: Vec<u8> = (0..=255u8).collect();
    let plain = encode_fec_inner(&header, &symbol);
    match parse_inner(&plain).expect("geçerli FEC-inner") {
        InnerPlain::FecShard {
            header: h,
            symbol: s,
        } => {
            assert_eq!(h, header, "header alanları roundtrip'te değişti");
            // symbol artık SensitiveSymbolBytes (M7): byte-karşılaştırma borrow-only AsRef üzerinden.
            assert_eq!(s.as_ref(), symbol.as_slice(), "symbol roundtrip'te değişti");
        }
        InnerPlain::Control { .. } => panic!("FEC beklendi"),
    }
}

#[test]
fn accepted_control_roundtrips_id_and_body() {
    let body = vec![0x11u8, 0x22, 0x33, 0x44, 0x55];
    let plain = encode_control_inner(inner_kind::SACK, 0xFFFF_FFFF, &body);
    match parse_inner(&plain).expect("geçerli control-inner") {
        InnerPlain::Control {
            inner_kind: k,
            control_id,
            body: b,
        } => {
            assert_eq!(k, inner_kind::SACK);
            assert_eq!(control_id, 0xFFFF_FFFF);
            assert_eq!(b, body);
        }
        InnerPlain::FecShard { .. } => panic!("Control beklendi"),
    }
}

#[test]
fn parse_inner_never_leaves_the_inner_parse_error_class() {
    // Katman sınırı: bu fonksiyon YALNIZ InnerParse üretmelidir. Başka bir variant
    // (AeadFail, EpochNotAccepted, PnReject…) sızarsa hata taksonomisi bozulmuştur ve
    // üst katmanlar yanlış sınıflandırma yapar. Sistematik bozma taraması.
    let base = valid_fec(32);
    for pos in 0..base.len().min(16) {
        for delta in [0x01u8, 0x7F, 0x80, 0xFF] {
            let mut plain = base.clone();
            plain[pos] ^= delta;
            match parse_inner(&plain) {
                Ok(_) | Err(WireError::InnerParse(_)) => {}
                Err(other) => panic!("pos={pos} delta={delta:#04x}: katman dışı hata {other:?}"),
            }
        }
    }
    // Kısaltma taraması.
    for len in 0..base.len() {
        match parse_inner(&base[..len]) {
            Ok(_) | Err(WireError::InnerParse(_)) => {}
            Err(other) => panic!("len={len}: katman dışı hata {other:?}"),
        }
    }
}
