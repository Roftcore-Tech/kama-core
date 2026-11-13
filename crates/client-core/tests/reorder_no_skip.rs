//! [F3 · sıra başı için yer koruma] Oturum sıralayıcısı eksik sıra başını ATLAMAZ; bellek veri
//! alıcısının kabul penceresiyle sınırlıdır.
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması; kabul penceresi genişliği =
//! `max_inflight_units`, testte [`WINDOW`] = 512 — gerekçe [`cfg`]'de; FEC kapalı), zarflar
//! `MemoryHub` üzerinden, veri UDP loopback. Bob'un kriptosunu saran test sarmalayıcısı oturumun İLK biriminin (`unit_id` 0) DATA'sını,
//! pencerenin dışından (kimliği ≥ [`WINDOW`]) [`SEEN_BEYOND`] DATA görene dek düşürür: sıra başı 0 iken
//! bu DATA'lar zorunlu olarak reddedilir (pencere her koşuda dolar; belirlenimli, hıza bağlı değil).
//! Sonra birim 0'ın yeniden iletimi geçer. Alice [`UNITS`] (> 4096) güvenilir birim gönderir.
//!
//! Gözlem (ölçüldü): gönderici eksik sıra başından ~4096 + 50 kimliğin ötesine geçmez ve sıra başı
//! eksikken yeniden iletimi en eski teyitsiz birime yoğunlaştırır; pencere dışı birimler sıra başı
//! geçince yeniden iletilir. Eski kodda tutulan birim bu yüzden 4096 tavanını meşru akışta aşar →
//! atlama gerçekleşir (F3).
//!
//! İddia: birim kaybı 0 — Bob 0..UNITS'i tam bir kez ve SIRAYLA alır; bellek sınırlı — tutulan
//! (sırası gelmemiş) birim hiçbir anda `genişlik − 1`'i aşmaz; pencere gerçekten devreye girdi —
//! `data_not_admitted_window > 0` (sıra başından pencere genişliği ve ötesindeki birim ACK'lenmedi,
//! gönderici sıra başı geçince yeniden iletti).
//!
//! Eski davranış (atlama): tutulan birim tavanı aşınca en küçük boşluk (birim 0) atlanır; birim 0
//! sonra gelince veri alıcısı onu ACK'ler ama sıralayıcı `unit < next` diye atar → ACK'lenmiş birim
//! kaybolur, akış 1'den başlar.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::memory_link::MemoryHub;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::wire::{decode_data, decode_data_frag};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit, TrafficClass,
    TransportError,
};
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit, SessionCommand, SessionControl, SessionEvent, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
/// Gönderilen güvenilir birim (> varsayılan pencere 4096 ve ≫ test penceresi).
const UNITS: u64 = 5000;
/// Kabul penceresi genişliği (test; bkz. [`cfg`]).
const WINDOW: u64 = 512;
/// Birim 0 serbest bırakılmadan önce görülmesi gereken pencere dışı DATA sayısı.
const SEEN_BEYOND: u64 = 16;
/// Beklenen olayın üst sınırı.
const WAIT: Duration = Duration::from_secs(120);

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn crypto(me: u8) -> NoiseWireProtection {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0
}

/// Gerçek kripto + alıcı yönünde oturumun `unit_id` 0 DATA'sı, kimliği ≥ `window` olan
/// [`SEEN_BEYOND`] DATA görülene dek düşer (S3 K5: `open` `None`). Başka hiçbir şeye dokunmaz.
struct DropHeadFor {
    inner: NoiseWireProtection,
    window: u64,
    beyond: u64,
    max_seen: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
}

impl PacketProtection for DropHeadFor {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let unit = decode_data(&od.frame)
            .map(|(_, u, _)| u)
            .or_else(|| decode_data_frag(&od.frame).map(|(_, u, _, _)| u));
        if let Some(u) = unit {
            self.max_seen.fetch_max(u, Ordering::Relaxed);
            if u >= self.window {
                self.beyond += 1;
            }
            if u == 0 && self.beyond < SEEN_BEYOND {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        }
        Some(od)
    }
    fn sealed_len(&self, ctx: ProtectionContext, n: usize) -> Result<usize, SealError> {
        self.inner.sealed_len(ctx, n)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        self.inner.conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        self.inner.limits(ctx)
    }
}

impl SessionControl for DropHeadFor {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        self.inner.command(cmd)
    }
    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.inner.poll_event()
    }
}

fn cfg() -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    // Pencere (= `max_inflight_units`) 512: mekanizma varsayılan 4096'yla aynıdır, ama pencere
    // sıra başının yeniden iletim bütçesi tükenmeden dolar. Ölçüm (varsayılan 4096, loopback):
    // gönderici eksik sıra başını ~1,4 s'de 255 kez yeniden iletir, sonra `arq_retx_cap = u8::MAX`
    // tavanı ARQ'yu bırakır (bir kopya, +6,5 s), sonra 180 s boyunca hiç — pencere o sürede ancak
    // ~4096'ya dolar. Bu motor bulgusu PR gövdesinde AÇIK olarak yazılı; bu test onu değil
    // sıralayıcıyı ve kabul penceresini sınar.
    cfg.runtime.max_inflight_units = WINDOW as usize;
    // [ŞERİT-ÜRÜN] Bu test ŞERİTSİZ yolun sıralayıcısını ve kabul penceresini sınar (birim
    // kimlikleri etiketsiz; `unit_id` 0 = sıra başı). Şeritler açıkken (ürün profili) dosya
    // şeridinin uca verdiği teyitsiz birim sayısı W ile sınırlıdır (`lanes::file_window_for_path`,
    // ≤ 1024 < kabul penceresi): alıcı penceresinin dolduğu bu senaryo orada yapısal olarak
    // oluşmaz; şerit yolunun bloklamama kanıtı `lane_no_hol_block.rs`.
    cfg.lanes = None;
    cfg
}

#[test]
fn missing_head_is_never_skipped_and_held_units_stay_bounded() {
    let window = cfg().runtime.max_inflight_units as u64;
    assert_eq!(window, WINDOW);
    assert!(UNITS > window, "test ön koşulu: pencereden fazla birim");
    let hub = MemoryHub::new();
    let dropped = Arc::new(AtomicU64::new(0));
    let max_seen = Arc::new(AtomicU64::new(0));
    let mut alice =
        ClientTransport::new(cfg(), crypto(ALICE), Box::new(hub.link(id(ALICE)))).expect("alice");
    let mut bob = ClientTransport::new(
        cfg(),
        DropHeadFor {
            inner: crypto(BOB),
            window,
            beyond: 0,
            max_seen: Arc::clone(&max_seen),
            dropped: Arc::clone(&dropped),
        },
        Box::new(hub.link(id(BOB))),
    )
    .expect("bob");
    alice.add_peer_addr(id(BOB), bob.data_addr());
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    let deadline = Instant::now() + WAIT;
    let h: SessionHandle = loop {
        hub.relay();
        while alice.poll_event().is_some() {}
        while bob.poll_event().is_some() {}
        if let (Some(h), Some(_)) = (
            alice.active_session_for_test(id(BOB)),
            bob.active_session_for_test(id(ALICE)),
        ) {
            break h;
        }
        assert!(Instant::now() < deadline, "oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(2));
    };

    let started = Instant::now();
    let unit = |n: u64| {
        let mut bytes = n.to_be_bytes().to_vec();
        bytes.resize(64, 0);
        PreparedUnit {
            bytes,
            compression_policy: CompressionPolicy::Never,
        }
    };
    let mut next_send: u64 = 0;
    let mut got: Vec<u64> = Vec::with_capacity(UNITS as usize);
    let mut max_held: u64 = 0;
    let mut window_refused_seen = 0;
    while (got.len() as u64) < UNITS {
        // Gönderici kuyruğu doluysa (`Backpressure`) pompalanır, sonra yeniden denenir.
        while next_send < UNITS {
            match alice.send(
                h,
                unit(next_send),
                DeliveryPolicy::Reliable,
                TrafficClass::Bk,
            ) {
                Ok(_) => next_send += 1,
                Err(TransportError::Backpressure) => break,
                Err(e) => panic!("send: {e:?}"),
            }
        }
        while alice.poll_event().is_some() {}
        while let Some(d) = bob.recv() {
            got.push(u64::from_be_bytes(d.unit[..8].try_into().expect("8 B")));
        }
        while bob.poll_event().is_some() {}
        for (_, diag) in bob.reorder_diags() {
            max_held = max_held.max(u64::from(diag.held_units));
        }
        window_refused_seen = window_refused_seen.max(bob.data_not_admitted_window());
        assert!(
            Instant::now() < deadline,
            "üst süre içinde bütün birimler teslim edilmeli: {}/{UNITS}, gönderilen {next_send}, \
             ilk {:?}, Bob'un gördüğü en büyük kimlik {}, en çok tutulan {max_held}",
            got.len(),
            got.first(),
            max_seen.load(Ordering::Relaxed)
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let elapsed = started.elapsed();
    eprintln!(
        "ÖLÇÜM reorder-no-skip: birim {UNITS} · pencere {window} · teslim {} · süre {:.2} s · \
         birim-0 düşen emisyon {} · en çok tutulan {max_held} · Bob'un gördüğü en büyük kimlik {} · \
         pencere dışı reddedilen DATA {} · dolu kanal reddi {}",
        got.len(),
        elapsed.as_secs_f64(),
        dropped.load(Ordering::Relaxed),
        max_seen.load(Ordering::Relaxed),
        window_refused_seen,
        bob.data_not_admitted_full()
    );
    assert_eq!(
        got,
        (0..UNITS).collect::<Vec<_>>(),
        "kayıp yok, sıra doğru, tekrar yok"
    );
    assert!(
        dropped.load(Ordering::Relaxed) > 0,
        "ön koşul: sıra başı gerçekten eksik kaldı"
    );
    assert!(
        max_held < window,
        "bellek sınırlı: tutulan birim {max_held} ≥ pencere {window}"
    );
    assert!(
        window_refused_seen > 0,
        "pencere devreye girdi (sıra başından {window} ötesi ACK'lenmedi)"
    );
    assert_eq!(bob.delivery_dropped(), 0);
}
