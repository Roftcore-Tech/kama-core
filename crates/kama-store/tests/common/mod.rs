//! `kama-store` testlerinin ortak yardımcıları: geçici dizin (`tempfile` crate'i YOK —
//! `std::env::temp_dir()` altında benzersiz alt dizin), sentetik eşler ve outbox kayıtları.

// Bu ortak modül birden fazla test ikilisi tarafından ayrı ayrı derlenir; her ikili yardımcıların
// hepsini kullanmaz.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use kama_app::messaging::MsgId;
use kama_app::store::OutboxUnit;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{CompressionPolicy, PreparedUnit};

/// Testlerin depo dosyası adı (ürün yolunu bileşim kökü seçer).
pub const DB_FILE: &str = "kama-store.redb";

/// Benzersiz, düşünce silinen geçici dizin.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// `temp_dir()/kama-store-<etiket>-<pid>-<sıra>-<ns>`. `create_dir` var olan dizinde hata
    /// verir: başka bir koşunun ya da testin dizini asla paylaşılmaz.
    pub fn new(tag: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "kama-store-{tag}-{}-{seq}-{nanos}",
            std::process::id()
        ));
        if let Err(error) = std::fs::create_dir(&path) {
            panic!("geçici dizin {} yaratılamadı: {error}", path.display());
        }
        Self { path }
    }

    /// Dizinin yolu.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Dizindeki depo dosyasının yolu.
    pub fn db(&self) -> PathBuf {
        self.path.join(DB_FILE)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Drop'ta panik YOK: temizlik en iyi çabadır.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Sentetik eş: `dest_hash = [tag; 32]`.
pub fn peer(tag: u8) -> PeerRef {
    PeerRef::from_dest_hash([tag; 32])
}

/// `len` baytlık, `msg_id`'den türetilmiş deterministik kayıtla outbox kaydı.
pub fn outbox_unit(msg_id: MsgId, len: usize, compression_policy: CompressionPolicy) -> OutboxUnit {
    let seed = msg_id.to_be_bytes()[3];
    let bytes = (0..len)
        .map(|i| seed.wrapping_add((i % 251) as u8))
        .collect();
    OutboxUnit::new(
        msg_id,
        PreparedUnit {
            bytes,
            compression_policy,
        },
    )
    .expect("kayıt tavanın altında")
}
