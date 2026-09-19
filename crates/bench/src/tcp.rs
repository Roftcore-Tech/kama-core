//! [FAZ-4] TCP TABAN KOLU — `tcp-default`.
//!
//! Denetim bulgusu: depoda TCP karşılaştırma kolu **YOKTU**; hedef zarf iddiası TCP'yi kapsadığı
//! hâlde hiçbir ölçüm TCP'ye karşı yapılamıyordu. Bu modül platformun NORMAL TCP yığınını kullanır
//! (özel ARQ döngüsü simülasyonu YOK, egzotik tuning YOK).
//!
//! ## Anlamsal eşleşme (Faz-4 sözleşmesi)
//!
//! * **Teslim tanımı:** `delivered_ns` = mesajı kapsayan **uygulama-düzeyi ACK'in** gönderene
//!   varış anı. Bu, `qdgram` kolunun tanımıyla BİREBİR aynıdır ve `kama-product`'ın "ilk teslim
//!   teyidi" tanımıyla aynı katmandır. TCP'de yazma-tamamlanması ÇEKİRDEK TAMPONUNU ölçer, teslimi
//!   DEĞİL — bu yüzden kullanılmaz.
//! * **Çerçeveleme:** TCP bir byte akışıdır ve mesaj sınırı KORUMAZ. Harness minimum, simetrik ve
//!   muhasebeye DÂHİL bir çerçeve ekler: `u32 len ‖ u64 msg_id ‖ payload`. ACK: `u64 msg_id`.
//!   Bu ek yük `app_overhead_bytes` olarak raporlanır.
//! * **Güvenilirlik/sıra:** TCP = güvenilir + SIRALI. Kama v1 = güvenilir, mesaj-granülü
//!   (sıra garantisi YOK). Fark manifestte AÇIKÇA raporlanır; normalize EDİLMEZ (TCP'ye fazladan
//!   iş yüklemek de, Kama'ya sıra garantisi eklemek de ölçümü bozardı).
//! * **Kurulum maliyeti:** `connect()` süresi ayrı ölçülür (`setup_ns`) ve completion penceresinin
//!   DIŞINDA tutulur (`setup_in_window=false`) — dört kolun tamamında aynı kural.
//! * **Tel maliyeti — BEYAN EDİLEN SINIR:** harness yalnız UYGULAMA katmanındaki byte'ları sayar.
//!   TCP'nin çekirdek içi retransmit/ACK/başlık trafiği kullanıcı alanından GÖRÜNMEZ. Bu yüzden
//!   TCP için `udp_tx_bytes`/`udp_rx_bytes` alanları **uygulama byte'larıdır** ve Kama/QUIC'in
//!   tam tel muhasebesiyle DOĞRUDAN KIYASLANAMAZ. Rapor bunu açıkça belirtmek ZORUNDADIR.
//!
//! Ayar: yalnız `TCP_NODELAY` açık (mesaj-granüllü, gecikmeye duyarlı yük için savunulabilir
//! ürün-benzeri ayar). Başka hiçbir socket ayarı değiştirilmez.

use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};
use crate::transport::{BenchTransport, ClientCfg, ServerCfg};

/// `u32 len ‖ u64 msg_id` — çerçeve başlığı (harness ek yükü; muhasebeye dâhil).
const FRAME_HDR: usize = 12;
/// ACK çerçevesi: `u64 msg_id`.
const ACK_LEN: usize = 8;

/// TCP taban kolu (`tcp-default`).
pub struct TcpTransport;

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

impl BenchTransport for TcpTransport {
    async fn run_server(&self, cfg: ServerCfg) -> Result<(), BenchError> {
        {
            let listener = TcpListener::bind(cfg.bind).await.map_err(BenchError::Io)?;
            loop {
                let Ok((mut sock, _peer)) = listener.accept().await else {
                    continue;
                };
                sock.set_nodelay(true).ok();
                tokio::spawn(async move {
                    let mut delivered: u64 = 0;
                    let mut app_rx: u64 = 0;
                    let mut app_tx: u64 = 0;
                    let mut hdr = [0u8; FRAME_HDR];
                    loop {
                        if sock.read_exact(&mut hdr).await.is_err() {
                            break; // peer kapandı
                        }
                        let len = u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
                        let msg_id = u64::from_be_bytes([
                            hdr[4], hdr[5], hdr[6], hdr[7], hdr[8], hdr[9], hdr[10], hdr[11],
                        ]);
                        // bounds-before-alloc (SR-AUDIT-001): çerçeve boyu sınırlı.
                        if len > 1 << 20 {
                            break;
                        }
                        let mut body = vec![0u8; len];
                        if sock.read_exact(&mut body).await.is_err() {
                            break;
                        }
                        app_rx += (FRAME_HDR + len) as u64;
                        delivered += 1;
                        // Uygulama-düzeyi ACK (teslim tanımının taşıyıcısı; qdgram ile simetrik).
                        if sock.write_all(&msg_id.to_be_bytes()).await.is_err() {
                            break;
                        }
                        app_tx += ACK_LEN as u64;
                    }
                    eprintln!("TCP-SERVER delivered={delivered} app_rx={app_rx}B app_tx={app_tx}B");
                });
            }
        }
    }

    async fn run_client(&self, cfg: ClientCfg) -> Result<RunOutput, BenchError> {
        {
            // ── KURULUM MALİYETİ ─────────────────────────────────────────────────────────
            // [FAZ-4 §M4 DÜZELTME] İlk yazımda TCP'nin teslim saati kurulumdan ÖNCE başlıyordu →
            // TCP el sıkışmasını completion penceresi İÇİNDE ödüyordu, diğer üç kol ÖDEMİYORDU.
            // Bu sessiz bir kurulum-sınıfı karışımıdır (kıyas geçersiz). Kural TEK: kurulum AYRI
            // ölçülür ve completion penceresinin DIŞINDA kalır. Sayı manifeste yazılır.
            let setup_clock = Instant::now();
            let sock =
                match tokio::time::timeout(Duration::from_secs(20), TcpStream::connect(cfg.server))
                    .await
                {
                    Ok(Ok(s)) => s,
                    _ => {
                        let ns =
                            u64::try_from(setup_clock.elapsed().as_nanos()).unwrap_or(u64::MAX);
                        eprintln!("TCP-CLIENT connect FAILED (setup_ns={ns})");
                        return Ok(failed_output(&cfg, ns));
                    }
                };
            sock.set_nodelay(true).ok();
            let setup_ns = u64::try_from(setup_clock.elapsed().as_nanos()).unwrap_or(u64::MAX);
            // Ölçüm saati kurulumdan SONRA başlar (dört kolda aynı).
            let start = Instant::now();
            let (mut rd, mut wr) = sock.into_split();

            let n = usize::try_from(cfg.msg_count).unwrap_or(0);
            let payload = vec![0x5Au8; usize::try_from(cfg.payload_bytes).unwrap_or(0)];
            let mut sent_ns: Vec<u64> = vec![0; n];
            let delivered: std::sync::Arc<std::sync::Mutex<Vec<Option<u64>>>> =
                std::sync::Arc::new(std::sync::Mutex::new(vec![None; n]));

            // ACK okuyucusu: teslim damgası BURADA konur (ilk-ack-kazanır).
            let d2 = std::sync::Arc::clone(&delivered);
            let reader = tokio::spawn(async move {
                let mut buf = [0u8; ACK_LEN];
                let mut app_rx: u64 = 0;
                while rd.read_exact(&mut buf).await.is_ok() {
                    app_rx += ACK_LEN as u64;
                    let id = u64::from_be_bytes(buf) as usize;
                    if let Ok(mut g) = d2.lock() {
                        if let Some(slot) = g.get_mut(id) {
                            if slot.is_none() {
                                *slot = Some(elapsed_ns(start));
                            }
                        }
                    }
                }
                app_rx
            });

            let mut app_tx: u64 = 0;
            let mut send_errors: u64 = 0;
            for (i, slot) in sent_ns.iter_mut().enumerate().take(n) {
                let id = i as u64;
                let mut frame = Vec::with_capacity(FRAME_HDR + payload.len());
                frame.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_be_bytes());
                frame.extend_from_slice(&id.to_be_bytes());
                frame.extend_from_slice(&payload);
                *slot = elapsed_ns(start); // app-offer anı (diğer kollarla AYNI sıfır noktası)
                if wr.write_all(&frame).await.is_err() {
                    send_errors += 1;
                    break;
                }
                app_tx += frame.len() as u64;
                if !cfg.submit_interval.is_zero() {
                    tokio::time::sleep(cfg.submit_interval).await;
                }
            }
            // Deadline penceresi dolana dek ACK'leri bekle, sonra bağlantıyı kapat.
            let wait = cfg.max_deadline.min(Duration::from_secs(60));
            tokio::time::sleep(wait).await;
            drop(wr);
            let app_rx = tokio::time::timeout(Duration::from_secs(2), reader)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(0);

            let d = delivered.lock().map(|g| g.clone()).unwrap_or_default();
            let done = d.iter().filter(|x| x.is_some()).count();
            eprintln!(
                "TCP-CLIENT delivered={done}/{n} setup={setup_ns}ns app_tx={app_tx}B app_rx={app_rx}B \
                 send_errors={send_errors} semantics=[reliable-ordered, app-ack-delivery, setup-excluded] \
                 wire_note=kernel-tcp-overhead-INVISIBLE"
            );
            let events = (0..n)
                .map(|i| RawEvent {
                    msg_id: i as u64,
                    sent_ns: sent_ns[i],
                    delivered_ns: d.get(i).copied().flatten(),
                    payload_bytes: cfg.payload_bytes,
                })
                .collect();
            Ok(RunOutput {
                meta: meta_for(&cfg, app_tx, app_rx, setup_ns),
                events,
            })
        }
    }
}

fn meta_for(cfg: &ClientCfg, app_tx: u64, app_rx: u64, setup_ns: u64) -> RunMeta {
    RunMeta {
        profile: cfg.profile.clone(),
        cc: "tcp-kernel".to_string(),
        transport: "tcp-default".to_string(),
        quinn_version: String::new(),
        seed: cfg.seed,
        repeat: cfg.repeat,
        msg_count: cfg.msg_count,
        payload_bytes: cfg.payload_bytes,
        max_deadline_ns: u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX),
        // BEYAN: bunlar UYGULAMA byte'larıdır (çekirdek TCP ek yükü görünmez) — bkz. modül dokümanı.
        udp_tx_bytes: app_tx,
        udp_rx_bytes: app_rx,
        tx: TxComponents::default(),
        kama_arm: format!("tcp-default;setup=excluded-from-window;setup_ns={setup_ns}"),
        submit_interval_ns: u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX),
        budget: crate::budget::BudgetSnapshot::default(),
        t0_unix_ns: 0,
        events_epoch_offset_ns: 0,
        warmup_residue: 0,
        activation: None,
        setup_ns,
        setup_in_window: false,
        profile_id: String::new(),
        profile_delta: Vec::new(),
    }
}

/// Bağlantı kurulamadı → hiçbir mesaj teslim edilmedi (0-completion; GEÇERLİ ölçüm sonucu,
/// harness hatası DEĞİL — `connection failure` sınıfı).
fn failed_output(cfg: &ClientCfg, setup_ns: u64) -> RunOutput {
    let events = (0..cfg.msg_count)
        .map(|msg_id| RawEvent {
            msg_id,
            sent_ns: 0,
            delivered_ns: None,
            payload_bytes: cfg.payload_bytes,
        })
        .collect();
    RunOutput {
        meta: meta_for(cfg, 0, 0, setup_ns),
        events,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{BenchTransport, Cc, ClientCfg, KamaArm};
    use std::net::SocketAddr;

    /// [FAZ-4 §22] TCP ÇERÇEVELEMESİ BENCHMARK YÜK ANLAMINI KORUR.
    ///
    /// TCP bir byte akışıdır: mesaj sınırı YOKTUR. Harness `u32 len ‖ u64 msg_id` çerçevesi
    /// ekler. Bu test, N mesajın N ayrı uygulama teslimi olarak geri geldiğini ve her olayın
    /// benchmark yük boyutunu taşıdığını kilitler — akışın tek bir bloğa yapışmadığını.
    #[tokio::test]
    async fn cerceveleme_mesaj_anlamini_korur() {
        let bind: SocketAddr = "127.0.0.1:0".parse().expect("addr");
        // Portu önceden bağlayıp öğren: 0 portu ile server'ın seçtiği portu bilemezdik.
        let probe = tokio::net::TcpListener::bind(bind).await.expect("bind");
        let addr = probe.local_addr().expect("addr");
        drop(probe);
        let srv = tokio::spawn(async move {
            let _ = TcpTransport
                .run_server(crate::transport::ServerCfg {
                    bind: addr,
                    cc: Cc::Cubic,
                    quota_r: 4,
                    budget: crate::budget::BudgetMode::Unlimited,
                    gate_watch: None,
                    fec: crate::transport::FecArm::Off,
                    payload_bytes: 137,
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
                    rp_suppress: None,
                    spec_l2_permille: None,
                    spec_l1_permille: None,
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(150)).await;
        let n = 25u64;
        let payload = 137u64; // 256'dan farklı: boyutun gerçekten taşındığını görelim
        let out = TcpTransport
            .run_client(ClientCfg {
                server: addr,
                msg_count: n,
                payload_bytes: payload,
                max_deadline: Duration::from_millis(600),
                cc: Cc::Cubic,
                seed: 7,
                repeat: 0,
                profile: "loopback".to_string(),
                kama_arm: KamaArm::Arq,
                submit_interval: Duration::ZERO,
                budget: crate::budget::BudgetMode::Unlimited,
                warmup_msgs: 0,
                gate: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                rp_suppress: None,
                establish_gate: None,
                fec: crate::transport::FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_out: None,
                gate_permille: None,
                spec_l2_permille: None,
                spec_l1_permille: None,
            })
            .await
            .expect("client");
        srv.abort();

        assert_eq!(out.events.len(), n as usize, "N mesaj = N olay");
        let delivered = out
            .events
            .iter()
            .filter(|e| e.delivered_ns.is_some())
            .count();
        assert_eq!(delivered, n as usize, "loopback'te hepsi teslim edilmeli");
        for (i, e) in out.events.iter().enumerate() {
            assert_eq!(e.msg_id, i as u64, "mesaj kimliği korunmalı");
            assert_eq!(e.payload_bytes, payload, "yük boyutu korunmalı");
            let d = e.delivered_ns.expect("teslim");
            assert!(d >= e.sent_ns, "teslim gönderimden önce olamaz");
        }
        // [§9] Muhasebe: çerçeve başlığı tel maliyetine DAHİL, gizlenmiyor.
        let beklenen_tx = n * (payload + FRAME_HDR as u64);
        assert_eq!(
            out.meta.udp_tx_bytes, beklenen_tx,
            "başlık maliyeti sayılmalı"
        );
        assert_eq!(out.meta.udp_rx_bytes, n * ACK_LEN as u64);
        // [§M4] Kurulum ölçüm penceresinin DIŞINDA ve ayrıca ölçülmüş.
        assert!(!out.meta.setup_in_window);
        assert!(out.meta.setup_ns > 0, "kurulum süresi ölçülmeli");
    }
}
