//! quinn (0.11.11) baseline transport (§4.4.1 best-available QUIC). CC seçilebilir:
//! Cubic (default, production) / Bbr (`Experimental! Use at your own risk.` — quinn kaynağı).
//! Self-signed cert (bench-içi; client doğrulamayı atlar — yalnız benchmark, güvenlik değil).
//!
//! Ölçüm modeli: her mesaj = bi-stream; client payload yazar + `finish`, server 1-byte ack
//! döner; client ack'i deadline ile bekler → delivered latency (tek clock, container-sync yok).

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use quinn::congestion::{BbrConfig, CubicConfig};
use quinn::{
    AsyncUdpSocket, ClientConfig, Endpoint, EndpointConfig, ServerConfig, TokioRuntime,
    TransportConfig, UdpPoller,
};
use tokio::time::timeout;

use crate::budget::{BudgetGovernor, BudgetMode, BudgetSnapshot, IPV4_UDP_OVERHEAD};
use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};
use crate::transport::{BenchTransport, Cc, ClientCfg, ServerCfg};

/// quinn tabanlı QUIC baseline.
pub struct QuinnTransport;

/// [KAPI-1 W2] Governor tutamağı: sarmalayıcı socket'le AYNI saat tabanını (epoch) paylaşır —
/// arm/permit aynı monotonic eksende akar (paced refill doğruluğu buna bağlı).
#[derive(Debug, Clone)]
pub(crate) struct GovHandle {
    gov: Arc<Mutex<BudgetGovernor>>,
    epoch: Instant,
}

impl GovHandle {
    pub(crate) fn now_ns(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    /// DÜZLEM-A t=0: bütçe uygulaması buradan başlar (öncesi = handshake, muaf + ayrı sayılır).
    pub(crate) fn arm_now(&self) {
        let now = self.now_ns();
        self.gov.lock().expect("governor lock").arm(now);
    }

    pub(crate) fn snapshot(&self) -> BudgetSnapshot {
        self.gov.lock().expect("governor lock").snapshot()
    }
}

/// [KAPI-1 W2] quinn `AsyncUdpSocket` sarmalayıcısı: `try_send` packet-atomic bütçe kapısından
/// geçer; ret = sessiz düşürme (`Ok(())`) = kanal-silinmesi eşdeğeri (ön-kayıtlı semantik —
/// io-hatası döndürmek quinn'i socket-arızası sanmasına iterdi). Governor bekletmez (pacing-nötr).
/// `max_transmit_segments()=1` GSO-batching'i kapatır → 1 transmit = 1 datagram = 1 IP paketi
/// (packet-atomic sayım bire bir; r5 H offload kuralıyla tutarlı).
#[derive(Debug)]
struct GovernedUdpSocket {
    inner: Arc<dyn AsyncUdpSocket>,
    handle: GovHandle,
}

impl AsyncUdpSocket for GovernedUdpSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.inner.clone().create_io_poller()
    }

    fn try_send(&self, transmit: &quinn::udp::Transmit) -> io::Result<()> {
        let now = self.handle.now_ns();
        // Savunmacı GSO yolu: max_transmit_segments()=1 bunu tekile indirger; yine de çok-segment
        // gelirse atomik (ya hepsi ya hiçbiri) sayılır — sessiz yanlış-sayım yerine korumalı yol.
        let (total_ip, pkts) = match transmit.segment_size {
            None => (transmit.contents.len() as u64 + IPV4_UDP_OVERHEAD, 1),
            Some(seg) => {
                let n = transmit.contents.len().div_ceil(seg.max(1)) as u64;
                (
                    transmit.contents.len() as u64 + n * IPV4_UDP_OVERHEAD,
                    n.max(1),
                )
            }
        };
        let granted = self
            .handle
            .gov
            .lock()
            .expect("governor lock")
            .permit_n(total_ip, pkts, now);
        if granted {
            self.inner.try_send(transmit)
        } else {
            Ok(())
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [io::IoSliceMut<'_>],
        meta: &mut [quinn::udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        self.inner.poll_recv(cx, bufs, meta)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn max_transmit_segments(&self) -> usize {
        1
    }

    fn max_receive_segments(&self) -> usize {
        self.inner.max_receive_segments()
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}

/// Bütçeli/bütçesiz endpoint kuruluşu. `Unlimited` → quinn'in kendi yolu AYNEN (banked yolun
/// sıfır-perturbasyonu); bütçeli → governed socket ile `new_with_abstract_socket`.
pub(crate) fn build_endpoint(
    bind: SocketAddr,
    server: Option<ServerConfig>,
    budget: BudgetMode,
) -> Result<(Endpoint, Option<GovHandle>), BenchError> {
    if budget == BudgetMode::Unlimited {
        let ep = match server {
            Some(sc) => Endpoint::server(sc, bind),
            None => Endpoint::client(bind),
        }
        .map_err(|e| BenchError::Quic(e.to_string()))?;
        return Ok((ep, None));
    }
    let sock = std::net::UdpSocket::bind(bind).map_err(|e| BenchError::Quic(e.to_string()))?;
    let inner = quinn::Runtime::wrap_udp_socket(&TokioRuntime, sock)
        .map_err(|e| BenchError::Quic(e.to_string()))?;
    let handle = GovHandle {
        gov: Arc::new(Mutex::new(BudgetGovernor::new(budget))),
        epoch: Instant::now(),
    };
    let governed: Arc<dyn AsyncUdpSocket> = Arc::new(GovernedUdpSocket {
        inner,
        handle: handle.clone(),
    });
    let ep = Endpoint::new_with_abstract_socket(
        EndpointConfig::default(),
        server,
        governed,
        Arc::new(TokioRuntime),
    )
    .map_err(|e| BenchError::Quic(e.to_string()))?;
    Ok((ep, Some(handle)))
}

pub(crate) const QUINN_VERSION: &str = "0.11.11";
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
    // [inceleme #8/#21/#23] Ön-kayıtlı profil-şeması pinleri (kapi_manifest full_schema):
    // pmtud_policy=off → MTU discovery kapalı; max_udp_payload=1452 → initial_mtu pini;
    // message_coalescing=off → segmentation-offload kapalı (Unlimited/sarmalanmamış yolda da);
    // warmup 10 bi-stream kredi tüketip burst-100 ölçümünü KISITLAMASIN → kredi 512.
    t.mtu_discovery_config(None);
    t.initial_mtu(1452);
    t.enable_segmentation_offload(false);
    t.max_concurrent_bidi_streams(512u32.into());
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

pub(crate) fn server_config(cc: Cc) -> Result<ServerConfig, BenchError> {
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

pub(crate) fn client_config(cc: Cc) -> Result<ClientConfig, BenchError> {
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
        let (endpoint, gov) = build_endpoint(cfg.bind, Some(sc), cfg.budget)?;
        spawn_gate_watch(gov.clone(), cfg.gate_watch.clone());
        run_server_loop(endpoint, gov, cfg.gate_watch.is_some()).await;
        Ok(())
    }

    async fn run_client(&self, cfg: ClientCfg) -> Result<RunOutput, BenchError> {
        let (mut endpoint, gov) =
            build_endpoint("[::]:0".parse().expect("valid addr"), None, cfg.budget)?;
        endpoint.set_default_client_config(client_config(cfg.cc)?);
        // Handshake'i max_deadline ile BOUNDED yap. Severe loss'ta handshake dakikalarca sürebilir;
        // tek sınır max_idle_timeout (300s) olurdu → koşu HANG eder (smoke'ta r1 %80 loss'ta 2m17s
        // asılı kaldı). deadline içinde bağlanamıyorsa hiçbir mesaj o deadline'da teslim edilemezdi
        // → koşu 0 completion. GEÇERLİ ölçüm sonucu (§4.4.3 "handshake success" metriği), HATA değil
        // → agregaya dahil (dışlanırsa QUIC completion'ı şişer). connect() Result'ı (ConnectError,
        // örn. bozuk adres) hâlâ gerçek hata → ? ile yayılır; yalnız handshake fail/timeout 0-completion.
        let connecting = endpoint
            .connect(cfg.server, "localhost")
            .map_err(|e| BenchError::Quic(e.to_string()))?;
        let conn = match timeout(cfg.max_deadline, connecting).await {
            Ok(Ok(c)) => c,
            Ok(Err(_handshake_err)) => return Ok(handshake_failed_output(&cfg, gov.as_ref())),
            Err(_timed_out) => return Ok(handshake_failed_output(&cfg, gov.as_ref())),
        };
        // [W6] Kuruluş-gate'i: handshake TEMİZ ağda tamam (DÜZLEM-A); harness şimdi netem'i
        // uygular (A2) — warmup impaired koşar.
        if let Some(g) = &cfg.establish_gate {
            crate::transport::gate_sync(g).await?;
        }
        // [KAPI-1 W6 + inceleme #0/#3] Warmup (ölçüm-DIŞI): aynı-sınıf PACED akış (kama ile aynı
        // sunum planı, i×interval), per-mesaj 10 s tavan; tavanda stream RESET edilir → residue
        // post-t0 retransmit ETMEZ (QUIC'in gerçek iptal semantiği; kama motoru birim terk edemez —
        // fark warmup_residue alanında görünür + KAPI-1 §3.2 amendment'ında belgeli).
        let mut warmup_residue: u64 = 0;
        if cfg.warmup_msgs > 0 {
            let wpayload = Arc::new(vec![
                0x5Au8;
                usize::try_from(cfg.payload_bytes).unwrap_or(usize::MAX)
            ]);
            let mut hs = Vec::new();
            for i in 0..cfg.warmup_msgs {
                let conn = conn.clone();
                let p = Arc::clone(&wpayload);
                let delay = cfg
                    .submit_interval
                    .saturating_mul(u32::try_from(i).unwrap_or(u32::MAX));
                hs.push(tokio::spawn(async move {
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    warmup_one(&conn, &p).await
                }));
            }
            for h in hs {
                if !h.await.unwrap_or(false) {
                    warmup_residue += 1;
                }
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        if let Some(g) = &cfg.gate {
            crate::transport::gate_sync(g).await?;
        }
        // Ölçüm-t0 (DÜZLEM-A): bütçe ARM (handshake+warmup pre_arm'da, muaf + ayrı sayılır);
        // wire sayaçları t0-fotoğrafından DELTA alınır (r5 D.2: handshake/warmup byte'ları
        // ölçüm-toplamına girmez; sayaç-reset-state-korunur, r5 B.2).
        if let Some(h) = &gov {
            h.arm_now();
        }
        let stats0 = conn.stats();
        let t0_unix_ns = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        )
        .unwrap_or(0);

        // Mesajlar EŞZAMANLI gönderilir: timeout'lu bir mesaj sonrakileri BLOKLAMAZ (aksi halde
        // %80 loss'ta koşu saatlerce sürerdi + sent_ns şişerdi). sent_ns/delivered_ns MUTLAK (start).
        //
        // TASARIM SAPMASI (bilinçli): sent_ns msg_id'ye göre MONOTON ARTMAZ. Task'lar yarışır →
        // dispatch sırası ≠ id sırası. Monoton yapmak gönderimi SERİLEŞTİRMEyi gerektirirdi = bu
        // eşzamanlılığı yok eder VE client-tarafı HoL'u geri getirir (server-tarafı #1/#3/#8'in
        // ikizi). Frame doğruluğu invariant'ı monotonluk DEĞİL: delivered_ns > sent_ns (mutlak,
        // aynı `start` saati) + tüm sent_ns'in dar eşzamanlı-dispatch penceresinde kalması. Loopback
        // testi tam da bunu assert eder (monoton değil, pencere) — "neden monoton değil" cevabı budur.
        let start = Instant::now();
        let payload = Arc::new(vec![
            0x5Au8;
            usize::try_from(cfg.payload_bytes).unwrap_or(usize::MAX)
        ]);
        let mut handles = Vec::with_capacity(usize::try_from(cfg.msg_count).unwrap_or(0));
        for msg_id in 0..cfg.msg_count {
            let conn = conn.clone();
            let payload = Arc::clone(&payload);
            // [inceleme #12] Gözlem penceresi koşu-sonuna dek (D + kalan-sunum-süresi) — kama/qdgram
            // gibi geç teslimler de KAYDEDİLİR (completion(D) yine yalnız latency≤D sayar; stale
            // teslim bilgisi üç kolda simetrik toplanır).
            let deadline = cfg.max_deadline.saturating_add(
                cfg.submit_interval
                    .saturating_mul(u32::try_from(cfg.msg_count - 1 - msg_id).unwrap_or(u32::MAX)),
            );
            let payload_bytes = cfg.payload_bytes;
            // [KAPI-1 W3] paced: mesaj i, t=i×interval'de sunulur (kama ile aynı semantik);
            // sent_ns sleep SONRASI damgalanır = sunum anı. ZERO → sleep yok (burst aynen).
            let submit_at = cfg
                .submit_interval
                .saturating_mul(u32::try_from(msg_id).unwrap_or(u32::MAX));
            handles.push(tokio::spawn(async move {
                if !submit_at.is_zero() {
                    tokio::time::sleep(submit_at).await;
                }
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
        // wait_idle'ı da BOUNDED yap: delivered_ns'leri zaten kaydettik; kayıp CONNECTION_CLOSE
        // altında tam drain'i beklemek koşuyu 300s idle'a kadar asabilir. 3s temiz kapanışa yeter.
        let _ = timeout(Duration::from_secs(3), endpoint.wait_idle()).await;

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
                udp_tx_bytes: stats.udp_tx.bytes.saturating_sub(stats0.udp_tx.bytes),
                udp_rx_bytes: stats.udp_rx.bytes.saturating_sub(stats0.udp_rx.bytes),
                tx: TxComponents::default(),
                kama_arm: String::new(),
                submit_interval_ns: u64::try_from(cfg.submit_interval.as_nanos())
                    .unwrap_or(u64::MAX),
                budget: gov.as_ref().map(GovHandle::snapshot).unwrap_or_default(),
                t0_unix_ns,
                events_epoch_offset_ns: 0, // event'ler ölçüm-t0 epoch'unda
                warmup_residue,
            },
            events,
        })
    }
}

/// Handshake başarısız/timeout → bağlanılamadı: `msg_count` mesajın TAMAMI teslim EDİLMEDİ
/// (completion 0). Bağlantı kurulmadığı için wire bytes 0. §4.4.3'te bu bir ölçüm sonucudur
/// (handshake fail), hata DEĞİL — koşu agregaya 0-completion olarak girer (QUIC'in gerçek
/// zayıflığını yansıtır; severe loss'ta bağlanamamak Kama'nın kazandığı yer).
fn handshake_failed_output(cfg: &ClientCfg, gov: Option<&GovHandle>) -> RunOutput {
    let events = (0..cfg.msg_count)
        .map(|msg_id| RawEvent {
            msg_id,
            sent_ns: 0,
            delivered_ns: None,
            payload_bytes: cfg.payload_bytes,
        })
        .collect();
    RunOutput {
        meta: RunMeta {
            profile: cfg.profile.clone(),
            cc: cfg.cc.label().to_string(),
            transport: "quinn".to_string(),
            quinn_version: QUINN_VERSION.to_string(),
            seed: cfg.seed,
            repeat: cfg.repeat,
            msg_count: cfg.msg_count,
            payload_bytes: cfg.payload_bytes,
            max_deadline_ns: cfg.max_deadline.as_nanos() as u64,
            udp_tx_bytes: 0,
            udp_rx_bytes: 0,
            tx: TxComponents::default(),
            kama_arm: String::new(),
            submit_interval_ns: u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX),
            budget: gov.map(GovHandle::snapshot).unwrap_or_default(),
            t0_unix_ns: 0,
            events_epoch_offset_ns: 0,
            warmup_residue: 0,
        },
        events,
    }
}

/// Server accept döngüsü (test edilebilirlik için ayrıldı): her bağlantı ayrı task,
/// bi-stream oku → TAM okunursa 1-byte ack.
/// [W6] gate-watch: `<g>.go` belirince reverse governor'ü arm eden izleme task'i (5 ms poll).
/// Warmup/handshake reverse-trafiği böylece pre_arm'da muaf kalır (r5 D.2, iki yönde de).
pub(crate) fn spawn_gate_watch(gov: Option<GovHandle>, gate: Option<std::path::PathBuf>) {
    if let (Some(h), Some(g)) = (gov, gate) {
        let go = format!("{}.go", g.display());
        tokio::spawn(async move {
            while !std::path::Path::new(&go).exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            h.arm_now();
        });
    }
}

async fn run_server_loop(endpoint: Endpoint, gov: Option<GovHandle>, gate_watched: bool) {
    // [KAPI-1 W2] reverse-yön bütçe raporu: server RunOutput üretmez → periyodik BUDGET-REV satırı.
    if let Some(h) = gov.clone() {
        tokio::spawn(async move {
            let mut t = tokio::time::interval(Duration::from_secs(10));
            loop {
                t.tick().await;
                let s = h.snapshot();
                eprintln!(
                    "BUDGET-REV(quinn) injected={}B/{}p refused={}B/{}p pre_arm={}B first_refusal_ns={}",
                    s.injected_bytes,
                    s.injected_pkts,
                    s.refused_bytes,
                    s.refused_pkts,
                    s.pre_arm_bytes,
                    s.first_refusal_ns
                );
            }
        });
    }
    while let Some(incoming) = endpoint.accept().await {
        let Ok(conn) = incoming.await else { continue };
        // [KAPI-1 W2+W6] reverse bütçe: gate-watch VARSA arm'ı izleme task'i yapar (ölçüm-t0);
        // yoksa eski kural (ilk established bağlantıda arm).
        if !gate_watched {
            if let Some(h) = &gov {
                h.arm_now();
            }
        }
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

/// [inceleme #0] Warmup mesajı: teslim → true; 10 s tavanında stream RESET edilir (post-t0
/// retransmit kalmaz) → false (residue). Ölçüm kayıtlarına hiçbir şey yazmaz.
async fn warmup_one(conn: &quinn::Connection, payload: &[u8]) -> bool {
    let Ok((mut send, mut recv)) = conn.open_bi().await else {
        return false;
    };
    let attempt = async {
        send.write_all(payload).await.ok()?;
        send.finish().ok()?;
        let mut ack = [0u8; 1];
        recv.read_exact(&mut ack).await.ok()?;
        Some(())
    };
    match timeout(Duration::from_secs(10), attempt).await {
        Ok(Some(())) => true,
        _ => {
            let _ = send.reset(0u32.into());
            false
        }
    }
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

    /// [KAPI-1 W2] governed socket pini: handshake bütçe-MUAF (pre_arm'da görünür, düşülmez);
    /// arm sonrası dar burst cap stream verisini enjekte ettirmez → teslim 0; ret sayaçları
    /// meta'da. Sessiz-düşürme yolu quinn'i öldürmemeli (bounded, Err değil).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn governed_budget_exempts_handshake_and_refuses_stream_data() {
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = Endpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("server bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(run_server_loop(server_ep, None, false));

        let out = QuinnTransport
            .run_client(ClientCfg {
                server: addr,
                msg_count: 3,
                payload_bytes: 128,
                max_deadline: Duration::from_secs(2),
                cc: Cc::Cubic,
                seed: 0,
                repeat: 0,
                profile: "loopback-budget".to_string(),
                kama_arm: crate::transport::KamaArm::Arq,
                submit_interval: Duration::ZERO,
                budget: crate::budget::BudgetMode::Burst { cap_bytes: 60 },
                warmup_msgs: 0,
                gate: None,
                establish_gate: None,
                // Bu kolda FEC yok (yalnız kama-product kolu okur).
                fec: crate::transport::FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_out: None,
                gate_permille: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                em_mode: None,
                em_min: None,
                em_full: None,
                spec_l2_permille: None,
                spec_l1_permille: None,
            })
            .await
            .expect("governed run bounded Ok");

        let b = &out.meta.budget;
        assert!(
            b.pre_arm_bytes > 0,
            "handshake byte'ları pre-arm'da görünmeli (DÜZLEM-A muafiyeti): {b:?}"
        );
        assert!(
            b.refused_pkts > 0,
            "60B cap post-handshake stream paketlerini reddetmeli: {b:?}"
        );
        assert!(b.injected_bytes <= 60, "hard-cap overshoot yok: {b:?}");
        let delivered = out
            .events
            .iter()
            .filter(|e| e.delivered_ns.is_some())
            .count();
        assert_eq!(delivered, 0, "veri enjekte edilemedi → teslim olamaz");
    }

    /// [KAPI-1 W6] İki-fazlı koşu: warmup stream'leri ölçüm kayıtlarına girmez; udp sayaçları
    /// t0-delta (handshake+warmup hariç, r5 D.2); gate protokolü işler; ölçüm 3/3 teslim.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn warmup_and_gate_two_phase_run() {
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = Endpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("server bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(run_server_loop(server_ep, None, false));

        let dir = std::env::temp_dir().join(format!("kapi-gate-q-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gate = dir.join("g1");
        let g2 = gate.clone();
        let harness = tokio::spawn(async move {
            let ready = format!("{}.ready", g2.display());
            for _ in 0..2000 {
                if std::path::Path::new(&ready).exists() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            std::fs::write(format!("{}.go", g2.display()), b"go").unwrap();
        });

        let out = QuinnTransport
            .run_client(ClientCfg {
                server: addr,
                msg_count: 3,
                payload_bytes: 128,
                max_deadline: Duration::from_secs(10),
                cc: Cc::Cubic,
                seed: 0,
                repeat: 0,
                profile: "loopback-gate".to_string(),
                kama_arm: crate::transport::KamaArm::Arq,
                submit_interval: Duration::ZERO,
                budget: crate::budget::BudgetMode::Unlimited,
                warmup_msgs: 2,
                gate: Some(gate.clone()),
                establish_gate: None,
                // Bu kolda FEC yok (yalnız kama-product kolu okur).
                fec: crate::transport::FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_out: None,
                gate_permille: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                em_mode: None,
                em_min: None,
                em_full: None,
                spec_l2_permille: None,
                spec_l1_permille: None,
            })
            .await
            .expect("client run");
        harness.await.unwrap();

        assert_eq!(out.events.len(), 3, "yalnız ölçüm mesajları");
        assert!(out.events.iter().all(|e| e.delivered_ns.is_some()));
        assert!(
            out.meta.udp_tx_bytes > 0 && out.meta.udp_tx_bytes < 10_000,
            "udp_tx t0-delta olmalı (handshake ~KB'ler hariç): {}",
            out.meta.udp_tx_bytes
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Ölçüm aracının FRAME doğruluğunu KODA gömen guard (delivered_ns bug sınıfı + ALPN + provider).
    // 127.0.0.1 quinn server+client, netem YOK, port 0 (OS atar). Buggy delivered_ns (latency)
    // burada FAIL eder (d > sent_ns yanlış + completion(0ns)≈1); ALPN/provider eksikse handshake fail
    // → run_client Err → test fail. Golden-vector felsefesinin ARACIN kendisine uygulanması.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn loopback_frame_invariants() {
        let sc = server_config(Cc::Cubic).expect("server config");
        let server_ep = Endpoint::server(sc, "127.0.0.1:0".parse().unwrap()).expect("server bind");
        let addr = server_ep.local_addr().expect("local addr");
        tokio::spawn(run_server_loop(server_ep, None, false));

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
                kama_arm: crate::transport::KamaArm::Arq,
                submit_interval: Duration::ZERO,
                budget: crate::budget::BudgetMode::Unlimited,
                warmup_msgs: 0,
                gate: None,
                establish_gate: None,
                // Bu kolda FEC yok (yalnız kama-product kolu okur).
                fec: crate::transport::FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_out: None,
                gate_permille: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                em_mode: None,
                em_min: None,
                em_full: None,
                spec_l2_permille: None,
                spec_l1_permille: None,
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
        // sent_ns eşzamanlı-send penceresinde (buggy sıralı+timeout halde dakikalara şişerdi).
        // NB: id'ye göre MONOTON assert ETMİYORUZ — eşzamanlı dispatch monotonluğu bozar (bkz.
        // run_client "TASARIM SAPMASI"). Pencere = "hepsi ~aynı anda gönderildi" kanıtı, niyet bu.
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

    // Handshake BOUNDED: ölü adrese (kimse QUIC konuşmuyor) bağlan → handshake tamamlanamaz.
    // Bounded değilse max_idle_timeout'a (300s) kadar HANG (smoke'taki r1 %80 loss bug'ı).
    // max_deadline=2s → run_client ~deadline'da dönmeli + 0 completion. Toplam run_client süresinin
    // sınırlı olduğunu (300s idle'a asılmadığını) kanıtlar.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn handshake_timeout_is_bounded() {
        let dead: std::net::SocketAddr = "127.0.0.1:9".parse().unwrap(); // discard portu; QUIC server YOK
        let t0 = Instant::now();
        let out = QuinnTransport
            .run_client(ClientCfg {
                server: dead,
                msg_count: 5,
                payload_bytes: 64,
                max_deadline: Duration::from_secs(2),
                cc: Cc::Cubic,
                seed: 0,
                repeat: 0,
                profile: "deadaddr".to_string(),
                kama_arm: crate::transport::KamaArm::Arq,
                submit_interval: Duration::ZERO,
                budget: crate::budget::BudgetMode::Unlimited,
                warmup_msgs: 0,
                gate: None,
                establish_gate: None,
                // Bu kolda FEC yok (yalnız kama-product kolu okur).
                fec: crate::transport::FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_out: None,
                gate_permille: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                em_mode: None,
                em_min: None,
                em_full: None,
                spec_l2_permille: None,
                spec_l1_permille: None,
            })
            .await
            .expect("bounded return, HANG değil");
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "handshake ~max_deadline ile bounded olmalı (300s HANG değil), süre: {:?}",
            t0.elapsed()
        );
        assert_eq!(out.events.len(), 5, "msg_count kadar undelivered event");
        assert!(
            out.events.iter().all(|e| e.delivered_ns.is_none()),
            "ölü adres → hiçbiri teslim edilmedi"
        );
        let m = crate::metrics::run_metrics(&out, &[Duration::from_secs(2)]);
        assert_eq!(
            m.per_deadline[0].completion, 0.0,
            "bağlanamadı → 0 completion (agregaya dahil)"
        );
    }
}
