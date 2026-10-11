//! PTT (bas-konuş) yaşam döngüsü (B2.4; K4G-08) — AMP `PTT_OPEN` (0x06), `PTT_CLOSE` (0x07) ve
//! `MEDIA_BUNDLE` (0x0A) kabı (`specs/application-message-protocol.md` §3, K4G-08 yaması 2026-10-12:
//! PTT-R0…PTT-R5; sahip kararları S7 / S8 / S9, Abkarada 2026-10-08; plan D5, D7 ek).
//!
//! İş emrindeki `PTT_START` / `PTT_END`, AMP'nin `PTT_OPEN` / `PTT_CLOSE`'udur (S9); kodda AMP
//! adları kullanılır. Ses verisinin kendisi media ekibinindir: bu modül kodlayıcı ya da medya
//! motoru çalıştırmaz, `MEDIA_BUNDLE.payload`'ı okumaz — opak baytları kapta taşır.
//!
//! [`Ptt`] [`Messenger`]'ın üstünde durur ve Transport'u sahiplenmez:
//!
//! * **Gönderen** — [`Ptt::open`] eşin sıradaki `stream_id`'sini Messenger'ın kalıcı deposundan
//!   ayırır ve `PTT_OPEN`'ı eşin güncel oturumuna verir; [`Ptt::send_bundle`] `MEDIA_BUNDLE`'ı
//!   akışın SABİT `codec` / `bundle_ms`'iyle verir; [`Ptt::close`] `PTT_CLOSE`'u verir. Tutamaç
//!   `Messenger::session_of`'tan, gönderim `Messenger::transport_mut` üzerinden.
//! * **Alıcı** — [`Ptt::poll`], [`Messenger::poll`]'un TEK okuyucusudur: PTT kayıtlarını
//!   [`PttEvent`]'e çevirir, diğer her olayı [`PttEvent::Messenger`] ile AYNEN geçirir.
//!
//! # Teslim politikası (PTT-R0; plan D7 ek, PoC-KAPSAM-001 §2 PTT satırı + EK-2 madde 1)
//!
//! `PTT_OPEN`, `MEDIA_BUNDLE` ve `PTT_CLOSE`'un ÜÇÜ DE `DeliveryPolicy::Reliable` ile gider: PTT
//! Reliable bir kliptir — ağ takılırsa ses bozulmaz, durur; geç gelen ses düşürülmez. S9'un
//! "`MEDIA_BUNDLE` `BestEffort`" kararı K4G-08 ile kalktı (kapsam notu üstündür); `BestEffort` /
//! `DeadlineBounded` yalnız canlı arama medyasınındır (K4G-10). Hiçbir PTT kaydı sıkıştırılmaz
//! (AMP §5 kural 6; codec bugün hiçbir tipi sıkıştırmaz).
//!
//! Telde demet sıra numarası YOKTUR (AMP §3 0x0A). Sırayla oynatma iki kaynaktan gelir: S1 §4 (aynı
//! oturumda `Reliable` birimler gönderim sırasıyla teslim edilir; eksik birim Transport'ta
//! kurtarılana dek arkasındakiler bekler) ve yükün içindeki media çerçeve sırası (media ekibinin).
//! Bu motorun payı: kabul ettiği her demeti teslim sırasıyla [`PttEvent::PttAudio`] olarak verir ve
//! hiçbir demeti GEÇ geldiği için düşürmez — eski bir oturumun drain'inden, aynı eşin daha yeni bir
//! akışının `PTT_OPEN`'ından sonra gelse de. Demet yalnız akışı bilinmiyorsa (PTT-R3) ya da `codec`
//! / `bundle_ms`'i akışınkinden farklıysa (S9) düşer.
//!
//! # `stream_id` — eş başına, kalıcı (plan D5; K4G-06 `CounterKind::StreamId`)
//!
//! Her `stream_id` `AppStore::reserve_next(eş, CounterKind::StreamId)` ile [`Messenger`]'ın
//! deposundan ayrılır (aynı depo; bileşim kökü Application'a tek depo bağlar). Depo değeri
//! döndürmeden ÖNCE kalıcılaştırır: aynı depoyla yeniden kurulan motor sayaca kaldığı yerden devam
//! eder ve aynı eşe sıradaki `stream_id` öncekilerin hepsinden büyüktür. Motorda bellek içi sayaç
//! YOKTUR.
//!
//! Gönderen API'si eş almaz ([`Ptt::send_bundle`] ve [`Ptt::close`] yalnız `stream_id` alır); eş
//! başına sayaçlarda iki eş aynı değeri alabilir. Bu yüzden [`Ptt::open`] AÇIK giden akışlar
//! arasında `stream_id` benzersizliğini korur: depo başka bir eşin açık akışının değerini verirse o
//! değer atlanır ve yenisi ayrılır. Depo değerleri eş başına kesin artandır ve açık akış eş başına
//! en çok birdir; bu yüzden atlama en çok `MAX_PEERS − 1` kez olur.
//!
//! Atlanan değer de, hata yolunda ayrılmış değer de YANAR (boşluk): ayırmadan sonraki adım (kodlama
//! ya da Transport reddi) başarısızsa değer kullanılmaz, sıradaki [`Ptt::open`] yenisini ayırır.
//! Alıcı `stream_id` sırası ya da ardışıklığı ZORLAMAZ (AMP §3 `stream_id` için sıra kuralı koymaz;
//! alıcı yalnız eşin etkin akışlarıyla eşleştirir); boşluk anlamsızdır. Depo `u32` alanının
//! tükendiğini kalıcı olarak bildirir → [`PttError::StreamIdExhausted`]; sarma YOK. Diğer depo
//! hataları [`PttError::Store`]: `PTT_OPEN` gönderilmez, akış açılmaz.
//!
//! # Gönderen
//!
//! Eş başına en çok bir açık giden akış (YORUM): konuşan oturumun eşidir (AMP §3, 1:1); akış
//! açıkken ikinci [`Ptt::open`] → [`PttError::StreamActive`]. [`Ptt::open`]'ın hata yollarında
//! akış açılmaz (ayırdığı değer yanabilir, yukarıda). [`Ptt::send_bundle`]'ın hata yollarında akış
//! açık kalır; tek iz: demetten önce sunulan yeniden `PTT_OPEN` (aşağıda) kabul edilip demet
//! reddedildiyse bu kabul kaydedilir (`PTT_OPEN` o oturuma bir daha gitmez). [`Ptt::close`] ise
//! akışı HER DURUMDA kapatır (YORUM): basış bitmiştir. `PTT_CLOSE`
//! Transport'a verilemezse hata döner ve kayıt yeniden denenmez (S1 K8); alıcı akışı PTT-R4 ile
//! (en geç [`PTT_STREAM_IDLE_TTL`] sonra) ya da oturum sonunda bitirir. Reddedilen demeti de motor
//! yeniden denemez: çağıran (media) bekler ve aynı demeti yeniden verir. Giden akış çağıranındır:
//! oturum bitse de kendiliğinden kapanmaz.
//!
//! # Oturum değişimi — yeniden `PTT_OPEN` (plan D5, K4G-08 §1)
//!
//! Akış eşindir (YORUM): supersession (`Ready(h')` → `Closed(h, Superseded)`) akışı bitirmez; S1
//! K7 `Ready(h')`'yi `Closed(h)`'den önce verir, eşin güncel oturumu boşalmaz; eski tutamaçtan geç
//! gelen kayıtlar da eşine bağlanır (Messenger'ın emekli tutamaç eşlemesi).
//!
//! Gönderen her açık akış için `PTT_OPEN`'ın en son KABUL edildiği tutamacı tutar. Eşin güncel
//! oturumu ([`Messenger::session_of`]) başka bir tutamaçsa akışın `PTT_OPEN`'ı oraya AYNI
//! `stream_id`, `codec` ve `bundle_ms` ile, `Reliable`, BİR KEZ yeniden verilir. Denetim her
//! [`Ptt::poll`]'un başında, eşin `Ready` olayı geçirilirken ve her [`Ptt::send_bundle`] /
//! [`Ptt::close`]'tan önce yapılır: yeni oturum `Ready` olayı okunmadan da benimsenebilir (senkron
//! [`Messenger::connect`] → `ConnectResult::Ready`), bu yüzden yalnız olaya güvenilmez. Transport
//! reddederse (ör. `Backpressure`) tutamaç işaretlenmez ve kayıt sonraki denetimde yeniden sunulur
//! (kabul edilmemiş birim tele çıkmamıştır; S1 K8 "bekler"); kabul edildikten sonra o tutamaca bir
//! daha verilmez — akış ve oturum başına en çok bir kabul. O tutamaçtaki demet `PTT_OPEN` kabul
//! edilmeden GİTMEZ (`send_bundle` reddi döndürür). `close`'ta ise yeniden `PTT_OPEN` reddedilirse
//! `PTT_CLOSE` yine sunulur — alıcıdaki akışı bitiren odur; ret sayılır ve akış kapandığı için
//! `PTT_OPEN` bir daha sunulmaz: bu tutamaçta `PTT_CLOSE` `PTT_OPEN`'sız gidebilir (akışı bilen
//! alıcıda akışı bitirir, bilmeyende PTT-R2). Akışı bilen alıcıda yeniden `PTT_OPEN` PTT-R1'dir
//! (etkisiz); `PTT_OPEN`'ı eski oturumla kaybetmiş alıcıda akışı başlatır.
//!
//! Eşin güncel oturumu kalmadıysa (`UserClosed`, `PeerClosed` ya da `KeyHygiene` ile kapanış)
//! eşin gelen akışları `PttEnded` ile biter: her [`Ptt::poll`] önce böyle akışları bitirir, yani
//! bu `PttEnded`'ler `Closed` geçirildikten sonraki ilk olaylardır.
//!
//! # Alıcı (AMP §3 PTT-R1…R5; S8: vektörsüz karar fail-closed DROP — panik yok, olay yok, sayaç var)
//!
//! Eş başına en çok [`PTT_MAX_ACTIVE_STREAMS`] etkin gelen akış (PTT-R5):
//!
//! * `PTT_OPEN` → yeni `stream_id` ise [`PttEvent::PttStarted`]. Eşin güncel oturumu yoksa
//!   (kapanmış oturumdan geç teslim) DROP. BİLİNEN bir `stream_id`'yle tekrar gelirse
//!   idempotenttir (PTT-R1): yeni akış yok, hata yok, olay yok. Bilinen = eşin etkin akışlarından
//!   biri (akışın `codec` / `bundle_ms`'i ilk `PTT_OPEN`'la sabitlendi, S9) ya da eşin kapanmış akış
//!   kaydındaki bir akış (aşağıda). Eşin [`PTT_MAX_ACTIVE_STREAMS`] etkin akışı varken yeni
//!   `stream_id` DROP (PTT-R5). Yeni akış eşin öbür akışını BİTİRMEZ: önceki "en yeni kazanır"
//!   YORUM'u K4G-08 ile kalktı (PTT-R5 iki eşzamanlı akış ister; PTT-R0 eski akışın geç demetini
//!   düşürmeyi yasaklar). Eski akış kendi `PTT_CLOSE`'uyla, PTT-R4 ile ya da oturum sonuyla biter.
//! * Kapanmış akış kaydı (PTT-R1): supersession'da akışın asıl `PTT_OPEN`'ı eski oturumun yalnız-alım
//!   drain'inde kalabilir (S1 K7, KS-GRACE-1: `N_g = 64` / `T_g = 30 s`); akışın yeni oturumdaki
//!   yeniden `PTT_OPEN`'ı, demetleri ve `PTT_CLOSE`'u ondan ÖNCE işlenebilir. Geç gelen o `PTT_OPEN`
//!   kapanmış akışı yeniden başlatsaydı aynı basışa ikinci `PttStarted` ve `PTT_CLOSE`'u hiç
//!   gelmeyecek, PTT-R4'e dek bir PTT-R5 yuvası tutan hayalet bir akış doğardı. Bu yüzden YALNIZ
//!   `PTT_CLOSE` ile kapanan akışların kimliği eş başına tutulur: en çok [`PTT_MAX_ACTIVE_STREAMS`]
//!   kimlik (en eski kapanış düşer), ömür [`PTT_STREAM_IDLE_TTL`] — yeni sayı yok. Türetme: drain
//!   `T_g = 30 s` < 120 s, geç `PTT_OPEN` kaydın ömrü içinde gelir; supersession anında eş başına en
//!   çok bir açık giden akış vardır, bir drain eşe en çok bir hayalet adayı bırakır (iki kimlik art
//!   arda iki supersession'a pay bırakır). Kayıttaki kimliğe gelen `PTT_OPEN` `opens_after_close`'ta
//!   sayılır; aynı akışın geç demeti yine PTT-R3'tür. PTT-R4 ya da oturum sonuyla biten akışlar
//!   kayda GİRMEZ: gönderen o akışı hâlâ açık tutuyor olabilir ve yeni oturumdaki yeniden
//!   `PTT_OPEN`'ı (D5) onu yeniden başlatmalıdır.
//! * `MEDIA_BUNDLE` → `stream_id` eşin etkin akışlarından biriyse ve `codec` VE `bundle_ms` o
//!   akışınkiyle aynıysa [`PttEvent::PttAudio`], teslim sırasıyla (PTT-R0). `stream_id` eşin etkin
//!   akışlarından biri değilse — eşin etkin akışı hiç yoksa da (S7: `PTT_OPEN` öncesi ya da
//!   `PTT_CLOSE` sonrası) — DROP ve `ptt_orphan_bundles` artar (PTT-R3). `codec` ya da `bundle_ms`
//!   farklıysa DROP (S9).
//! * `PTT_CLOSE` → eşin etkin akışıysa [`PttEvent::PttEnded`]; değilse sessizce yok sayılır
//!   (PTT-R2).
//! * Hareketsizlik (PTT-R4): bir akışa [`PTT_STREAM_IDLE_TTL`] boyunca `MEDIA_BUNDLE` gelmezse
//!   motor akışı yerel olarak kapatır ve [`PttEvent::PttEnded`] üretir (uygulama akışın bittiğini
//!   görür); eşe hiçbir şey gönderilmez (`PTT_CLOSE` üretilmez). Süre akışın `PTT_OPEN`'ı kabul
//!   edilince başlar ve her kabul edilen demette sıfırlanır; tekrar `PTT_OPEN` ya da düşen demet
//!   sıfırlamaz. Zaman enjekte edilen saatten ([`MonotonicClock`]) okunur ve kayıt İŞLENDİĞİ anda
//!   damgalanır. [`Ptt::poll`] süreyi yalnız Messenger'da okunacak bir şey kalmadığında denetler:
//!   Transport kuyruğunda bekleyen demet önce işlenir ve akışı canlı tutar. Denetim YALNIZ
//!   [`Ptt::poll`] içindedir: trafik olmasa da uygulama `poll`'u periyodik çağırmalıdır, yoksa
//!   süresi dolan akış ancak sonraki `poll`'da biter.
//!
//! # Tablolar
//!
//! Giden akış tablosu [`MAX_PEERS`] ile sınırlıdır (eş başına en çok bir akış). Gelen akış tablosu
//! `MAX_PEERS × PTT_MAX_ACTIVE_STREAMS` ile sınırlıdır (eş başına PTT-R5 tavanı; yeni sayı yok).
//! Eşler [`Messenger`]'ın eş tablosundaki eşlerdir. Tavan doluyken yeni giden akış
//! [`PttError::StreamTableFull`], yeni gelen akış DROP (`opens_refused`). Kapanmış akış kaydı eş
//! başına [`PTT_MAX_ACTIVE_STREAMS`], toplam `MAX_PEERS × PTT_MAX_ACTIVE_STREAMS` kimlikle sınırlıdır;
//! dolunca en eski kapanış yer açar, süresi dolan kayıt düşer. Denetim her eklemeden ÖNCE yapılır.
//!
//! # Bilinen sınırlar (sahibe açık)
//!
//! * Supersede edilen oturumda teslim edilmemiş `MEDIA_BUNDLE` ve `PTT_CLOSE` yeniden
//!   GÖNDERİLMEZ (S1 K7; D5 yalnız `PTT_OPEN`'ı yeniden gönderir): supersession anında yolda olan
//!   demetler kaybolabilir. Planın "60 s kesintiden sonra klip kaldığı yerden devam eder" kazancı
//!   AYNI oturumdaki kesinti içindir (Reliable ARQ). Kaybolan `PTT_CLOSE`'un etkisi PTT-R4 ve
//!   oturum sonuyla sınırlıdır.
//! * Oturumlar arası sıra garanti DEĞİLDİR (S1 §4 yalnız aynı oturumu sıralar): eski oturumun
//!   drain'inden gelen demet yeni oturumdakilerden sonra verilebilir (çerçeve sırası yükün, media
//!   ekibinin işidir). Akışın yeni oturumdan gelen `PTT_CLOSE`'undan SONRA işlenirse akış
//!   kapanmıştır ve demet PTT-R3 ile düşer; aynı drain'den geç gelen `PTT_OPEN` PTT-R1'dir
//!   (kapanmış akış kaydı). Kayıt taşarsa (drain süresince eşten iki yeni `PTT_CLOSE`) geç
//!   `PTT_OPEN` akışı yeniden başlatır; o hayalet akış PTT-R4 ile en geç [`PTT_STREAM_IDLE_TTL`]
//!   sonra biter.
//! * [`PTT_STREAM_IDLE_TTL`]'den uzun bir kesintiden sonra gelen demetler, akış alıcıda kapandığı
//!   için PTT-R3 ile düşer: gönderen aynı oturumda akışı yeniden açmaz (yeni oturumdaki yeniden
//!   `PTT_OPEN` akışı yeniden başlatır).
//! * Kapatılmış bir `stream_id` başka bir eşin açık akışına yeniden verilebilir: sayaçlar eş
//!   başınadır ve benzersizlik yalnız AÇIK giden akışlar arasında korunur (K4G-08 kararı). Kapattığı
//!   `stream_id`'yi yeniden kullanan çağıran (use-after-close) [`Ptt::send_bundle`] /
//!   [`Ptt::close`] ile o başka eşin akışına ses ya da kapanış yollar: çağıran kapattığı kimliği
//!   KULLANMAMALIDIR. K4G-11 (`ptt_push(stream_id)` / `ptt_close(stream_id)` eş almaz) bu sınırı
//!   kendi tutamacıyla kapatmalıdır.
//! * D4 (PoC-KAPSAM-001 §5 EK-1): kalıcı durum tamamen kaybolur ve aynı kimlik yeniden
//!   kullanılırsa `stream_id`'ler baştan başlar; alıcıda hâlâ bilinen (etkin ya da kapanmış akış
//!   kaydındaki) bir akışın değeri yeniden gelirse PTT-R1 onu tekrar sayar ve yeni akış başlamaz:
//!   etkin akışınsa yeni basışın demetleri o akışa bağlanır (parametreleri farklıysa S9 ile düşer),
//!   kapanmış akışınsa PTT-R3 ile düşer. Kural: yeniden provisioning.
//!
//! # Gizlilik
//!
//! [`PttEvent`]'in `Debug` çıktısı ses yükünü değil uzunluğunu yazar; [`Ptt`] `Debug` değildir.

use core::fmt;
use core::time::Duration;
use std::collections::btree_map::{Entry, Range};
use std::collections::BTreeMap;
use std::sync::Arc;

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{DeliveryPolicy, KamaTransport, SessionEvent, TransportError};

use crate::clock::MonotonicClock;
use crate::codec::limits::MEDIA_BUNDLE_PREFIX_BYTES;
use crate::codec::{self, AmpLimits, AppMessage, EncodeError, PttCodec};
use crate::messaging::{Messenger, MessengerEvent, MAX_PEERS};
use crate::store::{AppStore, AppStoreError, CounterKind};

/// `PTT_OPEN` / `PTT_CLOSE` / `MEDIA_BUNDLE` akış kimliği (AMP §3; telde `u32_be`).
pub type StreamId = u32;

/// Taze depoda bir eşe atanan ilk `stream_id`. Spec başlangıç değeri koymaz; değer bu uygulamanındır
/// ve depo onu `store::FIRST_COUNTER_VALUE` olarak verir (eşitlik `store`'da derleme zamanında
/// denetlenir).
pub const FIRST_STREAM_ID: StreamId = 0;

/// PTT-R4 (AMP §3, K4G-08): bir gelen akışa bu süre boyunca `MEDIA_BUNDLE` gelmezse alıcı akışı
/// yerel olarak kapatır ([`PttEvent::PttEnded`]; `PTT_CLOSE` üretmez). 120 s: PoC-KAPSAM-001 K8'deki
/// 60 s kesinti senaryosunda akış yaşamalı (plan K4G-08 §2).
pub const PTT_STREAM_IDLE_TTL: Duration = Duration::from_secs(120);

/// PTT-R5 (AMP §3, K4G-08): aynı eşten aynı anda en çok bu kadar etkin gelen akış; fazlası
/// düşürülür.
pub const PTT_MAX_ACTIVE_STREAMS: usize = 2;

/// Gelen akış tablosunun tavanı: eş tablosu tavanı × PTT-R5 tavanı (türetilmiş; yeni sayı yok).
const MAX_INCOMING_STREAMS: usize = MAX_PEERS * PTT_MAX_ACTIVE_STREAMS;

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
    /// Eş konuşmaya başladı: yeni `stream_id`'li `PTT_OPEN` kabul edildi. `codec` ve `bundle_ms` bu
    /// akış boyunca sabittir (S9).
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
    /// Eşin etkin akışlarından birinden bir ses demeti, teslim sırasıyla (PTT-R0). Yük opaktır; içi
    /// media ekibinindir.
    PttAudio {
        /// Konuşan eş.
        peer: PeerRef,
        /// Akış kimliği.
        stream_id: StreamId,
        /// `MEDIA_BUNDLE.payload`, aynen.
        payload: Vec<u8>,
    },
    /// Eşin akışı bitti: `PTT_CLOSE`, PTT-R4 hareketsizliği (yerel kapanış) ya da eşin güncel
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

/// [`Ptt::open`], [`Ptt::send_bundle`] ve [`Ptt::close`] hatası. `open`'da akış açılmaz (ayrılmış
/// `stream_id` yanabilir — modül belgesi); `send_bundle`'da akış açık kalır (demetten önce kabul
/// edilmiş bir yeniden `PTT_OPEN` kaydedilir); `close`'da akış yine de kapanmıştır. Motor reddedilen
/// kaydı yeniden denemez (S1 K8); tek istisna oturum değişiminin yeniden `PTT_OPEN`'ıdır (D5: akış
/// açık kaldıkça kabul edilene dek sonraki denetimlerde yeniden sunulur).
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
    /// Eşin `u32` `stream_id` alanı tükendi (depo bunu kalıcı bildirir); sarma YOK.
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
    /// Kalıcı depo ([`AppStore`]) `stream_id` ayıramadı ya da sözleşmesine aykırı bir değer verdi
    /// (`Corrupt`): `PTT_OPEN` gönderilmedi, akış açılmadı. Depo tükenmesi
    /// [`PttError::StreamIdExhausted`] olarak döner, burada değil.
    Store(AppStoreError),
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
            Self::StreamIdExhausted => {
                f.write_str("stream_id space of this peer is exhausted (no wrap)")
            }
            Self::PayloadTooLarge {
                payload_len,
                max_payload_len,
            } => write!(
                f,
                "MEDIA_BUNDLE payload of {payload_len} bytes exceeds {max_payload_len} bytes"
            ),
            Self::Encode(e) => write!(f, "PTT record does not fit one AMP record: {e}"),
            Self::Transport(e) => write!(f, "transport refused the record: {e}"),
            Self::Store(e) => write!(f, "app store failed, no PTT_OPEN was sent: {e}"),
        }
    }
}

impl std::error::Error for PttError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::Store(e) => Some(e),
            Self::NoSession
            | Self::StreamActive { .. }
            | Self::UnknownStream
            | Self::StreamTableFull
            | Self::StreamIdExhausted
            | Self::PayloadTooLarge { .. } => None,
        }
    }
}

/// Gözlem sayaçları — salt-okunur; protokol davranışını değiştirmez. Alıcı tarafının DROP'ları
/// (S8), olay üreten akış sonları ve gönderenin yeniden `PTT_OPEN`'ları (D5).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PttStats {
    /// PTT-R3: `stream_id`'si eşin etkin akışlarından biri olmayan `MEDIA_BUNDLE`'lar — eşin etkin
    /// akışı hiç yokken gelenler dahil (`PTT_OPEN` öncesi, S7; `PTT_CLOSE` sonrası).
    pub ptt_orphan_bundles: u64,
    /// `stream_id`'si eşin etkin akışlarından birini tutan ama `codec` ya da `bundle_ms`'i o
    /// akışınkinden farklı `MEDIA_BUNDLE`'lar (S9).
    pub bundles_mismatched: u64,
    /// PTT-R1: eşin etkin akışlarından birinin `stream_id`'siyle yeniden gelen `PTT_OPEN`'lar
    /// (idempotent; parametreleri aynı ya da farklı).
    pub duplicate_opens: u64,
    /// PTT-R1: eşin kapanmış akış kaydındaki (`PTT_STREAM_IDLE_TTL` içinde `PTT_CLOSE` ile kapanmış)
    /// bir akışın `stream_id`'siyle gelen `PTT_OPEN`'lar — eski oturumun drain'inden geç teslim;
    /// akışı yeniden başlatmaz, olay yok.
    pub opens_after_close: u64,
    /// Güncel oturumu olmayan eşten gelen `PTT_OPEN`'lar (kapanmış oturumdan geç teslim).
    pub opens_without_session: u64,
    /// PTT-R5: eşin [`PTT_MAX_ACTIVE_STREAMS`] etkin akışı varken gelen yeni `stream_id`'li
    /// `PTT_OPEN`'lar.
    pub opens_over_active_limit: u64,
    /// Gelen akış tablosu tavandayken (`MAX_PEERS × PTT_MAX_ACTIVE_STREAMS`) gelen yeni
    /// `stream_id`'li `PTT_OPEN`'lar.
    pub opens_refused: u64,
    /// PTT-R2: eşin etkin akışlarından birine ait olmayan `PTT_CLOSE`'lar (sessizce yok sayıldı).
    pub unknown_closes: u64,
    /// PTT-R4: [`PTT_STREAM_IDLE_TTL`] boyunca demet gelmediği için yerel olarak kapatılan akışlar
    /// (olay üretir).
    pub streams_ended_by_idle: u64,
    /// Eşin güncel oturumu kalmadığı için biten akışlar (olay üretir).
    pub streams_ended_by_session: u64,
    /// Gönderen (D5): yeni oturuma Transport'un kabul ettiği yeniden `PTT_OPEN`'lar — akış ve
    /// oturum başına en çok bir.
    pub opens_resent: u64,
    /// Gönderen (D5): Transport'un reddettiği yeniden `PTT_OPEN` denemeleri. Yoklama ve
    /// `send_bundle` yolundaki ret sonraki denetimde yeniden sunulur; `close` yolundaki ret yeniden
    /// SUNULMAZ (akış kapandı; `PTT_CLOSE` yine gider).
    pub opens_resend_refused: u64,
}

/// B2.4 PTT motoru. Ayrıntı: modül belgesi.
pub struct Ptt {
    /// PTT-R4'ün zaman kaynağı.
    clock: Arc<dyn MonotonicClock>,
    /// Açık giden akışlar, `stream_id` sırasıyla (≤ [`MAX_PEERS`]; eş başına en çok bir; `stream_id`
    /// açık akışlar arasında benzersiz).
    outgoing: BTreeMap<StreamId, Outgoing>,
    /// Etkin gelen akışlar, (eş, `stream_id`) sırasıyla (≤ [`MAX_INCOMING_STREAMS`]; eş başına
    /// ≤ [`PTT_MAX_ACTIVE_STREAMS`]).
    incoming: BTreeMap<(PeerRef, StreamId), Incoming>,
    /// Kapanmış akış kaydı (PTT-R1): `PTT_CLOSE` ile kapanan gelen akışlar → kapanış anı (saatin
    /// `now()`'ı). Eş başına ≤ [`PTT_MAX_ACTIVE_STREAMS`], toplam ≤ [`MAX_INCOMING_STREAMS`]; ömür
    /// [`PTT_STREAM_IDLE_TTL`] (modül belgesi, "Alıcı").
    closed: BTreeMap<(PeerRef, StreamId), Duration>,
    stats: PttStats,
}

/// Açık giden akış: talk-spurt'ün sabit parametreleri.
struct Outgoing {
    peer: PeerRef,
    codec: PttCodec,
    bundle_ms: u8,
    /// `PTT_OPEN`'ı en son KABUL eden tutamaç; eşin güncel oturumu başkaysa `PTT_OPEN` oraya bir kez
    /// yeniden verilir (D5).
    opened_on: SessionHandle,
}

/// Etkin gelen akış: ilk `PTT_OPEN`'ın sabitlediği parametreler ve PTT-R4 süresinin başı.
struct Incoming {
    codec: PttCodec,
    bundle_ms: u8,
    /// `PTT_OPEN`'ın ya da son kabul edilen demetin işlendiği an (saatin `now()`'ı).
    last_bundle_at: Duration,
}

impl Ptt {
    /// Boş motor: açık akış yok. `clock` PTT-R4'ün zaman kaynağıdır — ürün yolu
    /// [`SystemClock`](crate::clock::SystemClock)'u verir (bileşim kökü), testler elle ilerletilen
    /// bir saat. `stream_id`'ler [`Messenger`]'ın deposundan ayrılır; motor sayaç tutmaz.
    #[must_use]
    pub fn new(clock: Arc<dyn MonotonicClock>) -> Self {
        Self {
            clock,
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
            closed: BTreeMap::new(),
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

    /// Eşin etkin gelen akışları, `stream_id` artan sırasıyla (≤ [`PTT_MAX_ACTIVE_STREAMS`]).
    #[must_use]
    pub fn incoming_streams(&self, peer: PeerRef) -> Vec<StreamId> {
        self.streams_of(peer)
            .map(|(&(_, stream_id), _)| stream_id)
            .collect()
    }

    /// Eşin etkin gelen akışları (tablonun eşe düşen dilimi).
    fn streams_of(&self, peer: PeerRef) -> Range<'_, (PeerRef, StreamId), Incoming> {
        self.incoming
            .range((peer, StreamId::MIN)..=(peer, StreamId::MAX))
    }

    /// Basış başlar: eşin sıradaki `stream_id`'si kalıcı depodan ayrılır, `PTT_OPEN` eşin güncel
    /// oturumuna `DeliveryPolicy::Reliable` ile verilir ve `stream_id` döner. `Ok` = Transport kaydı
    /// kuyruğa aldı; teslim DEĞİL (S1 K8). `codec` ve `bundle_ms` akış boyunca sabittir; `bundle_ms`
    /// yorumlanmaz (media ekibinin).
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`PttError::StreamActive`] · [`PttError::StreamTableFull`] ·
    /// [`PttError::NoSession`] — üçünde de depoya dokunulmaz · [`PttError::StreamIdExhausted`] ya da
    /// [`PttError::Store`] (`stream_id` ayrılamadı) · [`PttError::Encode`] · [`PttError::Transport`]
    /// — son ikisinde ayrılan `stream_id` yanar. Hiçbirinde akış açılmaz ve `PTT_OPEN` kuyruğa
    /// girmez.
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
        let handle = messenger.session_of(peer).ok_or(PttError::NoSession)?;
        let stream_id = self.reserve_stream_id(messenger.store(), peer)?;
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
                opened_on: handle,
            },
        );
        Ok(stream_id)
    }

    /// Depodan eşin sıradaki `stream_id`'si; açık bir giden akışın değeriyse atlanır ve yenisi
    /// ayrılır (modül belgesi, "`stream_id`").
    ///
    /// # Errors
    ///
    /// [`PttError::StreamIdExhausted`] (depo `Exhausted`) · [`PttError::Store`] — değer ayrılmadı,
    /// `u32`'ye sığmıyor ya da (aynı değeri yineleyen depo) atlama sınırı aşıldı; ikisi depo
    /// sözleşmesine aykırıdır (`Corrupt`).
    fn reserve_stream_id(&self, store: &dyn AppStore, peer: PeerRef) -> Result<StreamId, PttError> {
        // Eşin açık akışı yok (çağıran denetledi): çakışabilecek değerler öbür eşlerin en çok
        // `outgoing.len()` açık akışınındır. Depo değerleri eş başına kesin artan olduğundan her biri
        // en çok bir kez çıkar; sınır, sözleşmeye aykırı bir depoya karşı döngüyü keser.
        for _ in 0..=self.outgoing.len() {
            let value = store
                .reserve_next(peer, CounterKind::StreamId)
                .map_err(store_error)?;
            let stream_id =
                StreamId::try_from(value).map_err(|_| PttError::Store(AppStoreError::Corrupt))?;
            if !self.outgoing.contains_key(&stream_id) {
                return Ok(stream_id);
            }
        }
        Err(PttError::Store(AppStoreError::Corrupt))
    }

    /// Ses demeti: `MEDIA_BUNDLE` akışın sabit `codec` / `bundle_ms`'iyle, eşin O ANKİ güncel
    /// oturumuna `DeliveryPolicy::Reliable` ile verilir (PTT-R0). Eşin güncel oturumu akışın
    /// `PTT_OPEN`'ını henüz almadıysa önce o verilir (D5; modül belgesi, "Oturum değişimi"). Yük
    /// opaktır ve sıkıştırılmaz; boyu kopyalanmadan ÖNCE denetlenir.
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`PttError::UnknownStream`] · [`PttError::PayloadTooLarge`] ·
    /// [`PttError::NoSession`] · yeniden `PTT_OPEN`'ın [`PttError::Encode`] ya da
    /// [`PttError::Transport`]'u (demet verilmedi; `PTT_OPEN` sonraki denetimde yeniden sunulur) ·
    /// demetin [`PttError::Encode`] ya da [`PttError::Transport`]'u (yeniden `PTT_OPEN` bu çağrıda
    /// kabul edildiyse o kabul kalır). Her hatada akış açık kalır. Reddedilen demeti motor yeniden
    /// denemez (S1 K8): çağıran bekler ve aynı demeti yeniden verir.
    pub fn send_bundle<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        stream_id: StreamId,
        payload: &[u8],
    ) -> Result<(), PttError> {
        let stream = self
            .outgoing
            .get_mut(&stream_id)
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
        reopen(messenger, stream_id, stream, &mut self.stats)?;
        let bundle = AppMessage::MediaBundle {
            stream_id,
            codec: stream.codec.code(),
            bundle_ms: stream.bundle_ms,
            payload: payload.to_vec(),
        };
        send_record(messenger, handle, &bundle, DeliveryPolicy::Reliable)
    }

    /// Basış biter: akış kapanır ve `PTT_CLOSE` eşin güncel oturumuna `DeliveryPolicy::Reliable`
    /// ile TEK KEZ sunulur. Eşin güncel oturumu akışın `PTT_OPEN`'ını henüz almadıysa önce o sunulur
    /// (D5); sunulamazsa `PTT_CLOSE` yine sunulur (alıcıdaki akışı bitiren odur), ret
    /// `opens_resend_refused`'ta sayılır ve `PTT_OPEN` bir daha sunulmaz (akış kapandı).
    ///
    /// # Errors
    ///
    /// [`PttError::UnknownStream`] — açık akış değil; hiçbir şey değişmez. Diğer hatalarda akış
    /// YİNE DE kapanmıştır, yalnız `PTT_CLOSE` Transport'a verilemedi: [`PttError::NoSession`] ·
    /// [`PttError::Encode`] · [`PttError::Transport`]. Yeniden deneme yok (S1 K8); alıcı akışı PTT-R4
    /// ile ya da oturum sonunda bitirir (modül belgesi).
    pub fn close<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        stream_id: StreamId,
    ) -> Result<(), PttError> {
        let mut stream = self
            .outgoing
            .remove(&stream_id)
            .ok_or(PttError::UnknownStream)?;
        let handle = messenger
            .session_of(stream.peer)
            .ok_or(PttError::NoSession)?;
        // Ret `opens_resend_refused`'ta sayıldı; `PTT_CLOSE`'un sunulması ondan bağımsızdır.
        let _ = reopen(messenger, stream_id, &mut stream, &mut self.stats);
        let close = AppMessage::PttClose { stream_id };
        send_record(messenger, handle, &close, DeliveryPolicy::Reliable)
    }

    /// Bir olay döndürür; okunacak bir şey kalmadıysa `None`. [`Messenger::poll`]'un YERİNE
    /// çağrılır: araya doğrudan `Messenger::poll` girerse PTT kayıtları ve oturum olayları bu
    /// motoru atlar.
    ///
    /// Sıra: (1) açık giden akışların yeni oturuma yeniden `PTT_OPEN`'ı (D5; olay üretmez);
    /// (2) güncel oturumu kalmamış eşlerin gelen akışları için `PttEnded`, birer birer;
    /// (3) Messenger'ın olayları — eşin `Ready` olayı geçirilmeden önce (1) yinelenir;
    /// (4) Messenger'da okunacak bir şey kalmadıysa [`PTT_STREAM_IDLE_TTL`]'i dolmuş ilk gelen akış
    /// için `PttEnded` (PTT-R4). Olay üretmeyen (atılan) PTT kayıtları aynı çağrıda atlanır.
    ///
    /// PTT-R4 denetimi yalnız burada yapılır: trafik olmasa da uygulama `poll`'u periyodik
    /// çağırmalıdır (zamanlayıcı ya da olay döngüsü turu); yoksa süresi dolan akış ancak sonraki
    /// `poll`'da biter.
    #[must_use]
    pub fn poll<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>) -> Option<PttEvent> {
        self.reopen_all(messenger);
        if let Some(event) = self.end_orphaned(messenger) {
            return Some(event);
        }
        loop {
            let Some(event) = messenger.poll() else {
                return self.end_idle();
            };
            if let Some(event) = self.on_event(messenger, event) {
                return Some(event);
            }
        }
    }

    /// D5: her açık giden akışın `PTT_OPEN`'ını, eşin güncel oturumu onu henüz almadıysa oraya
    /// sunar ([`reopen`]). Ret sayılır ve sonraki denetimde yeniden denenir.
    fn reopen_all<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>) {
        for (&stream_id, stream) in &mut self.outgoing {
            // Ret `opens_resend_refused`'ta sayıldı; tutamaç işaretlenmedi, sonraki denetim yeniden
            // sunar.
            let _ = reopen(messenger, stream_id, stream, &mut self.stats);
        }
    }

    /// Güncel oturumu kalmamış eşin ilk gelen akışını bitirir.
    fn end_orphaned<T: KamaTransport>(&mut self, messenger: &Messenger<T>) -> Option<PttEvent> {
        let (peer, stream_id) = self
            .incoming
            .keys()
            .copied()
            .find(|(peer, _)| messenger.session_of(*peer).is_none())?;
        self.incoming.remove(&(peer, stream_id));
        self.stats.streams_ended_by_session += 1;
        Some(PttEvent::PttEnded { peer, stream_id })
    }

    /// PTT-R4: [`PTT_STREAM_IDLE_TTL`] boyunca demet almamış ilk gelen akışı yerel olarak bitirir;
    /// eşe hiçbir şey gönderilmez.
    fn end_idle(&mut self) -> Option<PttEvent> {
        let now = self.clock.now();
        let (peer, stream_id) = self
            .incoming
            .iter()
            .find(|(_, stream)| now.saturating_sub(stream.last_bundle_at) >= PTT_STREAM_IDLE_TTL)
            .map(|(&key, _)| key)?;
        self.incoming.remove(&(peer, stream_id));
        self.stats.streams_ended_by_idle += 1;
        Some(PttEvent::PttEnded { peer, stream_id })
    }

    fn on_event<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        event: MessengerEvent,
    ) -> Option<PttEvent> {
        match event {
            MessengerEvent::Other { peer, message } => self.on_record(messenger, peer, message),
            MessengerEvent::Session(SessionEvent::Ready(..)) => {
                // D5: Messenger yeni oturumu benimsedi; açık akışların `PTT_OPEN`'ı, olay uygulamaya
                // verilmeden (uygulama o oturumdan demet göndermeden) önce sunulur.
                self.reopen_all(messenger);
                Some(PttEvent::Messenger(event))
            }
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
            } => self.on_open(messenger, peer, stream_id, codec, bundle_ms),
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

    /// `PTT_OPEN`: yeni akış (PTT-R5 ve tablo tavanı içinde) ya da bilinen akışın tekrarı (PTT-R1:
    /// etkin ya da kapanmış akış kaydında) — modül belgesi.
    fn on_open<T: KamaTransport>(
        &mut self,
        messenger: &Messenger<T>,
        peer: PeerRef,
        stream_id: StreamId,
        codec: PttCodec,
        bundle_ms: u8,
    ) -> Option<PttEvent> {
        if messenger.session_of(peer).is_none() {
            self.stats.opens_without_session += 1;
            return None;
        }
        if self.recently_closed(peer, stream_id) {
            // PTT-R1: kapanmış akış da bilinen akıştır — eski oturumun drain'inden geç gelen
            // `PTT_OPEN` onu yeniden başlatmaz.
            self.stats.opens_after_close += 1;
            return None;
        }
        let active = self.streams_of(peer).count();
        let tracked = self.incoming.len();
        match self.incoming.entry((peer, stream_id)) {
            Entry::Occupied(_) => {
                // PTT-R1: yeni akış yok, hata yok, olay yok. Akışın `codec` / `bundle_ms`'i ilk
                // `PTT_OPEN`'la sabitlendi (S9); tekrar onu değiştiremez.
                self.stats.duplicate_opens += 1;
                None
            }
            Entry::Vacant(slot) => {
                if active >= PTT_MAX_ACTIVE_STREAMS {
                    self.stats.opens_over_active_limit += 1;
                    return None;
                }
                if tracked >= MAX_INCOMING_STREAMS {
                    self.stats.opens_refused += 1;
                    return None;
                }
                slot.insert(Incoming {
                    codec,
                    bundle_ms,
                    last_bundle_at: self.clock.now(),
                });
                Some(PttEvent::PttStarted {
                    peer,
                    stream_id,
                    codec,
                    bundle_ms,
                })
            }
        }
    }

    /// `PTT_CLOSE`: yalnız eşin etkin akışlarından birini bitirir ve onu kapanmış akış kaydına
    /// yazar (PTT-R1); değilse PTT-R2.
    fn on_close(&mut self, peer: PeerRef, stream_id: StreamId) -> Option<PttEvent> {
        if self.incoming.remove(&(peer, stream_id)).is_some() {
            self.remember_closed(peer, stream_id);
            return Some(PttEvent::PttEnded { peer, stream_id });
        }
        self.stats.unknown_closes += 1;
        None
    }

    /// Eşin kapanmış akış kaydı (tablonun eşe düşen dilimi).
    fn closed_of(&self, peer: PeerRef) -> Range<'_, (PeerRef, StreamId), Duration> {
        self.closed
            .range((peer, StreamId::MIN)..=(peer, StreamId::MAX))
    }

    /// PTT-R1: (eş, `stream_id`) [`PTT_STREAM_IDLE_TTL`] içinde `PTT_CLOSE` ile kapandı mı.
    fn recently_closed(&self, peer: PeerRef, stream_id: StreamId) -> bool {
        let now = self.clock.now();
        self.closed
            .get(&(peer, stream_id))
            .is_some_and(|&closed_at| now.saturating_sub(closed_at) < PTT_STREAM_IDLE_TTL)
    }

    /// `PTT_CLOSE` ile kapanan akışı kapanmış akış kaydına yazar (modül belgesi, "Alıcı"). Süresi
    /// dolan kayıtlar düşer; eşin kaydı ya da tablo doluysa en eski kapanış yer açar — sınır her
    /// eklemeden ÖNCE.
    fn remember_closed(&mut self, peer: PeerRef, stream_id: StreamId) {
        let now = self.clock.now();
        self.closed
            .retain(|_, &mut closed_at| now.saturating_sub(closed_at) < PTT_STREAM_IDLE_TTL);
        let evicted = if self.closed_of(peer).count() >= PTT_MAX_ACTIVE_STREAMS {
            earliest_closed(self.closed_of(peer))
        } else if self.closed.len() >= MAX_INCOMING_STREAMS {
            earliest_closed(self.closed.iter())
        } else {
            None
        };
        if let Some(key) = evicted {
            self.closed.remove(&key);
        }
        self.closed.insert((peer, stream_id), now);
    }

    /// `MEDIA_BUNDLE`: `stream_id` eşin etkin akışlarından biri olmalı (PTT-R3) ve `codec` /
    /// `bundle_ms` o akışınkiyle eşleşmeli (S9). Kabul edilen demet PTT-R4 süresini sıfırlar.
    fn on_bundle(
        &mut self,
        peer: PeerRef,
        stream_id: StreamId,
        codec: u8,
        bundle_ms: u8,
        payload: Vec<u8>,
    ) -> Option<PttEvent> {
        let Some(active) = self.incoming.get_mut(&(peer, stream_id)) else {
            self.stats.ptt_orphan_bundles += 1;
            return None;
        };
        if active.codec.code() != codec || active.bundle_ms != bundle_ms {
            self.stats.bundles_mismatched += 1;
            return None;
        }
        active.last_bundle_at = self.clock.now();
        Some(PttEvent::PttAudio {
            peer,
            stream_id,
            payload,
        })
    }
}

/// D5: akışın `PTT_OPEN`'ı eşin güncel oturumuna henüz kabul ettirilmediyse oraya BİR KEZ, aynı
/// `stream_id` / `codec` / `bundle_ms` ile, `Reliable` sunulur. Eşin güncel oturumu yoksa ya da
/// `PTT_OPEN` o tutamaçta zaten kabul edildiyse hiçbir şey yapmaz.
///
/// # Errors
///
/// [`PttError::Encode`] · [`PttError::Transport`] — tutamaç işaretlenmez ve ret
/// `opens_resend_refused`'ta sayılır; sonraki denetim kaydı yeniden sunar.
fn reopen<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    stream_id: StreamId,
    stream: &mut Outgoing,
    stats: &mut PttStats,
) -> Result<(), PttError> {
    let Some(handle) = messenger.session_of(stream.peer) else {
        return Ok(());
    };
    if handle == stream.opened_on {
        return Ok(());
    }
    let open = AppMessage::PttOpen {
        stream_id,
        codec: stream.codec,
        bundle_ms: stream.bundle_ms,
    };
    match send_record(messenger, handle, &open, DeliveryPolicy::Reliable) {
        Ok(()) => {
            stream.opened_on = handle;
            stats.opens_resent += 1;
            Ok(())
        }
        Err(error) => {
            stats.opens_resend_refused += 1;
            Err(error)
        }
    }
}

/// Kapanmış akış kayıtlarından en erken kapanmış olanın anahtarı.
fn earliest_closed<'a>(
    records: impl Iterator<Item = (&'a (PeerRef, StreamId), &'a Duration)>,
) -> Option<(PeerRef, StreamId)> {
    records
        .min_by_key(|&(_, &closed_at)| closed_at)
        .map(|(&key, _)| key)
}

/// `stream_id` ayırma hatası: depo tükenmesi kalıcıdır ve sarma yoktur.
fn store_error(error: AppStoreError) -> PttError {
    match error {
        AppStoreError::Exhausted => PttError::StreamIdExhausted,
        other => PttError::Store(other),
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
    //! Tablo tavanları, `stream_id` tükenmesi, depo sözleşmesi ihlalleri ve `Debug` — uçtan uca
    //! akış (yeniden `PTT_OPEN`, PTT-R0…R5, kalıcılık) `tests/ptt.rs`'te.

    use std::sync::Mutex;

    use super::*;
    use crate::clock::SystemClock;
    use crate::messaging::MsgId;
    use crate::store::{DedupWindow, MemoryAppStore, OutboxUnit};
    use kama_contracts::s1::{
        ConnectResult, ConnectTicket, Delivered, PreparedUnit, TransportCapabilities,
    };

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
    /// Test talk-spurt'ünün demet süresi (ms; normatif değil).
    const TEST_BUNDLE_MS: u8 = 20;

    /// Her `connect`'e yeni bir hazır oturumla yanıt veren, her gönderimi kabul edip tutamacını
    /// kaydeden, hiçbir şey teslim etmeyen Transport.
    #[derive(Default)]
    struct ReadyTransport {
        /// Basılan son tutamacın ham değeri.
        last_handle: u64,
        /// Kabul edilen gönderimlerin tutamaçları, sırasıyla.
        sent_on: Vec<SessionHandle>,
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

    /// `stream_id` sayacı testin koyduğu değerden süren depo (eşten bağımsız; tükenme kuralı ürün
    /// depolarıyla aynı: [`CounterKind::advance`]). `repeating` ise sözleşmeye AYKIRI olarak hep aynı
    /// değeri, tavan denetimi olmadan verir. Geri kalan her şey bellek içi depoya gider.
    struct SeededStore {
        next: Mutex<u64>,
        repeating: bool,
        rest: MemoryAppStore,
    }

    impl SeededStore {
        fn counting_from(next: u64) -> Self {
            Self {
                next: Mutex::new(next),
                repeating: false,
                rest: MemoryAppStore::new(),
            }
        }

        fn repeating(value: u64) -> Self {
            Self {
                repeating: true,
                ..Self::counting_from(value)
            }
        }
    }

    impl AppStore for SeededStore {
        fn reserve_next(&self, peer: PeerRef, kind: CounterKind) -> Result<u64, AppStoreError> {
            if kind != CounterKind::StreamId {
                return self.rest.reserve_next(peer, kind);
            }
            let mut next = self.next.lock().expect("kilit zehirlenmedi");
            let value = *next;
            if !self.repeating {
                *next = kind.advance(value)?;
            }
            Ok(value)
        }

        fn dedup_load(&self, peer: PeerRef) -> Result<DedupWindow, AppStoreError> {
            self.rest.dedup_load(peer)
        }

        fn dedup_save(&self, peer: PeerRef, window: &DedupWindow) -> Result<(), AppStoreError> {
            self.rest.dedup_save(peer, window)
        }

        fn outbox_put(&self, peer: PeerRef, unit: &OutboxUnit) -> Result<(), AppStoreError> {
            self.rest.outbox_put(peer, unit)
        }

        fn outbox_remove(&self, peer: PeerRef, msg_id: MsgId) -> Result<bool, AppStoreError> {
            self.rest.outbox_remove(peer, msg_id)
        }

        fn outbox_list(&self, peer: PeerRef) -> Result<Vec<OutboxUnit>, AppStoreError> {
            self.rest.outbox_list(peer)
        }
    }

    /// Birim testlerinin saati: zaman davranışı `tests/ptt.rs`'te elle ilerletilen saatle sınanır.
    fn clock() -> Arc<dyn MonotonicClock> {
        Arc::new(SystemClock::new())
    }

    /// Elle ilerletilen saat (kapanmış akış kaydının ömrü için); kopyalar aynı zamanı paylaşır.
    #[derive(Clone, Default)]
    struct TestClock(Arc<Mutex<Duration>>);

    impl TestClock {
        fn advance(&self, by: Duration) {
            let mut now = self.0.lock().expect("kilit zehirlenmedi");
            *now = now.saturating_add(by);
        }
    }

    impl MonotonicClock for TestClock {
        fn now(&self) -> Duration {
            *self.0.lock().expect("kilit zehirlenmedi")
        }
    }

    /// `n`'den türetilmiş sentetik eş (ilk sekiz bayt `n`, BE).
    fn peer(n: usize) -> PeerRef {
        let mut dest_hash = [0u8; 32];
        let index = u64::try_from(n).expect("test sayısı u64'e sığar");
        dest_hash[..8].copy_from_slice(&index.to_be_bytes());
        PeerRef::from_dest_hash(dest_hash)
    }

    /// Verilen her eşle hazır oturumu olan, `store` deposuyla kurulmuş motor.
    fn messenger_on(
        store: Arc<dyn AppStore>,
        peers: impl IntoIterator<Item = PeerRef>,
    ) -> Messenger<ReadyTransport> {
        let mut messenger =
            Messenger::with_store(ReadyTransport::default(), store).expect("limitler geçerli");
        for p in peers {
            assert!(matches!(messenger.connect(p), Ok(ConnectResult::Ready(_))));
        }
        messenger
    }

    /// Verilen her eşle hazır oturumu olan, taze bellek içi depolu motor.
    fn messenger_with(peers: impl IntoIterator<Item = PeerRef>) -> Messenger<ReadyTransport> {
        messenger_on(Arc::new(MemoryAppStore::new()), peers)
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

    /// `peer`'den gelmiş gibi bir `PTT_CLOSE` olayı.
    fn close_from(peer: PeerRef, stream_id: StreamId) -> MessengerEvent {
        MessengerEvent::Other {
            peer,
            message: AppMessage::PttClose { stream_id },
        }
    }

    /// `peer`'in `stream_id` akışını açıp `PTT_CLOSE` ile kapatır (alıcı tarafı).
    fn open_and_close<T: KamaTransport>(
        ptt: &mut Ptt,
        messenger: &mut Messenger<T>,
        peer: PeerRef,
        stream_id: StreamId,
    ) {
        assert!(matches!(
            ptt.on_event(messenger, open_from(peer, stream_id)),
            Some(PttEvent::PttStarted { .. })
        ));
        assert_eq!(
            ptt.on_event(messenger, close_from(peer, stream_id)),
            Some(PttEvent::PttEnded { peer, stream_id })
        );
    }

    /// PTT-R1 kapanmış akış kaydı sınırlıdır: eş başına en çok [`PTT_MAX_ACTIVE_STREAMS`] kimlik (en
    /// eski kapanış düşer), toplam en çok [`MAX_INCOMING_STREAMS`] (en eski kapanış yer açar), ömür
    /// [`PTT_STREAM_IDLE_TTL`]. Kayıttaki kimliğe gelen `PTT_OPEN` akış başlatmaz; düşmüş ya da süresi
    /// dolmuş kimliğe gelen başlatır.
    ///
    /// `b1cce203`'te (K4G-08'in inceleme öncesi hâli) UYARLANAMAZ: kayıt yoktu, kapanmış her
    /// kimliğe gelen `PTT_OPEN` yeni akış başlatıyordu.
    #[test]
    fn closed_stream_record_is_bounded_and_expires() {
        let clock = TestClock::default();
        let p = peer(0);
        let mut messenger = messenger_with([p]);
        let mut ptt = Ptt::new(Arc::new(clock.clone()));
        // Üç akış birer saniye arayla kapanır: eşin kaydında son ikisi kalır.
        for stream_id in 0..3 {
            open_and_close(&mut ptt, &mut messenger, p, stream_id);
            clock.advance(Duration::from_secs(1));
        }
        assert_eq!(ptt.closed_of(p).count(), PTT_MAX_ACTIVE_STREAMS);
        for stream_id in [1, 2] {
            assert_eq!(ptt.on_event(&mut messenger, open_from(p, stream_id)), None);
        }
        assert_eq!(ptt.stats().opens_after_close, 2);
        assert!(ptt.incoming_streams(p).is_empty());
        // Düşen en eski kimlik ve süresi dolan kayıt artık engellemez.
        assert!(matches!(
            ptt.on_event(&mut messenger, open_from(p, 0)),
            Some(PttEvent::PttStarted { .. })
        ));
        clock.advance(PTT_STREAM_IDLE_TTL);
        assert!(matches!(
            ptt.on_event(&mut messenger, open_from(p, 2)),
            Some(PttEvent::PttStarted { .. })
        ));
        assert_eq!(ptt.stats().opens_after_close, 2);

        // Toplam tavan: her eş kendi tavanında; yeni kapanış en eski kaydın yerini alır.
        let mut full = messenger_with((0..MAX_PEERS).map(peer));
        let extra = peer(MAX_PEERS);
        let mut other = messenger_with([extra]);
        let mut ptt = Ptt::new(Arc::new(TestClock::default()));
        for n in 0..MAX_PEERS {
            for stream in 0..PTT_MAX_ACTIVE_STREAMS {
                let stream_id = StreamId::try_from(stream).expect("küçük sayı");
                open_and_close(&mut ptt, &mut full, peer(n), stream_id);
            }
        }
        assert_eq!(ptt.closed.len(), MAX_INCOMING_STREAMS);
        open_and_close(&mut ptt, &mut other, extra, FIRST_STREAM_ID);
        assert_eq!(ptt.closed.len(), MAX_INCOMING_STREAMS);
        assert_eq!(
            ptt.on_event(&mut other, open_from(extra, FIRST_STREAM_ID)),
            None
        );
    }

    /// Depo eşin `u32` alanının tükendiğini bildirince: tipli hata, sarma yok, `PTT_OPEN` yok.
    ///
    /// `0904f723`'te UYARLANAMAZ: sayaç motorun belleğindeydi (`next_stream_id`), depo okunmuyordu;
    /// eski test o alanı doğrudan kuruyordu.
    #[test]
    fn stream_id_exhaustion_is_typed_and_never_wraps() {
        let p = peer(0);
        let store = Arc::new(SeededStore::counting_from(u64::from(StreamId::MAX)));
        let mut messenger = messenger_on(store, [p]);
        let mut ptt = Ptt::new(clock());
        let last = ptt
            .open(&mut messenger, p, PttCodec::Opus, TEST_BUNDLE_MS)
            .expect("son stream_id kullanılabilir");
        assert_eq!(last, StreamId::MAX);
        ptt.close(&mut messenger, last).expect("kapanır");
        let sent = messenger.transport().sent_on.len();
        for _ in 0..2 {
            assert_eq!(
                ptt.open(&mut messenger, p, PttCodec::Opus, TEST_BUNDLE_MS),
                Err(PttError::StreamIdExhausted),
                "tükenme kalıcıdır"
            );
        }
        assert_eq!(ptt.outgoing_stream(p), None);
        assert_eq!(messenger.transport().sent_on.len(), sent, "PTT_OPEN yok");
    }

    /// Depo sözleşmesine aykırı değerler kullanılmaz: tel alanını aşan değer ve (açık bir akışla
    /// çakışan değeri) yineleyen depo `Store(Corrupt)` verir; atlama döngüsü sınırlıdır; akış
    /// açılmaz, `PTT_OPEN` gönderilmez.
    ///
    /// `0904f723`'te UYARLANAMAZ: motor depoyu hiç okumuyordu (`PttError::Store` yoktu).
    #[test]
    fn a_store_that_breaks_the_counter_contract_is_corrupt_and_opens_nothing() {
        let p = peer(0);
        let wide = Arc::new(SeededStore::repeating(u64::from(StreamId::MAX) + 1));
        let mut messenger = messenger_on(wide, [p]);
        let mut ptt = Ptt::new(clock());
        assert_eq!(
            ptt.open(&mut messenger, p, PttCodec::Opus, TEST_BUNDLE_MS),
            Err(PttError::Store(AppStoreError::Corrupt))
        );
        assert_eq!(ptt.outgoing_stream(p), None);
        assert!(messenger.transport().sent_on.is_empty());

        let (a, b) = (peer(0), peer(1));
        let mut messenger = messenger_on(Arc::new(SeededStore::repeating(7)), [a, b]);
        let mut ptt = Ptt::new(clock());
        assert_eq!(
            ptt.open(&mut messenger, a, PttCodec::Opus, TEST_BUNDLE_MS),
            Ok(7)
        );
        assert_eq!(
            ptt.open(&mut messenger, b, PttCodec::Opus, TEST_BUNDLE_MS),
            Err(PttError::Store(AppStoreError::Corrupt)),
            "a'nın açık akışıyla çakışan değeri yineleyen depo"
        );
        assert_eq!(ptt.outgoing_stream(b), None);
        assert_eq!(ptt.outgoing_stream(a), Some(7), "a'nın akışı ezilmedi");
        assert_eq!(
            messenger.transport().sent_on.len(),
            1,
            "yalnız a'nın OPEN'ı"
        );
    }

    /// Giden tablo eş başına bir akışla [`MAX_PEERS`]'te, gelen tablo eş başına
    /// [`PTT_MAX_ACTIVE_STREAMS`] akışla `MAX_PEERS × PTT_MAX_ACTIVE_STREAMS`'te durur. Eş başına
    /// sayaçlar her yeni eşe ilk değeri önerir; açık akışların değerleri atlanır (eş n, n'yi alır —
    /// son eş tam `MAX_PEERS − 1` atlamayla).
    ///
    /// `0904f723`'te UYARLANAMAZ (gelen tablo eş başına bir akıştı, `incoming_streams` yoktu);
    /// uyarlansa gelen tabloda eş başına ikinci akış en yeni kazanır kuralıyla birinciyi bitirirdi.
    #[test]
    fn stream_tables_are_bounded() {
        // Bir motorun eşleri `MAX_PEERS` ile sınırlı; tavanın ötesindeki eş ikinci motordadır.
        let mut full = messenger_with((0..MAX_PEERS).map(peer));
        let extra = peer(MAX_PEERS);
        let mut other = messenger_with([extra]);
        let mut ptt = Ptt::new(clock());
        for n in 0..MAX_PEERS {
            let stream_id = ptt
                .open(&mut full, peer(n), PttCodec::Opus, TEST_BUNDLE_MS)
                .expect("tavanın altında");
            assert_eq!(
                stream_id,
                StreamId::try_from(n).expect("küçük sayı"),
                "açık akışların değerleri atlandı"
            );
            for stream in 0..PTT_MAX_ACTIVE_STREAMS {
                let stream = StreamId::try_from(stream).expect("küçük sayı");
                assert!(matches!(
                    ptt.on_event(&mut full, open_from(peer(n), stream)),
                    Some(PttEvent::PttStarted { .. })
                ));
            }
        }
        assert_eq!(ptt.incoming.len(), MAX_INCOMING_STREAMS);
        assert_eq!(
            ptt.open(&mut other, extra, PttCodec::Opus, TEST_BUNDLE_MS),
            Err(PttError::StreamTableFull)
        );
        assert_eq!(ptt.outgoing_stream(extra), None);
        assert_eq!(
            ptt.on_event(&mut other, open_from(extra, FIRST_STREAM_ID)),
            None
        );
        assert_eq!(ptt.stats().opens_refused, 1);
        assert!(ptt.incoming_streams(extra).is_empty());
        assert_eq!(ptt.incoming.len(), MAX_INCOMING_STREAMS);
        assert_eq!(ptt.incoming_streams(peer(0)), vec![0, 1]);
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
