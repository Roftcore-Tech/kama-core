//! Hub istemcisi — S4 (`docs/seams/KM-SEAM-TH-001.md` v1.3.1) §4 istemci yükümlülükleri (A3-min).
//!
//! **Sans-IO** (KK-H33): bu modül soket, iş parçacığı ya da async runtime TUTMAZ. [`HubLink`] her
//! girişte `now: Instant` alır, çıkışını kuyruktan verir; zamanlayıcılar [`HubLink::next_deadline`] /
//! [`HubLink::on_tick`] ile sürülür. Soket ve mühürleme `HubEndpoint`'in (HB2) işidir.
//!
//! | dosya | iş |
//! |---|---|
//! | (tel) | S4 §2 codec'i `kama_contracts::s4`'tedir (KK-H34; yerel codec kaldırıldı) |
//! | [`bind`] | PoC-BIND: CLIENT_BIND → cookie → BIND_PROOF → imzalı yanıt (C2, C8) |
//! | [`heartbeat`] | sessizlikte heartbeat, aktif yoklama, "koptu" (C6, C7); jitter (KK-H48) |
//! | [`t3`] | T3 sabitleri ve aralık dizisi (KR-18, KK-H35) |
//! | [`link`] | [`HubLink`] çekirdeği ve [`HubEnvelopeLink`] (`EnvelopeLink` cephesi) |
//! | [`signer`] | [`BindSigner`] (anahtar bileşim kökünde, KK-H43) |
//!
//! Çerçeve sınırı: [`HubLink`]'in verdiği ve aldığı kontrol çerçevesi **açılmış** biçimdedir:
//! `[0;4] ‖ frame_type ‖ gövde` — 16 baytlık kuyruk YOK. Mühürleme / açma kriptonun
//! `seal(HubChannel, …)` / `open`'ıdır (KK-H53). Üretilen her çerçeve + 16 B ≤ `MAX_DATAGRAM`.
//!
//! Kapsam dışı (KK-H42; S4 §9): RELAY_OPEN gönderimi, `MEDIA_LINK`, `SESSION_REBIND`,
//! `SESSION_INIT_REF`, mailbox, hop-ARQ, HANDLE_ASSIGN çerçevesi, extended zarf.

pub mod bind;
pub mod heartbeat;
pub mod link;
pub mod signer;
pub mod t3;
// [S2-A3-2a / HB2] KK-H31: HB2 dosyaları.
pub mod endpoint;
pub mod shared_crypto;

use core::fmt;
use std::net::SocketAddr;

use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;

pub use link::{HubClock, HubEnvelopeLink, HubLink, SystemHubClock};
pub use signer::BindSigner;

/// Hub istemcisinin yapılandırması (provisioning dosyasından, bileşim kökünde kurulur).
///
/// `Debug` kimlik özetlerini BASMAZ (KK-H44): `self_dest_hash` ve `device_cert_ref` log'a düşmez.
#[derive(Clone, PartialEq, Eq)]
pub struct HubConfig {
    /// Hub'ın UDP adresi (HubEndpoint kullanır; HubLink yalnız taşır).
    pub hub_addr: SocketAddr,
    /// Hub'ın Ed25519 açık anahtarı — bind yanıtı bununla `verify_strict` edilir (C2).
    pub hub_ed25519_pub: [u8; 32],
    /// `device_cert_ref = SHA-256(compact_device_cert)` (`handshake::cert_ref_of`).
    pub device_cert_ref: [u8; 32],
    /// Bu cihazın `dest_hash`'i (bilgi; telde STORE isteğinde taşınmaz — hub bind'den bilir).
    pub self_dest_hash: [u8; 32],
}

impl fmt::Debug for HubConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HubConfig")
            .field("hub_addr", &self.hub_addr)
            .field("hub_ed25519_pub", &"<32 B>")
            .field("device_cert_ref", &"<redacted>")
            .field("self_dest_hash", &"<redacted>")
            .finish()
    }
}

/// Hub istemcisi hatası. İçerik (anahtar, nonce, `dest_hash`) TAŞIMAZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    /// API-first saplaması (CLAUDE.md §6): gövde henüz yazılmadı.
    #[error("hub istemcisi: henüz uygulanmadı ({0})")]
    NotImplemented(&'static str),
    /// Entropi kaynağı kullanılamıyor (`request_id` başlangıcı ya da `client_nonce`).
    #[error("hub istemcisi: entropi kaynağı kullanılamıyor")]
    Entropy,
    /// Sınırlı bir kuyruk dolu (SR-AUDIT-001; KK-H40: bind öncesi STORE kuyruğu ≤ 16).
    #[error("hub istemcisi: kuyruk dolu")]
    QueueFull,
    /// Çerçeve + 16 B `MAX_DATAGRAM`'ı (1280 B) aşar.
    #[error("hub istemcisi: çerçeve MAX_DATAGRAM'ı aşıyor")]
    FrameTooLong,
    /// S4 §2.4(c) tablosunda geçersiz `envelope_type`.
    #[error("hub istemcisi: geçersiz zarf tipi")]
    BadEnvelopeType,
}

/// Bileşim köküne (HB2) giden bildirim. Transport'un bilmediği hub olaylarıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubNotice {
    /// Hub imzası doğrulanmış bind (ilk bind ya da yeniden bind).
    Bound,
    /// Cevapsız aktif yoklama sonrası "koptu" (C6); yeniden bind başladı (C7).
    Disconnected,
    /// `ROUTE_MISS{h}`: `h` bırakıldı; oturum için yeni INIT gerekir (C5). Tetik HB2'de (KK-H39).
    HandleLost { peer: PeerRef, h: HubHandle },
    /// Açılış denemesi sonlandı (`{UNKNOWN_HANDLE, 0}`, `ATTEMPT_MAX_AGE`, çelişkili handle ya da
    /// yeniden bind sonrası; C3, C7). Niyet sürüyorsa yeni kriptografik INIT gerekir (KK-H39).
    AttemptEnded { peer: PeerRef },
}

/// Salt-okunur sayaçlar (S4 C3/C4/C8/C9 sayaç adları). Hepsi doyarak artar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct HubCounters {
    /// `on_control`'e verilen çerçeve sayısı.
    pub rx_frames: u64,
    /// `poll_transmit`'ten çıkan çerçeve sayısı.
    pub tx_frames: u64,
    /// Çözülemeyen çerçeve (kısa, uzun, yanlış önek, katı codec hatası).
    pub rx_malformed: u64,
    /// Bilinmeyen `frame_type` / `method_id` (C9 `hub_frames_dropped`).
    pub hub_frames_dropped: u64,
    /// Kabul edilen bind (hub imzası doğrulanmış).
    pub binds_accepted: u64,
    /// Hub imzası `verify_strict`'ten geçmeyen bind yanıtı (bind "tamam" sayılmaz).
    pub bind_bad_signature: u64,
    /// `BAD_COOKIE` → 1. tura dönüş.
    pub bind_bad_cookie: u64,
    /// İmzasız BIND hata çerçevesi (yalnız ipucu; bind'i düşürmez, C8).
    pub bind_error_hints: u64,
    /// Bir bind turunun 3 gönderimi cevapsız kaldı.
    pub bind_rounds_exhausted: u64,
    /// Eşleşmeyen / geç bind yanıtı.
    pub bind_unmatched: u64,
    /// Gönderilen heartbeat (yoklamalar dahil).
    pub heartbeats_sent: u64,
    /// Eşleşen heartbeat ACK.
    pub heartbeat_acks: u64,
    /// Eşleşmeyen heartbeat ACK.
    pub heartbeat_ack_unmatched: u64,
    /// "Koptu" ilanı (cevapsız aktif yoklama).
    pub disconnects: u64,
    /// İlk gönderim + T3 tekrarı dahil INIT STORE gönderimi.
    pub init_stores_sent: u64,
    /// T3 tekrarı (bayt-özdeş).
    pub init_retransmits: u64,
    /// `StoreResp{OK, h}` → `HandleAssigned`.
    pub handles_assigned: u64,
    /// Aynı denemeye aynı `h` ile gelen ikinci `StoreResp{OK}` (idempotent).
    pub store_ok_duplicate: u64,
    /// `PEER_UNBOUND` / `RATE_LIMITED` / `HUB_CAPACITY` → ≥ 5 s geri çekilme.
    pub store_backoffs: u64,
    /// `attempt_handle_conflict` (C3): aynı denemede farklı `h`.
    pub attempt_handle_conflict: u64,
    /// `{UNKNOWN_HANDLE, 0}` ile biten deneme.
    pub attempt_unknown_handle: u64,
    /// `ATTEMPT_MAX_AGE` ile biten deneme.
    pub attempt_expired: u64,
    /// Aynı eşe yeni `send_init` ile sonlandırılan eski deneme.
    pub attempt_superseded: u64,
    /// Yeniden bind sonrası sonlandırılan (gönderilmiş, tamamlanmamış) deneme (C7).
    pub attempt_rebind_ended: u64,
    /// `peer_ready` ile tamamlanan deneme (T3 durdu).
    pub attempt_completed: u64,
    /// Eşleşen denemesi olmayan `peer_ready` (eski oturumun `Ready`'si; KK-H36).
    pub ready_unmatched: u64,
    /// Sonlanmış / bilinmeyen denemenin geç `StoreResp`'i (K9; atanmaz).
    pub late_store_resp: u64,
    /// RESPONSE STORE'unun yanıtı (bilgi).
    pub response_store_resp: u64,
    /// `DELIVER{INIT}` → `IncomingInit`.
    pub incoming_inits: u64,
    /// Aynı `(h, INIT)` ikinci DELIVER → saklı RESPONSE yeniden gitti (KK-H38).
    pub init_duplicate_resend: u64,
    /// Aynı `(h, INIT)` ikinci DELIVER, henüz RESPONSE yok (transport'a verilmedi).
    pub init_duplicate_pending: u64,
    /// `init_handle_conflict` (C4): aynı INIT baytları farklı `h` ile.
    pub init_handle_conflict: u64,
    /// `DELIVER{RESPONSE}` → `Response`.
    pub responses_delivered: u64,
    /// Aynı `(h, RESPONSE)` ikinci kez (poll'dan çıkmaz).
    pub response_duplicate: u64,
    /// StoreResp'ten önce gelen RESPONSE tutuldu (K3, KK-H37).
    pub response_held: u64,
    /// Eşleşen denemesi olmayan (geç / sonlanmış) RESPONSE (K9).
    pub late_response: u64,
    /// `DELIVER{RELAY_OPEN}` (yalnız sayılır; KK-H42).
    pub deliver_relay_open: u64,
    /// `ROUTE_MISS{h}` → `HandleLost`.
    pub route_miss: u64,
    /// Bilinmeyen `h` için `ROUTE_MISS`.
    pub route_miss_unknown: u64,
    /// Sınırlı tablo / kuyruk doluydu, en eski düştü ya da yeni reddedildi (SR-AUDIT-001).
    pub bounded_drops: u64,
    /// `send_response` için eşleşen `(h, INIT)` kaydı yoktu (gönderildi ama yeniden gönderilemez).
    pub response_unrecorded: u64,
    /// Entropi kaynağı hatası.
    pub entropy_errors: u64,
}
