//! PTT (bas-konuş) yaşam döngüsü (B2.4) — AMP `PTT_OPEN` (0x06), `PTT_CLOSE` (0x07) ve
//! `MEDIA_BUNDLE` (0x0A) kabı (`specs/application-message-protocol.md` §3; sahip kararları
//! S7 / S8 / S9, Abkarada 2026-10-08).
//!
//! İş emrindeki `PTT_START` / `PTT_END`, AMP'nin `PTT_OPEN` / `PTT_CLOSE`'udur (S9); kodda AMP
//! adları kullanılır. Ses verisinin kendisi media ekibinindir: bu modül kodlayıcı ya da medya
//! motoru çalıştırmaz, `MEDIA_BUNDLE.payload`'ı okumaz — opak baytları kapta taşır.
//!
//! [`Ptt`] [`Messenger`]'ın üstünde durur ve Transport'u sahiplenmez:
//!
//! * **Gönderen** — [`Ptt::open`] `PTT_OPEN`'ı eşin güncel oturumuna verir ve akışı kaydeder;
//!   [`Ptt::send_bundle`] `MEDIA_BUNDLE`'ı akışın SABİT `codec` / `bundle_ms`'iyle verir;
//!   [`Ptt::close`] `PTT_CLOSE`'u verir. Tutamaç `Messenger::session_of`'tan, gönderim
//!   `Messenger::transport_mut` üzerinden.
//! * **Alıcı** — [`Ptt::poll`], [`Messenger::poll`]'un TEK okuyucusudur: PTT kayıtlarını
//!   [`PttEvent`]'e çevirir, diğer her olayı [`PttEvent::Messenger`] ile AYNEN geçirir.
//!
//! # Teslim politikası (S9)
//!
//! `MEDIA_BUNDLE` → `DeliveryPolicy::BestEffort`: geç gelen ses çöptür. `PTT_OPEN` / `PTT_CLOSE`
//! → `DeliveryPolicy::Reliable` (YORUM): yaşam döngüsü kaydı kaybolursa alıcı akışı hiç başlatmaz
//! ya da bitirmez. Hiçbir PTT kaydı sıkıştırılmaz (AMP §5 kural 6; codec bugün hiçbir tipi
//! sıkıştırmaz). S1 §4: aynı oturumda `Reliable` birimler gönderim sırasıyla gelir, `BestEffort`
//! birimler sırasız gelebilir — bir demet kendi `PTT_OPEN`'ından önce ya da `PTT_CLOSE`'undan
//! sonra varabilir; ikisi de düşer (aşağıda, "Alıcı").
//!
//! # `stream_id` — gönderen başına monoton
//!
//! Tek sayaç, eşten bağımsız: [`FIRST_STREAM_ID`]'den kesin artar; bu yüzden `stream_id` gönderen
//! tarafta akışı tek başına adlandırır ([`Ptt::send_bundle`] ve [`Ptt::close`] eş almaz). Sayaç
//! YALNIZ Transport `PTT_OPEN`'ı kabul edince (`send` → `Ok(())`) ilerler; `u32` alanı tükenince
//! [`PttError::StreamIdExhausted`], sarma YOK. Sayaç kalıcı değildir; alıcı sıra zorlamaz, yalnız
//! eşin etkin akışının `stream_id`'siyle eşleştirir.
//!
//! # Gönderen
//!
//! Eş başına en çok bir açık giden akış (YORUM): konuşan oturumun eşidir (AMP §3, 1:1); akış
//! açıkken ikinci [`Ptt::open`] → [`PttError::StreamActive`]. [`Ptt::open`] ve
//! [`Ptt::send_bundle`] hata yollarında motor durumu DEĞİŞMEZ. [`Ptt::close`] ise akışı HER
//! DURUMDA kapatır (YORUM): basış bitmiştir. `PTT_CLOSE` Transport'a verilemezse hata döner ve
//! kayıt yeniden denenmez (S1 K8); alıcı akışı eşin bir sonraki `PTT_OPEN`'ında ya da oturum
//! sonunda bitirir. Giden akış çağıranındır: oturum bitse de kendiliğinden kapanmaz.
//!
//! # Alıcı (S8: vektörsüz kararlar fail-closed DROP — panik yok, olay yok, sayaç var)
//!
//! Eş başına en çok bir etkin gelen akış:
//!
//! * `PTT_OPEN` → [`PttEvent::PttStarted`]. Eşin güncel oturumu yoksa (kapanmış oturumdan geç
//!   teslim) DROP. Etkin akışın `stream_id`'siyle yeniden gelirse DROP: akışın `codec` /
//!   `bundle_ms`'i ilk `PTT_OPEN`'la sabitlendi. Başka bir `stream_id`'yle gelirse en yeni kazanır
//!   (YORUM): önce eski akışa [`PttEvent::PttEnded`], sonra yenisine `PttStarted`. Eski akışın
//!   `PTT_CLOSE`'u supersede edilen oturumla kaybolmuş (S1 K7: teslim edilmemiş birim yeni oturuma
//!   taşınmaz) ya da gönderici yeniden başlamış olabilir; DROP, eşin PTT'sini oturum bitene kadar
//!   kilitlerdi.
//! * `MEDIA_BUNDLE` → `stream_id`, `codec` VE `bundle_ms` etkin akışınkiyle aynıysa
//!   [`PttEvent::PttAudio`], varış sırasıyla (sıralama yükteki çerçevenin, media ekibinin). Etkin
//!   akış yoksa DROP (S7: `PTT_OPEN` öncesi ya da `PTT_CLOSE` sonrası); `stream_id` farklıysa
//!   DROP; `codec` ya da `bundle_ms` farklıysa DROP (S9).
//! * `PTT_CLOSE` → etkin akışınsa [`PttEvent::PttEnded`]; değilse DROP.
//!
//! # Oturum değişimi — akış eşindir (YORUM)
//!
//! Supersession (`Ready(h')` → `Closed(h, Superseded)`) akışı bitirmez: S1 K7 `Ready(h')`'yi
//! `Closed(h)`'den önce verir, eşin güncel oturumu boşalmaz; eski tutamaçtan geç gelen kayıtlar da
//! eşine bağlanır (Messenger'ın emekli tutamaç eşlemesi). Gönderen demetleri eşin O ANKİ güncel
//! oturumuna verir. Eşin güncel oturumu kalmadıysa (`UserClosed`, `PeerClosed` ya da `KeyHygiene`
//! ile kapanış) gelen akışı `PttEnded` ile biter: her [`Ptt::poll`] önce böyle akışları bitirir,
//! yani `PttEnded`, `Closed` geçirildikten sonraki ilk olaydır.
//!
//! # Tablolar
//!
//! Giden ve gelen akış tabloları [`MAX_PEERS`] ile sınırlıdır (YORUM): ikisi de eş başına en çok
//! bir kayıt tutar ve anahtarları [`Messenger`]'ın eş tablosundaki eşlerdir; yeni sabit yok.
//! Tavan doluyken yeni giden akış [`PttError::StreamTableFull`], yeni gelen akış DROP.
//!
//! # Bilinen sınır (sahibe açık)
//!
//! PTT kayıtları oturum değişiminde yeniden GÖNDERİLMEZ (TEXT'in aksine alındıları yoktur).
//! Supersede edilen oturumda teslim edilmemiş bir `PTT_OPEN` kaybolursa o basışın demetleri
//! alıcıda düşer (S7); kaybolan `PTT_CLOSE`'un etkisi yukarıdaki alıcı kurallarıyla sınırlıdır.
//!
//! # Gizlilik
//!
//! [`PttEvent`]'in `Debug` çıktısı ses yükünü değil uzunluğunu yazar; [`Ptt`] `Debug` değildir.

use core::fmt;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{DeliveryPolicy, KamaTransport, TransportError};

use crate::codec::limits::MEDIA_BUNDLE_PREFIX_BYTES;
use crate::codec::{self, AmpLimits, AppMessage, EncodeError, PttCodec};
use crate::messaging::{Messenger, MessengerEvent, MAX_PEERS};

/// `PTT_OPEN` / `PTT_CLOSE` / `MEDIA_BUNDLE` akış kimliği (AMP §3; telde `u32_be`).
pub type StreamId = u32;

/// Bu göndericinin ilk `stream_id`'si. Spec başlangıç değeri koymaz; değer bu uygulamanındır.
pub const FIRST_STREAM_ID: StreamId = 0;

/// Bir `MEDIA_BUNDLE`'ın taşıyabileceği en büyük ses yükü (bayt): `MAX_BODY_BYTES −
/// MEDIA_BUNDLE_PREFIX_BYTES` — A2'deki `MAX_FILE_CHUNK_BYTES` formülünün `MEDIA_BUNDLE`
/// karşılığı; spec'te adı yoktur. Media ekibi demet boyunu buna göre seçer; aşan yük
/// [`Ptt::send_bundle`]'da [`PttError::PayloadTooLarge`] verir.
#[must_use]
pub const fn max_bundle_payload_bytes(limits: &AmpLimits) -> usize {
    // `AmpLimits`, `MAX_BODY_BYTES ≥ FILE_CHUNK_PREFIX_BYTES (18) > MEDIA_BUNDLE_PREFIX_BYTES (8)`
    // garanti eder; çıkarma taşmaz, yine de doyurulur (panik yok).
    limits
        .max_body_bytes()
        .saturating_sub(MEDIA_BUNDLE_PREFIX_BYTES)
}

/// [`Ptt::poll`] çıktısı.
#[derive(Clone, PartialEq, Eq)]
pub enum PttEvent {
    /// PTT kaydı olmayan [`Messenger`] olayı — AYNEN iletilir (oturum olayları, metin ve alındı,
    /// diğer kartların kayıtları).
    Messenger(MessengerEvent),
    /// Eş konuşmaya başladı: `PTT_OPEN` kabul edildi. `codec` ve `bundle_ms` bu akış boyunca
    /// sabittir (S9).
    PttStarted {
        /// Konuşan eş (oturumun eşi; AMP §3).
        peer: PeerRef,
        /// Akış kimliği.
        stream_id: StreamId,
        /// Ses kodlayıcısı.
        codec: PttCodec,
        /// Demet süresi (ms).
        bundle_ms: u8,
    },
    /// Etkin akıştan bir ses demeti. Yük opaktır; içi media ekibinindir.
    PttAudio {
        /// Konuşan eş.
        peer: PeerRef,
        /// Akış kimliği.
        stream_id: StreamId,
        /// `MEDIA_BUNDLE.payload`, aynen.
        payload: Vec<u8>,
    },
    /// Eşin akışı bitti: `PTT_CLOSE`, aynı eşten başka `stream_id`'li `PTT_OPEN` ya da eşin güncel
    /// oturumunun kalmaması.
    PttEnded {
        /// Konuşan eş.
        peer: PeerRef,
        /// Biten akışın kimliği.
        stream_id: StreamId,
    },
}

/// Ses yükü YAZILMAZ; yalnız uzunluğu. `PeerRef` ve `MessengerEvent` kendi `Debug`'larıyla (ikisi
/// de içerik basmaz).
impl fmt::Debug for PttEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Messenger(event) => f.debug_tuple("Messenger").field(event).finish(),
            Self::PttStarted {
                peer,
                stream_id,
                codec,
                bundle_ms,
            } => f
                .debug_struct("PttStarted")
                .field("peer", peer)
                .field("stream_id", stream_id)
                .field("codec", codec)
                .field("bundle_ms", bundle_ms)
                .finish(),
            Self::PttAudio {
                peer,
                stream_id,
                payload,
            } => f
                .debug_struct("PttAudio")
                .field("peer", peer)
                .field("stream_id", stream_id)
                .field("payload_len", &payload.len())
                .finish_non_exhaustive(),
            Self::PttEnded { peer, stream_id } => f
                .debug_struct("PttEnded")
                .field("peer", peer)
                .field("stream_id", stream_id)
                .finish(),
        }
    }
}

/// [`Ptt::open`], [`Ptt::send_bundle`] ve [`Ptt::close`] hatası. `open` ve `send_bundle`'da motor
/// durumu DEĞİŞMEZ; `close`'da akış yine de kapanmıştır. Motor hiçbirini yeniden denemez (S1 K8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttError {
    /// Eşin hazır oturumu yok: `Ready` görülmedi ya da `Closed` okundu.
    NoSession,
    /// Bu eşe açık bir giden akış var; önce [`Ptt::close`].
    StreamActive {
        /// Açık akışın kimliği.
        stream_id: StreamId,
    },
    /// Bu `stream_id` açık bir giden akış değil (hiç açılmadı ya da kapatıldı).
    UnknownStream,
    /// Giden akış tablosu [`MAX_PEERS`]'te.
    StreamTableFull,
    /// `u32` `stream_id` alanı tükendi; sarma YOK.
    StreamIdExhausted,
    /// Ses yükü bir `MEDIA_BUNDLE` kaydına sığmıyor ([`max_bundle_payload_bytes`]).
    PayloadTooLarge {
        /// Verilen yükün uzunluğu.
        payload_len: usize,
        /// Bu limitlerdeki en büyük yük.
        max_payload_len: usize,
    },
    /// Kayıt kodlanamadı. Savunmadır: yük ön denetiminden sonra beklenmez.
    Encode(EncodeError),
    /// Transport reddetti; kayıt kuyruğa ALINMADI (S1 §3).
    Transport(TransportError),
}

impl fmt::Display for PttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSession => f.write_str("peer has no ready session"),
            Self::StreamActive { stream_id } => write!(
                f,
                "outgoing PTT stream {stream_id} to this peer is still open"
            ),
            Self::UnknownStream => f.write_str("no open outgoing PTT stream with this stream_id"),
            Self::StreamTableFull => {
                write!(f, "outgoing PTT stream table is full ({MAX_PEERS} streams)")
            }
            Self::StreamIdExhausted => f.write_str("stream_id space is exhausted (no wrap)"),
            Self::PayloadTooLarge {
                payload_len,
                max_payload_len,
            } => write!(
                f,
                "MEDIA_BUNDLE payload of {payload_len} bytes exceeds {max_payload_len} bytes"
            ),
            Self::Encode(e) => write!(f, "PTT record does not fit one AMP record: {e}"),
            Self::Transport(e) => write!(f, "transport refused the record: {e}"),
        }
    }
}

impl std::error::Error for PttError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::NoSession
            | Self::StreamActive { .. }
            | Self::UnknownStream
            | Self::StreamTableFull
            | Self::StreamIdExhausted
            | Self::PayloadTooLarge { .. } => None,
        }
    }
}

/// Gözlem sayaçları — salt-okunur; protokol davranışını değiştirmez. Hepsi alıcı tarafının
/// DROP'larıdır (S8) ya da olay üreten akış sonlarıdır.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PttStats {
    /// Eşin etkin akışı yokken gelen `MEDIA_BUNDLE`'lar: `PTT_OPEN` öncesi (S7) ya da
    /// `PTT_CLOSE` sonrası.
    pub bundles_without_stream: u64,
    /// Etkin akıştan başka `stream_id` taşıyan `MEDIA_BUNDLE`'lar.
    pub bundles_unknown_stream: u64,
    /// `stream_id`'si tutan ama `codec` ya da `bundle_ms`'i akışınkinden farklı `MEDIA_BUNDLE`'lar
    /// (S9).
    pub bundles_mismatched: u64,
    /// Etkin akışın `stream_id`'siyle yeniden gelen `PTT_OPEN`'lar (parametreleri aynı ya da
    /// farklı).
    pub duplicate_opens: u64,
    /// Güncel oturumu olmayan eşten gelen `PTT_OPEN`'lar (kapanmış oturumdan geç teslim).
    pub opens_without_session: u64,
    /// Gelen akış tablosu [`MAX_PEERS`]'teyken yeni eşten gelen `PTT_OPEN`'lar.
    pub opens_refused: u64,
    /// Eşin etkin akışına ait olmayan `PTT_CLOSE`'lar.
    pub unknown_closes: u64,
    /// Aynı eşten başka `stream_id`'li `PTT_OPEN`'ın bitirdiği akışlar (olay üretir).
    pub streams_replaced: u64,
    /// Eşin güncel oturumu kalmadığı için biten akışlar (olay üretir).
    pub streams_ended_by_session: u64,
}

/// B2.4 PTT motoru. Ayrıntı: modül belgesi.
pub struct Ptt {
    /// Sıradaki `stream_id`; `None` = `u32` alanı tükendi.
    next_stream_id: Option<StreamId>,
    /// Açık giden akışlar, `stream_id` sırasıyla (≤ [`MAX_PEERS`]; eş başına en çok bir).
    outgoing: BTreeMap<StreamId, Outgoing>,
    /// Eş başına etkin gelen akış (≤ [`MAX_PEERS`]).
    incoming: BTreeMap<PeerRef, Incoming>,
    /// Tek kayıttan doğan ikinci olay: örtük bitişin ardından yeni akışın `PttStarted`'ı. Tek yuva;
    /// [`Ptt::poll`] her turda önce onu boşaltır.
    pending: Option<PttEvent>,
    stats: PttStats,
}

/// Açık giden akış: talk-spurt'ün sabit parametreleri.
struct Outgoing {
    peer: PeerRef,
    codec: PttCodec,
    bundle_ms: u8,
}

/// Eşin etkin gelen akışı: ilk `PTT_OPEN`'ın sabitlediği parametreler.
struct Incoming {
    stream_id: StreamId,
    codec: PttCodec,
    bundle_ms: u8,
}

impl Default for Ptt {
    fn default() -> Self {
        Self::new()
    }
}

impl Ptt {
    /// Boş motor: açık akış yok, sıradaki `stream_id` [`FIRST_STREAM_ID`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_stream_id: Some(FIRST_STREAM_ID),
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
            pending: None,
            stats: PttStats::default(),
        }
    }

    /// Gözlem sayaçları.
    #[must_use]
    pub fn stats(&self) -> PttStats {
        self.stats
    }

    /// Bu eşe açık giden akış; yoksa `None`.
    #[must_use]
    pub fn outgoing_stream(&self, peer: PeerRef) -> Option<StreamId> {
        self.outgoing
            .iter()
            .find(|(_, stream)| stream.peer == peer)
            .map(|(&stream_id, _)| stream_id)
    }

    /// Eşin etkin gelen akışı; yoksa `None`.
    #[must_use]
    pub fn incoming_stream(&self, peer: PeerRef) -> Option<StreamId> {
        self.incoming.get(&peer).map(|stream| stream.stream_id)
    }

    /// Basış başlar: `PTT_OPEN` eşin güncel oturumuna `DeliveryPolicy::Reliable` ile verilir ve
    /// atanan `stream_id` döner. `Ok` = Transport kaydı kuyruğa aldı; teslim DEĞİL (S1 K8).
    /// `codec` ve `bundle_ms` akış boyunca sabittir; `bundle_ms` yorumlanmaz (media ekibinin).
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`PttError::StreamActive`] · [`PttError::StreamTableFull`] ·
    /// [`PttError::StreamIdExhausted`] · [`PttError::NoSession`] · [`PttError::Encode`] ·
    /// [`PttError::Transport`]. Hiçbiri `stream_id` tüketmez ya da akış açmaz.
    pub fn open<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        peer: PeerRef,
        codec: PttCodec,
        bundle_ms: u8,
    ) -> Result<StreamId, PttError> {
        if let Some(stream_id) = self.outgoing_stream(peer) {
            return Err(PttError::StreamActive { stream_id });
        }
        if self.outgoing.len() >= MAX_PEERS {
            return Err(PttError::StreamTableFull);
        }
        let stream_id = self.next_stream_id.ok_or(PttError::StreamIdExhausted)?;
        let handle = messenger.session_of(peer).ok_or(PttError::NoSession)?;
        let open = AppMessage::PttOpen {
            stream_id,
            codec,
            bundle_ms,
        };
        send_record(messenger, handle, &open, DeliveryPolicy::Reliable)?;
        self.outgoing.insert(
            stream_id,
            Outgoing {
                peer,
                codec,
                bundle_ms,
            },
        );
        self.next_stream_id = stream_id.checked_add(1);
        Ok(stream_id)
    }

    /// Ses demeti: `MEDIA_BUNDLE` akışın sabit `codec` / `bundle_ms`'iyle, eşin O ANKİ güncel
    /// oturumuna `DeliveryPolicy::BestEffort` ile verilir (S9). Yük opaktır ve sıkıştırılmaz; boyu
    /// kopyalanmadan ÖNCE denetlenir.
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`PttError::UnknownStream`] · [`PttError::PayloadTooLarge`] ·
    /// [`PttError::NoSession`] · [`PttError::Encode`] · [`PttError::Transport`]. Reddedilen demet
    /// yeniden denenmez; geç ses çöptür.
    pub fn send_bundle<T: KamaTransport>(
        &self,
        messenger: &mut Messenger<T>,
        stream_id: StreamId,
        payload: &[u8],
    ) -> Result<(), PttError> {
        let stream = self
            .outgoing
            .get(&stream_id)
            .ok_or(PttError::UnknownStream)?;
        let max_payload_len = max_bundle_payload_bytes(messenger.limits());
        if payload.len() > max_payload_len {
            return Err(PttError::PayloadTooLarge {
                payload_len: payload.len(),
                max_payload_len,
            });
        }
        let handle = messenger
            .session_of(stream.peer)
            .ok_or(PttError::NoSession)?;
        let bundle = AppMessage::MediaBundle {
            stream_id,
            codec: stream.codec.code(),
            bundle_ms: stream.bundle_ms,
            payload: payload.to_vec(),
        };
        send_record(messenger, handle, &bundle, DeliveryPolicy::BestEffort)
    }

    /// Basış biter: akış kapanır ve `PTT_CLOSE` eşin güncel oturumuna `DeliveryPolicy::Reliable`
    /// ile TEK KEZ sunulur.
    ///
    /// # Errors
    ///
    /// [`PttError::UnknownStream`] — açık akış değil; hiçbir şey değişmez. Diğer hatalarda akış
    /// YİNE DE kapanmıştır, yalnız `PTT_CLOSE` Transport'a verilemedi: [`PttError::NoSession`] ·
    /// [`PttError::Encode`] · [`PttError::Transport`]. Yeniden deneme yok (S1 K8); alıcı akışı
    /// eşin bir sonraki `PTT_OPEN`'ında ya da oturum sonunda bitirir (modül belgesi).
    pub fn close<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        stream_id: StreamId,
    ) -> Result<(), PttError> {
        let stream = self
            .outgoing
            .remove(&stream_id)
            .ok_or(PttError::UnknownStream)?;
        let handle = messenger
            .session_of(stream.peer)
            .ok_or(PttError::NoSession)?;
        let close = AppMessage::PttClose { stream_id };
        send_record(messenger, handle, &close, DeliveryPolicy::Reliable)
    }

    /// Bir olay döndürür; okunacak bir şey kalmadıysa `None`. [`Messenger::poll`]'un YERİNE
    /// çağrılır: araya doğrudan `Messenger::poll` girerse PTT kayıtları ve oturum olayları bu
    /// motoru atlar.
    ///
    /// Sıra: (1) bir önceki kaydın ikinci olayı (örtük bitişten sonra `PttStarted`); (2) güncel
    /// oturumu kalmamış eşlerin gelen akışları için `PttEnded`, birer birer; (3) Messenger'ın
    /// olayları. Olay üretmeyen (atılan) PTT kayıtları aynı çağrıda atlanır.
    #[must_use]
    pub fn poll<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>) -> Option<PttEvent> {
        if let Some(event) = self.pending.take() {
            return Some(event);
        }
        if let Some(event) = self.end_orphaned(messenger) {
            return Some(event);
        }
        loop {
            let event = messenger.poll()?;
            if let Some(event) = self.on_event(messenger, event) {
                return Some(event);
            }
        }
    }

    /// Güncel oturumu kalmamış ilk eşin gelen akışını bitirir.
    fn end_orphaned<T: KamaTransport>(&mut self, messenger: &Messenger<T>) -> Option<PttEvent> {
        let peer = self
            .incoming
            .keys()
            .copied()
            .find(|peer| messenger.session_of(*peer).is_none())?;
        let ended = self.incoming.remove(&peer)?;
        self.stats.streams_ended_by_session += 1;
        Some(PttEvent::PttEnded {
            peer,
            stream_id: ended.stream_id,
        })
    }

    fn on_event<T: KamaTransport>(
        &mut self,
        messenger: &Messenger<T>,
        event: MessengerEvent,
    ) -> Option<PttEvent> {
        match event {
            MessengerEvent::Other { peer, message } => self.on_record(messenger, peer, message),
            event => Some(PttEvent::Messenger(event)),
        }
    }

    fn on_record<T: KamaTransport>(
        &mut self,
        messenger: &Messenger<T>,
        peer: PeerRef,
        message: AppMessage,
    ) -> Option<PttEvent> {
        match message {
            AppMessage::PttOpen {
                stream_id,
                codec,
                bundle_ms,
            } => self.on_open(
                messenger,
                peer,
                Incoming {
                    stream_id,
                    codec,
                    bundle_ms,
                },
            ),
            AppMessage::PttClose { stream_id } => self.on_close(peer, stream_id),
            AppMessage::MediaBundle {
                stream_id,
                codec,
                bundle_ms,
                payload,
            } => self.on_bundle(peer, stream_id, codec, bundle_ms, payload),
            message => Some(PttEvent::Messenger(MessengerEvent::Other { peer, message })),
        }
    }

    /// `PTT_OPEN`: yeni akış, tekrar (DROP) ya da örtük bitiş + yeni akış (modül belgesi).
    fn on_open<T: KamaTransport>(
        &mut self,
        messenger: &Messenger<T>,
        peer: PeerRef,
        opened: Incoming,
    ) -> Option<PttEvent> {
        if messenger.session_of(peer).is_none() {
            self.stats.opens_without_session += 1;
            return None;
        }
        let started = PttEvent::PttStarted {
            peer,
            stream_id: opened.stream_id,
            codec: opened.codec,
            bundle_ms: opened.bundle_ms,
        };
        let tracked = self.incoming.len();
        match self.incoming.entry(peer) {
            Entry::Vacant(slot) => {
                if tracked >= MAX_PEERS {
                    self.stats.opens_refused += 1;
                    return None;
                }
                slot.insert(opened);
                Some(started)
            }
            Entry::Occupied(mut slot) => {
                if slot.get().stream_id == opened.stream_id {
                    // Akışın `codec` / `bundle_ms`'i ilk `PTT_OPEN`'la sabitlendi (S9); tekrar
                    // onu değiştiremez.
                    self.stats.duplicate_opens += 1;
                    return None;
                }
                let replaced = slot.insert(opened);
                self.stats.streams_replaced += 1;
                self.pending = Some(started);
                Some(PttEvent::PttEnded {
                    peer,
                    stream_id: replaced.stream_id,
                })
            }
        }
    }

    /// `PTT_CLOSE`: yalnız eşin etkin akışını bitirir.
    fn on_close(&mut self, peer: PeerRef, stream_id: StreamId) -> Option<PttEvent> {
        if self
            .incoming
            .get(&peer)
            .is_some_and(|active| active.stream_id == stream_id)
        {
            self.incoming.remove(&peer);
            return Some(PttEvent::PttEnded { peer, stream_id });
        }
        self.stats.unknown_closes += 1;
        None
    }

    /// `MEDIA_BUNDLE`: etkin akışla `stream_id`, `codec` ve `bundle_ms` eşleşmeli (S7, S9).
    fn on_bundle(
        &mut self,
        peer: PeerRef,
        stream_id: StreamId,
        codec: u8,
        bundle_ms: u8,
        payload: Vec<u8>,
    ) -> Option<PttEvent> {
        let Some(active) = self.incoming.get(&peer) else {
            self.stats.bundles_without_stream += 1;
            return None;
        };
        if active.stream_id != stream_id {
            self.stats.bundles_unknown_stream += 1;
            return None;
        }
        if active.codec.code() != codec || active.bundle_ms != bundle_ms {
            self.stats.bundles_mismatched += 1;
            return None;
        }
        Some(PttEvent::PttAudio {
            peer,
            stream_id,
            payload,
        })
    }
}

/// Kaydı kodlar ve `handle`'a tek deneme verir; hata yolunda hiçbir şey kuyruğa girmez.
fn send_record<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    handle: SessionHandle,
    message: &AppMessage,
    policy: DeliveryPolicy,
) -> Result<(), PttError> {
    let unit = codec::encode(message, messenger.limits()).map_err(PttError::Encode)?;
    messenger
        .transport_mut()
        .send(handle, unit, policy)
        .map_err(PttError::Transport)
}

#[cfg(test)]
mod tests {
    //! Tablo tavanları, `stream_id` tükenmesi ve `Debug` — uçtan uca akış `tests/ptt.rs`'te.

    use super::*;
    use kama_contracts::s1::{
        ConnectResult, ConnectTicket, Delivered, PreparedUnit, SessionEvent, TransportCapabilities,
    };

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
    /// Test talk-spurt'ünün demet süresi (ms; normatif değil).
    const TEST_BUNDLE_MS: u8 = 20;

    /// Her `connect`'e yeni bir hazır oturumla yanıt veren, her gönderimi kabul eden, hiçbir şey
    /// teslim etmeyen Transport.
    #[derive(Default)]
    struct ReadyTransport {
        /// Basılan son tutamacın ham değeri.
        last_handle: u64,
    }

    impl KamaTransport for ReadyTransport {
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
            _peer: PeerRef,
            _early_unit: Option<PreparedUnit>,
        ) -> Result<ConnectResult, TransportError> {
            self.last_handle += 1;
            Ok(ConnectResult::Ready(SessionHandle::from_raw(
                self.last_handle,
            )))
        }

        fn send(
            &mut self,
            _session: SessionHandle,
            _unit: PreparedUnit,
            _policy: DeliveryPolicy,
        ) -> Result<(), TransportError> {
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

    /// `n`'den türetilmiş sentetik eş (ilk sekiz bayt `n`, BE).
    fn peer(n: usize) -> PeerRef {
        let mut dest_hash = [0u8; 32];
        let index = u64::try_from(n).expect("test sayısı u64'e sığar");
        dest_hash[..8].copy_from_slice(&index.to_be_bytes());
        PeerRef::from_dest_hash(dest_hash)
    }

    /// Verilen her eşle hazır oturumu olan motor.
    fn messenger_with(peers: impl IntoIterator<Item = PeerRef>) -> Messenger<ReadyTransport> {
        let mut messenger = Messenger::new(ReadyTransport::default()).expect("limitler geçerli");
        for p in peers {
            assert!(matches!(messenger.connect(p), Ok(ConnectResult::Ready(_))));
        }
        messenger
    }

    /// `peer`'den gelmiş gibi bir `PTT_OPEN` olayı.
    fn open_from(peer: PeerRef, stream_id: StreamId) -> MessengerEvent {
        MessengerEvent::Other {
            peer,
            message: AppMessage::PttOpen {
                stream_id,
                codec: PttCodec::Opus,
                bundle_ms: TEST_BUNDLE_MS,
            },
        }
    }

    #[test]
    fn stream_id_exhaustion_is_typed_and_never_wraps() {
        let p = peer(0);
        let mut messenger = messenger_with([p]);
        let mut ptt = Ptt::new();
        assert_eq!(ptt.next_stream_id, Some(FIRST_STREAM_ID));
        ptt.next_stream_id = Some(StreamId::MAX);
        let last = ptt
            .open(&mut messenger, p, PttCodec::Opus, TEST_BUNDLE_MS)
            .expect("son stream_id kullanılabilir");
        assert_eq!(last, StreamId::MAX);
        ptt.close(&mut messenger, last).expect("kapanır");
        assert_eq!(
            ptt.open(&mut messenger, p, PttCodec::Opus, TEST_BUNDLE_MS),
            Err(PttError::StreamIdExhausted)
        );
        assert_eq!(ptt.outgoing_stream(p), None);
    }

    #[test]
    fn stream_tables_are_bounded() {
        // Bir motorun eşleri `MAX_PEERS` ile sınırlı; tavanın ötesindeki eş ikinci motordadır.
        let mut full = messenger_with((0..MAX_PEERS).map(peer));
        let extra = peer(MAX_PEERS);
        let mut other = messenger_with([extra]);
        let mut ptt = Ptt::new();
        for n in 0..MAX_PEERS {
            ptt.open(&mut full, peer(n), PttCodec::Opus, TEST_BUNDLE_MS)
                .expect("tavanın altında");
            assert!(matches!(
                ptt.on_event(&full, open_from(peer(n), FIRST_STREAM_ID)),
                Some(PttEvent::PttStarted { .. })
            ));
        }
        assert_eq!(
            ptt.open(&mut other, extra, PttCodec::Opus, TEST_BUNDLE_MS),
            Err(PttError::StreamTableFull)
        );
        assert_eq!(ptt.outgoing_stream(extra), None);
        assert_eq!(
            ptt.on_event(&other, open_from(extra, FIRST_STREAM_ID)),
            None
        );
        assert_eq!(ptt.stats().opens_refused, 1);
        assert_eq!(ptt.incoming_stream(extra), None);
        assert_eq!(ptt.incoming_stream(peer(0)), Some(FIRST_STREAM_ID));
    }

    #[test]
    fn debug_redacts_the_audio_payload() {
        let payload = b"gizli-ses".to_vec();
        let event = PttEvent::PttAudio {
            peer: peer(0),
            stream_id: FIRST_STREAM_ID,
            payload: payload.clone(),
        };
        let shown = format!("{event:?}");
        assert!(
            !shown.contains(&format!("{payload:?}")),
            "Debug ses yükünü sızdırdı: {shown}"
        );
        assert!(shown.contains("payload_len"), "{shown}");
    }
}
