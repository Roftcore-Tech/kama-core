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
