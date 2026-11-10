//! [S2-A3-2a / HB2] `HubEndpoint` — telefonun hub'a bakan TEK UDP soketi (S4 C1, KR-14).
//!
//! # Topoloji (KK-H51; `runtime.rs` ve görev gövdeleri DEĞİŞMEDEN)
//!
//! `HubEndpoint` iki soket tutar: hub'a bakan TEK soket (üründe `0.0.0.0:0`) ve yerel geri döngü
//! RÖLE soketi (`127.0.0.1:0`). Hub kipinde veri alıcısı (`ServerEndpoint`) `127.0.0.1:0`'a bağlanır
//! ve her eşin veri adresi röle adresidir; oturum uçları yalnız geri döngüde konuşur. Hub'ın
//! gördüğü TEK kaynak adres hub soketidir. İç geri döngü soketleri beyanlı uygulama ayrıntısıdır
//! (Aralık'ta SAAT-G/Ç `DatagramIo` ile kalkar).
//!
//! ```text
//! giden kontrol : HubControl::poll_transmit ─▶ seal(HubChannel) (‖0×16) ─▶ ≤1280? ─▶ hub
//! giden oturum  : uç ─(127.0.0.1)─▶ röle ─▶ ilk4 = h: istemci portunu öğren,
//!                 on_session_traffic(h, out) ─▶ ≤1280? ─▶ hub        (bayt DEĞİŞMEZ; K2)
//! gelen         : hub ─▶ ilk4 = 0 ─▶ open ─▶ HubChannel ─▶ HubControl::on_control(çerçeve)
//!                      └ ilk4 = h ─▶ yuva ayır ─▶ open (TEK) ─▶ Session(h)? ─▶ tip ─▶ rol
//!                          ├ istemci tipi ─▶ kanala taşı ─▶ zil ─▶ h'nin istemci ucu portu
//!                          └ sunucu tipi  ─▶ kanala taşı ─▶ zil ─▶ veri alıcısı
//! ```
//!
//! Tek açma ve kaydın sahipliği `super::shared_crypto`'dadır (tasarım notu
//! `docs/design/HB2-ALIM-YOLU.md`, seçenek (a)).
//!
//! # Rol tablosu (T3; bağlam AEAD'den sonra `Session(h)` doğrulandıktan sonra)
//!
//! İstemci ucu: `0x02 RECOVERED`, `0x04 PARAM_ACK`, `0x07 CS_ECHO`, `0x10`/`0x14 ACK`.
//! Veri alıcısı: `0x00 DATA`, `0x01 FEC`, `0x03 PARAM`, `0x05 DATA_FRAG`, `0x06 CS_PROBE`.
//! Bilinmeyen tip → düşer + `hub_role_unknown`. Görevin kendi bağlam kapısı yine çalışır.
//!
//! # HB1 sınırı
//!
//! [`HubControl`], HB1'in `HubLink` sans-IO çekirdeğinin `HubEndpoint`'in çağırdığı yüzeyidir
//! (iş emri §6 sözleşmesi; imzalar HB1 `HubLink` ile birebir, #158). `impl HubControl for HubLink`
//! saf iletimdir; trait yalnız testlerde sahte çekirdek koyabilmek içindir.
//!
//! KK-H66 ("hub'ın atadığı handle = kriptonun yazdığı handle") ölçüsünün handle kümesi HB1
//! dosyalarına dokunmadan tutulur: [`RecordingLink`] transport'a verilen `EnvelopeLink`'i sarar ve
//! taşıyıcının bildirdiği her handle'ı (`HandleAssigned`, `IncomingInit`, `Response`) ve
//! `send_response`'un handle'ını [`KnownHandles`]'a yazar; `HubEndpoint` giden datagramın ilk dört
//! baytını bu kümeyle karşılaştırır.
//!
//! # Kilit kuralı
//!
//! `HubControl` kilidi tutulurken kripto kilidi alınmaz; kripto kilidi bırakılmadan `HubControl`
//! kilidi alınmaz. Kanal (`OpenedStash`) kilidi ikisinden biriyle birlikte tutulmaz.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use adaptive_transport::ack::{FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2};
use kama_contracts::handles::PeerRef;
use kama_contracts::s3::{HubHandle, ProtectionContext};

use super::link::HubLink;
use super::shared_crypto::{HubCryptoAccess, StashCounters};
use crate::transport::{EnvelopeLink, LinkAttempt, LinkError, LinkEvent};
use crate::wire::{
    FRAME_CS_ECHO, FRAME_CS_PROBE, FRAME_DATA, FRAME_DATA_FRAG, FRAME_FEC, FRAME_PARAM,
    FRAME_PARAM_ACK, FRAME_RECOVERED,
};

/// S4 §6 `MAX_DATAGRAM`: hub'a giden ve hub'dan gelen datagram tavanı.
pub const MAX_DATAGRAM: usize = 1280;

/// Hub alım döngüsünün azami uyku süresi (`HubControl::next_deadline` daha erkense o).
pub const HUB_POLL: Duration = Duration::from_millis(20);

/// Röle alım döngüsünün okuma zaman aşımı (yalnız kapanış bayrağının denetim sıklığı).
const RELAY_POLL: Duration = Duration::from_millis(50);

/// `h → istemci ucu portu` eşlemesinin tavanı (SR-AUDIT-001); dolunca en eski eşleme düşer.
pub const CLIENT_PORT_CAP: usize = 256;

/// Bir turda `poll_transmit`'ten çekilen kontrol çerçevesi tavanı.
const CONTROL_BURST: usize = 64;

/// Alım tamponu: UDP'nin taşıyabileceği en büyük yük (büyük datagram kesilmeden görülür ve
/// sayaçla düşer).
const RECV_BUF: usize = 65_536;

fn lock<T: ?Sized>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Oturum datagramının ilk dört baytı (`hub_handle`, big-endian). `None` = 4 bayttan kısa.
#[must_use]
pub fn wire_handle(bytes: &[u8]) -> Option<u32> {
    let b: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_be_bytes(b))
}

// ── Rol ──────────────────────────────────────────────────────────────────────────────────────

/// Açılmış oturum çerçevesinin görevi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Oturumun istemci ucu (`ClientEndpoint`): bu ucun gönderdiğine cevaplar.
    Client,
    /// Veri alıcısı (`ServerEndpoint`): eşin gönderdiği veri.
    Server,
}

/// T3 tablosu: çerçevenin ilk baytından rol. Bilinmeyen tip `None`.
#[must_use]
pub fn role_of(frame_type: u8) -> Option<Role> {
    match frame_type {
        FRAME_RECOVERED | FRAME_PARAM_ACK | FRAME_CS_ECHO | FRAME_TYPE_ACK | FRAME_TYPE_ACK_V2 => {
            Some(Role::Client)
        }
        FRAME_DATA | FRAME_FEC | FRAME_PARAM | FRAME_DATA_FRAG | FRAME_CS_PROBE => {
            Some(Role::Server)
        }
        _ => None,
    }
}

// ── HB1 yüzeyi ───────────────────────────────────────────────────────────────────────────────

/// HB1 `HubLink`'in `HubEndpoint`'ten çağrılan yüzeyi (iş emri §6). Çerçeveler `[0;4] ‖ type ‖
/// gövde` biçimindedir (16 B kuyruk YOK); mühürleme `HubEndpoint`'indir.
pub trait HubControl: Send {
    /// Açılmış kontrol çerçevesi.
    fn on_control(&mut self, now: Instant, opened_frame: &[u8]);
    /// Gönderilecek sıradaki kontrol çerçevesi.
    fn poll_transmit(&mut self, now: Instant) -> Option<Vec<u8>>;
    /// Sıradaki zamanlayıcı.
    fn next_deadline(&self) -> Option<Instant>;
    /// Zamanlayıcı turu.
    fn on_tick(&mut self, now: Instant);
    /// Oturum trafiği (`outbound = true` giden; `false` hub'dan gelen — C9 yaşı).
    fn on_session_traffic(&mut self, now: Instant, h: HubHandle, outbound: bool);
    /// Hub soketi hata verdi (KK-H41 yoklama tetiği).
    fn on_socket_error(&mut self, now: Instant);
    /// Bind doğrulandı mı (C2: bind olmadan hiçbir şey gönderilmez).
    fn is_bound(&self) -> bool;
}

/// HB1 `HubLink`: saf iletim (her yöntem aynı adlı `HubLink` yöntemini çağırır).
impl HubControl for HubLink {
    fn on_control(&mut self, now: Instant, opened_frame: &[u8]) {
        HubLink::on_control(self, now, opened_frame);
    }
    fn poll_transmit(&mut self, now: Instant) -> Option<Vec<u8>> {
        HubLink::poll_transmit(self, now)
    }
    fn next_deadline(&self) -> Option<Instant> {
        HubLink::next_deadline(self)
    }
    fn on_tick(&mut self, now: Instant) {
        HubLink::on_tick(self, now);
    }
    fn on_session_traffic(&mut self, now: Instant, h: HubHandle, outbound: bool) {
        HubLink::on_session_traffic(self, now, h, outbound);
    }
    fn on_socket_error(&mut self, now: Instant) {
        HubLink::on_socket_error(self, now);
    }
    fn is_bound(&self) -> bool {
        HubLink::is_bound(self)
    }
}

// ── KK-H66 handle kümesi ─────────────────────────────────────────────────────────────────────

/// [`KnownHandles`]'ın tavanı (SR-AUDIT-001); dolunca en eski handle düşer.
pub const KNOWN_HANDLE_CAP: usize = 1024;

#[derive(Default)]
struct HandleSet {
    set: HashSet<u32>,
    order: VecDeque<u32>,
}

/// Taşıyıcının atadığı ya da teslim ettiği handle'lar (KK-H66). Paylaşılan, sınırlı.
#[derive(Clone, Default)]
pub struct KnownHandles(Arc<Mutex<HandleSet>>);

impl std::fmt::Debug for KnownHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnownHandles")
            .field("len", &lock(&self.0).set.len())
            .finish()
    }
}

impl KnownHandles {
    /// Boş küme.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `h`'yi ekler (`0` eklenmez).
    pub fn insert(&self, h: HubHandle) {
        let raw = h.raw();
        if raw == 0 {
            return;
        }
        let mut s = lock(&self.0);
        if !s.set.insert(raw) {
            return;
        }
        s.order.push_back(raw);
        if s.set.len() > KNOWN_HANDLE_CAP {
            if let Some(old) = s.order.pop_front() {
                s.set.remove(&old);
            }
        }
    }

    /// `h` kümede mi.
    #[must_use]
    pub fn contains(&self, h: u32) -> bool {
        lock(&self.0).set.contains(&h)
    }
}

/// Transport'a verilen `EnvelopeLink`'in gözlemcisi: davranışı DEĞİŞTİRMEZ (aynı çağrılar, aynı
/// dönüşler, aynı sıra), yalnız handle'ları [`KnownHandles`]'a yazar.
pub struct RecordingLink<L> {
    inner: L,
    known: KnownHandles,
}

impl<L: EnvelopeLink> RecordingLink<L> {
    /// Sarar.
    #[must_use]
    pub fn new(inner: L, known: KnownHandles) -> Self {
        Self { inner, known }
    }
}

impl<L: EnvelopeLink> EnvelopeLink for RecordingLink<L> {
    fn send_init(
        &mut self,
        peer: PeerRef,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<LinkAttempt, LinkError> {
        self.inner.send_init(peer, envelope_type, bytes)
    }

    fn send_response(
        &mut self,
        peer: PeerRef,
        hub_handle: HubHandle,
        envelope_type: u8,
        bytes: Vec<u8>,
    ) -> Result<(), LinkError> {
        self.known.insert(hub_handle);
        self.inner
            .send_response(peer, hub_handle, envelope_type, bytes)
    }

    fn poll(&mut self) -> Option<LinkEvent> {
        let ev = self.inner.poll()?;
        match &ev {
            LinkEvent::HandleAssigned { hub_handle, .. }
            | LinkEvent::IncomingInit { hub_handle, .. }
            | LinkEvent::Response { hub_handle, .. } => self.known.insert(*hub_handle),
        }
        Some(ev)
    }
}

// ── Hatalar ve sayaçlar ──────────────────────────────────────────────────────────────────────

/// Hub'a gönderim hatası.
#[derive(Debug, thiserror::Error)]
pub enum HubSendError {
    /// Datagram `MAX_DATAGRAM`'ı aşıyor: gönderilmedi (KK-H54; S4 §6).
    #[error("datagram {len} B > {MAX_DATAGRAM} B (MAX_DATAGRAM): gönderilmedi")]
    DatagramTooLarge {
        /// Datagram boyu.
        len: usize,
    },
    /// Soket hatası.
    #[error("hub soketi: {0}")]
    Io(#[from] io::Error),
}

/// `HubEndpoint` sayaçları (salt-okunur anlık görüntü).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HubEndpointCounters {
    /// Hub'dan alınan datagram (hub adresinden).
    pub hub_rx: u64,
    /// Hub'a gönderilen datagram.
    pub hub_tx: u64,
    /// `MAX_DATAGRAM`'ı aştığı için gönderilmeyen.
    pub hub_oversize_drops: u64,
    /// Açılmış oturum çerçevesinin tipi T3 tablosunda yok.
    pub hub_role_unknown: u64,
    /// İlk dört baytla bağlam uyuşmadı (`≠ 0` iken `HubChannel`, `0` iken `Session`).
    pub hub_ctx_mismatch: u64,
    /// Kripto açmadı.
    pub hub_open_fail: u64,
    /// İstemci rolü çerçevesi, ama `h` için istemci ucu portu bilinmiyor.
    pub hub_no_client_port: u64,
    /// Sunucu rolü çerçevesi, ama veri alıcısı adresi verilmedi.
    pub hub_no_server_addr: u64,
    /// Giden oturum datagramının `h`'si [`KnownHandles`] dışında (KK-H66).
    pub hub_out_unknown_handle: u64,
    /// Hub adresi dışından gelen datagram (sessiz düşer).
    pub hub_rx_foreign: u64,
    /// Bind yokken giden oturum datagramı (C2; düşer, ARQ toparlar).
    pub hub_tx_unbound: u64,
    /// Röleye gelen geçersiz datagram (kısa, `h = 0`, geri döngü dışı kaynak, aşırı büyük).
    pub hub_relay_bad: u64,
    /// Hub soketi gönderim/alım hatası.
    pub hub_socket_errors: u64,
    /// `on_control`'e verilen kontrol çerçevesi.
    pub control_rx: u64,
    /// Gönderilen kontrol çerçevesi.
    pub control_tx: u64,
    /// Tek-açma kanalının sayaçları (`stash.lost()` = `hub_stash_miss`).
    pub stash: StashCounters,
}

#[derive(Default)]
struct Atomics {
    hub_rx: AtomicU64,
    hub_tx: AtomicU64,
    hub_oversize_drops: AtomicU64,
    hub_role_unknown: AtomicU64,
    hub_ctx_mismatch: AtomicU64,
    hub_open_fail: AtomicU64,
    hub_no_client_port: AtomicU64,
    hub_no_server_addr: AtomicU64,
    hub_out_unknown_handle: AtomicU64,
    hub_rx_foreign: AtomicU64,
    hub_tx_unbound: AtomicU64,
    hub_relay_bad: AtomicU64,
    hub_socket_errors: AtomicU64,
    control_rx: AtomicU64,
    control_tx: AtomicU64,
}

fn inc(c: &AtomicU64) {
    c.fetch_add(1, Ordering::Relaxed);
}

fn get(c: &AtomicU64) -> u64 {
    c.load(Ordering::Relaxed)
}

/// `h → istemci ucu portu`, sınırlı (en eski düşer).
#[derive(Default)]
struct ClientPorts {
    map: HashMap<u32, SocketAddr>,
    order: VecDeque<u32>,
}

impl ClientPorts {
    fn learn(&mut self, h: u32, addr: SocketAddr) {
        if let Some(a) = self.map.get_mut(&h) {
            *a = addr;
            return;
        }
        if self.map.len() >= CLIENT_PORT_CAP {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
        self.map.insert(h, addr);
        self.order.push_back(h);
    }

    fn get(&self, h: u32) -> Option<SocketAddr> {
        self.map.get(&h).copied()
    }
}

/// Hub'a tek gönderim noktası: `MAX_DATAGRAM` denetimi (KK-H54 yedeği) + sayaçlar.
///
/// # Errors
/// [`HubSendError::DatagramTooLarge`] (tele ÇIKMADI) · [`HubSendError::Io`].
fn send_to_hub(
    sock: &UdpSocket,
    hub: SocketAddr,
    bytes: &[u8],
    c: &Atomics,
) -> Result<(), HubSendError> {
    if bytes.len() > MAX_DATAGRAM {
        inc(&c.hub_oversize_drops);
        return Err(HubSendError::DatagramTooLarge { len: bytes.len() });
    }
    match sock.send_to(bytes, hub) {
        Ok(_) => {
            inc(&c.hub_tx);
            Ok(())
        }
        Err(e) => {
            inc(&c.hub_socket_errors);
            Err(HubSendError::Io(e))
        }
    }
}

// ── Çekirdek ─────────────────────────────────────────────────────────────────────────────────

/// İki döngünün paylaştığı durum. Döngü gövdeleri `on_*` yöntemleridir (testler doğrudan sürer).
struct Core {
    hub_sock: UdpSocket,
    relay_sock: UdpSocket,
    hub_addr: SocketAddr,
    server_addr: Mutex<Option<SocketAddr>>,
    client_ports: Mutex<ClientPorts>,
    crypto: Box<dyn HubCryptoAccess>,
    control: Arc<Mutex<dyn HubControl>>,
    known: KnownHandles,
    counters: Atomics,
    stop: AtomicBool,
}

impl Core {
    /// Hub soketinden gelen datagram.
    fn on_hub_datagram(&self, now: Instant, src: SocketAddr, bytes: &[u8]) {
        if src != self.hub_addr {
            inc(&self.counters.hub_rx_foreign);
            return;
        }
        inc(&self.counters.hub_rx);
        if bytes.len() > MAX_DATAGRAM {
            inc(&self.counters.hub_oversize_drops);
            return;
        }
        match wire_handle(bytes) {
            None => inc(&self.counters.hub_open_fail),
            Some(0) => self.on_control_datagram(now, bytes),
            Some(h) => {
                lock(&self.control).on_session_traffic(now, HubHandle::from_raw(h), false);
                self.on_session_datagram(now, h, bytes);
            }
        }
    }

    /// `[0;4] ‖ type ‖ gövde ‖ 0×16` → `HubControl::on_control` (KK-H53).
    fn on_control_datagram(&self, now: Instant, bytes: &[u8]) {
        let Some(op) = self.crypto.open(bytes) else {
            inc(&self.counters.hub_open_fail);
            return;
        };
        if op.ctx != ProtectionContext::HubChannel {
            inc(&self.counters.hub_ctx_mismatch);
            return;
        }
        inc(&self.counters.control_rx);
        lock(&self.control).on_control(now, &op.frame);
    }

    /// `h ≠ 0`: yuva ayır → TEK açma → bağlam → tip → rol → kanala taşı → zil.
    fn on_session_datagram(&self, now: Instant, h: u32, bytes: &[u8]) {
        let stash = self.crypto.stash();
        // Önce yer: yoksa AÇMADAN düşer (kanal sayar; kripto durumu değişmez).
        let Some(r) = lock(stash).reserve(h) else {
            return;
        };
        let Some(op) = self.crypto.open(bytes) else {
            lock(stash).cancel(r);
            inc(&self.counters.hub_open_fail);
            return;
        };
        let ProtectionContext::Session(session) = op.ctx else {
            // Kriptoda tutamacı bilinmeyen ve kuyruğu sıfır datagram (`protector.rs` open_hub).
            lock(stash).cancel(r);
            inc(&self.counters.hub_ctx_mismatch);
            return;
        };
        let dest = match op.frame.first().copied().and_then(role_of) {
            None => {
                lock(stash).cancel(r);
                inc(&self.counters.hub_role_unknown);
                return;
            }
            Some(Role::Server) => match *lock(&self.server_addr) {
                Some(a) => a,
                None => {
                    lock(stash).cancel(r);
                    inc(&self.counters.hub_no_server_addr);
                    return;
                }
            },
            Some(Role::Client) => match lock(&self.client_ports).get(h) {
                Some(a) => a,
                None => {
                    lock(stash).cancel(r);
                    inc(&self.counters.hub_no_client_port);
                    return;
                }
            },
        };
        lock(stash).fill(r, session, bytes.to_vec(), op, dest, now);
        // Zil sahiplik taşımaz: gönderim başarısız olsa da kayıt kanalda kalır, yeniden çalınır.
        let _ = self.relay_sock.send_to(bytes, dest);
    }

    /// Röle soketine yerel uçtan gelen oturum datagramı → baytına dokunmadan hub'a.
    fn on_relay_datagram(&self, now: Instant, src: SocketAddr, bytes: &[u8]) {
        let h = match wire_handle(bytes) {
            Some(h) if h != 0 && src.ip().is_loopback() => h,
            _ => {
                inc(&self.counters.hub_relay_bad);
                return;
            }
        };
        let is_server = *lock(&self.server_addr) == Some(src);
        if !is_server {
            lock(&self.client_ports).learn(h, src);
        }
        {
            let mut c = lock(&self.control);
            if !c.is_bound() {
                inc(&self.counters.hub_tx_unbound);
                return;
            }
            c.on_session_traffic(now, HubHandle::from_raw(h), true);
        }
        if !self.known.contains(h) {
            // Yine gönderilir (hub ROUTE_MISS verir); dumanda 0 olmalı.
            inc(&self.counters.hub_out_unknown_handle);
        }
        if let Err(HubSendError::Io(_)) =
            send_to_hub(&self.hub_sock, self.hub_addr, bytes, &self.counters)
        {
            lock(&self.control).on_socket_error(now);
        }
    }

    /// Zamanlayıcı turu: `on_tick` → kontrol çerçevelerini mühürle/gönder → zilleri yeniden çal.
    fn on_tick(&self, now: Instant) {
        let frames: Vec<Vec<u8>> = {
            let mut c = lock(&self.control);
            c.on_tick(now);
            let mut v = Vec::new();
            while v.len() < CONTROL_BURST {
                match c.poll_transmit(now) {
                    Some(f) => v.push(f),
                    None => break,
                }
            }
            v
        };
        for f in frames {
            let Ok(sealed) = self.crypto.seal_hub(&f) else {
                inc(&self.counters.hub_oversize_drops);
                continue;
            };
            match send_to_hub(&self.hub_sock, self.hub_addr, &sealed.bytes, &self.counters) {
                Ok(()) => inc(&self.counters.control_tx),
                Err(HubSendError::Io(_)) => lock(&self.control).on_socket_error(now),
                Err(HubSendError::DatagramTooLarge { .. }) => {}
            }
        }
        let rings = lock(self.crypto.stash()).due_rings(now);
        for (dest, bytes) in rings {
            let _ = self.relay_sock.send_to(&bytes, dest);
        }
    }

    fn snapshot(&self) -> HubEndpointCounters {
        let c = &self.counters;
        HubEndpointCounters {
            hub_rx: get(&c.hub_rx),
            hub_tx: get(&c.hub_tx),
            hub_oversize_drops: get(&c.hub_oversize_drops),
            hub_role_unknown: get(&c.hub_role_unknown),
            hub_ctx_mismatch: get(&c.hub_ctx_mismatch),
            hub_open_fail: get(&c.hub_open_fail),
            hub_no_client_port: get(&c.hub_no_client_port),
            hub_no_server_addr: get(&c.hub_no_server_addr),
            hub_out_unknown_handle: get(&c.hub_out_unknown_handle),
            hub_rx_foreign: get(&c.hub_rx_foreign),
            hub_tx_unbound: get(&c.hub_tx_unbound),
            hub_relay_bad: get(&c.hub_relay_bad),
            hub_socket_errors: get(&c.hub_socket_errors),
            control_rx: get(&c.control_rx),
            control_tx: get(&c.control_tx),
            stash: lock(self.crypto.stash()).counters(),
        }
    }

    /// Hub alım döngüsü.
    fn hub_loop(&self) {
        let mut buf = vec![0u8; RECV_BUF];
        while !self.stop.load(Ordering::Acquire) {
            let now = Instant::now();
            let wait = lock(&self.control)
                .next_deadline()
                .map_or(HUB_POLL, |d| d.saturating_duration_since(now).min(HUB_POLL))
                .max(Duration::from_millis(1));
            if self.hub_sock.set_read_timeout(Some(wait)).is_err() {
                inc(&self.counters.hub_socket_errors);
            }
            match self.hub_sock.recv_from(&mut buf) {
                Ok((n, src)) => self.on_hub_datagram(Instant::now(), src, &buf[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => {
                    inc(&self.counters.hub_socket_errors);
                    lock(&self.control).on_socket_error(Instant::now());
                }
            }
            self.on_tick(Instant::now());
        }
    }

    /// Röle alım döngüsü.
    fn relay_loop(&self) {
        let mut buf = vec![0u8; RECV_BUF];
        while !self.stop.load(Ordering::Acquire) {
            match self.relay_sock.recv_from(&mut buf) {
                Ok((n, src)) => self.on_relay_datagram(Instant::now(), src, &buf[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => inc(&self.counters.hub_relay_bad),
            }
        }
    }
}

// ── HubEndpoint ──────────────────────────────────────────────────────────────────────────────

/// Kurulum yapılandırması.
#[derive(Debug, Clone, Copy)]
pub struct HubEndpointConfig {
    /// Hub'ın adresi (provisioning `hub.addr`, açılışta bir kez çözülmüş).
    pub hub_addr: SocketAddr,
    /// Hub soketinin bağlanacağı yerel adres (üründe `0.0.0.0:0`; NAT arkası telefon).
    pub hub_bind: SocketAddr,
    /// Röle soketinin bağlanacağı yerel adres (üründe `127.0.0.1:0`).
    pub relay_bind: SocketAddr,
}

impl HubEndpointConfig {
    /// Ürün yapılandırması: hub soketi `0.0.0.0:0`, röle `127.0.0.1:0`.
    #[must_use]
    pub fn product(hub_addr: SocketAddr) -> Self {
        Self {
            hub_addr,
            hub_bind: SocketAddr::from(([0, 0, 0, 0], 0)),
            relay_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        }
    }
}

/// Hub'a bakan tek soket + yerel röle (bkz. modül belgesi). Düşerken iki döngü durur
/// (Drop panik etmez).
pub struct HubEndpoint {
    core: Arc<Core>,
    relay_addr: SocketAddr,
    hub_local: SocketAddr,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for HubEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HubEndpoint")
            .field("relay_addr", &self.relay_addr)
            .finish_non_exhaustive()
    }
}

impl HubEndpoint {
    /// Soketleri açar ve iki döngüyü (`std::thread`; yeni async runtime YOK) başlatır.
    ///
    /// # Errors
    /// Soket ya da iş parçacığı kurulamadı.
    pub fn start(
        cfg: HubEndpointConfig,
        crypto: Box<dyn HubCryptoAccess>,
        control: Arc<Mutex<dyn HubControl>>,
        known: KnownHandles,
    ) -> io::Result<Self> {
        let hub_sock = UdpSocket::bind(cfg.hub_bind)?;
        let relay_sock = UdpSocket::bind(cfg.relay_bind)?;
        relay_sock.set_read_timeout(Some(RELAY_POLL))?;
        hub_sock.set_read_timeout(Some(HUB_POLL))?;
        let relay_addr = relay_sock.local_addr()?;
        let hub_local = hub_sock.local_addr()?;
        let core = Arc::new(Core {
            hub_sock,
            relay_sock,
            hub_addr: cfg.hub_addr,
            server_addr: Mutex::new(None),
            client_ports: Mutex::new(ClientPorts::default()),
            crypto,
            control,
            known,
            counters: Atomics::default(),
            stop: AtomicBool::new(false),
        });
        let mut ep = Self {
            core,
            relay_addr,
            hub_local,
            threads: Vec::new(),
        };
        let c = Arc::clone(&ep.core);
        ep.threads.push(
            thread::Builder::new()
                .name("kama-hub-rx".to_owned())
                .spawn(move || c.hub_loop())?,
        );
        let c = Arc::clone(&ep.core);
        ep.threads.push(
            thread::Builder::new()
                .name("kama-hub-relay".to_owned())
                .spawn(move || c.relay_loop())?,
        );
        Ok(ep)
    }

    /// Röle soketinin adresi: hub kipinde her eşin veri adresi budur.
    #[must_use]
    pub fn relay_addr(&self) -> SocketAddr {
        self.relay_addr
    }

    /// Hub soketinin yerel adresi.
    #[must_use]
    pub fn hub_local_addr(&self) -> SocketAddr {
        self.hub_local
    }

    /// Veri alıcısının (`ClientTransport::data_addr`) adresi: sunucu rolü çerçeveleri buraya,
    /// bu kaynaktan gelen giden datagramlar sunucu rolüdür.
    pub fn set_server_addr(&self, addr: SocketAddr) {
        *lock(&self.core.server_addr) = Some(addr);
    }

    /// Sayaçlar.
    #[must_use]
    pub fn counters(&self) -> HubEndpointCounters {
        self.core.snapshot()
    }
}

impl Drop for HubEndpoint {
    fn drop(&mut self) {
        self.core.stop.store(true, Ordering::Release);
        for t in self.threads.drain(..) {
            // Döngü paniği burada raporlanacak yer bulamaz; Drop panik etmez.
            let _ = t.join();
        }
    }
}

// ── Testler ──────────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::shared_crypto::{OpenedStash, StashConfig};
    use kama_contracts::handles::SessionHandle;
    use kama_contracts::s3::{Opened, SealError, SealedPacketMeta, SealedUnit};
    use std::collections::{BTreeMap, HashSet};

    const H1: u32 = 0x7E57_0001;
    const H2: u32 = 0x7E57_0002;
    const TAG: usize = 16;
    const WAIT: Duration = Duration::from_secs(3);

    /// Sahte kripto: `[h ≠ 0][çerçeve…]` kayıtlı `h` için `Session`, `[0;4]…‖0×16` HubChannel;
    /// aynı baytların ikinci açılışı reddedilir (replay; T2). Açma sayısı kaydedilir.
    struct FakeCrypto {
        sessions: BTreeMap<u32, SessionHandle>,
        seen: Mutex<HashSet<Vec<u8>>>,
        opens: Mutex<Vec<Vec<u8>>>,
        stash: Arc<Mutex<OpenedStash>>,
    }

    impl FakeCrypto {
        fn new(stash: Arc<Mutex<OpenedStash>>) -> Arc<Self> {
            Arc::new(Self {
                sessions: [
                    (H1, SessionHandle::from_raw(1)),
                    (H2, SessionHandle::from_raw(2)),
                ]
                .into_iter()
                .collect(),
                seen: Mutex::new(HashSet::new()),
                opens: Mutex::new(Vec::new()),
                stash,
            })
        }
        fn opens_of(&self, bytes: &[u8]) -> usize {
            lock(&self.opens)
                .iter()
                .filter(|b| b.as_slice() == bytes)
                .count()
        }
    }

    fn meta() -> SealedPacketMeta {
        SealedPacketMeta {
            full_wire_pn: 0,
            dh_epoch: 0,
            sealed_len: 0,
        }
    }

    impl HubCryptoAccess for Arc<FakeCrypto> {
        fn open(&self, wire: &[u8]) -> Option<Opened> {
            lock(&self.opens).push(wire.to_vec());
            let h = wire_handle(wire)?;
            if let Some(s) = self.sessions.get(&h) {
                if !lock(&self.seen).insert(wire.to_vec()) {
                    return None;
                }
                return Some(Opened {
                    ctx: ProtectionContext::Session(*s),
                    frame: wire[4..].to_vec(),
                    meta: meta(),
                });
            }
            let (frame, tail) = wire.split_at(wire.len().checked_sub(TAG)?);
            if tail.iter().any(|&b| b != 0) || frame.is_empty() {
                return None;
            }
            Some(Opened {
                ctx: ProtectionContext::HubChannel,
                frame: frame.to_vec(),
                meta: meta(),
            })
        }
        fn seal_hub(&self, frame: &[u8]) -> Result<SealedUnit, SealError> {
            let mut bytes = frame.to_vec();
            bytes.resize(frame.len() + TAG, 0);
            Ok(SealedUnit {
                bytes,
                meta: meta(),
            })
        }
        fn stash(&self) -> &Arc<Mutex<OpenedStash>> {
            &self.stash
        }
    }

    #[derive(Default)]
    struct FakeControl {
        bound: bool,
        control_in: Vec<Vec<u8>>,
        to_send: VecDeque<Vec<u8>>,
        traffic: Vec<(HubHandle, bool)>,
        socket_errors: u32,
        ticks: u64,
    }

    impl HubControl for FakeControl {
        fn on_control(&mut self, _now: Instant, f: &[u8]) {
            self.control_in.push(f.to_vec());
        }
        fn poll_transmit(&mut self, _now: Instant) -> Option<Vec<u8>> {
            self.to_send.pop_front()
        }
        fn next_deadline(&self) -> Option<Instant> {
            None
        }
        fn on_tick(&mut self, _now: Instant) {
            self.ticks += 1;
        }
        fn on_session_traffic(&mut self, _now: Instant, h: HubHandle, outbound: bool) {
            self.traffic.push((h, outbound));
        }
        fn on_socket_error(&mut self, _now: Instant) {
            self.socket_errors += 1;
        }
        fn is_bound(&self) -> bool {
            self.bound
        }
    }

    fn local() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 0))
    }

    fn sock() -> UdpSocket {
        let s = UdpSocket::bind(local()).expect("soket");
        s.set_read_timeout(Some(WAIT)).expect("zaman aşımı");
        s
    }

    fn recv(s: &UdpSocket) -> Option<(Vec<u8>, SocketAddr)> {
        let mut b = vec![0u8; RECV_BUF];
        s.recv_from(&mut b).ok().map(|(n, a)| (b[..n].to_vec(), a))
    }

    fn quiet(s: &UdpSocket) -> bool {
        s.set_read_timeout(Some(Duration::from_millis(200)))
            .expect("zaman aşımı");
        let r = recv(s).is_none();
        s.set_read_timeout(Some(WAIT)).expect("zaman aşımı");
        r
    }

    fn wait_until(mut f: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + WAIT;
        while Instant::now() < end {
            if f() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        f()
    }

    fn session_dgram(h: u32, frame_type: u8, tail: &[u8]) -> Vec<u8> {
        let mut v = h.to_be_bytes().to_vec();
        v.push(frame_type);
        v.extend_from_slice(tail);
        v
    }

    struct Rig {
        hub: UdpSocket,
        ep: HubEndpoint,
        crypto: Arc<FakeCrypto>,
        control: Arc<Mutex<FakeControl>>,
        server: UdpSocket,
    }

    fn rig() -> Rig {
        let hub = sock();
        let stash = Arc::new(Mutex::new(OpenedStash::new(StashConfig::default())));
        let crypto = FakeCrypto::new(stash);
        let control = Arc::new(Mutex::new(FakeControl {
            bound: true,
            ..FakeControl::default()
        }));
        let ctl: Arc<Mutex<dyn HubControl>> = control.clone();
        let known = KnownHandles::new();
        known.insert(HubHandle::from_raw(H1));
        known.insert(HubHandle::from_raw(H2));
        let ep = HubEndpoint::start(
            HubEndpointConfig {
                hub_addr: hub.local_addr().expect("adres"),
                hub_bind: local(),
                relay_bind: local(),
            },
            Box::new(Arc::clone(&crypto)),
            ctl,
            known,
        )
        .expect("başlat");
        let server = sock();
        ep.set_server_addr(server.local_addr().expect("adres"));
        Rig {
            hub,
            ep,
            crypto,
            control,
            server,
        }
    }

    impl Rig {
        fn client(&self) -> UdpSocket {
            let s = sock();
            s.connect(self.ep.relay_addr()).expect("connect");
            s
        }
        fn hub_send(&self, bytes: &[u8]) {
            self.hub
                .send_to(bytes, self.ep.hub_local_addr())
                .expect("hub gönderimi");
        }
    }

    /// T3: tablodaki her tip doğru role, diğer 246 tip `None`.
    #[test]
    fn role_table_maps_every_frame_type() {
        let client = [0x02, 0x04, 0x07, 0x10, 0x14];
        let server = [0x00, 0x01, 0x03, 0x05, 0x06];
        for t in 0..=u8::MAX {
            let want = if client.contains(&t) {
                Some(Role::Client)
            } else if server.contains(&t) {
                Some(Role::Server)
            } else {
                None
            };
            assert_eq!(role_of(t), want, "tip {t:#04x}");
        }
    }

    /// KK-H54 / Y5: 1281 B → `DatagramTooLarge`, tele çıkmaz; 1280 B gider (bayt aynen).
    #[test]
    fn oversize_is_a_typed_error_and_never_hits_the_wire() {
        let hub = sock();
        let tx = UdpSocket::bind(local()).expect("soket");
        let c = Atomics::default();
        let at = hub.local_addr().expect("adres");
        let big = vec![0xAB; MAX_DATAGRAM + 1];
        match send_to_hub(&tx, at, &big, &c) {
            Err(HubSendError::DatagramTooLarge { len }) => assert_eq!(len, MAX_DATAGRAM + 1),
            other => panic!("tipli hata bekleniyordu: {other:?}"),
        }
        assert!(quiet(&hub), "1281 B tele çıkmadı");
        let max = vec![0xCD; MAX_DATAGRAM];
        send_to_hub(&tx, at, &max, &c).expect("1280 B gider");
        let (got, _) = recv(&hub).expect("1280 B ulaştı");
        assert_eq!(got, max);
        assert_eq!((get(&c.hub_oversize_drops), get(&c.hub_tx)), (1, 1));
    }

    /// Giden: iki istemci ucu + veri alıcısı → hub tek kaynak adres görür; baytlar aynen (K2);
    /// istemci portu öğrenilir; gidiş HubControl'e bildirilir (KK-H58).
    #[test]
    fn outbound_has_one_source_address_and_bytes_are_untouched() {
        let r = rig();
        let (c1, c2) = (r.client(), r.client());
        let d1 = session_dgram(H1, FRAME_DATA, b"one");
        let d2 = session_dgram(H2, FRAME_DATA, b"two");
        let d3 = session_dgram(H1, FRAME_TYPE_ACK, b"ack");
        c1.send(&d1).expect("gönder");
        c2.send(&d2).expect("gönder");
        r.server.send_to(&d3, r.ep.relay_addr()).expect("gönder");
        let mut got = Vec::new();
        let mut srcs = HashSet::new();
        for _ in 0..3 {
            let (b, a) = recv(&r.hub).expect("hub aldı");
            got.push(b);
            srcs.insert(a);
        }
        got.sort();
        let mut want = vec![d1, d2, d3];
        want.sort();
        assert_eq!(got, want, "giden bayt = görevin yazdığı bayt");
        assert_eq!(srcs.len(), 1, "hub tek kaynak adres görür");
        let ports = lock(&r.ep.core.client_ports);
        assert_eq!(ports.get(H1), Some(c1.local_addr().expect("adres")));
        assert_eq!(ports.get(H2), Some(c2.local_addr().expect("adres")));
        drop(ports);
        let ctl = lock(&r.control);
        assert_eq!(ctl.traffic.iter().filter(|(_, out)| *out).count(), 3);
        drop(ctl);
        let n = r.ep.counters();
        assert_eq!((n.hub_tx, n.hub_out_unknown_handle), (3, 0));
    }

    /// Gelen: ilk 4 bayt dağıtımı, tek açma, rol, bilinmeyen tip; kaydın tüketimi.
    #[test]
    fn inbound_dispatch_opens_once_and_routes_by_frame_type() {
        let r = rig();
        let c1 = r.client();
        // İstemci portunu öğret.
        c1.send(&session_dgram(H1, FRAME_DATA, b"hi"))
            .expect("gönder");
        let _ = recv(&r.hub).expect("hub aldı");
        // Sunucu rolü: veri alıcısına.
        let data = session_dgram(H1, FRAME_DATA, b"peer-data");
        r.hub_send(&data);
        let (b, from) = recv(&r.server).expect("veri alıcısı aldı");
        assert_eq!((b.as_slice(), from), (data.as_slice(), r.ep.relay_addr()));
        // İstemci rolü: h'nin istemci ucuna.
        let ack = session_dgram(H1, FRAME_TYPE_ACK_V2, b"ack");
        r.hub_send(&ack);
        let (b, _) = recv(&c1).expect("istemci ucu aldı");
        assert_eq!(b, ack);
        // Tek açma: her datagram kriptoya bir kez gitti; kayıt kanaldan alınır.
        assert_eq!(r.crypto.opens_of(&data), 1);
        assert_eq!(r.crypto.opens_of(&ack), 1);
        let stash = r.crypto.stash();
        assert_eq!(lock(stash).take(&data).expect("kabul").frame[0], FRAME_DATA);
        assert_eq!(
            lock(stash).take(&ack).expect("kabul").frame[0],
            FRAME_TYPE_ACK_V2
        );
        // Bilinmeyen tip → düşer + sayaç, kanala girmez.
        let odd = session_dgram(H1, 0x99, b"?");
        r.hub_send(&odd);
        assert!(wait_until(|| r.ep.counters().hub_role_unknown == 1));
        assert!(quiet(&r.server) && quiet(&c1));
        // İstemci tipi ama H2 için port yok.
        r.hub_send(&session_dgram(H2, FRAME_CS_ECHO, b"e"));
        assert!(wait_until(|| r.ep.counters().hub_no_client_port == 1));
        // Kontrol çerçevesi: [0;4]‖type‖gövde‖0×16 → on_control (kuyruksuz çerçeve).
        let frame = [0, 0, 0, 0, 0x1C, 1, 2, 3, 4];
        let mut ctl = frame.to_vec();
        ctl.resize(frame.len() + TAG, 0);
        r.hub_send(&ctl);
        assert!(wait_until(|| lock(&r.control).control_in.len() == 1));
        assert_eq!(lock(&r.control).control_in[0], frame);
        // İlk 4 bayt ≠ 0 ama kriptoda bilinmeyen + sıfır kuyruk → HubChannel → bağlam uyumsuz.
        let mut forged = 0x0BAD_0000_u32.to_be_bytes().to_vec();
        forged.extend_from_slice(&[0x10; 3]);
        forged.resize(forged.len() + TAG, 0);
        r.hub_send(&forged);
        assert!(wait_until(|| r.ep.counters().hub_ctx_mismatch == 1));
        let n = r.ep.counters();
        assert_eq!((n.stash.inserted, n.stash.taken, n.stash.lost()), (2, 2, 0));
        // C9: gelen oturum datagramları HubControl'e bildirildi.
        assert!(lock(&r.control)
            .traffic
            .iter()
            .any(|(h, out)| h.raw() == H1 && !*out));
    }

    /// Tüketilmeyen kaydın zili yeniden çalınır; ikinci zil kriptoyu çağırmaz.
    #[test]
    fn unconsumed_record_is_rung_again_without_a_second_open() {
        let r = rig();
        let data = session_dgram(H1, FRAME_FEC, b"shard");
        r.hub_send(&data);
        let (b1, _) = recv(&r.server).expect("ilk zil");
        let (b2, _) = recv(&r.server).expect("yeniden zil");
        assert_eq!(
            (b1.as_slice(), b2.as_slice()),
            (data.as_slice(), data.as_slice())
        );
        assert_eq!(r.crypto.opens_of(&data), 1, "zil kriptoyu çağırmaz");
        assert!(lock(r.crypto.stash()).take(&data).is_some());
        assert!(r.ep.counters().stash.rings >= 1);
    }

    /// Hub adresi dışından gelen datagram düşer; bind yokken giden düşer (C2); bilinmeyen
    /// tutamaç sayılır ama gönderilir (KK-H66).
    #[test]
    fn foreign_source_unbound_and_unknown_handle_are_counted() {
        let r = rig();
        let stranger = sock();
        stranger
            .send_to(&session_dgram(H1, FRAME_DATA, b"x"), r.ep.hub_local_addr())
            .expect("gönder");
        assert!(wait_until(|| r.ep.counters().hub_rx_foreign == 1));
        assert!(quiet(&r.server));
        let c = r.client();
        c.send(&session_dgram(0x0000_0777, FRAME_DATA, b"u"))
            .expect("gönder");
        assert!(recv(&r.hub).is_some(), "bilinmeyen tutamaç yine gönderilir");
        assert_eq!(r.ep.counters().hub_out_unknown_handle, 1);
        lock(&r.control).bound = false;
        c.send(&session_dgram(H1, FRAME_DATA, b"v"))
            .expect("gönder");
        assert!(wait_until(|| r.ep.counters().hub_tx_unbound == 1));
        assert!(quiet(&r.hub));
        // h = 0 ve kısa datagram röleden geçmez.
        c.send(&[0, 0, 0, 0, 1]).expect("gönder");
        c.send(&[1, 2]).expect("gönder");
        assert!(wait_until(|| r.ep.counters().hub_relay_bad == 2));
    }

    /// Giden kontrol çerçevesi kriptonun HubChannel mührüyle (`‖ 0×16`) çıkar.
    #[test]
    fn control_frames_are_sealed_and_sent() {
        let r = rig();
        let frame = vec![0, 0, 0, 0, 0x1B];
        lock(&r.control).to_send.push_back(frame.clone());
        let (b, _) = recv(&r.hub).expect("hub aldı");
        let mut want = frame;
        want.resize(want.len() + TAG, 0);
        assert_eq!(b, want);
        assert!(wait_until(|| r.ep.counters().control_tx == 1));
    }

    /// Kanal doluyken gelen datagram AÇILMAZ.
    #[test]
    fn full_stash_drops_before_open() {
        let r = rig();
        let cap = StashConfig::default().per_handle_cap;
        let mut sent = Vec::new();
        for i in 0..=cap {
            let d = session_dgram(H1, FRAME_DATA, &(i as u32).to_be_bytes());
            r.hub_send(&d);
            sent.push(d);
        }
        assert!(wait_until(|| r.ep.counters().stash.backpressure == 1));
        let opened: usize = sent.iter().map(|d| r.crypto.opens_of(d)).sum();
        assert_eq!(opened, cap, "taşan datagram kriptoya gitmedi");
    }

    /// Drop iki döngüyü durdurur ve panik etmez.
    #[test]
    fn drop_stops_the_loops() {
        let r = rig();
        let relay = r.ep.relay_addr();
        drop(r.ep);
        let s = sock();
        s.send_to(&session_dgram(H1, FRAME_DATA, b"z"), relay)
            .expect("gönder");
        assert!(quiet(&r.hub));
    }

    struct ScriptLink(VecDeque<LinkEvent>);

    impl EnvelopeLink for ScriptLink {
        fn send_init(
            &mut self,
            _p: PeerRef,
            _t: u8,
            _b: Vec<u8>,
        ) -> Result<LinkAttempt, LinkError> {
            Ok(LinkAttempt(7))
        }
        fn send_response(
            &mut self,
            _p: PeerRef,
            _h: HubHandle,
            _t: u8,
            _b: Vec<u8>,
        ) -> Result<(), LinkError> {
            Err(LinkError)
        }
        fn poll(&mut self) -> Option<LinkEvent> {
            self.0.pop_front()
        }
    }

    /// KK-H66: `RecordingLink` davranışı değiştirmez, taşıyıcının handle'larını kaydeder.
    #[test]
    fn recording_link_is_transparent_and_records_handles() {
        let known = KnownHandles::new();
        let events = [
            LinkEvent::HandleAssigned {
                attempt: LinkAttempt(7),
                hub_handle: HubHandle::from_raw(11),
            },
            LinkEvent::IncomingInit {
                hub_handle: HubHandle::from_raw(12),
                envelope_type: 1,
                bytes: vec![1],
            },
            LinkEvent::Response {
                hub_handle: HubHandle::from_raw(13),
                envelope_type: 2,
                bytes: vec![2],
            },
        ];
        let mut link = RecordingLink::new(ScriptLink(events.clone().into()), known.clone());
        let peer = PeerRef::from_dest_hash([1; 32]);
        assert_eq!(link.send_init(peer, 1, vec![]), Ok(LinkAttempt(7)));
        assert_eq!(
            link.send_response(peer, HubHandle::from_raw(14), 2, vec![]),
            Err(LinkError)
        );
        let got: Vec<LinkEvent> = std::iter::from_fn(|| link.poll()).collect();
        assert_eq!(got, events.to_vec());
        for h in [11, 12, 13, 14] {
            assert!(known.contains(h), "{h}");
        }
        assert!(!known.contains(15));
        known.insert(HubHandle::from_raw(0));
        assert!(!known.contains(0), "0 hiçbir zaman handle değildir");
        for h in 100..(100 + KNOWN_HANDLE_CAP as u32 + 1) {
            known.insert(HubHandle::from_raw(h));
        }
        assert!(!known.contains(11), "tavan: en eski düşer");
    }
}
