//! [ŞERİT-GENİŞLİK] Şerit başına kabul genişliği ve gönderici açıklık sınırı: bir şeridin sıra
//! başı eksikken o şerit kendi genişliğinde durur, öbür şeritler akmayı sürdürür.
//!
//! DÜZENEK: iki gerçek `ClientTransport`, ürün yapılandırması (şeritler açık; kabul genişlikleri
//! iki uçta aynı `LaneConfig::admit_widths` = PTT/SIG/TEXT 256, FILE 4096), FEC kapalı (eksik
//! birim ARQ ile gelir, süre belirlenimli). Zarflar `MemoryHub`, veri UDP loopback. Bob'un
//! kriptosunu saran sarmalayıcı HEDEF şeridin 0. biriminin (sıra başı) DATA emisyonlarını test
//! serbest bırakana dek düşürür; başka hiçbir şeye dokunmaz.
//!
//! Alice hedef şeride [`TARGET_UNITS`] (> 256) birim sunar (geri basınca saygılı, olabildiğince
//! hızlı); aynı anda öbür gerçek zamanlı şeride ve dosya şeridine sabit hızla sunar.
//!
//! İddialar:
//! 1. Sıra başı eksikken hedef şeritten uca verilen birim sayısı tam genişliktir (256): sıra başı
//!    ve arkasındaki 255 birim. Fazlası şerit katmanında bekler (`span_blocked > 0`). Bob hedef
//!    şeritte tam 255 birim tutar.
//! 2. Bu sırada öbür iki şerit teslim edilmeyi sürdürür ([`PLATEAU`] boyunca ikisinde de artış).
//! 3. Hiçbir DATA Bob'un kabul penceresinin dışına düşmez (`data_not_admitted_window == 0`).
//! 4. Serbest bırakılınca hedef şeridin bütün birimleri ve öbür şeritlerin bütün birimleri sırayla,
//!    tam bir kez teslim edilir; teslim kanalında düşme yok; durgunlukta tekrar yok.
//!
//! Koşu 3 ([`ptt_head_arrives_while_the_receiver_adapter_is_not_draining`]): sıra başı geldiği
//! anda Bob'un adaptörü kanalı çekmez (test Bob'u sürmez). Göndericinin teyit sınırı kabul
//! anında ilerler, alıcının sıralayıcı başı ilerlemez; pencere tabanı veri alıcısının kabul
//! sınırı olduğu için gönderilen hiçbir birim pencere dışı kalmaz.
//!
//! Karşı-olgular (birer kez koşuldu, PR gövdesinde): göndericinin PTT açıklık sınırı kaldırılınca
//! (`room = u64::MAX`) koşu 1 düşer; alıcının pencere tabanı yalnız sıralayıcı başı olunca koşu 3
//! düşer.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::{Lane, LaneConfig, DEFAULT_ADMIT_WIDTHS};
use client_core::memory_link::MemoryHub;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::unit_id::{index_of_lane, tagged};
use client_core::wire::decode_data;
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
/// Hedef şeride sunulan birim (> genişlik 256).
const TARGET_UNITS: u64 = 600;
/// Öbür gerçek zamanlı şerit ve dosya şeridine sunulan birim.
const OTHER_RT_UNITS: u64 = 300;
const FILE_UNITS: u64 = 1500;
/// Öbür şeritlerin sunum hızı (birim/s): sıra başı eksikken de akıyor olsunlar.
const OTHER_RT_RATE: u64 = 100;
const FILE_RATE: u64 = 500;
/// Hedef şerit sınıra dayandıktan sonra öbür şeritlerin ilerlemesinin ölçüldüğü süre.
const PLATEAU: Duration = Duration::from_millis(800);
/// Bütün akışın üst süresi.
const WAIT: Duration = Duration::from_secs(60);
/// Koşu 3: serbest bırakmadan sonra Bob'un adaptörünün sürülmediği süre.
const FREEZE: Duration = Duration::from_millis(400);

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

/// Gerçek kripto + alıcı yönünde `head` biriminin DATA emisyonları `release` kalkana dek düşer.
struct HeadDrop {
    inner: NoiseWireProtection,
    head: u64,
    release: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
}

impl PacketProtection for HeadDrop {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        if let Some((_, u, _)) = decode_data(&od.frame) {
            if u == self.head && !self.release.load(Ordering::Relaxed) {
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

impl SessionControl for HeadDrop {
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
    assert_eq!(
        cfg.lanes.map(|l| l.admit_widths),
        Some(DEFAULT_ADMIT_WIDTHS),
        "ürün profili varsayılan genişliklerle"
    );
    cfg.lanes = Some(LaneConfig::default());
    cfg
}

fn class_of(lane: Lane) -> TrafficClass {
    match lane {
        Lane::Ptt => TrafficClass::VoRel,
        Lane::Sig => TrafficClass::Sig,
        Lane::Text => TrafficClass::Be,
        Lane::File => TrafficClass::Bk,
    }
}

fn unit(lane: Lane, n: u64) -> PreparedUnit {
    let mut bytes = vec![u8::try_from(index_of_lane(lane)).expect("şerit")];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(64, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

/// Bir koşunun ölçümü.
#[derive(Debug)]
struct Measured {
    /// Sıra başı eksikken hedef şeritten uca verilen en çok birim.
    target_given_max: u64,
    /// O anda hedef şeridin şerit katmanında bekleyen birimi.
    target_waiting: usize,
    /// Bob'un hedef şeritte tuttuğu en çok birim.
    target_held_max: usize,
    /// Açıklık sınırına takılan pompa turu (hedef şerit).
    span_blocked: u64,
    /// [`PLATEAU`] boyunca öbür gerçek zamanlı şeridin ve dosyanın teslim artışı.
    other_rt_progress: usize,
    file_progress: usize,
    /// Serbest bırakmadan hedef şeridin tamamlanmasına dek geçen süre.
    target_done_after: Duration,
    not_admitted_window: u64,
    head_drops: u64,
    /// `freeze` kolunda: Bob'un adaptörü sürülmezken hedef şeritten verilen birim ve o andaki
    /// pencere dışı DATA.
    given_while_frozen: u64,
    not_admitted_window_frozen: u64,
}

#[allow(clippy::too_many_lines)]
fn run(target: Lane, other_rt: Lane, freeze: bool) -> Measured {
    let hub = MemoryHub::new();
    let release = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    let tl = index_of_lane(target);
    let width = DEFAULT_ADMIT_WIDTHS[tl];
    let mut alice =
        ClientTransport::new(cfg(), crypto(ALICE), Box::new(hub.link(id(ALICE)))).expect("alice");
    let mut bob = ClientTransport::new(
        cfg(),
        HeadDrop {
            inner: crypto(BOB),
            head: tagged(tl, 0),
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

    // Şerit sırası: hedef, öbür gerçek zamanlı, dosya.
    let lanes = [target, other_rt, Lane::File];
    let totals = [TARGET_UNITS, OTHER_RT_UNITS, FILE_UNITS];
    let rates = [None, Some(OTHER_RT_RATE), Some(FILE_RATE)];
    let mut sent = [0u64; 3];
    let mut got: [Vec<u64>; 4] = Default::default();
    let started = Instant::now();
    let mut m = Measured {
        target_given_max: 0,
        target_waiting: 0,
        target_held_max: 0,
        span_blocked: 0,
        other_rt_progress: 0,
        file_progress: 0,
        target_done_after: Duration::ZERO,
        not_admitted_window: 0,
        head_drops: 0,
        given_while_frozen: 0,
        not_admitted_window_frozen: 0,
    };

    // `bob_on = false`: Bob'un hiçbir yöntemi çağrılmaz — adaptörü teslim kanalından çekmez,
    // sıralayıcı başı ilerlemez (veri alıcısı kendi görevinde kabul etmeyi sürdürür).
    let step = |alice: &mut ClientTransport<NoiseWireProtection>,
                bob: &mut ClientTransport<HeadDrop>,
                sent: &mut [u64; 3],
                got: &mut [Vec<u64>; 4],
                bob_on: bool| {
        let since_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        for i in 0..3 {
            let allowed = rates[i].map_or(totals[i], |r| (r * since_ms / 1000 + 1).min(totals[i]));
            while sent[i] < allowed {
                match alice.send(
                    h,
                    unit(lanes[i], sent[i]),
                    DeliveryPolicy::Reliable,
                    class_of(lanes[i]),
                ) {
                    Ok(()) => sent[i] += 1,
                    Err(TransportError::Backpressure) => break,
                    Err(e) => panic!("{:?} send: {e:?}", lanes[i]),
                }
            }
        }
        while alice.poll_event().is_some() {}
        if bob_on {
            while let Some(d) = bob.recv() {
                let lane = usize::from(d.unit[0]);
                got[lane].push(u64::from_be_bytes(d.unit[1..9].try_into().expect("8 B")));
            }
            while bob.poll_event().is_some() {}
        }
        std::thread::sleep(Duration::from_millis(1));
    };

    // Faz 1 — hedef sıra başı eksik: hedef şerit genişliğine dayanana dek.
    let plateau_at = loop {
        step(&mut alice, &mut bob, &mut sent, &mut got, true);
        let st = alice.lane_stats().expect("şeritler");
        let given = st.given[tl];
        m.target_given_max = m.target_given_max.max(given);
        let (_, _, held) = bob.receiver_buffered_for_test();
        m.target_held_max = m.target_held_max.max(held[tl]);
        if dropped.load(Ordering::Relaxed) > 0 && given >= width && st.span_blocked[tl] > 0 {
            break Instant::now();
        }
        assert!(
            Instant::now() < deadline,
            "{target:?}: şerit genişliğe dayanmadı (verilen {given}, düşen sıra başı {}; teslim \
             {:?}; Bob'da pencere dışı DATA {})",
            dropped.load(Ordering::Relaxed),
            got.iter().map(Vec::len).collect::<Vec<_>>(),
            bob.data_not_admitted_window()
        );
    };
    // Faz 2 — PLATEAU boyunca hedef şerit durur, öbürleri akar.
    let rt_before = got[index_of_lane(other_rt)].len();
    let file_before = got[index_of_lane(Lane::File)].len();
    while plateau_at.elapsed() < PLATEAU {
        step(&mut alice, &mut bob, &mut sent, &mut got, true);
        let given = alice.lane_stats().expect("şeritler").given[tl];
        m.target_given_max = m.target_given_max.max(given);
        let (_, _, held) = bob.receiver_buffered_for_test();
        m.target_held_max = m.target_held_max.max(held[tl]);
        assert!(
            got[tl].is_empty(),
            "{target:?}: sıra başı eksikken hedef şerit teslim edilmez"
        );
    }
    m.other_rt_progress = got[index_of_lane(other_rt)].len() - rt_before;
    m.file_progress = got[index_of_lane(Lane::File)].len() - file_before;
    let st = alice.lane_stats().expect("şeritler");
    m.target_waiting = usize::try_from(st.accepted[tl] - st.given[tl]).expect("bekleyen");
    m.span_blocked = st.span_blocked[tl];

    // Faz 3 — serbest bırak: her şey tamamlanır. `freeze`: serbest bırakıldıktan sonra [`FREEZE`]
    // boyunca Bob'un adaptörü sürülmez (sıra başı gelir, kanalda bekler; adaptör onu çekmez).
    release.store(true, Ordering::Relaxed);
    let released_at = Instant::now();
    if freeze {
        while released_at.elapsed() < FREEZE {
            step(&mut alice, &mut bob, &mut sent, &mut got, false);
        }
        m.given_while_frozen = alice.lane_stats().expect("şeritler").given[tl];
        m.not_admitted_window_frozen = bob.data_not_admitted_window();
    }
    let mut target_done: Option<Instant> = None;
    loop {
        step(&mut alice, &mut bob, &mut sent, &mut got, true);
        if target_done.is_none() && got[tl].len() as u64 == TARGET_UNITS {
            target_done = Some(Instant::now());
        }
        let done = (0..3).all(|i| got[index_of_lane(lanes[i])].len() as u64 == totals[i]);
        if done {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{target:?}: tamamlanmadı — sunulan {sent:?}, teslim {:?}",
            got.iter().map(Vec::len).collect::<Vec<_>>()
        );
    }
    m.target_done_after = target_done.expect("hedef").duration_since(released_at);
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        while alice.poll_event().is_some() {}
        assert!(bob.recv().is_none(), "tekrar teslim yok");
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }
    for i in 0..3 {
        assert_eq!(
            got[index_of_lane(lanes[i])],
            (0..totals[i]).collect::<Vec<_>>(),
            "{:?} şeridi: sırayla, tam bir kez, kayıpsız",
            lanes[i]
        );
    }
    assert_eq!(bob.delivery_dropped(), 0);
    m.not_admitted_window = bob.data_not_admitted_window();
    m.head_drops = dropped.load(Ordering::Relaxed);
    eprintln!("ÖLÇÜM şerit-genişlik {target:?} sıra başı eksik: {m:?}");

    assert_eq!(
        m.target_given_max, width,
        "{target:?}: sıra başı eksikken şeritten tam genişlik kadar birim verilir"
    );
    assert_eq!(
        m.target_held_max,
        usize::try_from(width - 1).expect("genişlik"),
        "{target:?}: Bob sıra başının arkasında genişlik − 1 birim tutar"
    );
    assert!(
        m.target_waiting > 0 && m.span_blocked > 0,
        "{target:?}: fazlası şerit katmanında bekledi"
    );
    assert!(
        m.other_rt_progress > 0 && m.file_progress > 0,
        "{target:?}: öbür şeritler {PLATEAU:?} boyunca ilerledi: {m:?}"
    );
    assert_eq!(
        m.not_admitted_window, 0,
        "{target:?}: alıcının kabul penceresinin dışına DATA gitti"
    );
    m
}

/// Koşu 1 — PTT'nin sıra başı eksik; metin ve dosya akar.
#[test]
fn ptt_head_missing_ptt_stops_at_its_width_text_and_file_keep_flowing() {
    run(Lane::Ptt, Lane::Text, false);
}

/// Koşu 2 — metnin sıra başı eksik; PTT ve dosya akar.
#[test]
fn text_head_missing_text_stops_at_its_width_ptt_and_file_keep_flowing() {
    run(Lane::Text, Lane::Ptt, false);
}

/// Koşu 3 — PTT sıra başı, alıcının adaptörü kanalı çekmezken gelir. Veri alıcısı sıra başını
/// kabul eder ve teyit eder; göndericinin teyit sınırı 256 ileri atlar, gönderici 256 birim daha
/// yazar. Alıcının sıralayıcı başı ise adaptör sürülmediği için 0'da kalır. Pencere tabanı yalnız
/// sıralayıcı başı olsaydı bu 256 birim pencere dışı kalırdı (belirlenimli: adaptör hiç sürülmez).
/// Taban kabul sınırı olduğu için hepsi kabul edilir.
#[test]
fn ptt_head_arrives_while_the_receiver_adapter_is_not_draining() {
    let m = run(Lane::Ptt, Lane::Text, true);
    assert!(
        m.given_while_frozen > DEFAULT_ADMIT_WIDTHS[0],
        "ön koşul: alıcının adaptörü donukken gönderici teyit sınırının atlamasıyla yazdı: {m:?}"
    );
    assert_eq!(
        m.not_admitted_window_frozen, 0,
        "adaptör donukken kabul edilmesi gereken birim pencere dışı sayıldı"
    );
}
