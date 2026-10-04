//! Kama sözleşme crate'i — katmanlar arası iki seam'in derlenen hâli.
//!
//! ```text
//! Application ──S1──▶ Transport ──S3──▶ Crypto ──▶ UDP        (INV-NEIGHBOR-1)
//! ```
//!
//! * [`s1`] — Application ↔ Transport (`docs/seams/KM-SEAM-AT-001.md` §1 + §3).
//! * [`s3`] — Transport ↔ Crypto (`docs/seams/KM-SEAM-TC-001.md` §1.1 + §1.2).
//! * [`handles`] — iki seam'in ORTAK tuttuğu tipler ([`handles::SessionHandle`],
//!   [`handles::PeerRef`], [`handles::CloseReason`]).
//!
//! # Bu crate neden var
//!
//! Sözleşme hiçbir katmana ait değildir. Trait bir katmanın crate'inde dururken komşusu onu
//! implemente etmek için o crate'e bağımlı olmak zorundadır; bu S3'te bağımlılık döngüsü, S1'de
//! alt katmanın üst katmana bağlanması demekti. Burada duran sözleşmeye üç katman da (`kama-app`,
//! `client-core`, `kama-noise-wire`) aşağıdan yukarı değil YANDAN bağlanır; birbirlerini görmeleri
//! gerekmez.
//!
//! # Kurallar (`scripts/ci/layer_check.py` kapıda denetler)
//!
//! * Bu crate'in bağımlılığı YOKTUR: ne workspace crate'i ne harici crate.
//! * İki seam'de aynı adı taşıyan ama farklı olan tipler AYRI modüllerdedir
//!   ([`s1::SessionEvent`] ≠ [`s3::SessionEvent`]); glob import'la ikisini birden almayın.
//! * `test-util` özelliği yalnız testler içindir ve `kama-app` onu açamaz.
//!
//! # Spec ile ilişki
//!
//! `scripts/ci/seam-api-drift.py` S1 ve S3 belgelerindeki trait bloklarını bu crate'in kaynağıyla
//! karşılaştırır; ikisi ayrışırsa CI kırmızıdır. Spec'ten tek bilinçli fark: spec `Bytes` yazar,
//! burada `Vec<u8>` kullanılır (`bytes` crate'i bağımlılık yapılmadı; sözleşme byte-özdeşlik ister,
//! sahiplik modelini değil).

#![forbid(unsafe_code)]

pub mod handles;
pub mod s1;
pub mod s3;
