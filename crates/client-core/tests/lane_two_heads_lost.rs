//! [ŞERİT-SIRA · merge öncesi] İki şeritte aynı anda kayıp: dosya şeridinin VE metin şeridinin
//! sıra başı aynı anda düşerken bas-konuş (PTT) akmaya devam eder.
//!
//! DÜZENEK: iki gerçek `ClientTransport` (ürün yapılandırması, şeritler açık), zarflar
//! `MemoryHub`, veri UDP loopback. Bob'un kriptosunu saran sarmalayıcı yalnız iki birime dokunur:
//! dosya şeridinin 0. birimi ve metin şeridinin 0. birimi (şerit etiketli kimlikler,
//! `client_core::unit_id`). Alice sürekli PTT gönderir (her [`PTT_PERIOD`]), aynı anda
//! [`FILE_UNITS`] dosya ve [`TEXT_UNITS`] metin birimi.
//!
//! Yapılandırma iki uçta aynıdır. Şerit başına kabul genişliği (`LaneConfig::admit_widths`) dört
//! şeritte [`ADMIT_WINDOW`] (ürün FILE 4096): pencerenin dolması kısa sürsün; dosya penceresi
//! W = [`FILE_WINDOW`].
//!
//! Kollar:
//! * [`Arm::FirstEmission`] — iki sıra başının yalnız İLK emisyonu düşer; test hiçbir şeyi serbest
//!   bırakmaz, toparlanma motorun ARQ/PTO yoluyla olur (FEC kapalı). Kayıp anında dosya penceresi
//!   (W = [`FILE_WINDOW`]) DOLUDUR (ön koşul olarak sınanır).
//! * [`Arm::UntilRelease`] — iki sıra başının BÜTÜN emisyonları test serbest bırakana dek düşer;
//!   sonra ARQ/PTO taşır (FEC kapalı). Dosya bu sırada [`BIG_FILE_UNITS`] birim akar: sıra başı
//!   eksikken arkası teyit aldıkça şerit ilerler ve ALICININ KABUL PENCERESİNİ doldurur. Serbest bırakma ancak dosya şeridi açıklık sınırına dayandıktan
//!   (`LaneStats::span_blocked[FILE] > 0`) ve [`HOLD`] geçtikten sonradır. Bu kol açıklık sınırı
//!   yokken PTT'nin DURDUĞUNU yakaladı: alıcı pencere dışı dosya birimlerini kabul etmiyor,
//!   motorun uçuşu ACK almayacak dosya birimleriyle doluyor, PTT ve metin tele çıkamıyordu (60 s
//!   içinde metin sıra başının tek emisyonu bile yok).
//! * [`Arm::Fec`] — statik-r FEC (profil 0x05, K = 8, r = 2). Dosya sıra başının DATA emisyonları
//!   HİÇ geçmez: tek yol, dosya bloğu 0'ın onarımlarıyla alıcıda kurtarmadır. Metin sıra başının
//!   DATA emisyonları ve metin bloğu 0'ın onarımları serbest bırakılana dek düşer. Dosya kurtarılıp
//!   RECOVERED Alice'e döner; metin bu sırada eksiktir. RECOVERED yanlış şeride eşlenseydi (örn.
//!   şerit etiketi düşmüş blok kimliği) metin sıra başını "teyitli" sayıp yeniden iletimini iptal
//!   ederdi — onarımları da düştüğü için metin hiç gelmezdi. Serbest bırakılınca metin ARQ ile gelir.
//!
//! İddialar (her kol):
//! 1. PTT zamanında: her PTT biriminin gönderimden teslime süresi ≤ [`PTT_BOUND`], sıra başları
//!    eksikken de.
//! 2. Metin ve dosya toparlanır: kayıptan (serbest bırakmadan) sonra ≤ [`RECOVER_BOUND`].
//! 3. Her şerit kendi içinde sırayla ve tam bir kez teslim edilir; teslim kanalında düşme yok;
//!    durgunlukta tekrar yok.
//! 4. Hiçbir DATA alıcının kabul penceresinin dışına düşmez (`data_not_admitted_window == 0`):
//!    gönderici dosya şeridini alıcının penceresi içinde tutar.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::LaneConfig;
use client_core::memory_link::MemoryHub;
use client_core::runtime::FecRuntimeConfig;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::unit_id::{first_fec_block_of_lane, index_of_lane, tagged};
use client_core::wire::{decode_data, decode_fec};
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

use client_core::lanes::Lane;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
/// Dosya birimi sayısı (ilk-emisyon ve FEC kolları).
const FILE_UNITS: u64 = 400;
/// Dosya birimi sayısı (serbest bırakmalı kol; > [`ADMIT_WINDOW`]).
const BIG_FILE_UNITS: u64 = 600;
/// Şerit başına kabul genişliği (iki uçta `LaneConfig::admit_widths`).
const ADMIT_WINDOW: u64 = 256;
/// `LaneStats::span_blocked` içinde FILE şeridinin indeksi.
const FILE_IDX: usize = 3;
/// PTT'nin en az bu kadar sürekli aktığı süre (kol ne kadar çabuk toparlanırsa toparlansın).
const MIN_PTT_RUN: Duration = Duration::from_secs(1);
/// Metin birimi sayısı (> FEC bloğu K = 8).
const TEXT_UNITS: u64 = 12;
/// Dosya penceresi tabanı.
const FILE_WINDOW: u32 = 64;
/// PTT gönderim aralığı.
const PTT_PERIOD: Duration = Duration::from_millis(20);
/// Bir PTT biriminin gönderimden teslime üst süresi.
const PTT_BOUND: Duration = Duration::from_secs(1);
/// Sıra başı düşerken (serbest bırakmalı kollarda) bekleme.
const HOLD: Duration = Duration::from_millis(1500);
/// Kayıptan / serbest bırakmadan sonra metin ve dosyanın tamamlanma üst süresi.
const RECOVER_BOUND: Duration = Duration::from_secs(10);
/// Bütün akışın üst süresi.
const WAIT: Duration = Duration::from_secs(60);

const TAG_FILE: u8 = 0xF1;
const TAG_TEXT: u8 = 0x7E;
const TAG_PTT: u8 = 0x9A;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    FirstEmission,
    UntilRelease,
    Fec,
}

fn file_head() -> u64 {
    tagged(index_of_lane(Lane::File), 0)
}

fn text_head() -> u64 {
    tagged(index_of_lane(Lane::Text), 0)
}

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

#[derive(Default)]
struct Drops {
    file: AtomicU64,
    text: AtomicU64,
    text_repairs: AtomicU64,
}

/// Gerçek kripto + alıcı yönünde yalnız iki sıra başına (ve FEC kolunda metin bloğu 0'ın
/// onarımlarına) dokunan düşürücü (S3 K5: `open` → `None`, durum değişmez).
struct LaneHeadDrop {
    inner: NoiseWireProtection,
    arm: Arm,
    release: Arc<AtomicBool>,
    drops: Arc<Drops>,
}

impl LaneHeadDrop {
    fn drop_now(&self, counter: &AtomicU64, until_release: bool) -> bool {
        let drop = if until_release {
            !self.release.load(Ordering::Relaxed)
        } else {
            counter.load(Ordering::Relaxed) == 0
        };
        if drop {
            counter.fetch_add(1, Ordering::Relaxed);
        }
        drop
    }
}

impl PacketProtection for LaneHeadDrop {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        if let Some((_, u, _)) = decode_data(&od.frame) {
            let dropped = if u == file_head() {
                match self.arm {
                    Arm::FirstEmission => self.drop_now(&self.drops.file, false),
                    Arm::UntilRelease => self.drop_now(&self.drops.file, true),
                    Arm::Fec => {
                        self.drops.file.fetch_add(1, Ordering::Relaxed);
                        true // hiç geçmez: yalnız FEC kurtarabilir
                    }
                }
            } else if u == text_head() {
                self.drop_now(&self.drops.text, self.arm != Arm::FirstEmission)
            } else {
                false
            };
            if dropped {
                return None;
            }
        } else if self.arm == Arm::Fec {
            // v0 telinde FEC çerçevesi yalnız onarım taşır (kaynaklar DATA ile gider).
            if let Some((h, _)) = decode_fec(&od.frame) {
                if h.block_id == first_fec_block_of_lane(index_of_lane(Lane::Text))
                    && self.drop_now(&self.drops.text_repairs, true)
                {
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

impl SessionControl for LaneHeadDrop {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        self.inner.command(cmd)
    }
    fn poll_event(&mut self) -> Option<SessionEvent> {
        self.inner.poll_event()
    }
}

fn file_units(arm: Arm) -> u64 {
    match arm {
        Arm::UntilRelease => BIG_FILE_UNITS,
        Arm::FirstEmission | Arm::Fec => FILE_UNITS,
    }
}

fn cfg(arm: Arm) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.fec = match arm {
        Arm::Fec => Some(FecRuntimeConfig {
            profile_id: 0x05,
            manual_r: Some(2),
        }),
        Arm::FirstEmission | Arm::UntilRelease => None,
    };
    let mut lanes = LaneConfig::with_file_window(FILE_WINDOW);
    lanes.admit_widths = [ADMIT_WINDOW; 4];
    cfg.lanes = Some(lanes);
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

/// Bir kolun ölçümü.
#[derive(Debug)]
struct Measured {
    ptt_sent: u64,
    ptt_max: Duration,
    ptt_p50: Duration,
    text_done_after: Duration,
    file_done_after: Duration,
    file_drops: u64,
    text_drops: u64,
    text_repair_drops: u64,
    not_admitted_window: u64,
    /// Dosya şeridinin gözlenen en çok teyitsiz birimi (W doluluğu).
    max_file_inflight: usize,
    /// Dosya şeridinin açıklık sınırına takıldığı pompa turu sayısı.
    file_span_blocked: u64,
    /// Sıra başları eksikken (serbest bırakmadan / toparlanmadan önce) teslim edilen PTT.
    ptt_while_heads_missing: usize,
}

/// Alice'in sunum durumu.
struct Tx {
    h: SessionHandle,
    next_file: u64,
    next_text: u64,
    ptt_sent_at: Vec<Instant>,
    last_ptt: Option<Instant>,
    file_units: u64,
    max_file_inflight: usize,
}

struct Rx {
    files: Vec<u64>,
    texts: Vec<u64>,
    ptts: Vec<u64>,
    ptt_lat: Vec<Duration>,
    file_done: Option<Instant>,
    text_done: Option<Instant>,
}

#[allow(clippy::too_many_lines)]
fn run(arm: Arm) -> Measured {
    let hub = MemoryHub::new();
    let release = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(Drops::default());
    let mut alice = ClientTransport::new(cfg(arm), crypto(ALICE), Box::new(hub.link(id(ALICE))))
        .expect("alice");
    let mut bob = ClientTransport::new(
        cfg(arm),
        LaneHeadDrop {
            inner: crypto(BOB),
            arm,
            release: Arc::clone(&release),
            drops: Arc::clone(&drops),
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

    let mut tx = Tx {
        h,
        next_file: 0,
        next_text: 0,
        ptt_sent_at: Vec::new(),
        last_ptt: None,
        file_units: file_units(arm),
        max_file_inflight: 0,
    };
    let mut ptt_on = true;
    let mut rx = Rx {
        files: Vec::new(),
        texts: Vec::new(),
        ptts: Vec::new(),
        ptt_lat: Vec::new(),
        file_done: None,
        text_done: None,
    };
    let started = Instant::now();

    // Bir tur: PTT (vadesi geldiyse), dosya ve metin sunumu (geri basınca saygılı), Bob'un
    // teslimleri.
    let step = |alice: &mut ClientTransport<NoiseWireProtection>,
                bob: &mut ClientTransport<LaneHeadDrop>,
                tx: &mut Tx,
                rx: &mut Rx,
                ptt_on: bool| {
        let h = tx.h;
        if ptt_on && tx.last_ptt.is_none_or(|t| t.elapsed() >= PTT_PERIOD) {
            let n = tx.ptt_sent_at.len() as u64;
            match alice.send(
                h,
                unit(TAG_PTT, n),
                DeliveryPolicy::Reliable,
                TrafficClass::VoRel,
            ) {
                Ok(()) => {
                    tx.ptt_sent_at.push(Instant::now());
                    tx.last_ptt = Some(Instant::now());
                }
                Err(TransportError::Backpressure) => {}
                Err(e) => panic!("ptt send: {e:?}"),
            }
        }
        while tx.next_file < tx.file_units {
            match alice.send(
                h,
                unit(TAG_FILE, tx.next_file),
                DeliveryPolicy::Reliable,
                TrafficClass::Bk,
            ) {
                Ok(()) => tx.next_file += 1,
                Err(TransportError::Backpressure) => break,
                Err(e) => panic!("dosya send: {e:?}"),
            }
        }
        while tx.next_text < TEXT_UNITS {
            match alice.send(
                h,
                unit(TAG_TEXT, tx.next_text),
                DeliveryPolicy::Reliable,
                TrafficClass::Be,
            ) {
                Ok(()) => tx.next_text += 1,
                Err(TransportError::Backpressure) => break,
                Err(e) => panic!("metin send: {e:?}"),
            }
        }
        while alice.poll_event().is_some() {}
        if let Some((_, inflight)) = alice.lane_backlog(h) {
            tx.max_file_inflight = tx.max_file_inflight.max(inflight);
        }
        while let Some(d) = bob.recv() {
            let (tag, n) = parse(&d.unit);
            match tag {
                TAG_FILE => {
                    rx.files.push(n);
                    if rx.files.len() as u64 == tx.file_units {
                        rx.file_done = Some(Instant::now());
                    }
                }
                TAG_TEXT => {
                    rx.texts.push(n);
                    if rx.texts.len() as u64 == TEXT_UNITS {
                        rx.text_done = Some(Instant::now());
                    }
                }
                TAG_PTT => {
                    let sent = tx.ptt_sent_at[usize::try_from(n).expect("n")];
                    rx.ptt_lat.push(sent.elapsed());
                    rx.ptts.push(n);
                }
                _ => panic!("bilinmeyen birim"),
            }
        }
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(1));
    };

    // Faz 1 — iki sıra başı düşerken. Serbest bırakmalı kollarda HOLD boyunca sürer; ilk-emisyon
    // kolunda kayıp anından toparlanmaya dek (serbest bırakma yok).
    let lost_at;
    loop {
        step(&mut alice, &mut bob, &mut tx, &mut rx, ptt_on);
        let heads_dropped =
            drops.file.load(Ordering::Relaxed) > 0 && drops.text.load(Ordering::Relaxed) > 0;
        if arm == Arm::FirstEmission && heads_dropped {
            lost_at = Instant::now();
            break;
        }
        let span_reached = arm != Arm::UntilRelease
            || alice.lane_stats().expect("şeritler").span_blocked[FILE_IDX] > 0;
        if arm != Arm::FirstEmission && heads_dropped && span_reached && started.elapsed() >= HOLD {
            // Ön koşul: sıra başları hâlâ düşüyor, iki şeridin arkası Bob'da tutuluyor.
            assert!(
                rx.texts.is_empty(),
                "metin sıra başı düşerken metin teslim edilmez"
            );
            if arm == Arm::UntilRelease {
                assert!(
                    rx.files.is_empty(),
                    "dosya sıra başı düşerken dosya teslim edilmez"
                );
            }
            let held: u64 = bob
                .reorder_diags()
                .iter()
                .map(|(_, d)| u64::from(d.held_units))
                .sum();
            assert!(held > 0, "sıra başlarının arkasında birim tutulmalı");
            if arm == Arm::Fec {
                // Dosya sıra başı yalnız FEC ile gelebilir; metin onarımları düşürülüyor.
                if rx.file_done.is_none() {
                    assert!(
                        started.elapsed() < WAIT,
                        "FEC: dosya bloğu 0 kurtarılmadı (dosya {}/{})",
                        rx.files.len(),
                        tx.file_units
                    );
                    continue;
                }
                assert!(
                    drops.text_repairs.load(Ordering::Relaxed) > 0,
                    "ön koşul: metin bloğu 0'ın onarımı düşmeliydi"
                );
                assert!(
                    rx.texts.is_empty(),
                    "dosya RECOVERED'ı metin sıra başını serbest bırakmamalı"
                );
            }
            release.store(true, Ordering::Relaxed);
            lost_at = Instant::now();
            break;
        }
        assert!(
            started.elapsed() < WAIT,
            "ön koşul sağlanmadı: düşen dosya {}, metin {}; açıklık sınırı {span_reached};              sunulan dosya {}, teslim PTT {}/{}",
            drops.file.load(Ordering::Relaxed),
            drops.text.load(Ordering::Relaxed),
            tx.next_file,
            rx.ptts.len(),
            tx.ptt_sent_at.len()
        );
    }
    let ptt_while_heads_missing = rx.ptts.len();

    // Faz 2 — toparlanma: metin ve dosya tamamlanır, PTT akmayı sürdürür (en az MIN_PTT_RUN).
    while rx.file_done.is_none() || rx.text_done.is_none() || started.elapsed() < MIN_PTT_RUN {
        step(&mut alice, &mut bob, &mut tx, &mut rx, ptt_on);
        assert!(
            lost_at.elapsed() < RECOVER_BOUND,
            "{arm:?}: {RECOVER_BOUND:?} içinde toparlanmadı — dosya {}/{} (sunulan \
             {}), metin {}/{TEXT_UNITS}; düşen dosya {} metin {}; Bob {:?}",
            rx.files.len(),
            tx.file_units,
            tx.next_file,
            rx.texts.len(),
            drops.file.load(Ordering::Relaxed),
            drops.text.load(Ordering::Relaxed),
            bob.reorder_diags()
        );
    }
    // Faz 3 — PTT durur; gönderilen her PTT gelir.
    ptt_on = false;
    let ptt_total = tx.ptt_sent_at.len() as u64;
    while (rx.ptts.len() as u64) < ptt_total {
        step(&mut alice, &mut bob, &mut tx, &mut rx, ptt_on);
        assert!(
            Instant::now() < deadline,
            "PTT tamamlanmadı: {}/{ptt_total}",
            rx.ptts.len()
        );
    }
    // Durgunluk: tekrar teslim yok.
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        while alice.poll_event().is_some() {}
        assert!(bob.recv().is_none(), "tekrar teslim yok");
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(
        rx.files,
        (0..tx.file_units).collect::<Vec<_>>(),
        "{arm:?}: dosya şeridi sırayla, tam bir kez"
    );
    assert_eq!(
        rx.texts,
        (0..TEXT_UNITS).collect::<Vec<_>>(),
        "{arm:?}: metin şeridi sırayla, tam bir kez"
    );
    assert_eq!(
        rx.ptts,
        (0..ptt_total).collect::<Vec<_>>(),
        "{arm:?}: PTT şeridi sırayla, tam bir kez"
    );
    assert_eq!(bob.delivery_dropped(), 0);
    let mut lat = rx.ptt_lat.clone();
    lat.sort_unstable();
    let m = Measured {
        ptt_sent: ptt_total,
        ptt_max: *lat.last().expect("ptt"),
        ptt_p50: lat[lat.len() / 2],
        text_done_after: rx
            .text_done
            .expect("metin")
            .saturating_duration_since(lost_at),
        file_done_after: rx
            .file_done
            .expect("dosya")
            .saturating_duration_since(lost_at),
        file_drops: drops.file.load(Ordering::Relaxed),
        text_drops: drops.text.load(Ordering::Relaxed),
        text_repair_drops: drops.text_repairs.load(Ordering::Relaxed),
        not_admitted_window: bob.data_not_admitted_window(),
        max_file_inflight: tx.max_file_inflight,
        file_span_blocked: alice.lane_stats().expect("şeritler").span_blocked[FILE_IDX],
        ptt_while_heads_missing,
    };
    eprintln!("ÖLÇÜM iki-şerit-kayıp {arm:?}: {m:?}");
    assert!(
        m.ptt_max <= PTT_BOUND,
        "{arm:?}: PTT gecikmesi {:?} > {PTT_BOUND:?} (p50 {:?})",
        m.ptt_max,
        m.ptt_p50
    );
    assert!(
        m.ptt_sent > 0 && m.text_done_after <= RECOVER_BOUND && m.file_done_after <= RECOVER_BOUND,
        "{arm:?}: {m:?}"
    );
    assert_eq!(
        m.not_admitted_window, 0,
        "{arm:?}: alıcının kabul penceresinin dışına DATA gitti"
    );
    m
}

/// Serbest bırakma yok: iki sıra başının ilk emisyonu düşer, pencere dolar, PTT sürer;
/// toparlanma ARQ/PTO ile.
#[test]
fn two_lane_heads_lost_first_emission_recovered_by_arq_pto() {
    let m = run(Arm::FirstEmission);
    assert_eq!(
        (m.file_drops, m.text_drops),
        (1, 1),
        "yalnız ilk emisyonlar düşer"
    );
    assert!(
        m.max_file_inflight >= FILE_WINDOW as usize,
        "ön koşul: dosya penceresi en az tabanı (W = {FILE_WINDOW}) kadar doldu: {}",
        m.max_file_inflight
    );
}

/// İki sıra başı test serbest bırakana dek düşer; PTT bu sırada zamanında; sonra toparlanma.
#[test]
fn two_lane_heads_lost_until_release() {
    let m = run(Arm::UntilRelease);
    assert!(m.file_drops >= 1 && m.text_drops >= 1);
    assert!(
        m.file_span_blocked > 0,
        "ön koşul: dosya şeridi alıcının kabul penceresine dayandı"
    );
    assert!(
        m.ptt_while_heads_missing as u128 >= HOLD.as_millis() / PTT_PERIOD.as_millis() / 2,
        "sıra başları eksikken PTT akmayı sürdürdü: {} birim",
        m.ptt_while_heads_missing
    );
}

/// FEC: dosya sıra başı yalnız kendi bloğunun onarımlarıyla kurtarılır; RECOVERED metin şeridinin
/// eksik sıra başını serbest bırakmaz.
#[test]
fn two_lane_heads_lost_fec_recovered_maps_to_its_own_lane() {
    let m = run(Arm::Fec);
    assert!(m.file_drops >= 1 && m.text_drops >= 1 && m.text_repair_drops >= 1);
}
