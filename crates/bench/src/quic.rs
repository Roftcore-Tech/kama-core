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

fn server_config(cc: Cc) -> Result<ServerConfig, BenchError> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .map_err(|e| BenchError::Tls(e.to_string()))?;
    let cert_der = cert.cert.der().clone();
    let key_der = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into();
    let mut sc = ServerConfig::with_single_cert(vec![cert_der], key_der)
        .map_err(|e| BenchError::Tls(e.to_string()))?;
    sc.transport_config(transport_config(cc));
    Ok(sc)
}

fn client_config(cc: Cc) -> Result<ClientConfig, BenchError> {
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
        while let Some(incoming) = endpoint.accept().await {
            let Ok(conn) = incoming.await else { continue };
            // Her bağlantı ayrı task: bi-stream oku → 1-byte ack.
            tokio::spawn(async move {
                while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                    // Yalnız mesaj TAM okunursa ack (read hatası/aşımı → teslim SAYILMAZ).
                    if recv.read_to_end(1 << 20).await.is_ok() {
                        let _ = send.write_all(&[1u8]).await;
                        let _ = send.finish();
                    }
                }
            });
        }
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
