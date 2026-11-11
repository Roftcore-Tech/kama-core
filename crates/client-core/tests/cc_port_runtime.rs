//! [CC-PORT] Ürün runtime'ı port motoruyla (`RuntimeConfig::engine = CcPort`): gerçek UDP loopback,
//! N birim → hepsi teslim + ACK. `runtime_e2e::loopback_all_units_delivered_and_acked`'ın port
//! eşi; ürün varsayılanı (`Legacy`) o testte olduğu gibi kalır.

use std::time::Duration;

use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint, TransportEngine};
use client_core::seam::IdentitySeam;

fn cfg(engine: TransportEngine) -> RuntimeConfig {
    RuntimeConfig {
        engine,
        ..RuntimeConfig::default()
    }
}

async fn loopback(engine: TransportEngine, n: u64) {
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
            .send(format!("cc-port-unit-{i}").into_bytes())
            .expect("offer");
    }

    let all = tokio::time::timeout(Duration::from_secs(30), async {
        let mut delivered = 0u64;
        let mut acked = 0u64;
        while delivered < n || acked < n {
            tokio::select! {
                Some(d) = server.next_delivery(), if delivered < n => {
                    assert_eq!(d.payload, format!("cc-port-unit-{}", d.unit_id).into_bytes());
                    delivered += 1;
                }
                Some(_unit) = client.acked(), if acked < n => { acked += 1; }
            }
        }
        (delivered, acked)
    })
    .await
    .expect("30 s içinde tamamlanmalı (loopback, kayıpsız)");
    assert_eq!(all, (n, n));

    let cs = client.shutdown().await;
    let ss = server.shutdown().await;
    assert_eq!(cs.offered, n);
    assert_eq!(cs.acked, n);
    assert_eq!(cs.undelivered, 0);
    assert_eq!(cs.open_failures, 0);
    assert_eq!(ss.delivered, n);
    assert_eq!(ss.delivery_dropped, 0);
}

/// Port motoru ürün runtime'ında: 200 birim loopback, hepsi teslim + ACK.
#[tokio::test(flavor = "multi_thread")]
async fn cc_port_loopback_all_units_delivered_and_acked() {
    loopback(TransportEngine::CcPort, 200).await;
}

/// Varsayılan motor aynı harness'ta (regresyon çapası; `runtime_e2e` ile çift).
#[tokio::test(flavor = "multi_thread")]
async fn legacy_loopback_all_units_delivered_and_acked() {
    loopback(TransportEngine::Legacy, 200).await;
}

/// `TransportEngine::parse` sözleşmesi (kama-node `KAMA_TRANSPORT_ENGINE`, bench aynı).
#[test]
fn engine_parse_contract() {
    assert_eq!(TransportEngine::parse(""), Some(TransportEngine::Legacy));
    assert_eq!(
        TransportEngine::parse("legacy"),
        Some(TransportEngine::Legacy)
    );
    assert_eq!(
        TransportEngine::parse("cc-port"),
        Some(TransportEngine::CcPort)
    );
    assert_eq!(
        TransportEngine::parse(" cc_port "),
        Some(TransportEngine::CcPort)
    );
    assert_eq!(TransportEngine::parse("bbr"), None);
    assert_eq!(RuntimeConfig::default().engine, TransportEngine::Legacy);
}
