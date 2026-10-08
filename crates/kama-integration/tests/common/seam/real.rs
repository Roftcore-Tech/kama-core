//! **real** hedefi — S1 sözleşme süitinin GERÇEK implementasyona karşı koşan hedefi (L-T1).
//!
//! Hedef = `client_core::transport::ClientTransport` (ürün builder'ı `new`, ürün veri profili) +
//! gerçek kripto (`NoiseWireProtection`, ürün kurulumu `with_control`; TEST anahtarları
//! `kama_noise_wire::seam_mock::fixture_*`) + gerçek veri yolu (UDP loopback). Karşı uçlar aynı
//! adaptörün başka örnekleridir: hedef bir eşe ilk kez `connect` ettiğinde o eşin düğümü kurulur
//! (kimliği eşin `dest_hash`'i) ve aldığı her birimi AYNI oturumda geri yollar (echo — satırların
//! `send → recv` biçimi; `seam-coverage.json` "peer echo fixture"). Zarflar
//! `client_core::memory_link::MemoryHub` üzerinden ve YALNIZ `pump`'ta taşınır: bekleyen bir niyet
//! test onu sürmedikçe çözülmez.
//!
//! # Kancaların gerçek karşılıkları
//!
//! * `pump` — hub rölesi, karşı uçların sürülmesi (olaylar + echo), hedef saatinin
//!   [`PUMP_CLOCK_STEP`] ilerlemesi, kısa gerçek bekleme (UDP/tokio ilerlesin). Saat adımı,
//!   yalnız-alım drain'inin (`T_g` = 30 s) `PUMP_BUDGET` içinde bitmesini sağlar.
//! * `require_renew` — hedef saati `SESSION_MAX_AGE` ilerler; Crypto `RenewRequired(h)` üretir,
//!   adaptör arka planda yeniler.
//! * `complete_renew` — hedefin yenilemesi bitene (eşin aktif oturumu değişene) dek pump.
//! * `inject_peer_supersession` — KARŞI UCUN saati `SESSION_MAX_AGE` ilerler → karşı uç yeniler →
//!   hedefte provisional supersession; karşı ucun yeni veri ucunun ilk çerçevesi (PARAM_SET,
//!   epoch-1) onaylar → hedefte `Ready(h')`.
//! * `inject_invalid_response` — hedefin gelen kutusuna sahte SSR1; Crypto opak `Failed` döner.
//! * `advance_clock` — hedef saati.
//! * `set_recovery` / `drop_next_units` — adaptörün test kancaları: kurtarma kapalıyken ARQ
//!   retransmit/kopya tele çıkmaz; kaybettirilen ilk emisyonun tel baytı bozulur (alıcı AEAD'i
//!   reddeder).
//! * `set_link` / `probe_emissions` — GERÇEK KARŞILIĞI YOK: `Suspended`/`LongOutage` tetikleri ve
//!   hub prob'u bu sprint KAPSAM DIŞI (iş emri §4, PARK P-4). Tek kullanıcıları H-PROGRESS'tir ve o
//!   satır bu hedefte BEYANLI BOŞLUKTUR ([`RealTarget::ROW_GAPS`]); çağrılırlarsa panik.
//!
//! # `recv()` — veri yolunun durulması
//!
//! Sözleşme gövdeleri senkron bir sahteye göre yazıldı (gönderim anında teslim). Gerçek veri yolu
//! asenkrondur: harness'in `recv()`'i önce yolu DURULTUR — teslim gelene, yol durgunlaşana (hiçbir
//! uçta teyitsiz birim ve bekleyen echo yok) ya da ilerleme [`SETTLE_STALL`] boyunca durana dek
//! bekler. Durultma zarf TAŞIMAZ: bekleyen bir niyet `recv` ile çözülmez.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use client_core::memory_link::MemoryHub;
use client_core::transport::{ClientTransport, ClientTransportConfig, LinkEvent, PENDING_INIT_CAP};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport,
    PreparedUnit, SessionEvent, TransportCapabilities, TransportError,
};
use kama_contracts::s3::HubHandle;
use kama_noise_wire::clock::ManualClock;
use kama_noise_wire::peer_dir::PeerDirectory;
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::{EnvelopeKind, ENVELOPE_INIT_RESPONSE, SESSION_MAX_AGE};

use super::target::{RowGap, SeamTarget, TargetKind, TargetUnavailable};

type Node = ClientTransport<NoiseWireProtection>;

/// Hedef düğümün kimliği — satırların andığı eşlerin hiçbiriyle çakışmaz.
const TARGET_ID: u64 = 0x7A12_0001;
/// Satırların andığı eşler: `test_peer(0x00..=0xFF)` ve H-PENDING-CAP'in tabanı.
const SMALL_PEERS: core::ops::RangeInclusive<u64> = 0x00..=0xFF;
const PENDING_CAP_PEERS: core::ops::Range<u64> = 0x0050_0000..0x0050_0040;

/// `pump` başına hedef saati adımı (yalnız-alım drain'i `T_g` = 30 s → ~30 pump).
pub const PUMP_CLOCK_STEP: Duration = Duration::from_secs(1);
/// `pump` başına gerçek bekleme (UDP + tokio görevleri ilerlesin).
const PUMP_SLEEP: Duration = Duration::from_millis(2);
/// `recv` durultması: ilerleme bu süre durursa bekleme biter.
pub const SETTLE_STALL: Duration = Duration::from_millis(100);
/// `recv` durultmasının mutlak tavanı.
const SETTLE_CAP: Duration = Duration::from_secs(2);
const SETTLE_TICK: Duration = Duration::from_millis(1);
/// Yenileme / supersession kancalarının mutlak tavanı (kurtarma RTO'su gerçek zamanlıdır).
const HOOK_CAP: Duration = Duration::from_secs(15);
/// Sahte SSR1'in boyu (gerçek bir SSR1 kadar; içerik anlamsız).
const FAKE_SSR1_LEN: usize = 160;

fn peer(n: u64) -> PeerRef {
    super::test_peer(n)
}

fn fixture_peers() -> impl Iterator<Item = u64> {
    SMALL_PEERS
        .chain(PENDING_CAP_PEERS)
        .chain(core::iter::once(TARGET_ID))
}

fn is_fixture_peer(p: PeerRef) -> bool {
    fixture_peers().any(|n| peer(n) == p)
}

/// G1 eş dizini: fikstürün bütün kimlikleri (bir kez kurulur, düğüm başına kopyalanır).
fn directory() -> PeerDirectory {
    static DIR: OnceLock<PeerDirectory> = OnceLock::new();
    DIR.get_or_init(|| {
        fixture_directory(fixture_peers().map(|n| *peer(n).as_bytes()))
            .expect("fikstür dizini kurulmalı")
    })
    .clone()
}

/// Bir düğüm: ürün builder'ı + ürün veri profili (kapanış boşaltması kısaltılmış).
fn node(id: PeerRef, hub: &MemoryHub) -> Result<(Node, ManualClock), TargetUnavailable> {
    let (crypto, clock) = fixture_protection(*id.as_bytes(), directory(), EnvelopeKind::Legacy);
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.renew_drain_cap = Duration::from_secs(5);
    let node = ClientTransport::new(cfg, crypto, Box::new(hub.link(id)))
        .map_err(|_| TargetUnavailable::Environment("düğüm kurulamadı (soket/çalışma-zamanı)"))?;
    Ok((node, clock))
}

struct Counterparty {
    node: Node,
    clock: ManualClock,
}

/// Hedef + karşı uçlar + zarf hub'ı (bkz. modül belgesi).
pub struct RealHarness {
    me: PeerRef,
    t: Node,
    clock: ManualClock,
    hub: MemoryHub,
    peers: BTreeMap<PeerRef, Counterparty>,
}

impl RealHarness {
    fn new() -> Result<Self, TargetUnavailable> {
        let hub = MemoryHub::new();
        let me = peer(TARGET_ID);
        let (t, clock) = node(me, &hub)?;
        Ok(Self {
            me,
            t,
            clock,
            hub,
            peers: BTreeMap::new(),
        })
    }

    /// Eşin düğümü yoksa kurar ve iki yönde veri adreslerini kaydeder (fikstür dışı eşe dokunmaz:
    /// onun `connect`'i dizinde olmadığı için kalıcı yerel hatayla biter).
    fn ensure_peer(&mut self, p: PeerRef) {
        if self.peers.contains_key(&p) || !is_fixture_peer(p) || p == self.me {
            return;
        }
        let Ok((mut cp, clock)) = node(p, &self.hub) else {
            return;
        };
        cp.add_peer_addr(self.me, self.t.data_addr());
        self.t.add_peer_addr(p, cp.data_addr());
        self.peers.insert(p, Counterparty { node: cp, clock });
    }

    /// Karşı uçları sürer: olaylarını tüketir, aldıkları her birimi aynı oturumda geri yollar.
    /// Geri yollanan birim sayısını döner.
    fn drive_peers(&mut self) -> usize {
        let mut echoed = 0;
        for cp in self.peers.values_mut() {
            while cp.node.poll_event().is_some() {}
            while let Some(d) = cp.node.recv() {
                let unit = PreparedUnit {
                    bytes: d.unit,
                    compression_policy: CompressionPolicy::Never,
                };
                let _ = cp.node.send(d.session, unit, DeliveryPolicy::Reliable);
                echoed += 1;
            }
        }
        echoed
    }

    fn peers_outstanding(&self) -> usize {
        self.peers
            .values()
            .map(|cp| cp.node.outstanding_for_test())
            .sum()
    }

    fn pump(&mut self) {
        self.hub.relay();
        self.drive_peers();
        self.clock.advance(PUMP_CLOCK_STEP);
        std::thread::sleep(PUMP_SLEEP);
    }

    /// Durultmalı `recv` (bkz. modül belgesi).
    fn settle_recv(&mut self) -> Option<Delivered> {
        let start = Instant::now();
        let mut last = None;
        let mut last_change = Instant::now();
        loop {
            if let Some(d) = self.t.recv() {
                return Some(d);
            }
            // Sayaçlar echo'dan ÖNCE okunur: teyit gelmişse birim karşı ucun kanalındadır ve
            // aşağıdaki sürüş onu geri yollar (aksi hâlde durgunluk erken ilan edilebilirdi).
            let sig = (self.t.outstanding_for_test(), self.peers_outstanding());
            let echoed = self.drive_peers();
            if echoed == 0 && sig == (0, 0) {
                return self.t.recv();
            }
            if echoed > 0 || last != Some(sig) {
                last = Some(sig);
                last_change = Instant::now();
            }
            if last_change.elapsed() >= SETTLE_STALL || start.elapsed() >= SETTLE_CAP {
                return self.t.recv();
            }
            std::thread::sleep(SETTLE_TICK);
        }
    }

    /// `peer`'in aktif oturumu `old` olmaktan çıkana (yenisi aktif olana) dek pump.
    fn pump_until_replaced(&mut self, p: PeerRef, old: SessionHandle) {
        let start = Instant::now();
        while start.elapsed() < HOOK_CAP {
            self.pump();
            self.t.drive();
            if self.t.active_session_for_test(p).is_some_and(|h| h != old) {
                return;
            }
        }
    }
}

impl KamaTransport for RealHarness {
    fn capabilities(&self) -> TransportCapabilities {
        self.t.capabilities()
    }

    fn connect(
        &mut self,
        peer: PeerRef,
        early_unit: Option<PreparedUnit>,
    ) -> Result<ConnectResult, TransportError> {
        self.ensure_peer(peer);
        self.t.connect(peer, early_unit)
    }

    fn send(
        &mut self,
        session: SessionHandle,
        unit: PreparedUnit,
        policy: DeliveryPolicy,
    ) -> Result<(), TransportError> {
        self.t.send(session, unit, policy)
    }

    fn recv(&mut self) -> Option<Delivered> {
        self.settle_recv()
    }

    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.t.poll_event()
    }

    fn close(&mut self, session: SessionHandle) -> Result<(), TransportError> {
        self.t.close(session)
    }

    fn cancel(&mut self, ticket: ConnectTicket) {
        self.t.cancel(ticket);
    }
}

/// Gerçek hedef kaydı.
pub struct RealTarget;

impl SeamTarget for RealTarget {
    type Transport = RealHarness;
    const ID: &'static str = "real";
    const KIND: TargetKind = TargetKind::Real;
    const PENDING_INIT_CAP: usize = PENDING_INIT_CAP;
    /// Ürün builder'ı kurtarma kapalı yapılandırmayı reddeder; hedef ürün builder'ıyla kurulur.
    const RELIABLE: bool = true;
    const ROW_GAPS: &'static [RowGap] = &[RowGap {
        row: "H-PROGRESS",
        gap: "GAP-S1-REAL-PROGRESS",
        why: "Suspended/LongOutage tetikleri ve hub prob'u bu sprint KAPSAM DIŞI (iş emri §4, \
              PARK P-4); ürün Transport'u yalnız Measuring üretir — satırın Suspended yarısının \
              ve prob sayacının gerçek karşılığı yok",
    }];

    fn build() -> Result<Self::Transport, TargetUnavailable> {
        RealHarness::new()
    }

    fn pump(transport: &mut Self::Transport) {
        transport.pump();
    }

    fn require_renew(transport: &mut Self::Transport, _session: SessionHandle) {
        transport.clock.advance(SESSION_MAX_AGE);
    }

    fn complete_renew(transport: &mut Self::Transport, old: SessionHandle) {
        let Some(p) = transport.t.peer_of_for_test(old) else {
            return;
        };
        transport.pump_until_replaced(p, old);
    }

    fn inject_peer_supersession(transport: &mut Self::Transport, old: SessionHandle) {
        let Some(p) = transport.t.peer_of_for_test(old) else {
            return;
        };
        if let Some(cp) = transport.peers.get(&p) {
            cp.clock.advance(SESSION_MAX_AGE);
        }
        transport.pump_until_replaced(p, old);
    }

    fn inject_invalid_response(transport: &mut Self::Transport, _peer: PeerRef) {
        transport.hub.inject(
            transport.me,
            LinkEvent::Response {
                hub_handle: HubHandle::from_raw(0x00DE_AD00),
                envelope_type: ENVELOPE_INIT_RESPONSE,
                bytes: vec![0xA5; FAKE_SSR1_LEN],
            },
        );
    }

    fn set_link(_transport: &mut Self::Transport, _up: bool) {
        panic!("real hedefte link kancasının karşılığı yok (H-PROGRESS beyanlı boşluk)");
    }

    fn advance_clock(transport: &mut Self::Transport, by: Duration) {
        transport.clock.advance(by);
    }

    fn probe_emissions(_transport: &Self::Transport) -> u64 {
        panic!("real hedefte hub prob sayacı yok (H-PROGRESS beyanlı boşluk)");
    }

    fn set_recovery(transport: &mut Self::Transport, on: bool) {
        transport.t.set_recovery_for_test(on);
    }

    fn drop_next_units(transport: &mut Self::Transport, n: usize) {
        transport.t.drop_next_units_for_test(n);
    }
}
