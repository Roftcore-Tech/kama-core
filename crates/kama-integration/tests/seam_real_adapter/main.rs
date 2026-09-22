//! **real-adapter** — yalnız gerçek implementasyonda anlamı olan testler.
//!
//! Socket reset, PMTU kara deliği, gerçek kayıp altında kurtarma: bunlar sahtede anlamsızdır
//! (sahtede socket yoktur), o yüzden conformance-core'un dışında dururlar. Ayrı ikilide olmaları
//! sahteyi "gerçek gibi" davranmaya zorlayan sahte-sadakat baskısını da ortadan kaldırır.
//!
//! # Bugün: BEYAN EDİLMİŞ BOŞLUK
//!
//! Gerçek adapter yok. Bu ikili boş değildir ve "0 test koştu" vakumuna düşmez: boşluğun
//! BEYAN EDİLDİĞİNİ sınar ve runner'ın okuduğu sentinel satırını basar. Gerçek hedef kaydedildiği
//! an [`real_target_is_declared_absent`] KIRMIZI olur — kapsam sessizce değişemez, birileri bu
//! dosyayı açıp gerçek testleri yazmak zorunda kalır.

#[path = "../common/seam/harness.rs"]
mod harness;

use harness::target::{TargetKind, GAP_REAL_ADAPTER, REGISTERED_TARGETS};

/// Runner'ın okuduğu makine-satırı (çıkış kodu 3 = beyan edilmiş boşluk).
const REAL_TARGET_SENTINEL: &str = "SEAM-TARGET-REAL";

/// Gerçek adapter indiğinde buraya yazılacak, sahtede ANLAMSIZ olan vakalar.
///
/// Liste beyandır, iddia değildir: boş olması "bu vakalar geçti" demek değil, "bu vakalar henüz
/// yok" demektir. `scripts/ci/seam-coverage.json` aynı boşluğu `GAP-SEAM-REAL-1` altında sayar.
const REAL_ONLY_CASES_PLANNED: &[&str] = &[
    "socket reset (ECONNRESET) — oturum durumu ne olur",
    "PMTU kara deliği — segment bütçesi daralınca capabilities ne der",
    "gerçek kayıp altında H-APP-RETRY-0 (recovery ON/OFF farkı)",
];

/// Gerçek hedef kayıtlı DEĞİL ve bu boşluk kimlikli olarak beyan edilmiştir.
#[test]
fn real_target_is_declared_absent() {
    let real: Vec<&str> = REGISTERED_TARGETS
        .iter()
        .filter(|(_, kind)| *kind == TargetKind::Real)
        .map(|(id, _)| *id)
        .collect();

    assert!(
        real.is_empty(),
        "gerçek hedef kaydedilmiş ({real:?}) ama bu ikili hâlâ boşluk beyan ediyor — \
         real-adapter testlerini yaz ve bu sınamayı kaldır (gap {GAP_REAL_ADAPTER})"
    );

    println!("{REAL_TARGET_SENTINEL}: ABSENT gap={GAP_REAL_ADAPTER}");
}

/// Planlanan gerçek-hedef vakaları kimlikli ve tekrarsız beyan edilir.
#[test]
fn planned_real_only_cases_are_declared() {
    assert!(
        !REAL_ONLY_CASES_PLANNED.is_empty(),
        "gerçek hedefe özgü tek bir vaka bile planlanmamışsa bu kategori niye var?"
    );
    for (i, case) in REAL_ONLY_CASES_PLANNED.iter().enumerate() {
        assert!(!case.trim().is_empty(), "boş vaka beyanı (index {i})");
        assert!(
            !REAL_ONLY_CASES_PLANNED[..i].contains(case),
            "vaka iki kez beyan edildi: {case}"
        );
    }
}
