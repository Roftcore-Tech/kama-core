//! **`kama-product`** — bench kolu olarak GERÇEK ÜRÜN RUNTIME'ı ([`client_core::runtime`]).
//!
//! Neden ayrı bir kol: [`crate::kama`] bir *security-stub sürücüsüdür* — kripto yoktur, bu yüzden
//! FEC parity YAPISAL OLARAK emit edilemez (shard'lar per-shard AEAD ile mühürlenmek zorunda,
//! INV-FEC-1). Sonuç: quinn kıyaslarımızın tamamı bugüne dek Kama'nın **FEC'siz** yarısıyla
//! koştu. Bu kol o boşluğu kapatır: ürünün kendi `ClientEndpoint`/`ServerEndpoint`'ini sürer, yani
//! wire'da gerçekte ne varsa o ölçülür —
//!
//! - **FEC + ARQ füzyonu** (§8 latch + repair emisyonu + RECOVERED-control ile retransmit iptali),
//! - **fuzzy decision making** (motorun `Sender` kontrolcüsü; `FecActuationMode::Controller`
//!   modunda yapı-gate kararı latch'e akar),
//! - **tail recovery** (`ClientSession::new()` sevk default'u = `TailRecoveryConfig::validated_v1()`,
//!   G2 kararı — bu kolda kapatılamaz, ürün neyse o),
//! - **handshake-siz kuruluş** (soket bağlanır bağlanmaz veri akar; RTT maliyeti yok),
//! - **PARAM bootstrap** (alıcı fail-closed doğrular).
//!
//! **SoC korunur (INV-ENGINE-SOC):** bench kripto TAKMAZ; ürünün güvenlik dikişine
//! [`IdentitySeam`] (byte-eşdeğer +16 B sahte tag) takılır — [`crate::kama`]'nın `TAG_STUB`'ıyla
//! AYNI byte maliyeti, dolayısıyla iki Kama kolu birbirleriyle de byte-kıyaslanabilir. S2'de
//! aynı dikişe kama-noise-wire takılınca bu kol DEĞİŞMEDEN gerçek kriptoyla koşar.
//!
//! **Ölçüm modeli — üç kolda da AYNI çerçeve** ([`crate::event::RawEvent`]):
//! `sent_ns` = APP-OFFER anı (enqueue; transportun iç kuyruğu latency'ye DAHİL),
//! `delivered_ns` = birimin ilk teslim-teyidi (ACK veya FEC-RECOVERED — ikisi de ürünün
//! `acked()` akışına girer, ilk-kazanır). Bütçe governor'ü ürünün egress dikişine
//! ([`client_core::runtime::EgressPolicy`]) takılır: paket-atomik, pacing-nötr, yön-başına —
//! [`crate::budget`]'teki TEK implementasyon, quinn/qdgram kollarıyla birebir aynı.
//!
//! **`udp_tx_bytes` = DENEME düzlemi** (governor reddettikleri DAHİL — üç kolda da aynı kural);
//! gerçek-enjekte düzlem `RunMeta.budget.injected_bytes`.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use client_core::runtime::{
    fec_chunk_capacity, ClientEndpoint, ClientSender, EgressPolicy, FecRuntimeConfig,
    FecStructurePolicy, RuntimeConfig, RuntimeError, ServerEndpoint,
};
use client_core::seam::IdentitySeam;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::interval;

use crate::budget::{BudgetGovernor, BudgetMode, IPV4_UDP_OVERHEAD};
use crate::error::BenchError;
use crate::event::{RawEvent, RunMeta, RunOutput, TxComponents};
use crate::transport::{BenchTransport, ClientCfg, FecArm, ServerCfg};

/// transport-version (RunMeta.quinn_version alanına — alan donmuş, yeniden adlandırma YOK).
const KAMA_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Gate-watch poll periyodu (server): ölçüm-t0 sapması ≤ bu değer.
const GATE_POLL: Duration = Duration::from_millis(20);
/// Offer kuyruğu doluyken yeniden-deneme aralığı (app-tarafı backpressure sözleşmesi).
const OFFER_RETRY: Duration = Duration::from_millis(1);
/// Warmup faz tavanı (sunum planına EKLENİR) — bounded: ölü sunucuda faz asılı kalmaz.
const WARMUP_CAP_EXTRA: Duration = Duration::from_secs(30);

/// Bütçe governor'ünü ürün-runtime'ının egress dikişine takan adaptör.
///
/// Governor `Arc<Mutex<_>>` ile paylaşılır: sahibi endpoint görevidir, ama koşu sonunda
/// `snapshot()`'ı çağıran taraf (RunMeta) okur. Kilit ASLA `await` boyunca tutulmaz.
struct BudgetEgress(Arc<Mutex<BudgetGovernor>>);

impl EgressPolicy for BudgetEgress {
    fn admit(&mut self, len: usize, now_ns: u64) -> bool {
        // IP-düzeyi bütçe (r5 D.2/H): UDP payload + IPv4/UDP başlıkları.
        let ip_len = len as u64 + IPV4_UDP_OVERHEAD;
        // Zehirli kilit = koşu geçersiz; sessizce "geçir" demek bütçeyi sessizce kaldırırdı.
        // Panik yerine REDDET: fail-closed, sayaçlar donar, anomali ölçümde görünür.
        self.0.lock().is_ok_and(|mut g| g.permit(ip_len, now_ns))
    }

    fn arm(&mut self, now_ns: u64) {
        if let Ok(mut g) = self.0.lock() {
            g.arm(now_ns);
        }
    }
}

/// [`FecArm`] → ürün `RuntimeConfig`.
///
/// **[S1B] Birim vs mesaj.** FEC açıkken bir chunk en çok `E − SOURCE_PREFIX_LEN` byte taşır
/// (profil 0x03'te 53 B). Benchmark yükü bundan büyükse (256 B, 1200 B …) fragmenter AÇILIR:
/// `max_unit_bytes` = symbol kapasitesi, `max_message_bytes` = tam mesaj. Böylece **FEC kolu
/// gerçek mesaj boylarında koşabilir** — fragmenter'dan önce bu yapısal olarak imkânsızdı ve
/// FEC ölçümleri 48 B'lık yapay bir yüke hapsolmuştu.
///
/// FEC kapalıyken mesaj = birim (fragmentasyon YOK), yani wire eski kollarla birebir aynıdır.
///
/// `drain_cap = 0`: bench'te kapanış-drenajı YOKTUR — aksi hâlde deadline SONRASI akan
/// retransmit'ler `tx_bytes`'a girer ve ölçüm penceresini sessizce genişletirdi.
fn runtime_cfg(
    payload_bytes: u64,
    quota_r: u32,
    fec: FecArm,
    burst_fusion: bool,
    bootstrap_rtt: bool,
    initial_window_bytes: u64,
    bootstrap_fast: bool,
) -> RuntimeConfig {
    let payload = usize::try_from(payload_bytes).unwrap_or(usize::MAX);
    let profile_id = match fec {
        FecArm::Off => None,
        FecArm::StaticR { profile_id, .. } | FecArm::Adaptive { profile_id } => Some(profile_id),
    };
    // Bilinmeyen profil: birim tavanını kısıtlama — `validate_fec` koşu-öncesi typed reddedecek.
    let unit = profile_id
        .and_then(fec_chunk_capacity)
        .map_or(payload, |cap| payload.min(cap));
    RuntimeConfig {
        max_unit_bytes: unit.max(1),
        // Mesaj chunk'a sığmıyorsa fragmenter AÇIK (açık opt-in; eşitsizlikten türemez).
        max_message_bytes: (payload > unit).then_some(payload),
        quota_r,
        // [BURST-FUSION] opt-in; false = sevk davranışı (byte-özdeş).
        fec_structure_policy: if burst_fusion {
            FecStructurePolicy::BurstFusion
        } else {
            FecStructurePolicy::Q18Iid
        },
        // [DR-CS-3] opt-in; false = sevk davranışı (probe planlanmaz).
        bootstrap_rtt,
        // [IW] opt-in; 0 = sevk davranışı (ilk uçuş tek paket).
        initial_window_bytes,
        // [KURULUŞ] merdiven seçimi (bootstrap_rtt kapalıyken etkisiz).
        bootstrap_fast_ladder: bootstrap_fast,
        drain_cap: Duration::ZERO,
        fec: match fec {
            FecArm::Off => None,
            FecArm::StaticR { profile_id, r } => Some(FecRuntimeConfig {
                profile_id,
                manual_r: Some(r),
            }),
            FecArm::Adaptive { profile_id } => Some(FecRuntimeConfig {
                profile_id,
                manual_r: None,
            }),
        },
        ..RuntimeConfig::default()
    }
}

fn rt_err(what: &str, e: RuntimeError) -> BenchError {
    BenchError::Other(format!("kama-product {what}: {e}"))
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Bir birim sunar; kuyruk doluyken bekler-yeniden-dener (bounded backpressure sözleşmesi).
///
/// `Closed`/`UnitTooLarge` = koşu GEÇERSİZ: birim id'leri offer sırasına bire bir bağlıdır
/// (`unit_id = warmup_msgs + msg_id`), sessizce atlanan bir offer tüm eşlemeyi kaydırırdı.
/// Bu yüzden typed hata yutulmaz, koşuyu düşürür.
async fn offer(tx: &ClientSender, payload: &[u8]) -> Result<(), BenchError> {
    loop {
        match tx.send(payload.to_vec()) {
            Ok(()) => return Ok(()),
            Err(RuntimeError::QueueFull) => tokio::time::sleep(OFFER_RETRY).await,
            Err(e) => return Err(rt_err("offer", e)),
        }
    }
}

/// Ürün-runtime bench kolu. `BenchTransport` sözleşmesi diğer kollarla birebir aynı.
pub struct KamaProductTransport;

impl BenchTransport for KamaProductTransport {
    fn run_server(
        &self,
        cfg: ServerCfg,
    ) -> impl std::future::Future<Output = Result<(), BenchError>> + Send {
        server_loop(cfg)
    }

    fn run_client(
        &self,
        cfg: ClientCfg,
    ) -> impl std::future::Future<Output = Result<RunOutput, BenchError>> + Send {
        client_loop(cfg)
    }
}

/// Server: ürün `ServerEndpoint`'i + reverse-yön governor. Teslimler drene edilir (app
/// sözleşmesi); ölçüm client tarafında üretilir — server `RunOutput` YAZMAZ.
async fn server_loop(cfg: ServerCfg) -> Result<(), BenchError> {
    let start = Instant::now();
    let gov = Arc::new(Mutex::new(BudgetGovernor::new(cfg.budget)));
    let policy: Box<dyn EgressPolicy> = Box::new(BudgetEgress(Arc::clone(&gov)));
    let mut srv = ServerEndpoint::bind_with_policy(
        cfg.bind,
        runtime_cfg(
            cfg.payload_bytes,
            cfg.quota_r,
            cfg.fec,
            cfg.burst_fusion,
            cfg.bootstrap_rtt,
            cfg.initial_window_bytes,
            cfg.bootstrap_fast,
        ),
        Box::new(IdentitySeam),
        Some(policy),
    )
    .await
    .map_err(|e| rt_err("bind", e))?;

    // [W6] gate-watch: reverse bütçe client'ın ölçüm-t0'ına (`<gate>.go`) senkronlanır — warmup
    // ACK'leri pre_arm'da muaf (r5 D.2). Verilmezse hemen arm (eski davranış).
    let go_file = cfg
        .gate_watch
        .as_ref()
        .map(|g| format!("{}.go", g.display()));
    let mut armed = go_file.is_none();
    if armed {
        srv.mark_t0().await;
    }
    let mut poll = interval(GATE_POLL);
    let mut delivered: u64 = 0;
    // [KAPI-DÜZELTME] Kapanış sinyali. Bu kol olmadan `server_loop` YAPISAL OLARAK istatistik
    // yazamıyordu: döngü yalnız `next_delivery()` None dönünce çıkar → bunun için `server_task`
    // bitmeli → bunun için `shutdown()` çağrılmalı → o da döngüden SONRA. Dairesel bağımlılık;
    // harness sunucuyu `kill` ettiğinden (product_vs_quinn.sh) süreç iki eprintln!'e hiç
    // ulaşmıyor ve `s.err` HER rep'te 0 byte kalıyordu. Bedeli: alıcı-tarafı FEC kanıtının
    // tamamı (fec_repairs_rx / fec_recovered_units / fec_decode_failures / msgs_reassembled /
    // param_bound_peers) kaybediliyordu — yani "FEC emit edildi" görünürken "FEC KURTARDI"
    // görünmüyordu, ki kampanyanın sorusu tam olarak ikincisi.
    let mut sigterm = signal(SignalKind::terminate())
        .map_err(|e| BenchError::Other(format!("kama-product sigterm: {e}")))?;
    let mut sigint = signal(SignalKind::interrupt())
        .map_err(|e| BenchError::Other(format!("kama-product sigint: {e}")))?;

    loop {
        tokio::select! {
            _ = sigterm.recv() => break,
            _ = sigint.recv() => break,
            _ = poll.tick() => {
                if !armed {
                    if let Some(f) = &go_file {
                        if Path::new(f).exists() {
                            srv.mark_t0().await;
                            armed = true;
                        }
                    }
                }
            }
            d = srv.next_delivery() => {
                if d.is_none() {
                    break; // endpoint görevi kapandı
                }
                delivered += 1;
            }
        }
    }

    let stats = srv.shutdown().await;
    let snap = gov.lock().map_or_else(
        |_| BudgetGovernor::new(BudgetMode::Unlimited).snapshot(),
        |g| g.snapshot(),
    );
    eprintln!(
        "BUDGET-REV mode={} injected={}B/{}pkt refused={}B/{}pkt armed_at_ns={} pre_arm={}B/{}pkt",
        snap.mode,
        snap.injected_bytes,
        snap.injected_pkts,
        snap.refused_bytes,
        snap.refused_pkts,
        snap.armed_at_ns,
        snap.pre_arm_bytes,
        snap.pre_arm_pkts,
    );
    eprintln!(
        "KP-SERVER delivered={delivered} t={:.1}s stats={stats:?}",
        elapsed_ns(start) as f64 / 1e9
    );
    Ok(())
}

/// Client: ürün `ClientEndpoint`'i + forward-yön governor + üç-kol-ortak ölçüm çerçevesi.
#[allow(clippy::too_many_lines)]
async fn client_loop(cfg: ClientCfg) -> Result<RunOutput, BenchError> {
    let start = Instant::now();
    let msg_count = cfg.msg_count;
    let n_msgs = usize::try_from(msg_count).unwrap_or(usize::MAX);
    let interval_ns = u64::try_from(cfg.submit_interval.as_nanos()).unwrap_or(u64::MAX);
    let payload = vec![0x5Au8; usize::try_from(cfg.payload_bytes).unwrap_or(0)];

    let gov = Arc::new(Mutex::new(BudgetGovernor::new(cfg.budget)));
    let policy: Box<dyn EgressPolicy> = Box::new(BudgetEgress(Arc::clone(&gov)));
    let mut cli = ClientEndpoint::connect_with_policy(
        cfg.server,
        runtime_cfg(
            cfg.payload_bytes,
            4,
            cfg.fec,
            cfg.burst_fusion,
            cfg.bootstrap_rtt,
            cfg.initial_window_bytes,
            cfg.bootstrap_fast,
        ),
        Box::new(IdentitySeam),
        Some(policy),
    )
    .await
    .map_err(|e| rt_err("connect", e))?;
    // Offer tutamağı AYRI: `acked()` `&mut cli` ödünç alır, sunum aynı `select!` içinde sürsün.
    let tx = cli.sender();

    // [W6] Kuruluş-gate'i: Kama'da handshake YOK → soket bağlanır bağlanmaz "kurulu"
    // (DÜZLEM-A temiz-ağ kuruluş alanı). Harness burada (A2) netem'i uygular.
    if let Some(g) = &cfg.establish_gate {
        crate::transport::gate_sync(g).await?;
    }

    // ── FAZ-0: warmup (ölçüm-DIŞI) ────────────────────────────────────────────────────────
    // Warmup birimleri 0..warmup_msgs-1 id'lerini işgal eder; ölçüm birimleri warmup_msgs'ten
    // başlar. Amaç: estimator/pacer/RTT + PARAM bağlanması ölçümden ÖNCE otursun.
    let mut warm_acked: u64 = 0;
    if cfg.warmup_msgs > 0 {
        let cap = Duration::from_nanos(interval_ns.saturating_mul(cfg.warmup_msgs))
            .saturating_add(WARMUP_CAP_EXTRA);
        let wdl = tokio::time::sleep(cap);
        tokio::pin!(wdl);
        let wstart = elapsed_ns(start);
        let mut wnext: u64 = 0;
        while warm_acked < cfg.warmup_msgs {
            tokio::select! {
                () = &mut wdl => break,
                () = tokio::time::sleep(Duration::from_nanos(
                    wstart
                        .saturating_add(interval_ns.saturating_mul(wnext))
                        .saturating_sub(elapsed_ns(start)),
                )), if wnext < cfg.warmup_msgs => {
                    offer(&tx, &payload).await?;
                    wnext += 1;
                }
                a = cli.acked() => {
                    if a.is_none() { break } // endpoint öldü
                    warm_acked += 1;
                }
            }
        }
    }

    // ── Ölçüm barrier'ı + t0 ──────────────────────────────────────────────────────────────
    if let Some(gate) = &cfg.gate {
        crate::transport::gate_sync(gate).await?;
    }
    let t0 = elapsed_ns(start);
    let t0_unix_ns = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    )
    .unwrap_or(0);
    let warmup_residue = cfg.warmup_msgs.saturating_sub(warm_acked);
    // Ürün-runtime'ında sayaç sıfırlama + governor arm TEK işaretle olur; MOTOR DURUMU KORUNUR
    // (counter_reset_without_state_reset). İşaret görev döngüsünde ≤ PACE_TICK (1 ms) içinde
    // işlenir → governor'ün `armed_at_ns`'i endpoint'in KENDİ epoch'undadır; t0-göreli
    // normalizasyon için `armed_at_ns − events_epoch_offset_ns` DEĞİL, snapshot'taki
    // `first_refusal_ns − armed_at_ns` farkı kullanılır (r5 inceleme #18 kuralı).
    cli.mark_t0().await;

    // ── FAZ-1: ölçüm ──────────────────────────────────────────────────────────────────────
    let base_id = cfg.warmup_msgs;
    let mut sent_ns: Vec<Option<u64>> = vec![None; n_msgs];
    let mut delivered_ns: Vec<Option<u64>> = vec![None; n_msgs];
    let mut delivered_count: u64 = 0;

    let mut next_submit: u64 = 0;
    if interval_ns == 0 {
        // burst: tümü t0'da sunulur (mevcut kolların davranışıyla aynı).
        for slot in &mut sent_ns {
            *slot = Some(elapsed_ns(start));
            offer(&tx, &payload).await?;
        }
        next_submit = msg_count;
    }

    // Koşu sonu = son-sunum + max_deadline (r5 §G).
    let total_deadline_ns = t0
        .saturating_add(interval_ns.saturating_mul(msg_count.saturating_sub(1)))
        .saturating_add(u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX));
    let deadline = tokio::time::sleep(Duration::from_nanos(
        total_deadline_ns.saturating_sub(elapsed_ns(start)),
    ));
    tokio::pin!(deadline);

    while delivered_count < msg_count {
        tokio::select! {
            () = &mut deadline => break,
            // [W3] paced sunum: mesaj i, t0 + i×interval'de app-offer edilir.
            () = tokio::time::sleep(Duration::from_nanos(
                t0.saturating_add(interval_ns.saturating_mul(next_submit))
                    .saturating_sub(elapsed_ns(start)),
            )), if next_submit < msg_count => {
                let i = usize::try_from(next_submit).unwrap_or(usize::MAX);
                if let Some(slot) = sent_ns.get_mut(i) {
                    *slot = Some(elapsed_ns(start));
                }
                next_submit += 1;
                offer(&tx, &payload).await?;
            }
            a = cli.acked() => {
                let Some(unit) = a else { break }; // endpoint öldü → deadline'a kadar beklemeye gerek yok
                // Teslim-teyidi ACK'ten de FEC-RECOVERED'dan da gelebilir; ikisi de aynı akış.
                // Warmup id'leri (unit < base_id) checked_sub ile YAPISAL olarak düşer.
                let Some(idx) = unit.checked_sub(base_id) else { continue };
                let Ok(i) = usize::try_from(idx) else { continue };
                if let Some(slot) = delivered_ns.get_mut(i) {
                    if slot.is_none() {
                        *slot = Some(elapsed_ns(start));
                        delivered_count += 1;
                    }
                }
            }
        }
    }

    let stats = cli.shutdown().await;
    let snap = gov.lock().map_or_else(
        |_| BudgetGovernor::new(cfg.budget).snapshot(),
        |g| g.snapshot(),
    );

    let events: Vec<RawEvent> = (0..n_msgs)
        .map(|i| RawEvent {
            msg_id: i as u64,
            sent_ns: sent_ns[i].unwrap_or(0),
            delivered_ns: delivered_ns[i],
            payload_bytes: cfg.payload_bytes,
        })
        .collect();

    // Teşhis satırı (SALT-GÖZLEM): FEC gerçekten çalıştı mı, tek bakışta görünsün.
    eprintln!(
        "KP-CLIENT delivered={delivered_count}/{msg_count} warm_ack={warm_acked} residue={warmup_residue} \
         tx={}B(data={} retx={} dup={} fec={} ctrl={}) rx={}B refused={}B/{}pkt \
         fec_blocks={} fec_repairs_tx={} fec_recovered_ctrl={} param_confirmed={} \
         seal_fail={} send_err={} stale={} cs_probe_tx={} cs_echo_rx={} \
         state={} left_preconv={} establish_tx={}",
        stats.tx_bytes,
        stats.tx_data_bytes,
        stats.tx_retx_bytes,
        stats.tx_dup_bytes,
        stats.tx_fec_bytes,
        stats.tx_ctrl_bytes,
        stats.rx_bytes,
        stats.egress_refused_bytes,
        stats.egress_refused_pkts,
        stats.fec_blocks_latched,
        stats.fec_repairs_tx,
        stats.fec_recovered_ctrl_units,
        stats.param_confirmed,
        stats.seal_failures,
        stats.send_errors,
        stats.stale_decisions,
        stats.cs_probes_tx,
        stats.cs_echoes_rx,
        stats.fate.sender_state,
        stats.fate.left_preconverge,
        stats.fate.establish_probes_tx,
    );

    let meta = RunMeta {
        profile: cfg.profile,
        cc: cfg.cc.label().to_string(),
        transport: "kama-product".to_string(),
        quinn_version: KAMA_VERSION.to_string(),
        seed: cfg.seed,
        repeat: cfg.repeat,
        msg_count,
        payload_bytes: cfg.payload_bytes,
        max_deadline_ns: u64::try_from(cfg.max_deadline.as_nanos()).unwrap_or(u64::MAX),
        udp_tx_bytes: stats.tx_bytes,
        udp_rx_bytes: stats.rx_bytes,
        tx: TxComponents {
            data: stats.tx_data_bytes,
            retx: stats.tx_retx_bytes,
            dup: stats.tx_dup_bytes,
            fec: stats.tx_fec_bytes,
            ctrl: stats.tx_ctrl_bytes,
        },
        // Ürün kolunda tail-recovery KAPATILAMAZ (G2 sevk default'u) — etiket bunu ve FEC
        // ayarını birlikte taşır; `--kama-arm` bu kolda YOK SAYILIR.
        // [KAPI-DÜZELTME] Etiket TÜM davranış-seçici bayrakları taşır. Önceden yalnız FEC ayarı
        // yazılıyordu → `--fec-burst-fusion` ve `--bootstrap-rtt` kolları metadata'da adaptif/
        // fec-off kollarıyla BYTE-ÖZDEŞ oluyordu ve provenans yalnız DİZİN ADINA dayanıyordu.
        // Dosya kopyalanınca/havuzlanınca kanıt tanımlanamaz hale gelirdi.
        kama_arm: format!(
            "kama-product-tailv1-{}{}{}{}{}",
            cfg.fec.label(),
            if cfg.burst_fusion { "-burstfusion" } else { "" },
            if cfg.bootstrap_rtt {
                "-bootstraprtt"
            } else {
                ""
            },
            if cfg.initial_window_bytes > 0 {
                format!("-iw{}", cfg.initial_window_bytes)
            } else {
                String::new()
            },
            if cfg.bootstrap_fast {
                "-fastladder"
            } else {
                ""
            },
        ),
        submit_interval_ns: interval_ns,
        budget: snap,
        t0_unix_ns,
        events_epoch_offset_ns: t0,
        warmup_residue,
    };
    Ok(RunOutput { meta, events })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Cc;
    use std::net::SocketAddr;

    fn cfg(server: SocketAddr, msgs: u64, deadline_ms: u64, fec: FecArm) -> ClientCfg {
        ClientCfg {
            server,
            msg_count: msgs,
            payload_bytes: 48,
            max_deadline: Duration::from_millis(deadline_ms),
            cc: Cc::Cubic,
            seed: 7,
            repeat: 0,
            profile: "loopback".to_string(),
            kama_arm: crate::transport::KamaArm::Arq, // bu kolda yok sayılır
            submit_interval: Duration::ZERO,
            budget: BudgetMode::Unlimited,
            warmup_msgs: 0,
            gate: None,
            establish_gate: None,
            fec,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
        }
    }

    fn srv_cfg(bind: SocketAddr, fec: FecArm) -> ServerCfg {
        ServerCfg {
            bind,
            cc: Cc::Cubic,
            quota_r: 4,
            budget: BudgetMode::Unlimited,
            gate_watch: None,
            fec,
            payload_bytes: 48,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
        }
    }

    /// FecArm CLI çözümü: 0 profil = kapalı; r=0 = adaptif; aksi statik.
    #[test]
    fn fec_arm_from_args_maps_all_three_modes() {
        assert_eq!(FecArm::from_args(0, 4), FecArm::Off);
        assert_eq!(FecArm::from_args(0, 0), FecArm::Off);
        assert_eq!(
            FecArm::from_args(0x03, 0),
            FecArm::Adaptive { profile_id: 0x03 }
        );
        assert_eq!(
            FecArm::from_args(0x03, 4),
            FecArm::StaticR {
                profile_id: 0x03,
                r: 4
            }
        );
    }

    /// `runtime_cfg` FEC ayarını ürün config'ine birebir taşır ve drenajı kapatır.
    #[test]
    fn runtime_cfg_carries_fec_and_disables_drain() {
        let off = runtime_cfg(48, 4, FecArm::Off, false, false, 0, false);
        assert!(off.fec.is_none());
        assert_eq!(off.max_unit_bytes, 48);
        assert_eq!(off.drain_cap, Duration::ZERO);

        let st = runtime_cfg(
            48,
            8,
            FecArm::StaticR {
                profile_id: 0x03,
                r: 4,
            },
            false,
            false,
            0,
            false,
        );
        let f = st.fec.expect("statik-r FEC config üretmeli");
        assert_eq!(f.profile_id, 0x03);
        assert_eq!(f.manual_r, Some(4));
        assert_eq!(st.quota_r, 8);

        let ad = runtime_cfg(
            48,
            4,
            FecArm::Adaptive { profile_id: 0x05 },
            false,
            false,
            0,
            false,
        );
        let f = ad.fec.expect("adaptif FEC config üretmeli");
        assert_eq!(f.profile_id, 0x05);
        assert_eq!(f.manual_r, None, "adaptif = Controller modu (manual_r yok)");
    }

    /// [S1B] Symbol'e sığmayan yük artık FRAGMENTER'a düşer, reddedilmez: profil 0x03'te chunk
    /// kapasitesi 53 B'tır; 256 B'lık benchmark yükü 53 B'lık chunk'lara bölünür ve mesaj
    /// tavanı 256 B olur. Fragmenter'dan önce bu kurulum koşu-öncesi typed REDDEDİLİYORDU —
    /// FEC ölçümlerini 48 B'lık yapay yüke hapseden duvar buydu.
    #[test]
    fn benchmark_payload_now_fragments_instead_of_being_refused() {
        let cap = fec_chunk_capacity(0x03).expect("profil 0x03 bilinmeli");
        assert_eq!(cap, 53, "E(64) − SOURCE_PREFIX_LEN(11)");

        let c = runtime_cfg(
            256,
            4,
            FecArm::StaticR {
                profile_id: 0x03,
                r: 4,
            },
            false,
            false,
            0,
            false,
        );
        assert_eq!(c.max_unit_bytes, 53, "birim symbol kapasitesine kilitlenir");
        assert_eq!(
            c.max_message_bytes,
            Some(256),
            "tam benchmark mesajı taşınır (fragmenter AÇIK)"
        );

        // Chunk'a sığan yükte fragmenter AÇILMAZ (gereksiz 11 B/chunk ödenmez).
        let small = runtime_cfg(
            48,
            4,
            FecArm::StaticR {
                profile_id: 0x03,
                r: 4,
            },
            false,
            false,
            0,
            false,
        );
        assert_eq!(small.max_unit_bytes, 48);
        assert_eq!(small.max_message_bytes, None, "48 ≤ 53 → fragmentasyon yok");
    }

    /// Ve gerçekten koşar: 256 B × 8 mesaj, FEC açık, uçtan uca TAM teslim + wire'da repair.
    #[tokio::test]
    async fn fec_arm_runs_at_the_256b_benchmark_payload_end_to_end() {
        let arm = FecArm::StaticR {
            profile_id: 0x03,
            r: 4,
        };
        let srv = ServerEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            runtime_cfg(256, 4, arm, false, false, 0, false),
            Box::new(IdentitySeam),
        )
        .await
        .expect("bind");
        let addr = srv.local_addr();
        let pump = tokio::spawn(async move {
            let mut srv = srv;
            while srv.next_delivery().await.is_some() {}
        });

        let mut c = cfg(addr, 8, 8_000, arm);
        c.payload_bytes = 256;
        let out = KamaProductTransport.run_client(c).await.expect("koşu");
        pump.abort();

        let delivered = out
            .events
            .iter()
            .filter(|e| e.delivered_ns.is_some())
            .count();
        assert_eq!(delivered, 8, "256 B mesajların hepsi teslim edilmeli");
        assert_eq!(out.meta.payload_bytes, 256);
        assert!(
            out.meta.tx.fec > 0,
            "256 B yükte de repair akmalı: {:?}",
            out.meta.tx
        );
    }

    /// Uçtan uca loopback: FEC KAPALI kolda ürün-runtime tüm birimleri teslim eder ve
    /// wire byte muhasebesi dolar (data > 0, toplam bileşenler = udp_tx_bytes).
    #[tokio::test]
    async fn loopback_delivers_all_units_with_fec_off() {
        // Not: `run_server` port-0 bind edip adresi DIŞARI vermez (harness sabit port verir), bu
        // yüzden testler endpoint'i doğrudan kurar; ölçülen kol yine client tarafıdır.
        let srv = ServerEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            runtime_cfg(48, 4, FecArm::Off, false, false, 0, false),
            Box::new(IdentitySeam),
        )
        .await
        .expect("bind");
        let addr = srv.local_addr();
        let pump = tokio::spawn(async move {
            let mut srv = srv;
            while srv.next_delivery().await.is_some() {}
        });

        let out = KamaProductTransport
            .run_client(cfg(addr, 20, 3_000, FecArm::Off))
            .await
            .expect("client koşusu");
        pump.abort();

        let delivered = out
            .events
            .iter()
            .filter(|e| e.delivered_ns.is_some())
            .count();
        assert_eq!(delivered, 20, "loopback'te kayıp yok — hepsi teslim olmalı");
        assert_eq!(out.meta.transport, "kama-product");
        assert!(
            out.meta.kama_arm.contains("tailv1"),
            "ürün kolu tail-ON etiketlemeli"
        );
        assert!(out.meta.kama_arm.contains("fec-off"));
        assert!(out.meta.tx.data > 0, "DATA byte'ı sayılmalı");
        assert_eq!(out.meta.tx.fec, 0, "FEC kapalıyken repair byte'ı olamaz");
        let sum = out.meta.tx.data
            + out.meta.tx.retx
            + out.meta.tx.dup
            + out.meta.tx.fec
            + out.meta.tx.ctrl;
        assert_eq!(
            sum, out.meta.udp_tx_bytes,
            "bileşen toplamı udp_tx_bytes'a eşit olmalı"
        );
    }

    /// **Sprint-A'nın asıl iddiası:** ürün kolunda FEC AÇIKKEN wire'da gerçek repair-shard vardır.
    /// Bu, [`crate::kama`] stub kolunda YAPISAL OLARAK imkânsızdı (kripto yok → parity emit yok).
    #[tokio::test]
    async fn fec_static_arm_actually_emits_repair_shards_on_the_wire() {
        let arm = FecArm::StaticR {
            profile_id: 0x03,
            r: 4,
        };
        let srv = ServerEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            runtime_cfg(48, 4, arm, false, false, 0, false),
            Box::new(IdentitySeam),
        )
        .await
        .expect("bind");
        let addr = srv.local_addr();
        let pump = tokio::spawn(async move {
            let mut srv = srv;
            while srv.next_delivery().await.is_some() {}
        });

        // 16 birim = profil 0x03'te (K=8) tam İKİ blok → latch iki kez kapanır.
        let out = KamaProductTransport
            .run_client(cfg(addr, 16, 5_000, arm))
            .await
            .expect("client koşusu");
        pump.abort();

        assert!(
            out.meta.tx.fec > 0,
            "FEC-static kolunda wire'da repair byte'ı OLMALI (stub kolun kapatamadığı boşluk); \
             ölçülen bileşenler: {:?}",
            out.meta.tx
        );
        assert!(
            out.meta.tx.ctrl > 0,
            "PARAM_SET bootstrap'i kontrol byte'ı üretmeli: {:?}",
            out.meta.tx
        );
    }

    /// TAM KOL uçtan uca: `run_server` + `run_client`, FEC AÇIK, ikisi de kendi egress
    /// kapısıyla. Harness'ın gerçekte koşturduğu yol budur (testler yukarıda endpoint'i
    /// doğrudan kurar; burada `BenchTransport` sözleşmesinin İKİ yarısı da koşar).
    #[tokio::test]
    async fn full_arm_server_and_client_run_end_to_end_with_fec() {
        let arm = FecArm::StaticR {
            profile_id: 0x03,
            r: 4,
        };
        // Boş port keşfi: bind → adres oku → bırak (harness sabit port verir, test kendi bulur).
        let probe = std::net::UdpSocket::bind("127.0.0.1:0").expect("probe bind");
        let addr: SocketAddr = probe.local_addr().expect("probe addr");
        drop(probe);

        let server =
            tokio::spawn(async move { KamaProductTransport.run_server(srv_cfg(addr, arm)).await });
        let out = KamaProductTransport
            .run_client(cfg(addr, 16, 5_000, arm))
            .await
            .expect("client koşusu");
        server.abort();

        let delivered = out
            .events
            .iter()
            .filter(|e| e.delivered_ns.is_some())
            .count();
        assert_eq!(delivered, 16, "temiz loopback'te tam teslim beklenir");
        assert!(
            out.meta.tx.fec > 0,
            "tam kolda da repair akmalı: {:?}",
            out.meta.tx
        );
        // Dönüş yolu gerçekten kullanıldı (server ACK üretti) — tek yönlü sahte-yeşil değil.
        assert!(out.meta.udp_rx_bytes > 0, "ACK dönüş yolu ölçülmeli");
    }

    /// Egress kapısı gerçekten kapı: sıfır-bütçe burst modunda arm sonrası HİÇBİR datagram
    /// enjekte edilmez (paket-atomik ret), ama motor durumu geri itilmez (INV-RATE-1).
    #[tokio::test]
    async fn zero_budget_refuses_every_datagram_after_arm() {
        let srv = ServerEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            runtime_cfg(48, 4, FecArm::Off, false, false, 0, false),
            Box::new(IdentitySeam),
        )
        .await
        .expect("bind");
        let addr = srv.local_addr();
        let pump = tokio::spawn(async move {
            let mut srv = srv;
            while srv.next_delivery().await.is_some() {}
        });

        let mut c = cfg(addr, 5, 700, FecArm::Off);
        c.budget = BudgetMode::Burst { cap_bytes: 0 };
        let out = KamaProductTransport.run_client(c).await.expect("koşu");
        pump.abort();

        assert_eq!(
            out.meta.budget.injected_bytes, 0,
            "sıfır bütçede enjeksiyon olamaz"
        );
        assert!(
            out.meta.budget.refused_pkts > 0,
            "reddedilen paketler sayılmalı: {:?}",
            out.meta.budget
        );
        assert!(
            out.meta.udp_tx_bytes > 0,
            "DENEME düzlemi reddedilenleri DE sayar (üç kolda ortak kural)"
        );
        assert_eq!(
            out.events
                .iter()
                .filter(|e| e.delivered_ns.is_some())
                .count(),
            0,
            "hiçbir şey enjekte edilmediyse teslim de olamaz"
        );
    }
}
