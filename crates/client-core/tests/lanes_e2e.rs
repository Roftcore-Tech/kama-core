//! [YOL3-T1] İnce şeritler — S1 adaptörü düzeyinde (gösterim; kapılı PoC ölçümü Aralık; ARQ
//! yolu, FEC Aralık'ta).
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün builder'ı; FEC iki uçta KAPALI, gösterim düğümü gibi
//! `runtime.fec = None`), zarflar `MemoryHub` üzerinden (röle elle sürülür), veri UDP loopback.
//! Bekleme süreleri yalnız beklenen olayın ÜST SINIRIDIR; iddialar sayaçlara ve teslim sırasına
//! dayanır, zamana değil.
//!
//! * T-L6 — kapalı yol: `lanes = None` → sayaç yok, teslim `send` çağrı sırasıyla (sınıf okunmaz).
//! * T-L8 — uçtan uca: 150 dosya birimi + TEXT + PTT; TEXT'in önünde motor kuyruğunda en çok W
//!   teyitsiz dosya birimi; dosya, TEXT ve PTT tam ve kendi sıralarıyla teslim.
//! * T-L10 — şeritte süresi dolan `DeadlineBounded` birim uca verilmez, sayılır.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use client_core::lanes::{LaneConfig, DEFAULT_FILE_WINDOW_UNITS};
use client_core::memory_link::MemoryHub;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit, TrafficClass,
    TransportError,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

/// Beklenen olayın üst sınırı (gerçek zaman; loopback).
const WAIT: Duration = Duration::from_secs(30);

type Node = ClientTransport<NoiseWireProtection>;

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

fn transport(me: u8, hub: &MemoryHub, lanes: Option<LaneConfig>) -> Node {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    cfg.lanes = lanes;
    ClientTransport::new(cfg, crypto(me), Box::new(hub.link(id(me)))).expect("düğüm")
}

/// Alice → Bob oturumu; iki uçta etkin tutamaç.
fn pair(lanes: Option<LaneConfig>) -> (Node, Node, SessionHandle) {
    let hub = MemoryHub::new();
    let mut alice = transport(ALICE, &hub, lanes);
    let mut bob = transport(BOB, &hub, None);
    alice.add_peer_addr(id(BOB), bob.data_addr());
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    let deadline = Instant::now() + WAIT;
    loop {
        hub.relay();
        while alice.poll_event().is_some() {}
        while bob.poll_event().is_some() {}
        if let (Some(h), Some(_)) = (
            alice.active_session_for_test(id(BOB)),
            bob.active_session_for_test(id(ALICE)),
        ) {
            return (alice, bob, h);
        }
        assert!(Instant::now() < deadline, "oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn unit(tag: u8, n: u16, len: usize) -> PreparedUnit {
    let mut bytes = vec![0u8; len.max(3)];
    bytes[0] = tag;
    bytes[1..3].copy_from_slice(&n.to_be_bytes());
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

fn tag_of(bytes: &[u8]) -> (u8, u16) {
    (bytes[0], u16::from_be_bytes([bytes[1], bytes[2]]))
}

/// Bob'un teslim kuyruğunu boşaltır; Alice'in olay/teyit/pompa yolunu sürer.
fn spin(alice: &mut Node, bob: &mut Node, got: &mut Vec<(u8, u16)>) {
    while alice.poll_event().is_some() {}
    while let Some(d) = bob.recv() {
        got.push(tag_of(&d.unit));
    }
    while bob.poll_event().is_some() {}
}

const T_FILE: u8 = b'F';
const T_TEXT: u8 = b'T';
const T_PTT: u8 = b'P';

/// T-L6 — kapalı yol: sayaç yok; farklı sınıflar karışık gönderilse de teslim `send` çağrı
/// sırasıyladır (sınıf okunmaz; bugünkü davranış).
#[test]
fn t_l6_lanes_off_delivers_in_call_order_and_has_no_lane_state() {
    let (mut alice, mut bob, h) = pair(None);
    assert_eq!(alice.lane_stats(), None);
    assert_eq!(alice.lane_backlog(h), None);
    let classes = [
        TrafficClass::Bk,
        TrafficClass::Be,
        TrafficClass::VoRel,
        TrafficClass::Sig,
        TrafficClass::Bk,
        TrafficClass::Ctrl,
    ];
    let mut want = Vec::new();
    for round in 0..10u16 {
        for (i, class) in classes.into_iter().enumerate() {
            let n = round * 10 + i as u16;
            alice
                .send(h, unit(T_FILE, n, 200), DeliveryPolicy::Reliable, class)
                .expect("kabul");
            want.push((T_FILE, n));
        }
    }
    let mut got = Vec::new();
    let deadline = Instant::now() + WAIT;
    while got.len() < want.len() {
        spin(&mut alice, &mut bob, &mut got);
        assert!(
            Instant::now() < deadline,
            "teslim tamamlanmadı: {}",
            got.len()
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(got, want, "şeritsiz: teslim çağrı sırasıyla");
}

/// T-L8 — uçtan uca: dosya akarken TEXT ve PTT dosya şeridini geçer; TEXT motor kuyruğunda en
/// çok W teyitsiz dosya biriminin arkasındadır; her şerit kendi sırasıyla, tam teslim edilir.
#[test]
fn t_l8_text_and_ptt_overtake_file_lane_and_everything_arrives_in_order() {
    const FILE_UNITS: u16 = 150;
    const PTT_UNITS: u16 = 3;
    let w = DEFAULT_FILE_WINDOW_UNITS as usize;
    let (mut alice, mut bob, h) = pair(Some(LaneConfig::default()));
    let mut got = Vec::new();
    let mut next_file = 0u16;
    let mut text_sent = false;
    let mut file_given_before_text = 0u64;
    let mut ptt_sent = 0u16;
    let deadline = Instant::now() + WAIT;
    let total = usize::from(FILE_UNITS + PTT_UNITS) + 1;
    while got.len() < total {
        // Uygulama gibi: dosya birimlerini geri basınca dek ver (pump'ta bekler).
        while next_file < FILE_UNITS {
            match alice.send(
                h,
                unit(T_FILE, next_file, 1000),
                DeliveryPolicy::Reliable,
                TrafficClass::Bk,
            ) {
                Ok(()) => next_file += 1,
                Err(TransportError::Backpressure) => break,
                Err(e) => panic!("beklenmeyen hata: {e}"),
            }
        }
        let (pending, inflight) = alice.lane_backlog(h).expect("şeritler açık");
        assert!(inflight <= w, "FILE penceresi W'yi aşmaz ({inflight})");
        assert!(pending <= 2 * w, "FILE şeridi tavanla sınırlı ({pending})");
        if !text_sent && next_file >= 20 {
            let before = alice.lane_stats().expect("şeritler açık");
            file_given_before_text = before.given[3];
            alice
                .send(
                    h,
                    unit(T_TEXT, 0, 50),
                    DeliveryPolicy::Reliable,
                    TrafficClass::Be,
                )
                .expect("TEXT kabul");
            let after = alice.lane_stats().expect("şeritler açık");
            assert_eq!(
                after.given[2],
                before.given[2] + 1,
                "TEXT aynı çağrıda uca verildi"
            );
            let (_, inflight) = alice.lane_backlog(h).expect("şeritler açık");
            assert!(
                inflight <= w,
                "TEXT'in önünde en çok W teyitsiz dosya birimi"
            );
            for n in 0..PTT_UNITS {
                alice
                    .send(
                        h,
                        unit(T_PTT, n, 80),
                        DeliveryPolicy::Reliable,
                        TrafficClass::VoRel,
                    )
                    .expect("PTT kabul");
                ptt_sent += 1;
            }
            text_sent = true;
        }
        spin(&mut alice, &mut bob, &mut got);
        assert!(
            Instant::now() < deadline,
            "teslim tamamlanmadı: {}",
            got.len()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(ptt_sent, PTT_UNITS);
    // Şerit içi sıra ve tamlık.
    let files: Vec<u16> = got
        .iter()
        .filter(|(t, _)| *t == T_FILE)
        .map(|&(_, n)| n)
        .collect();
    assert_eq!(
        files,
        (0..FILE_UNITS).collect::<Vec<_>>(),
        "dosya tam ve sıralı"
    );
    let ptts: Vec<u16> = got
        .iter()
        .filter(|(t, _)| *t == T_PTT)
        .map(|&(_, n)| n)
        .collect();
    assert_eq!(
        ptts,
        (0..PTT_UNITS).collect::<Vec<_>>(),
        "PTT tam ve sıralı"
    );
    // TEXT, kendisinden önce uca verilmiş dosya birimlerinin hemen ardından teslim edildi: şeritte
    // bekleyen dosya birimlerini geçti.
    let text_at = got
        .iter()
        .position(|&(t, _)| t == T_TEXT)
        .expect("TEXT teslim");
    let files_before_text = got[..text_at].iter().filter(|(t, _)| *t == T_FILE).count();
    assert_eq!(
        files_before_text as u64, file_given_before_text,
        "TEXT'ten önce yalnız ondan önce uca verilmiş dosya birimleri"
    );
    assert!(
        (file_given_before_text as usize) < 20,
        "TEXT şeritte bekleyen dosya birimlerini geçti"
    );
    let stats = alice.lane_stats().expect("şeritler açık");
    assert_eq!(stats.given[3], u64::from(FILE_UNITS));
    assert_eq!(
        stats.lane_expired + stats.lane_dropped + stats.give_failed,
        0
    );
    assert!(stats.file_acked >= u64::from(FILE_UNITS) - w as u64);
}

/// T-L10 — şeritte bekleyen `DeadlineBounded` birim süresi dolunca uca verilmez ve sayılır.
#[test]
fn t_l10_expired_unit_in_lane_is_dropped_and_counted_endpoint_not_called() {
    let cfg = LaneConfig {
        file_window_units: 1,
        caps: [8, 8, 8, 8],
    };
    let (mut alice, mut bob, h) = pair(Some(cfg));
    // Pencereyi doldur: birinci dosya birimi uca verilir, ikincisi (kısa deadline'lı) şeritte bekler.
    alice
        .send(
            h,
            unit(T_FILE, 0, 500),
            DeliveryPolicy::Reliable,
            TrafficClass::Bk,
        )
        .expect("kabul");
    let short = Instant::now() + Duration::from_millis(1);
    alice
        .send(
            h,
            unit(T_FILE, 1, 500),
            DeliveryPolicy::DeadlineBounded { deadline: short },
            TrafficClass::Bk,
        )
        .expect("kabul (şeritte bekler)");
    let given_before = alice.lane_stats().expect("şeritler açık").given[3];
    assert_eq!(given_before, 1, "pencere W = 1: ikinci birim şeritte");
    std::thread::sleep(Duration::from_millis(5));
    let mut got = Vec::new();
    let deadline = Instant::now() + WAIT;
    loop {
        spin(&mut alice, &mut bob, &mut got);
        let s = alice.lane_stats().expect("şeritler açık");
        if s.lane_expired == 1 {
            assert_eq!(s.given[3], 1, "süresi dolan birim uca verilmedi");
            break;
        }
        assert!(Instant::now() < deadline, "süresi dolan birim sayılmadı");
        std::thread::sleep(Duration::from_millis(2));
    }
    // Uca verilen birinci birimin teyidi pencereyi boşaltır; şeritte bir şey kalmaz.
    while alice.lane_backlog(h) != Some((0, 0)) {
        spin(&mut alice, &mut bob, &mut got);
        assert!(Instant::now() < deadline, "teyit pencereyi boşaltmadı");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        got,
        [(T_FILE, 0)],
        "yalnız süresi dolmayan birim teslim edildi"
    );
}
