//! [ASİMETRİK-YOL · ürün sonucu] Dar dönüşlü hatta dosya aktarılırken metin ve alındısı gelir,
//! PTT ilerler ve dosya biter — dört gözlem tek koşuda, gerçek uygulama motorlarıyla.
//!
//! # Neyi sınar
//!
//! Ürün hedefi: akışlar birbirini bloklamaz; yüksek kayıp, yüksek gecikme ve düşük bantta iletişim
//! sürer. Yön hızı profili (`RuntimeConfig::link_rates`) ve ona bağlı üç değişiklik (alıcı ACK
//! dönüş bütçesi + profil pacer tavanı + dosya penceresi tabanı; dar hatta hat modeli ve akış sonu
//! ACK düzeltmeleri; dosya biriminin motora birikim sınırıyla verilmesi) tek bir ürün sonucuna
//! bağlıdır. Bu test o sonucu uygulama katmanında ölçer: tanımlı asimetrik hatta bir dosya
//! aktarılırken
//!
//! 1. gerçek bir TEXT ve onun AMP alındısı (`RECEIPT(delivered)`) gelir,
//! 2. PTT ilerlemeye devam eder,
//! 3. dosya tamamlanır.
//!
//! # Düzenek
//!
//! İki gerçek `ClientTransport`, ürün yapılandırması (`ClientTransportConfig::product`: port motoru,
//! şeritler açık, uyarlamalı FEC; yalnız kapanış boşaltma tavanı 100 ms — ölçülen süreye girmez).
//! Gerçek kripto (`NoiseWireProtection`, ürünün `Legacy` zarfı). Üstünde gerçek `kama-app`
//! motorları, ürün bileşim kökündeki (`kama-node` düğüm turu) sırayla: `Messenger` (Transport'un
//! sahibi), `Ptt` (`Messenger::poll`'un tek okuyucusu), `FileTransfer` (`handle` zinciri ve
//! kuyruk boşalınca `pump`). El sıkışma zarfları `MemoryHub`'dan geçer (hattın dışında, beyanlı).
//!
//! Veri yolu kullanıcı-alanı biçimlendirici röleden geçer; hücre `client-core`
//! `tests/shaped_relay.rs` asimetrik hücresidir ve biçimlendirici onun kuralıyla aynıdır
//! (serileştirme tel baytıyla — UDP/IP 28 B dahil —, sınırlı FIFO, sabit tek yön gecikme,
//! ileri yönde her pakette tohumlu rastgele kayıp, aynı LCG; dönüş tohumu `tohum ^ 0x5A5A`):
//!
//! * ileri (Alice tarafı → Bob tarafı): 12,5 kB/s, 64 paket kuyruk, 300 ms, %20 kayıp;
//! * dönüş (Bob tarafı → Alice tarafı): 62 B/s, 64 paket kuyruk, 300 ms, kayıpsız.
//!
//! `shaped_relay.rs`'ten tek fark: o hücrede yalnız tek yönlü oturum vardır. Burada iki uç da
//! veri gönderir (Bob alındıları), bu yüzden her yönün HATTI iki trafiği birlikte taşır: ileri hat
//! Alice'in verisini ve Alice'in Bob'a ACK'lerini, dönüş hattı Bob'un ACK'lerini ve Bob'un
//! verisini (alındılar). Gerçek asimetrik hatta da böyledir; alındı 62 B/s hattı ACK'lerle paylaşır.
//!
//! Yön hızları iki uca kendi bakış açısından verilir ve hattın gerçek hızlarıdır: Alice yukarı
//! 12 500 / aşağı 62 B/s, Bob yukarı 62 / aşağı 12 500 B/s. `KAMA_ASYM_PROFILE=off` profili iki
//! uçtan da kaldırır (ürün varsayılanı: `link_rates = None`).
//!
//! # Senaryo (t = 0: dosya kuyruğa alınır)
//!
//! * t = 0: Alice 40 000 B'lik dosyayı gönderir (`FileTransfer::send_file`; 41 parça × ≤ 990 B).
//! * t = 0,3 s: metin 1 (dosyanın başı).
//! * t = 0,5 s: PTT açılır (`Ptt::open`, Lyra, 200 ms demet) ve her 200 ms'de 80 B'lik bir demet
//!   gider (gerçek Lyra 3,2 kbit/s: 20 ms'de 8 B; [PTT-BANT · 2026-11-22] eskiden 40 B yapay yük).
//!   Yük ve demet süresi `KAMA_ASYM_PTT_BYTES` / `KAMA_ASYM_PTT_MS` ile değişir ([`ptt_shape`]).
//!   Demetler Bob dosyayı alana dek sürer, sonra `Ptt::close`.
//! * Bob dosyanın yarısını (parça sayısının yarısı) aldığı turda metin 2 (dosyanın ortası).
//!
//! # Ölçülenler (koşu başına)
//!
//! * metin gecikmesi: `send_text` → Bob'da `TextReceived`;
//! * alındı dönüşü: Bob'da `TextReceived` (alındının sunulduğu an) → Alice'te `Delivered`; ayrıca
//!   gidiş-dönüş (`send_text` → `Delivered`);
//! * PTT: ilk demetin gecikmesi, demet başına gecikme (sesin oluştuğu an = demetin ilk 20 ms
//!   çerçevesinin başı, planlanan an − demet süresi → Bob'da `PttAudio`; [PTT-BANT · sahip kararı
//!   2026-11-22] eskiden planlanan sunum anından), dosya
//!   sürerken teslimler arası en büyük boşluk;
//! * dosya: `send_file` → Bob'da `FileEvent::Received` (bayt bayt doğrulanır); Alice'te
//!   `FileEvent::Delivered` ayrıca;
//! * iki yönde tel baytı ve paket (UDP/IP dahil), kuyruk ve rastgele düşmeler.
//!
//! Kayıp sınıflandırması ölçümle değil kayıtla yapılır: iki ucun kriptosu ince bir sarmalayıcıyla
//! ([`Tap`]) sarılır; mühürlenen her DATA datagramının parmak izi (FNV-1a 64) çözülmüş AMP kaydıyla
//! eşlenir, röle düşürdüğü her datagramın parmak izini yazar. "Birim kayboldu" = birimin İLK
//! iletimi rölede düştü. Sarmalayıcı yalnız okur; mühürlü baytlara ve kararlara dokunmaz.
//!
//! # Eşikler (ölçümden ÖNCE sabitlendi) ve ürün gerekçesi
//!
//! * METİN ≤ 1 s (her biri). Konuşma düzeyinde etkileşim: 300 ms tek yönün üstüne dosyanın önüne
//!   geçmiş birkaç datagramdan fazlası beklenmez. İstisna: metnin kendi ilk iletimi düştüyse
//!   teslim ACK tabanlı kurtarmaya kalır (bu hatta ACK ≈ 5 s'de bir); o zaman "kayıp-kurtarma"
//!   olarak süresiyle yazılır ve ölçüt düşmüş sayılmaz. Hiç gelmezse düşer.
//! * ALINDI dönüşü ≤ 4 s (her biri). Fizik: alındı datagramı 71 B tel (AMP 10 B + DATA başlığı 11 +
//!   Noise `Legacy` 22 + UDP/IP 28), 62 B/s'de 1,15 s serileşir; + 300 ms tek yön; + önünde hatta
//!   serileşmekte olan en çok bir denetim datagramı (en büyük ACK V2 çerçevesi 79 B → 129 B tel,
//!   2,08 s). Toplam 3,53 s; kalan 0,47 s iki çekirdekli makinede iş parçacığı zamanlaması. Dönüş
//!   kayıpsızdır; alındının ilk iletimi yine de rölede (kuyruk taşması) düşerse "kayıp-kurtarma".
//! * PTT ≤ 1 s (her demet, ilki dahil) ve gönderilen her demet teslim edilir. Tek istisna
//!   "kayıp-kurtarma": demetin KENDİ ilk iletimi düştü. [2026-11-16 düzeltmesi: ilk ölçümde
//!   istisna sayılan "demet Bob'a vardığında PTT şeridinde ilk iletimi düşmüş bir öncülü henüz
//!   teslim edilmemişti" durumu artık istisna DEĞİLDİR; sınır (1 s) aynı.] PTT ürün kararıyla
//!   güvenilir ve sıralı bir kliptir (`kama-app` `ptt.rs`, PTT-R0: "ağ takılırsa ses bozulmaz,
//!   durur"): öncülünü bekleyen demet sınırı aşarsa ölçüt düşer; bu demetler ayrı sütunda
//!   ("öncül_bekledi") sayılır ve en büyük gecikmeleriyle yazılır, ki düşüşün kaynağı (dosyanın
//!   bloklaması mı, PTT'nin kendi sıralı kurtarması mı) okunabilsin. [2026-11-16 ürün kabulü:
//!   profil açıkken "kayıp-kurtarma" istisnası da YOKTUR — bütün demetlerin en büyük gecikmesi
//!   ≤ 1 s; istisna yalnız profil kapalı teşhis kolunda raporlanır. Sınır (1 s) aynı.] PTT'nin dar dönüşte önlemsel
//!   fazlalık ya da son tarih politikası alması sahibin kararıdır; bu test politikayı değiştirmez.
//! * DOSYA ≤ 14 s (`send_file` → Bob'da tamamlanma). Bu hücrenin dosya sınırı 40 × 1000 B için
//!   14 s'dir; sınır yük baytıyla ölçeklenir: 14 s × 40 000 B / 40 000 B = 14 s. Kredi verilmeyen
//!   iki fark: parça başına AMP + DATA + Noise zarfı teli ≈ %3 büyütür (41 datagram, ≈ 43,4 kB;
//!   `shaped_relay` hücresi 40 × 1055 B ≈ 42,2 kB) ve PTT (≈ 114 B tel / 200 ms ≈ 0,57 kB/s, hattın
//!   %4,6'sı) ile metinler aynı ileri hattı paylaşır.
//! * Koşu tavanı 60 s; o ana dek gelmeyen her gözlem düşmüş sayılır.
//!
//! # Kalıcı kapı hücreleri (2026-11-20; sınırlar ve gerekçeleri ölçümden ÖNCE yazıldı)
//!
//! Yön ve trafik her hücrede aynıdır: Alice (baz; beyan yukarı 12 500 / aşağı 62 B/s) dosyayı,
//! PTT'yi ve metinleri Bob'a (saha; beyan yukarı 62 / aşağı 12 500 B/s) gönderir; Bob alındıları
//! gönderir. Saha → baz PTT (62 B/s) kapsam dışıdır. Beyan edilen hızlar (profil) her hücrede
//! aynıdır; hücre yalnız GERÇEK hattı değiştirir. Gerçek dönüş hızı her hücrede 62 B/s'dir.
//!
//! | hücre | gerçek ileri | beyan ileri | gerçek dönüş | dosya sınırı |
//! |---|---|---|---|---|
//! | `dürüst-12500` | 12 500 | 12 500 | 62, 300 ms | 14 s |
//! | `gerçek-10000` | 10 000 | 12 500 | 62, 300 ms | 17,5 s |
//! | `gerçek-6250` | 6 250 | 12 500 | 62, 300 ms | 28 s |
//! | `düşüş-toparlanma` | 12 500 → (8 s) 6 250 → (20 s) 12 500 | 12 500 | 62, 300 ms | 20 s; dosya 2 (24 s'de) 14 s |
//! | `dönüş-gecikmesi-değişken` | 12 500 | 12 500 | 62, 300–700 ms | 14 s |
//!
//! Sınırlar (hepsi her hücrede, iddia edilir):
//! * PTT ≤ 1 s her demet, METİN ≤ 1 s her biri, ALINDI dönüşü ≤ 4 s: tanımlar yukarıdaki
//!   "Eşikler" bölümüyle aynıdır (metnin kendi ilk iletimi düştüyse "kayıp-kurtarma" istisnası aynen;
//!   PTT'de profil açıkken istisna yok). Gerekçe: ürün hedefi gerçek hat profilden yavaş olsa da
//!   aynıdır; PTT + metin (≈ 0,6 kB/s) 6 250 B/s hattın %10'undan azdır, sınır fizikle tutulabilir.
//! * Ölçütler EVRE başına ayrıca raporlanır ve iddia edilir (demet planlandığı, metin gönderildiği
//!   evreye düşer): `başlangıç` = 0–8 s (ilk bilgi taşıyan ACK ölçümlerde 4,3–7,9 s'de geldi;
//!   oturum başı yalnız 15 s sonrası değil, bu pencerede de sınırın içinde olmalı), `sürekli` = 8 s
//!   sonrası. Düşüş hücresinde `başlangıç` 0–8 s, `daralma` 8–20 s, `toparlanma` 20 s sonrası; tek
//!   oturum, yeniden bağlanma yok. Yalnız dosyanın bitmesi hücreyi geçirmez.
//! * DOSYA sınırı gerçek hızla ölçeklenir: gerçek hattın t0'dan taşıyabildiği bayt, dürüst hücrenin
//!   bütçesine (14 s × 12 500 B/s = 175 000 B) ulaştığı an. Sabit hızda 14 s × 12 500 / gerçek;
//!   düşüş hücresinde 8 × 12 500 + 12 × 6 250 = 175 000 B → 20 s. Düşüş hücresinde dosya 2 hat
//!   12 500'e döndükten 4 s sonra (24 s) gönderilir ve dürüst sınıra (14 s) tabidir: oturum içinde
//!   hızlanan hattın yeniden kullanılması ölçütüdür.
//! * Değişken dönüş gecikmesi hücresinde ALINDI sınırı 4 s + 0,4 s: alındının kendisi bu dönüş
//!   hattından geçer ve gecikme fazlası en çok 400 ms'dir (fizik gerekçesi aynı, gecikme terimi
//!   300 ms yerine en çok 700 ms). İleri hat dürüst olduğundan dosya sınırı 14 s: dönüş gecikmesinin
//!   artışı ileri hattın daralması sayılmamalıdır.
//! * Metinler: metin 1 (0,3 s), metin 2 (dosya yarısı), sabit hızlı hücrelerde ayrıca 10 s; düşüş
//!   hücresinde ayrıca 12 s (daralma) ve 26 s (toparlanma). PTT, dosya(lar) Bob'a varana dek sürer.
//!
//! # Fikstür düzeltmesi (2026-11-21, sahip koşullu kabulü; sınırlar aynı)
//!
//! İlk turda PTT üretimi dosyanın bitmesine bağlıydı (`files_done`): düşüş hücresinde PTT dosya 2
//! Bob'a varınca kapanıyor, toparlanma evresinin kapsamı dosya 2'nin süresine kalıyordu. Artık
//! düşüş hücresinde PTT dosyadan bağımsız 0,5–40 s konuşur (üç evrenin üçünde) ve metinler
//! 0,3 s (başlangıç), 12 s (daralma), 26 s ve 34 s (toparlanma) gönderilir. Sabit hızlı ve dönüş
//! gecikmesi hücrelerinde PTT eski kuralla dosya bitene dek sürer (sürekli evrede PTT niyeti yok).
//! Evre sınıflandırması: evrede planlanmış demet/metin yoksa niyetli evrede "fikstür geçersiz"
//! (kanıt yok; düşmüş sayılır), niyetsiz evrede "gözlem yok"; planlanmış ama uygulamaca kabul
//! edilmemiş (geri basınç), teslim edilmemiş ya da sınırı aşmış demet KALDI. Bu değişiklik eski
//! KALDI sonuçlarını geriye dönük değiştirmez.
//!
//! # Canlılık (2026-11-21; ölçümden önce)
//!
//! Her hücrede ek ölçüt CANLILIK: planlanan her PTT demeti (uygulamaca kabul edilmeyenler dahil)
//! planlandığı andan en çok 15 s içinde Bob'a teslim edilir ([`LIVENESS_HORIZON`] gerekçesi). Ayrı
//! hücre `gate_ptt_liveness_long_session`: düşüş-toparlanma hattında PTT 0,5–60 s (298 demet, şerit
//! kabul açıklığı 256'dan uzun), koşu tavanı 80 s; yalnız canlılık iddia edilir.
//!
//! # PTT teslim sözleşmesi (sahip kararı 2026-11-22; ölçümden önce)
//!
//! PTT media'daki gibi: sesin oluştuğu andan (demet başı; 2026-11-22, önce sunum anı) sonra ≤ 1 s ([`PTT_BOUND`]) içinde Bob'a varırsa
//! zamanında sayılır; geç kalan ya da hiç gelmeyen demet atlanır ve kaybı kodek örter. PTT ölçütü
//! (genel ve evre başına): zamanında demet oranı ≥ %75 ([`PTT_TIMELY_MIN_PERMILLE`]; payda
//! planlanan + uygulamaca kabul edilmeyen demet). Bant ölçütü: PTT bir yönde ≤ 5 kbit/s (2026-11-22; önce 4,5)
//! (media'nın iki yönlü görüşmesinin bir yönü); 5,3 kbit/s'lik yedekli biçim reddedildi.
//! CANLILIK bu sözleşmede: uygulamaca kabul edilmeyen demet yok ve art arda teslim edilmeyen demet
//! dizisi [`LIVENESS_HORIZON`]'dan uzun değil (akış ölmez). Önceki "her demet ≤ 1 s / her demet
//! ufukta" ölçütü kopyalı PTT sözleşmesiydi; eski KALDI sonuçlarını geriye dönük değiştirmez.
//!
//! Hücreler aynı süreçte sırayla koşar (zamanlama ölçümü). `KAMA_ASYM_SEED` hepsine uygulanır.
//! `İZ-NEDEN` satırları her yeniden gönderimin tetikleyicisini (ARQ paket/zaman eşiği, PTO probu,
//! kopya türü) ve gerekliliğini yazar; "gereksiz" = birim o an Bob'a DATA olarak varmıştı. FEC ile
//! kurtarılan birim DATA varışı sayılmaz (paket numarası alınmadı); RECOVERED çerçeve sayısı ayrı.
//!
//! # Koşturma
//!
//! Varsayılan (CI): tohum 1, profil açık; dört ölçütten biri düşerse test düşer. Ölçüm:
//! `KAMA_ASYM_SEED=<n>` (tohum), `KAMA_ASYM_PROFILE=off` (profil kapalı kol),
//! `KAMA_ASYM_REAL_FWD_BPS=<B/s>` (yanlış profil teşhis kolu: profil ileri hattı yine
//! [`FWD_BPS`] bildirir, rölenin gerçek ileri hızı bu değer olur; ölçütler raporlanır, kabul
//! iddiası yalnız ürün kolunda — gerçek = profil — uygulanır). Her koşu `ÜRÜN-SONUÇ`
//! ve `ÖLÇÜT` satırlarını basar (`--nocapture`); ölçütler önce basılır, sonra iddia edilir. Düşen
//! ölçütün mekanizması aynı koşunun izinden okunur: `İZ-UYG` (uygulama olayları), `İZ-PTT` (demet
//! başına plan, ilk çıkış, iletim sayısı, ilk varış, teslim) ve `İZ-HAT` (rölenin gördüğü her
//! datagram: yön, yol, an, tel boyu, sonuç, varış ve taşıdığı AMP kaydı).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use adaptive_transport::ack::AckFrame;
use adaptive_transport::sender::{EmitCause, EmitDiag, LineGuardAckDecision, LineGuardAckTrace};
use client_core::memory_link::MemoryHub;
use client_core::runtime::ClientLiveDiag;
use client_core::runtime::EmitTraceSink;
use client_core::runtime::{AckFormat, LinkRates, PTT_RELIABLE_COPIES};
use client_core::transport::{ClientTransport, ClientTransportConfig};
use client_core::unit_id::fec_block_of;
use client_core::wire::{
    decode_data, decode_fec, decode_ptt, decode_recovered, FRAME_FEC, FRAME_PTT, FRAME_RECOVERED,
};
use kama_app::clock::{MonotonicClock, SystemClock};
use kama_app::codec::{bundle_code_from_ms, decode, AmpLimits, AppMessage, PttCodec};
use kama_app::file_transfer::{FileEvent, FileTransfer};
use kama_app::messaging::{Messenger, MessengerEvent, MsgId};
use kama_app::ptt::{Ptt, PttError, PttEvent, PttStats, StreamId};
use kama_app::store::MemoryAppStore;
use kama_contracts::handles::PeerRef;
use kama_contracts::s1::{ConnectResult, TransportCapabilities, TransportError};
use kama_contracts::s3::{
    Opened, PacketProtection, ProtectionClass, ProtectionContext, ProtectionLimits, SealError,
    SealedUnit, SessionCommand, SessionControl, SessionEvent as CryptoEvent, SessionResult,
};
use kama_noise_wire::protector::NoiseWireProtection;
use kama_noise_wire::seam_mock::{fixture_directory, fixture_protection};
use kama_noise_wire::session_control::EnvelopeKind;

const ALICE: u8 = 0xA1;
const BOB: u8 = 0xB0;

// ── Hücre (`client-core` `tests/shaped_relay.rs` asimetrik hücresi) ──────────────────────────
const FWD_BPS: u64 = 12_500;
const REV_BPS: u64 = 62;
const QUEUE_PKTS: usize = 64;
const ONE_WAY: Duration = Duration::from_millis(300);
const FWD_LOSS_PERMILLE: u64 = 200;
/// UDP/IP zarfı (serileştirmeye girer; röle yalnız UDP gövdesini görür).
const UDP_IP_OVERHEAD: u64 = 28;

/// [PTT-BANT · sahip kararı 2026-11-22] PTT'nin bir yönde tavanı (bit/s, telde, IPv4/UDP dahil;
/// açılış, kapanış, kopya, yeniden iletim, kaybolan ve anlaşma öncesi PTT datagramları dahil).
const PTT_BAND_MAX_BPS: u64 = 5_000;

/// [PTT-BANT] Fikstürün PTT biçimi: demet yükü, demet süresi (= kadans) ve `bundle_ms` kodu.
#[derive(Debug, Clone, Copy)]
struct PttShape {
    /// Demet yükü (bayt). Varsayılan gerçek Lyra 3,2 kbit/s: 200 ms × 8 B / 20 ms = 80 B.
    bytes: usize,
    /// Demet süresi ve kadansı.
    period: Duration,
    /// `PTT_OPEN.bundle_ms` kodu ([`kama_app::codec::bundle_code_from_ms`]).
    code: u8,
    /// [PTT-BANT · kıyas · sahip 2026-11-22] `KAMA_ASYM_PTT_PRESS_MS` verilirse basış tam bu kadar
    /// sürer (demet süresinden bağımsız; son demet kısmidir: bırakışta var olan 20 ms
    /// çerçeveleri taşır). Kollar aynı basışla kıyaslanır. Yoksa hücrenin kuralı.
    press: Option<Duration>,
    /// [PTT-BANT · kol A] `KAMA_ASYM_PTT_COMPACT=1`: Alice demetleri kompakt `MEDIA_BUNDLE`
    /// (2 B başlık) ile gönderir.
    compact: bool,
    /// [PTT-BİRLEŞİK · deney] `KAMA_ASYM_PTT_COALESCE`: açılış/kapanış demetlerin içinde.
    coalesce: Coalesce,
    /// [PTT-BİRLEŞİK · deney] `KAMA_ASYM_PTT_OPEN_K` (varsayılan 2): açılış bilgisini taşıyan ilk
    /// demet sayısı.
    open_k: u8,
}

/// [PTT-BİRLEŞİK · deney, aday dışı] Birleşik biçimin kolu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coalesce {
    /// `off` (varsayılan): bugünkü ayrı güvenilir `PTT_OPEN` / `PTT_CLOSE`.
    Off,
    /// `neg`: iki uçta da açık; Alice birleşik akışı yalnız Bob'un 0x17 ACK'ini gördükten sonra
    /// açılan basışta kullanır (bant içi yetenek). Öncesinde bugünkü ayrı kayıtlar.
    Negotiated,
    /// `known`: iki uçta da açık; Bob'un yeteneği oturum başından biliniyor (bant DIŞI bilgi:
    /// önceki oturumdan hatırlanan ya da dağıtımla verilen). Transport yine soğuk başlar.
    Known,
}

/// Sonuç satırı için PTT biçimi.
fn ptt_shape_label() -> String {
    let s = ptt_shape();
    format!(
        "yük={}B/demet={}ms/başlık={}/basış={}",
        s.bytes,
        s.period.as_millis(),
        match (s.coalesce, s.compact) {
            (Coalesce::Off, true) => "kompakt2B".to_string(),
            (Coalesce::Off, false) => "tam13B".to_string(),
            (c, _) => format!("birleşik-{c:?}-K{}", s.open_k),
        },
        s.press
            .map_or("hücre".to_string(), |p| format!("{}ms", p.as_millis())),
    )
}

/// Lyra çerçeve süresi (oluşum saati ve çerçeve sayımı).
const LYRA_FRAME: Duration = Duration::from_millis(20);

/// [PTT-BANT] `KAMA_ASYM_PTT_BYTES` (varsayılan 80) ve `KAMA_ASYM_PTT_MS` (varsayılan 200).
fn ptt_shape() -> PttShape {
    let ms = u16::try_from(env_u64("KAMA_ASYM_PTT_MS", 200)).expect("KAMA_ASYM_PTT_MS u16");
    let bytes = usize::try_from(env_u64("KAMA_ASYM_PTT_BYTES", 80)).expect("bayt");
    assert!(bytes >= 4, "KAMA_ASYM_PTT_BYTES ≥ 4 (ilk 4 B demet sırası)");
    let press = std::env::var("KAMA_ASYM_PTT_PRESS_MS")
        .ok()
        .map(|v| Duration::from_millis(v.parse().expect("KAMA_ASYM_PTT_PRESS_MS sayı")));
    PttShape {
        press,
        compact: std::env::var("KAMA_ASYM_PTT_COMPACT").is_ok_and(|v| v == "1"),
        coalesce: match std::env::var("KAMA_ASYM_PTT_COALESCE").as_deref() {
            Ok("neg") => Coalesce::Negotiated,
            Ok("known") => Coalesce::Known,
            Ok("off") | Err(_) => Coalesce::Off,
            Ok(v) => panic!("KAMA_ASYM_PTT_COALESCE: off | neg | known (verilen {v})"),
        },
        open_k: u8::try_from(env_u64("KAMA_ASYM_PTT_OPEN_K", 2)).expect("KAMA_ASYM_PTT_OPEN_K u8"),
        bytes,
        period: Duration::from_millis(u64::from(ms)),
        code: bundle_code_from_ms(ms)
            .expect("KAMA_ASYM_PTT_MS kodlanabilir (≤ 200 ya da 10 ms katı ≤ 750)"),
    }
}

/// Koşunun ACK biçimi (`KAMA_ASYM_ACK_FORMAT`, varsayılan `bitmap`).
fn fixture_ack_format() -> AckFormat {
    std::env::var("KAMA_ASYM_ACK_FORMAT").map_or(AckFormat::Bitmap, |v| {
        AckFormat::parse(&v).expect("KAMA_ASYM_ACK_FORMAT: v2 | bitmap")
    })
}

/// [PTT-BANT] Bir yönün PTT bandı (bit/s): mühürlenen bütün PTT datagramlarının (açılış, kapanış,
/// kopya, konuşmadan sonra çıkan yeniden iletim dahil) tel baytı ÷ basış süresi (basış başından
/// bırakışa; sahip 2026-11-22: bütün kollarda aynı payda). Payda konuşma süresidir, datagramların
/// yayıldığı süre değil: geç yeniden iletim ortalamayı seyreltemez. PTT datagramı yoksa ya da
/// süre 0 ise `None`.
fn ptt_band_bps(wire: &[(Instant, u64)], press: Duration) -> Option<u64> {
    // (kanıt: `ptt_band_gate_passes_the_lean_payload_and_fails_the_80_byte_control`)
    if wire.is_empty() {
        return None;
    }
    window_bps(wire.iter().map(|w| w.1).sum(), press)
}

/// [PTT-BANT · gerçek zaman · sahip 2026-11-22] 20 ms çerçeveler basış başından gerçek zamanda
/// oluşur; `k`. demet `created = basış + k × demet süresi`'nde başlar ve çerçeveleri var olunca
/// (`created + demet süresi`) sunulur. Bırakış (`release`) demetin ortasındaysa demet kısmidir:
/// bırakışta var olan çerçeveler (`⌊(release − created) / 20 ms⌋`). Döner: (sunum anı, çerçeve
/// sayısı); demet hiç çerçeve taşımıyorsa `None`.
fn bundle_slot(
    opened: Instant,
    period: Duration,
    k: u32,
    release: Option<Instant>,
) -> Option<(Instant, u32)> {
    let fpb = u32::try_from(period.as_nanos() / LYRA_FRAME.as_nanos()).unwrap_or(u32::MAX);
    let created = opened + period * k;
    let full = created + period;
    match release {
        Some(r) if full > r => {
            let frames = u32::try_from(
                r.saturating_duration_since(created).as_nanos() / LYRA_FRAME.as_nanos(),
            )
            .unwrap_or(0);
            (frames > 0).then_some((r, frames.min(fpb)))
        }
        _ => Some((full, fpb)),
    }
}

/// Bayt ÷ süre → bit/s (süre 0 → `None`).
fn window_bps(bytes: u64, span: Duration) -> Option<u64> {
    if span.is_zero() {
        return None;
    }
    u64::try_from(u128::from(bytes * 8) * 1_000_000_000 / span.as_nanos()).ok()
}

// ── Senaryo ──────────────────────────────────────────────────────────────────────────────────
const FILE_BYTES: usize = 40_000;
const TEXT1_AT: Duration = Duration::from_millis(300);
const PTT_AT: Duration = Duration::from_millis(500);

/// [PTT-BANT · kıyas] Basışın başlangıcı: `KAMA_ASYM_PTT_AT_MS` (varsayılan [`PTT_AT`]). Geç
/// başlangıç (ör. 4000) yalın PTT anlaşmasından (ilk 0x16) SONRA başlayan basışı ölçer.
fn ptt_at() -> Duration {
    Duration::from_millis(env_u64(
        "KAMA_ASYM_PTT_AT_MS",
        u64::try_from(PTT_AT.as_millis()).unwrap_or(500),
    ))
}
const TEXT1: &str = "metin 1: dosyanin basinda gonderildi";
const TEXT2: &str = "metin 2: dosyanin ortasinda gonderildi";

// ── Eşikler (ölçümden önce; gerekçe modül belgesinde) ────────────────────────────────────────
const TEXT_BOUND: Duration = Duration::from_secs(1);
const RECEIPT_RETURN_BOUND: Duration = Duration::from_secs(4);
const PTT_BOUND: Duration = Duration::from_secs(1);
/// [PTT-SÖZLEŞME · sahip kararı 2026-11-22] Zamanında (≤ [`PTT_BOUND`]) varan demetlerin en küçük
/// oranı (binde); kalan demet atlanır, kaybı kodek örter (media sözleşmesi).
const PTT_TIMELY_MIN_PERMILLE: u64 = 750;

/// [PTT-SÖZLEŞME] `timely / total ≥ PTT_TIMELY_MIN_PERMILLE` (total = 0 → `false`).
fn ptt_timely_ok(timely: u64, total: u64) -> bool {
    total > 0 && timely * 1000 >= total * PTT_TIMELY_MIN_PERMILLE
}
const FILE_BOUND: Duration = Duration::from_secs(14);
const RUN_CAP: Duration = Duration::from_secs(60);

/// Kayıt tavanları (SR-AUDIT-001: sınırlı bellek).
const LEDGER_CAP: usize = 16_384;
const DROP_CAP: usize = 16_384;
/// Düğüm turundaki olay bütçesi (`kama-node` `MAX_EVENTS_PER_TURN` ile aynı rol).
const EVENTS_PER_TURN: usize = 256;
/// [İZ-ARALIK] Hat koruması ACK kaydının toplanan en çok kaydı ve aynanın yoklama aralığı (ayna
/// halkası 64 kayıt; bu hatta ACK'ler ≥ 0,5 s aralıklı).
const GUARD_TRACE_CAP: usize = 4_096;
const GUARD_TRACE_POLL: Duration = Duration::from_millis(200);

fn id(n: u8) -> PeerRef {
    let mut dest_hash = [0u8; 32];
    dest_hash[0] = 0x4E;
    dest_hash[1] = n;
    PeerRef::from_dest_hash(dest_hash)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// FNV-1a 64: datagramın parmak izi (şifreli metin iletim başına tekildir — nonce = PN).
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ── Kayıt: mühürlenen ve açılan DATA birimleri ───────────────────────────────────────────────

/// Bir DATA biriminin taşıdığı AMP kaydı (eşleme anahtarı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Item {
    Text(u32),
    Receipt(u32),
    PttOpen(u32),
    PttClose(u32),
    /// Demet sırası (yükün ilk 4 baytı).
    Bundle(u32),
    FileBegin,
    Chunk(u64),
    FileEnd,
    FileReceipt,
    Other,
}

fn item_of(message: &AppMessage) -> Item {
    match message {
        AppMessage::Text { msg_id, .. } => Item::Text(*msg_id),
        AppMessage::Receipt { ref_msg_id, .. } => Item::Receipt(*ref_msg_id),
        AppMessage::PttOpen { stream_id, .. } => Item::PttOpen(*stream_id),
        AppMessage::PttClose { stream_id } => Item::PttClose(*stream_id),
        AppMessage::MediaBundle { payload, .. }
        | AppMessage::MediaBundleCompact { payload, .. }
        | AppMessage::MediaBundleOpen { payload, .. }
        | AppMessage::MediaBundleEnd { payload, .. }
            if payload.len() >= 4 =>
        {
            Item::Bundle(u32::from_be_bytes(payload[..4].try_into().expect("4 B")))
        }
        // [PTT-BİRLEŞİK · deney] Yüksüz `END` demeti kapanış kaydının yerine geçer.
        AppMessage::MediaBundleOpen {
            stream_id,
            end: true,
            ..
        } => Item::PttClose(*stream_id),
        AppMessage::MediaBundleEnd { stream_tag, .. } => Item::PttClose(u32::from(*stream_tag)),
        AppMessage::FileBegin { .. } => Item::FileBegin,
        AppMessage::FileChunk { offset, .. } => Item::Chunk(*offset),
        AppMessage::FileEnd { .. } => Item::FileEnd,
        AppMessage::FileReceipt { .. } => Item::FileReceipt,
        _ => Item::Other,
    }
}

#[derive(Default)]
struct Ledger {
    /// Bu ucun mühürlediği her DATA datagramı: parmak izi → kayıt.
    by_fp: HashMap<u64, Item>,
    /// Kaydın ilk iletimi (an, parmak izi).
    first_tx: HashMap<Item, (Instant, u64)>,
    /// Kaydın iletim sayısı (ilk + yeniden iletim + kopya).
    tx_count: HashMap<Item, u32>,
    /// Kaydın bu uca ilk varışı (AEAD doğrulandı, DATA çerçevesi açıldı).
    first_rx: HashMap<Item, Instant>,
    /// AMP olarak çözülemeyen DATA çerçevesi (akıl sağlığı; 0 beklenir).
    unparsed: u64,
    /// Tavan dolduğu için yazılamayan kayıt.
    overflow: u64,
    /// [FEC-TEŞHİS] Bu ucun mühürlediği / açtığı FEC onarım çerçevesi ve RECOVERED çerçevesi.
    fec_tx: u64,
    fec_rx: u64,
    recovered_tx: u64,
    recovered_rx: u64,
    /// [İZ-NEDEN] Bu ucun mühürlediği her DATA datagramı: (an, PN, motor iletim sırası u16, birim,
    /// parmak izi).
    seals: Vec<(Instant, u64, u16, u64, u64)>,
    /// [CANLILIK-TEŞHİS] Birim kimliği → taşıdığı AMP kaydı (mühürleyen uç).
    unit_item: HashMap<u64, Item>,
    /// [İZ-NEDEN] Bu uca açılan DATA datagramları: motor iletim sırası (u16) → ilk varış; birim →
    /// ilk varış (DATA ile; FEC ile kurtarılan birim burada YOKTUR, ayrı sayılır).
    rx_seq: HashMap<u16, Instant>,
    rx_unit: HashMap<u64, Instant>,
    /// [İZ-NEDEN] Bu uca açılan ACK çerçeveleri: (an, teyit edilen PN'ler).
    acks_rx: Vec<(Instant, Vec<u64>)>,
    /// [İZ-ACK] `acks_rx` ile aynı sırada: ACK datagramının tel boyu (UDP gövdesi + UDP/IP 28 B)
    /// ve çerçeve türü baytı.
    ack_wire: Vec<(u64, u8)>,
    /// [PTT-BANT · #175 dış inceleme] Bu ucun mühürlediği, PTT kaydı taşıyan HER datagram (ilk
    /// iletim, kopya, yeniden iletim; yolda kaybolanlar dahil): (an, tel boyu = UDP gövdesi +
    /// UDP/IP 28 B, yalın 0x09 mu, taşıdığı kayıt). Yalın 0x09 ve veri çerçevesi (0x00) ikisi de.
    ptt_wire: Vec<PttWire>,
    /// [PTT-BANT] Bu ucun mühürlediği yalın PTT çerçevesi (0x09) sayısı.
    ptt_lean_tx: u64,
    /// [PTT-BİRLEŞİK · deney] Bu ucun mühürlediği açılış bilgili demet ve `END`'li demet sayısı.
    spurt_open_tx: u64,
    spurt_end_tx: u64,
    /// [İZ-BLOK] Bu ucun mühürlediği her DATA datagramı: (an, birim, parmak izi).
    data_log: Vec<(Instant, u64, u64)>,
    /// [İZ-BLOK] Bu ucun mühürlediği her FEC onarım datagramı: (an, blok, ESI, k_actual, parmak izi).
    fec_log: Vec<(Instant, u32, u16, u8, u64)>,
    /// [İZ-BLOK] Bu uca açılan RECOVERED çerçeveleri: (an, blok).
    recovered_log: Vec<(Instant, u32)>,
}

/// [PTT-BANT] Mühürlenen PTT datagramı: (an, tel boyu, yalın 0x09 mu, taşıdığı kayıt).
type PttWire = (Instant, u64, bool, Item);

/// [FEC-TEŞHİS] Çerçeve türü sayacı (DATA dışı).
fn note_kind(l: &mut Ledger, frame: &[u8], sealed: bool) {
    match (frame.first().copied(), sealed) {
        (Some(FRAME_FEC), true) => l.fec_tx += 1,
        (Some(FRAME_FEC), false) => l.fec_rx += 1,
        (Some(FRAME_RECOVERED), true) => l.recovered_tx += 1,
        (Some(FRAME_RECOVERED), false) => l.recovered_rx += 1,
        _ => {}
    }
}

/// Gerçek kriptoyu saran salt-okur sarmalayıcı: DATA çerçevesini çözüp kayda yazar.
struct Tap {
    inner: NoiseWireProtection,
    ledger: Arc<Mutex<Ledger>>,
    limits: AmpLimits,
}

impl Tap {
    /// [PTT-BİRLEŞİK · deney] Çerçevenin taşıdığı AMP kaydı (çözülemezse `None`).
    fn message(&self, frame: &[u8]) -> Option<AppMessage> {
        let payload = match decode_data(frame) {
            Some((_, _, p)) => p,
            None => decode_ptt(frame)?.1,
        };
        decode(payload, &self.limits).ok()
    }

    fn classify(&self, frame: &[u8]) -> Option<Result<Item, ()>> {
        // [PTT-BANT] Yalın PTT çerçevesi (0x09) de AMP kaydı taşır; eskiden yalnız 0x00 çözülüyordu.
        let payload = match decode_data(frame) {
            Some((_, _, p)) => p,
            None => decode_ptt(frame)?.1,
        };
        Some(
            decode(payload, &self.limits)
                .map(|m| item_of(&m))
                .map_err(|_| ()),
        )
    }
}

impl PacketProtection for Tap {
    fn seal(&mut self, ctx: ProtectionContext, frame: &[u8]) -> Result<SealedUnit, SealError> {
        let sealed = self.inner.seal(ctx, frame)?;
        note_kind(&mut lock(&self.ledger), frame, true);
        {
            let mut l = lock(&self.ledger);
            let fp = fingerprint(&sealed.bytes);
            if let Some((_, unit, _)) = decode_data(frame) {
                if l.data_log.len() < LEDGER_CAP {
                    l.data_log.push((Instant::now(), unit, fp));
                }
            } else if let Some((h, _)) = decode_fec(frame) {
                if l.fec_log.len() < LEDGER_CAP {
                    l.fec_log
                        .push((Instant::now(), h.block_id, h.esi, h.k_actual, fp));
                }
            }
        }
        if let Some((wseq, unit, _)) = decode_data(frame) {
            let item = self.classify(frame).and_then(Result::ok);
            let mut l = lock(&self.ledger);
            if let Some(item) = item {
                if l.unit_item.len() < LEDGER_CAP {
                    l.unit_item.insert(unit, item);
                }
            }
            if l.seals.len() < LEDGER_CAP {
                l.seals.push((
                    Instant::now(),
                    sealed.meta.full_wire_pn,
                    wseq,
                    unit,
                    fingerprint(&sealed.bytes),
                ));
            }
        }
        // [İZ-PTT-KAYIT · demet] Yalın çerçeve (0x09) birim kimliği taşımaz; PTT şeridi 0
        // olduğundan kimlik = şerit-içi sıra (fikstürde < 2¹⁶). Yalnız iz eşlemesi.
        if let Some((lo, _)) = decode_ptt(frame) {
            if let Some(Ok(item)) = self.classify(frame) {
                let mut l = lock(&self.ledger);
                if l.unit_item.len() < LEDGER_CAP {
                    l.unit_item.insert(u64::from(lo), item);
                }
            }
        }
        if let Some(c) = self.classify(frame) {
            let now = Instant::now();
            let mut l = lock(&self.ledger);
            if let Ok(item @ (Item::PttOpen(_) | Item::PttClose(_) | Item::Bundle(_))) = c {
                if l.ptt_wire.len() < LEDGER_CAP {
                    l.ptt_wire.push((
                        now,
                        sealed.bytes.len() as u64 + UDP_IP_OVERHEAD,
                        frame.first() == Some(&FRAME_PTT),
                        item,
                    ));
                }
                if frame.first() == Some(&FRAME_PTT) {
                    l.ptt_lean_tx += 1;
                }
                match self.message(frame) {
                    Some(AppMessage::MediaBundleOpen { end, .. }) => {
                        l.spurt_open_tx += 1;
                        l.spurt_end_tx += u64::from(end);
                    }
                    Some(AppMessage::MediaBundleEnd { .. }) => l.spurt_end_tx += 1,
                    _ => {}
                }
            }
            match c {
                Ok(item) if l.by_fp.len() < LEDGER_CAP => {
                    let fp = fingerprint(&sealed.bytes);
                    l.by_fp.insert(fp, item);
                    l.first_tx.entry(item).or_insert((now, fp));
                    *l.tx_count.entry(item).or_insert(0) += 1;
                }
                Ok(_) => l.overflow += 1,
                Err(()) => l.unparsed += 1,
            }
        }
        Ok(sealed)
    }
    fn open(&mut self, wire: &[u8]) -> Option<Opened> {
        let opened = self.inner.open(wire)?;
        note_kind(&mut lock(&self.ledger), &opened.frame, false);
        if let Some((block, _)) = decode_recovered(&opened.frame) {
            let mut l = lock(&self.ledger);
            if l.recovered_log.len() < LEDGER_CAP {
                l.recovered_log.push((Instant::now(), block));
            }
        }
        {
            let now = Instant::now();
            let mut l = lock(&self.ledger);
            if let Some((wseq, unit, _)) = decode_data(&opened.frame) {
                l.rx_seq.entry(wseq).or_insert(now);
                l.rx_unit.entry(unit).or_insert(now);
            } else if let Some((lo, _)) = decode_ptt(&opened.frame) {
                // [İZ-PTT-KAYIT · demet] Yalın demetin varışı (kimlik = şerit-içi sıra, şerit 0).
                l.rx_unit.entry(u64::from(lo)).or_insert(now);
            } else if let Ok(f) = AckFrame::decode_with_bitmap(&opened.frame) {
                if l.acks_rx.len() < LEDGER_CAP {
                    let pns = f.acked_full(u64::from(f.largest_seq));
                    l.acks_rx.push((now, pns));
                    l.ack_wire
                        .push((wire.len() as u64 + UDP_IP_OVERHEAD, opened.frame[0]));
                }
            }
        }
        if let Some(c) = self.classify(&opened.frame) {
            let now = Instant::now();
            let mut l = lock(&self.ledger);
            match c {
                Ok(item) => {
                    l.first_rx.entry(item).or_insert(now);
                }
                Err(()) => l.unparsed += 1,
            }
        }
        Some(opened)
    }
    fn sealed_len(&self, ctx: ProtectionContext, n: usize) -> Result<usize, SealError> {
        self.inner.sealed_len(ctx, n)
    }
    fn conservative_limits(&self, class: ProtectionClass) -> ProtectionLimits {
        self.inner.conservative_limits(class)
    }
    fn limits(&self, ctx: ProtectionContext) -> ProtectionLimits {
        self.inner.limits(ctx)
    }
}

impl SessionControl for Tap {
    fn command(&mut self, cmd: SessionCommand) -> SessionResult {
        self.inner.command(cmd)
    }
    fn poll_event(&mut self) -> Option<CryptoEvent> {
        self.inner.poll_event()
    }
}

fn tap(me: u8, ledger: &Arc<Mutex<Ledger>>) -> Tap {
    let ids = [ALICE, BOB].map(|n| *id(n).as_bytes());
    let dir = fixture_directory(ids).expect("fikstür dizini");
    // Çözme sınırı geniş tutulur (yalnız okuma): her geçerli kayıt çözülür.
    let limits = AmpLimits::from_capabilities(&TransportCapabilities {
        max_plaintext_unit_bytes: 65_540,
        preferred_plaintext_unit_bytes: 65_540,
        max_early_unit_bytes: 65_540,
        reliable: true,
    })
    .expect("geniş AMP limiti");
    Tap {
        inner: fixture_protection(*id(me).as_bytes(), dir, EnvelopeKind::Legacy).0,
        ledger: Arc::clone(ledger),
        limits,
    }
}

// ── Röle ─────────────────────────────────────────────────────────────────────────────────────

/// Datagramın röledeki yolu. `A*` Alice'in oturum ucu (Alice istemci → Bob sunucu), `B*` Bob'un
/// oturum ucu (Bob istemci → Alice sunucu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Alice istemci → Bob sunucu (Alice'in verisi; ileri hat).
    AUp,
    /// Bob sunucu → Alice istemci (Bob'un ACK'leri; dönüş hattı).
    ADown,
    /// Bob istemci → Alice sunucu (Bob'un verisi, alındılar; dönüş hattı).
    BUp,
    /// Alice sunucu → Bob istemci (Alice'in ACK'leri; ileri hat).
    BDown,
}

/// Rölenin gördüğü bir datagram (iz; ölçümden sonra yazılır, sıcak yolda biçimlendirme yok).
#[derive(Debug, Clone, Copy)]
struct LineRec {
    offered: Instant,
    route: Route,
    /// Tel boyu (UDP/IP dahil).
    wire: u64,
    /// 0 iletildi · 1 kuyruk taştı · 2 rastgele kayıp.
    fate: u8,
    arrives: Option<Instant>,
    fp: u64,
}

/// Bir yönün hattı: `shaped_relay.rs` `Shaper::offer` kuralı (jitter'sız).
struct Shaper {
    rate_bps: u64,
    /// [KAPI] Hız çizelgesi: (t0'dan bu yana, B/s); boşsa `rate_bps` sabit. Paket serileşmeye
    /// başladığı andaki hızla serileşir.
    rate_plan: Vec<(Duration, u64)>,
    /// [KAPI] Tek yön gecikme çizelgesi: (t0'dan bu yana, gecikme); boşsa `delay` sabit. Varışlar
    /// FIFO kalır (gecikme azalınca paket öncülünü geçmez).
    delay_plan: Vec<(Duration, Duration)>,
    /// [KAPI] Çizelgelerin saat tabanı (t0; ana iş parçacığı yazar). Ayarlanmadan önce ilk değer.
    t0: Arc<Mutex<Option<Instant>>>,
    last_arrival: Option<Instant>,
    limit: usize,
    delay: Duration,
    loss_permille: u64,
    lcg: u64,
    busy: VecDeque<Instant>,
    wire: VecDeque<(Instant, Route, Vec<u8>)>,
    sent: u64,
    bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
    dropped_fp: Vec<u64>,
    /// Her sunulan datagramın izi (sığa [`DROP_CAP`]).
    trace: Vec<LineRec>,
}

impl Shaper {
    fn new(rate_bps: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            rate_plan: Vec::new(),
            delay_plan: Vec::new(),
            t0: Arc::new(Mutex::new(None)),
            last_arrival: None,
            limit: QUEUE_PKTS,
            delay: ONE_WAY,
            loss_permille,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            sent: 0,
            bytes: 0,
            dropped_queue: 0,
            dropped_random: 0,
            dropped_fp: Vec::new(),
            trace: Vec::new(),
        }
    }

    fn since_t0(&self, at: Instant) -> Option<Duration> {
        lock(&self.t0).map(|t| at.saturating_duration_since(t))
    }

    fn rate_at(&self, at: Instant) -> u64 {
        let el = self.since_t0(at).unwrap_or(Duration::ZERO);
        self.rate_plan
            .iter()
            .rev()
            .find(|(from, _)| *from <= el)
            .map_or(self.rate_bps, |p| p.1)
    }

    fn delay_at(&self, at: Instant) -> Duration {
        let el = self.since_t0(at).unwrap_or(Duration::ZERO);
        self.delay_plan
            .iter()
            .rev()
            .find(|(from, _)| *from <= el)
            .map_or(self.delay, |p| p.1)
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    fn note_drop(&mut self, pkt: &[u8]) {
        if self.dropped_fp.len() < DROP_CAP {
            self.dropped_fp.push(fingerprint(pkt));
        }
    }

    fn note(
        &mut self,
        offered: Instant,
        route: Route,
        wire: u64,
        fate: u8,
        arrives: Option<Instant>,
        pkt: &[u8],
    ) {
        if self.trace.len() < DROP_CAP {
            self.trace.push(LineRec {
                offered,
                route,
                wire,
                fate,
                arrives,
                fp: fingerprint(pkt),
            });
        }
    }

    fn offer(&mut self, now: Instant, route: Route, pkt: Vec<u8>) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        let size = pkt.len() as u64 + UDP_IP_OVERHEAD;
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            self.note_drop(&pkt);
            self.note(now, route, size, 1, None, &pkt);
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let rate = self.rate_at(start);
        let done = start + Duration::from_nanos(size * 1_000_000_000 / rate.max(1));
        self.busy.push_back(done);
        self.sent += 1;
        self.bytes += size;
        if self.loss_permille > 0 && self.rand_permille() < self.loss_permille {
            self.dropped_random += 1;
            self.note_drop(&pkt);
            self.note(now, route, size, 2, None, &pkt);
            return;
        }
        // FIFO serileştirme; gecikme çizelgesi varsa varış öncülün varışından önce olamaz.
        let arrive = (done + self.delay_at(done)).max(self.last_arrival.unwrap_or(done));
        self.last_arrival = Some(arrive);
        self.note(now, route, size, 0, Some(arrive), &pkt);
        self.wire.push_back((arrive, route, pkt));
    }

    fn take_due(&mut self, now: Instant) -> Vec<(Route, Vec<u8>)> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|(t, _, _)| *t <= now) {
            let (_, r, p) = self.wire.pop_front().expect("var");
            out.push((r, p));
        }
        out
    }
}

struct RelaySockets {
    front_a: UdpSocket,
    back_a: UdpSocket,
    front_b: UdpSocket,
    back_b: UdpSocket,
}

fn relay(s: RelaySockets, mut fwd: Shaper, mut rev: Shaper, stop: &AtomicBool) -> (Shaper, Shaper) {
    for sock in [&s.front_a, &s.back_a, &s.front_b, &s.back_b] {
        sock.set_nonblocking(true).expect("nonblocking");
    }
    let mut alice_client: Option<SocketAddr> = None;
    let mut bob_client: Option<SocketAddr> = None;
    let mut buf = vec![0u8; 2048];
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        while let Ok((n, from)) = s.front_a.recv_from(&mut buf) {
            alice_client = Some(from);
            fwd.offer(now, Route::AUp, buf[..n].to_vec());
        }
        while let Ok(n) = s.back_a.recv(&mut buf) {
            rev.offer(now, Route::ADown, buf[..n].to_vec());
        }
        while let Ok((n, from)) = s.front_b.recv_from(&mut buf) {
            bob_client = Some(from);
            rev.offer(now, Route::BUp, buf[..n].to_vec());
        }
        while let Ok(n) = s.back_b.recv(&mut buf) {
            fwd.offer(now, Route::BDown, buf[..n].to_vec());
        }
        let now = Instant::now();
        for (route, p) in fwd.take_due(now).into_iter().chain(rev.take_due(now)) {
            let _ = match route {
                Route::AUp => s.back_a.send(&p),
                Route::BUp => s.back_b.send(&p),
                Route::ADown => alice_client.map_or(Ok(0), |to| s.front_a.send_to(&p, to)),
                Route::BDown => bob_client.map_or(Ok(0), |to| s.front_b.send_to(&p, to)),
            };
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    (fwd, rev)
}

// ── Uç: Transport + uygulama motorları (düğüm turu sırası) ───────────────────────────────────

type Transport = ClientTransport<Tap>;

enum Ev {
    Msg(MessengerEvent),
    Ptt(PttEvent),
    File(FileEvent),
    /// Zincirde görülen `FILE_CHUNK` kaydı (dosya motoruna verilmeden önce sayılır).
    Chunk,
}

struct Side {
    m: Messenger<Transport>,
    ptt: Ptt,
    files: FileTransfer,
}

impl Side {
    fn new(transport: Transport, clock: &Arc<dyn MonotonicClock>) -> Self {
        Self {
            m: Messenger::with_store(transport, Arc::new(MemoryAppStore::new()))
                .expect("AMP limitleri"),
            ptt: Ptt::new(Arc::clone(clock)),
            files: FileTransfer::new(Arc::clone(clock)),
        }
    }

    fn drain_files(&mut self, at: Instant, out: &mut Vec<(Instant, Ev)>) {
        while let Some(e) = self.files.poll_event() {
            out.push((at, Ev::File(e)));
        }
    }

    /// `kama-node` düğüm turu: `Ptt::poll` → `FileTransfer::handle` → olay; kuyruk boşaldıysa
    /// `FileTransfer::pump`; ardından dosya olayları.
    fn turn(&mut self, out: &mut Vec<(Instant, Ev)>) {
        let mut budget = EVENTS_PER_TURN;
        let drained = loop {
            if budget == 0 {
                break false;
            }
            budget -= 1;
            let Some(event) = self.ptt.poll(&mut self.m) else {
                break true;
            };
            let at = Instant::now();
            match event {
                PttEvent::Messenger(e) => {
                    if matches!(
                        &e,
                        MessengerEvent::Other {
                            message: AppMessage::FileChunk { .. },
                            ..
                        }
                    ) {
                        out.push((at, Ev::Chunk));
                    }
                    if let Some(e) = self.files.handle(&mut self.m, e) {
                        out.push((at, Ev::Msg(e)));
                    }
                    self.drain_files(at, out);
                }
                other => out.push((at, Ev::Ptt(other))),
            }
        };
        if drained {
            self.files.pump(&mut self.m);
        }
        self.drain_files(Instant::now(), out);
    }
}

fn transport(
    me: u8,
    ledger: &Arc<Mutex<Ledger>>,
    hub: &MemoryHub,
    link: Option<LinkRates>,
    emit: Option<EmitTraceSink>,
) -> Transport {
    let mut cfg = ClientTransportConfig::product(SocketAddr::from(([127, 0, 0, 1], 0)));
    cfg.runtime.drain_cap = Duration::from_millis(100);
    cfg.runtime.link_rates = link;
    cfg.runtime.diag_emit_sink = emit;
    // [YENİDEN-RAPOR] Ürün varsayılanı kapalı (spec §2.1); `KAMA_ASYM_ACK_REREPORT=on` ölçüm kolu.
    cfg.runtime.ack_rereport = std::env::var("KAMA_ASYM_ACK_REREPORT").is_ok_and(|v| v == "on");
    // [ACK-BITMAP · ölçüm prototipi] `KAMA_ASYM_ACK_FORMAT=bitmap` iki uca da 0x15 biçimini verir
    // (yetenek anlaşması yok; iki uç açıkça). [PTT-BANT · #175 dış inceleme] Fikstür varsayılanı
    // `bitmap`: birleşik adayın ölçülen yolu (pair_rate, yalın PTT 0x16 / 0x09) yalnız onunla açılır;
    // `v2` kolu bugünkü ürün varsayılanını (0x14) ölçer.
    cfg.runtime.ack_format = fixture_ack_format();
    // [PTT-BİRLEŞİK · deney] İki uçta da (CAPS biti / 0x17 ACK'i).
    cfg.runtime.ptt_coalesce = ptt_shape().coalesce != Coalesce::Off;
    // [ACK-PAYI TEŞHİS KOLU] `KAMA_ASYM_RETURN_Y_PERMILLE` (yalnız teşhis; varsayılan ürün Y = 250).
    if let Ok(y) = std::env::var("KAMA_ASYM_RETURN_Y_PERMILLE") {
        cfg.runtime.diag_return_budget_permille = y.parse().ok();
    }
    let mut t =
        ClientTransport::new(cfg, tap(me, ledger), Box::new(hub.link(id(me)))).expect("transport");
    // [FEC-TEŞHİS] Canlı ayna (salt gözlem): FEC latch, PARAM, PTO ve kopya sayaçları.
    t.set_live_diag_for_test(true);
    t
}

// ── Koşu ─────────────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Texts {
    sent: Option<(MsgId, Instant)>,
    received: Option<Instant>,
    delivered: Option<Instant>,
}

struct Bundle {
    planned: Instant,
    /// [PTT-BANT · sahip kararı 2026-11-22] Sesin oluştuğu an: demetin ilk 20 ms çerçevesinin
    /// başı (planlanan an − demet süresi). Gecikme buradan ölçülür.
    created: Instant,
    /// [PTT-BANT · gerçek zaman] Demetin taşıdığı 20 ms çerçeve sayısı (son demet kısmi olabilir).
    frames: u32,
    accepted: Instant,
    received: Option<Instant>,
}

struct Run {
    seed: u64,
    profile: bool,
    cell: Cell,
    t0: Instant,
    texts: Vec<Texts>,
    ptt_open: Option<(StreamId, Instant)>,
    ptt_started: Option<Instant>,
    /// [PTT-BİRLEŞİK · deney] Bob'da ilk `PttEnded`; Bob'un PTT sayaçları; Alice'in Bob'u yetenekli
    /// bildiği an.
    ptt_ended: Option<Instant>,
    bob_ptt: PttStats,
    alice_peer_capable: Option<Instant>,
    bundles: BTreeMap<u32, Bundle>,
    file_received: Option<Instant>,
    file_delivered: Option<Instant>,
    file_ok: bool,
    /// [KAPI] İkinci dosya (hücre isterse): gönderim ve Bob'da tamamlanma, bayt doğruluğu.
    file2: Option<(Instant, Option<Instant>, bool)>,
    /// [KAPI] Uygulamanın bildirdiği dosya hataları (gönderim / alım).
    file_failures: Vec<String>,
    /// [FİKSTÜR] Planlanmış ama uygulamaca kabul edilmemiş demetlerin planlanan anları.
    unaccepted: Vec<(Instant, u32)>,
    /// [PTT-BANT] Basış süresi (basış başından bırakışa; bant paydası).
    press_len: Option<Duration>,
    /// [CANLILIK-TEŞHİS] Şerit açıklık sınırına takılan pompa turları (şerit sırasıyla).
    lane_span_blocked: [u64; 4],
    chunk_count: u64,
    /// Bob'un uygulama zincirinde görülen her `FILE_CHUNK` kaydının anı.
    chunk_rx: Vec<Instant>,
    alice: Ledger,
    bob: Ledger,
    fwd: Shaper,
    rev: Shaper,
    file_backlog_held: u64,
    /// [FEC-TEŞHİS] Koşu sonunda iki ucun oturum aynası (Alice → Bob, Bob → Alice).
    alice_live: Option<ClientLiveDiag>,
    bob_live: Option<ClientLiveDiag>,
    /// [ONARIM-SIRASI · salt gözlem] Ucun alıcısının FEC ile çözdüğü blok (Alice'in, Bob'un).
    /// Karşıya ulaşan RECOVERED bildirimi bundan ayrıdır (`Ledger::recovered_rx`).
    alice_decoded: u64,
    bob_decoded: u64,
    /// [İZ-NEDEN] Alice göndericisinin emisyon / kayıp ilanı izi (an, kayıt).
    emit: Vec<(Instant, EmitDiag)>,
    /// [İZ-ARALIK] Alice göndericisinin hat koruması ACK kaydı (an, kayıt; canlı aynadan).
    guard_trace: Vec<(Instant, LineGuardAckTrace)>,
}

/// [İZ-ARALIK] Canlı aynadan yeni (sırası son toplanandan büyük) hat koruması kayıtlarını toplar;
/// an = ucun saat tabanı + motor anı. Salt gözlem.
fn collect_guard_trace(live: Option<ClientLiveDiag>, out: &mut Vec<(Instant, LineGuardAckTrace)>) {
    let Some((live, origin)) = live.and_then(|l| l.clock_origin.map(|o| (l, o))) else {
        return;
    };
    let last = out.last().map_or(0, |(_, t)| t.index);
    for t in live.line_guard.recent().filter(|t| t.index > last) {
        if out.len() >= GUARD_TRACE_CAP {
            break;
        }
        out.push((origin + Duration::from_nanos(t.at_ns), *t));
    }
}

fn run(seed: u64, profile: bool, cell: Cell) -> Run {
    let hub = MemoryHub::new();
    let alice_ledger = Arc::new(Mutex::new(Ledger::default()));
    let bob_ledger = Arc::new(Mutex::new(Ledger::default()));
    let link = |up, down| {
        profile.then_some(LinkRates {
            uplink_bps: up,
            downlink_bps: down,
        })
    };
    let emit: EmitTraceSink = Arc::new(Mutex::new(Vec::new()));
    let mut alice_t = transport(
        ALICE,
        &alice_ledger,
        &hub,
        link(FWD_BPS, REV_BPS),
        Some(Arc::clone(&emit)),
    );
    let mut bob_t = transport(BOB, &bob_ledger, &hub, link(REV_BPS, FWD_BPS), None);
    let bind = || UdpSocket::bind("127.0.0.1:0").expect("röle soketi");
    let sockets = RelaySockets {
        front_a: bind(),
        back_a: bind(),
        front_b: bind(),
        back_b: bind(),
    };
    sockets
        .back_a
        .connect(bob_t.data_addr())
        .expect("röle → Bob");
    sockets
        .back_b
        .connect(alice_t.data_addr())
        .expect("röle → Alice");
    alice_t.add_peer_addr(id(BOB), sockets.front_a.local_addr().expect("adres"));
    bob_t.add_peer_addr(id(ALICE), sockets.front_b.local_addr().expect("adres"));
    let stop = Arc::new(AtomicBool::new(false));
    let relay_stop = Arc::clone(&stop);
    let shaper_t0: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let mut fwd = Shaper::new(cell.fwd_plan[0].1, FWD_LOSS_PERMILLE, seed);
    fwd.rate_plan = cell.fwd_plan.clone();
    fwd.t0 = Arc::clone(&shaper_t0);
    let mut rev = Shaper::new(REV_BPS, 0, seed ^ 0x5A5A);
    rev.delay_plan = cell.rev_delay_plan.clone();
    rev.t0 = Arc::clone(&shaper_t0);
    let relay_thread = std::thread::spawn(move || relay(sockets, fwd, rev, &relay_stop));

    let clock: Arc<dyn MonotonicClock> = Arc::new(SystemClock::new());
    let mut alice = Side::new(alice_t, &clock);
    let mut bob = Side::new(bob_t, &clock);
    let mut events: Vec<(Instant, Ev)> = Vec::new();

    // Oturum: Alice bağlanır, iki uç da Messenger'da hazır oturumu benimser.
    match alice.m.connect(id(BOB)).expect("connect") {
        ConnectResult::Pending(_) | ConnectResult::Ready(_) => {}
    }
    let setup_deadline = Instant::now() + RUN_CAP;
    while alice.m.session_of(id(BOB)).is_none() || bob.m.session_of(id(ALICE)).is_none() {
        hub.relay();
        alice.turn(&mut events);
        bob.turn(&mut events);
        events.clear();
        assert!(Instant::now() < setup_deadline, "oturum kurulmadı");
        std::thread::sleep(Duration::from_millis(1));
    }

    let file: Vec<u8> = (0..FILE_BYTES).map(|i| (i % 251) as u8).collect();
    let chunk_len = alice.m.limits().max_file_chunk_bytes() as u64;
    let chunk_count = (FILE_BYTES as u64).div_ceil(chunk_len);
    let t0 = Instant::now();
    *lock(&shaper_t0) = Some(t0);
    alice
        .files
        .send_file(&alice.m, id(BOB), "olcum.bin", file.clone())
        .expect("dosya kuyruğa alınır");
    let file2_bytes: Vec<u8> = (0..FILE_BYTES).map(|i| (i % 241) as u8).collect();
    let mut file2: Option<(Instant, Option<Instant>, bool)> = None;

    let mut texts: Vec<Texts> = (0..2 + cell.texts_at.len())
        .map(|_| Texts::default())
        .collect();
    let shape = ptt_shape();
    alice.ptt.set_compact_bundles(shape.compact);
    // [PTT-BİRLEŞİK · deney] İki uç da birleşik demetleri çözer; Alice yetenekli eşe birleşik açar.
    let coalesce_on = shape.coalesce != Coalesce::Off;
    alice.ptt.set_spurt_coalesce(coalesce_on);
    bob.ptt.set_spurt_coalesce(coalesce_on);
    alice.ptt.set_spurt_open_repeat(shape.open_k);
    let mut peer_capable_at: Option<Instant> = None;
    if shape.coalesce == Coalesce::Known {
        assert!(alice.ptt.set_peer_spurt_capable(id(BOB), true));
        peer_capable_at = Some(t0);
    }
    let mut ptt_ended: Option<Instant> = None;
    let mut ptt_open: Option<(StreamId, Instant)> = None;
    let mut ptt_closed = false;
    let mut ptt_release: Option<Instant> = None;
    let mut ptt_started: Option<Instant> = None;
    let mut bundles: BTreeMap<u32, Bundle> = BTreeMap::new();
    let mut next_bundle: u32 = 0;
    let mut bob_chunks: u64 = 0;
    let mut chunk_rx: Vec<Instant> = Vec::new();
    let mut file_received: Option<Instant> = None;
    let mut file_delivered: Option<Instant> = None;
    let mut file_ok = false;
    let mut file_failures: Vec<String> = Vec::new();
    let mut guard_trace: Vec<(Instant, LineGuardAckTrace)> = Vec::new();
    let mut guard_trace_poll = Instant::now();

    loop {
        hub.relay();
        let now = Instant::now();
        let el = now - t0;
        if texts[0].sent.is_none() && el >= TEXT1_AT {
            let msg_id = alice.m.send_text(id(BOB), TEXT1).expect("metin 1");
            texts[0].sent = Some((msg_id, now));
        }
        if texts[1].sent.is_none() && bob_chunks * 2 >= chunk_count {
            let msg_id = alice.m.send_text(id(BOB), TEXT2).expect("metin 2");
            texts[1].sent = Some((msg_id, Instant::now()));
        }
        for (k, at) in cell.texts_at.iter().enumerate() {
            if texts[2 + k].sent.is_none() && el >= *at {
                let msg_id = alice
                    .m
                    .send_text(id(BOB), "metin: evre denetimi")
                    .expect("evre metni");
                texts[2 + k].sent = Some((msg_id, Instant::now()));
            }
        }
        if let (None, Some(at)) = (file2, cell.file2_at) {
            if el >= at {
                alice
                    .files
                    .send_file(&alice.m, id(BOB), "olcum2.bin", file2_bytes.clone())
                    .expect("ikinci dosya kuyruğa alınır");
                file2 = Some((Instant::now(), None, false));
            }
        }
        let files_done = file_received.is_some()
            && (cell.file2_at.is_none() || file2.is_some_and(|f| f.1.is_some()));
        // [PTT-BİRLEŞİK · deney · neg] Bob'un 0x17'si (bant içi yetenek) uygulamaya verilir.
        if shape.coalesce == Coalesce::Negotiated && peer_capable_at.is_none() {
            let capable = alice
                .m
                .session_of(id(BOB))
                .is_some_and(|h| alice.m.transport().peer_ptt_coalesce(h));
            if capable {
                assert!(alice.ptt.set_peer_spurt_capable(id(BOB), true));
                peer_capable_at = Some(now);
            }
        }
        if ptt_open.is_none() && el >= ptt_at() {
            let stream = alice
                .ptt
                .open(&mut alice.m, id(BOB), PttCodec::Lyra, shape.code)
                .expect("PTT açılır");
            ptt_open = Some((stream, now));
        }
        if let (Some((stream, opened)), false) = (ptt_open, ptt_closed) {
            // [PTT-BANT · gerçek zaman · sahip 2026-11-22] Bırakış anı: verilen basış süresi
            // (demet süresinden bağımsız) ya da hücrenin kuralı.
            let release = match (shape.press, cell.ptt_until) {
                (Some(p), _) => (now >= opened + p).then_some(opened + p),
                (None, Some(until)) => (el >= until).then_some(t0 + until),
                (None, None) => files_done.then_some(now),
            };
            // Çerçeveler basış başından gerçek zamanda oluşur; demet çerçeveleri var olunca
            // sunulur (ilk demet basış + demet süresi), bırakışta kısmi son demet.
            let fpb = u32::try_from(shape.period.as_nanos() / LYRA_FRAME.as_nanos())
                .unwrap_or(u32::MAX)
                .max(1);
            while let Some((planned, frames)) =
                bundle_slot(opened, shape.period, next_bundle, release)
            {
                if planned > Instant::now() {
                    break;
                }
                let created = opened + shape.period * next_bundle;
                let len = (shape.bytes * frames as usize / fpb as usize).max(4);
                let mut payload = vec![0x5Au8; len];
                payload[..4].copy_from_slice(&next_bundle.to_be_bytes());
                // [PTT-BİRLEŞİK · deney] Bırakışta son demet `END`'i taşır (birleşik akışta; diğer
                // akışta sıradan demet).
                let last = release.is_some()
                    && bundle_slot(opened, shape.period, next_bundle + 1, release).is_none();
                let sent = if last {
                    alice
                        .ptt
                        .send_last_bundle_from(&mut alice.m, stream, &payload, created)
                } else {
                    alice
                        .ptt
                        .send_bundle_from(&mut alice.m, stream, &payload, created)
                };
                match sent {
                    Ok(()) => {
                        bundles.insert(
                            next_bundle,
                            Bundle {
                                planned,
                                created,
                                frames,
                                accepted: Instant::now(),
                                received: None,
                            },
                        );
                        next_bundle += 1;
                    }
                    // Geri basınç: konuşan bekler, aynı demet sonraki turda yeniden verilir.
                    Err(PttError::Transport(TransportError::Backpressure)) => break,
                    Err(e) => panic!("PTT demeti: {e:?}"),
                }
            }
            if let Some(r) = release {
                alice.ptt.close(&mut alice.m, stream).expect("PTT kapanır");
                ptt_closed = true;
                ptt_release = Some(r);
            }
        }

        alice.turn(&mut events);
        bob.turn(&mut events);
        if Instant::now() >= guard_trace_poll {
            guard_trace_poll = Instant::now() + GUARD_TRACE_POLL;
            collect_guard_trace(
                alice
                    .m
                    .transport()
                    .session_live_diags()
                    .first()
                    .map(|d| d.1),
                &mut guard_trace,
            );
        }
        for (at, ev) in events.drain(..) {
            match ev {
                Ev::Chunk => {
                    bob_chunks += 1;
                    chunk_rx.push(at);
                }
                Ev::Msg(MessengerEvent::TextReceived { msg_id, .. }) => {
                    for t in &mut texts {
                        if t.sent.is_some_and(|(m, _)| m == msg_id) {
                            t.received.get_or_insert(at);
                        }
                    }
                }
                Ev::Msg(MessengerEvent::Delivered { msg_id, .. }) => {
                    for t in &mut texts {
                        if t.sent.is_some_and(|(m, _)| m == msg_id) {
                            t.delivered.get_or_insert(at);
                        }
                    }
                }
                Ev::Ptt(PttEvent::PttStarted { .. }) => {
                    ptt_started.get_or_insert(at);
                }
                Ev::Ptt(PttEvent::PttEnded { .. }) => {
                    ptt_ended.get_or_insert(at);
                }
                Ev::Ptt(PttEvent::PttAudio { payload, .. }) if payload.len() >= 4 => {
                    let seq = u32::from_be_bytes(payload[..4].try_into().expect("4 B"));
                    if let Some(b) = bundles.get_mut(&seq) {
                        b.received.get_or_insert(at);
                    }
                }
                Ev::File(FileEvent::Received { bytes, name, .. }) if name == "olcum2.bin" => {
                    if let Some(f) = file2.as_mut() {
                        f.1.get_or_insert(at);
                        f.2 = bytes == file2_bytes;
                    }
                }
                Ev::File(FileEvent::Received { bytes, .. }) => {
                    file_ok = bytes == file;
                    file_received.get_or_insert(at);
                }
                Ev::File(FileEvent::Delivered { .. }) => {
                    file_delivered.get_or_insert(at);
                }
                // [KAPI] Uygulama dosyadan vazgeçtiyse koşu sürer ve ölçütler yazılır (dosya
                // ölçütü düşer); eskiden panik ölçüt satırlarını bastırıyordu.
                Ev::File(FileEvent::SendFailed { reason, .. }) => {
                    file_failures.push(format!("gönderim: {reason:?}"));
                }
                Ev::File(FileEvent::Failed { reason, .. }) => {
                    file_failures.push(format!("alım: {reason:?}"));
                }
                _ => {}
            }
        }
        let done = file_received.is_some()
            && (cell.file2_at.is_none() || file2.is_some_and(|f| f.1.is_some()))
            && file_delivered.is_some()
            && ptt_closed
            && texts.iter().all(|t| t.delivered.is_some())
            && bundles.values().all(|b| b.received.is_some());
        if done || t0.elapsed() >= cell.run_cap || file_failures.len() >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_micros(500));
    }

    // [FİKSTÜR] Planlanmış ama uygulamaca kabul edilmemiş demetler (geri basınç; kapanışa ya da
    // koşu sonuna dek): planlandıkları evrede KALDI sayılır.
    let mut unaccepted: Vec<(Instant, u32)> = Vec::new();
    let mut press_len: Option<Duration> = None;
    if let Some((_, opened)) = ptt_open {
        let end = ptt_release.unwrap_or_else(Instant::now);
        press_len = Some(end.saturating_duration_since(opened));
        let mut k = next_bundle;
        while let Some(slot) = bundle_slot(opened, shape.period, k, Some(end)) {
            unaccepted.push(slot);
            k += 1;
        }
    }
    let lane_span_blocked = alice
        .m
        .transport()
        .lane_stats()
        .map_or([0; 4], |s| s.span_blocked);
    let file_backlog_held = alice
        .m
        .transport()
        .lane_stats()
        .map_or(0, |s| s.file_backlog_held);
    let alice_live = alice
        .m
        .transport()
        .session_live_diags()
        .first()
        .map(|d| d.1);
    let bob_live = bob.m.transport().session_live_diags().first().map(|d| d.1);
    let alice_decoded = alice.m.transport().fec_blocks_decoded();
    let bob_decoded = bob.m.transport().fec_blocks_decoded();
    let bob_ptt = bob.ptt.stats();
    let alice_peer_capable = peer_capable_at;
    collect_guard_trace(alice_live, &mut guard_trace);
    drop(alice);
    drop(bob);
    stop.store(true, Ordering::Relaxed);
    let (fwd, rev) = relay_thread.join().expect("röle");
    let alice_ledger = std::mem::take(&mut *lock(&alice_ledger));
    let bob_ledger = std::mem::take(&mut *lock(&bob_ledger));
    let emit = std::mem::take(&mut *lock(&emit));
    Run {
        seed,
        profile,
        cell,
        t0,
        texts,
        ptt_open,
        ptt_started,
        ptt_ended,
        bob_ptt,
        alice_peer_capable,
        bundles,
        file_received,
        file_delivered,
        file_ok,
        file2,
        file_failures,
        unaccepted,
        press_len,
        lane_span_blocked,
        chunk_count,
        chunk_rx,
        alice: alice_ledger,
        bob: bob_ledger,
        fwd,
        rev,
        file_backlog_held,
        alice_live,
        bob_live,
        alice_decoded,
        bob_decoded,
        emit,
        guard_trace,
    }
}

// ── Değerlendirme ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    /// Birimin kendi ilk iletimi (ya da şeritte beklediği öncülü) kayboldu; ACK tabanlı kurtarma.
    LossRecovery,
    Fail,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "GEÇTİ",
            Self::LossRecovery => "kayıp-kurtarma",
            Self::Fail => "KALDI",
        }
    }
}

fn secs(d: Duration) -> String {
    format!("{:.2}", d.as_secs_f64())
}

fn opt_secs(d: Option<Duration>) -> String {
    d.map_or("YOK".to_string(), secs)
}

impl Run {
    fn lost_first(ledger: &Ledger, drops: &HashSet<u64>, item: Item) -> bool {
        ledger
            .first_tx
            .get(&item)
            .is_some_and(|(_, fp)| drops.contains(fp))
    }

    fn since(&self, at: Instant, base: Instant) -> Duration {
        let _ = self;
        at.saturating_duration_since(base)
    }

    /// [İLK-ACK · sahip kararı 2026-11-21 m. 1] İlk ACK'in gecikmesi üç ayrı süre olarak:
    ///
    /// * **ölçüm beklemesi:** boyu bilinen ilk varıştan bekletmenin sonuna. Alıcının ilk varış
    ///   penceresi kuralı (`arrival_rate::FirstArrivalWindow`, alıcının kullandığı kod) Bob'un
    ///   veri varış izi üzerinde yeniden oynatılır (gövde = tel − UDP/IP). Pencere kapanınca biter,
    ///   kapanmazsa süre tavanında;
    /// * **yerel kuyruk:** bekletmenin sonundan ACK'in dönüş hattına sunulmasına (dönüş ACK
    ///   bütçesi, hattı tutan veriye yol verme, ACK zamanlayıcısı);
    /// * **hat:** sunumdan Alice'e varışa (serileşme, hat kuyruğu, yayılım).
    ///
    /// İlk ACK, Alice'in işlediği ilk ACK çerçevesidir; dönüş izindeki kaydı varış anıyla eşlenir.
    fn report_first_ack(&self, tag: &str) {
        use adaptive_transport::arrival_rate::{FirstArrivalWindow, FIRST_ACK_HOLD_MAX_NS};
        let cap = Duration::from_nanos(FIRST_ACK_HOLD_MAX_NS);
        let ns = |x: Instant| u64::try_from(self.since(x, self.t0).as_nanos()).unwrap_or(u64::MAX);
        let mut win = FirstArrivalWindow::default();
        let mut first_seen: Option<Instant> = None;
        let mut hold_end: Option<(Instant, &str)> = None;
        for (seq, r) in self.fwd.trace.iter().enumerate() {
            let data = matches!(r.route, Route::AUp) && self.alice.by_fp.contains_key(&r.fp);
            let Some(at) = r.arrives.filter(|_| data && r.fate == 0) else {
                continue;
            };
            if let Some(seen) = first_seen {
                if at >= seen + cap {
                    hold_end = Some((seen + cap, "tavan"));
                    break;
                }
            }
            win.on_arrival(seq as u64, r.wire.saturating_sub(UDP_IP_OVERHEAD), ns(at));
            first_seen.get_or_insert(at);
            if !win.holds_first_ack(ns(at)) {
                let kind = if win.rate_field() == adaptive_transport::ack::ARRIVAL_RATE_SENTINEL {
                    "geçersiz"
                } else {
                    "pencere"
                };
                hold_end = Some((at, kind));
                break;
            }
        }
        if hold_end.is_none() {
            hold_end = first_seen.map(|s| (s + cap, "tavan"));
        }
        let first_ack = self.alice.acks_rx.first().map(|a| a.0);
        // Dönüş izinde Alice'e varışı ilk ACK'in işlenmesine en yakın (ondan sonra olmayan) kayıt.
        let line = first_ack.and_then(|fa| {
            self.rev
                .trace
                .iter()
                .filter(|r| matches!(r.route, Route::ADown))
                .filter_map(|r| r.arrives.filter(|&a| a <= fa).map(|a| (r.offered, a)))
                .max_by_key(|&(_, a)| a)
        });
        let t = |i: Instant| secs(self.since(i, self.t0));
        let d = |a: Instant, b: Instant| secs(b.saturating_duration_since(a));
        match (first_seen, hold_end, line) {
            (Some(seen), Some((end, kind)), Some((offered, arrived))) => println!(
                "İZ-İLK-ACK {tag} ilk_boylu_varış=+{} ölçüm_beklemesi={} ({kind}, alan={}) \
                 yerel_kuyruk={} hat={} Alice_varış=+{}",
                t(seen),
                d(seen, end),
                win.rate_field(),
                d(end, offered),
                d(offered, arrived),
                t(arrived)
            ),
            _ => println!(
                "İZ-İLK-ACK {tag} eksik: ilk_varış={:?} bekletme_sonu={:?} ilk_ACK={:?}",
                first_seen.map(t),
                hold_end.map(|(e, k)| (t(e), k)),
                first_ack.map(t)
            ),
        }
    }

    /// Yazdırır ve düşen ölçütlerin adlarını döndürür.
    fn report(&self) -> Vec<String> {
        let fwd_drops: HashSet<u64> = self.fwd.dropped_fp.iter().copied().collect();
        let rev_drops: HashSet<u64> = self.rev.dropped_fp.iter().copied().collect();
        let mut failed: Vec<String> = Vec::new();
        let mut line: Vec<String> = Vec::new();
        let tag = format!(
            "hücre={} tohum={} profil={}",
            self.cell.name,
            self.seed,
            if self.profile { "açık" } else { "kapalı" }
        );
        println!(
            "HÜCRE {tag} ileri_gerçek={:?} ileri_beyan={} dönüş_gerçek={} dönüş_beyan={} dönüş_gecikme={:?} dosya_sınırı={} s dosya2={:?} alındı_sınırı={} s evreler={:?}",
            self.cell
                .fwd_plan
                .iter()
                .map(|(t, r)| format!("{}s:{r}", t.as_secs()))
                .collect::<Vec<_>>(),
            FWD_BPS,
            REV_BPS,
            REV_BPS,
            self.cell
                .rev_delay_plan
                .iter()
                .map(|(t, d)| format!("{}s:{}ms", t.as_secs(), d.as_millis()))
                .collect::<Vec<_>>(),
            secs(self.cell.file_bound),
            self.cell.file2_at.map(|a| format!("+{} s, sınır {} s", secs(a), secs(FILE_BOUND))),
            secs(self.cell.receipt_bound),
            self.cell.phases,
        );

        // (1) Metin + alındı.
        let mut text_verdicts = Vec::new();
        let mut text_phase: BTreeMap<usize, (Duration, Verdict)> = BTreeMap::new();
        let mut receipt_verdicts = Vec::new();
        for (i, t) in self.texts.iter().enumerate() {
            let n = i + 1;
            let Some((msg_id, sent)) = t.sent else {
                println!("ÖLÇÜT {tag} metin{n}: gönderilmedi → KALDI");
                text_verdicts.push(Verdict::Fail);
                receipt_verdicts.push(Verdict::Fail);
                continue;
            };
            let item = Item::Text(msg_id);
            let lost = Self::lost_first(&self.alice, &fwd_drops, item);
            let delay = t.received.map(|r| self.since(r, sent));
            let first_tx = self
                .alice
                .first_tx
                .get(&item)
                .map(|(a, _)| self.since(*a, sent));
            let arrival = self.bob.first_rx.get(&item).map(|a| self.since(*a, sent));
            let v = match delay {
                None => Verdict::Fail,
                Some(d) if d <= TEXT_BOUND => Verdict::Pass,
                Some(_) if lost => Verdict::LossRecovery,
                Some(_) => Verdict::Fail,
            };
            println!(
                "ÖLÇÜT {tag} metin{n}: gecikme={} s (sınır {} s) ilk_çıkış=+{} s ilk_varış=+{} s iletim={} ilk_iletim_düştü={lost} → {}",
                opt_secs(delay),
                secs(TEXT_BOUND),
                opt_secs(first_tx),
                opt_secs(arrival),
                self.alice.tx_count.get(&item).copied().unwrap_or(0),
                v.label()
            );
            text_verdicts.push(v);
            let phase = self.cell.phase_of(self.since(sent, self.t0));
            let e = text_phase
                .entry(phase)
                .or_insert((Duration::ZERO, Verdict::Pass));
            e.0 = e.0.max(delay.unwrap_or(RUN_CAP));
            if v == Verdict::Fail {
                e.1 = Verdict::Fail;
            }
            // Alındı: Bob'da metnin teslim anı → Alice'te `Delivered`.
            let r_item = Item::Receipt(msg_id);
            let r_lost = Self::lost_first(&self.bob, &rev_drops, r_item);
            let ret = match (t.received, t.delivered) {
                (Some(r), Some(d)) => Some(self.since(d, r)),
                _ => None,
            };
            let rtt = t.delivered.map(|d| self.since(d, sent));
            let r_first_tx = match (t.received, self.bob.first_tx.get(&r_item)) {
                (Some(r), Some((a, _))) => Some(self.since(*a, r)),
                _ => None,
            };
            let r_arrival = match (t.received, self.alice.first_rx.get(&r_item)) {
                (Some(r), Some(a)) => Some(self.since(*a, r)),
                _ => None,
            };
            let rv = match ret {
                None => Verdict::Fail,
                Some(d) if d <= self.cell.receipt_bound => Verdict::Pass,
                Some(_) if r_lost => Verdict::LossRecovery,
                Some(_) => Verdict::Fail,
            };
            println!(
                "ÖLÇÜT {tag} alındı{n}: dönüş={} s (sınır {} s) gidiş_dönüş={} s Bob_ilk_çıkış=+{} s Alice_ilk_varış=+{} s iletim={} ilk_iletim_düştü={r_lost} → {}",
                opt_secs(ret),
                secs(self.cell.receipt_bound),
                opt_secs(rtt),
                opt_secs(r_first_tx),
                opt_secs(r_arrival),
                self.bob.tx_count.get(&r_item).copied().unwrap_or(0),
                rv.label()
            );
            receipt_verdicts.push(rv);
            line.push(format!(
                "metin{n}={}({}) alındı{n}_dönüş={}({}) alındı{n}_gd={}",
                opt_secs(delay),
                v.label(),
                opt_secs(ret),
                rv.label(),
                opt_secs(rtt)
            ));
        }
        if text_verdicts.contains(&Verdict::Fail) {
            failed.push("METİN".to_string());
        }
        // [FİKSTÜR · 2026-11-21] Evre sınıflandırması: planlanmış metin yoksa niyetli evrede
        // "fikstür geçersiz" (kanıt yok, düşmüş sayılır), niyetsiz evrede "gözlem yok".
        for (ph, (name, _)) in self.cell.phases.iter().enumerate() {
            let intended = self.cell.phase_intent.get(ph).is_some_and(|i| i.1);
            let (label, fail) = match text_phase.get(&ph) {
                Some((worst, v)) => {
                    println!(
                        "ÖLÇÜT {tag} evre={name} metin_en_kötü={} s (sınır {} s) → {}",
                        secs(*worst),
                        secs(TEXT_BOUND),
                        v.label()
                    );
                    (None, *v == Verdict::Fail)
                }
                None if intended => (Some("fikstür geçersiz"), true),
                None => (Some("gözlem yok"), false),
            };
            if let Some(l) = label {
                println!("ÖLÇÜT {tag} evre={name} metin: planlanmış metin yok → {l}");
            }
            if fail {
                failed.push(if label.is_some() {
                    format!("FİKSTÜR/metin/{name}")
                } else {
                    format!("METİN/{name}")
                });
            }
        }
        if receipt_verdicts.contains(&Verdict::Fail) {
            failed.push("ALINDI".to_string());
        }

        // (2) PTT.
        let file_end = self.file_received;
        let (ptt_verdict, ptt_line) = self.ptt_report(&tag, &fwd_drops, file_end);
        if ptt_verdict == Verdict::Fail {
            failed.push("PTT".to_string());
        }
        // [KAPI] Evre başına en kötü PTT gecikmesi (planlanan an → Bob'da teslim; teslim edilmeyen
        // demet koşu tavanı sayılır). Evre, demetin planlandığı andır.
        let mut ptt_phase: BTreeMap<usize, (Duration, u32, u32, u32)> = BTreeMap::new();
        for b in self.bundles.values() {
            let ph = self.cell.phase_of(self.since(b.planned, self.t0));
            let e = ptt_phase.entry(ph).or_insert((Duration::ZERO, 0, 0, 0));
            e.2 += 1;
            match b.received {
                Some(r) => {
                    let d = self.since(r, b.created);
                    e.0 = e.0.max(d);
                    if d <= PTT_BOUND {
                        e.3 += 1;
                    }
                }
                None => {
                    e.0 = RUN_CAP;
                    e.1 += 1;
                }
            }
        }
        let mut unaccepted_phase: BTreeMap<usize, u32> = BTreeMap::new();
        for &(p, _) in &self.unaccepted {
            *unaccepted_phase
                .entry(self.cell.phase_of(self.since(p, self.t0)))
                .or_insert(0) += 1;
        }
        for (i, (name, _)) in self.cell.phases.iter().enumerate() {
            let (worst, missing, n, timely) =
                ptt_phase
                    .get(&i)
                    .copied()
                    .unwrap_or((Duration::ZERO, 0, 0, 0));
            let unacc = unaccepted_phase.get(&i).copied().unwrap_or(0);
            let intended = self.cell.phase_intent.get(i).is_some_and(|x| x.0);
            // [FİKSTÜR · 2026-11-21, sahip koşullu kabulü] Evrede planlanmış demet yoksa: PTT
            // niyetli evrede "fikstür geçersiz" (kanıt yok, düşmüş sayılır), niyetsiz evrede "gözlem
            // yok". Planlanmış ama kabul edilmemiş / teslim edilmemiş / sınırı aşmış demet KALDI.
            // [2026-11-20 tanımı: boş evre "gözlem yok"; bu tur niyetli evre için daraltıldı.]
            let (label, fail) = if n + unacc == 0 {
                if intended {
                    ("fikstür geçersiz", true)
                } else {
                    ("gözlem yok", false)
                }
            } else if ptt_timely_ok(u64::from(timely), u64::from(n + unacc)) {
                ("GEÇTİ", false)
            } else {
                ("KALDI", true)
            };
            println!(
                "ÖLÇÜT {tag} evre={name} ptt_en_kötü={} s (sınır {} s) demet={n} zamanında={timely} (en az binde {PTT_TIMELY_MIN_PERMILLE}) teslim_yok={missing} kabul_edilmeyen={unacc} → {label}",
                secs(worst),
                secs(PTT_BOUND),
            );
            if fail {
                failed.push(if n + unacc == 0 {
                    format!("FİKSTÜR/ptt/{name}")
                } else {
                    format!("PTT/{name}")
                });
            }
        }
        line.push(ptt_line);
        // [PTT-BANT · sahip kararı, #175 dış inceleme] Yön başına PTT bandı: PTT kaydı taşıyan
        // mühürlenmiş her datagram (kopya, yeniden iletim, yolda kaybolan dahil), IPv4/UDP dahil.
        // Bitmap kolunda yalın çerçeve (0x09) tele çıkmış olmalı: çıkmadıysa bant hedefi sınanmadı.
        let bitmap = fixture_ack_format() == AckFormat::Bitmap;
        let shape = ptt_shape();
        for (side, ledger) in [("alice", &self.alice), ("bob", &self.bob)] {
            let press = self.press_len.unwrap_or(Duration::ZERO);
            let wire: Vec<(Instant, u64)> = ledger.ptt_wire.iter().map(|w| (w.0, w.1)).collect();
            let Some(bps) = ptt_band_bps(&wire, press) else {
                continue;
            };
            let lean_ok = !bitmap || ledger.ptt_lean_tx > 0;
            let band_ok = bps <= PTT_BAND_MAX_BPS;
            println!(
                "ÖLÇÜT {tag} ptt_bant yön={side} bit_s={bps} (en çok {PTT_BAND_MAX_BPS}; payda basış {} s) datagram={} yalın_0x09={} yük={} B demet={} ms → {}",
                secs(press),
                ledger.ptt_wire.len(),
                ledger.ptt_lean_tx,
                shape.bytes,
                shape.period.as_millis(),
                if band_ok && lean_ok { "GEÇTİ" } else { "KALDI" },
            );
            // [PTT-BANT · 2026-11-22] Bant pencereleri: oturum başı (açılış → ilk yalın 0x09;
            // eş yalın PTT'yi kanıtlamadan önceki veri çerçeveli trafik) ve sürekli (ilk 0x09 →
            // konuşma sonu; konuşmadan sonra çıkan yeniden iletim paya girer). Datagram türleri.
            if let Some((_, opened)) = self.ptt_open {
                let press_end = opened + press;
                let first_lean = ledger.ptt_wire.iter().find(|w| w.2).map(|w| w.0);
                let split = first_lean.unwrap_or(press_end).min(press_end);
                let (pre, post): (Vec<_>, Vec<_>) =
                    ledger.ptt_wire.iter().partition(|w| w.0 < split);
                let bytes = |v: &[&PttWire]| v.iter().map(|w| w.1).sum::<u64>();
                let start_bps = window_bps(bytes(&pre), self.since(split, opened));
                let steady_bps = window_bps(bytes(&post), self.since(press_end, split));
                let count =
                    |f: &dyn Fn(&PttWire) -> bool| ledger.ptt_wire.iter().filter(|w| f(w)).count();
                let lean = count(&|w| w.2);
                let data = count(&|w| !w.2);
                let opens = count(&|w| matches!(w.3, Item::PttOpen(_)));
                let closes = count(&|w| matches!(w.3, Item::PttClose(_)));
                let bundle_tx = count(&|w| matches!(w.3, Item::Bundle(_)));
                let unique: HashSet<Item> = ledger
                    .ptt_wire
                    .iter()
                    .filter(|w| matches!(w.3, Item::Bundle(_)))
                    .map(|w| w.3)
                    .collect();
                let data_bundles = count(&|w| !w.2 && matches!(w.3, Item::Bundle(_)));
                println!(
                    "ÖLÇÜT {tag} ptt_bant_pencere yön={side} başlangıç=[0, +{} s) bit_s={} datagram={} sürekli=[+{} s, +{} s] bit_s={} datagram={} | yalın_0x09={lean} veri_çerçeveli={data} (demet {data_bundles}) açılış_iletimi={opens} kapanış_iletimi={closes} demet_iletimi={bundle_tx} tekil_demet={} demet_fazla_iletim={}",
                    secs(self.since(split, opened)),
                    start_bps.map_or("-".to_string(), |b| b.to_string()),
                    pre.len(),
                    secs(self.since(split, opened)),
                    secs(self.since(press_end, opened)),
                    steady_bps.map_or("-".to_string(), |b| b.to_string()),
                    post.len(),
                    unique.len(),
                    bundle_tx.saturating_sub(unique.len()),
                );
                // [PTT-YALIN · KORUMA · 2026-11-22] Bitmap kolunda açılış ve kapanış en az
                // 1 + PTT_RELIABLE_COPIES kez tele çıkar (ayırt edici kapı: kapanış hat doluyken
                // sunulunca kopyaları sessizce planlanmıyordu — üç kolda da tek iletim).
                let want = 1 + usize::from(PTT_RELIABLE_COPIES);
                // [PTT-BİRLEŞİK · deney] Birleşik akışta ayrı açılış/kapanış kaydı yoktur; koruma
                // kayıp ekleme testleriyle (kama-app `tests/ptt.rs`) sınanır, burada sayılmaz.
                let coalesced_stream = self.alice_peer_capable.is_some_and(|c| c <= opened);
                let guarded = !bitmap || coalesced_stream || (opens >= want && closes >= want);
                println!(
                    "ÖLÇÜT {tag} ptt_koruma yön={side} açılış={opens} kapanış={closes} (en az {want}) → {}",
                    if guarded { "GEÇTİ" } else { "KALDI" }
                );
                if !guarded {
                    failed.push(format!("PTT/koruma/{side}"));
                }
            }
            if !band_ok {
                failed.push(format!("PTT/bant/{side}"));
            }
            if !lean_ok {
                failed.push(format!("PTT/yalın-yok/{side}"));
            }
        }
        self.ptt_audio_report(&tag);
        // [CANLILIK · 2026-11-21] Planlanan her demet (kabul edilmeyenler dahil) ufukta teslim.
        // [PTT-SÖZLEŞME · 2026-11-22] Akış ölmez: art arda ufukta teslim edilmeyen demetlerin
        // planlanma aralığı ufku aşmaz (tek tek atlanan demet kodekte örtülür).
        let mut late: Vec<u32> = Vec::new();
        let mut run_start: Option<Instant> = None;
        let mut longest_run = Duration::ZERO;
        for (&k, b) in &self.bundles {
            let ok = b
                .received
                .is_some_and(|r| self.since(r, b.created) <= LIVENESS_HORIZON);
            if ok {
                run_start = None;
            } else {
                late.push(k);
                let s0 = *run_start.get_or_insert(b.planned);
                longest_run = longest_run.max(self.since(b.planned, s0));
            }
        }
        let live = longest_run <= LIVENESS_HORIZON
            && self.unaccepted.is_empty()
            && !self.bundles.is_empty();
        println!(
            "ÖLÇÜT {tag} canlılık: demet={} ufukta_teslim_edilmeyen={} (ilki {:?}; en uzun art arda dizi {} s) kabul_edilmeyen={} ufuk={} s → {}",
            self.bundles.len(),
            late.len(),
            late.first(),
            secs(longest_run),
            self.unaccepted.len(),
            secs(LIVENESS_HORIZON),
            if live { "GEÇTİ" } else { "KALDI" }
        );
        if !live {
            failed.push("CANLILIK".to_string());
        }

        // (3) Dosya.
        let file_t = self.file_received.map(|r| self.since(r, self.t0));
        let file_d = self.file_delivered.map(|r| self.since(r, self.t0));
        let fv = match file_t {
            Some(d) if d <= self.cell.file_bound && self.file_ok => Verdict::Pass,
            _ => Verdict::Fail,
        };
        println!(
            "ÖLÇÜT {tag} dosya: tamamlanma={} s (sınır {} s) bayt_doğru={} parça={} Alice_teslim_bildirimi={} s dosya_bekletme_turu={} uygulama_hataları={:?} → {}",
            opt_secs(file_t),
            secs(self.cell.file_bound),
            self.file_ok,
            self.chunk_count,
            opt_secs(file_d),
            self.file_backlog_held,
            self.file_failures,
            fv.label()
        );
        if fv == Verdict::Fail {
            failed.push("DOSYA".to_string());
        }
        line.push(format!("dosya={}({})", opt_secs(file_t), fv.label()));
        if let Some((sent, rx, ok)) = self.file2 {
            let d = rx.map(|r| self.since(r, sent));
            let v = match d {
                Some(d) if d <= FILE_BOUND && ok => Verdict::Pass,
                _ => Verdict::Fail,
            };
            println!(
                "ÖLÇÜT {tag} dosya2: gönderim=+{} s süre={} s (sınır {} s, gerçek hat 12 500) bayt_doğru={ok} → {}",
                secs(self.since(sent, self.t0)),
                opt_secs(d),
                secs(FILE_BOUND),
                v.label()
            );
            if v == Verdict::Fail {
                failed.push("DOSYA2".to_string());
            }
            line.push(format!("dosya2={}({})", opt_secs(d), v.label()));
        } else if self.cell.file2_at.is_some() {
            println!("ÖLÇÜT {tag} dosya2: gönderilmedi → KALDI");
            failed.push("DOSYA2".to_string());
        }

        // (4) Baytlar.
        let bytes = format!(
            "ileri={} pkt/{} B (kuyruk_düşen={} rastgele_düşen={}) dönüş={} pkt/{} B (kuyruk_düşen={})",
            self.fwd.sent,
            self.fwd.bytes,
            self.fwd.dropped_queue,
            self.fwd.dropped_random,
            self.rev.sent,
            self.rev.bytes,
            self.rev.dropped_queue
        );
        println!(
            "ÖLÇÜT {tag} bayt: {bytes} çözülemeyen_DATA={}/{} kayıt_taşması={}/{}",
            self.alice.unparsed, self.bob.unparsed, self.alice.overflow, self.bob.overflow
        );
        self.trace_report(&tag);
        self.block_report(&tag);
        self.cause_report(&tag);
        self.copy_report(&tag);
        self.guard_trace_report(&tag);
        // [ACK-PAYI] Dönüş hattı bileşimi: Bob'un ACK/denetim datagramları (ADown) ve verisi (BUp)
        // sınıf başına (datagram, tel baytı).
        let mut comp: BTreeMap<String, (u64, u64)> = BTreeMap::new();
        for r in &self.rev.trace {
            let class = match r.route {
                Route::ADown => "ACK/denetim(Bob sunucu)".to_string(),
                Route::BUp => {
                    self.bob
                        .by_fp
                        .get(&r.fp)
                        .map_or("denetim(Bob istemci)".to_string(), |i| {
                            format!("{i:?}")
                                .split('(')
                                .next()
                                .unwrap_or("?")
                                .to_string()
                        })
                }
                _ => "?".to_string(),
            };
            let e = comp.entry(class).or_insert((0, 0));
            e.0 += 1;
            e.1 += r.wire;
        }
        let span = self
            .rev
            .trace
            .last()
            .map_or(Duration::ZERO, |r| self.since(r.offered, self.t0));
        println!(
            "DÖNÜŞ-BİLEŞİM {tag} Y={} süre={} s {:?}",
            std::env::var("KAMA_ASYM_RETURN_Y_PERMILLE").unwrap_or_else(|_| "250".to_string()),
            secs(span),
            comp
        );
        println!(
            "ÜRÜN-SONUÇ {tag} {} ptt_biçim={} {} {bytes} düşen_ölçüt={failed:?}",
            if fixture_ack_format() == AckFormat::Bitmap {
                "yol=bitmap aday (ürün varsayılanı V2)"
            } else {
                "yol=v2 (ürün varsayılanı)"
            },
            ptt_shape_label(),
            line.join(" ")
        );
        failed
    }

    /// [İZ-ARALIK] Alice göndericisinin hat koruması kararları, ACK başına bir satır: ölçüm aralığı
    /// notu (çalışma zamanının 0x15 çerçevesinden çevirdiği M'nin ilk / son seq'i ve alıcının
    /// `rx_span`'i), korumaya ölçüm olarak geçip geçmediği, kuyruk artışı `g`, karar ve korumanın
    /// gönderim tavanı / korunan hızı (önce→sonra). Kayıt yalnız profil açıkken (hat koruması)
    /// dolar; `sıra` boşluğu aynanın yoklamaları arasında halkadan düşen kayıttır.
    fn guard_trace_report(&self, tag: &str) {
        let t = |i: Instant| secs(self.since(i, self.t0));
        let opt = |v: Option<u64>| v.map_or("-".to_string(), |v| v.to_string());
        let label = |d: LineGuardAckDecision| match d {
            LineGuardAckDecision::IntervalSample => "aralık-örneği",
            LineGuardAckDecision::IntervalNoSample => "aralık-örnek-yok",
            LineGuardAckDecision::IntervalSampleUnapplied => "aralık-örneği-uygulanmadı",
            LineGuardAckDecision::IntervalUnresolved => "aralık-kurulamadı",
            LineGuardAckDecision::Frontier => "cephe-örneği",
            LineGuardAckDecision::NoMeasurement => "ölçüm-yok",
            LineGuardAckDecision::NotConsulted => "korumaya-ulaşmadı",
        };
        let mut by: BTreeMap<&str, u64> = BTreeMap::new();
        let mut gaps = 0u64;
        let mut prev = 0u64;
        for &(at, g) in &self.guard_trace {
            gaps += g.index.saturating_sub(prev + 1);
            prev = g.index;
            *by.entry(label(g.decision)).or_insert(0) += 1;
            let (first, last, span) =
                g.note
                    .map_or(("-".to_string(), "-".to_string(), "-".to_string()), |n| {
                        (
                            n.first_seq.to_string(),
                            n.last_seq.to_string(),
                            n.rx_span_ns
                                .map_or("yok".to_string(), |s| format!("{:.4}", s as f64 / 1e9)),
                        )
                    });
            println!(
                "İZ-ARALIK {tag} an=+{} sıra={} en_büyük={} first={first} last={last} rx_span={span} ölçüm={} g={} karar={} tavan={}→{} hız={}→{}{}{}{}",
                t(at),
                g.index,
                g.largest_seq,
                if g.measured { "evet" } else { "hayır" },
                g.g_ns
                    .map_or("-".to_string(), |v| format!("{:.4}", v as f64 / 1e9)),
                label(g.decision),
                g.pace_before,
                g.pace_after,
                opt(g.rate_before),
                opt(g.rate_after),
                g.first_window
                    .map_or(String::new(), |w| format!(" ilk_pencere={w:?}")),
                if g.raised { " yoklama=yükseltti" } else { "" },
                if g.probe_failed { " yoklama=başarısız" } else { "" },
            );
        }
        println!(
            "İZ-ARALIK-ÖZET {tag} kayıt={} sıra_boşluğu={gaps} kararlar={by:?}",
            self.guard_trace.len()
        );
    }

    /// Koşu izi: uygulama olayları ve rölenin gördüğü her datagram (an t0'a göre; öğe, mühürleyen
    /// ucun parmak izi kaydından). ACK ve denetim datagramları DATA değildir: `kontrol` yazılır.
    /// [İZ-BLOK] Alice→Bob FEC bloklarının onarım muhasebesi (blok başına bir satır, sonda özet).
    /// Varış ve kayıp rölenin ileri hat izinden (parmak izi). Blok alıcıda, sahip olduğu farklı
    /// sembol (kaynak + onarım) sayısı `k_actual`'a ulaştığı ilk anda tamamdır (`çözüm`);
    /// `kullanılan` onarım = o ana dek varmış onarım sayısı (kaynak kaybını onlar kapattı).
    /// Varan ama işe yaramayan onarım, bloğun nasıl tamamlandığına göre ayrılır: kayıpsız blok
    /// (ilk iletimde hiç kaynak düşmedi), ARQ sonrası (blok yeniden iletilen kaynakla tamamlandı),
    /// fazla (blok onarımla çözüldükten sonra). Gönderici bilgisi: onarımın mühürlendiği anda
    /// Alice bloğun bütün kaynaklarının ACK'ini / bloğun RECOVERED'ını almış mıydı.
    fn block_report(&self, tag: &str) {
        const K: u64 = 8;
        let t = |i: Instant| self.since(i, self.t0).as_secs_f64();
        let line: HashMap<u64, (u8, Option<Instant>, u64)> = self
            .fwd
            .trace
            .iter()
            .filter(|r| r.route == Route::AUp)
            .map(|r| (r.fp, (r.fate, r.arrives, r.wire)))
            .collect();
        let pn_unit: HashMap<u64, u64> = self.alice.seals.iter().map(|s| (s.1, s.3)).collect();
        let mut acked: HashMap<u64, Instant> = HashMap::new();
        for (at, pns) in &self.alice.acks_rx {
            for pn in pns {
                if let Some(&u) = pn_unit.get(pn) {
                    acked.entry(u).or_insert(*at);
                }
            }
        }
        let mut rec: HashMap<u32, Instant> = HashMap::new();
        for (at, b) in &self.alice.recovered_log {
            rec.entry(*b).or_insert(*at);
        }
        let mut srcs: BTreeMap<u32, BTreeMap<u64, Vec<(Instant, u64)>>> = BTreeMap::new();
        for &(at, u, fp) in &self.alice.data_log {
            srcs.entry(fec_block_of(u, K))
                .or_default()
                .entry(u)
                .or_default()
                .push((at, fp));
        }
        let mut pars: BTreeMap<u32, Vec<(Instant, u16, u8, u64)>> = BTreeMap::new();
        for &(at, b, esi, k, fp) in &self.alice.fec_log {
            pars.entry(b).or_default().push((at, esi, k, fp));
        }
        // Özet sayaçları (yalnız onarım gönderilmiş bloklar).
        #[derive(Default, Debug)]
        struct Sum {
            blocks: u64,
            src: u64,
            src_lost_first: u64,
            par_sent: u64,
            par_needed: u64,
            par_lost: u64,
            par_used: u64,
            par_after_noloss: u64,
            par_after_arq: u64,
            par_after_fec: u64,
            par_never_done: u64,
            par_sealed_after_ack: u64,
            par_sealed_after_rec: u64,
            par_sealed_after_done: u64,
            dec_fec: u64,
            dec_arq: u64,
            dec_clean: u64,
            undecoded: u64,
            par_bytes: u64,
            retx_after_done: u64,
            retx_after_done_bytes: u64,
            retx_after_rec: u64,
            retx_unit_there_block_open: u64,
        }
        let mut sum = Sum::default();
        let mut no_parity_blocks = 0u64;
        let mut no_parity_lost = 0u64;
        let keys: Vec<u32> = srcs
            .keys()
            .chain(pars.keys())
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        for b in keys {
            let units = srcs.get(&b).cloned().unwrap_or_default();
            let par = pars.get(&b).cloned().unwrap_or_default();
            let lane = b >> 30;
            let k = par.first().map_or(units.len() as u64, |p| u64::from(p.2));
            let mut syms: Vec<(Instant, u8)> = Vec::new(); // 0 kaynak ilk, 1 kaynak retx, 2 onarım
            let mut lost_first = 0u64;
            for txs in units.values() {
                let first_ok = txs
                    .first()
                    .and_then(|x| line.get(&x.1))
                    .is_some_and(|l| l.0 == 0);
                if !first_ok {
                    lost_first += 1;
                }
                let arr = txs
                    .iter()
                    .enumerate()
                    .filter_map(|(i, x)| {
                        line.get(&x.1)
                            .and_then(|l| l.1)
                            .map(|a| (a, u8::from(i > 0)))
                    })
                    .min_by_key(|x| x.0);
                if let Some(a) = arr {
                    syms.push(a);
                }
            }
            for p in &par {
                if let Some(a) = line.get(&p.3).and_then(|l| l.1) {
                    syms.push((a, 2));
                }
            }
            syms.sort_by_key(|x| x.0);
            let done: Option<(Instant, &str, u64)> = (k > 0 && syms.len() as u64 >= k).then(|| {
                let first_k = &syms[..k as usize];
                let used = first_k.iter().filter(|x| x.1 == 2).count() as u64;
                let how = if used > 0 {
                    "FEC"
                } else if first_k.iter().any(|x| x.1 == 1) {
                    "ARQ"
                } else {
                    "temiz"
                };
                (first_k[k as usize - 1].0, how, used)
            });
            let ack_done = (units.len() as u64 >= k && k > 0)
                .then(|| {
                    units
                        .keys()
                        .map(|u| acked.get(u).copied())
                        .collect::<Option<Vec<_>>>()
                })
                .flatten()
                .and_then(|v| v.into_iter().max());
            // Blok tamamlandıktan sonra mühürlenen kaynak yeniden iletimleri (ileri hat israfı).
            let (mut rx_before, mut rx_after) = (0u64, 0u64);
            for txs in units.values() {
                for x in txs.iter().skip(1) {
                    if rec.get(&b).is_some_and(|r| x.0 >= *r) {
                        sum.retx_after_rec += 1;
                    }
                    if done.is_some_and(|d| x.0 >= d.0) {
                        rx_after += 1;
                        sum.retx_after_done += 1;
                        sum.retx_after_done_bytes += line.get(&x.1).map_or(0, |l| l.2);
                    } else {
                        rx_before += 1;
                        let unit_arr = txs
                            .iter()
                            .filter_map(|y| line.get(&y.1).and_then(|l| l.1))
                            .min();
                        if unit_arr.is_some_and(|a| x.0 >= a) {
                            sum.retx_unit_there_block_open += 1;
                        }
                    }
                }
            }
            if par.is_empty() {
                no_parity_blocks += 1;
                no_parity_lost += lost_first;
                println!(
                    "İZ-BLOK {tag} blok={lane}:{} k={k} kaynak_ilk_kayıp={lost_first} onarım=0 çözüm={} yeniden_iletim(çözüm_öncesi,sonrası)=({rx_before},{rx_after})",
                    b & 0x3FFF_FFFF,
                    done.map_or("YOK".to_string(), |d| format!("{:.2}({})", t(d.0), d.1))
                );
                continue;
            }
            sum.blocks += 1;
            sum.src += k;
            sum.src_lost_first += lost_first;
            sum.par_sent += par.len() as u64;
            sum.par_needed += lost_first.min(par.len() as u64);
            let (mut lost, mut used, mut a_noloss, mut a_arq, mut a_fec, mut never) =
                (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
            let (mut s_ack, mut s_rec, mut s_done) = (0u64, 0u64, 0u64);
            for p in &par {
                sum.par_bytes += line.get(&p.3).map_or(0, |l| l.2);
                if ack_done.is_some_and(|a| p.0 >= a) {
                    s_ack += 1;
                }
                if rec.get(&b).is_some_and(|r| p.0 >= *r) {
                    s_rec += 1;
                }
                if done.is_some_and(|d| p.0 >= d.0) {
                    s_done += 1;
                }
                match (line.get(&p.3).and_then(|l| l.1), done) {
                    (None, _) => lost += 1,
                    (Some(_), None) => never += 1,
                    (Some(a), Some((d, how, _))) => {
                        if a <= d {
                            used += 1;
                        } else if lost_first == 0 {
                            a_noloss += 1;
                        } else if how == "FEC" {
                            a_fec += 1;
                        } else {
                            a_arq += 1;
                        }
                    }
                }
            }
            match done.map(|d| d.1) {
                Some("FEC") => sum.dec_fec += 1,
                Some("ARQ") => sum.dec_arq += 1,
                Some(_) => sum.dec_clean += 1,
                None => sum.undecoded += 1,
            }
            sum.par_lost += lost;
            sum.par_used += used;
            sum.par_after_noloss += a_noloss;
            sum.par_after_arq += a_arq;
            sum.par_after_fec += a_fec;
            sum.par_never_done += never;
            sum.par_sealed_after_ack += s_ack;
            sum.par_sealed_after_rec += s_rec;
            sum.par_sealed_after_done += s_done;
            let last_src_first = units.values().filter_map(|x| x.first()).map(|x| x.0).max();
            println!(
                "İZ-BLOK {tag} blok={lane}:{} k={k} kaynak_ilk_kayıp={lost_first} onarım={} (yolda_düşen={lost} kullanılan={used} kayıpsız_blokta={a_noloss} ARQ_sonrası={a_arq} çözüm_sonrası={a_fec} çözümsüz={never}) son_kaynak={} onarım_mühür=[{}] çözüm={} Alice_ACK_tamam={} RECOVERED_Alice={} mühür_sonra(ACK,REC,çözüm)=({s_ack},{s_rec},{s_done}) yeniden_iletim(çözüm_öncesi,sonrası)=({rx_before},{rx_after})",
                b & 0x3FFF_FFFF,
                par.len(),
                last_src_first.map_or("-".to_string(), |x| format!("{:.2}", t(x))),
                par.iter().map(|p| format!("{:.2}", t(p.0))).collect::<Vec<_>>().join(" "),
                done.map_or("YOK".to_string(), |d| format!("{:.2}({},{})", t(d.0), d.1, d.2)),
                ack_done.map_or("-".to_string(), |x| format!("{:.2}", t(x))),
                rec.get(&b).map_or("-".to_string(), |x| format!("{:.2}", t(*x))),
            );
        }
        println!(
            "ÖZET-BLOK {tag} {sum:?} onarımsız_blok={no_parity_blocks} (ilk_kayıp={no_parity_lost}) verim(kullanılan/gönderilen)={:.3} ileri_bayt={} lane_dağılımı={:?}",
            sum.par_used as f64 / sum.par_sent.max(1) as f64,
            self.fwd.bytes,
            pars.keys().fold([0u64; 4], |mut a, b| {
                a[(b >> 30) as usize] += pars[b].len() as u64;
                a
            }),
        );
    }

    fn trace_report(&self, tag: &str) {
        let t = |i: Instant| secs(self.since(i, self.t0));
        let ot = |i: Option<Instant>| i.map_or("-".to_string(), t);
        for (side, live, ledger, peer, peer_decoded) in [
            (
                "Alice",
                self.alice_live,
                &self.alice,
                &self.bob,
                self.bob_decoded,
            ),
            (
                "Bob",
                self.bob_live,
                &self.bob,
                &self.alice,
                self.alice_decoded,
            ),
        ] {
            let Some(l) = live else {
                println!("İZ-FEC {tag} {side}: ayna yok");
                continue;
            };
            println!(
                "İZ-FEC {tag} {side}→: latch_blok={} onarım_tx={} parite_kararı={} latch(no_eligible,mismatch,no_rule,enq_r,emit_r)={:?} son_bastırma={:?} onarım_tel_tx={} onarım_karşıda_rx={} karşının_çözdüğü_blok={} karşının_RECOVERED_tx={} RECOVERED_buraya_ulaşan={} | PARAM gönderim={} onay={} onay_anı={} | srtt={} ms taban_pto={} ms rtt_örnek={} | tail_prob={} retx={} kopya={} durum={:?}",
                l.fec_blocks_latched,
                l.fec_repairs_tx,
                l.parity_emitted,
                l.fec_latch_diag,
                l.fec_last_suppression,
                ledger.fec_tx,
                peer.fec_rx,
                peer_decoded,
                peer.recovered_tx,
                ledger.recovered_rx,
                l.param_sends,
                l.param_confirmed,
                if l.param_confirmed > 0 {
                    format!("{:.2} s", l.param_confirmed_at_ns as f64 / 1e9)
                } else {
                    "-".to_string()
                },
                l.srtt_us / 1000,
                l.base_pto_us / 1000,
                l.rtt_samples,
                l.tail_probes,
                l.retransmits,
                l.dups,
                l.control.state,
            );
        }
        for (i, x) in self.texts.iter().enumerate() {
            println!(
                "İZ-UYG {tag} metin{} gönderim={} Bob_teslim={} Alice_alındı={}",
                i + 1,
                ot(x.sent.map(|s| s.1)),
                ot(x.received),
                ot(x.delivered)
            );
        }
        let chunks: Vec<String> = self.chunk_rx.iter().map(|&c| t(c)).collect();
        println!(
            "İZ-UYG {tag} ptt_açılış={} PttStarted={} dosya_Bob={} dosya_Alice_bildirimi={} Bob_parça_teslimleri=[{}]",
            ot(self.ptt_open.map(|p| p.1)),
            ot(self.ptt_started),
            ot(self.file_received),
            ot(self.file_delivered),
            chunks.join(" ")
        );
        for (dir, shaper) in [("ileri", &self.fwd), ("dönüş", &self.rev)] {
            for r in &shaper.trace {
                let ledger = match r.route {
                    Route::AUp | Route::BDown => &self.alice,
                    Route::ADown | Route::BUp => &self.bob,
                };
                let item = ledger
                    .by_fp
                    .get(&r.fp)
                    .map_or("kontrol".to_string(), |i| format!("{i:?}"));
                let fate = match r.fate {
                    0 => "ok",
                    1 => "kuyruk_düştü",
                    _ => "yolda_düştü",
                };
                println!(
                    "İZ-HAT {tag} {dir} {:?} sunum={} tel={} {fate} varış={} {item}",
                    r.route,
                    t(r.offered),
                    r.wire,
                    ot(r.arrives)
                );
            }
        }
    }

    /// [İZ-NEDEN] Alice'in her yeniden gönderiminin (ARQ, PTO probu, kopya) tetikleyicisi ve
    /// gerekliliği. "Gereksiz" = birim o an Bob'a DATA olarak zaten varmıştı (FEC ile kurtarılan
    /// birim DATA varışı sayılmaz; RECOVERED çerçeve sayısı ayrı yazılır). ARQ ve probun hedef
    /// iletimi Bob'a varmışsa alt sınıf: hedefin PN'ini kapsayan ACK Alice'e gelmiş mi (teyit-geldi),
    /// varıştan sonra gelen ACK'ler hedefi kapsamadan daha büyük PN'leri teyit etmiş mi
    /// (aralık-kırpması: alıcı en çok 16 alt blok yazar), hiç ACK gelmemiş mi (ack-yok).
    fn cause_report(&self, tag: &str) {
        let t = |i: Instant| secs(self.since(i, self.t0));
        let seal_by_seq: HashMap<u16, (Instant, u64, u64)> = self
            .alice
            .seals
            .iter()
            .map(|&(at, pn, s, u, _)| (s, (at, pn, u)))
            .collect();
        let mut loss_cause: HashMap<u64, EmitCause> = HashMap::new();
        for (_, e) in &self.emit {
            if matches!(
                e.cause,
                EmitCause::LossPacketThreshold | EmitCause::LossTimeThreshold
            ) {
                loss_cause.entry(e.seq).or_insert(e.cause);
            }
        }
        // (tetik, gerekli/gereksiz, alt sınıf) → sayı
        let mut counts: BTreeMap<(String, &'static str, &'static str), u32> = BTreeMap::new();
        let mut before15: BTreeMap<(String, &'static str), u32> = BTreeMap::new();
        let mut unsealed = 0u32;
        for &(_, e) in &self.emit {
            let mech = match e.cause {
                EmitCause::Retransmit => match loss_cause.get(&e.ref_seq) {
                    Some(EmitCause::LossPacketThreshold) => "ARQ/paket-eşiği".to_string(),
                    Some(EmitCause::LossTimeThreshold) => "ARQ/zaman-eşiği".to_string(),
                    _ => "ARQ/ilan-izsiz".to_string(),
                },
                EmitCause::PtoProbe => "PTO-probu".to_string(),
                EmitCause::Copy => "kopya/vadeli".to_string(),
                EmitCause::CopyPriority => "kopya/PTT-öncelik".to_string(),
                EmitCause::CopyEstablish => "kopya/kuruluş".to_string(),
                _ => continue,
            };
            let Some(&(at, _, _)) = seal_by_seq.get(&((e.seq & 0xFFFF) as u16)) else {
                unsealed += 1;
                continue;
            };
            let had = self.bob.rx_unit.get(&e.unit_id).is_some_and(|&r| r <= at);
            let need = if had { "gereksiz" } else { "gerekli" };
            let sub: &'static str =
                if matches!(e.cause, EmitCause::Retransmit | EmitCause::PtoProbe) {
                    let tgt16 = (e.ref_seq & 0xFFFF) as u16;
                    match (
                        self.bob.rx_seq.get(&tgt16),
                        seal_by_seq.get(&tgt16).map(|x| x.1),
                    ) {
                        (None, _) => "hedef-iletim-düştü",
                        (Some(_), None) => "hedef-mühürsüz",
                        (Some(&r0), Some(pn0)) => {
                            let window = self
                                .alice
                                .acks_rx
                                .iter()
                                .filter(|(a, _)| *a >= r0 && *a <= at);
                            let mut any = false;
                            let mut covered = false;
                            let mut above = false;
                            for (_, pns) in window {
                                any = true;
                                covered |= pns.contains(&pn0);
                                above |= pns.iter().any(|&p| p > pn0);
                            }
                            match (any, covered, above) {
                                (_, true, _) => "teyit-geldi",
                                (true, false, true) => "aralık-kırpması",
                                (true, false, false) => "ack-hedefin-altında",
                                (false, _, _) => "ack-yok",
                            }
                        }
                    }
                } else {
                    "-"
                };
            *counts.entry((mech.clone(), need, sub)).or_insert(0) += 1;
            if self.since(at, self.t0) < Duration::from_secs(15) {
                *before15.entry((mech, need)).or_insert(0) += 1;
            }
        }
        for ((mech, need, sub), n) in &counts {
            println!("İZ-NEDEN {tag} {mech} {need} {sub} = {n}");
        }
        // [İZ-PTT-KAYIT · 2026-11-22] Güvenilir PTT kaydının (açılış / kapanış) her iletimi:
        // mekanizma ve o an Bob'un birimi DATA olarak almış olup olmadığı (mekanizma başına sayı).
        let mut rec_counts: BTreeMap<(&'static str, String, &'static str), u32> = BTreeMap::new();
        let head_hole: HashSet<u64> = self
            .emit
            .iter()
            .filter(|(_, e)| e.cause == EmitCause::PriorityHeadHole)
            .map(|(_, e)| e.seq)
            .collect();
        let mut first_emit: HashMap<u64, Instant> = HashMap::new();
        for &(at, e) in &self.emit {
            let kind = match self.alice.unit_item.get(&e.unit_id) {
                Some(Item::PttOpen(_)) => "açılış",
                Some(Item::PttClose(_)) => "kapanış",
                // [İZ-PTT-KAYIT · demet] Yalın demetin her iletimi de aynı satırla (fazla iletim
                // mekanizması: ilk iletimden sonraki her satır).
                Some(Item::Bundle(_)) => "demet",
                _ => continue,
            };
            let mech = match e.cause {
                EmitCause::Data => "ilk".to_string(),
                EmitCause::Retransmit => match loss_cause.get(&e.ref_seq) {
                    Some(EmitCause::LossPacketThreshold) => "ARQ/paket-eşiği".to_string(),
                    Some(EmitCause::LossTimeThreshold) => "ARQ/zaman-eşiği".to_string(),
                    _ if head_hole.contains(&e.ref_seq) => "ARQ/baş-deliği".to_string(),
                    _ => "ARQ/ilan-izsiz".to_string(),
                },
                EmitCause::PtoProbe => "PTO-probu".to_string(),
                EmitCause::Copy => "kopya/vadeli-ya-da-boş-hat".to_string(),
                EmitCause::CopyPriority => "kopya/PTT-öncelik".to_string(),
                EmitCause::CopyEstablish => "kopya/kuruluş".to_string(),
                _ => continue,
            };
            let at = seal_by_seq
                .get(&((e.seq & 0xFFFF) as u16))
                .map_or(at, |x| x.0);
            let had = self.bob.rx_unit.get(&e.unit_id).is_some_and(|&r| r <= at);
            let first = *first_emit.entry(e.unit_id).or_insert(at);
            println!(
                "İZ-PTT-KAYIT {tag} {kind} birim={} an={} ilkten=+{} {mech} seq={} hedef={} bob_almıştı={had}",
                e.unit_id,
                t(at),
                secs(self.since(at, first)),
                e.seq,
                e.ref_seq
            );
            *rec_counts
                .entry((kind, mech, if had { "gereksiz" } else { "gerekli" }))
                .or_insert(0) += 1;
        }
        for ((kind, mech, need), n) in &rec_counts {
            println!("İZ-PTT-KAYIT {tag} özet {kind} {mech} {need} = {n}");
        }
        for ((mech, need), n) in &before15 {
            println!("İZ-NEDEN {tag} ilk15s {mech} {need} = {n}");
        }
        // ACK çerçevelerinin Alice'e varış aralıkları (en büyük üç) ve kırpılmış çerçeveler.
        let mut gaps: Vec<(Duration, Instant)> = self
            .alice
            .acks_rx
            .windows(2)
            .map(|w| (w[1].0 - w[0].0, w[0].0))
            .collect();
        gaps.sort_by_key(|g| std::cmp::Reverse(g.0));
        let first_ack = self.alice.acks_rx.first().map(|a| self.since(a.0, self.t0));
        println!(
            "İZ-NEDEN {tag} ack_çerçevesi={} ilk=+{} en_büyük_aralıklar={:?} mühürsüz_emisyon={unsealed} RECOVERED(Bob→Alice)={}",
            self.alice.acks_rx.len(),
            opt_secs(first_ack),
            gaps.iter()
                .take(4)
                .map(|(d, a)| format!("{}@{}", secs(*d), t(*a)))
                .collect::<Vec<_>>(),
            self.alice.recovered_rx,
        );
        // [İZ-ACK · salt gözlem] Alice'e açılan ACK'ler: sayı, tel boyu, varış aralığı, bildirilen
        // kapsam (Bob'a varan DATA PN'lerinden — son ACK varışından önce varanlar — en az bir
        // ACK'te görünenlerin payı) ve dönüş hattındaki ACK bayt payı.
        {
            let n = self.alice.ack_wire.len() as u64;
            let sum: u64 = self.alice.ack_wire.iter().map(|w| w.0).sum();
            let max = self.alice.ack_wire.iter().map(|w| w.0).max().unwrap_or(0);
            let types: BTreeMap<u8, u32> =
                self.alice
                    .ack_wire
                    .iter()
                    .fold(BTreeMap::new(), |mut m, w| {
                        *m.entry(w.1).or_insert(0) += 1;
                        m
                    });
            let ivals: Vec<Duration> = self
                .alice
                .acks_rx
                .windows(2)
                .map(|w| w[1].0 - w[0].0)
                .collect();
            let ival_mean = if ivals.is_empty() {
                Duration::ZERO
            } else {
                ivals.iter().sum::<Duration>() / ivals.len() as u32
            };
            let ival_max = ivals.iter().max().copied().unwrap_or(Duration::ZERO);
            let reported: HashSet<u64> = self
                .alice
                .acks_rx
                .iter()
                .flat_map(|(_, p)| p.iter().copied())
                .collect();
            let last_ack = self.alice.acks_rx.last().map(|a| a.0);
            let (mut rx_n, mut cov_n) = (0u64, 0u64);
            for (wseq, &arr) in &self.bob.rx_seq {
                if last_ack.is_none_or(|l| arr > l) {
                    continue;
                }
                if let Some(&(_, pn, _)) = seal_by_seq.get(wseq) {
                    rx_n += 1;
                    if reported.contains(&pn) {
                        cov_n += 1;
                    }
                }
            }
            let rev_bytes = self.rev.bytes.max(1);
            println!(
                "İZ-ACK {tag} biçim={} ack={n} tür={types:?} tel_ort={} B tel_max={max} B aralık_ort={} s aralık_max={} s kapsam={cov_n}/{rx_n} ({:.1}%) dönüş_ack_payı={sum}/{} B ({:.1}%)",
                std::env::var("KAMA_ASYM_ACK_FORMAT").unwrap_or_else(|_| "v2".to_string()),
                sum.checked_div(n).unwrap_or(0),
                secs(ival_mean),
                secs(ival_max),
                if rx_n == 0 { 0.0 } else { cov_n as f64 * 100.0 / rx_n as f64 },
                self.rev.bytes,
                sum as f64 * 100.0 / rev_bytes as f64,
            );
        }
        if self.profile {
            self.report_first_ack(tag);
        }
        // [CANLILIK-TEŞHİS] Geç (> 2 s) ya da hiç teslim edilmeyen ilk beş demetin motor izi:
        // her iletim (neden), iletimlerin kayıp ilanları, birimin teyidi; şerit açıklık sınırı.
        println!(
            "İZ-CANLILIK {tag} şerit_açıklık_sınırı(PTT,SIG,TEXT,FILE)={:?} kabul_edilmeyen_demet={}",
            self.lane_span_blocked,
            self.unaccepted.len()
        );
        let mut shown = 0;
        for (&k, b) in &self.bundles {
            let late = b
                .received
                .is_none_or(|r| self.since(r, b.created) > Duration::from_secs(2));
            if !late || shown >= 5 {
                continue;
            }
            shown += 1;
            let units: HashSet<u64> = self
                .alice
                .unit_item
                .iter()
                .filter(|(_, i)| **i == Item::Bundle(k))
                .map(|(u, _)| *u)
                .collect();
            let seqs: HashSet<u64> = self
                .emit
                .iter()
                .filter(|(_, e)| units.contains(&e.unit_id) && e.cause != EmitCause::Confirmed)
                .map(|(_, e)| e.seq)
                .collect();
            let mut evs: Vec<String> = Vec::new();
            for &(at, e) in &self.emit {
                let mine = match e.cause {
                    EmitCause::LossPacketThreshold | EmitCause::LossTimeThreshold => {
                        seqs.contains(&e.seq)
                    }
                    EmitCause::AckFrame => false,
                    _ => units.contains(&e.unit_id),
                };
                if mine && evs.len() < 40 {
                    evs.push(format!("{}:{:?}#{}", t(at), e.cause, e.seq));
                }
            }
            println!(
                "İZ-CANLILIK {tag} demet={k} plan=+{} teslim={} birim={:?} olaylar=[{}]",
                secs(self.since(b.planned, self.t0)),
                b.received
                    .map_or("YOK".to_string(), |r| format!("+{}", t(r))),
                units,
                evs.join(" ")
            );
        }
        for &(at, e) in &self.emit {
            if e.cause == EmitCause::AckFrame {
                println!(
                    "İZ-ACK {tag} an={} yeni_teyit={} en_büyük_seq={} hat_koruması={}",
                    t(at),
                    e.unit_id,
                    e.seq,
                    e.ref_seq
                );
            }
        }
    }

    /// [PTT-BANT · kıyas] Bant düşüşü ses azaltarak mı geldi: 20 ms çerçeve sayıları (basışta
    /// planlanan, oluşan, uygulamaca kabul edilen, telde, teslim, oluşumdan ≤ 1 s teslim) ve
    /// gönderilen ses yükü baytı (tekil ve telde). Son demet kısmi olabilir: çerçeveleri sayılır.
    /// Gerçek zaman (sahip 2026-11-22): ilk ses gecikmesi = Bob'a ilk ses teslimi − basış başı
    /// (ilk 20 ms çerçevenin oluşumu); ilk PTT datagramının ve ilk ses datagramının tele çıkışı.
    fn ptt_audio_report(&self, tag: &str) {
        let shape = ptt_shape();
        let fpb = u64::try_from(shape.period.as_nanos() / LYRA_FRAME.as_nanos())
            .unwrap_or(1)
            .max(1);
        let press = self.press_len.unwrap_or(Duration::ZERO);
        let planned = u64::try_from(press.as_nanos() / LYRA_FRAME.as_nanos()).unwrap_or(0);
        let created: u64 = self
            .bundles
            .values()
            .map(|b| u64::from(b.frames))
            .sum::<u64>()
            + self.unaccepted.iter().map(|u| u64::from(u.1)).sum::<u64>();
        // Demetin ilk tel datagramı yalın 0x09 mu (ilk 0x16'dan sonra mı çıktı).
        let mut first_lean: HashMap<u32, bool> = HashMap::new();
        for w in &self.alice.ptt_wire {
            if let Item::Bundle(k) = w.3 {
                first_lean.entry(k).or_insert(w.2);
            }
        }
        // (kabul edilen, telde, teslim, zamanında) — 20 ms çerçeve sayısı.
        let mut rows: BTreeMap<&str, [u64; 4]> = BTreeMap::new();
        for (&k, b) in &self.bundles {
            let key = match first_lean.get(&k) {
                Some(true) => "0x16_sonrası",
                Some(false) => "0x16_öncesi",
                None => "tele_çıkmadı",
            };
            let f = u64::from(b.frames);
            let r = rows.entry(key).or_insert([0; 4]);
            r[0] += f;
            r[1] += f * u64::from(first_lean.contains_key(&k));
            r[2] += f * u64::from(b.received.is_some());
            r[3] += f * u64::from(
                b.received
                    .is_some_and(|rx| self.since(rx, b.created) <= PTT_BOUND),
            );
        }
        let total = rows.values().fold([0u64; 4], |mut a, r| {
            for (x, y) in a.iter_mut().zip(r) {
                *x += y;
            }
            a
        });
        let bytes_of = |b: &Bundle| (shape.bytes as u64 * u64::from(b.frames) / fpb).max(4);
        let unique_bytes: u64 = self.bundles.values().map(bytes_of).sum();
        let wire_bytes: u64 = self
            .bundles
            .iter()
            .map(|(&k, b)| {
                u64::from(
                    self.alice
                        .tx_count
                        .get(&Item::Bundle(k))
                        .copied()
                        .unwrap_or(0),
                ) * bytes_of(b)
            })
            .sum();
        let wire_tx: u64 = self
            .bundles
            .keys()
            .map(|&k| {
                u64::from(
                    self.alice
                        .tx_count
                        .get(&Item::Bundle(k))
                        .copied()
                        .unwrap_or(0),
                )
            })
            .sum();
        let split: Vec<String> = rows
            .iter()
            .map(|(k, r)| {
                format!(
                    "{k}:kabul={} telde={} teslim={} zamanında={}",
                    r[0], r[1], r[2], r[3]
                )
            })
            .collect();
        println!(
            "ÖLÇÜT {tag} ptt_ses basış={} s çerçeve(20 ms) planlanan={planned} oluşan={created} kabul={} telde={} teslim={} zamanında(oluşumdan ≤ {} s)={} | {} | ses_yükü_tekil={unique_bytes} B ses_yükü_telde={wire_bytes} B (demet {} , son demet çerçevesi {}, demet iletimi {wire_tx})",
            secs(press),
            total[0],
            total[1],
            total[2],
            secs(PTT_BOUND),
            total[3],
            split.join(" "),
            self.bundles.len(),
            self.bundles.values().last().map_or(0, |b| b.frames),
        );
        if let Some((_, opened)) = self.ptt_open {
            let first_rx = self.bundles.values().filter_map(|b| b.received).min();
            let first_dgram = self.alice.ptt_wire.first().map(|w| w.0);
            let first_audio_dgram = self
                .alice
                .ptt_wire
                .iter()
                .find(|w| matches!(w.3, Item::Bundle(_)))
                .map(|w| w.0);
            println!(
                "ÖLÇÜT {tag} ptt_zaman ilk_ses(ilk çerçeve oluşumundan Bob teslimine)={} s ilk_PTT_datagramı_telde=+{} s ilk_ses_datagramı_telde=+{} s (basış başından)",
                opt_secs(first_rx.map(|r| self.since(r, opened))),
                opt_secs(first_dgram.map(|d| self.since(d, opened))),
                opt_secs(first_audio_dgram.map(|d| self.since(d, opened))),
            );
            self.ptt_byte_split(tag, opened, press);
        }
    }

    /// [PTT-BİRLEŞİK · deney · bayt dökümü] Alice → Bob PTT tel baytının (IPv4/UDP dahil) dökümü:
    /// ses (her demetin ilk iletimi), açılış+kapanış (ayrı kaydın ilk iletimi; birleşik akışta
    /// açılış bilgisinin demetteki 5 B'si ve yüksüz `END`), kopya (açılış/kapanış kaydının sonraki
    /// iletimleri), yeniden iletim (demetin sonraki iletimleri), 0x16 öncesi (ilk yalın 0x09'dan
    /// önce veri çerçevesiyle çıkan PTT datagramları; yukarıdakilerin alt kümesi). Bant tavanı
    /// 1,5 s'lik basışta 937 B'dir. Birleşik akışta kapanışın Bob'daki anı ve sayaçlar.
    fn ptt_byte_split(&self, tag: &str, opened: Instant, press: Duration) {
        let mut seen: HashSet<Item> = HashSet::new();
        let (mut audio, mut ctrl, mut copies, mut retx) = (0u64, 0u64, 0u64, 0u64);
        let (mut n_audio, mut n_ctrl, mut n_copies, mut n_retx) = (0u32, 0u32, 0u32, 0u32);
        let first_lean = self.alice.ptt_wire.iter().find(|w| w.2).map(|w| w.0);
        let (mut pre, mut n_pre) = (0u64, 0u32);
        for w in &self.alice.ptt_wire {
            let first = seen.insert(w.3);
            match (w.3, first) {
                (Item::Bundle(_), true) => {
                    audio += w.1;
                    n_audio += 1;
                }
                (Item::Bundle(_), false) => {
                    retx += w.1;
                    n_retx += 1;
                }
                (_, true) => {
                    ctrl += w.1;
                    n_ctrl += 1;
                }
                (_, false) => {
                    copies += w.1;
                    n_copies += 1;
                }
            }
            if first_lean.is_none_or(|l| w.0 < l) && !w.2 {
                pre += w.1;
                n_pre += 1;
            }
        }
        // Birleşik akışta açılış bilgisi demetin içindedir: 5 B/demet (7 B − kompakt 2 B).
        let spurt_open_bytes = 5 * self.alice.spurt_open_tx;
        let total = audio + ctrl + copies + retx;
        let cap = PTT_BAND_MAX_BPS * u64::try_from(press.as_millis()).unwrap_or(0) / 8_000;
        println!(
            "ÖLÇÜT {tag} ptt_bayt toplam={total} B (tavan {cap} B) ses={audio} B/{n_audio} dgram (içinde birleşik açılış bilgisi {spurt_open_bytes} B, {} açılışlı demet, {} END'li demet) açılış+kapanış={ctrl} B/{n_ctrl} dgram kopya={copies} B/{n_copies} yeniden_iletim={retx} B/{n_retx} | 0x16_öncesi_veri_çerçeveli={pre} B/{n_pre} dgram → {}",
            self.alice.spurt_open_tx,
            self.alice.spurt_end_tx,
            if total <= cap { "GEÇTİ" } else { "KALDI" },
        );
        let release = opened + press;
        let b = &self.bob_ptt;
        println!(
            "ÖLÇÜT {tag} ptt_birleşik kol={:?} yetenek_alice=+{} s (t0'dan) bob_PttStarted=+{} s (basıştan) bob_PttEnded=+{} s (bırakıştan) | bob: açılış_demetiyle={} END_ile={} sessizlik_sonu={} yetim_demet={} END_sonrası={} reddedilen={} PTT-R4={} unknown_close={}",
            ptt_shape().coalesce,
            opt_secs(self.alice_peer_capable.map(|c| self.since(c, self.t0))),
            opt_secs(self.ptt_started.map(|s| self.since(s, opened))),
            opt_secs(self.ptt_ended.map(|e| self.since(e, release))),
            b.spurt_opens,
            b.spurt_ends,
            b.streams_ended_by_gap,
            b.ptt_orphan_bundles,
            b.spurt_after_end,
            b.spurt_refused,
            b.streams_ended_by_idle,
            b.unknown_closes,
        );
    }

    /// [İZ-KOPYA · salt gözlem] Daralma evresinde (düşüşten sonraki 6 s; düşüşsüz hücrede 8–14 s)
    /// ileri darboğaz kuyruğu ve kopyalar. Rölenin izi yeniden oynatılır (FIFO, hız çizelgesi):
    /// her datagramın kuyrukta önündeki tel baytı ve bekleme süresi. Satırlar: (1) kopya başına
    /// mekanizma (iz nedeni + kaynak kodu + şerit), sayı, bayt, gereksiz (birim o an Bob'a DATA olarak
    /// varmıştı); (2) en büyük kuyruk beklemesi, anı ve o an kuyruktaki baytın mekanizma dağılımı.
    fn copy_report(&self, tag: &str) {
        let drop_at = self
            .cell
            .fwd_plan
            .get(1)
            .map_or(Duration::from_secs(8), |p| p.0);
        let (w0, w1) = (drop_at, drop_at + Duration::from_secs(6));
        let src_name = |c: u64| match c {
            1 => "kural",
            2 => "planlı(kısmi-blok/PTT-güvenilir)",
            3 => "ön-yakınsama",
            4 => "kayıp-kopya-yeniden",
            5 => "RecoverFwd",
            6 => "ARQ-tavan-devri",
            7 => "saf-sessizlik",
            8 => "boş-hat-yeniden-iletim",
            _ => "?",
        };
        let lane = |u: u64| ["PTT", "SIG", "TEXT", "FILE"][client_core::unit_id::lane_index_of(u)];
        let mech_of = |e: &EmitDiag| -> Option<String> {
            Some(match e.cause {
                EmitCause::Data => format!("veri/{}", lane(e.unit_id)),
                EmitCause::Retransmit => format!("ARQ/{}", lane(e.unit_id)),
                EmitCause::PtoProbe => format!("PTO/{}", lane(e.unit_id)),
                EmitCause::Copy | EmitCause::CopyPriority | EmitCause::CopyEstablish => {
                    format!("kopya:{}/{}", src_name(e.ref_seq), lane(e.unit_id))
                }
                _ => return None,
            })
        };
        // Motor iletim sırası (u16) → parmak izi → mekanizma; FEC datagramı ayrı sınıf.
        let fp_of_seq: HashMap<u16, u64> = self.alice.seals.iter().map(|s| (s.2, s.4)).collect();
        let mut fp_mech: HashMap<u64, String> = HashMap::new();
        let mut copies: BTreeMap<String, (u32, u64, u32)> = BTreeMap::new();
        // Pencere kopyalarının dökümü: an, şerit içi sıra, kaynak, gereksiz mi, birimin önceki
        // iletiminin nedeni.
        let mut list: Vec<String> = Vec::new();
        let mut prev_cause: HashMap<u64, EmitCause> = HashMap::new();
        for &(at, e) in &self.emit {
            let Some(m) = mech_of(&e) else { continue };
            let prev = prev_cause.insert(e.unit_id, e.cause);
            let Some(&fp) = fp_of_seq.get(&((e.seq & 0xFFFF) as u16)) else {
                continue;
            };
            fp_mech.insert(fp, m.clone());
            let el = self.since(at, self.t0);
            if !m.starts_with("kopya") || el < w0 || el >= w1 {
                continue;
            }
            let had = self.bob.rx_unit.get(&e.unit_id).is_some_and(|&r| r <= at);
            list.push(format!(
                "{}:{}:{}:{}:{}",
                secs(el),
                client_core::unit_id::lane_seq_of(e.unit_id),
                e.ref_seq,
                if had { "var" } else { "yok" },
                prev.map_or("-".to_string(), |c| format!("{c:?}"))
            ));
            let wire = self
                .fwd
                .trace
                .iter()
                .find(|r| r.fp == fp)
                .map_or(0, |r| r.wire);
            let c = copies.entry(m).or_insert((0, 0, 0));
            c.0 += 1;
            c.1 += wire;
            c.2 += u32::from(had);
        }
        for &(_, _, _, _, fp) in &self.alice.fec_log {
            fp_mech.insert(fp, "FEC".to_string());
        }
        println!(
            "İZ-KOPYA {tag} daralma döküm(an:sıra:kaynak:Bob'da:önceki)=[{}]",
            list.join(" ")
        );
        for (m, (n, b, un)) in &copies {
            println!("İZ-KOPYA {tag} daralma {m} sayı={n} bayt={b} gereksiz={un}");
        }
        // Rölenin ileri kuyruğunu yeniden oynat: (sunum, bitiş, tel, sınıf).
        let mut q: Vec<(Duration, Duration, u64, String)> = Vec::new();
        let mut busy_until = Duration::ZERO;
        for r in &self.fwd.trace {
            if r.fate == 1 {
                continue;
            }
            let off = self.since(r.offered, self.t0);
            let start = busy_until.max(off);
            let rate = self.fwd.rate_at(self.t0 + start).max(1);
            let done = start + Duration::from_nanos(r.wire * 1_000_000_000 / rate);
            busy_until = done;
            let class = match r.route {
                Route::AUp => fp_mech
                    .get(&r.fp)
                    .cloned()
                    .unwrap_or_else(|| "denetim".to_string()),
                _ => "Bob-denetim".to_string(),
            };
            q.push((off, done, r.wire, class));
        }
        let peak = q
            .iter()
            .enumerate()
            .filter(|(_, x)| x.0 >= w0 && x.0 < w1)
            .max_by_key(|(_, x)| x.1 - x.0);
        if let Some((i, p)) = peak {
            let mut ahead: BTreeMap<String, (u32, u64)> = BTreeMap::new();
            for x in q[..i].iter().filter(|x| x.1 > p.0) {
                let e = ahead.entry(x.3.clone()).or_insert((0, 0));
                e.0 += 1;
                e.1 += x.2;
            }
            println!(
                "İZ-KOPYA {tag} kuyruk_tepe={} s an=+{} önündeki={ahead:?}",
                secs(p.1 - p.0),
                secs(p.0)
            );
        }
        let total: u32 = copies.values().map(|c| c.0).sum();
        let unneeded: u32 = copies.values().map(|c| c.2).sum();
        println!("İZ-KOPYA {tag} daralma kopya_toplam={total} gereksiz={unneeded}");
    }

    fn ptt_report(
        &self,
        tag: &str,
        fwd_drops: &HashSet<u64>,
        file_end: Option<Instant>,
    ) -> (Verdict, String) {
        let Some((stream, opened)) = self.ptt_open else {
            println!("ÖLÇÜT {tag} ptt: açılmadı → KALDI");
            return (Verdict::Fail, "ptt=AÇILMADI(KALDI)".to_string());
        };
        // PTT şeridindeki öncüller: açılış + demetler, gönderim sırasıyla. (öğe, uygulamaya teslim)
        let mut lane: Vec<(Item, Option<Instant>)> =
            vec![(Item::PttOpen(stream), self.ptt_started)];
        let mut missing = 0u32;
        let mut excused = 0u32;
        let mut over = 0u32;
        let mut max_delay = Duration::ZERO;
        let mut max_delay_counted = Duration::ZERO;
        let mut max_excused = Duration::ZERO;
        // Sınırı aşıp kalan demetlerden, kaybolmadan PTT şeridinde ilk iletimi düşmüş bir öncülünü
        // bekleyenler (ayrı sütun; istisna değil).
        let mut held_n = 0u32;
        let mut max_held = Duration::ZERO;
        let mut first_missing: Option<u32> = None;
        let mut max_gap = Duration::ZERO;
        let mut max_accept_wait = Duration::ZERO;
        let mut prev_rx: Option<Instant> = None;
        let mut first: Option<(Duration, Verdict)> = None;
        for (&seq, b) in &self.bundles {
            let item = Item::Bundle(seq);
            max_accept_wait = max_accept_wait.max(self.since(b.accepted, b.planned));
            let Some(rx) = b.received else {
                missing += 1;
                first_missing.get_or_insert(seq);
                println!(
                    "İZ-PTT {tag} demet={seq} plan=+{} ilk_çıkış=+{} iletim={} ilk_iletim_düştü={} ilk_varış=+{} teslim=YOK",
                    secs(self.since(b.planned, self.t0)),
                    opt_secs(self.alice.first_tx.get(&item).map(|(a, _)| self.since(*a, self.t0))),
                    self.alice.tx_count.get(&item).copied().unwrap_or(0),
                    Self::lost_first(&self.alice, fwd_drops, item),
                    opt_secs(self.bob.first_rx.get(&item).map(|a| self.since(*a, self.t0))),
                );
                lane.push((item, None));
                continue;
            };
            let delay = self.since(rx, b.created);
            max_delay = max_delay.max(delay);
            if file_end.is_none_or(|f| b.planned <= f) {
                if let Some(p) = prev_rx {
                    max_gap = max_gap.max(self.since(rx, p));
                }
            }
            prev_rx = Some(rx);
            let own = Self::lost_first(&self.alice, fwd_drops, item);
            let arrival = self.bob.first_rx.get(&item).copied();
            // Öncül beklemesi: demet Bob'a vardığında ilk iletimi düşmüş bir öncül henüz teslim
            // edilmemişti (varış bilinmiyorsa demet kurtarmayla gelmiştir).
            let held = lane.iter().any(|&(p, p_rx)| {
                Self::lost_first(&self.alice, fwd_drops, p)
                    && match (arrival, p_rx) {
                        (Some(a), Some(pr)) => pr >= a,
                        _ => true,
                    }
            });
            let v = if delay <= PTT_BOUND {
                Verdict::Pass
            } else if own {
                Verdict::LossRecovery
            } else {
                Verdict::Fail
            };
            match v {
                Verdict::Pass => max_delay_counted = max_delay_counted.max(delay),
                Verdict::LossRecovery => {
                    excused += 1;
                    max_excused = max_excused.max(delay);
                }
                Verdict::Fail => {
                    over += 1;
                    max_delay_counted = max_delay_counted.max(delay);
                    if held {
                        held_n += 1;
                        max_held = max_held.max(delay);
                    }
                    println!(
                        "ÖLÇÜT {tag} ptt demet {seq}: gecikme={} s ilk_çıkış=+{} s ilk_varış=+{} s iletim={} → KALDI",
                        secs(delay),
                        opt_secs(self.alice.first_tx.get(&item).map(|(a, _)| self.since(*a, b.created))),
                        opt_secs(arrival.map(|a| self.since(a, b.created))),
                        self.alice.tx_count.get(&item).copied().unwrap_or(0),
                    );
                }
            }
            if first.is_none() {
                first = Some((delay, v));
            }
            println!(
                "İZ-PTT {tag} demet={seq} plan=+{} ilk_çıkış=+{} iletim={} ilk_iletim_düştü={own} ilk_varış=+{} teslim=+{} gecikme={} öncül_bekledi={held} → {}",
                secs(self.since(b.planned, self.t0)),
                opt_secs(self.alice.first_tx.get(&item).map(|(a, _)| self.since(*a, self.t0))),
                self.alice.tx_count.get(&item).copied().unwrap_or(0),
                opt_secs(arrival.map(|a| self.since(a, self.t0))),
                secs(self.since(rx, self.t0)),
                secs(delay),
                v.label()
            );
            lane.push((item, Some(rx)));
        }
        let start = self.ptt_started.map(|s| self.since(s, opened));
        let n = self.bundles.len();
        // [ÜRÜN KABULÜ · 2026-11-16] Profil açıkken (ürün kolu) kendi ilk iletimi düşen demet de
        // sınır içinde olmalı — kullanıcı sesi geç duyar; ürün kabulünde "kayıp-kurtarma" başarı
        // sayılmaz. İstisna yalnız profil kapalı teşhis kolunda ayrı raporlanır.
        // [PTT-SÖZLEŞME · sahip kararı 2026-11-22] Media sözleşmesi: zamanında (≤ 1 s) oranı.
        let late_total = u64::from(missing) + u64::from(over) + u64::from(excused);
        let total = n as u64 + self.unaccepted.len() as u64;
        let timely = (n as u64).saturating_sub(late_total);
        let verdict = if ptt_timely_ok(timely, total) {
            Verdict::Pass
        } else {
            Verdict::Fail
        };
        // [PTT-BANT · 2026-11-22] Saat değişikliği görünsün: eski saat (sunum = planlanan an) yalnız
        // raporlanır; ölçüt oluşum saatiyle (demet başı) verilir.
        let timely_presented = self
            .bundles
            .values()
            .filter(|b| {
                b.received
                    .is_some_and(|r| self.since(r, b.planned) <= PTT_BOUND)
            })
            .count();
        let first_presented = self
            .bundles
            .values()
            .find_map(|b| b.received.map(|r| self.since(r, b.planned)));
        println!(
            "ÖLÇÜT {tag} ptt-sözleşme: zamanında(oluşum)={timely}/{total} (en az binde {PTT_TIMELY_MIN_PERMILLE}) zamanında(sunum, eski saat, yalnız rapor)={timely_presented}/{total} ilk_ses(oluşum)={} s ilk_ses(sunum)={} s → {}",
            opt_secs(first.map(|f| f.0)),
            opt_secs(first_presented),
            verdict.label()
        );
        println!(
            "ÖLÇÜT {tag} ptt: demet={n} teslim_yok={missing} (ilki {}) açılış→PttStarted={} s ilk_demet={} s ({}) sınır_üstü_kaldı={over} (bunların öncül_bekledi={held_n}, en büyük {} s) kayıp-kurtarma(kendi_kaybı)={excused} (en büyük {} s) en_büyük_gecikme_sayılan={} s en_büyük_gecikme_tümü={} s dosya_sürerken_en_büyük_boşluk={} s kabul_beklemesi_en_çok={} s → {}",
            first_missing.map_or("-".to_string(), |m| m.to_string()),
            opt_secs(start),
            opt_secs(first.map(|f| f.0)),
            first.map_or("YOK", |f| f.1.label()),
            secs(max_held),
            secs(max_excused),
            secs(max_delay_counted),
            secs(max_delay),
            secs(max_gap),
            secs(max_accept_wait),
            verdict.label()
        );
        (
            verdict,
            format!(
                "ptt_ilk={}({}) ptt_en_büyük_sayılan={} ptt_en_büyük_tümü={} ptt_boşluk={} ptt_kaldı={over}/{n}(öncül_bekledi={held_n}) ptt_kayıp_kurtarma={excused}/{n} ptt_teslim_yok={missing} ptt={}",
                opt_secs(first.map(|f| f.0)),
                first.map_or("YOK", |f| f.1.label()),
                secs(max_delay_counted),
                secs(max_delay),
                secs(max_gap),
                verdict.label()
            ),
        )
    }
}

fn env_u64(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

/// [KAPI] Bir ölçüm hücresi: gerçek ve beyan edilen hızlar, çizelgeler, sınırlar ve evreler.
/// Beyan edilen hızlar her hücrede aynıdır (Alice yukarı [`FWD_BPS`] / aşağı [`REV_BPS`], Bob ters
/// çevrilmiş); hücre yalnız gerçek hattı değiştirir.
#[derive(Debug, Clone)]
struct Cell {
    name: &'static str,
    /// Gerçek ileri hız çizelgesi (t0'dan, B/s); ilk kayıt 0 s.
    fwd_plan: Vec<(Duration, u64)>,
    /// Gerçek dönüş tek yön gecikme çizelgesi; boşsa [`ONE_WAY`] sabit.
    rev_delay_plan: Vec<(Duration, Duration)>,
    file_bound: Duration,
    receipt_bound: Duration,
    /// Metin 1 ve metin 2'ye ek, sabit anda gönderilen metinler.
    texts_at: Vec<Duration>,
    /// İkinci dosyanın gönderim anı (sınır [`FILE_BOUND`], gerçek hat 12 500 B/s iken).
    file2_at: Option<Duration>,
    /// Evreler: (ad, başlangıç); ölçütler evre başına ayrıca raporlanır ve iddia edilir.
    phases: Vec<(&'static str, Duration)>,
    /// [FİKSTÜR · 2026-11-21] Evre başına niyet: (PTT sınanır, metin sınanır). Niyetli evrede hiç
    /// planlanmış demet / metin yoksa sonuç "fikstür geçersiz" (kanıt yok, geçti sayılmaz).
    phase_intent: Vec<(bool, bool)>,
    /// [FİKSTÜR · 2026-11-21] PTT'nin kapanış anı (t0'dan); `None` = dosya(lar) Bob'a varınca
    /// (eski kural). Verilirse PTT dosyadan bağımsız bu ana dek konuşur.
    ptt_until: Option<Duration>,
    /// Koşu tavanı (o ana dek gelmeyen gözlem düşmüş sayılır).
    run_cap: Duration,
    /// [CANLILIK] `true` = hücre yalnız canlılık ölçütünü iddia eder (öbürleri raporlanır).
    liveness_only: bool,
}

impl Cell {
    fn phase_of(&self, at: Duration) -> usize {
        self.phases
            .iter()
            .rposition(|(_, from)| *from <= at)
            .unwrap_or(0)
    }

    fn fixed(name: &'static str, real: u64) -> Self {
        // Dosya sınırı gerçek hızla ölçeklenir: 14 s × 12 500 / gerçek.
        let file_bound = Duration::from_nanos(FILE_BOUND.as_nanos() as u64 * FWD_BPS / real.max(1));
        Self {
            name,
            fwd_plan: vec![(Duration::ZERO, real)],
            rev_delay_plan: Vec::new(),
            file_bound,
            receipt_bound: RECEIPT_RETURN_BOUND,
            texts_at: vec![LATE_TEXT_AT],
            file2_at: None,
            phases: vec![("başlangıç", Duration::ZERO), ("sürekli", START_PHASE)],
            // Sabit hızlı hücrelerde PTT dosya bitene dek sürer (eski senaryo): sürekli evrede PTT
            // niyeti yok; metin 10 s'de sürekli evreyi sınar.
            phase_intent: vec![(true, true), (false, true)],
            ptt_until: None,
            run_cap: RUN_CAP,
            liveness_only: false,
        }
    }

    fn rate_drop_recovery() -> Self {
        Self {
            name: "düşüş-toparlanma",
            fwd_plan: vec![
                (Duration::ZERO, FWD_BPS),
                (DROP_AT, FWD_BPS / 2),
                (RECOVER_AT, FWD_BPS),
            ],
            rev_delay_plan: Vec::new(),
            file_bound: DROP_FILE_BOUND,
            receipt_bound: RECEIPT_RETURN_BOUND,
            texts_at: vec![
                Duration::from_secs(12),
                Duration::from_secs(26),
                Duration::from_secs(34),
            ],
            file2_at: Some(FILE2_AT),
            phases: vec![
                ("başlangıç", Duration::ZERO),
                ("daralma", DROP_AT),
                ("toparlanma", RECOVER_AT),
            ],
            // [FİKSTÜR · 2026-11-21] Üç evrede de PTT ve metin; PTT dosyadan bağımsız 40 s'ye dek.
            phase_intent: vec![(true, true), (true, true), (true, true)],
            // Teşhis: `KAMA_ASYM_PTT_UNTIL_S` kapanışı değiştirir (kapı varsayılanı 40 s).
            ptt_until: Some(Duration::from_secs(env_u64(
                "KAMA_ASYM_PTT_UNTIL_S",
                DROP_PTT_UNTIL.as_secs(),
            ))),
            run_cap: RUN_CAP,
            liveness_only: false,
        }
    }

    /// [CANLILIK · 2026-11-21] Düşüş-toparlanma hattında uzun PTT oturumu (0,5–60 s, 298 demet:
    /// PTT şeridinin kabul açıklığından (256) uzun). Yalnız canlılık iddia edilir.
    fn ptt_liveness_long_session() -> Self {
        Self {
            name: "canlılık-uzun-ptt",
            ptt_until: Some(LIVENESS_PTT_UNTIL),
            run_cap: LIVENESS_RUN_CAP,
            liveness_only: true,
            ..Self::rate_drop_recovery()
        }
    }

    fn return_delay_varies() -> Self {
        let ms = Duration::from_millis;
        Self {
            name: "dönüş-gecikmesi-değişken",
            fwd_plan: vec![(Duration::ZERO, FWD_BPS)],
            rev_delay_plan: vec![
                (Duration::ZERO, ms(300)),
                (Duration::from_secs(3), ms(400)),
                (Duration::from_secs(6), ms(500)),
                (Duration::from_secs(9), ms(700)),
                (Duration::from_secs(12), ms(500)),
                (Duration::from_secs(15), ms(300)),
                (Duration::from_secs(18), ms(700)),
                (Duration::from_secs(21), ms(300)),
            ],
            file_bound: FILE_BOUND,
            receipt_bound: RECEIPT_RETURN_BOUND + REV_DELAY_EXTRA_MAX,
            texts_at: vec![LATE_TEXT_AT],
            file2_at: None,
            phases: vec![("başlangıç", Duration::ZERO), ("sürekli", START_PHASE)],
            phase_intent: vec![(true, true), (false, true)],
            ptt_until: None,
            run_cap: RUN_CAP,
            liveness_only: false,
        }
    }
}

/// Başlangıç evresinin sonu: ilk bilgi taşıyan ACK'in ölçülen en geç varışı (7,9 s) + pay.
const START_PHASE: Duration = Duration::from_secs(8);
/// Sürekli evrede gönderilen metin.
const LATE_TEXT_AT: Duration = Duration::from_secs(10);
const DROP_AT: Duration = Duration::from_secs(8);
const RECOVER_AT: Duration = Duration::from_secs(20);
const FILE2_AT: Duration = Duration::from_secs(24);
/// [CANLILIK · 2026-11-21] Uzun PTT oturumunun kapanışı ve koşu tavanı (tavan − kapanış = ufuk +
/// 5 s pay).
const LIVENESS_PTT_UNTIL: Duration = Duration::from_secs(60);
const LIVENESS_RUN_CAP: Duration = Duration::from_secs(80);
/// [CANLILIK] Planlanan her PTT demetinin teslim ufku (planlanan an → Bob'da teslim). Gerekçe:
/// kaybolan demetin kurtarılması bir ACK aralığı (bu hatta ≤ 8,6 s) + yol RTT'si (0,6 s) + yeniden
/// iletimin hatta çıkışı ister; 15 s bir ACK aralığını kaçırmaya da yer bırakır. Canlılık ölçütüdür,
/// PTT gecikme sınırı (1 s) değil.
const LIVENESS_HORIZON: Duration = Duration::from_secs(15);
/// [FİKSTÜR · 2026-11-21] Düşüş hücresinde PTT'nin kapanışı: toparlanma evresinin ilk 20 s'si.
const DROP_PTT_UNTIL: Duration = Duration::from_secs(40);
/// Düşüş hücresinde dosya 1: gerçek hattın t0'dan taşıdığı bayt 14 s × 12 500 B/s'ye ulaştığı an
/// (8 s × 12 500 + 12 s × 6 250 = 175 000 B) → 20 s.
const DROP_FILE_BOUND: Duration = Duration::from_secs(20);
/// Değişken dönüş gecikmesinin 300 ms üstündeki en büyük fazlası (alındı yolu buna maruz).
const REV_DELAY_EXTRA_MAX: Duration = Duration::from_millis(400);

/// Aynı süreçteki hücreler sırayla koşar (zamanlama ölçümü; paralel koşu iki çekirdeği paylaşır).
static SERIAL: Mutex<()> = Mutex::new(());

fn run_cell(cell: Cell, assert_pass: bool) {
    let _serial = lock(&SERIAL);
    let seed = env_u64("KAMA_ASYM_SEED", 1);
    let profile = std::env::var("KAMA_ASYM_PROFILE").map_or(true, |v| v != "off");
    let name = cell.name;
    let liveness_only = cell.liveness_only;
    let r = run(seed, profile, cell);
    let mut failed = r.report();
    if liveness_only {
        println!(
            "CANLILIK-HÜCRESİ {name}: yalnız canlılık iddia edilir; bütün düşenler {failed:?}"
        );
        failed.retain(|f| f == "CANLILIK");
    }
    if !assert_pass {
        println!("TEŞHİS-KOLU hücre={name} (tohum {seed}) — düşen ölçütler: {failed:?}");
        return;
    }
    assert!(
        failed.is_empty(),
        "asimetrik hatta düşen ürün ölçütleri (hücre {name}, tohum {seed}, profil {}): {failed:?}",
        if profile { "açık" } else { "kapalı" }
    );
}

#[test]
fn text_receipt_ptt_and_file_all_progress_on_the_asymmetric_line() {
    let real_fwd = env_u64("KAMA_ASYM_REAL_FWD_BPS", FWD_BPS);
    if real_fwd != FWD_BPS {
        // Yanlış profil teşhis kolu (#216): profil ileri hattı FWD_BPS bildirir, gerçek hat
        // farklıdır; ölçütler raporlanır, iddia edilmez (kalıcı kapılar aşağıdaki hücrelerdir).
        run_cell(Cell::fixed("teşhis-gerçek-ileri", real_fwd), false);
        return;
    }
    run_cell(Cell::fixed("dürüst-12500", FWD_BPS), true);
}

#[test]
fn gate_real_10000_declared_12500() {
    run_cell(Cell::fixed("gerçek-10000", 10_000), true);
}

#[test]
fn gate_real_6250_declared_12500() {
    run_cell(Cell::fixed("gerçek-6250", 6_250), true);
}

#[test]
fn gate_rate_drop_then_recovery_in_one_session() {
    run_cell(Cell::rate_drop_recovery(), true);
}

#[test]
fn gate_ptt_liveness_long_session() {
    run_cell(Cell::ptt_liveness_long_session(), true);
}

#[test]
fn gate_forward_fixed_return_delay_varies() {
    run_cell(Cell::return_delay_varies(), true);
}

/// [PTT-BANT · 2026-11-22, tavan 5 000 bit/s] Bant ölçütünün iki yönlü kanıtı (ağ koşusu yok).
/// Yalın 0x09 datagramı: UDP/IP 28 + mühür 22 + çerçeve 3 = 53 B sabit pay + AMP kaydı. Gerçek Lyra
/// (3,2 kbit/s = 20 ms'de 8 B):
/// * bugünkü biçim, 200 ms (80 B yük, 13 B tam `MEDIA_BUNDLE` başlığı) → 146 B → 5 840 bit/s KALIR;
/// * kol A: 300 ms demet, kompakt 2 B başlık (120 B yük) → 175 B → 4 666 bit/s GEÇER;
/// * 300 ms demet, tam başlık (120 B yük) → 186 B → 4 960 bit/s GEÇER (40 bit/s pay);
/// * 420 ms demet, tam başlık (168 B yük) → 234 B → 4 457 bit/s GEÇER.
///
/// Payda basış süresidir (bütün kollarda aynı).
#[test]
fn ptt_band_gate_passes_the_lean_payload_and_fails_the_80_byte_control() {
    let t0 = Instant::now();
    let spurt = |wire: u64, cadence: Duration, n: u32| -> Vec<(Instant, u64)> {
        (0..n).map(|i| (t0 + cadence * i, wire)).collect()
    };
    let ms = Duration::from_millis;
    let today = ptt_band_bps(&spurt(53 + 13 + 80, ms(200), 50), ms(200) * 50).expect("bant");
    assert_eq!(today, 5_840);
    assert!(today > PTT_BAND_MAX_BPS);
    // Kol A: 300 ms, kompakt 2 B başlık (120 B yük) → 175 B → 4 666 bit/s.
    let compact = ptt_band_bps(&spurt(53 + 2 + 120, ms(300), 50), ms(300) * 50).expect("bant");
    assert_eq!(compact, 4_666);
    assert!(compact <= PTT_BAND_MAX_BPS);
    // 300 ms, tam başlık (120 B yük) → 186 B → 4 960 bit/s: sınırın 40 bit/s altında.
    let mid = ptt_band_bps(&spurt(53 + 13 + 120, ms(300), 50), ms(300) * 50).expect("bant");
    assert_eq!(mid, 4_960);
    assert!(mid <= PTT_BAND_MAX_BPS);
    let long = ptt_band_bps(&spurt(53 + 13 + 168, ms(420), 50), ms(420) * 50).expect("bant");
    assert_eq!(long, 4_457);
    assert!(long <= PTT_BAND_MAX_BPS);
    assert_eq!(ptt_band_bps(&[], ms(200) * 50), None);
    // Konuşmadan sonra yayılan yeniden iletim paydayı büyütmez.
    let mut late = spurt(53 + 13 + 168, ms(420), 50);
    late.push((t0 + Duration::from_secs(60), 234));
    assert!(ptt_band_bps(&late, ms(420) * 50).expect("bant") > 4_457);
}
