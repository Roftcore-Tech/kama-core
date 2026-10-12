//! Dosya aktarımı (B2.3; K4G-09) — AMP `FILE_BEGIN` (0x02) · `FILE_CHUNK` (0x03) · `FILE_END` (0x04) ·
//! `FILE_ABORT` (0x05) · `FILE_RECEIPT` (0x0B) (`specs/application-message-protocol.md` §3–§4; S7/S8
//! 2026-10-08; K4G-09 yaması 2026-10-12: dosya alındısı, zamanlayıcılar, gönderen-tek `FILE_ABORT`;
//! plan D5, D6).
//!
//! [`FileTransfer`] [`Messenger`]'ın yanında durur ve Transport'u sahiplenmez.
//! [`Messenger::poll`]'u OKUMAZ: olayları zincirdeki okuyucudan alır (B2.4'te `Ptt::poll` →
//! `PttEvent::Messenger`).
//!
//! * **Gelen** — [`FileTransfer::handle`] `FILE_*` kaydı taşıyan [`MessengerEvent::Other`]'ı
//!   TÜKETİR, diğer her olayı
//!   AYNEN geri verir. (Eş, `transfer_id`) başına birleştirir: parçalar ve `FILE_END` her sırada
//!   gelebilir, tekrarlar etkisizdir. `FILE_END` görülmüş ve bütün parçalar gelmişse
//!   `whole_file_hash` doğrulanır: tutarsa [`FileEvent::Received`] ve eşe `FILE_RECEIPT(ok)`;
//!   tutmazsa [`FileEvent::Failed`] ve eşe `FILE_RECEIPT(hash_mismatch)`.
//! * **Giden** — [`FileTransfer::send_file`] dosyayı (bellekte; bu sprint dosya sistemi yok)
//!   kuyruğa alır ve kalıcı depodan `transfer_id` ayırır; [`FileTransfer::pump`] kayıtları eşin
//!   güncel oturumuna ([`Messenger::session_of`] + [`Messenger::transport_mut`])
//!   `DeliveryPolicy::Reliable` ile sırayla sunar: `FILE_BEGIN` → `FILE_CHUNK` × `chunk_count` →
//!   `FILE_END`. `FILE_END` kabul edilince [`FileEvent::Sent`] (Transport kabulü, teslim DEĞİL) doğar
//!   ve aktarım eşin `FILE_RECEIPT`'ini bekler: `ok` → [`FileEvent::Delivered`]; başka her son
//!   [`FileEvent::SendFailed`]. Sıkıştırma yok (bu sprint `compression_id` = NONE; AMP §5 kural 6:
//!   `FILE_CHUNK` → kapalı).
//!
//! Motorun kendi olayları [`FileTransfer::poll_event`] kuyruğundadır. Bir uygulama turu — sıra
//! önemlidir: zamanlayıcılar YALNIZ [`FileTransfer::pump`]'ta, Messenger kuyruğu boşaltıldıktan SONRA
//! denetlenir ("Zamanlayıcılar"):
//!
//! ```text
//! while let Some(ev) = ptt.poll(&mut m) { PttEvent::Messenger(ev) → files.handle(&mut m, ev) → … }
//! files.pump(&mut m);
//! while let Some(fe) = files.poll_event() { … }
//! ```
//!
//! # Retry yok (AMP §3, S1 K8, H-APP-RETRY-0)
//!
//! Application parça yeniden gönderimi, parça ACK'i, kayıp tespiti ve parça başına MAC/özet TUTMAZ:
//! kayıp Transport'un (`Reliable`), bütünlük Crypto'nun işidir; bütün-dosya özeti yalnız birleştirmenin
//! doğruluğunu sınar. Transport'un KABUL ettiği bir kayıt bir daha sunulmaz — oturum değişiminde de
//! (S2'nin TEXT yeniden gönderimi dosyaya UYGULANMAZ; kaldığı yerden devam YOK, "Oturum değişimi").
//! Transport'un REDDETTİĞİ kayıt tele çıkmamıştır: kuyrukta bekler ve sonraki
//! [`FileTransfer::pump`]'ta yeniden sunulur (K8 "bekler") — alındılar da. `FILE_RECEIPT` dosya
//! düzeyinde bir sonuç bildirimidir, parça ACK'i değildir: hiçbir yeniden gönderimi tetiklemez, tek
//! etkisi gönderende aktarımı bitirmektir. Özet tutmazsa çözüm parça yeniden göndermek DEĞİLDİR;
//! Transport'un Reliable sözleşmesine bakılır (BL B2.3).
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
//! aktarımları oluşturma sırasıyla TEK TEK sunar (alındı bekleyen aktarım sıradakini bekletmez);
//! eşler arasında tur başına bir kayıt (adil).
//!
//! # Alıcı kararları (S8: vektörsüz karar → fail-closed DROP; panik yok)
//!
//! * `FILE_BEGIN`: `(file_size = 0) ≠ (chunk_count = 0)` ya da `chunk_count > file_size` → DROP,
//!   sessiz (S8; alındı yok). Aynı (eş, `transfer_id`) için aynı alanlar → tekrar; farklı alanlar →
//!   DROP, sessiz (süren aktarım korunur; bir ret alındısı onu da bitirirdi). Kabul edilmeyen aktarım
//!   eşe `FILE_RECEIPT(rejected)` ile bildirilir — üç tetik: `file_size > MAX_REASSEMBLY_BYTES` (S7),
//!   yuva yok, aktarımın `FILE_ABORT`'u `FILE_BEGIN`'inden ÖNCE gelmiş (erken iptal kaydı, "Oturum
//!   değişimi"). Reddedilen aktarımın yerel olayı yoktur. Boyut her tahsisten ÖNCE denetlenir;
//!   birleştirme tamponu ve parça bit haritası ilk kabul edilen parçada ayrılır.
//! * `FILE_CHUNK` / `FILE_END`: bilinmeyen aktarım → DROP, sessiz (ön-tampon yok: aynı oturumda
//!   `FILE_BEGIN` her zaman önce gelir, S1 §4). Geometriye uymayan parça → DROP; durum bozulmaz.
//!   Eksik parça varken gelen `FILE_END` aktarımı bitirmez: eksikler [`FILE_IDLE_TTL`]'e dek beklenir.
//! * Yerel iptal: [`FileTransfer::cancel_incoming`] kabul edilmiş gelen aktarımı bırakır ve eşe
//!   `FILE_RECEIPT(aborted_by_receiver)` verir; olay yok (çağıran biliyor).
//!
//! # `FILE_ABORT` ve `FILE_RECEIPT` yönü (K4G-09, plan D6 — DUR #108 çözümü)
//!
//! `FILE_ABORT` yalnız GÖNDERENİNDİR (düzen değişmedi, yön alanı yok): bu motor onu yalnız kendi
//! GİDEN aktarımı için üretir (`user_cancel` — [`FileTransfer::cancel_send`]; `superseded` — oturum
//! değişimi) ve eşten geleni, nedeni ne olursa olsun, yalnız o eşin GÖNDERDİĞİ aktarıma (bizim gelen
//! aktarımımıza) uygular. `hash_mismatch` ve `storage_full` (eski alıcı nedenleri) üretilmez; gelirse
//! diğerleri gibi işlenir. `FILE_RECEIPT` yalnız ALICININDIR: bu motor onu gelen aktarım başına en çok
//! bir kez, aktarımın sonunda (`ok` · `hash_mismatch` · `aborted_by_receiver` · `timeout`) ya da
//! `FILE_BEGIN`'i kabul etmediğinde (`rejected`) üretir; eşten geleni yalnız o eşe GİDEN aktarıma
//! uygular. Kimlik başka bir eşin aktarımıyla eşleşmez. Alıcının hiçbir yolu `FILE_ABORT` üretmez.
//!
//! Alındılar tek kuyruktan, eşin O ANKİ güncel oturumuna `Reliable` ile sunulur. Oturum yoksa ya da
//! Transport geçici reddederse alındı bekler (tele çıkmamıştır; yeniden iletim DEĞİL) ve sonraki
//! sunumda yeniden denenir: alıcının kendi parçalarının kuyruğu doldurduğu iki yönlü aktarımda `ok`
//! alındısı kaybolup gönderende yanlış başarısızlık doğurmasın. Kuyruk [`MAX_INCOMING_TRANSFERS`]
//! alındı tutar (alındı bir gelen aktarımın son sözüdür; yeni sayı yok): taşarsa en eski düşer;
//! [`FILE_RECEIPT_TTL`]'den eski alındı da düşer (gönderen o aktarımdan çoktan vazgeçmiştir). İkisi
//! de `receipts_unsent`'te sayılır.
//!
//! # Zamanlayıcılar ([`FILE_IDLE_TTL`], [`FILE_RECEIPT_TTL`]; AMP §3 K4G-09)
//!
//! * Alıcı: kabul edilmiş bir gelen aktarıma [`FILE_IDLE_TTL`] boyunca yeni parça gelmezse yuva
//!   boşalır, eşe `FILE_RECEIPT(timeout)` gider ve [`FileEvent::Failed`] (`IdleTimeout`) doğar. Süre
//!   `FILE_BEGIN` kabul edilince başlar ve kabul edilen her YENİ parçada sıfırlanır; tekrar, atılan
//!   parça ve `FILE_END` sıfırlamaz.
//! * Gönderen: `FILE_END` kabul edildikten sonra [`FILE_RECEIPT_TTL`] içinde alındı gelmezse aktarım
//!   [`FileEvent::SendFailed`] (`ReceiptTimeout`) ile biter; eşe bir şey gitmez. Sonra gelen alındı
//!   yok sayılır (`receipts_ignored`).
//!
//! Zaman enjekte edilen saatten ([`MonotonicClock`]) okunur; kayıt İŞLENDİĞİ anda damgalanır. İki
//! denetim de YALNIZ [`FileTransfer::pump`] içindedir ve turun sonunda, Messenger kuyruğu
//! boşaltıldıktan sonra yapılmalıdır: Transport kuyruğunda bekleyen parça ya da alındı önce işlenir ve
//! aktarımı canlı tutar (K4G-08 PTT-R4 dersi). Trafik olmasa da uygulama `pump`'ı periyodik
//! çağırmalıdır; yoksa süresi dolan aktarım ancak sonraki `pump`'ta biter.
//!
//! # Oturum değişimi (plan D6; kaldığı yerden devam YOK)
//!
//! Gönderen her giden aktarım için `FILE_BEGIN`'i KABUL eden tutamacı tutar. Aktarım yarımken (son
//! kaydı `FILE_END` henüz kabul edilmemiş) eşin güncel oturumu ([`Messenger::session_of`]) başka bir
//! tutamaçsa kalan kayıtlar GÖNDERİLMEZ: aktarım yeni oturumdan `FILE_ABORT(superseded)` ile kapatılır
//! ve [`FileEvent::SendFailed`] (`Superseded`) doğar. Yeniden gönderim, uygulamanın yeni
//! `transfer_id`'li yeni [`FileTransfer::send_file`]'ıdır (UI "yeniden gönder"). Denetim kaydın
//! sunulduğu anda yapılır, yalnız `Ready` olayına güvenilmez: yeni oturum olay okunmadan da
//! benimsenebilir (senkron [`Messenger::connect`] → `ConnectResult::Ready`). Eşin oturumu yokken yarım
//! aktarım bekler; sıradaki oturumda aynı kuralla kapanır. `FILE_ABORT` geçici reddedilirse bekler.
//! `FILE_END`'i kabul edilmiş aktarım KAPATILMAZ: alındısını beklemeye devam eder (eski oturumun
//! yalnız-alım drain'iyle tamamlanmış olabilir; olmadıysa iki zamanlayıcı bitirir).
//!
//! Erken iptal kaydı (alıcı): supersession'da yarım aktarımın `FILE_BEGIN`'i eski oturumun yalnız-alım
//! drain'inde kalabilir (S1 K7, KS-GRACE-1: `N_g = 64` / `T_g = 30 s`) ve yeni oturumdan gelen
//! `FILE_ABORT(superseded)` ondan ÖNCE işlenebilir. Geç gelen `FILE_BEGIN` aktarımı başlatsaydı,
//! göndereni vazgeçmiş ve hiç bitmeyecek bir hayalet aktarım [`FILE_IDLE_TTL`] boyunca bir yuva tutar,
//! sonunda sahte bir `IdleTimeout` olayı doğururdu. Bu yüzden izlenen bir gelen aktarıma denk gelmeyen
//! her `FILE_ABORT`'un (eş, `transfer_id`)'si kayda girer; kayıttaki aktarımın `FILE_BEGIN`'i kabul
//! edilmez, eşe `FILE_RECEIPT(rejected)` gider (kabul edilmeden iptal edilmiş aktarım sessiz düşmez)
//! ve kayıt silinir. Bitmiş ya da iptal edilmiş, `FILE_BEGIN`'i İŞLENMİŞ bir aktarım kayda GİRMEZ ve
//! gerekmez: her kayıt en çok bir kez teslim edilir (S1 §4) ve Application kabul edilmiş kaydı yeniden
//! sunmaz (H-APP-RETRY-0) — o aktarımın ikinci bir `FILE_BEGIN`'i gelemez; geç parçaları bilinmeyen
//! aktarım olarak atılır. Kayıt eş başına [`MAX_INCOMING_TRANSFERS_PER_PEER`], toplam
//! `MAX_PEERS × MAX_INCOMING_TRANSFERS_PER_PEER` kimlik tutar (en eski düşer); ömrü [`FILE_IDLE_TTL`]
//! — yeni sayı yok. Türetme: drain `T_g = 30 s` < 60 s; supersession anında eş başına en çok bir yarım
//! aktarım vardır (gönderen bir eşe aktarımları tek tek sunar); iki kimlik art arda iki supersession'a
//! pay bırakır. Kayıttaki kimliğe gelen `FILE_BEGIN` `begins_after_abort`'ta sayılır.
//!
//! # `transfer_id` — eş başına, kalıcı (plan D5; K4G-06 `CounterKind::TransferId`)
//!
//! Her `transfer_id` `AppStore::reserve_next(eş, CounterKind::TransferId)` ile [`Messenger`]'ın
//! deposundan ayrılır (Application'ın tek deposu; bileşim kökü onu bir kez bağlar). Depo değeri
//! döndürmeden ÖNCE kalıcılaştırır: aynı depoyla yeniden kurulan motor sayaca kaldığı yerden devam eder
//! ve aynı eşe sıradaki `transfer_id` öncekilerin hepsinden büyüktür. Motorda bellek içi sayaç YOKTUR.
//!
//! [`FileTransfer::cancel_send`] eş almaz (K4G-11'in `cancel_file(transfer_id)`'si de); eş başına
//! sayaçlarda iki eş aynı değeri alabilir. Bu yüzden [`FileTransfer::send_file`] AÇIK giden aktarımlar
//! (alındı bekleyenler dahil) arasında `transfer_id` benzersizliğini korur: depo başka bir eşin açık
//! aktarımının değerini verirse o değer atlanır ve yenisi ayrılır. Atlama sınırı: depo değerleri eş
//! başına kesin artandır, bu yüzden her açık aktarımın değeri en çok bir kez çıkar; eşin KENDİ açık
//! aktarımları (aynı eşe birden çok aktarım kuyrukta olabilir) sayacın gerisinde kaldığı için hiç
//! çakışmaz. Atlama en çok açık aktarım sayısı kadardır (`≤ MAX_OUTGOING_TRANSFERS − 1`); döngü bu
//! sınırla kesilir, aşan (aynı değeri yineleyen) depo sözleşmeye aykırıdır → [`FileSendError::Store`]
//! (`Corrupt`).
//!
//! Hata yolu politikası: hiçbir hata yolu değer AYIRMAZ. Bütün girdi denetimleri (oturum, yuva, boyut,
//! parça yeri, `FILE_BEGIN`'in bir kayda sığması — boyu kimlikten bağımsızdır) ayırmadan ÖNCE yapılır
//! ve ayırmadan sonra başarısız olabilecek adım yoktur; depo hatasında değer zaten ayrılmamıştır (depo
//! sözleşmesi). Yalnız atlanan değerler YANAR (boşluk); alıcı `transfer_id` sırasına anlam yüklemez
//! (AMP §3 sıra kuralı koymaz). Depo `u64` alanının tükendiğini kalıcı olarak bildirir →
//! [`FileSendError::TransferIdExhausted`]; sarma YOK. Diğer depo hataları [`FileSendError::Store`]:
//! `FILE_BEGIN` gitmez, aktarım kuyruğa girmez.
//!
//! # Sınırlar ve olay kuyruğu
//!
//! Alıcı: aktarım başına `file_size ≤ MAX_REASSEMBLY_BYTES` (S7); eşzamanlı gelen aktarım eş başına
//! [`MAX_INCOMING_TRANSFERS_PER_PEER`], toplam [`MAX_INCOMING_TRANSFERS`]. Gönderen:
//! [`MAX_OUTGOING_TRANSFERS`] aktarım — alındı bekleyenler dahil; dosya baytları son kayıt kabul
//! edilene kadar tutulur. Her SON olay (`Received`, `Failed`, `Aborted`, `Delivered`, `SendFailed`)
//! doğduğu aktarımın YUVASINI [`FileTransfer::poll_event`] ile alınana kadar tutar; ara olay `Sent`
//! yuva tutmaz (aktarım tabloda kalır, yuvayı o tutar). Böylece kuyruk tavanı türetilmiştir (gelen
//! tavanı + 2 × giden tavanı: giden aktarım bir ara ve bir son olay üretir; yeni sayı yok), olay
//! düşürülmez ve boşaltılmayan kuyruk yeni aktarımları durdurur (geri basınç). Alındı kuyruğu ve erken
//! iptal kaydı yukarıdaki tavanlarla sınırlıdır; her tavan eklemeden ÖNCE denetlenir. En kötü durumda
//! birleştirme belleği `MAX_INCOMING_TRANSFERS × MAX_REASSEMBLY_BYTES`'tır (+ bit haritası, aktarım
//! başına ≤ `file_size / 8`).
//!
//! # Bilinen sınırlar (beyanlı)
//!
//! * Kaldığı yerden devam YOK: supersession yarım aktarımı bitirir; yeniden gönderim tam aktarımdır.
//! * [`FILE_RECEIPT_TTL`]'den uzun bir kesintiden sonra gelen alındı yok sayılır: aktarım gönderende
//!   `ReceiptTimeout` kalır, alıcıda teslim edilmiş olabilir (yanlış "başarısız"; yanlış "teslim
//!   edildi" DEĞİL). [`FILE_IDLE_TTL`]'den uzun parçasız kesinti de aktarımı alıcıda bitirir.
//! * Zamanlayıcılar yalnız [`FileTransfer::pump`]'ta işler: periyodik çağrı gerekir (yukarıda).
//! * Kapatılmış bir `transfer_id` başka bir eşin açık aktarımına yeniden verilebilir: sayaçlar eş
//!   başınadır ve benzersizlik yalnız AÇIK giden aktarımlar arasında korunur (K4G-08'in `stream_id`
//!   kararıyla aynı). Kapattığı kimliği yeniden kullanan çağıran (use-after-close)
//!   [`FileTransfer::cancel_send`] ile o başka eşin aktarımını iptal eder: çağıran kapattığı kimliği
//!   KULLANMAMALIDIR. Olaylar (eş, `transfer_id`) taşır; K4G-11 (`cancel_file(transfer_id)` eş almaz)
//!   bu sınırı düğüm-tekil kendi tutamacıyla kapatmalıdır.
//! * Erken iptal kaydı taşarsa ([`FILE_IDLE_TTL`] içinde aynı eşten ikiden çok eşleşmeyen
//!   `FILE_ABORT`) geç `FILE_BEGIN` hayalet aktarım başlatabilir; o aktarım en geç [`FILE_IDLE_TTL`]
//!   sonra biter.
//! * Alındı sınırlı bekler: kuyruk taşarsa ya da alındı [`FILE_RECEIPT_TTL`] içinde Transport'a
//!   verilemezse düşer; gönderen o aktarımı kendi zamanlayıcısıyla başarısız sayar.
//! * K4G-09 öncesi bir eş 0x0B'yi tanımaz (AMP §6: DROP): ona giden aktarım `ReceiptTimeout` ile
//!   biter. PoC'de iki uç aynı yapıdır.
//! * D4 (PoC-KAPSAM-001 §5 EK-1): kalıcı durum tamamen kaybolur ve aynı kimlik yeniden kullanılırsa
//!   `transfer_id`'ler baştan başlar; eşte hâlâ duran (birleştirilen ya da erken iptal kaydındaki) bir
//!   kimlik yeniden gelirse o aktarım tamamlanmaz ya da reddedilir. Kural: yeniden provisioning.
//!
//! # `whole_file_hash` (blake2 DAR istisnası, Abkarada 2026-10-08)
//!
//! BLAKE2s-256 (anahtarsız, 32 B; AMP §3 0x02 "ccr primitive") YALNIZ [`whole_file_hash`]'te
//! hesaplanır — `blake2`'nin bu crate'teki tek kullanımı; `scripts/ci/layer_check.py` L1 onu adıyla
//! geçirir. Özet gizli değildir; karşılaştırma sabit-zamanlı değildir.
//!
//! # Gizlilik
//!
//! [`FileEvent`]'in `Debug` çıktısı dosya adını ve baytlarını değil uzunluklarını yazar; kuyruk,
//! alındı kuyruğu ve birleştirme durumu hiçbir `Debug` çıktısına girmez ([`FileTransfer`] `Debug`
//! değildir).

use core::fmt;
use core::time::Duration;
use std::collections::btree_map::Range;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{DeliveryPolicy, KamaTransport, TransportError};

use crate::clock::MonotonicClock;
use crate::codec::limits::WHOLE_FILE_HASH_BYTES;
use crate::codec::{self, AppMessage, EncodeError, FileAbortReason, FileReceiptStatus};
use crate::messaging::{Messenger, MessengerEvent, MAX_PEERS};
use crate::store::{AppStore, AppStoreError, CounterKind};

/// `FILE_*.transfer_id` (AMP §3; telde `u64_be`).
pub type TransferId = u64;

/// Taze depoda bir eşe atanan ilk `transfer_id`. Spec başlangıç değeri koymaz; değer bu uygulamanındır
/// ve depo onu `store::FIRST_COUNTER_VALUE` olarak verir (eşitlik `store`'da derleme zamanında
/// denetlenir).
pub const FIRST_TRANSFER_ID: TransferId = 0;

/// S7 (Abkarada 2026-10-08) — alıcının birleştirdiği en büyük dosya (`FILE_BEGIN.file_size`,
/// bayt). Gönderen de daha büyüğünü kuyruğa almaz: eş onu `rejected` ile reddeder.
pub const MAX_REASSEMBLY_BYTES: u64 = 64 * 1024 * 1024;

/// AMP §3 K4G-09 (plan D6): alıcı zamanlayıcısı — kabul edilmiş bir gelen aktarıma bu süre boyunca
/// yeni `FILE_CHUNK` gelmezse yuva boşalır ve eşe `FILE_RECEIPT(timeout)` gider. Aynı süre erken iptal
/// kaydının ömrüdür (modül belgesi, "Oturum değişimi").
pub const FILE_IDLE_TTL: Duration = Duration::from_secs(60);

/// AMP §3 K4G-09 (plan D6): gönderen zamanlayıcısı — `FILE_END` Transport'a kabul ettirildikten sonra
/// bu süre içinde `FILE_RECEIPT` gelmezse aktarım [`SendFailure::ReceiptTimeout`] ile biter. Aynı süre
/// alıcıda bekleyen bir alındının ömrüdür.
pub const FILE_RECEIPT_TTL: Duration = Duration::from_secs(60);

/// Eş başına eşzamanlı gelen aktarım tavanı — yerel bellek sınırı, normatif DEĞİL `[KALİBRASYON]`.
/// Bu motorun göndereni bir eşe aktarımları tek tek sunar; ikinci yuva, göndereni kaybolmuş yarım bir
/// aktarım dururken (en geç [`FILE_IDLE_TTL`] sonra biter) yenisine yer bırakır.
pub const MAX_INCOMING_TRANSFERS_PER_PEER: usize = 2;

/// Toplam gelen aktarım yuvası (alınmamış alıcı olayları dahil) — yerel bellek sınırı, normatif
/// DEĞİL `[KALİBRASYON]`.
pub const MAX_INCOMING_TRANSFERS: usize = 4;

/// Giden aktarım yuvası (alındı bekleyen aktarımlar ve alınmamış son gönderen olayları dahil; dosya
/// baytları son kayıt kabul edilene kadar tutulur) — yerel bellek sınırı, normatif DEĞİL
/// `[KALİBRASYON]`.
pub const MAX_OUTGOING_TRANSFERS: usize = 4;

/// Olay kuyruğu tavanı — türetilmiş: her son olay doğduğu aktarımın yuvasını tutar, giden aktarım ek
/// olarak bir ara olay (`Sent`) üretir (modül belgesi, "Sınırlar ve olay kuyruğu").
const MAX_PENDING_FILE_EVENTS: usize = MAX_INCOMING_TRANSFERS + 2 * MAX_OUTGOING_TRANSFERS;

/// Transport'a henüz kabul ettirilememiş alındı tavanı — türetilmiş: alındı bir gelen aktarımın son
/// sözüdür (modül belgesi, "`FILE_ABORT` ve `FILE_RECEIPT` yönü").
const MAX_PENDING_RECEIPTS: usize = MAX_INCOMING_TRANSFERS;

/// Erken iptal kaydının toplam tavanı — türetilmiş: eş tablosu tavanı × eş başına gelen yuva tavanı
/// (modül belgesi, "Oturum değişimi").
const MAX_EARLY_ABORTS: usize = MAX_PEERS * MAX_INCOMING_TRANSFERS_PER_PEER;

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

/// [`FileEvent::Failed`] nedeni — GELEN aktarım; eşe karşılık gelen `FILE_RECEIPT` gitti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFailure {
    /// Birleştirilen baytların BLAKE2s-256 özeti `FILE_BEGIN.whole_file_hash`'e eşit değil
    /// (`FILE_RECEIPT(hash_mismatch)`).
    HashMismatch,
    /// [`FILE_IDLE_TTL`] boyunca yeni parça gelmedi (`FILE_RECEIPT(timeout)`).
    IdleTimeout,
}

/// [`FileEvent::SendFailed`] nedeni — GİDEN aktarım. İlk dördü eşin `FILE_RECEIPT`'idir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendFailure {
    /// `FILE_RECEIPT(aborted_by_receiver)`: alıcının kullanıcısı iptal etti.
    AbortedByReceiver,
    /// `FILE_RECEIPT(hash_mismatch)`: alıcıda özet tutmadı.
    HashMismatch,
    /// `FILE_RECEIPT(timeout)`: alıcıya [`FILE_IDLE_TTL`] boyunca parça ulaşmadı.
    ReceiverTimeout,
    /// `FILE_RECEIPT(rejected)`: alıcı `FILE_BEGIN`'i kabul etmedi.
    Rejected,
    /// `FILE_END` kabul edildikten sonra [`FILE_RECEIPT_TTL`] içinde alındı gelmedi.
    ReceiptTimeout,
    /// Yarım aktarımın oturumu değişti; aktarım `FILE_ABORT(superseded)` ile kapatıldı (plan D6).
    Superseded,
    /// Transport bir kaydı kalıcı hatayla reddetti (geçici olmayan her `TransportError`); eşe sinyal
    /// gitmedi.
    Transport(TransportError),
}

impl SendFailure {
    /// Eşin alındısının gönderendeki sonucu; `ok` → `None` (teslim).
    const fn from_receipt(status: FileReceiptStatus) -> Option<Self> {
        match status {
            FileReceiptStatus::Ok => None,
            FileReceiptStatus::AbortedByReceiver => Some(Self::AbortedByReceiver),
            FileReceiptStatus::HashMismatch => Some(Self::HashMismatch),
            FileReceiptStatus::Timeout => Some(Self::ReceiverTimeout),
            FileReceiptStatus::Rejected => Some(Self::Rejected),
        }
    }
}

/// [`FileTransfer::poll_event`] çıktısı. Kabul edilmiş her gelen aktarım tam bir SON olayla biter
/// (`Received` / `Failed` / `Aborted`) — yerel iptal ([`FileTransfer::cancel_incoming`]) hariç;
/// kuyruğa alınmış her giden aktarım `FILE_END` kabulünde bir ara olay (`Sent`) ve tam bir son olayla
/// biter (`Delivered` / `SendFailed`) — kullanıcı iptali ([`FileTransfer::cancel_send`]) hariç.
#[derive(Clone, PartialEq, Eq)]
pub enum FileEvent {
    /// Gelen dosya tamamlandı ve `whole_file_hash` tuttu; birleştirme durumu atıldı, eşe
    /// `FILE_RECEIPT(ok)` gitti.
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
    /// Gelen aktarım başarısız bitti; baytlar atıldı, eşe nedenin `FILE_RECEIPT`'i gitti.
    Failed {
        /// Gönderen eş.
        peer: PeerRef,
        /// Gönderenin `transfer_id`'si.
        transfer_id: TransferId,
        /// Neden.
        reason: FileFailure,
    },
    /// Eş gelen aktarımı iptal etti (`FILE_ABORT`, herhangi bir neden); durum atıldı.
    Aborted {
        /// Gönderen eş.
        peer: PeerRef,
        /// Gönderenin `transfer_id`'si.
        transfer_id: TransferId,
        /// Eşin bildirdiği neden.
        reason: FileAbortReason,
    },
    /// Giden aktarımın son kaydı (`FILE_END`) Transport'a kabul ettirildi — ara olay. Teslim DEĞİL
    /// (S1 K8): aktarım eşin `FILE_RECEIPT`'ini bekler.
    Sent {
        /// Alıcı eş.
        peer: PeerRef,
        /// [`FileTransfer::send_file`]'ın döndürdüğü kimlik.
        transfer_id: TransferId,
    },
    /// Eş giden dosyayı bütün olarak aldı ve doğruladı: `FILE_RECEIPT(ok)` — teslim edildi.
    Delivered {
        /// Alıcı eş.
        peer: PeerRef,
        /// [`FileTransfer::send_file`]'ın döndürdüğü kimlik.
        transfer_id: TransferId,
    },
    /// Giden aktarım teslim edilmeden bitti; kalan kayıtlar sunulmaz.
    SendFailed {
        /// Alıcı eş.
        peer: PeerRef,
        /// [`FileTransfer::send_file`]'ın döndürdüğü kimlik.
        transfer_id: TransferId,
        /// Neden.
        reason: SendFailure,
    },
}

/// Bir olayın tuttuğu yuva (modül belgesi, "Sınırlar ve olay kuyruğu").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventSlot {
    /// Gelen aktarımın son olayı.
    Incoming,
    /// Giden aktarımın son olayı.
    Outgoing,
    /// Ara olay (`Sent`): aktarım tabloda kalır, yuvayı o tutar.
    Progress,
}

impl FileEvent {
    /// Olayın tuttuğu yuva.
    const fn slot(&self) -> EventSlot {
        match self {
            Self::Received { .. } | Self::Failed { .. } | Self::Aborted { .. } => {
                EventSlot::Incoming
            }
            Self::Delivered { .. } | Self::SendFailed { .. } => EventSlot::Outgoing,
            Self::Sent { .. } => EventSlot::Progress,
        }
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
            Self::Delivered { peer, transfer_id } => f
                .debug_struct("Delivered")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .finish(),
            Self::SendFailed {
                peer,
                transfer_id,
                reason,
            } => f
                .debug_struct("SendFailed")
                .field("peer", peer)
                .field("transfer_id", transfer_id)
                .field("reason", reason)
                .finish(),
        }
    }
}

/// [`FileTransfer::send_file`] ve [`FileTransfer::cancel_send`] hatası. Her hata yolunda durum
/// DEĞİŞMEZ: `transfer_id` ayrılmaz, kuyruğa bir şey girmez. Motor hiçbirini yeniden denemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSendError {
    /// Eşin hazır oturumu yok ([`Messenger::session_of`] → `None`); dosya kuyruğa ALINMAZ.
    NoSession,
    /// [`MAX_OUTGOING_TRANSFERS`] giden yuva dolu (kuyruktaki ve alındı bekleyen aktarımlar +
    /// alınmamış son gönderen olayları) — geri basınç.
    QueueFull,
    /// Eşin `transfer_id` alanı tükendi (depo bunu kalıcı bildirir); sarma YOK.
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
    /// Kalıcı depo ([`AppStore`]) `transfer_id` ayıramadı ya da sözleşmesine aykırı bir değer
    /// yineledi (`Corrupt`): `FILE_BEGIN` gitmedi, aktarım kuyruğa girmedi. Depo tükenmesi
    /// [`FileSendError::TransferIdExhausted`] olarak döner, burada değil.
    Store(AppStoreError),
    /// İzlenen giden aktarım değil (hiç yoktu ya da bitti).
    UnknownTransfer,
    /// Aktarımın son kaydı (`FILE_END`) kabul edildi: iptal edilecek bir şey kalmadı, eş aktarımı
    /// kendi verisiyle bitirir. Alındı yine beklenir ([`FileEvent::Delivered`] /
    /// [`FileEvent::SendFailed`]).
    AlreadySent,
}

impl fmt::Display for FileSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSession => f.write_str("peer has no ready session"),
            Self::QueueFull => write!(
                f,
                "all {MAX_OUTGOING_TRANSFERS} outgoing transfer slots are in use"
            ),
            Self::TransferIdExhausted => {
                f.write_str("transfer_id space of this peer is exhausted (no wrap)")
            }
            Self::TooLarge { file_size } => write!(
                f,
                "file of {file_size} bytes exceeds MAX_REASSEMBLY_BYTES {MAX_REASSEMBLY_BYTES}"
            ),
            Self::NoChunkRoom => {
                f.write_str("MAX_FILE_CHUNK_BYTES is 0: the transport unit cannot carry file data")
            }
            Self::Encode(e) => write!(f, "FILE_BEGIN does not fit one AMP record: {e}"),
            Self::Store(e) => write!(f, "app store failed, nothing was queued: {e}"),
            Self::UnknownTransfer => f.write_str("no such outgoing transfer"),
            Self::AlreadySent => {
                f.write_str("the last record of this transfer was already accepted")
            }
        }
    }
}

impl std::error::Error for FileSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(e) => Some(e),
            Self::Store(e) => Some(e),
            Self::NoSession
            | Self::QueueFull
            | Self::TransferIdExhausted
            | Self::TooLarge { .. }
            | Self::NoChunkRoom
            | Self::UnknownTransfer
            | Self::AlreadySent => None,
        }
    }
}

/// Gözlem sayaçları — salt-okunur; protokol davranışını değiştirmez.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FileTransferStats {
    /// Transport'un kabul ettiği giden `FILE_*` kayıtları (gönderen kuyruğu; alındılar hariç). Her
    /// kayıt EN ÇOK bir kez kabul ettirilir.
    pub records_accepted: u64,
    /// Transport'un geçici reddi (geri basınç, eski tutamaç) — gönderen kaydı ya da alındı: tele
    /// çıkmadı, kuyrukta bekler.
    pub offers_refused: u64,
    /// Bilinmeyen aktarıma ait `FILE_CHUNK` / `FILE_END` (atıldı).
    pub unknown_transfer_records: u64,
    /// Geometriye ya da `FILE_BEGIN`'e uymayan kayıtlar (atıldı, S8).
    pub inconsistent_records: u64,
    /// Tekrar `FILE_BEGIN` / `FILE_CHUNK` / `FILE_END` (etkisiz).
    pub duplicate_records: u64,
    /// `rejected` ile reddedilen gelen aktarımlar: S7 boyutu ya da yuva yok.
    pub transfers_refused: u64,
    /// `FILE_ABORT`'u önce gelmiş (erken iptal kaydındaki) aktarımın `rejected` ile reddedilen
    /// `FILE_BEGIN`'i.
    pub begins_after_abort: u64,
    /// [`FILE_IDLE_TTL`] ile biten gelen aktarımlar (olay üretir).
    pub transfers_timed_out: u64,
    /// Transport'un kabul ettiği `FILE_RECEIPT`'ler (alıcı).
    pub receipts_sent: u64,
    /// Gönderilemeden düşen `FILE_RECEIPT`'ler: kalıcı Transport reddi, [`FILE_RECEIPT_TTL`] aşımı ya
    /// da alındı kuyruğu taşması.
    pub receipts_unsent: u64,
    /// İzlenen giden aktarıma uygulanamayan gelen `FILE_RECEIPT`'ler (bilinmeyen kimlik, başka eş,
    /// `FILE_END`'den önce `ok`, `FILE_BEGIN`'i sunulmamış aktarım; yok sayıldı).
    pub receipts_ignored: u64,
    /// [`FILE_RECEIPT_TTL`] ile biten giden aktarımlar (olay üretir).
    pub receipt_timeouts: u64,
    /// Oturum değişiminde `FILE_ABORT(superseded)` ile kapatılmaya başlanan yarım giden aktarımlar.
    pub transfers_superseded: u64,
    /// İzlenen gelen aktarıma denk gelmeyen `FILE_ABORT`'lar: erken iptal kaydına yazıldı.
    pub aborts_unmatched: u64,
}

/// B2.3 / K4G-09 dosya aktarım motoru. Ayrıntı: modül belgesi.
pub struct FileTransfer {
    /// [`FILE_IDLE_TTL`], [`FILE_RECEIPT_TTL`] ve erken iptal kaydı ömrünün zaman kaynağı.
    clock: Arc<dyn MonotonicClock>,
    /// Giden aktarımlar (alındı bekleyenler dahil), `transfer_id` sırasıyla — eş içinde oluşturma
    /// sırası (≤ [`MAX_OUTGOING_TRANSFERS`] − alınmamış son gönderen olayları; `transfer_id` açık
    /// aktarımlar arasında benzersiz).
    outgoing: BTreeMap<TransferId, Outgoing>,
    /// Gelen aktarımlar (+ alınmamış alıcı olayları ≤ [`MAX_INCOMING_TRANSFERS`]; eş başına
    /// ≤ [`MAX_INCOMING_TRANSFERS_PER_PEER`]).
    incoming: BTreeMap<IncomingKey, Incoming>,
    /// Erken iptal kaydı: `FILE_BEGIN`'i görülmeden `FILE_ABORT`'u gelen (eş, `transfer_id`) → kayıt
    /// anı (eş başına ≤ [`MAX_INCOMING_TRANSFERS_PER_PEER`], toplam ≤ [`MAX_EARLY_ABORTS`]; ömür
    /// [`FILE_IDLE_TTL`]).
    aborted_early: BTreeMap<IncomingKey, Duration>,
    /// Transport'a henüz kabul ettirilememiş alındılar, doğuş sırasıyla (≤ [`MAX_PENDING_RECEIPTS`]).
    receipts: VecDeque<PendingReceipt>,
    /// Son çağrıdan beri kuyruğa yeni alındı girdi: [`FileTransfer::handle`] onları hemen sunar.
    fresh_receipts: bool,
    /// Alınmayı bekleyen olaylar, doğuş sırasıyla (≤ [`MAX_PENDING_FILE_EVENTS`]).
    events: VecDeque<FileEvent>,
    /// [`FileTransfer::pump`] turunda kaydı sunulmuş eşler — her turda yeniden kullanılan tampon
    /// (≤ [`MAX_OUTGOING_TRANSFERS`]).
    served: Vec<PeerRef>,
    stats: FileTransferStats,
}

impl FileTransfer {
    /// Boş motor: kuyruklar, birleştirme tablosu ve kayıtlar boş. `clock` zamanlayıcıların kaynağıdır
    /// — ürün yolu [`SystemClock`](crate::clock::SystemClock)'u verir (bileşim kökü), testler elle
    /// ilerletilen bir saat. `transfer_id`'ler [`Messenger`]'ın deposundan ayrılır; motor sayaç
    /// tutmaz.
    #[must_use]
    pub fn new(clock: Arc<dyn MonotonicClock>) -> Self {
        Self {
            clock,
            outgoing: BTreeMap::new(),
            incoming: BTreeMap::new(),
            aborted_early: BTreeMap::new(),
            receipts: VecDeque::new(),
            fresh_receipts: false,
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

    /// İzlenen giden aktarım sayısı: kayıtları sunulmakta olanlar ve alındı bekleyenler.
    #[must_use]
    pub fn outgoing_count(&self) -> usize {
        self.outgoing.len()
    }

    /// Birleştirilmekte olan gelen aktarım sayısı.
    #[must_use]
    pub fn incoming_count(&self) -> usize {
        self.incoming.len()
    }

    /// Sıradaki olay, doğuş sırasıyla; yoksa `None`. Alınan son olay aktarımının yuvasını boşaltır.
    #[must_use]
    pub fn poll_event(&mut self) -> Option<FileEvent> {
        self.events.pop_front()
    }

    /// Dosyayı `peer`'e gönderilmek üzere kuyruğa alır ve eşin kalıcı deposundan ayrılan
    /// `transfer_id`'yi döndürür. Kayıtlar [`FileTransfer::pump`] ile sunulur: `Ok` hiçbir kaydın
    /// kabul edildiği anlamına gelmez. `messenger` sonraki `pump` çağrılarındakiyle aynı motor
    /// olmalıdır (parça uzunluğu onun limitlerinden, `transfer_id` onun deposundan).
    ///
    /// # Errors
    ///
    /// Denetim sırasıyla: [`FileSendError::NoSession`] · [`FileSendError::QueueFull`] ·
    /// [`FileSendError::TooLarge`] (özetten ÖNCE) · [`FileSendError::NoChunkRoom`] ·
    /// [`FileSendError::Encode`] — bunlarda depoya dokunulmaz · [`FileSendError::TransferIdExhausted`]
    /// ya da [`FileSendError::Store`] (`transfer_id` ayrılamadı). Hiçbirinde `FILE_BEGIN` sunulmaz ve
    /// aktarım kuyruğa girmez; hiçbiri değer ayırmaz (modül belgesi, "`transfer_id`").
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
        if self.slots_used(EventSlot::Outgoing) >= MAX_OUTGOING_TRANSFERS {
            return Err(FileSendError::QueueFull);
        }
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
        // `FILE_BEGIN` bir kayda sığmalı (ad ≤ 255 B, gövde ≤ `MAX_BODY_BYTES`). Boyu kimlikten ve
        // özetten bağımsızdır (sabit genişlik): yer tutucularla, kimlik AYRILMADAN önce denetlenir.
        // Parçalar `chunk_len` gereği, `FILE_END` / `FILE_ABORT` her geçerli `AmpLimits`'te sığar.
        let probe = AppMessage::FileBegin {
            transfer_id: FIRST_TRANSFER_ID,
            file_size,
            whole_file_hash: [0; WHOLE_FILE_HASH_BYTES],
            chunk_count,
            name: name.to_owned(),
        };
        codec::encode(&probe, messenger.limits()).map_err(FileSendError::Encode)?;
        // Buradan sonra başarısız olabilecek tek adım depodur; ayrılan değer kullanılır.
        let transfer_id = self.reserve_transfer_id(messenger.store(), peer)?;
        let transfer = Outgoing {
            peer,
            name: name.to_owned(),
            file_size,
            whole_file_hash: whole_file_hash(&bytes),
            chunk_len,
            chunk_count,
            phase: Phase::Offering(Step::Begin),
            began_on: None,
            bytes,
        };
        self.outgoing.insert(transfer_id, transfer);
        Ok(transfer_id)
    }

    /// Depodan eşin sıradaki `transfer_id`'si; açık bir giden aktarımın değeriyse atlanır ve yenisi
    /// ayrılır (modül belgesi, "`transfer_id`").
    ///
    /// # Errors
    ///
    /// [`FileSendError::TransferIdExhausted`] (depo `Exhausted`) · [`FileSendError::Store`] — değer
    /// ayrılmadı ya da (aynı değeri yineleyen depo) atlama sınırı aşıldı (`Corrupt`).
    fn reserve_transfer_id(
        &self,
        store: &dyn AppStore,
        peer: PeerRef,
    ) -> Result<TransferId, FileSendError> {
        // Çakışabilecek değerler öbür eşlerin en çok `outgoing.len()` açık aktarımınındır (eşin
        // kendi açık aktarımları sayacın gerisindedir). Depo değerleri eş başına kesin artan olduğundan
        // her biri en çok bir kez çıkar; sınır, sözleşmeye aykırı bir depoya karşı döngüyü keser.
        for _ in 0..=self.outgoing.len() {
            let transfer_id = store
                .reserve_next(peer, CounterKind::TransferId)
                .map_err(store_error)?;
            if !self.outgoing.contains_key(&transfer_id) {
                return Ok(transfer_id);
            }
        }
        Err(FileSendError::Store(AppStoreError::Corrupt))
    }

    /// Giden aktarımı iptal eder. `FILE_BEGIN` henüz kabul edilmediyse aktarım sessizce düşer (eş onu
    /// hiç görmedi; tele bir şey çıkmaz); edildiyse kalan kayıtların yerine `FILE_ABORT(user_cancel)`
    /// geçer ve sıradaki [`FileTransfer::pump`]'ta sunulur (kabul edilene kadar bekler — kabul
    /// edilmemiş kayıt tele çıkmamıştır, yeniden iletim DEĞİL). Dosya baytları hemen bırakılır. Olay
    /// yok (çağıran biliyor); aktarımın `FILE_ABORT`'u zaten sunulmaktaysa (ör. oturum değişimi)
    /// etkisizdir.
    ///
    /// # Errors
    ///
    /// [`FileSendError::UnknownTransfer`] — izlenen giden aktarım değil (hiç yoktu ya da bitti) ·
    /// [`FileSendError::AlreadySent`] — son kaydı kabul edildi; alındı yine beklenir.
    pub fn cancel_send(&mut self, transfer_id: TransferId) -> Result<(), FileSendError> {
        let transfer = self
            .outgoing
            .get_mut(&transfer_id)
            .ok_or(FileSendError::UnknownTransfer)?;
        match transfer.phase {
            Phase::Offering(Step::Begin) => {
                self.outgoing.remove(&transfer_id);
            }
            Phase::Offering(Step::Chunk(_) | Step::End) => {
                transfer.phase = Phase::Offering(Step::Abort(FileAbortReason::UserCancel));
                transfer.release_bytes();
            }
            Phase::Offering(Step::Abort(_)) => {}
            Phase::AwaitingReceipt { .. } => return Err(FileSendError::AlreadySent),
        }
        Ok(())
    }

    /// Kabul edilmiş gelen aktarımı yerel olarak bırakır (kullanıcı iptali) ve eşe
    /// `FILE_RECEIPT(aborted_by_receiver)` verir (alındı kuyruğundan, hemen sunulur; modül belgesi).
    /// Eşin sonraki kayıtları bilinmeyen aktarım olarak atılır. Olay yok. `true` = aktarım vardı ve
    /// bırakıldı; `false` = izlenen gelen aktarım değil (eşe bir şey gitmez).
    pub fn cancel_incoming<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        peer: PeerRef,
        transfer_id: TransferId,
    ) -> bool {
        if self.incoming.remove(&(peer, transfer_id)).is_none() {
            return false;
        }
        let now = self.clock.now();
        self.queue_receipt(peer, transfer_id, FileReceiptStatus::AbortedByReceiver, now);
        self.flush_receipts(messenger, now);
        true
    }

    /// Turun sonu: süresi dolan aktarımları bitirir (alıcıda [`FILE_IDLE_TTL`], gönderende
    /// [`FILE_RECEIPT_TTL`]), bekleyen alındıları sunar, sonra kuyruktaki kayıtları sunar; ilerleme
    /// kalmayınca döner. Doğan olaylar [`FileTransfer::poll_event`] kuyruğuna girer.
    ///
    /// Messenger kuyruğu boşaltıldıktan SONRA çağrılmalıdır (modül belgesi, "Zamanlayıcılar"):
    /// zamanlayıcılar YALNIZ burada denetlenir; trafik olmasa da periyodik çağrı gerekir.
    ///
    /// Her turda eş başına yalnız EN ESKİ sunulmakta olan aktarımın sıradaki kaydı BİR KEZ sunulur
    /// (eşler arası adil, eş içi sıralı; alındı bekleyen aktarım eşin sıradakini bekletmez). Hiçbir
    /// kaydın kabul edilmediği turda döner: reddedilen kayıt kuyrukta bekler ve sonraki çağrıda
    /// yeniden sunulur.
    pub fn pump<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>) {
        let now = self.clock.now();
        self.expire_incoming(now);
        self.expire_receipt_waits(now);
        self.flush_receipts(messenger, now);
        loop {
            self.served.clear();
            let mut accepted = false;
            let mut finished = None;
            for (&transfer_id, transfer) in &mut self.outgoing {
                if transfer.is_awaiting_receipt() || self.served.contains(&transfer.peer) {
                    // Alındı bekliyor ya da eşin daha eski bir aktarımı sürüyor.
                    continue;
                }
                self.served.push(transfer.peer);
                match offer(messenger, transfer_id, transfer, &mut self.stats, now) {
                    Offer::Waiting => {}
                    Offer::Accepted => accepted = true,
                    Offer::Sent => {
                        accepted = true;
                        // Ara olay: aktarım tabloda kalır ve yuvasını o tutar.
                        push_event(
                            &mut self.events,
                            FileEvent::Sent {
                                peer: transfer.peer,
                                transfer_id,
                            },
                        );
                    }
                    Offer::Finished(event) => {
                        finished = Some((transfer_id, event));
                        break;
                    }
                }
            }
            match finished {
                Some((transfer_id, event)) => {
                    // Son olay, bıraktığı aktarımın yuvasını alır.
                    self.outgoing.remove(&transfer_id);
                    if let Some(event) = event {
                        push_event(&mut self.events, event);
                    }
                }
                None if !accepted => return,
                None => {}
            }
        }
    }

    /// Zincirden gelen bir [`MessengerEvent`]'i işler: `FILE_*` kaydı taşıyan `Other`'ı TÜKETİR
    /// (`None`; doğan olay [`FileTransfer::poll_event`] kuyruğuna girer), diğer her olayı AYNEN
    /// geri verir. Kaydın doğurduğu alındı (`ok`, `hash_mismatch`, `rejected`) eşin güncel oturumuna
    /// hemen sunulur; Transport kabul etmezse alındı kuyruğunda bekler (modül belgesi).
    #[must_use]
    pub fn handle<T: KamaTransport>(
        &mut self,
        messenger: &mut Messenger<T>,
        event: MessengerEvent,
    ) -> Option<MessengerEvent> {
        match event {
            MessengerEvent::Other { peer, message } if is_file_record(&message) => {
                let now = self.clock.now();
                if let Some(event) = self.on_record(peer, message, now) {
                    push_event(&mut self.events, event);
                }
                if self.fresh_receipts {
                    self.flush_receipts(messenger, now);
                }
                None
            }
            event => Some(event),
        }
    }

    /// Bir yöne ait yuvaları kullananların sayısı: tablodaki aktarımlar + kuyruktaki son olayları.
    fn slots_used(&self, slot: EventSlot) -> usize {
        let table = match slot {
            EventSlot::Incoming => self.incoming.len(),
            EventSlot::Outgoing => self.outgoing.len(),
            EventSlot::Progress => 0,
        };
        table
            + self
                .events
                .iter()
                .filter(|event| event.slot() == slot)
                .count()
    }

    /// `FILE_*` kaydı (yalnız [`FileTransfer::handle`] çağırır).
    fn on_record(
        &mut self,
        peer: PeerRef,
        message: AppMessage,
        now: Duration,
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
                self.on_begin((peer, transfer_id), declared, name, now);
                None
            }
            AppMessage::FileChunk {
                transfer_id,
                offset,
                payload,
            } => self.on_chunk((peer, transfer_id), offset, &payload, now),
            AppMessage::FileEnd { transfer_id } => self.on_end((peer, transfer_id), now),
            AppMessage::FileAbort {
                transfer_id,
                reason,
            } => self.on_abort((peer, transfer_id), reason, now),
            AppMessage::FileReceipt {
                transfer_id,
                status,
            } => self.on_receipt(peer, transfer_id, status),
            // `handle` bunları geri verir; buraya gelmezler.
            AppMessage::Text { .. }
            | AppMessage::PttOpen { .. }
            | AppMessage::PttClose { .. }
            | AppMessage::Receipt { .. }
            | AppMessage::SignalingBlob { .. }
            | AppMessage::MediaBundle { .. } => None,
        }
    }

    /// `FILE_BEGIN`: kabul ya da ret (`rejected`). Olay üretmez.
    fn on_begin(&mut self, key: IncomingKey, declared: Declared, name: String, now: Duration) {
        if let Some(existing) = self.incoming.get(&key) {
            // Aynı alanlar → tekrar. Farklı alanlar (ör. yeniden başlamış gönderenin kimliği) →
            // DROP; süren aktarım korunur (ret alındısı onu da bitirirdi).
            if existing.declared == declared && existing.name == name {
                self.stats.duplicate_records += 1;
            } else {
                self.stats.inconsistent_records += 1;
            }
            return;
        }
        if !declared.is_consistent() {
            // S8: sessiz DROP, alındı yok.
            self.stats.inconsistent_records += 1;
            return;
        }
        let (peer, transfer_id) = key;
        if self.take_early_abort(key, now) {
            // Aktarımın `FILE_ABORT`'u `FILE_BEGIN`'inden önce geldi (oturumlar arası sırasızlık;
            // modül belgesi, "Oturum değişimi"): hayalet aktarım başlatılmaz.
            self.stats.begins_after_abort += 1;
            self.queue_receipt(peer, transfer_id, FileReceiptStatus::Rejected, now);
            return;
        }
        let from_peer = self.incoming_of(peer).count();
        if declared.file_size > MAX_REASSEMBLY_BYTES
            || from_peer >= MAX_INCOMING_TRANSFERS_PER_PEER
            || self.slots_used(EventSlot::Incoming) >= MAX_INCOMING_TRANSFERS
        {
            self.stats.transfers_refused += 1;
            self.queue_receipt(peer, transfer_id, FileReceiptStatus::Rejected, now);
            return;
        }
        self.incoming
            .insert(key, Incoming::new(declared, name, now));
    }

    /// `FILE_CHUNK`: parça yerleşir ve [`FILE_IDLE_TTL`] süresini sıfırlar; aktarım tamamlandıysa
    /// doğrulanır.
    fn on_chunk(
        &mut self,
        key: IncomingKey,
        offset: u64,
        payload: &[u8],
        now: Duration,
    ) -> Option<FileEvent> {
        let Some(transfer) = self.incoming.get_mut(&key) else {
            self.stats.unknown_transfer_records += 1;
            return None;
        };
        match transfer.accept_chunk(offset, payload) {
            ChunkVerdict::Stored => transfer.last_chunk_at = now,
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
            self.finish(key, now)
        } else {
            None
        }
    }

    /// `FILE_END`: eksik parça yoksa aktarım doğrulanır; varsa beklenir (süre sıfırlanmaz).
    fn on_end(&mut self, key: IncomingKey, now: Duration) -> Option<FileEvent> {
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
            self.finish(key, now)
        } else {
            None
        }
    }

    /// Bütün parçalar + `FILE_END`: özet doğrulanır; durum her iki sonuçta da atılır, eşe sonucun
    /// alındısı gider.
    fn finish(&mut self, key: IncomingKey, now: Duration) -> Option<FileEvent> {
        let transfer = self.incoming.remove(&key)?;
        let (peer, transfer_id) = key;
        if whole_file_hash(&transfer.data) == transfer.declared.whole_file_hash {
            self.queue_receipt(peer, transfer_id, FileReceiptStatus::Ok, now);
            Some(FileEvent::Received {
                peer,
                transfer_id,
                name: transfer.name,
                bytes: transfer.data,
            })
        } else {
            self.queue_receipt(peer, transfer_id, FileReceiptStatus::HashMismatch, now);
            Some(FileEvent::Failed {
                peer,
                transfer_id,
                reason: FileFailure::HashMismatch,
            })
        }
    }

    /// `FILE_ABORT` (yalnız gönderenindir): nedeni ne olursa olsun eşin GÖNDERDİĞİ aktarıma uygulanır.
    /// İzlenmeyen aktarım → erken iptal kaydı (modül belgesi, "Oturum değişimi"); olay yok.
    fn on_abort(
        &mut self,
        key: IncomingKey,
        reason: FileAbortReason,
        now: Duration,
    ) -> Option<FileEvent> {
        if self.incoming.remove(&key).is_some() {
            let (peer, transfer_id) = key;
            return Some(FileEvent::Aborted {
                peer,
                transfer_id,
                reason,
            });
        }
        self.stats.aborts_unmatched += 1;
        self.remember_early_abort(key, now);
        None
    }

    /// `FILE_RECEIPT` (yalnız alıcınındır): eşe GİDEN aktarıma uygulanır. Kimlik başka bir eşe
    /// gidiyorsa ya da alındı aktarımın durumuyla bağdaşmıyorsa yok sayılır.
    fn on_receipt(
        &mut self,
        peer: PeerRef,
        transfer_id: TransferId,
        status: FileReceiptStatus,
    ) -> Option<FileEvent> {
        let outcome = match self.outgoing.get(&transfer_id) {
            Some(transfer) if transfer.peer == peer => receipt_outcome(
                transfer.phase,
                peer,
                transfer_id,
                SendFailure::from_receipt(status),
            ),
            _ => ReceiptOutcome::Ignore,
        };
        match outcome {
            ReceiptOutcome::Ignore => {
                self.stats.receipts_ignored += 1;
                None
            }
            ReceiptOutcome::Finish(event) => {
                // Son olay (varsa), bıraktığı aktarımın yuvasını alır.
                self.outgoing.remove(&transfer_id);
                event
            }
        }
    }

    /// [`FILE_IDLE_TTL`] boyunca yeni parça almamış gelen aktarımları bitirir: yuva boşalır, eşe
    /// `FILE_RECEIPT(timeout)` kuyruğa girer, `Failed(IdleTimeout)` doğar.
    fn expire_incoming(&mut self, now: Duration) {
        while let Some(key) = self
            .incoming
            .iter()
            .find(|(_, transfer)| now.saturating_sub(transfer.last_chunk_at) >= FILE_IDLE_TTL)
            .map(|(&key, _)| key)
        {
            self.incoming.remove(&key);
            self.stats.transfers_timed_out += 1;
            let (peer, transfer_id) = key;
            self.queue_receipt(peer, transfer_id, FileReceiptStatus::Timeout, now);
            push_event(
                &mut self.events,
                FileEvent::Failed {
                    peer,
                    transfer_id,
                    reason: FileFailure::IdleTimeout,
                },
            );
        }
    }

    /// `FILE_END`'i kabul edileli [`FILE_RECEIPT_TTL`] olmuş, alındısı gelmemiş giden aktarımları
    /// bitirir (`SendFailed(ReceiptTimeout)`); eşe bir şey gitmez.
    fn expire_receipt_waits(&mut self, now: Duration) {
        while let Some(transfer_id) = self
            .outgoing
            .iter()
            .find(|(_, transfer)| transfer.receipt_overdue(now))
            .map(|(&transfer_id, _)| transfer_id)
        {
            let Some(transfer) = self.outgoing.remove(&transfer_id) else {
                return;
            };
            self.stats.receipt_timeouts += 1;
            push_event(
                &mut self.events,
                FileEvent::SendFailed {
                    peer: transfer.peer,
                    transfer_id,
                    reason: SendFailure::ReceiptTimeout,
                },
            );
        }
    }

    /// Alındıyı kuyruğa koyar (sunulmaz; [`FileTransfer::flush_receipts`] sunar). Kuyruk doluysa en
    /// eski alındı düşer — sınır eklemeden ÖNCE.
    fn queue_receipt(
        &mut self,
        peer: PeerRef,
        transfer_id: TransferId,
        status: FileReceiptStatus,
        now: Duration,
    ) {
        if self.receipts.len() >= MAX_PENDING_RECEIPTS {
            // En eski düşer: gönderen o aktarımı kendi `FILE_RECEIPT_TTL`'iyle başarısız sayar.
            self.receipts.pop_front();
            self.stats.receipts_unsent += 1;
        }
        self.receipts.push_back(PendingReceipt {
            peer,
            transfer_id,
            status,
            queued_at: now,
        });
        self.fresh_receipts = true;
    }

    /// Bekleyen alındıları doğuş sırasıyla, her birini eşin O ANKİ güncel oturumuna BİR KEZ sunar.
    /// Kabul edilen ya da kalıcı reddedilen çıkar; oturumu olmayan ya da geçici reddedilen bekler;
    /// [`FILE_RECEIPT_TTL`]'den eskisi düşer.
    fn flush_receipts<T: KamaTransport>(&mut self, messenger: &mut Messenger<T>, now: Duration) {
        self.fresh_receipts = false;
        let Self {
            receipts, stats, ..
        } = self;
        receipts.retain(|receipt| {
            if now.saturating_sub(receipt.queued_at) >= FILE_RECEIPT_TTL {
                stats.receipts_unsent += 1;
                return false;
            }
            let Some(session) = messenger.session_of(receipt.peer) else {
                // Oturum yok: alındı bekler; eşin sıradaki oturumunda sunulur.
                return true;
            };
            let message = AppMessage::FileReceipt {
                transfer_id: receipt.transfer_id,
                status: receipt.status,
            };
            // `FILE_RECEIPT` gövdesi 9 B'tır ve her geçerli `AmpLimits`'e sığar (AMP §4 A2); kodlama
            // hatası yine de kalıcı ret sayılır, panik yok.
            let sent = match codec::encode(&message, messenger.limits()) {
                Ok(unit) => messenger
                    .transport_mut()
                    .send(session, unit, DeliveryPolicy::Reliable),
                Err(_) => Err(TransportError::UnitTooLarge),
            };
            match sent {
                Ok(()) => {
                    stats.receipts_sent += 1;
                    false
                }
                Err(error) if is_transient(error) => {
                    stats.offers_refused += 1;
                    true
                }
                Err(_) => {
                    stats.receipts_unsent += 1;
                    false
                }
            }
        });
    }

    /// Eşin gelen aktarımları (tablonun eşe düşen dilimi).
    fn incoming_of(&self, peer: PeerRef) -> Range<'_, IncomingKey, Incoming> {
        self.incoming
            .range((peer, TransferId::MIN)..=(peer, TransferId::MAX))
    }

    /// Eşin erken iptal kayıtları (kaydın eşe düşen dilimi).
    fn early_aborts_of(&self, peer: PeerRef) -> Range<'_, IncomingKey, Duration> {
        self.aborted_early
            .range((peer, TransferId::MIN)..=(peer, TransferId::MAX))
    }

    /// Eşleşmeyen `FILE_ABORT`'un (eş, `transfer_id`)'sini erken iptal kaydına yazar. Süresi dolan
    /// kayıtlar düşer; kimlik zaten kayıtlıysa kayıt değişmez; eşin kaydı ya da kayıt doluysa en eski
    /// iptal yer açar — sınır her eklemeden ÖNCE.
    fn remember_early_abort(&mut self, key: IncomingKey, now: Duration) {
        self.aborted_early
            .retain(|_, &mut at| now.saturating_sub(at) < FILE_IDLE_TTL);
        if self.aborted_early.contains_key(&key) {
            return;
        }
        let (peer, _) = key;
        let evicted = if self.early_aborts_of(peer).count() >= MAX_INCOMING_TRANSFERS_PER_PEER {
            earliest(self.early_aborts_of(peer))
        } else if self.aborted_early.len() >= MAX_EARLY_ABORTS {
            earliest(self.aborted_early.iter())
        } else {
            None
        };
        if let Some(evicted) = evicted {
            self.aborted_early.remove(&evicted);
        }
        self.aborted_early.insert(key, now);
    }

    /// (Eş, `transfer_id`) son [`FILE_IDLE_TTL`] içinde erken iptal kaydına girdiyse `true`; kayıt her
    /// durumda silinir (`FILE_BEGIN` en çok bir kez gelir).
    fn take_early_abort(&mut self, key: IncomingKey, now: Duration) -> bool {
        self.aborted_early
            .remove(&key)
            .is_some_and(|at| now.saturating_sub(at) < FILE_IDLE_TTL)
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
            | AppMessage::FileReceipt { .. }
    )
}

/// Olayı kuyruğa koyar. Yuva hesabı tavanı garanti eder: son olay, kuyruktan önce tablodan çıkarılan
/// aktarımın yerini alır; ara olay tabloda kalan aktarıma aittir.
fn push_event(events: &mut VecDeque<FileEvent>, event: FileEvent) {
    debug_assert!(
        events.len() < MAX_PENDING_FILE_EVENTS,
        "olay kuyruğu yuva hesabını aştı"
    );
    events.push_back(event);
}

/// Kayıtlardan en erken yazılmış olanın anahtarı.
fn earliest<'a>(
    records: impl Iterator<Item = (&'a IncomingKey, &'a Duration)>,
) -> Option<IncomingKey> {
    records.min_by_key(|&(_, &at)| at).map(|(&key, _)| key)
}

/// `transfer_id` ayırma hatası: depo tükenmesi kalıcıdır ve sarma yoktur.
fn store_error(error: AppStoreError) -> FileSendError {
    match error {
        AppStoreError::Exhausted => FileSendError::TransferIdExhausted,
        other => FileSendError::Store(other),
    }
}

/// Transport'a henüz kabul ettirilememiş alındı.
struct PendingReceipt {
    /// Alındının gideceği eş (aktarımın göndereni).
    peer: PeerRef,
    /// Gönderenin `transfer_id`'si.
    transfer_id: TransferId,
    status: FileReceiptStatus,
    /// Doğduğu an (saatin `now()`'ı); [`FILE_RECEIPT_TTL`]'den eskisi düşer.
    queued_at: Duration,
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
    /// `FILE_ABORT` (`FILE_BEGIN` kabul edildikten sonra): kullanıcı iptali ya da oturum değişimi.
    Abort(FileAbortReason),
}

/// Giden aktarımın evresi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Kayıtlar sunuluyor; Transport'a henüz kabul ettirilmemiş sıradaki kayıt.
    Offering(Step),
    /// `FILE_END` `since` anında kabul edildi; eşin `FILE_RECEIPT`'i bekleniyor.
    AwaitingReceipt {
        /// `FILE_END`'in kabul anı (saatin `now()`'ı).
        since: Duration,
    },
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
    phase: Phase,
    /// `FILE_BEGIN`'i KABUL eden tutamaç (aktarımın oturumu); `None` = henüz kabul edilmedi.
    began_on: Option<SessionHandle>,
}

impl Outgoing {
    /// `step` kaydının mesajı.
    fn message(&self, transfer_id: TransferId, step: Step) -> AppMessage {
        match step {
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

    /// Dosya baytları ve adı artık gerekmez (son kayıt kabul edildi ya da aktarım iptal ediliyor).
    fn release_bytes(&mut self) {
        self.bytes = Vec::new();
        self.name = String::new();
    }

    const fn is_awaiting_receipt(&self) -> bool {
        matches!(self.phase, Phase::AwaitingReceipt { .. })
    }

    /// Alındı [`FILE_RECEIPT_TTL`] içinde gelmedi.
    fn receipt_overdue(&self, now: Duration) -> bool {
        matches!(
            self.phase,
            Phase::AwaitingReceipt { since } if now.saturating_sub(since) >= FILE_RECEIPT_TTL
        )
    }
}

/// [`offer`] sonucu.
enum Offer {
    /// Kayıt kabul edilmedi (oturum yok ya da geçici ret): tele çıkmadı, kuyrukta bekler.
    Waiting,
    /// Kayıt kabul edildi; aktarım sürüyor.
    Accepted,
    /// `FILE_END` kabul edildi: aktarım alındı bekler (ara olay `Sent`).
    Sent,
    /// Aktarım bitti (`FILE_ABORT` kabul edildi ya da kalıcı hata); varsa son olay.
    Finished(Option<FileEvent>),
}

/// Aktarımın sıradaki kaydını eşin güncel oturumuna BİR KEZ sunar. Kabul edilen kaydın adımı
/// ilerler; böylece hiçbir kayıt ikinci kez kabul ettirilemez. Yarım aktarımın oturumu değiştiyse
/// sıradaki kaydın yerine `FILE_ABORT(superseded)` geçer (modül belgesi, "Oturum değişimi").
fn offer<T: KamaTransport>(
    messenger: &mut Messenger<T>,
    transfer_id: TransferId,
    transfer: &mut Outgoing,
    stats: &mut FileTransferStats,
    now: Duration,
) -> Offer {
    let Phase::Offering(mut step) = transfer.phase else {
        // Alındı bekleyen aktarımın sunacağı kayıt yok.
        return Offer::Waiting;
    };
    let peer = transfer.peer;
    let Some(session) = messenger.session_of(peer) else {
        // Oturum yok: kayıt bekler; eşin sonraki oturumunda yeniden sunulur.
        return Offer::Waiting;
    };
    if matches!(step, Step::Chunk(_) | Step::End) && transfer.began_on != Some(session) {
        // Yarım aktarımın oturumu değişti: kaldığı yerden devam yok (D6).
        stats.transfers_superseded += 1;
        step = Step::Abort(FileAbortReason::Superseded);
        transfer.phase = Phase::Offering(step);
        transfer.release_bytes();
    }
    let encoded = codec::encode(&transfer.message(transfer_id, step), messenger.limits());
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
            match step {
                Step::Begin => {
                    transfer.began_on = Some(session);
                    transfer.phase = Phase::Offering(transfer.chunk_step(0));
                    Offer::Accepted
                }
                Step::Chunk(index) => {
                    transfer.phase = Phase::Offering(transfer.chunk_step(index.saturating_add(1)));
                    Offer::Accepted
                }
                Step::End => {
                    transfer.release_bytes();
                    transfer.phase = Phase::AwaitingReceipt { since: now };
                    Offer::Sent
                }
                Step::Abort(reason) => Offer::Finished(abort_event(peer, transfer_id, reason)),
            }
        }
        Err(error) if is_transient(error) => {
            stats.offers_refused += 1;
            Offer::Waiting
        }
        Err(error) => Offer::Finished(match step {
            // İptal kaydı kalıcı reddedildi: aktarım yine de bitti.
            Step::Abort(reason) => abort_event(peer, transfer_id, reason),
            Step::Begin | Step::Chunk(_) | Step::End => Some(FileEvent::SendFailed {
                peer,
                transfer_id,
                reason: SendFailure::Transport(error),
            }),
        }),
    }
}

/// Gönderenin iptaliyle biten aktarımın son olayı: oturum değişimi → `SendFailed(Superseded)`;
/// kullanıcı iptali → olay yok (çağıran biliyor).
fn abort_event(
    peer: PeerRef,
    transfer_id: TransferId,
    reason: FileAbortReason,
) -> Option<FileEvent> {
    match reason {
        FileAbortReason::Superseded => Some(FileEvent::SendFailed {
            peer,
            transfer_id,
            reason: SendFailure::Superseded,
        }),
        FileAbortReason::UserCancel
        | FileAbortReason::HashMismatch
        | FileAbortReason::StorageFull
        | FileAbortReason::Timeout => None,
    }
}

/// [`receipt_outcome`] sonucu.
enum ReceiptOutcome {
    /// Alındı bu aktarıma uygulanamaz; yok sayılır.
    Ignore,
    /// Aktarım biter; varsa son olay.
    Finish(Option<FileEvent>),
}

/// Eşin alındısının giden aktarıma etkisi. `failure` = alındının gönderendeki sonucu (`None` = `ok`).
fn receipt_outcome(
    phase: Phase,
    peer: PeerRef,
    transfer_id: TransferId,
    failure: Option<SendFailure>,
) -> ReceiptOutcome {
    match (phase, failure) {
        // `FILE_BEGIN`'i henüz kabul edilmemiş aktarımı eş görmedi; `ok` ise `FILE_END`'den önce
        // anlamsızdır.
        (Phase::Offering(Step::Begin), _) | (Phase::Offering(_), None) => ReceiptOutcome::Ignore,
        (Phase::AwaitingReceipt { .. }, None) => {
            ReceiptOutcome::Finish(Some(FileEvent::Delivered { peer, transfer_id }))
        }
        // Gönderenin iptali sürerken: alıcı aktarımı zaten kapattı; iptalin kendi son olayı.
        (Phase::Offering(Step::Abort(reason)), Some(_)) => {
            ReceiptOutcome::Finish(abort_event(peer, transfer_id, reason))
        }
        // Alıcı aktarımı bitirdi (red, iptal, zaman aşımı): kalan kayıtlar sunulmaz.
        (
            Phase::Offering(Step::Chunk(_) | Step::End) | Phase::AwaitingReceipt { .. },
            Some(reason),
        ) => ReceiptOutcome::Finish(Some(FileEvent::SendFailed {
            peer,
            transfer_id,
            reason,
        })),
    }
}

/// Geçici ret: kayıt kuyruğa alınmadı ve sonraki sunumda yeniden sunulur (S1 §3; K8 "bekler").
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
    /// [`FILE_IDLE_TTL`] süresinin başı: `FILE_BEGIN`'in ya da son kabul edilen YENİ parçanın işlendiği
    /// an (saatin `now()`'ı).
    last_chunk_at: Duration,
}

impl Incoming {
    fn new(declared: Declared, name: String, now: Duration) -> Self {
        Self {
            name,
            declared,
            chunk_len: None,
            data: Vec::new(),
            seen: Vec::new(),
            received: 0,
            end_seen: false,
            last_chunk_at: now,
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
    //! Parça geometrisi, yuvalar ve olay kuyruğu, `FILE_ABORT` / `FILE_RECEIPT` yönü, `handle`
    //! geçirgenliği, `pump` sırası ve hata sınıfları, erken iptal kaydı ve alındı kuyruğu sınırları,
    //! depo sözleşmesi ihlalleri, BLAKE2s-256 bilinen yanıtı — uçtan uca akış (zamanlayıcılar,
    //! supersession, `transfer_id` kalıcılığı) `tests/file_transfer.rs`'te.
    //!
    //! Her testin belgesi, test mantığının `d3bc7407`'deki (K4G-09 öncesi main) sınıfını yazar
    //! (KIRMIZI (uyarlansa da) / UYARLANAMAZ / AYNI DAVRANIŞ; `tests/file_transfer.rs` başlığı).

    use std::sync::Mutex;

    use super::*;
    use crate::clock::SystemClock;
    use crate::codec::MsgType;
    use crate::messaging::MsgId;
    use crate::store::{DedupWindow, MemoryAppStore, OutboxUnit};
    use kama_contracts::s1::{
        ConnectResult, ConnectTicket, Delivered, PreparedUnit, SessionEvent, TransportCapabilities,
    };

    /// Test parametresi (normatif değil).
    const TEST_MAX_PLAINTEXT_UNIT_BYTES: u32 = 1024;
    /// Saatin en küçük adımı (ömür sınırı denetimi).
    const TICK: Duration = Duration::from_nanos(1);

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

    /// Elle ilerletilen saat (erken iptal kaydı ve alındı kuyruğu ömürleri); kopyalar aynı zamanı
    /// paylaşır.
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

    /// `transfer_id` sayacı testin koyduğu değerden süren depo (eşten bağımsız; tükenme kuralı ürün
    /// depolarıyla aynı: [`CounterKind::advance`]). `repeating` ise sözleşmeye AYKIRI olarak hep aynı
    /// değeri verir. Geri kalan her şey bellek içi depoya gider.
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
            if kind != CounterKind::TransferId {
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

    /// Zamana bakmayan testlerin saati; zaman davranışı [`TestClock`] ile sınanır.
    fn clock() -> Arc<dyn MonotonicClock> {
        Arc::new(SystemClock::new())
    }

    fn peer(n: u8) -> PeerRef {
        PeerRef::from_dest_hash([n; 32])
    }

    /// Verilen eşlerin her biriyle hazır oturumu olan, `store` deposuyla kurulmuş motor.
    fn messenger_on(store: Arc<dyn AppStore>, peers: &[PeerRef]) -> Messenger<CountingTransport> {
        let mut messenger =
            Messenger::with_store(CountingTransport::default(), store).expect("limitler geçerli");
        for &p in peers {
            assert!(matches!(messenger.connect(p), Ok(ConnectResult::Ready(_))));
        }
        messenger
    }

    /// Verilen eşlerin her biriyle hazır oturumu olan, taze bellek içi depolu motor.
    fn messenger_with(peers: &[PeerRef]) -> Messenger<CountingTransport> {
        messenger_on(Arc::new(MemoryAppStore::new()), peers)
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

    fn receipt(transfer_id: TransferId, status: FileReceiptStatus) -> AppMessage {
        AppMessage::FileReceipt {
            transfer_id,
            status,
        }
    }

    fn transfer_of(message: &AppMessage) -> TransferId {
        match message {
            AppMessage::FileBegin { transfer_id, .. }
            | AppMessage::FileChunk { transfer_id, .. }
            | AppMessage::FileEnd { transfer_id }
            | AppMessage::FileAbort { transfer_id, .. }
            | AppMessage::FileReceipt { transfer_id, .. } => *transfer_id,
            other => panic!("FILE_* bekleniyordu, gelen {other:?}"),
        }
    }

    /// `d3bc7407`'de AYNI DAVRANIŞ.
    #[test]
    fn whole_file_hash_is_unkeyed_blake2s_256() {
        assert_eq!(whole_file_hash(b"abc"), BLAKE2S_256_ABC);
    }

    /// `d3bc7407`'de AYNI DAVRANIŞ (yapıcıya saat verildi).
    #[test]
    fn handle_passes_every_other_event_through_untouched() {
        let bob = peer(1);
        let mut messenger = messenger_with(&[bob]);
        let mut files = FileTransfer::new(clock());
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

    /// `d3bc7407`'de AYNI DAVRANIŞ.
    #[test]
    fn chunk_len_is_learned_from_any_first_chunk() {
        // 10 B, L = 4: [0,4) [4,8) [8,10).
        let data: Vec<u8> = (0..10).collect();
        // Son parça önce: L = 8 / (3 − 1).
        let mut transfer = Incoming::new(declared(10, 3), String::new(), Duration::ZERO);
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
        let mut transfer = Incoming::new(declared(10, 3), String::new(), Duration::ZERO);
        assert_eq!(transfer.accept_chunk(4, &data[4..8]), ChunkVerdict::Stored);
        assert_eq!(transfer.chunk_len, Some(4));
        // Tek parçalı dosya.
        let mut transfer = Incoming::new(declared(10, 1), String::new(), Duration::ZERO);
        assert_eq!(transfer.accept_chunk(0, &data), ChunkVerdict::Stored);
        assert_eq!(transfer.chunk_len, Some(10));
    }

    /// `d3bc7407`'de AYNI DAVRANIŞ.
    #[test]
    fn chunks_outside_the_declared_layout_are_inconsistent() {
        let mut transfer = Incoming::new(declared(10, 3), String::new(), Duration::ZERO);
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

    /// Kabul edilmeyen `FILE_BEGIN` (eş başına tavan, toplam tavan, S7 boyutu) eşe tam bir
    /// `FILE_RECEIPT(rejected)` ile bildirilir; alıcı `FILE_ABORT` göndermez.
    ///
    /// `d3bc7407`'de KIRMIZI (uyarlansa da): üç ret de eşe `FILE_ABORT(storage_full)` olarak
    /// gidiyordu.
    #[test]
    fn begin_beyond_size_or_table_caps_is_refused_with_rejected() {
        let count = u8::try_from(MAX_INCOMING_TRANSFERS + 1).expect("küçük sayı");
        let peers: Vec<PeerRef> = (1..=count).map(peer).collect();
        let mut messenger = messenger_with(&peers);
        let per_peer = TransferId::try_from(MAX_INCOMING_TRANSFERS_PER_PEER).expect("küçük sayı");

        // Eş başına tavan.
        let mut files = FileTransfer::new(clock());
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
        let mut files = FileTransfer::new(clock());
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
        let mut files = FileTransfer::new(clock());
        let oversize = begin(0, MAX_REASSEMBLY_BYTES + 1, 1);
        assert_eq!(feed(&mut files, &mut messenger, peers[0], oversize), None);
        assert_eq!(files.incoming_count(), 0);
        assert_eq!(files.stats().transfers_refused, 1);
        assert_eq!(files.stats().receipts_sent, 1);

        // Üç ret de eşe `rejected` alındısıyla bildirildi, birer kez; `FILE_ABORT` yok.
        let refused = receipt(per_peer, FileReceiptStatus::Rejected);
        let refused_first = receipt(0, FileReceiptStatus::Rejected);
        assert_eq!(
            sent(&messenger),
            vec![refused, refused_first.clone(), refused_first]
        );
    }

    /// Yuva hesabı: alınmamış SON olaylar (`Received`, `Delivered`) ve alındı bekleyen giden
    /// aktarımlar yuva tutar; ara olay `Sent` tutmaz. Kuyruk tavanı gelen + 2 × giden tavanıdır ve
    /// hiçbir olay düşmez.
    ///
    /// `d3bc7407`'de UYARLANAMAZ: `FILE_RECEIPT` ve `Delivered` yoktu; `Sent` son olaydı ve yuvayı o
    /// tutuyordu (aktarım `FILE_END` kabulüyle tablodan çıkıyordu).
    #[test]
    fn undrained_events_hold_their_transfer_slots() {
        let count = u8::try_from(MAX_INCOMING_TRANSFERS + 1).expect("küçük sayı");
        let peers: Vec<PeerRef> = (1..=count).map(peer).collect();
        let mut messenger = messenger_with(&peers);
        let mut files = FileTransfer::new(clock());

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
        assert_eq!(files.stats().transfers_refused, 1, "yuva yok: rejected");

        // Giden: alındı bekleyen aktarımlar yuvalarını tutar.
        let mut ids = Vec::new();
        for _ in 0..MAX_OUTGOING_TRANSFERS {
            let id = files
                .send_file(&messenger, peers[0], "a", Vec::new())
                .expect("yuva var");
            ids.push(id);
            files.pump(&mut messenger);
        }
        assert_eq!(
            files.outgoing_count(),
            MAX_OUTGOING_TRANSFERS,
            "hepsi alındı bekliyor"
        );
        assert_eq!(
            files.send_file(&messenger, peers[0], "a", Vec::new()),
            Err(FileSendError::QueueFull)
        );
        // Alındılar gelir: aktarımlar tablodan çıkar, alınmamış `Delivered`'ları yuvaları tutar.
        for &id in &ids {
            let message = receipt(id, FileReceiptStatus::Ok);
            let passed = files.handle(
                &mut messenger,
                MessengerEvent::Other {
                    peer: peers[0],
                    message,
                },
            );
            assert_eq!(passed, None);
        }
        assert_eq!(files.outgoing_count(), 0);
        assert_eq!(
            files.send_file(&messenger, peers[0], "a", Vec::new()),
            Err(FileSendError::QueueFull),
            "alınmamış son olaylar yuva tutar"
        );

        // Kuyruk boşalınca iki yönde de yer açılır; hiçbir olay düşmedi.
        let events = drain(&mut files);
        assert_eq!(events.len(), MAX_PENDING_FILE_EVENTS);
        let (received, outgoing) = events.split_at(MAX_INCOMING_TRANSFERS);
        let (sent_events, delivered) = outgoing.split_at(MAX_OUTGOING_TRANSFERS);
        assert!(received
            .iter()
            .all(|event| matches!(event, FileEvent::Received { .. })));
        assert!(sent_events
            .iter()
            .all(|event| matches!(event, FileEvent::Sent { .. })));
        assert!(delivered
            .iter()
            .all(|event| matches!(event, FileEvent::Delivered { .. })));
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

    /// `FILE_ABORT` (gönderen-tek) eşin GÖNDERDİĞİ aktarıma uygulanır — eski "alıcı nedeni" ile
    /// gelse de; `FILE_RECEIPT` (alıcı-tek) eşe GİDEN aktarıma. Hiçbiri başka bir eşin aktarımına
    /// geçmez; `FILE_BEGIN`'i henüz sunulmamış aktarımın alındısı yok sayılır. Alıcı hiçbir yolda
    /// `FILE_ABORT` göndermez.
    ///
    /// `d3bc7407`'de KIRMIZI (uyarlansa da): yön nedenden çıkarılıyordu — eşten gelen
    /// `FILE_ABORT(hash_mismatch)` GİDEN aktarımı bitiriyordu (`SendAborted`), gelen aktarım sürüyordu.
    #[test]
    fn file_abort_and_file_receipt_apply_to_opposite_directions() {
        let (bob, carol) = (peer(1), peer(2));
        let mut messenger = messenger_with(&[bob, carol]);
        let mut files = FileTransfer::new(clock());
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
        for reason in [FileAbortReason::StorageFull, FileAbortReason::UserCancel] {
            assert_eq!(
                feed(&mut files, &mut messenger, carol, abort(id, reason)),
                None
            );
        }
        assert_eq!((files.outgoing_count(), files.incoming_count()), both);
        // FILE_ABORT, nedeni ne olursa olsun, eşten GELEN aktarımı kapatır; giden aktarım sürer.
        assert_eq!(
            feed(
                &mut files,
                &mut messenger,
                bob,
                abort(id, FileAbortReason::HashMismatch)
            ),
            Some(FileEvent::Aborted {
                peer: bob,
                transfer_id: id,
                reason: FileAbortReason::HashMismatch,
            })
        );
        assert_eq!((files.outgoing_count(), files.incoming_count()), (1, 0));
        // FILE_BEGIN'i henüz sunulmamış aktarımın alındısı yok sayılır (eş onu görmedi).
        let refused = receipt(id, FileReceiptStatus::Rejected);
        assert_eq!(feed(&mut files, &mut messenger, bob, refused.clone()), None);
        assert_eq!(files.outgoing_count(), 1);
        files.pump(&mut messenger);
        assert_eq!(
            drain(&mut files),
            vec![FileEvent::Sent {
                peer: bob,
                transfer_id: id,
            }]
        );
        // Başka eşin alındısı alındı bekleyen aktarıma da dokunmaz (kimlik o eşe gitmiyor).
        assert_eq!(
            feed(&mut files, &mut messenger, carol, refused.clone()),
            None
        );
        assert_eq!(files.outgoing_count(), 1);
        // FILE_RECEIPT eşe GİDEN aktarımı bitirir.
        assert_eq!(
            feed(&mut files, &mut messenger, bob, refused),
            Some(FileEvent::SendFailed {
                peer: bob,
                transfer_id: id,
                reason: SendFailure::Rejected,
            })
        );
        assert_eq!(files.outgoing_count(), 0);
        let stats = files.stats();
        assert_eq!((stats.aborts_unmatched, stats.receipts_ignored), (2, 2));
        // Gidenler yalnız giden aktarımın kayıtları; alıcı FILE_ABORT ve alındı göndermedi.
        let types: Vec<MsgType> = sent(&messenger).iter().map(AppMessage::msg_type).collect();
        assert_eq!(
            types,
            vec![MsgType::FileBegin, MsgType::FileChunk, MsgType::FileEnd]
        );
    }

    /// `d3bc7407`'de KIRMIZI (uyarlansa da): son durum — `FILE_END` kabulüyle aktarım tablodan
    /// çıkıyordu (`outgoing_count() == 0`), alındı beklemiyordu; kimlikler tek, küresel bir bellek
    /// sayacından geliyordu (aynı değerler, başka nedenle). Kayıt sırası AYNI DAVRANIŞ.
    #[test]
    fn pump_is_fair_across_peers_and_sequential_within_a_peer() {
        let (bob, carol) = (peer(1), peer(2));
        let mut messenger = messenger_with(&[bob, carol]);
        let mut files = FileTransfer::new(clock());
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
        // Eş başına sayaçlar (bob 0, 1; carol 0) + açık aktarımlar arasında benzersizlik: carol'ın
        // 0'ı bob'un açık `a`'sıyla, bob'un 1'i carol'ın açık `b`'siyle çakışır ve atlanır.
        assert_eq!((a, b, c), (0, 1, 2));
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
        // Tur başına eş başına bir kayıt; BOB'un ikinci aktarımı birincisinin `FILE_END`'i kabul
        // edilince başlar (alındı beklemesi sıradakini bekletmez).
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
        assert_eq!(files.outgoing_count(), 3, "üçü de alındı bekliyor");
    }

    /// `d3bc7407`'de AYNI DAVRANIŞ (olayın alanı `error` → `reason: SendFailure::Transport`).
    #[test]
    fn transient_refusal_waits_and_permanent_refusal_fails_the_transfer() {
        let bob = peer(1);
        let mut messenger = messenger_with(&[bob]);
        let mut files = FileTransfer::new(clock());
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
                reason: SendFailure::Transport(TransportError::UnitTooLarge),
            })
        );
        assert_eq!(files.outgoing_count(), 0);
        assert!(sent(&messenger).is_empty(), "hiçbir kayıt kabul edilmedi");
    }

    /// Erken iptal kaydı: eş başına en çok [`MAX_INCOMING_TRANSFERS_PER_PEER`] kimlik (en eski düşer;
    /// aynı kimliğin tekrar `FILE_ABORT`'u başka bir kaydı düşürmez), toplam en çok
    /// [`MAX_EARLY_ABORTS`] (en eski yer açar, bir sonraki durur), ömür [`FILE_IDLE_TTL`] (sınırdan bir
    /// adım önce reddeder, tam sınırda reddetmez). Kayıttaki kimliğin `FILE_BEGIN`'i `rejected` alır ve
    /// kayıt silinir; düşmüş ya da süresi dolmuş kimliğinki kabul edilir.
    ///
    /// `d3bc7407`'de UYARLANAMAZ: kayıt ve saat yoktu; eşleşmeyen `FILE_ABORT` yalnız sayılıyordu ve
    /// geç `FILE_BEGIN` aktarımı başlatırdı.
    #[test]
    fn early_abort_record_is_bounded_and_expires() {
        // (a) Eş başına tavan: üç kimlik birer saniye arayla; son ikisi kalır.
        let clock = TestClock::default();
        let bob = peer(1);
        let mut messenger = messenger_with(&[bob]);
        let mut files = FileTransfer::new(Arc::new(clock.clone()));
        let superseded = |id| abort(id, FileAbortReason::Superseded);
        for id in 0..3 {
            assert_eq!(feed(&mut files, &mut messenger, bob, superseded(id)), None);
            clock.advance(Duration::from_secs(1));
        }
        assert_eq!(
            files.early_aborts_of(bob).count(),
            MAX_INCOMING_TRANSFERS_PER_PEER
        );
        assert_eq!(feed(&mut files, &mut messenger, bob, superseded(2)), None);
        assert_eq!(
            files.early_aborts_of(bob).count(),
            MAX_INCOMING_TRANSFERS_PER_PEER,
            "tekrar FILE_ABORT başka kaydı düşürmez"
        );
        for id in [1, 2] {
            assert_eq!(
                feed(&mut files, &mut messenger, bob, begin(id, 10, 1)),
                None
            );
        }
        assert_eq!(files.incoming_count(), 0, "kayıttaki kimlikler reddedildi");
        assert_eq!(files.stats().begins_after_abort, 2);
        assert_eq!(files.early_aborts_of(bob).count(), 0, "kayıt silindi");
        assert_eq!(feed(&mut files, &mut messenger, bob, begin(0, 10, 1)), None);
        assert_eq!(files.incoming_count(), 1, "düşen en eski kimlik reddetmez");
        assert_eq!(
            sent(&messenger),
            vec![
                receipt(1, FileReceiptStatus::Rejected),
                receipt(2, FileReceiptStatus::Rejected),
            ]
        );

        // (b) Ömür sınırı tam.
        let clock = TestClock::default();
        let mut files = FileTransfer::new(Arc::new(clock.clone()));
        assert_eq!(feed(&mut files, &mut messenger, bob, superseded(7)), None);
        clock.advance(FILE_IDLE_TTL - TICK);
        assert_eq!(feed(&mut files, &mut messenger, bob, begin(7, 10, 1)), None);
        assert_eq!(files.incoming_count(), 0, "sınırdan bir adım önce reddeder");
        assert_eq!(feed(&mut files, &mut messenger, bob, superseded(8)), None);
        clock.advance(FILE_IDLE_TTL);
        assert_eq!(feed(&mut files, &mut messenger, bob, begin(8, 10, 1)), None);
        assert_eq!(files.incoming_count(), 1, "tam sınırda reddetmez");

        // (c) Toplam tavan: eş tablosu dolu motor, her eşe tavan kadar kimlik birer ms arayla; yeni
        // kayıt EN ESKİ'yi düşürür.
        let clock = TestClock::default();
        let max_peers = u8::try_from(MAX_PEERS).expect("küçük sayı");
        let peers: Vec<PeerRef> = (1..=max_peers).map(peer).collect();
        let mut full = messenger_with(&peers);
        let extra = peer(max_peers + 1);
        let mut other = messenger_with(&[extra]);
        let mut files = FileTransfer::new(Arc::new(clock.clone()));
        let per_peer = TransferId::try_from(MAX_INCOMING_TRANSFERS_PER_PEER).expect("küçük sayı");
        for &p in &peers {
            for id in 0..per_peer {
                assert_eq!(feed(&mut files, &mut full, p, superseded(id)), None);
                clock.advance(Duration::from_millis(1));
            }
        }
        assert_eq!(files.aborted_early.len(), MAX_EARLY_ABORTS);
        assert_eq!(feed(&mut files, &mut other, extra, superseded(0)), None);
        assert_eq!(files.aborted_early.len(), MAX_EARLY_ABORTS);
        assert_eq!(feed(&mut files, &mut full, peers[0], begin(0, 10, 1)), None);
        assert_eq!(files.incoming_count(), 1, "en eski kayıt düştü");
        assert_eq!(feed(&mut files, &mut full, peers[0], begin(1, 10, 1)), None);
        assert_eq!(files.incoming_count(), 1, "bir sonraki duruyor");
        assert_eq!(feed(&mut files, &mut other, extra, begin(0, 10, 1)), None);
        assert_eq!(files.incoming_count(), 1, "yeni kayıt duruyor");
    }

    /// Alındı kuyruğu: Transport geçici reddederken alındılar bekler (tele çıkmamıştır); kuyruk en çok
    /// [`MAX_PENDING_RECEIPTS`] alındı tutar (en eski düşer); Transport açılınca kalanlar doğuş
    /// sırasıyla gider. [`FILE_RECEIPT_TTL`]'den eski alındı düşer (sınırdan bir adım önce gider).
    ///
    /// `d3bc7407`'de UYARLANAMAZ: alındı yoktu; alıcının tek kaydı (`FILE_ABORT`) tek denemelikti.
    #[test]
    fn pending_receipts_wait_and_are_bounded_and_expire() {
        let clock = TestClock::default();
        let bob = peer(1);
        let mut messenger = messenger_with(&[bob]);
        let mut files = FileTransfer::new(Arc::new(clock.clone()));
        let count = TransferId::try_from(MAX_PENDING_RECEIPTS + 1).expect("küçük sayı");
        messenger.transport_mut().refuse = Some(TransportError::Backpressure);
        for id in 0..count {
            let [first, end] = empty_file(id);
            assert_eq!(feed(&mut files, &mut messenger, bob, first), None);
            assert!(matches!(
                feed(&mut files, &mut messenger, bob, end),
                Some(FileEvent::Received { .. })
            ));
        }
        assert!(sent(&messenger).is_empty(), "hiçbiri tele çıkmadı");
        assert_eq!(
            files.stats().receipts_unsent,
            1,
            "kuyruk taştı: en eski düştü"
        );
        assert!(files.stats().offers_refused > 0);
        messenger.transport_mut().refuse = None;
        files.pump(&mut messenger);
        let ok = |id| receipt(id, FileReceiptStatus::Ok);
        assert_eq!(sent(&messenger), (1..count).map(ok).collect::<Vec<_>>());
        let pending = u64::try_from(MAX_PENDING_RECEIPTS).expect("küçük sayı");
        assert_eq!(files.stats().receipts_sent, pending);

        // Ömür: A tam sınırda düşer, bir adım genç olan B gider.
        let before = sent(&messenger).len();
        messenger.transport_mut().refuse = Some(TransportError::Backpressure);
        for message in empty_file(count) {
            let _ = feed(&mut files, &mut messenger, bob, message);
        }
        clock.advance(TICK);
        for message in empty_file(count + 1) {
            let _ = feed(&mut files, &mut messenger, bob, message);
        }
        clock.advance(FILE_RECEIPT_TTL - TICK);
        messenger.transport_mut().refuse = None;
        files.pump(&mut messenger);
        assert_eq!(sent(&messenger)[before..], [ok(count + 1)]);
        assert_eq!(files.stats().receipts_unsent, 2);
    }

    /// Depo eşin `u64` alanının tükendiğini bildirince: tipli hata, sarma yok, `FILE_BEGIN` yok.
    ///
    /// `d3bc7407`'de UYARLANAMAZ: sayaç motorun belleğindeydi (`next_transfer_id`), depo okunmuyordu.
    #[test]
    fn transfer_id_exhaustion_is_typed_and_never_wraps() {
        let bob = peer(1);
        let last_value = CounterKind::TransferId.max_value() - 1;
        let store = Arc::new(SeededStore::counting_from(last_value));
        let mut messenger = messenger_on(store, &[bob]);
        let mut files = FileTransfer::new(clock());
        let last = files
            .send_file(&messenger, bob, "a", Vec::new())
            .expect("son transfer_id kullanılabilir");
        assert_eq!(last, last_value);
        files.pump(&mut messenger);
        let sent_before = sent(&messenger).len();
        for _ in 0..2 {
            assert_eq!(
                files.send_file(&messenger, bob, "a", Vec::new()),
                Err(FileSendError::TransferIdExhausted),
                "tükenme kalıcıdır"
            );
        }
        files.pump(&mut messenger);
        assert_eq!(
            files.outgoing_count(),
            1,
            "yalnız ilk aktarım (alındı bekliyor)"
        );
        assert_eq!(sent(&messenger).len(), sent_before, "FILE_BEGIN yok");
    }

    /// Depo sözleşmesine aykırı (hep aynı değeri veren) depo: açık bir aktarımla çakışan değer
    /// kullanılmaz, atlama döngüsü sınırlıdır → `Store(Corrupt)`; aktarım kuyruğa girmez, yalnız ilk
    /// aktarımın kayıtları sunulur. Eşin KENDİ açık aktarımı da çakışır (sözleşmeye uyan depoda
    /// olmaz).
    ///
    /// `d3bc7407`'de UYARLANAMAZ: motor depoyu okumuyordu (`FileSendError::Store` yoktu).
    #[test]
    fn a_store_that_breaks_the_counter_contract_is_corrupt_and_queues_nothing() {
        let (bob, carol) = (peer(1), peer(2));
        let repeated = 7;
        let mut messenger = messenger_on(Arc::new(SeededStore::repeating(repeated)), &[bob, carol]);
        let mut files = FileTransfer::new(clock());
        assert_eq!(files.send_file(&messenger, bob, "a", vec![1]), Ok(repeated));
        for to in [carol, bob] {
            assert_eq!(
                files.send_file(&messenger, to, "b", vec![1]),
                Err(FileSendError::Store(AppStoreError::Corrupt))
            );
        }
        assert_eq!(files.outgoing_count(), 1);
        files.pump(&mut messenger);
        assert!(sent(&messenger)
            .iter()
            .all(|message| transfer_of(message) == repeated));
    }

    /// `d3bc7407`'de AYNI DAVRANIŞ.
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
