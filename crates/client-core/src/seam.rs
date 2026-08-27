//! P1 güvenlik dikişi (SecuritySeam) — INV-ENGINE-SOC'un çalışma-zamanı sınırı.
//!
//! Transport çalışma-zamanı datagram'ı OPAK taşır: dışarı çıkan her datagram `seal`'den, içeri
//! giren her datagram `open`'dan geçer. Kripto BU trait'in ARKASINDADIR — bugün [`IdentitySeam`]
//! (stub, bench TAG_STUB soyu: +16 B byte-eşdeğer sahte tag), yarın güvenlik düzlemi (workstream B,
//! Sprint-6) gerçek Noise/AEAD implementasyonunu BU dikiş-NOKTASINA takar. DÜRÜST NOT (freeze-audit
//! 2026-08-26): gerçek kama-noise-wire için trait İMZASI v1'e genişleyecek (fallible seal,
//! SealArgs/SealedPacketMeta, per-peer seçim, per-packet sizer — P4 belgesi §3); SINIR sabittir,
//! imza değil. `open` başarısızlığı sessiz düşürmedir (verify-before-admission: doğrulanamayan
//! datagram transport durumuna DOKUNMADAN çöpe gider — INV-FEC-1'in alıcı yarısıyla aynı kural).
//!
//! GÜVENLİK İDDİASI YOK: `IdentitySeam` kimlik/ gizlilik/bütünlük SAĞLAMAZ; yalnız byte-eşdeğer
//! maliyet modeli taşır (kampanya kolu W6/C ile aynı gerekçe). "Noise handshake verified" sınıfı
//! iddialar PRODUCT-RUNTIME-INTEGRATION-PENDING yasak listesinde ve bu stub'la YAPILAMAZ.

/// Datagram-düzeyi güvenlik dikişi. Implementasyonlar `Send + Sync` (tek görev kullanır ama
/// handle'lar arası taşınabilir).
pub trait SecuritySeam: Send + Sync + 'static {
    /// Düz çerçeveyi mühürler (şifrele+doğrulama-etiketi). Dönen byte'lar wire'a gider.
    fn seal(&self, plain: Vec<u8>) -> Vec<u8>;
    /// Wire datagram'ını açar/doğrular. `None` = doğrulama başarısız → çağıran SESSİZCE düşürür
    /// (durum değişikliği yok).
    fn open(&self, wire: &[u8]) -> Option<Vec<u8>>;
    /// Mühür ek yükü (byte) — pacing/bütçe muhasebesi datagram'ın GERÇEK wire boyunu kullansın diye.
    fn overhead(&self) -> usize;
}

/// AEAD-tag SİMÜLASYONU: her datagram'a +16 B sıfır-tag ekler/soyar. Kampanyanın byte-eşdeğerlik
/// modeliyle aynı (bench `TAG_STUB`); gerçek katman geldiğinde byte ölçüleri değişmez.
pub struct IdentitySeam;

/// Sahte tag boyu (ChaCha20-Poly1305 tag'iyle byte-eşdeğer).
pub const IDENTITY_TAG: usize = 16;

impl SecuritySeam for IdentitySeam {
    fn seal(&self, mut plain: Vec<u8>) -> Vec<u8> {
        plain.resize(plain.len() + IDENTITY_TAG, 0);
        plain
    }

    fn open(&self, wire: &[u8]) -> Option<Vec<u8>> {
        // Stub "doğrulaması": en az tag kadar uzun olmalı; tag soyulur. Gerçek implementasyon
        // burada AEAD verify yapar ve başarısızlıkta None döner.
        if wire.len() <= IDENTITY_TAG {
            return None;
        }
        Some(wire[..wire.len() - IDENTITY_TAG].to_vec())
    }

    fn overhead(&self) -> usize {
        IDENTITY_TAG
    }
}
