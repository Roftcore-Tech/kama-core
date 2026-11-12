//! [ŞERİT-SIRA · merge öncesi] Toplam alıcı sınırı ve geri basınç: dört şerit de gönderirken
//! uygulama tüketicisi birkaç saniye durur.
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması, şeritler açık, FEC kapalı),
//! zarflar `MemoryHub`, veri UDP loopback. Alice PTT, SIG, TEXT ve FILE şeritlerine birlikte
//! sunar (geri basınca saygılı). Bob'un uygulaması [`STALL`] boyunca `recv` ÇAĞIRMAZ (olay
//! döngüsü `poll_event` ile sürer). Alice ara ara bir birimin ilk emisyonunu kaybettirir: şeritlerde
//! boşluk açılır, Bob'un sıralayıcısı birim tutar. Bob'un sınırları küçük tutulur ki doldukları
//! görülsün: teslim kanalı [`RX_DELIVERY_QUEUE`], şerit başına kabul penceresi [`RX_WINDOW`].
//!
//! İddialar:
//! 1. Alıcıda tamponlanan birim (teslim kanalı + teslim kutusu + sıralayıcı) her an
//!    `receiver_buffer_bound_units` toplam sınırının altında; teslim kutusu `MAX_INBOX`'u, kanal
//!    kapasitesini hiç aşmaz.
//! 2. Geri basınç gerçekten devrede: duruş sırasında veri alıcısı kanalı dolu olduğu için DATA'yı
//!    kabul etmez (`data_not_admitted_full > 0`), Alice yine de duruş sırasında teyit (transport-ACK)
//!    alır — Bob'da tamponlanan birimler.
//! 3. Tüketici dönünce teyitli ya da değil, gönderilen her birim her şeritte sırayla ve tam bir
//!    kez teslim edilir; teslim kanalında düşme yok.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use client_core::lanes::LaneConfig;
use client_core::memory_link::MemoryHub;
use client_core::transport::{
    receiver_buffer_bound_units, ClientTransport, ClientTransportConfig, MAX_INBOX,
};
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
/// Bob'un veri alıcısı teslim kanalı (birim).
const RX_DELIVERY_QUEUE: usize = 64;
/// Bob'un şerit başına kabul penceresi (birim).
const RX_WINDOW: usize = 128;
/// Tüketicinin durduğu süre.
const STALL: Duration = Duration::from_secs(3);
/// Duruştan önce normal akış.
const WARMUP: Duration = Duration::from_millis(500);
/// Bütün akışın üst süresi.
const WAIT: Duration = Duration::from_secs(90);
/// Her bu kadar sunumda bir birimin ilk emisyonu kaybettirilir (şeritte boşluk).
const DROP_EVERY: u64 = 97;

/// Şerit başına birim sayısı: PTT, SIG, TEXT, FILE (şerit sırası).
const TOTALS: [u64; 4] = [250, 100, 500, 6000];
/// Şerit başına sunum hızı (birim/s; `None` = geri basının izin verdiği kadar). Hızlı olmayan
/// şeritler duruşun sonuna dek sunmayı sürdürür: duruş boyunca dört şerit de gönderir.
const RATES: [Option<u64>; 4] = [Some(50), Some(20), Some(100), None];
const CLASSES: [TrafficClass; 4] = [
    TrafficClass::VoRel,
    TrafficClass::Sig,
    TrafficClass::Be,
    TrafficClass::Bk,
];

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

fn cfg(receiver: bool) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    cfg.lanes = Some(LaneConfig::default());
    if receiver {
        cfg.runtime.delivery_queue = RX_DELIVERY_QUEUE;
        cfg.runtime.max_inflight_units = RX_WINDOW;
    }
    cfg
}

fn unit(lane: u8, n: u64) -> PreparedUnit {
    let mut bytes = vec![lane];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(64, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn receiver_buffer_stays_bounded_and_nothing_acked_is_lost_while_the_app_stalls() {
    let hub = MemoryHub::new();
    let bob_cfg = cfg(true);
    let bound = receiver_buffer_bound_units(&bob_cfg.runtime, 1);
    let mut alice = ClientTransport::new(cfg(false), crypto(ALICE), Box::new(hub.link(id(ALICE))))
        .expect("alice");
    let mut bob =
        ClientTransport::new(bob_cfg, crypto(BOB), Box::new(hub.link(id(BOB)))).expect("bob");
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
    let mut sent = [0u64; 4];
    let mut offers: u64 = 0;
    let mut got: [Vec<u64>; 4] = Default::default();
    let mut max_buffered = 0usize;
    let mut max_inbox = 0usize;
    let mut max_channel = 0usize;
    let mut max_held = 0usize;
    let acks = |a: &ClientTransport<NoiseWireProtection>| {
        a.lane_stats().expect("şeritler açık").acks_drained
    };
    // Alice'in teyit sayacı duruş başında ve sonunda.
    let mut stall_start_acks: Option<u64> = None;
    let mut stall_end_acks: Option<u64> = None;
    let mut not_admitted_full_in_stall = 0u64;

    loop {
        // Alice: dört şerit, şerit sırasıyla, hızına ve geri basınca saygılı.
        let since_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        for lane in 0..4 {
            let allowed = RATES[lane].map_or(TOTALS[lane], |r| {
                (r.saturating_mul(since_ms) / 1000 + 1).min(TOTALS[lane])
            });
            while sent[lane] < allowed {
                offers += 1;
                if offers.is_multiple_of(DROP_EVERY) {
                    alice.drop_next_units_for_test(1);
                }
                match alice.send(
                    h,
                    unit(u8::try_from(lane).expect("şerit"), sent[lane]),
                    DeliveryPolicy::Reliable,
                    CLASSES[lane],
                ) {
                    Ok(()) => sent[lane] += 1,
                    Err(TransportError::Backpressure) => break,
                    Err(e) => panic!("şerit {lane} send: {e:?}"),
                }
            }
        }
        while alice.poll_event().is_some() {}

        let elapsed = started.elapsed();
        let stalled = elapsed >= WARMUP && elapsed < WARMUP + STALL;
        if stalled && stall_start_acks.is_none() {
            stall_start_acks = Some(acks(&alice));
        }
        if !stalled && stall_start_acks.is_some() && stall_end_acks.is_none() {
            stall_end_acks = Some(acks(&alice));
            not_admitted_full_in_stall = bob.data_not_admitted_full();
        }
        if stalled {
            // Uygulama okumaz; olay döngüsü sürer (adaptör teslimleri sürer ama kutu sınırlı).
            while bob.poll_event().is_some() {}
        } else {
            while let Some(d) = bob.recv() {
                let lane = usize::from(d.unit[0]);
                let n = u64::from_be_bytes(d.unit[1..9].try_into().expect("8 B"));
                got[lane].push(n);
            }
            while bob.poll_event().is_some() {}
        }
        let (channel, inbox, held) = bob.receiver_buffered_for_test();
        assert!(
            channel <= RX_DELIVERY_QUEUE,
            "teslim kanalı sınırı aşıldı: {channel}"
        );
        assert!(inbox <= MAX_INBOX, "teslim kutusu sınırı aşıldı: {inbox}");
        let total = channel + inbox + held;
        assert!(
            total <= bound,
            "alıcı toplam tampon sınırı aşıldı: {total} > {bound} (kanal {channel}, kutu {inbox}, \
             sıralayıcı {held})"
        );
        max_buffered = max_buffered.max(total);
        max_inbox = max_inbox.max(inbox);
        max_channel = max_channel.max(channel);
        max_held = max_held.max(held);

        let done = (0..4).all(|l| got[l].len() as u64 == TOTALS[l]);
        if done && stall_end_acks.is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "akış tamamlanmadı: gönderilen {sent:?}, teslim {:?}",
            got.iter().map(Vec::len).collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(1));
    }

    // Durgunluk: tekrar teslim yok.
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        while alice.poll_event().is_some() {}
        assert!(bob.recv().is_none(), "tekrar teslim yok");
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }

    for lane in 0..4 {
        assert_eq!(
            got[lane],
            (0..TOTALS[lane]).collect::<Vec<_>>(),
            "şerit {lane}: sırayla, tam bir kez, kayıpsız"
        );
    }
    assert_eq!(bob.delivery_dropped(), 0);
    let acked_in_stall = stall_end_acks
        .expect("duruş bitti")
        .saturating_sub(stall_start_acks.expect("duruş başladı"));
    eprintln!(
        "ÖLÇÜM alıcı-sınırı: tavan {bound} birim; gözlenen en çok toplam {max_buffered} (kanal \
         {max_channel}/{RX_DELIVERY_QUEUE}, kutu {max_inbox}/{MAX_INBOX}, sıralayıcı {max_held}); \
         duruşta Alice'in aldığı teyit {acked_in_stall}; duruşta kanal dolu diye kabul edilmeyen \
         DATA {not_admitted_full_in_stall}; teslim {:?}",
        got.iter().map(Vec::len).collect::<Vec<_>>()
    );
    assert!(
        not_admitted_full_in_stall > 0,
        "ön koşul: duruşta geri basınç devreye girmeliydi (kanal dolu → DATA kabul edilmez)"
    );
    assert!(
        acked_in_stall > 0,
        "ön koşul: duruşta Bob'da tamponlanan birimler Alice'e teyit edilmiş olmalı"
    );
    assert_eq!(
        max_inbox, MAX_INBOX,
        "ön koşul: teslim kutusu duruşta tavanına dolmalıydı"
    );
    assert!(
        max_held > 0,
        "ön koşul: sıralayıcı boşluk arkasında birim tutmalıydı"
    );
}
