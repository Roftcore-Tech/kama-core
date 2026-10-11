//! `kama-provision` — PoC provisioning CLI (K4G-12; PROVISIONING-FORMAT-v0). Ayrıştırma ve çıktı
//! kuralları `kama_provision::cli`'dadır.

#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    kama_provision::cli::run(std::env::args_os())
}
