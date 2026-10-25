//! Sinyalleşme (K4G-14a; plan K4G-14 NE (1)) — AMP `SIGNALING_BLOB` (0x09) gönder/al
//! (`specs/application-message-protocol.md` §3, K4G-14a yaması 2026-10-12;
//! `specs/signaling-seam-v0.md`).
//!
//! Sesli / görüntülü aramanın kurulum ve kontrol mesajları bu kanaldan geçer (PoC-KAPSAM-001 §2
//! "Sesli / görüntülü arama"). İçerik OPAKTIR: blob'u medya motoru üretir ve yorumlar (K7: içi
//! protobuf); bu modül onu okumaz, ayrıştırmaz, doğrulamaz — baytları kayıtta aynen taşır. Rust'ta
//! şema yoktur (plan D11: protobuf bağımlılığı eklenmez).
//!
//! [`Signaling`] [`Messenger`]'ın yanında durur ve Transport'u sahiplenmez:
//!
//! * **Gönderen** — [`Signaling::send`] blob'un boyunu kopyalamadan ÖNCE denetler, `SIGNALING_BLOB`
//!   kaydını eşin güncel oturumuna ([`Messenger::session_of`] + [`Messenger::transport_mut`])
//!   `DeliveryPolicy::Reliable` ile verir ve motor-tekil bir [`SignalingId`] döndürür. `Ok` =
//!   Transport kaydı kuyruğa aldı; teslim DEĞİL (S1 K8). 0x09'un alındısı yoktur: teslim öğrenilemez.
//! * **Alıcı** — [`Signaling::handle`] [`Messenger::poll`]'u OKUMAZ: olayları zincirdeki okuyucudan
//!   alır. `SIGNALING_BLOB` taşıyan [`MessengerEvent::Other`]'ı TÜKETİR ve yerine
//!   [`SignalingEvent::SignalingReceived`] döndürür; diğer her olayı [`SignalingEvent::Messenger`]
//!   ile AYNEN geçirir.
//!
//! Bir uygulama turu — `Ptt::poll` Messenger'ın tek okuyucusudur; bu motor zincirin son halkasıdır:
//!
//! ```text
//! while let Some(ev) = ptt.poll(&mut m) {
//!     PttEvent::Messenger(ev) → files.handle(&mut m, ev) → Some(ev) → signaling.handle(&mut m, ev)
//! }
//! files.pump(&mut m);
//! signaling.pump(&mut m);
//! while let Some(fe) = files.poll_event() { … }
//! ```
//!
//! **Çağrı sözleşmesi:** bir motor TEK bir Messenger'la kullanılır — [`Signaling::send`],
//! [`Signaling::handle`] ve [`Signaling::pump`]'a her çağrıda aynı Messenger verilir
//! (`FileTransfer::send_file`'daki `messenger` kuralıyla aynı). Yeniden sunum kaydı o Messenger'ın
//! oturum tutamaçlarına dayanır: başka bir Messenger'la yapılan çağrı kaydı yanlış Transport'un
//! oturumuna sunabilir. Bu sözleşme dışıdır; tablonun bellek tavanı o durumda da tutar ("Tablolar").
//!
//! # Zincire bağlanış
//!
//! [`Signaling::handle`], [`FileTransfer::handle`](crate::file_transfer::FileTransfer::handle) gibi
//! zincirden bir [`MessengerEvent`] alır ve yalnız kendi kaydını tüketir. Tükettiğinin karşılığını bir
//! kuyruğa koymaz, HEMEN döndürür ([`SignalingEvent`]; biçim `PttEvent::Messenger` sarmalayıcısıyla
//! aynı). Alıcı tarafında durum yoktur — birleştirme, dedup, zamanlayıcı yok —; her gelen blob tam bir
//! olaydır. Bir olay kuyruğu, gelen blob sayısını sınırlayan hiçbir mevcut tavana yaslanamazdı: yeni
//! bir sınır değeri gerekirdi ve taşmada blob kaybolurdu. Doğrudan dönüşte kuyruk, sayı ve kayıp
//! yoktur.
//!
//! # Teslim politikası ve boyut
//!
//! Her `SIGNALING_BLOB` `DeliveryPolicy::Reliable` ile gider (plan K4G-14; `BestEffort` /
//! `DeadlineBounded` canlı arama MEDYASININDIR, K4G-10) ve sıkıştırılmaz (AMP §5 kural 6; codec bugün
//! hiçbir tipi sıkıştırmaz). Boy sınırı A2'nin 0x09 karşılığıdır: [`max_blob_bytes`] =
//! `MAX_BODY_BYTES − SIGNALING_BLOB_PREFIX_BYTES` (`blob_len u16_be`, 2 B; spec'te adı yoktur — A2'nin
//! `MAX_FILE_CHUNK_BYTES` formülünün 0x09 karşılığı). Aşan blob [`SignalingError::TooLarge`] verir:
//! denetim blob kopyalanmadan ve kodlanmadan ÖNCE, oturum denetiminden de önce yapılır; Transport'a
//! hiçbir şey verilmez.
//!
//! # Dedup YOK (AMP §3, K4G-14a)
//!
//! 0x09 telde `msg_id` TAŞIMAZ (AMP §3: `blob_len u16_be · blob bytes[blob_len]`): alıcı tekrarı
//! tespit edemez ve her geçerli blob'u — aynı baytlarla yeniden gelen dahil — teslim eder.
//! Messenger'ın `msg_id` dedup penceresi bu tipe UYGULANMAZ. Yinelenen blob'u medya motoru idempotent
//! işler (signaling-seam-v0 §3: at-least-once, duplicate mümkün; sıra garanti edilmez). Alıcı, eşin
//! güncel oturumu olmasa da blob'u teslim eder — ör. eşin oturumu `Closed` ile bittikten sonra emekli
//! tutamaçtan gelen geç blob (supersession drain'inde ise eşin güncel oturumu VARDIR): içerik opaktır,
//! geçerliliğine medya motoru karar verir.
//!
//! # Oturum değişimi — yeniden sunum (S1 K7)
//!
//! Transport `h`'de teslim edilmemiş birimi `h'`ye TAŞIMAZ (S1 K7) ve 0x09'un alındısı yoktur:
//! gönderen bir blob'un eski oturumla yolda kalıp kalmadığını bilemez. Messenger'ın outbox'ı bu iş için
//! KULLANILMAZ (outbox `msg_id`'li TEXT tutar ve `RECEIPT(delivered)` ile kapanır). Motor bunun yerine
//! K4G-08'in yeniden `PTT_OPEN` desenini uygular:
//!
//! * Her blob, kaydı en son KABUL eden tutamaçla tutulur. Eşin güncel oturumu
//!   ([`Messenger::session_of`]) başka bir tutamaçsa kayıt oraya AYNI baytlarla, `Reliable`, BİR KEZ
//!   yeniden sunulur; kabul edilen kayıt o tutamaca bir daha verilmez — blob ve oturum başına en çok
//!   bir kabul. Transport geçici reddederse (`Backpressure`, eski tutamaç, `NotConnected`) tutamaç
//!   işaretlenmez ve kayıt sonraki denetimde yeniden sunulur (kabul edilmemiş birim tele çıkmamıştır;
//!   S1 K8 "bekler"); eşin sonraki kayıtları da sıralarını korumak için bekler. Kalıcı ret (diğer her
//!   hata) kaydı düşürür. Yeniden sunum aynı-oturum retry'ı DEĞİLDİR (S1 K8).
//! * Denetim üç yerde yapılır: [`Signaling::handle`] eşin `Ready` olayını geçirirken (olay uygulamaya
//!   verilmeden önce), her [`Signaling::send`]'de yeni blob'dan önce (yalnız o eş) ve her
//!   [`Signaling::pump`]'ta (bütün eşler). Yeni oturum `Ready` olayı okunmadan da benimsenebilir
//!   (senkron `Messenger::connect` → `ConnectResult::Ready`; olay kuyruğunda düşmüş `Ready`): yalnız
//!   olaya güvenilmez, `pump` trafik olmasa da periyodik çağrılmalıdır.
//! * Eşin oturumu kapanırsa (`Closed`) kayıt kalır ve eşin sıradaki oturumunda aynı kuralla sunulur.
//!
//! **Tutma kuralı — sahip kararı (DUR-K4G-14a CEVAP, 2026-10-12).** Plan metni kaydın NE ZAMAN
//! bırakılacağını tanımlamıyordu (alındı yok); karar: kayıt ilk kabulden itibaren
//! [`SIGNALING_RESEND_TTL`] (60 s) tutulur — yeniden sunum ömrü UZATMAZ, blob'un teslim çabası
//! gönderimden itibaren süreyle sınırlıdır (signaling-seam-v0 §3 "deadline-bounded reliable") —; eş
//! başına en çok [`SIGNALING_RESEND_PER_PEER`] (256), toplam en çok
//! `MAX_PEERS × SIGNALING_RESEND_PER_PEER` kayıt; eşin tavanı dolunca eşin KENDİ en eski kaydı yer
//! açar. Yeni gönderim tavan yüzünden REDDEDİLMEZ: onaylanamayan eski kayıtlar yeni sinyalleşmeyi
//! ömürleri boyunca durdurmamalıdır. Ömrü dolmuş ya da düşmüş blob yeniden sunulmaz; kaybını medya
//! motorunun kendi uygulama-ACK'i karşılar (signaling-seam-v0 §3). 120 s reddedildi: sinyalleşme
//! zamana duyarlıdır, 60 s'den eski bir teklifin yeniden sunumu bayat çağrı üretir. İki sabitin
//! değeri sahip kararıyla `FILE_RECEIPT_TTL` ve `MAX_OUTBOX_UNACKED` ile EŞİT SEÇİLDİ, onlara BAĞLI
//! DEĞİLDİR (sabit belgeleri). Yukarıdaki yeniden sunum mekanizması kartın "supersession'da yeniden
//! gönderim" maddesinin K4G-08 deseniyle uygulanmasıdır.
//!
//! Kayıt BELLEKTEDİR (plan D8: `AppStore` yalnız sayaç / dedup / outbox taşır): süreç yeniden başlarsa
//! tutulan blob'lar yeniden sunulmaz (PoC-KAPSAM-001 K2: sahada uygulama yeniden başlatılmaz). Kartın
//! "outbox üzerinden" ifadesinden fark budur: Messenger outbox'ı kalıcıdır (K4G-07), bu kayıt değildir.
//!
//! Süre olay üretmez ve zamanlayıcı yoktur: saat ([`MonotonicClock`]) yalnız gönderimde (ömrün başı),
//! yeniden sunum kararından önceki budamada (`send`, `pump`, `Ready`) ve
//! [`Signaling::pending_count`]'ta okunur. Ömrü dolmuş kayıt hiçbir denetimde sunulmaz; `pump`
//! çağrılmazsa yalnız belleği bir sonraki budamaya dek (tavanla sınırlı) tutar.
//!
//! # [`SignalingId`]
//!
//! Telde YOKTUR ve kalıcı DEĞİLDİR: motor-tekil, kesin artan yerel tutamaç. Yalnız Transport'un kabul
//! ettiği gönderime verilir (hata yolları kimlik tüketmez) ve hiçbir zaman yeniden verilmez — eşten,
//! oturumdan ve kaydın düşmesinden bağımsız (K4G-08/09 use-after-close dersi: eş başına sayaç
//! kapatılmış bir kimliği başka bir eşe yeniden verebilirdi). `u64` alanı tükenirse
//! [`SignalingError::IdExhausted`]; sarma YOK. Yeniden kurulan motorun kimlikleri baştan başlar:
//! kimlik telde yoktur ve hiçbir kalıcı kayda bağlanmaz (AppStore'a girmez).
//!
//! # Tablolar (SR-AUDIT-001)
//!
//! Tek tablo yeniden sunum kaydıdır: eş başına ≤ [`SIGNALING_RESEND_PER_PEER`], toplam ≤
//! `MAX_PEERS × SIGNALING_RESEND_PER_PEER` (türetilmiş). Çağrı sözleşmesine uyan motorda toplam zaten
//! aşılmaz — kayıtlı eşler tek Messenger'ın ≤ `MAX_PEERS` eşidir —; toplam denetimi, sözleşme
//! çiğnense de belleği sınırlı tutan savunmadır. Her kayıt ≤ `max_plaintext_unit_bytes` bayttır. Tek
//! ekleme noktasında sınır eklemeden ÖNCE denetlenir: önce süresi dolanlar budanır, sonra eşin kendi ya
//! da tablonun en eski kaydı yer açar. Boş eş girdisi tutulmaz. Alıcı tarafında tablo ve kuyruk YOKTUR.
//!
//! # Bilinen sınırlar (beyanlı)
//!
//! * Dedup yok: eski oturumda teslim edilmiş bir blob yeni oturumda yeniden sunulur ve alıcıda ikinci
//!   kez teslim edilir (medya motoru idempotent; signaling-seam-v0 §3).
//! * Teslim öğrenilemez (alındı yok): ömrü dolan ya da tavandan düşen, yolda kalmış blob kaybolur;
//!   medya motorunun uygulama-ACK'i ve kendi yeniden denemesi karşılar.
//! * Sıra garanti DEĞİLDİR: aynı oturumda `Reliable` birimler gönderim sırasıyla teslim edilir (S1 §4)
//!   ve yeniden sunum yeni blob'dan önce denenir; ama oturumlar arası ve yeniden sunumu GEÇİCİ
//!   reddedilmiş bir kayıttan sonra kabul edilen yeni blob onu geçebilir (signaling-seam-v0 §3: medya
//!   kendi sırasını tutar).
//! * Yeniden sunum yalnız `handle(Ready)` / `send` / `pump` denetimlerindedir: trafik olmasa da `pump`
//!   periyodik çağrılmalıdır.
//! * Yeniden sunum kaydı bellektedir: süreç yeniden başlarsa tutulan blob'lar yeniden sunulmaz
//!   (PoC-KAPSAM-001 K2; D8). Kartın "outbox üzerinden" ifadesinden fark: Messenger outbox'ı kalıcıdır.
//! * Bir motor tek bir Messenger'la kullanılır ("Çağrı sözleşmesi"); başka bir Messenger'la yeniden
//!   sunum doğru değildir.
//! * Ömrü (60 s) dolan blob, sonraki oturum ne zaman gelirse gelsin yeniden sunulmaz — sahip
//!   kararı (DUR-K4G-14a CEVAP, 2026-10-12): sinyalleşme zamana duyarlıdır, bayat teklif bayat
//!   çağrı üretir.
//!
//! # Gizlilik
//!
//! [`SignalingEvent`]'in `Debug` çıktısı blob'u değil uzunluğunu yazar; [`Signaling`] `Debug`
//! değildir (kayıtlar blob taşır); [`SignalingError`] blob taşımaz. Bu modül log yazmaz.

use core::fmt;
use core::time::Duration;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent, TransportError,
};

use crate::clock::MonotonicClock;
use crate::codec::limits::SIGNALING_BLOB_PREFIX_BYTES;
use crate::codec::{self, AmpLimits, AppMessage, EncodeError};
use crate::messaging::{Messenger, MessengerEvent, MAX_PEERS};

/// Motor-tekil, kesin artan yerel blob tutamacı ([`Signaling::send`]'in dönüşü). Telde YOKTUR,
/// kalıcı DEĞİLDİR ve hiçbir zaman yeniden verilmez (modül belgesi, "`SignalingId`").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SignalingId(u64);

impl SignalingId {
    /// Ham değer — UI / FFI eşlemesi içindir (K4G-14b / K4G-15). Telde karşılığı yoktur.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Taze motorun ilk [`SignalingId`]'si. Değer bu uygulamanındır (spec'te kimlik yok).
pub const FIRST_SIGNALING_ID: SignalingId = SignalingId(0);

/// Yeniden sunum kaydının ömrü — kaydın ilk KABUL anından itibaren; yeniden sunum uzatmaz.
///
/// **Sahip kararı (DUR-K4G-14a CEVAP, 2026-10-12): 60 s**, ilk kabulden. Değer
/// [`FILE_RECEIPT_TTL`](crate::file_transfer::FILE_RECEIPT_TTL) ile EŞİT SEÇİLDİ (plan D6:
/// kama-app'te gönderenin onaysız kalmış bir gönderimle ilgilenmeyi bıraktığı süre), ona BAĞLI
/// DEĞİLDİR: sabit takma ad değil, kendi değeriyle yazılır — `FILE_RECEIPT_TTL` değişirse
/// sinyalleşme sessizce değişmemelidir. 120 s reddedildi: sinyalleşme zamana duyarlıdır, 60 s'den
/// eski bir teklifin yeniden sunumu bayat çağrı üretir.
pub const SIGNALING_RESEND_TTL: Duration = Duration::from_secs(60);

/// Eş başına yeniden sunum kaydı tavanı; dolunca eşin KENDİ en eski kaydı yer açar.
///
/// **Sahip kararı (DUR-K4G-14a CEVAP, 2026-10-12): 256**; taşmada en eski düşer, yeni gönderim
/// reddedilmez. Değer [`MAX_OUTBOX_UNACKED`](crate::messaging::MAX_OUTBOX_UNACKED) ile EŞİT SEÇİLDİ
/// (S7: eş başına Transport'un kabul ettiği ama onayı gelmemiş kayıtların tavanı, TEXT), ona BAĞLI
/// DEĞİLDİR: sabit takma ad değil, kendi değeriyle yazılır — `MAX_OUTBOX_UNACKED` değişirse
/// sinyalleşme sessizce değişmemelidir. Bellek profili Messenger outbox'ıyla aynıdır (kayıt ≤
/// `max_plaintext_unit_bytes`).
pub const SIGNALING_RESEND_PER_PEER: usize = 256;

/// Yeniden sunum kaydının toplam tavanı — türetilmiş: eş tablosu tavanı × eş başına tavan (yeni sayı
/// yok). Çağrı sözleşmesine uyan motorda (tek Messenger) kayıtlı eşler o Messenger'ın ≤ `MAX_PEERS`
/// eşidir ve tavan zaten aşılmaz; denetim, sözleşme çiğnense de belleği sınırlı tutan savunmadır.
const MAX_SIGNALING_RECORDS: usize = MAX_PEERS * SIGNALING_RESEND_PER_PEER;

// Tavan sıfır olsaydı her yeni kayıt kendini tahliye ederdi.
const _: () = assert!(SIGNALING_RESEND_PER_PEER > 0);

/// Bir `SIGNALING_BLOB`'un taşıyabileceği en büyük blob (bayt): `MAX_BODY_BYTES −
/// SIGNALING_BLOB_PREFIX_BYTES` (AMP §3 0x09 + §4 A2; spec'te adı yoktur). Medya motoru blob boyunu
/// buna göre seçer; aşan blob [`Signaling::send`]'de [`SignalingError::TooLarge`] verir.
#[must_use]
pub const fn max_blob_bytes(limits: &AmpLimits) -> usize {
    // `AmpLimits`, `MAX_BODY_BYTES ≥ FILE_CHUNK_PREFIX_BYTES (18) > SIGNALING_BLOB_PREFIX_BYTES (2)`
    // garanti eder; çıkarma taşmaz, yine de doyurulur (panik yok).
    limits
        .max_body_bytes()
        .saturating_sub(SIGNALING_BLOB_PREFIX_BYTES)
}

/// [`Signaling::handle`] çıktısı.
#[derive(Clone, PartialEq, Eq)]
pub enum SignalingEvent {
    /// `SIGNALING_BLOB` taşımayan [`Messenger`] olayı — AYNEN iletilir (oturum olayları, metin ve
    /// alındı, zincirin önceki halkalarının geçirdiği kayıtlar).
    Messenger(MessengerEvent),
    /// Eşten opak bir `SIGNALING_BLOB` geldi. Dedup YOKTUR: aynı blob iki kez gelebilir (modül
    /// belgesi). İçerik medya motorunundur.
    SignalingReceived {
        /// Blob'u gönderen eş.
        peer: PeerRef,
        /// `SIGNALING_BLOB.blob`, aynen.
        bytes: Vec<u8>,
    },
}

/// Blob YAZILMAZ; yalnız uzunluğu. `PeerRef` ve `MessengerEvent` kendi `Debug`'larıyla (ikisi de
/// içerik basmaz).
impl fmt::Debug for SignalingEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Messenger(event) => f.debug_tuple("Messenger").field(event).finish(),
            Self::SignalingReceived { peer, bytes } => f
                .debug_struct("SignalingReceived")
                .field("peer", peer)
                .field("bytes_len", &bytes.len())
                .finish_non_exhaustive(),
        }
    }
}

/// [`Signaling::send`] hatası. Her hata yolunda blob Transport'a verilmez (Transport'un kendi reddi
/// hariç), kayda girmez ve [`SignalingId`] tüketilmez. Motor reddedilen blob'u yeniden denemez (S1
/// K8): çağıran bekler ve aynı blob'u yeniden verir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalingError {
    /// Blob bir `SIGNALING_BLOB` kaydına sığmıyor ([`max_blob_bytes`]; AMP §4 A2). Blob kopyalanmadan
    /// ve oturum denetiminden önce denetlenir.
    TooLarge {
        /// Verilen blob'un uzunluğu.
        blob_len: usize,
        /// Bu limitlerdeki en büyük blob.
        max_blob_len: usize,
    },
    /// Eşin hazır oturumu yok ([`Messenger::session_of`] → `None`): eş hiç görülmedi, `Ready`
    /// görülmedi ya da `Closed` okundu. Motor blob'u kuyruğa ALMAZ; oturum `Messenger::connect` ile
    /// istenir.
    NoSession,
    /// Motorun `u64` [`SignalingId`] alanı tükendi; sarma YOK.
    IdExhausted,
    /// Kayıt kodlanamadı. Savunmadır: boyut ön denetiminden sonra beklenmez.
    Encode(EncodeError),
    /// Transport ya da Messenger'ın RECEIPT öncelik kapısı (K4G-05d, `Backpressure`) reddetti;
    /// kayıt kuyruğa ALINMADI (S1 §3). Yeniden deneme kararı çağıranındır.
    Transport(TransportError),
}

impl fmt::Display for SignalingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge {
                blob_len,
                max_blob_len,
            } => write!(
                f,
                "SIGNALING_BLOB of {blob_len} bytes exceeds {max_blob_len} bytes"
            ),
            Self::NoSession => f.write_str("peer has no ready session"),
            Self::IdExhausted => {
                f.write_str("SignalingId space of this engine is exhausted (no wrap)")
            }
            Self::Encode(e) => write!(f, "SIGNALING_BLOB does not fit one AMP record: {e}"),
            Self::Transport(e) => write!(f, "transport refused the record: {e}"),
        }
    }
}

impl std::error::Error for SignalingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::TooLarge { .. } | Self::NoSession | Self::IdExhausted => None,
        }
    }
}

/// Gözlem sayaçları — salt-okunur; protokol davranışını değiştirmez.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SignalingStats {
    /// Transport'un kabul ettiği yeni blob'lar ([`Signaling::send`] → `Ok`).
    pub blobs_sent: u64,
    /// [`SignalingEvent::SignalingReceived`] olarak verilen gelen blob'lar (tekrarlar dahil — dedup
    /// yok).
    pub blobs_received: u64,
    /// Eşin yeni oturumuna Transport'un kabul ettiği yeniden sunumlar — blob ve oturum başına en çok
    /// bir.
    pub resent_on_new_session: u64,
    /// Transport'un ya da Messenger'ın RECEIPT öncelik kapısının (K4G-05d) GEÇİCİ reddettiği
    /// yeniden sunum denemeleri (kayıt kalır, sonraki denetimde yeniden sunulur).
    pub resend_refused: u64,
    /// [`SIGNALING_RESEND_TTL`] dolduğu için budanan kayıtlar.
    pub records_expired: u64,
    /// Tavan dolu olduğu için en eski olarak düşen kayıtlar (eş başına ya da toplam).
    pub records_evicted: u64,
    /// Yeniden sunumu Transport'un KALICI reddettiği için düşen kayıtlar.
    pub records_failed: u64,
}

/// K4G-14a sinyalleşme motoru. Ayrıntı: modül belgesi. Bir motor TEK bir Messenger'la kullanılır:
/// [`Signaling::send`], [`Signaling::handle`] ve [`Signaling::pump`]'a her çağrıda aynı Messenger
/// verilir (modül belgesi, "Çağrı sözleşmesi").
pub struct Signaling {
    /// [`SIGNALING_RESEND_TTL`]'nin zaman kaynağı.
    clock: Arc<dyn MonotonicClock>,
    /// Sıradaki [`SignalingId`]'nin ham değeri; `None` = `u64` alanı tükendi.
    next_id: Option<u64>,
    /// Yeniden sunum kaydı: eş → kayıtlar, gönderim sırasıyla (eş başına ≤
    /// [`SIGNALING_RESEND_PER_PEER`], toplam ≤ [`MAX_SIGNALING_RECORDS`]; boş girdi tutulmaz).
    records: BTreeMap<PeerRef, VecDeque<Record>>,
    stats: SignalingStats,
}

/// Yeniden sunum için tutulan, Transport'un kabul ettiği blob kaydı.
struct Record {
    /// Kaydın kimliği (genel gönderim sırası; en eskiyi seçmek için).
    id: SignalingId,
    /// Kodlanmış `SIGNALING_BLOB` kaydı: yeniden sunum AYNI baytları taşır.
    unit: PreparedUnit,
    /// Kaydı en son KABUL eden tutamaç; eşin güncel oturumu başkaysa kayıt oraya bir kez sunulur.
    sent_on: SessionHandle,
    /// İlk kabul anı (saatin `now()`'ı) — ömrün başı; yeniden sunum değiştirmez.
    sent_at: Duration,
}

impl Signaling {
    /// Boş motor: kayıt yok, ilk kimlik [`FIRST_SIGNALING_ID`]. `clock` yeniden sunum kaydının ömür
    /// kaynağıdır — ürün yolu [`SystemClock`](crate::clock::SystemClock)'u verir (bileşim kökü),
    /// testler elle ilerletilen bir saat.
    #[must_use]
    pub fn new(clock: Arc<dyn MonotonicClock>) -> Self {
        Self {
            clock,
            next_id: Some(FIRST_SIGNALING_ID.0),
            records: BTreeMap::new(),
            stats: SignalingStats::default(),
        }
    }

    /// Gözlem sayaçları.
    #[must_use]
    pub fn stats(&self) -> SignalingStats {
        self.stats
    }

    /// Eşe yeniden sunum için tutulan, ömrü dolmamış kayıt sayısı (≤ [`SIGNALING_RESEND_PER_PEER`]).
    #[must_use]
    pub fn pending_count(&self, peer: PeerRef) -> usize {
        let now = self.clock.now();
        self.records.get(&peer).map_or(0, |queue| {
            queue.iter().filter(|record| alive(record, now)).count()
        })
    }

    /// Opak blob'u `peer`'e gönderir ve motor-tekil [`SignalingId`]'yi döndürür. `Ok` = Transport
    /// `SIGNALING_BLOB` kaydını `DeliveryPolicy::Reliable` ile kuyruğa aldı; teslim DEĞİL (S1 K8) —
    /// alındı yoktur. Kayıt sıkıştırılmaz. Eşin yeniden sunulacak kayıtları yeni blob'dan ÖNCE eşin
    /// güncel oturumuna sunulur (modül belgesi, "Oturum değişimi"); onların reddi bu gönderimi
    /// durdurmaz. Kabul edilen blob yeniden sunum kaydına girer. `messenger` motorun her çağrıda
    /// kullandığı TEK Messenger'dır (modül belgesi, "Çağrı sözleşmesi").
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`SignalingError::TooLarge`] (blob kopyalanmadan önce) ·
    /// [`SignalingError::NoSession`] · [`SignalingError::IdExhausted`] — üçünde de hiçbir şey
    /// gönderilmez · [`SignalingError::Encode`] · [`SignalingError::Transport`]. Hiçbirinde blob kayda
    /// girmez ve kimlik tüketilmez.
    pub fn send<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        peer: PeerRef,
        bytes: &[u8],
    ) -> Result<SignalingId, SignalingError> {
        let max_blob_len = max_blob_bytes(messenger.limits());
        if bytes.len() > max_blob_len {
            return Err(SignalingError::TooLarge {
                blob_len: bytes.len(),
                max_blob_len,
            });
        }
        let handle = messenger
            .session_of(peer)
            .ok_or(SignalingError::NoSession)?;
        let raw = self.next_id.ok_or(SignalingError::IdExhausted)?;
        let now = self.clock.now();
        self.refresh(messenger, Some(peer), now);
        let message = AppMessage::SignalingBlob {
            blob: bytes.to_vec(),
        };
        let unit = codec::encode(&message, messenger.limits()).map_err(SignalingError::Encode)?;
        // K4G-05d M2: sinyalleşme toplu üreticidir; RECEIPT öncelik kapısından geçer.
        messenger
            .send_gated(handle, unit.clone(), DeliveryPolicy::Reliable)
            .map_err(SignalingError::Transport)?;
        // Kabul edildi: kimlik ancak şimdi tüketilir; `u64` sonu kalıcı tükenmedir (sarma yok).
        let id = SignalingId(raw);
        self.next_id = raw.checked_add(1);
        self.stats.blobs_sent += 1;
        self.remember(
            peer,
            Record {
                id,
                unit,
                sent_on: handle,
                sent_at: now,
            },
        );
        Ok(id)
    }

    /// Zincirden gelen bir [`MessengerEvent`]'i işler: `SIGNALING_BLOB` taşıyan `Other`'ı TÜKETİR ve
    /// [`SignalingEvent::SignalingReceived`] döndürür (dedup yok); diğer her olayı
    /// [`SignalingEvent::Messenger`] ile AYNEN geçirir. Eşin `Ready` olayı geçirilmeden önce eşin
    /// yeniden sunulacak kayıtları yeni oturuma sunulur (modül belgesi, "Oturum değişimi").
    #[must_use]
    pub fn handle<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        event: MessengerEvent,
    ) -> SignalingEvent {
        match event {
            MessengerEvent::Other {
                peer,
                message: AppMessage::SignalingBlob { blob },
            } => {
                self.stats.blobs_received += 1;
                SignalingEvent::SignalingReceived { peer, bytes: blob }
            }
            MessengerEvent::Session(SessionEvent::Ready(handle, peer)) => {
                // Messenger yeni oturumu benimsedi; kayıtlar, olay uygulamaya verilmeden (uygulama o
                // oturumdan yeni blob göndermeden) önce sunulur.
                let now = self.clock.now();
                self.refresh(messenger, Some(peer), now);
                SignalingEvent::Messenger(MessengerEvent::Session(SessionEvent::Ready(
                    handle, peer,
                )))
            }
            other => SignalingEvent::Messenger(other),
        }
    }

    /// Turun denetimi: ömrü dolan kayıtları budar ve her eşin kayıtlarını, eşin güncel oturumu onları
    /// henüz kabul etmediyse oraya sunar (modül belgesi, "Oturum değişimi"). Olay üretmez.
    ///
    /// Trafik olmasa da periyodik çağrılmalıdır: `Ready` olayı okunmadan benimsenen oturuma (senkron
    /// `Messenger::connect` → `ConnectResult::Ready`; olay kuyruğunda düşmüş `Ready`) ve geçici
    /// reddedilmiş yeniden sunumlara tek güvence budur.
    pub fn pump<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>) {
        let now = self.clock.now();
        self.refresh(messenger, None, now);
    }

    /// Ömrü dolan kayıtları budar, sonra `only` eşinin (`None` = bütün eşlerin) kayıtlarını eşin
    /// güncel oturumuna sunar.
    fn refresh<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        only: Option<PeerRef>,
        now: Duration,
    ) {
        let Self { records, stats, .. } = self;
        records.retain(|_, queue| {
            // Eşin kayıtları gönderim sırasıyla durur ve saat geri gitmez: ömrü dolanlar baştadır.
            while queue.front().is_some_and(|record| !alive(record, now)) {
                queue.pop_front();
                stats.records_expired += 1;
            }
            !queue.is_empty()
        });
        match only {
            Some(peer) => {
                if let Some(queue) = records.get_mut(&peer) {
                    offer_queue(messenger, peer, queue, stats);
                    if queue.is_empty() {
                        records.remove(&peer);
                    }
                }
            }
            None => {
                for (&peer, queue) in records.iter_mut() {
                    offer_queue(messenger, peer, queue, stats);
                }
                records.retain(|_, queue| !queue.is_empty());
            }
        }
    }

    /// Kabul edilen blob'u yeniden sunum kaydına yazar. Sınır eklemeden ÖNCE: eşin kaydı doluysa
    /// eşin en eski kaydı, tablo doluysa tablonun en eski kaydı yer açar (süresi dolanlar bu çağrıdan
    /// önce `refresh`'te budandı).
    fn remember(&mut self, peer: PeerRef, record: Record) {
        let peer_len = self.records.get(&peer).map_or(0, VecDeque::len);
        if peer_len >= SIGNALING_RESEND_PER_PEER {
            if let Some(queue) = self.records.get_mut(&peer) {
                queue.pop_front();
            }
            self.stats.records_evicted += 1;
        } else if self.records.values().map(VecDeque::len).sum::<usize>() >= MAX_SIGNALING_RECORDS {
            self.evict_oldest();
        }
        self.records.entry(peer).or_default().push_back(record);
    }

    /// Tablonun en eski kaydını (en küçük kimlik) düşürür; boşalan eş girdisi kalkar.
    fn evict_oldest(&mut self) {
        let oldest = self
            .records
            .iter()
            .filter_map(|(&peer, queue)| queue.front().map(|record| (record.id, peer)))
            .min();
        let Some((_, peer)) = oldest else {
            return;
        };
        if let Some(queue) = self.records.get_mut(&peer) {
            queue.pop_front();
            if queue.is_empty() {
                self.records.remove(&peer);
            }
        }
        self.stats.records_evicted += 1;
    }
}

/// Kayıt ömrü içinde mi: ilk kabulden bu yana [`SIGNALING_RESEND_TTL`]'den az geçti.
fn alive(record: &Record, now: Duration) -> bool {
    now.saturating_sub(record.sent_at) < SIGNALING_RESEND_TTL
}

/// Eşin kayıtlarını, gönderim sırasıyla, eşin güncel oturumuna sunar: oturumu bu kaydı zaten kabul
/// etmişse atlar; kabulde tutamacı işaretler (o tutamaca bir daha sunulmaz); geçici retle durur —
/// kayıt ve eşin sonraki kayıtları sonraki denetimi bekler; kalıcı retteki kaydı düşürür. Eşin güncel
/// oturumu yoksa hiçbir şey yapmaz.
fn offer_queue<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    peer: PeerRef,
    queue: &mut VecDeque<Record>,
    stats: &mut SignalingStats,
) {
    let Some(current) = messenger.session_of(peer) else {
        return;
    };
    let mut index = 0;
    while let Some(record) = queue.get_mut(index) {
        if record.sent_on == current {
            index += 1;
            continue;
        }
        // K4G-05d M2: yeniden sunum da RECEIPT öncelik kapısından geçer.
        match messenger.send_gated(current, record.unit.clone(), DeliveryPolicy::Reliable) {
            Ok(()) => {
                record.sent_on = current;
                stats.resent_on_new_session += 1;
                index += 1;
            }
            Err(error) if is_transient(error) => {
                stats.resend_refused += 1;
                return;
            }
            Err(_) => {
                // Kalıcı ret: bu Transport kaydı hiçbir oturumda taşımaz.
                queue.remove(index);
                stats.records_failed += 1;
            }
        }
    }
}

/// Geçici ret: kayıt kuyruğa alınmadı ve sonraki sunumda yeniden sunulur (S1 §3; K8 "bekler"). Diğer
/// her hata kalıcı sayılır (fail-closed). Sınıflama `file_transfer`'deki alındı / kayıt sunumuyla
/// aynıdır.
const fn is_transient(error: TransportError) -> bool {
    matches!(
        error,
        TransportError::Backpressure
            | TransportError::UnknownSession
            | TransportError::SessionSuperseded
            | TransportError::NotConnected
    )
}

#[cfg(test)]
mod tests {
    //! Kimlik tükenmesi (denetim sırası dahil), yeniden sunum kaydının tavanları (eş başına — eşin
    //! kendi en eskisi — ve toplam), Transport ret sınıflaması (`TransportError`'ın bütün türleri),
    //! `pump`'ın bütün eşleri denetlemesi, `handle` geçirgenliği ve `Debug` — uçtan uca akış
    //! (gidiş-dönüş, boyut, oturum değişimi, ömür, dedup yokluğu) `tests/signaling.rs`'te.
    //!
    //! Her testin belgesi, test mantığının `ea0d3909`'daki (K4G-14a öncesi main) sınıfını yazar
    //! (KIRMIZI (uyarlansa da) / UYARLANAMAZ / AYNI DAVRANIŞ; `tests/signaling.rs` başlığı).

    use super::*;
    use crate::clock::SystemClock;
    use crate::codec::{PttCodec, ReceiptKind};
    use crate::store::MemoryAppStore;
    use kama_contracts::s1::{ConnectResult, ConnectTicket, Delivered, TransportCapabilities};

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;

    /// Her `connect`'e yeni bir hazır oturumla yanıt veren (aynı eşe yeniden `connect` = yeni
    /// tutamaç: senkron oturum değişimi), kabul ettiği gönderimlerin tutamacını kaydeden, hiçbir şey
    /// teslim etmeyen Transport. `refuse`'daki hatalar sıradaki gönderimlere birer birer verilir (tek
    /// atımlık); reddedilen gönderim kaydedilmez.
    #[derive(Default)]
    struct TestTransport {
        last_handle: u64,
        sent_on: Vec<SessionHandle>,
        refuse: VecDeque<TransportError>,
    }

    impl KamaTransport for TestTransport {
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
            session: SessionHandle,
            _unit: PreparedUnit,
            _policy: DeliveryPolicy,
        ) -> Result<(), TransportError> {
            if let Some(error) = self.refuse.pop_front() {
                return Err(error);
            }
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

    fn clock() -> Arc<dyn MonotonicClock> {
        Arc::new(SystemClock::new())
    }

    /// `n`'den türetilmiş sentetik eş (ilk sekiz bayt `n`, BE).
    fn peer(n: usize) -> PeerRef {
        let mut dest_hash = [0u8; 32];
        let index = u64::try_from(n).expect("test sayısı u64'e sığar");
        dest_hash[..8].copy_from_slice(&index.to_be_bytes());
        PeerRef::from_dest_hash(dest_hash)
    }

    /// Verilen her eşle hazır oturumu olan, taze bellek içi depolu motor.
    fn messenger_with(peers: impl IntoIterator<Item = PeerRef>) -> Messenger<TestTransport> {
        let mut messenger =
            Messenger::with_store(TestTransport::default(), Arc::new(MemoryAppStore::new()))
                .expect("limitler geçerli");
        for p in peers {
            assert!(matches!(messenger.connect(p), Ok(ConnectResult::Ready(_))));
        }
        messenger
    }

    /// Eşe senkron oturum değişimi: `Messenger::connect` yeni tutamacı olaysız benimser.
    fn reconnect(messenger: &mut Messenger<TestTransport>, p: PeerRef) -> SessionHandle {
        match messenger.connect(p) {
            Ok(ConnectResult::Ready(handle)) => handle,
            other => panic!("yeni hazır oturum bekleniyordu: {other:?}"),
        }
    }

    /// Eşin kayıtlarının kimlikleri, kayıt sırasıyla.
    fn ids_of(signaling: &Signaling, p: PeerRef) -> Vec<u64> {
        signaling
            .records
            .get(&p)
            .map(|queue| queue.iter().map(|record| record.id.get()).collect())
            .unwrap_or_default()
    }

    fn total(signaling: &Signaling) -> usize {
        signaling.records.values().map(VecDeque::len).sum()
    }

    /// `u64` kimlik alanının sonu: son değer verilir, sonra her gönderim tipli `IdExhausted` alır —
    /// sarma yok, kimlik yeniden verilmez. Denetim sırası: `IdExhausted` eşin bekleyen yeniden
    /// sunumundan ÖNCE döner — yeniden sunulacak bir kayıt varken de hiçbir şey gönderilmez; kayıt
    /// sonraki `pump`'ta sunulur.
    ///
    /// `ea0d3909`'da UYARLANAMAZ: `SignalingId` ve `Signaling` motoru yoktu.
    #[test]
    fn signaling_ids_never_wrap_and_exhaustion_is_typed() {
        let p = peer(0);
        let mut messenger = messenger_with([p]);
        let mut signaling = Signaling::new(clock());
        signaling.next_id = Some(u64::MAX);
        let last = signaling
            .send(&mut messenger, p, b"son")
            .expect("son kimlik kullanılabilir");
        assert_eq!(last.get(), u64::MAX);
        let second = reconnect(&mut messenger, p);
        let sent = messenger.transport().sent_on.len();
        for _ in 0..2 {
            assert_eq!(
                signaling.send(&mut messenger, p, b"fazla"),
                Err(SignalingError::IdExhausted),
                "tükenme kalıcıdır"
            );
        }
        assert_eq!(
            messenger.transport().sent_on.len(),
            sent,
            "bekleyen yeniden sunum da gitmedi"
        );
        assert_eq!(ids_of(&signaling, p), vec![u64::MAX]);
        assert_eq!(signaling.stats().blobs_sent, 1);
        signaling.pump(&mut messenger);
        assert_eq!(
            messenger.transport().sent_on.last(),
            Some(&second),
            "kayıt pump'ta sunuldu"
        );
        assert_eq!(signaling.stats().resent_on_new_session, 1);
    }

    /// Yeniden sunum kaydı eş başına [`SIGNALING_RESEND_PER_PEER`] (dolunca eşin EN ESKİ kaydı
    /// düşer) ve toplam [`MAX_SIGNALING_RECORDS`] (dolunca tablonun EN ESKİ kaydı düşer, başka
    /// eşinki olsa da) ile sınırlıdır; yeni gönderim hiçbir durumda reddedilmez. Toplam tavana çağrı
    /// sözleşmesine uyan motorda erişilemez (tek Messenger ≤ `MAX_PEERS` eş): (b) onu sözleşme DIŞI
    /// bir kullanımla — motor iki Messenger'la — yalnız BELLEK sınırı olarak sınar; yeniden sunum
    /// sınanmaz.
    ///
    /// `ea0d3909`'da UYARLANAMAZ: yeniden sunum kaydı yoktu.
    #[test]
    fn resend_records_are_bounded_per_peer_and_in_total() {
        // (a) Eş başına tavan.
        let p = peer(0);
        let mut messenger = messenger_with([p]);
        let mut signaling = Signaling::new(clock());
        for _ in 0..=SIGNALING_RESEND_PER_PEER {
            signaling
                .send(&mut messenger, p, b"b")
                .expect("tavan gönderimi durdurmaz");
        }
        assert_eq!(signaling.pending_count(p), SIGNALING_RESEND_PER_PEER);
        let per_peer = u64::try_from(SIGNALING_RESEND_PER_PEER).expect("küçük sayı");
        let expected: Vec<u64> = (1..=per_peer).collect();
        assert_eq!(ids_of(&signaling, p), expected, "en eski (0) düştü");
        assert_eq!(signaling.stats().records_evicted, 1);
        assert_eq!(signaling.stats().blobs_sent, per_peer + 1);

        // (b) Toplam tavan (sözleşme dışı kullanım; yalnız bellek sınırı): `MAX_PEERS` eş × eş başına
        // tavan, sonra ikinci bir Messenger'daki yeni bir eş.
        let mut full = messenger_with((0..MAX_PEERS).map(peer));
        let extra = peer(MAX_PEERS);
        let mut other = messenger_with([extra]);
        let mut signaling = Signaling::new(clock());
        for _ in 0..SIGNALING_RESEND_PER_PEER {
            for n in 0..MAX_PEERS {
                signaling
                    .send(&mut full, peer(n), b"b")
                    .expect("tavanın altında");
            }
        }
        assert_eq!(total(&signaling), MAX_SIGNALING_RECORDS);
        assert_eq!(signaling.stats().records_evicted, 0);
        let first_of_peer0 = ids_of(&signaling, peer(0))[0];
        assert_eq!(first_of_peer0, FIRST_SIGNALING_ID.get());
        let id = signaling
            .send(&mut other, extra, b"b")
            .expect("toplam tavan da gönderimi durdurmaz");
        assert_eq!(total(&signaling), MAX_SIGNALING_RECORDS);
        assert_eq!(signaling.stats().records_evicted, 1);
        assert_eq!(ids_of(&signaling, extra), vec![id.get()]);
        assert_eq!(
            signaling.pending_count(peer(0)),
            SIGNALING_RESEND_PER_PEER - 1,
            "tablonun en eskisi eş 0'ın ilk kaydıydı"
        );
        assert_eq!(
            ids_of(&signaling, peer(0))[0],
            first_of_peer0 + u64::try_from(MAX_PEERS).expect("küçük sayı"),
            "eş 0'ın ikinci kaydı (ilk turdan sonraki) şimdi en eski"
        );
        for n in 1..MAX_PEERS {
            assert_eq!(signaling.pending_count(peer(n)), SIGNALING_RESEND_PER_PEER);
        }
    }

    /// Eş başına tavan eşin KENDİ en eski kaydını düşürür, tablonun en eskisini (başka bir eşinki)
    /// değil: `q` ilk kaydı (genelin en eskisi) yazar, sonra `p` tavan + 1 kayıt yazar — `p`'nin ilk
    /// kaydı düşer, `q`'nunki kalır.
    ///
    /// `ea0d3909`'da UYARLANAMAZ: yeniden sunum kaydı yoktu.
    #[test]
    fn a_full_peer_evicts_its_own_oldest_record_not_another_peers() {
        let (p, q) = (peer(0), peer(1));
        let mut messenger = messenger_with([p, q]);
        let mut signaling = Signaling::new(clock());
        let oldest = signaling.send(&mut messenger, q, b"q").expect("gönderilir");
        let mut sent_to_p = Vec::new();
        for _ in 0..=SIGNALING_RESEND_PER_PEER {
            let id = signaling
                .send(&mut messenger, p, b"p")
                .expect("tavan gönderimi durdurmaz");
            sent_to_p.push(id.get());
        }
        assert_eq!(
            ids_of(&signaling, q),
            vec![oldest.get()],
            "başka eşin kaydı (genelin en eskisi) kaldı"
        );
        assert_eq!(
            ids_of(&signaling, p),
            sent_to_p[1..].to_vec(),
            "p'nin kendi en eskisi düştü"
        );
        assert_eq!(signaling.stats().records_evicted, 1);
    }

    /// Ret sınıflaması, `TransportError`'ın BÜTÜN türleriyle: yeniden sunum GEÇİCİ bir türle
    /// (`Backpressure`, `SessionSuperseded`, `UnknownSession`, `NotConnected`) reddedilirse kayıt
    /// KALIR (`resend_refused`) ve sonraki denetimde TAM BİR KEZ kabul edilir; KALICI bir türle
    /// (`UnitTooLarge`, `DeadlineExpired`, `EarlyUnitConflict`) reddedilirse kayıt düşer
    /// (`records_failed`), boş eş girdisi tutulmaz ve sonraki oturuma bir daha sunulmaz. Beklenen
    /// sınıf testte bağımsız, joker kolsuz bir `match` ile yazılıdır: yeni bir tür eklenirse test
    /// derlenmez. Oturum değişimi senkron `connect` ile (olay yok).
    ///
    /// `ea0d3909`'da UYARLANAMAZ: yeniden sunum yoktu.
    #[test]
    fn transient_refusals_keep_the_record_and_permanent_ones_drop_it() {
        const ALL: [TransportError; 7] = [
            TransportError::UnknownSession,
            TransportError::SessionSuperseded,
            TransportError::EarlyUnitConflict,
            TransportError::UnitTooLarge,
            TransportError::Backpressure,
            TransportError::DeadlineExpired,
            TransportError::NotConnected,
        ];
        let expected_transient = |error: TransportError| match error {
            TransportError::Backpressure
            | TransportError::SessionSuperseded
            | TransportError::UnknownSession
            | TransportError::NotConnected => true,
            TransportError::UnitTooLarge
            | TransportError::DeadlineExpired
            | TransportError::EarlyUnitConflict => false,
        };
        for error in ALL {
            let p = peer(0);
            let mut messenger = messenger_with([p]);
            let mut signaling = Signaling::new(clock());
            signaling.send(&mut messenger, p, b"b").expect("gönderilir");
            let first = messenger.session_of(p).expect("oturum");
            let second = reconnect(&mut messenger, p);
            messenger.transport_mut().refuse.push_back(error);
            signaling.pump(&mut messenger);
            let stats = signaling.stats();
            if expected_transient(error) {
                assert_eq!(
                    (stats.resend_refused, stats.records_failed),
                    (1, 0),
                    "{error:?}"
                );
                assert_eq!(signaling.pending_count(p), 1, "{error:?}: kayıt kaldı");
                signaling.pump(&mut messenger);
                signaling.pump(&mut messenger);
                assert_eq!(
                    messenger.transport().sent_on,
                    vec![first, second],
                    "{error:?}: sonraki denetimde tam bir kabul"
                );
                assert_eq!(signaling.stats().resent_on_new_session, 1, "{error:?}");
            } else {
                assert_eq!(
                    (stats.resend_refused, stats.records_failed),
                    (0, 1),
                    "{error:?}"
                );
                assert_eq!(signaling.pending_count(p), 0, "{error:?}");
                assert!(
                    signaling.records.is_empty(),
                    "{error:?}: boş eş girdisi tutulmaz"
                );
                reconnect(&mut messenger, p);
                signaling.pump(&mut messenger);
                assert_eq!(
                    messenger.transport().sent_on,
                    vec![first],
                    "{error:?}: düşen kayıt bir daha sunulmadı"
                );
            }
        }
    }

    /// `pump` BÜTÜN eşleri denetler: iki eşin oturumu değişir; birinci eşin yeniden sunumu GEÇİCİ
    /// reddedilirken (tek atımlık ret) ikinci eşin kaydı AYNI turda sunulur, birinci eşinki sonraki
    /// turda. Yalnız ilk eşi işleyen ya da ilk retle bütün turu kesen bir `pump` bunu karşılamaz.
    ///
    /// `ea0d3909`'da UYARLANAMAZ: yeniden sunum yoktu.
    #[test]
    fn pump_offers_every_peer_even_when_one_is_refused() {
        let (p, q) = (peer(0), peer(1));
        let mut messenger = messenger_with([p, q]);
        let mut signaling = Signaling::new(clock());
        signaling.send(&mut messenger, p, b"p").expect("gönderilir");
        signaling.send(&mut messenger, q, b"q").expect("gönderilir");
        let p_new = reconnect(&mut messenger, p);
        let q_new = reconnect(&mut messenger, q);
        let before = messenger.transport().sent_on.len();
        messenger
            .transport_mut()
            .refuse
            .push_back(TransportError::Backpressure);
        signaling.pump(&mut messenger);
        // Eşler `PeerRef` sırasıyla denetlenir: `p` (eş 0) önce denenir ve reddedilir.
        assert_eq!(
            messenger.transport().sent_on[before..].to_vec(),
            vec![q_new],
            "aynı turda ikinci eş sunuldu"
        );
        assert_eq!(signaling.stats().resend_refused, 1);
        signaling.pump(&mut messenger);
        assert_eq!(
            messenger.transport().sent_on[before..].to_vec(),
            vec![q_new, p_new]
        );
        assert_eq!(signaling.stats().resent_on_new_session, 2);
    }

    /// `SIGNALING_BLOB` olmayan her olay aynen geçer: oturum olayları (kaydı olmayan eşin `Ready`'si
    /// hiçbir şey göndermez), metin, alındı, zincirin önceki halkalarının geçirdiği kayıtlar.
    ///
    /// `ea0d3909`'da UYARLANAMAZ: `Signaling::handle` yoktu (aynı olaylar zincirin sonunda aynen
    /// çıkıyordu).
    #[test]
    fn handle_passes_every_other_event_through_untouched() {
        let p = peer(0);
        let mut messenger = messenger_with([p]);
        let mut signaling = Signaling::new(clock());
        let others = [
            MessengerEvent::Session(SessionEvent::Ready(SessionHandle::from_raw(9), p)),
            MessengerEvent::TextReceived {
                peer: p,
                msg_id: 1,
                text: "metin".to_owned(),
            },
            MessengerEvent::Delivered { peer: p, msg_id: 2 },
            MessengerEvent::Other {
                peer: p,
                message: AppMessage::Receipt {
                    ref_msg_id: 3,
                    kind: ReceiptKind::Read,
                },
            },
            MessengerEvent::Other {
                peer: p,
                message: AppMessage::PttOpen {
                    stream_id: 4,
                    codec: PttCodec::Opus,
                    bundle_ms: 20,
                },
            },
        ];
        for event in others {
            assert_eq!(
                signaling.handle(&mut messenger, event.clone()),
                SignalingEvent::Messenger(event)
            );
        }
        assert_eq!(signaling.stats(), SignalingStats::default());
        assert!(messenger.transport().sent_on.is_empty());
    }

    /// `Debug` blob'u hiçbir biçimde basmaz: ne bayt listesi, ne metin, ne hex (küçük / büyük harf);
    /// tek satırlık ve çok satırlık (`{:#?}`) biçimde; doğrudan olayda da geçirilen `Messenger` olayında
    /// da. Yalnız uzunluk (`bytes_len`) yazılır.
    ///
    /// `ea0d3909`'da UYARLANAMAZ: `SignalingEvent` yoktu (`AppMessage::SignalingBlob`'un `Debug`'ı
    /// zaten uzunluk yazıyordu — o kısım AYNI DAVRANIŞ).
    #[test]
    fn debug_redacts_the_blob() {
        let text = "gizli-sinyal";
        let blob: Vec<u8> = text.bytes().collect();
        let hex: String = blob.iter().map(|byte| format!("{byte:02x}")).collect();
        let event = SignalingEvent::SignalingReceived {
            peer: peer(0),
            bytes: blob.clone(),
        };
        let passed = SignalingEvent::Messenger(MessengerEvent::Other {
            peer: peer(0),
            message: AppMessage::SignalingBlob { blob: blob.clone() },
        });
        let shown_event = format!("{event:?}");
        assert!(shown_event.contains("bytes_len"), "{shown_event}");
        let forbidden = [
            text.to_owned(),
            hex.clone(),
            hex.to_uppercase(),
            format!("{blob:?}"),
        ];
        for shown in [
            shown_event,
            format!("{event:#?}"),
            format!("{passed:?}"),
            format!("{passed:#?}"),
        ] {
            for secret in &forbidden {
                assert!(
                    !shown.contains(secret.as_str()),
                    "Debug blob'u sızdırdı: {shown}"
                );
            }
        }
    }
}
