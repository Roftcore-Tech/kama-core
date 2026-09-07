//! Kama transport benchmark harness CLI (§4.4). SoC: `serve`/`client` HAM event üretir;
//! `metrics` HESAP yapar (transport'a dokunmaz). quinn baseline (§4.4.1) + kama stub.

mod budget;
mod error;
mod event;
mod kama;
mod kama_product;
mod metrics;
mod qdgram;
mod quic;
mod transport;

use clap::{Parser, Subcommand, ValueEnum};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::BenchError;
use crate::transport::{BenchTransport, Cc, ClientCfg, FecArm, FecModeArg, KamaArm, ServerCfg};

#[derive(Parser)]
#[command(name = "bench", about = "Kama transport benchmark harness (§4.4)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Transport server'ı çalıştır.
    Serve {
        #[arg(long, default_value = "[::]:4433")]
        bind: SocketAddr,
        #[arg(long, value_enum, default_value_t = CcArg::Cubic)]
        cc: CcArg,
        #[arg(long, value_enum, default_value_t = TransportArg::Quinn)]
        transport: TransportArg,
        /// [TUR-5 r3] Epizot-kotası R [CAL {4,8}] (kama receiver).
        #[arg(long, default_value_t = 4)]
        quota_r: u32,
        /// [KAPI-1 W2] Reverse-yön burst hard-cap (IP byte; 0 = yok). Paced ile birlikte VERİLMEZ.
        #[arg(long, default_value_t = 0)]
        budget_burst_bytes: u64,
        /// [KAPI-1 W2] Reverse-yön paced refill (IP byte/s; 0 = paced yok).
        #[arg(long, default_value_t = 0)]
        budget_refill_bytes_per_s: u64,
        /// [KAPI-1 W2] Paced kova (IP byte; refill>0 iken ZORUNLU).
        #[arg(long, default_value_t = 0)]
        budget_bucket_bytes: u64,
        /// [KAPI-1 W2] Paced kümülatif tavan (IP byte; 0 = tavansız).
        #[arg(long, default_value_t = 0)]
        budget_ceiling_bytes: u64,
        /// [KAPI-1 W6] Reverse-bütçe arm senkronu: `<path>.go` belirene dek arm etme.
        #[arg(long)]
        gate_watch: Option<PathBuf>,
        /// [SPRINT-A] FEC profil kimliği (0 = FEC kapalı). `kama-product` dışında yok sayılır.
        /// İSTEMCİYLE AYNI verilmelidir. Örn. 3 = K8/R4/E64 (payload ≤ 53 B).
        #[arg(long, default_value_t = 0)]
        fec_profile: u8,
        /// [SPRINT-A] Blok başına sabit repair sayısı. `--fec-mode static` ile birlikte
        /// **r=0 GEÇERLİDİR** ve tek-eksenli kontrol kolunu verir (fragmentasyon açık, parity yok).
        #[arg(long, default_value_t = 0)]
        fec_r: u16,
        /// [KAMPANYA-KRİTİK] FEC modu: `static` (r kadar repair, r=0 = kontrol) | `adaptive`
        /// (Controller). `r`'den ORTOGONAL — eski `r==0 ⇒ adaptif` kısayolu statik-r0 kolunu
        /// temsil edilemez kılıyordu.
        #[arg(long, value_enum, default_value_t = FecModeCli::Static)]
        fec_mode: FecModeCli,
        /// [DEADLINE-EKSENİ] `--fec-mode policy` kolunda her offer'la bildirilen teslim bütçesi
        /// (ms). Kalibre politika kararını bu bütçenin srtt'ye ORANINDAN verir; bildirilmezse
        /// (0) `NoDeadlineDeclared` ile ArqOnly'ye düşer — sessiz varsayılan YOK.
        #[arg(long, default_value_t = 0)]
        fec_deadline_ms: u64,
        /// [ONAY UFKU] Kayıp-defteri `Suspected → Confirmed` ufku: `0` = sevk davranışı
        /// (cadence-legacy, ölçüldü ki HİÇ ateşlemiyor), `>0` = kanal-türevli `k × srtt`
        /// per-mille (`2000` = 2×srtt). ÖLÇÜM KOLU.
        #[arg(long, default_value_t = 0)]
        loss_confirm_k: u32,
        /// [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi (Suspected olaylara yaş-ağırlıklı
        /// güven). KAPALI = sevk davranışı. TEK tüketici FEC oranı kararıdır.
        #[arg(long, default_value_t = false)]
        fec_risk: bool,
        /// [HANDOFF ENSTRÜMANI] Zaman-serisi CSV çıktısı (client): t_ms,instant,persistent,rp,
        /// confirmed,cprop,sprop,specprop,final,reason,level. 50 ms örnekleme, ≤4096 satır.
        #[arg(long)]
        ts_out: Option<std::path::PathBuf>,
        /// [R-SWEEP] Spekülatif L1 tavanı (‰; K=8'de 125=r1, 250=r2, 375=r3). None = default 125.
        #[arg(long)]
        spec_l1_permille: Option<u32>,
        /// [R-SWEEP] Spekülatif L2 tavanı (‰). None = default 250.
        #[arg(long)]
        spec_l2_permille: Option<u32>,
        /// [R-SWEEP] Yapı-gate seviyesi (‰; 500 = sevk r4). None = motor default (fec_med).
        #[arg(long)]
        gate_permille: Option<u32>,
        /// [SPRINT-A] Alıcı birim-payload tavanı — client `--payload` ile AYNI olmalı.
        #[arg(long, default_value_t = 256)]
        payload: u64,
        /// [BURST-FUSION] Yapı-gate'in burst dalında da kodlama açmasına izin ver (opt-in;
        /// KALİBRE EDİLMEMİŞ). Verilmezse sevk davranışı (burst → parity yok). İKİ UÇTA AYNI.
        #[arg(long, default_value_t = false)]
        fec_burst_fusion: bool,
        /// [DR-CS-3] Cold-start bootstrap-RTT merdivenini aç (opt-in). İKİ UÇTA AYNI.
        #[arg(long, default_value_t = false)]
        bootstrap_rtt: bool,
        /// [IW] PreConverge ilk-uçuş penceresi (byte; 0 = sevk davranışı, tek paket).
        #[arg(long, default_value_t = 0)]
        initial_window_bytes: u64,
        /// [KURULUŞ] Bootstrap merdivenini kuruluş-hızlı yap (0.2/0.6/1.8/5.4 s). Bütçe değişmez.
        #[arg(long, default_value_t = false)]
        bootstrap_fast: bool,
    },
    /// Client: N mesaj gönder, ham event JSONL yaz.
    Client {
        #[arg(long)]
        server: SocketAddr,
        #[arg(long, default_value_t = 100)]
        msgs: u64,
        #[arg(long, default_value_t = 256)]
        payload: u64,
        #[arg(long, default_value_t = 120)]
        max_deadline_s: u64,
        #[arg(long, value_enum, default_value_t = CcArg::Cubic)]
        cc: CcArg,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[arg(long, default_value_t = 0)]
        repeat: u32,
        #[arg(long, default_value = "default")]
        profile: String,
        #[arg(long, value_enum, default_value_t = TransportArg::Quinn)]
        transport: TransportArg,
        /// Kama kolu: arq (defaults, tail OFF) | tail-v1 (KAMA-TAIL-V1). quinn'de yok sayılır. [KAPI-1 W4]
        #[arg(long, value_enum, default_value_t = KamaArmArg::Arq)]
        kama_arm: KamaArmArg,
        /// Mesaj sunum aralığı ms (0 = burst; >0 = paced, mesaj i t=i×interval'de). [KAPI-1 W3]
        #[arg(long, default_value_t = 0)]
        submit_interval_ms: u64,
        /// [KAPI-1 W2] Forward-yön burst hard-cap (IP byte; 0 = yok). Paced ile birlikte VERİLMEZ.
        #[arg(long, default_value_t = 0)]
        budget_burst_bytes: u64,
        /// [KAPI-1 W2] Forward-yön paced refill (IP byte/s; 0 = paced yok).
        #[arg(long, default_value_t = 0)]
        budget_refill_bytes_per_s: u64,
        /// [KAPI-1 W2] Paced kova (IP byte; refill>0 iken ZORUNLU).
        #[arg(long, default_value_t = 0)]
        budget_bucket_bytes: u64,
        /// [KAPI-1 W2] Paced kümülatif tavan (IP byte; 0 = tavansız).
        #[arg(long, default_value_t = 0)]
        budget_ceiling_bytes: u64,
        /// [KAPI-1 W6] Ölçüm-öncesi warmup mesajı (0 = yok; ölçüme/bütçeye girmez).
        #[arg(long, default_value_t = 0)]
        warmup_msgs: u64,
        /// [KAPI-1 W6] Barrier gate yolu (`<gate>.ready` yazılır, `<gate>.go` beklenir).
        #[arg(long)]
        gate: Option<PathBuf>,
        /// [KAPI-1 W6] Kuruluş-gate'i (bağlantı kurulunca ready; netem uygulanınca go → warmup).
        #[arg(long)]
        establish_gate: Option<PathBuf>,
        /// [SPRINT-A] FEC profil kimliği (0 = FEC kapalı). `kama-product` dışında yok sayılır.
        /// SUNUCUYLA AYNI verilmelidir. Örn. 3 = K8/R4/E64 (payload ≤ 53 B).
        #[arg(long, default_value_t = 0)]
        fec_profile: u8,
        /// [SPRINT-A] Blok başına sabit repair sayısı. `--fec-mode static` ile birlikte
        /// **r=0 GEÇERLİDİR** ve tek-eksenli kontrol kolunu verir (fragmentasyon açık, parity yok).
        #[arg(long, default_value_t = 0)]
        fec_r: u16,
        /// [KAMPANYA-KRİTİK] FEC modu: `static` (r kadar repair, r=0 = kontrol) | `adaptive`
        /// (Controller). `r`'den ORTOGONAL — eski `r==0 ⇒ adaptif` kısayolu statik-r0 kolunu
        /// temsil edilemez kılıyordu.
        #[arg(long, value_enum, default_value_t = FecModeCli::Static)]
        fec_mode: FecModeCli,
        /// [DEADLINE-EKSENİ] `--fec-mode policy` kolunda her offer'la bildirilen teslim bütçesi
        /// (ms). Kalibre politika kararını bu bütçenin srtt'ye ORANINDAN verir; bildirilmezse
        /// (0) `NoDeadlineDeclared` ile ArqOnly'ye düşer — sessiz varsayılan YOK.
        #[arg(long, default_value_t = 0)]
        fec_deadline_ms: u64,
        /// [ONAY UFKU] Kayıp-defteri `Suspected → Confirmed` ufku: `0` = sevk davranışı
        /// (cadence-legacy, ölçüldü ki HİÇ ateşlemiyor), `>0` = kanal-türevli `k × srtt`
        /// per-mille (`2000` = 2×srtt). ÖLÇÜM KOLU.
        #[arg(long, default_value_t = 0)]
        loss_confirm_k: u32,
        /// [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi (Suspected olaylara yaş-ağırlıklı
        /// güven). KAPALI = sevk davranışı. TEK tüketici FEC oranı kararıdır.
        #[arg(long, default_value_t = false)]
        fec_risk: bool,
        /// [HANDOFF ENSTRÜMANI] Zaman-serisi CSV çıktısı (client): t_ms,instant,persistent,rp,
        /// confirmed,cprop,sprop,specprop,final,reason,level. 50 ms örnekleme, ≤4096 satır.
        #[arg(long)]
        ts_out: Option<std::path::PathBuf>,
        /// [R-SWEEP] Spekülatif L1 tavanı (‰; K=8'de 125=r1, 250=r2, 375=r3). None = default 125.
        #[arg(long)]
        spec_l1_permille: Option<u32>,
        /// [R-SWEEP] Spekülatif L2 tavanı (‰). None = default 250.
        #[arg(long)]
        spec_l2_permille: Option<u32>,
        /// [R-SWEEP] Yapı-gate seviyesi (‰; 500 = sevk r4). None = motor default (fec_med).
        #[arg(long)]
        gate_permille: Option<u32>,
        /// [BURST-FUSION] Bkz. `serve --fec-burst-fusion`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, default_value_t = false)]
        fec_burst_fusion: bool,
        /// [DR-CS-3] Bkz. `serve --bootstrap-rtt`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, default_value_t = false)]
        bootstrap_rtt: bool,
        /// [IW] PreConverge ilk-uçuş penceresi (byte; 0 = sevk davranışı, tek paket).
        #[arg(long, default_value_t = 0)]
        initial_window_bytes: u64,
        /// [KURULUŞ] Bkz. `serve --bootstrap-fast`. SUNUCUYLA AYNI verilmelidir.
        #[arg(long, default_value_t = false)]
        bootstrap_fast: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Ham JSONL koşularından metrik hesapla (A: varyans, B: deadline eğrisi).
    Metrics {
        #[arg(long, num_args = 1.., required = true)]
        runs: Vec<PathBuf>,
        #[arg(long, value_delimiter = ',', default_value = "10,30,120")]
        deadlines_s: Vec<u64>,
    },
}

/// CLI karşılığı — `transport::FecModeArg`'a birebir eşlenir.
#[derive(Copy, Clone, ValueEnum, PartialEq)]
enum FecModeCli {
    Static,
    Adaptive,
    /// [DEADLINE-EKSENİ] Kalibre füzyon politikası karar verir; `--fec-deadline-ms` zorunlu.
    Policy,
}

impl From<FecModeCli> for FecModeArg {
    fn from(m: FecModeCli) -> Self {
        match m {
            FecModeCli::Static => FecModeArg::Static,
            FecModeCli::Adaptive => FecModeArg::Adaptive,
            FecModeCli::Policy => FecModeArg::Policy,
        }
    }
}

#[derive(Copy, Clone, ValueEnum)]
enum CcArg {
    Cubic,
    Bbr,
}

impl From<CcArg> for Cc {
    fn from(c: CcArg) -> Cc {
        match c {
            CcArg::Cubic => Cc::Cubic,
            CcArg::Bbr => Cc::Bbr,
        }
    }
}

#[derive(Copy, Clone, ValueEnum, PartialEq)]
enum TransportArg {
    Quinn,
    /// Security-stub sürücüsü (FEC parity YAPISAL olarak emit edilemez — bkz. src/kama.rs).
    Kama,
    /// [KAPI-1 W1] quinn DATAGRAM + app-ARQ (Q-DGRAM-ARQ, FAZ-1 primary comparator).
    Qdgram,
    /// [SPRINT-A] GERÇEK ÜRÜN runtime'ı (client-core): FEC+ARQ füzyonu, tail-recovery,
    /// fuzzy kontrolcü, PARAM bootstrap, handshake-siz kuruluş.
    KamaProduct,
}

/// [KAPI-1 W4] CLI karşılığı — `transport::KamaArm`'a birebir eşlenir.
#[derive(Copy, Clone, ValueEnum)]
enum KamaArmArg {
    Arq,
    TailV1,
}

/// [KAPI-1 W2] CLI bütçe argları → `BudgetMode` (doğrulamalı: burst XOR paced; sessiz yanlış
/// kombinasyon YOK — kampanya-config hatası koşudan önce patlar).
fn budget_mode(
    burst: u64,
    refill: u64,
    bucket: u64,
    ceiling: u64,
) -> Result<budget::BudgetMode, BenchError> {
    match (burst, refill) {
        (0, 0) if bucket == 0 && ceiling == 0 => Ok(budget::BudgetMode::Unlimited),
        (0, 0) => Err(BenchError::Other(
            "bucket/ceiling yalnız --budget-refill-bytes-per-s ile anlamlı".into(),
        )),
        (b, 0) => {
            if bucket != 0 || ceiling != 0 {
                return Err(BenchError::Other(
                    "burst ve paced bütçe birlikte verilemez (r5 D.1 workload-ayrık)".into(),
                ));
            }
            Ok(budget::BudgetMode::Burst { cap_bytes: b })
        }
        (0, r) => {
            if bucket == 0 {
                return Err(BenchError::Other(
                    "paced bütçe --budget-bucket-bytes ister (D6: 2 s × refill)".into(),
                ));
            }
            Ok(budget::BudgetMode::Paced {
                refill_bytes_per_s: r,
                bucket_bytes: bucket,
                ceiling_bytes: if ceiling == 0 { u64::MAX } else { ceiling },
            })
        }
        _ => Err(BenchError::Other(
            "burst ve paced bütçe birlikte verilemez (r5 D.1 workload-ayrık)".into(),
        )),
    }
}

impl From<KamaArmArg> for KamaArm {
    fn from(a: KamaArmArg) -> KamaArm {
        match a {
            KamaArmArg::Arq => KamaArm::Arq,
            KamaArmArg::TailV1 => KamaArm::TailV1,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), BenchError> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve {
            bind,
            cc,
            transport,
            quota_r,
            budget_burst_bytes,
            budget_refill_bytes_per_s,
            budget_bucket_bytes,
            budget_ceiling_bytes,
            gate_watch,
            fec_profile,
            fec_r,
            fec_mode,
            fec_deadline_ms,
            loss_confirm_k,
            fec_risk,
            ts_out: _,
            spec_l1_permille,
            spec_l2_permille,
            gate_permille,
            payload,
            fec_burst_fusion,
            bootstrap_rtt,
            initial_window_bytes,
            bootstrap_fast,
        } => {
            run_server(
                transport,
                ServerCfg {
                    bind,
                    cc: cc.into(),
                    quota_r,
                    budget: budget_mode(
                        budget_burst_bytes,
                        budget_refill_bytes_per_s,
                        budget_bucket_bytes,
                        budget_ceiling_bytes,
                    )?,
                    gate_watch,
                    fec: FecArm::from_args(fec_profile, fec_r, fec_mode.into(), fec_deadline_ms),
                    loss_confirm_k,
                    fec_risk,
                    ts_out: None,
                    gate_permille,
                    spec_l2_permille,
                    spec_l1_permille,
                    payload_bytes: payload,
                    burst_fusion: fec_burst_fusion,
                    bootstrap_rtt,
                    initial_window_bytes,
                    bootstrap_fast,
                },
            )
            .await
        }
        Cmd::Client {
            server,
            msgs,
            payload,
            max_deadline_s,
            cc,
            seed,
            repeat,
            profile,
            transport,
            kama_arm,
            submit_interval_ms,
            budget_burst_bytes,
            budget_refill_bytes_per_s,
            budget_bucket_bytes,
            budget_ceiling_bytes,
            warmup_msgs,
            gate,
            establish_gate,
            fec_profile,
            fec_r,
            fec_mode,
            fec_deadline_ms,
            loss_confirm_k,
            fec_risk,
            ts_out,
            spec_l1_permille,
            spec_l2_permille,
            gate_permille,
            fec_burst_fusion,
            bootstrap_rtt,
            initial_window_bytes,
            bootstrap_fast,
            out,
        } => {
            let cfg = ClientCfg {
                server,
                msg_count: msgs,
                payload_bytes: payload,
                max_deadline: Duration::from_secs(max_deadline_s),
                cc: cc.into(),
                seed,
                repeat,
                profile,
                kama_arm: kama_arm.into(),
                submit_interval: Duration::from_millis(submit_interval_ms),
                budget: budget_mode(
                    budget_burst_bytes,
                    budget_refill_bytes_per_s,
                    budget_bucket_bytes,
                    budget_ceiling_bytes,
                )?,
                warmup_msgs,
                gate,
                establish_gate,
                fec: FecArm::from_args(fec_profile, fec_r, fec_mode.into(), fec_deadline_ms),
                loss_confirm_k,
                fec_risk,
                ts_out,
                gate_permille,
                spec_l2_permille,
                spec_l1_permille,
                burst_fusion: fec_burst_fusion,
                bootstrap_rtt,
                initial_window_bytes,
                bootstrap_fast,
            };
            let output = run_client(transport, cfg).await?;
            output.write_jsonl(&out)?;
            eprintln!("wrote {} events → {}", output.events.len(), out.display());
            Ok(())
        }
        Cmd::Metrics { runs, deadlines_s } => {
            let deadlines: Vec<Duration> = deadlines_s
                .iter()
                .map(|&s| Duration::from_secs(s))
                .collect();
            // Eksik/bozuk koşu dosyasında ABORT etme (workflow bulgusu #4: harsh profilde bir
            // handshake fail'i dosya yazmaz → tüm agregasyon ölmemeli). Atla + uyar.
            let outputs: Vec<_> = runs
                .iter()
                .filter_map(|p| match event::RunOutput::read_jsonl(p) {
                    Ok(o) => Some(o),
                    Err(e) => {
                        eprintln!("skip {}: {e}", p.display());
                        None
                    }
                })
                .collect();
            let per_run: Vec<_> = outputs
                .iter()
                .map(|o| metrics::run_metrics(o, &deadlines))
                .collect();
            let agg = metrics::aggregate(&per_run, &deadlines);
            println!(
                "# completion vs deadline — {} koşu (A: varyans, B: eğri)",
                per_run.len()
            );
            for a in &agg {
                println!(
                    "deadline={:>4}s  median={:.3}  IQR=[{:.3},{:.3}]  min={:.3}  max={:.3}  (n={})",
                    a.deadline.as_secs(),
                    a.completion_median,
                    a.completion_q1,
                    a.completion_q3,
                    a.completion_min,
                    a.completion_max,
                    a.runs,
                );
            }
            let sec = metrics::aggregate_secondary(&per_run);
            println!(
                "# secondary (§4.4.3): bytes/delivered-msg median={:.1}  ACK-overhead median={:.3}",
                sec.bytes_per_delivered_msg_median, sec.ack_overhead_ratio_median,
            );
            Ok(())
        }
    }
}

async fn run_server(t: TransportArg, cfg: ServerCfg) -> Result<(), BenchError> {
    match t {
        TransportArg::Quinn => quic::QuinnTransport.run_server(cfg).await,
        TransportArg::Kama => transport::KamaTransport.run_server(cfg).await,
        TransportArg::Qdgram => qdgram::QdgramTransport.run_server(cfg).await,
        TransportArg::KamaProduct => kama_product::KamaProductTransport.run_server(cfg).await,
    }
}

async fn run_client(t: TransportArg, cfg: ClientCfg) -> Result<event::RunOutput, BenchError> {
    match t {
        TransportArg::Quinn => quic::QuinnTransport.run_client(cfg).await,
        TransportArg::Kama => transport::KamaTransport.run_client(cfg).await,
        TransportArg::Qdgram => qdgram::QdgramTransport.run_client(cfg).await,
        TransportArg::KamaProduct => kama_product::KamaProductTransport.run_client(cfg).await,
    }
}
