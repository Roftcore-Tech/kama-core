//! [INV-ACK-DELIVER-1 · F4] "Teslim yoluna kabul edilmeyen birim ACK'lenmez" — dolu teslim
//! kanalı için. Gerçek UDP loopback; teslim kanalı küçük (`delivery_queue = 4`); tüketici önce
//! DURUR (kanal dolar, gönderici göndermeye devam eder), sonra açılır. İddia: gönderilen küme ==
//! uygulamaya teslim edilen küme (hiçbir ACK'lenmiş birim kaybolmaz), sıra doğru, `delivery_dropped`
//! 0 (düşme yerine motora verilmeme: `data_not_admitted_full > 0`), bellek sınırlı (kanal 4).
//!
//! Önceki davranış (main 6716eb8d): datagram motora girer → ACK → `push_delivery` dolu kanalda
//! düşer → gönderici birimi bir daha göndermez → birim kalıcı kaybolur; `delivery_dropped > 0`.

use std::time::Duration;

use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint, TransportEngine};
use client_core::seam::IdentitySeam;

const QUEUE: usize = 4;

async fn stalled_consumer(engine: TransportEngine, n: u64) {
    let cfg = |e| RuntimeConfig {
        engine: e,
        delivery_queue: QUEUE,
        ..RuntimeConfig::default()
    };
    let mut server = ServerEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        cfg(engine),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("server bind");
    let mut client = ClientEndpoint::connect(
        server.local_addr(),
        cfg(engine),
        Box::new(IdentitySeam::new()),
    )
    .await
    .expect("client connect");

    for i in 0..n {
        client
            .send(format!("admit-unit-{i}").into_bytes())
            .expect("offer");
    }

    // Tüketici 1,5 s durur: kanal (4) dolar; gönderici bu sırada göndermeye devam eder (loopback,
    // kayıpsız, RTT ~0). Eski kodda bu pencerede gelen birimler ACK'lenip düşerdi.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert!(
        server.delivery_dropped() == 0,
        "dolu kanalda düşme OLMAMALI (motora verilmeme bekleniyor)"
    );

    let delivered = tokio::time::timeout(Duration::from_secs(60), async {
        let mut got: Vec<u64> = Vec::new();
        while (got.len() as u64) < n {
            let Some(d) = server.next_delivery().await else {
                break;
            };
            assert_eq!(d.payload, format!("admit-unit-{}", d.unit_id).into_bytes());
            got.push(d.unit_id);
        }
        got
    })
    .await;
    // [TEŞHİS] Zaman aşımında iki ucun istatistiği ve eksik birimler dökülür (CI'da seyrek kırmızı:
    // 2026-11-12 #193 a87e8cbf, eski motor kolu; zaman aşımı mesajı tek başına kök göstermiyordu).
    // Şüphe: tüketici dururken kabul edilmeyen birimler için RecoverFwd'nin tek-atımlık kopyaları
    // da reddedilirse birim motordan düşer (kopya kaybı yeniden kuyruklanmaz) — bu döküm onu ayırt
    // eder (`undelivered` > 0 ve gönderici uçuşu boş ↔ hâlâ yeniden iletiyor).
    let delivered = match delivered {
        Ok(d) => d,
        Err(_) => {
            let cs = client.shutdown().await;
            let ss = server.shutdown().await;
            panic!(
                "60 s içinde hepsi teslim edilmeli — eksik {} birim; istemci: {cs:?}; sunucu: {ss:?}",
                n.saturating_sub(ss.delivered)
            );
        }
    };
    let mut sorted = delivered.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len() as u64, n, "her birim tam bir kez teslim");
    assert_eq!(sorted, (0..n).collect::<Vec<_>>(), "küme = 0..n");

    let acked = tokio::time::timeout(Duration::from_secs(30), async {
        let mut acked = 0u64;
        while acked < n {
            if client.acked().await.is_some() {
                acked += 1;
            } else {
                break;
            }
        }
        acked
    })
    .await
    .expect("ACK'ler");
    assert_eq!(acked, n);

    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    assert_eq!(cs.undelivered, 0);
    assert_eq!(ss.delivered, n);
    assert_eq!(ss.delivery_dropped, 0, "ACK'lenmiş birim düşmedi");
    assert!(
        ss.data_not_admitted_full > 0,
        "kanal gerçekten doldu ve motora verilmeyen DATA sayıldı (ön koşul): {}",
        ss.data_not_admitted_full
    );
}

/// Port motoru (ürün varsayılanı), 64 birim, 4'lük teslim kanalı, 1,5 s duran tüketici.
#[tokio::test(flavor = "multi_thread")]
async fn cc_port_stalled_consumer_loses_no_acked_unit() {
    stalled_consumer(TransportEngine::CcPort, 64).await;
}

/// Eski motor (geri alma yolu) aynı düzenekte.
#[tokio::test(flavor = "multi_thread")]
async fn legacy_stalled_consumer_loses_no_acked_unit() {
    stalled_consumer(TransportEngine::Legacy, 64).await;
}
