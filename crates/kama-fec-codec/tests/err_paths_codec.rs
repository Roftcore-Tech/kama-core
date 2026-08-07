//! TC-06 — `codec.rs` + `profile.rs` kalan hata yolları (SR-STR-CORE-001 §5.1, BUL-05).
//!
//! **Kapsanan bulgu:** BUL-05. STR §3.4 `codec.rs`'te altı, `profile.rs`'te bir sıfır-sayımlı
//! construct-site raporlar. `codec.rs` region kapsamı %89,24 ile workspace'in en düşüğüdür.
//! Mevcut `roundtrip.rs` şu site'ları zaten kapatır: `SourceCount`, `SymbolLen`(encode yolu),
//! `DuplicateEsi`(reconstruct-source yolu), `Insufficient`, `ProfileBound`×3. Bu dosya YALNIZ
//! kalan site'ları hedefler — çakışma yok.
//!
//! **Gerekçe:** ADR-16 FEC için "Doğruluk > hız" der. Tutarsız `(k, r, esi)` kombinasyonlarında
//! decoder'ın fail-closed olduğu, ilgili TÜM site'larda kanıtlanmalıdır; aksi hâlde bir off-by-one
//! sessizce yanlış-reconstruct üretir ve INV-FEC-1'in decoder-tarafı savunması kâğıt üstünde kalır.
//!
//! **Oracle (tüm vakalar):** Geçersiz parametrelerde tipli `FecError`; sessiz bozuk çıktı yok; panik yok.
//!
//! ---
//! **[ÖLÇÜLDÜ — bu dosya] YAPISAL BULGU (TC-06/a):** `ordered_source()`'un
//! `EsiOutOfRange { kind: "source-gap" }` site'ı **erişilemezdir**. Fonksiyon önce
//! `source.len() == k` doğrular, sonra her ESI için `idx < k` ve `!ordered[idx].is_some()`
//! kontrolü yapar. k adet, hepsi ayrık ve hepsi `[0, k)` içinde olan indeks → güvercin-yuvası
//! gereği tüm slot'lar dolar. Dolayısıyla `slot.ok_or(...)` hiçbir girdiyle tetiklenemez.
//! Bu site sıfır sayımlıdır ve öyle KALACAKTIR; kapsam hedefine yazılmamalıdır.
//! Öneri (Abkarada/Yaaz): `unreachable!()` YASAK (CLAUDE.md §6), ancak
//! `debug_assert!(slot.is_some())` + `.expect()` yerine mevcut tipli-hata korunabilir —
//! karar modül sahibinin. Bu bir teklif değil, kayıt talebidir. [BEKLİYOR — Abkarada]
//!
//! **[ÖLÇÜLDÜ — bu dosya] YAPISAL BULGU (TC-06/b):** `recompute_repair()`'in ikinci
//! `EsiOutOfRange` site'ı (`.ok_or` sonrası) de pratikte erişilemezdir: `esi < r_code_max`
//! ön-koşulu geçildikten sonra `encode()` her zaman `0..R` aralığında R adet repair üretir.
//! Aynı kayıt talebi geçerlidir. [BEKLİYOR — Abkarada]

use kama_fec_codec::codec::{FecCodec, ReceivedSymbol, RsSimdCodec, RsSimdEngine, SourceSymbol};
use kama_fec_codec::error::FecError;
use kama_fec_codec::profile::{by_id, FecProfile, CODE_SUITE_RS_SIMD_V1};

const E: usize = 64;

fn blocks(k: usize) -> Vec<Vec<u8>> {
    (0..k).map(|i| vec![i as u8; E]).collect()
}

fn src<'a>(data: &'a [Vec<u8>], esis: &[u16]) -> Vec<SourceSymbol<'a>> {
    esis.iter()
        .enumerate()
        .map(|(i, &esi)| SourceSymbol {
            esi,
            bytes: &data[i],
        })
        .collect()
}

fn recv<'a>(data: &'a [Vec<u8>], items: &[(u16, bool)]) -> Vec<ReceivedSymbol<'a>> {
    items
        .iter()
        .enumerate()
        .map(|(i, &(esi, is_repair))| ReceivedSymbol {
            esi,
            is_repair,
            bytes: &data[i % data.len()],
        })
        .collect()
}

// ── (a) encode yolu: ordered_source() ───────────────────────────────────────

#[test]
fn encode_source_esi_at_or_above_k_shape_is_out_of_range() {
    // K_shape=4, doğru SAYIDA source ama bir ESI code-space dışında.
    let c = RsSimdCodec;
    let p = by_id(0x02).unwrap(); // K=4, R=4, E=64
    let data = blocks(4);
    assert_eq!(
        c.encode(&p, &src(&data, &[0, 1, 2, 4])),
        Err(FecError::EsiOutOfRange {
            esi: 4,
            kind: "source"
        })
    );
    // Sınır: esi = k-1 kabul, esi = k reddedilir (off-by-one kapısı).
    assert!(c.encode(&p, &src(&data, &[0, 1, 2, 3])).is_ok());
}

#[test]
fn encode_duplicate_source_esi_is_duplicate() {
    // roundtrip.rs::duplicate_esi_reject reconstruct yolundaki site'ı kapatır; bu ENCODE yolu.
    let c = RsSimdCodec;
    let p = by_id(0x02).unwrap();
    let data = blocks(4);
    assert_eq!(
        c.encode(&p, &src(&data, &[0, 1, 1, 3])),
        Err(FecError::DuplicateEsi(1))
    );
}

#[test]
fn engine_reuse_rejects_same_inputs_as_stateless() {
    // RsSimdEngine ve RsSimdCodec aynı çekirdek `ordered_source`'u paylaşır; hata davranışı
    // byte-eş olduğu gibi HATA-EŞ de olmalıdır (aksi hâlde reuse-yolu ayrı bir kabul yüzeyi açar).
    let engine = RsSimdEngine::new();
    let stateless = RsSimdCodec;
    let p = by_id(0x02).unwrap();
    let data = blocks(4);
    for esis in [vec![0u16, 1, 1, 3], vec![0, 1, 2, 9]] {
        assert_eq!(
            engine.encode(&p, &src(&data, &esis)),
            stateless.encode(&p, &src(&data, &esis)),
            "engine/stateless hata-eşliği bozuldu: {esis:?}"
        );
    }
}

// ── (b) recompute_repair(): repair ESI code-space ───────────────────────────

#[test]
fn recompute_repair_esi_at_or_above_r_code_max_is_out_of_range() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8, R=4
    let data = blocks(8);
    let s = src(&data, &[0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(
        c.recompute_repair(&p, &s, 4),
        Err(FecError::EsiOutOfRange {
            esi: 4,
            kind: "repair"
        })
    );
    // Sınır: R-1 geçerli.
    assert!(c.recompute_repair(&p, &s, 3).is_ok());
}

// ── (c) reconstruct yolu: prepare_received() ────────────────────────────────

#[test]
fn reconstruct_symbol_len_mismatch_is_rejected() {
    // roundtrip.rs::wrong_symbol_len_fail_closed ENCODE yolunu kapatır; bu DECODE yolu.
    let c = RsSimdCodec;
    let p = by_id(0x02).unwrap(); // E=64
    let short = vec![0u8; 32];
    let got = c.reconstruct(
        &p,
        &[ReceivedSymbol {
            esi: 0,
            is_repair: false,
            bytes: &short,
        }],
        0,
    );
    assert_eq!(
        got,
        Err(FecError::SymbolLen {
            got: 32,
            expected: 64
        })
    );
}

#[test]
fn reconstruct_repair_esi_at_or_above_r_code_max_is_out_of_range() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8, R=4
    let data = blocks(8);
    assert_eq!(
        c.reconstruct(&p, &recv(&data, &[(4, true)]), 0),
        Err(FecError::EsiOutOfRange {
            esi: 4,
            kind: "repair"
        })
    );
}

#[test]
fn reconstruct_source_esi_at_or_above_k_shape_is_out_of_range() {
    // §6 fail-closed: source ESI önce K_shape'e karşı sınanır (SourceInZeroRegion'dan ÖNCE).
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8
    let data = blocks(8);
    assert_eq!(
        c.reconstruct(&p, &recv(&data, &[(8, false)]), 0),
        Err(FecError::EsiOutOfRange {
            esi: 8,
            kind: "source"
        })
    );
}

#[test]
fn reconstruct_duplicate_repair_esi_is_duplicate() {
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap(); // K=8, R=4
    let data = blocks(8);
    assert_eq!(
        c.reconstruct(&p, &recv(&data, &[(1, true), (1, true)]), 0),
        Err(FecError::DuplicateEsi(1))
    );
}

#[test]
fn reject_order_is_stable_len_before_range() {
    // Hem kısa symbol hem out-of-range ESI taşıyan girdi: symbol_len ÖNCE reddedilir.
    // Bu sıra, reddetme-nedeninin gözlemciye hangi bilgiyi verdiğini sabitler (BUL-10 hattı).
    let c = RsSimdCodec;
    let p = by_id(0x03).unwrap();
    let short = vec![0u8; 8];
    assert_eq!(
        c.reconstruct(
            &p,
            &[ReceivedSymbol {
                esi: 99,
                is_repair: true,
                bytes: &short
            }],
            0
        ),
        Err(FecError::SymbolLen {
            got: 8,
            expected: 64
        })
    );
}

// ── (d) profile.rs: kalan ProfileBound ──────────────────────────────────────

#[test]
fn profile_r_code_max_zero_is_profile_bound() {
    // roundtrip.rs::profile_validation_bounds K_shape>255 / odd-E / tiny-E'yi kapatır;
    // R_code_max < 1 sıfır sayımlı kalan tek site'tır.
    let p = FecProfile {
        profile_id: 0x7F,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 4,
        r_code_max: 0,
        e: 64,
    };
    assert_eq!(p.validate(), Err(FecError::ProfileBound("R_code_max >= 1")));
}

#[test]
fn profile_k_shape_zero_is_profile_bound() {
    let p = FecProfile {
        profile_id: 0x7F,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 0,
        r_code_max: 2,
        e: 64,
    };
    assert_eq!(
        p.validate(),
        Err(FecError::ProfileBound("1 <= K_shape <= 255"))
    );
}

#[test]
fn invalid_profile_is_rejected_before_any_engine_work() {
    // encode/reconstruct'ın İLK adımı profile.validate() olmalı: geçersiz profille RS-engine
    // hiç kurulmaz. (Aksi hâlde tutarsız profil RS-katmanına opak `RsEngine` hatası olarak sızar.)
    let c = RsSimdCodec;
    let bad = FecProfile {
        profile_id: 0x7F,
        code_suite_id: CODE_SUITE_RS_SIMD_V1,
        k_shape: 4,
        r_code_max: 0,
        e: 64,
    };
    let data = blocks(4);
    assert!(matches!(
        c.encode(&bad, &src(&data, &[0, 1, 2, 3])),
        Err(FecError::ProfileBound(_))
    ));
    assert!(matches!(
        c.reconstruct(&bad, &recv(&data, &[(0, false)]), 0),
        Err(FecError::ProfileBound(_))
    ));
}

// ── (e) validate() ile RS-backend kısıtı: ÖLÇÜLDÜ, boşluk YOK ───────────────

#[test]
fn even_non_64_multiple_e_roundtrips_correctly() {
    // [ÖLÇÜLDÜ — bu dosya] Hipotez ÇÜRÜDÜ. `FecProfile::validate()` yalnız "E çift" ister;
    // `profile.rs` doc-comment'ı 64-katını "mümkünse" der. Bunun bir FAIL-OPEN olduğu
    // (validate geçer, motor sessizce bozuk çıktı verir) hipotezi kuruldu ve ölçüldü:
    //
    //   E ∈ {64, 66, 70, 128, 130} · K=2, R=2 · source-0 silinip repair-0 ile reconstruct
    //   → beşinde de `MATCH=true` (recovered bytes == orijinal source-0).
    //
    // Yani reed-solomon-simd 3.1 64-katı-olmayan even-E'yi DOĞRU işler; `validate()`'in
    // 64-katı kontrolü içermemesi bir doğruluk açığı DEĞİLDİR (performans tercihi olabilir —
    // o ayrı bir ölçüm konusudur, bu belgenin kapsamı dışında).
    //
    // Test yine de tutulur: katalog dışı bir E ile üretilen çıktının reconstruct-doğruluğunu
    // kapıya bağlar. RS backend sürümü değişip bu davranış bozulursa burada kırmızı yanar.
    let c = RsSimdCodec;
    for e in [66usize, 70, 130] {
        let p = FecProfile {
            profile_id: 0x7F,
            code_suite_id: CODE_SUITE_RS_SIMD_V1,
            k_shape: 2,
            r_code_max: 2,
            e: e as u16,
        };
        assert_eq!(
            p.validate(),
            Ok(()),
            "E={e} validate() beklenmedik şekilde reddetti"
        );

        let d0: Vec<u8> = (0..e)
            .map(|i| (i as u8).wrapping_mul(7).wrapping_add(1))
            .collect();
        let d1: Vec<u8> = (0..e)
            .map(|i| (i as u8).wrapping_mul(13).wrapping_add(5))
            .collect();
        let repairs = c
            .encode(
                &p,
                &[
                    SourceSymbol { esi: 0, bytes: &d0 },
                    SourceSymbol { esi: 1, bytes: &d1 },
                ],
            )
            .unwrap_or_else(|err| panic!("E={e} encode başarısız: {err:?}"));

        // source-0 kayıp: source-1 + repair-0 ile reconstruct.
        let out = c
            .reconstruct(
                &p,
                &[
                    ReceivedSymbol {
                        esi: 1,
                        is_repair: false,
                        bytes: &d1,
                    },
                    ReceivedSymbol {
                        esi: 0,
                        is_repair: true,
                        bytes: &repairs[0].bytes,
                    },
                ],
                0,
            )
            .unwrap_or_else(|err| panic!("E={e} reconstruct başarısız: {err:?}"));

        assert_eq!(out.len(), 1, "E={e} tek kayıp bekleniyordu");
        assert_eq!(out[0].esi, 0);
        assert_eq!(
            out[0].bytes, d0,
            "E={e} SESSİZ BOZULMA: recovered != orijinal"
        );
    }
}
