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
/// `data`=yeni DATA, `retx`=ARQ retransmit (§9), `dup`=forward duplication (§11). Toplam = udp_tx_bytes.
/// quinn bunu ayırt etmez → `default()` (tümü 0); yalnız kama doldurur.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxComponents {
    pub data: u64,
    pub retx: u64,
    pub dup: u64,
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
    /// Toplam UDP tx byte (wire; bytes/msg + overhead metriği).
    pub udp_tx_bytes: u64,
    /// Toplam UDP rx byte (dönüş yolu yükü).
    pub udp_rx_bytes: u64,
    /// tx wire byte bileşen ayrımı (teşhis; kama doldurur, quinn=default). [D2]
    #[serde(default)]
    pub tx: TxComponents,
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
    Meta(RunMeta),
    Event(RawEvent),
}

impl RunOutput {
    /// JSONL yazar (1. satır meta, sonrası event).
    ///
    /// # Errors
    /// Dosya/serileştirme hatası.
    pub fn write_jsonl(&self, path: &Path) -> Result<(), BenchError> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
        writeln!(f, "{}", serde_json::to_string(&Line::Meta(&self.meta))?)?;
        for e in &self.events {
            writeln!(f, "{}", serde_json::to_string(&Line::Event(e))?)?;
        }
        f.flush()?;
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
                OwnedLine::Meta(m) => meta = Some(m),
                OwnedLine::Event(e) => events.push(e),
            }
        }
        let meta = meta.ok_or_else(|| BenchError::Other("JSONL'de meta satırı yok".into()))?;
        Ok(RunOutput { meta, events })
    }
}
