//! Transport soyutlaması (SoC seam): quinn baseline ([`crate::quic`]) + kama stub. Kama
//! ileride adaptive-transport engine loop'una bağlanacak (ADIM 3+); şimdi typed-error stub.

use crate::error::BenchError;
use crate::event::RunOutput;
use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

/// Congestion control seçimi (§4.4.1: Cubic default vs Bbr experimental).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cc {
    /// quinn default, production.
    Cubic,
    /// quinn `Experimental! Use at your own risk.` (kaynak notu).
    Bbr,
}

impl Cc {
    /// results/rapor etiketi.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Cc::Cubic => "cubic",
            Cc::Bbr => "bbr",
        }
    }
}

/// Kama kol seçimi (KAPI-1 §0 / W4). `Arq` = `Sender::new()` defaults (tail OFF); `TailV1` =
/// KAMA-TAIL-V1 (`TailRecoveryConfig::validated_v1()` — 2290-satır grid'in koştuğu adlandırılmış
/// sabit, post-hoc tuning yok). quinn kollarında yok sayılır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KamaArm {
    /// Ürün defaults'ları (tail recovery OFF).
    #[default]
    Arq,
    /// Bounded tail recovery, doğrulanmış v1 konfigürasyonuyla ON.
    TailV1,
}

impl KamaArm {
    /// Rapor/manifest etiketi (RunMeta.kama_arm).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Arq => "kama-arq",
            Self::TailV1 => "kama-tail-v1",
        }
    }
}

/// [SPRINT-A] Ürün-runtime kolunun (`kama-product`) FEC ayarı. Diğer kollarda YOK SAYILIR.
///
/// İKİ UÇTA AYNI kurulmalıdır: blok/ESI ataması `unit_id`'den deterministik türetilir ve alıcı
/// PARAM_SET hash'iyle fail-closed doğrular — uyumsuz kurulum shard'ların düşürülmesine yol açar
/// (sessiz bozulma değil, sayaçlı ret).
/// [KAMPANYA-KRİTİK] FEC aktüasyon modu — `r`'den ORTOGONAL seçici. Bkz. [`FecArm::from_args`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FecModeArg {
    /// Statik-r (`r` kadar repair; **r=0 geçerli ve tek-eksenli kontrol koludur**).
    #[default]
    Static,
    /// Controller/adaptif (yapı-gate + fuzzy; `r` yok sayılır).
    Adaptive,
    /// [DEADLINE-EKSENİ] Kalibre füzyon politikası (`calibration_v2_deadline`) karar otoritesi;
    /// yapı-gate SUSAR, `r` yok sayılır. Bu kol ÜRÜN POLİTİKASINI ölçer — v1/v2 kampanyaları
    /// statik-r ile politikayı BYPASS ediyordu ve raporun "cevaplanmadı" listesinin ilk maddesi
    /// tam olarak buydu.
    Policy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FecArm {
    /// FEC KAPALI. Ürün sevk default'u: kol yalnız ARQ + tail-recovery + fuzzy pacing koşar.
    #[default]
    Off,
    /// Statik-r: her blokta sabit `r` repair (`FecActuationMode::Manual` + manuel-close).
    /// Operatör-forced FEC / kampanyaların STATIC_R kolu.
    StaticR {
        /// `kama_fec_codec::by_id` profil kimliği (ör. 0x03 = K8/R4/E64).
        profile_id: u8,
        /// Blok başına repair sayısı (1..=`r_code_max`).
        r: u16,
    },
    /// Adaptif: `FecActuationMode::Controller` — yapı-gate (Q18-fix) kararı latch'e akar.
    Adaptive {
        /// `kama_fec_codec::by_id` profil kimliği.
        profile_id: u8,
    },
    /// [DEADLINE-EKSENİ] Kalibre politika kolu: `FusionPolicy::calibration_v2_deadline()` karar
    /// verir (yapı-gate susar) ve istemci her offer'da teslim bütçesini BİLDİRİR.
    PolicyV2Deadline {
        /// `kama_fec_codec::by_id` profil kimliği.
        profile_id: u8,
        /// Bildirilecek teslim bütçesi (ns). Kampanya bunu RTT'nin katı olarak verir.
        deadline_ns: u64,
    },
}

impl FecArm {
    /// CLI argümanlarından çözer.
    ///
    /// **[KAMPANYA-KRİTİK]** `mode` seçicisi `r`'den ORTOGONALDİR. Eski iki-argüman biçiminde
    /// `r == 0` "adaptif" demekti ve **statik r=0 kolu TEMSİL EDİLEMİYORDU**: `--fec-profile 0`
    /// fragmenter'ı da kapatıyor (`runtime_cfg`'de profil yoksa `unit = payload`), dolayısıyla
    /// FEC-kapalı kol 256 B birim, FEC-açık kol 53 B chunk kullanıyor ve **birim granülü ikinci
    /// bir eksen olarak kayıyordu** — tam da kontrol edilmesi gereken confound.
    ///
    /// `StaticR { r: 0 }` geçerli ve anlamlıdır: `close_fec_block_manual`'ın `for esi in 0..r`
    /// döngüsü r=0'da no-op'tur ve `Ok(0)` döner → parity YOK, ama profil kurulu olduğu için
    /// fragmentasyon ve wire çerçevesi (0x05 + `SourcePrefix`) FEC kollarıyla BYTE-ÖZDEŞ kalır.
    /// Yani gerçek tek-eksenli kontrol budur.
    #[must_use]
    pub fn from_args(profile_id: u8, r: u16, mode: FecModeArg, deadline_ms: u64) -> Self {
        match (profile_id, mode) {
            (0, _) => Self::Off,
            (p, FecModeArg::Adaptive) => Self::Adaptive { profile_id: p },
            (p, FecModeArg::Static) => Self::StaticR { profile_id: p, r },
            (p, FecModeArg::Policy) => Self::PolicyV2Deadline {
                profile_id: p,
                deadline_ns: deadline_ms.saturating_mul(1_000_000),
            },
        }
    }

    /// Rapor/manifest etiketi (RunMeta.kama_arm'ın kuyruğuna girer).
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Off => "fec-off".to_string(),
            Self::StaticR { profile_id, r } => format!("fec-static-r{r}-p{profile_id:#04x}"),
            Self::Adaptive { profile_id } => format!("fec-adaptive-p{profile_id:#04x}"),
            Self::PolicyV2Deadline {
                profile_id,
                deadline_ns,
            } => format!(
                "fec-policy-v2d{}ms-p{profile_id:#04x}",
                deadline_ns / 1_000_000
            ),
        }
    }
}

/// Client koşu parametreleri.
#[derive(Debug, Clone)]
pub struct ClientCfg {
    /// Server adresi.
    pub server: SocketAddr,
    /// Gönderilecek mesaj sayısı.
    pub msg_count: u64,
    /// Mesaj payload boyutu.
    pub payload_bytes: u64,
    /// Client'ın ack için bekleyeceği maksimum süre.
    pub max_deadline: Duration,
    /// CC.
    pub cc: Cc,
    /// Reproducibility seed / repeat kimliği.
    pub seed: u64,
    /// Tekrar indeksi (A varyans).
    pub repeat: u32,
    /// netem profil adı.
    pub profile: String,
    /// Kama kolu (quinn'de yok sayılır). [KAPI-1 W4]
    pub kama_arm: KamaArm,
    /// Mesaj sunum aralığı (workload ekseni, r5 §G). ZERO = burst (tümü t0'da — mevcut davranış
    /// aynen). >0 = paced: mesaj i, t = i×interval'de sunulur; mesaj-başı deadline
    /// `submission_i + max_deadline`; koşu sonu = son-sunum + max_deadline. [KAPI-1 W3]
    pub submit_interval: Duration,
    /// Forward-yön (client-egress) bütçesi — packet-atomic hard-cap (r5 §D). [KAPI-1 W2]
    pub budget: crate::budget::BudgetMode,
    /// [KAPI-1 W6] Ölçüm-öncesi warmup mesajı sayısı (0 = warmup yok). Warmup trafiği aynı sınıf
    /// akıştır; ölçüm kayıtlarına ve bütçeye GİRMEZ (governor pre-arm). A1/A2 farkını harness'ın
    /// netem'i ne zaman uyguladığı belirler (r5 B.2).
    pub warmup_msgs: u64,
    /// [KAPI-1 W6] Barrier gate yolu: warmup+quiescence sonrası `<gate>.ready` yazılır,
    /// `<gate>.go` beklenir; ölçüm-t0 = GO anı. None = gate yok (t0 ≈ koşu başı).
    pub gate: Option<std::path::PathBuf>,
    /// [KAPI-1 W6] Kuruluş-gate'i: DÜZLEM-A "session temiz ağda kurulur" alanı için İKİNCİ senkron
    /// noktası — bağlantı kurulunca `<g>.ready`, harness netem'i (A2: burada; A1: ölçüm-gate'inde)
    /// uygulayıp `<g>.go` yazar; warmup ancak ondan sonra başlar. None = senkronsuz.
    pub establish_gate: Option<std::path::PathBuf>,
    /// [SPRINT-A] Ürün-runtime kolunun FEC ayarı; diğer kollarda yok sayılır.
    pub fec: FecArm,
    /// [BURST-FUSION] Yapı-gate'in burst dalı politikası (opt-in, KALİBRE EDİLMEMİŞ).
    /// İKİ UÇTA AYNI kurulmalıdır. Diğer kollarda yok sayılır.
    pub burst_fusion: bool,
    /// [DR-CS-3] Cold-start bootstrap-RTT merdiveni (opt-in). İKİ UÇTA AYNI.
    pub bootstrap_rtt: bool,
    /// [IW] PreConverge ilk-uçuş penceresi (byte; 0 = sevk davranışı).
    pub initial_window_bytes: u64,
    /// [KURULUŞ] Bootstrap merdiveni kuruluş-hızlı mı (bkz. `RuntimeConfig::bootstrap_fast_ladder`).
    pub bootstrap_fast: bool,
    /// [ONAY UFKU] Kayıp-defteri confirm ufku: `0` = sevk davranışı (cadence-legacy),
    /// `>0` = kanal-türevli `k × srtt` per-mille. ÖLÇÜM KOLU — bkz.
    /// `reports/loss-ledger-confirm-defect/FINDING.md`.
    pub loss_confirm_k: u32,
    /// [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi (`false` = sevk davranışı).
    pub fec_risk: bool,
    /// [HANDOFF ENSTRÜMANI] Zaman-serisi CSV yolu (`Some` → 50 ms örnekleme). Server'da yok sayılır.
    pub ts_out: Option<std::path::PathBuf>,
    /// [R-SWEEP] Spekülatif L1/L2 tavanı ve yapı-gate seviyesi override'ları (‰); None = default.
    pub spec_l1_permille: Option<u32>,
    pub spec_l2_permille: Option<u32>,
    pub gate_permille: Option<u32>,
    /// [PERSIST] Ufuk modu ("fixed"|"learned"), taban α (‰ srtt), ρ (‰), quantile (‰). None = default.
    pub persist_mode: Option<String>,
    pub persist_alpha: Option<u32>,
    pub persist_rho: Option<u32>,
    pub persist_q: Option<u32>,
    pub rp_suppress: Option<u32>,
    /// [KTP2-F0 / C2] Yalnız `kama-product`: her ölçüm mesajı bu kadar AYRI birim olarak sunulur
    /// (gerçek tekrar kolu); teslim = kopyalardan ilkinin teyidi. `1` = eski davranış (kopya yok).
    /// Birim kimliği: `warmup_msgs + msg_id × dup_copies + kopya`.
    pub dup_copies: u32,
}

/// [KAPI-1 W6] Dosya-tabanlı barrier: `<gate>.ready` yaz → `<gate>.go` bekle (5 ms poll, 120 s
/// tavan — harness ölmüşse koşu asılı kalmaz, tipli hata döner). Ortak t=0 harness'ın GO yazdığı
/// andır; üç adaptör de AYNI protokolü kullanır (r5 B.2 "evaluation-trace başlangıcı AYNI").
pub(crate) async fn gate_sync(gate: &std::path::Path) -> Result<(), crate::error::BenchError> {
    let ready = format!("{}.ready", gate.display());
    let go = format!("{}.go", gate.display());
    std::fs::write(&ready, b"ready")?;
    for _ in 0..24_000u32 {
        if std::path::Path::new(&go).exists() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    Err(crate::error::BenchError::Other(format!(
        "gate timeout (120 s): {go} hiç yazılmadı — harness barrier'ı ölü"
    )))
}

/// Server koşu parametreleri.
#[derive(Debug, Clone)]
pub struct ServerCfg {
    /// Bind adresi.
    pub bind: SocketAddr,
    /// CC.
    pub cc: Cc,
    /// [TUR-5 r3] Epizot-kotası R [CAL {4,8}] — ölçüm-matrisi (kama receiver'ına geçer).
    pub quota_r: u32,
    /// Reverse-yön (server-egress: ACK/kontrol) bütçesi — yön-başına ayrı, aktarım YOK (r5 §D).
    /// [KAPI-1 W2]
    pub budget: crate::budget::BudgetMode,
    /// [KAPI-1 W6] Reverse-bütçe arm senkronu: verilirse governor `<path>.go` dosyası BELİRENE
    /// dek arm edilmez (handshake + warmup ACK'leri pre_arm'da muaf — r5 D.2 "warmup bütçeye
    /// girmez" REVERSE yönde de). Dosya sistemi netns'ler arasında paylaşımlı (unshare yalnız -n),
    /// bu yüzden server client'ın ölçüm-gate'ini izleyebilir. None = hemen arm (eski davranış).
    pub gate_watch: Option<std::path::PathBuf>,
    /// [SPRINT-A] Ürün-runtime kolunun FEC ayarı — İSTEMCİYLE AYNI olmalı (bkz. [`FecArm`]).
    pub fec: FecArm,
    /// [SPRINT-A] Alıcı birim-payload tavanı (ürün-runtime `max_unit_bytes`); FEC açıkken
    /// `payload + SOURCE_PREFIX_LEN ≤ profile.e` koşulunu koşu-öncesi typed doğrular.
    pub payload_bytes: u64,
    /// [BURST-FUSION] Bkz. [`ClientCfg::burst_fusion`]; İSTEMCİYLE AYNI olmalı.
    pub burst_fusion: bool,
    /// [DR-CS-3] Bkz. [`ClientCfg::bootstrap_rtt`]; İSTEMCİYLE AYNI olmalı.
    pub bootstrap_rtt: bool,
    /// [IW] Bkz. [`ClientCfg::initial_window_bytes`] (alıcıda etkisiz; simetri için taşınır).
    pub initial_window_bytes: u64,
    /// [KURULUŞ] Bkz. [`ClientCfg::bootstrap_fast`] (alıcıda etkisiz; simetri için taşınır).
    pub bootstrap_fast: bool,
    /// [ONAY UFKU] Kayıp-defteri confirm ufku: `0` = sevk davranışı (cadence-legacy),
    /// `>0` = kanal-türevli `k × srtt` per-mille. ÖLÇÜM KOLU — bkz.
    /// `reports/loss-ledger-confirm-defect/FINDING.md`.
    pub loss_confirm_k: u32,
    /// [FEC-RISK] Deneysel FEC kayıp-riski tahmincisi (`false` = sevk davranışı).
    pub fec_risk: bool,
    /// [HANDOFF ENSTRÜMANI] Zaman-serisi CSV yolu (`Some` → 50 ms örnekleme). Server'da yok sayılır.
    pub ts_out: Option<std::path::PathBuf>,
    /// [R-SWEEP] Spekülatif L1/L2 tavanı ve yapı-gate seviyesi override'ları (‰); None = default.
    pub spec_l1_permille: Option<u32>,
    pub spec_l2_permille: Option<u32>,
    pub gate_permille: Option<u32>,
    /// [PERSIST] Ufuk modu ("fixed"|"learned"), taban α (‰ srtt), ρ (‰), quantile (‰). None = default.
    pub persist_mode: Option<String>,
    pub persist_alpha: Option<u32>,
    pub persist_rho: Option<u32>,
    pub persist_q: Option<u32>,
    pub rp_suppress: Option<u32>,
    /// [KTP2-F0 / C1] Alıcı-tarafı teslim damgası (`serve --rx-log`; bkz. [`crate::rxlog`]).
    /// `None` = kayıt yok, dosya yok (eski davranış). `kama-product` ve `tcp` sunucularında.
    pub rx_log: Option<std::path::PathBuf>,
}

/// Bench transport portu. quinn impl + kama stub aynı harness'ta koşar (ADIM 5).
pub trait BenchTransport {
    /// Server'ı çalıştırır (endpoint kapanana dek).
    // RPITIT + Send (async fn in trait lint'inden kaçınır, clippy -D temiz).
    fn run_server(&self, cfg: ServerCfg) -> impl Future<Output = Result<(), BenchError>> + Send;
    /// Client'ı çalıştırır → ham event çıktısı (metric HESAPLAMAZ, SoC).
    fn run_client(
        &self,
        cfg: ClientCfg,
    ) -> impl Future<Output = Result<RunOutput, BenchError>> + Send;
}

/// Kama transport — adaptive-transport engine'i (§3/§4) gerçek UDP üstünde sürer.
/// `BenchTransport` impl'i [`crate::kama`]'da (identity security-stub + UDP event pump, ADIM-5).
pub struct KamaTransport;
