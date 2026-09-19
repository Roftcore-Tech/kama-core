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
    fec_chunk_capacity, ClientEndpoint, ClientSender, EgressPolicy, FecLossRiskConfig,
    FecRuntimeConfig, FecStructurePolicy, KamaV1Profile, LossConfirmHorizon, PersistMode,
    RuntimeConfig, RuntimeError, ServerEndpoint,
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
#[derive(Debug, Clone)]
struct ArmCfg {
    payload_bytes: u64,
    quota_r: u32,
    fec: FecArm,
    burst_fusion: bool,
    bootstrap_rtt: bool,
    initial_window_bytes: u64,
    bootstrap_fast: bool,
    loss_confirm_k: u32,
    fec_risk: bool,
    ts_sample: bool,
    spec_l1: Option<u32>,
    spec_l2: Option<u32>,
    gate: Option<u32>,
    persist_mode: Option<String>,
    persist_alpha: Option<u32>,
    persist_rho: Option<u32>,
    persist_q: Option<u32>,
    rp_suppress: Option<u32>,
}

fn runtime_cfg(a: ArmCfg) -> RuntimeConfig {
    let ArmCfg {
        payload_bytes,
        quota_r,
        fec,
        burst_fusion,
        bootstrap_rtt,
        initial_window_bytes,
        bootstrap_fast,
        loss_confirm_k,
        fec_risk,
        ts_sample,
        spec_l1,
        spec_l2,
        gate,
        persist_mode,
        persist_alpha,
        persist_rho,
        persist_q,
        rp_suppress,
    } = a;
    let payload = usize::try_from(payload_bytes).unwrap_or(usize::MAX);
    let profile_id = match fec {
        FecArm::Off => None,
        FecArm::StaticR { profile_id, .. }
        | FecArm::Adaptive { profile_id }
        | FecArm::PolicyV2Deadline { profile_id, .. } => Some(profile_id),
    };
    // Bilinmeyen profil: birim tavanını kısıtlama — `validate_fec` koşu-öncesi typed reddedecek.
    // FEC açıkken birim tavanı = profilin symbol kapasitesi (ÜRÜN PROFİLİYLE AYNI); payload daha
    // küçükse davranış değişmez (mesaj tek chunk kalır) ama config v1'den SAPMAZ → manifest
    // client_core::runtime::TRANSPORT_V1_PROFILE_ID diyebilir. FEC kapalıyken birim = payload (kol zaten v1 değil).
    let unit = profile_id.and_then(fec_chunk_capacity).unwrap_or(payload);
    // [DENETİM B-5] TEK DOĞRULUK KAYNAĞI: ürün v1 profilinden başla, YALNIZ açık override uygula.
    // Eskiden burada `RuntimeConfig { .. }` elle kuruluyordu; ürün default'ları (IW 2400,
    // bounded_v1 risk kolu, yapı-gate 250) benchmark'a HİÇ ulaşmıyordu ve "sevk" etiketli kol
    // ürünün sevk etmediği bir yapılandırmayı ölçüyordu.
    let v1 = KamaV1Profile::runtime_config();
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
        // [IW] 0 = ÜRÜN PROFİLİ değeri (2400). Eskiden 0 = "ilk uçuş tek paket" idi; o değer
        // ürünün CLAMP HATASI diye düzelttiği davranıştı ve `kp-sevk` kolunu ürün-dışı yapıyordu.
        initial_window_bytes: if initial_window_bytes == 0 {
            v1.initial_window_bytes
        } else {
            initial_window_bytes
        },
        // [KURULUŞ] merdiven seçimi (bootstrap_rtt kapalıyken etkisiz).
        bootstrap_fast_ladder: bootstrap_fast,
        drain_cap: Duration::ZERO,
        fec: match fec {
            FecArm::Off => None,
            FecArm::StaticR { profile_id, r } => Some(FecRuntimeConfig {
                profile_id,
                manual_r: Some(r),
            }),
            FecArm::Adaptive { profile_id } | FecArm::PolicyV2Deadline { profile_id, .. } => {
                Some(FecRuntimeConfig {
                    profile_id,
                    manual_r: None,
                })
            }
        },
        // [ONAY UFKU] Ölçüm kolu: 0 = sevk davranışı (byte-özdeş), >0 = k×srtt per-mille.
        loss_confirm_horizon: if loss_confirm_k == 0 {
            LossConfirmHorizon::CadenceLegacy
        } else {
            LossConfirmHorizon::ChannelDerived {
                k_permille: loss_confirm_k,
            }
        },
        // [FEC-RISK] false = ÜRÜN PROFİLİ (`bounded_v1`: A 250-250-250 + P2 + C4 guard'ı G1);
        // true = kampanya kolu (`experimental_v1`, knob override'larıyla). Eskiden false
        // `FecLossRiskConfig::default()` (KAPALI) demekti → ürün kolu spekülatif kolu hiç koşmazdı.
        fec_loss_risk: {
            let mut c = if fec_risk {
                FecLossRiskConfig::experimental_v1()
            } else {
                v1.fec_loss_risk
            };
            // [R-SWEEP] tavan override'ları (None = [CAL] default'lar: 125/250).
            if let Some(v) = spec_l1 {
                c.bootstrap_cap_permille = v;
            }
            if let Some(v) = spec_l2 {
                c.persistent_cap_permille = v;
            }
            // [PERSIST] ufuk modu ve knob'ları (None = sevk default'ları).
            if let Some(m) = persist_mode.as_deref() {
                c.persist_mode = if m == "learned" {
                    PersistMode::LearnedSpuriousAge
                } else {
                    PersistMode::FixedSrtt
                };
            }
            if let Some(v) = persist_alpha {
                c.persist_srtt_permille = v;
            }
            if let Some(v) = persist_rho {
                c.persist_rho_permille = v;
            }
            if let Some(v) = persist_q {
                c.persist_q_permille = v;
            }
            // [C4 G1] reorder-baskısı guard eşiği (None = kapalı = sevk).
            if let Some(v) = rp_suppress {
                c.reorder_suppress_permille = v;
            }
            c
        },
        // [R-SWEEP] gate seviyesi; None = ÜRÜN PROFİLİ değeri (250). Eskiden None = motor
        // `fec_med` (500) idi → bench ürün-dışı bir parity seviyesi ölçüyordu.
        structure_gate_level_permille: gate.or(v1.structure_gate_level_permille),
        // [HANDOFF ENSTRÜMANI] 50 ms zaman serisi (salt gözlem); None = kapalı.
        ts_sample_period_ns: ts_sample.then_some(50_000_000),
        // [DEADLINE-EKSENİ] Politika kolu kalibrasyonu KURAR; diğer kollarda `None` (yapı-gate
        // koşar, sevk davranışı). Politika fail-safe'tir: bütçe bildirilmezse ArqOnly.
        fusion_policy: matches!(fec, FecArm::PolicyV2Deadline { .. })
            .then(adaptive_transport::fusion_policy::FusionPolicy::calibration_v2_deadline),
        ..RuntimeConfig::default()
    }
}

// ── [DENETİM] MİMARİ-AKTİF MANİFEST ────────────────────────────────────────────────────────────
// Her koşu, NE koştuğunu ve NEYİN fiilen ateşlendiğini kendisi beyan eder. Amacı tek bir sınıf
// hatayı imkânsız kılmak: bir ARQ-only alt-kümesini "Kama tam mimarisi" diye etiketlemek.
// Kıyas raporları `profile=` alanını AYNEN taşımak zorundadır (`kama-v1-full` dışındaki hiçbir
// koşu "Kama v1" adıyla raporlanamaz).

/// Yürürlükteki config'in v1 ürün profilinden sapmaları (boş liste = tam v1 profili).
fn profile_delta(c: &RuntimeConfig) -> Vec<String> {
    let v = KamaV1Profile::runtime_config();
    let mut d = Vec::new();
    if c.max_unit_bytes != v.max_unit_bytes {
        d.push(format!(
            "max_unit_bytes {} != {}",
            c.max_unit_bytes, v.max_unit_bytes
        ));
    }
    if c.max_message_bytes != v.max_message_bytes {
        d.push(format!(
            "max_message_bytes {:?} != {:?}",
            c.max_message_bytes, v.max_message_bytes
        ));
    }
    if c.initial_window_bytes != v.initial_window_bytes {
        d.push(format!(
            "initial_window_bytes {} != {}",
            c.initial_window_bytes, v.initial_window_bytes
        ));
    }
    match (c.fec, v.fec) {
        (Some(a), Some(b)) if a.profile_id == b.profile_id && a.manual_r == b.manual_r => {}
        (a, b) => d.push(format!(
            "fec {:?} != {:?}",
            a.map(|f| (f.profile_id, f.manual_r)),
            b.map(|f| (f.profile_id, f.manual_r))
        )),
    }
    if c.structure_gate_level_permille != v.structure_gate_level_permille {
        d.push(format!(
            "structure_gate_level_permille {:?} != {:?}",
            c.structure_gate_level_permille, v.structure_gate_level_permille
        ));
    }
    if c.fec_loss_risk.enabled != v.fec_loss_risk.enabled
        || c.fec_loss_risk.bootstrap_cap_permille != v.fec_loss_risk.bootstrap_cap_permille
        || c.fec_loss_risk.persistent_cap_permille != v.fec_loss_risk.persistent_cap_permille
        || c.fec_loss_risk.reorder_suppress_permille != v.fec_loss_risk.reorder_suppress_permille
        || c.fec_loss_risk.persist_mode != v.fec_loss_risk.persist_mode
    {
        d.push(format!(
            "fec_loss_risk(enabled={} l1={} l2={} rp_suppress={} persist={:?}) != v1(enabled={} l1={} l2={} rp_suppress={} persist={:?})",
            c.fec_loss_risk.enabled, c.fec_loss_risk.bootstrap_cap_permille,
            c.fec_loss_risk.persistent_cap_permille, c.fec_loss_risk.reorder_suppress_permille,
            c.fec_loss_risk.persist_mode,
            v.fec_loss_risk.enabled, v.fec_loss_risk.bootstrap_cap_permille,
            v.fec_loss_risk.persistent_cap_permille, v.fec_loss_risk.reorder_suppress_permille,
            v.fec_loss_risk.persist_mode
        ));
    }
    if c.bootstrap_rtt != v.bootstrap_rtt {
        d.push(format!(
            "bootstrap_rtt {} != {}",
            c.bootstrap_rtt, v.bootstrap_rtt
        ));
    }
    if c.fusion_policy.is_some() != v.fusion_policy.is_some() {
        d.push("fusion_policy farkı".to_string());
    }
    if c.fec_structure_policy != v.fec_structure_policy {
        d.push(format!(
            "fec_structure_policy {:?} != {:?}",
            c.fec_structure_policy, v.fec_structure_policy
        ));
    }
    if !matches!(
        (c.loss_confirm_horizon, v.loss_confirm_horizon),
        (
            LossConfirmHorizon::CadenceLegacy,
            LossConfirmHorizon::CadenceLegacy
        )
    ) {
        d.push(format!(
            "loss_confirm_horizon {:?} != {:?}",
            c.loss_confirm_horizon, v.loss_confirm_horizon
        ));
    }
    d
}

/// Koşu ÖNCESİ beyan: profil kimliği, sapmalar, açık/kapalı mekanizmalar ve kapalı olma nedenleri.
fn print_manifest(role: &str, c: &RuntimeConfig) {
    let delta = profile_delta(c);
    let profile = if delta.is_empty() {
        client_core::runtime::TRANSPORT_V1_PROFILE_ID
    } else {
        "kama-custom"
    };
    let fec = c.fec.map_or_else(
        || "OFF(subsystem yok)".to_string(),
        |f| match f.manual_r {
            None => format!("ON adaptive p{:#04x}", f.profile_id),
            Some(r) => format!("ON static-r{r} p{:#04x}", f.profile_id),
        },
    );
    eprintln!(
        "KP-MANIFEST role={role} profile={profile} commit={} unit={}B msg_cap={:?} \
         fec={fec} fec_risk={} spec_caps={}/{} rp_suppress={} persist={:?} gate={:?} \
         iw={}B arq=ON(selective+dup) tail=ON(validated_v1) establish_probe=ON \
         bootstrap_ladder={} fusion_policy={} structure_policy={:?} confirm_horizon={:?} \
         features=[diag-sched-telemetry={}]",
        option_env!("KAMA_GIT_SHA").unwrap_or("unknown"),
        c.max_unit_bytes,
        c.max_message_bytes,
        c.fec_loss_risk.enabled,
        c.fec_loss_risk.bootstrap_cap_permille,
        c.fec_loss_risk.persistent_cap_permille,
        c.fec_loss_risk.reorder_suppress_permille,
        c.fec_loss_risk.persist_mode,
        c.structure_gate_level_permille,
        c.initial_window_bytes,
        if c.bootstrap_rtt {
            "ON"
        } else {
            "OFF(kalibre-değil)"
        },
        if c.fusion_policy.is_some() {
            "ON"
        } else {
            "OFF(opt-in ölçüm kapısı)"
        },
        c.fec_structure_policy,
        c.loss_confirm_horizon,
        cfg!(feature = "diag-sched-telemetry"),
    );
    for x in &delta {
        eprintln!("KP-MANIFEST role={role} DELTA-vs-v1: {x}");
    }
    if delta.is_empty() {
        eprintln!(
            "KP-MANIFEST role={role} DELTA-vs-v1: (yok — bu koşu 'Kama v1' adıyla raporlanabilir)"
        );
    }
}

/// Koşu SONRASI beyan: hangi mekanizmalar FİİLEN ateşledi (sayaçtan; performanstan çıkarım YOK).
fn print_activation(stats: &client_core::runtime::ClientStats) {
    let f = &stats.fate;
    let sched = f.sched;
    let tail = if cfg!(feature = "diag-sched-telemetry") {
        format!("{}", sched[11])
    } else {
        "n/a(diag-sched-telemetry gerekli)".to_string()
    };
    eprintln!(
        "KP-ACTIVATED arq_retx={}B dup={}B fec_parity={}B fec_blocks={} fec_repairs_tx={} \
         spec_levels={:?} arbiter={:?} gate_opened={} ledger_peak_ppm={} spurious={} \
         establish_probe={} bootstrap_probe={} tail_probe={} rtt_samples={} state={} \
         data_denied={} retx_denied={} preconv_parity_blocked={} \
         return_path=[bw={}B/s ack_int={}ms silence={}ms cont={}‰ fwd_loss={}‰ ret_impair={}‰ ret_ack_loss={}‰ dpa={} dir_ev={} conf={}‰ {:?} n={}/{}] fec2=[repair_not_arqed={} clamp={} would_zero={} partial_blocks={}]\n\
         KP-ACTIVATION cfg=[tail={} fec={}] fired=[tail_probe={}@{}ms tail_cancel={} parity={} parity_denied={} retx={} dup={}] \
         reach=[transitions={} recover_arq={} recover_fwd={} probe_up={} left_preconv={}ms] \
         est=[btlbw={}B/s srtt={}us rtprop={}us qdelay={}us pace={}B/s tokens={} inflight={}B]",
        stats.tx_retx_bytes,
        stats.tx_dup_bytes,
        stats.tx_fec_bytes,
        stats.fec_blocks_latched,
        stats.fec_repairs_tx,
        f.fec_spec_levels,
        f.fec_arbiter_reasons,
        f.structure_gate_opened,
        f.ledger_loss_ppm_peak,
        f.spurious_units,
        f.establish_probes_tx,
        stats.cs_probes_tx,
        tail,
        f.rtt_samples,
        f.sender_state,
        sched[1],
        sched[4],
        f.preconverge_parity_blocked,
        f.return_path.measured_return_bw_bps,
        f.return_path.ack_interval_ns / 1_000_000,
        f.return_path.silence_ns / 1_000_000,
        f.return_path.feedback_continuity_permille,
        f.return_path.forward_loss_permille,
        f.return_path.return_impairment_permille,
        f.return_path.return_ack_loss_permille,
        f.return_path.datagrams_per_ack_x100 / 100,
        f.return_path.direction_evidence,
        f.return_path.confidence_permille,
        f.return_path.health,
        f.return_path.ack_samples,
        f.return_path.v2_samples,
        f.fec_completeness.0,
        f.fec_completeness.1,
        f.fec_completeness.2,
        stats.fec_partial_blocks_closed,
        f.activation.tail_configured,
        f.activation.fec_configured,
        f.activation.tail_probes,
        f.activation.tail_first_ns / 1_000_000,
        f.activation.tail_cancelled,
        f.activation.parity_emitted,
        f.activation.parity_token_denied,
        f.activation.retransmits,
        f.activation.dups,
        f.activation.state_transitions,
        f.activation.recover_arq_entries,
        f.activation.recover_fwd_entries,
        f.activation.probe_up_entries,
        f.activation.left_preconverge_ns / 1_000_000,
        f.activation.btlbw_bps,
        f.activation.srtt_us,
        f.activation.rtprop_us,
        f.activation.queue_delay_us,
        f.activation.pacer_rate_bps,
        f.activation.pacer_tokens,
        f.activation.inflight_bytes,
    );
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
async fn offer(
    tx: &ClientSender,
    payload: &[u8],
    deadline_ns: Option<u64>,
) -> Result<(), BenchError> {
    loop {
        match tx.send_with_deadline(payload.to_vec(), deadline_ns) {
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
    let rt_cfg = runtime_cfg(ArmCfg {
        payload_bytes: cfg.payload_bytes,
        quota_r: cfg.quota_r,
        fec: cfg.fec,
        burst_fusion: cfg.burst_fusion,
        bootstrap_rtt: cfg.bootstrap_rtt,
        initial_window_bytes: cfg.initial_window_bytes,
        bootstrap_fast: cfg.bootstrap_fast,
        loss_confirm_k: cfg.loss_confirm_k,
        fec_risk: cfg.fec_risk,
        ts_sample: cfg.ts_out.is_some(),
        spec_l1: cfg.spec_l1_permille,
        spec_l2: cfg.spec_l2_permille,
        gate: cfg.gate_permille,
        persist_mode: cfg.persist_mode.clone(),
        persist_alpha: cfg.persist_alpha,
        persist_rho: cfg.persist_rho,
        persist_q: cfg.persist_q,
        rp_suppress: cfg.rp_suppress,
    });
    print_manifest("server", &rt_cfg);
    let mut srv =
        ServerEndpoint::bind_with_policy(cfg.bind, rt_cfg, Box::new(IdentitySeam), Some(policy))
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
    let rt_cfg = runtime_cfg(ArmCfg {
        payload_bytes: cfg.payload_bytes,
        quota_r: 4,
        fec: cfg.fec,
        burst_fusion: cfg.burst_fusion,
        bootstrap_rtt: cfg.bootstrap_rtt,
        initial_window_bytes: cfg.initial_window_bytes,
        bootstrap_fast: cfg.bootstrap_fast,
        loss_confirm_k: cfg.loss_confirm_k,
        fec_risk: cfg.fec_risk,
        ts_sample: cfg.ts_out.is_some(),
        spec_l1: cfg.spec_l1_permille,
        spec_l2: cfg.spec_l2_permille,
        gate: cfg.gate_permille,
        persist_mode: cfg.persist_mode.clone(),
        persist_alpha: cfg.persist_alpha,
        persist_rho: cfg.persist_rho,
        persist_q: cfg.persist_q,
        rp_suppress: cfg.rp_suppress,
    });
    print_manifest("client", &rt_cfg);
    // [FAZ-4 §1.3] PROFİL KİMLİĞİ — koddan türetilir (rt_cfg taşınmadan ÖNCE), operatör
    // disiplinine bırakılmaz. Tek bir alan bile saparsa kimlik `kama-custom` olur.
    let profile_delta_v = profile_delta(&rt_cfg);
    let profile_id_label = if profile_delta_v.is_empty() {
        client_core::runtime::TRANSPORT_V1_PROFILE_ID.to_string()
    } else {
        "kama-custom".to_string()
    };
    // [FAZ-4 §M4] KURULUM MALİYETİ AYRI ÖLÇÜLÜR. Kama'da el sıkışma yoktur; bu süre soket
    // bağlama + endpoint kurulumudur. Ölçüm-t0 aşağıda warmup/gate SONRASINDA konur → kurulum
    // completion penceresinin DIŞINDADIR (üç kolun tamamıyla aynı). Sayı gizlenmez, raporlanır.
    let setup_t0 = elapsed_ns(start);
    let mut cli = ClientEndpoint::connect_with_policy(
        cfg.server,
        rt_cfg,
        Box::new(IdentitySeam),
        Some(policy),
    )
    .await
    .map_err(|e| rt_err("connect", e))?;
    let setup_ns = elapsed_ns(start).saturating_sub(setup_t0);
    // Offer tutamağı AYRI: `acked()` `&mut cli` ödünç alır, sunum aynı `select!` içinde sürsün.
    let tx = cli.sender();
    // [DEADLINE-EKSENİ] Politika kolunda her offer teslim bütçesini bildirir; diğer kollarda
    // `None` → politika zaten kapalı, wire byte-özdeş.
    let deadline_ns: Option<u64> = match cfg.fec {
        FecArm::PolicyV2Deadline { deadline_ns, .. } => Some(deadline_ns),
        _ => None,
    };

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
                    offer(&tx, &payload, deadline_ns).await?;
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
            offer(&tx, &payload, deadline_ns).await?;
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
                offer(&tx, &payload, deadline_ns).await?;
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
         state={} left_preconv={} establish_tx={} \
         deadline_ns={:?} fusion={:?} last_obs={:?} bw_range={:?} loss_range={:?} ledger_peak={} ledger_now={} gate_opened={} ledger_int={:?} confirm_path={:?} tick_span={:?} spurious={} confirm_k={:?} latch={:?} suppress={:?} risk_t={:?} risk_ppm={:?} flow_end={} arb={:?} lvl={:?} spec={:?} persist={:?} em_l1={:?} em_l2={:?} sched={:?}",
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
        // [DEADLINE-EKSENİ] Politika kolunda NULL sonuç teşhis edilebilir olmalı: bütçe motora
        // ulaştı mı, ve politika hangi kovada reddetti. Bu iki alan olmadan "politika FEC açmadı"
        // gözlemi ile "politika hiç koşmadı" gözlemi AYIRT EDİLEMEZ (vakum-sayaç tuzağı).
        stats.fate.declared_deadline_ns,
        stats.fate.fusion_actions,
        stats.fate.last_fusion_obs,
        stats.fate.fusion_bw_range,
        stats.fate.fusion_loss_range,
        stats.fate.ledger_loss_ppm_peak,
        stats.fate.ledger_loss_ppm,
        stats.fate.structure_gate_opened,
        stats.fate.ledger_internals,
        stats.fate.ledger_confirm_path,
        stats.fate.ledger_tick_span,
        stats.fate.spurious_units,
        cfg.loss_confirm_k,
        stats.fate.fec_latch_diag,
        stats.fate.fec_last_suppression,
        stats.fate.fec_risk_times,
        stats.fate.fec_risk_ppm,
        stats.flow_end_ns,
        stats.fate.fec_arbiter_reasons,
        stats.fate.fec_spec_levels,
        stats.fate.fec_spec_signals,
        stats.fate.fec_persist,
        stats.fate.fec_spec_transitions.0,
        stats.fate.fec_spec_transitions.1,
        stats.fate.sched,
    );
    print_activation(&stats);
    // [HANDOFF ENSTRÜMANI] zaman serisini CSV'ye dök (varsa).
    if let Some(path) = cfg.ts_out.as_ref() {
        let mut csv = String::from(
            "t_ms,instant_ppm,persistent_ppm,rp_permille,confirmed_ppm,confirmed_prop,structure_prop,spec_prop,final_ratio,reason_idx,level_idx,evidence_mass,declared_units,pacer_tokens,pending_parity,pending_lost,data_denied,retx_denied,parity_denied,retx_emitted,parity_emitted,data_emitted\n",
        );
        for (t, x) in &stats.ts {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                t / 1_000_000,
                x.instant_ppm,
                x.persistent_ppm,
                x.reorder_pressure_permille,
                x.confirmed_ppm,
                x.confirmed_prop,
                x.structure_prop,
                x.spec_prop,
                x.final_ratio,
                x.reason_idx,
                x.level_idx,
                x.evidence_mass,
                x.declared_units,
                x.pacer_tokens,
                x.pending_parity,
                x.pending_lost,
                x.sched_data_denied,
                x.sched_retx_denied,
                x.sched_parity_denied,
                x.sched_retx_emitted,
                x.sched_parity_emitted,
                x.sched_data_emitted
            ));
        }
        if let Err(e) = std::fs::write(path, csv) {
            eprintln!("ts-out yazılamadı: {e}");
        }
    }

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
        // [FAZ-4] Ateşleme özeti manifeste taşınır: "yapılandırıldı" ile "ateşledi" bir daha
        // karıştırılamaz (denetim: "FEC enabled" denip hiç ateşlememiş olması).
        activation: Some(crate::manifest::ActivationSummary {
            tail_configured: stats.fate.activation.tail_configured,
            fec_configured: stats.fate.activation.fec_configured,
            tail_probes: stats.fate.activation.tail_probes,
            tail_cancelled: stats.fate.activation.tail_cancelled,
            parity_emitted: stats.fate.activation.parity_emitted,
            parity_token_denied: stats.fate.activation.parity_token_denied,
            retransmits: stats.fate.activation.retransmits,
            dups: stats.fate.activation.dups,
            state_transitions: stats.fate.activation.state_transitions,
            recover_arq_entries: stats.fate.activation.recover_arq_entries,
            left_preconverge_ns: stats.fate.activation.left_preconverge_ns,
            btlbw_bps: stats.fate.activation.btlbw_bps,
            srtt_us: stats.fate.activation.srtt_us,
            rtprop_us: stats.fate.activation.rtprop_us,
            return_ack_loss_permille: stats.fate.return_path.return_ack_loss_permille,
            return_forward_loss_permille: stats.fate.return_path.forward_loss_permille,
            bytes_data: stats.tx_data_bytes,
            bytes_retx: stats.tx_retx_bytes,
            bytes_parity: stats.tx_fec_bytes,
        }),
        setup_ns,
        setup_in_window: false,
        profile_id: profile_id_label.clone(),
        profile_delta: profile_delta_v.clone(),
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
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            establish_gate: None,
            fec,
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
        }
    }

    /// [FAZ-4 §22] RESMÎ KIYAS YOLU KANONİK CONSTRUCTOR'I KULLANIR.
    ///
    /// Denetim bulgusu: "sevk profili" etiketli kol ürünün sevk etmediği bir yapılandırmayı
    /// ölçmüştü. Bu test benchmark'ın bayraksız yolunun `KamaV1Profile::runtime_config()`
    /// ile ALAN ALAN aynı olduğunu kilitler — sapma listesi boş olmalıdır.
    #[test]
    fn bayraksiz_kiyas_yolu_kanonik_profildir() {
        let bayraksiz = ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            // BAYRAKSIZ CLI değeri: `--fec-profile` ürün sabitine bağlı, `--fec-mode` adaptif.
            // `FecArm::Off` DEĞİL — o, FEC'i açıkça kapatan bir koldur.
            fec: FecArm::from_args(
                KamaV1Profile::FEC_PROFILE_ID,
                0,
                crate::transport::FecModeArg::Adaptive,
                0,
            ),
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0, // 0 = "verilmedi" → ürün değeri
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false, // bayrak yok → bounded_v1
            ts_sample: false,
            spec_l1: None,
            spec_l2: None,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
        };
        let c = runtime_cfg(bayraksiz);
        let d = profile_delta(&c);
        assert!(d.is_empty(), "bayraksız yol ÜRÜN profili DEĞİL: {d:?}");
        // Ve kimliğin kendisi de kanonik etiketi almalı.
        // `FecRuntimeConfig` PartialEq türetmiyor (transport crate'ine dokunmamak için burada
        // alan alan kıyaslıyoruz — Faz-4 transport kodunu DEĞİŞTİRMEZ).
        let (a, b) = (
            c.fec.expect("ürün profilinde FEC var"),
            KamaV1Profile::runtime_config()
                .fec
                .expect("ürün profilinde FEC var"),
        );
        assert_eq!((a.profile_id, a.manual_r), (b.profile_id, b.manual_r));
        assert_eq!(
            c.initial_window_bytes,
            KamaV1Profile::INITIAL_WINDOW_BYTES,
            "iw=0 CLAMP hatasıydı; ürün değerine düşmeli"
        );
    }

    /// [FAZ-4 §22] TEK alan sapması sapma listesine YANSIR (kimlik `kama-custom` olur).
    #[test]
    fn tek_alan_sapmasi_gorunur() {
        let mut sapan = ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::from_args(
                KamaV1Profile::FEC_PROFILE_ID,
                0,
                crate::transport::FecModeArg::Adaptive,
                0,
            ),
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 1200, // ← TEK sapma
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            spec_l1: None,
            spec_l2: None,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
        };
        let d = profile_delta(&runtime_cfg(sapan.clone()));
        assert_eq!(d.len(), 1, "yalnız bir sapma beklenirdi: {d:?}");
        assert!(d[0].contains("initial_window_bytes"), "{d:?}");
        // Yapı-gate seviyesinin sapması da görünür.
        sapan.initial_window_bytes = 0;
        sapan.gate = Some(500);
        let d2 = profile_delta(&runtime_cfg(sapan));
        assert!(
            d2.iter()
                .any(|x| x.contains("structure_gate_level_permille")),
            "{d2:?}"
        );
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
        }
    }

    /// [KAMPANYA-KRİTİK] FecArm CLI çözümü: mod seçicisi `r`'den ORTOGONAL.
    ///
    /// Eski biçimde `r == 0` "adaptif" demekti ve **statik r=0 kolu TEMSİL EDİLEMİYORDU**.
    /// Sonuç: tek-eksenli kontrol için `--fec-profile 0` kullanmak gerekiyordu, o da fragmenter'ı
    /// kapatıp birim granülünü (256 B vs 53 B) ikinci bir eksen hâline getiriyordu — kampanyanın
    /// kontrol etmesi gereken confound'un ta kendisi. Adversaryal tasarım incelemesinin
    /// kill-shot'ı buydu; koşudan ÖNCE yakalandı.
    #[test]
    fn static_r0_control_arm_is_reachable_and_keeps_fragmentation() {
        use crate::transport::FecModeArg::{Adaptive, Static};
        // Profil 0 = FEC tamamen kapalı (fragmenter de kapalı) — kontrol kolu BU DEĞİL.
        assert_eq!(FecArm::from_args(0, 4, Static, 0), FecArm::Off);
        assert_eq!(FecArm::from_args(0, 0, Adaptive, 0), FecArm::Off);
        // Mod ORTOGONAL: aynı r ile iki farklı kol.
        assert_eq!(
            FecArm::from_args(0x03, 0, Adaptive, 0),
            FecArm::Adaptive { profile_id: 0x03 }
        );
        assert_eq!(
            FecArm::from_args(0x03, 0, Static, 0),
            FecArm::StaticR {
                profile_id: 0x03,
                r: 0
            },
            "statik r=0 ULAŞILABİLİR olmalı — tek-eksenli kontrol kolu budur"
        );
        assert_eq!(
            FecArm::from_args(0x03, 4, Static, 0),
            FecArm::StaticR {
                profile_id: 0x03,
                r: 4
            }
        );

        // VE kritik özellik: r=0 kolu fragmentasyonu KORUR, yani DATA çerçeveleme FEC
        // kollarıyla byte-özdeş kalır; tek fark parity çerçevelerinin varlığıdır.
        let ctrl = runtime_cfg(ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::from_args(0x03, 0, Static, 0),
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        let r2 = runtime_cfg(ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::from_args(0x03, 2, Static, 0),
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        assert_eq!(
            ctrl.max_unit_bytes, r2.max_unit_bytes,
            "birim granülü AYNI olmalı"
        );
        assert_eq!(
            ctrl.max_message_bytes, r2.max_message_bytes,
            "fragmentasyon AYNI olmalı"
        );
        assert!(
            ctrl.fec.is_some(),
            "profil kurulu kalmalı (fragmentasyon için)"
        );
        assert_eq!(
            ctrl.fec.unwrap().manual_r,
            Some(0),
            "kontrol kolunda parity YOK"
        );

        // Kıyas: profil 0 kullanılsaydı granül kayardı — kaçınılan confound.
        let naive_off = runtime_cfg(ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::Off,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        assert_ne!(
            naive_off.max_unit_bytes, r2.max_unit_bytes,
            "profil-0 kontrolü birim granülünü kaydırır (bu yüzden kullanılmaz)"
        );
    }

    /// [R-SWEEP] Tavan/gate override'ları RuntimeConfig'e akar; None = default'lar (125/250/None).
    #[test]
    fn rsweep_caps_flow_to_runtime() {
        let base = ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::Off,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: true,
            ts_sample: false,
            spec_l1: None,
            spec_l2: None,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
        };
        let d = runtime_cfg(base.clone());
        assert_eq!(d.fec_loss_risk.bootstrap_cap_permille, 125);
        assert_eq!(d.fec_loss_risk.persistent_cap_permille, 250);
        // [DENETİM B-5] gate override yokken ÜRÜN PROFİLİ değeri gelir (eskiden None → motor 500).
        assert_eq!(d.structure_gate_level_permille, Some(250));
        let o = runtime_cfg(ArmCfg {
            spec_l1: Some(250),
            spec_l2: Some(375),
            gate: Some(250),
            ..base.clone()
        });
        assert_eq!(o.fec_loss_risk.bootstrap_cap_permille, 250);
        assert_eq!(o.fec_loss_risk.persistent_cap_permille, 375);
        assert_eq!(o.structure_gate_level_permille, Some(250));
    }

    /// [HANDOFF ENSTRÜMANI] `ts_sample` → 50 ms örnekleme periyodu; kapalıyken `None`.
    /// (Smoke kapısı bunun ÜRÜN yolundaki kablosunu ayrıca doğrular: ts.csv > 10 satır.)
    #[test]
    fn ts_sample_maps_to_50ms_period() {
        let base = ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::Off,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        };
        assert_eq!(runtime_cfg(base.clone()).ts_sample_period_ns, None);
        assert_eq!(
            runtime_cfg(ArmCfg {
                ts_sample: true,
                gate: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                rp_suppress: None,
                spec_l2: None,
                spec_l1: None,
                ..base
            })
            .ts_sample_period_ns,
            Some(50_000_000)
        );
    }

    /// `runtime_cfg` FEC ayarını ürün config'ine birebir taşır ve drenajı kapatır.
    #[test]
    fn runtime_cfg_carries_fec_and_disables_drain() {
        let off = runtime_cfg(ArmCfg {
            payload_bytes: 48,
            quota_r: 4,
            fec: FecArm::Off,
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        assert!(off.fec.is_none());
        assert_eq!(off.max_unit_bytes, 48);
        assert_eq!(off.drain_cap, Duration::ZERO);

        let st = runtime_cfg(ArmCfg {
            payload_bytes: 48,
            quota_r: 8,
            fec: FecArm::StaticR {
                profile_id: 0x03,
                r: 4,
            },
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        let f = st.fec.expect("statik-r FEC config üretmeli");
        assert_eq!(f.profile_id, 0x03);
        assert_eq!(f.manual_r, Some(4));
        assert_eq!(st.quota_r, 8);

        let ad = runtime_cfg(ArmCfg {
            payload_bytes: 48,
            quota_r: 4,
            fec: FecArm::Adaptive { profile_id: 0x05 },
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
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

        let c = runtime_cfg(ArmCfg {
            payload_bytes: 256,
            quota_r: 4,
            fec: FecArm::StaticR {
                profile_id: 0x03,
                r: 4,
            },
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        assert_eq!(c.max_unit_bytes, 53, "birim symbol kapasitesine kilitlenir");
        assert_eq!(
            c.max_message_bytes,
            Some(256),
            "tam benchmark mesajı taşınır (fragmenter AÇIK)"
        );

        // Chunk'a sığan yükte fragmenter AÇILMAZ (gereksiz 11 B/chunk ödenmez).
        let small = runtime_cfg(ArmCfg {
            payload_bytes: 48,
            quota_r: 4,
            fec: FecArm::StaticR {
                profile_id: 0x03,
                r: 4,
            },
            burst_fusion: false,
            bootstrap_rtt: false,
            initial_window_bytes: 0,
            bootstrap_fast: false,
            loss_confirm_k: 0,
            fec_risk: false,
            ts_sample: false,
            gate: None,
            persist_mode: None,
            persist_alpha: None,
            persist_rho: None,
            persist_q: None,
            rp_suppress: None,
            spec_l2: None,
            spec_l1: None,
        });
        // [DENETİM B-5] Birim tavanı artık profilin symbol kapasitesi (53), payload değil —
        // davranış aynı (48 B mesaj tek chunk), ama config ürün profilinden sapmaz.
        assert_eq!(small.max_unit_bytes, 53);
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
            runtime_cfg(ArmCfg {
                payload_bytes: 256,
                quota_r: 4,
                fec: arm,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_sample: false,
                gate: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                rp_suppress: None,
                spec_l2: None,
                spec_l1: None,
            }),
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
            runtime_cfg(ArmCfg {
                payload_bytes: 48,
                quota_r: 4,
                fec: FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_sample: false,
                gate: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                rp_suppress: None,
                spec_l2: None,
                spec_l1: None,
            }),
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
            runtime_cfg(ArmCfg {
                payload_bytes: 48,
                quota_r: 4,
                fec: arm,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_sample: false,
                gate: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                rp_suppress: None,
                spec_l2: None,
                spec_l1: None,
            }),
            Box::new(IdentitySeam),
        )
        .await
        .expect("bind");
        let addr = srv.local_addr();
        // Pump durdurulabilir olmalı: sunucu istatistiği (param_bound_peers) koşu sonunda okunur.
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        let pump = tokio::spawn(async move {
            let mut srv = srv;
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    d = srv.next_delivery() => { if d.is_none() { break; } }
                }
            }
            srv.shutdown().await
        });

        // 16 birim = profil 0x03'te (K=8) tam İKİ blok → latch iki kez kapanır.
        // Sunum 1 ms aralıklı: akış ≥ 15 ms yaşar → ilk `rtprop` tick'ini (≈1 ms) KESİN görür.
        // Gerekçe aşağıdaki [KÖK NEDEN] notunda.
        let mut ccfg = cfg(addr, 16, 5_000, arm);
        ccfg.submit_interval = Duration::from_millis(1);
        let out = KamaProductTransport
            .run_client(ccfg)
            .await
            .expect("client koşusu");
        let _ = stop_tx.send(());
        let srv_stats = pump.await.expect("pump görevi");

        assert!(
            out.meta.tx.fec > 0,
            "FEC-static kolunda wire'da repair byte'ı OLMALI (stub kolun kapatamadığı boşluk); \
             ölçülen bileşenler: {:?}",
            out.meta.tx
        );
        // [KÖK NEDEN — 2026-09-19, hosted CI'da iki kez yakalandı; İLK TEŞHİSİM YANLIŞTI]
        // Eski assertion `out.meta.tx.ctrl > 0` idi. İlk teşhis "PARAM_SET `mark_t0`'dan önce
        // gidiyor, sayaç sıfırlanıyor" idi ve assertion sunucu-tarafı `param_bound_peers == 1`
        // ile değiştirildi. O da CI'da düştü (bound=0, rejects=0, param_confirmed=0) → PARAM t0'dan
        // önce gitmiyordu, HİÇ gitmiyordu. Ölçülen mekanizma: PARAM_SET istemci döngüsünün
        // `rtprop` interval'inin ilk tick'inde gönderilir; tokio zamanlayıcı çarkı 1 ms
        // çözünürlüklüdür ve deadline'ı yukarı yuvarlar → "anında" ilk tick current_thread
        // runtime'da 0.93–1.07 ms sonra ateşler (tokio 1.52.3, 200/200 ölçüm). Burst sunumda akış
        // CI'da 0.34 ms / 0.97 ms'de bitti (yerelde 51–82 ms, 60/60 geçer) → ana döngü PARAM
        // gönderilmeden çıktı; drain döngüsünde `rtprop` kolu yoktur. Bu, DONMUŞ runtime'ın
        // belgelenen bir özelliğidir (KNOWN-LIMITATIONS), burada DEĞİŞTİRİLMEZ; test, iddiasının
        // ön koşulunu (akış ilk tick'ten uzun yaşar) sunum aralığıyla deterministik kurar.
        // İddia: PARAM_SET telden geçti, sunucu hash'i doğrulayıp eşi BAĞLADI (t0 ile sıfırlanmaz).
        assert_eq!(
            srv_stats.param_bound_peers, 1,
            "PARAM_SET telden geçmeli ve sunucuda doğrulanıp bağlanmalı; rejects={} istemci tx={:?}",
            srv_stats.param_rejects, out.meta.tx
        );
        assert_eq!(srv_stats.param_rejects, 0);
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

        // [FAZ-4 §22] AKTİVASYON MANİFESTİ FAZ-3 TANILARINI GERÇEKTEN TÜKETİR.
        // "yapılandırıldı ≠ ateşledi" ayrımı manifeste TAŞINMALI; alanlar vakum olmamalı.
        let act = out
            .meta
            .activation
            .as_ref()
            .expect("aktivasyon manifesti YOK");
        assert!(act.fec_configured, "bu kolda FEC yapılandırılmıştı");
        assert!(act.tail_configured, "tail ürün profilinde AÇIK");
        assert!(act.bytes_data > 0, "veri byte'ı taşınmalı: {act:?}");
        assert_eq!(
            act.bytes_parity, out.meta.tx.fec,
            "parity byte'ı iki kaynakta AYNI olmalı (tanı ile wire sayacı ayrışamaz)"
        );
        assert_eq!(act.bytes_retx, out.meta.tx.retx);
        // [FAZ-4 §1.3] KİMLİK CANLI KOŞUDA DA DOĞRU: bu kol statik r=4 / p0x03 kullanır,
        // yani ÜRÜN PROFİLİ DEĞİLDİR. Manifest bunu `kama-custom` olarak etiketlemeli ve
        // sapmayı adıyla saymalı — "sevk profili" diye raporlanması imkânsız olsun.
        assert_eq!(
            out.meta.profile_id, "kama-custom",
            "statik-r kolu ürün profili DEĞİL"
        );
        assert!(
            out.meta.profile_delta.iter().any(|d| d.contains("fec")),
            "sapma FEC alanında görünmeli: {:?}",
            out.meta.profile_delta
        );
    }

    /// Egress kapısı gerçekten kapı: sıfır-bütçe burst modunda arm sonrası HİÇBİR datagram
    /// enjekte edilmez (paket-atomik ret), ama motor durumu geri itilmez (INV-RATE-1).
    #[tokio::test]
    async fn zero_budget_refuses_every_datagram_after_arm() {
        let srv = ServerEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            runtime_cfg(ArmCfg {
                payload_bytes: 48,
                quota_r: 4,
                fec: FecArm::Off,
                burst_fusion: false,
                bootstrap_rtt: false,
                initial_window_bytes: 0,
                bootstrap_fast: false,
                loss_confirm_k: 0,
                fec_risk: false,
                ts_sample: false,
                gate: None,
                persist_mode: None,
                persist_alpha: None,
                persist_rho: None,
                persist_q: None,
                rp_suppress: None,
                spec_l2: None,
                spec_l1: None,
            }),
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
