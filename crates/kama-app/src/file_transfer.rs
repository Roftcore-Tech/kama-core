//! Dosya aktarımı (B2.3) — AMP `FILE_BEGIN` (0x02) · `FILE_CHUNK` (0x03) · `FILE_END` (0x04) ·
//! `FILE_ABORT` (0x05) (`specs/application-message-protocol.md` §3–§4; S7/S8 2026-10-08).
//!
//! [`FileTransfer`] [`Messenger`]'ın yanında durur ve Transport'u sahiplenmez.
//! [`Messenger::poll`]'u OKUMAZ: olayları zincirdeki okuyucudan alır (B2.4'te `Ptt::poll` →
//! `PttEvent::Messenger`).
//!
//! * **Gelen** — [`FileTransfer::handle`] `FILE_*` kaydı taşıyan
//!   [`MessengerEvent::Other`](crate::messaging::MessengerEvent::Other)'ı TÜKETİR, diğer her olayı
//!   AYNEN geri verir. (Eş, `transfer_id`) başına birleştirir: parçalar ve `FILE_END` her sırada
//!   gelebilir, tekrarlar etkisizdir. `FILE_END` görülmüş ve bütün parçalar gelmişse
//!   `whole_file_hash` doğrulanır: tutarsa [`FileEvent::Received`]; tutmazsa [`FileEvent::Failed`]
//!   ve eşe `FILE_ABORT(hash_mismatch)`.
//! * **Giden** — [`FileTransfer::send_file`] dosyayı (bellekte; bu sprint dosya sistemi yok)
//!   kuyruğa alıp `transfer_id` atar; [`FileTransfer::pump`] kayıtları eşin güncel oturumuna
//!   ([`Messenger::session_of`] + [`Messenger::transport_mut`]) `DeliveryPolicy::Reliable` ile
//!   sırayla sunar: `FILE_BEGIN` → `FILE_CHUNK` × `chunk_count` → `FILE_END`. Sıkıştırma yok (bu
//!   sprint `compression_id` = NONE; AMP §5 kural 6: `FILE_CHUNK` → kapalı).
//!
//! Motorun kendi olayları [`FileTransfer::poll_event`] kuyruğundadır. Bir uygulama turu:
//!
//! ```text
//! while let Some(ev) = ptt.poll(&mut m) { PttEvent::Messenger(ev) → files.handle(&mut m, ev) → … }
//! files.pump(&mut m);
//! while let Some(fe) = files.poll_event() { … }
//! ```
//!
//! # Retry yok (AMP §3, S1 K8, H-APP-RETRY-0)
//!
//! Application parça yeniden gönderimi, parça ACK'i, kayıp tespiti, zamanlayıcı ve parça başına
//! MAC/özet TUTMAZ: kayıp Transport'un (`Reliable`), bütünlük Crypto'nun işidir; bütün-dosya özeti
//! yalnız birleştirmenin doğruluğunu sınar. Transport'un KABUL ettiği bir kayıt bir daha sunulmaz —
//! oturum değişiminde de (S2'nin TEXT yeniden gönderimi dosyaya UYGULANMAZ; resume NON-GATING).
//! Transport'un REDDETTİĞİ kayıt tele çıkmamıştır: kuyrukta bekler ve sonraki
//! [`FileTransfer::pump`]'ta yeniden sunulur (K8 "bekler"). Özet tutmazsa çözüm parça yeniden
//! göndermek DEĞİLDİR; Transport'un Reliable sözleşmesine bakılır (BL B2.3).
//!
//! # Parça geometrisi
//!
//! Gönderen parça uzunluğu `L` olarak `MAX_FILE_CHUNK_BYTES`'ı alır (A2;
//! [`AmpLimits::max_file_chunk_bytes`](crate::codec::AmpLimits::max_file_chunk_bytes)): parça `i` =
//! `[i·L, min((i+1)·L, file_size))`, `chunk_count = ⌈file_size / L⌉`. Spec yerleşim koymaz ve
//! alıcı gönderenin limitlerini bilmez (iki uçta farklı olabilir): `L`'yi KABUL ettiği ilk
//! parçadan öğrenir — son olmayan parçanın uzunluğu `L`'dir, son parça `(chunk_count − 1)·L`'de
//! başlar. `⌈file_size / L⌉ = chunk_count` sağlamayan aday reddedilir; sonraki her parça `L`
//! sınırında ve `min(L, file_size − offset)` uzunluğunda olmalıdır. Gönderen, aynı eşe giden
//! aktarımları oluşturma sırasıyla TEK TEK sunar; eşler arasında tur başına bir kayıt (adil).
//!
//! # Alıcı kararları (S8: vektörsüz karar → fail-closed DROP; panik yok, olay yok)
//!
//! * `FILE_BEGIN`: `(file_size = 0) ≠ (chunk_count = 0)` ya da `chunk_count > file_size` → DROP.
//!   `file_size > MAX_REASSEMBLY_BYTES` (S7) ya da yuva yok → aktarım alınmaz, eşe
//!   `FILE_ABORT(storage_full)` (spec'in neden kodu; tek deneme). Boyut her tahsisten ÖNCE
//!   denetlenir; birleştirme tamponu ve parça bit haritası ilk kabul edilen parçada ayrılır. Aynı
//!   (eş, `transfer_id`) için aynı alanlar → tekrar; farklı alanlar → DROP.
//! * `FILE_CHUNK` / `FILE_END`: bilinmeyen aktarım → DROP (ön-tampon yok: aynı oturumda
//!   `FILE_BEGIN` her zaman önce gelir, S1 §4). Geometriye uymayan parça → DROP; durum bozulmaz.
//!   Eksik parça varken gelen `FILE_END` aktarımı bitirmez: eksikler beklenir (oturumlar arası
//!   sırasızlık).
//!
//! # `FILE_ABORT` yönü (sahibe açık — AMP §3'te yön alanı yok)
//!
//! `transfer_id` gönderen başınadır; iki eş birbirine aynı kimlikle dosya gönderebilir. Yön
//! nedenden çıkarılır: `hash_mismatch` ve `storage_full` yalnız alıcının kararıdır → eşe GİDEN
//! aktarıma; `user_cancel` ve `timeout` → eşten GELEN aktarıma. Bu motor `user_cancel`'ı yalnız
//! gönderen olarak, `timeout`'u hiç üretmez; alıcı tarafında bırakma
//! ([`FileTransfer::abandon_incoming`]) sessizdir. Kimlik başka bir eşin aktarımıyla eşleşmez.
//!
//! # `transfer_id`
//!
//! Bu [`FileTransfer`] örneğinin tek, kesin artan sayacı ([`FIRST_TRANSFER_ID`]'den); YALNIZ
//! [`FileTransfer::send_file`] `Ok` döndüğünde tüketilir, hata yolu tüketmez. Eş başına da artar
//! (boşluklu). `u64` tükenirse [`FileSendError::TransferIdExhausted`]; sarma YOK.
//!
//! # Sınırlar ve olay kuyruğu
//!
//! Alıcı: aktarım başına `file_size ≤ MAX_REASSEMBLY_BYTES` (S7); eşzamanlı gelen aktarım eş başına
//! [`MAX_INCOMING_TRANSFERS_PER_PEER`], toplam [`MAX_INCOMING_TRANSFERS`]. Gönderen:
//! [`MAX_OUTGOING_TRANSFERS`] aktarım, dosya baytlarıyla. Her olay doğduğu aktarımın YUVASINI
//! [`FileTransfer::poll_event`] ile alınana kadar tutar: alınmamış alıcı olayları toplam gelen
//! tavanına, gönderen olayları giden tavanına sayılır. Böylece kuyruk tavanı türetilmiştir (iki
//! tavanın toplamı; yeni sayı yok), olay düşürülmez ve boşaltılmayan kuyruk yeni aktarımları
//! durdurur (geri basınç). En kötü durumda birleştirme belleği `MAX_INCOMING_TRANSFERS ×
//! MAX_REASSEMBLY_BYTES`'tır (+ bit haritası, aktarım başına ≤ `file_size / 8`).
//!
//! # Bilinen sınırlar (sahibe açık)
//!
//! * AMP v1'de dosya düzeyinde teslim onayı yok (RECEIPT yalnız TEXT için): gönderen
//!   [`FileEvent::Sent`] (Transport kabulü, teslim DEĞİL) ve alıcının `FILE_ABORT`'u dışında sinyal
//!   almaz. `FILE_END` kabul edildikten sonra gelen `hash_mismatch` izlenmeyen aktarıma düşer
//!   (sayılır, olay yok).
//! * Zamanlayıcı yok: göndereni kaybolmuş (yeniden başlamış ya da birimleri eski oturumla düşmüş,
//!   S1 K7) yarım gelen aktarım `FILE_ABORT`'a ya da [`FileTransfer::abandon_incoming`]'e kadar
//!   tabloda kalır; tavan doluyken yeni aktarım `storage_full` ile reddedilir.
//! * Sayaç kalıcı değil: yeniden başlayan bir gönderen eşte hâlâ duran bir kimliği yeniden
//!   kullanırsa farklı alanlı `FILE_BEGIN` DROP edilir ve o aktarım tamamlanmaz.
//!
//! # `whole_file_hash` (blake2 DAR istisnası, Abkarada 2026-10-08)
//!
//! BLAKE2s-256 (anahtarsız, 32 B; AMP §3 0x02 "ccr primitive") YALNIZ [`whole_file_hash`]'te
//! hesaplanır — `blake2`'nin bu crate'teki tek kullanımı; `scripts/ci/layer_check.py` L1 onu adıyla
//! geçirir. Özet gizli değildir; karşılaştırma sabit-zamanlı değildir.
//!
//! # Gizlilik
//!
//! [`FileEvent`]'in `Debug` çıktısı dosya adını ve baytlarını değil uzunluklarını yazar; kuyruk ve
//! birleştirme durumu hiçbir `Debug` çıktısına girmez ([`FileTransfer`] `Debug` değildir).

use core::fmt;
use std::collections::{BTreeMap, VecDeque};

use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{DeliveryPolicy, KamaTransport, TransportError};

use crate::codec::limits::WHOLE_FILE_HASH_BYTES;
use crate::codec::{self, AppMessage, EncodeError, FileAbortReason};
use crate::messaging::{Messenger, MessengerEvent};

/// `FILE_*.transfer_id` (AMP §3; telde `u64_be`).
pub type TransferId = u64;

/// Atanan ilk `transfer_id`. Spec başlangıç değeri koymaz; değer bu uygulamanındır.
pub const FIRST_TRANSFER_ID: TransferId = 0;

/// S7 (Abkarada 2026-10-08) — alıcının birleştirdiği en büyük dosya (`FILE_BEGIN.file_size`,
/// bayt). Gönderen de daha büyüğünü kuyruğa almaz: eş onu `storage_full` ile reddeder.
pub const MAX_REASSEMBLY_BYTES: u64 = 64 * 1024 * 1024;

/// Eş başına eşzamanlı gelen aktarım tavanı — yerel bellek sınırı, normatif DEĞİL `[KALİBRASYON]`.
/// Bu motorun göndereni bir eşe aktarımları tek tek gönderir; ikinci yuva, göndereni kaybolmuş
/// yarım bir aktarım dururken yenisine yer bırakır.
pub const MAX_INCOMING_TRANSFERS_PER_PEER: usize = 2;

/// Toplam gelen aktarım yuvası (alınmamış alıcı olayları dahil) — yerel bellek sınırı, normatif
/// DEĞİL `[KALİBRASYON]`.
pub const MAX_INCOMING_TRANSFERS: usize = 4;

/// Giden aktarım yuvası (alınmamış gönderen olayları dahil; dosya baytları son kayıt kabul edilene
/// kadar tutulur) — yerel bellek sınırı, normatif DEĞİL `[KALİBRASYON]`.
pub const MAX_OUTGOING_TRANSFERS: usize = 4;

/// Olay kuyruğu tavanı — türetilmiş: her olay doğduğu aktarımın yuvasını tutar (modül belgesi,
/// "Sınırlar ve olay kuyruğu").
const MAX_PENDING_FILE_EVENTS: usize = MAX_INCOMING_TRANSFERS + MAX_OUTGOING_TRANSFERS;

/// Parça bit haritasının kelime genişliği (bit).
const SEEN_WORD_BITS: u64 = u64::BITS as u64;

// Eş başına tavan toplam tavanı aşamaz.
const _: () = assert!(MAX_INCOMING_TRANSFERS_PER_PEER <= MAX_INCOMING_TRANSFERS);

/// Gelen aktarım anahtarı: (gönderen eş, onun `transfer_id`'si).
type IncomingKey = (PeerRef, TransferId);

/// AMP §3 0x02 `FILE_BEGIN.whole_file_hash`: anahtarsız BLAKE2s-256 (32 B). `blake2`'nin bu
/// crate'teki TEK kullanımı (DAR istisna, Abkarada 2026-10-08; `scripts/ci/layer_check.py` L1).
#[must_use]
pub fn whole_file_hash(bytes: &[u8]) -> [u8; WHOLE_FILE_HASH_BYTES] {
    use blake2::{Blake2s256, Digest};
    let mut hasher = Blake2s256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// [`FileEvent::Failed`] nedeni.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFailure {
    /// Birleştirilen baytların BLAKE2s-256 özeti `FILE_BEGIN.whole_file_hash`'e eşit değil.
    HashMismatch,
}

/// [`FileTransfer::poll_event`] çıktısı.
#[derive(Clone, PartialEq, Eq)]
pub enum FileEvent {
    /// Gelen dosya tamamlandı ve `whole_file_hash` tuttu; birleştirme durumu atıldı.
    Received {
        /// Gönderen eş.
        peer: PeerRef,
        /// Gönderenin `transfer_id`'si.
        transfer_id: TransferId,
        /// `FILE_BEGIN.name` (UTF-8, ≤ 255 B; codec doğruladı).
        name: String,
        /// Birleştirilen dosya.
        bytes: Vec<u8>,
    },
    /// Gelen dosyanın bütün parçaları geldi ama doğrulanamadı: baytlar atıldı, eşe
    /// `FILE_ABORT(hash_mismatch)` gönderildi (tek deneme).
    Failed {
        /// Gönderen eş.
        peer: PeerRef,
        /// Gönderenin `transfer_id`'si.
        transfer_id: TransferId,
        /// Neden.
        reason: FileFailure,
    },
    /// Eş gelen aktarımı iptal etti (`FILE_ABORT` — `user_cancel` ya da `timeout`); durum atıldı.
    Aborted {
        /// Gönderen eş.
        peer: PeerRef,
        /// Gönderenin `transfer_id`'si.
        transfer_id: TransferId,
        /// Eşin bildirdiği neden.
        reason: FileAbortReason,
    },
    /// Giden aktarımın son kaydı (`FILE_END`) Transport'a kabul ettirildi. Teslim DEĞİL (S1 K8):
    /// AMP v1'de dosya düzeyinde onay yok.
    Sent {
        /// Alıcı eş.
        peer: PeerRef,
        /// [`FileTransfer::send_file`]'ın döndürdüğü kimlik.
        transfer_id: TransferId,
    },
    /// Eş giden aktarımı reddetti (`FILE_ABORT` — `storage_full` ya da `hash_mismatch`); kalan
    /// kayıtlar sunulmaz.
    SendAborted {
        /// Alıcı eş.
        peer: PeerRef,
        /// [`FileTransfer::send_file`]'ın döndürdüğü kimlik.
        transfer_id: TransferId,
        /// Eşin bildirdiği neden.
        reason: FileAbortReason,
    },
    /// Transport bir kaydı kalıcı hatayla reddetti (geçici olmayan her `TransportError`); aktarım
    /// bırakıldı, eşe sinyal gitmedi (AMP §3'te gönderen-tarafı hata kodu yok).
    SendFailed {
        /// Alıcı eş.
        peer: PeerRef,
        /// [`FileTransfer::send_file`]'ın döndürdüğü kimlik.
        transfer_id: TransferId,
        /// Transport'un hatası.
        error: TransportError,
    },
}

impl FileEvent {
    /// Gelen aktarımın olayı mı (yuvası gelen tavanına sayılır); değilse giden aktarımın.
    const fn is_incoming(&self) -> bool {
        matches!(
            self,
            Self::Received { .. } | Self::Failed { .. } | Self::Aborted { .. }
        )
    }
}

/// Dosya adı ve baytları YAZILMAZ; yalnız uzunlukları. `PeerRef` kendi `Debug`'ıyla (içerik
/// basmaz).
impl fmt::Debug for FileEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Received {
                peer,
                transfer_id,
                name,
                bytes,
            } => f
                .debug_struct("Received")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .field("name_len", &name.len())
                .field("len", &bytes.len())
                .finish_non_exhaustive(),
            Self::Failed {
                peer,
                transfer_id,
                reason,
            } => f
                .debug_struct("Failed")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .field("reason", reason)
                .finish(),
            Self::Aborted {
                peer,
                transfer_id,
                reason,
            } => f
                .debug_struct("Aborted")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .field("reason", reason)
                .finish(),
            Self::Sent { peer, transfer_id } => f
                .debug_struct("Sent")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .finish(),
            Self::SendAborted {
                peer,
                transfer_id,
                reason,
            } => f
                .debug_struct("SendAborted")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .field("reason", reason)
                .finish(),
            Self::SendFailed {
                peer,
                transfer_id,
                error,
            } => f
                .debug_struct("SendFailed")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .field("error", error)
                .finish(),
        }
    }
}

/// [`FileTransfer::send_file`] ve [`FileTransfer::cancel_send`] hatası. Her hata yolunda durum
/// DEĞİŞMEZ: `transfer_id` tüketilmez, kuyruğa bir şey girmez. Motor hiçbirini yeniden denemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSendError {
    /// Eşin hazır oturumu yok ([`Messenger::session_of`] → `None`); dosya kuyruğa ALINMAZ.
    NoSession,
    /// [`MAX_OUTGOING_TRANSFERS`] giden yuva dolu (kuyruktaki aktarımlar + alınmamış gönderen
    /// olayları) — geri basınç.
    QueueFull,
    /// `transfer_id` alanı tükendi; sarma YOK.
    TransferIdExhausted,
    /// Dosya [`MAX_REASSEMBLY_BYTES`]'ı aşıyor: eş birleştirmez (S7).
    TooLarge {
        /// Dosya boyu (bayt; taşmada `u64::MAX`).
        file_size: u64,
    },
    /// Boş olmayan dosya için `MAX_FILE_CHUNK_BYTES = 0`: Transport'un birimi parça yükü taşımıyor.
    NoChunkRoom,
    /// `FILE_BEGIN` bir kayda sığmıyor (ad 255 baytı ya da gövde `MAX_BODY_BYTES`'ı aşıyor).
    Encode(EncodeError),
    /// İzlenen giden aktarım değil (hiç yoktu ya da son kaydı kabul edildi).
    UnknownTransfer,
}

impl fmt::Display for FileSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSession => f.write_str("peer has no ready session"),
            Self::QueueFull => write!(
                f,
                "all {MAX_OUTGOING_TRANSFERS} outgoing transfer slots are in use"
            ),
            Self::TransferIdExhausted => f.write_str("transfer_id space is exhausted (no wrap)"),
            Self::TooLarge { file_size } => write!(
                f,
                "file of {file_size} bytes exceeds MAX_REASSEMBLY_BYTES {MAX_REASSEMBLY_BYTES}"
            ),
            Self::NoChunkRoom => {
                f.write_str("MAX_FILE_CHUNK_BYTES is 0: the transport unit cannot carry file data")
            }
            Self::Encode(e) => write!(f, "FILE_BEGIN does not fit one AMP record: {e}"),
            Self::UnknownTransfer => f.write_str("no such outgoing transfer"),
        }
    }
}

impl std::error::Error for FileSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::NoSession
            | Self::QueueFull
            | Self::TransferIdExhausted
            | Self::TooLarge { .. }
            | Self::NoChunkRoom
            | Self::UnknownTransfer => None,
        }
    }
}

/// Gözlem sayaçları — salt-okunur; protokol davranışını değiştirmez.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FileTransferStats {
    /// Transport'un kabul ettiği giden `FILE_*` kayıtları (gönderen kuyruğu). Her kayıt EN ÇOK bir
    /// kez kabul ettirilir.
    pub records_accepted: u64,
    /// Transport'un geçici reddi (geri basınç, eski tutamaç): kayıt tele çıkmadı, kuyrukta bekler.
    pub offers_refused: u64,
    /// Bilinmeyen aktarıma ait `FILE_CHUNK` / `FILE_END` (atıldı).
    pub unknown_transfer_records: u64,
    /// Geometriye ya da `FILE_BEGIN`'e uymayan kayıtlar (atıldı, S8).
    pub inconsistent_records: u64,
    /// Tekrar `FILE_BEGIN` / `FILE_CHUNK` / `FILE_END` (etkisiz).
    pub duplicate_records: u64,
    /// `storage_full` ile reddedilen gelen aktarımlar (S7 boyutu ya da yuva yok).
    pub transfers_refused: u64,
    /// Transport'un kabul ettiği alıcı-tarafı `FILE_ABORT`'lar.
    pub aborts_sent: u64,
    /// Gönderilemeyen alıcı-tarafı `FILE_ABORT`'lar (oturum yok ya da Transport reddi; yeniden
    /// denenmez).
    pub aborts_unsent: u64,
    /// İzlenen aktarıma denk gelmeyen `FILE_ABORT`'lar (yok sayıldı).
    pub aborts_ignored: u64,
}

/// B2.3 dosya aktarım motoru. Ayrıntı: modül belgesi.
pub struct FileTransfer {
    /// Sıradaki `transfer_id`; `None` = `u64` alanı tükendi.
    next_transfer_id: Option<TransferId>,
    /// Giden aktarımlar, `transfer_id` (= oluşturma) sırasıyla (+ alınmamış gönderen olayları
    /// ≤ [`MAX_OUTGOING_TRANSFERS`]).
    outgoing: BTreeMap<TransferId, Outgoing>,
    /// Gelen aktarımlar (+ alınmamış alıcı olayları ≤ [`MAX_INCOMING_TRANSFERS`]; eş başına
    /// ≤ [`MAX_INCOMING_TRANSFERS_PER_PEER`]).
    incoming: BTreeMap<IncomingKey, Incoming>,
    /// Alınmayı bekleyen olaylar, doğuş sırasıyla (≤ `MAX_PENDING_FILE_EVENTS`).
    events: VecDeque<FileEvent>,
    /// [`FileTransfer::pump`] turunda kaydı sunulmuş eşler — her turda yeniden kullanılan tampon
    /// (≤ [`MAX_OUTGOING_TRANSFERS`]).
    served: Vec<PeerRef>,
    stats: FileTransferStats,
}

impl Default for FileTransfer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileTransfer {
    /// Boş motor: kuyruklar ve birleştirme tablosu boş, sayaç [`FIRST_TRANSFER_ID`]'de.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_transfer_id: Some(FIRST_TRANSFER_ID),
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
            events: VecDeque::new(),
            served: Vec::new(),
            stats: FileTransferStats::default(),
        }
    }

    /// Gözlem sayaçları.
    #[must_use]
    pub fn stats(&self) -> FileTransferStats {
        self.stats
    }

    /// Kuyruktaki (son kaydı henüz kabul edilmemiş) giden aktarım sayısı.
    #[must_use]
    pub fn outgoing_count(&self) -> usize {
        self.outgoing.len()
    }

    /// Birleştirilmekte olan gelen aktarım sayısı.
    #[must_use]
    pub fn incoming_count(&self) -> usize {
        self.incoming.len()
    }

    /// Sıradaki olay, doğuş sırasıyla; yoksa `None`. Alınan olay aktarımının yuvasını boşaltır.
    #[must_use]
    pub fn poll_event(&mut self) -> Option<FileEvent> {
        self.events.pop_front()
    }

    /// Dosyayı `peer`'e gönderilmek üzere kuyruğa alır ve atanan `transfer_id`'yi döndürür.
    /// Kayıtlar [`FileTransfer::pump`] ile sunulur: `Ok` hiçbir kaydın kabul edildiği anlamına
    /// gelmez. `messenger` sonraki `pump` çağrılarındakiyle aynı motor olmalıdır (parça uzunluğu
    /// onun limitlerinden alınır).
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`FileSendError::NoSession`] · [`FileSendError::QueueFull`] ·
    /// [`FileSendError::TransferIdExhausted`] · [`FileSendError::TooLarge`] (özetten ÖNCE) ·
    /// [`FileSendError::NoChunkRoom`] · [`FileSendError::Encode`]. Hiçbiri `transfer_id` tüketmez.
    pub fn send_file<T: KamaTransport>(
        &mut self,
        messenger: &Messenger<T>,
        peer: PeerRef,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<TransferId, FileSendError> {
        if messenger.session_of(peer).is_none() {
            return Err(FileSendError::NoSession);
        }
        if self.outgoing.len() + self.queued_events(false) >= MAX_OUTGOING_TRANSFERS {
            return Err(FileSendError::QueueFull);
        }
        let transfer_id = self
            .next_transfer_id
            .ok_or(FileSendError::TransferIdExhausted)?;
        // S7: boyut özetten ÖNCE.
        let file_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if file_size > MAX_REASSEMBLY_BYTES {
            return Err(FileSendError::TooLarge { file_size });
        }
        let chunk_len = messenger.limits().max_file_chunk_bytes();
        let chunk_count = if bytes.is_empty() {
            0
        } else if chunk_len == 0 {
            return Err(FileSendError::NoChunkRoom);
        } else {
            // `file_size ≤ MAX_REASSEMBLY_BYTES` ve `chunk_len ≥ 1` ⇒ `chunk_count ≤ 2^26`.
            u32::try_from(bytes.len().div_ceil(chunk_len))
                .map_err(|_| FileSendError::TooLarge { file_size })?
        };
        let transfer = Outgoing {
            peer,
            name: name.to_owned(),
            file_size,
            whole_file_hash: whole_file_hash(&bytes),
            chunk_len,
            chunk_count,
            next: Step::Begin,
            bytes,
        };
        // `FILE_BEGIN` bir kayda sığmalı (ad ≤ 255 B, gövde ≤ `MAX_BODY_BYTES`). Parçalar
        // `chunk_len` gereği, `FILE_END` / `FILE_ABORT` her geçerli `AmpLimits`'te sığar.
        codec::encode(&transfer.message(transfer_id), messenger.limits())
            .map_err(FileSendError::Encode)?;
        self.outgoing.insert(transfer_id, transfer);
        self.next_transfer_id = transfer_id.checked_add(1);
        Ok(transfer_id)
    }

    /// Giden aktarımı iptal eder. `FILE_BEGIN` henüz kabul edilmediyse aktarım sessizce düşer (eş
    /// onu hiç görmedi); edildiyse kalan kayıtların yerine `FILE_ABORT(user_cancel)` geçer ve
    /// sıradaki [`FileTransfer::pump`]'ta sunulur (kabul edilene kadar bekler — kabul edilmemiş
    /// kayıt tele çıkmamıştır, yeniden iletim DEĞİL). Dosya baytları hemen bırakılır. Olay yok.
    ///
    /// # Errors
    ///
    /// [`FileSendError::UnknownTransfer`] — izlenen giden aktarım değil (son kaydı kabul edildiyse
    /// de: o andan sonra eş aktarımı kendi verisiyle bitirir).
    pub fn cancel_send(&mut self, transfer_id: TransferId) -> Result<(), FileSendError> {
        let transfer = self
            .outgoing
            .get_mut(&transfer_id)
            .ok_or(FileSendError::UnknownTransfer)?;
        let step = transfer.next;
        match step {
            Step::Begin => {
                self.outgoing.remove(&transfer_id);
            }
            Step::Chunk(_) | Step::End => {
                transfer.next = Step::Abort(FileAbortReason::UserCancel);
                transfer.bytes = Vec::new();
            }
            Step::Abort(_) => {}
        }
        Ok(())
    }

    /// Gelen aktarımı yerel olarak bırakır (ör. göndereni kaybolmuş yarım aktarım — zamanlayıcı
    /// yok). Eşe sinyal GİTMEZ (modül belgesi, "`FILE_ABORT` yönü"); eşin sonraki kayıtları
    /// bilinmeyen aktarım olarak atılır. Olay yok. `true` = durum vardı ve atıldı.
    pub fn abandon_incoming(&mut self, peer: PeerRef, transfer_id: TransferId) -> bool {
        self.incoming.remove(&(peer, transfer_id)).is_some()
    }

    /// Kuyruktaki kayıtları sunar; ilerleme kalmayınca döner. Biten aktarımların olayları
    /// ([`FileEvent::Sent`], [`FileEvent::SendFailed`]) [`FileTransfer::poll_event`] kuyruğuna
    /// girer.
    ///
    /// Her turda eş başına yalnız EN ESKİ aktarımın sıradaki kaydı BİR KEZ sunulur (eşler arası
    /// adil, eş içi sıralı). Hiçbir kaydın kabul edilmediği turda döner: reddedilen kayıt kuyrukta
    /// bekler ve sonraki çağrıda yeniden sunulur — Application zamanlayıcı tutmaz.
    pub fn pump<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>) {
        loop {
            self.served.clear();
            let mut accepted = false;
            let mut finished = None;
            for (&transfer_id, transfer) in &mut self.outgoing {
                if self.served.contains(&transfer.peer) {
                    // Eşin daha eski bir aktarımı sürüyor.
                    continue;
                }
                self.served.push(transfer.peer);
                match offer(messenger, transfer_id, transfer, &mut self.stats) {
                    Offer::Waiting => {}
                    Offer::Accepted => accepted = true,
                    Offer::Finished(event) => {
                        finished = Some((transfer_id, event));
                        break;
                    }
                }
            }
            match finished {
                Some((transfer_id, event)) => {
                    // Olay, bıraktığı aktarımın yuvasını alır.
                    self.outgoing.remove(&transfer_id);
                    if let Some(event) = event {
                        self.push_event(event);
                    }
                }
                None if !accepted => return,
                None => {}
            }
        }
    }

    /// Zincirden gelen bir [`MessengerEvent`]'i işler: `FILE_*` kaydı taşıyan `Other`'ı TÜKETİR
    /// (`None`; doğan olay [`FileTransfer::poll_event`] kuyruğuna girer), diğer her olayı AYNEN
    /// geri verir. Alıcı-tarafı `FILE_ABORT` (`storage_full`, `hash_mismatch`) eşin güncel
    /// oturumundan TEK denemeyle gider (RECEIPT gibi: gönderilemezse kaybolur, yeniden denenmez).
    #[must_use]
    pub fn handle<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        event: MessengerEvent,
    ) -> Option<MessengerEvent> {
        match event {
            MessengerEvent::Other { peer, message } if is_file_record(&message) => {
                if let Some(event) = self.on_record(messenger, peer, message) {
                    self.push_event(event);
                }
                None
            }
            event => Some(event),
        }
    }

    /// Kuyruktaki olaylardan bir yöne ait olanların sayısı (≤ `MAX_PENDING_FILE_EVENTS`).
    fn queued_events(&self, incoming: bool) -> usize {
        self.events
            .iter()
            .filter(|event| event.is_incoming() == incoming)
            .count()
    }

    /// Olayı kuyruğa koyar. Yuva hesabı tavanı garanti eder: olay, kuyruktan önce tablodan
    /// çıkarılan aktarımın yerini alır.
    fn push_event(&mut self, event: FileEvent) {
        debug_assert!(
            self.events.len() < MAX_PENDING_FILE_EVENTS,
            "olay kuyruğu yuva hesabını aştı"
        );
        self.events.push_back(event);
    }

    /// `FILE_*` kaydı (yalnız [`FileTransfer::handle`] çağırır).
    fn on_record<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        peer: PeerRef,
        message: AppMessage,
    ) -> Option<FileEvent> {
        match message {
            AppMessage::FileBegin {
                transfer_id,
                file_size,
                whole_file_hash: hash,
                chunk_count,
                name,
            } => {
                let declared = Declared {
                    file_size,
                    whole_file_hash: hash,
                    chunk_count,
                };
                self.on_begin(messenger, (peer, transfer_id), declared, name);
                None
            }
            AppMessage::FileChunk {
                transfer_id,
                offset,
                payload,
            } => self.on_chunk(messenger, (peer, transfer_id), offset, &payload),
            AppMessage::FileEnd { transfer_id } => self.on_end(messenger, (peer, transfer_id)),
            AppMessage::FileAbort {
                transfer_id,
                reason,
            } => self.on_abort(peer, transfer_id, reason),
            // `handle` bunları geri verir; buraya gelmezler.
            AppMessage::Text { .. }
            | AppMessage::PttOpen { .. }
            | AppMessage::PttClose { .. }
            | AppMessage::Receipt { .. }
            | AppMessage::SignalingBlob { .. }
            | AppMessage::MediaBundle { .. } => None,
        }
    }

    /// `FILE_BEGIN`: kabul ya da ret. Olay üretmez.
    fn on_begin<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        key: IncomingKey,
        declared: Declared,
        name: String,
    ) {
        if let Some(existing) = self.incoming.get(&key) {
            // Aynı alanlar → tekrar. Farklı alanlar (ör. yeniden başlamış gönderenin kimliği) →
            // DROP; süren aktarım korunur.
            if existing.declared == declared && existing.name == name {
                self.stats.duplicate_records += 1;
            } else {
                self.stats.inconsistent_records += 1;
            }
            return;
        }
        if !declared.is_consistent() {
            self.stats.inconsistent_records += 1;
            return;
        }
        let (peer, transfer_id) = key;
        let from_peer = self
            .incoming
            .range((peer, TransferId::MIN)..=(peer, TransferId::MAX))
            .count();
        if declared.file_size > MAX_REASSEMBLY_BYTES
            || from_peer >= MAX_INCOMING_TRANSFERS_PER_PEER
            || self.incoming.len() + self.queued_events(true) >= MAX_INCOMING_TRANSFERS
        {
            self.stats.transfers_refused += 1;
            send_abort(
                messenger,
                peer,
                transfer_id,
                FileAbortReason::StorageFull,
                &mut self.stats,
            );
            return;
        }
        self.incoming.insert(key, Incoming::new(declared, name));
    }

    /// `FILE_CHUNK`: parça yerleşir; aktarım tamamlandıysa doğrulanır.
    fn on_chunk<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        key: IncomingKey,
        offset: u64,
        payload: &[u8],
    ) -> Option<FileEvent> {
        let Some(transfer) = self.incoming.get_mut(&key) else {
            self.stats.unknown_transfer_records += 1;
            return None;
        };
        match transfer.accept_chunk(offset, payload) {
            ChunkVerdict::Stored => {}
            ChunkVerdict::Duplicate => {
                self.stats.duplicate_records += 1;
                return None;
            }
            ChunkVerdict::Inconsistent => {
                self.stats.inconsistent_records += 1;
                return None;
            }
        }
        if transfer.is_complete() {
            self.finish(messenger, key)
        } else {
            None
        }
    }

    /// `FILE_END`: eksik parça yoksa aktarım doğrulanır; varsa beklenir.
    fn on_end<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        key: IncomingKey,
    ) -> Option<FileEvent> {
        let Some(transfer) = self.incoming.get_mut(&key) else {
            self.stats.unknown_transfer_records += 1;
            return None;
        };
        if transfer.end_seen {
            self.stats.duplicate_records += 1;
            return None;
        }
        transfer.end_seen = true;
        if transfer.is_complete() {
            self.finish(messenger, key)
        } else {
            None
        }
    }

    /// Bütün parçalar + `FILE_END`: özet doğrulanır; durum her iki sonuçta da atılır.
    fn finish<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        key: IncomingKey,
    ) -> Option<FileEvent> {
        let transfer = self.incoming.remove(&key)?;
        let (peer, transfer_id) = key;
        if whole_file_hash(&transfer.data) == transfer.declared.whole_file_hash {
            Some(FileEvent::Received {
                peer,
                transfer_id,
                name: transfer.name,
                bytes: transfer.data,
            })
        } else {
            send_abort(
                messenger,
                peer,
                transfer_id,
                FileAbortReason::HashMismatch,
                &mut self.stats,
            );
            Some(FileEvent::Failed {
                peer,
                transfer_id,
                reason: FileFailure::HashMismatch,
            })
        }
    }

    /// `FILE_ABORT`: yön nedenden çıkarılır (modül belgesi). İzlenmeyen aktarım → yok sayılır.
    fn on_abort(
        &mut self,
        peer: PeerRef,
        transfer_id: TransferId,
        reason: FileAbortReason,
    ) -> Option<FileEvent> {
        let event = match reason {
            // Gönderenin iptali: eşten GELEN aktarım.
            FileAbortReason::UserCancel | FileAbortReason::Timeout => self
                .incoming
                .remove(&(peer, transfer_id))
                .map(|_| FileEvent::Aborted {
                    peer,
                    transfer_id,
                    reason,
                }),
            // Alıcının kararı: eşe GİDEN aktarım. Kimlik başka bir eşe gidiyorsa eşleşmez.
            FileAbortReason::HashMismatch | FileAbortReason::StorageFull => {
                if self
                    .outgoing
                    .get(&transfer_id)
                    .is_some_and(|transfer| transfer.peer == peer)
                {
                    self.outgoing.remove(&transfer_id);
                    Some(FileEvent::SendAborted {
                        peer,
                        transfer_id,
                        reason,
                    })
                } else {
                    None
                }
            }
        };
        if event.is_none() {
            self.stats.aborts_ignored += 1;
        }
        event
    }
}

/// [`FileTransfer::handle`]'ın tükettiği kayıtlar.
const fn is_file_record(message: &AppMessage) -> bool {
    matches!(
        message,
        AppMessage::FileBegin { .. }
            | AppMessage::FileChunk { .. }
            | AppMessage::FileEnd { .. }
            | AppMessage::FileAbort { .. }
    )
}

/// Giden aktarımın sıradaki kaydı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// `FILE_BEGIN`.
    Begin,
    /// `FILE_CHUNK` — parça indeksi (`< chunk_count`).
    Chunk(u32),
    /// `FILE_END`.
    End,
    /// `FILE_ABORT` — kullanıcı iptali (`FILE_BEGIN` kabul edildikten sonra).
    Abort(FileAbortReason),
}

/// Giden aktarım. Dosya baytları son kayıt kabul edilene (ya da iptale) kadar tutulur.
struct Outgoing {
    /// Alıcı eş.
    peer: PeerRef,
    name: String,
    bytes: Vec<u8>,
    file_size: u64,
    whole_file_hash: [u8; WHOLE_FILE_HASH_BYTES],
    /// Parça uzunluğu `L` (kuyruğa alınırken `MAX_FILE_CHUNK_BYTES`); boş dosyada kullanılmaz.
    chunk_len: usize,
    chunk_count: u32,
    /// Transport'a henüz kabul ettirilmemiş sıradaki kayıt.
    next: Step,
}

impl Outgoing {
    /// Sıradaki kaydın mesajı.
    fn message(&self, transfer_id: TransferId) -> AppMessage {
        match self.next {
            Step::Begin => AppMessage::FileBegin {
                transfer_id,
                file_size: self.file_size,
                whole_file_hash: self.whole_file_hash,
                chunk_count: self.chunk_count,
                name: self.name.clone(),
            },
            Step::Chunk(index) => {
                let (offset, payload) = self.chunk(index);
                AppMessage::FileChunk {
                    transfer_id,
                    offset,
                    payload: payload.to_vec(),
                }
            }
            Step::End => AppMessage::FileEnd { transfer_id },
            Step::Abort(reason) => AppMessage::FileAbort {
                transfer_id,
                reason,
            },
        }
    }

    /// `index`. parça: (dosyadaki konum, yük). Kurulum `index < chunk_count ⇒ index·L < file_size`
    /// sağlar; doymalı yedekler yalnız bozulmuş bir durumda boş yük verir ve alıcı onu atar.
    fn chunk(&self, index: u32) -> (u64, &[u8]) {
        let start = usize::try_from(index)
            .unwrap_or(usize::MAX)
            .saturating_mul(self.chunk_len);
        let end = start.saturating_add(self.chunk_len).min(self.bytes.len());
        let payload = self.bytes.get(start..end).unwrap_or_default();
        (u64::try_from(start).unwrap_or(u64::MAX), payload)
    }

    /// `index`. parçanın adımı; parça kalmadıysa `FILE_END`.
    const fn chunk_step(&self, index: u32) -> Step {
        if index < self.chunk_count {
            Step::Chunk(index)
        } else {
            Step::End
        }
    }
}

/// [`offer`] sonucu.
enum Offer {
    /// Kayıt kabul edilmedi (oturum yok ya da geçici ret): tele çıkmadı, kuyrukta bekler.
    Waiting,
    /// Kayıt kabul edildi; aktarım sürüyor.
    Accepted,
    /// Aktarım bitti (son kayıt kabul edildi ya da kalıcı hata); varsa olay.
    Finished(Option<FileEvent>),
}

/// Aktarımın sıradaki kaydını eşin güncel oturumuna BİR KEZ sunar. Kabul edilen kaydın adımı
/// ilerler; böylece hiçbir kayıt ikinci kez kabul ettirilemez.
fn offer<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    transfer_id: TransferId,
    transfer: &mut Outgoing,
    stats: &mut FileTransferStats,
) -> Offer {
    let peer = transfer.peer;
    let Some(session) = messenger.session_of(peer) else {
        // Oturum yok: kayıt bekler; eşin sonraki `Ready`'sinden sonra yeniden sunulur.
        return Offer::Waiting;
    };
    let encoded = codec::encode(&transfer.message(transfer_id), messenger.limits());
    let sent = match encoded {
        Ok(unit) => messenger
            .transport_mut()
            .send(session, unit, DeliveryPolicy::Reliable),
        // Kuyruğa alınırken bu limitlere sığan kayıt burada sığmıyorsa `pump`'a başka bir motor
        // verilmiştir: kayıt birime sığmıyor (S1 K3) — Transport'un `UnitTooLarge`'ıyla aynı anlam.
        Err(_) => Err(TransportError::UnitTooLarge),
    };
    match sent {
        Ok(()) => {
            stats.records_accepted += 1;
            transfer.next = match transfer.next {
                Step::Begin => transfer.chunk_step(0),
                Step::Chunk(index) => transfer.chunk_step(index.saturating_add(1)),
                Step::End => return Offer::Finished(Some(FileEvent::Sent { peer, transfer_id })),
                // Kullanıcı iptali eşe verildi; olay yok.
                Step::Abort(_) => return Offer::Finished(None),
            };
            Offer::Accepted
        }
        Err(error) if is_transient(error) => {
            stats.offers_refused += 1;
            Offer::Waiting
        }
        Err(error) => Offer::Finished(match transfer.next {
            // İptal kaydı kalıcı reddedildi: kullanıcı zaten iptal etmişti; olay yok.
            Step::Abort(_) => None,
            Step::Begin | Step::Chunk(_) | Step::End => Some(FileEvent::SendFailed {
                peer,
                transfer_id,
                error,
            }),
        }),
    }
}

/// Geçici ret: kayıt kuyruğa alınmadı ve sonraki `pump`'ta yeniden sunulur (S1 §3; K8 "bekler").
/// Diğer her hata kalıcı sayılır (fail-closed).
const fn is_transient(error: TransportError) -> bool {
    matches!(
        error,
        TransportError::Backpressure
            | TransportError::UnknownSession
            | TransportError::SessionSuperseded
            | TransportError::NotConnected
    )
}

/// Alıcı-tarafı `FILE_ABORT`'u eşin güncel oturumundan verir. Tek deneme: oturum yoksa ya da
/// Transport reddederse kaybolur (retry yok).
fn send_abort<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    peer: PeerRef,
    transfer_id: TransferId,
    reason: FileAbortReason,
    stats: &mut FileTransferStats,
) {
    let abort = AppMessage::FileAbort {
        transfer_id,
        reason,
    };
    // FILE_ABORT gövdesi 9 B'tır ve her geçerli `AmpLimits`'e sığar; kodlama hatası yine de
    // gönderilemeyen kayıt sayılır, panik yok.
    let encoded = codec::encode(&abort, messenger.limits());
    let accepted = match (messenger.session_of(peer), encoded) {
        (Some(session), Ok(unit)) => messenger
            .transport_mut()
            .send(session, unit, DeliveryPolicy::Reliable)
            .is_ok(),
        _ => false,
    };
    if accepted {
        stats.aborts_sent += 1;
    } else {
        stats.aborts_unsent += 1;
    }
}

/// `FILE_BEGIN`'in beyan ettiği, aktarım boyunca sabit alanlar.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Declared {
    file_size: u64,
    whole_file_hash: [u8; WHOLE_FILE_HASH_BYTES],
    chunk_count: u32,
}

impl Declared {
    /// Yerleşimden bağımsız tutarlılık: boş dosya ⇔ sıfır parça; her parça en az bir bayt.
    fn is_consistent(&self) -> bool {
        (self.file_size == 0) == (self.chunk_count == 0)
            && u64::from(self.chunk_count) <= self.file_size
    }

    /// `chunk_len` beyan edilen yerleşimi veriyor mu: `⌈file_size / chunk_len⌉ = chunk_count`.
    fn fits(&self, chunk_len: u64) -> bool {
        chunk_len != 0 && self.file_size.div_ceil(chunk_len) == u64::from(self.chunk_count)
    }
}

/// [`Incoming::accept_chunk`] kararı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChunkVerdict {
    /// Yeni parça yerleşti.
    Stored,
    /// Bu indeks zaten alınmıştı: etkisiz.
    Duplicate,
    /// Geometriye uymuyor: atıldı (S8).
    Inconsistent,
}

/// Gelen aktarımın birleştirme durumu.
struct Incoming {
    name: String,
    declared: Declared,
    /// Parça uzunluğu `L`: kabul edilen ilk parçadan öğrenilir (modül belgesi, "Parça geometrisi").
    chunk_len: Option<u64>,
    /// Birleştirme tamponu: ilk kabul edilen parçada `file_size` bayt ayrılır.
    data: Vec<u8>,
    /// Alınan parça indeksleri (`chunk_count` bit); `data` ile birlikte ayrılır.
    seen: Vec<u64>,
    /// Alınan farklı parça sayısı (≤ `chunk_count`).
    received: u32,
    /// `FILE_END` görüldü.
    end_seen: bool,
}

impl Incoming {
    fn new(declared: Declared, name: String) -> Self {
        Self {
            name,
            declared,
            chunk_len: None,
            data: Vec::new(),
            seen: Vec::new(),
            received: 0,
            end_seen: false,
        }
    }

    /// Bütün parçalar alındı ve `FILE_END` görüldü.
    fn is_complete(&self) -> bool {
        self.end_seen && self.received == self.declared.chunk_count
    }

    /// İlk parçadan `L`: son olmayan parça tam uzunluktadır; son parça `(chunk_count − 1)·L`'de
    /// başlar (tek parçalı dosyada `L` = uzunluğu). Yerleşimi vermeyen aday → `None`.
    fn learn_chunk_len(&self, offset: u64, end: u64, len: u64) -> Option<u64> {
        let declared = &self.declared;
        let candidate = if end < declared.file_size {
            len
        } else {
            let before_last = u64::from(declared.chunk_count.checked_sub(1)?);
            if before_last == 0 {
                len
            } else if offset.is_multiple_of(before_last) {
                offset / before_last
            } else {
                return None;
            }
        };
        declared.fits(candidate).then_some(candidate)
    }

    /// `[offset, offset + payload.len())` parçasını yerleştirir. Sıra serbest; tekrar etkisiz
    /// (ilk kopya kalır; tutarsız bir kopya özet denetiminde yakalanır).
    fn accept_chunk(&mut self, offset: u64, payload: &[u8]) -> ChunkVerdict {
        let size = self.declared.file_size;
        let len = u64::try_from(payload.len()).unwrap_or(u64::MAX);
        let Some(end) = offset.checked_add(len) else {
            return ChunkVerdict::Inconsistent;
        };
        if len == 0 || end > size {
            return ChunkVerdict::Inconsistent;
        }
        let Some(chunk_len) = self
            .chunk_len
            .or_else(|| self.learn_chunk_len(offset, end, len))
        else {
            return ChunkVerdict::Inconsistent;
        };
        // `end ≤ size` ve `len ≥ 1` ⇒ `offset < size`: çıkarma taşmaz.
        if !offset.is_multiple_of(chunk_len) || len != chunk_len.min(size - offset) {
            return ChunkVerdict::Inconsistent;
        }
        // Yerleşim `index < chunk_count` sağlar; `size ≤ MAX_REASSEMBLY_BYTES` dönüşümleri kayıpsız
        // kılar (başarısız dönüşüm yine de atılır, panik yok).
        let index = offset / chunk_len;
        let (Ok(word), Ok(start), Ok(stop)) = (
            usize::try_from(index / SEEN_WORD_BITS),
            usize::try_from(offset),
            usize::try_from(end),
        ) else {
            return ChunkVerdict::Inconsistent;
        };
        let mask = 1u64 << (index % SEEN_WORD_BITS);
        if self.data.is_empty() {
            // İlk kabul edilen parça: tamponlar ŞİMDİ ayrılır (boyut FILE_BEGIN'de S7'ye karşı
            // denetlendi).
            let (Ok(size_bytes), Ok(words)) = (
                usize::try_from(size),
                usize::try_from(u64::from(self.declared.chunk_count).div_ceil(SEEN_WORD_BITS)),
            ) else {
                return ChunkVerdict::Inconsistent;
            };
            self.data = vec![0; size_bytes];
            self.seen = vec![0; words];
        }
        let Some(word_bits) = self.seen.get_mut(word) else {
            return ChunkVerdict::Inconsistent;
        };
        if *word_bits & mask != 0 {
            return ChunkVerdict::Duplicate;
        }
        let Some(target) = self.data.get_mut(start..stop) else {
            return ChunkVerdict::Inconsistent;
        };
        target.copy_from_slice(payload);
        *word_bits |= mask;
        self.received += 1;
        self.chunk_len = Some(chunk_len);
        ChunkVerdict::Stored
    }
}

#[cfg(test)]
mod tests {
    //! Parça geometrisi, yuvalar ve olay kuyruğu, `FILE_ABORT` yönü, `handle` geçirgenliği, `pump`
    //! sırası ve hata sınıfları, BLAKE2s-256 bilinen yanıtı — uçtan uca akış
    //! `tests/file_transfer.rs`'te.

    use super::*;
    use crate::codec::MsgType;
    use kama_contracts::handles::SessionHandle;
    use kama_contracts::s1::{
        ConnectResult, ConnectTicket, Delivered, PreparedUnit, SessionEvent, TransportCapabilities,
    };

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;

    /// RFC 7693 Ek B — BLAKE2s-256("abc").
    const BLAKE2S_256_ABC: [u8; WHOLE_FILE_HASH_BYTES] = [
        0x50, 0x8C, 0x5E, 0x8C, 0x32, 0x7C, 0x14, 0xE2, 0xE1, 0xA7, 0x2B, 0xA3, 0x4E, 0xEB, 0x45,
        0x2F, 0x37, 0x45, 0x8B, 0x20, 0x9E, 0xD6, 0x3A, 0x29, 0x4D, 0x99, 0x9B, 0x4C, 0x86, 0x67,
        0x59, 0x82,
    ];

    /// Her eşe `connect`'te hazır oturum veren, gönderimleri kaydeden (ya da `refuse` ile
    /// reddeden) Transport.
    #[derive(Default)]
    struct CountingTransport {
        peers: Vec<PeerRef>,
        sent: Vec<PreparedUnit>,
        refuse: Option<TransportError>,
    }

    impl KamaTransport for CountingTransport {
        fn capabilities(&self) -> TransportCapabilities {
            TransportCapabilities {
                max_plaintext_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
                preferred_plaintext_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
                max_early_unit_bytes: TEST_MAX_PLAINTEXT_UNIT_BYTES,
                reliable: true,
            }
        }

        fn connect(
            &mut self,
            peer: PeerRef,
            _early_unit: Option<PreparedUnit>,
        ) -> Result<ConnectResult, TransportError> {
            let index = match self.peers.iter().position(|known| *known == peer) {
                Some(index) => index,
                None => {
                    self.peers.push(peer);
                    self.peers.len() - 1
                }
            };
            let raw = u64::try_from(index).expect("test sayısı u64'e sığar");
            Ok(ConnectResult::Ready(SessionHandle::from_raw(raw)))
        }

        fn send(
            &mut self,
            _session: SessionHandle,
            unit: PreparedUnit,
            _policy: DeliveryPolicy,
        ) -> Result<(), TransportError> {
            if let Some(error) = self.refuse {
                return Err(error);
            }
            self.sent.push(unit);
            Ok(())
        }

        fn recv(&mut self) -> Option<Delivered> {
            None
        }

        fn poll_event(&mut self) -> Option<SessionEvent> {
            None
        }

        fn close(&mut self, _session: SessionHandle) -> Result<(), TransportError> {
            Err(TransportError::UnknownSession)
        }

        fn cancel(&mut self, _ticket: ConnectTicket) {}
    }

    fn peer(n: u8) -> PeerRef {
        PeerRef::from_dest_hash([n; 32])
    }

    /// Verilen eşlerin her biriyle hazır oturumu olan motor.
    fn messenger_with(peers: &[PeerRef]) -> Messenger<CountingTransport> {
        let mut messenger = Messenger::new(CountingTransport::default()).expect("limitler geçerli");
        for &p in peers {
            assert!(matches!(messenger.connect(p), Ok(ConnectResult::Ready(_))));
        }
        messenger
    }

    /// Transport'un kabul ettiği kayıtlar, sırasıyla.
    fn sent(messenger: &Messenger<CountingTransport>) -> Vec<AppMessage> {
        messenger
            .transport()
            .sent
            .iter()
            .map(|unit| codec::decode(&unit.bytes, messenger.limits()).expect("geçerli kayıt"))
            .collect()
    }

    /// Kaydı zincirden gelmiş gibi verir (dosya kaydı tüketilmeli); kuyruğun başındaki olayı
    /// döndürür.
    fn feed(
        files: &mut FileTransfer,
        messenger: &mut Messenger<CountingTransport>,
        peer: PeerRef,
        message: AppMessage,
    ) -> Option<FileEvent> {
        let passed = files.handle(messenger, MessengerEvent::Other { peer, message });
        assert_eq!(passed, None, "FILE_* kaydı tüketilmeli");
        files.poll_event()
    }

    /// Kuyruktaki bütün olaylar.
    fn drain(files: &mut FileTransfer) -> Vec<FileEvent> {
        std::iter::from_fn(|| files.poll_event()).collect()
    }

    fn declared(file_size: u64, chunk_count: u32) -> Declared {
        Declared {
            file_size,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count,
        }
    }

    fn begin(transfer_id: TransferId, file_size: u64, chunk_count: u32) -> AppMessage {
        AppMessage::FileBegin {
            transfer_id,
            file_size,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count,
            name: "a".to_owned(),
        }
    }

    /// Boş dosyanın (doğru özetli) iki kaydı: `FILE_BEGIN`, `FILE_END`.
    fn empty_file(transfer_id: TransferId) -> [AppMessage; 2] {
        [
            AppMessage::FileBegin {
                transfer_id,
                file_size: 0,
                whole_file_hash: whole_file_hash(&[]),
                chunk_count: 0,
                name: "bos".to_owned(),
            },
            AppMessage::FileEnd { transfer_id },
        ]
    }

    fn abort(transfer_id: TransferId, reason: FileAbortReason) -> AppMessage {
        AppMessage::FileAbort {
            transfer_id,
            reason,
        }
    }

    fn transfer_of(message: &AppMessage) -> TransferId {
        match message {
            AppMessage::FileBegin { transfer_id, .. }
            | AppMessage::FileChunk { transfer_id, .. }
            | AppMessage::FileEnd { transfer_id }
            | AppMessage::FileAbort { transfer_id, .. } => *transfer_id,
            other => panic!("FILE_* bekleniyordu, gelen {other:?}"),
        }
    }

    #[test]
    fn whole_file_hash_is_unkeyed_blake2s_256() {
        assert_eq!(whole_file_hash(b"abc"), BLAKE2S_256_ABC);
    }

    #[test]
    fn handle_passes_every_other_event_through_untouched() {
        let bob = peer(1);
        let mut messenger = messenger_with(&[bob]);
        let mut files = FileTransfer::new();
        let others = [
            MessengerEvent::Session(SessionEvent::Ready(SessionHandle::from_raw(9), bob)),
            MessengerEvent::TextReceived {
                peer: bob,
                msg_id: 1,
                text: "metin".to_owned(),
            },
            MessengerEvent::Delivered {
                peer: bob,
                msg_id: 2,
            },
            MessengerEvent::Other {
                peer: bob,
                message: AppMessage::PttClose { stream_id: 3 },
            },
        ];
        for event in others {
            assert_eq!(files.handle(&mut messenger, event.clone()), Some(event));
        }
        assert_eq!(files.poll_event(), None);
        assert_eq!(files.stats(), FileTransferStats::default());
        assert!(sent(&messenger).is_empty());
    }

    #[test]
    fn chunk_len_is_learned_from_any_first_chunk() {
        // 10 B, L = 4: [0,4) [4,8) [8,10).
        let data: Vec<u8> = (0..10).collect();
        // Son parça önce: L = 8 / (3 − 1).
        let mut transfer = Incoming::new(declared(10, 3), String::new());
        assert_eq!(transfer.accept_chunk(8, &data[8..]), ChunkVerdict::Stored);
        assert_eq!(transfer.chunk_len, Some(4));
        assert_eq!(transfer.accept_chunk(0, &data[..4]), ChunkVerdict::Stored);
        assert_eq!(
            transfer.accept_chunk(0, &data[..4]),
            ChunkVerdict::Duplicate
        );
        assert_eq!(transfer.accept_chunk(4, &data[4..8]), ChunkVerdict::Stored);
        assert_eq!(transfer.received, 3);
        assert_eq!(transfer.data, data);
        // Ortadaki parça önce: L = kendi uzunluğu.
        let mut transfer = Incoming::new(declared(10, 3), String::new());
        assert_eq!(transfer.accept_chunk(4, &data[4..8]), ChunkVerdict::Stored);
        assert_eq!(transfer.chunk_len, Some(4));
        // Tek parçalı dosya.
        let mut transfer = Incoming::new(declared(10, 1), String::new());
        assert_eq!(transfer.accept_chunk(0, &data), ChunkVerdict::Stored);
        assert_eq!(transfer.chunk_len, Some(10));
    }

    #[test]
    fn chunks_outside_the_declared_layout_are_inconsistent() {
        let mut transfer = Incoming::new(declared(10, 3), String::new());
        // Son olmayan 5 B'lık parça: ⌈10 / 5⌉ = 2 ≠ 3.
        assert_eq!(
            transfer.accept_chunk(0, &[0; 5]),
            ChunkVerdict::Inconsistent
        );
        // 0'da başlayan son parça (L = 0) · konumu (3 − 1)'e bölünmeyen son parça.
        assert_eq!(
            transfer.accept_chunk(0, &[0; 10]),
            ChunkVerdict::Inconsistent
        );
        assert_eq!(
            transfer.accept_chunk(7, &[0; 3]),
            ChunkVerdict::Inconsistent
        );
        // Dosyanın dışına taşan · boş · konum + uzunluk taşan.
        assert_eq!(
            transfer.accept_chunk(8, &[0; 3]),
            ChunkVerdict::Inconsistent
        );
        assert_eq!(transfer.accept_chunk(4, &[]), ChunkVerdict::Inconsistent);
        assert_eq!(
            transfer.accept_chunk(u64::MAX, &[0]),
            ChunkVerdict::Inconsistent
        );
        // Atılan parça ne L'yi sabitler ne tampon ayırır.
        assert_eq!(transfer.chunk_len, None);
        assert!(transfer.data.is_empty() && transfer.seen.is_empty());
        // L sabitlendikten sonra: sınır dışı konum ve yanlış uzunluk.
        assert_eq!(transfer.accept_chunk(0, &[0; 4]), ChunkVerdict::Stored);
        assert_eq!(
            transfer.accept_chunk(2, &[0; 4]),
            ChunkVerdict::Inconsistent
        );
        assert_eq!(
            transfer.accept_chunk(8, &[0; 1]),
            ChunkVerdict::Inconsistent
        );
        assert_eq!(transfer.received, 1);
    }

    #[test]
    fn begin_beyond_size_or_table_caps_is_refused_with_storage_full() {
        let count = u8::try_from(MAX_INCOMING_TRANSFERS + 1).expect("küçük sayı");
        let peers: Vec<PeerRef> = (1..=count).map(peer).collect();
        let mut messenger = messenger_with(&peers);
        let per_peer = TransferId::try_from(MAX_INCOMING_TRANSFERS_PER_PEER).expect("küçük sayı");

        // Eş başına tavan.
        let mut files = FileTransfer::new();
        for id in 0..per_peer {
            assert_eq!(
                feed(&mut files, &mut messenger, peers[0], begin(id, 10, 1)),
                None
            );
        }
        let over = begin(per_peer, 10, 1);
        assert_eq!(feed(&mut files, &mut messenger, peers[0], over), None);
        assert_eq!(files.incoming_count(), MAX_INCOMING_TRANSFERS_PER_PEER);

        // Toplam tavan: eş başına birer aktarım.
        let mut files = FileTransfer::new();
        for &p in &peers[..MAX_INCOMING_TRANSFERS] {
            assert_eq!(feed(&mut files, &mut messenger, p, begin(0, 10, 1)), None);
        }
        let last = peers[MAX_INCOMING_TRANSFERS];
        assert_eq!(
            feed(&mut files, &mut messenger, last, begin(0, 10, 1)),
            None
        );
        assert_eq!(files.incoming_count(), MAX_INCOMING_TRANSFERS);

        // S7: boyut, yuvadan ve her tahsisten önce.
        let mut files = FileTransfer::new();
        let oversize = begin(0, MAX_REASSEMBLY_BYTES + 1, 1);
        assert_eq!(feed(&mut files, &mut messenger, peers[0], oversize), None);
        assert_eq!(files.incoming_count(), 0);
        assert_eq!(files.stats().transfers_refused, 1);

        // Üç ret de eşe `storage_full` olarak bildirildi, birer kez.
        let refused = abort(per_peer, FileAbortReason::StorageFull);
        let refused_first = abort(0, FileAbortReason::StorageFull);
        assert_eq!(
            sent(&messenger),
            vec![refused, refused_first.clone(), refused_first]
        );
    }

    #[test]
    fn undrained_events_hold_their_transfer_slots() {
        let count = u8::try_from(MAX_INCOMING_TRANSFERS + 1).expect("küçük sayı");
        let peers: Vec<PeerRef> = (1..=count).map(peer).collect();
        let mut messenger = messenger_with(&peers);
        let mut files = FileTransfer::new();

        // Gelen: boş dosyalar tamamlanır, olaylar ALINMAZ — her biri bir gelen yuvası tutar.
        for &p in &peers[..MAX_INCOMING_TRANSFERS] {
            for message in empty_file(0) {
                let passed =
                    files.handle(&mut messenger, MessengerEvent::Other { peer: p, message });
                assert_eq!(passed, None);
            }
        }
        assert_eq!(files.incoming_count(), 0);
        let last = peers[MAX_INCOMING_TRANSFERS];
        let [first, _] = empty_file(0);
        let passed = files.handle(
            &mut messenger,
            MessengerEvent::Other {
                peer: last,
                message: first.clone(),
            },
        );
        assert_eq!(passed, None);
        assert_eq!(files.stats().transfers_refused, 1, "yuva yok: storage_full");

        // Giden: biten aktarımların alınmamış `Sent` olayları giden yuvalarını tutar.
        for _ in 0..MAX_OUTGOING_TRANSFERS {
            files
                .send_file(&messenger, peers[0], "a", Vec::new())
                .expect("yuva var");
            files.pump(&mut messenger);
        }
        assert_eq!(files.outgoing_count(), 0);
        assert_eq!(
            files.send_file(&messenger, peers[0], "a", Vec::new()),
            Err(FileSendError::QueueFull)
        );

        // Kuyruk boşalınca iki yönde de yer açılır; hiçbir olay düşmedi.
        let events = drain(&mut files);
        assert_eq!(
            events.len(),
            MAX_INCOMING_TRANSFERS + MAX_OUTGOING_TRANSFERS
        );
        assert!(events[..MAX_INCOMING_TRANSFERS]
            .iter()
            .all(|event| matches!(event, FileEvent::Received { .. })));
        assert!(events[MAX_INCOMING_TRANSFERS..]
            .iter()
            .all(|event| matches!(event, FileEvent::Sent { .. })));
        let passed = files.handle(
            &mut messenger,
            MessengerEvent::Other {
                peer: last,
                message: first,
            },
        );
        assert_eq!(passed, None);
        assert_eq!(files.incoming_count(), 1);
        assert!(files
            .send_file(&messenger, peers[0], "a", Vec::new())
            .is_ok());
    }

    #[test]
    fn abort_direction_follows_the_reason_and_never_crosses_peers() {
        let (bob, carol) = (peer(1), peer(2));
        let mut messenger = messenger_with(&[bob, carol]);
        let mut files = FileTransfer::new();
        // Aynı kimlik iki yönde: BOB'a giden ve BOB'dan gelen aktarım.
        let id = files
            .send_file(&messenger, bob, "a", vec![1; 3])
            .expect("kuyruğa alınır");
        assert_eq!(
            feed(&mut files, &mut messenger, bob, begin(id, 10, 1)),
            None
        );
        let both = (1, 1);
        // Başka eşten gelen FILE_ABORT hiçbir aktarıma dokunmaz.
        let storage_full = abort(id, FileAbortReason::StorageFull);
        let user_cancel = abort(id, FileAbortReason::UserCancel);
        assert_eq!(feed(&mut files, &mut messenger, carol, storage_full), None);
        assert_eq!(
            feed(&mut files, &mut messenger, carol, user_cancel.clone()),
            None
        );
        assert_eq!((files.outgoing_count(), files.incoming_count()), both);
        // Gönderen nedeni → eşten GELEN aktarım.
        assert_eq!(
            feed(&mut files, &mut messenger, bob, user_cancel),
            Some(FileEvent::Aborted {
                peer: bob,
                transfer_id: id,
                reason: FileAbortReason::UserCancel,
            })
        );
        assert_eq!((files.outgoing_count(), files.incoming_count()), (1, 0));
        // Alıcı nedeni → eşe GİDEN aktarım.
        assert_eq!(
            feed(
                &mut files,
                &mut messenger,
                bob,
                abort(id, FileAbortReason::HashMismatch)
            ),
            Some(FileEvent::SendAborted {
                peer: bob,
                transfer_id: id,
                reason: FileAbortReason::HashMismatch,
            })
        );
        assert_eq!(files.outgoing_count(), 0);
        assert_eq!(files.stats().aborts_ignored, 2);
        // `pump` çağrılmadı: hiçbir kayıt Transport'a verilmedi.
        assert!(sent(&messenger).is_empty());
    }

    #[test]
    fn pump_is_fair_across_peers_and_sequential_within_a_peer() {
        let (bob, carol) = (peer(1), peer(2));
        let mut messenger = messenger_with(&[bob, carol]);
        let mut files = FileTransfer::new();
        let chunk_len = messenger.limits().max_file_chunk_bytes();
        let a = files
            .send_file(&messenger, bob, "a", vec![0; chunk_len + 1])
            .expect("kuyruğa alınır");
        let b = files
            .send_file(&messenger, carol, "b", vec![0; 1])
            .expect("kuyruğa alınır");
        let c = files
            .send_file(&messenger, bob, "c", vec![0; 1])
            .expect("kuyruğa alınır");
        files.pump(&mut messenger);
        assert_eq!(
            drain(&mut files),
            vec![
                FileEvent::Sent {
                    peer: carol,
                    transfer_id: b,
                },
                FileEvent::Sent {
                    peer: bob,
                    transfer_id: a,
                },
                FileEvent::Sent {
                    peer: bob,
                    transfer_id: c,
                },
            ]
        );
        // Tur başına eş başına bir kayıt; BOB'un ikinci aktarımı birincisi bitince başlar.
        let order: Vec<(MsgType, TransferId)> = sent(&messenger)
            .iter()
            .map(|message| (message.msg_type(), transfer_of(message)))
            .collect();
        assert_eq!(
            order,
            vec![
                (MsgType::FileBegin, a),
                (MsgType::FileBegin, b),
                (MsgType::FileChunk, a),
                (MsgType::FileChunk, b),
                (MsgType::FileChunk, a),
                (MsgType::FileEnd, b),
                (MsgType::FileEnd, a),
                (MsgType::FileBegin, c),
                (MsgType::FileChunk, c),
                (MsgType::FileEnd, c),
            ]
        );
        assert_eq!(files.stats().records_accepted, 10);
        assert_eq!(files.outgoing_count(), 0);
    }

    #[test]
    fn transient_refusal_waits_and_permanent_refusal_fails_the_transfer() {
        let bob = peer(1);
        let mut messenger = messenger_with(&[bob]);
        let mut files = FileTransfer::new();
        let id = files
            .send_file(&messenger, bob, "a", vec![1; 3])
            .expect("kuyruğa alınır");
        // Geçici ret: kayıt kuyrukta bekler, olay yok.
        messenger.transport_mut().refuse = Some(TransportError::Backpressure);
        files.pump(&mut messenger);
        files.pump(&mut messenger);
        assert_eq!(files.poll_event(), None);
        assert_eq!(
            (files.outgoing_count(), files.stats().offers_refused),
            (1, 2)
        );
        // Kalıcı ret: aktarım bırakılır ve kullanıcıya yansıtılır.
        messenger.transport_mut().refuse = Some(TransportError::UnitTooLarge);
        files.pump(&mut messenger);
        assert_eq!(
            files.poll_event(),
            Some(FileEvent::SendFailed {
                peer: bob,
                transfer_id: id,
                error: TransportError::UnitTooLarge,
            })
        );
        assert_eq!(files.outgoing_count(), 0);
        assert!(sent(&messenger).is_empty(), "hiçbir kayıt kabul edilmedi");
    }

    #[test]
    fn debug_redacts_file_name_and_bytes() {
        let name = "gizli-ad.txt";
        let bytes = vec![0xA5; 7];
        let event = FileEvent::Received {
            peer: peer(1),
            transfer_id: 1,
            name: name.to_owned(),
            bytes: bytes.clone(),
        };
        let shown = format!("{event:?}");
        assert!(!shown.contains(name), "Debug dosya adını sızdırdı: {shown}");
        let raw = format!("{bytes:?}");
        assert!(!shown.contains(&raw), "Debug baytları sızdırdı: {shown}");
    }
}
