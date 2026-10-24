//! **K4G-05c P0 — alıcı kabul bütünlüğü: TABANDA KIRMIZI beklenen belirlenimli regresyonlar**
//! (CEVAP 9 §3 K4G-05c, §4 P0 listesi). Taban: main 3439fccd. Bu dosya düzeltme İÇERMEZ; kusuru
//! görünür kılar ve K4G-05c merge olunca yeşile dönmesi beklenir.
//!
//! Kapsam:
//! * R2 — veri alıcısı düzeyi (`ServerEndpoint` / `ClientEndpoint`): teslim kanalı doluyken birim
//!   düşer, ama PN ACK'lenir ve teslim-dedup'ı ilerler; gönderen ACK üzerine payload'u siler.
//! * R2 → HOL — S1 adaptörü düzeyi (`ClientTransport`): düşen birim `Reorder`'ın `next`'ini
//!   sabitler; arkasındaki teslimler `held`'de `max_inflight_units` (4096) dolana dek bekler.
//!
//! R1 (`Reorder` boşluk atlaması) özel bir tip olduğundan `src/transport.rs` birim testindedir:
//! `p0_r1_reorder_never_skips_missing_reliable`.
//!
//! YAZILMAYAN: sızıntı testi (`seq_to_unit` / `unit_seqs`, runtime.rs:1382-1383, ekleme
//! :1469-1470, budama yalnız `store.remove(..).is_some()` dalında :1687-1692 ve :1736-1743).
//! Tablolar `client_task` içinde yereldir; hiçbir istatistik ya da erişimci boyutlarını
//! taşımaz. Ürün koduna gözlem kancası eklenmedi (talimat); gereken kanca orkestratör raporunda.
//!
//! ÜRÜN YOLU: iki uç da `RuntimeConfig::default()` (= `KamaV1Profile`, FEC açık) ya da
//! `ClientTransportConfig::product` ile kurulur. TEK sapma alıcının `delivery_queue`'sudur (ürün
//! 1024 → [`SMALL_DELIVERY_QUEUE`]): kusur dolu kanaldaki `try_send` başarısızlığıdır
//! (runtime.rs:2380-2387) ve kapasiteden bağımsızdır; küçük kanal taşmayı birkaç düzine birimle,
//! saniyeler içinde ve belirlenimli tetikler. `test-util` gerekir (`memory_link`, `*_for_test`
//! erişimcileri): dev-dependency onu açar, `--all-features` de kapsar.
//!
//! BEKLEMELER yalnız ÜST SINIRDIR: her evre koşul olayıyla (tüm ACK'ler / tüm teslimler) ya da
//! "ilerleme durdu" sessizliğiyle biter. Tabanda iki test de birkaç saniyede kırmızı biter;
//! düzeltilmiş kodda ilerleme sürdükçe bekler (ARQ / RTO geri çekilmesi için pay).

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use client_core::memory_link::MemoryHub;
use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint};
use client_core::seam::IdentitySeam;
use client_core::transport::{ClientTransport, ClientTransportConfig};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    CompressionPolicy, ConnectResult, Delivered, DeliveryPolicy, KamaTransport, PreparedUnit,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

/// Alıcının teslim kanalı kapasitesi (ürün `KamaV1Profile::runtime_config`: 1024). Gerekçe: modül
/// belgesi "ÜRÜN YOLU".
const SMALL_DELIVERY_QUEUE: usize = 4;
/// Tüketici dururken: teyit akışı bu süre değişmezse durmuş sayılır (tabanda tüm birimler bundan
/// çok önce ACK'lenir; düzeltilmiş kodda kanaldakiler dışında ACK gelmez).
const STALL_QUIET: Duration = Duration::from_secs(3);
/// Durak evresinin üst sınırı.
const STALL_MAX: Duration = Duration::from_secs(20);
/// Drenaja dönüşte: gönderende teyitsiz birim kalmadıysa ve bu süre yeni teslim gelmediyse
/// gelebilecek teslim yoktur (gönderen artık hiçbir birimi yeniden yollamaz).
const DRAIN_QUIET: Duration = Duration::from_secs(1);
/// Drenaj evresinin üst sınırı (düzeltilmiş kodda geri çekilmiş yeniden gönderim; RTO_MAX 30 s
/// için pay).
const DRAIN_MAX: Duration = Duration::from_secs(90);
/// Yoklama adımı.
const POLL: Duration = Duration::from_millis(5);

// ── R2: veri alıcısı düzeyi ──────────────────────────────────────────────────────────────────

/// Birime özgü içerik (teslimin byte-eşliği birimle doğrulanır).
fn r2_payload(unit: u64) -> Vec<u8> {
    format!("p0-r2-{unit}").into_bytes()
}

/// [K4G-05c P0 · R2] Teslim kanalı dolu: tutulamayan birim ACK'lenmemeli; her birim TAM bir kez
/// teslim edilmeli.
///
/// BULGU (R2; CEVAP 9 §3, inceleme S09/S10, doğrulama R2): veri alıcısının DATA kolunda
/// (runtime.rs:2933-2960) `Receiver::on_datagram` PN'i önce ACK kümesine yazar
/// (receiver.rs:233), `admit_chunk` birimi teslimden ÖNCE `delivered_seen`'e koyar
/// (runtime.rs:2410), `push_delivery` dolu kanalda `try_send` başarısız olunca payload'u
/// düşürüp yalnız `delivery_dropped`'ı artırır (runtime.rs:2380-2387); `admit_chunk` sonuç
/// döndürmediği için ACK yine gider (runtime.rs:2957-2960). Gönderen ACK'te birimi store'dan
/// siler (runtime.rs:1735-1743) → Reliable birim kalıcı ve sessiz kaybolur; taze PN'li bir kopya
/// gelse de `delivered_seen` onu bastırır (runtime.rs:2410-2411).
///
/// DÜZENEK: `runtime_e2e::delivery_overflow_is_counted_not_grown` kurgusu (o test bu davranışı
/// beklenen diye sabitler — runtime_e2e.rs:288; K4G-05c'de DUR-8 biçiminde değişir). Gönderen
/// `N` birim sunar; tüketici DURUR (teslim kanalı okunmaz) ve teyit akışı durana dek beklenir;
/// sonra tüketici drenaja döner ve teslimler toplanır (teyitler de toplanmaya devam eder).
///
/// TABANDA BEKLENEN: KIRMIZI — `N` birimin hepsi teyitli, ama yalnız ilk
/// [`SMALL_DELIVERY_QUEUE`] tanesi teslim edilir; kalanlar `delivery_dropped`'ta sayılır. İlk
/// kırmızı iddia "ACK'lenen her birim teslim edilmiş olmalı"dır. Kanıtın kalanı (kopyasızlık,
/// payload eşliği) tabanda yeşildir. "Dedup ilerledi" iç durumdur, ayrıca gözlenmez; son
/// eksiksizlik iddiası onu da kapsar: ACK'i tutup dedup'ı ilerleten yarım bir düzeltmede yeniden
/// gönderim bastırılır ve birim yine eksik kalır.
#[tokio::test(flavor = "multi_thread")]
async fn p0_r2_full_channel_never_acks_undelivered() {
    const N: u64 = 40;
    let mut server = ServerEndpoint::bind(
        SocketAddr::from(([127, 0, 0, 1], 0)),
        RuntimeConfig {
            delivery_queue: SMALL_DELIVERY_QUEUE,
            ..RuntimeConfig::default()
        },
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("veri alıcısı");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        RuntimeConfig::default(),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("gönderen uç");
    for unit in 0..N {
        client.send(r2_payload(unit)).expect("sun");
    }

    // ── Evre 1: tüketici DURUR — teslim kanalı okunmaz. ──
    let mut acked: BTreeSet<u64> = BTreeSet::new();
    let start = Instant::now();
    let mut last_progress = start;
    while (acked.len() as u64) < N
        && start.elapsed() < STALL_MAX
        && last_progress.elapsed() < STALL_QUIET
    {
        if let Ok(Some(unit)) = tokio::time::timeout(POLL, client.acked()).await {
            if acked.insert(unit) {
                last_progress = Instant::now();
            }
        }
    }
    let acked_while_stalled = acked.len();
    let dropped_while_stalled = server.delivery_dropped();

    // ── Evre 2: tüketici drenaja döner. ──
    let mut delivered: Vec<u64> = Vec::new();
    let mut wrong_payload: Vec<u64> = Vec::new();
    let start = Instant::now();
    let mut last_progress = start;
    loop {
        let distinct: BTreeSet<u64> = delivered.iter().copied().collect();
        if distinct.len() as u64 >= N
            || start.elapsed() >= DRAIN_MAX
            || (client.outstanding() == 0 && last_progress.elapsed() >= DRAIN_QUIET)
        {
            break;
        }
        tokio::select! {
            Some(d) = server.next_delivery() => {
                if d.payload != r2_payload(d.unit_id) {
                    wrong_payload.push(d.unit_id);
                }
                delivered.push(d.unit_id);
                last_progress = Instant::now();
            }
            Some(unit) = client.acked() => {
                if acked.insert(unit) {
                    last_progress = Instant::now();
                }
            }
            () = tokio::time::sleep(POLL) => {}
        }
    }
    let live_dropped = server.delivery_dropped();
    let _ = client.shutdown().await;
    let ss = server.shutdown().await;

    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let duplicates: Vec<u64> = delivered
        .iter()
        .copied()
        .filter(|u| !seen.insert(*u))
        .collect();
    let acked_not_delivered: Vec<u64> = acked.difference(&seen).copied().collect();
    let missing: Vec<u64> = (0..N).filter(|u| !seen.contains(u)).collect();
    let summary = format!(
        "N={N}; durakta: teyitli={acked_while_stalled}, delivery_dropped={dropped_while_stalled}; \
         sonda: teslim={} (tekil {}), teyitli={}, delivery_dropped(canlı)={live_dropped}, \
         ServerStats{{delivered={}, delivery_dropped={}}}",
        delivered.len(),
        seen.len(),
        acked.len(),
        ss.delivered,
        ss.delivery_dropped
    );
    assert!(
        wrong_payload.is_empty(),
        "teslim edilen payload birimiyle byte-eş olmalı: {wrong_payload:?} — {summary}"
    );
    assert!(
        duplicates.is_empty(),
        "her birim TAM bir kez teslim edilmeli; çift teslim: {duplicates:?} — {summary}"
    );
    assert!(
        acked_not_delivered.is_empty(),
        "R2 KUSURU: gönderen {} birimi ACK ile teyitli gördü ve store'dan sildi \
         (runtime.rs:1735-1743), ama alıcı onları uygulamaya HİÇ teslim etmedi: \
         {acked_not_delivered:?}. Neden: veri alıcısında tek kabul kararı yok — \
         `Receiver::on_datagram` PN'i ACK kümesine yazar (receiver.rs:233), `admit_chunk` \
         `delivered_seen`'i teslimden ÖNCE ilerletir (runtime.rs:2410), `push_delivery` dolu \
         kanalda `try_send` başarısız olunca payload'u düşürüp yalnız `delivery_dropped` sayar \
         (runtime.rs:2380-2387); ACK yine gider (runtime.rs:2957-2960) — {summary}",
        acked_not_delivered.len()
    );
    assert_eq!(
        live_dropped, 0,
        "teslim kanalı taşması birimi düşürdü (`ServerHandle::delivery_dropped` > 0): kanal \
         doluyken birim ACK'siz reddedilmeli (gönderen yeniden yollar), düşürülmemeli — {summary}"
    );
    assert_eq!(
        ss.delivery_dropped, 0,
        "kapanış istatistiğinde teslim kanalı taşması var — {summary}"
    );
    assert!(
        missing.is_empty(),
        "tüketici drenaja dönünce her birim teslim edilmeli; eksik: {missing:?}. Düşen birimin \
         taze PN'li kopyası gelse bile `delivered_seen` onu bastırır (runtime.rs:2410-2411): \
         dedup teslim olmadan ilerlemiş — {summary}"
    );
}

// ── R2 → HOL: S1 adaptörü düzeyi ─────────────────────────────────────────────────────────────

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

type Node = ClientTransport<NoiseWireProtection>;

/// Sentetik test kimliği.
fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4B;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

/// İki kimliği bilen dizinle ürün Crypto kurulumu (TEST anahtarları
/// `kama_noise_wire::seam_mock::fixture_*`, GİZLİ DEĞİL); saat hiç ilerletilmez (yenileme yok).
fn crypto(me: u8) -> NoiseWireProtection {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    let (protection, _clock) = fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy);
    protection
}

/// Ürün builder'ı + ürün veri profili (`ClientTransportConfig::product`). Yalnız alıcıda
/// `delivery_queue` küçültülür (bkz. modül belgesi "ÜRÜN YOLU").
fn s1_node(me: u8, hub: &MemoryHub, delivery_queue: Option<usize>) -> Node {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    if let Some(q) = delivery_queue {
        cfg.runtime.delivery_queue = q;
    }
    ClientTransport::new(cfg, crypto(me), Box::new(hub.link(id(me)))).expect("ürün adaptörü")
}

/// Alice başlatır, Bob yanıtlar; `MemoryHub` rölesi elle sürülür (belirlenimli). Döner: (Alice'in
/// tutamacı, Bob'un tutamacı).
fn connect(alice: &mut Node, bob: &mut Node, hub: &MemoryHub) -> (SessionHandle, SessionHandle) {
    alice.add_peer_addr(id(BOB), bob.data_addr());
    bob.add_peer_addr(id(ALICE), alice.data_addr());
    let Ok(ConnectResult::Pending(_)) = alice.connect(id(BOB), None) else {
        panic!("bağlantı beklemede başlamalı");
    };
    assert_eq!(hub.relay(), 1, "röle 1: INIT → Bob, HANDLE_ASSIGN → Alice");
    alice.drive();
    bob.drive();
    assert_eq!(hub.relay(), 1, "röle 2: RESPONSE → Alice");
    alice.drive();
    let h_alice = alice
        .active_session_for_test(id(BOB))
        .expect("Alice'in oturumu Ready olmalı");
    let h_bob = bob
        .active_session_for_test(id(ALICE))
        .expect("Bob'un oturumu Ready olmalı");
    (h_alice, h_bob)
}

fn hol_payload(unit: u64) -> Vec<u8> {
    format!("p0-hol-{unit}").into_bytes()
}

fn hol_unit(unit: u64) -> PreparedUnit {
    PreparedUnit {
        bytes: hol_payload(unit),
        compression_policy: CompressionPolicy::Never,
    }
}

/// Teslimin hangi birim olduğu (içerikten); tanınmayan içerik → `None`.
fn hol_index(d: &Delivered) -> Option<u64> {
    std::str::from_utf8(&d.unit)
        .ok()?
        .strip_prefix("p0-hol-")?
        .parse()
        .ok()
}

/// `read()` 0'a inene ya da [`STALL_QUIET`] boyunca değişmeyene dek (üst sınır [`STALL_MAX`])
/// bekler; son değeri döner.
fn settle(read: impl Fn() -> usize) -> usize {
    let start = Instant::now();
    let mut last = read();
    let mut last_change = start;
    loop {
        if last == 0 || start.elapsed() >= STALL_MAX || last_change.elapsed() >= STALL_QUIET {
            return last;
        }
        std::thread::sleep(POLL);
        let now = read();
        if now != last {
            last = now;
            last_change = Instant::now();
        }
    }
}

/// [K4G-05c P0 · R2 → HOL] Kanal taşmasıyla düşen birimin ARKASINDAKİ teslimler beklememeli.
///
/// BULGU (doğrulama R2 ek sonucu; CEVAP 9 DUR-18): R2 ile düşen (ama ACK'lenmiş) birim bir daha
/// gelmez. S1 adaptörünün oturum başına `Reorder`'ı `next`'i o birimde sabit tutar; sonraki her
/// birim `held`'de bekler (transport.rs:382-399) ve boşluk ancak `held.len() > cap` olunca
/// atlanır — `cap` = `max_inflight_units` = 4096 (transport.rs:981, runtime.rs:248). Az
/// trafikli oturumda (metin) bu fiilen kalıcı takılmadır.
///
/// DÜZENEK: iki gerçek `ClientTransport` (ürün builder'ı, gerçek `NoiseWireProtection`, UDP
/// loopback). Evre 1: Bob'un uygulaması DURUR — Bob'a hiçbir S1 çağrısı yapılmaz (`recv` /
/// `poll_event` `drive` eder ve teslim kanalını boşaltırdı; `delivery_dropped` salt okunurdur,
/// sürmez); Bob'un veri alıcısı arka plan görevinde sürer. Alice `N1` birim yollar ve teyit akışı
/// durana dek beklenir. Evre 2: Bob döner (kanaldakileri okur), ardından Alice `M` YENİ
/// (takipçi) birim yollar; teslimler toplanır.
///
/// TABANDA BEKLENEN: KIRMIZI — Bob `0..SMALL_DELIVERY_QUEUE` birimlerini alır; `4..N1` kanal
/// taşmasında düşer (hepsi ACK'li); takipçiler `N1..N1+M` veri alıcısından geçer ama `Reorder`'da
/// `next = 4` arkasında bekler → hiçbiri teslim edilmez. İlk kırmızı iddia takipçilerin
/// teslimidir; ikincisi `0..N1+M`'in eksiksiz, sıralı, tam bir kez teslimidir.
#[test]
fn p0_r2_dropped_unit_blocks_followers() {
    const N1: u64 = 12;
    const M: u64 = 4;
    let hub = MemoryHub::new();
    let mut alice = s1_node(ALICE, &hub, None);
    let mut bob = s1_node(BOB, &hub, Some(SMALL_DELIVERY_QUEUE));
    let (h_alice, h_bob) = connect(&mut alice, &mut bob, &hub);

    // ── Evre 1: Bob'un uygulaması DURUR (Bob'a S1 çağrısı yok). ──
    for unit in 0..N1 {
        alice
            .send(h_alice, hol_unit(unit), DeliveryPolicy::Reliable)
            .expect("gönder");
    }
    let outstanding_while_stalled = settle(|| alice.outstanding_for_test());
    let dropped_while_stalled = bob.delivery_dropped();

    // ── Evre 2: Bob döner; ardından Alice takipçi birimleri yollar. ──
    let mut got: Vec<Delivered> = Vec::new();
    while let Some(d) = bob.recv() {
        got.push(d);
    }
    let before_followers = got.len();
    for unit in N1..N1 + M {
        alice
            .send(h_alice, hol_unit(unit), DeliveryPolicy::Reliable)
            .expect("gönder");
    }
    let start = Instant::now();
    let mut last_progress = start;
    loop {
        while let Some(d) = bob.recv() {
            got.push(d);
            last_progress = Instant::now();
        }
        if got.len() as u64 >= N1 + M
            || start.elapsed() >= DRAIN_MAX
            || (alice.outstanding_for_test() == 0 && last_progress.elapsed() >= DRAIN_QUIET)
        {
            break;
        }
        std::thread::sleep(POLL);
    }

    let units: Vec<Option<u64>> = got.iter().map(hol_index).collect();
    let wrong_session: Vec<usize> = got
        .iter()
        .enumerate()
        .filter(|(_, d)| d.session != h_bob)
        .map(|(i, _)| i)
        .collect();
    let followers: Vec<u64> = units
        .iter()
        .flatten()
        .copied()
        .filter(|u| *u >= N1)
        .collect();
    let want_followers: Vec<u64> = (N1..N1 + M).collect();
    let want: Vec<Option<u64>> = (0..N1 + M).map(Some).collect();
    let summary = format!(
        "durakta: Alice teyitsiz={outstanding_while_stalled}, Bob \
         delivery_dropped={dropped_while_stalled}; dönüşte takipçilerden önce \
         teslim={before_followers}; sonda: teslim sırası={units:?}, Alice teyitsiz={}, Bob \
         delivery_dropped={}",
        alice.outstanding_for_test(),
        bob.delivery_dropped()
    );
    assert!(
        wrong_session.is_empty(),
        "teslimler Bob'daki oturuma ({h_bob:?}) atfedilmeli; yanlış sıralar: {wrong_session:?} \
         — {summary}"
    );
    assert_eq!(
        followers, want_followers,
        "R2→HOL KUSURU: kanal taşmasıyla düşen birimlerin arkasındaki takipçiler teslim \
         edilmedi. Düşen birim ACK'lendiği için gönderen onu bir daha yollamaz; S1 adaptörünün \
         `Reorder`'ı `next`'i ilk düşen birimde sabit tutar ve sonraki her birimi `held`'de \
         bekletir (transport.rs:382-399); boşluk ancak `held.len() > cap` = \
         `max_inflight_units` = 4096 olunca atlanır (transport.rs:981, runtime.rs:248) — \
         {summary}"
    );
    assert_eq!(
        units,
        want,
        "Bob'un uygulaması dönünce 0..{} eksiksiz, sıralı ve tam bir kez teslim edilmeli (S1 §4 \
         sıralı Reliable teslim) — {summary}",
        N1 + M
    );
}
