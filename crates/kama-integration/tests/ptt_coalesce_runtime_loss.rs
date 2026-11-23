//! [PTT-BİRLEŞİK · deney · #233 inceleme 4] Açılış/kapanış güvenilirliği GERÇEK çalışma zamanında.
//!
//! # Neyi sınar
//!
//! Sahibin ölçütü: ilk ya da son ses datagramı kaybolsa da basış uygulamaya ulaşır ve sınırlı
//! sürede biter. `kama-app` testleri bunu sahte bağlantıda (elle ilerletilen saat) sınar; orada
//! taşıma katmanının tek iletimi, PTT şeridinin delik atlaması ve son tarihler yoktur. Bu test
//! iki gerçek `ClientTransport` (ürün yapılandırması, bitmap ACK, geri bildirimi aç yön profili →
//! yalın 0x09 PTT çerçevesi) ve üstünde gerçek `Messenger` + `Ptt` ile koşar.
//!
//! # Düzenek
//!
//! Veri UDP loopback, el sıkışma zarfları `MemoryHub`. Bob'un kriptosunu saran sarmalayıcı
//! yalnız yalın PTT datagramlarına (0x09) dokunur: basışın `i`. 0x09 datagramını (`i` kümesi
//! senaryonun) AEAD'den sonra atar (S3 K5: `open` → `None`). Basıştan önce PTT trafiği yoktur;
//! basıştaki her demet tam bir 0x09 datagramıdır (birleşik akış tek iletim, kopya yok — sayı
//! iddia edilir).
//!
//! Basıştan önce bileşim kökünün işi yapılır ve İDDİA edilir: Alice'in CAPS'i (bit 2) Bob'a
//! varır, Bob 0x17 yazar, Alice `ClientTransport::peer_ptt_coalesce` ile `Some(true)` görür ve
//! `Ptt::on_peer_spurt_capability`'ye verir. Zincir kurulmazsa test DÜŞER (sessiz geri düşüş yok).
//! Eski eş kolu: Bob'un bayrağı kapalı → ilk yalın ACK 0x16 → `Some(false)` → bugünkü ayrı
//! güvenilir kayıtlar, uçtan uca çalışır.
//!
//! # Senaryolar ve iddialar (sınırlar ölçümden ÖNCE)
//!
//! Basış: [`BUNDLES`] × [`BUNDLE_MS`] demet ve ürün kolları (300 ms, 420 ms demet, 1,5 s basış; son
//! demet kısmi), son demet `END`'li. Kayıp: yok · ilk · son · ilk+son · ilk iki. Zamanında
//! (varış − oluşum ≤ 1 s) 20 ms çerçeve oranı yalnız raporlanır. Her senaryoda:
//! * tam BİR `PttStarted` ve BİR `PttEnded`;
//! * `PttStarted` ≤ ilk atılmayan demetin oluşumu + 1 s + alıcının delik beklemesi (100 ms);
//! * `PttEnded` ≤ bırakış + `spurt_gap_end(bundle_ms)` + [`ONE_WAY_ALLOWANCE`];
//! * ses: teslim edilen demetler tam olarak atılmayanlardır (kurtarılmış açılış, kaybolan sesin
//!   kurtarıldığı anlamına gelmez; kaybolan 20 ms çerçeveler ayrıca sayılıp yazılır).
//!
//! [PTT-BİRLEŞİK · pilot · #233] Pilot kolu: basış `Ptt::open_press` ile açılır (çağıranın süresi
//! [`PILOT_FALLBACK_MS`]); doğrulanmış eşte motor 300 ms'yi seçer ve bu İDDİA edilir. 1,5 s ve
//! 8,4 s basış, aynı dört kayıp. Kaybolan (hiç gelmeyen) ve geç (gelen ama oluşumundan > 1 s)
//! 20 ms çerçeveler ayrı sayılır.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use client_core::memory_link::MemoryHub;
use client_core::runtime::{AckFormat, LinkRates};
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::wire::decode_ptt;
use kama_app::clock::{MonotonicClock, SystemClock};
use kama_app::codec::{bundle_code_from_ms, PttCodec};
use kama_app::messaging::Messenger;
use kama_app::ptt::{spurt_gap_end, Ptt, PttEvent, SPURT_PILOT_BUNDLE_MS};
use kama_app::store::MemoryAppStore;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::ConnectResult;
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit, SessionCommand, SessionControl, SessionEvent as CryptoEvent, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;
/// Demet süresi (5 × 20 ms çerçeve).
const BUNDLE_MS: u16 = 100;
/// Basıştaki demet sayısı (basış 0,8 s).
const BUNDLES: u32 = 8;
/// Lyra 3,2 kbit/s: 20 ms çerçevede 8 B. Demet yükü = çerçeve sayısı × bu (ilk 4 B demet sırası;
/// 100 ms demette 40 B, pilotun 300 ms'sinde 120 B).
const LYRA_FRAME_BYTES: usize = 8;
/// Lyra çerçevesi.
const FRAME: Duration = Duration::from_millis(20);
/// İlk sesin / açılışın üst sınırı (basıştan).
const START_BOUND: Duration = Duration::from_secs(1);
/// Loopback tek yön + iş parçacığı zamanlaması payı (beyanlı; hat gecikmesi yok).
const ONE_WAY_ALLOWANCE: Duration = Duration::from_millis(150);
/// Kurulum ve yetenek zinciri için üst süre.
const SETUP_WAIT: Duration = Duration::from_secs(30);
/// [PTT-BİRLEŞİK · pilot] Pilot kolunda çağıranın (media) verdiği süre: bugünkü fikstür
/// varsayılanı 200 ms. Doğrulanmış eşte motor bunu değil [`SPURT_PILOT_BUNDLE_MS`]'yi seçmeli.
const PILOT_FALLBACK_MS: u16 = 200;

/// Basışın nasıl açıldığı.
#[derive(Debug, Clone, Copy)]
enum Open {
    /// `Ptt::open`, açık süreyle (100 ms senaryoları ve 420 ms test kolu).
    Explicit(u16),
    /// `Ptt::open_press`, çağıranın süresi [`PILOT_FALLBACK_MS`]; motorun seçtiği süre kullanılır.
    Pilot,
}

impl Open {
    /// Beklenen demet süresi (ms).
    fn expected_ms(self) -> u16 {
        match self {
            Self::Explicit(ms) => ms,
            Self::Pilot => SPURT_PILOT_BUNDLE_MS,
        }
    }
}

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x5C;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

/// Gerçek kripto + Bob'un alıcı yönünde basışın seçilen 0x09 datagramlarını atan sarmalayıcı.
struct PttDrop {
    inner: NoiseWireProtection,
    drop_idx: BTreeSet<u64>,
    seen: Arc<AtomicU64>,
    dropped: Arc<Mutex<Vec<u64>>>,
}

impl PacketProtection for PttDrop {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        self.inner.seal(ctx, frame)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let od = self.inner.open(wire)?;
        if decode_ptt(&od.frame).is_some() {
            let i = self.seen.fetch_add(1, Ordering::SeqCst);
            if self.drop_idx.contains(&i) {
                self.dropped.lock().expect("kilit").push(i);
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

impl SessionControl for PttDrop {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        self.inner.command(cmd)
    }
    fn poll_event(&mut self) -> Option<CryptoEvent> {
        self.inner.poll_event()
    }
}

fn crypto(me: u8) -> NoiseWireProtection {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0
}

/// Ürün yapılandırması + bitmap ACK + geri bildirimi aç yön profili (yalın PTT yolu) + deney
/// bayrağı.
fn cfg(up: u64, down: u64, coalesce: bool) -> ClientTransportConfig {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.ack_format = AckFormat::Bitmap;
    cfg.runtime.link_rates = Some(LinkRates {
        uplink_bps: up,
        downlink_bps: down,
    });
    cfg.runtime.ptt_coalesce = coalesce;
    cfg
}

struct Side<C: PacketProtection + SessionControl + Send + 'static> {
    m: Messenger<ClientTransport<C>>,
    ptt: Ptt,
}

impl<C: PacketProtection + SessionControl + Send + 'static> Side<C> {
    fn new(t: ClientTransport<C>, clock: &Arc<dyn MonotonicClock>) -> Self {
        let m = Messenger::with_store(t, Arc::new(MemoryAppStore::new())).expect("AMP limitleri");
        let mut ptt = Ptt::new(Arc::clone(clock));
        // Tek anahtar: motorun anahtarı Transport'un bayrağından (kama-node `ptt_engine`).
        ptt.set_spurt_coalesce(m.transport().ptt_coalesce_enabled());
        Self { m, ptt }
    }

    fn turn(&mut self, out: &mut Vec<(Instant, PttEvent)>) {
        for _ in 0..256 {
            let Some(e) = self.ptt.poll(&mut self.m) else {
                break;
            };
            out.push((Instant::now(), e));
        }
    }
}

#[derive(Debug)]
struct Outcome {
    capability: Option<bool>,
    /// Akışın gerçek demet süresi (ms; pilot kolunda motorun seçtiği).
    bundle_ms: u16,
    lean_seen: u64,
    dropped: Vec<u64>,
    started: Vec<Duration>,
    ended: Vec<Duration>,
    release_to_end: Option<Duration>,
    delivered: BTreeSet<u32>,
    frames_lost: u64,
    /// Planlanan 20 ms çerçeve sayısı (basış / 20 ms).
    frames_planned: u64,
    /// Zamanında (Bob'a varış − demet oluşumu ≤ 1 s) teslim edilen 20 ms çerçeve sayısı.
    frames_timely: u64,
    /// Teslim edilen ama geç (varış − oluşum > 1 s) 20 ms çerçeve sayısı (kaybolanlardan ayrı).
    frames_late: u64,
    /// Bob'un birleşik açılışla açtığı akış sayısı (0 = ayrı `PTT_OPEN`).
    bob_spurt_opens: u64,
    /// Bob'un `END` ile bitirdiği akış sayısı.
    bob_spurt_ends: u64,
    /// Bob'un sessizlik sonuyla bitirdiği akış sayısı.
    bob_gap_ends: u64,
    /// Bob'da yetim (akışı bulunamayan) demet.
    bob_orphans: u64,
}

#[allow(clippy::too_many_lines)]
fn run(open: Open, press_len: Duration, drop_idx: &[u64], bob_coalesce: bool) -> Outcome {
    let hub = MemoryHub::new();
    let seen = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(Mutex::new(Vec::new()));
    let alice_t = ClientTransport::new(
        cfg(12_500, 62, true),
        crypto(ALICE),
        Box::new(hub.link(id(ALICE))),
    )
    .expect("alice");
    let bob_t = ClientTransport::new(
        cfg(62, 12_500, bob_coalesce),
        PttDrop {
            inner: crypto(BOB),
            drop_idx: drop_idx.iter().copied().collect(),
            seen: Arc::clone(&seen),
            dropped: Arc::clone(&dropped),
        },
        Box::new(hub.link(id(BOB))),
    )
    .expect("bob");
    let clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
    let mut alice = Side::new(alice_t, &clock);
    let mut bob = Side::new(bob_t, &clock);
    let (a_addr, b_addr) = (
        alice.m.transport().data_addr(),
        bob.m.transport().data_addr(),
    );
    alice.m.transport_mut().add_peer_addr(id(BOB), b_addr);
    bob.m.transport_mut().add_peer_addr(id(ALICE), a_addr);
    match alice.m.connect(id(BOB)).expect("connect") {
        ConnectResult::Pending(_) | ConnectResult::Ready(_) => {}
    }
    let mut events = Vec::new();
    let deadline = Instant::now() + SETUP_WAIT;
    let mut texted = false;
    // Oturum ve bant içi yetenek (CAPS bit 2 → 0x17 / 0x16 → peer_ptt_coalesce → Ptt).
    let capability = loop {
        hub.relay();
        alice.turn(&mut events);
        bob.turn(&mut events);
        events.clear();
        if let Some(h) = alice.m.session_of(id(BOB)) {
            // ACK'ler veriye cevaptır: yetenek bildirimi için önce bir metin gider (PTT değil;
            // PTT şeridinde basıştan önce trafik yok).
            if !texted && bob.m.session_of(id(ALICE)).is_some() {
                alice.m.send_text(id(BOB), "yetenek").expect("metin");
                texted = true;
            }
            if let Some(v) = alice.m.transport().peer_ptt_coalesce(h) {
                assert!(alice.ptt.on_peer_spurt_capability(id(BOB), h, v));
                break Some(v);
            }
        }
        if Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(
        capability,
        Some(bob_coalesce),
        "yetenek zinciri kurulmadı ya da yanlış (sessiz geri düşüş)"
    );
    assert!(bob.m.session_of(id(ALICE)).is_some());

    let press = Instant::now();
    let (stream, code) = match open {
        Open::Explicit(ms) => {
            let code = bundle_code_from_ms(ms).expect("kod");
            let stream = alice
                .ptt
                .open(&mut alice.m, id(BOB), PttCodec::Lyra, code)
                .expect("PTT açılır");
            (stream, code)
        }
        Open::Pilot => alice
            .ptt
            .open_press(
                &mut alice.m,
                id(BOB),
                PttCodec::Lyra,
                bundle_code_from_ms(PILOT_FALLBACK_MS).expect("kod"),
            )
            .expect("PTT açılır"),
    };
    // Demetler akışın GERÇEK süresiyle paketlenir (pilot kolunda motorun seçtiği).
    let bundle_ms = kama_app::codec::bundle_ms_from_code(code);
    let period = Duration::from_millis(u64::from(bundle_ms));
    let n = u32::try_from(press_len.as_nanos().div_ceil(period.as_nanos())).expect("demet sayısı");
    let payload_len = usize::from(bundle_ms) / 20 * LYRA_FRAME_BYTES;
    let release = press + press_len;
    // Demetin çerçeveleri var olunca (ya da bırakışta, kısmi son demet) gönderilir.
    let ready = |k: u32| (press + period * (k + 1)).min(release);
    let mut next = 0u32;
    let mut closed = false;
    let mut bob_events: Vec<(Instant, PttEvent)> = Vec::new();
    let end_bound = release + spurt_gap_end(code) + ONE_WAY_ALLOWANCE;
    let hard_stop = end_bound + Duration::from_secs(2);
    loop {
        let tick = Instant::now();
        while next < n && ready(next) <= tick {
            let created = press + period * next;
            let mut payload = vec![0x5A; payload_len];
            payload[..4].copy_from_slice(&next.to_be_bytes());
            let r = if next + 1 == n {
                alice
                    .ptt
                    .send_last_bundle_from(&mut alice.m, stream, &payload, created)
            } else {
                alice
                    .ptt
                    .send_bundle_from(&mut alice.m, stream, &payload, created)
            };
            r.expect("demet kabul edilir");
            next += 1;
        }
        if next == n && !closed {
            alice.ptt.close(&mut alice.m, stream).expect("kapanır");
            closed = true;
        }
        hub.relay();
        alice.turn(&mut events);
        events.clear();
        bob.turn(&mut bob_events);
        let ended = bob_events
            .iter()
            .any(|(_, e)| matches!(e, PttEvent::PttEnded { .. }));
        // Kapanıştan sonra bir süre daha (ikinci PttEnded'i yakalamak için) sürülür.
        if (ended && tick >= release + Duration::from_millis(300)) || tick >= hard_stop {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // İkinci `PttEnded` / `PttStarted` aranır: sessizlik sonundan sonra kısa ek bekleme.
    let extra = Instant::now() + Duration::from_millis(300);
    while Instant::now() < extra {
        hub.relay();
        bob.turn(&mut bob_events);
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut started = Vec::new();
    let mut ended = Vec::new();
    let mut release_to_end = None;
    let mut delivered = BTreeSet::new();
    let mut arrival: std::collections::BTreeMap<u32, Instant> = std::collections::BTreeMap::new();
    for (at, e) in &bob_events {
        match e {
            PttEvent::PttStarted { .. } => started.push(at.duration_since(press)),
            PttEvent::PttEnded { .. } => {
                ended.push(at.duration_since(press));
                release_to_end.get_or_insert(at.saturating_duration_since(release));
            }
            PttEvent::PttAudio { payload, .. } if payload.len() >= 4 => {
                let k = u32::from_be_bytes(payload[..4].try_into().expect("4 B"));
                delivered.insert(k);
                arrival.entry(k).or_insert(*at);
            }
            _ => {}
        }
    }
    // k. demetin çerçeveleri: tam demette B / 20 ms, son demette bırakışta var olanlar.
    let frames_of = |k: u32| -> u64 {
        let span = release
            .saturating_duration_since(press + period * k)
            .min(period);
        u64::try_from(span.as_nanos() / FRAME.as_nanos()).unwrap_or(0)
    };
    let frames_planned: u64 = (0..n).map(frames_of).sum();
    let frames_lost: u64 = (0..n)
        .filter(|k| !delivered.contains(k))
        .map(frames_of)
        .sum();
    let frames_timely: u64 = arrival
        .iter()
        .filter(|(&k, &at)| at.saturating_duration_since(press + period * k) <= START_BOUND)
        .map(|(&k, _)| frames_of(k))
        .sum();
    let frames_late: u64 = arrival
        .iter()
        .filter(|(&k, &at)| at.saturating_duration_since(press + period * k) > START_BOUND)
        .map(|(&k, _)| frames_of(k))
        .sum();
    let bob_stats = bob.ptt.stats();
    let dropped = dropped.lock().expect("kilit").clone();
    Outcome {
        capability,
        bundle_ms,
        lean_seen: seen.load(Ordering::SeqCst),
        dropped,
        started,
        ended,
        release_to_end,
        delivered,
        frames_lost,
        frames_planned,
        frames_timely,
        frames_late,
        bob_spurt_opens: bob_stats.spurt_opens,
        bob_spurt_ends: bob_stats.spurt_ends,
        bob_gap_ends: bob_stats.streams_ended_by_gap,
        bob_orphans: bob_stats.ptt_orphan_bundles,
    }
}

/// Kayıp senaryosu: basıştaki demet sıraları; `LAST` = son demet.
const LAST: u64 = u64::MAX;

fn check(name: &str, open: Open, press_len: Duration, scenario: &[u64]) {
    let bundle_ms = open.expected_ms();
    let period = Duration::from_millis(u64::from(bundle_ms));
    let n = u64::try_from(press_len.as_nanos().div_ceil(period.as_nanos())).expect("n");
    let drop_idx: Vec<u64> = scenario
        .iter()
        .map(|&k| if k == LAST { n - 1 } else { k })
        .collect();
    let o = run(open, press_len, &drop_idx, true);
    assert_eq!(o.bundle_ms, bundle_ms, "{open:?}: akışın demet süresi");
    let code = bundle_code_from_ms(bundle_ms).expect("kod");
    let end_limit = spurt_gap_end(code) + ONE_WAY_ALLOWANCE;
    println!(
        "PTT-BİRLEŞİK-ÇALIŞMA {name} demet={bundle_ms}ms basış={:?}: kayıp={drop_idx:?} 0x09_görülen={} başladı={} bitti={} tek_başla/tek_bitir={}/{} PttStarted=+{:?} (basıştan) PttEnded=+{:?} (bırakıştan; sınır {end_limit:?}) teslim_demet={:?} kaybolan_20ms_çerçeve={} geç_20ms_çerçeve={} zamanında={}/{} ({} %)",
        press_len,
        o.lean_seen,
        !o.started.is_empty(),
        !o.ended.is_empty(),
        o.started.len() == 1,
        o.ended.len() == 1,
        o.started.first(),
        o.release_to_end,
        o.delivered,
        o.frames_lost,
        o.frames_late,
        o.frames_timely,
        o.frames_planned,
        o.frames_timely * 100 / o.frames_planned.max(1),
    );
    // Yalnız açılış/kapanış güvenilirliği ve teslim kümesi iddia edilir; zamanında oranı
    // (≥ %75 kapısı rastgele kayıp içindir) burada yalnız raporlanır.
    assert_eq!(o.capability, Some(true));
    assert_eq!(o.bob_spurt_opens, 1, "akış birleşik açılışla açıldı");
    assert_eq!(
        o.bob_spurt_ends,
        u64::from(!drop_idx.contains(&(n - 1))),
        "END yalnız son demet vardıysa"
    );
    assert_eq!(o.lean_seen, n, "her demet tek 0x09 datagramı");
    assert_eq!(o.dropped, drop_idx, "seçilen datagramlar atıldı");
    assert_eq!(o.started.len(), 1, "tam bir PttStarted");
    assert_eq!(o.ended.len(), 1, "tam bir PttEnded");
    // PttStarted: ilk atılmayan demetin oluşumundan ≤ 1 s + alıcının delik beklemesi.
    let first = (0..n)
        .find(|k| !drop_idx.contains(k))
        .expect("en az bir demet");
    let start_limit = period * u32::try_from(first).expect("u32")
        + START_BOUND
        + kama_app::ptt::SPURT_RX_HOLE_WAIT;
    assert!(o.started[0] <= start_limit, "PttStarted {:?}", o.started[0]);
    let end = o.release_to_end.expect("bitti");
    assert!(end <= end_limit, "PttEnded bırakıştan {end:?}");
    let want: BTreeSet<u32> = (0..u32::try_from(n).expect("u32"))
        .filter(|k| !drop_idx.contains(&u64::from(*k)))
        .collect();
    assert_eq!(
        o.delivered, want,
        "teslim = atılmayan demetler (kaybolan ses kurtarılmaz)"
    );
}

fn short_press() -> Duration {
    Duration::from_millis(u64::from(BUNDLE_MS)) * BUNDLES
}

#[test]
fn coalesced_press_without_loss_on_the_real_runtime() {
    check("kayıpsız", Open::Explicit(BUNDLE_MS), short_press(), &[]);
}

#[test]
fn coalesced_press_survives_losing_the_first_datagram_on_the_real_runtime() {
    check("ilk", Open::Explicit(BUNDLE_MS), short_press(), &[0]);
}

#[test]
fn coalesced_press_survives_losing_the_last_datagram_on_the_real_runtime() {
    check("son", Open::Explicit(BUNDLE_MS), short_press(), &[LAST]);
}

#[test]
fn coalesced_press_survives_losing_the_first_and_last_datagrams_on_the_real_runtime() {
    check(
        "ilk+son",
        Open::Explicit(BUNDLE_MS),
        short_press(),
        &[0, LAST],
    );
}

#[test]
fn coalesced_press_survives_losing_the_first_two_datagrams_on_the_real_runtime() {
    check("ilk-iki", Open::Explicit(BUNDLE_MS), short_press(), &[0, 1]);
}

/// Dört belirlenimli kayıp senaryosu.
const LOSS_SCENARIOS: [(&str, &[u64]); 4] = [
    ("ilk", &[0]),
    ("son", &[LAST]),
    ("ilk+son", &[0, LAST]),
    ("ilk-iki", &[0, 1]),
];

/// [#233 tur 2 · 2] 420 ms TEST KOLU (pilot ayarı değil; `Ptt::open` açık süreyle), 1,5 s basış:
/// aynı dört kayıp. Açılış/kapanış güvenilirliği iddia edilir; zamanında oranı yalnız raporlanır
/// (belirlenimli kayıpta kaybolan demetin çerçeveleri zamanında olamaz).
#[test]
fn test_arm_420_ms_keeps_open_and_close_reliable_on_the_real_runtime() {
    let press = Duration::from_millis(1_500);
    for (name, scenario) in LOSS_SCENARIOS {
        check(name, Open::Explicit(420), press, scenario);
    }
}

/// [PTT-BİRLEŞİK · pilot · #233] Pilot (doğrulanmış eş, `open_press`): motor 300 ms seçer
/// (iddia); 1,5 s ve 8,4 s basışta ilk / son / ilk+son / ilk iki datagram kaybında tam bir
/// `PttStarted` ve bir `PttEnded`, sınırlı sürede; kaybolan ve geç çerçeveler ayrı yazılır.
#[test]
fn pilot_300_ms_keeps_one_open_and_one_close_under_loss_on_the_real_runtime() {
    for press_ms in [1_500u64, 8_400] {
        for (name, scenario) in LOSS_SCENARIOS {
            check(name, Open::Pilot, Duration::from_millis(press_ms), scenario);
        }
    }
}

/// Eski eş: Bob'un bayrağı kapalı → ilk yalın ACK 0x16 → Alice bugünkü ayrı güvenilir kayıtlara
/// döner; basış uçtan uca çalışır (tam bir başla/bitir, bütün ses).
#[test]
fn old_peer_gets_the_compatible_separate_records_on_the_real_runtime() {
    let o = run(Open::Explicit(BUNDLE_MS), short_press(), &[], false);
    println!("PTT-BİRLEŞİK-ÇALIŞMA eski-eş: {o:?}");
    assert_eq!(o.capability, Some(false));
    assert_eq!(
        (o.bob_spurt_opens, o.bob_spurt_ends),
        (0, 0),
        "ayrı kayıtlar"
    );
    assert_eq!((o.started.len(), o.ended.len()), (1, 1));
    assert!(o.started[0] <= START_BOUND);
    assert_eq!(o.delivered, (0..BUNDLES).collect::<BTreeSet<u32>>());
}

/// [#233 inceleme 7087771388 · 1, tur 2] Basış ortasında `spurt_gap_end`'den uzun kesinti (100 ms
/// demette 14 demet = 1,4 s > 1,3 s): sessizlik kuralı akışı bitirir (`PttEnded`); kesintiden
/// sonraki ilk açılışlı demet (her 4. demet: 16.) akışı yeniden açar ve sesin kalanı çalınır
/// (yetim yok; kesinti 16. demette bittiği için). Bedel: basış iki yaşam döngüsü olur; tek
/// yaşam döngüsü için kesintiyi sessizlik sonundan ayırt edecek bir kapanış sinyali gerekir
/// (açık karar). Yalnız ilk K demet açılış taşısaydı kesintiden sonraki bütün demetler yetim
/// kalırdı.
#[test]
fn coalesced_press_resumes_after_an_outage_longer_than_the_silence_end_on_the_real_runtime() {
    let drop: Vec<u64> = (2..=15).collect();
    let o = run(Open::Explicit(BUNDLE_MS), short_press() * 3, &drop, true);
    println!(
        "PTT-BİRLEŞİK-ÇALIŞMA kesinti: kayıp=2..=15 başla={:?} bitir={:?} teslim_demet={:?} kaybolan_20ms_çerçeve={} sessizlik_sonu={} yetim={}",
        o.started, o.ended, o.delivered, o.frames_lost, o.bob_gap_ends, o.bob_orphans
    );
    assert_eq!(o.capability, Some(true));
    assert_eq!(o.dropped, drop);
    let want: BTreeSet<u32> = (0..24).filter(|k| !drop.contains(&u64::from(*k))).collect();
    assert_eq!(o.delivered, want, "kesintiden sonraki ses çalınır");
    assert_eq!(o.bob_orphans, 0);
    assert_eq!((o.started.len(), o.ended.len()), (2, 2));
    assert_eq!((o.bob_gap_ends, o.bob_spurt_ends), (1, 1));
}
