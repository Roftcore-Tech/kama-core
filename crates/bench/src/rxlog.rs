//! [KTP2-F0 / Ajan C isteği C1] ALICI-TARAFI TESLİM DAMGASI — `serve --rx-log <PATH>`.
//!
//! Verilmezse hiçbir şey kaydedilmez ve hiçbir dosya yazılmaz (davranış ve çıktı eskisiyle aynı).
//! Verilirse her teslim `(birim kimliği, CLOCK_REALTIME unix ns, yük boyu)` olarak sınırlı bir
//! belleğe eklenir (tavan [`RX_LOG_CAP`] kayıt; aşanlar `dropped` sayacına — SR-AUDIT-001: sınırsız
//! büyüme yok) ve sunucu döngüsü çıkınca (SIGTERM/SIGINT dahil) JSONL olarak tmp + rename ile
//! atomik yazılır:
//!
//! ```text
//! {"rx_log_schema":"ktp2-c-rx-v1","dropped":N}
//! {"u":<birim>,"t":<unix_ns>,"n":<bayt>}
//! …
//! ```
//!
//! Birim kimliği: `kama-product` = `Delivery::unit_id` (fragmenter açıkken mesaj kimliği;
//! `warmup_msgs` ölçüm birimlerinin önündedir). `tcp` = çerçevedeki `msg_id`. quinn kolunda yük
//! mesaj kimliği taşımadığı için desteklenmez.

use std::io::Write;
use std::path::PathBuf;

/// Şema kimliği (C'nin analizörü bunu denetler).
pub const RX_LOG_SCHEMA: &str = "ktp2-c-rx-v1";
/// Kayıt tavanı (2^20).
pub const RX_LOG_CAP: usize = 1 << 20;

/// CLOCK_REALTIME (unix ns).
#[must_use]
pub fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// Sınırlı alım kaydedicisi.
#[derive(Debug)]
pub struct RxRecorder {
    path: PathBuf,
    cap: usize,
    recs: Vec<(u64, u64, u64)>,
    dropped: u64,
}

impl RxRecorder {
    /// Tavan [`RX_LOG_CAP`].
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self::with_cap(path, RX_LOG_CAP)
    }

    /// Açık tavanla (test).
    #[must_use]
    pub fn with_cap(path: PathBuf, cap: usize) -> Self {
        Self {
            path,
            cap,
            recs: Vec::new(),
            dropped: 0,
        }
    }

    /// Bir teslimi damgalar (damga BU çağrıda alınır).
    pub fn record(&mut self, unit: u64, len: usize) {
        let t = unix_ns();
        if self.recs.len() < self.cap {
            self.recs.push((unit, t, len as u64));
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    /// Atomik yazım (tmp + rename). Hata sessizce yutulmaz: stderr'e `RX-LOG-WRITE-FAILED`.
    pub fn write_or_warn(&self) {
        if let Err(e) = self.write() {
            eprintln!("RX-LOG-WRITE-FAILED {}: {e}", self.path.display());
        } else {
            eprintln!(
                "RX-LOG {} kayıt={} dropped={}",
                self.path.display(),
                self.recs.len(),
                self.dropped
            );
        }
    }

    fn write(&self) -> std::io::Result<()> {
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
            writeln!(
                f,
                "{{\"rx_log_schema\":\"{RX_LOG_SCHEMA}\",\"dropped\":{}}}",
                self.dropped
            )?;
            for (u, t, n) in &self.recs {
                writeln!(f, "{{\"u\":{u},\"t\":{t},\"n\":{n}}}")?;
            }
            f.flush()?;
        }
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Biçim + tavan: başlık satırı, kayıt satırları, tavanı aşan `dropped`'a gider.
    #[test]
    fn bicim_ve_tavan() {
        let dir = std::env::temp_dir().join(format!("ktp2-rxlog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("rx.jsonl");
        let mut r = RxRecorder::with_cap(p.clone(), 2);
        r.record(7, 65);
        r.record(8, 1024);
        r.record(9, 1);
        r.write_or_warn();
        let text = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        let h: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(h["rx_log_schema"], RX_LOG_SCHEMA);
        assert_eq!(h["dropped"], 1);
        let a: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(a["u"], 7);
        assert_eq!(a["n"], 65);
        assert!(a["t"].as_u64().unwrap() > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
