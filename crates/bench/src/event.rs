//! Ham event kaydı (SoC: app HAM event yazar; oran/percentile HESABI ayrı → [`crate::metrics`]).

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

use crate::error::BenchError;

/// Tek mesajın ham teslim eventi. HESAP yok — yalnız ölçülen gerçek.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawEvent {
    /// Mesaj sırası.
    pub msg_id: u64,
    /// Koşu başından gönderim anı (ns).
    pub sent_ns: u64,
    /// Teslim onayı (ack) anı (ns); `None` = deadline içinde onay gelmedi (timeout/loss).
    pub delivered_ns: Option<u64>,
    /// Payload boyutu (byte).
    pub payload_bytes: u64,
}

/// tx wire byte'larının bileşen ayrımı (teşhis dekompozisyonu — D2: fix ÖNCESİ/SONRASI aynı format).
/// `data`=yeni DATA, `retx`=ARQ retransmit (§9), `dup`=forward duplication (§11),
/// `fec`=FEC repair-shard (§8), `ctrl`=kontrol düzlemi (PARAM_SET vb.).
/// Toplam = udp_tx_bytes. quinn bunu ayırt etmez → `default()` (tümü 0).
/// `fec`/`ctrl` yalnız ürün-runtime kolunda (`kama-product`) dolar; eski JSONL'lerde alan yok →
/// serde default 0 (geriye-uyumlu okuma), o kayıtlarda toplam data+retx+dup olarak kapanır.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxComponents {
    pub data: u64,
    pub retx: u64,
    pub dup: u64,
    #[serde(default)]
    pub fec: u64,
    #[serde(default)]
    pub ctrl: u64,
}

/// Bir koşunun (profil × CC × seed × repeat) metadata + connection-level wire sayaçları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    /// netem profil adı.
    pub profile: String,
    /// Congestion control ("cubic" | "bbr").
    pub cc: String,
    /// Transport ("quinn" | "kama").
    pub transport: String,
    /// quinn sürümü (rapor/reproducibility, §4.4.9).
    pub quinn_version: String,
    /// Reproducibility seed / repeat kimliği.
    pub seed: u64,
    /// Tekrar indeksi (varyans için, A).
    pub repeat: u32,
    /// Gönderilen mesaj sayısı.
    pub msg_count: u64,
    /// Mesaj başı payload.
    pub payload_bytes: u64,
    /// Maksimum deadline (ns) — client bu kadar bekler.
    pub max_deadline_ns: u64,
    /// Toplam UDP tx byte — TRANSPORT-DENEMESİ düzlemi (governor reddettikleri DAHİL; üç kol da
    /// aynı kural — inceleme #1 simetri düzeltmesi). Gerçek-ENJEKTE düzlem `budget.injected_bytes`.
    pub udp_tx_bytes: u64,
    /// Toplam UDP rx byte (dönüş yolu yükü).
    pub udp_rx_bytes: u64,
    /// tx wire byte bileşen ayrımı (teşhis; kama doldurur, quinn=default). [D2]
    #[serde(default)]
    pub tx: TxComponents,
    /// Kama kol etiketi ("kama-arq" | "kama-tail-v1"; quinn kollarında boş). Eski JSONL'lerde
    /// alan yok → default boş string (geriye-uyumlu okuma). [KAPI-1 W4]
    #[serde(default)]
    pub kama_arm: String,
    /// Mesaj sunum aralığı ns (0 = burst; >0 = paced, r5 §G). Eski JSONL → 0. [KAPI-1 W3]
    #[serde(default)]
    pub submit_interval_ns: u64,
    /// Forward-yön bütçe sayaçları (client-egress governor fotoğrafı; r5 D.5). Eski JSONL →
    /// default (mode boş). [KAPI-1 W2]
    #[serde(default)]
    pub budget: crate::budget::BudgetSnapshot,
    /// Ölçüm-t0'ın CLOCK_REALTIME damgası (ns) — pkt_capture (unix alanı) ve realized-log ile
    /// hizalama tabanı. 0 = kaydedilmedi (eski kayıt). [inceleme #7/#30]
    #[serde(default)]
    pub t0_unix_ns: u64,
    /// Event damgalarının (sent/delivered) koşu-epoch'una göre t0 ofseti (ns). kama: t0 (event'ler
    /// koşu-başından); quinn/qdgram: 0 (event'ler zaten t0-tabanlı). Analiz normalize eder.
    #[serde(default)]
    pub events_epoch_offset_ns: u64,
    /// t0 anında hâlâ teslim-edilmemiş warmup birimi sayısı (residue). Severe-loss'ta 0 garanti
    /// edilemez; TÜM kollarda simetrik ölçülür+raporlanır; post-t0 residue trafiği her kolda
    /// ölçüme dahildir (steady-state semantiği). [inceleme #0 çözümü]
    #[serde(default)]
    pub warmup_residue: u64,
    /// [FAZ-4] Ateşleme özeti (yalnız Kama kolları doldurur; manifest bunu taşır).
    #[serde(default)]
    pub activation: Option<crate::manifest::ActivationSummary>,
    /// [FAZ-4] Kurulum maliyeti (ns) ve sınıfı — kolların KARIŞTIRILMAMASI için.
    #[serde(default)]
    pub setup_ns: u64,
    /// [FAZ-4 §M4] Kurulum maliyeti ÖLÇÜM PENCERESİ İÇİNDE mi? Bu, kolun KODUNDAN gelen yapısal
    /// gerçektir — operatör bayrağı DEĞİL. Manifest `setup_class`'ı bundan TÜRETİR; operatörün
    /// iddiası bununla çelişirse koşu manifest YAZMADAN hata verir (sessiz karışım imkânsız).
    #[serde(default)]
    pub setup_in_window: bool,
    /// [FAZ-4] PROFİL KİMLİĞİ — `client_core::runtime::TRANSPORT_V1_PROFILE_ID` (v1.1:
    /// `kama-v1.1-full`, K4G-05b) yalnız sapma YOKKEN yazılır; aksi hâlde `kama-custom`.
    /// Yorum/operatör disiplinine DEĞİL, koddan türetilen farka dayanır.
    #[serde(default)]
    pub profile_id: String,
    /// Kanonik ürün profilinden sapmalar (boşsa kimlik `TRANSPORT_V1_PROFILE_ID`).
    #[serde(default)]
    pub profile_delta: Vec<String>,
}

/// Bir koşunun tüm çıktısı: meta + ham eventler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutput {
    /// Koşu metadata'sı.
    pub meta: RunMeta,
    /// Ham eventler.
    pub events: Vec<RawEvent>,
}

// JSONL satır tipleri: `{"meta":{...}}` sonra her satır `{"event":{...}}`.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Line<'a> {
    Meta(&'a RunMeta),
    Event(&'a RawEvent),
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum OwnedLine {
    // Box: RunMeta W2 bütçe fotoğrafıyla büyüdü → varyant-boyu farkı (clippy large_enum_variant).
    // Satır-başına tek geçici alloc; JSON biçimi DEĞİŞMEZ (serde Box'ı şeffaf açar).
    Meta(Box<RunMeta>),
    Event(RawEvent),
}

impl RunOutput {
    /// JSONL yazar (1. satır meta, sonrası event).
    ///
    /// # Errors
    /// Dosya/serileştirme hatası.
    pub fn write_jsonl(&self, path: &Path) -> Result<(), BenchError> {
        // Atomik: tmp'ye yaz + rename — yarıda ölen client bozuk out.jsonl BIRAKAMAZ (sürücünün
        // resume kontrolü varlık-tabanlı; kısmi dosya "tamamlanmış koşu" sayılırdı). [inceleme #28]
        let tmp = path.with_extension("jsonl.tmp");
        {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
            writeln!(f, "{}", serde_json::to_string(&Line::Meta(&self.meta))?)?;
            for e in &self.events {
                writeln!(f, "{}", serde_json::to_string(&Line::Event(e))?)?;
            }
            f.flush()?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// JSONL okur.
    ///
    /// # Errors
    /// Dosya/deserileştirme hatası veya meta satırı yoksa.
    pub fn read_jsonl(path: &Path) -> Result<Self, BenchError> {
        let text = std::fs::read_to_string(path)?;
        let mut meta = None;
        let mut events = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<OwnedLine>(line)? {
                OwnedLine::Meta(m) => meta = Some(*m),
                OwnedLine::Event(e) => events.push(e),
            }
        }
        let meta = meta.ok_or_else(|| BenchError::Other("JSONL'de meta satırı yok".into()))?;
        Ok(RunOutput { meta, events })
    }
}
