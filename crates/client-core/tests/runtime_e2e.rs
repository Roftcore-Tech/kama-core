//! P1 kapı-1 — in-process loopback e2e (gerçek UDP 127.0.0.1; tasarım D8/D9, item 8/9'un CI yarısı).
//!
//! netem süreç-düzeyi kapısı (`scripts/netem/p1_e2e_gate.sh`) bunun iki-netns/kayıplı ikizidir;
//! buradaki testler kayıpsız yolda TAM teslim + typed sınırlar + temiz kapanışı mühürler.

use std::sync::Arc;
use std::time::Duration;

use client_core::runtime::{ClientEndpoint, RuntimeConfig, RuntimeError, ServerEndpoint};
use client_core::seam::IdentitySeam;

/// N birim → hepsi teslim (payload byte-eş) + hepsi ACK; kapanış drain'i temiz; sayaçlar tutarlı.
#[tokio::test(flavor = "multi_thread")]
async fn loopback_all_units_delivered_and_acked() {
    const N: u64 = 50;
    let seam = Arc::new(IdentitySeam);
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        RuntimeConfig::default(),
        seam.clone(),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(server.local_addr(), RuntimeConfig::default(), seam)
        .await
        .expect("client connect");

    for i in 0..N {
        // Payload birim-özgü: teslim byte-eşliği içerikle doğrulanır (sabit-doku maskelemesin).
        client
            .send(format!("kama-p1-unit-{i}").into_bytes())
            .expect("offer");
    }

    let all = tokio::time::timeout(Duration::from_secs(30), async {
        let mut delivered = 0u64;
        let mut acked = 0u64;
        while delivered < N || acked < N {
            tokio::select! {
                Some(d) = server.next_delivery(), if delivered < N => {
                    assert_eq!(d.payload, format!("kama-p1-unit-{}", d.unit_id).into_bytes(),
                        "payload byte-eş olmalı (unit {})", d.unit_id);
                    delivered += 1;
                }
                Some(_unit) = client.acked(), if acked < N => { acked += 1; }
            }
        }
        (delivered, acked)
    })
    .await
    .expect("30 s içinde tamamlanmalı (loopback, kayıpsız)");
    assert_eq!(all, (N, N));

    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    assert_eq!(cs.offered, N);
    assert_eq!(cs.acked, N);
    assert_eq!(cs.undelivered, 0, "kayıpsız yolda residue olmamalı");
    assert_eq!(cs.open_failures, 0);
    assert_eq!(ss.delivered, N);
    assert_eq!(ss.delivery_dropped, 0);
    assert_eq!(ss.dropped_new_peers, 0);
}

/// Bounded offer: ölü hedefe küçük kuyruk+store → typed `QueueFull` (bloklamaz, panik yok).
#[tokio::test(flavor = "multi_thread")]
async fn offer_backpressure_is_typed_queue_full() {
    let cfg = RuntimeConfig {
        offer_queue: 1,
        max_inflight_units: 1,
        drain_cap: Duration::from_millis(50),
        ..RuntimeConfig::default()
    };
    // Ölü hedef (bağlı ama dinleyen yok): ACK gelmez → store boşalmaz → kuyruk dolar.
    let client =
        ClientEndpoint::connect("127.0.0.1:9".parse().unwrap(), cfg, Arc::new(IdentitySeam))
            .await
            .expect("connect");
    client.send(b"a".to_vec()).expect("1. birim store'a");
    // Görev 1. birimi store'a alsın (store cap=1 dolu kalır; ACK yok).
    tokio::time::sleep(Duration::from_millis(100)).await;
    client
        .send(b"b".to_vec())
        .expect("2. birim kuyruğa (cap=1)");
    let err = client
        .send(b"c".to_vec())
        .expect_err("3. birim typed reddedilmeli");
    assert!(
        matches!(err, RuntimeError::QueueFull),
        "beklenen QueueFull, gelen: {err:?}"
    );
    let stats = client.shutdown().await; // drain cap'li → ölü hedefte bile sonlanır (D8).
    assert!(stats.undelivered >= 1, "ölü hedefte residue kalmalı");
}

/// Bounds-before-alloc: birim tavanı aşımı SUNUMDA typed reddedilir (tahsis yok).
#[tokio::test(flavor = "multi_thread")]
async fn oversized_unit_is_typed_refused() {
    let client = ClientEndpoint::connect(
        "127.0.0.1:9".parse().unwrap(),
        RuntimeConfig {
            max_unit_bytes: 64,
            drain_cap: Duration::from_millis(50),
            ..RuntimeConfig::default()
        },
        Arc::new(IdentitySeam),
    )
    .await
    .expect("connect");
    let err = client
        .send(vec![0u8; 65])
        .expect_err("tavan aşımı reddedilmeli");
    assert!(matches!(
        err,
        RuntimeError::UnitTooLarge { got: 65, cap: 64 }
    ));
    client.shutdown().await;
}

/// Peer-cap FAIL-CLOSED: tablo doluyken İKİNCİ eş sessizce düşer (typed sayaç), İLK eş etkilenmez.
#[tokio::test(flavor = "multi_thread")]
async fn peer_cap_is_fail_closed() {
    let seam = Arc::new(IdentitySeam);
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        RuntimeConfig {
            max_peers: 1,
            ..RuntimeConfig::default()
        },
        seam.clone(),
    )
    .await
    .expect("server bind");
    let addr = server.local_addr();
    let c1 = ClientEndpoint::connect(addr, RuntimeConfig::default(), seam.clone())
        .await
        .expect("c1");
    c1.send(b"first-peer".to_vec()).expect("offer");
    // İlk eş teslim edilene kadar bekle (slotu işgal etsin).
    tokio::time::timeout(Duration::from_secs(10), server.next_delivery())
        .await
        .expect("c1 teslim")
        .expect("delivery");
    let c2 = ClientEndpoint::connect(
        addr,
        RuntimeConfig {
            drain_cap: Duration::from_millis(200),
            ..RuntimeConfig::default()
        },
        seam,
    )
    .await
    .expect("c2");
    c2.send(b"second-peer".to_vec()).expect("offer");
    tokio::time::sleep(Duration::from_millis(500)).await; // c2 datagram'ları düşmeli.
    let c2s = c2.shutdown().await;
    assert!(
        c2s.acked == 0 && c2s.undelivered == 1,
        "cap'li tabloda 2. eş ACK alamamalı: {c2s:?}"
    );
    c1.shutdown().await;
    let ss = server.shutdown().await;
    assert!(
        ss.dropped_new_peers > 0,
        "yeni-peer reddi sayaçla görünmeli"
    );
    assert_eq!(ss.delivered, 1);
}

/// Item 9 — ERROR yolu: server ORTA-KOŞUDA ölür; client asılı kalmaz, drain cap'lidir, residue
/// typed görünür (bench "dead peer → bounded exit" disiplini, tasarım D8/D9).
#[tokio::test(flavor = "multi_thread")]
async fn client_survives_server_death_mid_run() {
    let seam = Arc::new(IdentitySeam);
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        RuntimeConfig::default(),
        seam.clone(),
    )
    .await
    .expect("server bind");
    let addr = server.local_addr();
    let client = ClientEndpoint::connect(
        addr,
        RuntimeConfig {
            drain_cap: Duration::from_millis(500),
            ..RuntimeConfig::default()
        },
        seam,
    )
    .await
    .expect("client connect");
    for i in 0..10u64 {
        client
            .send(format!("pre-death-{i}").into_bytes())
            .expect("offer");
    }
    // İlk teslimler aksın, sonra server ÖL (orta-koşu ölüm).
    tokio::time::timeout(Duration::from_secs(10), server.next_delivery())
        .await
        .expect("ilk teslim")
        .expect("delivery");
    let _ = server.shutdown().await;
    for i in 10..20u64 {
        client
            .send(format!("post-death-{i}").into_bytes())
            .expect("offer (ölü hedefe de kabul)");
    }
    // Kapanış SINIRLI sürede dönmeli (drain_cap 500 ms + pay); asılı kalma = başarısızlık.
    let t0 = std::time::Instant::now();
    let stats = tokio::time::timeout(Duration::from_secs(5), client.shutdown())
        .await
        .expect("shutdown drain cap'li olmalı — asılı kaldı");
    assert!(
        t0.elapsed() < Duration::from_secs(3),
        "drain cap aşıldı: {:?}",
        t0.elapsed()
    );
    assert_eq!(
        stats.offered + stats.unaccepted_offers,
        20,
        "kabul edilen her offer ya işlendi ya unaccepted sayacında: {stats:?}"
    );
    assert!(
        stats.undelivered > 0,
        "ölü server sonrası residue typed görünmeli: {stats:?}"
    );
}

/// Item 9 — teslim-kanalı taşması: app drenaj yapmazken bellek BÜYÜMEZ, taşan teslim SAYILIR,
/// app sonradan drenaja dönebilir (kanal bozulmaz).
#[tokio::test(flavor = "multi_thread")]
async fn delivery_overflow_is_counted_not_grown() {
    let seam = Arc::new(IdentitySeam);
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        RuntimeConfig {
            delivery_queue: 4,
            ..RuntimeConfig::default()
        },
        seam.clone(),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(server.local_addr(), RuntimeConfig::default(), seam)
        .await
        .expect("client connect");
    for i in 0..40u64 {
        client
            .send(format!("overflow-{i}").into_bytes())
            .expect("offer");
    }
    // App teslimleri BİLEREK drenaj etmiyor; birimlerin ACK'lenmesini bekle (teslim ≠ drenaj).
    let mut acked = 0u64;
    let _ = tokio::time::timeout(Duration::from_secs(20), async {
        while acked < 40 {
            if client.acked().await.is_some() {
                acked += 1;
            }
        }
    })
    .await;
    assert_eq!(acked, 40, "teslim-kanalı taşması ACK yolunu ETKİLEMEMELİ");
    // Şimdi drenaja dön: kanal hâlâ işler (ilk 4 bekliyor).
    let d = tokio::time::timeout(Duration::from_secs(5), server.next_delivery())
        .await
        .expect("drenaj dönünce teslim akmalı")
        .expect("delivery");
    assert!(d.payload.starts_with(b"overflow-"));
    client.shutdown().await;
    let ss = server.shutdown().await;
    assert!(
        ss.delivery_dropped > 0,
        "taşan teslim sayaçla görünmeli: {ss:?}"
    );
    assert!(
        ss.delivered + ss.delivery_dropped >= 40,
        "teslim + taşan ≥ 40 olmalı: {ss:?}"
    );
}

/// Item 3 — idle-peer emekliliği: kısa idle_hold ile eş emekli edilir (sayaç), tablo boşalır ve
/// AYNI eş yeniden gelirse yeni slot açılır (yaşam-döngüsü çevrimi).
#[tokio::test(flavor = "multi_thread")]
async fn idle_peer_is_retired_and_slot_reusable() {
    let seam = Arc::new(IdentitySeam);
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        RuntimeConfig {
            idle_hold: Duration::from_millis(300),
            ..RuntimeConfig::default()
        },
        seam.clone(),
    )
    .await
    .expect("server bind");
    let client = ClientEndpoint::connect(server.local_addr(), RuntimeConfig::default(), seam)
        .await
        .expect("client connect");
    client.send(b"before-idle".to_vec()).expect("offer");
    tokio::time::timeout(Duration::from_secs(10), server.next_delivery())
        .await
        .expect("teslim")
        .expect("delivery");
    // idle_hold + patience-tick payı: emeklilik gerçekleşsin.
    tokio::time::sleep(Duration::from_millis(1600)).await;
    // Aynı eş yeniden trafik üretir → yeni slot (yeni Receiver) açılmalı ve teslim akmalı.
    client.send(b"after-idle".to_vec()).expect("offer 2");
    let d = tokio::time::timeout(Duration::from_secs(10), server.next_delivery())
        .await
        .expect("emeklilik sonrası teslim")
        .expect("delivery");
    assert_eq!(d.payload, b"after-idle".to_vec());
    client.shutdown().await;
    let ss = server.shutdown().await;
    assert!(
        ss.retired_idle_peers >= 1,
        "emeklilik sayaçla görünmeli: {ss:?}"
    );
}
