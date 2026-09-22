//! Sözleşme süiti — **tek yerde yazılır, her hedefte ayrı koşar**.
//!
//! Doktrin (Google SWE ch.13 "contract tests" / Fowler "ContractTest"): sahtenin doğruluğu,
//! gerçek implementasyonla AYNI test kodunun ikisine de koşturulmasıyla kanıtlanır. Burada test
//! gövdeleri generic `fn`'lerdir; [`seam_mechanism_suite!`] / [`seam_contract_suite!`] makroları
//! onları hedef başına ayrı `#[test]` olarak sarar. Böylece:
//!
//! * gövde TEK kopyadır (sahte ile gerçek ayrışamaz — drift imkânsız),
//! * `cargo test` çıktısında her hedef AYRI isimle görünür (`mock::m3_…`, `real::m3_…`),
//!   yani "hangi hedef koştu" çıktıdan okunur; çalışma-zamanı `if target == …` dallanması
//!   bilerek REDDEDİLDİ (tek test adı iki davranış gizler).
//!
//! # İki katman, karıştırılmaz
//!
//! * **mekanizma** ([`mechanism`]) — harness'in kendisinin çalıştığını kanıtlar (hedef kaydı,
//!   typed kurulum, taze hedefte oturum icat edilmemesi, kapasite tutarlılığı). Bunlar KM-SEAM-
//!   AT-001 §6 satırları DEĞİLDİR ve öyleymiş gibi sayılmazlar.
//!   Not: §6'nın 11 satırı ortak koşum yüzeyinde (conformance-core) yaşar; `H-NO-CRYPTO-EDGE` ve
//!   `H-NO-PLANE1` spec'te STATİK kontrol olarak tanımlıdır (derleme bağımlılığı denetimi) —
//!   sırası geldiğinde koşum değil, kapı-seviyesi denetim olarak uygulanmaları gerekir.
//! * **sözleşme** ([`seam_contract_suite!`]) — §6'nın 11 satırı. **Bugün sıfırı uygulanmıştır.**
//!   Bu boşluk `scripts/ci/seam-coverage.json` içinde satır satır beyan edilir; runner, Rust
//!   tarafının bildirdiği listeyi (aşağıdaki sentinel) o beyanla ÇAPRAZ KONTROL eder. Beyanla
//!   gerçeklik ayrışırsa kapı kırmızı — "boş süit sessizce yeşil" hâli mümkün değildir.

#![allow(dead_code)]

/// Rust tarafının GERÇEKLİĞİ: §6'dan uygulanmış satırların kimlikleri.
///
/// Burası tek doğruluk kaynağı DEĞİLDİR; §6 satır listesinin ve durumlarının otoritesi
/// `scripts/ci/seam-coverage.json`'dır (o da spec §6 tablosuna karşı `seam-api-drift.py` ile
/// denetlenir). Bu sabit yalnız "kodda fiilen ne koşuyor" der; runner ikisini karşılaştırır.
/// Yeni bir §6 satırı uygulandığında AYNI commit'te buraya ve JSON'a eklenir.
pub const IMPLEMENTED_CONTRACT_ROWS: &[&str] = &[];

/// Runner'ın okuduğu makine-satırı. `--nocapture` ile basılır.
pub const CONTRACT_ROWS_SENTINEL: &str = "SEAM-CONTRACT-ROWS";

/// Hedefe bakmaksızın sabit olan beyan tutarlılığı + sentinel emisyonu.
///
/// # Panics
/// Beyan listesi kendi içinde tutarsızsa (boş kimlik, tekrar eden kimlik).
pub fn emit_contract_row_sentinel(target_id: &str) {
    let rows = IMPLEMENTED_CONTRACT_ROWS;
    for (i, id) in rows.iter().enumerate() {
        assert!(!id.is_empty(), "boş §6 satır kimliği (index {i})");
        assert!(
            !rows[..i].contains(id),
            "§6 satır kimliği iki kez beyan edildi: {id}"
        );
    }
    println!(
        "{CONTRACT_ROWS_SENTINEL}: target={target_id} rows={}",
        rows.join(",")
    );
}

/// Mekanizma gövdeleri — her hedefte AYNI kod koşar.
pub mod mechanism {
    use super::super::target::{SeamTarget, TargetUnavailable, REGISTERED_TARGETS};
    use crate::harness::api::{KamaTransport, SessionHandle};

    /// Hedef kaydı ile hedefin kendi beyanı tutarlı mı.
    ///
    /// # Panics
    /// Hedef `REGISTERED_TARGETS` içinde yoksa ya da ailesi (`KIND`) kayıtla uyuşmuyorsa.
    pub fn target_registered<T: SeamTarget>() {
        let hit = REGISTERED_TARGETS.iter().find(|(id, _)| *id == T::ID);
        let (_, kind) = hit.unwrap_or_else(|| {
            panic!(
                "hedef '{}' REGISTERED_TARGETS içinde kayıtlı değil — süit kayıtsız bir hedefte koşuyor",
                T::ID
            )
        });
        assert_eq!(
            *kind,
            T::KIND,
            "hedef '{}' kayıtta {:?}, kendi beyanında {:?}",
            T::ID,
            kind,
            T::KIND
        );
    }

    /// Kurulum typed'dır: ya `Ok`, ya boşluk kimliği taşıyan typed bir red. Panik/`todo!()` yok.
    ///
    /// # Panics
    /// Red boş bir boşluk kimliği taşıyorsa (beyansız boşluk = sessiz atlama).
    pub fn build_is_typed<T: SeamTarget>() {
        match T::build() {
            Ok(_) => {}
            Err(TargetUnavailable::NotImplemented { gap }) => {
                assert!(
                    !gap.is_empty(),
                    "hedef '{}' kimliksiz bir boşlukla reddetti — beyan edilmemiş boşluk",
                    T::ID
                );
            }
        }
    }

    /// Taze hedef oturum İCAT ETMEZ: `recv()`/`poll_event()` boş ve gözlem yan etkisiz.
    ///
    /// Mekanizma seviyesidir; §6 `H-EVENTS` satırının yerine GEÇMEZ (o, `Ready → RenewRequired →
    /// Ready(h') → Closed(h, Renewed)` sırasını ister).
    ///
    /// # Panics
    /// Hiç `connect` çağrılmadan teslim ya da olay üretilirse.
    pub fn fresh_target_has_no_sessions<T: SeamTarget>() {
        let Ok(mut t) = T::build() else {
            return; // boşluk beyanı build_is_typed'ın işi; burada sessizce geçilir
        };
        assert!(
            t.recv().is_none(),
            "hedef '{}' connect edilmeden teslim üretti",
            T::ID
        );
        assert!(
            t.poll_event().is_none(),
            "hedef '{}' connect edilmeden olay üretti",
            T::ID
        );
        // Gözlem yan etkisiz: ikinci çağrı da boş.
        assert!(t.recv().is_none(), "ikinci recv() farklı davrandı");
        assert!(
            t.poll_event().is_none(),
            "ikinci poll_event() farklı davrandı"
        );
        // §1: `close` hata kanalı taşımaz; bilinmeyen handle no-op olmalı, panik değil.
        t.close(SessionHandle::from_raw(u64::MAX));
    }

    /// `capabilities()` oturum GEREKTİRMEZ (K5: global, session öncesi) ve kararlıdır.
    ///
    /// `preferred ≤ max` kontrolü spec'te yazılı DEĞİLDİR; türetilmiştir: app record'larını
    /// `preferred`'a göre boyutlandırırsa (K5 S) ve `preferred > max` olsaydı ipucu doğrudan
    /// `UnitTooLarge`'a (K3 S) götürürdü.
    ///
    /// # Panics
    /// İki çağrı farklı değer döndürürse ya da `preferred > max` ise.
    pub fn capabilities_queryable_without_session<T: SeamTarget>() {
        let Ok(t) = T::build() else {
            return;
        };
        let a = t.capabilities();
        let b = t.capabilities();
        assert_eq!(a, b, "hedef '{}' kapasiteyi kararsız döndürdü", T::ID);
        assert!(
            a.preferred_plaintext_unit_bytes <= a.max_plaintext_unit_bytes,
            "hedef '{}': preferred({}) > max({}) — ipucu kullanılamaz",
            T::ID,
            a.preferred_plaintext_unit_bytes,
            a.max_plaintext_unit_bytes
        );
    }
}

/// Mekanizma süitini bir hedef için örnekler.
///
/// Çağıranın kök modülünde `mod harness;` bulunmalıdır (repo idiomu: `#[path]` ile dahil edilir).
/// Hedef yolu ÇAĞRI YERİNDEN geçer, örn. `harness::target::MockTarget`.
#[macro_export]
macro_rules! seam_mechanism_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            use super::*;
            use $crate::harness::contract::mechanism;

            type Target = $target;

            #[test]
            fn m1_target_registered() {
                mechanism::target_registered::<Target>();
            }

            #[test]
            fn m2_build_is_typed() {
                mechanism::build_is_typed::<Target>();
            }

            #[test]
            fn m3_fresh_target_has_no_sessions() {
                mechanism::fresh_target_has_no_sessions::<Target>();
            }

            #[test]
            fn m4_capabilities_queryable_without_session() {
                mechanism::capabilities_queryable_without_session::<Target>();
            }
        }
    };
}

/// Sözleşme süitini (KM-SEAM-AT-001 §6) bir hedef için örnekler.
///
/// **Bugün 0/11 satır uygulanmıştır.** Makro, uygulanan satırları hedef başına saracak yerdir;
/// şimdilik yalnız beyan-sentinel'ini üretir. Yeni satır eklerken: gövde `contract.rs` içine
/// generic `fn` olarak yazılır, buraya `#[test]` sarmalayıcısı eklenir, kimliği
/// `IMPLEMENTED_CONTRACT_ROWS` ve `scripts/ci/seam-coverage.json`'a AYNI commit'te işlenir.
#[macro_export]
macro_rules! seam_contract_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            use super::*;
            use $crate::harness::contract;
            use $crate::harness::target::SeamTarget;

            type Target = $target;

            // ── §6 satırları buraya ── (bugün: hiçbiri; boşluk seam-coverage.json'da beyanlı)

            #[test]
            fn c0_declared_contract_rows_are_consistent() {
                contract::emit_contract_row_sentinel(<Target as SeamTarget>::ID);
            }
        }
    };
}
