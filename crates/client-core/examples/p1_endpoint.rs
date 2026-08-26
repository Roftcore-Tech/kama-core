//! P1 kapı-2 sürücüsü — süreç-düzeyi e2e (item 8): gerçek iki süreç, gerçek UDP, netem altında.
//!
//! Kullanım:
//!   p1_endpoint server --bind A:P --expect N --deadline-s S
//!   p1_endpoint client --server A:P --units N --interval-ms I --deadline-s S
//!
//! Çıkış: 0 = hedef tam (server: N teslim; client: N ACK), 1 = eksik (sayılar stderr'e).
//! Ölçüm iddiası YOK — bu bir işlevsel kapıdır (teslim/kapanış/sınır), benchmark değildir.

use std::sync::Arc;
use std::time::Duration;

use client_core::runtime::{ClientEndpoint, RuntimeConfig, ServerEndpoint};
use client_core::seam::IdentitySeam;

fn arg(a: &[String], k: &str, d: &str) -> String {
    a.iter()
        .position(|x| x == k)
        .and_then(|i| a.get(i + 1))
        .cloned()
        .unwrap_or_else(|| d.to_string())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let role = a.get(1).cloned().unwrap_or_default();
    let deadline = Duration::from_secs(arg(&a, "--deadline-s", "60").parse().unwrap_or(60));
    let seam = Arc::new(IdentitySeam);
    let code = match role.as_str() {
        "server" => {
            let bind = arg(&a, "--bind", "127.0.0.1:4600").parse().expect("bind");
            let expect: u64 = arg(&a, "--expect", "100").parse().expect("expect");
            let mut srv = ServerEndpoint::bind(bind, RuntimeConfig::default(), seam)
                .await
                .expect("server bind");
            let linger = Duration::from_secs(arg(&a, "--linger-s", "5").parse().unwrap_or(5));
            let mut delivered = 0u64;
            let _ = tokio::time::timeout(deadline, async {
                while delivered < expect {
                    if srv.next_delivery().await.is_some() {
                        delivered += 1;
                    }
                }
            })
            .await;
            // Linger: hedef doldu diye HEMEN çıkma — client'ın kuyruk-ACK'leri/retransmit-teyitleri
            // için ACK-servisini sessizlik-penceresi kadar sürdür (ilk koşunun RED dersi: erken çıkan
            // server, client'ın son birimlerinin ACK yolunu öldürüyordu).
            tokio::time::sleep(linger).await;
            let stats = srv.shutdown().await;
            eprintln!("P1-SERVER delivered={delivered}/{expect} stats={stats:?}");
            u8::from(delivered < expect)
        }
        "client" => {
            let server = arg(&a, "--server", "127.0.0.1:4600")
                .parse()
                .expect("server");
            let units: u64 = arg(&a, "--units", "100").parse().expect("units");
            let interval =
                Duration::from_millis(arg(&a, "--interval-ms", "50").parse().unwrap_or(50));
            let mut cli = ClientEndpoint::connect(server, RuntimeConfig::default(), seam)
                .await
                .expect("client connect");
            let mut acked = 0u64;
            let _ = tokio::time::timeout(deadline, async {
                for i in 0..units {
                    // Offer bounded: dolu kuyrukta kısa bekle-yeniden-dene (app-tarafı sözleşme).
                    let payload = format!("p1-gate-unit-{i}").into_bytes();
                    let mut p = payload;
                    loop {
                        match cli.send(std::mem::take(&mut p)) {
                            Ok(()) => break,
                            Err(client_core::runtime::RuntimeError::QueueFull) => {
                                // payload taşındı; yeniden kur (take boşalttı).
                                p = format!("p1-gate-unit-{i}").into_bytes();
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                            Err(e) => panic!("offer: {e}"),
                        }
                    }
                    tokio::time::sleep(interval).await;
                }
                while acked < units {
                    if cli.acked().await.is_some() {
                        acked += 1;
                    }
                }
            })
            .await;
            let stats = cli.shutdown().await;
            eprintln!("P1-CLIENT acked={acked}/{units} stats={stats:?}");
            u8::from(acked < units)
        }
        _ => {
            eprintln!("kullanım: p1_endpoint server|client [--bind|--server A:P] [--units N] ...");
            2
        }
    };
    std::process::exit(i32::from(code));
}
