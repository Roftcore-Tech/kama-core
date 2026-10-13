//! Düğüm olayları ([`NodeEvent`]) ve ölçüler ([`NodeMetrics`]) — plan K4G-11 §2 listesi.
//!
//! Eşler provisioning adıyla anılır (`peer_id`). Giden akış ve aktarımlar DÜĞÜM TUTAMACIYLA
//! ([`crate::StreamHandle`], [`FileHandle`]) anılır; gelen akış ve aktarımlar eşin tel kimliğiyle
//! (`(peer_id, stream_id)` / `(peer_id, transfer_id)`): API'de hiçbir çağrı gelen kimlik almaz,
//! use-after-close yüzeyi yoktur.
//!
//! **Gizlilik:** `Debug` çıktısı metni, ses yükünü, sinyalleşme blob'unu ve dosya baytlarını değil
//! uzunluklarını yazar.
//! Alınan dosyanın yolu düğümün ürettiği addır (`<eş>-<transfer_id>`; eşten gelen ad değil) ve
//! basılır.

use core::fmt;
use std::path::PathBuf;

use kama_app::codec::{FileAbortReason, PttCodec};
use kama_app::file_transfer::{FileFailure, FileSendError, SendFailure, TransferId};
use kama_app::messaging::MsgId;
use kama_app::ptt::StreamId;
use kama_contracts::handles::CloseReason;
use kama_contracts::s1::{AbandonReason, ConnectPhase};

use crate::handle::FileHandle;

/// [`crate::NodeListener::on_event`] çıktısı.
#[derive(Clone, PartialEq)]
pub enum NodeEvent {
    /// Eşe bekleyen bağlantı niyetinin fazı (S1 `ConnectProgress`; hub yolunun durumudur, eşin
    /// canlılığını BİLDİRMEZ — S1 §4). Direct-mode'da yalnız `Measuring` üretilir.
    ConnectProgress {
        /// Eş.
        peer_id: String,
        /// Faz.
        phase: ConnectPhase,
    },
    /// Eşle hazır oturum var (eşin güncel oturumu yokken oldu). Supersession'ın yeni oturumu yeni
    /// olay ÜRETMEZ: eş zaten bağlıdır.
    Connected {
        /// Eş.
        peer_id: String,
    },
    /// Eşin güncel oturumu kalmadı ya da bekleyen bağlantı niyeti bitti.
    Closed {
        /// Eş.
        peer_id: String,
        /// Neden.
        reason: CloseCause,
    },
    /// Eşten yeni metin (alıcı dedup penceresinde ilk kez görüldü; `RECEIPT(delivered)` gitti).
    TextReceived {
        /// Gönderen eş.
        peer_id: String,
        /// Gönderenin eş başına `msg_id`'si.
        msg_id: MsgId,
        /// UTF-8 metin.
        text: String,
    },
    /// Eş, `send_text`'in döndürdüğü `msg_id`'yi işlediğini bildirdi (`RECEIPT(delivered)`) —
    /// `(eş, msg_id)` başına tam bir kez. `msg_id` eş başına kalıcı ve kesin artandır (D4):
    /// `(peer_id, msg_id)` çifti tekildir.
    Receipt {
        /// Eş.
        peer_id: String,
        /// Teslim edilen metnin `msg_id`'si.
        msg_id: MsgId,
    },
    /// Giden aktarımın kilometre taşı.
    FileProgress {
        /// `send_file`'ın döndürdüğü tutamaç.
        file: FileHandle,
        /// Alıcı eş.
        peer_id: String,
        /// Aşama.
        stage: FileStage,
    },
    /// Gelen dosya tamamlandı, `whole_file_hash` tuttu (eşe `FILE_RECEIPT(ok)` gitti) ve diske
    /// yazıldı. Düğüm dizin TUTMAZ (D8): yol yalnız bu olayla verilir.
    FileReceived {
        /// Gönderen eş.
        peer_id: String,
        /// Gönderenin `transfer_id`'si.
        transfer_id: TransferId,
        /// Yazılan dosya: `<data_dir>/files/<eş>-<transfer_id>` (çakışmada `-<n>` eki).
        path: PathBuf,
        /// Bayt.
        size: u64,
    },
    /// Aktarım başarısız bitti (gelen ya da giden).
    FileFailed {
        /// Aktarım.
        file: FileRef,
        /// Karşı eş.
        peer_id: String,
        /// Neden.
        reason: FileFailReason,
    },
    /// Eş konuşmaya başladı (yeni `stream_id`'li `PTT_OPEN`).
    PttOpened {
        /// Konuşan eş.
        peer_id: String,
        /// Eşin akış kimliği.
        stream_id: StreamId,
        /// Ses kodlayıcısı (akış boyunca sabit).
        codec: PttCodec,
        /// Demet süresi (ms; akış boyunca sabit, yorumlanmaz).
        bundle_ms: u8,
    },
    /// Eşin etkin akışından ses demeti, teslim sırasıyla (PTT-R0: Reliable; geç gelen düşmez).
    PttBundle {
        /// Konuşan eş.
        peer_id: String,
        /// Eşin akış kimliği.
        stream_id: StreamId,
        /// `MEDIA_BUNDLE.payload`, aynen (opak).
        payload: Vec<u8>,
    },
    /// Eşin akışı bitti (`PTT_CLOSE`, PTT-R4 hareketsizliği ya da oturum sonu).
    PttClosed {
        /// Konuşan eş.
        peer_id: String,
        /// Eşin akış kimliği.
        stream_id: StreamId,
    },
    /// Eşten sinyalleşme blob'u (`SIGNALING_BLOB`, AMP §3 0x09; K4G-14b) — eşin
    /// `KamaNode::send_signaling`'i. İçerik opaktır ve medya motorunundur (düğüm okumaz). Alındısız
    /// ve dedupsuz: aynı blob birden çok kez gelebilir (göndericinin oturum değişiminde yeniden
    /// sunumu); sıra yalnız aynı oturum içinde gönderim sırasıdır.
    SignalingReceived {
        /// Gönderen eş.
        peer_id: String,
        /// `SIGNALING_BLOB.blob`, aynen.
        bytes: Vec<u8>,
    },
    /// Periyodik ölçü anlık görüntüsü ([`crate::METRICS_TICK`]).
    Metrics(NodeMetrics),
}

/// [`NodeEvent::Closed`] nedeni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseCause {
    /// Oturum kapandı ve eşin yerine geçen oturumu yok.
    Session(CloseReason),
    /// Bekleyen bağlantı niyeti bitti (S1 `ConnectAbandoned`; `cancel_connect` → `Cancelled`).
    Abandoned(AbandonReason),
}

/// Giden aktarımın aşaması.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStage {
    /// Son kayıt (`FILE_END`) Transport'a kabul ettirildi — teslim DEĞİL (S1 K8).
    Sent,
    /// Eş dosyayı bütün olarak aldı ve doğruladı: `FILE_RECEIPT(ok)` — teslim edildi.
    Delivered,
}

/// Başarısız aktarımın kimliği.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRef {
    /// Giden: `send_file`'ın tutamacı.
    Outgoing(FileHandle),
    /// Gelen: gönderen eşin `transfer_id`'si (olayın `peer_id`'siyle birlikte tekil).
    Incoming(TransferId),
}

/// [`NodeEvent::FileFailed`] nedeni — `kama-app`'in tipli nedenleri aynen (#123 beyanlı istisna 7
/// eşlemesi) + düğümün yerel dosya sistemi nedenleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFailReason {
    /// Gelen: özet tutmadı ya da parça gelmedi; eşe karşılık gelen `FILE_RECEIPT` gitti.
    Incoming(FileFailure),
    /// Gelen: gönderen iptal etti (`FILE_ABORT`).
    AbortedBySender(FileAbortReason),
    /// Gelen: dosya doğrulandı (eşe `FILE_RECEIPT(ok)` GİTTİ) ama diske yazılamadı. Yerel hatadır,
    /// eşe bildirilmez (`FILE_RECEIPT(ok)` birleştirme + özet sonrası gider; kama-app sözleşmesi).
    Storage(std::io::ErrorKind),
    /// Giden: aktarım teslim edilmeden bitti (eşin alındısı, zaman aşımı, supersession, iptal).
    Outgoing(SendFailure),
    /// Giden: dosya kuyruğa ALINMADI (`FileTransfer::send_file` reddi); eşe hiçbir şey gitmedi.
    NotQueued(FileSendError),
    /// Giden: kaynak dosya okunamadı (`InvalidInput` = düzenli dosya değil); eşe hiçbir şey
    /// gitmedi.
    SourceUnreadable(std::io::ErrorKind),
    /// Giden: kaynak dosyanın adı UTF-8 değil (`FILE_BEGIN.name` UTF-8 ister); eşe hiçbir şey
    /// gitmedi.
    SourceName,
}

/// Ölçüler (plan K4G-11 §2 alan adları; `deadline_expired_drops` YOK — DUR-K4G-10 CEVAP: K4G-10 bu
/// sprintte yapılmaz). `None` alanlar bugün salt-okunur bir kaynaktan ALINAMAYAN değerlerdir —
/// uydurma değer verilmez.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeMetrics {
    /// `None`: transport RTT tahminini dışarı açmıyor (yalnız kapanış istatistiğinde).
    pub rtt_ms: Option<u32>,
    /// `None`: transport kayıp oranını dışarı açmıyor.
    pub loss_pct: Option<f32>,
    /// `None`: transport bit hızını dışarı açmıyor.
    pub bitrate_kbps: Option<u32>,
    /// Hazır oturumların EN DÜŞÜK gönderim DH epoch'u (`0` / `1`); hazır oturum yoksa `None`.
    /// Eş başına değer [`PeerConnectState::epoch`]'ta. En düşük değer, bütün hazır oturumlar
    /// epoch-1'e geçince `1` olur (PoC-KAPSAM-001 K1 "her eş için epoch-1").
    pub epoch: Option<u8>,
    /// PoC'de daima `None`: heartbeat hub sprintindedir (D18).
    pub last_heartbeat_age_s: Option<u64>,
    /// K4G-01: beklenen tutamaçla birebir eşleşmeyen `Session(_)` datagramı (düğüm geneli).
    pub ctx_mismatch_drops: u64,
    /// K4G-01: HubChannel bağlamında açılan ve düşürülen datagram (direct-mode'da hub yok).
    pub hub_frames_dropped: u64,
    /// DUR-K4G-01 CEVAP (i): veri alıcısının teslim kanalı doluyken düşen teslim
    /// (`ServerStats::delivery_dropped`). INT-02'de 0 olmalı.
    pub delivery_channel_drops: u64,
    /// Eş başına bağlantı durumu, ad sırasıyla (provisioning dosyasındaki bütün eşler).
    pub connect_state: Vec<PeerConnectState>,
}

/// Bir eşin bağlantı durumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerConnectState {
    /// Eş.
    pub peer_id: String,
    /// Durum.
    pub state: LinkState,
    /// Eşin güncel oturumunun gönderim DH epoch'u; hazır oturum yoksa `None`.
    pub epoch: Option<u8>,
}

/// Eşin bağlantı durumu (UI). Eşin canlılığını BİLDİRMEZ: canlılığın tek sinyali alındılardır
/// (`Receipt`, `FileProgress::Delivered`; S1 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// Hazır oturum da bekleyen niyet de yok.
    Idle,
    /// Bekleyen niyet ve son fazı.
    Connecting(ConnectPhase),
    /// Eşin güncel oturumu var.
    Connected,
}

/// Metin, ses yükü, sinyalleşme blob'u ve dosya baytları YAZILMAZ; yalnız uzunlukları.
impl fmt::Debug for NodeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectProgress { peer_id, phase } => f
                .debug_struct("ConnectProgress")
                .field("peer_id", peer_id)
                .field("phase", phase)
                .finish(),
            Self::Connected { peer_id } => f
                .debug_struct("Connected")
                .field("peer_id", peer_id)
                .finish(),
            Self::Closed { peer_id, reason } => f
                .debug_struct("Closed")
                .field("peer_id", peer_id)
                .field("reason", reason)
                .finish(),
            Self::TextReceived {
                peer_id,
                msg_id,
                text,
            } => f
                .debug_struct("TextReceived")
                .field("peer_id", peer_id)
                .field("msg_id", msg_id)
                .field("text_len", &text.len())
                .finish_non_exhaustive(),
            Self::Receipt { peer_id, msg_id } => f
                .debug_struct("Receipt")
                .field("peer_id", peer_id)
                .field("msg_id", msg_id)
                .finish(),
            Self::FileProgress {
                file,
                peer_id,
                stage,
            } => f
                .debug_struct("FileProgress")
                .field("file", file)
                .field("peer_id", peer_id)
                .field("stage", stage)
                .finish(),
            Self::FileReceived {
                peer_id,
                transfer_id,
                path,
                size,
            } => f
                .debug_struct("FileReceived")
                .field("peer_id", peer_id)
                .field("transfer_id", transfer_id)
                .field("path", path)
                .field("size", size)
                .finish(),
            Self::FileFailed {
                file,
                peer_id,
                reason,
            } => f
                .debug_struct("FileFailed")
                .field("file", file)
                .field("peer_id", peer_id)
                .field("reason", reason)
                .finish(),
            Self::PttOpened {
                peer_id,
                stream_id,
                codec,
                bundle_ms,
            } => f
                .debug_struct("PttOpened")
                .field("peer_id", peer_id)
                .field("stream_id", stream_id)
                .field("codec", codec)
                .field("bundle_ms", bundle_ms)
                .finish(),
            Self::PttBundle {
                peer_id,
                stream_id,
                payload,
            } => f
                .debug_struct("PttBundle")
                .field("peer_id", peer_id)
                .field("stream_id", stream_id)
                .field("payload_len", &payload.len())
                .finish_non_exhaustive(),
            Self::PttClosed { peer_id, stream_id } => f
                .debug_struct("PttClosed")
                .field("peer_id", peer_id)
                .field("stream_id", stream_id)
                .finish(),
            Self::SignalingReceived { peer_id, bytes } => f
                .debug_struct("SignalingReceived")
                .field("peer_id", peer_id)
                .field("bytes_len", &bytes.len())
                .finish_non_exhaustive(),
            Self::Metrics(metrics) => f.debug_tuple("Metrics").field(metrics).finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_text_audio_and_keeps_lengths() {
        let secret_text = "gizli-metin-içeriği";
        let text = NodeEvent::TextReceived {
            peer_id: "alice".to_owned(),
            msg_id: 3,
            text: secret_text.to_owned(),
        };
        let shown = format!("{text:?}");
        assert!(!shown.contains(secret_text), "{shown}");
        assert!(
            shown.contains(&format!("text_len: {}", secret_text.len())),
            "{shown}"
        );

        const AUDIO_BYTE: u8 = 0xA7;
        const AUDIO_LEN: usize = 40;
        let audio = NodeEvent::PttBundle {
            peer_id: "alice".to_owned(),
            stream_id: 1,
            payload: vec![AUDIO_BYTE; AUDIO_LEN],
        };
        let shown = format!("{audio:?}");
        assert!(
            shown.contains(&format!("payload_len: {AUDIO_LEN}")),
            "{shown}"
        );
        // Ses baytı ne ondalık ne onaltılık basılır.
        for needle in [AUDIO_BYTE.to_string(), format!("{AUDIO_BYTE:x}")] {
            assert!(
                !shown.contains(&needle),
                "Debug çıktısında {needle}: {shown}"
            );
        }
    }

    /// K4G-14b: sinyalleşme blob'unun baytları `Debug`'a girmez; uzunluğu girer.
    #[test]
    fn debug_redacts_the_signaling_blob_and_keeps_its_length() {
        const BLOB_BYTE: u8 = 0xC3;
        const BLOB_LEN: usize = 48;
        let blob = NodeEvent::SignalingReceived {
            peer_id: "alice".to_owned(),
            bytes: vec![BLOB_BYTE; BLOB_LEN],
        };
        let shown = format!("{blob:?}");
        assert!(shown.contains(&format!("bytes_len: {BLOB_LEN}")), "{shown}");
        // Blob baytı ne ondalık ne onaltılık basılır.
        for needle in [BLOB_BYTE.to_string(), format!("{BLOB_BYTE:x}")] {
            assert!(
                !shown.contains(&needle),
                "Debug çıktısında {needle}: {shown}"
            );
        }
    }
}
