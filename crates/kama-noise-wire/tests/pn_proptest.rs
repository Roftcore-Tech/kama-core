//! §4 PN anti-replay property-testleri (proptest): rastgele kayıp/reorder/dup dizisi → invariant'lar.
//! İKİ-SABİT-SETİ (DR-3) ayrı-ayrı: extended-pn32 + legacy-seq16.

use kama_noise_wire::pn::{
    reconstruct, PnParams, ReplayWindow, EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS,
};
use proptest::prelude::*;
use std::collections::HashSet;

/// Bir accept(candidate) simülasyonu: precheck geçerse commit (AEAD-success varsay).
/// Döner: kabul-edildi mi.
fn try_accept(w: &mut ReplayWindow, candidate: u64) -> bool {
    if w.precheck(candidate).is_ok() {
        w.commit(candidate);
        true
    } else {
        false
    }
}

proptest! {
    // INVARIANT-1: hiçbir candidate İKİ KEZ kabul edilmez (replay-guard), rastgele-sıra.
    #[test]
    fn no_double_accept_extended(seq in prop::collection::vec(0u64..2000, 1..400)) {
        let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
        let mut accepted: HashSet<u64> = HashSet::new();
        for pn in seq {
            let ok = try_accept(&mut w, pn);
            if ok {
                prop_assert!(accepted.insert(pn), "pn={pn} iki kez kabul edildi (replay!)");
            } else {
                // Reddedilen ya zaten-kabul (dup) ya pencere-dışı — ikisi de invariant-korur.
            }
        }
    }

    // INVARIANT-2: monoton-artan dizi (adımlar future_gap içinde, ilk-paket expected_pn=0'a yakın) HEPSI
    // kabul + expected_pn = max+1. (İlk-paketin future_gap ötesi olması DOĞRU şekilde reddedilir — o ayrı
    // invariant future_gap_rejects_far_first.) start < future_gap_max, step < future_gap_max.
    #[test]
    fn monotonic_all_accepted(start in 0u64..8000, count in 1u64..300, step in 1u64..1000) {
        let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
        let mut last = None;
        for i in 0..count {
            let pn = start + i * step;
            prop_assert!(try_accept(&mut w, pn), "monoton pn={pn} reddedildi (adım future_gap içinde)");
            last = Some(pn);
        }
        if let Some(l) = last {
            prop_assert_eq!(w.expected_pn(), l + 1);
        }
    }

    // INVARIANT-2b: expected_pn=0'dan future_gap_max ötesi İLK-paket DOĞRU reddedilir (DoS-guard).
    #[test]
    fn future_gap_rejects_far_first(far in 8193u64..1_000_000) {
        let w = ReplayWindow::new(EXTENDED_PN_PARAMS);
        prop_assert!(w.precheck(far).is_err(), "future_gap ötesi kabul edilmemeli: {far}");
    }

    // INVARIANT-3: reconstruct her zaman low_bits'i korur (cand & mask == low_bits).
    #[test]
    fn reconstruct_preserves_low_bits(expected in 0u64..(1u64<<40), low in 0u64..(1u64<<32)) {
        let p = &EXTENDED_PN_PARAMS;
        let cand = reconstruct(p, expected, low);
        let mask = (1u64 << p.low_width_bits) - 1;
        prop_assert_eq!(cand & mask, low);
    }

    // INVARIANT-4: reconstruct sonucu expected'e en-yakın-aday (|cand-expected| minimum, span/2 içinde
    // ya da underflow-guard tarafından sınırlı).
    #[test]
    fn reconstruct_closest_candidate(expected in (1u64<<33)..(1u64<<40), low in 0u64..(1u64<<32)) {
        let p = &EXTENDED_PN_PARAMS;
        let span = 1u64 << p.low_width_bits;
        let cand = reconstruct(p, expected, low);
        // cand ve komşu adaylar arasında cand en-yakın (expected yeterince büyük → underflow yok).
        let dist = expected.abs_diff(cand);
        for delta in [cand.wrapping_sub(span), cand.wrapping_add(span)] {
            if delta & (span - 1) == low {
                prop_assert!(dist <= expected.abs_diff(delta), "cand daha-yakın-değil");
            }
        }
    }

    // INVARIANT-5 (legacy-seti): duplicate hiç kabul-edilmez, reorder-mesafe-sınırı içinde geç-paket kabul.
    #[test]
    fn legacy_no_double_accept(seq in prop::collection::vec(0u64..5000, 1..300)) {
        let mut w = ReplayWindow::new(LEGACY_PN_PARAMS);
        let mut accepted: HashSet<u64> = HashSet::new();
        for pn in seq {
            if try_accept(&mut w, pn) {
                prop_assert!(accepted.insert(pn), "legacy pn={pn} iki kez kabul");
            }
        }
    }
}

// Deterministik kenar-durum: params-tutarlılık (değerler struct'tan okunur — clippy-const-assert-değil).
#[test]
fn params_constants_sane() {
    fn check(p: &PnParams) {
        assert!(p.window.is_power_of_two());
        assert!(
            p.future_gap_max < (1u64 << (p.low_width_bits - 1)),
            "future-gap < span/2 tek-anlamlı"
        );
        // ccr-§35.8 <2^15 invariant (legacy'de max_reorder var; extended'de yok).
        if let Some(mr) = p.max_reorder {
            assert!(mr < (1u64 << 15), "reorder < 2^15 (seq16 wrap-ambiguity)");
            assert!(p.future_gap_max < (1u64 << 15));
        }
    }
    check(&EXTENDED_PN_PARAMS);
    check(&LEGACY_PN_PARAMS);
}
