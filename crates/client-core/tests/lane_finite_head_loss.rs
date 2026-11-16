//! [ŞERİT-SIRA · ürün hedefi] Akışlar birbirini bloklamaz: büyük bir dosyanın birimleri SONLU bir
//! süre art arda kaybolurken bas-konuş (PTT) ve metin (TEXT) ilerler; yol açılınca dosya tamamlanır.
//!
//! # Hangi boşluğu kapatır
//!
//! Port motorunda (`adaptive-transport`, CC-PORT) üç kural bir araya gelince ürün boyu (1 013 B)
//! bir dosya biriminin art arda kaybı öbür şeritleri durdurabilir:
//!
//! 1. tıkanıklık penceresi en az 4 × MTU = 4 800 B'dir (`cc/bbr.rs` `calculate_min_window`); ürün
//!    boyu birim motorun uçuş muhasebesinde 1 046 B'dir, yani pencere dört-beş birimle dolar;
//! 2. PTO probu pencereden muaftır ve en küçük kimlikli teyitsiz birimi taşır; kopyası uçuşta kalır,
//!    birim başına en çok 8 kopya (`sender.rs` `oldest_probe_target_or_pending`,
//!    `CC_MAX_UNIT_COPIES_IN_FLIGHT`);
//! 3. kayıp yalnız daha yeni bir paket ACK alınca ilan edilir (RFC 9002 §6.1).
//!
//! Son ACK'ten sonra gönderilen her şey kaybolursa uçuş, hiç ACK almayacak ve kayıp da ilan
//! edilmeyecek kopyalarla dolar. PTT ve metin pencereye sığmaz, ACK gelmez, PTO geri çekilir.
//! Her yeniden iletimin taze paket numarası taşıması bunu önlemez: sonlu bir kesintide ya da
//! boyuta duyarlı bir yolda aynı birimin art arda emisyonları birlikte kaybolur. Mevcut şerit
//! testleri 64 B birim kullanır (`lane_two_heads_lost`, `lane_span_per_lane`); bu kopyalar
//! pencereyi dolduramaz.
//!
//! # Düzenek
//!
//! İki gerçek `ClientTransport`, ürün yapılandırması (`ClientTransportConfig::product`: port motoru,
//! şeritler açık, dosya penceresi tabanı W = 5 ve yolun BDP'si, kabul genişlikleri
//! `DEFAULT_ADMIT_WIDTHS` = [256, 256, 256, 4096], uyarlamalı FEC). Zarflar `MemoryHub`, veri UDP
//! loopback. Bob'un kriptosunu saran sarmalayıcı (`PathOutage`) yolu temsil eder: Alice → Bob
//! yönündeki datagramları kesinti boyunca düşürür; kesinti bitince hiçbir şeye dokunmaz. Bob → Alice
//! (ACK) yönü etkilenmez.
//!
//! Alice aynı anda gönderir:
//! * PTT: her [`PTT_PERIOD`]'da bir [`PTT_BYTES`] B'lik birim, dosya bitene dek;
//! * TEXT: [`TEXT_UNITS`] × [`TEXT_BYTES`] B, her [`TEXT_PERIOD`]'da bir (hepsi kesinti içinde);
//! * FILE: [`FILE_UNITS`] birim, geri basının izin verdiği hızda.
//!
//! Kollar ([`Path`]):
//! * [`Path::HeadOutage`] — sıra başı sonlu kesinti: dosya şeridinin 0. biriminin bütün DATA
//!   emisyonları ve dosya şeridinin ilk FEC bloğunun (0. birimi taşıyan blok) onarımları kesinti
//!   boyunca düşer, sonra geçer. Öbür birimler etkilenmez. Onarımlar da düşer, çünkü ürün FEC'i
//!   uyarlamalıdır: ilk koşuda (onarımlar geçerken) sıra başı 3–4 DATA kaybından sonra onarımla
//!   kurtarıldı ve kesinti hiç sınanmadı.
//! * [`Path::SizeSensitive`] — boyuta duyarlı yol: kesinti boyunca [`SIZE_LIMIT`] B'tan büyük her
//!   datagram düşer (DATA, yeniden iletim, prob, FEC onarımı); küçükler geçer. Bütün dosya birimleri
//!   etkilenir; PTT ve TEXT etkilenmez.
//!
//! Her kol 5 s ve 15 s kesintiyle koşar; [`Path::HeadOutage`] ayrıca 64 B dosya birimiyle
//! (karşılaştırma: kopyalar pencereyi dolduramaz).
//!
//! # Eşikler (ölçümden önce, ürün hedefinden)
//!
//! * [`PTT_BOUND`] = 1 s: her PTT biriminin gönderimden (`send` kabulü) teslime süresi, kesinti
//!   sırasında da. Loopback RTT 1 ms'nin altında; taban PTO ≈ srtt + 4·rttvar + 25 ms ≈ 30 ms.
//!   Dosyanın kaybı PTT'yi geciktirmemeli; sınır yalnız iş parçacığı zamanlaması ve yük titremesi
//!   içindir. 1 s'yi aşan bekleme, PTT'nin dosya kaybına bağlı bir şeyi (geri çekilmiş PTO, duran
//!   ACK saati) beklediğini gösterir. `lane_two_heads_lost` ile aynı.
//! * [`TEXT_BOUND`] = 1 s: aynı gerekçe, metin şeridi dosyadan bağımsızdır. Son metin kesinti
//!   bitmeden en az 1 s önce sunulur, yani bütün metin kesinti içinde teslim edilmiş olmalı.
//! * [`RECOVER_BOUND`] = 10 s: yol açıldıktan sonra dosyanın tamamlanması (yani akış başından
//!   T_out + 10 s). Sıra başı kolunda kalan iş tek birimin bir yeniden iletimidir (arkası Bob'da
//!   tutulur). Boyut kolunda bütün dosya (500 × 1 046 B ≈ 523 KB) yeniden gider; en küçük
//!   pencereyle (4 800 B) ve yükteki RTT ile (≤ 20 ms) bile ≈ 240 KB/s, ≈ 2,2 s. 10 s bunun
//!   üstünde ve PTO geri çekilme tavanının (30 s) altında: ACK saati kesinti boyunca sürdüyse dosya
//!   hemen devam eder; durduysa ilk prob geri çekilmiş PTO kadar gecikir ve sınır aşılır.
//! * Alıcı tamponu her an `receiver_buffer_bound_units` içinde; pencere dışı DATA 0. Her şerit
//!   kendi içinde sırayla ve tam bir kez; teslim kanalında düşme yok; durgunlukta tekrar yok.
//!
//! # Teşhis
//!
//! `FHL_DIAG` ortam değişkeni kuruluysa Alice'in canlı teşhis aynası açılır ve her
//! [`DIAG_PERIOD`]'da motor durumu yazılır (uçuş baytı ve kaydı, kayıp ve gönderim kuyrukları, PTO
//! turu ve geri çekilmiş PTO, son ACK'ten beri, en eski teyitsiz birim). Değişken yokken ayna
//! kapalıdır (ürün varsayılanı); davranış aynıdır.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use client_core::lanes::{Lane, DEFAULT_ADMIT_WIDTHS};
use client_core::memory_link::MemoryHub;
use client_core::runtime::KamaV1Profile;
use client_core::transport::{receiver_buffer_bound_units, ClientTransport, ClientTransportConfig};
use client_core::unit_id::{
    first_fec_block_of_lane, index_of_lane, lane_index_of, lane_seq_of, tagged,
};
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

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

/// PTT gönderim aralığı.
const PTT_PERIOD: Duration = Duration::from_millis(100);
/// PTT birim boyu (bayt).
const PTT_BYTES: usize = 64;
/// Metin birimi sayısı.
const TEXT_UNITS: u64 = 40;
/// Metin birim boyu (bayt).
const TEXT_BYTES: usize = 100;
/// Metin gönderim aralığı: 40 birim 3,2 s'de, en kısa kesintinin (5 s) içinde.
const TEXT_PERIOD: Duration = Duration::from_millis(80);
/// Dosya birimi sayısı.
const FILE_UNITS: u64 = 500;
/// Boyuta duyarlı yolun geçirdiği en büyük datagram (bayt; PTT ve TEXT bunun altında, ürün boyu
/// dosya birimi üstünde).
const SIZE_LIMIT: usize = 600;

/// PTT gönderimden teslime üst süresi.
const PTT_BOUND: Duration = Duration::from_secs(1);
/// TEXT gönderimden teslime üst süresi.
const TEXT_BOUND: Duration = Duration::from_secs(1);
/// Yol açıldıktan sonra dosyanın tamamlanma üst süresi.
const RECOVER_BOUND: Duration = Duration::from_secs(10);
/// Sınır aşılsa da gözlemin sürdüğü ek süre (kırmızı koşuda dosyanın ne zaman bittiğini ölçmek
/// için; PTO geri çekilme tavanı 30 s'nin üstünde).
const OBSERVE_EXTRA: Duration = Duration::from_secs(40);
/// `FHL_DIAG` döküm aralığı.
const DIAG_PERIOD: Duration = Duration::from_millis(250);

const TAG_FILE: u8 = 0xF1;
const TAG_TEXT: u8 = 0x7E;
const TAG_PTT: u8 = 0x9A;

/// Yolun kesinti biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Path {
    /// Dosya şeridinin 0. biriminin her DATA emisyonu ve o birimin FEC bloğunun her onarımı düşer.
    HeadOutage,
    /// [`SIZE_LIMIT`]'ten büyük her datagram düşer.
    SizeSensitive,
}

/// Bir kol.
#[derive(Debug, Clone, Copy)]
struct Arm {
    path: Path,
    outage: Duration,
    file_bytes: usize,
}

fn file_head() -> u64 {
    tagged(index_of_lane(Lane::File), 0)
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

/// Yolun düşürdükleri (Alice → Bob).
#[derive(Default)]
struct PathDrops {
    /// Dosya sıra başının düşen DATA emisyonları.
    head: AtomicU64,
    /// Düşen dosya DATA datagramları (sıra başı dahil).
    file: AtomicU64,
    /// Düşen FEC çerçeveleri.
    fec: AtomicU64,
    /// Düşen öbür datagramlar (boyut kolu: denetim çerçeveleri).
    other: AtomicU64,
    /// Kesinti boyunca geçen datagramlar.
    passed_in_outage: AtomicU64,
    /// Bob'un gördüğü FEC çerçeveleri (geçen ya da düşen).
    fec_seen: AtomicU64,
}

/// Gerçek kripto + yol: `outage` kalkıkken Alice → Bob datagramlarını kolun biçimine göre düşürür
/// (S3 K5: `open` → `None`).
struct PathOutage {
    inner: NoiseWireProtection,
    path: Path,
    outage: Arc<AtomicBool>,
    drops: Arc<PathDrops>,
}

impl PacketProtection for PathOutage {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let wire_len = wire.len();
        let od = self.inner.open(wire)?;
        let data_unit = decode_data(&od.frame).map(|(_, u, _)| u);
        // v0 telinde FEC çerçevesi yalnız onarım taşır (kaynaklar DATA ile gider).
        let fec_block = if data_unit.is_none() {
            decode_fec(&od.frame).map(|(h, _)| h.block_id)
        } else {
            None
        };
        let is_fec = fec_block.is_some();
        if is_fec {
            self.drops.fec_seen.fetch_add(1, Ordering::Relaxed);
        }
        if !self.outage.load(Ordering::Relaxed) {
            return Some(od);
        }
        let is_file = data_unit.is_some_and(|u| lane_index_of(u) == index_of_lane(Lane::File));
        let drop = match self.path {
            Path::HeadOutage => {
                data_unit == Some(file_head())
                    || fec_block == Some(first_fec_block_of_lane(index_of_lane(Lane::File)))
            }
            Path::SizeSensitive => wire_len > SIZE_LIMIT,
        };
        if !drop {
            self.drops.passed_in_outage.fetch_add(1, Ordering::Relaxed);
            return Some(od);
        }
        if data_unit == Some(file_head()) {
            self.drops.head.fetch_add(1, Ordering::Relaxed);
        }
        if is_file {
            self.drops.file.fetch_add(1, Ordering::Relaxed);
        } else if is_fec {
            self.drops.fec.fetch_add(1, Ordering::Relaxed);
        } else {
            self.drops.other.fetch_add(1, Ordering::Relaxed);
        }
        None
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

impl SessionControl for PathOutage {
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
    cfg
}

fn unit(tag: u8, n: u64, len: usize) -> PreparedUnit {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(&n.to_be_bytes());
    bytes.resize(len, 0);
    PreparedUnit {
        bytes,
        compression_policy: CompressionPolicy::Never,
    }
}

fn parse(d: &[u8]) -> (u8, u64) {
    (d[0], u64::from_be_bytes(d[1..9].try_into().expect("8 B")))
}

/// Bir kolun ölçümü (süreler akış başına göre değilse adında belirtilir). Alanların bir kısmı
/// yalnız `ÖLÇÜM` satırında (`Debug`) okunur.
#[derive(Debug)]
#[allow(dead_code)]
struct Measured {
    outage: Duration,
    /// Kesintinin gerçekte sürdüğü (test döngüsünün kapattığı an).
    outage_actual: Duration,
    ptt_sent: u64,
    ptt_refused: u64,
    ptt_p50: Duration,
    ptt_max: Duration,
    /// Kesinti içinde sunulan PTT'lerin en büyük gecikmesi.
    ptt_max_in_outage: Duration,
    /// Kesinti içinde ardışık iki PTT teslimi arasındaki en uzun boşluk.
    ptt_gap_in_outage: Duration,
    /// Kesinti içinde sunulan / kesinti bitmeden teslim edilen PTT.
    ptt_sent_in_outage: usize,
    ptt_delivered_in_outage: usize,
    text_max: Duration,
    texts_in_outage: usize,
    /// Dosyanın kesinti bitmeden teslim edilen birimi (ön koşul: 0).
    files_in_outage: usize,
    /// Yol açıldıktan dosya tamamlanana dek.
    file_done_after_open: Option<Duration>,
    head_drops: u64,
    file_drops: u64,
    fec_drops: u64,
    other_drops: u64,
    fec_seen: u64,
    passed_in_outage: u64,
    buffer_bound: usize,
    buffer_max: usize,
    not_admitted_window: u64,
}

/// Alice'in sunum durumu.
struct Tx {
    h: SessionHandle,
    file_bytes: usize,
    next_file: u64,
    ptt_sent_at: Vec<Instant>,
    ptt_refused: u64,
    text_sent_at: Vec<Instant>,
    ptt_on: bool,
}

/// Bob'un teslimleri.
#[derive(Default)]
struct Rx {
    files: Vec<u64>,
    texts: Vec<u64>,
    ptts: Vec<u64>,
    ptt_lat: Vec<Duration>,
    ptt_at: Vec<Instant>,
    text_lat: Vec<Duration>,
    text_at: Vec<Instant>,
    file_done: Option<Instant>,
    buffer_max: usize,
}

/// Bir tur: PTT ve metin (vadesi geldiyse), dosya (geri basınca saygılı), Bob'un teslimleri, alıcı
/// tamponu sınırı.
fn step(
    alice: &mut ClientTransport<NoiseWireProtection>,
    bob: &mut ClientTransport<PathOutage>,
    tx: &mut Tx,
    rx: &mut Rx,
    started: Instant,
    bound: usize,
) {
    let h = tx.h;
    let ptt_due = started + PTT_PERIOD * u32::try_from(tx.ptt_sent_at.len()).expect("ptt");
    if tx.ptt_on && Instant::now() >= ptt_due {
        let n = tx.ptt_sent_at.len() as u64;
        match alice.send(
            h,
            unit(TAG_PTT, n, PTT_BYTES),
            DeliveryPolicy::Reliable,
            TrafficClass::VoRel,
        ) {
            Ok(()) => tx.ptt_sent_at.push(Instant::now()),
            Err(TransportError::Backpressure) => tx.ptt_refused += 1,
            Err(e) => panic!("ptt send: {e:?}"),
        }
    }
    let text_due = started + TEXT_PERIOD * u32::try_from(tx.text_sent_at.len()).expect("text");
    if (tx.text_sent_at.len() as u64) < TEXT_UNITS && Instant::now() >= text_due {
        let n = tx.text_sent_at.len() as u64;
        match alice.send(
            h,
            unit(TAG_TEXT, n, TEXT_BYTES),
            DeliveryPolicy::Reliable,
            TrafficClass::Be,
        ) {
            Ok(()) => tx.text_sent_at.push(Instant::now()),
            Err(TransportError::Backpressure) => {}
            Err(e) => panic!("metin send: {e:?}"),
        }
    }
    while tx.next_file < FILE_UNITS {
        match alice.send(
            h,
            unit(TAG_FILE, tx.next_file, tx.file_bytes),
            DeliveryPolicy::Reliable,
            TrafficClass::Bk,
        ) {
            Ok(()) => tx.next_file += 1,
            Err(TransportError::Backpressure) => break,
            Err(e) => panic!("dosya send: {e:?}"),
        }
    }
    while alice.poll_event().is_some() {}
    while let Some(d) = bob.recv() {
        let (tag, n) = parse(&d.unit);
        let i = usize::try_from(n).expect("n");
        match tag {
            TAG_FILE => {
                rx.files.push(n);
                if rx.files.len() as u64 == FILE_UNITS {
                    rx.file_done = Some(Instant::now());
                }
            }
            TAG_TEXT => {
                rx.text_lat.push(tx.text_sent_at[i].elapsed());
                rx.text_at.push(Instant::now());
                rx.texts.push(n);
            }
            TAG_PTT => {
                rx.ptt_lat.push(tx.ptt_sent_at[i].elapsed());
                rx.ptt_at.push(Instant::now());
                rx.ptts.push(n);
            }
            _ => panic!("bilinmeyen birim"),
        }
    }
    while bob.poll_event().is_some() {}
    let (channel, inbox, held) = bob.receiver_buffered_for_test();
    let total = channel + inbox + held.iter().sum::<usize>();
    assert!(
        total <= bound,
        "alıcı toplam tampon sınırı aşıldı: {total} > {bound} (kanal {channel}, kutu {inbox}, \
         sıralayıcı {held:?})"
    );
    rx.buffer_max = rx.buffer_max.max(total);
    std::thread::sleep(Duration::from_millis(1));
}

/// `FHL_DIAG`: Alice'in motor durumu (canlı ayna).
fn diag_line(
    alice: &ClientTransport<NoiseWireProtection>,
    bob: &ClientTransport<PathOutage>,
    tx: &Tx,
    rx: &Rx,
    started: Instant,
    outage_on: bool,
) {
    let st = alice.lane_stats().expect("şeritler");
    let d = alice.session_live_diags();
    let ctl = d.first().map(|(_, d)| {
        (
            d.control.state,
            d.control.inflight_bytes,
            d.retransmits,
            d.tail_probes,
            d.srtt_us,
            d.tx_datagrams,
            d.rx_datagrams,
        )
    });
    let rec = d.first().and_then(|(_, d)| {
        d.recovery.as_ref().map(|r| {
            (
                r.timers.inflight_len,
                r.timers.pending_lost_len,
                r.timers.tx_queue_len,
                r.timers.tail_probe_index,
                r.timers.tail_pto_ns / 1_000_000,
                r.since_last_ack_ns.map(|n| n / 1_000_000),
                r.oldest_unacked.map(|u| {
                    (
                        lane_index_of(u.unit_id),
                        lane_seq_of(u.unit_id),
                        u.emission_decisions,
                    )
                }),
            )
        })
    });
    eprintln!(
        "FHL t={:?} kesinti={outage_on} ptt {}/{} metin {}/{} dosya sunulan {} teslim {} \
         verilen={:?} bob={:?} motor(durum, uçuş B, retx, prob, srtt µs, tx, rx)={ctl:?} \
         kurtarma(uçuş, kayıp, gönderim, PTO turu, PTO ms, son ACK ms, en eski (şerit, sıra, \
         emisyon))={rec:?}",
        started.elapsed(),
        rx.ptts.len(),
        tx.ptt_sent_at.len(),
        rx.texts.len(),
        tx.text_sent_at.len(),
        tx.next_file,
        rx.files.len(),
        st.given,
        bob.receiver_buffered_for_test(),
    );
}

#[allow(clippy::too_many_lines)]
fn run(arm: Arm) -> Measured {
    let hub = MemoryHub::new();
    let outage = Arc::new(AtomicBool::new(false));
    let drops = Arc::new(PathDrops::default());
    let diag_on = std::env::var_os("FHL_DIAG").is_some();
    let mut alice_cfg = cfg();
    alice_cfg.runtime.live_diag = diag_on;
    let bob_cfg = cfg();
    assert_eq!(
        bob_cfg.lanes.expect("ürün: şeritler açık").admit_widths,
        DEFAULT_ADMIT_WIDTHS,
        "ürün kabul genişlikleri"
    );
    let bound = receiver_buffer_bound_units(&bob_cfg.runtime, bob_cfg.lanes.as_ref(), 1);
    let mut alice = ClientTransport::new(alice_cfg, crypto(ALICE), Box::new(hub.link(id(ALICE))))
        .expect("alice");
    let mut bob = ClientTransport::new(
        bob_cfg,
        PathOutage {
            inner: crypto(BOB),
            path: arm.path,
            outage: Arc::clone(&outage),
            drops: Arc::clone(&drops),
        },
        Box::new(hub.link(id(BOB))),
    )
    .expect("bob");
    assert!(
        arm.file_bytes <= alice.capabilities().max_plaintext_unit_bytes as usize,
        "dosya birimi adaptörün birim tavanına sığar"
    );
    alice.add_peer_addr(id(BOB), bob.data_addr());
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    let setup_deadline = Instant::now() + Duration::from_secs(30);
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
        assert!(Instant::now() < setup_deadline, "oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(2));
    };

    let mut tx = Tx {
        h,
        file_bytes: arm.file_bytes,
        next_file: 0,
        ptt_sent_at: Vec::new(),
        ptt_refused: 0,
        text_sent_at: Vec::new(),
        ptt_on: true,
    };
    let mut rx = Rx::default();
    // Kesinti ilk gönderimle başlar: dosyanın 0. biriminin ilk emisyonu kesinti içindedir.
    outage.store(true, Ordering::Relaxed);
    let started = Instant::now();
    let mut next_diag = Duration::ZERO;

    // Faz 1 — kesinti.
    while started.elapsed() < arm.outage {
        step(&mut alice, &mut bob, &mut tx, &mut rx, started, bound);
        if diag_on && started.elapsed() >= next_diag {
            next_diag += DIAG_PERIOD;
            diag_line(&alice, &bob, &tx, &rx, started, true);
        }
    }
    outage.store(false, Ordering::Relaxed);
    let opened = Instant::now();
    let outage_actual = opened.saturating_duration_since(started);
    let ptt_sent_in_outage = tx.ptt_sent_at.len();
    let ptt_delivered_in_outage = rx.ptts.len();
    let texts_in_outage = rx.texts.len();
    let files_in_outage = rx.files.len();

    // Faz 2 — yol açık: dosya tamamlanır, PTT sürer. Sınır aşılırsa da gözlem OBSERVE_EXTRA kadar
    // sürer (dosyanın ne zaman bittiği ölçülür); iddia aşağıda.
    let observe_until = opened + RECOVER_BOUND + OBSERVE_EXTRA;
    while (rx.file_done.is_none() || (rx.texts.len() as u64) < TEXT_UNITS)
        && Instant::now() < observe_until
    {
        step(&mut alice, &mut bob, &mut tx, &mut rx, started, bound);
        if diag_on && started.elapsed() >= next_diag {
            next_diag += DIAG_PERIOD;
            diag_line(&alice, &bob, &tx, &rx, started, false);
        }
    }
    // Faz 3 — PTT durur; gönderilen her PTT gelir.
    tx.ptt_on = false;
    let ptt_total = tx.ptt_sent_at.len() as u64;
    let ptt_deadline = Instant::now() + RECOVER_BOUND;
    while (rx.ptts.len() as u64) < ptt_total && Instant::now() < ptt_deadline {
        step(&mut alice, &mut bob, &mut tx, &mut rx, started, bound);
    }
    // Durgunluk: tekrar teslim yok.
    let settle = Instant::now() + Duration::from_millis(300);
    while Instant::now() < settle {
        while alice.poll_event().is_some() {}
        assert!(bob.recv().is_none(), "tekrar teslim yok");
        while bob.poll_event().is_some() {}
        std::thread::sleep(Duration::from_millis(5));
    }

    // Kesinti içinde sunulan PTT'ler: en büyük gecikme ve teslimler arasındaki en uzun boşluk
    // (kesinti başından ya da sonuna dek teslim yoksa o boşluklar da sayılır).
    let ptt_max_in_outage = rx
        .ptt_lat
        .iter()
        .take(ptt_sent_in_outage)
        .copied()
        .max()
        .unwrap_or(Duration::MAX);
    let mut marks: Vec<Instant> = vec![started];
    marks.extend(rx.ptt_at.iter().copied().filter(|&t| t < opened));
    marks.push(opened);
    let ptt_gap_in_outage = marks
        .windows(2)
        .map(|w| w[1].saturating_duration_since(w[0]))
        .max()
        .unwrap_or_default();
    let mut lat = rx.ptt_lat.clone();
    lat.sort_unstable();
    let m = Measured {
        outage: arm.outage,
        outage_actual,
        ptt_sent: ptt_total,
        ptt_refused: tx.ptt_refused,
        ptt_p50: lat.get(lat.len() / 2).copied().unwrap_or(Duration::MAX),
        ptt_max: lat.last().copied().unwrap_or(Duration::MAX),
        ptt_max_in_outage,
        ptt_gap_in_outage,
        ptt_sent_in_outage,
        ptt_delivered_in_outage,
        text_max: rx.text_lat.iter().copied().max().unwrap_or(Duration::MAX),
        texts_in_outage,
        files_in_outage,
        file_done_after_open: rx.file_done.map(|t| t.saturating_duration_since(opened)),
        head_drops: drops.head.load(Ordering::Relaxed),
        file_drops: drops.file.load(Ordering::Relaxed),
        fec_drops: drops.fec.load(Ordering::Relaxed),
        other_drops: drops.other.load(Ordering::Relaxed),
        fec_seen: drops.fec_seen.load(Ordering::Relaxed),
        passed_in_outage: drops.passed_in_outage.load(Ordering::Relaxed),
        buffer_bound: bound,
        buffer_max: rx.buffer_max,
        not_admitted_window: bob.data_not_admitted_window(),
    };
    eprintln!(
        "ÖLÇÜM sonlu-kesinti {:?} T_out={:?} birim={} B: {m:?}",
        arm.path, arm.outage, arm.file_bytes
    );

    // Ön koşullar: kesinti dosyayı gerçekten durdurdu, aynı birimin art arda emisyonları düştü.
    assert_eq!(
        m.files_in_outage, 0,
        "ön koşul: kesinti boyunca dosya teslim edilmez"
    );
    assert!(
        m.file_drops >= 2,
        "ön koşul: dosyanın art arda emisyonları düştü ({})",
        m.file_drops
    );
    if arm.path == Path::HeadOutage {
        assert!(
            m.head_drops >= 2,
            "ön koşul: sıra başının art arda emisyonları düştü ({})",
            m.head_drops
        );
    }
    // 1. PTT kesinti sırasında da zamanında.
    assert_eq!(m.ptt_refused, 0, "PTT şeridi hiç geri basmadı: {m:?}");
    assert!(
        m.ptt_max <= PTT_BOUND,
        "PTT gecikmesi {:?} > {PTT_BOUND:?} (kesinti içinde {:?}, en uzun teslim boşluğu {:?}, \
         kesintide teslim {}/{}): {m:?}",
        m.ptt_max,
        m.ptt_max_in_outage,
        m.ptt_gap_in_outage,
        m.ptt_delivered_in_outage,
        m.ptt_sent_in_outage
    );
    // 2. Metin kesinti sırasında teslim edildi.
    assert_eq!(
        m.texts_in_outage as u64, TEXT_UNITS,
        "bütün metin kesinti içinde teslim edilir: {m:?}"
    );
    assert!(
        m.text_max <= TEXT_BOUND,
        "metin gecikmesi {:?} > {TEXT_BOUND:?}: {m:?}",
        m.text_max
    );
    // 3. Yol açılınca dosya tamamlanır.
    assert!(
        m.file_done_after_open.is_some_and(|d| d <= RECOVER_BOUND),
        "yol açıldıktan sonra dosya {RECOVER_BOUND:?} içinde tamamlanmadı ({:?}; teslim {}/{}): \
         {m:?}",
        m.file_done_after_open,
        rx.files.len(),
        FILE_UNITS
    );
    // 4. Her şerit sırayla, tam bir kez; teslim kanalında düşme yok.
    assert_eq!(
        rx.files,
        (0..FILE_UNITS).collect::<Vec<_>>(),
        "dosya şeridi sırayla, tam bir kez"
    );
    assert_eq!(
        rx.texts,
        (0..TEXT_UNITS).collect::<Vec<_>>(),
        "metin şeridi sırayla, tam bir kez"
    );
    assert_eq!(
        rx.ptts,
        (0..ptt_total).collect::<Vec<_>>(),
        "PTT şeridi sırayla, tam bir kez"
    );
    assert_eq!(bob.delivery_dropped(), 0);
    // 5. Alıcı penceresinin dışına DATA gitmedi (tampon sınırı her turda sınandı).
    assert_eq!(
        m.not_admitted_window, 0,
        "alıcının kabul penceresinin dışına DATA gitti"
    );
    m
}

const PRODUCT_UNIT: usize = KamaV1Profile::MAX_UNIT_BYTES;

/// Dosya sıra başı 5 s art arda kaybolur; PTT ve metin akar, yol açılınca dosya tamamlanır.
#[test]
fn file_head_outage_5s_ptt_and_text_flow() {
    run(Arm {
        path: Path::HeadOutage,
        outage: Duration::from_secs(5),
        file_bytes: PRODUCT_UNIT,
    });
}

/// Aynısı, 15 s kesinti.
#[test]
fn file_head_outage_15s_ptt_and_text_flow() {
    run(Arm {
        path: Path::HeadOutage,
        outage: Duration::from_secs(15),
        file_bytes: PRODUCT_UNIT,
    });
}

/// Karşılaştırma: aynı kol 64 B dosya birimiyle (kopyalar pencereyi dolduramaz).
#[test]
fn file_head_outage_5s_small_units() {
    run(Arm {
        path: Path::HeadOutage,
        outage: Duration::from_secs(5),
        file_bytes: 64,
    });
}

/// Boyuta duyarlı yol 5 s: büyük datagramlar (bütün dosya) düşer, küçükler (PTT, metin) geçer.
#[test]
fn size_sensitive_path_5s_ptt_and_text_flow() {
    run(Arm {
        path: Path::SizeSensitive,
        outage: Duration::from_secs(5),
        file_bytes: PRODUCT_UNIT,
    });
}

/// Aynısı, 15 s.
#[test]
fn size_sensitive_path_15s_ptt_and_text_flow() {
    run(Arm {
        path: Path::SizeSensitive,
        outage: Duration::from_secs(15),
        file_bytes: PRODUCT_UNIT,
    });
}
