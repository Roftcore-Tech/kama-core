//! [ASİMETRİK-YOL · ürün sonucu] Dar dönüşlü hatta dosya aktarılırken metin ve alındısı gelir,
//! PTT ilerler ve dosya biter — dört gözlem tek koşuda, gerçek uygulama motorlarıyla.
//!
//! # Neyi sınar
//!
//! Ürün hedefi: akışlar birbirini bloklamaz; yüksek kayıp, yüksek gecikme ve düşük bantta iletişim
//! sürer. Yön hızı profili (`RuntimeConfig::link_rates`) ve ona bağlı üç değişiklik (alıcı ACK
//! dönüş bütçesi + profil pacer tavanı + dosya penceresi tabanı; dar hatta hat modeli ve akış sonu
//! ACK düzeltmeleri; dosya biriminin motora birikim sınırıyla verilmesi) tek bir ürün sonucuna
//! bağlıdır. Bu test o sonucu uygulama katmanında ölçer: tanımlı asimetrik hatta bir dosya
//! aktarılırken
//!
//! 1. gerçek bir TEXT ve onun AMP alındısı (`RECEIPT(delivered)`) gelir,
//! 2. PTT ilerlemeye devam eder,
//! 3. dosya tamamlanır.
//!
//! # Düzenek
//!
//! İki gerçek `ClientTransport`, ürün yapılandırması (`ClientTransportConfig::product`: port motoru,
//! şeritler açık, uyarlamalı FEC; yalnız kapanış boşaltma tavanı 100 ms — ölçülen süreye girmez).
//! Gerçek kripto (`NoiseWireProtection`, ürünün `Legacy` zarfı). Üstünde gerçek `kama-app`
//! motorları, ürün bileşim kökündeki (`kama-node` düğüm turu) sırayla: `Messenger` (Transport'un
//! sahibi), `Ptt` (`Messenger::poll`'un tek okuyucusu), `FileTransfer` (`handle` zinciri ve
//! kuyruk boşalınca `pump`). El sıkışma zarfları `MemoryHub`'dan geçer (hattın dışında, beyanlı).
//!
//! Veri yolu kullanıcı-alanı biçimlendirici röleden geçer; hücre `client-core`
//! `tests/shaped_relay.rs` asimetrik hücresidir ve biçimlendirici onun kuralıyla aynıdır
//! (serileştirme tel baytıyla — UDP/IP 28 B dahil —, sınırlı FIFO, sabit tek yön gecikme,
//! ileri yönde her pakette tohumlu rastgele kayıp, aynı LCG; dönüş tohumu `tohum ^ 0x5A5A`):
//!
//! * ileri (Alice tarafı → Bob tarafı): 12,5 kB/s, 64 paket kuyruk, 300 ms, %20 kayıp;
//! * dönüş (Bob tarafı → Alice tarafı): 62 B/s, 64 paket kuyruk, 300 ms, kayıpsız.
//!
//! `shaped_relay.rs`'ten tek fark: o hücrede yalnız tek yönlü oturum vardır. Burada iki uç da
//! veri gönderir (Bob alındıları), bu yüzden her yönün HATTI iki trafiği birlikte taşır: ileri hat
//! Alice'in verisini ve Alice'in Bob'a ACK'lerini, dönüş hattı Bob'un ACK'lerini ve Bob'un
//! verisini (alındılar). Gerçek asimetrik hatta da böyledir; alındı 62 B/s hattı ACK'lerle paylaşır.
//!
//! Yön hızları iki uca kendi bakış açısından verilir ve hattın gerçek hızlarıdır: Alice yukarı
//! 12 500 / aşağı 62 B/s, Bob yukarı 62 / aşağı 12 500 B/s. `KAMA_ASYM_PROFILE=off` profili iki
//! uçtan da kaldırır (ürün varsayılanı: `link_rates = None`).
//!
//! # Senaryo (t = 0: dosya kuyruğa alınır)
//!
//! * t = 0: Alice 40 000 B'lik dosyayı gönderir (`FileTransfer::send_file`; 41 parça × ≤ 990 B).
//! * t = 0,3 s: metin 1 (dosyanın başı).
//! * t = 0,5 s: PTT açılır (`Ptt::open`, Lyra, 200 ms demet) ve her 200 ms'de 40 B'lik bir demet
//!   gider (≈ 1,6 kbit/s; `kama-integration` `examples/f0_flows.rs` varsayılanı). Demetler Bob dosyayı
//!   alana dek sürer, sonra `Ptt::close`.
//! * Bob dosyanın yarısını (parça sayısının yarısı) aldığı turda metin 2 (dosyanın ortası).
//!
//! # Ölçülenler (koşu başına)
//!
//! * metin gecikmesi: `send_text` → Bob'da `TextReceived`;
//! * alındı dönüşü: Bob'da `TextReceived` (alındının sunulduğu an) → Alice'te `Delivered`; ayrıca
//!   gidiş-dönüş (`send_text` → `Delivered`);
//! * PTT: ilk demetin gecikmesi, demet başına gecikme (planlanan an → Bob'da `PttAudio`), dosya
//!   sürerken teslimler arası en büyük boşluk;
//! * dosya: `send_file` → Bob'da `FileEvent::Received` (bayt bayt doğrulanır); Alice'te
//!   `FileEvent::Delivered` ayrıca;
//! * iki yönde tel baytı ve paket (UDP/IP dahil), kuyruk ve rastgele düşmeler.
//!
//! Kayıp sınıflandırması ölçümle değil kayıtla yapılır: iki ucun kriptosu ince bir sarmalayıcıyla
//! ([`Tap`]) sarılır; mühürlenen her DATA datagramının parmak izi (FNV-1a 64) çözülmüş AMP kaydıyla
//! eşlenir, röle düşürdüğü her datagramın parmak izini yazar. "Birim kayboldu" = birimin İLK
//! iletimi rölede düştü. Sarmalayıcı yalnız okur; mühürlü baytlara ve kararlara dokunmaz.
//!
//! # Eşikler (ölçümden ÖNCE sabitlendi) ve ürün gerekçesi
//!
//! * METİN ≤ 1 s (her biri). Konuşma düzeyinde etkileşim: 300 ms tek yönün üstüne dosyanın önüne
//!   geçmiş birkaç datagramdan fazlası beklenmez. İstisna: metnin kendi ilk iletimi düştüyse
//!   teslim ACK tabanlı kurtarmaya kalır (bu hatta ACK ≈ 5 s'de bir); o zaman "kayıp-kurtarma"
//!   olarak süresiyle yazılır ve ölçüt düşmüş sayılmaz. Hiç gelmezse düşer.
//! * ALINDI dönüşü ≤ 4 s (her biri). Fizik: alındı datagramı 71 B tel (AMP 10 B + DATA başlığı 11 +
//!   Noise `Legacy` 22 + UDP/IP 28), 62 B/s'de 1,15 s serileşir; + 300 ms tek yön; + önünde hatta
//!   serileşmekte olan en çok bir denetim datagramı (en büyük ACK V2 çerçevesi 79 B → 129 B tel,
//!   2,08 s). Toplam 3,53 s; kalan 0,47 s iki çekirdekli makinede iş parçacığı zamanlaması. Dönüş
//!   kayıpsızdır; alındının ilk iletimi yine de rölede (kuyruk taşması) düşerse "kayıp-kurtarma".
//! * PTT ≤ 1 s (her demet, ilki dahil) ve gönderilen her demet teslim edilir. Tek istisna
//!   "kayıp-kurtarma": demetin KENDİ ilk iletimi düştü. [2026-11-16 düzeltmesi: ilk ölçümde
//!   istisna sayılan "demet Bob'a vardığında PTT şeridinde ilk iletimi düşmüş bir öncülü henüz
//!   teslim edilmemişti" durumu artık istisna DEĞİLDİR; sınır (1 s) aynı.] PTT ürün kararıyla
//!   güvenilir ve sıralı bir kliptir (`kama-app` `ptt.rs`, PTT-R0: "ağ takılırsa ses bozulmaz,
//!   durur"): öncülünü bekleyen demet sınırı aşarsa ölçüt düşer; bu demetler ayrı sütunda
//!   ("öncül_bekledi") sayılır ve en büyük gecikmeleriyle yazılır, ki düşüşün kaynağı (dosyanın
//!   bloklaması mı, PTT'nin kendi sıralı kurtarması mı) okunabilsin. [2026-11-16 ürün kabulü:
//!   profil açıkken "kayıp-kurtarma" istisnası da YOKTUR — bütün demetlerin en büyük gecikmesi
//!   ≤ 1 s; istisna yalnız profil kapalı teşhis kolunda raporlanır. Sınır (1 s) aynı.] PTT'nin dar dönüşte önlemsel
//!   fazlalık ya da son tarih politikası alması sahibin kararıdır; bu test politikayı değiştirmez.
//! * DOSYA ≤ 14 s (`send_file` → Bob'da tamamlanma). Bu hücrenin dosya sınırı 40 × 1000 B için
//!   14 s'dir; sınır yük baytıyla ölçeklenir: 14 s × 40 000 B / 40 000 B = 14 s. Kredi verilmeyen
//!   iki fark: parça başına AMP + DATA + Noise zarfı teli ≈ %3 büyütür (41 datagram, ≈ 43,4 kB;
//!   `shaped_relay` hücresi 40 × 1055 B ≈ 42,2 kB) ve PTT (≈ 114 B tel / 200 ms ≈ 0,57 kB/s, hattın
//!   %4,6'sı) ile metinler aynı ileri hattı paylaşır.
//! * Koşu tavanı 60 s; o ana dek gelmeyen her gözlem düşmüş sayılır.
//!
//! # Koşturma
//!
//! Varsayılan (CI): tohum 1, profil açık; dört ölçütten biri düşerse test düşer. Ölçüm:
//! `KAMA_ASYM_SEED=<n>` (tohum), `KAMA_ASYM_PROFILE=off` (profil kapalı kol),
//! `KAMA_ASYM_REAL_FWD_BPS=<B/s>` (yanlış profil teşhis kolu: profil ileri hattı yine
//! [`FWD_BPS`] bildirir, rölenin gerçek ileri hızı bu değer olur; ölçütler raporlanır, kabul
//! iddiası yalnız ürün kolunda — gerçek = profil — uygulanır). Her koşu `ÜRÜN-SONUÇ`
//! ve `ÖLÇÜT` satırlarını basar (`--nocapture`); ölçütler önce basılır, sonra iddia edilir. Düşen
//! ölçütün mekanizması aynı koşunun izinden okunur: `İZ-UYG` (uygulama olayları), `İZ-PTT` (demet
//! başına plan, ilk çıkış, iletim sayısı, ilk varış, teslim) ve `İZ-HAT` (rölenin gördüğü her
//! datagram: yön, yol, an, tel boyu, sonuç, varış ve taşıdığı AMP kaydı).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use client_core::memory_link::MemoryHub;
use client_core::runtime::ClientLiveDiag;
use client_core::runtime::LinkRates;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::wire::{decode_data, FRAME_FEC, FRAME_RECOVERED};
use kama_app::clock::{MonotonicClock, SystemClock};
use kama_app::codec::{decode, AmpLimits, AppMessage, PttCodec};
use kama_app::file_transfer::{FileEvent, FileTransfer};
use kama_app::messaging::{Messenger, MessengerEvent, MsgId};
use kama_app::ptt::{Ptt, PttError, PttEvent, StreamId};
use kama_app::store::MemoryAppStore;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{ConnectResult, TransportCapabilities, TransportError};
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit, SessionCommand, SessionControl, SessionEvent as CryptoEvent, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

// ── Hücre (`client-core` `tests/shaped_relay.rs` asimetrik hücresi) ──────────────────────────
const FWD_BPS: u64 = 12_500;
const REV_BPS: u64 = 62;
const QUEUE_PKTS: usize = 64;
const ONE_WAY: Duration = Duration::from_millis(300);
const FWD_LOSS_PERMILLE: u64 = 200;
/// UDP/IP zarfı (serileştirmeye girer; röle yalnız UDP gövdesini görür).
const UDP_IP_OVERHEAD: u64 = 28;

// ── Senaryo ──────────────────────────────────────────────────────────────────────────────────
const FILE_BYTES: usize = 40_000;
const TEXT1_AT: Duration = Duration::from_millis(300);
const PTT_AT: Duration = Duration::from_millis(500);
const PTT_BUNDLE_MS: u8 = 200;
const PTT_PAYLOAD_BYTES: usize = 40;
const TEXT1: &str = "metin 1: dosyanin basinda gonderildi";
const TEXT2: &str = "metin 2: dosyanin ortasinda gonderildi";

// ── Eşikler (ölçümden önce; gerekçe modül belgesinde) ────────────────────────────────────────
const TEXT_BOUND: Duration = Duration::from_secs(1);
const RECEIPT_RETURN_BOUND: Duration = Duration::from_secs(4);
const PTT_BOUND: Duration = Duration::from_secs(1);
const FILE_BOUND: Duration = Duration::from_secs(14);
const RUN_CAP: Duration = Duration::from_secs(60);

/// Kayıt tavanları (SR-AUDIT-001: sınırlı bellek).
const LEDGER_CAP: usize = 16_384;
const DROP_CAP: usize = 16_384;
/// Düğüm turundaki olay bütçesi (`kama-node` `MAX_EVENTS_PER_TURN` ile aynı rol).
const EVENTS_PER_TURN: usize = 256;

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4E;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// FNV-1a 64: datagramın parmak izi (şifreli metin iletim başına tekildir — nonce = PN).
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ── Kayıt: mühürlenen ve açılan DATA birimleri ───────────────────────────────────────────────

/// Bir DATA biriminin taşıdığı AMP kaydı (eşleme anahtarı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Item {
    Text(u32),
    Receipt(u32),
    PttOpen(u32),
    PttClose(u32),
    /// Demet sırası (yükün ilk 4 baytı).
    Bundle(u32),
    FileBegin,
    Chunk(u64),
    FileEnd,
    FileReceipt,
    Other,
}

fn item_of(message: &AppMessage) -> Item {
    match message {
        AppMessage::Text { msg_id, .. } => Item::Text(*msg_id),
        AppMessage::Receipt { ref_msg_id, .. } => Item::Receipt(*ref_msg_id),
        AppMessage::PttOpen { stream_id, .. } => Item::PttOpen(*stream_id),
        AppMessage::PttClose { stream_id } => Item::PttClose(*stream_id),
        AppMessage::MediaBundle { payload, .. } if payload.len() >= 4 => {
            Item::Bundle(u32::from_be_bytes(payload[..4].try_into().expect("4 B")))
        }
        AppMessage::FileBegin { .. } => Item::FileBegin,
        AppMessage::FileChunk { offset, .. } => Item::Chunk(*offset),
        AppMessage::FileEnd { .. } => Item::FileEnd,
        AppMessage::FileReceipt { .. } => Item::FileReceipt,
        _ => Item::Other,
    }
}

#[derive(Default)]
struct Ledger {
    /// Bu ucun mühürlediği her DATA datagramı: parmak izi → kayıt.
    by_fp: HashMap<u64, Item>,
    /// Kaydın ilk iletimi (an, parmak izi).
    first_tx: HashMap<Item, (Instant, u64)>,
    /// Kaydın iletim sayısı (ilk + yeniden iletim + kopya).
    tx_count: HashMap<Item, u32>,
    /// Kaydın bu uca ilk varışı (AEAD doğrulandı, DATA çerçevesi açıldı).
    first_rx: HashMap<Item, Instant>,
    /// AMP olarak çözülemeyen DATA çerçevesi (akıl sağlığı; 0 beklenir).
    unparsed: u64,
    /// Tavan dolduğu için yazılamayan kayıt.
    overflow: u64,
    /// [FEC-TEŞHİS] Bu ucun mühürlediği / açtığı FEC onarım çerçevesi ve RECOVERED çerçevesi.
    fec_tx: u64,
    fec_rx: u64,
    recovered_tx: u64,
    recovered_rx: u64,
}

/// [FEC-TEŞHİS] Çerçeve türü sayacı (DATA dışı).
fn note_kind(l: &mut Ledger, frame: &[u8], sealed: bool) {
    match (frame.first().copied(), sealed) {
        (Some(FRAME_FEC), true) => l.fec_tx += 1,
        (Some(FRAME_FEC), false) => l.fec_rx += 1,
        (Some(FRAME_RECOVERED), true) => l.recovered_tx += 1,
        (Some(FRAME_RECOVERED), false) => l.recovered_rx += 1,
        _ => {}
    }
}

/// Gerçek kriptoyu saran salt-okur sarmalayıcı: DATA çerçevesini çözüp kayda yazar.
struct Tap {
    inner: NoiseWireProtection,
    ledger: Arc<Mutex<Ledger>>,
    limits: AmpLimits,
}

impl Tap {
    fn classify(&self, frame: &[u8]) -> Option<Result<Item, ()>> {
        let (_, _, payload) = decode_data(frame)?;
        Some(
            decode(payload, &self.limits)
                .map(|m| item_of(&m))
                .map_err(|_| ()),
        )
    }
}

impl PacketProtection for Tap {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        let sealed = self.inner.seal(ctx, frame)?;
        note_kind(&mut lock(&self.ledger), frame, true);
        if let Some(c) = self.classify(frame) {
            let now = Instant::now();
            let mut l = lock(&self.ledger);
            match c {
                Ok(item) if l.by_fp.len() < LEDGER_CAP => {
                    let fp = fingerprint(&sealed.bytes);
                    l.by_fp.insert(fp, item);
                    l.first_tx.entry(item).or_insert((now, fp));
                    *l.tx_count.entry(item).or_insert(0) += 1;
                }
                Ok(_) => l.overflow += 1,
                Err(()) => l.unparsed += 1,
            }
        }
        Ok(sealed)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let opened = self.inner.open(wire)?;
        note_kind(&mut lock(&self.ledger), &opened.frame, false);
        if let Some(c) = self.classify(&opened.frame) {
            let now = Instant::now();
            let mut l = lock(&self.ledger);
            match c {
                Ok(item) => {
                    l.first_rx.entry(item).or_insert(now);
                }
                Err(()) => l.unparsed += 1,
            }
        }
        Some(opened)
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

impl SessionControl for Tap {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        self.inner.command(cmd)
    }
    fn poll_event(&mut self) -> Option<CryptoEvent> {
        self.inner.poll_event()
    }
}

fn tap(me: u8, ledger: &Arc<Mutex<Ledger>>) -> Tap {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    // Çözme sınırı geniş tutulur (yalnız okuma): her geçerli kayıt çözülür.
    let limits = AmpLimits::from_capabilities(&TransportCapabilities {
        max_plaintext_unit_bytes: 65_540,
        preferred_plaintext_unit_bytes: 65_540,
        max_early_unit_bytes: 65_540,
        reliable: true,
    })
    .expect("geniş AMP limiti");
    Tap {
        inner: fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0,
        ledger: Arc::clone(ledger),
        limits,
    }
}

// ── Röle ─────────────────────────────────────────────────────────────────────────────────────

/// Datagramın röledeki yolu. `A*` Alice'in oturum ucu (Alice istemci → Bob sunucu), `B*` Bob'un
/// oturum ucu (Bob istemci → Alice sunucu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Alice istemci → Bob sunucu (Alice'in verisi; ileri hat).
    AUp,
    /// Bob sunucu → Alice istemci (Bob'un ACK'leri; dönüş hattı).
    ADown,
    /// Bob istemci → Alice sunucu (Bob'un verisi, alındılar; dönüş hattı).
    BUp,
    /// Alice sunucu → Bob istemci (Alice'in ACK'leri; ileri hat).
    BDown,
}

/// Rölenin gördüğü bir datagram (iz; ölçümden sonra yazılır, sıcak yolda biçimlendirme yok).
#[derive(Debug, Clone, Copy)]
struct LineRec {
    offered: Instant,
    route: Route,
    /// Tel boyu (UDP/IP dahil).
    wire: u64,
    /// 0 iletildi · 1 kuyruk taştı · 2 rastgele kayıp.
    fate: u8,
    arrives: Option<Instant>,
    fp: u64,
}

/// Bir yönün hattı: `shaped_relay.rs` `Shaper::offer` kuralı (jitter'sız).
struct Shaper {
    rate_bps: u64,
    limit: usize,
    delay: Duration,
    loss_permille: u64,
    lcg: u64,
    busy: VecDeque<Instant>,
    wire: VecDeque<(Instant, Route, Vec<u8>)>,
    sent: u64,
    bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
    dropped_fp: Vec<u64>,
    /// Her sunulan datagramın izi (sığa [`DROP_CAP`]).
    trace: Vec<LineRec>,
}

impl Shaper {
    fn new(rate_bps: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            limit: QUEUE_PKTS,
            delay: ONE_WAY,
            loss_permille,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            sent: 0,
            bytes: 0,
            dropped_queue: 0,
            dropped_random: 0,
            dropped_fp: Vec::new(),
            trace: Vec::new(),
        }
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    fn note_drop(&mut self, pkt: &[u8]) {
        if self.dropped_fp.len() < DROP_CAP {
            self.dropped_fp.push(fingerprint(pkt));
        }
    }

    fn note(
        &mut self,
        offered: Instant,
        route: Route,
        wire: u64,
        fate: u8,
        arrives: Option<Instant>,
        pkt: &[u8],
    ) {
        if self.trace.len() < DROP_CAP {
            self.trace.push(LineRec {
                offered,
                route,
                wire,
                fate,
                arrives,
                fp: fingerprint(pkt),
            });
        }
    }

    fn offer(&mut self, now: Instant, route: Route, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        let size = pkt.len() as u64 + UDP_IP_OVERHEAD;
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            self.note_drop(&pkt);
            self.note(now, route, size, 1, None, &pkt);
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + Duration::from_nanos(size * 1_000_000_000 / self.rate_bps.max(1));
        self.busy.push_back(done);
        self.sent += 1;
        self.bytes += size;
        if self.loss_permille > 0 && self.rand_permille() < self.loss_permille {
            self.dropped_random += 1;
            self.note_drop(&pkt);
            self.note(now, route, size, 2, None, &pkt);
            return;
        }
        // Sabit gecikme, FIFO serileştirme: varış anları artandır.
        self.note(now, route, size, 0, Some(done + self.delay), &pkt);
        self.wire.push_back((done + self.delay, route, pkt));
    }

    fn take_due(&mut self, now: Instant) -> Vec<(Route, Vec<u8>)> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|(t, _, _)| *t <= now) {
            let (_, r, p) = self.wire.pop_front().expect("var");
            out.push((r, p));
        }
        out
    }
}

struct RelaySockets {
    front_a: UdpSocket,
    back_a: UdpSocket,
    front_b: UdpSocket,
    back_b: UdpSocket,
}

fn relay(s: RelaySockets, mut fwd: Shaper, mut rev: Shaper, stop: &AtomicBool) -> (Shaper, Shaper) {
    for sock in [&s.front_a, &s.back_a, &s.front_b, &s.back_b] {
        sock.set_nonblocking(true).expect("nonblocking");
    }
    let mut alice_client: Option<SocketAddr> = None;
    let mut bob_client: Option<SocketAddr> = None;
    let mut buf = vec![0u8; 2048];
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        while let Ok((n, from)) = s.front_a.recv_from(&mut buf) {
            alice_client = Some(from);
            fwd.offer(now, Route::AUp, buf[..n].to_vec());
        }
        while let Ok(n) = s.back_a.recv(&mut buf) {
            rev.offer(now, Route::ADown, buf[..n].to_vec());
        }
        while let Ok((n, from)) = s.front_b.recv_from(&mut buf) {
            bob_client = Some(from);
            rev.offer(now, Route::BUp, buf[..n].to_vec());
        }
        while let Ok(n) = s.back_b.recv(&mut buf) {
            fwd.offer(now, Route::BDown, buf[..n].to_vec());
        }
        let now = Instant::now();
        for (route, p) in fwd.take_due(now).into_iter().chain(rev.take_due(now)) {
            let _ = match route {
                Route::AUp => s.back_a.send(&p),
                Route::BUp => s.back_b.send(&p),
                Route::ADown => alice_client.map_or(Ok(0), |to| s.front_a.send_to(&p, to)),
                Route::BDown => bob_client.map_or(Ok(0), |to| s.front_b.send_to(&p, to)),
            };
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    (fwd, rev)
}

// ── Uç: Transport + uygulama motorları (düğüm turu sırası) ───────────────────────────────────

type Transport = ClientTransport<Tap>;

enum Ev {
    Msg(MessengerEvent),
    Ptt(PttEvent),
    File(FileEvent),
    /// Zincirde görülen `FILE_CHUNK` kaydı (dosya motoruna verilmeden önce sayılır).
    Chunk,
}

struct Side {
    m: Messenger<Transport>,
    ptt: Ptt,
    files: FileTransfer,
}

impl Side {
    fn new(transport: Transport, clock: &Arc<dyn MonotonicClock>) -> Self {
        Self {
            m: Messenger::with_store(transport, Arc::new(MemoryAppStore::new()))
                .expect("AMP limitleri"),
            ptt: Ptt::new(Arc::clone(clock)),
            files: FileTransfer::new(Arc::clone(clock)),
        }
    }

    fn drain_files(&mut self, at: Instant, out: &mut Vec<(Instant, Ev)>) {
        while let Some(e) = self.files.poll_event() {
            out.push((at, Ev::File(e)));
        }
    }

    /// `kama-node` düğüm turu: `Ptt::poll` → `FileTransfer::handle` → olay; kuyruk boşaldıysa
    /// `FileTransfer::pump`; ardından dosya olayları.
    fn turn(&mut self, out: &mut Vec<(Instant, Ev)>) {
        let mut budget = EVENTS_PER_TURN;
        let drained = loop {
            if budget == 0 {
                break false;
            }
            budget -= 1;
            let Some(event) = self.ptt.poll(&mut self.m) else {
                break true;
            };
            let at = Instant::now();
            match event {
                PttEvent::Messenger(e) => {
                    if matches!(
                        &e,
                        MessengerEvent::Other {
                            message: AppMessage::FileChunk { .. },
                            ..
                        }
                    ) {
                        out.push((at, Ev::Chunk));
                    }
                    if let Some(e) = self.files.handle(&mut self.m, e) {
                        out.push((at, Ev::Msg(e)));
                    }
                    self.drain_files(at, out);
                }
                other => out.push((at, Ev::Ptt(other))),
            }
        };
        if drained {
            self.files.pump(&mut self.m);
        }
        self.drain_files(Instant::now(), out);
    }
}

fn transport(
    me: u8,
    ledger: &Arc<Mutex<Ledger>>,
    hub: &MemoryHub,
    link: Option<LinkRates>,
) -> Transport {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.link_rates = link;
    let mut t =
        ClientTransport::new(cfg, tap(me, ledger), Box::new(hub.link(id(me)))).expect("transport");
    // [FEC-TEŞHİS] Canlı ayna (salt gözlem): FEC latch, PARAM, PTO ve kopya sayaçları.
    t.set_live_diag_for_test(true);
    t
}

// ── Koşu ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Texts {
    sent: Option<(MsgId, Instant)>,
    received: Option<Instant>,
    delivered: Option<Instant>,
}

struct Bundle {
    planned: Instant,
    accepted: Instant,
    received: Option<Instant>,
}

struct Run {
    seed: u64,
    profile: bool,
    t0: Instant,
    texts: [Texts; 2],
    ptt_open: Option<(StreamId, Instant)>,
    ptt_started: Option<Instant>,
    bundles: BTreeMap<u32, Bundle>,
    file_received: Option<Instant>,
    file_delivered: Option<Instant>,
    file_ok: bool,
    chunk_count: u64,
    /// Bob'un uygulama zincirinde görülen her `FILE_CHUNK` kaydının anı.
    chunk_rx: Vec<Instant>,
    alice: Ledger,
    bob: Ledger,
    fwd: Shaper,
    rev: Shaper,
    file_backlog_held: u64,
    /// [FEC-TEŞHİS] Koşu sonunda iki ucun oturum aynası (Alice → Bob, Bob → Alice).
    alice_live: Option<ClientLiveDiag>,
    bob_live: Option<ClientLiveDiag>,
}

fn run(seed: u64, profile: bool, real_fwd_bps: u64) -> Run {
    let hub = MemoryHub::new();
    let alice_ledger = Arc::new(Mutex::new(Ledger::default()));
    let bob_ledger = Arc::new(Mutex::new(Ledger::default()));
    let link = |up, down| {
        profile.then_some(LinkRates {
            uplink_bps: up,
            downlink_bps: down,
        })
    };
    let mut alice_t = transport(ALICE, &alice_ledger, &hub, link(FWD_BPS, REV_BPS));
    let mut bob_t = transport(BOB, &bob_ledger, &hub, link(REV_BPS, FWD_BPS));
    let bind = || UdpSocket::bind("127.0.0.1:0").expect("röle soketi");
    let sockets = RelaySockets {
        front_a: bind(),
        back_a: bind(),
        front_b: bind(),
        back_b: bind(),
    };
    sockets
        .back_a
        .connect(bob_t.data_addr())
        .expect("röle → Bob");
    sockets
        .back_b
        .connect(alice_t.data_addr())
        .expect("röle → Alice");
    alice_t.add_peer_addr(id(BOB), sockets.front_a.local_addr().expect("adres"));
    bob_t.add_peer_addr(id(ALICE), sockets.front_b.local_addr().expect("adres"));
    let stop = Arc::new(AtomicBool::new(false));
    let relay_stop = Arc::clone(&stop);
    let fwd = Shaper::new(real_fwd_bps, FWD_LOSS_PERMILLE, seed);
    let rev = Shaper::new(REV_BPS, 0, seed ^ 0x5A5A);
    let relay_thread = std::thread::spawn(move || relay(sockets, fwd, rev, &relay_stop));

    let clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
    let mut alice = Side::new(alice_t, &clock);
    let mut bob = Side::new(bob_t, &clock);
    let mut events: Vec<(Instant, Ev)> = Vec::new();

    // Oturum: Alice bağlanır, iki uç da Messenger'da hazır oturumu benimser.
    match alice.m.connect(id(BOB)).expect("connect") {
        ConnectResult::Pending(_) | ConnectResult::Ready(_) => {}
    }
    let setup_deadline = Instant::now() + RUN_CAP;
    while alice.m.session_of(id(BOB)).is_none() || bob.m.session_of(id(ALICE)).is_none() {
        hub.relay();
        alice.turn(&mut events);
        bob.turn(&mut events);
        events.clear();
        assert!(Instant::now() < setup_deadline, "oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(1));
    }

    let file: Vec<u8> = (0..FILE_BYTES).map(|i| (i % 251) as u8).collect();
    let chunk_len = alice.m.limits().max_file_chunk_bytes() as u64;
    let chunk_count = (FILE_BYTES as u64).div_ceil(chunk_len);
    let t0 = Instant::now();
    alice
        .files
        .send_file(&alice.m, id(BOB), "olcum.bin", file.clone())
        .expect("dosya kuyruğa alınır");

    let mut texts: [Texts; 2] = Default::default();
    let mut ptt_open: Option<(StreamId, Instant)> = None;
    let mut ptt_closed = false;
    let mut ptt_started: Option<Instant> = None;
    let mut bundles: BTreeMap<u32, Bundle> = BTreeMap::new();
    let mut next_bundle: u32 = 0;
    let mut bob_chunks: u64 = 0;
    let mut chunk_rx: Vec<Instant> = Vec::new();
    let mut file_received: Option<Instant> = None;
    let mut file_delivered: Option<Instant> = None;
    let mut file_ok = false;

    loop {
        hub.relay();
        let now = Instant::now();
        let el = now - t0;
        if texts[0].sent.is_none() && el >= TEXT1_AT {
            let msg_id = alice.m.send_text(id(BOB), TEXT1).expect("metin 1");
            texts[0].sent = Some((msg_id, now));
        }
        if texts[1].sent.is_none() && bob_chunks * 2 >= chunk_count {
            let msg_id = alice.m.send_text(id(BOB), TEXT2).expect("metin 2");
            texts[1].sent = Some((msg_id, Instant::now()));
        }
        if ptt_open.is_none() && el >= PTT_AT {
            let stream = alice
                .ptt
                .open(&mut alice.m, id(BOB), PttCodec::Lyra, PTT_BUNDLE_MS)
                .expect("PTT açılır");
            ptt_open = Some((stream, now));
        }
        if let (Some((stream, opened)), false) = (ptt_open, ptt_closed) {
            if file_received.is_none() {
                // Konuşan sabit hızda konuşur: demet k, açılıştan k × 200 ms sonra hazırdır.
                loop {
                    let planned =
                        opened + Duration::from_millis(u64::from(PTT_BUNDLE_MS)) * next_bundle;
                    if planned > Instant::now() {
                        break;
                    }
                    let mut payload = vec![0x5Au8; PTT_PAYLOAD_BYTES];
                    payload[..4].copy_from_slice(&next_bundle.to_be_bytes());
                    match alice.ptt.send_bundle(&mut alice.m, stream, &payload) {
                        Ok(()) => {
                            bundles.insert(
                                next_bundle,
                                Bundle {
                                    planned,
                                    accepted: Instant::now(),
                                    received: None,
                                },
                            );
                            next_bundle += 1;
                        }
                        // Geri basınç: konuşan bekler, aynı demet sonraki turda yeniden verilir.
                        Err(PttError::Transport(TransportError::Backpressure)) => break,
                        Err(e) => panic!("PTT demeti: {e:?}"),
                    }
                }
            } else {
                alice.ptt.close(&mut alice.m, stream).expect("PTT kapanır");
                ptt_closed = true;
            }
        }

        alice.turn(&mut events);
        bob.turn(&mut events);
        for (at, ev) in events.drain(..) {
            match ev {
                Ev::Chunk => {
                    bob_chunks += 1;
                    chunk_rx.push(at);
                }
                Ev::Msg(MessengerEvent::TextReceived { msg_id, .. }) => {
                    for t in &mut texts {
                        if t.sent.is_some_and(|(m, _)| m == msg_id) {
                            t.received.get_or_insert(at);
                        }
                    }
                }
                Ev::Msg(MessengerEvent::Delivered { msg_id, .. }) => {
                    for t in &mut texts {
                        if t.sent.is_some_and(|(m, _)| m == msg_id) {
                            t.delivered.get_or_insert(at);
                        }
                    }
                }
                Ev::Ptt(PttEvent::PttStarted { .. }) => {
                    ptt_started.get_or_insert(at);
                }
                Ev::Ptt(PttEvent::PttAudio { payload, .. }) if payload.len() >= 4 => {
                    let seq = u32::from_be_bytes(payload[..4].try_into().expect("4 B"));
                    if let Some(b) = bundles.get_mut(&seq) {
                        b.received.get_or_insert(at);
                    }
                }
                Ev::File(FileEvent::Received { bytes, .. }) => {
                    file_ok = bytes == file;
                    file_received.get_or_insert(at);
                }
                Ev::File(FileEvent::Delivered { .. }) => {
                    file_delivered.get_or_insert(at);
                }
                Ev::File(FileEvent::SendFailed { reason, .. }) => {
                    panic!("dosya gönderimi başarısız: {reason:?}")
                }
                Ev::File(FileEvent::Failed { reason, .. }) => {
                    panic!("dosya alımı başarısız: {reason:?}")
                }
                _ => {}
            }
        }
        let done = file_received.is_some()
            && file_delivered.is_some()
            && ptt_closed
            && texts.iter().all(|t| t.delivered.is_some())
            && bundles.values().all(|b| b.received.is_some());
        if done || t0.elapsed() >= RUN_CAP {
            break;
        }
        std::thread::sleep(Duration::from_micros(500));
    }

    let file_backlog_held = alice
        .m
        .transport()
        .lane_stats()
        .map_or(0, |s| s.file_backlog_held);
    let alice_live = alice
        .m
        .transport()
        .session_live_diags()
        .first()
        .map(|d| d.1);
    let bob_live = bob.m.transport().session_live_diags().first().map(|d| d.1);
    drop(alice);
    drop(bob);
    stop.store(true, Ordering::Relaxed);
    let (fwd, rev) = relay_thread.join().expect("röle");
    let alice_ledger = std::mem::take(&mut *lock(&alice_ledger));
    let bob_ledger = std::mem::take(&mut *lock(&bob_ledger));
    Run {
        seed,
        profile,
        t0,
        texts,
        ptt_open,
        ptt_started,
        bundles,
        file_received,
        file_delivered,
        file_ok,
        chunk_count,
        chunk_rx,
        alice: alice_ledger,
        bob: bob_ledger,
        fwd,
        rev,
        file_backlog_held,
        alice_live,
        bob_live,
    }
}

// ── Değerlendirme ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    /// Birimin kendi ilk iletimi (ya da şeritte beklediği öncülü) kayboldu; ACK tabanlı kurtarma.
    LossRecovery,
    Fail,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "GEÇTİ",
            Self::LossRecovery => "kayıp-kurtarma",
            Self::Fail => "KALDI",
        }
    }
}

fn secs(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64())
}

fn opt_secs(d: Option<Duration>) -> String {
    d.map_or("YOK".to_string(), secs)
}

impl Run {
    fn lost_first(ledger: &Ledger, drops: &HashSet<u64>, item: Item) -> bool {
        ledger
            .first_tx
            .get(&item)
            .is_some_and(|(_, fp)| drops.contains(fp))
    }

    fn since(&self, at: Instant, base: Instant) -> Duration {
        let _ = self;
        at.saturating_duration_since(base)
    }

    /// Yazdırır ve düşen ölçütlerin adlarını döndürür.
    fn report(&self) -> Vec<String> {
        let fwd_drops: HashSet<u64> = self.fwd.dropped_fp.iter().copied().collect();
        let rev_drops: HashSet<u64> = self.rev.dropped_fp.iter().copied().collect();
        let mut failed: Vec<String> = Vec::new();
        let mut line: Vec<String> = Vec::new();
        let tag = format!(
            "tohum={} profil={}",
            self.seed,
            if self.profile { "açık" } else { "kapalı" }
        );

        // (1) Metin + alındı.
        let mut text_verdicts = Vec::new();
        let mut receipt_verdicts = Vec::new();
        for (i, t) in self.texts.iter().enumerate() {
            let n = i + 1;
            let Some((msg_id, sent)) = t.sent else {
                println!("ÖLÇÜT {tag} metin{n}: gönderilmedi → KALDI");
                text_verdicts.push(Verdict::Fail);
                receipt_verdicts.push(Verdict::Fail);
                continue;
            };
            let item = Item::Text(msg_id);
            let lost = Self::lost_first(&self.alice, &fwd_drops, item);
            let delay = t.received.map(|r| self.since(r, sent));
            let first_tx = self
                .alice
                .first_tx
                .get(&item)
                .map(|(a, _)| self.since(*a, sent));
            let arrival = self.bob.first_rx.get(&item).map(|a| self.since(*a, sent));
            let v = match delay {
                None => Verdict::Fail,
                Some(d) if d <= TEXT_BOUND => Verdict::Pass,
                Some(_) if lost => Verdict::LossRecovery,
                Some(_) => Verdict::Fail,
            };
            println!(
                "ÖLÇÜT {tag} metin{n}: gecikme={} s (sınır {} s) ilk_çıkış=+{} s ilk_varış=+{} s iletim={} ilk_iletim_düştü={lost} → {}",
                opt_secs(delay),
                secs(TEXT_BOUND),
                opt_secs(first_tx),
                opt_secs(arrival),
                self.alice.tx_count.get(&item).copied().unwrap_or(0),
                v.label()
            );
            text_verdicts.push(v);
            // Alındı: Bob'da metnin teslim anı → Alice'te `Delivered`.
            let r_item = Item::Receipt(msg_id);
            let r_lost = Self::lost_first(&self.bob, &rev_drops, r_item);
            let ret = match (t.received, t.delivered) {
                (Some(r), Some(d)) => Some(self.since(d, r)),
                _ => None,
            };
            let rtt = t.delivered.map(|d| self.since(d, sent));
            let r_first_tx = match (t.received, self.bob.first_tx.get(&r_item)) {
                (Some(r), Some((a, _))) => Some(self.since(*a, r)),
                _ => None,
            };
            let r_arrival = match (t.received, self.alice.first_rx.get(&r_item)) {
                (Some(r), Some(a)) => Some(self.since(*a, r)),
                _ => None,
            };
            let rv = match ret {
                None => Verdict::Fail,
                Some(d) if d <= RECEIPT_RETURN_BOUND => Verdict::Pass,
                Some(_) if r_lost => Verdict::LossRecovery,
                Some(_) => Verdict::Fail,
            };
            println!(
                "ÖLÇÜT {tag} alındı{n}: dönüş={} s (sınır {} s) gidiş_dönüş={} s Bob_ilk_çıkış=+{} s Alice_ilk_varış=+{} s iletim={} ilk_iletim_düştü={r_lost} → {}",
                opt_secs(ret),
                secs(RECEIPT_RETURN_BOUND),
                opt_secs(rtt),
                opt_secs(r_first_tx),
                opt_secs(r_arrival),
                self.bob.tx_count.get(&r_item).copied().unwrap_or(0),
                rv.label()
            );
            receipt_verdicts.push(rv);
            line.push(format!(
                "metin{n}={}({}) alındı{n}_dönüş={}({}) alındı{n}_gd={}",
                opt_secs(delay),
                v.label(),
                opt_secs(ret),
                rv.label(),
                opt_secs(rtt)
            ));
        }
        if text_verdicts.contains(&Verdict::Fail) {
            failed.push("METİN".to_string());
        }
        if receipt_verdicts.contains(&Verdict::Fail) {
            failed.push("ALINDI".to_string());
        }

        // (2) PTT.
        let file_end = self.file_received;
        let (ptt_verdict, ptt_line) = self.ptt_report(&tag, &fwd_drops, file_end);
        if ptt_verdict == Verdict::Fail {
            failed.push("PTT".to_string());
        }
        line.push(ptt_line);

        // (3) Dosya.
        let file_t = self.file_received.map(|r| self.since(r, self.t0));
        let file_d = self.file_delivered.map(|r| self.since(r, self.t0));
        let fv = match file_t {
            Some(d) if d <= FILE_BOUND && self.file_ok => Verdict::Pass,
            _ => Verdict::Fail,
        };
        println!(
            "ÖLÇÜT {tag} dosya: tamamlanma={} s (sınır {} s) bayt_doğru={} parça={} Alice_teslim_bildirimi={} s dosya_bekletme_turu={} → {}",
            opt_secs(file_t),
            secs(FILE_BOUND),
            self.file_ok,
            self.chunk_count,
            opt_secs(file_d),
            self.file_backlog_held,
            fv.label()
        );
        if fv == Verdict::Fail {
            failed.push("DOSYA".to_string());
        }
        line.push(format!("dosya={}({})", opt_secs(file_t), fv.label()));

        // (4) Baytlar.
        let bytes = format!(
            "ileri={} pkt/{} B (kuyruk_düşen={} rastgele_düşen={}) dönüş={} pkt/{} B (kuyruk_düşen={})",
            self.fwd.sent,
            self.fwd.bytes,
            self.fwd.dropped_queue,
            self.fwd.dropped_random,
            self.rev.sent,
            self.rev.bytes,
            self.rev.dropped_queue
        );
        println!(
            "ÖLÇÜT {tag} bayt: {bytes} çözülemeyen_DATA={}/{} kayıt_taşması={}/{}",
            self.alice.unparsed, self.bob.unparsed, self.alice.overflow, self.bob.overflow
        );
        self.trace_report(&tag);
        println!(
            "ÜRÜN-SONUÇ {tag} {} {bytes} düşen_ölçüt={failed:?}",
            line.join(" ")
        );
        failed
    }

    /// Koşu izi: uygulama olayları ve rölenin gördüğü her datagram (an t0'a göre; öğe, mühürleyen
    /// ucun parmak izi kaydından). ACK ve denetim datagramları DATA değildir: `kontrol` yazılır.
    fn trace_report(&self, tag: &str) {
        let t = |i: Instant| secs(self.since(i, self.t0));
        let ot = |i: Option<Instant>| i.map_or("-".to_string(), t);
        for (side, live, ledger, peer) in [
            ("Alice", self.alice_live, &self.alice, &self.bob),
            ("Bob", self.bob_live, &self.bob, &self.alice),
        ] {
            let Some(l) = live else {
                println!("İZ-FEC {tag} {side}: ayna yok");
                continue;
            };
            println!(
                "İZ-FEC {tag} {side}→: latch_blok={} onarım_tx={} parite_kararı={} latch(no_eligible,mismatch,no_rule,enq_r,emit_r)={:?} son_bastırma={:?} onarım_tel_tx={} onarım_karşıda_rx={} RECOVERED_karşıdan={} | PARAM gönderim={} onay={} onay_anı={} | srtt={} ms taban_pto={} ms rtt_örnek={} | tail_prob={} retx={} kopya={} durum={:?}",
                l.fec_blocks_latched,
                l.fec_repairs_tx,
                l.parity_emitted,
                l.fec_latch_diag,
                l.fec_last_suppression,
                ledger.fec_tx,
                peer.fec_rx,
                ledger.recovered_rx,
                l.param_sends,
                l.param_confirmed,
                if l.param_confirmed > 0 {
                    format!("{:.2} s", l.param_confirmed_at_ns as f64 / 1e9)
                } else {
                    "-".to_string()
                },
                l.srtt_us / 1000,
                l.base_pto_us / 1000,
                l.rtt_samples,
                l.tail_probes,
                l.retransmits,
                l.dups,
                l.control.state,
            );
        }
        for (i, x) in self.texts.iter().enumerate() {
            println!(
                "İZ-UYG {tag} metin{} gönderim={} Bob_teslim={} Alice_alındı={}",
                i + 1,
                ot(x.sent.map(|s| s.1)),
                ot(x.received),
                ot(x.delivered)
            );
        }
        let chunks: Vec<String> = self.chunk_rx.iter().map(|&c| t(c)).collect();
        println!(
            "İZ-UYG {tag} ptt_açılış={} PttStarted={} dosya_Bob={} dosya_Alice_bildirimi={} Bob_parça_teslimleri=[{}]",
            ot(self.ptt_open.map(|p| p.1)),
            ot(self.ptt_started),
            ot(self.file_received),
            ot(self.file_delivered),
            chunks.join(" ")
        );
        for (dir, shaper) in [("ileri", &self.fwd), ("dönüş", &self.rev)] {
            for r in &shaper.trace {
                let ledger = match r.route {
                    Route::AUp | Route::BDown => &self.alice,
                    Route::ADown | Route::BUp => &self.bob,
                };
                let item = ledger
                    .by_fp
                    .get(&r.fp)
                    .map_or("kontrol".to_string(), |i| format!("{i:?}"));
                let fate = match r.fate {
                    0 => "ok",
                    1 => "kuyruk_düştü",
                    _ => "yolda_düştü",
                };
                println!(
                    "İZ-HAT {tag} {dir} {:?} sunum={} tel={} {fate} varış={} {item}",
                    r.route,
                    t(r.offered),
                    r.wire,
                    ot(r.arrives)
                );
            }
        }
    }

    fn ptt_report(
        &self,
        tag: &str,
        fwd_drops: &HashSet<u64>,
        file_end: Option<Instant>,
    ) -> (Verdict, String) {
        let Some((stream, opened)) = self.ptt_open else {
            println!("ÖLÇÜT {tag} ptt: açılmadı → KALDI");
            return (Verdict::Fail, "ptt=AÇILMADI(KALDI)".to_string());
        };
        // PTT şeridindeki öncüller: açılış + demetler, gönderim sırasıyla. (öğe, uygulamaya teslim)
        let mut lane: Vec<(Item, Option<Instant>)> =
            vec![(Item::PttOpen(stream), self.ptt_started)];
        let mut missing = 0u32;
        let mut excused = 0u32;
        let mut over = 0u32;
        let mut max_delay = Duration::ZERO;
        let mut max_delay_counted = Duration::ZERO;
        let mut max_excused = Duration::ZERO;
        // Sınırı aşıp kalan demetlerden, kaybolmadan PTT şeridinde ilk iletimi düşmüş bir öncülünü
        // bekleyenler (ayrı sütun; istisna değil).
        let mut held_n = 0u32;
        let mut max_held = Duration::ZERO;
        let mut first_missing: Option<u32> = None;
        let mut max_gap = Duration::ZERO;
        let mut max_accept_wait = Duration::ZERO;
        let mut prev_rx: Option<Instant> = None;
        let mut first: Option<(Duration, Verdict)> = None;
        for (&seq, b) in &self.bundles {
            let item = Item::Bundle(seq);
            max_accept_wait = max_accept_wait.max(self.since(b.accepted, b.planned));
            let Some(rx) = b.received else {
                missing += 1;
                first_missing.get_or_insert(seq);
                println!(
                    "İZ-PTT {tag} demet={seq} plan=+{} ilk_çıkış=+{} iletim={} ilk_iletim_düştü={} ilk_varış=+{} teslim=YOK",
                    secs(self.since(b.planned, self.t0)),
                    opt_secs(self.alice.first_tx.get(&item).map(|(a, _)| self.since(*a, self.t0))),
                    self.alice.tx_count.get(&item).copied().unwrap_or(0),
                    Self::lost_first(&self.alice, fwd_drops, item),
                    opt_secs(self.bob.first_rx.get(&item).map(|a| self.since(*a, self.t0))),
                );
                lane.push((item, None));
                continue;
            };
            let delay = self.since(rx, b.planned);
            max_delay = max_delay.max(delay);
            if file_end.is_none_or(|f| b.planned <= f) {
                if let Some(p) = prev_rx {
                    max_gap = max_gap.max(self.since(rx, p));
                }
            }
            prev_rx = Some(rx);
            let own = Self::lost_first(&self.alice, fwd_drops, item);
            let arrival = self.bob.first_rx.get(&item).copied();
            // Öncül beklemesi: demet Bob'a vardığında ilk iletimi düşmüş bir öncül henüz teslim
            // edilmemişti (varış bilinmiyorsa demet kurtarmayla gelmiştir).
            let held = lane.iter().any(|&(p, p_rx)| {
                Self::lost_first(&self.alice, fwd_drops, p)
                    && match (arrival, p_rx) {
                        (Some(a), Some(pr)) => pr >= a,
                        _ => true,
                    }
            });
            let v = if delay <= PTT_BOUND {
                Verdict::Pass
            } else if own {
                Verdict::LossRecovery
            } else {
                Verdict::Fail
            };
            match v {
                Verdict::Pass => max_delay_counted = max_delay_counted.max(delay),
                Verdict::LossRecovery => {
                    excused += 1;
                    max_excused = max_excused.max(delay);
                }
                Verdict::Fail => {
                    over += 1;
                    max_delay_counted = max_delay_counted.max(delay);
                    if held {
                        held_n += 1;
                        max_held = max_held.max(delay);
                    }
                    println!(
                        "ÖLÇÜT {tag} ptt demet {seq}: gecikme={} s ilk_çıkış=+{} s ilk_varış=+{} s iletim={} → KALDI",
                        secs(delay),
                        opt_secs(self.alice.first_tx.get(&item).map(|(a, _)| self.since(*a, b.planned))),
                        opt_secs(arrival.map(|a| self.since(a, b.planned))),
                        self.alice.tx_count.get(&item).copied().unwrap_or(0),
                    );
                }
            }
            if first.is_none() {
                first = Some((delay, v));
            }
            println!(
                "İZ-PTT {tag} demet={seq} plan=+{} ilk_çıkış=+{} iletim={} ilk_iletim_düştü={own} ilk_varış=+{} teslim=+{} gecikme={} öncül_bekledi={held} → {}",
                secs(self.since(b.planned, self.t0)),
                opt_secs(self.alice.first_tx.get(&item).map(|(a, _)| self.since(*a, self.t0))),
                self.alice.tx_count.get(&item).copied().unwrap_or(0),
                opt_secs(arrival.map(|a| self.since(a, self.t0))),
                secs(self.since(rx, self.t0)),
                secs(delay),
                v.label()
            );
            lane.push((item, Some(rx)));
        }
        let start = self.ptt_started.map(|s| self.since(s, opened));
        let n = self.bundles.len();
        // [ÜRÜN KABULÜ · 2026-11-16] Profil açıkken (ürün kolu) kendi ilk iletimi düşen demet de
        // sınır içinde olmalı — kullanıcı sesi geç duyar; ürün kabulünde "kayıp-kurtarma" başarı
        // sayılmaz. İstisna yalnız profil kapalı teşhis kolunda ayrı raporlanır.
        let verdict = if missing > 0 || over > 0 || n == 0 || (self.profile && excused > 0) {
            Verdict::Fail
        } else {
            Verdict::Pass
        };
        println!(
            "ÖLÇÜT {tag} ptt: demet={n} teslim_yok={missing} (ilki {}) açılış→PttStarted={} s ilk_demet={} s ({}) sınır_üstü_kaldı={over} (bunların öncül_bekledi={held_n}, en büyük {} s) kayıp-kurtarma(kendi_kaybı)={excused} (en büyük {} s) en_büyük_gecikme_sayılan={} s en_büyük_gecikme_tümü={} s dosya_sürerken_en_büyük_boşluk={} s kabul_beklemesi_en_çok={} s → {}",
            first_missing.map_or("-".to_string(), |m| m.to_string()),
            opt_secs(start),
            opt_secs(first.map(|f| f.0)),
            first.map_or("YOK", |f| f.1.label()),
            secs(max_held),
            secs(max_excused),
            secs(max_delay_counted),
            secs(max_delay),
            secs(max_gap),
            secs(max_accept_wait),
            verdict.label()
        );
        (
            verdict,
            format!(
                "ptt_ilk={}({}) ptt_en_büyük_sayılan={} ptt_en_büyük_tümü={} ptt_boşluk={} ptt_kaldı={over}/{n}(öncül_bekledi={held_n}) ptt_kayıp_kurtarma={excused}/{n} ptt_teslim_yok={missing} ptt={}",
                opt_secs(first.map(|f| f.0)),
                first.map_or("YOK", |f| f.1.label()),
                secs(max_delay_counted),
                secs(max_delay),
                secs(max_gap),
                verdict.label()
            ),
        )
    }
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

#[test]
fn text_receipt_ptt_and_file_all_progress_on_the_asymmetric_line() {
    let seed = env_u64("KAMA_ASYM_SEED", 1);
    let profile = std::env::var("KAMA_ASYM_PROFILE").map_or(true, |v| v != "off");
    let real_fwd = env_u64("KAMA_ASYM_REAL_FWD_BPS", FWD_BPS);
    let r = run(seed, profile, real_fwd);
    let failed = r.report();
    if real_fwd != FWD_BPS {
        // Yanlış profil teşhis kolu: profil ileri hattı FWD_BPS bildirir, gerçek hat daha yavaş
        // (ya da hızlı). Soru "ürün çöküyor mu"; kabul sınırları bu hat için yazılmadı.
        println!(
            "TEŞHİS-KOLU yanlış profil: profil ileri {FWD_BPS} B/s, gerçek ileri {real_fwd} B/s \
             (tohum {seed}) — düşen ölçütler: {failed:?}"
        );
        return;
    }
    assert!(
        failed.is_empty(),
        "asimetrik hatta düşen ürün ölçütleri (tohum {seed}, profil {}): {failed:?}",
        if profile { "açık" } else { "kapalı" }
    );
}
