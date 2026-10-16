//! Kotlin bağlayıcı üreticisi (K4G-15 NE 3) — `uniffi`'nin `cli` özelliği (clap; D11'de onaysız)
//! yerine `uniffi_bindgen` kütüphane API'si. Yalnız `bindgen` özelliğiyle derlenir:
//!
//! ```text
//! cargo run -p kama-mobile --features bindgen --bin uniffi-bindgen -- <libkama_mobile.so> <çıktı>
//! ```
//!
//! Kütüphane kipi: arayüz derlenmiş kütüphanenin UniFFI meta-veri sembollerinden okunur
//! (proc-macro yüzeyi; UDL yok) — Android `.so`'su da olur. `cargo metadata` ÇAĞRILMAZ
//! (`uniffi`'nin `cargo-metadata` özelliği kapalı); yapılandırma varsayılandır: paket
//! `uniffi.kama_mobile`, yüklenen kütüphane `kama_mobile`. Biçimlendirici (ktlint) çağrılmaz.
//! Çıktı: `<çıktı>/uniffi/kama_mobile/kama_mobile.kt`.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::process::ExitCode;

/// Bağlayıcıları üretilen crate (meta-veri süzgeci: başka crate'in meta-verisi bağlayıcıya girmez).
const CRATE_NAME: &str = "kama_mobile";
/// Kullanım hatası çıkış kodu.
const EXIT_USAGE: u8 = 2;
/// Üretim hatası çıkış kodu.
const EXIT_FAILURE: u8 = 1;
/// Kullanım metni.
const USAGE: &str = "kullanım: uniffi-bindgen <libkama_mobile.so> <çıktı dizini>";

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let [library, out_dir] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    };
    let (Some(library), Some(out_dir)) = (library.to_str(), out_dir.to_str()) else {
        eprintln!("uniffi-bindgen: yollar UTF-8 olmalı");
        return ExitCode::from(EXIT_USAGE);
    };
    let options = uniffi::GenerateOptions {
        languages: vec![uniffi::TargetLanguage::Kotlin],
        source: library.to_owned().into(),
        out_dir: out_dir.to_owned().into(),
        format: false,
        crate_filter: Some(CRATE_NAME.to_owned()),
        ..Default::default()
    };
    match uniffi::generate(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("uniffi-bindgen: {error:#}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}
