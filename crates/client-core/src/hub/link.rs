//! [`HubLink`] — sans-IO hub istemcisi çekirdeği — ve [`HubEnvelopeLink`] (`EnvelopeLink` cephesi).
//!
//! HB1-M0 (API iskeleti, KK-H32): imzalar kesindir, gövdeler tipli hata saplamasıdır. Erişilebilir
//! `todo!()` yoktur: `Result` dönen her yol `Err(HubError::NotImplemented(..))`, dönmeyenler etkisizdir.
//! Hiçbir ürün yolundan çağrılmaz.
//!
//! Kilit kuralı (HB1 ↔ HB2 sözleşmesi): `HubLink` kilidi tutulurken kripto kilidi alınmaz;
//! `HubEndpoint` kripto kilidini bırakmadan `HubLink` kilidini almaz. `HubLink` hiçbir Session
//! durumuna (ARQ, PN, oturum tablosu) erişmez (C8, D1).

use core::time::Duration;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use kama_contracts::handles::PeerRef;
use kama_contracts::s3::HubHandle;
use kama_noise_wire::entropy::EntropySource;

use super::{BindSigner, HubConfig, HubCounters, HubError, HubNotice};
use crate::transport::{EnvelopeLink, LinkAttempt, LinkError, LinkEvent};

/// Sans-IO hub istemcisi (S4 §4 C2–C9).
pub struct HubLink {
    cfg: HubConfig,
    counters: HubCounters,
    // HB1-M1/M2: bind / heartbeat / T3 / röle durumu buraya gelir.
    _signer: Box<dyn BindSigner>,
    _entropy: Box<dyn EntropySource>,
}

impl HubLink {
    /// Yeni istemci; bind hemen başlar (ilk `poll_transmit` CLIENT_BIND'i verir).
    ///
    /// `entropy`: üründe `OsEntropy`; testte sabit tohum. `request_id` başlangıcı ve her bind
    /// denemesinin `client_nonce`'u buradan alınır (S4 §2.3, C2).
    ///
    /// # Errors
    /// [`HubError::Entropy`] — `request_id` başlangıcı alınamadı.
    pub fn new(
        cfg: HubConfig,
        signer: Box<dyn BindSigner>,
        entropy: Box<dyn EntropySource>,
        now: Instant,
    ) -> Result<Self, HubError> {
        let _ = (cfg, signer, entropy, now);
        Err(HubError::NotImplemented("hub_link_new"))
    }

    /// Yapılandırma (salt-okunur).
    #[must_use]
    pub fn config(&self) -> &HubConfig {
        &self.cfg
    }

    /// Kriptonun açtığı bir HubChannel çerçevesi: `[0;4] ‖ frame_type ‖ gövde` (16 B kuyruk YOK).
    pub fn on_control(&mut self, now: Instant, opened_frame: &[u8]) {
        let _ = (now, opened_frame);
    }

    /// Gönderilecek sıradaki kontrol çerçevesi: `[0;4] ‖ frame_type ‖ gövde`. `HubEndpoint` mühürler
    /// (`seal(HubChannel, …)`) ve hub adresine yollar. Vadesi gelmiş zamanlayıcıları da işler.
    pub fn poll_transmit(&mut self, now: Instant) -> Option<Vec<u8>> {
        let _ = now;
        None
    }

    /// En yakın zamanlayıcı vadesi (yoksa `None`).
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        None
    }

    /// Vadesi gelmiş zamanlayıcıları işler (bind tekrarı, heartbeat, yoklama, T3, deneme yaşı).
    pub fn on_tick(&mut self, now: Instant) {
        let _ = now;
    }

    /// Bir oturum datagramı hub'a gitti (`outbound = true`) ya da hub'dan geldi (`false`).
    /// Yalnız sessizlik ölçümü içindir (heartbeat vadesi, C9 yaşı); T3'ü DURDURMAZ (KK-H36).
    pub fn on_session_traffic(&mut self, now: Instant, h: HubHandle, outbound: bool) {
        let _ = (now, h, outbound);
    }

    /// Soket hatası bildirimi (HB2): bağlıysa aktif yoklamayı başlatır (KK-H41).
    pub fn on_socket_error(&mut self, now: Instant) {
        let _ = now;
    }

    /// Bileşim kökünden S1 `Ready` (KK-H36): `peer` için atanmış `h`'si EŞLEŞEN denemenin T3'ü durur.
    /// Eşleşmeyen (eski oturumun) `Ready`'si yeni denemeyi kapatmaz.
    pub fn peer_ready(&mut self, peer: PeerRef, h: HubHandle) {
        let _ = (peer, h);
    }

    /// INIT: `STORE_MESSAGE{0, 0x01, peer, env}` kuyruğu + T3 (C3). Bind öncesi kuyrukta bekler.
    ///
    /// # Errors
    /// [`HubError::BadEnvelopeType`] · [`HubError::FrameTooLong`] · [`HubError::QueueFull`].
    pub fn send_init(
        &mut self,
        now: Instant,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, HubError> {
        let _ = (now, peer, envelope_type, bytes);
        Err(HubError::NotImplemented("hub_send_init"))
    }

    /// RESPONSE: `STORE_MESSAGE{h, 0x02, peer, env}` (C4); baytlar `(h, INIT)` kaydına bağlanır
    /// (KK-H38).
    ///
    /// # Errors
    /// [`HubError::BadEnvelopeType`] · [`HubError::FrameTooLong`] · [`HubError::QueueFull`].
    pub fn send_response(
        &mut self,
        now: Instant,
        peer: PeerRef,
        h: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), HubError> {
        let _ = (now, peer, h, envelope_type, bytes);
        Err(HubError::NotImplemented("hub_send_response"))
    }

    /// Transport'a giden sıradaki zarf olayı (KK-H37, KK-H38 kurallarıyla).
    pub fn poll_event(&mut self) -> Option<LinkEvent> {
        None
    }

    /// Hub imzası doğrulanmış bind var mı.
    #[must_use]
    pub fn is_bound(&self) -> bool {
        false
    }

    /// Hub'dan gelen son datagramın (her tür) yaşı (C9 `last_heartbeat_age_s`).
    #[must_use]
    pub fn last_rx_age(&self, now: Instant) -> Option<Duration> {
        let _ = now;
        None
    }

    /// Bileşim köküne sıradaki bildirim.
    pub fn take_notice(&mut self) -> Option<HubNotice> {
        None
    }

    /// Sayaçlar.
    #[must_use]
    pub fn counters(&self) -> HubCounters {
        self.counters
    }
}

/// `EnvelopeLink` cephesinin zamanı. `EnvelopeLink` çağrıları zaman taşımaz; cephe onu buradan alır.
/// Üründe [`SystemHubClock`]; testte sahte saat.
pub trait HubClock: Send + Sync {
    fn now(&self) -> Instant;
}

/// `Instant::now()`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemHubClock;

impl HubClock for SystemHubClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Transport'un `EnvelopeLink`'i: paylaşılan [`HubLink`] üstünde ince cephe.
#[derive(Clone)]
pub struct HubEnvelopeLink {
    link: Arc<Mutex<HubLink>>,
    clock: Arc<dyn HubClock>,
}

impl HubEnvelopeLink {
    /// Üretim saatiyle cephe.
    #[must_use]
    pub fn new(link: Arc<Mutex<HubLink>>) -> Self {
        Self::with_clock(link, Arc::new(SystemHubClock))
    }

    /// Verilen saatle cephe.
    #[must_use]
    pub fn with_clock(link: Arc<Mutex<HubLink>>, clock: Arc<dyn HubClock>) -> Self {
        Self { link, clock }
    }

    /// Paylaşılan çekirdek.
    #[must_use]
    pub fn shared(&self) -> &Arc<Mutex<HubLink>> {
        &self.link
    }

    fn with<T>(&self, f: impl FnOnce(&mut HubLink, Instant) -> T) -> T {
        let now = self.clock.now();
        let mut guard = self.link.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard, now)
    }
}

impl EnvelopeLink for HubEnvelopeLink {
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        self.with(|l, now| l.send_init(now, peer, envelope_type, bytes))
            .map_err(|_| LinkError)
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        self.with(|l, now| l.send_response(now, peer, hub_handle, envelope_type, bytes))
            .map_err(|_| LinkError)
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        self.with(|l, _| l.poll_event())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::HubConfig;
    use std::net::{Ipv4Addr, SocketAddr};

    struct NullSigner;
    impl BindSigner for NullSigner {
        fn sign_bind(&self, _tbs: &[u8; 112]) -> [u8; 64] {
            [0; 64]
        }
    }

    #[test]
    fn api_skeleton_returns_typed_errors() {
        let cfg = HubConfig {
            hub_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 7000)),
            hub_ed25519_pub: [0; 32],
            device_cert_ref: [0; 32],
            self_dest_hash: [0; 32],
        };
        let r = HubLink::new(
            cfg,
            Box::new(NullSigner),
            Box::new(kama_noise_wire::entropy::OsEntropy::new()),
            Instant::now(),
        );
        assert!(matches!(r, Err(HubError::NotImplemented(_))));
    }
}
