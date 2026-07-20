//! K-WIRE-SEALED-TYPE-1: OpenedTransportShard dış-crate'ten ÜRETİLEMEZ (compile-enforce).
//!
//! **YALNIZ production-görünümünde (test-util KAPALI) koşulur.** Sealed-tip garantisi production-build
//! içindir; `test-util` feature `test_new`-kaçışını KASITEN açar (entegrasyon-testleri için) → o modda
//! forge-etmek tasarımca-serbest. `--all-features` bu dosyayı boş-derler (rustc-help-metni test_new'i-de
//! önerdiği-için stderr-eşleşmesi feature-bağımlı olurdu; invariant her-iki-modda-da geçerli).

#![cfg(not(feature = "test-util"))]

#[test]
fn sealed_type_cannot_be_forged() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/forge_opened.rs");
    t.compile_fail("tests/ui/forge_proof.rs");
}
