//! BİÇİMSEL DOĞRULAMA (Kani) — `pn.rs::reconstruct` taşma güvenliği ve sonuç bandı.
//!
//! **Neden bu fonksiyon:** `reconstruct` truncated wire-PN'i 64-bit tam sayaca çevirir
//! (RFC 9000 A.3'ün özelleştirmesi). Saf, sınırlı tamsayı aritmetiği — Kani'nin tam olarak
//! ispatlayabildiği şekil. Ve ÜÇ ayrı taşma guard'ı taşıyor:
//!
//! ```text
//! cand.checked_add(half).is_some_and(|v| v < center)   // taşma-güvenli toplama
//! cand > center.saturating_add(half)                    // doygun toplama
//! && cand >= span                                       // underflow guard (§4)
//! ```
//!
//! **Test ile ispatın farkı:** `pn_boundary_mutants.rs` bu guard'ların TEST EDİLDİĞİNİ
//! gösterdi (mutation skoru 13 missed → 2, ikisi eşdeğer). `pn_proptest.rs` de örnekleme
//! yapıyor. Ama ikisi de SONLU sayıda nokta deniyor. Kani `(expected_pn, low_bits)`
//! uzayının TAMAMI için ispatlar — 2^96 kombinasyon, hiçbiri koşturulmadan.
//!
//! **Neden önemli:** Bu fonksiyon her gelen datagram için çalışır ve girdisi
//! SALDIRGAN KONTROLÜNDEDİR (`pn_trunc` cleartext outer'dan okunuyor, AEAD'den ÖNCE).
//! Bir panik burada uzaktan tetiklenebilir DoS demektir. Örnekleme "bulamadım" der;
//! ispat "yok" der.
//!
//! Koş:
//! ```text
//! cargo kani -p kama-noise-wire --tests --harness reconstruct_never_panics
//! cargo kani -p kama-noise-wire --tests            # tüm harness'lar
//! ```
//! `--tests` ZORUNLU: Kani varsayılan olarak yalnız lib target'ını tarar, entegrasyon
//! testlerindeki harness'ları görmez.
//!
//! CI: haftalık işte koşar (`core-weekly` / kani; Kani kurulumu + CBMC ~1 GB, PR başına uygun değil).
//!
//! **DURUM (2026-10-04, M1):** K-1 düzeltildi. Eskiden kısıtsız `expected_pn` ile koşan beş ispat ve
//! `near_max` ispatı `pn.rs`'te `cand += span` taşmasıyla düşüyordu (8 harness'ten 6'sı kırmızı).
//! `reconstruct` artık u64'ün en üst span bloğunda ileri adayı ÜRETMİYOR; `commit` de uç değerde
//! taşmıyor. Tüm harness'lar yeşil beklenir.

#![cfg(kani)]

use kama_noise_wire::pn::{reconstruct, PnParams, EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};
use kama_noise_wire::SEND_PN_CAP;

/// Doğrulanan parametre setleri. `low_width_bits` yalnız 16 ve 32 olabilir (§4);
/// keyfi genişlik ispatı anlamsız olurdu çünkü üretimde başka değer üretilmiyor.
fn any_params() -> PnParams {
    if kani::any::<bool>() {
        EXTENDED_PN_PARAMS
    } else {
        LEGACY_PN_PARAMS
    }
}

/// **İSPAT 1 — panik yok.**
///
/// Hiçbir `(expected_pn, low_bits)` çifti `reconstruct`'ta panik üretmez: ne taşma,
/// ne underflow, ne shift-overflow.
///
/// Bu, saldırgan-kontrollü girdi için en temel güvenlik özelliğidir. `pn_trunc` cleartext
/// okunuyor ve `reconstruct` AEAD doğrulamasından ÖNCE çağrılıyor — yani buradaki bir panik
/// kimlik doğrulaması olmadan uzaktan tetiklenebilir.
#[kani::proof]
fn reconstruct_never_panics() {
    let p = any_params();
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    let _ = reconstruct(&p, expected, low);
}

/// **İSPAT 2 — düşük bitler korunur.**
///
/// Sözleşme içi girdi (`low_bits < span`) için sonucun düşük `low_width_bits` biti
/// HER ZAMAN `low_bits`'e eşittir.
#[kani::proof]
fn reconstruct_preserves_low_bits() {
    let p = any_params();
    let span: u64 = 1u64 << p.low_width_bits;
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    kani::assume(low < span); // sözleşme ön-koşulu

    let got = reconstruct(&p, expected, low);
    assert!(
        got & (span - 1) == low,
        "düşük bitler korunmadı — yeniden kurulum wire'ı bozdu"
    );
}

/// **İSPAT 3 — sonuç bandı.**
///
/// Sonuç `expected`'e en fazla bir `span` uzaklıktadır. `seq_and_reader_bounds.rs`'teki
/// `result_is_always_within_one_window_of_expected` bunu ÖRNEKLEME ile (20 nokta) test
/// ediyor — bu, ispat hâli.
#[kani::proof]
fn reconstruct_stays_within_one_span() {
    let p = any_params();
    let span: u64 = 1u64 << p.low_width_bits;
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    kani::assume(low < span);

    let got = reconstruct(&p, expected, low);
    let dist = if got >= expected {
        got - expected
    } else {
        expected - got
    };
    assert!(
        dist <= span,
        "sonuç bir span'den uzağa taşındı — kollardan biri yanlış tetikleniyor"
    );
}

/// **İSPAT 4 — determinizm.**
///
/// Aynı girdi her zaman aynı sonucu verir (saf fonksiyon). ADR-17 "RNG yok" kuralının
/// bu fonksiyondaki karşılığı; gizli global durum sızarsa burada yakalanır.
#[kani::proof]
fn reconstruct_is_deterministic() {
    let p = any_params();
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    assert!(
        reconstruct(&p, expected, low) == reconstruct(&p, expected, low),
        "aynı girdi farklı sonuç verdi — saf değil"
    );
}

/// **İSPAT 5 — sözleşme dışı girdi de panik üretmez.**
///
/// `low_bits >= span` ön-koşul ihlalidir (bkz. `pn_boundary_mutants.rs` — üretimdeki iki
/// çağıran da bunu sağlıyor ama fonksiyon public). İhlal hâlinde sonuç TANIMSIZ olabilir
/// ama PANİK OLMAMALIDIR; aksi hâlde public API'de bir DoS yüzeyi kalır.
#[kani::proof]
fn reconstruct_does_not_panic_on_contract_violation() {
    let p = any_params();
    let span: u64 = 1u64 << p.low_width_bits;
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    kani::assume(low >= span); // ön-koşul İHLALİ
    let _ = reconstruct(&p, expected, low);
}

// ════════════════════════════════════════════════════════════════════════════
// ERİŞİLEBİLİRLİK SINIRI — K-1 defektinin (düzeltildi) üretimde tetiklenip tetiklenmediği
// ════════════════════════════════════════════════════════════════════════════
//
// [TARİHÇE] Düzeltmeden önce yukarıdaki beş ispat KISITSIZ `expected_pn` ile `cand += span`
// satırında düşüyordu. "Fonksiyon yanlış" ile "sistem güvensiz" AYRI iddialardır; aşağıdaki iki
// ispat o sınırı çizmişti ve düzeltmeden sonra regresyon kapısı olarak kalır.

/// **İSPAT 6 — üretim alanında panik YOK.**
///
/// [ÖLÇÜLDÜ] `expected_pn < SEND_PN_CAP` (2^62) ve `low_bits < 2^32` (u32 `pn_trunc`)
/// kısıtları altında Kani **SUCCESSFUL** verir. Gönderici `SEND_PN_CAP`'te durduğu için
/// alıcının `expected_pn`'i oraya bile ulaşamaz.
///
/// Yani düzeltmeden önce de mevcut çağıranlarla defekt ERİŞİLEMEZDİ.
#[kani::proof]
fn bounded_expected_never_panics() {
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    kani::assume(expected < SEND_PN_CAP);
    kani::assume(low < (1u64 << 32));
    let _ = reconstruct(&EXTENDED_PN_PARAMS, expected, low);
}

/// **İSPAT 7 — eski tetikleyici bölgede panik YOK.**
///
/// `expected_pn > u64::MAX - 2^31` bölgesi K-1'in tetikleyicisiydi (2^64 uzayın 2^-33'ü; rastgele
/// örnekleme oraya pratikte düşmez — proptest ve fuzz bu yüzden bulamamıştı). Düzeltmeden önce bu
/// harness KASTEN KIRMIZI idi; düzeltmeyle yeşile döndü ve adı değişti
/// (eski ad: `near_max_expected_panics_known_defect`).
#[kani::proof]
fn near_max_expected_never_panics() {
    let expected: u64 = kani::any();
    let low: u64 = kani::any();
    kani::assume(expected > u64::MAX - (1u64 << 31));
    kani::assume(low < (1u64 << 32));
    let _ = reconstruct(&EXTENDED_PN_PARAMS, expected, low);
}

// ── ReplayWindow::precheck — saf karar fonksiyonu ───────────────────────────

/// **İSPAT 8 — precheck panik üretmez.**
/// `candidate` saldırgan-kontrollü (`pn_trunc`'tan türetiliyor) ve `precheck` AEAD'den
/// ÖNCE çalışıyor. Aritmetiği: `candidate + window`, `ew - candidate`, `candidate - ew`.
/// [ÖLÇÜLDÜ] SUCCESSFUL.
#[kani::proof]
#[kani::unwind(20)]
fn precheck_never_panics() {
    use kama_noise_wire::pn::ReplayWindow;
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    // commit() ew..=candidate döngüsü içerir; window=2^14 olduğundan sınırsız unwind
    // pratik değil. Küçük bir commit ile döngüyü sınırlayıp precheck'in KENDİ
    // aritmetiğini ispatlıyoruz (asıl saldırgan yüzeyi orası).
    w.commit(1);
    let candidate: u64 = kani::any();
    let _ = w.precheck(candidate);
}

// ── ReplayWindow::commit — uç değerde taşma yok ─────────────────────────────

/// **İSPAT 9 — `precheck`'ten geçen her `candidate` için `commit` panik üretmez.**
///
/// [D-GAP-1] legacy'de far-future kapısı kalktığı için `candidate` `expected`ten keyfi uzakta
/// olabilir (ör. `expected = 0`, `candidate = u64::MAX - 1`). Eski `candidate - ew + 1` ve
/// `candidate + 1` ifadeleri o uçta taşardı. Pencere KÜÇÜK seçilir (64) ki `commit`'in bit-temizleme
/// döngüsü sınırlı unwind ile tam ispatlanabilsin; aritmetik pencere boyundan bağımsızdır.
#[kani::proof]
#[kani::unwind(66)]
fn commit_never_panics_after_precheck() {
    use kama_noise_wire::pn::ReplayWindow;
    let params = PnParams {
        low_width_bits: 16,
        window: 64,
        future_gap_max: None,
    };
    let mut w = ReplayWindow::new(params);
    let first: u64 = kani::any();
    if w.precheck(first).is_ok() {
        w.commit(first);
    }
    let second: u64 = kani::any();
    if w.precheck(second).is_ok() {
        w.commit(second);
        assert!(
            w.expected_pn() > second,
            "kabul edilen PN her zaman expected'in gerisinde kalmalı"
        );
    }
}
