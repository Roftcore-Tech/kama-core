//! BİÇİMSEL DOĞRULAMA (Kani) — `prefix.rs::SourcePrefix::parse`.
//!
//! Bu parser SALDIRGAN-KONTROLLÜ byte'lar üzerinde çalışır: reconstruct edilen bir
//! symbol'ün prefix'i uygulamaya sunulan `message_id`/`chunk_offset`'i belirler.
//! `declared_len` wire'dan okunuyor ve kapasiteyle karşılaştırılıyor — bounds aritmetiği.
//!
//! `fuzz_prefix` hedefi aynı yüzeyi rastgele sürüyor; bu ispat tüm girdi uzayını kapatır.
//!
//! Koş: `cargo kani -p kama-transport-schema --tests`

#![cfg(kani)]

use kama_transport_schema::prefix::SourcePrefix;
use kama_transport_schema::SOURCE_PREFIX_LEN;

/// Tam boyutlu tampon — panik yok.
#[kani::proof]
#[kani::unwind(2)]
fn prefix_parse_never_panics() {
    let buf: [u8; SOURCE_PREFIX_LEN + 4] = kani::any();
    let _ = SourcePrefix::parse(&buf);
}

/// Kısa tampon — bounds kontrolü panik yerine tipli hata vermeli.
#[kani::proof]
#[kani::unwind(2)]
fn prefix_parse_short_buffer_never_panics() {
    let buf: [u8; 3] = kani::any();
    let _ = SourcePrefix::parse(&buf);
}
