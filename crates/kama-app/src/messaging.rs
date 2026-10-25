//! Metin + alındı (B2.2) — AMP `TEXT` (0x01) ve `RECEIPT` (0x08) akışı
//! (`specs/application-message-protocol.md` §3; S2 yaması 2026-10-08, K4G-07 yaması 2026-10-12).
//!
//! [`Messenger`] S1 [`KamaTransport`]'u sahiplenir ve onun üstünde iki yarıyı yürütür:
//!
//! * **Gönderen** — [`Messenger::send_text`] eşin sıradaki `msg_id`'sini kalıcı depodan ayırır
//!   ([`AppStore::reserve_next`]), TEXT kaydını codec'le kurar ([`codec::encode`]), kaydı kalıcı
//!   outbox'a yazar ve eşin güncel oturumuna `DeliveryPolicy::Reliable` ile verir. Kayıt, eşin
//!   `RECEIPT(delivered)`'ı gelene kadar outbox'ta tutulur.
//! * **Alıcı** — [`Messenger::poll`] önce Transport olaylarını, sonra teslim edilen birimleri okur.
//!   Dedup penceresinde ilk kez görülen TEXT [`MessengerEvent::TextReceived`] üretir; HER TEXT'e
//!   (tekrar dahil) `RECEIPT(delivered)` borcu doğar; aynı `msg_id` için bekleyen RECEIPT varsa
//!   tekrar onunla birleşir ("RECEIPT"). RECEIPT outbox'taki kaydı kapatır ve `msg_id` başına tam
//!   bir kez [`MessengerEvent::Delivered`] üretir.
//!
//! # `msg_id` eş başınadır (S2, Abkarada 2026-10-08) ve kalıcıdır (K4G-07, plan D4 v1.1)
//!
//! Gönderen sayacı ve alıcı dedup'ı `PeerRef` anahtarlıdır, `SessionHandle` değil: supersession
//! (`Ready(h')` → `Closed(h, Superseded)`) de kapanış + yeniden bağlanma da ikisini sıfırlamaz.
//! Sayaç motorda DEĞİL, depodadır: her `msg_id` `AppStore::reserve_next(eş, CounterKind::MsgId)`
//! ile ayrılır ve depo değeri döndürmeden ÖNCE kalıcılaştırır; aynı depoyla yeniden kurulan motor
//! sayaca kaldığı yerden devam eder. Ayrılan değer YALNIZ kayıt Transport'a kabul ettirildiğinde
//! (`send` / `connect` → `Ok`) tüketilir. Ayırmadan sonraki bir adım hata verirse (her `Err` kaydın
//! kuyruğa alınmadığı anlamına gelir, S1 §3) değer eşin yedeğinde kalır ve sıradaki gönderim onu
//! kullanır: yerel hata `msg_id` yakmaz. Böylece alıcıdaki `msg_id` boşluğu yolda kaybolmuş bir
//! TEXT'i (identity T-16 tespit sinyali) ya da süreç ölürken yedekte kalmış, tele hiç çıkmamış bir
//! değeri gösterir. Depo `u32` alanının tükendiğini kalıcı olarak bildirir →
//! [`MessengerError::MsgIdExhausted`]; sarma YOK.
//!
//! # Kalıcı outbox (K4G-07)
//!
//! Outbox'ın kalıcı kopyası depodadır ([`AppStore::outbox_put`] / [`AppStore::outbox_remove`] /
//! [`AppStore::outbox_list`]). Motor bellekte onun aynasını ve kaydın en son hangi tutamaca kabul
//! ettirildiğini (kalıcı DEĞİL: tutamaç süreç ömrüyle sınırlıdır) tutar. Yazma ÖNCE gelir: kayıt
//! Transport'a verilmeden önce depoya yazılır, Transport reddederse kalıcı kopya geri alınır. Böylece
//! Transport'un kabul ettiği her kayıt depodadır; süreç hangi anda ölürse ölsün RECEIPT'siz kayıt
//! kaybolmaz, en kötü durumda bir kez fazladan gider ve alıcının dedup'ında söner. RECEIPT kaydı
//! bellekten ve depodan kapatır.
//!
//! Yeniden başlatma: motor kurulurken depoya dokunmaz. Bir eşin kalıcı outbox'ı, eş motorda İLK
//! göründüğünde ([`Messenger::connect`], [`Messenger::connect_with_text`] ya da eşin `Ready` olayı)
//! yüklenir ve kayıtları hiçbir tutamaca gönderilmemiş sayılır: eşin sıradaki oturumunda, bugünkü
//! oturum-değişimi yolundan ESKİ `msg_id` (aynı bayt) ile gönderilirler ("Retry yok" bölümü).
//! Yükleme başarısızsa eş motorda kurulmaz: çağrı yolunda [`MessengerError::Store`] döner; olay
//! yolunda eşin oturumu benimsenmez (o tutamacın birimleri atılır) ve
//! [`MessengerStats::store_errors`] artar; uygulamanın sonraki `connect`'i yüklemeyi yeniden dener.
//!
//! Depo hatası sessiz yutulmaz: `msg_id` ayrılamazsa ya da kayıt depoya yazılamazsa kayıt
//! Transport'a VERİLMEZ, outbox'a girmez (depo belirsizliği "Beyanlı sınırlar"da) ve çağırana
//! [`MessengerError::Store`] döner (depo tükenmesi [`MessengerError::MsgIdExhausted`], depo outbox
//! tavanı [`MessengerError::OutboxFull`] olarak). Çağırana dönemeyen depo hataları (RECEIPT'in
//! kalıcı kapanışı, geri alma, olayla gelen eşin yüklemesi) [`MessengerStats::store_errors`]'ta
//! sayılır. Kalıcı kapanışı başarısız olmuş bir kaydın RECEIPT'i yinelenirse (ör. yeni oturumdaki
//! yeniden gönderimin RECEIPT'i) silme idempotent olarak yeniden denenir.
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
//! penceresinin İÇİNDE kalır ve dedup kararı kesindir. Pencere-altı durumu yalnız kalıcı durumunu
//! yitirmiş bir göndericide doğar ("Beyanlı sınırlar", D4).
//!
//! # RECEIPT
//!
//! `delivered` = alıcı Application kaydı işledi (AMP §3, A4). Application üretir; Transport'un
//! `Ok(())`'undan ya da başka bir Transport sinyalinden TÜREMEZ (S1 K8). Eşin canlılığının tek
//! sinyali [`MessengerEvent::Delivered`]'dır; `ConnectProgress` hub yolunun durumudur (S1 §4).
//! RECEIPT eşin güncel oturumundan gider. Eşin bekleyen kuyruğu boşsa hemen sunulur. `Backpressure`
//! alırsa ya da önünde bekleyen varsa eş başına sınırlı kuyrukta bekler (`(eş, msg_id)` tekil,
//! ≤ [`MAX_OUTBOX_UNACKED`]). Kuyruk her [`Messenger::poll`] başında baştan, ilk `Backpressure`'a
//! dek AYNI oturuma sunulur (K8 "bekler": reddedilen birim tele çıkmamıştır; zamanlayıcı yok).
//! Kabulden sonrası Transport'undur. Oturum yoksa, ret başka bir hataysa ya da eşin oturumu değişir
//! veya kapanırsa RECEIPT bırakılır: gönderen kaydı bir sonraki oturumda yeniden gönderir, tekrar
//! TEXT RECEIPT'i yeniden üretir. Kuyruk taşarsa en eski kayıt düşer.
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
//! Reddedilen RECEIPT de aynı kuralla bekler ("RECEIPT").
//!
//! # Bağlantı (B2.5)
//!
//! * [`Messenger::connect_state`] — UI için eşin bağlantı durumu: hazır oturum, bekleyen niyetin
//!   SON `ConnectProgress` fazı ya da boşta. Faz HUB YOLUNUN durumudur; eşin canlılığı yalnız
//!   [`MessengerEvent::Delivered`] ile bilinir (S1 §4).
//! * [`Messenger::cancel_connect`] — bekleyen niyetten TEK çıkış (`KamaTransport::cancel`); sonucu
//!   `ConnectAbandoned(ticket, Cancelled)` olayıyla gelir.
//! * [`Messenger::connect_with_text`] — ilk metin `early_unit` olarak (S1 K6): kayıt Transport'ta
//!   durur ve motor onu KENDİSİ yollamaz; kalıcı kopyası Transport'a verilmeden ÖNCE depoya yazılır
//!   (K4G-07). Erken metin onu taşıyan niyetin BİLETİNE bağlıdır: yalnız o niyet bir oturumla
//!   çözülürse (`Ready(h, eş)`) kayıt "h'ye gönderilmiş" olarak outbox'a girer (Transport onu `h`'de
//!   ilk normal `send` olarak yollar; glare kaybında kazanan oturumda) — sonrası her TEXT gibidir
//!   (RECEIPT, oturum değişiminde yeniden gönderim). Niyet biterse birim teslim edilmemiştir (S1 §4)
//!   ve kayıt outbox'a "hiçbir tutamaca gönderilmemiş" olarak girer: kendi biletiyle gelen
//!   `ConnectAbandoned`'da ya da eşe başka biletli bir niyet açıldığında (ör. iptalden sonra, olay
//!   okunmadan yeni `connect` / `connect_with_text`; bir eşin en çok bir bekleyen niyeti vardır).
//!   Kayıt eşin sıradaki oturumunda AYNI `msg_id`'yle yollanır (yeni oturumu uygulama
//!   [`Messenger::connect`] ile ister; motor kendiliğinden bağlanmaz).
//!
//! # Beyanlı sınırlar (K4G-07)
//!
//! * **D4 (PROVISIONING-FORMAT-v0 §1; PoC-KAPSAM-001 §5 EK-1) — ÇÖZÜLMEDİ, sınırdır.** Kalıcı
//!   durum tamamen kaybolur (veri silme, yeniden kurulum; burada: boş depo) ve aynı eski kimlik
//!   yeniden kullanılırsa `msg_id`'ler baştan başlar; eşin hâlâ tuttuğu dedup penceresinde görülmüş
//!   sayılan TEXT'ler teslim edilmez ama RECEIPT alır (yanlış "teslim edildi"). Yerel tespit
//!   imkânsızdır (geçmişi bilen kaynak yok); bu durumda yeniden provisioning zorunludur (yeni kimlik
//!   → alıcıda yeni eş).
//! * **Alıcı dedup penceresi kalıcı DEĞİLDİR** (bu kartta depoya bağlanmadı): yeniden başlayan
//!   alıcı, gönderenin oturum değişiminde yeniden gönderdiği ve daha önce teslim ettiği bir TEXT'i
//!   bir kez daha teslim edebilir (yinelenen gösterim; yanlış "teslim edildi" DEĞİL).
//! * **Depo hatasının belirsizliği:** depo sözleşmesi gereği `Err` dönen bir yazma yine de kalıcı
//!   olmuş olabilir. Geri alma da başarısızsa kayıt depoda kalır: süreç yaşadıkça aynı `msg_id`'yle
//!   gönderilen sıradaki kayıt onun yerine geçer; süreç ölürse yeniden başlatmadan sonra çağırana
//!   hata dönmüş kayıt gönderilir.
//!
//! # Gizlilik
//!
//! [`MessengerEvent`]'in `Debug` çıktısı metni değil uzunluğunu yazar; outbox kayıtları hiçbir
//! `Debug` çıktısına girmez ([`Messenger`] `Debug` değildir).

use core::fmt;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    ConnectPhase, ConnectResult, ConnectTicket, Delivered, DeliveryPolicy, KamaTransport,
    PreparedUnit, SessionEvent, TransportError,
};

use crate::codec::{self, AmpLimits, AppMessage, EncodeError, LimitsError, ReceiptKind};
#[cfg(test)]
use crate::store::MemoryAppStore;
use crate::store::{AppStore, AppStoreError, CounterKind, OutboxUnit};

/// `TEXT.msg_id` ve `RECEIPT.ref_msg_id` (AMP §3; telde `u32_be`).
pub type MsgId = u32;

/// Taze depoda bir eşe atanan ilk `msg_id`. Spec başlangıç değeri koymaz; değer bu uygulamanındır
/// ve depo onu `store::FIRST_COUNTER_VALUE` olarak verir (eşitlik `store`'da derleme zamanında
/// denetlenir).
pub const FIRST_MSG_ID: MsgId = 0;

/// S7 (Abkarada 2026-10-08) — alıcının eş başına dedup penceresi (`msg_id` sayısı).
pub const MSG_DEDUP_WINDOW: MsgId = 4096;

/// S7 (Abkarada 2026-10-08) — eş başına Transport'un kabul ettiği ama `RECEIPT(delivered)`'ı
/// gelmemiş TEXT tavanı.
pub const MAX_OUTBOX_UNACKED: usize = 256;

/// Eş tablosu tavanı — yerel bellek sınırı, normatif DEĞİL `[KALİBRASYON]`. Eş durumu oturumlardan
/// sağ çıkmak zorunda olduğu için tablodan eş ÇIKARILMAZ: sayaç ve outbox depoda kalsa da
/// çıkarılan eşin dedup penceresi (kalıcı değil) kaybolurdu. Tavan doluyken YENİ eş reddedilir.
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
    /// bu olay üretilirken Transport'a sunuldu, (geri basınçta) bekleyen kuyruğa alındı ya da
    /// (oturum yok / kalıcı ret) bırakıldı (`receipts_unsent`).
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

/// [B2.5] Eşin bağlantı durumu — UI içindir; eşin canlılığını BİLDİRMEZ (S1 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectState {
    /// Hazır oturum da bekleyen niyet de yok.
    Idle,
    /// Bekleyen niyet ve onun son `ConnectProgress` fazı.
    Connecting {
        /// Niyetin bileti ([`Messenger::cancel_connect`] onu iptal eder).
        ticket: ConnectTicket,
        /// Son okunan faz (`connect` dönüşünde `Measuring`).
        phase: ConnectPhase,
    },
    /// Eşin güncel oturumu.
    Ready(SessionHandle),
}

/// [`Messenger::send_text`] ve [`Messenger::connect`] hatası. Her hata yolunda kayıt Transport'a
/// verilmez (Transport'un kendi reddi hariç) ve outbox'a girmez — bellekte hiçbir zaman; depoda da,
/// tek beyanlı istisnayla: depo `Err` döndürdüğü hâlde yazmayı kalıcılaştırmış ve geri alma da
/// başarısız olmuşsa kayıt depoda kalır (modül belgesi, "Beyanlı sınırlar"). `msg_id` tüketilmez —
/// depodan ayrılmış ama kullanılmamış değer eşin sıradaki gönderimine kalır. Motor hiçbirini yeniden
/// denemez.
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
    /// Kalıcı depo ([`AppStore`]) `msg_id` ayıramadı, eşin outbox'ını okuyamadı ya da kaydı
    /// outbox'a yazamadı: kayıt Transport'a VERİLMEDİ ve outbox'a girmedi (depo belirsizliği
    /// istisnası tür belgesinde; modül belgesi, "Kalıcı outbox"). Depo tükenmesi
    /// [`MessengerError::MsgIdExhausted`], depo outbox tavanı [`MessengerError::OutboxFull`] olarak
    /// döner, burada değil.
    Store(AppStoreError),
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
            Self::Store(e) => write!(f, "app store failed, nothing was sent: {e}"),
        }
    }
}

impl std::error::Error for MessengerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::Store(e) => Some(e),
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
    /// Dedup penceresinde görülmüş `msg_id`'li TEXT'ler (teslim yok; RECEIPT yeniden ya da
    /// bekleyenle birleşir).
    pub duplicate_texts: u64,
    /// Pencerenin altında kalan `msg_id`'li TEXT'ler (fail-closed tekrar; teslim yok; RECEIPT
    /// yeniden ya da bekleyenle birleşir).
    pub below_window_texts: u64,
    /// Transport'un kabul ettiği RECEIPT sunumları (ilk sunum ya da bekleyen kuyruktan).
    pub receipts_sent: u64,
    /// Transport'a kabul ettirilmeden bırakılan RECEIPT'ler: oturum yokken üretilen, `Backpressure`
    /// dışı bir hatayla reddedilen, eşin oturumu değişince ya da kapanınca kuyruktan düşen, taşmada
    /// düşen. Telafisi gönderenin oturum değişimindeki yeniden gönderimidir (S2).
    pub receipts_unsent: u64,
    /// Bekleyen kuyrukta kalan yeni RECEIPT (ilk sunumu `Backpressure` aldı ya da önünde bekleyen
    /// vardı; birleşen tekrar sayılmaz).
    pub receipts_deferred: u64,
    /// ANLIK değer, sayaç değil: `stats()` anında bütün eşlerin bekleyen RECEIPT sayısı.
    pub receipts_pending: u64,
    /// Outbox'ta karşılığı olmayan `RECEIPT(delivered)`'lar (tekrar ya da bilinmeyen `msg_id`).
    pub receipts_ignored: u64,
    /// Oturum değişiminde Transport'un kabul ettiği yeniden gönderimler (S1 K7).
    pub resent_on_new_session: u64,
    /// Transport'un reddettiği oturum-değişimi yeniden gönderim denemeleri (sonraki `poll`'da
    /// beklemeye devam eder).
    pub resend_refused: u64,
    /// Tablo doluyken `Ready`'si gelen yeni eşler (izlenmedi; birimleri atılır).
    pub peers_refused: u64,
    /// Çağırana dönemeyen depo hataları: RECEIPT'in kalıcı kapanışı, Transport'a verilmeyen
    /// kaydın kalıcı kopyasının geri alınması, olayla gelen yeni eşin outbox yüklemesi (modül
    /// belgesi, "Kalıcı outbox"). Çağırana dönen depo hatası [`MessengerError::Store`]'dur ve
    /// burada sayılmaz.
    pub store_errors: u64,
}

/// B2.2 metin motoru. Ayrıntı: modül belgesi.
pub struct Messenger<T> {
    transport: T,
    /// Kalıcı durum: eş başına `msg_id` sayacı ve outbox'ın kalıcı kopyası (K4G-07). Alıcı dedup
    /// penceresi depoya bağlı DEĞİLDİR (modül belgesi, "Beyanlı sınırlar").
    store: Arc<dyn AppStore>,
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
    /// `store` uygulamanın kalıcı durumudur (K4G-07): `msg_id`'ler YALNIZ ondan ayrılır, outbox ona
    /// yazılır; `Ptt` de `stream_id`'lerini bu depodan ayırır (K4G-08, `Messenger::store`
    /// üzerinden). Aynı depoyla yeniden kurulan motor sayaçlara kaldığı yerden devam eder ve bir eşin
    /// kalıcı outbox'ını eş motorda ilk göründüğünde yükler (modül belgesi, "Kalıcı outbox"). Ürün
    /// yolu kalıcı depoyu verir (`kama-store::RedbStore`, bileşim kökü); yeni boş bir depo kalıcı
    /// durumu kaybolmuş bir cihazdır (D4 beyanlı sınırı). Kurulum depoya dokunmaz.
    ///
    /// # Errors
    ///
    /// [`LimitsError`] — Transport'un beyan ettiği birim AMP için çok küçük.
    pub fn with_store(transport: T, store: Arc<dyn AppStore>) -> Result<Self, LimitsError> {
        let limits = AmpLimits::from_capabilities(&transport.capabilities())?;
        Ok(Self {
            transport,
            store,
            limits,
            peers: BTreeMap::new(),
            by_handle: BTreeMap::new(),
            retired: VecDeque::new(),
            stats: MessengerStats::default(),
        })
    }

    /// YALNIZ bu crate'in birim testleri (`messaging`, `file_transfer`): taze bellek içi depoyla
    /// kurar — kalıcı durumu olmayan cihaz. Kalıcılık isteyen her yol (K4G-08'den beri `ptt` birim
    /// testleri de) [`Messenger::with_store`]'u kullanır; ürün kodunda kalıcı olmayan yapıcı YOKTUR.
    ///
    /// # Errors
    ///
    /// [`Messenger::with_store`] ile aynı.
    #[cfg(test)]
    pub(crate) fn new(transport: T) -> Result<Self, LimitsError> {
        Self::with_store(transport, Arc::new(MemoryAppStore::new()))
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

    /// Kalıcı depo — YALNIZ bu crate'in motorları için: `Ptt` `stream_id`'yi buradan ayırır
    /// (K4G-08). Application'ın tek deposudur; bileşim kökü onu bir kez bağlar.
    pub(crate) fn store(&self) -> &dyn AppStore {
        &*self.store
    }

    /// Gözlem sayaçları.
    #[must_use]
    pub fn stats(&self) -> MessengerStats {
        // `receipts_pending` sayaç değil, anlık değerdir: motor onu tutmaz (iç kopyada hep 0),
        // burada eşlerin bekleyen kuyruklarından hesaplanır (≤ `MAX_PEERS` × `MAX_OUTBOX_UNACKED`).
        let pending: usize = self
            .peers
            .values()
            .map(|state| state.pending_receipts.len())
            .sum();
        MessengerStats {
            receipts_pending: u64::try_from(pending).unwrap_or(u64::MAX),
            ..self.stats
        }
    }

    /// Eşin güncel oturum tutamacı (gönderimin gideceği); yoksa `None`.
    #[must_use]
    pub fn session_of(&self, peer: PeerRef) -> Option<SessionHandle> {
        self.peers.get(&peer).and_then(|state| state.session)
    }

    /// Eşin outbox'ındaki, `RECEIPT(delivered)`'ı henüz gelmemiş TEXT sayısı. Motorun tanıdığı
    /// eşler içindir: yeniden başlatmadan sonra eşin kalıcı outbox'ı eş motorda ilk göründüğünde
    /// yüklenir (modül belgesi, "Kalıcı outbox"); o ana dek 0'dır.
    #[must_use]
    pub fn unacked_count(&self, peer: PeerRef) -> usize {
        self.peers.get(&peer).map_or(0, |state| state.outbox.len())
    }

    /// Eşin bekleyen `RECEIPT(delivered)` sayısı: Transport'a henüz kabul ettirilemeyen ve sıradaki
    /// [`Messenger::poll`] başında eşin güncel oturumuna sunulacak borçlar (≤
    /// [`MAX_OUTBOX_UNACKED`]; modül belgesi, "RECEIPT"). Motorun tanımadığı eş için 0; bütün
    /// eşlerin toplamı [`MessengerStats::receipts_pending`]'tir. Sayaç değişmezi (K4G-05d):
    /// `receipts_deferred` = kuyruktan kabul edilen + kuyruktan düşen + `receipts_pending`.
    #[must_use]
    pub fn pending_receipts(&self, peer: PeerRef) -> usize {
        self.peers
            .get(&peer)
            .map_or(0, |state| state.pending_receipts.len())
    }

    /// `KamaTransport::connect(peer, None)` (S1 K2, KARAR-1 idempotent). Aktif oturumun tutamacı
    /// `ConnectResult::Ready(h)` ile olay YAYINLANMADAN dönebildiği için burada benimsenir;
    /// `Pending` [`Messenger::poll`]'da okunan `Ready` ile çözülür. `early_unit` B2.5'indir; burada
    /// verilmez. Motor eşi henüz tanımıyorsa önce eşin kalıcı outbox'ı yüklenir (modül belgesi,
    /// "Kalıcı outbox").
    ///
    /// # Errors
    ///
    /// [`MessengerError::PeerTableFull`] · [`MessengerError::Store`] (eşin outbox'ı yüklenemedi) —
    /// ikisinde de Transport çağrılmaz · [`MessengerError::Transport`].
    pub fn connect(&mut self, peer: PeerRef) -> Result<ConnectResult, MessengerError> {
        materialize(&mut self.peers, &*self.store, peer)?;
        let result = self
            .transport
            .connect(peer, None)
            .map_err(MessengerError::Transport)?;
        match result {
            ConnectResult::Ready(handle) => self.adopt(handle, peer),
            ConnectResult::Pending(ticket) => self.note_pending(peer, ticket),
        }
        Ok(result)
    }

    /// [B2.5] İlk metni `early_unit` olarak taşıyan `connect` (S1 K6; modül belgesi, "Bağlantı").
    /// Dönen `msg_id` metnin kimliğidir: teslimi [`MessengerEvent::Delivered`] ile öğrenilir.
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`MessengerError::PeerTableFull`] · [`MessengerError::Store`] (eşin
    /// outbox'ı yüklenemedi) · [`MessengerError::OutboxFull`] · [`MessengerError::MsgIdExhausted`]
    /// ya da [`MessengerError::Store`] (`msg_id` ayrılamadı) · [`MessengerError::UnackedSpanFull`] ·
    /// [`MessengerError::Encode`] · [`MessengerError::OutboxFull`] ya da [`MessengerError::Store`]
    /// (kayıt depoya yazılamadı) · [`MessengerError::Transport`] (aktif ya da bekleyen eşe
    /// `EarlyUnitConflict`, tavan `UnitTooLarge`, `Backpressure`). Hiçbiri `msg_id` tüketmez ve
    /// hiçbirinde kayıt outbox'a girmez (depo belirsizliği istisnası: [`MessengerError`]).
    pub fn connect_with_text(
        &mut self,
        peer: PeerRef,
        text: &str,
    ) -> Result<(ConnectResult, MsgId), MessengerError> {
        let state = materialize(&mut self.peers, &*self.store, peer)?;
        let msg_id = state.next_send_id(&*self.store, peer)?;
        let message = AppMessage::Text {
            msg_id,
            text: text.to_owned(),
        };
        let unit = codec::encode(&message, &self.limits).map_err(MessengerError::Encode)?;
        persist_unsent(&*self.store, &mut self.stats, peer, msg_id, &unit)?;
        let result = match self.transport.connect(peer, Some(unit.clone())) {
            Ok(result) => result,
            Err(error) => {
                forget_unsent(&*self.store, &mut self.stats, peer, msg_id);
                return Err(MessengerError::Transport(error));
            }
        };
        // Kayıt Transport'ta (K6): ayrılan `msg_id` tüketildi.
        state.spare_msg_id = None;
        match result {
            ConnectResult::Pending(ticket) => {
                // Transport bu erken metni yeni bir niyetle kabul etti (K6: bekleyen eşe
                // `EarlyUnitConflict`): önceki erken metnin niyeti — olayı henüz okunmamış olsa da
                // (ör. iptal) — bitmiştir. O metin EZİLMEZ, outbox'a "gönderilmemiş" olarak geçer.
                state.release_early();
                state.early = Some(EarlyText {
                    ticket,
                    msg_id,
                    unit,
                });
                state.connecting = Some((ticket, ConnectPhase::Measuring));
            }
            ConnectResult::Ready(handle) => {
                // K6: aktif eşe `early_unit` `EarlyUnitConflict`'tir; buraya ancak sözleşme dışı bir
                // Transport'la gelinir. Kayıt kaybolmasın: outbox'a "gönderilmemiş" girer ve
                // benimsenen oturumdan normal gönderimle yollanır.
                state.outbox.insert(
                    msg_id,
                    OutboxEntry {
                        unit,
                        sent_on: None,
                    },
                );
                self.adopt(handle, peer);
                if let Some(state) = self.peers.get_mut(&peer) {
                    state.resend_pending = true;
                    flush_peer(&mut self.transport, state, &mut self.stats);
                }
            }
        }
        Ok((result, msg_id))
    }

    /// [B2.5] Eşin bekleyen niyetini iptal eder (`KamaTransport::cancel`). Bekleyen niyet yoksa
    /// `false` (olay yok). Sonuç `ConnectAbandoned(ticket, Cancelled)` olayıyla gelir; niyetin
    /// erken metni o olayla — olay okunmadan eşe başka biletli bir niyet açılırsa o anda — outbox'a
    /// "gönderilmemiş" olarak geçer.
    pub fn cancel_connect(&mut self, peer: PeerRef) -> bool {
        let Some((ticket, _)) = self.peers.get(&peer).and_then(|s| s.connecting) else {
            return false;
        };
        self.transport.cancel(ticket);
        true
    }

    /// [B2.5] Eşin bağlantı durumu (UI).
    #[must_use]
    pub fn connect_state(&self, peer: PeerRef) -> ConnectState {
        match self.peers.get(&peer) {
            Some(PeerState {
                session: Some(handle),
                ..
            }) => ConnectState::Ready(*handle),
            Some(PeerState {
                connecting: Some((ticket, phase)),
                ..
            }) => ConnectState::Connecting {
                ticket: *ticket,
                phase: *phase,
            },
            _ => ConnectState::Idle,
        }
    }

    /// `Pending(ticket)`: eşin bekleyen niyeti (faz olaylarla güncellenir). Eşin durumu
    /// [`Messenger::connect`]'te, Transport çağrılmadan önce kuruldu. Erken metin başka bir bilete
    /// aitse o niyet bitmiştir (bir eşin en çok bir bekleyen niyeti vardır, KARAR-1; ör. iptalin
    /// olayı henüz okunmadı): metin outbox'a "gönderilmemiş" olarak geçer.
    fn note_pending(&mut self, peer: PeerRef, ticket: ConnectTicket) {
        let Some(state) = self.peers.get_mut(&peer) else {
            return;
        };
        if state
            .early
            .as_ref()
            .is_some_and(|early| early.ticket != ticket)
        {
            state.release_early();
        }
        if state.connecting.map(|(t, _)| t) != Some(ticket) {
            state.connecting = Some((ticket, ConnectPhase::Measuring));
        }
    }

    /// TEXT gönderir ve atanan `msg_id`'yi döndürür. `Ok` = Transport kaydı kuyruğa aldı; teslim
    /// DEĞİL (S1 K8) — teslim [`MessengerEvent::Delivered`] ile öğrenilir. Kayıt sıkıştırılmaz
    /// (bu sprint `compression_id` = NONE).
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`MessengerError::PeerTableFull`] (izlenmeyen eş, tablo dolu) ya da
    /// [`MessengerError::NoSession`] · [`MessengerError::OutboxFull`] ·
    /// [`MessengerError::MsgIdExhausted`] ya da [`MessengerError::Store`] (`msg_id` ayrılamadı) ·
    /// [`MessengerError::UnackedSpanFull`] · [`MessengerError::Encode`] ·
    /// [`MessengerError::OutboxFull`] ya da [`MessengerError::Store`] (kayıt depoya yazılamadı) ·
    /// [`MessengerError::Transport`]. Hiçbiri `msg_id` tüketmez; hiçbirinde kayıt outbox'a girmez
    /// (depo belirsizliği istisnası: [`MessengerError`]) ya da Transport'a verilmez (Transport'un
    /// kendi reddi hariç).
    pub fn send_text(&mut self, peer: PeerRef, text: &str) -> Result<MsgId, MessengerError> {
        let Some(state) = self.peers.get_mut(&peer) else {
            return Err(if self.peers.len() >= MAX_PEERS {
                MessengerError::PeerTableFull
            } else {
                MessengerError::NoSession
            });
        };
        let handle = state.session.ok_or(MessengerError::NoSession)?;
        let msg_id = state.next_send_id(&*self.store, peer)?;
        let message = AppMessage::Text {
            msg_id,
            text: text.to_owned(),
        };
        let unit = codec::encode(&message, &self.limits).map_err(MessengerError::Encode)?;
        persist_unsent(&*self.store, &mut self.stats, peer, msg_id, &unit)?;
        if let Err(error) = self
            .transport
            .send(handle, unit.clone(), DeliveryPolicy::Reliable)
        {
            forget_unsent(&*self.store, &mut self.stats, peer, msg_id);
            return Err(MessengerError::Transport(error));
        }
        state.commit_send(msg_id, unit, handle);
        Ok(msg_id)
    }

    /// Bir olay döndürür; Transport'ta okunacak bir şey kalmadıysa `None`.
    ///
    /// Sıra: (1) Transport'un reddettiği oturum-değişimi yeniden gönderimleri yeniden sunulur;
    /// (2) bekleyen RECEIPT'ler eş başına baştan, ilk `Backpressure`'a dek sunulur; (3) Transport
    /// olayları, birer birer; (4) teslim edilen birimler. Olaylar birimlerden ÖNCE okunur: yeni
    /// oturumun ilk birimi `Ready(h')` okunmadan işlenseydi eşi çözülemezdi. Olay üretmeyen
    /// birimler (tekrar TEXT, karşılıksız RECEIPT, atılan kayıt) aynı çağrıda atlanır; her tur
    /// Transport'tan bir birim tüketir.
    #[must_use]
    pub fn poll(&mut self) -> Option<MessengerEvent> {
        self.flush_resends();
        self.flush_receipts();
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
            SessionEvent::ConnectProgress(ticket, phase) => {
                if let Some(state) = self.state_of_ticket(ticket) {
                    state.connecting = Some((ticket, phase));
                }
            }
            SessionEvent::ConnectAbandoned(ticket, _) => {
                // Niyet ve erken metin AYRI eşlenir: erken metin kendi biletine bağlıdır (modül
                // belgesi, "Bağlantı"). S1 §4: `early_unit` teslim edilmemiştir; eşin sıradaki
                // oturumunda AYNI `msg_id`'yle (aynı baytlar) yollanır.
                for state in self.peers.values_mut() {
                    if state.connecting.is_some_and(|(t, _)| t == ticket) {
                        state.connecting = None;
                    }
                    if state
                        .early
                        .as_ref()
                        .is_some_and(|early| early.ticket == ticket)
                    {
                        state.release_early();
                    }
                }
            }
            SessionEvent::RenewRequired(_) => {}
        }
    }

    /// Bekleyen niyetin bileti `ticket` olan eşin durumu.
    fn state_of_ticket(&mut self, ticket: ConnectTicket) -> Option<&mut PeerState> {
        self.peers
            .values_mut()
            .find(|s| s.connecting.is_some_and(|(t, _)| t == ticket))
    }

    /// `handle`'ı `peer`'in güncel tutamacı yapar (`Ready` olayı ya da `ConnectResult::Ready`).
    /// Önceki güncel tutamaç emekliye ayrılır; RECEIPT'siz kayıtlar yeni tutamaçtan yeniden
    /// gönderilir (S1 K7). Motorun tanımadığı eşin kalıcı outbox'ı burada yüklenir; yüklenemezse eş
    /// kurulmaz ve tutamaç benimsenmez (modül belgesi, "Kalıcı outbox").
    fn adopt(&mut self, handle: SessionHandle, peer: PeerRef) {
        let state = match materialize(&mut self.peers, &*self.store, peer) {
            Ok(state) => state,
            Err(MessengerError::PeerTableFull) => {
                self.stats.peers_refused += 1;
                return;
            }
            Err(_) => {
                self.stats.store_errors += 1;
                return;
            }
        };
        if state.session == Some(handle) {
            // Aynı tutamacın ikinci kez bildirilmesi (ör. `connect` dönüşü + `Ready` olayı).
            return;
        }
        // Eşe oturum geldi: bekleyen niyet çözüldü (kendi niyetimiz ya da glare'de eşinki).
        let resolved = state.connecting.take().map(|(ticket, _)| ticket);
        // K6: erken metin YALNIZ onu taşıyan niyet bu oturumla çözüldüyse Transport'tadır ve bu
        // oturumda ilk normal `send` olarak gider — o durumda burada YOLLANMAZ, yalnız RECEIPT ve
        // oturum değişimi için "h'ye gönderilmiş" olarak kaydedilir. Başka bir bilete aitse niyeti
        // bitmiştir: "gönderilmemiş" olarak girer ve aşağıda bu oturumdan gönderilir.
        if let Some(early) = state.early.take() {
            let sent_on = (resolved == Some(early.ticket)).then_some(handle);
            state.outbox.insert(
                early.msg_id,
                OutboxEntry {
                    unit: early.unit,
                    sent_on,
                },
            );
        }
        // K4G-05d (DUR-05d-4): bekleyen RECEIPT'ler eşin eski oturumundaydı ve bırakılır; gönderen
        // RECEIPT'siz kayıtları yeni oturumda yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir.
        drop_pending_receipts(state, &mut self.stats);
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
            // K4G-05d (DUR-05d-4): oturum kapandı; bekleyen RECEIPT'ler bırakılır.
            drop_pending_receipts(state, &mut self.stats);
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

    /// K4G-05d: bekleyen RECEIPT'i olan eşlerin kuyruğunu güncel tutamaca baştan sunar
    /// ([`flush_peer_receipts`]).
    fn flush_receipts(&mut self) {
        for state in self.peers.values_mut() {
            if !state.pending_receipts.is_empty() {
                flush_peer_receipts(&mut self.transport, state, &self.limits, &mut self.stats);
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
        owe_receipt(
            &mut self.transport,
            state,
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

    /// `RECEIPT(delivered)`: outbox'taki kaydı bellekten ve depodan kapatır — `msg_id` başına
    /// (motor ömründe) tam bir [`Delivered`]. Kalıcı kopya silinemezse teslim yine gerçektir
    /// (olay üretilir, `store_errors` artar); kayıt depoda kalır, yeniden başlatmadan sonra bir kez
    /// fazladan gider ve alıcının dedup'ında söner — yanlış "teslim edildi" doğmaz. Bellekte
    /// karşılığı olmayan (tekrar ya da bilinmeyen) RECEIPT olay üretmez ama kalıcı silmeyi
    /// idempotent olarak yineler: önceki bir silme hatasından kalan kopya böyle kapanır. Bellekte
    /// bekleyen erken metnin kalıcı kopyasına dokunulmaz.
    ///
    /// [`Delivered`]: MessengerEvent::Delivered
    fn on_receipt(&mut self, peer: PeerRef, ref_msg_id: MsgId) -> Option<MessengerEvent> {
        let state = self.peers.get_mut(&peer)?;
        if state.outbox.remove(&ref_msg_id).is_none() {
            self.stats.receipts_ignored += 1;
            let early_pending = state
                .early
                .as_ref()
                .is_some_and(|early| early.msg_id == ref_msg_id);
            if !early_pending && self.store.outbox_remove(peer, ref_msg_id).is_err() {
                self.stats.store_errors += 1;
            }
            return None;
        }
        if self.store.outbox_remove(peer, ref_msg_id).is_err() {
            self.stats.store_errors += 1;
        }
        Some(MessengerEvent::Delivered {
            peer,
            msg_id: ref_msg_id,
        })
    }
}

/// Eş başına durum — oturumlardan bağımsız (S2). Yalnız [`materialize`] kurar: outbox aynası
/// HER ZAMAN depodaki kalıcı outbox'tan başlar.
struct PeerState {
    /// Gönderimin gideceği güncel tutamaç.
    session: Option<SessionHandle>,
    /// Depodan ayrılmış ama henüz hiçbir kayda verilmemiş `msg_id` (ayırmadan sonraki bir adım
    /// hata verdi): sıradaki gönderim onu kullanır, yerel hata `msg_id` yakmaz. Kalıcı DEĞİLDİR;
    /// süreç ölürse değer yanar (alıcıda boşluk).
    spare_msg_id: Option<MsgId>,
    /// RECEIPT bekleyen kayıtlar — depodaki kalıcı outbox'ın aynası; `msg_id` sırasıyla
    /// (≤ [`MAX_OUTBOX_UNACKED`]).
    outbox: BTreeMap<MsgId, OutboxEntry>,
    /// Outbox'ta güncel tutamaca henüz kabul ettirilememiş kayıt var.
    resend_pending: bool,
    /// Alıcı dedup penceresi.
    dedup: DedupWindow,
    /// [K4G-05d] Transport'a henüz kabul ettirilemeyen `RECEIPT(delivered)` borçlarının
    /// `ref_msg_id`'leri, doğuş sırasıyla: eşte tekil, ≤ [`MAX_OUTBOX_UNACKED`] (taşmada en eski
    /// düşer), kalıcı DEĞİL. Yalnız `session` doluyken dolu olabilir: kayıt yalnız eşin o anki
    /// oturumunda bekler, oturum değişince ya da kapanınca kuyruk boşalır (modül belgesi,
    /// "RECEIPT").
    pending_receipts: VecDeque<MsgId>,
    /// [B2.5] Bekleyen niyet: bilet + son `ConnectProgress` fazı.
    connecting: Option<(ConnectTicket, ConnectPhase)>,
    /// [B2.5] Transport'ta duran erken metin (S1 K6), kendi niyetinin biletiyle: o niyet `Ready`
    /// ile çözülürse outbox'a "gönderilmiş", niyet biterse "gönderilmemiş" olarak geçer (modül
    /// belgesi, "Bağlantı").
    early: Option<EarlyText>,
}

/// [B2.5] Erken metin ve onu taşıyan niyet (S1 K6). Bilet, metnin hangi niyetle Transport'a
/// verildiğini kaydeder: `ConnectAbandoned` ve `Ready` metni ANCAK bu biletle eşler.
struct EarlyText {
    /// Metni `early_unit` olarak taşıyan niyetin bileti.
    ticket: ConnectTicket,
    /// Metnin `msg_id`'si (depodan ayrıldı, kalıcı kopyası outbox'ta).
    msg_id: MsgId,
    /// Kodlanmış TEXT kaydı.
    unit: PreparedUnit,
}

/// RECEIPT bekleyen kayıt.
struct OutboxEntry {
    /// Kodlanmış TEXT kaydı: yeniden gönderim AYNI baytları (aynı `msg_id`) taşır.
    unit: PreparedUnit,
    /// Kaydı en son kabul eden tutamaç (kalıcı DEĞİL); `None` = bu süreçte hiçbir tutamaca kabul
    /// ettirilmedi (vazgeçilen niyetin erken metni ya da depodan yüklenmiş kayıt).
    sent_on: Option<SessionHandle>,
}

impl PeerState {
    /// Depodan yüklenmiş outbox'la eş durumu: kayıtlar bu süreçte hiçbir tutamaca gönderilmemiştir
    /// ve eşin sıradaki oturumunda yeniden gönderilir.
    fn restored(units: Vec<OutboxUnit>) -> Self {
        let outbox: BTreeMap<MsgId, OutboxEntry> = units
            .into_iter()
            .map(|record| {
                let msg_id = record.msg_id();
                let entry = OutboxEntry {
                    unit: record.into_unit(),
                    sent_on: None,
                };
                (msg_id, entry)
            })
            .collect();
        Self {
            session: None,
            spare_msg_id: None,
            resend_pending: !outbox.is_empty(),
            outbox,
            dedup: DedupWindow::new(),
            pending_receipts: VecDeque::new(),
            connecting: None,
            early: None,
        }
    }

    /// Gönderim ön denetimleri; atanacak `msg_id`'yi döndürür (henüz TÜKETMEZ). Değer önce yedekten,
    /// yoksa depodan gelir (`reserve_next` — değer dönmeden kalıcıdır) ve kayıt Transport'a kabul
    /// ettirilene dek yedekte durur ([`PeerState::commit_send`]).
    fn next_send_id(
        &mut self,
        store: &dyn AppStore,
        peer: PeerRef,
    ) -> Result<MsgId, MessengerError> {
        if self.outbox.len() >= MAX_OUTBOX_UNACKED {
            return Err(MessengerError::OutboxFull);
        }
        let msg_id = match self.spare_msg_id {
            Some(spare) => spare,
            None => {
                let msg_id = reserve_msg_id(store, peer)?;
                // Depo sözleşmesi (değerler kesin artar) eşin bekleyen her kaydının `msg_id`'sinden
                // büyük bir değer verir. Vermezse kayıt bekleyen bir kaydın yerine geçer ve onun
                // RECEIPT'i yenisini kapatırdı (#105 sınıfı): değer kullanılmaz.
                let newest_pending = self
                    .outbox
                    .last_key_value()
                    .map(|(&newest, _)| newest)
                    .max(self.early.as_ref().map(|early| early.msg_id));
                if newest_pending.is_some_and(|newest| msg_id <= newest) {
                    return Err(MessengerError::Store(AppStoreError::Corrupt));
                }
                self.spare_msg_id = Some(msg_id);
                msg_id
            }
        };
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

    /// Transport kaydı kabul etti: `msg_id` tüketilir (yedek boşalır), kayıt outbox aynasına girer
    /// (kalıcı kopyası Transport'tan önce yazıldı).
    fn commit_send(&mut self, msg_id: MsgId, unit: PreparedUnit, handle: SessionHandle) {
        self.outbox.insert(
            msg_id,
            OutboxEntry {
                unit,
                sent_on: Some(handle),
            },
        );
        self.spare_msg_id = None;
    }

    /// Erken metnin niyeti bitti (vazgeçildi ya da yerini başka biletli bir niyet aldı): birim
    /// teslim edilmemiştir (S1 §4). Kayıt outbox aynasına "hiçbir tutamaca gönderilmemiş" olarak
    /// geçer ve eşin sıradaki oturumunda AYNI `msg_id`'yle (aynı bayt) gider; kalıcı kopyası
    /// Transport'tan önce yazıldığı için zaten depodadır. Erken metin yoksa etkisizdir.
    fn release_early(&mut self) {
        if let Some(early) = self.early.take() {
            self.outbox.insert(
                early.msg_id,
                OutboxEntry {
                    unit: early.unit,
                    sent_on: None,
                },
            );
            self.resend_pending = true;
        }
    }
}

/// `peer`'in durumu; motor eşi henüz tanımıyorsa eşin kalıcı outbox'ı depodan yüklenerek kurulur
/// (tablo tavanı denetimi önce). Yükleme başarısızsa eş KURULMAZ: eksik bir aynayla yeni gönderime
/// izin vermek, yüklenemeyen eski kayıtları alıcının dedup penceresinin altına itebilir ve onlara
/// yanlış "teslim edildi" ürettirebilirdi (pencere-altı = fail-closed tekrar + RECEIPT).
///
/// # Errors
///
/// [`MessengerError::PeerTableFull`] · [`MessengerError::Store`] — iki durumda da tablo değişmez.
fn materialize<'a>(
    peers: &'a mut BTreeMap<PeerRef, PeerState>,
    store: &dyn AppStore,
    peer: PeerRef,
) -> Result<&'a mut PeerState, MessengerError> {
    let known_peers = peers.len();
    match peers.entry(peer) {
        Entry::Occupied(slot) => Ok(slot.into_mut()),
        Entry::Vacant(slot) => {
            if known_peers >= MAX_PEERS {
                return Err(MessengerError::PeerTableFull);
            }
            let units = store.outbox_list(peer).map_err(MessengerError::Store)?;
            // Depo sözleşmesi eş başına en çok `MAX_OUTBOX_UNACKED` kayıt verir; fazlası bozuk
            // depodur (sınır, aynaya almadan önce).
            if units.len() > MAX_OUTBOX_UNACKED {
                return Err(MessengerError::Store(AppStoreError::Corrupt));
            }
            Ok(slot.insert(PeerState::restored(units)))
        }
    }
}

/// Depodan eşin sıradaki `msg_id`'si. Tükenme kalıcıdır ve sarma yoktur.
///
/// # Errors
///
/// [`MessengerError::MsgIdExhausted`] (depo `Exhausted`) · [`MessengerError::Store`] — değer
/// ayrılmamıştır ya da (`u32`'ye sığmayan, depo sözleşmesine aykırı değer) kullanılamaz.
fn reserve_msg_id(store: &dyn AppStore, peer: PeerRef) -> Result<MsgId, MessengerError> {
    let value = store
        .reserve_next(peer, CounterKind::MsgId)
        .map_err(|error| match error {
            AppStoreError::Exhausted => MessengerError::MsgIdExhausted,
            other => MessengerError::Store(other),
        })?;
    MsgId::try_from(value).map_err(|_| MessengerError::Store(AppStoreError::Corrupt))
}

/// Yazma önce: kayıt Transport'a verilmeden ÖNCE eşin kalıcı outbox'ına yazılır. Yazılamazsa kayıt
/// Transport'a verilmez; depoda kalmış olabilecek kopya geri alınır ([`forget_unsent`]).
///
/// # Errors
///
/// [`MessengerError::OutboxFull`] (depo outbox tavanı; hiçbir şey yazılmadı) ·
/// [`MessengerError::Store`].
fn persist_unsent(
    store: &dyn AppStore,
    stats: &mut MessengerStats,
    peer: PeerRef,
    msg_id: MsgId,
    unit: &PreparedUnit,
) -> Result<(), MessengerError> {
    let record = OutboxUnit::new(msg_id, unit.clone()).map_err(MessengerError::Store)?;
    match store.outbox_put(peer, &record) {
        Ok(()) => Ok(()),
        Err(AppStoreError::OutboxFull) => Err(MessengerError::OutboxFull),
        Err(error) => {
            // Depo sözleşmesi: `Err` sonrası yazma kalıcı olmuş OLABİLİR.
            forget_unsent(store, stats, peer, msg_id);
            Err(MessengerError::Store(error))
        }
    }
}

/// Transport'a verilmeyen kaydın kalıcı kopyasını geri alır (tek deneme). Başarısızsa kayıt depoda
/// kalır ve `store_errors` artar; `msg_id` yedekte durduğu için eşin sıradaki gönderimi aynı
/// `msg_id`'yle onun yerine geçer (modül belgesi, "Beyanlı sınırlar").
fn forget_unsent(store: &dyn AppStore, stats: &mut MessengerStats, peer: PeerRef, msg_id: MsgId) {
    if store.outbox_remove(peer, msg_id).is_err() {
        stats.store_errors += 1;
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
        if entry.sent_on == Some(handle) {
            continue;
        }
        if transport
            .send(handle, entry.unit.clone(), DeliveryPolicy::Reliable)
            .is_err()
        {
            stats.resend_refused += 1;
            return;
        }
        entry.sent_on = Some(handle);
        stats.resent_on_new_session += 1;
    }
    state.resend_pending = false;
}

/// `RECEIPT(delivered)` borcunu eşe kaydeder. Eşin kuyruğu boşsa borcu güncel tutamaca hemen sunar
/// (tek çağrı); değilse kuyruğun sonuna ekler. Dolu kuyruğu `Messenger::poll` başı sunar. Ayrıntı:
/// modül belgesi, "RECEIPT".
fn owe_receipt<T: KamaTransport>(
    transport: &mut T,
    state: &mut PeerState,
    ref_msg_id: MsgId,
    limits: &AmpLimits,
    stats: &mut MessengerStats,
) {
    let Some(handle) = state.session else {
        // Oturum yok: borç bırakılır. Telafi S2'dir: gönderen kaydı eşin sıradaki oturumunda
        // yeniden gönderir, tekrar TEXT RECEIPT'i yeniden üretir.
        stats.receipts_unsent += 1;
        return;
    };
    if state.pending_receipts.contains(&ref_msg_id) {
        // Aynı `(eş, msg_id)` zaten bekliyor: tekrar TEXT onunla birleşir.
        return;
    }
    if state.pending_receipts.is_empty() {
        // Önünde bekleyen yok: tek çağrı.
        match offer_receipt(transport, handle, ref_msg_id, limits) {
            ReceiptOffer::Accepted => {
                stats.receipts_sent += 1;
                return;
            }
            ReceiptOffer::Dropped => {
                stats.receipts_unsent += 1;
                return;
            }
            ReceiptOffer::Deferred => {}
        }
    }
    // Kuyruğun yalnız sonuna eklenir; dolu kuyruğu `Messenger::poll` başı sunar.
    state.pending_receipts.push_back(ref_msg_id);
    stats.receipts_deferred += 1;
}

/// [K4G-05d] Bir RECEIPT sunumunun sonucu — yalnız `Backpressure` geçicidir (DUR-05d-3).
enum ReceiptOffer {
    /// Transport kabul etti; sonrası Transport'undur (ikinci ağ ARQ'su yok).
    Accepted,
    /// `Backpressure`: birim tele çıkmadı; kayıt aynı oturumda bekler (S1 K8 "bekler").
    Deferred,
    /// Başka her hata (oturum sonu ya da sözleşme dışı) ve kodlama hatası: kayıt bırakılır.
    Dropped,
}

/// [K4G-05d] `RECEIPT(delivered, ref_msg_id)`'yi `handle`'a BİR KEZ sunar ve sonucu sınıflar
/// ([`ReceiptOffer`]).
fn offer_receipt<T: KamaTransport>(
    transport: &mut T,
    handle: SessionHandle,
    ref_msg_id: MsgId,
    limits: &AmpLimits,
) -> ReceiptOffer {
    let receipt = AppMessage::Receipt {
        ref_msg_id,
        kind: ReceiptKind::Delivered,
    };
    // RECEIPT gövdesi 5 B'tır ve her geçerli `AmpLimits`'e sığar (MAX_BODY_BYTES ≥ FILE_CHUNK
    // öneki); kodlama hatası yine de gönderilemeyen RECEIPT sayılır, panik yok.
    let Ok(unit) = codec::encode(&receipt, limits) else {
        return ReceiptOffer::Dropped;
    };
    match transport.send(handle, unit, DeliveryPolicy::Reliable) {
        Ok(()) => ReceiptOffer::Accepted,
        Err(TransportError::Backpressure) => ReceiptOffer::Deferred,
        Err(_) => ReceiptOffer::Dropped,
    }
}

/// [K4G-05d] Eşin bekleyen RECEIPT'lerini güncel tutamaca baştan, her birini BİR KEZ sunar: kabul
/// edilen çıkar, başka bir hatayla reddedilen bırakılır. İlk `Backpressure`'da durur: reddedilen
/// baştaki kayıt ve arkası kuyrukta kalır, aynı çağrıda yeniden denenmez — bir [`Messenger::poll`]
/// çağrısında eş başına en çok bir `Backpressure`. Zamanlayıcı yoktur.
fn flush_peer_receipts<T: KamaTransport>(
    transport: &mut T,
    state: &mut PeerState,
    limits: &AmpLimits,
    stats: &mut MessengerStats,
) {
    let Some(handle) = state.session else {
        // Kuyruk yalnız oturum varken doludur (oturum değişince ve kapanınca boşalır).
        return;
    };
    while let Some(&ref_msg_id) = state.pending_receipts.front() {
        match offer_receipt(transport, handle, ref_msg_id, limits) {
            ReceiptOffer::Accepted => stats.receipts_sent += 1,
            ReceiptOffer::Dropped => stats.receipts_unsent += 1,
            ReceiptOffer::Deferred => return,
        }
        state.pending_receipts.pop_front();
    }
}

/// [K4G-05d] Eşin oturumu değişti ya da kapandı (DUR-05d-4): bekleyen RECEIPT'ler bırakılır ve
/// `receipts_unsent`'te sayılır. Telafi gönderenin yeni oturumdaki yeniden gönderimi ve onun tekrar
/// TEXT'inin RECEIPT'idir (S2): kuyruktaki her `msg_id` ya gönderenin outbox'ındadır ya da
/// (oturum-değişimi drain tekrarı) gönderende zaten `Delivered`'dır.
fn drop_pending_receipts(state: &mut PeerState, stats: &mut MessengerStats) {
    for _ in state.pending_receipts.drain(..) {
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
    //! Pencere sınırları, sayaç tükenmesi, depo sözleşmesi ihlalleri ve tablo tavanları — uçtan
    //! uca akış (yeniden başlatma, kalıcı outbox, depo hatası dahil) `tests/text_receipt.rs`'te.
    //! K4G-05d bekleyen RECEIPT kuyruğunun yeni API'yi okuyan testleri (T1, T3b, T4, T6; aynı
    //! tutamacın yeniden bildirilmesi, çok eşli dağılım) burada; tabanda derlenen davranışsal
    //! eşleri `tests/k4g_p0_receipt.rs`'te.

    use std::collections::BTreeSet;
    use std::sync::Mutex;

    use super::*;
    use crate::store::FIRST_COUNTER_VALUE;
    use kama_contracts::handles::CloseReason;
    use kama_contracts::s1::{CompressionPolicy, ConnectTicket, TransportCapabilities};

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
    /// [`drain_scripted`] döngüsünün tavanı (test parametresi).
    const TEST_DRAIN_CAP: usize = 4 * MAX_OUTBOX_UNACKED;

    /// `msg_id` sayacı testin koyduğu değerden süren (tükenme kuralı ürün depolarıyla aynı:
    /// [`CounterKind::advance`]) ve istenirse outbox listesini sabit veren depo; geri kalan her şey
    /// bellek içi depoya gider.
    struct TestStore {
        next: Mutex<u64>,
        listed: Option<Vec<OutboxUnit>>,
        rest: MemoryAppStore,
    }

    impl TestStore {
        fn seeded(next: u64) -> Self {
            Self {
                next: Mutex::new(next),
                listed: None,
                rest: MemoryAppStore::new(),
            }
        }

        fn listing(units: Vec<OutboxUnit>) -> Self {
            Self {
                listed: Some(units),
                ..Self::seeded(FIRST_COUNTER_VALUE)
            }
        }

        fn set_next(&self, next: u64) {
            *self.next.lock().expect("kilit zehirlenmedi") = next;
        }
    }

    impl AppStore for TestStore {
        fn reserve_next(&self, _peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
            let mut next = self.next.lock().expect("kilit zehirlenmedi");
            let value = *next;
            *next = kind.advance(value)?;
            Ok(value)
        }

        fn dedup_load(&self, peer: PeerRef) -> Result<crate::store::DedupWindow, AppStoreError> {
            self.rest.dedup_load(peer)
        }

        fn dedup_save(
            &self,
            peer: PeerRef,
            window: &crate::store::DedupWindow,
        ) -> Result<(), AppStoreError> {
            self.rest.dedup_save(peer, window)
        }

        fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError> {
            self.rest.outbox_put(peer, unit)
        }

        fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError> {
            self.rest.outbox_remove(peer, msg_id)
        }

        fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
            match &self.listed {
                Some(units) => Ok(units.clone()),
                None => self.rest.outbox_list(peer),
            }
        }
    }

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

    /// [`RecordingTransport::connect`]'in her zaman döndürdüğü bilet.
    const RECORDING_TICKET: ConnectTicket = ConnectTicket::from_raw(1);

    /// Her gönderimi kabul eden ve hangi tutamaca gittiğini kaydeden Transport; `connect` her
    /// zaman `Pending(RECORDING_TICKET)` döner, hiçbir şey teslim etmez.
    #[derive(Default)]
    struct RecordingTransport {
        sent_on: Vec<SessionHandle>,
    }

    impl KamaTransport for RecordingTransport {
        fn capabilities(&self) -> TransportCapabilities {
            NullTransport.capabilities()
        }

        fn connect(
            &mut self,
            _peer: PeerRef,
            _early_unit: Option<PreparedUnit>,
        ) -> Result<ConnectResult, TransportError> {
            Ok(ConnectResult::Pending(RECORDING_TICKET))
        }

        fn send(
            &mut self,
            session: SessionHandle,
            _unit: PreparedUnit,
            _policy: DeliveryPolicy,
        ) -> Result<(), TransportError> {
            self.sent_on.push(session);
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
        let p = peer(0);
        let mut state = PeerState::restored(Vec::new());
        assert_eq!(
            state.next_send_id(&MemoryAppStore::new(), p),
            Ok(FIRST_MSG_ID),
            "taze depo"
        );
        let store = TestStore::seeded(u64::from(MsgId::MAX));
        let mut state = PeerState::restored(Vec::new());
        let last = state
            .next_send_id(&store, p)
            .expect("son msg_id kullanılabilir");
        assert_eq!(last, MsgId::MAX);
        state.commit_send(last, unit(), handle(1));
        assert_eq!(
            state.next_send_id(&store, p),
            Err(MessengerError::MsgIdExhausted)
        );
        // RECEIPT outbox'u boşaltsa da sarma yok: tükenme depoda kalıcıdır.
        state.outbox.clear();
        assert_eq!(
            state.next_send_id(&store, p),
            Err(MessengerError::MsgIdExhausted)
        );
        assert_eq!(state.spare_msg_id, None, "ayrılamayan değer yedeğe girmez");
    }

    #[test]
    fn unacked_span_stays_inside_the_dedup_window() {
        let p = peer(0);
        let store = TestStore::seeded(FIRST_COUNTER_VALUE);
        let mut state = PeerState::restored(Vec::new());
        let oldest = state.next_send_id(&store, p).expect("ilk gönderim");
        state.commit_send(oldest, unit(), handle(1));
        store.set_next(u64::from(oldest + MSG_DEDUP_WINDOW - 1));
        let edge = state.next_send_id(&store, p).expect("pencerenin son üyesi");
        assert_eq!(edge, oldest + MSG_DEDUP_WINDOW - 1);
        state.commit_send(edge, unit(), handle(1));
        // Sıradaki değer `oldest`'u alıcı penceresinin altına iterdi: tipli geri basınç.
        assert_eq!(
            state.next_send_id(&store, p),
            Err(MessengerError::UnackedSpanFull { oldest })
        );
        // Red değeri yakmadı: `oldest`'un RECEIPT'i gelince AYNI değer kullanılır, depo bir daha
        // sorulmaz.
        state.outbox.remove(&oldest);
        assert_eq!(state.next_send_id(&store, p), Ok(oldest + MSG_DEDUP_WINDOW));
        assert_eq!(
            store.reserve_next(p, CounterKind::MsgId),
            Ok(u64::from(oldest + MSG_DEDUP_WINDOW) + 1)
        );
    }

    /// Kodlanmış TEXT kaydının `msg_id`'si ve metni.
    fn text_of(unit: &PreparedUnit, limits: &AmpLimits) -> (MsgId, String) {
        match codec::decode(&unit.bytes, limits) {
            Ok(AppMessage::Text { msg_id, text }) => (msg_id, text),
            other => panic!("TEXT bekleniyordu: {other:?}"),
        }
    }

    #[test]
    fn a_store_that_repeats_a_pending_msg_id_is_corrupt_and_overwrites_nothing() {
        let p = peer(0);
        let seeded = Arc::new(TestStore::seeded(5));
        let store: Arc<dyn AppStore> = seeded.clone();
        let mut messenger = Messenger::with_store(NullTransport, store).expect("limitler geçerli");
        messenger.adopt(handle(1), p);
        let five = messenger.send_text(p, "bekleyen").expect("ilk gönderim");
        assert_eq!(five, 5);
        // Sözleşme ihlali (depo geri sardı): bekleyen kaydın değeri yeniden verildi. Denetim
        // olmasaydı yeni kayıt bekleyenin yerine geçerdi (bellekte ve depoda) ve bekleyenin
        // RECEIPT'i yenisini kapatırdı.
        seeded.set_next(u64::from(five));
        assert_eq!(
            messenger.send_text(p, "ezen"),
            Err(MessengerError::Store(AppStoreError::Corrupt))
        );
        let state = &messenger.peers[&p];
        assert_eq!(state.outbox.len(), 1);
        assert_eq!(
            text_of(&state.outbox[&five].unit, messenger.limits()),
            (five, "bekleyen".to_owned()),
            "bellekteki kayıt ezilmedi"
        );
        let persisted = seeded.outbox_list(p).expect("depo okunur");
        assert_eq!(persisted.len(), 1);
        assert_eq!(
            text_of(persisted[0].unit(), messenger.limits()),
            (five, "bekleyen".to_owned()),
            "depodaki kayıt ezilmedi"
        );
        assert_eq!(state.spare_msg_id, None, "geçersiz değer yedeğe girmez");

        // Bellekte bekleyen erken metnin değeri de bekleyen sayılır.
        let mut state = PeerState::restored(Vec::new());
        state.early = Some(EarlyText {
            ticket: ConnectTicket::from_raw(0),
            msg_id: 7,
            unit: unit(),
        });
        let repeating = TestStore::seeded(7);
        assert_eq!(
            state.next_send_id(&repeating, p),
            Err(MessengerError::Store(AppStoreError::Corrupt))
        );
        assert_eq!(state.spare_msg_id, None);
        assert_eq!(state.next_send_id(&repeating, p), Ok(8));
    }

    #[test]
    fn an_outbox_listing_over_the_cap_is_corrupt_and_builds_no_peer() {
        let units: Vec<OutboxUnit> = (0..=MAX_OUTBOX_UNACKED)
            .map(|n| {
                let msg_id = MsgId::try_from(n).expect("küçük sayı");
                OutboxUnit::new(msg_id, unit()).expect("sınır içinde")
            })
            .collect();
        let store = Arc::new(TestStore::listing(units));
        let mut messenger = Messenger::with_store(NullTransport, store).expect("limitler geçerli");
        assert_eq!(
            messenger.connect(peer(0)),
            Err(MessengerError::Store(AppStoreError::Corrupt))
        );
        assert!(messenger.peers.is_empty(), "çağrı yolunda eş kurulmadı");
        // Olay yolu: eş kurulmaz, tutamaç benimsenmez, hata sayılır.
        messenger.adopt(handle(1), peer(0));
        assert!(messenger.peers.is_empty(), "olay yolunda eş kurulmadı");
        assert_eq!(messenger.peer_of(handle(1)), None);
        assert_eq!(messenger.stats().store_errors, 1);
    }

    /// K4G-07 (inceleme, mutasyon M3): `adopt` erken metni YALNIZ onu taşıyan niyet bu oturumla
    /// çözüldüyse "h'ye gönderilmiş" sayar ve o durumda h'den YOLLAMAZ (Transport onu h'de ilk
    /// normal `send` olarak yollar, K6). Çözülen niyet erken metnin niyeti değilse (bekleyen niyet
    /// kaydı yok — ör. eş-başlatımlı oturum — ya da başka bilet) Transport metni bu oturumda
    /// taşımıyordur: kayıt "gönderilmemiş" girer ve h'den TAM BİR KEZ gönderilir. Genel API bu
    /// durumları `adopt`'tan önce (`note_pending`, `ConnectAbandoned`) çözer; test `adopt`'un kendi
    /// kuralını sabitler: çözülen bilet, `adopt` anındaki `connecting` kaydıdır.
    ///
    /// M3 (`sent_on = Some(handle)`, bilete bakmadan) bu testi KIRMIZI yapar: "niyet kaydı yok" ve
    /// "başka bilet" durumlarında kayıt "gönderilmiş" sayılır, h'ye hiçbir gönderim olmaz
    /// (`sent_on` boş, `resent_on_new_session` 0) ve metin bir sonraki oturum değişimine dek
    /// takılır.
    #[test]
    fn adopt_marks_the_early_text_sent_only_for_its_own_ticket() {
        let p = peer(0);
        let h = handle(9);
        for (case, resolved) in [
            ("bilet eşit", Some(RECORDING_TICKET)),
            ("bekleyen niyet kaydı yok", None),
            ("başka bilet", Some(ConnectTicket::from_raw(2))),
        ] {
            let mut messenger =
                Messenger::new(RecordingTransport::default()).expect("limitler geçerli");
            let (result, msg_id) = messenger
                .connect_with_text(p, "erken")
                .expect("niyet açıldı");
            assert_eq!(result, ConnectResult::Pending(RECORDING_TICKET), "{case}");
            messenger.peers.get_mut(&p).expect("eş kuruldu").connecting =
                resolved.map(|ticket| (ticket, ConnectPhase::Measuring));

            messenger.adopt(h, p);
            let carried = resolved == Some(RECORDING_TICKET);
            let expected_sends = if carried { Vec::new() } else { vec![h] };
            assert_eq!(
                messenger.transport().sent_on,
                expected_sends,
                "{case}: h'den gönderim"
            );
            assert_eq!(
                messenger.stats().resent_on_new_session,
                u64::from(!carried),
                "{case}"
            );
            let state = &messenger.peers[&p];
            assert!(state.early.is_none(), "{case}: erken metin outbox'a geçti");
            assert_eq!(
                state.outbox[&msg_id].sent_on,
                Some(h),
                "{case}: kayıt h'de (Transport'un ya da yeniden gönderimin)"
            );
            assert_eq!(state.connecting, None, "{case}: niyet çözüldü");
        }
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

    // ── K4G-05d: bekleyen RECEIPT kuyruğu — yeni API testleri (tabanda derlenmez) ──────────────

    /// Teslim kuyruğunu ve olayları testin koyduğu, gönderimin sonucunu testin seçtiği Transport:
    /// `refusal` doluyken her gönderim o hatayla reddedilir; boşken `full_sessions`'taki
    /// tutamaçlara gönderim `Backpressure` alır (oturum başına dolu offer kanalı), diğerlerine
    /// `room` kadar gönderim kabul edilir, sonrası `Backpressure`. Her deneme tutamacı, kaydı ve
    /// sonucuyla sırayla kaydedilir.
    struct ScriptedTransport {
        inbox: VecDeque<Delivered>,
        events: VecDeque<SessionEvent>,
        refusal: Option<TransportError>,
        room: usize,
        full_sessions: BTreeSet<SessionHandle>,
        attempts: Vec<ScriptedAttempt>,
    }

    /// Bir `send` denemesi: tutamaç, kayıt baytları, sonuç.
    type ScriptedAttempt = (SessionHandle, Vec<u8>, Result<(), TransportError>);

    impl ScriptedTransport {
        /// Geri basınçtaki Transport: her gönderim `Backpressure` alır.
        fn full() -> Self {
            Self {
                inbox: VecDeque::new(),
                events: VecDeque::new(),
                refusal: Some(TransportError::Backpressure),
                room: 0,
                full_sessions: BTreeSet::new(),
                attempts: Vec::new(),
            }
        }

        /// Sıradaki `room` gönderim kabul edilir, sonrası `Backpressure`.
        fn open(&mut self, room: usize) {
            self.refusal = None;
            self.room = room;
        }

        /// Her gönderim `error` ile reddedilir.
        fn refuse(&mut self, error: TransportError) {
            self.refusal = Some(error);
        }
    }

    impl KamaTransport for ScriptedTransport {
        fn capabilities(&self) -> TransportCapabilities {
            NullTransport.capabilities()
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
            session: SessionHandle,
            unit: PreparedUnit,
            _policy: DeliveryPolicy,
        ) -> Result<(), TransportError> {
            let result = match self.refusal {
                Some(error) => Err(error),
                None if self.room == 0 || self.full_sessions.contains(&session) => {
                    Err(TransportError::Backpressure)
                }
                None => {
                    self.room -= 1;
                    Ok(())
                }
            };
            self.attempts.push((session, unit.bytes, result));
            result
        }

        fn recv(&mut self) -> Option<Delivered> {
            self.inbox.pop_front()
        }

        fn poll_event(&mut self) -> Option<SessionEvent> {
            self.events.pop_front()
        }

        fn close(&mut self, _session: SessionHandle) -> Result<(), TransportError> {
            Err(TransportError::UnknownSession)
        }

        fn cancel(&mut self, _ticket: ConnectTicket) {}
    }

    /// Geri basınçtaki Transport'la kurulmuş, `p`'nin `h` oturumunu benimsemiş motor.
    fn scripted_with_session(p: PeerRef, h: SessionHandle) -> Messenger<ScriptedTransport> {
        let mut messenger = Messenger::new(ScriptedTransport::full()).expect("limitler geçerli");
        messenger
            .transport_mut()
            .events
            .push_back(SessionEvent::Ready(h, p));
        assert_eq!(
            drain_scripted(&mut messenger),
            vec![MessengerEvent::Session(SessionEvent::Ready(h, p))]
        );
        messenger
    }

    /// `h` üzerinden eşten gelmiş gibi `TEXT(msg_id)` koyar.
    fn deliver_text(messenger: &mut Messenger<ScriptedTransport>, h: SessionHandle, msg_id: MsgId) {
        let message = AppMessage::Text {
            msg_id,
            text: "metin".to_owned(),
        };
        let unit = codec::encode(&message, messenger.limits()).expect("kodlanır");
        messenger.transport_mut().inbox.push_back(Delivered {
            session: h,
            unit: unit.bytes,
        });
    }

    /// `poll` `None` verene dek olaylar.
    fn drain_scripted(messenger: &mut Messenger<ScriptedTransport>) -> Vec<MessengerEvent> {
        let mut events = Vec::new();
        while let Some(event) = messenger.poll() {
            assert!(events.len() < TEST_DRAIN_CAP, "olay akışı bitmiyor");
            events.push(event);
        }
        events
    }

    /// `p`'den gelen `TEXT(msg_id)`'nin teslim olayı.
    fn text_received(p: PeerRef, msg_id: MsgId) -> MessengerEvent {
        MessengerEvent::TextReceived {
            peer: p,
            msg_id,
            text: "metin".to_owned(),
        }
    }

    /// `RECEIPT(delivered, ref_msg_id)` sunumlarının sonuçları, sırayla.
    fn receipt_results(
        messenger: &Messenger<ScriptedTransport>,
        ref_msg_id: MsgId,
    ) -> Vec<Result<(), TransportError>> {
        let limits = messenger.limits();
        messenger
            .transport()
            .attempts
            .iter()
            .filter(|(_, bytes, _)| {
                matches!(
                    codec::decode(bytes, limits),
                    Ok(AppMessage::Receipt { ref_msg_id: of, kind: ReceiptKind::Delivered })
                        if of == ref_msg_id
                )
            })
            .map(|&(_, _, result)| result)
            .collect()
    }

    /// Kabul edilen `RECEIPT(delivered)`'ların `ref_msg_id`'leri, kabul sırasıyla.
    fn accepted_receipt_ids(messenger: &Messenger<ScriptedTransport>) -> Vec<MsgId> {
        let limits = messenger.limits();
        messenger
            .transport()
            .attempts
            .iter()
            .filter(|(_, _, result)| result.is_ok())
            .filter_map(|(_, bytes, _)| match codec::decode(bytes, limits) {
                Ok(AppMessage::Receipt {
                    ref_msg_id,
                    kind: ReceiptKind::Delivered,
                }) => Some(ref_msg_id),
                _ => None,
            })
            .collect()
    }

    /// `usize` sayısı `u64` sayaç değeri olarak.
    fn count(n: usize) -> u64 {
        u64::try_from(n).expect("küçük sayı")
    }

    /// K4G-05d **T3b** (not Ek B.2; yalnız 05d — yeni API, tabanda derlenmez): eşin oturumu
    /// KAPANINCA (`Closed(h)` okunur, `retire`) bekleyen RECEIPT'ler bırakılır (DUR-05d-4):
    /// `pending` 0'a döner, her biri `receipts_unsent`'te sayılır ve yeni oturuma taşınmaz (telafi
    /// tekrar TEXT'tir, S2). Boşaltmayan `retire` mutantı kayıtları oturumsuz eşte sıradaki
    /// `adopt`'a dek tutar (`pending` 2); davranışça görünmez — `adopt` da boşaltır — bu yüzden
    /// sayaçla sınanır.
    #[test]
    fn t3b_closing_the_session_drops_the_pending_receipts() {
        let p = peer(0);
        let h = handle(1);
        let mut messenger = scripted_with_session(p, h);
        deliver_text(&mut messenger, h, 0);
        deliver_text(&mut messenger, h, 1);
        assert_eq!(
            drain_scripted(&mut messenger),
            vec![text_received(p, 0), text_received(p, 1)]
        );
        assert_eq!(
            messenger.pending_receipts(p),
            2,
            "ön koşul: iki RECEIPT bekliyor"
        );

        let closed = SessionEvent::Closed(h, CloseReason::PeerClosed);
        messenger.transport_mut().events.push_back(closed);
        assert_eq!(messenger.poll(), Some(MessengerEvent::Session(closed)));
        assert_eq!(messenger.session_of(p), None);
        assert_eq!(messenger.pending_receipts(p), 0, "kapanış kuyruğu boşaltır");
        let stats = messenger.stats();
        assert_eq!(stats.receipts_pending, 0);
        assert_eq!(stats.receipts_unsent, 2);
        assert_eq!(stats.receipts_deferred, 2);

        // Kapasite açılsa ve yeni oturum gelse de kuyruktan hiçbir şey sunulmaz.
        messenger.transport_mut().open(usize::MAX);
        let h2 = handle(2);
        messenger
            .transport_mut()
            .events
            .push_back(SessionEvent::Ready(h2, p));
        let before = messenger.transport().attempts.len();
        assert_eq!(
            drain_scripted(&mut messenger),
            vec![MessengerEvent::Session(SessionEvent::Ready(h2, p))]
        );
        assert_eq!(
            messenger.transport().attempts.len(),
            before,
            "bırakılan RECEIPT'ler yeni oturuma taşınmaz"
        );
        assert!(accepted_receipt_ids(&messenger).is_empty());
    }

    /// K4G-05d **T4** (not Ek B.2; yalnız 05d — yeni API, tabanda derlenmez): yalnız `Backpressure`
    /// geçicidir (DUR-05d-3). Başka her `TransportError` — oturum sonu (`UnknownSession`,
    /// `SessionSuperseded`, `NotConnected`) ya da sözleşme dışı — RECEIPT'i bırakır: ilk sunumda
    /// reddedilen kuyruğa girmez, kuyruktan sunumda reddedilen çıkar; ikisinde de `pending` 0'a
    /// döner, `receipts_unsent` artar, `receipts_deferred` yalnız `Backpressure`'la gireni sayar ve
    /// bırakılan kayıt bir daha sunulmaz. Oturum yokken doğan RECEIPT de kuyruğa girmez ve hiç
    /// sunulmaz. Geçici sınıfı genişleten mutant (ör. `file_transfer.rs` `is_transient` sınıfı)
    /// kaydı tutar (`pending` 1). Davranışsal eşi T4 (`tests/k4g_p0_receipt.rs`).
    #[test]
    fn t4_only_backpressure_keeps_a_receipt_pending() {
        let p = peer(0);
        let h = handle(1);
        for error in [
            TransportError::UnknownSession,
            TransportError::SessionSuperseded,
            TransportError::NotConnected,
            TransportError::UnitTooLarge,
            TransportError::DeadlineExpired,
            TransportError::EarlyUnitConflict,
        ] {
            let mut messenger = scripted_with_session(p, h);
            // (a) İlk sunum `error` ile reddedilir: kayıt kuyruğa girmez.
            messenger.transport_mut().refuse(error);
            deliver_text(&mut messenger, h, 0);
            assert_eq!(messenger.poll(), Some(text_received(p, 0)), "{error:?}");
            assert_eq!(
                messenger.pending_receipts(p),
                0,
                "{error:?}: ilk sunumda ret"
            );
            // (b) Kuyruktan sunum `error` ile reddedilir: kayıt çıkar.
            messenger
                .transport_mut()
                .refuse(TransportError::Backpressure);
            deliver_text(&mut messenger, h, 1);
            assert_eq!(messenger.poll(), Some(text_received(p, 1)), "{error:?}");
            assert_eq!(
                messenger.pending_receipts(p),
                1,
                "{error:?}: Backpressure → bekler"
            );
            messenger.transport_mut().refuse(error);
            assert_eq!(messenger.poll(), None, "{error:?}");
            assert_eq!(
                messenger.pending_receipts(p),
                0,
                "{error:?}: kuyruktan sunumda ret"
            );
            // Bırakılanlar kapasite açılınca da sunulmaz.
            messenger.transport_mut().open(usize::MAX);
            assert_eq!(messenger.poll(), None, "{error:?}");
            assert_eq!(
                receipt_results(&messenger, 0),
                vec![Err(error)],
                "{error:?}"
            );
            assert_eq!(
                receipt_results(&messenger, 1),
                vec![Err(TransportError::Backpressure), Err(error)],
                "{error:?}"
            );
            let stats = messenger.stats();
            assert_eq!(
                (
                    stats.receipts_deferred,
                    stats.receipts_unsent,
                    stats.receipts_sent,
                    stats.receipts_pending
                ),
                (1, 2, 0, 0),
                "{error:?}: (deferred, unsent, sent, pending)"
            );
        }

        // (c) Oturum yok: `Closed(h)` okunduktan sonra eski tutamaçla gelen TEXT (yalnız-alım
        // drain) teslim edilir; RECEIPT'i kuyruğa girmez, Transport'a da verilmez.
        let mut messenger = scripted_with_session(p, h);
        messenger.transport_mut().open(usize::MAX);
        let closed = SessionEvent::Closed(h, CloseReason::PeerClosed);
        messenger.transport_mut().events.push_back(closed);
        assert_eq!(messenger.poll(), Some(MessengerEvent::Session(closed)));
        deliver_text(&mut messenger, h, 0);
        assert_eq!(messenger.poll(), Some(text_received(p, 0)));
        assert_eq!(messenger.pending_receipts(p), 0);
        assert!(
            messenger.transport().attempts.is_empty(),
            "oturumsuz RECEIPT sunulmaz"
        );
        let stats = messenger.stats();
        assert_eq!(
            (
                stats.receipts_deferred,
                stats.receipts_unsent,
                stats.receipts_pending
            ),
            (0, 1, 0)
        );
    }

    /// `receipts_deferred` = kuyruktan kabul edilen + kuyruktan düşen + `receipts_pending`
    /// değişmezini (T6) ve beklenen `pending`'i denetler. T6 senaryosunda her RECEIPT kuyruktan
    /// geçer: "kuyruktan kabul" Transport'un kabul kaydıdır (= `receipts_sent`), "kuyruktan düşen"
    /// senaryonun saydığı `dropped`'tır (= `receipts_unsent`).
    fn assert_receipt_invariant(
        messenger: &Messenger<ScriptedTransport>,
        dropped: u64,
        pending: u64,
        step: &str,
    ) {
        let stats = messenger.stats();
        let accepted = count(accepted_receipt_ids(messenger).len());
        assert_eq!(stats.receipts_pending, pending, "{step}: pending");
        assert_eq!(
            messenger.pending_receipts(peer(0)),
            usize::try_from(pending).expect("küçük sayı"),
            "{step}: pending_receipts"
        );
        assert_eq!(
            stats.receipts_deferred,
            accepted + dropped + stats.receipts_pending,
            "{step}: deferred = kuyruktan kabul + kuyruktan düşen + pending"
        );
        assert_eq!(stats.receipts_sent, accepted, "{step}: her kabul kuyruktan");
        assert_eq!(
            stats.receipts_unsent, dropped,
            "{step}: her bırakılan kuyruktan"
        );
    }

    /// K4G-05d **T6** (not Ek B.2; yalnız 05d — yeni API, tabanda derlenmez; sayaç sözleşmesi,
    /// DUR-05d-2): kuyruğa giren her kayıt tam bir yoldan çıkar ya da bekler; her an
    /// `receipts_deferred` = kuyruktan kabul edilen + kuyruktan düşen + `receipts_pending`. Senaryo
    /// kuyruğun bütün yollarını sırayla geçer: birleşen tekrar (sayılmaz), kısmi kapasite (baştaki
    /// kabul, sonra `Backpressure`'da dur), kalıcı ret, oturum değişimi (`adopt` boşaltır), taşma;
    /// sonunda kapasite açılır, `pending` 0'a döner ve `receipts_deferred` ≥ 1'dir. `pending` sabit
    /// 0 mutantı ilk denetimde düşer.
    #[test]
    fn t6_deferred_equals_accepted_plus_dropped_from_the_queue_plus_pending() {
        let p = peer(0);
        let h = handle(1);
        let mut messenger = scripted_with_session(p, h);

        // 1. Geri basınç: üç yeni kayıt (ilkinin ilk sunumu `Backpressure` alır, diğerleri
        //    arkasına eklenir) ve kuyruktakiyle birleşen bir tekrar.
        for msg_id in [0, 1, 2, 1] {
            deliver_text(&mut messenger, h, msg_id);
        }
        assert_eq!(
            drain_scripted(&mut messenger).len(),
            3,
            "tekrar olay üretmez"
        );
        assert_eq!(messenger.stats().duplicate_texts, 1);
        assert_receipt_invariant(&messenger, 0, 3, "geri basınç");

        // 2. Kısmi kapasite: baştaki kabul edilir, ikincisi `Backpressure` alır, sunum durur.
        messenger.transport_mut().open(1);
        assert_eq!(messenger.poll(), None);
        assert_eq!(accepted_receipt_ids(&messenger), vec![0]);
        assert_receipt_invariant(&messenger, 0, 2, "kısmi kapasite");

        // 3. Kalıcı ret: kuyruktaki iki kayıt bırakılır.
        messenger
            .transport_mut()
            .refuse(TransportError::UnknownSession);
        assert_eq!(messenger.poll(), None);
        assert_receipt_invariant(&messenger, 2, 0, "kalıcı ret");

        // 4. Oturum değişimi: bekleyen yeni kayıt düşer.
        messenger
            .transport_mut()
            .refuse(TransportError::Backpressure);
        deliver_text(&mut messenger, h, 3);
        assert_eq!(drain_scripted(&mut messenger), vec![text_received(p, 3)]);
        assert_receipt_invariant(&messenger, 2, 1, "yeni kayıt bekliyor");
        let h2 = handle(2);
        messenger
            .transport_mut()
            .events
            .push_back(SessionEvent::Ready(h2, p));
        assert_eq!(
            messenger.poll(),
            Some(MessengerEvent::Session(SessionEvent::Ready(h2, p)))
        );
        assert_receipt_invariant(&messenger, 3, 0, "oturum değişimi");

        // 6. Kapasite: kalanların hepsi kabul edilir.
        messenger.transport_mut().open(usize::MAX);
        assert_eq!(messenger.poll(), None);
        assert_receipt_invariant(&messenger, 4, 0, "kapasite sonrası");
        assert!(messenger.stats().receipts_deferred >= 1);
    }

    /// K4G-05d (yalnız 05d — yeni API, tabanda derlenmez): aynı tutamacın ikinci kez bildirilmesi
    /// (ör. `connect` dönüşü `ConnectResult::Ready(h)` + aynı `h`'nin `Ready` olayı) oturum
    /// DEĞİŞİMİ değildir: `adopt` aynı tutamaçta erken döner ve bekleyen kuyruk KORUNUR — hiçbir
    /// kayıt bırakılmaz, kapasite açılınca hepsi aynı oturumda doğuş sırasıyla kabul edilir.
    /// Öldürdüğü mutant: `drop_pending_receipts`'i `adopt`'un aynı-tutamaç erken dönüşünün üstüne
    /// taşıyan değişiklik (kuyruk boşalır: `pending` 0, `receipts_unsent` 2, kabul yok).
    #[test]
    fn re_announcing_the_same_handle_keeps_the_pending_receipts() {
        let p = peer(0);
        let h = handle(1);
        let mut messenger = scripted_with_session(p, h);
        deliver_text(&mut messenger, h, 0);
        deliver_text(&mut messenger, h, 1);
        assert_eq!(
            drain_scripted(&mut messenger),
            vec![text_received(p, 0), text_received(p, 1)]
        );
        assert_eq!(
            messenger.pending_receipts(p),
            2,
            "ön koşul: iki RECEIPT bekliyor"
        );

        let again = SessionEvent::Ready(h, p);
        messenger.transport_mut().events.push_back(again);
        assert_eq!(messenger.poll(), Some(MessengerEvent::Session(again)));
        assert_eq!(messenger.session_of(p), Some(h), "oturum değişmedi");
        assert_eq!(
            messenger.pending_receipts(p),
            2,
            "aynı tutamaç: bekleyen kuyruk korunur"
        );
        assert_eq!(
            messenger.stats().receipts_unsent,
            0,
            "hiçbir RECEIPT bırakılmadı"
        );

        messenger.transport_mut().open(usize::MAX);
        assert_eq!(messenger.poll(), None);
        assert_eq!(
            accepted_receipt_ids(&messenger),
            vec![0, 1],
            "kuyruk aynı oturumda, doğuş sırasıyla kabul edilir"
        );
        assert_eq!(messenger.pending_receipts(p), 0);
    }

    /// K4G-05d (yalnız 05d — yeni API, tabanda derlenmez): bekleyen kuyruk EŞ BAŞINADIR, geri
    /// basınç OTURUM BAŞINADIR. (1) İki eşin RECEIPT'i geri basınçta bekler; `receipts_pending` iki
    /// eşin toplamıdır. (2) Yalnız `p0`'ın oturumu dolu kalır (`p0` eş tablosunda önce gelir): TEK
    /// `poll`'da `p1`'in RECEIPT'i kabul edilir, `p0`'ınki yine sunulur ve bekler. (3) Poll sırası:
    /// oturum-değişimi yeniden gönderimleri bekleyen RECEIPT'lerden ÖNCE sunulur (`poll` belgesi,
    /// (1) → (2)) — tek kabul yerini `p1`'in yeniden gönderimi alır, `p0`'ın RECEIPT'i
    /// `Backpressure` alır. Öldürdüğü mutantlar: bir eşin `Backpressure`'ında bütün eşlerin
    /// sunumunu durduran `flush_receipts` (`p1` kabul edilmez); `receipts_pending`'i tek eşten
    /// okuyan `stats()` (toplam 1); `flush_resends` ile `flush_receipts` sırasını çeviren `poll`
    /// (RECEIPT kabul edilir, yeniden gönderim reddedilir).
    #[test]
    fn pending_receipts_are_per_peer_and_backpressure_is_per_session() {
        let (p0, h0) = (peer(0), handle(1));
        let (p1, h1) = (peer(1), handle(2));
        let mut messenger = Messenger::new(ScriptedTransport::full()).expect("limitler geçerli");
        for (p, h) in [(p0, h0), (p1, h1)] {
            messenger
                .transport_mut()
                .events
                .push_back(SessionEvent::Ready(h, p));
        }
        assert_eq!(drain_scripted(&mut messenger).len(), 2, "iki oturum");

        // (1) İki eşin RECEIPT'i bekler; toplam iki eşinkidir.
        deliver_text(&mut messenger, h0, 10);
        deliver_text(&mut messenger, h1, 20);
        assert_eq!(
            drain_scripted(&mut messenger),
            vec![text_received(p0, 10), text_received(p1, 20)]
        );
        assert_eq!(
            (
                messenger.pending_receipts(p0),
                messenger.pending_receipts(p1)
            ),
            (1, 1)
        );
        assert_eq!(
            messenger.stats().receipts_pending,
            2,
            "receipts_pending iki eşin toplamı"
        );

        // (2) Yalnız p0'ın oturumu dolu: tek poll'da p1'inki kabul edilir, p0'ınki bekler.
        messenger.transport_mut().open(usize::MAX);
        messenger.transport_mut().full_sessions.insert(h0);
        assert_eq!(messenger.poll(), None);
        assert_eq!(
            accepted_receipt_ids(&messenger),
            vec![20],
            "p1'in RECEIPT'i p0'ın geri basıncına takılmaz"
        );
        assert_eq!(
            receipt_results(&messenger, 10).last(),
            Some(&Err(TransportError::Backpressure)),
            "p0'ın RECEIPT'i bu poll'da da sunuldu ve bekliyor"
        );
        assert_eq!(
            (
                messenger.pending_receipts(p0),
                messenger.pending_receipts(p1)
            ),
            (1, 0)
        );
        assert_eq!(messenger.stats().receipts_pending, 1);

        // (3) p1 oturum değiştirir; RECEIPT'siz TEXT'inin yeniden gönderimi reddedilir ve bekler.
        //     Sonra tek kabul yeri açılır: önce yeniden gönderim, sonra bekleyen RECEIPT sunulur.
        messenger.transport_mut().full_sessions.clear();
        let sent = messenger
            .send_text(p1, "metin")
            .expect("p1'in oturumu açık");
        messenger
            .transport_mut()
            .refuse(TransportError::Backpressure);
        let h1b = handle(3);
        let renewed = SessionEvent::Ready(h1b, p1);
        messenger.transport_mut().events.push_back(renewed);
        assert_eq!(messenger.poll(), Some(MessengerEvent::Session(renewed)));
        assert_eq!(
            messenger.stats().resend_refused,
            1,
            "yeniden gönderim reddedildi, bekliyor"
        );
        messenger.transport_mut().open(1);
        let before = messenger.transport().attempts.len();
        assert_eq!(messenger.poll(), None);
        let in_poll: Vec<(SessionHandle, Result<(), TransportError>)> =
            messenger.transport().attempts[before..]
                .iter()
                .map(|(on, _, result)| (*on, *result))
                .collect();
        assert_eq!(
            in_poll,
            vec![(h1b, Ok(())), (h0, Err(TransportError::Backpressure))],
            "poll sırası: önce p1'in yeniden gönderimi (kabul), sonra p0'ın RECEIPT'i \
             (Backpressure)"
        );
        let resent = &messenger.transport().attempts[before];
        assert!(
            matches!(
                codec::decode(&resent.1, messenger.limits()),
                Ok(AppMessage::Text { msg_id, .. }) if msg_id == sent
            ),
            "kabul edilen, p1'in RECEIPT'siz TEXT'inin ({sent}) yeniden gönderimi"
        );
        assert_eq!(messenger.stats().resent_on_new_session, 1);
        assert_eq!(
            messenger.pending_receipts(p0),
            1,
            "p0'ın RECEIPT'i bekliyor"
        );
    }
}
