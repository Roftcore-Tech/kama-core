//! `kama-provision` — PoC provisioning biçimi (PROVISIONING-FORMAT-v0) ve onu üreten CLI'ın kütüphane
//! yarısı (K4G-12; plan D4, D9, D11).
//!
//! Biçim otoritesi `docs/provisioning/PROVISIONING-FORMAT-v0.md`'dir. Modüller:
//!
//! * [`format`] — `root.json` / `device-<ad>.json` tipleri (serde), doğrulayan yükleyici
//!   ([`format::load_device_config`]) ve `kama-noise-wire` ürün kurulumuna eşleme
//!   ([`format::DeviceConfig::local_identity`], [`format::DeviceConfig::peer_directory`]). K4G-11
//!   bileşim kökü cihaz dosyasını bu tiplerle okur.
//! * [`cert`] — CompactDeviceCertV1 kodlayıcısı (ccr §13.1 / §13.2) ve kimlik türetmeleri.
//! * [`encoding`] — std-only hex ve base64 (D11: harici kodlama crate'i yok).
//! * [`provision`] — `init`: anahtar üretimi, öz-denetim, dosya yazımı (`.gitignore`, `0600`).
//! * [`cli`] — `std::env::args_os` ayrıştırıcısı ve `kama-provision` giriş noktası.
//!
//! **Gizli anahtar hijyeni:** gizli anahtar taşıyan tek biçim tipi [`format::Secret32`]'dir — Debug'ta
//! maskeli, `Clone` değil, Drop'ta sıfırlanır. Cihaz yapılandırmasında ([`format::DeviceConfig`]) kök
//! gizli anahtarı için alan YOKTUR (D9). CLI standart çıktıya yalnız dosya yollarını basar.

#![forbid(unsafe_code)]

pub mod cert;
pub mod cli;
pub mod encoding;
pub mod format;
pub mod provision;
