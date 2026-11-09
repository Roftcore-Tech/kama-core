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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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
            // [KTP2-F0 / C1] Alıcı-tarafı damga — yalnız `--rx-log` ile. Bu sunucu döngüsü
            // kendiliğinden çıkmaz (kill ile biter): SIGTERM/SIGINT yakalanır, kayıt yazılır, süreç
            // çıkar. Bayraksız yolda sinyal işleyicisi KURULMAZ (eski davranış aynen).
            let rx_rec: Option<crate::rxlog::SharedRx> = cfg
                .rx_log
                .clone()
                .map(|p| Arc::new(Mutex::new(crate::rxlog::RxRecorder::new(p))));
            if let Some(r) = rx_rec.clone() {
                crate::rxlog::write_on_signal(r).map_err(BenchError::Io)?;
            }
            loop {
                let Ok((mut sock, _peer)) = listener.accept().await else {
                    continue;
                };
                sock.set_nodelay(true).ok();
                // [KTP2-F0 / D] transport-ack: uygulama ACK'i YAZILMAZ (teslim istemcide snd_una).
                let tack = crate::rival::get().transport_ack();
                let rec = rx_rec.clone();
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
                        crate::rxlog::record_shared(rec.as_ref(), msg_id, len);
                        // Uygulama-düzeyi ACK (teslim tanımının taşıyıcısı; qdgram ile simetrik).
                        if !tack {
                            if sock.write_all(&msg_id.to_be_bytes()).await.is_err() {
                                break;
                            }
                            app_tx += ACK_LEN as u64;
                        }
                    }
                    eprintln!("TCP-SERVER delivered={delivered} app_rx={app_rx}B app_tx={app_tx}B");
                });
            }
        }
    }

    async fn run_client(&self, cfg: ClientCfg) -> Result<RunOutput, BenchError> {
        // [KTP2-F0 / D] Rakip-kol ayarları (bayraksız = eski kol; bkz. `crate::rival`).
        let tu = crate::rival::get();
        if tu.transport_ack() {
            return run_client_tack(cfg, tu).await;
        }
        let delta = tu.tcp_delta();
        let connect_timeout = tu
            .handshake_timeout
            .unwrap_or(crate::rival::LEGACY_TCP_CONNECT_TIMEOUT);
        {
            // ── KURULUM MALİYETİ ─────────────────────────────────────────────────────────
            // [FAZ-4 §M4 DÜZELTME] İlk yazımda TCP'nin teslim saati kurulumdan ÖNCE başlıyordu →
            // TCP el sıkışmasını completion penceresi İÇİNDE ödüyordu, diğer üç kol ÖDEMİYORDU.
            // Bu sessiz bir kurulum-sınıfı karışımıdır (kıyas geçersiz). Kural TEK: kurulum AYRI
            // ölçülür ve completion penceresinin DIŞINDA kalır. Sayı manifeste yazılır.
            let setup_clock = Instant::now();
            let sock = match tokio::time::timeout(connect_timeout, TcpStream::connect(cfg.server))
                .await
            {
                Ok(Ok(s)) => s,
                _ => {
                    let ns = u64::try_from(setup_clock.elapsed().as_nanos()).unwrap_or(u64::MAX);
                    eprintln!("TCP-CLIENT connect FAILED (setup_ns={ns})");
                    return Ok(failed_output(&cfg, ns, TCP_LEGACY, delta));
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
            if tu.early_exit {
                // [İSTEK-D1] Aynı pencere; bütün ACK'ler geldiyse erken çık. Damgalar okuyucu
                // görevde konduğundan teslim olayları sabit beklemeyle birebir aynıdır.
                let until = tokio::time::Instant::now() + wait;
                loop {
                    let all = delivered
                        .lock()
                        .map(|g| g.iter().all(Option::is_some))
                        .unwrap_or(false);
                    if all || tokio::time::Instant::now() >= until {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            } else {
                tokio::time::sleep(wait).await;
            }
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
                meta: meta_for(
                    &cfg,
                    app_tx,
                    app_rx,
                    setup_ns,
                    TCP_LEGACY,
                    format!("tcp-default;setup=excluded-from-window;setup_ns={setup_ns}"),
                    delta,
                ),
                events,
            })
        }
    }
}

/// Eski kolun taşıyıcı adı (manifest anlamı `tcp-default`).
const TCP_LEGACY: &str = "tcp-default";
/// [KTP2-F0 / D] `transport-ack` kolunun taşıyıcı adı (manifest anlamı ayrı: `tcp-tack`).
const TCP_TACK: &str = "tcp-tack";

fn meta_for(
    cfg: &ClientCfg,
    app_tx: u64,
    app_rx: u64,
    setup_ns: u64,
    transport: &str,
    arm_label: String,
    delta: Vec<String>,
) -> RunMeta {
    RunMeta {
        profile: cfg.profile.clone(),
        cc: "tcp-kernel".to_string(),
        transport: transport.to_string(),
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
        kama_arm: arm_label,
        submit_interval_ns: u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX),
        budget: crate::budget::BudgetSnapshot::default(),
        t0_unix_ns: 0,
        events_epoch_offset_ns: 0,
        warmup_residue: 0,
        activation: None,
        setup_ns,
        setup_in_window: false,
        profile_id: String::new(),
        profile_delta: delta,
    }
}

/// Bağlantı kurulamadı → hiçbir mesaj teslim edilmedi (0-completion; GEÇERLİ ölçüm sonucu,
/// harness hatası DEĞİL — `connection failure` sınıfı).
fn failed_output(cfg: &ClientCfg, setup_ns: u64, transport: &str, delta: Vec<String>) -> RunOutput {
    let events = (0..cfg.msg_count)
        .map(|msg_id| RawEvent {
            msg_id,
            sent_ns: 0,
            delivered_ns: None,
            payload_bytes: cfg.payload_bytes,
        })
        .collect();
    RunOutput {
        meta: meta_for(
            cfg,
            0,
            0,
            setup_ns,
            transport,
            // Eski kolda etiket bit-bit eski biçim (`tcp-default;setup=…;setup_ns=…`).
            format!("{transport};setup=excluded-from-window;setup_ns={setup_ns}"),
            delta,
        ),
        events,
    }
}

/// [KTP2-F0 / D] `transport-ack` örnekleme periyodu: teslim damgasının çözünürlüğü (≤ 5 ms geç).
const TACK_POLL: Duration = Duration::from_millis(5);

/// [KTP2-F0 / D] İstemci netns'indeki soketin `tx_queue` alanı (= `write_seq − snd_una`, bayt).
///
/// `/proc/self/net/tcp` okuyan sürecin ağ ad alanını gösterir; eşleme yerel + uzak port ve
/// ESTABLISHED (`01`) durumuyla yapılır. Satır yoksa (`None`) bu örnek ölçülemedi sayılır.
fn read_tx_queue(local_port: u16, peer_port: u16) -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/net/tcp").ok()?;
    let lp = format!(":{local_port:04X}");
    let pp = format!(":{peer_port:04X}");
    for line in text.lines().skip(1) {
        let mut f = line.split_whitespace();
        let (Some(_sl), Some(local), Some(remote), Some(state), Some(queues)) =
            (f.next(), f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };
        if local.ends_with(&lp) && remote.ends_with(&pp) && state == "01" {
            let (tx, _rx) = queues.split_once(':')?;
            return u64::from_str_radix(tx, 16).ok();
        }
    }
    None
}

/// [KTP2-F0 / D] TCP `transport-ack` kolu (`tcp-tack`).
///
/// * Bağlantı ve çerçeve aynı (`u32 len ‖ u64 msg_id ‖ payload`); sunucu uygulama ACK'i YAZMAZ.
/// * Sunum anı PLANDIR: mesaj i'nin `sent_ns` = i × aralık (ölçüm-t0'dan). Yazıcı geride kalırsa
///   (çekirdek gönderim tamponu dolu) bekleme gecikmeye DAHİLDİR — eski kolda damga yazma anında
///   konduğu için bu bekleme görünmüyordu (eski yol: `*slot = elapsed_ns(start)` yazmadan önce).
/// * Teslim: mesajın kümülatif sonu `E_i`, onaylanan bayt `A = W − tx_queue` sınırını geçince
///   (`W` = soketin kabul ettiği bayt, yazma bittikten SONRA artırılır → `A` hiçbir zaman gerçek
///   `snd_una`'nın önüne geçmez; erken teslim yazılamaz).
/// * Gözlem penceresi mesaj başına `max_deadline` (son plan + `max_deadline`'a dek); hepsi teslim
///   edilince erken biter. Eski kolun `min(max_deadline, 60 s)` kesmesi burada YOK.
async fn run_client_tack(
    cfg: ClientCfg,
    tu: &crate::rival::RivalTuning,
) -> Result<RunOutput, BenchError> {
    let delta = tu.tcp_delta();
    let connect_timeout = tu
        .handshake_timeout
        .unwrap_or(crate::rival::LEGACY_TCP_CONNECT_TIMEOUT);
    let setup_clock = Instant::now();
    let sock = match tokio::time::timeout(connect_timeout, TcpStream::connect(cfg.server)).await {
        Ok(Ok(s)) => s,
        _ => {
            let ns = elapsed_ns(setup_clock);
            eprintln!("TCP-TACK-CLIENT connect FAILED (setup_ns={ns})");
            return Ok(failed_output(&cfg, ns, TCP_TACK, delta));
        }
    };
    sock.set_nodelay(true).ok();
    let setup_ns = elapsed_ns(setup_clock);
    let local_port = sock.local_addr().map_err(BenchError::Io)?.port();
    let peer_port = sock.peer_addr().map_err(BenchError::Io)?.port();
    let t0 = tokio::time::Instant::now();
    let (rd, mut wr) = sock.into_split();

    let n = usize::try_from(cfg.msg_count).unwrap_or(0);
    let payload_len = usize::try_from(cfg.payload_bytes).unwrap_or(0);
    let frame_len = (FRAME_HDR + payload_len) as u64;
    let interval_ns = u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX);
    let sent_ns: Vec<u64> = (0..n as u64)
        .map(|i| interval_ns.saturating_mul(i))
        .collect();
    let ends: Vec<u64> = (1..=n as u64)
        .map(|i| i.saturating_mul(frame_len))
        .collect();

    // Yazıcı: plana göre yazar; `written` yazma TAMAMLANINCA artar. Yazma yarısını geri döner —
    // düşürülmesi FIN gönderir ve soket ESTABLISHED'dan çıkar (örnekleme biterdi).
    let written = Arc::new(AtomicU64::new(0));
    let w2 = Arc::clone(&written);
    let payload = vec![0x5Au8; payload_len];
    let writer = tokio::spawn(async move {
        let mut send_errors: u64 = 0;
        for i in 0..n as u64 {
            tokio::time::sleep_until(t0 + Duration::from_nanos(interval_ns.saturating_mul(i)))
                .await;
            let mut frame = Vec::with_capacity(FRAME_HDR + payload.len());
            frame.extend_from_slice(&u32::try_from(payload.len()).unwrap_or(0).to_be_bytes());
            frame.extend_from_slice(&i.to_be_bytes());
            frame.extend_from_slice(&payload);
            if wr.write_all(&frame).await.is_err() {
                send_errors += 1;
                break;
            }
            w2.fetch_add(frame.len() as u64, Ordering::AcqRel);
        }
        (wr, send_errors)
    });

    // Örnekleyici (bu görevde): son plan + max_deadline'a dek ya da hepsi teslim edilene dek.
    let last_ns = interval_ns.saturating_mul(n.saturating_sub(1) as u64);
    let window_end = t0 + Duration::from_nanos(last_ns) + cfg.max_deadline;
    let mut delivered: Vec<Option<u64>> = vec![None; n];
    let mut next = 0usize;
    let mut polls: u64 = 0;
    let mut poll_fail: u64 = 0;
    let mut tick = tokio::time::interval(TACK_POLL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    while next < n && tokio::time::Instant::now() < window_end {
        tick.tick().await;
        polls += 1;
        // ÖNCE W, SONRA tx_queue: araya giren yazma A'yı yalnız küçültür (alt tahmin).
        let w = written.load(Ordering::Acquire);
        match read_tx_queue(local_port, peer_port) {
            Some(txq) => {
                let acked = w.saturating_sub(txq);
                let now = u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
                while next < n && ends[next] <= acked {
                    delivered[next] = Some(now);
                    next += 1;
                }
            }
            None => poll_fail += 1,
        }
    }
    writer.abort();
    let send_errors = match writer.await {
        Ok((w, e)) => {
            drop(w);
            e
        }
        Err(_) => 0,
    };
    drop(rd);
    let app_tx = written.load(Ordering::Acquire);
    if polls > 0 && poll_fail == polls {
        return Err(BenchError::Other(format!(
            "tcp-tack: /proc/self/net/tcp'de soket bulunamadı ({polls} örnek) — teslim ölçülemedi"
        )));
    }
    eprintln!(
        "TCP-TACK-CLIENT delivered={next}/{n} setup={setup_ns}ns app_tx={app_tx}B polls={polls} \
         poll_fail={poll_fail} send_errors={send_errors} semantics=[reliable-ordered, \
         transport-ack(snd_una), schedule-offer, setup-excluded] \
         wire_note=kernel-tcp-overhead-INVISIBLE"
    );
    let events = (0..n)
        .map(|i| RawEvent {
            msg_id: i as u64,
            sent_ns: sent_ns[i],
            delivered_ns: delivered[i],
            payload_bytes: cfg.payload_bytes,
        })
        .collect();
    let label = format!(
        "{TCP_TACK};setup=excluded-from-window;setup_ns={setup_ns};poll_ms={};polls={polls};\
         poll_fail={poll_fail};send_errors={send_errors}",
        TACK_POLL.as_millis()
    );
    Ok(RunOutput {
        meta: meta_for(&cfg, app_tx, 0, setup_ns, TCP_TACK, label, delta),
        events,
    })
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
                    rx_log: None,
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
                dup_copies: 1,
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

    /// [KTP2-F0 / D] `transport-ack` kolu: uygulama ACK'i OLMADAN, `snd_una` örneklemesiyle her
    /// mesaj teslim damgası alır; damga plandan önce olamaz; anlam etiketi `tcp-tack`. Global
    /// ayar (`rival::set`) KULLANILMAZ — tuning doğrudan verilir, eski testler etkilenmez.
    #[tokio::test]
    async fn tack_snd_una_ile_teslim() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        // Yanıt YAZMAYAN sunucu (tack sunucusunun davranışı): yalnız okur.
        let srv = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.expect("accept");
            let mut buf = vec![0u8; 4096];
            while let Ok(k) = s.read(&mut buf).await {
                if k == 0 {
                    break;
                }
            }
        });
        let n = 12u64;
        let tu = crate::rival::RivalTuning::from_cli(
            crate::rival::Delivery::TransportAck,
            5,
            0,
            0,
            0,
            false,
        );
        let out = run_client_tack(
            ClientCfg {
                server: addr,
                msg_count: n,
                payload_bytes: 65,
                max_deadline: Duration::from_secs(5),
                cc: Cc::Cubic,
                seed: 0,
                repeat: 0,
                profile: "loopback-tack".to_string(),
                kama_arm: KamaArm::Arq,
                submit_interval: Duration::from_millis(10),
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
                dup_copies: 1,
            },
            &tu,
        )
        .await
        .expect("tack client");
        srv.abort();
        assert_eq!(out.meta.transport, TCP_TACK);
        assert!(out.meta.kama_arm.starts_with("tcp-tack;"));
        assert_eq!(
            out.meta.profile_delta.len(),
            3,
            "{:?}",
            out.meta.profile_delta
        );
        assert_eq!(out.events.len(), n as usize);
        for (i, e) in out.events.iter().enumerate() {
            assert_eq!(e.sent_ns, 10_000_000 * i as u64, "sunum anı PLAN olmalı");
            let d = e.delivered_ns.expect("loopback'te her mesaj onaylanmalı");
            assert!(d >= e.sent_ns, "teslim plandan önce olamaz");
        }
        assert_eq!(out.meta.udp_tx_bytes, n * (65 + FRAME_HDR as u64));
        assert_eq!(out.meta.udp_rx_bytes, 0, "uygulama ACK'i yok");
    }
}
