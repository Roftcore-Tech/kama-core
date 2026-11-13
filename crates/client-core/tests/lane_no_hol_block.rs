//! [ŞERİT-SIRA] Akışlar birbirini bloklamaz: dosyanın kayıp sıra başı, aynı oturumda sonradan
//! gönderilen METİN ve PTT birimlerinin uygulamaya teslimini BEKLETMEZ.
//!
//! S1 (D-09): sıralı teslim aynı oturum ve aynı sınıf içindedir; sınıflar arası sıra garantisi
//! yoktur. Eski alıcı oturumun bütün birimlerini tek sırada teslim ediyordu: dosya birimi 0
//! gelmeden metin de PTT de verilmiyordu (ürün hedefi "akışlar birbirini bloklamaz" ihlali).
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması + şeritler açık, FEC kapalı — kayıp
//! parça FEC ile değil ARQ ile gelir ki bloklama penceresi belirlenimli olsun), zarflar `MemoryHub`,
//! veri UDP loopback. Bob'un kriptosunu saran sarmalayıcı oturumun İLK DATA birimini (dosya
//! şeridinin sıra başı) test "serbest" diyene dek düşürür (S3 K5: `open` → `None`). Alice önce
//! [`FILE_UNITS`] dosya birimi, sonra bir metin ve bir PTT birimi gönderir.
//!
//! İddia 1 (bloklamama): dosya sıra başı HÂLÂ düşürülürken metin ve PTT Bob'a [`TEXT_WAIT`]
//! içinde teslim edilir. Eski alıcıda bu iddia düşer (metin dosya 0'ı bekler; süre aşımı).
//! İddia 2 (kayıpsızlık): serbest bırakılınca dosya birimleri sırayla, tam bir kez gelir; metin
//! ve PTT tam bir kez; teslim kanalında düşme yok.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::LaneConfig;
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
/// Dosya birimi sayısı (şerit penceresi W = 5'ten büyük; birkaç tur).
const FILE_UNITS: u64 = 40;
/// Metin ve PTT'nin, dosya sıra başı düşürülürken teslim edilmesi için üst süre.
const TEXT_WAIT: Duration = Duration::from_secs(10);
/// Bütün akışın üst süresi.
const WAIT: Duration = Duration::from_secs(60);

const TAG_FILE: u8 = 0xF1;
const TAG_TEXT: u8 = 0x7E;
const TAG_PTT: u8 = 0x9A;

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4C;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn crypto(me: u8) -> NoiseWireProtection {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0
}

/// Gerçek kripto + alıcı yönünde oturumun İLK görülen DATA biriminin (dosya sıra başı) her
/// emisyonu `release` kalkana dek düşer. Başka hiçbir şeye dokunmaz. Birim kimliğinin düzenine
/// bağlı değildir (eski tek-sıra kimlik de, şerit etiketli kimlik de olur).
struct DropFirstUntil {
    inner: NoiseWireProtection,
    first: Option<u64>,
    release: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl PacketProtection for DropFirstUntil {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        let unit = decode_data(&od.frame)
            .map(|(_, u, _)| u)
            .or_else(|| decode_data_frag(&od.frame).map(|(_, u, _, _)| u));
        if let Some(u) = unit {
            let first = *self.first.get_or_insert(u);
            if u == first && !self.release.load(Ordering::Relaxed) {
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

impl SessionControl for DropFirstUntil {
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
    cfg.lanes = Some(LaneConfig::default());
    cfg
}

fn unit(tag: u8, n: u64) -> PreparedUnit {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(64, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

fn parse(d: &[u8]) -> (u8, u64) {
    (d[0], u64::from_be_bytes(d[1..9].try_into().expect("8 B")))
}

#[test]
fn text_and_ptt_are_delivered_while_the_file_head_is_missing() {
    let hub = MemoryHub::new();
    let release = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    let mut alice =
        ClientTransport::new(cfg(), crypto(ALICE), Box::new(hub.link(id(ALICE)))).expect("alice");
    let mut bob = ClientTransport::new(
        cfg(),
        DropFirstUntil {
            inner: crypto(BOB),
            first: None,
            release: Arc::clone(&release),
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

    // Faz 1 — dosya birimleri (şeride girer; pompa W = 5 ile uca verir). Dosya 0 Bob'da düşer.
    let mut next_file: u64 = 0;
    let push_file = |alice: &mut ClientTransport<_>, next_file: &mut u64| {
        while *next_file < FILE_UNITS {
            match alice.send(
                h,
                unit(TAG_FILE, *next_file),
                DeliveryPolicy::Reliable,
                TrafficClass::Bk,
            ) {
                Ok(()) => *next_file += 1,
                Err(TransportError::Backpressure) => break,
                Err(e) => panic!("dosya send: {e:?}"),
            }
        }
    };
    push_file(&mut alice, &mut next_file);
    // Dosya sıra başının en az bir emisyonu düşene ve bir dosya birimi Bob'da tutulana dek sür.
    let started = Instant::now();
    loop {
        push_file(&mut alice, &mut next_file);
        while alice.poll_event().is_some() {}
        while bob.poll_event().is_some() {}
        let held = bob
            .reorder_diags()
            .iter()
            .map(|(_, d)| u64::from(d.held_units))
            .sum::<u64>();
        if dropped.load(Ordering::Relaxed) > 0 && held > 0 {
            break;
        }
        assert!(
            started.elapsed() < TEXT_WAIT,
            "ön koşul: dosya sıra başı düşmeli ve arkasında birim tutulmalı (düşen {}, tutulan {held})",
            dropped.load(Ordering::Relaxed)
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        bob.recv().is_none(),
        "dosya sıra başı eksikken dosya birimi teslim edilmez"
    );

    // Faz 2 — metin ve PTT, dosya sıra başı HÂLÂ düşürülürken.
    alice
        .send(
            h,
            unit(TAG_TEXT, 0),
            DeliveryPolicy::Reliable,
            TrafficClass::Be,
        )
        .expect("metin send");
    alice
        .send(
            h,
            unit(TAG_PTT, 0),
            DeliveryPolicy::Reliable,
            TrafficClass::VoRel,
        )
        .expect("ptt send");
    let text_sent_at = Instant::now();
    let mut got: Vec<(u8, u64)> = Vec::new();
    let (mut text_at, mut ptt_at) = (None, None);
    while text_at.is_none() || ptt_at.is_none() {
        push_file(&mut alice, &mut next_file);
        while alice.poll_event().is_some() {}
        while let Some(d) = bob.recv() {
            let (tag, n) = parse(&d.unit);
            match tag {
                TAG_TEXT => text_at = Some(text_sent_at.elapsed()),
                TAG_PTT => ptt_at = Some(text_sent_at.elapsed()),
                TAG_FILE => panic!("dosya sıra başı eksikken dosya birimi {n} teslim edildi"),
                _ => panic!("bilinmeyen birim"),
            }
            got.push((tag, n));
        }
        while bob.poll_event().is_some() {}
        assert!(
            text_sent_at.elapsed() < TEXT_WAIT,
            "BLOKLAMA: dosya sıra başı eksikken metin/PTT {TEXT_WAIT:?} içinde teslim edilmedi \
             (metin {text_at:?}, PTT {ptt_at:?}; dosya 0 düşen emisyon {}, Bob'da tutulan {:?})",
            dropped.load(Ordering::Relaxed),
            bob.reorder_diags()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        !release.load(Ordering::Relaxed) && dropped.load(Ordering::Relaxed) > 0,
        "ön koşul: dosya sıra başı metin teslim edilirken hâlâ düşürülüyordu"
    );
    eprintln!(
        "ÖLÇÜM şerit-sıra: metin {:?} · PTT {:?} sonra teslim; o anda dosya 0'ın düşen emisyonu {} · \
         Bob'da tutulan dosya birimi {}",
        text_at.expect("metin"),
        ptt_at.expect("ptt"),
        dropped.load(Ordering::Relaxed),
        bob.reorder_diags()
            .iter()
            .map(|(_, d)| u64::from(d.held_units))
            .sum::<u64>()
    );

    // Faz 3 — dosya sıra başı serbest: dosya sırayla, tam bir kez tamamlanır.
    release.store(true, Ordering::Relaxed);
    let mut files: Vec<u64> = Vec::new();
    while (files.len() as u64) < FILE_UNITS {
        push_file(&mut alice, &mut next_file);
        while alice.poll_event().is_some() {}
        while let Some(d) = bob.recv() {
            let (tag, n) = parse(&d.unit);
            assert_eq!(
                tag, TAG_FILE,
                "serbest bırakıldıktan sonra yalnız dosya kalmıştı"
            );
            files.push(n);
        }
        while bob.poll_event().is_some() {}
        assert!(
            Instant::now() < deadline,
            "dosya tamamlanmalı: {}/{FILE_UNITS}, gönderilen {next_file}",
            files.len()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        files,
        (0..FILE_UNITS).collect::<Vec<_>>(),
        "dosya şeridi: sıra doğru, kayıp yok, tekrar yok"
    );
    got.sort_unstable();
    assert_eq!(
        got,
        [(TAG_TEXT, 0), (TAG_PTT, 0)],
        "metin ve PTT tam bir kez (şeritler arası sıra garantisi yok; ikisi de dosyadan önce)"
    );
    assert_eq!(bob.delivery_dropped(), 0);
    // Durgunluk: sonradan gelen bir tekrar teslim yok.
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        while alice.poll_event().is_some() {}
        assert!(bob.recv().is_none(), "tekrar teslim yok");
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }
}
