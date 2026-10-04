//! Transport ↔ Crypto dikişi (S3) — `docs/seams/KM-SEAM-TC-001.md` §1.1 + §1.2'nin derlenen hâli.
//!
//! INV-ENGINE-SOC'un çalışma-zamanı sınırı. Transport datagram'ı OPAK taşır: dışarı çıkan her
//! çerçeve [`PacketProtection::seal`]'den, içeri giren her datagram [`PacketProtection::open`]'dan
//! geçer; oturum kurulumu [`SessionControl`]'ün typed komutlarıyla sürülür. Kripto bu iki trait'in
//! ARKASINDADIR — bugün [`IdentitySeam`] (stub, +16 B byte-eşdeğer sahte tag); gerçek implementasyon
//! `kama-noise-wire`'da yazılır (R-7(b)) ve aynı dikiş noktasına takılır.
//!
//! **v2 (2026-10-04, A4.1):** S3 K1–K10 imzaları.
//!  * **K1** — [`ProtectionContext`] çağıranın verdiği TEK bağlamdır; Crypto çerçeve içeriğine
//!    bakarak anahtar seçmez. v1'in `SeamPeer`'ı `Session(SessionHandle)` oldu.
//!  * **K2** — PN sayacını CRYPTO tutar. v1'in `SealArgs{wire_pn, dh_epoch, send_generation}` argümanı
//!    KALKTI (API değişikliği; wire değişikliği DEĞİL). Transport PN üretmez; `seal()` dönüşündeki
//!    [`SealedPacketMeta::full_wire_pn`]'i kendi ARQ/ACK tablosuna kaydeder. Gerekçe: sayaç
//!    transport'tayken her PeerSlot reset'i ve `wrapping_add` aynı anahtar altında nonce tekrarı
//!    riskiydi.
//!  * **K5** — `open()` başarısızlığı SESSİZDİR (`None`) ve state değiştirmez. v1'in typed
//!    `OpenReject`'i kalktı: "neden düştü" bilgisi demux/replay oracle'ıdır.
//!  * **K10 / DR-CONNECT-1 K5** — [`SessionResult::ProvisionalAndReply`] ve
//!    [`SessionEvent::Confirmed`]: mevcut oturumu olan eşten gelen yeni-sayaçlı INIT provisional
//!    kurulur; Transport Application'a `Ready`'yi `Confirmed`'de yayınlar.
//!
//! SPEC'TEN TEK SAPMA: spec `Bytes` yazar, burada `Vec<u8>` (KM-SEAM-AT-001 aynasındaki SEAM-OPEN-1
//! ile aynı kalem — `bytes` crate'ini doğrudan bağımlılık yapma kararı verilmedi).
//!
//! GÜVENLİK İDDİASI YOK: [`IdentitySeam`] kimlik/gizlilik/bütünlük SAĞLAMAZ; yalnız byte-eşdeğer
//! maliyet modeli taşır. `open` reddi sessiz-düşürmedir (verify-before-admission: doğrulanamayan
//! datagram transport durumuna DOKUNMADAN çöpe gider — INV-FEC-1'in alıcı yarısıyla aynı kural).

// ── Kimlikler ────────────────────────────────────────────────────────────────────────────────

/// Crypto'nun bastığı oturum tutamacı (S3 K8; yaşam döngüsü invariantları transport-tdd §0).
///
/// Transport için OPAKTIR: içeriği yorumlanmaz, epoch DEĞİLDİR, wire'da gönderilmez, `Closed`
/// sonrası kullanılamaz. `HubHandle` (wire routing, ccr §35.7) ile AYRI tiptir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionHandle(u64);

impl SessionHandle {
    /// Yalnız Crypto tarafı (ve sahteleri) tutamaç basar.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

/// Opak peer referansı (`StaticPeerMaterialProvider` anahtarı; S1 `PeerRef` ile aynı kavram).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerRef(u64);

impl PeerRef {
    /// Çağıran peer'ı adlandırır; içerik opak kalır.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

// ── §1.1 PacketProtection ────────────────────────────────────────────────────────────────────

/// Çağıranın verdiği TEK koruma bağlamı (S3 K1).
///
/// `Session(h)` → o oturumun E2E cipher state'i ve PN uzayı. `HubChannel` → client↔hub hop anahtarı,
/// AYRI PN uzayı (engine ACK, HANDLE_ASSIGN, Plane 1 RPC, hop heartbeat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProtectionContext {
    Session(SessionHandle),
    HubChannel,
}

/// Tutamaç gerektirmeyen koruma sınıfı ([`PacketProtection::conservative_limits`] için).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtectionClass {
    Session,
    HubChannel,
}

/// Bir bağlamın/sınıfın boyut sınırları. Transport kripto format matematiğini TEKRAR ETMEZ; bu
/// sayıları Crypto'dan okur (S1 K5 zinciri).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionLimits {
    /// Mühürlemenin eklediği wire baytı (başlık + tag).
    pub wire_overhead_bytes: u32,
    /// Tek `seal()`'e verilebilecek azami plaintext.
    pub max_plaintext_bytes: u32,
}

/// Mühürlü paketin YALNIZ Crypto'ya ait meta alanları (noise-wire P0-4). `logical_ref` Transport'un
/// kendi tablosundadır (S3 K4), burada yoktur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedPacketMeta {
    /// Crypto'nun atadığı TAM sayaç (S3 K2). Transport bunu ARQ/ACK tablosuna yazar.
    pub full_wire_pn: u64,
    /// Mühürde/açmada kullanılan DH epoch'u.
    pub dh_epoch: u8,
    /// Wire'daki kesin bayt sayısı.
    pub sealed_len: u32,
}

/// Mühürlü birim: wire'a gidecek byte'lar (ek çerçeveleme YOK) + meta.
#[derive(Debug)]
pub struct SealedUnit {
    pub bytes: Vec<u8>,
    pub meta: SealedPacketMeta,
}

/// Açılmış datagram: Crypto'nun kimliklendirdiği bağlam + transport'un verdiği plaintext çerçeve.
#[derive(Debug)]
pub struct Opened {
    /// Server çok-oturumda demux bununla yapılır (v1'in `OpenedDatagram.peer`'ı).
    pub ctx: ProtectionContext,
    /// Doğrulanmış düz çerçeve byte'ları.
    pub frame: Vec<u8>,
    pub meta: SealedPacketMeta,
}

/// Seal hataları — YALNIZ yapısal (S3 §3); kripto nedeni taşımaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SealError {
    /// Çerçeve bağlamın `max_plaintext_bytes` sınırını aşıyor.
    #[error("frame too large")]
    FrameTooLarge,
    /// Bağlamdaki oturum bilinmiyor / kapanmış / superseded.
    #[error("unknown session")]
    UnknownSession,
    /// Sayaç tavanı (`SEND_PN_CAP`): yeni mühür YOK → oturum yenilenir. Sayaç ASLA sarmaz.
    #[error("cap reached")]
    CapReached,
}

/// Datagram-düzeyi koruma (S3 §1.1). Endpoint başına TEK sahip (`Box<dyn PacketProtection>`;
/// `&mut` — gerçek katman PN/replay/oturum durumu tutar).
pub trait PacketProtection: Send + 'static {
    /// Düz çerçeveyi `ctx` bağlamında mühürler; PN'i Crypto atar (K2).
    ///
    /// # Errors
    /// Yapısal seal-reddi ([`SealError`]) — çağıran sayar, emisyonu kayıp-sayar (INV-RATE-1).
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError>;

    /// Wire datagram'ını açar/doğrular ve bağlamını kimliklendirir. `None` = SESSİZ düşürme;
    /// hiçbir state ilerlemez (K5).
    fn open(&mut self, wire: &[u8]) -> Option<Opened>;

    /// `plaintext_len` baytlık çerçevenin mühürlü wire boyu — KESİN (üst sınır değil); pacer
    /// admission'ı bununla yapılır: `seal(ctx, p)?.bytes.len() == sealed_len(ctx, p.len())?`.
    ///
    /// # Errors
    /// [`SealError::FrameTooLarge`] / [`SealError::UnknownSession`].
    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError>;

    /// Sınıfın TÜM profilleri için güvenli MİNİMUM; tutamaç gerektirmez (S1 `capabilities()`).
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits;

    /// Aktif bağlamın gerçek profili.
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits;
}

// ── §1.2 SessionControl ──────────────────────────────────────────────────────────────────────

/// Typed oturum komutu (K6: komut typed, zarf payload'ı OPAK).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// `early_unit` opak byte'tır (ccr §35.3 AMENDMENT-6); Crypto anlamını yorumlamaz.
    Initiate {
        peer: PeerRef,
        early_unit: Option<Vec<u8>>,
    },
    /// STORE_MESSAGE/DELIVER'dan gelen SSI1.
    AcceptInit {
        wire: Vec<u8>,
    },
    /// SSR1 — candidate seçimi Crypto'nun İÇİNDE (K7).
    AcceptResponse {
        wire: Vec<u8>,
    },
    Renew {
        session: SessionHandle,
    },
    Close {
        session: SessionHandle,
    },
}

/// Komut sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionResult {
    /// Transport bu zarfı Plane 1 ile yollar.
    OutboundEnvelope {
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    Established {
        session: SessionHandle,
    },
    /// `AcceptInit`: eşle mevcut oturum YOK → oturum hemen kullanılır, zarf = RESPONSE.
    EstablishedAndReply {
        session: SessionHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    /// `AcceptInit`: eşle mevcut oturum VAR (K10 / DR-CONNECT-1 K5). `new` PROVISIONAL'dır,
    /// `supersedes` TAM çalışmaya devam eder; zarf = RESPONSE. Onayda [`SessionEvent::Confirmed`].
    ProvisionalAndReply {
        new: SessionHandle,
        supersedes: SessionHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    },
    NoOutput,
    /// OPAK (S3 §3): alt-sınıfsız; yanlış candidate ile bozuk paket ayırt edilemez.
    Failed,
}

/// Oturum kapanış nedeni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// Atomik cutover (S1 KARAR-2).
    Renewed,
    /// Provisional supersession onaylandı (K10; DR-CONNECT-1 H6).
    Superseded,
}

/// Oturum yaşam-döngüsü olayı. Transport yorumlamadan Application'a iletir (S1 K7); tek istisna
/// `Confirmed`: Transport onu `Ready(new)` olarak yayınlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    Ready(SessionHandle),
    RenewRequired(SessionHandle),
    Closed(SessionHandle, CloseReason),
    /// Provisional oturum onaylandı: `new` üzerinde ilk AEAD-geçerli epoch-1 paketi açıldı.
    /// Ardından `supersedes` yalnız-alım drain'e düşer (KS-GRACE-1) → `Closed(_, Superseded)`.
    Confirmed(SessionHandle),
}

/// Oturum kontrolü (S3 §1.2). Crypto ağ retransmisyon timer'ı TUTMAZ (K9); retry Transport'un
/// cache'lediği zarfın byte-for-byte yeniden gönderimidir (INV-INIT-KEY-1).
pub trait SessionControl: Send + 'static {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult;
    fn poll_event(&mut self) -> Option<SessionEvent>;
}

// ── Entropi ──────────────────────────────────────────────────────────────────────────────────

/// Entropi kaynağı tükendi / kullanılamıyor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("entropy source unavailable")]
pub struct EntropyError;

/// Crypto'nun ephemeral anahtar üretimi için ENJEKTE EDİLEBİLİR entropi kaynağı.
///
/// Üretimde OS kaynağıdır (implementasyonu `kama-noise-wire`'da, A5). Testte deterministik kaynak
/// verilir: SSI1/SSR1 exact vektörleri (A2.3/A2.4) ancak ephemeral anahtar sabitlenebilirse
/// byte-for-byte doğrulanabilir.
pub trait EntropySource: Send + 'static {
    /// `dst`'yi TAMAMEN doldurur ya da hata döner (kısmi doldurma YOK).
    ///
    /// # Errors
    /// [`EntropyError`] — kaynak tükendi.
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError>;
}

/// Önceden verilmiş byte'ları sırayla döndüren deterministik kaynak — YALNIZ TEST.
///
/// Tükenince hata verir (sessizce sıfır ya da tekrar ÜRETMEZ: vektör testinde fazladan bir
/// ephemeral çekilirse bu görünür olmalı).
#[cfg(any(test, feature = "test-util"))]
#[derive(Debug, Clone)]
pub struct FixedEntropy {
    bytes: Vec<u8>,
    pos: usize,
}

#[cfg(any(test, feature = "test-util"))]
impl FixedEntropy {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Henüz tüketilmemiş bayt sayısı.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
}

#[cfg(any(test, feature = "test-util"))]
impl EntropySource for FixedEntropy {
    fn fill(&mut self, dst: &mut [u8]) -> Result<(), EntropyError> {
        if dst.len() > self.remaining() {
            return Err(EntropyError);
        }
        dst.copy_from_slice(&self.bytes[self.pos..self.pos + dst.len()]);
        self.pos += dst.len();
        Ok(())
    }
}

// ── IdentitySeam ─────────────────────────────────────────────────────────────────────────────

/// AEAD-tag SİMÜLASYONU: her datagram'a +16 B sıfır-tag ekler/soyar (byte-eşdeğer maliyet modeli;
/// kampanya TAG_STUB soyu). Güvenlik iddiası olmayan stub.
///
/// Tek örtük oturumu vardır ([`IdentitySeam::SESSION`]); `open` her datagram'ı ona atfeder (peer
/// ayrımı YOK). PN'i kendisi sayar (K2), bağlam sınıfı başına AYRI sayaçla; sayaç wire'a YAZILMAZ,
/// dolayısıyla v1'e göre wire byte'ları DEĞİŞMEDİ.
#[derive(Debug, Default)]
pub struct IdentitySeam {
    session_pn: u64,
    hub_pn: u64,
}

/// Sahte tag boyu (ChaCha20-Poly1305 tag'iyle byte-eşdeğer).
pub const IDENTITY_TAG: usize = 16;

/// Bir UDP/IPv4 datagramının taşıyabileceği azami payload (65 535 − 20 IP − 8 UDP).
const MAX_UDP_PAYLOAD: usize = 65_507;

impl IdentitySeam {
    /// Stub'ın tek örtük oturumu.
    pub const SESSION: SessionHandle = SessionHandle::from_raw(0);

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    const LIMITS: ProtectionLimits = ProtectionLimits {
        wire_overhead_bytes: IDENTITY_TAG as u32,
        max_plaintext_bytes: (MAX_UDP_PAYLOAD - IDENTITY_TAG) as u32,
    };

    fn check(ctx: ProtectionContext, plaintext_len: usize) -> Result<(), SealError> {
        if let ProtectionContext::Session(h) = ctx {
            if h != Self::SESSION {
                return Err(SealError::UnknownSession);
            }
        }
        if plaintext_len > Self::LIMITS.max_plaintext_bytes as usize {
            return Err(SealError::FrameTooLarge);
        }
        Ok(())
    }
}

impl PacketProtection for IdentitySeam {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        Self::check(ctx, frame.len())?;
        let counter = match ctx {
            ProtectionContext::Session(_) => &mut self.session_pn,
            ProtectionContext::HubChannel => &mut self.hub_pn,
        };
        let pn = *counter;
        // Sayaç SARMAZ (K2: wrap = aynı anahtar altında nonce tekrarı).
        *counter = pn.checked_add(1).ok_or(SealError::CapReached)?;
        let mut bytes = Vec::with_capacity(frame.len() + IDENTITY_TAG);
        bytes.extend_from_slice(frame);
        bytes.resize(frame.len() + IDENTITY_TAG, 0);
        let sealed_len = bytes.len() as u32; // ≤ MAX_UDP_PAYLOAD (check)
        Ok(SealedUnit {
            bytes,
            meta: SealedPacketMeta {
                full_wire_pn: pn,
                dh_epoch: 0,
                sealed_len,
            },
        })
    }

    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        if wire.len() <= IDENTITY_TAG || wire.len() > MAX_UDP_PAYLOAD {
            return None;
        }
        Some(Opened {
            ctx: ProtectionContext::Session(Self::SESSION),
            frame: wire[..wire.len() - IDENTITY_TAG].to_vec(),
            meta: SealedPacketMeta {
                // Stub PN'i wire'da taşımaz; alım tarafında bilinemez.
                full_wire_pn: 0,
                dh_epoch: 0,
                sealed_len: wire.len() as u32,
            },
        })
    }

    fn sealed_len(&self, ctx: ProtectionContext, plaintext_len: usize) -> Result<usize, SealError> {
        Self::check(ctx, plaintext_len)?;
        Ok(plaintext_len + IDENTITY_TAG)
    }

    fn conservative_limits(&self, _class: ProtectionClass) -> ProtectionLimits {
        Self::LIMITS
    }

    fn limits(&self, _ctx: ProtectionContext) -> ProtectionLimits {
        Self::LIMITS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: ProtectionContext = ProtectionContext::Session(IdentitySeam::SESSION);

    #[test]
    fn sealed_len_is_exact_and_bytes_match_v1_wire() {
        let mut seam = IdentitySeam::new();
        let frame = [0xABu8; 37];
        let u = seam.seal(S, &frame).expect("seal");
        // v1 wire'ı: çerçeve + 16 sıfır bayt. Değişmedi.
        assert_eq!(&u.bytes[..37], &frame);
        assert_eq!(&u.bytes[37..], &[0u8; IDENTITY_TAG]);
        assert_eq!(u.bytes.len(), seam.sealed_len(S, frame.len()).expect("len"));
        assert_eq!(u.meta.sealed_len as usize, u.bytes.len());
        let o = seam.open(&u.bytes).expect("open");
        assert_eq!(o.frame, frame);
        assert_eq!(o.ctx, S);
    }

    #[test]
    fn pn_is_owned_by_the_seam_and_separate_per_context_class() {
        let mut seam = IdentitySeam::new();
        let pns: Vec<u64> = (0..3)
            .map(|_| seam.seal(S, b"x").expect("seal").meta.full_wire_pn)
            .collect();
        assert_eq!(pns, [0, 1, 2], "Session PN ardışık ve Crypto'da (K2)");
        let hub = seam
            .seal(ProtectionContext::HubChannel, b"x")
            .expect("seal")
            .meta
            .full_wire_pn;
        assert_eq!(hub, 0, "HubChannel AYRI PN uzayı (K1)");
        assert_eq!(seam.seal(S, b"x").expect("seal").meta.full_wire_pn, 3);
    }

    #[test]
    fn pn_never_wraps() {
        let mut seam = IdentitySeam {
            session_pn: u64::MAX,
            hub_pn: 0,
        };
        assert_eq!(seam.seal(S, b"x").unwrap_err(), SealError::CapReached);
        assert_eq!(
            seam.session_pn,
            u64::MAX,
            "reddedilen seal sayacı ilerletmez"
        );
    }

    #[test]
    fn structural_rejects_are_typed_and_open_is_silent() {
        let mut seam = IdentitySeam::new();
        let other = ProtectionContext::Session(SessionHandle::from_raw(7));
        assert_eq!(
            seam.seal(other, b"x").unwrap_err(),
            SealError::UnknownSession
        );
        assert_eq!(
            seam.sealed_len(other, 1).unwrap_err(),
            SealError::UnknownSession
        );
        let big = vec![0u8; MAX_UDP_PAYLOAD];
        assert_eq!(seam.seal(S, &big).unwrap_err(), SealError::FrameTooLarge);
        assert!(
            seam.open(&[0u8; IDENTITY_TAG]).is_none(),
            "tag kadar kısa → sessiz drop"
        );
        assert!(seam.open(&[]).is_none());
    }

    #[test]
    fn fixed_entropy_is_exact_and_fails_closed_when_exhausted() {
        let mut e = FixedEntropy::new((0u8..40).collect());
        let mut a = [0u8; 32];
        e.fill(&mut a).expect("ilk 32");
        assert_eq!(a[0], 0);
        assert_eq!(a[31], 31);
        let mut b = [0u8; 32];
        assert_eq!(
            e.fill(&mut b),
            Err(EntropyError),
            "8 bayt kaldı; kısmi doldurma yok"
        );
        assert_eq!(b, [0u8; 32]);
        assert_eq!(e.remaining(), 8);
    }
}
