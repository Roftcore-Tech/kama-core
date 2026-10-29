//! [KTP2-F0 / Ajan D] RAKİP KOLLARIN (quinn, tcp) ADİL-KIYAS AYARLARI.
//!
//! Hepsi YALNIZ açık bayrakla gelir. Bayraksız yol eski davranışın birebir aynısıdır: bu modül
//! hiç [`set`] edilmezse [`get`] varsayılanı döner (testler ve eski hücre dosyaları etkilenmez).
//! Ayarlar süreç başına BİR kez, `main` komutu çözdükten hemen sonra yazılır; okuyan taraf
//! (`quic.rs`, `tcp.rs`, `mixed.rs`) yalnız okur.
//!
//! Neden ayrı bir modül (ClientCfg/ServerCfg alanı değil): iki struct'ın her literal'i (dört
//! modülün testleri dahil) değişmek zorunda kalırdı; aday ağacına birleştirmede çakışma yüzeyi de
//! büyürdü. Ayarlar Kama kollarını HİÇ etkilemez.
//!
//! Teslim tanımı (`--rival-delivery`):
//! * `app-reply` (eski): quinn mesaj başına bi-stream açar, sunucu tamamını okuyunca 1 B yanıt
//!   yazar; tcp sunucusu çerçeve başına 8 B uygulama ACK'i yazar. Bu yanıtlar ack-eliciting'dir:
//!   istemci bunları da ACK'lemek ZORUNDADIR → mesaj başına ileri yönde ek bir saf-ACK paketi.
//!   Düşük bantta (25 B/s, 65 B mesaj) bu, ileri yönde mesajın kendisinin ~yarısı kadar ek tel
//!   yüküdür; Kama kolunda karşılığı yoktur (Kama'nın taşıma ACK'i ACK'lenmez).
//! * `transport-ack`: teslim = eşin TAŞIMA katmanı onayı (Kama kolunun `acked()` tanımıyla aynı
//!   katman). quinn: mesaj başına tek-yönlü stream, `finish` + [`quinn::SendStream::stopped`]
//!   `Ok(None)` (eş bütün stream verisini ACK'ledi). tcp: uygulama yanıtı YOK; istemci
//!   `/proc/self/net/tcp` `tx_queue` (= write_seq − snd_una) alanını 5 ms'de bir okur, mesajın
//!   kümülatif sonu onaylanan bayt sınırını geçince teslim damgalanır (alt tahmin: erken teslim
//!   asla yazılmaz). Sunum anı PLANDIR (i × aralık); gönderen tarafı bekleme gecikmeye dahildir.

use std::sync::OnceLock;
use std::time::Duration;

use clap::ValueEnum;

/// Rakip kolların teslim tanımı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Delivery {
    /// Eski: uygulama-düzeyi yanıt (quinn 1 B bi-stream yanıtı; tcp 8 B çerçeve ACK'i).
    #[default]
    AppReply,
    /// Eşin taşıma ACK'i (quinn `SendStream::stopped`; tcp `snd_una`).
    TransportAck,
}

impl Delivery {
    /// Etiket (manifest/stderr).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::AppReply => "app-reply",
            Self::TransportAck => "transport-ack",
        }
    }
}

/// Bayraksız (eski) TCP bağlanma tavanı — `tcp.rs`'in sabit 20 s'si.
pub const LEGACY_TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Rakip kol ayarları. `Default` = eski davranış.
#[derive(Debug, Clone, Default)]
pub struct RivalTuning {
    /// Teslim tanımı.
    pub delivery: Delivery,
    /// El sıkışma / bağlanma tavanı. `None` = eski (quinn: `--max-deadline-s`; tcp: 20 s).
    pub handshake_timeout: Option<Duration>,
    /// quinn `initial_rtt`. `None` = quinn varsayılanı (333 ms).
    pub quinn_initial_rtt: Option<Duration>,
    /// quinn CC ilk penceresi (bayt). `None` = CC varsayılanı (cubic 12000, bbr 240000).
    pub quinn_iw_bytes: Option<u64>,
    /// quinn keep-alive aralığı. `None` = yok.
    pub quinn_keepalive: Option<Duration>,
    /// Koşu sonunda `QUINN-STATS` satırı (salt-gözlem; tele etkisi yok).
    pub stats: bool,
    /// [İSTEK-D1] tcp `app-reply` kolu: bütün uygulama ACK'leri gelince ya da pencere dolunca
    /// biter (eski: her koşuda sabit `min(max_deadline, 60 s)` bekler). Teslim olayları AYNIDIR —
    /// damgaları okuyucu görev koyar; erken bitiş yalnız boş beklemeyi keser.
    pub early_exit: bool,
    /// [İSTEK-D2] quinn: mesaj kimliği yükün ilk 8 baytına (BE) yazılır (alıcının `--rx-log`
    /// eşlemesi).
    /// Yük boyu ve zamanlama değişmez; yalnız yük içeriği (`0x5A` dolgu yerine kimlik) farklıdır.
    pub payload_id: bool,
}

impl RivalTuning {
    /// CLI değerlerinden kurar (`0` = verilmedi = eski davranış).
    #[must_use]
    pub fn from_cli(
        delivery: Delivery,
        handshake_timeout_s: u64,
        quinn_initial_rtt_ms: u64,
        quinn_iw_bytes: u64,
        quinn_keepalive_ms: u64,
        stats: bool,
    ) -> Self {
        let nz = |v: u64| (v > 0).then_some(v);
        Self {
            delivery,
            handshake_timeout: nz(handshake_timeout_s).map(Duration::from_secs),
            quinn_initial_rtt: nz(quinn_initial_rtt_ms).map(Duration::from_millis),
            quinn_iw_bytes: nz(quinn_iw_bytes),
            quinn_keepalive: nz(quinn_keepalive_ms).map(Duration::from_millis),
            stats,
        }
    }

    /// `transport-ack` seçili mi?
    #[must_use]
    pub fn transport_ack(&self) -> bool {
        self.delivery == Delivery::TransportAck
    }

    /// quinn kolunun eski koldan sapmaları (`RunMeta.profile_delta`; boş = eski kol AYNEN).
    #[must_use]
    pub fn quinn_delta(&self) -> Vec<String> {
        let mut d = Vec::new();
        if self.transport_ack() {
            d.push("delivery app-reply(bi-stream) -> transport-ack(uni-stream+stopped)".into());
        }
        if let Some(t) = self.handshake_timeout {
            d.push(format!(
                "handshake_timeout max_deadline -> {}s",
                t.as_secs()
            ));
        }
        if let Some(r) = self.quinn_initial_rtt {
            d.push(format!("initial_rtt 333ms -> {}ms", r.as_millis()));
        }
        if let Some(w) = self.quinn_iw_bytes {
            d.push(format!("initial_window cc-default -> {w}B"));
        }
        if let Some(k) = self.quinn_keepalive {
            d.push(format!("keep_alive none -> {}ms", k.as_millis()));
        }
        if self.payload_id {
            d.push("payload filler -> msg_id(8B BE)+filler".into());
        }
        d
    }

    /// tcp kolunun eski koldan sapmaları (boş = eski kol AYNEN).
    #[must_use]
    pub fn tcp_delta(&self) -> Vec<String> {
        let mut d = Vec::new();
        if self.transport_ack() {
            d.push(
                "delivery app-ack(8B) -> transport-ack(snd_una via /proc/self/net/tcp)".into(),
            );
            d.push("offer_clock write-time -> schedule".into());
        }
        if let Some(t) = self.handshake_timeout {
            d.push(format!("connect_timeout 20s -> {}s", t.as_secs()));
        }
        if self.early_exit && !self.transport_ack() {
            d.push("exit fixed-wait -> early-exit(all-acked)".into());
        }
        d
    }

    /// quinn kol etiketi (`RunMeta.kama_arm` — tcp.rs'in yapılandırma-etiketi emsali). Sapma
    /// yoksa boş (eski kol aynen).
    #[must_use]
    pub fn quinn_label(&self, cc: &str) -> String {
        if self.quinn_delta().is_empty() {
            return String::new();
        }
        format!(
            "quinn;cc={cc};delivery={};hs_timeout_s={};initial_rtt_ms={};iw_bytes={};\
             keepalive_ms={}",
            self.delivery.label(),
            self.handshake_timeout
                .map_or_else(|| "max_deadline".to_string(), |t| t.as_secs().to_string()),
            self.quinn_initial_rtt
                .map_or_else(|| "default".to_string(), |r| r.as_millis().to_string()),
            self.quinn_iw_bytes
                .map_or_else(|| "default".to_string(), |w| w.to_string()),
            self.quinn_keepalive
                .map_or_else(|| "none".to_string(), |k| k.as_millis().to_string()),
        )
    }
}

static TUNING: OnceLock<RivalTuning> = OnceLock::new();

/// Süreç başına bir kez (main). İkinci çağrı yok sayılır.
pub fn set(t: RivalTuning) {
    let _ = TUNING.set(t);
}

/// Yürürlükteki ayarlar; hiç `set` edilmediyse eski davranış.
pub fn get() -> &'static RivalTuning {
    TUNING.get_or_init(RivalTuning::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bayraksız CLI = eski kol: sapma listeleri boş, etiket boş.
    #[test]
    fn bayraksiz_eski_kol() {
        let t = RivalTuning::from_cli(Delivery::AppReply, 0, 0, 0, 0, false);
        assert!(t.quinn_delta().is_empty());
        assert!(t.tcp_delta().is_empty());
        assert!(t.quinn_label("bbr").is_empty());
        assert!(t.handshake_timeout.is_none() && t.quinn_initial_rtt.is_none());
    }

    /// Her bayrak sapma listesinde GÖRÜNÜR (sessiz kol farkı yok).
    #[test]
    fn her_bayrak_gorunur() {
        let t = RivalTuning::from_cli(Delivery::TransportAck, 120, 2000, 14720, 1000, true);
        let q = t.quinn_delta();
        assert_eq!(q.len(), 5, "{q:?}");
        assert!(t.quinn_label("bbr").contains("delivery=transport-ack"));
        assert_eq!(t.tcp_delta().len(), 3);
    }
}
