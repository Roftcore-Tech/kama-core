//! [ŞERİT-SIRA · merge öncesi] Toplam alıcı sınırı ve geri basınç: dört şerit de gönderirken
//! uygulama tüketicisi birkaç saniye durur.
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması, şeritler açık, FEC kapalı),
//! zarflar `MemoryHub`, veri UDP loopback. Şerit ayarları (kabul genişlikleri dahil) iki uçta
//! aynıdır. Bob'un uygulaması duruş boyunca `recv` ÇAĞIRMAZ (olay döngüsü `poll_event` ile sürer).
//!
//! İki kol:
//! * **küçük sınırlar** ([`receiver_buffer_stays_bounded_with_small_limits`]) — teslim kanalı
//!   [`SMALL_DELIVERY_QUEUE`], dört şeritte genişlik [`SMALL_WIDTH`]: sınırların dolduğu kısa
//!   sürede görülür. Alice ara ara bir birimin ilk emisyonunu kaybettirir (şeritte boşluk).
//! * **ürün ayarları** ([`receiver_buffer_stays_bounded_with_product_limits`]) — alıcı ayarları
//!   ürünün: teslim kanalı 1024, teslim kutusu 1024, genişlikler PTT/SIG/TEXT 256, FILE 4096;
//!   birim ürünün birim tavanı boyunda (1013 B). Göndericinin dosya penceresi tabanı W = 512
//!   (ürün 5; loopback'te BDP ölçümü W'yi büyütmez ve dört sıra başı sürekli düşerken dosya
//!   penceresinin 4096'ya dolması dakikalar sürerdi). W yalnız göndericinin hızını belirler,
//!   alıcının tampon sınırlarını değil. Dört şeridin sıra başı Bob'da serbest bırakılana dek
//!   düşer: önce dosya, sonra PTT/SIG/TEXT penceresini doldurur (sıralayıcı şerit başına
//!   genişlik − 1 birim tutar). Sonra uygulama durur ve sıra
//!   başları serbest bırakılır: tutulan koşular teslim kutusuna, ardından gelenler teslim kanalına
//!   dolar. Bu, alıcı tamponunun sınırına yaklaşan en kötü durumdur.
//!
//! İddialar (iki kol):
//! 1. Alıcıda tamponlanan birim (teslim kanalı + teslim kutusu + sıralayıcı) her an
//!    `receiver_buffer_bound_units` toplam sınırının altında; teslim kutusu `MAX_INBOX`'u, kanal
//!    kapasitesini, sıralayıcının her şeridi kendi genişliğini hiç aşmaz.
//! 2. Geri basınç gerçekten devrede: duruşta veri alıcısı kanalı dolu olduğu için DATA'yı kabul
//!    etmez (`data_not_admitted_full > 0`); duruşta Alice yine de teyit (transport-ACK) alır.
//! 3. Tüketici dönünce gönderilen her birim her şeritte sırayla ve tam bir kez teslim edilir;
//!    teslim kanalında düşme yok; durgunlukta tekrar yok.
//! 4. Hiçbir DATA alıcının kabul penceresinin dışına düşmez (`data_not_admitted_window == 0`),
//!    uygulama dururken de. Karşı-olgu (bir kez koşuldu, PR gövdesinde): pencere tabanı yalnız
//!    sıralayıcı başı olunca küçük sınırlar kolu düşer.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::{LaneConfig, DEFAULT_ADMIT_WIDTHS};
use client_core::memory_link::MemoryHub;
use client_core::runtime::KamaV1Profile;
use client_core::transport::{
    receiver_buffer_bound_units, ClientTransport, ClientTransportConfig, MAX_INBOX,
};
use client_core::unit_id::tagged;
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
/// Küçük kol: Bob'un teslim kanalı (birim).
const SMALL_DELIVERY_QUEUE: usize = 64;
/// Küçük kol: dört şeritte kabul genişliği (birim; iki uçta).
const SMALL_WIDTH: u64 = 128;
/// Tüketicinin en az durduğu süre.
const STALL: Duration = Duration::from_secs(3);
/// Duruş, teslim kanalı tavanına dolup veri alıcısı DATA reddedene dek uzar; en çok bu kadar (serbest bırakılan sıra
/// başlarının yeniden iletimi geri çekilmeli zamanlayıcıyla gelir).
const STALL_MAX: Duration = Duration::from_secs(30);
/// Bütün akışın üst süresi.
const WAIT: Duration = Duration::from_secs(120);

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

/// Gerçek kripto + alıcı yönünde `heads` birimlerinin DATA emisyonları `release` kalkana dek
/// düşer (boş liste: hiçbir şeye dokunmaz).
struct HeadDrop {
    inner: NoiseWireProtection,
    heads: Vec<u64>,
    release: Arc<AtomicBool>,
}

impl PacketProtection for HeadDrop {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        if !self.release.load(Ordering::Relaxed) {
            if let Some((_, u, _)) = decode_data(&od.frame) {
                if self.heads.contains(&u) {
                    return None;
                }
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

/// Bir kolun planı.
struct Plan {
    name: &'static str,
    /// İki ucun şerit ayarı.
    lanes: LaneConfig,
    /// Bob'un teslim kanalı.
    delivery_queue: usize,
    /// Şerit başına birim sayısı (PTT, SIG, TEXT, FILE).
    totals: [u64; 4],
    /// Şerit başına sunum hızı (birim/s; `None` = geri basının izin verdiği kadar).
    rates: [Option<u64>; 4],
    /// Birim yük boyu (bayt).
    unit_bytes: usize,
    /// Her bu kadar sunumda bir birimin ilk emisyonu kaybettirilir (0 = yok).
    drop_every: u64,
    /// Bob'da serbest bırakılana dek düşen birimler (her şeridin sıra başı ya da hiç).
    heads_dropped: bool,
}

/// Kolun ölçümü.
#[derive(Debug, Default)]
struct Measured {
    bound: usize,
    max_total: usize,
    max_channel: usize,
    max_inbox: usize,
    max_held: [usize; 4],
    acked_in_stall: u64,
    not_admitted_full_in_stall: u64,
    not_admitted_window: u64,
}

fn unit(lane: usize, n: u64, len: usize) -> PreparedUnit {
    let mut bytes = vec![u8::try_from(lane).expect("şerit")];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(len, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

fn base_cfg(lanes: LaneConfig) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = None;
    cfg.lanes = Some(lanes);
    cfg
}

#[allow(clippy::too_many_lines)]
fn run(plan: &Plan) -> Measured {
    let hub = MemoryHub::new();
    let release = Arc::new(AtomicBool::new(false));
    let widths = plan.lanes.admit_widths;
    let mut bob_cfg = base_cfg(plan.lanes);
    bob_cfg.runtime.delivery_queue = plan.delivery_queue;
    let bound = receiver_buffer_bound_units(&bob_cfg.runtime, bob_cfg.lanes.as_ref(), 1);
    let mut alice = ClientTransport::new(
        base_cfg(plan.lanes),
        crypto(ALICE),
        Box::new(hub.link(id(ALICE))),
    )
    .expect("alice");
    let heads = if plan.heads_dropped {
        (0..4).map(|l| tagged(l, 0)).collect()
    } else {
        Vec::new()
    };
    let mut bob = ClientTransport::new(
        bob_cfg,
        HeadDrop {
            inner: crypto(BOB),
            heads,
            release: Arc::clone(&release),
        },
        Box::new(hub.link(id(BOB))),
    )
    .expect("bob");
    assert!(
        plan.unit_bytes <= alice.capabilities().max_plaintext_unit_bytes as usize,
        "{}: birim yükü adaptörün birim tavanına sığar",
        plan.name
    );
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
    let mut m = Measured {
        bound,
        ..Measured::default()
    };
    let acks = |a: &ClientTransport<NoiseWireProtection>| {
        a.lane_stats().expect("şeritler açık").acks_drained
    };

    // Bir tur: Alice sunar (hızına ve geri basınca saygılı), Bob okursa okur, tampon ölçülür.
    let mut step = |alice: &mut ClientTransport<NoiseWireProtection>,
                    bob: &mut ClientTransport<HeadDrop>,
                    got: &mut [Vec<u64>; 4],
                    m: &mut Measured,
                    reading: bool,
                    rt_on: bool| {
        let since_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        for lane in 0..4 {
            if lane != 3 && !rt_on {
                continue;
            }
            let allowed = plan.rates[lane].map_or(plan.totals[lane], |r| {
                (r.saturating_mul(since_ms) / 1000 + 1).min(plan.totals[lane])
            });
            while sent[lane] < allowed {
                offers += 1;
                if plan.drop_every > 0 && offers.is_multiple_of(plan.drop_every) {
                    alice.drop_next_units_for_test(1);
                }
                match alice.send(
                    h,
                    unit(lane, sent[lane], plan.unit_bytes),
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
        if reading {
            while let Some(d) = bob.recv() {
                let lane = usize::from(d.unit[0]);
                got[lane].push(u64::from_be_bytes(d.unit[1..9].try_into().expect("8 B")));
            }
        }
        while bob.poll_event().is_some() {}
        let (channel, inbox, held) = bob.receiver_buffered_for_test();
        assert!(
            channel <= plan.delivery_queue,
            "{}: teslim kanalı sınırı aşıldı: {channel}",
            plan.name
        );
        assert!(
            inbox <= MAX_INBOX,
            "{}: teslim kutusu sınırı aşıldı: {inbox}",
            plan.name
        );
        for lane in 0..4 {
            assert!(
                held[lane] as u64 <= widths[lane],
                "{}: şerit {lane} sıralayıcısı genişliğini aştı: {}",
                plan.name,
                held[lane]
            );
            m.max_held[lane] = m.max_held[lane].max(held[lane]);
        }
        let total = channel + inbox + held.iter().sum::<usize>();
        assert!(
            total <= bound,
            "{}: alıcı toplam tampon sınırı aşıldı: {total} > {bound} (kanal {channel}, kutu \
             {inbox}, sıralayıcı {held:?})",
            plan.name
        );
        m.max_total = m.max_total.max(total);
        m.max_inbox = m.max_inbox.max(inbox);
        m.max_channel = m.max_channel.max(channel);
        std::thread::sleep(Duration::from_millis(1));
    };

    // Faz 1 — okuyan uygulama. Sıra başları düşüyorsa her şerit penceresini doldurana dek; yoksa
    // kısa bir ısınma.
    // Sıra başları düşüyorsa önce dosya penceresini doldurur, sonra PTT/SIG/TEXT başlar: dört sıra
    // başı birlikte yalnız kısa süre kara delikte kalır (sürekli kaybolan dört birim motorun yeni
    // veri hızını düşürür; bu düzeneğin yapay koşuludur, alıcı tamponuyla ilgisi yoktur).
    let mut rt_on = !plan.heads_dropped;
    loop {
        step(&mut alice, &mut bob, &mut got, &mut m, true, rt_on);
        let full = if plan.heads_dropped {
            let (_, _, held) = bob.receiver_buffered_for_test();
            rt_on = rt_on || held[3] as u64 + 1 >= widths[3];
            (0..4).all(|l| held[l] as u64 + 1 >= widths[l])
        } else {
            started.elapsed() >= Duration::from_millis(500)
        };
        if full {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{}: pencereler dolmadı — sunulan {sent:?}, Bob {:?}",
            plan.name,
            bob.receiver_buffered_for_test()
        );
    }
    // Faz 2 — uygulama durur; sıra başları serbest.
    let stall_start_acks = acks(&alice);
    release.store(true, Ordering::Relaxed);
    let stall_at = Instant::now();
    while stall_at.elapsed() < STALL
        || ((m.max_channel < plan.delivery_queue || bob.data_not_admitted_full() == 0)
            && stall_at.elapsed() < STALL_MAX)
    {
        step(&mut alice, &mut bob, &mut got, &mut m, false, true);
    }
    m.acked_in_stall = acks(&alice).saturating_sub(stall_start_acks);
    m.not_admitted_full_in_stall = bob.data_not_admitted_full();
    // Faz 3 — uygulama döner: her şey tamamlanır.
    while (0..4).any(|l| got[l].len() as u64 != plan.totals[l]) {
        step(&mut alice, &mut bob, &mut got, &mut m, true, true);
        assert!(
            Instant::now() < deadline,
            "{}: akış tamamlanmadı: sunulan {sent:?}, teslim {:?}",
            plan.name,
            got.iter().map(Vec::len).collect::<Vec<_>>()
        );
    }
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        while alice.poll_event().is_some() {}
        assert!(bob.recv().is_none(), "{}: tekrar teslim yok", plan.name);
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }
    for (lane, got_lane) in got.iter().enumerate() {
        assert_eq!(
            *got_lane,
            (0..plan.totals[lane]).collect::<Vec<_>>(),
            "{}: şerit {lane}: sırayla, tam bir kez, kayıpsız",
            plan.name
        );
    }
    assert_eq!(bob.delivery_dropped(), 0);
    m.not_admitted_window = bob.data_not_admitted_window();
    let ub = plan.unit_bytes;
    let held_total: usize = m.max_held.iter().sum();
    eprintln!(
        "ÖLÇÜM alıcı-sınırı [{}]: tavan {} birim ({} kB yük); gözlenen en çok toplam {} birim \
         ({} kB) — kanal {} ({} kB), kutu {} ({} kB), sıralayıcı şerit başına {:?} (toplam {} = {} \
         kB); birim yükü {ub} B; duruşta Alice'in aldığı teyit {}; duruşta kanal dolu diye kabul \
         edilmeyen DATA {}; pencere dışı DATA {}",
        plan.name,
        m.bound,
        m.bound * ub / 1000,
        m.max_total,
        m.max_total * ub / 1000,
        m.max_channel,
        m.max_channel * ub / 1000,
        m.max_inbox,
        m.max_inbox * ub / 1000,
        m.max_held,
        held_total,
        held_total * ub / 1000,
        m.acked_in_stall,
        m.not_admitted_full_in_stall,
        m.not_admitted_window
    );
    assert!(
        m.not_admitted_full_in_stall > 0,
        "{}: ön koşul — duruşta geri basınç devreye girmeliydi (kanal dolu → DATA kabul edilmez)",
        plan.name
    );
    assert!(
        m.acked_in_stall > 0,
        "{}: ön koşul — duruşta Bob'da tamponlanan birimler Alice'e teyit edilmiş olmalı",
        plan.name
    );
    assert_eq!(
        m.max_inbox, MAX_INBOX,
        "{}: ön koşul — teslim kutusu duruşta tavanına dolmalıydı",
        plan.name
    );
    // Gönderici hiçbir şeritte alıcının penceresini aşmadı — uygulama dururken de: pencere tabanı
    // veri alıcısının kabul sınırıdır, göndericinin teyit sınırı da aynı olaydan (kabul) gelir.
    assert_eq!(
        m.not_admitted_window, 0,
        "{}: alıcının kabul penceresinin dışına DATA gitti",
        plan.name
    );
    m
}

#[test]
fn receiver_buffer_stays_bounded_with_small_limits() {
    let lanes = LaneConfig {
        admit_widths: [SMALL_WIDTH; 4],
        ..LaneConfig::default()
    };
    let m = run(&Plan {
        name: "küçük sınırlar",
        lanes,
        delivery_queue: SMALL_DELIVERY_QUEUE,
        totals: [250, 100, 500, 6000],
        // Yavaş şeritler duruşun sonuna dek sunmayı sürdürür: duruş boyunca dört şerit gönderir.
        rates: [Some(50), Some(20), Some(100), None],
        unit_bytes: 64,
        drop_every: 97,
        heads_dropped: false,
    });
    assert!(
        m.max_held.iter().sum::<usize>() > 0,
        "ön koşul: sıralayıcı boşluk arkasında birim tutmalıydı"
    );
}

#[test]
fn receiver_buffer_stays_bounded_with_product_limits() {
    // Gönderici hızı: W tabanı 512 (tavanlar toplamı teklif kuyruğuna — 1024 — sığar). Kabul
    // genişlikleri ürünün varsayılanı.
    let lanes = LaneConfig {
        file_window_units: 512,
        caps: [64, 64, 256, 512],
        ..LaneConfig::default()
    };
    assert_eq!(lanes.admit_widths, DEFAULT_ADMIT_WIDTHS);
    let product = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0))).runtime;
    assert_eq!(product.delivery_queue, 1024, "ürün teslim kanalı");
    let m = run(&Plan {
        name: "ürün ayarları",
        lanes,
        delivery_queue: product.delivery_queue,
        // Pencereleri doldurduktan sonra kalan birimler teslim kanalını (1024) da doldurur.
        totals: [700, 700, 700, 6500],
        rates: [None; 4],
        unit_bytes: KamaV1Profile::MAX_UNIT_BYTES,
        drop_every: 0,
        heads_dropped: true,
    });
    for (lane, width) in DEFAULT_ADMIT_WIDTHS.iter().enumerate() {
        assert_eq!(
            m.max_held[lane] as u64,
            width - 1,
            "ön koşul: şerit {lane} sıra başı eksikken penceresini doldurdu"
        );
    }
    assert_eq!(
        m.max_channel, 1024,
        "ön koşul: teslim kanalı duruşta tavanına doldu"
    );
}
