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

/// H-HANDLE (tip yarısı; KM-SEAM-TC-001 §6, K3): `HubHandle`, `SessionHandle`'ın beklendiği yere
/// KONAMAZ — kod derlenmez. İkisi de bir tamsayıyı saran tutamaçtır; karışırlarsa wire routing
/// değeri oturum bağlamı olarak (ya da tersi) kullanılırdı.
///
/// Bu test rustc'nin hata metnine dayanır. Toolchain'den bağımsız karşılığı:
/// `kama-contracts` birim testi `s3::tests::hub_handle_does_not_convert_to_session_handle`.
#[test]
fn hub_handle_cannot_stand_in_for_session_handle() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/hub_handle_is_not_session_handle.rs");
}
