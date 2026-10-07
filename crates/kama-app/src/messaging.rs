//! Metin + alındı (B2.2) — AMP `TEXT` (0x01) ve `RECEIPT` (0x08) akışı
//! (`specs/application-message-protocol.md` §3; S2 yaması 2026-10-08).
//!
//! [`Messenger`] S1 [`KamaTransport`]'u sahiplenir ve onun üstünde iki yarıyı yürütür:
//!
//! * **Gönderen** — [`Messenger::send_text`] eşin sıradaki `msg_id`'sini atar, TEXT kaydını
//!   codec'le kurar ([`codec::encode`]) ve eşin güncel oturumuna `DeliveryPolicy::Reliable` ile
//!   verir. Kayıt, eşin `RECEIPT(delivered)`'ı gelene kadar outbox'ta tutulur.
//! * **Alıcı** — [`Messenger::poll`] önce Transport olaylarını, sonra teslim edilen birimleri okur.
//!   Dedup penceresinde ilk kez görülen TEXT [`MessengerEvent::TextReceived`] üretir; HER TEXT'e
//!   (tekrar dahil) `RECEIPT(delivered)` döner. RECEIPT outbox'taki kaydı kapatır ve `msg_id`
//!   başına tam bir kez [`MessengerEvent::Delivered`] üretir.
//!
//! # `msg_id` eş başınadır (S2, Abkarada 2026-10-08)
//!
//! Gönderen sayacı ve alıcı dedup'ı `PeerRef` anahtarlıdır, `SessionHandle` değil: supersession
//! (`Ready(h')` → `Closed(h, Superseded)`) de kapanış + yeniden bağlanma da ikisini sıfırlamaz.
//! Sayaç kesin artar ve YALNIZ Transport kaydı kabul ettiğinde (`send` → `Ok(())`) ilerler. Her
//! `Err` kaydın kuyruğa alınmadığı anlamına gelir (S1 §3): o `msg_id` tele çıkmamıştır ve sıradaki
//! gönderimde kullanılır. Böylece alıcıdaki `msg_id` boşluğu yalnız yolda kaybolmuş bir TEXT'i
//! gösterir (identity T-16 tespit sinyali). `u32` alanı tükenince
//! [`MessengerError::MsgIdExhausted`]; sarma YOK.
//!
//! # Alıcı dedup penceresi ([`MSG_DEDUP_WINDOW`], S7)
//!
//! Eş başına, görülen en büyük `msg_id` ile biten [`MSG_DEDUP_WINDOW`] ardışık `msg_id`'lik bit
//! haritası. Alıcı teslim sırasını `msg_id` ile ZORLAMAZ: pencere içindeki görülmemiş her `msg_id`
//! (en büyükten küçük olsa da) teslim edilir. Pencerede görülmüş `msg_id` → olay YOK, `RECEIPT`
//! yeniden (S2). Pencerenin ALTINDA kalan `msg_id`'nin görülüp görülmediği bilinemez →
//! fail-closed: tekrar sayılır (teslim yok, `RECEIPT` yeniden). Codec'in attığı kayıt pencereye
//! dokunmaz.
//!
//! # Gönderen penceresi
//!
//! Outbox eş başına en çok [`MAX_OUTBOX_UNACKED`] (S7) kayıt tutar. Ayrıca en eski RECEIPT'siz
//! `msg_id` ile atanacak `msg_id` arası [`MSG_DEDUP_WINDOW`]'a ulaşamaz
//! ([`MessengerError::UnackedSpanFull`]). Alıcının gördüğü en büyük `msg_id`, göndericinin
//! atadığı en büyükten büyük olamaz; bu yüzden outbox'tan yeniden gönderilen her kayıt alıcı
//! penceresinin İÇİNDE kalır ve dedup kararı kesindir. Pencere-altı durumu yalnız sayacını
//! yitirmiş (yeniden başlamış) bir göndericide doğar.
//!
//! # RECEIPT
//!
//! `delivered` = alıcı Application kaydı işledi (AMP §3, A4). Application üretir; Transport'un
//! `Ok(())`'undan ya da başka bir Transport sinyalinden TÜREMEZ (S1 K8). Eşin canlılığının tek
//! sinyali [`MessengerEvent::Delivered`]'dır; `ConnectProgress` hub yolunun durumudur (S1 §4).
//! RECEIPT eşin güncel oturumundan gider. Gönderilemezse (oturum yok ya da Transport reddi)
//! kaybolur ve yeniden denenmez: gönderen kaydı bir sonraki oturum değişiminde yeniden gönderir,
//! tekrar TEXT RECEIPT'i yeniden üretir.
//!
//! # Retry yok — tek yeniden gönderim yolu oturum değişimi (S1 K7/K8, identity §16 adım 3)
//!
//! Application retransmit timer'ı tutmaz, kayıp tespit etmez; Transport'un KABUL ettiği bir kayıt
//! aynı oturum tutamacına bir daha verilmez. Eşin tutamacı değişince (`Ready(h', eş)`) Transport
//! `h`'de teslim edilmemiş birimleri `h'`ye TAŞIMAZ (K7); Application RECEIPT'i gelmemiş kayıtları
//! `h'` üzerinden ESKİ `msg_id` (aynı bayt) ile yeniden gönderir — her kayıt her tutamaca en çok
//! bir kez kabul ettirilir. `Ready(h')` okunduğunda `h`'nin drain'inden gelecek RECEIPT'ler henüz
//! okunmamış olabilir; o kayıtlar da yeniden gider ve alıcının dedup'ında söner (teslim yok,
//! RECEIPT yeniden). Transport'un REDDETTİĞİ (kuyruğa almadığı) bir yeniden gönderim sonraki
//! [`Messenger::poll`] çağrılarında bekler (K8 "bekler"): kabul edilmemiş birim tele çıkmamıştır.
//!
//! # Bilinen sınır (sahibe açık)
//!
//! Sayaç kalıcı değildir (AMP §3: yeniden başlatmada devam garantisi yok). Durumunu yitirip
//! yeniden başlayan bir göndericinin `msg_id`'leri, eşin hâlâ tuttuğu dedup penceresinde görülmüş
//! sayılabilir: o TEXT'ler teslim edilmez ama RECEIPT alır.
//!
//! # Gizlilik
//!
//! [`MessengerEvent`]'in `Debug` çıktısı metni değil uzunluğunu yazar; outbox kayıtları hiçbir
//! `Debug` çıktısına girmez ([`Messenger`] `Debug` değildir).

use core::fmt;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    ConnectResult, Delivered, DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent,
    TransportError,
};

use crate::codec::{self, AmpLimits, AppMessage, EncodeError, LimitsError, ReceiptKind};

/// `TEXT.msg_id` ve `RECEIPT.ref_msg_id` (AMP §3; telde `u32_be`).
pub type MsgId = u32;

/// Bir eşe atanan ilk `msg_id`. Spec başlangıç değeri koymaz; değer bu uygulamanındır.
pub const FIRST_MSG_ID: MsgId = 0;

/// S7 (Abkarada 2026-10-08) — alıcının eş başına dedup penceresi (`msg_id` sayısı).
pub const MSG_DEDUP_WINDOW: MsgId = 4096;

/// S7 (Abkarada 2026-10-08) — eş başına Transport'un kabul ettiği ama `RECEIPT(delivered)`'ı
/// gelmemiş TEXT tavanı.
pub const MAX_OUTBOX_UNACKED: usize = 256;

/// Eş tablosu tavanı — yerel bellek sınırı, normatif DEĞİL `[KALİBRASYON]`. Eş durumu (sayaç,
/// dedup, outbox) oturumlardan sağ çıkmak zorunda olduğu için tablodan eş ÇIKARILMAZ: çıkarılan
/// eşin sayacı baştan başlar ve eşin dedup penceresine çarpardı. Tavan doluyken YENİ eş reddedilir.
pub const MAX_PEERS: usize = 64;

/// Güncelliğini yitirmiş tutamaç → eş eşlemelerinin FIFO tavanı — yerel sınır, normatif DEĞİL.
/// `Closed(h)` okunduktan sonra teslim kuyruğunda kalmış `h` birimleri (S1 K7 yalnız-alım drain) bu
/// eşlemeyle eşine bağlanır; tavan taşınca en eski eşleme düşer ve o tutamacın geç birimi atılır.
pub const MAX_RETIRED_SESSIONS: usize = MAX_PEERS;

/// Dedup bit haritasının `u64` kelime sayısı.
const DEDUP_WORDS: usize = (MSG_DEDUP_WINDOW / u64::BITS) as usize;

// Pencere tam kelimelerle kaplanır (bit konumu = `msg_id % MSG_DEDUP_WINDOW`).
const _: () = assert!(MSG_DEDUP_WINDOW.is_multiple_of(u64::BITS));

/// [`Messenger::poll`] çıktısı.
#[derive(Clone, PartialEq, Eq)]
pub enum MessengerEvent {
    /// Transport olayı — motorun tutamaç eşlemesi güncellendikten sonra AYNEN iletilir (bağlantı
    /// ve UI katmanı için). `ConnectProgress` eşin canlılığını BİLDİRMEZ (S1 §4).
    Session(SessionEvent),
    /// Eşten yeni metin: bu `msg_id` eşin dedup penceresinde ilk kez görüldü. `RECEIPT(delivered)`
    /// bu olay üretilirken gönderildi.
    TextReceived {
        /// Gönderen eş.
        peer: PeerRef,
        /// Gönderenin `msg_id`'si.
        msg_id: MsgId,
        /// UTF-8 metin (codec doğruladı).
        text: String,
    },
    /// Eş, gönderilen TEXT'i işlediğini bildirdi (`RECEIPT(delivered)`) — `msg_id` başına tam bir
    /// kez. Eşin canlılığının TEK sinyali (S1 K8, §4).
    Delivered {
        /// RECEIPT'i gönderen eş.
        peer: PeerRef,
        /// [`Messenger::send_text`]'in döndürdüğü `msg_id`.
        msg_id: MsgId,
    },
    /// Bu motorun işlemediği geçerli AMP kaydı (`FILE_*`, `PTT_*`, `SIGNALING_BLOB`,
    /// `MEDIA_BUNDLE`, `RECEIPT(read)`) — aynen iletilir; RECEIPT üretilmez.
    Other {
        /// Kaydı gönderen eş.
        peer: PeerRef,
        /// Çözülmüş kayıt.
        message: AppMessage,
    },
}

/// Metin YAZILMAZ; yalnız uzunluğu. `PeerRef` ve `AppMessage` kendi `Debug`'larıyla (ikisi de
/// içerik basmaz).
impl fmt::Debug for MessengerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(event) => f.debug_tuple("Session").field(event).finish(),
            Self::TextReceived { peer, msg_id, text } => f
                .debug_struct("TextReceived")
                .field("peer", peer)
                .field("msg_id", msg_id)
                .field("text_len", &text.len())
                .finish_non_exhaustive(),
            Self::Delivered { peer, msg_id } => f
                .debug_struct("Delivered")
                .field("peer", peer)
                .field("msg_id", msg_id)
                .finish(),
            Self::Other { peer, message } => f
                .debug_struct("Other")
                .field("peer", peer)
                .field("message", message)
                .finish(),
        }
    }
}

/// [`Messenger::send_text`] ve [`Messenger::connect`] hatası. Her hata yolunda motor durumu
/// DEĞİŞMEZ: `msg_id` tüketilmez, outbox'a kayıt girmez. Motor hiçbirini yeniden denemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessengerError {
    /// Eşin hazır oturumu yok: `Ready` görülmedi ya da `Closed` okundu. Motor metni kuyruğa
    /// ALMAZ; oturum [`Messenger::connect`] ile istenir.
    NoSession,
    /// Eş tablosu [`MAX_PEERS`]'te ve bu eş izlenmiyor (yeni).
    PeerTableFull,
    /// Eşe [`MAX_OUTBOX_UNACKED`] TEXT RECEIPT bekliyor — geri basınç.
    OutboxFull,
    /// En eski RECEIPT'siz `msg_id` ile sıradaki `msg_id` arası [`MSG_DEDUP_WINDOW`]'a ulaştı —
    /// geri basınç (modül belgesi, "Gönderen penceresi").
    UnackedSpanFull {
        /// RECEIPT'i gelmemiş en eski `msg_id`.
        oldest: MsgId,
    },
    /// Eşin `u32` `msg_id` alanı tükendi; sarma YOK.
    MsgIdExhausted,
    /// Metin bir `AppMessageRecord`'a sığmıyor (`MAX_BODY_BYTES`, S1 K3).
    Encode(EncodeError),
    /// Transport reddetti; kayıt kuyruğa ALINMADI (S1 §3). Yeniden deneme kararı çağıranındır.
    Transport(TransportError),
}

impl fmt::Display for MessengerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSession => f.write_str("peer has no ready session"),
            Self::PeerTableFull => write!(f, "peer table is full ({MAX_PEERS} peers)"),
            Self::OutboxFull => write!(
                f,
                "{MAX_OUTBOX_UNACKED} TEXT records already await RECEIPT from this peer"
            ),
            Self::UnackedSpanFull { oldest } => write!(
                f,
                "msg_id {oldest} still awaits RECEIPT; the next msg_id would leave the \
                 {MSG_DEDUP_WINDOW}-wide dedup window"
            ),
            Self::MsgIdExhausted => f.write_str("msg_id space of this peer is exhausted (no wrap)"),
            Self::Encode(e) => write!(f, "TEXT does not fit one AMP record: {e}"),
            Self::Transport(e) => write!(f, "transport refused the record: {e}"),
        }
    }
}

impl std::error::Error for MessengerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::NoSession
            | Self::PeerTableFull
            | Self::OutboxFull
            | Self::UnackedSpanFull { .. }
            | Self::MsgIdExhausted => None,
        }
    }
}

/// Gözlem sayaçları — salt-okunur; protokol davranışını değiştirmez.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MessengerStats {
    /// Codec'in DROP/REJECT ettiği kayıtlar (sinyal yok — AMP §5 kural 3, S8).
    pub records_dropped: u64,
    /// Hiçbir eşe bağlanamayan tutamaçtan gelen birimler (atıldı).
    pub units_unknown_session: u64,
    /// Dedup penceresinde görülmüş `msg_id`'li TEXT'ler (teslim yok, RECEIPT yeniden).
    pub duplicate_texts: u64,
    /// Pencerenin altında kalan `msg_id`'li TEXT'ler (fail-closed tekrar; teslim yok, RECEIPT
    /// yeniden).
    pub below_window_texts: u64,
    /// Transport'un kabul ettiği RECEIPT'ler.
    pub receipts_sent: u64,
    /// Gönderilemeyen RECEIPT'ler (oturum yok ya da Transport reddi; yeniden denenmez).
    pub receipts_unsent: u64,
    /// Outbox'ta karşılığı olmayan `RECEIPT(delivered)`'lar (tekrar ya da bilinmeyen `msg_id`).
    pub receipts_ignored: u64,
    /// Oturum değişiminde Transport'un kabul ettiği yeniden gönderimler (S1 K7).
    pub resent_on_new_session: u64,
    /// Transport'un reddettiği oturum-değişimi yeniden gönderim denemeleri (sonraki `poll`'da
    /// beklemeye devam eder).
    pub resend_refused: u64,
    /// Tablo doluyken `Ready`'si gelen yeni eşler (izlenmedi; birimleri atılır).
    pub peers_refused: u64,
}

/// B2.2 metin motoru. Ayrıntı: modül belgesi.
pub struct Messenger<T> {
    transport: T,
    limits: AmpLimits,
    /// Eş başına durum (≤ [`MAX_PEERS`]); hiçbir eş çıkarılmaz.
    peers: BTreeMap<PeerRef, PeerState>,
    /// Güncel tutamaç → eş. `PeerState::session` ile birbirinin aynası; her eşin en çok bir güncel
    /// tutamacı vardır.
    by_handle: BTreeMap<SessionHandle, PeerRef>,
    /// Güncelliğini yitirmiş tutamaçlar (yerine yenisi geçti ya da `Closed` okundu), eskiden yeniye
    /// (≤ [`MAX_RETIRED_SESSIONS`]).
    retired: VecDeque<(SessionHandle, PeerRef)>,
    stats: MessengerStats,
}

impl<T: KamaTransport> Messenger<T> {
    /// Motoru kurar. AMP limitleri `transport.capabilities()`'ten bir kez türetilir (A2, S1 K5).
    ///
    /// # Errors
    ///
    /// [`LimitsError`] — Transport'un beyan ettiği birim AMP için çok küçük.
    pub fn new(transport: T) -> Result<Self, LimitsError> {
        let limits = AmpLimits::from_capabilities(&transport.capabilities())?;
        Ok(Self {
            transport,
            limits,
            peers: BTreeMap::new(),
            by_handle: BTreeMap::new(),
            retired: VecDeque::new(),
            stats: MessengerStats::default(),
        })
    }

    /// Alttaki Transport.
    #[must_use]
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Alttaki Transport — `close`, `cancel` ve sahte senaryoları için. Buradan çağrılan `recv` /
    /// `poll_event` motorun göreceği birimi ve olayı tüketir; `connect` için [`Messenger::connect`]
    /// kullanılır (dönen tutamacı motor benimser).
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Bu motorun AMP limitleri.
    #[must_use]
    pub fn limits(&self) -> &AmpLimits {
        &self.limits
    }

    /// Gözlem sayaçları.
    #[must_use]
    pub fn stats(&self) -> MessengerStats {
        self.stats
    }

    /// Eşin güncel oturum tutamacı (gönderimin gideceği); yoksa `None`.
    #[must_use]
    pub fn session_of(&self, peer: PeerRef) -> Option<SessionHandle> {
        self.peers.get(&peer).and_then(|state| state.session)
    }

    /// Eşe Transport'un kabul ettiği, `RECEIPT(delivered)`'ı henüz gelmemiş TEXT sayısı.
    #[must_use]
    pub fn unacked_count(&self, peer: PeerRef) -> usize {
        self.peers.get(&peer).map_or(0, |state| state.outbox.len())
    }

    /// `KamaTransport::connect(peer, None)` (S1 K2, KARAR-1 idempotent). Aktif oturumun tutamacı
    /// `ConnectResult::Ready(h)` ile olay YAYINLANMADAN dönebildiği için burada benimsenir;
    /// `Pending` [`Messenger::poll`]'da okunan `Ready` ile çözülür. `early_unit` B2.5'indir; burada
    /// verilmez.
    ///
    /// # Errors
    ///
    /// [`MessengerError::PeerTableFull`] (Transport çağrılmaz) · [`MessengerError::Transport`].
    pub fn connect(&mut self, peer: PeerRef) -> Result<ConnectResult, MessengerError> {
        if !self.peers.contains_key(&peer) && self.peers.len() >= MAX_PEERS {
            return Err(MessengerError::PeerTableFull);
        }
        let result = self
            .transport
            .connect(peer, None)
            .map_err(MessengerError::Transport)?;
        if let ConnectResult::Ready(handle) = result {
            self.adopt(handle, peer);
        }
        Ok(result)
    }

    /// TEXT gönderir ve atanan `msg_id`'yi döndürür. `Ok` = Transport kaydı kuyruğa aldı; teslim
    /// DEĞİL (S1 K8) — teslim [`MessengerEvent::Delivered`] ile öğrenilir. Kayıt sıkıştırılmaz
    /// (bu sprint `compression_id` = NONE).
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`MessengerError::PeerTableFull`] (izlenmeyen eş, tablo dolu) ya da
    /// [`MessengerError::NoSession`] · [`MessengerError::OutboxFull`] ·
    /// [`MessengerError::MsgIdExhausted`] · [`MessengerError::UnackedSpanFull`] ·
    /// [`MessengerError::Encode`] · [`MessengerError::Transport`]. Hiçbiri `msg_id` tüketmez.
    pub fn send_text(&mut self, peer: PeerRef, text: &str) -> Result<MsgId, MessengerError> {
        let Some(state) = self.peers.get_mut(&peer) else {
            return Err(if self.peers.len() >= MAX_PEERS {
                MessengerError::PeerTableFull
            } else {
                MessengerError::NoSession
            });
        };
        let handle = state.session.ok_or(MessengerError::NoSession)?;
        let msg_id = state.next_send_id()?;
        let message = AppMessage::Text {
            msg_id,
            text: text.to_owned(),
        };
        let unit = codec::encode(&message, &self.limits).map_err(MessengerError::Encode)?;
        self.transport
            .send(handle, unit.clone(), DeliveryPolicy::Reliable)
            .map_err(MessengerError::Transport)?;
        state.commit_send(msg_id, unit, handle);
        Ok(msg_id)
    }

    /// Bir olay döndürür; Transport'ta okunacak bir şey kalmadıysa `None`.
    ///
    /// Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur;
    /// (2) Transport olayları, birer birer; (3) teslim edilen birimler. Olaylar birimlerden ÖNCE
    /// okunur: yeni oturumun ilk birimi `Ready(h')` okunmadan işlenseydi eşi çözülemezdi. Olay
    /// üretmeyen birimler (tekrar TEXT, karşılıksız RECEIPT, atılan kayıt) aynı çağrıda atlanır;
    /// her tur Transport'tan bir birim tüketir.
    #[must_use]
    pub fn poll(&mut self) -> Option<MessengerEvent> {
        self.flush_resends();
        loop {
            if let Some(event) = self.transport.poll_event() {
                self.on_session_event(event);
                return Some(MessengerEvent::Session(event));
            }
            let delivered = self.transport.recv()?;
            if let Some(event) = self.on_unit(delivered) {
                return Some(event);
            }
        }
    }

    fn on_session_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::Ready(handle, peer) => self.adopt(handle, peer),
            SessionEvent::Closed(handle, _) => self.retire(handle),
            SessionEvent::RenewRequired(_)
            | SessionEvent::ConnectAbandoned(..)
            | SessionEvent::ConnectProgress(..) => {}
        }
    }

    /// `handle`'ı `peer`'in güncel tutamacı yapar (`Ready` olayı ya da `ConnectResult::Ready`).
    /// Önceki güncel tutamaç emekliye ayrılır; RECEIPT'siz kayıtlar yeni tutamaçtan yeniden
    /// gönderilir (S1 K7).
    fn adopt(&mut self, handle: SessionHandle, peer: PeerRef) {
        let known_peers = self.peers.len();
        let state = match self.peers.entry(peer) {
            Entry::Occupied(slot) => slot.into_mut(),
            Entry::Vacant(slot) => {
                if known_peers >= MAX_PEERS {
                    self.stats.peers_refused += 1;
                    return;
                }
                slot.insert(PeerState::new())
            }
        };
        if state.session == Some(handle) {
            // Aynı tutamacın ikinci kez bildirilmesi (ör. `connect` dönüşü + `Ready` olayı).
            return;
        }
        if let Some(previous) = state.session.replace(handle) {
            self.by_handle.remove(&previous);
            push_retired(&mut self.retired, previous, peer);
        }
        self.by_handle.insert(handle, peer);
        state.resend_pending = !state.outbox.is_empty();
        flush_peer(&mut self.transport, state, &mut self.stats);
    }

    /// `Closed(handle, _)`: eşin güncel tutamacıysa eşin oturumu kalmaz; tutamaç emekliye ayrılır
    /// (geç teslimler hâlâ eşine bağlanır).
    fn retire(&mut self, handle: SessionHandle) {
        let Some(peer) = self.by_handle.remove(&handle) else {
            // Zaten emekli (yerine yenisi geçmişti) ya da hiç benimsenmemiş tutamaç.
            return;
        };
        if let Some(state) = self.peers.get_mut(&peer) {
            state.session = None;
        }
        push_retired(&mut self.retired, handle, peer);
    }

    /// Birimin geldiği tutamacın eşi: önce güncel, sonra emekli tutamaçlar.
    fn peer_of(&self, handle: SessionHandle) -> Option<PeerRef> {
        self.by_handle.get(&handle).copied().or_else(|| {
            self.retired
                .iter()
                .find(|(retired, _)| *retired == handle)
                .map(|&(_, peer)| peer)
        })
    }

    /// Reddedilmiş yeniden gönderimi olan eşler için güncel tutamaca yeniden sunar.
    fn flush_resends(&mut self) {
        for state in self.peers.values_mut() {
            if state.resend_pending {
                flush_peer(&mut self.transport, state, &mut self.stats);
            }
        }
    }

    fn on_unit(&mut self, delivered: Delivered) -> Option<MessengerEvent> {
        let Some(peer) = self.peer_of(delivered.session) else {
            self.stats.units_unknown_session += 1;
            return None;
        };
        let Ok(message) = codec::decode(&delivered.unit, &self.limits) else {
            // DROP ve REJECT alıcıda aynı sonucu verir: kayıt atılır, karşıya sinyal gitmez.
            self.stats.records_dropped += 1;
            return None;
        };
        match message {
            AppMessage::Text { msg_id, text } => self.on_text(peer, msg_id, text),
            AppMessage::Receipt {
                ref_msg_id,
                kind: ReceiptKind::Delivered,
            } => self.on_receipt(peer, ref_msg_id),
            message => Some(MessengerEvent::Other { peer, message }),
        }
    }

    /// TEXT: dedup kararı + her durumda `RECEIPT(delivered)` (S2: tekrar TEXT → RECEIPT yeniden).
    fn on_text(&mut self, peer: PeerRef, msg_id: MsgId, text: String) -> Option<MessengerEvent> {
        let state = self.peers.get_mut(&peer)?;
        let admission = state.dedup.admit(msg_id);
        send_receipt(
            &mut self.transport,
            state.session,
            msg_id,
            &self.limits,
            &mut self.stats,
        );
        match admission {
            Admission::New => Some(MessengerEvent::TextReceived { peer, msg_id, text }),
            Admission::Duplicate => {
                self.stats.duplicate_texts += 1;
                None
            }
            Admission::BelowWindow => {
                self.stats.below_window_texts += 1;
                None
            }
        }
    }

    /// `RECEIPT(delivered)`: outbox'taki kaydı kapatır — `msg_id` başına tam bir [`Delivered`].
    ///
    /// [`Delivered`]: MessengerEvent::Delivered
    fn on_receipt(&mut self, peer: PeerRef, ref_msg_id: MsgId) -> Option<MessengerEvent> {
        let state = self.peers.get_mut(&peer)?;
        if state.outbox.remove(&ref_msg_id).is_some() {
            Some(MessengerEvent::Delivered {
                peer,
                msg_id: ref_msg_id,
            })
        } else {
            self.stats.receipts_ignored += 1;
            None
        }
    }
}

/// Eş başına durum — oturumlardan bağımsız (S2).
struct PeerState {
    /// Gönderimin gideceği güncel tutamaç.
    session: Option<SessionHandle>,
    /// Sıradaki `msg_id`; `None` = `u32` alanı tükendi.
    next_msg_id: Option<MsgId>,
    /// Transport'un kabul ettiği, RECEIPT bekleyen kayıtlar; `msg_id` sırasıyla
    /// (≤ [`MAX_OUTBOX_UNACKED`]).
    outbox: BTreeMap<MsgId, OutboxEntry>,
    /// Outbox'ta güncel tutamaca henüz kabul ettirilememiş kayıt var.
    resend_pending: bool,
    /// Alıcı dedup penceresi.
    dedup: DedupWindow,
}

/// RECEIPT bekleyen kayıt.
struct OutboxEntry {
    /// Kodlanmış TEXT kaydı: yeniden gönderim AYNI baytları (aynı `msg_id`) taşır.
    unit: PreparedUnit,
    /// Kaydı en son kabul eden tutamaç.
    sent_on: SessionHandle,
}

impl PeerState {
    fn new() -> Self {
        Self {
            session: None,
            next_msg_id: Some(FIRST_MSG_ID),
            outbox: BTreeMap::new(),
            resend_pending: false,
            dedup: DedupWindow::new(),
        }
    }

    /// Gönderim ön denetimleri; atanacak `msg_id`'yi döndürür (henüz TÜKETMEZ).
    fn next_send_id(&self) -> Result<MsgId, MessengerError> {
        if self.outbox.len() >= MAX_OUTBOX_UNACKED {
            return Err(MessengerError::OutboxFull);
        }
        let msg_id = self.next_msg_id.ok_or(MessengerError::MsgIdExhausted)?;
        if let Some((&oldest, _)) = self.outbox.first_key_value() {
            // Outbox'taki her `msg_id` atanacaktan küçüktür; `checked_sub` yalnız bozulmuş bir
            // durumda `None` verir ve o da reddedilir (fail-closed).
            if msg_id
                .checked_sub(oldest)
                .is_none_or(|span| span >= MSG_DEDUP_WINDOW)
            {
                return Err(MessengerError::UnackedSpanFull { oldest });
            }
        }
        Ok(msg_id)
    }

    /// Transport kaydı kabul etti: `msg_id` tüketilir, kayıt outbox'a girer.
    fn commit_send(&mut self, msg_id: MsgId, unit: PreparedUnit, handle: SessionHandle) {
        self.outbox.insert(
            msg_id,
            OutboxEntry {
                unit,
                sent_on: handle,
            },
        );
        self.next_msg_id = msg_id.checked_add(1);
    }
}

/// Eşin RECEIPT'siz kayıtlarını güncel tutamaca, `msg_id` sırasıyla kabul ettirir. Bir kayıt bir
/// tutamaca en çok bir kez kabul ettirilir (`sent_on`); ilk retle durur ve kalanlar bekler.
fn flush_peer<T: KamaTransport>(
    transport: &mut T,
    state: &mut PeerState,
    stats: &mut MessengerStats,
) {
    let Some(handle) = state.session else {
        // Oturum yok: bir sonraki `Ready` yeniden başlatır.
        return;
    };
    for entry in state.outbox.values_mut() {
        if entry.sent_on == handle {
            continue;
        }
        if transport
            .send(handle, entry.unit.clone(), DeliveryPolicy::Reliable)
            .is_err()
        {
            stats.resend_refused += 1;
            return;
        }
        entry.sent_on = handle;
        stats.resent_on_new_session += 1;
    }
    state.resend_pending = false;
}

/// `RECEIPT(delivered)`'ı eşin güncel tutamacından verir. Tek deneme: oturum yoksa ya da Transport
/// reddederse kaybolur (retry yok; modül belgesi, "RECEIPT").
fn send_receipt<T: KamaTransport>(
    transport: &mut T,
    session: Option<SessionHandle>,
    ref_msg_id: MsgId,
    limits: &AmpLimits,
    stats: &mut MessengerStats,
) {
    let receipt = AppMessage::Receipt {
        ref_msg_id,
        kind: ReceiptKind::Delivered,
    };
    // RECEIPT gövdesi 5 B'tır ve her geçerli `AmpLimits`'e sığar (MAX_BODY_BYTES ≥ FILE_CHUNK
    // öneki); kodlama hatası yine de gönderilemeyen RECEIPT sayılır, panik yok.
    let accepted = match (session, codec::encode(&receipt, limits)) {
        (Some(handle), Ok(unit)) => transport
            .send(handle, unit, DeliveryPolicy::Reliable)
            .is_ok(),
        _ => false,
    };
    if accepted {
        stats.receipts_sent += 1;
    } else {
        stats.receipts_unsent += 1;
    }
}

/// Emekli tutamacı FIFO'ya ekler; tavan doluysa en eskiyi düşürür.
fn push_retired(
    retired: &mut VecDeque<(SessionHandle, PeerRef)>,
    handle: SessionHandle,
    peer: PeerRef,
) {
    if retired.len() >= MAX_RETIRED_SESSIONS {
        retired.pop_front();
    }
    retired.push_back((handle, peer));
}

/// [`DedupWindow::admit`] kararı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
    /// İlk kez görüldü: teslim edilir.
    New,
    /// Pencerede görülmüş: teslim edilmez.
    Duplicate,
    /// Pencerenin altında: görülüp görülmediği bilinemez; fail-closed tekrar sayılır.
    BelowWindow,
}

/// Eş başına alıcı dedup penceresi: `highest − (MSG_DEDUP_WINDOW − 1) ..= highest`.
struct DedupWindow {
    /// Görülen en büyük `msg_id`; `None` = eşten henüz TEXT gelmedi.
    highest: Option<MsgId>,
    /// Pencere içindeki görülmüşler; bit konumu `msg_id % MSG_DEDUP_WINDOW` (pencerenin
    /// `MSG_DEDUP_WINDOW` ardışık `msg_id`'si farklı konumlara düşer).
    seen: [u64; DEDUP_WORDS],
}

impl DedupWindow {
    const fn new() -> Self {
        Self {
            highest: None,
            seen: [0; DEDUP_WORDS],
        }
    }

    /// `msg_id`'nin konumu: (kelime, bit maskesi).
    fn slot(msg_id: MsgId) -> (usize, u64) {
        let bit = msg_id % MSG_DEDUP_WINDOW;
        ((bit / u64::BITS) as usize, 1u64 << (bit % u64::BITS))
    }

    fn mark(&mut self, msg_id: MsgId) {
        let (word, mask) = Self::slot(msg_id);
        self.seen[word] |= mask;
    }

    fn clear(&mut self, msg_id: MsgId) {
        let (word, mask) = Self::slot(msg_id);
        self.seen[word] &= !mask;
    }

    fn is_marked(&self, msg_id: MsgId) -> bool {
        let (word, mask) = Self::slot(msg_id);
        self.seen[word] & mask != 0
    }

    /// `msg_id`'yi pencereye karşı sınar; yeniyse işaretler. Sıra zorlanmaz: pencere içindeki
    /// görülmemiş, en büyükten küçük bir `msg_id` de [`Admission::New`]'dur. Aritmetik taşmaz:
    /// çıkarmalar yalnız büyükten küçüğe yapılır.
    fn admit(&mut self, msg_id: MsgId) -> Admission {
        let Some(highest) = self.highest else {
            self.highest = Some(msg_id);
            self.mark(msg_id);
            return Admission::New;
        };
        if msg_id > highest {
            let advance = msg_id - highest;
            if advance >= MSG_DEDUP_WINDOW {
                self.seen = [0; DEDUP_WORDS];
            } else {
                // `highest + 1 ..= msg_id` konumları pencereden düşen eski `msg_id`'lerindi.
                for step in 1..=advance {
                    self.clear(highest + step);
                }
            }
            self.highest = Some(msg_id);
            self.mark(msg_id);
            return Admission::New;
        }
        if highest - msg_id >= MSG_DEDUP_WINDOW {
            return Admission::BelowWindow;
        }
        if self.is_marked(msg_id) {
            return Admission::Duplicate;
        }
        self.mark(msg_id);
        Admission::New
    }
}

#[cfg(test)]
mod tests {
    //! Pencere sınırları, sayaç tükenmesi ve tablo tavanları — uçtan uca akış
    //! `tests/text_receipt.rs`'te.

    use super::*;
    use kama_contracts::s1::{CompressionPolicy, ConnectTicket, TransportCapabilities};

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;

    /// Hiçbir şey teslim etmeyen, her gönderimi kabul eden Transport (tablo tavanları için).
    struct NullTransport;

    impl KamaTransport for NullTransport {
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
            Ok(ConnectResult::Pending(ConnectTicket::from_raw(0)))
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

    fn handle(n: usize) -> SessionHandle {
        SessionHandle::from_raw(u64::try_from(n).expect("test sayısı u64'e sığar"))
    }

    /// `n`'den türetilmiş sentetik eş (ilk sekiz bayt `n`, BE).
    fn peer(n: usize) -> PeerRef {
        let mut dest_hash = [0u8; 32];
        let index = u64::try_from(n).expect("test sayısı u64'e sığar");
        dest_hash[..8].copy_from_slice(&index.to_be_bytes());
        PeerRef::from_dest_hash(dest_hash)
    }

    fn unit() -> PreparedUnit {
        PreparedUnit {
            bytes: Vec::new(),
            compression_policy: CompressionPolicy::Never,
        }
    }

    #[test]
    fn dedup_accepts_out_of_order_and_suppresses_repeats() {
        let mut window = DedupWindow::new();
        assert_eq!(window.admit(5), Admission::New);
        assert_eq!(window.admit(3), Admission::New, "sıra zorlanmaz");
        assert_eq!(window.admit(4), Admission::New);
        for repeat in [5, 3, 4] {
            assert_eq!(window.admit(repeat), Admission::Duplicate, "{repeat}");
        }
    }

    #[test]
    fn dedup_window_edges_are_exact() {
        let mut window = DedupWindow::new();
        let top: MsgId = 10_000;
        assert_eq!(window.admit(top), Admission::New);
        let oldest_inside = top - (MSG_DEDUP_WINDOW - 1);
        assert_eq!(window.admit(oldest_inside), Admission::New);
        assert_eq!(window.admit(oldest_inside), Admission::Duplicate);
        assert_eq!(window.admit(oldest_inside - 1), Admission::BelowWindow);
        // Pencere bir ilerleyince en eski üye düşer (konumunu yeni en büyük alır).
        assert_eq!(window.admit(top + 1), Admission::New);
        assert_eq!(window.admit(oldest_inside), Admission::BelowWindow);
    }

    #[test]
    fn dedup_advance_clears_reused_slots() {
        // Kısmi ilerleme: pencerede kalan üyeler korunur, düşen konumlar temizlenir.
        let mut window = DedupWindow::new();
        assert_eq!(window.admit(5), Admission::New);
        assert_eq!(window.admit(6), Admission::New);
        let top = 5 + MSG_DEDUP_WINDOW - 1;
        assert_eq!(window.admit(top), Admission::New);
        assert_eq!(window.admit(5), Admission::Duplicate);
        assert_eq!(window.admit(6), Admission::Duplicate);
        assert_eq!(window.admit(top + 1), Admission::New);
        assert_eq!(window.admit(5), Admission::BelowWindow);
        assert_eq!(window.admit(6), Admission::Duplicate);

        // Pencereden büyük sıçrama: hiçbir eski iz kalmaz. `5 + W` ile 5 aynı konumdadır ama
        // `5 + W` hiç görülmedi.
        let mut window = DedupWindow::new();
        assert_eq!(window.admit(5), Admission::New);
        assert_eq!(window.admit(5 + MSG_DEDUP_WINDOW + 10), Admission::New);
        assert_eq!(window.admit(5 + MSG_DEDUP_WINDOW), Admission::New);
    }

    #[test]
    fn dedup_does_not_overflow_at_the_ends_of_u32() {
        let mut window = DedupWindow::new();
        assert_eq!(window.admit(0), Admission::New);
        assert_eq!(window.admit(MsgId::MAX), Admission::New);
        assert_eq!(window.admit(MsgId::MAX), Admission::Duplicate);
        assert_eq!(
            window.admit(MsgId::MAX - (MSG_DEDUP_WINDOW - 1)),
            Admission::New
        );
        assert_eq!(
            window.admit(MsgId::MAX - MSG_DEDUP_WINDOW),
            Admission::BelowWindow
        );
        assert_eq!(window.admit(0), Admission::BelowWindow);
    }

    #[test]
    fn msg_id_exhaustion_is_typed_and_never_wraps() {
        let mut state = PeerState::new();
        assert_eq!(state.next_send_id(), Ok(FIRST_MSG_ID));
        state.next_msg_id = Some(MsgId::MAX);
        let last = state.next_send_id().expect("son msg_id kullanılabilir");
        assert_eq!(last, MsgId::MAX);
        state.commit_send(last, unit(), handle(1));
        assert_eq!(state.next_send_id(), Err(MessengerError::MsgIdExhausted));
        // RECEIPT outbox'u boşaltsa da sarma yok.
        state.outbox.clear();
        assert_eq!(state.next_send_id(), Err(MessengerError::MsgIdExhausted));
    }

    #[test]
    fn unacked_span_stays_inside_the_dedup_window() {
        let mut state = PeerState::new();
        let oldest = state.next_send_id().expect("ilk gönderim");
        state.commit_send(oldest, unit(), handle(1));
        state.next_msg_id = Some(oldest + MSG_DEDUP_WINDOW - 1);
        assert_eq!(state.next_send_id(), Ok(oldest + MSG_DEDUP_WINDOW - 1));
        state.next_msg_id = Some(oldest + MSG_DEDUP_WINDOW);
        assert_eq!(
            state.next_send_id(),
            Err(MessengerError::UnackedSpanFull { oldest })
        );
    }

    #[test]
    fn peer_table_is_bounded_and_never_evicts() {
        let mut messenger = Messenger::new(NullTransport).expect("limitler geçerli");
        for n in 0..MAX_PEERS {
            messenger.adopt(handle(n), peer(n));
        }
        let extra = peer(MAX_PEERS);
        messenger.adopt(handle(MAX_PEERS), extra);
        assert_eq!(messenger.stats().peers_refused, 1);
        assert_eq!(messenger.session_of(extra), None);
        assert_eq!(messenger.peer_of(handle(MAX_PEERS)), None);
        assert_eq!(messenger.session_of(peer(0)), Some(handle(0)));
        assert_eq!(messenger.connect(extra), Err(MessengerError::PeerTableFull));
        assert_eq!(
            messenger.send_text(extra, "izlenmiyor"),
            Err(MessengerError::PeerTableFull)
        );
        assert_eq!(
            messenger.connect(peer(0)),
            Ok(ConnectResult::Pending(ConnectTicket::from_raw(0)))
        );
    }

    #[test]
    fn retired_handles_are_a_bounded_fifo() {
        let mut messenger = Messenger::new(NullTransport).expect("limitler geçerli");
        let p = peer(0);
        // Her yeni tutamaç öncekini emekliye ayırır: 0..MAX_RETIRED_SESSIONS emekli, sonuncusu
        // güncel.
        for n in 0..=MAX_RETIRED_SESSIONS {
            messenger.adopt(handle(n), p);
        }
        assert_eq!(messenger.peer_of(handle(0)), Some(p));
        messenger.adopt(handle(MAX_RETIRED_SESSIONS + 1), p);
        assert_eq!(messenger.peer_of(handle(0)), None, "en eski eşleme düşmeli");
        assert_eq!(messenger.peer_of(handle(1)), Some(p));
        // `Closed(güncel)`: eşin oturumu kalmaz, tutamaç geç teslimler için eşine bağlı kalır.
        let current = handle(MAX_RETIRED_SESSIONS + 1);
        messenger.retire(current);
        assert_eq!(messenger.session_of(p), None);
        assert_eq!(messenger.peer_of(current), Some(p));
        assert_eq!(
            messenger.send_text(p, "kapalı"),
            Err(MessengerError::NoSession)
        );
    }

    #[test]
    fn debug_redacts_text() {
        let secret = "gizli-metin";
        let event = MessengerEvent::TextReceived {
            peer: peer(0),
            msg_id: 1,
            text: secret.to_owned(),
        };
        let shown = format!("{event:?}");
        assert!(!shown.contains(secret), "Debug metni sızdırdı: {shown}");
    }
}
