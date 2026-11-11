//! [Ö-M] İSTEMCİ TIKANIKLIK İZİ — `client --cc-trace <PATH>` (SALT GÖZLEM).
//!
//! Verilmezse hiçbir şey kaydedilmez, dosya açılmaz ve iki kolun kodu eski yolu izler. Verilirse
//! zaman damgalı JSONL yazılır (her satır tek nesne, `k` alanı tür):
//!
//! ```text
//! {"k":"start","schema":"bench-cc-trace-v1","transport":…,"arm":…,"engine":…,"sample_ns":100000000,"clock":…}
//! {"k":"t0",…}       ölçüm-t0 (istemci RunMeta.t0_unix_ns ile aynı değer) + o anki örnek
//! {"k":"mode",…}     [kama] BBR kip geçişi (from → to), geçişi gösteren ilk olay dönüşünde
//! {"k":"s",…}        periyodik örnek (koşu saatinde 100 ms ızgarası)
//! {"k":"end",…}      koşu sonu örneği (yoksa iz eksik: süreç yarıda öldü)
//! ```
//!
//! **Karar yoluna girmez (kodla):**
//! * `kama`: [`KamaTrace::observe`] göndericiyi yalnız `&Sender` olarak alır ve yalnız `&self`
//!   erişimcilerini çağırır (`diag_cc`, `diag_pacer`, `diag_rate`, `diag_loss`,
//!   `diag_confirmed_cancels`, `diag_activation`, `tail_probe_due_edge_count`). `Sender` iç
//!   değişkenlik taşımaz (`Cell`/`RefCell`/kilit yok); paylaşımlı ödünçle durumu değişemez.
//!   Çağrı yeri `ClientRun::drive`'da `on_event` kararlarının tele yazılmasından SONRADIR ve dönüş
//!   değeri yoktur: hiçbir karar, zamanlayıcı ya da `select!` kolu izden okumaz. Ayrı zamanlayıcı
//!   da kurulmaz: örnek, zaten her 1 ms'de çağrılan `drive`'da 100 ms sınırı geçilince alınır.
//! * `quinn`: [`QuinnTrace`] ayrı bir görevde `Connection::stats()` kopyasını okur (bağlantı
//!   durumunu değiştirmez; quinn sürücüsüyle aynı kilidi kısa süre paylaşır).
//! * Yazım tamponludur ([`BufWriter`]); yazım hatasında stderr'e bir kez `CC-TRACE-WRITE-FAILED`
//!   düşer ve iz kapanır (koşu sürer). Satır tavanı [`CC_TRACE_CAP`] (SR-AUDIT-001).
//!
//! Saat: `t_run_ns` = istemci koşu saati (kama: `ClientRun` olaylarına verilen `now`; quinn:
//! ölçüm-t0'dan beri monotonik), `t_unix_ns` = CLOCK_REALTIME (istemci `t0_unix_ns`, sunucu
//! `--rx-log` ve arayüz yakalaması ile aynı saat). Sayaçlar KÜMÜLATİFTİR (oturum boyu; bench'in
//! `tx_*` baytları ölçüm-t0'da sıfırlanır — `t0` satırı farkın tabanıdır).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use adaptive_transport::cc::Mode;
use adaptive_transport::sender::Sender;
use serde_json::{json, Value};

/// Şema kimliği (okuyucu denetler).
pub const CC_TRACE_SCHEMA: &str = "bench-cc-trace-v1";
/// Periyodik örnek aralığı (koşu saati, ns).
pub const SAMPLE_NS: u64 = 100_000_000;
/// Satır tavanı: 300 s × 10 örnek/s ≈ 3 000 satır + geçişler; tavan 2^18 (aşılırsa iz kapanır).
pub const CC_TRACE_CAP: usize = 1 << 18;

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// `main` komutu çözünce BİR kez yazar (bkz. `rival::set` deseni: 14 `ClientCfg` literalini
/// değiştirmemek için süreç-genel ayar). İkinci çağrı yok sayılır.
pub fn set(p: PathBuf) {
    let _ = PATH.set(p);
}

/// İz yolu (`None` = iz kapalı, eski yol).
#[must_use]
pub fn path() -> Option<&'static Path> {
    PATH.get().map(PathBuf::as_path)
}

/// Tamponlu, tavanlı satır yazıcı (iki kolun ortak kısmı).
struct Lines {
    path: PathBuf,
    w: Option<BufWriter<File>>,
    lines: usize,
}

impl Lines {
    fn create(path: &Path, header: &Value) -> std::io::Result<Self> {
        let mut w = BufWriter::new(File::create(path)?);
        writeln!(w, "{header}")?;
        Ok(Self {
            path: path.to_path_buf(),
            w: Some(w),
            lines: 1,
        })
    }

    fn put(&mut self, v: &Value) {
        let Some(w) = self.w.as_mut() else {
            return;
        };
        if self.lines >= CC_TRACE_CAP {
            let _ = writeln!(w, "{}", json!({"k": "cap_reached", "cap": CC_TRACE_CAP}));
            let _ = w.flush();
            eprintln!(
                "CC-TRACE-CAP {} tavan={CC_TRACE_CAP} — sonraki satırlar yazılmadı",
                self.path.display()
            );
            self.w = None;
            return;
        }
        if let Err(e) = writeln!(w, "{v}") {
            eprintln!("CC-TRACE-WRITE-FAILED {}: {e}", self.path.display());
            self.w = None;
            return;
        }
        self.lines += 1;
    }

    fn close(&mut self) {
        if let Some(mut w) = self.w.take() {
            if let Err(e) = w.flush() {
                eprintln!("CC-TRACE-WRITE-FAILED {}: {e}", self.path.display());
            }
        }
    }
}

fn mode_label(m: Mode) -> &'static str {
    match m {
        Mode::Startup => "Startup",
        Mode::Drain => "Drain",
        Mode::ProbeBw => "ProbeBw",
        Mode::ProbeRtt => "ProbeRtt",
    }
}

/// Bench `ClientRun`'ın kendi sayaçları (motor dışı; ölçüm-t0'da sıfırlanan tx baytları dahil).
#[derive(Debug, Clone, Copy, Default)]
pub struct BenchCounters {
    /// Bench'in teslim saydığı (ACK'lenmiş) ölçüm birimi.
    pub acked_units: u64,
    /// Teslim edilen ısınma birimi.
    pub warmup_acked: u64,
    /// Tele denenen bayt, sınıf başına (DATA çerçevesi boyu; bkz. `ClientRun::drive`).
    pub tx_data_b: u64,
    pub tx_retx_b: u64,
    pub tx_dup_b: u64,
}

/// `kama` kolu izi.
pub struct KamaTrace {
    out: Lines,
    next_sample_ns: u64,
    last_mode: Option<Mode>,
}

impl KamaTrace {
    /// Dosyayı açar ve `start` başlığını yazar. Açılamazsa hata (koşu başlamadan; sessiz
    /// izsiz koşu yok).
    ///
    /// # Errors
    /// Dosya oluşturulamaz ya da başlık yazılamazsa.
    pub fn create(path: &Path, arm: &str, engine: &str) -> std::io::Result<Self> {
        let header = json!({
            "k": "start",
            "schema": CC_TRACE_SCHEMA,
            "transport": "kama",
            "arm": arm,
            "engine": engine,
            "sample_ns": SAMPLE_NS,
            "clock": "t_run_ns=ClientRun now; t_unix_ns=CLOCK_REALTIME",
        });
        Ok(Self {
            out: Lines::create(path, &header)?,
            next_sample_ns: 0,
            last_mode: None,
        })
    }

    /// Bir `drive` dönüşünde çağrılır: kip değiştiyse `mode` satırı, 100 ms sınırı geçildiyse
    /// `s` satırı. Göndericiyi yalnız okur (`&Sender`).
    pub fn observe(&mut self, s: &Sender, now_ns: u64, c: BenchCounters) {
        let mode = s.diag_cc().map(|d| d.0);
        if let Some(m) = mode {
            match self.last_mode {
                Some(prev) if prev == m => {}
                prev => {
                    let round = s.diag_cc().map_or(0, |d| d.4);
                    self.out.put(&json!({
                        "k": "mode",
                        "t_run_ns": now_ns,
                        "t_unix_ns": crate::rxlog::unix_ns(),
                        "from": prev.map(mode_label),
                        "to": mode_label(m),
                        "round": round,
                    }));
                    self.last_mode = Some(m);
                }
            }
        }
        if now_ns >= self.next_sample_ns {
            let v = sample("s", s, now_ns, c);
            self.out.put(&v);
            self.next_sample_ns = (now_ns / SAMPLE_NS + 1) * SAMPLE_NS;
        }
    }

    /// Ölçüm-t0 satırı (istemcinin `RunMeta.t0_unix_ns` değeriyle) + o anki örnek.
    pub fn mark_t0(&mut self, s: &Sender, t0_run_ns: u64, t0_unix_ns: u64, c: BenchCounters) {
        let mut v = sample("t0", s, t0_run_ns, c);
        if let Some(o) = v.as_object_mut() {
            o.insert("t0_unix_ns".into(), json!(t0_unix_ns));
        }
        self.out.put(&v);
    }

    /// Koşu sonu örneği + tampon boşaltma.
    pub fn finish(&mut self, s: &Sender, now_ns: u64, c: BenchCounters) {
        let v = sample("end", s, now_ns, c);
        self.out.put(&v);
        self.out.close();
    }
}

/// Tek örnek satırı (salt okuma erişimcileri).
fn sample(kind: &str, s: &Sender, now_ns: u64, c: BenchCounters) -> Value {
    let (state, btlbw, srtt_us, _rate, inflight) = s.diag_rate();
    let (pacer_rate, pacer_tokens, burst_cap) = s.diag_pacer();
    let (loss_pkt, loss_time, pending_lost) = s.diag_loss();
    let cancels = s.diag_confirmed_cancels();
    let act = s.diag_activation();
    let mut v = json!({
        "k": kind,
        "t_run_ns": now_ns,
        "t_unix_ns": crate::rxlog::unix_ns(),
        "state": format!("{state:?}"),
        "btlbw": btlbw,
        "srtt_us": srtt_us,
        "inflight_bytes": inflight,
        "pacer_rate": pacer_rate,
        "pacer_tokens": pacer_tokens,
        "burst_cap": burst_cap,
        "loss_pkt": loss_pkt,
        "loss_time": loss_time,
        "pending_lost": pending_lost,
        "lost_not_requeued": cancels.lost_not_requeued,
        "pto_due_edges": s.tail_probe_due_edge_count(),
        "tail_probes": act.tail_probes,
        "tail_cancelled": act.tail_cancelled,
        "retransmits": act.retransmits,
        "dups": act.dups,
        "acked_units": c.acked_units,
        "warmup_acked": c.warmup_acked,
        "tx_data_b": c.tx_data_b,
        "tx_retx_b": c.tx_retx_b,
        "tx_dup_b": c.tx_dup_b,
    });
    if let (Some(o), Some((mode, window, rate, min_rtt, round, ce, sx, cwnd, rw))) =
        (v.as_object_mut(), s.diag_cc())
    {
        o.insert("mode".into(), json!(mode_label(mode)));
        o.insert("window".into(), json!(window));
        o.insert("delivery_rate".into(), json!(rate));
        o.insert("min_rtt_ns".into(), json!(min_rtt));
        o.insert("round".into(), json!(round));
        o.insert("cong_events".into(), json!(ce));
        o.insert("startup_exit_round".into(), json!(sx));
        o.insert("cwnd".into(), json!(cwnd));
        o.insert("recovery_window".into(), json!(rw));
    }
    v
}

/// `quinn` kolu izi: ölçüm-t0'dan koşu sonuna dek 100 ms'de bir `Connection::stats()`.
pub struct QuinnTrace {
    stop: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Lines>,
    t0: Instant,
}

fn quinn_sample(kind: &str, t_run_ns: u64, st: &quinn::ConnectionStats) -> Value {
    let p = &st.path;
    json!({
        "k": kind,
        "t_run_ns": t_run_ns,
        "t_unix_ns": crate::rxlog::unix_ns(),
        "rtt_us": u64::try_from(p.rtt.as_micros()).unwrap_or(u64::MAX),
        "min_rtt_us": u64::try_from(p.min_rtt.as_micros()).unwrap_or(u64::MAX),
        "cwnd": p.cwnd,
        "cong_events": p.congestion_events,
        "lost_packets": p.lost_packets,
        "lost_bytes": p.lost_bytes,
        "sent_packets": p.sent_packets,
        "udp_tx_b": st.udp_tx.bytes,
        "udp_tx_dg": st.udp_tx.datagrams,
        "udp_rx_b": st.udp_rx.bytes,
        "udp_rx_dg": st.udp_rx.datagrams,
        "frame_tx_stream": st.frame_tx.stream,
        "frame_tx_ping": st.frame_tx.ping,
        "frame_rx_acks": st.frame_rx.acks,
    })
}

impl QuinnTrace {
    /// Başlık + `t0` satırını yazar ve örnekleme görevini başlatır.
    ///
    /// # Errors
    /// Dosya oluşturulamaz ya da başlık yazılamazsa (koşu başlamadan).
    pub fn spawn(
        path: &Path,
        conn: quinn::Connection,
        arm: &str,
        t0_unix_ns: u64,
        stats0: &quinn::ConnectionStats,
    ) -> std::io::Result<Self> {
        let header = json!({
            "k": "start",
            "schema": CC_TRACE_SCHEMA,
            "transport": "quinn",
            "arm": arm,
            "engine": "quinn",
            "sample_ns": SAMPLE_NS,
            "clock": "t_run_ns=monotonic since t0; t_unix_ns=CLOCK_REALTIME",
        });
        let mut out = Lines::create(path, &header)?;
        let mut v0 = quinn_sample("t0", 0, stats0);
        if let Some(o) = v0.as_object_mut() {
            o.insert("t0_unix_ns".into(), json!(t0_unix_ns));
        }
        out.put(&v0);
        let t0 = Instant::now();
        let (stop, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_nanos(SAMPLE_NS));
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    _ = tick.tick() => {
                        let t = u64::try_from(t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
                        out.put(&quinn_sample("s", t, &conn.stats()));
                    }
                }
            }
            out
        });
        Ok(Self { stop, task, t0 })
    }

    /// Örneklemeyi durdurur, `end` satırını yazar, tamponu boşaltır.
    pub async fn finish(self, last: &quinn::ConnectionStats) {
        let _ = self.stop.send(());
        if let Ok(mut out) = self.task.await {
            let t = u64::try_from(self.t0.elapsed().as_nanos()).unwrap_or(u64::MAX);
            out.put(&quinn_sample("end", t, last));
            out.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adaptive_transport::sender::{Event, OpaqueUnit};

    fn read(p: &Path) -> Vec<Value> {
        std::fs::read_to_string(p)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn kama_trace_writes_header_t0_modes_samples_end() {
        let dir = std::env::temp_dir().join(format!("cctrace-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.jsonl");
        let mut s = Sender::with_cc_port();
        let mut t = KamaTrace::create(&p, "cc-port", "cc-port").unwrap();
        let c = BenchCounters::default();
        t.mark_t0(&s, 0, 42, c);
        // 250 ms boyunca 1 ms'de bir observe: 0, 100, 200 ms örnekleri (3) + ilk kip satırı.
        let _ = s.on_event(Event::AppData(OpaqueUnit { id: 0, size: 1151 }), 0);
        for ms in 0..250u64 {
            t.observe(&s, ms * 1_000_000, c);
        }
        t.finish(&s, 250_000_000, c);
        let v = read(&p);
        assert_eq!(v[0]["k"], "start");
        assert_eq!(v[0]["schema"], CC_TRACE_SCHEMA);
        assert_eq!(v[1]["k"], "t0");
        assert_eq!(v[1]["t0_unix_ns"], 42);
        let modes: Vec<_> = v.iter().filter(|x| x["k"] == "mode").collect();
        assert_eq!(modes.len(), 1, "yalnız ilk kip (Startup) — geçiş yok");
        assert_eq!(modes[0]["to"], "Startup");
        assert!(modes[0]["from"].is_null());
        let samples: Vec<u64> = v
            .iter()
            .filter(|x| x["k"] == "s")
            .map(|x| x["t_run_ns"].as_u64().unwrap())
            .collect();
        assert_eq!(samples, vec![0, 100_000_000, 200_000_000]);
        assert_eq!(v.last().unwrap()["k"], "end");
        assert_eq!(v.last().unwrap()["mode"], "Startup");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_sender_has_no_mode_lines_but_samples() {
        let dir = std::env::temp_dir().join(format!("cctrace-l-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.jsonl");
        let s = Sender::new();
        let mut t = KamaTrace::create(&p, "tail-v1", "legacy").unwrap();
        t.observe(&s, 0, BenchCounters::default());
        t.finish(&s, 1, BenchCounters::default());
        let v = read(&p);
        assert!(v.iter().all(|x| x["k"] != "mode"));
        assert!(v[1].get("mode").is_none(), "eski motorda BBR alanı yok");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn observe_does_not_change_sender_decisions() {
        // İki eş gönderici aynı olay dizisini alır; yalnız birinin yanında iz var. Karar dizileri
        // (seq, birim, tür) bayt bayt aynı olmalı.
        let dir = std::env::temp_dir().join(format!("cctrace-d-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.jsonl");
        let mut a = Sender::with_cc_port();
        let mut b = Sender::with_cc_port();
        let mut t = KamaTrace::create(&p, "cc-port", "cc-port").unwrap();
        let mut da = Vec::new();
        let mut db = Vec::new();
        for id in 0..40u64 {
            let ev = Event::AppData(OpaqueUnit { id, size: 1151 });
            da.extend(a.on_event(ev.clone(), id * 1_000_000));
            db.extend(b.on_event(ev, id * 1_000_000));
            t.observe(&b, id * 1_000_000, BenchCounters::default());
        }
        for ms in 40..400u64 {
            let ev = Event::TimerFired(adaptive_transport::sender::Timer::Pto);
            da.extend(a.on_event(ev.clone(), ms * 1_000_000));
            db.extend(b.on_event(ev, ms * 1_000_000));
            t.observe(&b, ms * 1_000_000, BenchCounters::default());
        }
        t.finish(&b, 400_000_000, BenchCounters::default());
        assert!(!da.is_empty());
        assert_eq!(da, db);
        assert!(
            read(&p).len() > 4,
            "iz yazıldı (başlık + kip + örnekler + son)"
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
