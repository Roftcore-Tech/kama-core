//! Kama Hub — `hub-server` (sözleşme `KM-SEAM-TH-001` = S4 v1.3.1; tasarım ve testler `KM-HUB-TDD-001`
//! v0.6.1).
//!
//! Tek UDP soketi, tek iş parçacığı, bellekte birkaç tablo. Gelen her datagram için: ilk 4 bayt
//! sıfırsa **kontrol** (bind / zarf rölesi / heartbeat), değilse **veri** (bir hattın paketi; yükü
//! okumadan karşı uca at). Hub zarf açmaz, mesaj saklamaz, "koptu" kararı vermez; tabloları yalnız gelen
//! mesajlar değiştirir.
//!
//! Bu kütüphane IO'suz çekirdektir: [`Hub::on_datagram`] bir datagram alır, çıkışını [`Sink`]'e yazar;
//! [`Hub::on_tick`] tombstone süresi, kilit tablosu, cookie yenileme ve `hub stats` satırını yürütür
//! (datagram YOLLAMAZ). Saat ([`Clock`]) ve rastgelelik (`kama_noise_wire::entropy::EntropySource`)
//! dışarıdan verilir. İkili (`src/main.rs`) soket döngüsünü ve `probe` alt komutunu taşır.
//!
//! | modül | iş |
//! |---|---|
//! | [`config`] | `hub.json` v1, S4 §6 sabitleri |
//! | [`wire`] | S4 §2 codec'i ve bayt birleştiricileri (`kama-contracts::s4` gelene dek) |
//! | [`bind`] | cookie[`bind_epoch`], imzalar, kayıp-ACK tekrarı, adres kilidi; CLIENT_BIND / BIND_PROOF / HEARTBEAT |
//! | [`table`] | uçlar, hatlar, tombstone'lar, tahsis anahtarı indeksleri, emeklilik planlayıcısı |
//! | [`relay`] | STORE_MESSAGE, DELIVER, veri iletimi, ROUTE_MISS |
//! | [`rate`] | token bucket |
//! | [`stats`] | sayaçlar, log satırları |
//! | [`probe`] | `hub-server probe` istemcisi |
//!
//! K8 / H12 (hiçbir girdi hub'ı düşürmez) derleme zamanında da tutulur: ürün kodunda indeksleme,
//! `unwrap`/`expect` ve `panic!` aşağıdaki `deny` listesiyle yasaktır.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

pub mod bind;
pub mod config;
pub mod probe;
pub mod rate;
pub mod relay;
pub mod stats;
pub mod table;
pub mod wire;

use core::fmt;
use core::time::Duration;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use kama_noise_wire::entropy::EntropySource;

use crate::bind::{BindFailTable, CookieJar, HubIdentity};
use crate::config::{Config, Device, Limits};
use crate::rate::TokenBucket;
use crate::stats::{RateWindow, Sizes, Stats};
use crate::table::Table;
use crate::wire::{
    classify, flags, frame, method, CertRef, Datagram, DestHash, ErrorPayload, Payload, RpcFrame,
    DATA_MIN_LEN, PUBLIC_KEY_LEN,
};

/// Monoton saat: keyfi bir başlangıçtan (hub açılışı) bu yana geçen süre; asla geri gitmez. Cookie
/// dakikası, hız kovaları, tombstone süresi ve kilit süresi yalnız bundan okunur.
pub trait Clock {
    fn now(&self) -> Duration;
}

/// Üretim saati (`Instant`).
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// Rastgelelik kaynağı kullanılamıyor (açılışta sır üretilemedi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntropyUnavailable;

impl fmt::Display for EntropyUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("entropy source unavailable")
    }
}

impl std::error::Error for EntropyUnavailable {}

/// Hub'ın çıkışı: datagramlar ve log satırları.
pub trait Sink {
    /// Yollanamazsa düşer; kuyruk yoktur.
    fn send(&mut self, to: SocketAddr, datagram: &[u8]);
    /// Bir log satırı (satır sonu olmadan).
    fn log(&mut self, line: fmt::Arguments<'_>);
}

/// Dizin (H2): `hub.json`'dan, değişmez.
#[derive(Debug)]
pub struct Directory {
    devices: Vec<Device>,
    by_cert: HashMap<CertRef, usize>,
    by_dest: HashMap<DestHash, usize>,
}

impl Directory {
    fn new(devices: Vec<Device>) -> Self {
        let by_cert = devices
            .iter()
            .enumerate()
            .map(|(i, d)| (d.cert_ref, i))
            .collect();
        let by_dest = devices
            .iter()
            .enumerate()
            .map(|(i, d)| (d.dest_hash, i))
            .collect();
        Self {
            devices,
            by_cert,
            by_dest,
        }
    }

    #[must_use]
    pub fn by_cert_ref(&self, cert_ref: &CertRef) -> Option<&Device> {
        self.devices.get(*self.by_cert.get(cert_ref)?)
    }

    #[must_use]
    pub fn by_dest_hash(&self, dest_hash: &DestHash) -> Option<&Device> {
        self.devices.get(*self.by_dest.get(dest_hash)?)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

/// Hub çekirdeği.
pub struct Hub<C, R> {
    pub(crate) clock: C,
    pub(crate) rng: R,
    pub(crate) limits: Limits,
    pub(crate) identity: HubIdentity,
    pub(crate) directory: Directory,
    pub(crate) cookies: CookieJar,
    pub(crate) bind_fail: BindFailTable,
    pub(crate) verify_budget: TokenBucket,
    pub(crate) table: Table,
    pub(crate) stats: Stats,
    window: RateWindow,
    last_sweep: Duration,
    /// Gönderim tamponları; paket başına bellek ayrılmaz.
    pub(crate) tx: Vec<u8>,
    pub(crate) payload_buffer: Vec<u8>,
}

impl<C: Clock, R: EntropySource> Hub<C, R> {
    /// S4 §6 sabitleriyle hub.
    ///
    /// # Errors
    /// [`EntropyUnavailable`] — cookie / handle sırrı üretilemedi.
    pub fn new(config: Config, clock: C, rng: R) -> Result<Self, EntropyUnavailable> {
        Self::with_limits(config, Limits::S4, clock, rng)
    }

    /// Başka sınırlarla hub — YALNIZ test ve ölçüm içindir; üretim [`Self::new`] ile koşar.
    ///
    /// # Errors
    /// [`EntropyUnavailable`].
    pub fn with_limits(
        config: Config,
        limits: Limits,
        clock: C,
        mut rng: R,
    ) -> Result<Self, EntropyUnavailable> {
        let now = clock.now();
        let cookie_secret = bind::secret(&mut rng)?;
        let handle_secret = bind::secret(&mut rng)?;
        Ok(Self {
            identity: HubIdentity::new(&config.hub_secret),
            directory: Directory::new(config.devices),
            cookies: CookieJar::new(cookie_secret, now),
            bind_fail: BindFailTable::default(),
            verify_budget: TokenBucket::full(limits.verify_budget, now),
            table: Table::new(limits, handle_secret),
            stats: Stats::default(),
            window: RateWindow::starting_at(now),
            last_sweep: now,
            tx: Vec::with_capacity(limits.max_datagram),
            payload_buffer: Vec::with_capacity(limits.max_datagram),
            limits,
            clock,
            rng,
        })
    }

    /// Bir datagramı işler (H1). Çıkış yalnız kaynağa cevap ya da bağlı bir uca iletimdir (H10).
    pub fn on_datagram(&mut self, src: SocketAddr, datagram: &[u8], sink: &mut dyn Sink) {
        stats::bump(&mut self.stats.rx);
        if datagram.len() > self.limits.max_datagram || datagram.len() < DATA_MIN_LEN {
            stats::bump(&mut self.stats.drop_size);
            return;
        }
        let now = self.clock.now();
        match classify(datagram) {
            Ok(Datagram::Data { handle }) => self.on_data(src, handle.get(), datagram, now, sink),
            Ok(Datagram::Control { frame_type, body }) => match frame_type {
                frame::HEARTBEAT => self.on_heartbeat(src, body, now, sink),
                frame::PLANE1_RPC => self.on_rpc(src, body, now, sink),
                _ => stats::bump(&mut self.stats.drop_unknown_frame),
            },
            Err(_) => stats::bump(&mut self.stats.drop_malformed),
        }
    }

    fn on_rpc(&mut self, src: SocketAddr, body: &[u8], now: Duration, sink: &mut dyn Sink) {
        let Ok(request) = RpcFrame::decode(body) else {
            stats::bump(&mut self.stats.drop_malformed);
            return;
        };
        // Telefondan hub'a yalnız istek gelir.
        if request.flags != flags::REQUEST || request.method_id & method::HUB_INITIATED != 0 {
            stats::bump(&mut self.stats.drop_malformed);
            return;
        }
        match request.method_id {
            method::CLIENT_BIND => self.on_bind1(src, &request, now, sink),
            method::BIND_PROOF => self.on_bind2(src, &request, now, sink),
            method::STORE_MESSAGE => self.on_store(src, &request, now, sink),
            _ => stats::bump(&mut self.stats.drop_unknown_method),
        }
    }

    /// Zamanlayıcı: süresi dolan tombstone ve kilit girdileri (`sweep_period`'da bir), cookie
    /// yenileme, `hub stats` (`STATS_PERIOD`'da bir). Sık çağrılabilir. Datagram YOLLAMAZ (H10).
    pub fn on_tick(&mut self, sink: &mut dyn Sink) {
        let now = self.clock.now();
        if now.saturating_sub(self.last_sweep) >= self.limits.sweep_period {
            self.table.sweep(now);
            self.bind_fail.sweep(now, &self.limits);
            self.last_sweep = now;
        }
        if self
            .cookies
            .rotate_if_due(now, self.limits.cookie_rotate, &mut self.rng)
        {
            stats::log_cookie_rotate(sink);
        }
        if self.window.due(now, self.limits.stats_period) {
            let sizes = Sizes {
                endpoints: self.table.endpoint_count(),
                links: self.table.link_count(),
                tombstones: self.table.tombstone_count(),
                bind_fail_entries: self.bind_fail.len(),
            };
            self.window.emit(sink, now, &self.stats, sizes);
        }
    }

    // ── Gözlem (testler ve probe) ─────────────────────────────────────────────────────────────

    #[must_use]
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    #[must_use]
    pub fn table(&self) -> &Table {
        &self.table
    }

    #[must_use]
    pub fn hub_pub(&self) -> &[u8; PUBLIC_KEY_LEN] {
        self.identity.public()
    }

    #[must_use]
    pub fn bind_fail_entries(&self) -> usize {
        self.bind_fail.len()
    }

    /// `addr` bağlı bir uç adresi mi.
    #[must_use]
    pub fn is_registered_addr(&self, addr: SocketAddr) -> bool {
        self.table.key_at(addr).is_some()
    }

    // ── Gönderim yardımcıları ─────────────────────────────────────────────────────────────────

    /// İsteğe yanıt (aynı `method_id`, aynı `request_id`, `flags = 0x01`).
    pub(crate) fn reply_rpc<'p, P: Payload<'p>>(
        &mut self,
        sink: &mut dyn Sink,
        src: SocketAddr,
        request: &RpcFrame<'_>,
        payload: &P,
    ) {
        self.reply_frame(
            sink,
            src,
            request.method_id,
            flags::RESPONSE,
            request.request_id,
            payload,
        );
    }

    /// Hata çerçevesi (`flags = 0x03`, `ErrorPayload{code, 0}`).
    pub(crate) fn reply_error(
        &mut self,
        sink: &mut dyn Sink,
        src: SocketAddr,
        request: &RpcFrame<'_>,
        code: u16,
    ) {
        let payload = ErrorPayload { code };
        self.reply_frame(
            sink,
            src,
            request.method_id,
            flags::RESPONSE_ERROR,
            request.request_id,
            &payload,
        );
    }

    fn reply_frame<'p, P: Payload<'p>>(
        &mut self,
        sink: &mut dyn Sink,
        src: SocketAddr,
        method_id: u8,
        flags: u8,
        request_id: u32,
        payload: &P,
    ) {
        let mut body = core::mem::take(&mut self.payload_buffer);
        body.clear();
        payload.write(&mut body);
        let frame = RpcFrame {
            method_id,
            flags,
            request_id,
            payload: &body,
        };
        let mut tx = core::mem::take(&mut self.tx);
        if frame.encode(&mut tx).is_ok() {
            sink.send(src, &tx);
            stats::bump(&mut self.stats.tx_reply);
        }
        self.tx = tx;
        self.payload_buffer = body;
    }

    /// Yanıt datagramını yeni bir vektöre kodlar (bind ACK'i `last_accepted`'ta saklanır).
    pub(crate) fn encode_rpc<'p, P: Payload<'p>>(
        &self,
        request: &RpcFrame<'_>,
        flags: u8,
        payload: &P,
    ) -> Option<Vec<u8>> {
        let body = payload.to_vec();
        let mut out = Vec::new();
        RpcFrame {
            method_id: request.method_id,
            flags,
            request_id: request.request_id,
            payload: &body,
        }
        .encode(&mut out)
        .ok()?;
        Some(out)
    }

    /// HubChannel kontrol cevabı (HEARTBEAT_ACK).
    pub(crate) fn reply_control(
        &mut self,
        sink: &mut dyn Sink,
        src: SocketAddr,
        frame_type: u8,
        body: &[u8],
    ) {
        let mut tx = core::mem::take(&mut self.tx);
        wire::control_datagram(&mut tx, frame_type, body);
        sink.send(src, &tx);
        stats::bump(&mut self.stats.tx_reply);
        self.tx = tx;
    }
}
