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
//! `warmup_msgs` ölçüm birimlerinin önündedir; `--dup-copies N` ile birim = mesaj × N + kopya).
//! `tcp` = çerçevedeki `msg_id`. `quinn` = yükün ilk 8 baytı (BE) — istemci `--payload-id` ile
//! yazar; yazılmamışsa ya da yük < 8 B ise `u64::MAX`. Damga anı: kama = teslim kanalından alındığı
//! an; tcp = çerçevenin tamamı okunduğu an; quinn = stream sonuna (FIN) dek okunduğu an.
//!
//! **Bench `kama` sunucusu (security-stub kolu) AYRI yazıcı kullanır: [`RxStream`]** — satır satır,
//! kullanıcı-alanı tamponu olmadan (sunucu bu kolda sinyal işleyicisi kurmaz; `kill` ile ölür).
//! Kayıt alanları yukarıdakilerle aynıdır (`u`, `t`, `n`), ek alanlar `s` ve `dup`; ilk satır
//! `server_start` başlığıdır ve `rx_log_schema` anahtarını da taşır (başlığı anahtardan ya da
//! "ilk satırı atla" kuralıyla tanıyan okuyucular aynen okur). Ayrıntı: [`RxStream`].

use std::collections::HashSet;
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::signal::unix::{signal, SignalKind};

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

/// Görevler arası paylaşılan kaydedici (quinn/tcp sunucuları bağlantı başına görev açar).
pub type SharedRx = Arc<Mutex<RxRecorder>>;

/// Kendiliğinden çıkmayan sunucular (quinn, tcp: `kill` ile biter) için: SIGTERM/SIGINT gelince
/// kaydı yazar ve süreci 0 ile bitirir. YALNIZ `--rx-log` verilince çağrılır.
///
/// # Errors
/// Sinyal işleyicisi kurulamazsa.
pub fn write_on_signal(r: SharedRx) -> std::io::Result<()> {
    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    tokio::spawn(async move {
        tokio::select! {
            _ = term.recv() => {},
            _ = int.recv() => {},
        }
        if let Ok(g) = r.lock() {
            g.write_or_warn();
        }
        std::process::exit(0);
    });
    Ok(())
}

/// Paylaşımlı kaydediciye tek kayıt (`None` = kayıt kapalı, hiçbir şey yapmaz).
pub fn record_shared(r: Option<&SharedRx>, unit: u64, len: usize) {
    if let Some(r) = r {
        if let Ok(mut g) = r.lock() {
            g.record(unit, len);
        }
    }
}

/// quinn yükünün ilk 8 baytı (BE) = mesaj kimliği (`client --payload-id`). Kısa yük → `u64::MAX`.
#[must_use]
pub fn id_prefix(data: &[u8]) -> u64 {
    data.get(..8)
        .and_then(|b| <[u8; 8]>::try_from(b).ok())
        .map_or(u64::MAX, u64::from_be_bytes)
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

/// [Ö-P] Akışlı alıcı kaydı — bench `kama` sunucusu (`serve --transport kama --rx-log <PATH>`).
///
/// **Ne damgalanır:** birimin alıcı UYGULAMAYA (bench sunucu döngüsüne) teslim anı — `Receiver::
/// on_datagram` dönüşünde `RxOutput::delivered` içinde çıktığı an. Ağ varışı DEĞİLDİR (o, arayüz
/// yakalamasıdır). Aynı `seq`'in ikinci varışı taşımada bastırılır (`was_duplicate`, teslim yok →
/// satır yok); aynı birimin YENİ `seq`'le (yeniden iletim/kopya) ikinci teslimi satır yazar,
/// `dup: true` ile.
///
/// **Saat:** `t` = CLOCK_REALTIME unix ns ([`unix_ns`], `SystemTime::now`) — arayüz yakalamasının
/// `t_unix_ns` alanı (`time.time_ns()`) ve istemcinin `RunMeta.t0_unix_ns` değeri ile AYNI saat.
///
/// **Dayanıklılık:** her satır tek `write(2)` çağrısıyla dosyaya gider; kullanıcı-alanı tamponu
/// YOK (`BufWriter` yok, çıkışta yazım yok). Sunucu SIGTERM/SIGKILL ile ölse de `record` dönmüş
/// her satır dosyadadır.
///
/// **Biçim:**
/// ```text
/// {"kind":"server_start","transport":"kama","engine":"<legacy|cc-port>","rx_log_schema":"ktp2-c-rx-v1","clock":"CLOCK_REALTIME"}
/// {"u":<birim>,"t":<unix_ns>,"n":<yük baytı>,"s":<taşıma seq>,"dup":<bool>}
/// …
/// ```
/// Başlıkta `dropped` YOKTUR (başlangıçta bilinemez). Tavan ([`RX_LOG_CAP`] satır) aşılırsa bir kez
/// `{"kind":"cap_reached","rx_log_schema":"ktp2-c-rx-v1","dropped":1,"cap":N}` yazılır (anahtardan
/// başlık tanıyan okuyucu `dropped > 0` görür; sonraki teslimler yazılmaz) ve stderr'e
/// `RX-LOG-CAP` satırı düşer. Birim kimliği = DATA çerçevesindeki `unit_id` (ısınma birimleri
/// `2^40 + i`; ölçüm birimleri `0..msgs`). `dup` anahtarı (eş adresi, birim) çiftidir.
#[derive(Debug)]
pub struct RxStream {
    path: PathBuf,
    file: Option<std::fs::File>,
    cap: usize,
    lines: usize,
    seen: HashSet<(SocketAddr, u64)>,
    line: String,
}

impl RxStream {
    /// Dosyayı açar ve `server_start` başlığını yazar (tavan [`RX_LOG_CAP`]).
    ///
    /// # Errors
    /// Dosya oluşturulamaz ya da başlık yazılamazsa (koşu başlamadan hata — sessizce kayıtsız
    /// koşmak yerine).
    pub fn create(path: &Path, transport: &str, engine: &str) -> std::io::Result<Self> {
        Self::with_cap(path, transport, engine, RX_LOG_CAP)
    }

    /// Açık tavanla (test).
    ///
    /// # Errors
    /// Bkz. [`RxStream::create`].
    pub fn with_cap(
        path: &Path,
        transport: &str,
        engine: &str,
        cap: usize,
    ) -> std::io::Result<Self> {
        let mut file = std::fs::File::create(path)?;
        // Anahtar sırası sabit (`kind` önce): `json!` haritası anahtarları sıralardı. Değerler
        // `Value` gösterimiyle JSON-kaçışlı yazılır.
        let header = format!(
            "{{\"kind\":\"server_start\",\"transport\":{},\"engine\":{},\
             \"rx_log_schema\":\"{RX_LOG_SCHEMA}\",\"clock\":\"CLOCK_REALTIME\"}}\n",
            serde_json::Value::from(transport),
            serde_json::Value::from(engine),
        );
        file.write_all(header.as_bytes())?;
        Ok(Self {
            path: path.to_path_buf(),
            file: Some(file),
            cap,
            lines: 0,
            seen: HashSet::new(),
            line: String::with_capacity(96),
        })
    }

    /// Bir uygulama teslimini yazar. `t_unix_ns` çağıran tarafından teslim anında alınır
    /// ([`unix_ns`]); yazım ACK gönderiminden SONRA yapılabilsin diye ayrıdır. Yazım hatasında
    /// stderr'e bir kez `RX-LOG-WRITE-FAILED` düşer ve kayıt kapanır (sunucu çalışmayı sürdürür).
    pub fn record(&mut self, peer: SocketAddr, unit: u64, seq: u64, len: usize, t_unix_ns: u64) {
        use std::fmt::Write as _;
        let Some(f) = self.file.as_mut() else {
            return;
        };
        if self.lines >= self.cap {
            let res = f.write_all(
                format!(
                    "{{\"kind\":\"cap_reached\",\"rx_log_schema\":\"{RX_LOG_SCHEMA}\",\
                     \"dropped\":1,\"cap\":{}}}\n",
                    self.cap
                )
                .as_bytes(),
            );
            eprintln!(
                "RX-LOG-CAP {} tavan={} — sonraki teslimler yazılmadı{}",
                self.path.display(),
                self.cap,
                res.err()
                    .map_or_else(String::new, |e| format!(" (işaret yazılamadı: {e})"))
            );
            self.file = None;
            return;
        }
        let dup = !self.seen.insert((peer, unit));
        self.line.clear();
        // `String`'e yazım başarısız olamaz.
        let _ = writeln!(
            self.line,
            "{{\"u\":{unit},\"t\":{t_unix_ns},\"n\":{len},\"s\":{seq},\"dup\":{dup}}}"
        );
        if let Err(e) = f.write_all(self.line.as_bytes()) {
            eprintln!("RX-LOG-WRITE-FAILED {}: {e}", self.path.display());
            self.file = None;
            return;
        }
        self.lines += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ktp2-rxs-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn json_lines(p: &Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// [Ö-P] Başlık + kayıt biçimi: ilk satır `server_start` (motor etiketi + şema anahtarı),
    /// kayıtlar `RxRecorder` satırının alanlarını (`u`, `t`, `n`) aynı adlarla taşır; `dup` aynı
    /// eşten aynı birimin ikinci teslimidir. Satırlar `drop`/`flush` OLMADAN okunabilir olmalı —
    /// sunucu `kill` ile öldüğünde de yazılmış satır kaybolmaz.
    #[test]
    fn stream_bicim_baslik_dup_ve_tamponsuz() {
        let dir = tmp_dir("bicim");
        let p = dir.join("rx.jsonl");
        let a: SocketAddr = "10.0.0.1:1000".parse().unwrap();
        let b: SocketAddr = "10.0.0.2:1000".parse().unwrap();
        let mut s = RxStream::create(&p, "kama", "cc-port").unwrap();
        s.record(a, 7, 3, 1140, 1_000);
        s.record(a, 7, 9, 1140, 2_000); // aynı birim, yeni seq (yeniden iletim) → dup
        s.record(b, 7, 3, 1140, 3_000); // başka eş → dup değil
        s.record(a, 8, 4, 65, 4_000);
        // `s` hâlâ canlı: dosya içeriği şimdiden tam olmalı (kullanıcı-alanı tamponu yok).
        let v = json_lines(&p);
        assert_eq!(v.len(), 5, "1 başlık + 4 kayıt");
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(
            raw.starts_with(r#"{"kind":"server_start","transport":"kama","engine":"cc-port","#),
            "başlık anahtar sırası: {raw}"
        );
        assert_eq!(v[0]["kind"], "server_start");
        assert_eq!(v[0]["transport"], "kama");
        assert_eq!(v[0]["engine"], "cc-port");
        assert_eq!(v[0]["rx_log_schema"], RX_LOG_SCHEMA);
        assert_eq!(v[0]["clock"], "CLOCK_REALTIME");
        assert!(v[0].get("u").is_none(), "başlık birim sayılmaz");
        use serde_json::json;
        assert_eq!(
            v[1],
            json!({"u": 7, "t": 1_000, "n": 1140, "s": 3, "dup": false})
        );
        assert_eq!(
            v[2],
            json!({"u": 7, "t": 2_000, "n": 1140, "s": 9, "dup": true})
        );
        assert_eq!(v[3]["dup"], false, "başka eşin aynı kimliği dup değil");
        assert_eq!(
            v[4],
            json!({"u": 8, "t": 4_000, "n": 65, "s": 4, "dup": false})
        );
        assert_eq!(s.lines, 4);
        // Alan adları `RxRecorder` kaydıyla aynı (aynı ayrıştırıcı): u/t/n.
        let q = dir.join("rec.jsonl");
        let mut r = RxRecorder::new(q.clone());
        r.record(7, 1140);
        r.write_or_warn();
        let old = json_lines(&q);
        for k in old[1].as_object().unwrap().keys() {
            assert!(
                v[1].get(k).is_some(),
                "RxRecorder alanı `{k}` akışlı kayıtta yok"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// [Ö-P] Tavan: `cap` satırdan sonra bir kez `cap_reached` işareti (`dropped` > 0, şema
    /// anahtarıyla) ve kayıt kapanır; sonraki teslimler satır yazmaz.
    #[test]
    fn stream_tavan_isareti() {
        let dir = tmp_dir("tavan");
        let p = dir.join("rx.jsonl");
        let a: SocketAddr = "10.0.0.1:1000".parse().unwrap();
        let mut s = RxStream::with_cap(&p, "kama", "legacy", 2).unwrap();
        for u in 0..5 {
            s.record(a, u, u, 10, 1 + u);
        }
        let v = json_lines(&p);
        assert_eq!(v.len(), 4, "başlık + 2 kayıt + işaret: {v:?}");
        assert_eq!(v[0]["engine"], "legacy");
        assert_eq!(v[3]["kind"], "cap_reached");
        assert_eq!(v[3]["rx_log_schema"], RX_LOG_SCHEMA);
        assert_eq!(v[3]["dropped"], 1);
        assert_eq!(s.lines, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// [Ö-P] Dosya açılamazsa koşu başlamadan hata (sessiz kayıtsız koşu yok).
    #[test]
    fn stream_acilamazsa_hata() {
        let p = std::env::temp_dir().join("ktp2-rxs-yok-dizin/alt/rx.jsonl");
        assert!(RxStream::create(&p, "kama", "legacy").is_err());
    }

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
