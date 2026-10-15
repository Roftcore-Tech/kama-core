//! [K4G-05] YALNIZ `test-util` — INT-02 ölçüm sondası ([`crate::KamaNode::probe_for_test`]).
//!
//! INT-02 (plan K4G-05) her tohum ve her kol için transport ARQ yeniden gönderim sayısını,
//! Application ve Crypto yeniden gönderimini, bağlam-önce dağıtım düşüşlerini ve oturum sayısını
//! ister. Bunların çoğunun ürün yüzeyinde ([`crate::NodeMetrics`]) salt-okunur karşılığı yoktur;
//! sonda onları düğüm iş parçacığında, tek bir komutun işlendiği anda okur. SALT OKUMADIR: hiçbir
//! motorun durumunu değiştirmez, ürün yolu (`KamaNode::start`) bu modülü derlemez.
//!
//! Kaynaklar (hepsi mevcut sayaçların okunmasıdır):
//!
//! * Transport ARQ / RTT: `ClientTransport::session_live_diags` (canlı oturum uçları) ve
//!   `ClientTransport::retired_live_diag` (ucu bırakılan oturumların sayaç toplamı) —
//!   `client_core::runtime::ClientLiveDiag`; gönderen kurtarma görüntüsü (takılı birim zaman
//!   çizelgesi) `ClientLiveDiag::recovery` içindedir.
//! * Alıcı sıralı teslim (takılı birim): `ClientTransport::reorder_diags` (`ReorderDiag`).
//! * Bağlam-önce dağıtım: `ClientTransport::ctx_dispatch_stats` (`not_ready_drops` dahil) ve
//!   `ClientTransport::delivery_dropped`.
//! * Application: `Messenger::stats` (`resent_on_new_session`, `resend_refused`, …).
//! * Oturum olayları: düğümün işlediği S1 olaylarının sayımı ([`SessionEventCounts`]).
//! * Crypto el sıkışma zarfları: süreç-içi röle kutusunun gönderim kaydı ([`EnvelopeCounts`]).

use std::collections::VecDeque;

use client_core::runtime::ClientLiveDiag;
use client_core::transport::ReorderDiag;
use kama_app::messaging::MessengerStats;
use kama_contracts::handles::SessionHandle;

/// Bir uç noktanın bayt-özdeş yeniden gönderimi tanımak için tuttuğu son zarf sayısı — yerel
/// bellek sınırı, normatif DEĞİL `[KALİBRASYON]` (SR-AUDIT-001). Meşru akışta bir uç nokta bir eşe
/// oturum başına bir INIT ya da bir RESPONSE gönderir; tavan, yeniden gönderimin ilk zarfın hemen
/// ardından gelmesini yakalamaya fazlasıyla yeter.
const RECENT_ENVELOPES: usize = 16;

/// Düğümün ölçüm sondası (bkz. modül belgesi).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeProbe {
    /// Ucu açık S1 oturumları, tutamaç sırasıyla.
    pub sessions: Vec<SessionProbe>,
    /// Ucu bırakılan (kapanan) oturumların ARQ / kurtarma sayaçlarının toplamı (RTT alanları 0;
    /// ucun kapanış boşaltmasındaki kararlar girmez — alt sınır).
    pub retired: ClientLiveDiag,
    /// [K4G-05 TEŞHİS] Sıralı teslim durumu tutulan ama ucu açık bir oturuma eşlenemeyen oturumlar
    /// (ör. supersede edilip `Closed`'u okunmamış oturum).
    pub reorder_other: Vec<ReorderDiag>,
    /// INV-ACK-DELIVER-1: beklenen ama henüz `Ready` olmayan oturumun ACK'siz düşen datagramı.
    pub not_ready_drops: u64,
    /// K4G-01: beklenen tutamaçla eşleşmeyen `Session(_)` datagramı.
    pub ctx_mismatch_drops: u64,
    /// K4G-01: HubChannel bağlamında açılıp düşen datagram.
    pub hub_frames_dropped: u64,
    /// DUR-K4G-01 (i): teslim kanalı doluyken düşen teslim.
    pub delivery_channel_drops: u64,
    /// Bütün oturum uçlarında kabul edilmiş ama teyidi gelmemiş birim (0 = Transport sakin).
    pub outstanding_units: usize,
    /// Metin motorunun gözlem sayaçları (Application yeniden gönderimi dahil).
    pub messenger: MessengerStats,
    /// Düğümün işlediği S1 oturum olayları.
    pub events: SessionEventCounts,
    /// Bu düğümün süreç-içi röleye verdiği el sıkışma zarfları.
    pub envelopes: EnvelopeCounts,
}

/// Ucu açık bir S1 oturumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionProbe {
    /// Oturum tutamacı (opak; yalnız eşitlik için).
    pub session: SessionHandle,
    /// Oturumun eşinin provisioning adı (bulunamazsa `None`).
    pub peer_id: Option<String>,
    /// Oturumun gönderim DH epoch'u.
    pub epoch: Option<u8>,
    /// Oturum ucunun motor / ARQ aynası (gönderen kurtarma görüntüsü dahil).
    pub diag: ClientLiveDiag,
    /// [K4G-05 TEŞHİS] Bu oturumda alıcı tarafın sıralı teslim durumu (karşı ucun gönderdiği
    /// birimler); durum tutulmuyorsa `None`.
    pub reorder: Option<ReorderDiag>,
}

/// Düğümün işlediği S1 oturum olaylarının sayısı.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SessionEventCounts {
    /// `Ready` (yeni S1 oturumu; supersession'ın yeni oturumu dahil).
    pub ready: u64,
    /// `Closed`.
    pub closed: u64,
    /// `RenewRequired`.
    pub renew_required: u64,
    /// `ConnectAbandoned`.
    pub connect_abandoned: u64,
}

/// Bir uç noktanın süreç-içi röleye verdiği el sıkışma zarfları (yalnız röle kabul ettiyse).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeCounts {
    /// Gönderilen INIT (SSI1).
    pub inits_sent: u64,
    /// Gönderilen RESPONSE (SSR1).
    pub responses_sent: u64,
    /// Aynı uç noktanın daha önce gönderdiği zarfla AYNI tür ve AYNI baytta zarf — Crypto
    /// yeniden gönderimi (INIT'in byte-özdeş yeniden gönderimi ya da önbellekli yanıtın yeniden
    /// verilmesi). Son [`RECENT_ENVELOPES`] zarfa karşı sınanır.
    pub identical_resends: u64,
}

/// Zarfın türü (röle kaydı için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvelopeSent {
    Init,
    Response,
}

/// Röle kutusunda tutulan gönderim kaydı (bkz. [`EnvelopeCounts`]); en çok [`RECENT_ENVELOPES`]
/// zarf baytı saklanır.
#[derive(Debug, Default)]
pub(crate) struct EnvelopeLog {
    counts: EnvelopeCounts,
    recent: VecDeque<(EnvelopeSent, Vec<u8>)>,
}

impl EnvelopeLog {
    /// Röleye kabul ettirilen bir zarfı kaydeder.
    pub(crate) fn record(&mut self, kind: EnvelopeSent, bytes: &[u8]) {
        let counter = match kind {
            EnvelopeSent::Init => &mut self.counts.inits_sent,
            EnvelopeSent::Response => &mut self.counts.responses_sent,
        };
        *counter = counter.saturating_add(1);
        let repeated = self
            .recent
            .iter()
            .any(|(seen, old)| *seen == kind && old.as_slice() == bytes);
        if repeated {
            self.counts.identical_resends = self.counts.identical_resends.saturating_add(1);
        }
        if self.recent.len() >= RECENT_ENVELOPES {
            self.recent.pop_front();
        }
        self.recent.push_back((kind, bytes.to_vec()));
    }

    /// Sayaçlar.
    pub(crate) fn counts(&self) -> EnvelopeCounts {
        self.counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tür ayrı sayılır; bayt-özdeş tekrar (aynı tür) yeniden gönderimdir, farklı tür ya da farklı
    /// bayt değildir; kayıt sınırlıdır.
    #[test]
    fn identical_resend_is_same_kind_and_same_bytes_only() {
        let mut log = EnvelopeLog::default();
        log.record(EnvelopeSent::Init, b"ssi1-a");
        log.record(EnvelopeSent::Response, b"ssi1-a");
        log.record(EnvelopeSent::Init, b"ssi1-b");
        assert_eq!(
            log.counts(),
            EnvelopeCounts {
                inits_sent: 2,
                responses_sent: 1,
                identical_resends: 0,
            }
        );
        log.record(EnvelopeSent::Init, b"ssi1-a");
        assert_eq!(log.counts().identical_resends, 1);
        for i in 0..(RECENT_ENVELOPES * 2) {
            log.record(EnvelopeSent::Response, format!("ssr1-{i}").as_bytes());
        }
        assert_eq!(log.recent.len(), RECENT_ENVELOPES, "kayıt sınırlı");
        assert_eq!(log.counts().identical_resends, 1);
    }
}
