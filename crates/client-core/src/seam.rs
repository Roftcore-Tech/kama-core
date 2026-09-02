//! Güvenlik dikişi v1 (SecuritySeam) — INV-ENGINE-SOC'un çalışma-zamanı sınırı.
//!
//! Transport çalışma-zamanı datagram'ı OPAK taşır: dışarı çıkan her datagram `seal`'den, içeri
//! giren her datagram `open`'dan geçer. Kripto BU trait'in ARKASINDADIR — bugün [`IdentitySeam`]
//! (stub, +16 B byte-eşdeğer sahte tag), S2'de kama-noise-wire aynı dikiş-noktasına takılır.
//!
//! **v1 (2026-08-26, freeze-audit sonucu):** imza gerçek Noise katmanının (kama-noise-wire)
//! sözleşmesine hizalandı — v0'ın `seal(&self,Vec<u8>)->Vec<u8>` şekli yapısal-yetersizdi:
//! fallible-seal yok, `SealArgs{wire_pn,dh_epoch,send_generation}`/meta akışı yok (§0.5
//! send-record), per-peer anahtar-seçimi yok (server tek dikişi tüm peer'lara paylaşır),
//! per-packet boyut yok. v1: `seal(&mut self, peer, frame, SealArgs) -> Result<SealedDatagram,
//! SealError>` · `open(&mut self, datagram) -> Result<OpenedDatagram, OpenReject>` (self-demux;
//! typed-red) · `sealed_len(frame_len)`. PN sayacı ÇAĞIRANDA (noise-wire caller-supplied
//! sözleşmesi; protector monotonluğu zorlar). SINIR sabittir; S2 yalnız implementasyonu değiştirir.
//!
//! GÜVENLİK İDDİASI YOK: [`IdentitySeam`] kimlik/gizlilik/bütünlük SAĞLAMAZ; yalnız byte-eşdeğer
//! maliyet modeli taşır. `open` reddi sessiz-düşürmedir (verify-before-admission: doğrulanamayan
//! datagram transport durumuna DOKUNMADAN çöpe gider — INV-FEC-1'in alıcı yarısıyla aynı kural).

/// Seam-düzeyi peer kimliği (opak; runtime atar/eşler — noise-wire'da h_ext'e karşılık gelir).
pub type SeamPeer = u64;

/// Seal-argümanları (noise-wire `SealArgs` alan-hizası; §0.5 send-record akışının girişi).
#[derive(Debug, Clone, Copy, Default)]
pub struct SealArgs {
    /// Wire packet-number (per-peer monoton; çağıran sayar, seam ihlali reddeder).
    pub wire_pn: u64,
    /// DH-epoch (0 = e0; rekey S2).
    pub dh_epoch: u8,
    /// Retransmit-jenerasyonu (§0.5).
    pub send_generation: u32,
}

/// Mühürlü datagram + meta (noise-wire `SealedPacket{bytes, meta}` hizası).
#[derive(Debug)]
pub struct SealedDatagram {
    /// Wire'a gidecek byte'lar (ek çerçeveleme YOK).
    pub bytes: Vec<u8>,
    /// Mühürde kullanılan tam PN (motor send-record tamamlaması için).
    pub full_wire_pn: u64,
    /// Mühürde kullanılan epoch.
    pub dh_epoch: u8,
}

/// Açılmış datagram: seam'in kimliklendirdiği peer + düz çerçeve.
#[derive(Debug)]
pub struct OpenedDatagram {
    /// Seam-peer (server çok-oturumda demux; IdentitySeam daima 0).
    pub peer: SeamPeer,
    /// Doğrulanmış düz çerçeve byte'ları.
    pub plain: Vec<u8>,
}

/// Seal hataları (typed; v0'ın infallible-seal'i gerçek katmanda temsil-edilemezdi).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SealError {
    /// Oturum/anahtar hazır değil (handshake tamamlanmadı / kapandı).
    #[error("seam not ready")]
    NotReady,
    /// PN tavanı/monotonluk ihlali (nonce-reuse koruması).
    #[error("pn exhausted or non-monotonic")]
    PnExhausted,
    /// Epoch anahtarı kurulu değil.
    #[error("epoch keys missing")]
    EpochKeysMissing,
    /// İmplementasyon-içi diğer red (typed mesajlı).
    #[error("seal: {0}")]
    Internal(&'static str),
}

/// Open reddi (typed; sessiz-düşür + sayaç — durum değişikliği YOK).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenReject {
    /// AEAD/doğrulama başarısız.
    AuthFail,
    /// Replay penceresi reddi.
    Replay,
    /// Bilinmeyen peer/handle.
    UnknownPeer,
    /// Epoch kabul-dışı.
    EpochNotAccepted,
    /// Çerçeve/parse bozuk.
    Malformed,
}

/// Datagram-düzeyi güvenlik dikişi v1. Endpoint başına TEK sahip (`Box<dyn SecuritySeam>`;
/// `&mut` — gerçek katman PN/replay/oturum durumu tutar).
pub trait SecuritySeam: Send + 'static {
    /// Düz çerçeveyi `peer` için mühürler. Dönen byte'lar wire'a gider.
    ///
    /// # Errors
    /// Typed seal-reddi ([`SealError`]) — çağıran sayar, emisyonu kayıp-sayar (INV-RATE-1).
    fn seal(
        &mut self,
        peer: SeamPeer,
        frame: Vec<u8>,
        args: SealArgs,
    ) -> Result<SealedDatagram, SealError>;
    /// Wire datagram'ını açar/doğrular + peer'ı kimliklendirir (self-demux).
    ///
    /// # Errors
    /// Typed open-reddi ([`OpenReject`]) — çağıran SESSİZCE düşürür (durum değişikliği yok).
    fn open(&mut self, datagram: &[u8]) -> Result<OpenedDatagram, OpenReject>;
    /// `frame_len` byte'lık düz çerçevenin mühürlü wire-boyu (pacing gerçek-boy muhasebesi;
    /// noise-wire `sealed_len` NORMATİF sözleşmesi: `seal(p).bytes.len() == sealed_len(...)`).
    fn sealed_len(&self, frame_len: usize) -> usize;
}

/// AEAD-tag SİMÜLASYONU: her datagram'a +16 B sıfır-tag ekler/soyar (byte-eşdeğer maliyet modeli;
/// kampanya TAG_STUB soyu). Peer-ayrımı YOK (daima 0) — güvenlik iddiası olmayan stub.
pub struct IdentitySeam;

/// Sahte tag boyu (ChaCha20-Poly1305 tag'iyle byte-eşdeğer).
pub const IDENTITY_TAG: usize = 16;

impl SecuritySeam for IdentitySeam {
    fn seal(
        &mut self,
        _peer: SeamPeer,
        mut frame: Vec<u8>,
        args: SealArgs,
    ) -> Result<SealedDatagram, SealError> {
        frame.resize(frame.len() + IDENTITY_TAG, 0);
        Ok(SealedDatagram {
            bytes: frame,
            full_wire_pn: args.wire_pn,
            dh_epoch: args.dh_epoch,
        })
    }

    fn open(&mut self, datagram: &[u8]) -> Result<OpenedDatagram, OpenReject> {
        if datagram.len() <= IDENTITY_TAG {
            return Err(OpenReject::Malformed);
        }
        Ok(OpenedDatagram {
            peer: 0,
            plain: datagram[..datagram.len() - IDENTITY_TAG].to_vec(),
        })
    }

    fn sealed_len(&self, frame_len: usize) -> usize {
        frame_len + IDENTITY_TAG
    }
}
