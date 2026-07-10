//! quinn (0.11.11) baseline transport (§4.4.1 best-available QUIC). CC seçilebilir:
//! Cubic (default, production) / Bbr (`Experimental! Use at your own risk.` — quinn kaynağı).
//! Self-signed cert (bench-içi; client doğrulamayı atlar — yalnız benchmark, güvenlik değil).
//!
//! Ölçüm modeli: her mesaj = bi-stream; client payload yazar + `finish`, server 1-byte ack
//! döner; client ack'i deadline ile bekler → delivered latency (tek clock, container-sync yok).

use std::sync::Arc;
use std::time::{Duration, Instant};

use quinn::congestion::{BbrConfig, CubicConfig};
use quinn::{ClientConfig, Endpoint, ServerConfig, TransportConfig};
use tokio::time::timeout;

use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput};
use crate::transport::{BenchTransport, Cc, ClientCfg, ServerCfg};

/// quinn tabanlı QUIC baseline.
pub struct QuinnTransport;

const QUINN_VERSION: &str = "0.11.11";
const ALPN: &[u8] = b"kama-bench";

fn transport_config(cc: Cc) -> Arc<TransportConfig> {
    let mut t = TransportConfig::default();
    // max_idle_timeout default 30s < deadline (120s) → bağlantı 30s'de ÖLÜR, eğri kesilir
    // (workflow bulgusu #2). Ölçüm penceresini kapsayan cömert idle (her iki uç min alır).
    t.max_idle_timeout(Some(
        Duration::from_secs(300)
            .try_into()
            .expect("300s valid idle timeout"),
    ));
    match cc {
        Cc::Cubic => {
            t.congestion_controller_factory(Arc::new(CubicConfig::default()));
        }
        Cc::Bbr => {
            t.congestion_controller_factory(Arc::new(BbrConfig::default()));
        }
    }
    Arc::new(t)
}

/// rustls default crypto provider (ring) kur — idempotent. rustls 0.23 config builder'ları
/// process-default provider ister; yoksa RUNTIME PANIC (derleme YAKALAMAZ). Bench'in
/// client/server yolları bunu gerektirir; loopback testi kanıtlar.
fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn server_config(cc: Cc) -> Result<ServerConfig, BenchError> {
    ensure_crypto_provider();
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .map_err(|e| BenchError::Tls(e.to_string()))?;
    let cert_der = cert.cert.der().clone();
    let key_der = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into();
    // ALPN client ile EŞLEŞMELİ (workflow bulgusu #7: server ALPN yoksa TÜM handshake fail → 0 veri).
    let mut crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .map_err(|e| BenchError::Tls(e.to_string()))?;
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let quic = quinn::crypto::rustls::QuicServerConfig::try_from(crypto)
        .map_err(|e| BenchError::Tls(e.to_string()))?;
    let mut sc = ServerConfig::with_crypto(Arc::new(quic));
    sc.transport_config(transport_config(cc));
    Ok(sc)
}

fn client_config(cc: Cc) -> Result<ClientConfig, BenchError> {
    ensure_crypto_provider();
    let mut crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipVerify))
        .with_no_client_auth();
    crypto.alpn_protocols = vec![ALPN.to_vec()];
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
        .map_err(|e| BenchError::Tls(e.to_string()))?;
    let mut cfg = ClientConfig::new(Arc::new(quic));
    cfg.transport_config(transport_config(cc));
    Ok(cfg)
}

impl BenchTransport for QuinnTransport {
    async fn run_server(&self, cfg: ServerCfg) -> Result<(), BenchError> {
        let sc = server_config(cfg.cc)?;
        let endpoint =
            Endpoint::server(sc, cfg.bind).map_err(|e| BenchError::Quic(e.to_string()))?;
        run_server_loop(endpoint).await;
        Ok(())
    }

    async fn run_client(&self, cfg: ClientCfg) -> Result<RunOutput, BenchError> {
        let mut endpoint = Endpoint::client("[::]:0".parse().expect("valid addr"))
            .map_err(|e| BenchError::Quic(e.to_string()))?;
        endpoint.set_default_client_config(client_config(cfg.cc)?);
        let conn = endpoint
            .connect(cfg.server, "localhost")
            .map_err(|e| BenchError::Quic(e.to_string()))?
            .await
            .map_err(|e| BenchError::Quic(e.to_string()))?;

        // Mesajlar EŞZAMANLI gönderilir: timeout'lu bir mesaj sonrakileri BLOKLAMAZ (aksi halde
        // %80 loss'ta koşu saatlerce sürerdi + sent_ns şişerdi). sent_ns/delivered_ns MUTLAK (start).
        let start = Instant::now();
        let payload = Arc::new(vec![
            0x5Au8;
            usize::try_from(cfg.payload_bytes).unwrap_or(usize::MAX)
        ]);
        let mut handles = Vec::with_capacity(usize::try_from(cfg.msg_count).unwrap_or(0));
        for msg_id in 0..cfg.msg_count {
            let conn = conn.clone();
            let payload = Arc::clone(&payload);
            let deadline = cfg.max_deadline;
            let payload_bytes = cfg.payload_bytes;
            handles.push(tokio::spawn(async move {
                let sent_ns = start.elapsed().as_nanos() as u64;
                let delivered_ns = send_one(&conn, &payload, deadline, start).await;
                RawEvent {
                    msg_id,
                    sent_ns,
                    delivered_ns,
                    payload_bytes,
                }
            }));
        }
        let mut events = Vec::with_capacity(handles.len());
        for h in handles {
            events.push(h.await.map_err(|e| BenchError::Quic(e.to_string()))?);
        }
        events.sort_by_key(|e| e.msg_id);

        let stats = conn.stats();
        conn.close(0u32.into(), b"done");
        endpoint.wait_idle().await;

        Ok(RunOutput {
            meta: RunMeta {
                profile: cfg.profile,
                cc: cfg.cc.label().to_string(),
                transport: "quinn".to_string(),
                quinn_version: QUINN_VERSION.to_string(),
                seed: cfg.seed,
                repeat: cfg.repeat,
                msg_count: cfg.msg_count,
                payload_bytes: cfg.payload_bytes,
                max_deadline_ns: cfg.max_deadline.as_nanos() as u64,
                udp_tx_bytes: stats.udp_tx.bytes,
                udp_rx_bytes: stats.udp_rx.bytes,
            },
            events,
        })
    }
}

/// Server accept döngüsü (test edilebilirlik için ayrıldı): her bağlantı ayrı task,
/// bi-stream oku → TAM okunursa 1-byte ack.
async fn run_server_loop(endpoint: Endpoint) {
    while let Some(incoming) = endpoint.accept().await {
        let Ok(conn) = incoming.await else { continue };
        tokio::spawn(async move {
            // Her STREAM ayrı task → HoL blocking YOK (workflow bulgusu #1/#3: bağlantı-başı
            // tek task + sıralı read_to_end, stalled stream sonrakilerin ack'ini bloklardı).
            while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                tokio::spawn(async move {
                    // Yalnız mesaj TAM okunursa ack (read hatası/aşımı → teslim SAYILMAZ).
                    if recv.read_to_end(1 << 20).await.is_ok() {
                        let _ = send.write_all(&[1u8]).await;
                        let _ = send.finish();
                    }
                });
            }
        });
    }
}

/// Tek mesaj: bi-stream aç, payload yaz + finish, ack'i `deadline` ile bekle. Ack gelirse
/// `start`'a göre **MUTLAK teslim anını** (ns) döner (sent_ns ile AYNI frame — metrics doğru
/// latency hesaplar); timeout/hata → `None` (teslim edilmedi).
async fn send_one(
    conn: &quinn::Connection,
    payload: &[u8],
    deadline: Duration,
    start: Instant,
) -> Option<u64> {
    let attempt = async {
        let (mut send, mut recv) = conn.open_bi().await.ok()?;
        send.write_all(payload).await.ok()?;
        send.finish().ok()?;
        let mut ack = [0u8; 1];
        recv.read_exact(&mut ack).await.ok()?;
        Some(start.elapsed().as_nanos() as u64)
    };
    timeout(deadline, attempt).await.ok().flatten()
}

/// Self-signed cert doğrulamasını ATLAYAN verifier — YALNIZ bench (güvenlik değil).
#[derive(Debug)]
struct SkipVerify;

impl rustls::client::danger::ServerCertVerifier for SkipVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod loopback_tests {
    use super::*;

    // Ölçüm aracının FRAME doğruluğunu KODA gömen guard (delivered_ns bug sınıfı + ALPN + provider).
    // 127.0.0.1 quinn server+client, netem YOK, port 0 (OS atar). Buggy delivered_ns (latency)
    // burada FAIL eder (d > sent_ns yanlış + completion(0ns)≈1); ALPN/provider eksikse handshake fail
    // → run_client Err → test fail. Golden-vector felsefesinin ARACIN kendisine uygulanması.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loopback_frame_invariants() {
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = Endpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("server bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(run_server_loop(server_ep));

        let out = QuinnTransport
            .run_client(ClientCfg {
                server: addr,
                msg_count: 10,
                payload_bytes: 128,
                max_deadline: Duration::from_secs(30),
                cc: Cc::Cubic,
                seed: 0,
                repeat: 0,
                profile: "loopback".to_string(),
            })
            .await
            .expect("client run (ALPN/provider/handshake OK)");

        assert_eq!(out.events.len(), 10);
        // loopback (kayıp yok) → hepsi teslim
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_some()),
            "loopback: tüm mesajlar teslim edilmeli"
        );
        // FRAME: delivered_ns MUTLAK (sent'ten SONRA) → latency > 0 (buggy: latency 0'a saturate olurdu)
        for e in &out.events {
            let d = e.delivered_ns.expect("delivered");
            assert!(
                d > e.sent_ns,
                "delivered_ns ({d}) mutlak + sent_ns ({}) sonrası olmalı",
                e.sent_ns
            );
        }
        // sent_ns eşzamanlı-send penceresinde (buggy sıralı+timeout halde dakikalara şişerdi)
        assert!(
            out.events
                .iter()
                .all(|e| e.sent_ns < Duration::from_secs(5).as_nanos() as u64),
            "sent_ns eşzamanlı send penceresinde (şişme yok)"
        );
        // completion EĞRİSİ ayrışır: 0ns'de hiçbiri (tüm latency>0), 30s'de hepsi.
        // buggy kodda latency≈0 → completion(0ns)≈1 → bu assert FAIL ederdi (bug catch).
        let m = crate::metrics::run_metrics(&out, &[Duration::ZERO, Duration::from_secs(30)]);
        assert_eq!(
            m.per_deadline[0].completion, 0.0,
            "0ns: hiçbiri (latency>0)"
        );
        assert_eq!(m.per_deadline[1].completion, 1.0, "30s: hepsi");
    }
}
