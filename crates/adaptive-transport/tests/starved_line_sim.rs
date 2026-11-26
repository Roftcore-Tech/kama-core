//! [İŞ-34] Geri bildirimi aç profilde hat korumasının karar girdisi (ölçüm aralığı) ve beş PTT
//! iletiminin kaybı: ayrık olaylı, deterministik benzetim (1 ms adım; gerçek `Sender` + gerçek
//! `Receiver`; ürünün geri bildirimi aç profil kurulumu 12 500 / 62 B/s).
//!
//! # Düzenek
//!
//! * İleri hat: hız çizelgesi (serileşme başladığı andaki hız), sınırlı FIFO, 300 ms yayılım,
//!   serileşmeden sonra tohumlu rastgele kayıp (LCG), isteğe bağlı yeniden sıralama (alıcıda bir
//!   datagram bir sonrakinden sonra işlenir) ve hedefli kayıp (bir birimin ilk `n` iletimi).
//! * Dönüş hattı: 62 B/s serileşme, gecikme çizelgesi (varışlar FIFO kalır).
//! * Alıcının ACK bütçesi dönüş hızından ayrı verilebilir (`ack_return_bw`): bitmap kolunun sık
//!   ACK'leri (ürün testi, dönüş-gecikmesi hücresi: ACK aralığı ≈ 1,2–2,5 s).
//! * Ölçüm aralığı (sözleşme #175 7067964700 §2) ve çift alt sınırı (`pair_rate`): bu düzenek
//!   alıcının yerine hesaplar (alıcı tarafı `receiver.rs` koordinatörün işidir), alıcının bugünkü
//!   kuralıyla (`RxMirror`; İŞ-35 gece hizası: aralık bir ACK geriden, `[span_first, largest]`; önceki
//!   düzenek iki ACK geriden kuruyordu ve `pair_rate` vermiyordu). `no_interval` belirli bir zaman
//!   aralığında alıcının çıkardığı ACK'lerin ölçümünü siler (aralık kipinde ölçümsüz ACK dizisi).
//!   Gönderici girdisi ürün çalışma zamanının sırasıyla `Sender::note_ack_pair_rate`,
//!   `Sender::note_ack_forward_interval` (sözleşme §3).
//!   `interval = false` kolu bu çağrıları yapmaz: eski cephe tahmincisi (karşı-olgu).
//!
//! # Ölçülen
//!
//! Darboğazda her datagramın kuyruk beklemesi ve kuyrukta düşmeler, korumanın hız tahmininin
//! zaman izi, PTT demetlerinin SIRALI teslim gecikmesi (ürün PTT şeridi sıralıdır), metnin
//! gecikmesi, dosyanın bitişi, ileri hatta PTT tel baytı.
//!
//! [İŞ-35 AÇIK 2] PTT koruması tükendiğinde kurtarma: her PTT iletiminin darboğaz kaydı (`PttTx`),
//! her ACK'in yolculuğu (`AckRec`: alıcının verdiği an, dönüş serileşmesinin başı, varış) ve
//! göndericinin izi (`Scenario::diag`, salt gözlem). Testler dosyanın sonunda.
//!
//! # [BUA] Ürün alıcısı, anahtarlı kayıp, darboğaz defteri (ölçüm aracı)
//!
//! Üç senaryo seçeneği; varsayılanları var olan bütün testleri bayt-aynı bırakır:
//!
//! * `Scenario::rx` — [`RxModel::Mirror`] (varsayılan): 0x14 alıcısının ACK kadansı + ölçüm aralığı
//!   ve `pair_rate` [`RxMirror`] kopyasından (melez; ACK bütçesi `ack_return_bw`'den).
//!   [`RxModel::Product`]: gerçek `Receiver` ürünün sunucu yuvası gibi kurulur (bütçe = dönüş
//!   hattı [`PROFILE_DOWN`], CAPS kolu: `set_ack_bitmap(true)`, `set_pair_squeeze_line(beyan ileri
//!   hat)`, `set_ack_bitmap_ptt_lean(ptt_lean)`); her ACK üretildiği anda kodlanır, mühürlü boyla
//!   ([`ACK_SEAL`]) dönüş hattında serileşir, varışta `AckFrame::decode_with_bitmap` ile çözülür ve
//!   göndericiye `client_core::runtime` ACK kolunun sırasıyla verilir: `observe_ack_feedback`
//!   (çerçevenin `seen_count`/`v2`'si), `note_ack_arrival_rate`, `note_ack_pair_rate`,
//!   `set_next_ack_evidence` (kanıt aralığı), `note_ack_forward_interval` (`span_first_full`,
//!   `rx_span` × 256 µs, M = teyitli ∩ [span_first, largest]), `Event::AckReceived`. Alıcıya her
//!   `on_datagram` / `on_ack_timer` çağrısından önce dönüş hattının o anki durumu verilir
//!   (`set_return_line_ahead_ns`, `set_uplink_yield`).
//! * `Scenario::loss` — [`LossModel::Lcg`] (varsayılan): sunulan datagram sırasına bağlı tek LCG.
//!   [`LossModel::Keyed`]: karar = `splitmix64(tohum, sınıf, kimlik, o öğenin kaçıncı iletimi)`
//!   binde değeri < `loss_permille` (ürün fikstürü `asym_product_outcome` anahtarlı kipiyle aynı
//!   karma; sınıf: veri birimi, kimlik birim; FEC paritesi, kimlik `blok << 16 | ESI`). Sıra
//!   göndericinin iletim kararında sayılır (kuyrukta düşen iletim de sayılır): iki kol farklı
//!   davransa da aynı mantıksal iletim aynı kaderi alır.
//! * `Scenario::ledger` — darboğaz defteri (salt gözlem): darboğaza giren tel baytı sınıf
//!   (emisyon nedeni: yeni DATA, ARQ, PTO probu, kopya [kaynağıyla], FEC paritesi) × yük (PTT,
//!   metin, dosya, FEC) başına, kuyrukta ve hatta düşen bayt, FIFO en büyük bekleme ve doluluk,
//!   gönderim tavanının izi (an, değer, ACK no), göndericide bekleyen veri (saniyede bir). Defter
//!   koşu sonunda hat modelinin sayaçlarıyla birebir sağlanır (`assert`; ölçüm aracının
//!   doğrulaması, kabul kapısı değil). Açıkken göndericinin emisyon izi her olaydan sonra
//!   boşaltılır (sınıf kaynağı).
//!
//! Rapor kolları: `bua_product_receiver_report` (yalnız sayı basar; aracın sağlaması dışında iddia
//! yok). Koşu: `cargo test --release -p adaptive-transport --test starved_line_sim
//! bua_product_receiver_report -- --nocapture`. Çıktı satırları makine okunurdur, alanlar
//! `anahtar=değer`: `BUA <kol> <alıcı> tohum=<n> ...` (özet), `BUA-B` / `BUA-D2` (var olan (b) ve
//! (d-2) testlerinin ölçüleri ve sınırları), `BUA-DEFTER` (sınıf × yük), `BUA-HAT` (sayaçlar),
//! `BUA-TAVAN` (tavan izi), `BUA-BİRİKİM` (gönderici). Alıcı adı `urun` | `ayna`.

//!
//! # [KAPI] Uçuş penceresi kapısı gözlemi (ölçüm aracı)
//!
//! Üç senaryo seçeneği; varsayılanları var olan bütün testleri bayt-aynı bırakır:
//!
//! * `Scenario::pair_probe` — çift yoklaması deney anahtarı (`Sender::set_pair_probe_datagrams`,
//!   kural kurulduktan SONRA çağrılır: kural hat korumasını yeniden kurar). `None`: çağrı yapılmaz.
//! * `Scenario::feedback_ns_override` — kuralın geri bildirim ufku (uçuş tabanı bununla kurulur).
//!   Yalnız aracın kendi sınaması içindir (kapının bağladığı bilinen kol).
//! * `Scenario::gate_diag` — ACK başına gözlem satırı ([`GateAck`]: ACK göndericiye verilmeden
//!   hemen önce ve `Event::AckReceived` döndükten hemen sonra `Sender::diag_flight_gate`, korumanın
//!   ACK kaydı, çerçevenin `pair_rate` alanı) ve adım başına iz ([`GateStep`]: 1 ms; uçuş, kapının
//!   penceresi, tavan, o adımda darboğaza giren datagram / tel baytı, bekleyen birim). Salt gözlem:
//!   göndericinin korumasının ACK kaydı açılır (`set_diag_line_guard_trace`), başka hiçbir çağrı
//!   eklenmez; darboğaz her iletimin akıbetini de yazar ([`GateTx`]). Bellek: adım başına bir
//!   kayıt, koşu süresi × 1 000.
//!
//! Birimler: uçuş ve pencere kapının birimiyle (bayt, UDP gövdesi; kapı `inflight + boy ≤ pencere`
//! ifadesinde `boy` = birimin UDP gövdesi), tavan ve darboğaza giren bayt tel baytıyla (UDP + 28).
//!
//! Rapor: `pair_probe_first_three_acks_flight_gate_report` (yalnız aracın tutarlılığını sınar;
//! ön kayıttaki öngörüler iddia değildir, basılır). Aracın kendi sınaması:
//! `flight_gate_report_sees_a_binding_gate`. Çıktı satırları: `KAPI-ACK` (markdown tablo satırı;
//! başlığı `KAPI-ACK-BAŞLIK`), `KAPI-HÜCRE` (hücre özeti), `KAPI-YOKLAMA` (koşudaki her yoklama:
//! pencere, pencerede giden her iletim `seq@an:akıbet`, hüküm zinciri), `KAPI-EVRE` (evre başına
//! kapı görünümü), `KAPI-ÖZET` (sınıf sayıları), `KAPI-ÖNKAYIT` (ön kayıttaki ölçülerin sayımı),
//! `KAPI-SINAMA` (aracın kendi sınaması).
//!
//! # [K2] Kovalı darboğaz (ölçüm aracı)
//!
//! Dört senaryo seçeneği; varsayılanları var olan bütün testleri bayt-aynı bırakır:
//!
//! * `Scenario::bucket_burst` — `None`: ileri hat serileştiren FIFO'dur (bugünkü yol). `Some(B)`:
//!   jeton kovası (tbf): hız = çizelgedeki hat hızı, burst `B` bayt, IP boyu sayılır (UDP gövdesi +
//!   28), paket tanecikli (sıradaki datagram kovada IP boyu kadar jeton olunca çıkar; jeton hat
//!   hızıyla dolar, kuyruk boşken `B`'de doyar; hız değişince dolma hızı o andan itibaren yeni
//!   hız), kuyruk `queue_pkts` PAKET (doluysa girişte düşer), çıkıştan sonra 300 ms yayılım ve
//!   kayıp. Gerçek düzeneğin karşılığı: `tbf rate <hız> burst 1200 limit 64` + `stab overhead -14`,
//!   ardında `pfifo limit 64`. Kovalı kipte "serileşme başlangıcı" (`ser_start`, `waits`) çıkış
//!   anıdır. Modelin gerçek tbf'den farkı: jeton sürekli dolar (çekirdeğin zamanlayıcı tanecikliği
//!   ve `peakrate`/MTU kovası yok), tek kuyruk (tbf'nin kendi `limit`'i ile `pfifo` ayrı değil),
//!   burst'ten büyük datagram girişte düşer.
//! * `Scenario::file_unit`, `Scenario::text_unit` — dosya ve metin biriminin UDP gövdesi
//!   (`None`: 1 000 B ve 80 B, düzeneğin bugünkü değerleri; ürün datagramları 1 046 B ve 108 B).
//! * `Scenario::text_until_ns` — ek metin akışının bitişi (`None`: konuşmanın bitişi).
//!
//! `Scenario::gate_diag` açıkken ayrıca: alıcının çift sayımının aynası ([`PairMirror`]; yalnız
//! ürün alıcısında), uçuş kapısının gönderim denemesi ret sayacı (`Sender::diag_gate_refusals`),
//! korumanın başlattığı çift yoklaması sayısı (`Sender::diag_pair_probes`). Hat modeli her
//! iletimin (giriş, çıkış, IP boyu, çıkış anındaki hız) kaydını her zaman tutar (`Outcome::passes`).
//!
//! Testler: `k2_bucket_model_matches_the_measured_gaps` ve
//! `k2_gate_refusal_counter_sees_a_binding_gate` (aracın doğrulaması, `K2ARAC`),
//! `k2_bucket_two_counterexamples_report` (yalnız rapor: `K2ROW`, `K2OZET`, `K2NOT`).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use adaptive_transport::ack::{
    AckFrame, ACK_BITMAP_MAX_BITS, ACK_DELAY_UNIT_US, PAIR_MIN_UDP_BYTES, PAIR_RATE_NONE,
    RX_SPAN_NONE,
};
use adaptive_transport::line_guard::{ProbeOutcome, ProbeVerdictDiag};
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, EmitCause, EmitDiag, Event, FeedbackStarvedRule, FlightGateDiag, ForwardCopyLineScale,
    ForwardCopyRule, LineGuardAckDecision, LineGuardAckTrace, OpaqueUnit, RecoveredInfo,
    RecoveredUnit, Sender, SrLogicalRef, Timer, TransmitDecision, TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const ACK_SEAL: u64 = 22;
const ONE_WAY_NS: u64 = 300 * MS;
const FILE_UNIT: u64 = 1000;
const FILE_BASE_ID: u64 = 1_000_000;
const TEXT_ID: u64 = 500_000;
const VOICE_UNIT: u64 = 86;
/// Koordinatörün yazdığı paket içi yedekli PTT biçimi (#175, sahip 2026-11-22): 200 ms'de tek paket;
/// o anki demetle önceki iki demetin sesi (3 × 40 B) + tek başlık takımı (46 B) + demet başına
/// küçük etiket payı ≈ 172 B UDP gövdesi (≈ 200 B tel, ≈ 8 kbit/s). Hat koruması testleri bu yükle de
/// sınanır: kopya karışımına bağlı geçiş sağlam sayılmaz.
// Henüz hiçbir senaryo bu biçimi seçmiyor (yalnız `Scenario::voice_unit` belgesinde); clippy
// `-D warnings` kapısı için.
#[allow(dead_code)]
const PTT_PACKED_UNIT: u64 = 172;
const VOICE_PERIOD_NS: u64 = 200 * MS;
const VOICE_START_NS: u64 = 500 * MS;
/// Ürün kabul ölçütleri (`asym_product_outcome`): PTT demeti ve metin 1 s, dürüst dosya 14 s.
const PTT_BOUND_NS: u64 = SEC;
const TEXT_BOUND_NS: u64 = SEC;
const FILE_BOUND_NS: u64 = 14 * SEC;
/// Ürünün PTT kuralı (`client_core::runtime::PTT_FORWARD_COPIES` = 7,
/// `PTT_FORWARD_COPIES_MIN` = 6; aralık 200 ms / 7). 2026-11-16 kuralı: 5 iletim, 40 ms.
const PRODUCT_COPIES: u8 = 6;
const PRODUCT_MIN_COPIES: u8 = 5;
const PRODUCT_SPACING_NS: u64 = 200 * MS / 7;
const OLD_COPIES: u8 = 4;
const OLD_SPACING_NS: u64 = 40 * MS;

/// Ürün testinin dönüş-gecikmesi hücresinin çizelgesi (`asym_product_outcome`
/// `Cell::return_delay_varies`).
const REV_DELAY_PLAN: [(u64, u64); 8] = [
    (0, 300 * MS),
    (3 * SEC, 400 * MS),
    (6 * SEC, 500 * MS),
    (9 * SEC, 700 * MS),
    (12 * SEC, 500 * MS),
    (15 * SEC, 300 * MS),
    (18 * SEC, 700 * MS),
    (21 * SEC, 300 * MS),
];

#[derive(Debug, Clone)]
struct Scenario {
    /// İleri hattın hız çizelgesi: (başlangıç anı, B/s tel).
    fwd_plan: Vec<(u64, u64)>,
    /// Dönüş hattının tek yön gecikme çizelgesi: (başlangıç anı, ns).
    rev_delay_plan: Vec<(u64, u64)>,
    loss_permille: u64,
    queue_pkts: usize,
    /// Alıcıda bir datagramın bir sonrakinden sonra işlenme olasılığı (‰).
    reorder_permille: u64,
    /// Hedefli kayıp: bu birimin ilk `n` iletimi serileşmeden sonra düşer.
    targeted_drop: Option<(u64, u32)>,
    /// Dosya birimleri ve ilk sunum anı; `file_gap_ns > 0` ise birimler bu aralıkla tek tek
    /// sunulur (uygulamanın az göndermesi), 0 ise hepsi birden.
    file_units: u64,
    file_at_ns: u64,
    file_gap_ns: u64,
    voice_until_ns: u64,
    text_at_ns: Option<u64>,
    copies: u8,
    spacing_ns: u64,
    copy_scale_min: Option<u8>,
    /// PTT biriminin UDP gövde boyu (B). Bugünkü biçim [`VOICE_UNIT`] (40 B ses + 46 B başlık);
    /// paket içi yedekli biçim [`PTT_PACKED_UNIT`].
    voice_unit: u64,
    /// Alıcının ACK bütçesinin hesaplandığı dönüş hızı (B/s).
    ack_return_bw: u64,
    /// Ölçüm aralığı göndericiye verilir mi (`false` = karşı-olgu: eski cephe tahmincisi).
    interval: bool,
    /// Alıcının bu aralıkta (başlangıç, bitiş; ns, alıcıda ACK'in çıkış anı) çıkardığı ACK'ler ölçüm
    /// taşımaz (`rx_span` = yok): aralık kipinde ölçümsüz ACK dizisi.
    no_interval: Option<(u64, u64)>,
    /// Göndericinin emisyon / kayıp ilanı izi açılır (salt gözlem; karar okumaz).
    diag: bool,
    /// [PTT-YALIN] Kopya kuralı `copies`/`spacing_ns` ile OLDUĞU GİBİ kurulur (ürünün
    /// `ptt_forward_copy_rule(copies, spacing_ns)`: `below_id` = PTT şeridinin üstü, öncelik dosyanın
    /// altı; `copies == 0` = kopyasız öncelik kuralı). `false`: eski kurulum (`copies == 0` → 1 kopya,
    /// `below_id` 0).
    raw_copy_rule: bool,
    /// [PTT-YALIN] Bırakma: sunumundan bu kadar sonra teyitsiz PTT birimi motora
    /// `Event::RecoveredControl` (block_id 0, esi 0) ile bildirilir (ürünün son tarih yolu).
    ptt_deadline_ns: Option<u64>,
    /// `> 0`: `text_at_ns`'ten sonra bu aralıkla konuşma bitene dek ek metin birimleri
    /// (`TEXT_ID + k`). 0: tek metin birimi.
    text_period_ns: u64,
    /// [BUA] Alıcı modeli (modül belgesi).
    rx: RxModel,
    /// [BUA] İleri hattın rastgele kayıp kipi (modül belgesi).
    loss: LossModel,
    /// [BUA] Darboğaz defteri ve sağlaması (modül belgesi).
    ledger: bool,
    /// [KAPI] Çift yoklaması deney anahtarı (datagram bütçesi); `None` = ayarlayıcı çağrılmaz
    /// (bugünkü yol).
    pair_probe: Option<u64>,
    /// [KAPI] Kuralın geri bildirim ufku (ns); `None` = ürün değeri (`feedback_horizon_ns`).
    /// Yalnız aracın kendi sınaması için.
    feedback_ns_override: Option<u64>,
    /// [KAPI] Uçuş kapısı gözlemi (modül belgesi).
    gate_diag: bool,
    /// [K2] Kovalı darboğaz (modül belgesi, [K2]): `Some(B)` = jeton kovası, burst `B` bayt (IP
    /// boyu). `None` = serileştiren FIFO (bugünkü yol).
    bucket_burst: Option<u64>,
    /// [K2] Dosya biriminin UDP gövdesi (B); `None` = [`FILE_UNIT`].
    file_unit: Option<u64>,
    /// [K2] Metin biriminin UDP gövdesi (B); `None` = 80 (düzeneğin bugünkü değeri).
    text_unit: Option<u64>,
    /// [K2] Ek metin akışının (`text_period_ns`) bitişi; `None` = `voice_until_ns` (bugünkü yol).
    text_until_ns: Option<u64>,
    seed: u64,
    t_max_ns: u64,
}

/// [BUA] Alıcı modeli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RxModel {
    /// 0x14 alıcısı + [`RxMirror`] ölçüm kopyası (var olan testlerin düzeneği).
    Mirror,
    /// Ürünün bitmap alıcısı (0x15; `ptt_lean` = CAPS'te yalın PTT biti → 0x16). `return_bw` =
    /// alıcının ACK bütçesinin hesaplandığı dönüş hızı (B/s); ürün çalışma zamanında bu ucun
    /// yukarı yönü, yani [`PROFILE_DOWN`] ([`RxModel::PRODUCT`]). Başka değer yalnız ayrıştırma
    /// kolu içindir (aynanın bütçesiyle ürün alıcısı).
    Product { ptt_lean: bool, return_bw: u64 },
}

impl RxModel {
    /// Ürün çalışma zamanının kurulumu (0x15, bütçe dönüş hattından).
    const PRODUCT: Self = Self::Product {
        ptt_lean: false,
        return_bw: PROFILE_DOWN,
    };

    fn label(self) -> String {
        match self {
            Self::Mirror => "ayna".to_string(),
            Self::Product {
                ptt_lean,
                return_bw,
            } => {
                let mut l = "urun".to_string();
                if ptt_lean {
                    l.push_str("-yalin");
                }
                if return_bw != PROFILE_DOWN {
                    l.push_str(&format!("-bw{return_bw}"));
                }
                l
            }
        }
    }
}

/// [BUA] İleri hattın rastgele kayıp kipi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LossModel {
    /// Sunulan (serileşen) datagram sırasına bağlı tek LCG.
    Lcg,
    /// Mantıksal iletime anahtarlı karar ([`keyed_permille`]).
    Keyed,
}

/// [BUA · ANAHTARLI-KAYIP] Sınıf: veri birimi (kimlik = birim kimliği). Ürün fikstürü
/// `asym_product_outcome` `KEY_CLASS_UNIT` ile aynı.
const KEY_CLASS_UNIT: u8 = 0x00;
/// [BUA · ANAHTARLI-KAYIP] Sınıf: FEC paritesi (kimlik = `blok << 16 | ESI`); ürün fikstürünün
/// `KEY_CLASS_FEC` değeri (`client_core::wire::FRAME_FEC` = 0x01).
const KEY_CLASS_FEC: u8 = 0x01;
/// [BUA · ANAHTARLI-KAYIP] Mühürleyen uç: düzenekte tek gönderici (ürün fikstüründe `ALICE` = 1).
const KEY_SIDE: u8 = 1;

/// splitmix64 sonlandırıcısı (ürün fikstürüyle aynı sabitler).
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// [BUA · ANAHTARLI-KAYIP] Anahtarın binde değeri (0..1000): tohum ve (uç, sınıf, kimlik, sıra)
/// anahtarının saf işlevi (ürün fikstürü `keyed_permille` ile aynı karma).
fn keyed_permille(seed: u64, class: u8, id: u64, ordinal: u32) -> u64 {
    let mut h = splitmix64(seed);
    h = splitmix64(h ^ (u64::from(KEY_SIDE) << 8 | u64::from(class)));
    h = splitmix64(h ^ id);
    h = splitmix64(h ^ u64::from(ordinal));
    h % 1000
}

/// [BUA · DEFTER] Darboğaza giren iletimin sınıfı (göndericinin emisyon nedeni).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TxClass {
    /// Birimin ilk iletimi.
    NewData,
    /// ARQ yeniden iletimi.
    Arq,
    /// PTO / kuyruk probu.
    PtoProbe,
    /// Zamanlanmış kopya ya da boş hat yeniden iletimi (kaynak kodu `DUP_SRC_*`).
    Copy(u64),
    /// Öncelikli şeridin kopyası (kaynak kodu).
    CopyPriority(u64),
    /// Kuruluş merdiveni kopyası (kaynak kodu).
    CopyEstablish(u64),
    /// FEC onarım sembolü.
    FecParity,
    /// İzde nedeni bulunmayan iletim (defter açıkken olmamalı; sağlama bunu da sınar).
    Unknown(&'static str),
}

impl TxClass {
    fn label(self) -> String {
        // Kaynak kodları `sender.rs` `DUP_SRC_*` (crate içi sabitler).
        let src = |c: u64| match c {
            1 => "kural",
            2 => "zamanlanmis",
            3 => "yakinsama-oncesi",
            4 => "kayip-kopya",
            5 => "ileri-kurtarma",
            6 => "ileriye-devir",
            7 => "kuyruk-canlilik",
            8 => "bos-hat",
            _ => "?",
        };
        match self {
            Self::NewData => "yeni".to_string(),
            Self::Arq => "arq".to_string(),
            Self::PtoProbe => "pto".to_string(),
            Self::Copy(c) => format!("kopya/{}", src(c)),
            Self::CopyPriority(c) => format!("kopya-oncelik/{}", src(c)),
            Self::CopyEstablish(c) => format!("kopya-kurulus/{}", src(c)),
            Self::FecParity => "fec".to_string(),
            Self::Unknown(k) => format!("bilinmiyor/{k}"),
        }
    }
}

/// [BUA · DEFTER] İletimin yükü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Payload {
    Ptt,
    Text,
    File,
    Fec,
}

/// [BUA · DEFTER] Bir (sınıf, yük) hücresinin darboğaz sayaçları (tel baytı = UDP + 28).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ClassBytes {
    pkts: u64,
    bytes: u64,
    queue_drop_pkts: u64,
    queue_drop_bytes: u64,
    lost_pkts: u64,
    lost_bytes: u64,
}

/// [BUA] Bir iletimin darboğaza verilen meta verisi.
#[derive(Debug, Clone, Copy)]
struct TxMeta {
    key_class: u8,
    key_id: u64,
    class: TxClass,
    payload: Payload,
}

/// [BUA · DEFTER] Hat modelinin sayaçları (tel baytı).
#[derive(Debug, Clone, Copy, Default)]
struct LineCounters {
    /// Darboğaza sunulan.
    offered_bytes: u64,
    offered_pkts: u64,
    /// Kuyruk dolu: düştü.
    queue_drop_bytes: u64,
    /// Serileşti.
    serialized_bytes: u64,
    serialized_pkts: u64,
    /// Serileşti, hatta düştü (rastgele ya da hedefli kayıp).
    lost_bytes: u64,
    /// Serileşti ve yayılıma girdi.
    delivered_bytes: u64,
    /// FIFO en büyük doluluk (serileşen dahil; paket, bayt).
    max_occ_pkts: u64,
    max_occ_bytes: u64,
    /// En büyük kuyruk beklemesi (serileşme başlangıcı − sunum).
    max_wait_ns: u64,
}

impl Scenario {
    fn honest(seed: u64) -> Self {
        Self {
            fwd_plan: vec![(0, PROFILE_UP)],
            rev_delay_plan: vec![(0, ONE_WAY_NS)],
            loss_permille: 200,
            queue_pkts: 64,
            reorder_permille: 0,
            targeted_drop: None,
            file_units: 41,
            file_at_ns: 0,
            file_gap_ns: 0,
            voice_until_ns: 0,
            text_at_ns: None,
            copies: PRODUCT_COPIES,
            spacing_ns: PRODUCT_SPACING_NS,
            copy_scale_min: Some(PRODUCT_MIN_COPIES),
            voice_unit: VOICE_UNIT,
            ack_return_bw: PROFILE_DOWN,
            interval: true,
            no_interval: None,
            diag: false,
            raw_copy_rule: false,
            ptt_deadline_ns: None,
            text_period_ns: 0,
            rx: RxModel::Mirror,
            loss: LossModel::Lcg,
            ledger: false,
            pair_probe: None,
            feedback_ns_override: None,
            gate_diag: false,
            bucket_burst: None,
            file_unit: None,
            text_unit: None,
            text_until_ns: None,
            seed,
            t_max_ns: 60 * SEC,
        }
    }
}

/// [K2] Hız çizelgesinin `[from, to)` aralığındaki integrali (bayt × 10⁹).
fn plan_fill(plan: &[(u64, u64)], from: u64, to: u64) -> u128 {
    let rate_at = |t: u64| {
        plan.iter()
            .rev()
            .find(|&&(a, _)| a <= t)
            .map_or(plan[0].1, |&(_, r)| r)
    };
    let mut acc = 0u128;
    let mut t = from;
    while t < to {
        let next = plan
            .iter()
            .map(|&(a, _)| a)
            .filter(|&a| a > t)
            .min()
            .unwrap_or(u64::MAX)
            .min(to);
        acc += u128::from(rate_at(t)) * u128::from(next - t);
        t = next;
    }
    acc
}

struct Bottleneck {
    plan: Vec<(u64, u64)>,
    queue_pkts: usize,
    loss_permille: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64, u64)>,
    lcg: u64,
    dropped_queue: Vec<u64>,
    waits: Vec<(u64, u64)>,
    targeted: Option<(u64, u32)>,
    tx_of_unit: HashMap<u64, u32>,
    /// PTT iletimleri (`unit_id < TEXT_ID`) darboğazda.
    ptt_log: Vec<PttTx>,
    /// Kuyruğa giren her iletimin serileşmesinin başladığı an (seq → ns).
    ser_start: HashMap<u64, u64>,
    /// Metin iletimleri (`TEXT_ID ≤ unit_id < FILE_BASE_ID`) darboğazda (kuyrukta düşenler dahil).
    text_log: Vec<PttTx>,
    /// [BUA] Kayıp kipi ve anahtarlı kipte (sınıf, kimlik) başına iletim sırası.
    mode: LossModel,
    seed: u64,
    ordinal: HashMap<(u8, u64), u32>,
    /// [BUA · DEFTER] `busy` ile aynı sırada: kuyruktaki iletimlerin tel baytı ve toplamı.
    busy_bytes: VecDeque<u64>,
    busy_bytes_sum: u64,
    /// [BUA · DEFTER] Sınıf × yük sayaçları ve hat sayaçları.
    book: BTreeMap<(TxClass, Payload), ClassBytes>,
    line: LineCounters,
    /// [KAPI] Her iletimin akıbeti (`Scenario::gate_diag`; kapalıyken boş kalır).
    track_fates: bool,
    fates: Vec<GateTx>,
    /// [K2] Jeton kovası: burst (B, IP boyu); `None` = serileştiren FIFO.
    bucket: Option<u64>,
    /// [K2] Kovadaki jeton (bayt × 10⁹) `tb_at` anında; `tb_at` = son çıkışın anı (başta 0, kova dolu).
    tb_tokens: u128,
    tb_at: u64,
    /// [K2] Darboğazdan geçen her iletim: (giriş, çıkış, IP boyu, çıkış anındaki hat hızı). Çıkış:
    /// FIFO'da serileşmenin bitişi, kovada jetonun yettiği an. Kuyrukta düşen iletim yazılmaz.
    passes: Vec<(u64, u64, u64, u64)>,
    /// [K2] Kovada burst'ten büyük olduğu için düşen datagram (gerçek tbf de düşürür).
    oversize_drops: u64,
}

/// [KAPI] Bir iletimin darboğazdaki akıbeti.
#[derive(Debug, Clone, Copy)]
struct GateTx {
    seq: u64,
    /// Göndericinin iletimi verdiği an.
    at: u64,
    /// `V` alıcıya varır, `K` hatta düştü, `Q` kuyrukta düştü.
    fate: char,
}

/// Bir PTT iletiminin darboğazdaki kaydı.
#[derive(Debug, Clone, Copy)]
struct PttTx {
    /// Göndericinin iletimi verdiği an.
    at: u64,
    seq: u64,
    unit: u64,
    /// Serileşmenin bittiği an; kuyrukta düştüyse `None`.
    done: Option<u64>,
    /// Hatta düşmedi (alıcıya varır).
    delivered: bool,
}

impl Bottleneck {
    /// Senaryonun ileri hattı (boş kuyruk; kovalı kipte kova dolu).
    fn new(sc: &Scenario) -> Self {
        Self {
            plan: sc.fwd_plan.clone(),
            queue_pkts: sc.queue_pkts,
            loss_permille: sc.loss_permille,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            lcg: sc
                .seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            dropped_queue: Vec::new(),
            waits: Vec::new(),
            targeted: sc.targeted_drop,
            tx_of_unit: HashMap::new(),
            ptt_log: Vec::new(),
            ser_start: HashMap::new(),
            text_log: Vec::new(),
            mode: sc.loss,
            seed: sc.seed,
            ordinal: HashMap::new(),
            busy_bytes: VecDeque::new(),
            busy_bytes_sum: 0,
            book: BTreeMap::new(),
            line: LineCounters::default(),
            track_fates: sc.gate_diag,
            fates: Vec::new(),
            bucket: sc.bucket_burst,
            tb_tokens: u128::from(sc.bucket_burst.unwrap_or(0)) * u128::from(SEC),
            tb_at: 0,
            passes: Vec::new(),
            oversize_drops: 0,
        }
    }

    /// [K2] Hız çizelgesinde `t`'den sonraki ilk değişim anı (yoksa `u64::MAX`).
    fn next_change(&self, t: u64) -> u64 {
        self.plan
            .iter()
            .map(|&(from, _)| from)
            .filter(|&from| from > t)
            .min()
            .unwrap_or(u64::MAX)
    }

    /// [K2] `[from, to)` aralığında dolan jeton (bayt × 10⁹): hız çizelgesinin integrali (hız
    /// değişince dolma hızı o andan itibaren yeni hız).
    fn tb_fill(&self, from: u64, to: u64) -> u128 {
        plan_fill(&self.plan, from, to)
    }

    /// [K2] Jeton kovası (tbf), paket tanecikli: sıradaki datagram kovada IP boyu kadar jeton
    /// olunca çıkar. Jeton hat hızıyla dolar ve yalnız kuyruk boşken (`now` son çıkıştan
    /// sonraysa) burst'te doyar; kuyruk doluyken baştaki datagram jetonu burst'e varmadan tüketir
    /// (IP boyu ≤ burst). `now` anında giren `ip` baytlık datagramın çıkış anını döner ve kovanın
    /// durumunu o ana taşır.
    fn tb_exit(&mut self, now: u64, ip: u64, burst: u64) -> u64 {
        let cap = u128::from(burst) * u128::from(SEC);
        let need = u128::from(ip) * u128::from(SEC);
        let head = now.max(self.tb_at);
        let mut tok = (self.tb_tokens + self.tb_fill(self.tb_at, head)).min(cap);
        let mut t = head;
        while tok < need {
            let rate = u128::from(self.rate_at(t).max(1));
            let boundary = self.next_change(t);
            let dt = u64::try_from((need - tok).div_ceil(rate)).unwrap_or(u64::MAX);
            if dt <= boundary - t {
                tok += rate * u128::from(dt);
                t += dt;
            } else {
                tok += rate * u128::from(boundary - t);
                t = boundary;
            }
        }
        self.tb_tokens = tok - need;
        self.tb_at = t;
        t
    }

    fn rate_at(&self, t: u64) -> u64 {
        self.plan
            .iter()
            .rev()
            .find(|&&(from, _)| from <= t)
            .map_or(self.plan[0].1, |&(_, r)| r)
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    fn book(&mut self, meta: TxMeta) -> &mut ClassBytes {
        self.book.entry((meta.class, meta.payload)).or_default()
    }

    fn send(&mut self, now: u64, seq: u64, udp: u64, unit_id: u64, meta: TxMeta) {
        let wire = udp + OVH;
        self.line.offered_bytes += wire;
        self.line.offered_pkts += 1;
        {
            let b = self.book(meta);
            b.pkts += 1;
            b.bytes += wire;
        }
        // [BUA · ANAHTARLI-KAYIP] Sıra iletim kararında sayılır (kuyrukta düşen de).
        let ordinal = match self.mode {
            LossModel::Lcg => 0,
            LossModel::Keyed => {
                let n = self
                    .ordinal
                    .entry((meta.key_class, meta.key_id))
                    .or_insert(0);
                let o = *n;
                *n += 1;
                o
            }
        };
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
            self.busy_bytes_sum -= self.busy_bytes.pop_front().unwrap_or(0);
        }
        // [K2] Kovada burst'ten büyük datagram hiç geçemez (gerçek tbf girişte düşürür).
        let oversize = self.bucket.is_some_and(|b| wire > b);
        if oversize {
            self.oversize_drops += 1;
        }
        if self.busy.len() >= self.queue_pkts || oversize {
            self.line.queue_drop_bytes += wire;
            {
                let b = self.book(meta);
                b.queue_drop_pkts += 1;
                b.queue_drop_bytes += wire;
            }
            self.dropped_queue.push(now);
            if self.track_fates {
                self.fates.push(GateTx {
                    seq,
                    at: now,
                    fate: 'Q',
                });
            }
            if unit_id < TEXT_ID {
                self.ptt_log.push(PttTx {
                    at: now,
                    seq,
                    unit: unit_id,
                    done: None,
                    delivered: false,
                });
            } else if unit_id < FILE_BASE_ID {
                self.text_log.push(PttTx {
                    at: now,
                    seq,
                    unit: unit_id,
                    done: None,
                    delivered: false,
                });
            }
            return;
        }
        // [K2] Kovalı kipte "serileşme" yoktur: `start` = `done` = jetonun yettiği an (çıkış).
        let (start, done) = match self.bucket {
            None => {
                let start = self.busy.back().copied().unwrap_or(now).max(now);
                (start, start + (udp + OVH) * SEC / self.rate_at(start))
            }
            Some(burst) => {
                let exit = self.tb_exit(now, wire, burst);
                (exit, exit)
            }
        };
        self.passes.push((now, done, wire, self.rate_at(done)));
        self.ser_start.insert(seq, start);
        self.busy.push_back(done);
        self.busy_bytes.push_back(wire);
        self.busy_bytes_sum += wire;
        self.line.max_occ_pkts = self.line.max_occ_pkts.max(self.busy.len() as u64);
        self.line.max_occ_bytes = self.line.max_occ_bytes.max(self.busy_bytes_sum);
        self.line.max_wait_ns = self.line.max_wait_ns.max(start - now);
        self.line.serialized_bytes += wire;
        self.line.serialized_pkts += 1;
        self.waits.push((now, start - now));
        let n = self.tx_of_unit.entry(unit_id).or_insert(0);
        *n += 1;
        let targeted = self.targeted.is_some_and(|(u, k)| u == unit_id && *n <= k);
        let lost = match self.mode {
            LossModel::Lcg => self.rand_permille() < self.loss_permille,
            LossModel::Keyed => {
                self.loss_permille > 0
                    && keyed_permille(self.seed, meta.key_class, meta.key_id, ordinal)
                        < self.loss_permille
            }
        };
        let rec = PttTx {
            at: now,
            seq,
            unit: unit_id,
            done: Some(done),
            delivered: !(lost || targeted),
        };
        if unit_id < TEXT_ID {
            self.ptt_log.push(rec);
        } else if unit_id < FILE_BASE_ID {
            self.text_log.push(rec);
        }
        if self.track_fates {
            self.fates.push(GateTx {
                seq,
                at: now,
                fate: if lost || targeted { 'K' } else { 'V' },
            });
        }
        if lost || targeted {
            self.line.lost_bytes += wire;
            let b = self.book(meta);
            b.lost_pkts += 1;
            b.lost_bytes += wire;
            return;
        }
        self.line.delivered_bytes += wire;
        self.wire.push_back((done + ONE_WAY_NS, seq, unit_id, udp));
    }
}

#[derive(Default)]
struct TxCount {
    datagrams: u64,
    /// Verilen en büyük seq (bırakma anının sınırı).
    last_seq: u64,
    ptt_wire_bytes: u64,
    /// PTT iletimlerinin türü (seq → tür).
    ptt_kind: HashMap<u64, TransmitKind>,
    /// [BUA · DEFTER] Defter açık: göndericinin emisyon izi her olaydan sonra boşaltılır.
    ledger: bool,
    /// [BUA · DEFTER] Emisyon nedeni (seq → (neden, ref_seq)) ve izin tamamı (sırayla).
    causes: HashMap<u64, (EmitCause, u64)>,
    emit_all: Vec<EmitDiag>,
    /// [BUA · DEFTER] Darboğaza verilen bütün iletimlerin tel baytı (bağımsız sayaç) ve bunların
    /// `unit_id < TEXT_ID` olan FEC paritesi payı (`ptt_wire_bytes` onu da sayar).
    wire_bytes: u64,
    fec_low_bytes: u64,
    /// [BUA · ÜRÜN ALICISI] Verilen seq'ler (ürün çalışma zamanının PN → motor seq tablosu;
    /// düzenekte PN = seq).
    track_sent: bool,
    sent: BTreeSet<u64>,
    any_sent: bool,
    /// [KAPI] Bu adımda darboğaza verilen datagram / tel baytı ve bunların öncelikli şerit payı
    /// (PTT, metin; FEC paritesi hariç). Her adımın sonunda sıfırlanır.
    step_pkts: u64,
    step_wire: u64,
    step_pri_pkts: u64,
    step_pri_wire: u64,
}

/// Emisyon nedeni olan iz kayıtları (kayıp ilanı, teyit, ACK çerçevesi değil).
fn is_emission(c: EmitCause) -> bool {
    matches!(
        c,
        EmitCause::Data
            | EmitCause::Retransmit
            | EmitCause::PtoProbe
            | EmitCause::Copy
            | EmitCause::CopyPriority
            | EmitCause::CopyEstablish
            | EmitCause::FecParity
    )
}

impl TxCount {
    /// [BUA · DEFTER] Göndericinin izini boşaltır (defter kapalıyken hiçbir şey yapmaz).
    fn drain(&mut self, s: &mut Sender) {
        if !self.ledger {
            return;
        }
        for e in s.take_diag_emit_log() {
            if is_emission(e.cause) {
                self.causes.insert(e.seq, (e.cause, e.ref_seq));
            }
            self.emit_all.push(e);
        }
    }

    /// [BUA] İletimin anahtarı, sınıfı ve yükü.
    fn meta(&self, d: &TransmitDecision) -> TxMeta {
        let payload = if d.kind == TransmitKind::FecParity {
            Payload::Fec
        } else if d.unit_id < TEXT_ID {
            Payload::Ptt
        } else if d.unit_id < FILE_BASE_ID {
            Payload::Text
        } else {
            Payload::File
        };
        let (key_class, key_id) = match d.send_record.map(|r| r.logical_ref) {
            Some(SrLogicalRef::Fec { block_id, esi }) if d.kind == TransmitKind::FecParity => {
                (KEY_CLASS_FEC, u64::from(block_id) << 16 | u64::from(esi))
            }
            _ => (KEY_CLASS_UNIT, d.unit_id),
        };
        let by_kind = |k: TransmitKind| match k {
            TransmitKind::Data => "data",
            TransmitKind::Retransmit => "retx",
            TransmitKind::Duplicate => "kopya",
            TransmitKind::FecParity => "fec",
            TransmitKind::CsProbe => "cs",
        };
        let class = match self.causes.get(&d.seq) {
            Some(&(EmitCause::Data, _)) => TxClass::NewData,
            Some(&(EmitCause::Retransmit, _)) => TxClass::Arq,
            Some(&(EmitCause::PtoProbe, _)) => TxClass::PtoProbe,
            Some(&(EmitCause::Copy, r)) => TxClass::Copy(r),
            Some(&(EmitCause::CopyPriority, r)) => TxClass::CopyPriority(r),
            Some(&(EmitCause::CopyEstablish, r)) => TxClass::CopyEstablish(r),
            Some(&(EmitCause::FecParity, _)) => TxClass::FecParity,
            _ if self.ledger => TxClass::Unknown(by_kind(d.kind)),
            _ => match d.kind {
                TransmitKind::Data => TxClass::NewData,
                TransmitKind::Retransmit => TxClass::Arq,
                TransmitKind::FecParity => TxClass::FecParity,
                _ => TxClass::Copy(0),
            },
        };
        TxMeta {
            key_class,
            key_id,
            class,
            payload,
        }
    }
}

struct ReturnLine {
    plan: Vec<(u64, u64)>,
    free_at: u64,
    last_arrival: u64,
    /// (varış, UDP yükü, ileti).
    wire: VecDeque<(u64, u64, RetMsg)>,
    /// Her ACK: (alıcının verdiği an, serileşmenin başladığı an).
    log: Vec<(u64, u64)>,
    /// [BUA] Çerçeve türü baytı → sayı; dönüş hattına verilen toplam tel baytı (UDP + 28).
    types: BTreeMap<u8, u64>,
    bytes: u64,
}

/// Dönüş hattında taşınan ACK.
enum RetMsg {
    /// [`RxModel::Mirror`]: 0x14 çerçevesinin çözülmüş hâli + `RxMirror` ölçümü.
    Mirror {
        arrival_rate: u16,
        ack: AckInfo,
        iv: IntervalMsg,
    },
    /// [`RxModel::Product`]: kodlanmış çerçeve; `measured = false` → `no_interval` aralığında
    /// çıktı (ölçüm göndericiye verilmez).
    Product { bytes: Vec<u8>, measured: bool },
}

/// Ölçüm aralığı (sözleşme §2) ve çift alt sınırı: (m_first, largest, rx_span, M, pair_rate).
type IntervalMsg = (u64, u64, Option<u64>, Vec<u64>, u16);

/// [İŞ-35 GECE HİZASI] Alıcının 0x15 alanlarının kuralı (`receiver.rs`, alıcı `482ef1e1` +
/// `df0630c3`), düzeneğin 0x14 alıcısının yanında aynı varışlarla üretilir:
/// * ölçüm aralığı `[span_first, largest]` (`Receiver::rx_span_field`): `span_first` = son çıkan
///   ACK'in en büyüğünden sonraki ilk alınan seq (bir ACK geriye; çıkan ACK yoksa bitmap tabanından,
///   `bitmap_base`: iki ACK öncesinin largest'ı + 1, `ACK_BITMAP_MAX_BITS` ile kırpılmış);
///   `rx_span = ilk_varış(largest) − ilk_varış(span_first)`, 256 µs'ye aşağı; `span_first ≥ largest`
///   ya da 0xFFFE birimi aşan süre → ölçüm yok (span_first = largest). M = alınan seq'lerin bu
///   aralıktakileri (ürün çalışma zamanı: çerçevenin teyit ettiği PN'ler `[span_first, largest]`);
/// * çift alt sınırı (`receiver::PairRate`): her varışta (kopya dahil) önceki varışa aralık `g` ve
///   bu datagramın UDP gövdesi `b`; `b ≥ PAIR_MIN_UDP_BYTES` ve `g ≥ 2 ms` ise `b / g`; alan son
///   ACK'ten bu yana en büyük İKİNCİ değer (yoksa `PAIR_RATE_NONE`), ACK'te sıfırlanır.
///   [SIKIŞMA · E · alıcı 4af53e94] Her varışta `g < 0,9 × (b + 28) / beyan_hat` ise çift sıkışmıştır:
///   o çift ve ARDINDAKİ çift sayılmaz (karar varış başına; `b`/`g` alt sınırlarından önce,
///   ACK'te sıfırlanmaz; `PairRate::squeezed`, `on_arrival`). Benzetimde beyan hat = [`PROFILE_UP`]
///   (ürünün sunucu yuvası `set_pair_squeeze_line(downlink_bps)`; düzenekte profil hep açık).
///
/// Alıcının 0x15 çerçevesinin kendisi (boyu, daralma alarmının erken ACK'i) bu kipte yoktur: ACK
/// kadansı 0x14 alıcısınındır (MELEZ). Gerçek 0x15 alıcısı [`RxModel::Product`] seçeneğindedir
/// (modül belgesi, [BUA]). (Gerçek alıcı bitmap kipinde bu düzenekte bir ara yeniden sıralamada
/// panik veriyordu: en büyüğü ilerlemeyen ACK'te `span_first` aralığı `largest + 1 ..= largest`;
/// `Receiver::rx_span_field` bugün o durumda ölçümsüz döner.)
#[derive(Default)]
struct RxMirror {
    first_arrival: HashMap<u64, u64>,
    received: std::collections::BTreeSet<u64>,
    /// Son iki çıkan ACK'in largest'ı (alıcının `bitmap_emitted`).
    emitted: VecDeque<u64>,
    pair_prev: Option<u64>,
    pair_top: [u64; 2],
    /// [SIKIŞMA · E] Önceki varışın çifti sıkışmıştı (alıcının `PairRate::after_squeeze`).
    pair_after_squeeze: bool,
}

/// [SIKIŞMA · E] Sıkışma sınamasının payı (binde); `receiver.rs` `PAIR_SQUEEZE_PERMILLE` ile aynı
/// (orada crate içi sabit).
const PAIR_SQUEEZE_PERMILLE: u64 = 900;

/// [SIKIŞMA · E] Sıkışma sınamasının datagram zarfı (B, IPv4 + UDP); `receiver.rs`
/// `PAIR_SQUEEZE_OVERHEAD` ile aynı (orada crate içi sabit).
const PAIR_SQUEEZE_OVERHEAD: u64 = 28;

/// [SIKIŞMA · E] `receiver::PairRate::squeezed`: aralık `g` (ns) beyan hatta (`PROFILE_UP`, tel B/s)
/// `b + 28` baytın serileşmesinin binde 900'ünden kısa mı.
fn pair_squeezed(g: u64, b: u64) -> bool {
    u128::from(g) * 1_000 * u128::from(PROFILE_UP)
        < u128::from(PAIR_SQUEEZE_PERMILLE)
            * u128::from(b.saturating_add(PAIR_SQUEEZE_OVERHEAD))
            * u128::from(SEC)
}

impl RxMirror {
    fn on_arrival(&mut self, seq: u64, udp: u64, now: u64) {
        if let Some(p) = self.pair_prev {
            let g = now.saturating_sub(p);
            // [SIKIŞMA · E] Sıkışmış çift ve ardındaki çift sayılmaz.
            let squeezed = pair_squeezed(g, udp);
            let skip = squeezed || self.pair_after_squeeze;
            self.pair_after_squeeze = squeezed;
            if !skip && udp >= PAIR_MIN_UDP_BYTES && g >= 2 * MS {
                let r = udp * SEC / g;
                if r > self.pair_top[0] {
                    self.pair_top = [r, self.pair_top[0]];
                } else if r > self.pair_top[1] {
                    self.pair_top[1] = r;
                }
            }
        }
        self.pair_prev = Some(now);
        self.first_arrival.entry(seq).or_insert(now);
        self.received.insert(seq);
    }

    /// Alıcı `largest` en büyüklü bir ACK çıkardı: (span_first, largest, rx_span, M, pair_rate).
    fn on_ack(&mut self, largest: u64) -> (u64, u64, Option<u64>, Vec<u64>, u16) {
        let base = if self.emitted.len() >= 2 {
            self.emitted.front().map_or(0, |&l| l + 1)
        } else {
            0
        }
        .max(largest.saturating_sub(ACK_BITMAP_MAX_BITS));
        let anchor = self.emitted.back().map_or(base, |&l| l + 1).max(base);
        let first = (anchor <= largest)
            .then(|| self.received.range(anchor..=largest).next().copied())
            .flatten()
            .filter(|&f| f < largest);
        let span = first.and_then(|f| {
            let d = self.first_arrival[&largest].saturating_sub(self.first_arrival[&f]);
            let u = d / (ACK_DELAY_UNIT_US * 1000);
            (u < u64::from(RX_SPAN_NONE)).then_some(u * ACK_DELAY_UNIT_US * 1000)
        });
        let first = if span.is_some() {
            first.unwrap_or(largest)
        } else {
            largest
        };
        let m: Vec<u64> = self.received.range(first..=largest).copied().collect();
        let pair = if self.pair_top[1] == 0 {
            PAIR_RATE_NONE
        } else {
            u16::try_from(self.pair_top[1].min(u64::from(PAIR_RATE_NONE) - 1)).unwrap_or(0)
        };
        self.pair_top = [0, 0];
        if self.emitted.len() >= 2 {
            self.emitted.pop_front();
        }
        self.emitted.push_back(largest);
        (first, largest, span, m, pair)
    }
}

impl ReturnLine {
    fn delay_at(&self, t: u64) -> u64 {
        self.plan
            .iter()
            .rev()
            .find(|&&(from, _)| from <= t)
            .map_or(self.plan[0].1, |&(_, d)| d)
    }

    fn send(&mut self, now: u64, udp: u64, frame_type: u8, msg: RetMsg) {
        let start = self.free_at.max(now);
        self.log.push((now, start));
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        let arrival = (self.free_at + self.delay_at(self.free_at)).max(self.last_arrival);
        self.last_arrival = arrival;
        *self.types.entry(frame_type).or_insert(0) += 1;
        self.bytes += udp + OVH;
        self.wire.push_back((arrival, udp, msg));
    }

    /// [BUA · ÜRÜN ALICISI] Alıcının ürettiği çerçeve o anda kodlanır ve mühürlü boyla hatta
    /// verilir (ürün çalışma zamanı ACK'i üretildiği yerde mühürleyip gönderir).
    fn send_product(&mut self, now: u64, frame: &AckFrame, measured: bool) {
        let bytes = frame.encode();
        let udp = bytes.len() as u64 + ACK_SEAL;
        let t = bytes.first().copied().unwrap_or(0);
        self.send(now, udp, t, RetMsg::Product { bytes, measured });
    }
}

#[derive(Debug)]
struct Outcome {
    file_done_ns: Option<u64>,
    /// (gönderim anı, darboğaz beklemesi).
    waits: Vec<(u64, u64)>,
    drops_queue: usize,
    /// Kuyrukta düşmelerin anları (göndericinin hatta verdiği an).
    drop_times: Vec<u64>,
    /// Korumanın hız tahmininin değiştiği anlar: (an, tahmin).
    rate_trace: Vec<(u64, Option<u64>)>,
    guard: Option<(Option<u64>, u64)>,
    /// Demet başına (plan, sıralı teslim gecikmesi); teslim edilmeyen `None`.
    voice: Vec<(u64, Option<u64>)>,
    text_delay_ns: Option<u64>,
    ptt_wire_bytes: u64,
    acks_at: Vec<u64>,
    /// `acks_at` ile aynı sırada: ACK ölçüm aralığı taşıyor muydu.
    ack_measured: Vec<bool>,
    interval: Option<(u64, u64, Option<i64>)>,
    /// Gönderim tavanının değiştiği anlar: (an, tavan B/s).
    pace_trace: Vec<(u64, u64)>,
    /// PTT iletimleri (darboğaz kaydı + göndericinin verdiği tür).
    ptt_tx: Vec<(PttTx, TransmitKind)>,
    /// ACK'ler varış sırasıyla.
    acks: Vec<AckRec>,
    /// Demet başına ilk varış (alıcı); varmayan `None`.
    voice_first: Vec<Option<u64>>,
    /// Göndericinin izi (`Scenario::diag`).
    emit_log: Vec<EmitDiag>,
    /// ACK başına (varış, en büyük teyitli seq'in darboğazda serileşme başlangıcı, korunan hız,
    /// gönderim tavanı), varış sırasıyla.
    per_ack: Vec<(u64, u64, Option<u64>, u64)>,
    /// [PTT-YALIN] Bırakılan PTT birimleri: (birim, an, bırakmadan önce verilen en büyük seq).
    ptt_expired: Vec<(u64, u64, u64)>,
    /// Metin birimleri: (sunum, teslim gecikmesi).
    text_delays: Vec<(u64, Option<u64>)>,
    /// Metin iletimleri (darboğaz kaydı).
    text_tx: Vec<PttTx>,
    /// [BUA · DEFTER] Sınıf × yük sayaçları, hat sayaçları (defter kapalıyken de sayılır; sınıf
    /// yalnız defter açıkken emisyon nedenidir).
    book: BTreeMap<(TxClass, Payload), ClassBytes>,
    line: LineCounters,
    /// [BUA · DEFTER] Gönderim tavanının izi: (an, tavan B/s, göndericiye varan kaçıncı ACK).
    ceil_trace: Vec<(u64, u64, usize)>,
    /// [BUA · DEFTER] (an, gönderilmemiş birim, uçuştaki bayt), saniyede bir (defter açıkken).
    backlog: Vec<(u64, usize, u64)>,
    /// [BUA] Dönüş hattı: çerçeve türü → sayı, toplam tel baytı.
    ack_types: BTreeMap<u8, u64>,
    ack_wire_bytes: u64,
    /// [KAPI] ACK başına gözlem satırı, varış sırasıyla (`Scenario::gate_diag`).
    gate_acks: Vec<GateAck>,
    /// [KAPI] Adım başına iz (`Scenario::gate_diag`).
    gate_steps: Vec<GateStep>,
    /// [KAPI] Her iletimin darboğazdaki akıbeti, veriliş sırasıyla (`Scenario::gate_diag`).
    gate_tx: Vec<GateTx>,
    /// [K2] Darboğazdan geçen her iletim: (giriş, çıkış, IP boyu, çıkış anındaki hat hızı).
    passes: Vec<(u64, u64, u64, u64)>,
    /// [K2] Kovada burst'ten büyük olduğu için düşen datagram.
    oversize_drops: u64,
}

/// [KAPI] Bir ACK'in gözlem satırı.
#[derive(Debug, Clone, Copy)]
struct GateAck {
    /// Göndericiye varan kaçıncı ACK (1'den).
    index: usize,
    arrival: u64,
    /// En büyük teyitli seq'in darboğazda serileşmesinin başladığı an (sayılan ACK tanımı).
    ser_start: u64,
    /// ACK ölçüm aralığı taşıyor.
    measured: bool,
    /// Çerçevenin `pair_rate` alanı (B/s, UDP gövdesi); `None` = ölçüm yok.
    pair_rate: Option<u64>,
    /// ACK göndericiye verilmeden hemen önce / `Event::AckReceived` döndükten hemen sonra (olayın
    /// kendi gönderimleri dahil).
    before: FlightGateDiag,
    after: FlightGateDiag,
    unsent_before: usize,
    unsent_after: usize,
    /// Olayın dönüşünde darboğaza verilen datagram / tel baytı.
    emitted_pkts: u64,
    emitted_wire: u64,
    /// Korumanın bu ACK için yazdığı kayıt; yazılmadıysa `None`.
    trace: Option<LineGuardAckTrace>,
    /// [K2] Uçuş kapısının gönderim denemesi ret sayacı (`Sender::diag_gate_refusals`, kümülatif)
    /// ve son reddi `(boy, uçuş, etkin pencere)`, olay döndükten sonra.
    gate_refused: u64,
    gate_last: Option<(u64, u64, u64)>,
    /// [K2] Korumanın başlattığı çift yoklaması sayısı (`Sender::diag_pair_probes`), olay
    /// döndükten sonra.
    pair_probes: u64,
    /// [K2] Alıcının bu ACK'i çıkarırken kapattığı aralığın çift sayımı ([`PairMirror`]); ürün
    /// alıcısı değilse `None`.
    pair_obs: Option<PairObs>,
}

/// [K2] Ürün alıcısının çift kuralının (`receiver.rs` `PairRate` + daralma alarmının saklama /
/// bırakma adımları) aynası: aynı varışlarla, alıcının yanında yürür. Alarm olayları alıcının var
/// olan gözlem sayaçlarından okunur (`diag_drop_detects`: daralma algısı, en büyük iki değer
/// saklanır ve sıfırlanır; `diag_drop_alarms`: kanıtlı bırakma, saklanan atılır). Her ACK'te
/// aynanın alanı çerçevenin `pair_rate` alanıyla karşılaştırılır ([`PairObs::matches`]).
#[derive(Debug, Default, Clone, Copy)]
struct PairMirror {
    prev: Option<u64>,
    top: [u64; 2],
    saved: [u64; 2],
    after_squeeze: bool,
    /// Alarm sıfırlamasından etkilenmeyen kopya (yalnız ACK'te sıfırlanır).
    top_all: [u64; 2],
    counted: u64,
    squeezed: u64,
    skipped: u64,
    resets: u64,
    erased: u64,
}

/// [K2] Bir ACK aralığının çift sayımı ([`PairMirror`]).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PairObs {
    /// Sayılan çift (sıkışmamış, sıkışmış çiftin ardında değil, `b ≥ PAIR_MIN_UDP_BYTES`, `g ≥ 2 ms`).
    counted: u64,
    /// Sıkışmış sayılan çift (`g < 0,9 × (b + 28) / beyan hat`).
    squeezed: u64,
    /// Sıkışmış çiftin ardında olduğu için atlanan çift.
    skipped: u64,
    /// Çift kaydını sıfırlayan olay: daralma algısı (saklar ve sıfırlar) + kanıtlı bırakma
    /// (saklananı atar).
    resets: u64,
    /// Bu olaylarla alana giremeyen sayılmış değer (kanıtlı bırakmada atılan ya da ikinci algıda
    /// üstüne yazılan saklı değer).
    erased: u64,
    /// Sıfırlama olmasaydı alanın değeri (B/s, UDP gövdesi); iki sayılan çift yoksa `None`.
    no_reset: Option<u64>,
    /// Aynanın alanı çerçevenin `pair_rate` alanıyla aynı.
    matches: bool,
}

fn top2_push(top: &mut [u64; 2], r: u64) {
    if r > top[0] {
        *top = [r, top[0]];
    } else if r > top[1] {
        top[1] = r;
    }
}

fn top2_field(top: [u64; 2]) -> u16 {
    if top[1] == 0 {
        return PAIR_RATE_NONE;
    }
    u16::try_from(top[1].min(u64::from(PAIR_RATE_NONE) - 1)).unwrap_or(PAIR_RATE_NONE - 1)
}

impl PairMirror {
    /// `receiver::PairRate::on_arrival` (beyan hat [`PROFILE_UP`]).
    fn on_arrival(&mut self, now: u64, udp: u64) {
        if let Some(p) = self.prev {
            let g = now.saturating_sub(p);
            let squeezed = pair_squeezed(g, udp);
            let skip = squeezed || self.after_squeeze;
            if squeezed {
                self.squeezed += 1;
            } else if self.after_squeeze {
                self.skipped += 1;
            }
            self.after_squeeze = squeezed;
            if !skip && udp >= PAIR_MIN_UDP_BYTES && g >= 2 * MS {
                let r = u64::try_from(u128::from(udp) * u128::from(SEC) / u128::from(g))
                    .unwrap_or(u64::MAX);
                self.counted += 1;
                top2_push(&mut self.top, r);
                top2_push(&mut self.top_all, r);
            }
        }
        self.prev = Some(now);
    }

    /// Daralma algısı (`restart_at_drop_alarm`): en büyük iki değer saklanır ve sıfırlanır.
    fn on_detect(&mut self) {
        self.resets += 1;
        self.erased += self.saved.iter().filter(|&&v| v > 0).count() as u64;
        self.saved = std::mem::take(&mut self.top);
    }

    /// Kanıtlı bırakma: saklanan değerler atılır.
    fn on_evidence(&mut self) {
        self.resets += 1;
        self.erased += self.saved.iter().filter(|&&v| v > 0).count() as u64;
        self.saved = [0, 0];
    }

    /// ACK çıktı: alan = en büyük iki ile saklananın birleşiminin ikincisi (`field_with`), sonra
    /// sıfırlanır (`restore` + `on_ack`).
    fn on_ack(&mut self, frame: &AckFrame) -> PairObs {
        let mut all = [self.top[0], self.top[1], self.saved[0], self.saved[1]];
        all.sort_unstable_by(|a, b| b.cmp(a));
        let field = top2_field([all[0], all[1]]);
        let no_reset = Some(top2_field(self.top_all))
            .filter(|&f| f != PAIR_RATE_NONE)
            .map(u64::from);
        let obs = PairObs {
            counted: self.counted,
            squeezed: self.squeezed,
            skipped: self.skipped,
            resets: self.resets,
            erased: self.erased,
            no_reset,
            matches: frame.bitmap.as_ref().map(|b| b.pair_rate) == Some(field),
        };
        *self = Self {
            prev: self.prev,
            after_squeeze: self.after_squeeze,
            ..Self::default()
        };
        obs
    }
}

/// [KAPI] Bir adımın (1 ms) sonundaki durum.
#[derive(Debug, Clone, Copy)]
struct GateStep {
    now: u64,
    inflight: u64,
    /// Kapının penceresi (`FlightGateDiag::effective_window`), BBR penceresi (toparlanmasız) ve
    /// kuralın uçuş tabanı.
    window: u64,
    bbr: u64,
    floor: u64,
    /// Korumanın gönderim tavanı (B/s, tel).
    pace: u64,
    /// Bu adımda darboğaza verilen datagram / tel baytı; öncelikli şerit payı.
    pkts: u64,
    wire: u64,
    pri_pkts: u64,
    pri_wire: u64,
    /// Göndericide bekleyen, hiç tele çıkmamış birim.
    unsent: u64,
    /// Bu adımda göndericiye ACK verildi.
    ack: bool,
    /// [K2] Uçuş kapısının gönderim denemesi ret sayacı (kümülatif), adımın sonunda.
    gate_refused: u64,
}

/// [BUA · ÜRÜN ALICISI] `client_core::runtime` ACK kolunun göndericiye giden çağrıları, aynı
/// sırayla: `observe_ack_feedback` → `note_ack_arrival_rate` → (`note_ack_pair_rate`) →
/// `set_next_ack_evidence` → (`note_ack_forward_interval`) → [çağıran: `Event::AckReceived`].
/// Düzenekte PN = motor seq; çalışma zamanının PN → seq tablosu = verilen seq'ler (`tx.sent`).
/// `interval = false` karşı-olgu kolunun anahtarıdır (ürün çalışma zamanında yok): parantezli iki
/// çağrı yapılmaz. Döner: (ACK, en büyük teyitli seq, ACK ölçüm aralığı taşıyor mu).
fn product_ack_into_sender(
    s: &mut Sender,
    tx: &TxCount,
    interval: bool,
    bytes: &[u8],
    udp: u64,
    measured: bool,
    now: u64,
) -> (AckInfo, u64, bool) {
    let frame = AckFrame::decode_with_bitmap(bytes).expect("ürün ACK'i çözülür");
    s.observe_ack_feedback(
        AckObservation {
            wire_bytes: udp,
            seen_count: frame.seen_count,
            v2: frame.v2,
            sent_datagrams: tx.datagrams,
        },
        now,
    );
    s.note_ack_arrival_rate(frame.arrival_rate);
    if interval {
        if let Some(bm) = frame.bitmap.as_ref() {
            s.note_ack_pair_rate(bm.pair_rate);
        }
    }
    let expected = if tx.any_sent { tx.last_seq + 1 } else { 0 };
    let pns = frame.acked_full(expected);
    let in_table = |p: u64| tx.sent.contains(&p);
    let evidence = frame.evidence_span_full(expected);
    if let Some((lo, hi)) = evidence {
        let span = match (
            tx.sent.range(lo..=hi).next().copied(),
            tx.sent.range(lo..=hi).next_back().copied(),
        ) {
            (Some(a), Some(b)) => (a, b),
            _ => (1, 0),
        };
        s.set_next_ack_evidence(Some(span));
    }
    let mut has_span = false;
    if let (Some(bm), Some((lo, hi))) = (frame.bitmap.as_ref(), evidence) {
        let from = frame.span_first_full(expected).unwrap_or(lo).max(lo);
        let mut m: Vec<u64> = pns
            .iter()
            .copied()
            .filter(|&p| p >= from && p <= hi)
            .collect();
        m.sort_unstable();
        let first = m.first().copied().filter(|&p| in_table(p));
        let last = Some(hi).filter(|&p| in_table(p));
        if let (Some(first), Some(last)) = (first, last) {
            let received: Vec<u64> = m.iter().copied().filter(|&p| in_table(p)).collect();
            let span_ns = (bm.rx_span != RX_SPAN_NONE && measured)
                .then(|| u64::from(bm.rx_span) * ACK_DELAY_UNIT_US * 1000);
            has_span = span_ns.is_some();
            if interval {
                s.note_ack_forward_interval(first, last, span_ns, &received);
            }
        }
    }
    let largest = pns.iter().copied().max().unwrap_or(0);
    let acked_seqs = pns.into_iter().filter(|&p| in_table(p)).collect();
    (
        AckInfo {
            acked_seqs,
            ack_delay: frame.ack_delay_duration(),
        },
        largest,
        has_span,
    )
}

/// Bir ACK'in yolculuğu.
#[derive(Debug, Clone, Copy)]
struct AckRec {
    /// Alıcının ACK'i verdiği an.
    sent: u64,
    /// Dönüş hattında serileşmenin başladığı an.
    start: u64,
    /// Göndericiye varış.
    arrival: u64,
    /// UDP yükü (B).
    udp: u64,
    /// Çerçevenin teyit ettiği en büyük seq.
    largest: u64,
}

impl Outcome {
    fn min_rate(&self) -> Option<u64> {
        self.rate_trace.iter().filter_map(|&(_, r)| r).min()
    }

    fn rate_at_end(&self) -> Option<u64> {
        self.guard.and_then(|g| g.0)
    }

    fn voice_max_from(&self, t: u64) -> u64 {
        self.voice
            .iter()
            .filter(|&&(p, _)| p >= t)
            .map(|&(_, d)| d.unwrap_or(u64::MAX))
            .max()
            .unwrap_or(0)
    }

    /// Gönderim tavanının en küçüğü (ilk ACK'ten sonra; iz boşsa profil hattı).
    fn min_pace(&self) -> u64 {
        self.pace_trace
            .iter()
            .map(|&(_, p)| p)
            .min()
            .unwrap_or(PROFILE_UP)
    }

    /// Göndericiye üst üste gelen ölçümsüz ACK'lerin en uzun dizisi: (uzunluk, dizinin ikinci
    /// ACK'inin varışı, diziden sonraki ilk ölçülü ACK'in varışı).
    fn longest_unmeasured_run(&self) -> (usize, Option<u64>, Option<u64>) {
        let mut best = (0usize, None, None);
        let mut i = 0;
        while i < self.ack_measured.len() {
            if self.ack_measured[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < self.ack_measured.len() && !self.ack_measured[i] {
                i += 1;
            }
            if i - start > best.0 {
                best = (
                    i - start,
                    self.acks_at
                        .get(start + 1)
                        .copied()
                        .filter(|_| i - start >= 2),
                    self.acks_at.get(i).copied(),
                );
            }
        }
        best
    }

    fn drops_between(&self, from: u64, to: u64) -> usize {
        self.drop_times
            .iter()
            .filter(|&&t| t >= from && t < to)
            .count()
    }

    fn max_wait_between(&self, from: u64, to: u64) -> u64 {
        self.waits
            .iter()
            .filter(|&&(t, _)| t >= from && t < to)
            .map(|&(_, w)| w)
            .max()
            .unwrap_or(0)
    }
}

fn sender(sc: &Scenario) -> Sender {
    let mut s = Sender::with_cc_port();
    let t_ack = adaptive_transport::receiver::budgeted_ack_interval_ns(PROFILE_DOWN);
    s.set_cc_initial_window(PROFILE_UP * t_ack / SEC);
    let cap = PROFILE_UP * MTU / (MTU + OVH);
    s.set_cc_profile_pacing_cap(Some(cap));
    s.set_cc_feedback_starved(Some(FeedbackStarvedRule {
        pacing_cap_bps: cap,
        feedback_ns: sc
            .feedback_ns_override
            .unwrap_or_else(|| feedback_horizon_ns(PROFILE_DOWN)),
        line: LineModel {
            rate_bps: PROFILE_UP,
            datagram_overhead: OVH,
        },
        return_line: LineModel {
            rate_bps: PROFILE_DOWN,
            datagram_overhead: OVH,
        },
    }));
    // [KAPI] Deney anahtarı kuraldan SONRA: kural hat korumasını yeniden kurar ve anahtarı ortam
    // değişkeninden okur; ayarlayıcı kurulu korumaya yazar. `None`: çağrı yok (bugünkü yol).
    if sc.pair_probe.is_some() {
        s.set_pair_probe_datagrams(sc.pair_probe);
    }
    if sc.gate_diag {
        s.set_diag_line_guard_trace(true);
    }
    // `copies == 0`: kopya yok (paket içi yedekli biçim), öncelikli şerit kalır (`below_id` 0 →
    // hiçbir birim kopyalanmaz; kural yalnız öncelik için kurulur).
    s.set_forward_copy_rule(Some(if sc.raw_copy_rule {
        ForwardCopyRule {
            below_id: TEXT_ID,
            priority_below_id: FILE_BASE_ID,
            copies: sc.copies,
            spacing_ns: sc.spacing_ns,
        }
    } else {
        ForwardCopyRule {
            below_id: if sc.copies == 0 { 0 } else { TEXT_ID },
            priority_below_id: FILE_BASE_ID,
            copies: sc.copies.max(1),
            spacing_ns: sc.spacing_ns,
        }
    }));
    s.set_forward_copy_line_scale(sc.copy_scale_min.map(|m| ForwardCopyLineScale {
        declared_bps: PROFILE_UP,
        min_copies: m,
    }));
    s
}

#[allow(clippy::too_many_lines)]
fn run(sc: &Scenario) -> Outcome {
    let mut s = sender(sc);
    let mut r = Receiver::with_cc_port();
    match sc.rx {
        RxModel::Mirror => r.set_return_bandwidth(Some(sc.ack_return_bw)),
        // [BUA · ÜRÜN ALICISI] `client_core::runtime` sunucu yuvası: `PeerSlot::new` (kota R =
        // `RuntimeConfig::quota_r` varsayılanı 4, bütçe = bu ucun yukarı yönü = dönüş hattı),
        // `set_ack_rereport(cfg.ack_rereport)` (varsayılan kapalı), CAPS kolu (bitmap açık, sıkışma
        // sınaması eşin yukarı yönünün beyanıyla, yalın PTT biti). CAPS düzenekte ilk varıştan
        // önce işlenmiş sayılır.
        RxModel::Product {
            ptt_lean,
            return_bw,
        } => {
            r.set_quota_r(4);
            r.set_return_bandwidth(Some(return_bw));
            r.set_ack_rereport(false);
            r.set_ack_bitmap(true);
            r.set_pair_squeeze_line(Some(PROFILE_UP));
            r.set_ack_bitmap_ptt_lean(ptt_lean);
        }
    }
    // [ÇİFT YOKLAMASI · KAPSAMA · #175 7186813951] Alıcının deney anahtarı göndericininkiyle
    // birlikte (ürün yolunda ikisi de `KAMA_PAIR_PROBE_DATAGRAMS`'tan okunur).
    if sc.pair_probe.is_some() {
        r.set_pair_probe_span(true);
    }
    let product_rx = matches!(sc.rx, RxModel::Product { .. });
    let mut fwd = Bottleneck::new(sc);
    s.set_diag_emit_log(sc.diag || sc.ledger);
    let mut reorder_lcg = sc.seed ^ 0x5A5A_5A5A;
    let mut ret = ReturnLine {
        plan: sc.rev_delay_plan.clone(),
        free_at: 0,
        last_arrival: 0,
        wire: VecDeque::new(),
        log: Vec::new(),
        types: BTreeMap::new(),
        bytes: 0,
    };
    let mut ack_log: Vec<AckRec> = Vec::new();
    let mut rx = RxMirror::default();
    let mut held: Option<(u64, u64, u64)> = None;
    let mut voice_offered: Vec<u64> = Vec::new();
    let mut voice_arrived: HashMap<u64, u64> = HashMap::new();
    let mut text_arrived: Option<u64> = None;
    let mut extra_text_offered: Vec<u64> = Vec::new();
    let mut extra_text_arrived: HashMap<u64, u64> = HashMap::new();
    let mut ptt_confirmed: HashSet<u64> = HashSet::new();
    let mut ptt_expired: Vec<(u64, u64, u64)> = Vec::new();
    let mut file_seen: HashSet<u64> = HashSet::new();
    let mut file_offered = 0u64;
    let mut file_done_ns = None;
    let mut tx = TxCount {
        ledger: sc.ledger,
        track_sent: product_rx,
        ..TxCount::default()
    };
    // [BUA · DEFTER] Tavan izi ACK numarasıyla (an, tavan, göndericiye varan kaçıncı ACK) ve
    // göndericide bekleyen veri (an, gönderilmemiş birim, uçuştaki bayt; saniyede bir).
    let mut ceil_trace: Vec<(u64, u64, usize)> = Vec::new();
    let mut backlog: Vec<(u64, usize, u64)> = Vec::new();
    let mut rx_bytes = 0u64;
    let mut rate_trace: Vec<(u64, Option<u64>)> = Vec::new();
    let mut acks_at = Vec::new();
    let mut pace_trace: Vec<(u64, u64)> = Vec::new();
    let mut ack_measured = Vec::new();
    let mut per_ack = Vec::new();
    let mut gate_acks: Vec<GateAck> = Vec::new();
    let mut gate_steps: Vec<GateStep> = Vec::new();
    let mut step_ack = false;
    // [K2] Çift aynası (salt gözlem; yalnız ürün alıcısı ve gözlem açıkken) ve ACK'lerin çıkış
    // sırasıyla sayımları (dönüş hattı FIFO: varış sırası aynı).
    let track_pairs = sc.gate_diag && product_rx;
    let mut pair_mirror = PairMirror::default();
    let mut pair_obs: Vec<PairObs> = Vec::new();
    let emit = |decs: Vec<TransmitDecision>,
                now: u64,
                fwd: &mut Bottleneck,
                tx: &mut TxCount,
                s: &mut Sender| {
        tx.drain(s);
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            tx.datagrams += 1;
            tx.last_seq = tx.last_seq.max(d.seq);
            if d.unit_id < TEXT_ID {
                tx.ptt_wire_bytes += d.size + OVH;
                tx.ptt_kind.insert(d.seq, d.kind);
                if d.kind == TransmitKind::FecParity {
                    tx.fec_low_bytes += d.size + OVH;
                }
            }
            if tx.track_sent {
                tx.sent.insert(d.seq);
            }
            tx.any_sent = true;
            tx.wire_bytes += d.size + OVH;
            tx.step_pkts += 1;
            tx.step_wire += d.size + OVH;
            if d.unit_id < FILE_BASE_ID && d.kind != TransmitKind::FecParity {
                tx.step_pri_pkts += 1;
                tx.step_pri_wire += d.size + OVH;
            }
            let meta = tx.meta(&d);
            fwd.send(now, d.seq, d.size, d.unit_id, meta);
        }
    };
    let mut now = 0u64;
    while now < sc.t_max_ns {
        // Uygulama.
        let file_due = if sc.file_gap_ns == 0 {
            if now >= sc.file_at_ns {
                sc.file_units
            } else {
                0
            }
        } else if now >= sc.file_at_ns {
            ((now - sc.file_at_ns) / sc.file_gap_ns + 1).min(sc.file_units)
        } else {
            0
        };
        while file_offered < file_due {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: FILE_BASE_ID + file_offered,
                    size: sc.file_unit.unwrap_or(FILE_UNIT),
                }),
                now,
            );
            file_offered += 1;
            emit(decs, now, &mut fwd, &mut tx, &mut s);
        }
        let next_voice = voice_offered.len() as u64;
        let voice_at = VOICE_START_NS + next_voice * VOICE_PERIOD_NS;
        if voice_at < sc.voice_until_ns && now >= voice_at {
            voice_offered.push(voice_at);
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: sc.voice_unit,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx, &mut s);
        }
        if sc.text_at_ns == Some(now) {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: TEXT_ID,
                    size: sc.text_unit.unwrap_or(80),
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx, &mut s);
        }
        if let Some(t0) = sc.text_at_ns.filter(|_| sc.text_period_ns > 0) {
            let at = t0 + (extra_text_offered.len() as u64 + 1) * sc.text_period_ns;
            if at < sc.text_until_ns.unwrap_or(sc.voice_until_ns) && now >= at {
                let id = TEXT_ID + 1 + extra_text_offered.len() as u64;
                extra_text_offered.push(at);
                let size = sc.text_unit.unwrap_or(80);
                let decs = s.on_event(Event::AppData(OpaqueUnit { id, size }), now);
                emit(decs, now, &mut fwd, &mut tx, &mut s);
            }
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut tx, &mut s);
        // [PTT-YALIN] Ürünün son tarih yolu (`runtime`, pace tick'te Pto'dan sonra): teyitsiz ve
        // sunumundan `deadline` geçmiş PTT birimleri motora teyitli bildirilir.
        if let Some(deadline) = sc.ptt_deadline_ns {
            ptt_confirmed.extend(
                s.take_confirmed_units()
                    .into_iter()
                    .filter(|&u| u < TEXT_ID),
            );
            let due: Vec<u64> = voice_offered
                .iter()
                .enumerate()
                .map(|(k, &at)| (k as u64, at))
                .filter(|&(k, at)| {
                    now.saturating_sub(at) >= deadline
                        && !ptt_confirmed.contains(&k)
                        && !ptt_expired.iter().any(|&(u, _, _)| u == k)
                })
                .map(|(k, _)| k)
                .collect();
            if !due.is_empty() {
                for &u in &due {
                    ptt_expired.push((u, now, tx.last_seq));
                }
                let decs = s.on_event(
                    Event::RecoveredControl(RecoveredInfo {
                        units: due
                            .iter()
                            .map(|&unit_id| RecoveredUnit {
                                unit_id,
                                block_id: 0,
                                esi: 0,
                            })
                            .collect(),
                    }),
                    now,
                );
                emit(decs, now, &mut fwd, &mut tx, &mut s);
            }
        }
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut tx, &mut s);
        }
        // Alıcı.
        if !product_rx {
            r.set_uplink_yield(ret.free_at > now);
        }
        let mut arrived = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, udp) = fwd.wire.pop_front().expect("var");
            arrived.push((seq, uid, udp));
        }
        let mut deliver = Vec::new();
        for a in arrived {
            if let Some(h) = held.take() {
                deliver.push(a);
                deliver.push(h);
                continue;
            }
            reorder_lcg = reorder_lcg
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            if (reorder_lcg >> 33) % 1000 < sc.reorder_permille {
                held = Some(a);
            } else {
                deliver.push(a);
            }
        }
        // [BUA] `no_interval` aralığında çıkan ACK'in ölçümü göndericiye verilmez.
        let measured_now = !sc.no_interval.is_some_and(|(a, b)| now >= a && now < b);
        let mut acks = Vec::new();
        for (seq, uid, udp) in deliver {
            rx_bytes += udp + OVH;
            if product_rx {
                // [BUA · ÜRÜN ALICISI] Çalışma zamanı her `on_datagram`'dan önce yukarı yön
                // hattının durumunu verir.
                r.set_return_line_ahead_ns(ret.free_at.saturating_sub(now));
                r.set_uplink_yield(ret.free_at > now);
            } else {
                rx.on_arrival(seq, udp, now);
            }
            r.set_next_datagram_bytes(udp);
            let alarms_before = (r.diag_drop_detects(), r.diag_drop_alarms());
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: RxKind::Data,
                },
                now,
            );
            if track_pairs {
                // Alıcının sırası: çift, daralma algısı, kanıtlı bırakma, (varsa) ACK.
                pair_mirror.on_arrival(now, udp);
                if r.diag_drop_detects() > alarms_before.0 {
                    pair_mirror.on_detect();
                }
                if r.diag_drop_alarms() > alarms_before.1 {
                    pair_mirror.on_evidence();
                }
            }
            for u in out.delivered {
                if u >= FILE_BASE_ID {
                    file_seen.insert(u);
                    if file_seen.len() as u64 == sc.file_units && file_done_ns.is_none() {
                        file_done_ns = Some(now);
                    }
                } else if u == TEXT_ID {
                    text_arrived.get_or_insert(now);
                } else if u > TEXT_ID {
                    extra_text_arrived.entry(u).or_insert(now);
                } else {
                    voice_arrived.entry(u).or_insert(now);
                }
            }
            if product_rx {
                if let Some(f) = out.ack {
                    if track_pairs {
                        pair_obs.push(pair_mirror.on_ack(&f));
                    }
                    ret.send_product(now, &f, measured_now);
                }
            } else {
                acks.extend(
                    out.ack
                        .map(|f| (f, rx.on_ack(r.largest_seq().unwrap_or(0)))),
                );
            }
        }
        if product_rx {
            r.set_return_line_ahead_ns(ret.free_at.saturating_sub(now));
            r.set_uplink_yield(ret.free_at > now);
            if let Some(f) = r.on_ack_timer(now) {
                if track_pairs {
                    pair_obs.push(pair_mirror.on_ack(&f));
                }
                ret.send_product(now, &f, measured_now);
            }
        } else {
            acks.extend(
                r.on_ack_timer(now)
                    .map(|f| (f, rx.on_ack(r.largest_seq().unwrap_or(0)))),
            );
        }
        for (frame, (m_first, largest, rx_span, m, pair_rate)) in acks {
            let expected = largest + 1;
            let bytes = frame.encode();
            let udp = bytes.len() as u64 + ACK_SEAL;
            // Ölçüm aralığı ve çift alt sınırı alıcının kuralıyla (`RxMirror`).
            let rx_span = rx_span.filter(|_| measured_now);
            ret.send(
                now,
                udp,
                bytes.first().copied().unwrap_or(0),
                RetMsg::Mirror {
                    arrival_rate: frame.arrival_rate,
                    ack: AckInfo {
                        acked_seqs: frame.acked_full(expected),
                        ack_delay: frame.ack_delay_duration(),
                    },
                    iv: (m_first, largest, rx_span, m, pair_rate),
                },
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _)| t <= now) {
            let (_, udp, msg) = ret.wire.pop_front().expect("var");
            let (sent, start) = ret.log[ack_log.len()];
            let ack_pair_obs = pair_obs.get(ack_log.len()).copied();
            // [KAPI] ACK göndericiye verilmeden hemen önce (salt gözlem).
            let gate_pre = sc.gate_diag.then(|| {
                (
                    s.diag_flight_gate().unwrap_or_default(),
                    s.unsent_units(),
                    s.diag_line_guard_trace_total(),
                    tx.datagrams,
                    tx.wire_bytes,
                )
            });
            let gate_pair = match &msg {
                RetMsg::Mirror { iv, .. } => Some(iv.4),
                RetMsg::Product { bytes, .. } if sc.gate_diag => {
                    AckFrame::decode_with_bitmap(bytes)
                        .ok()
                        .and_then(|f| f.bitmap.map(|b| b.pair_rate))
                }
                RetMsg::Product { .. } => None,
            }
            .filter(|&p| p != PAIR_RATE_NONE)
            .map(u64::from);
            let ack = match msg {
                RetMsg::Mirror {
                    arrival_rate,
                    ack,
                    iv: (first, last, rx_span, m, pair_rate),
                } => {
                    ack_log.push(AckRec {
                        sent,
                        start,
                        arrival: now,
                        udp,
                        largest: ack.acked_seqs.iter().copied().max().unwrap_or(0),
                    });
                    acks_at.push(now);
                    ack_measured.push(rx_span.is_some());
                    s.observe_ack_feedback(
                        AckObservation {
                            wire_bytes: udp,
                            seen_count: 0,
                            v2: true,
                            sent_datagrams: tx.datagrams,
                        },
                        now,
                    );
                    s.note_ack_arrival_rate(arrival_rate);
                    // Ürün çalışma zamanının sırası: `pair_rate`, ölçüm aralığı, sonra ACK.
                    if sc.interval {
                        s.note_ack_pair_rate(pair_rate);
                        s.note_ack_forward_interval(first, last, rx_span, &m);
                    }
                    ack
                }
                RetMsg::Product { bytes, measured } => {
                    let (ack, largest, has_span) = product_ack_into_sender(
                        &mut s,
                        &tx,
                        sc.interval,
                        &bytes,
                        udp,
                        measured,
                        now,
                    );
                    ack_log.push(AckRec {
                        sent,
                        start,
                        arrival: now,
                        udp,
                        largest,
                    });
                    acks_at.push(now);
                    ack_measured.push(has_span);
                    ack
                }
            };
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut tx, &mut s);
            let g = s.diag_line_guard().and_then(|g| g.0);
            if rate_trace.last().map(|&(_, r)| r) != Some(g) {
                rate_trace.push((now, g));
            }
            let p = s.diag_line_guard_pace().map_or(0, |p| p.0);
            if pace_trace.last().map(|&(_, q)| q) != Some(p) {
                pace_trace.push((now, p));
                ceil_trace.push((now, p, acks_at.len()));
            }
            let largest = ack_log.last().map_or(0, |a| a.largest);
            let ser = fwd.ser_start.get(&largest).copied().unwrap_or(0);
            per_ack.push((now, ser, g, p));
            // [KAPI] `Event::AckReceived` döndükten hemen sonra (salt gözlem).
            if let Some((before, unsent_before, trace_total, d0, w0)) = gate_pre {
                step_ack = true;
                gate_acks.push(GateAck {
                    index: acks_at.len(),
                    arrival: now,
                    ser_start: ser,
                    measured: ack_measured.last().copied().unwrap_or(false),
                    pair_rate: gate_pair,
                    before,
                    after: s.diag_flight_gate().unwrap_or_default(),
                    unsent_before,
                    unsent_after: s.unsent_units(),
                    emitted_pkts: tx.datagrams - d0,
                    emitted_wire: tx.wire_bytes - w0,
                    trace: (s.diag_line_guard_trace_total() > trace_total)
                        .then(|| s.diag_line_guard_trace().last().copied())
                        .flatten(),
                    gate_refused: s.diag_gate_refusals().0,
                    gate_last: s.diag_gate_refusals().1,
                    pair_probes: s.diag_pair_probes().unwrap_or(0),
                    pair_obs: ack_pair_obs,
                });
            }
        }
        if sc.ledger && now.is_multiple_of(SEC) {
            backlog.push((now, s.unsent_units(), s.inflight_bytes()));
        }
        // [KAPI] Adımın sonundaki durum (salt gözlem); adım sayaçları her adımda sıfırlanır.
        if sc.gate_diag {
            let g = s.diag_flight_gate().unwrap_or_default();
            gate_steps.push(GateStep {
                now,
                inflight: g.inflight,
                window: g.effective_window,
                bbr: g.bbr_window,
                floor: g.line_floor.unwrap_or(0),
                pace: g.pace_bps.unwrap_or(0),
                pkts: tx.step_pkts,
                wire: tx.step_wire,
                pri_pkts: tx.step_pri_pkts,
                pri_wire: tx.step_pri_wire,
                unsent: s.unsent_units() as u64,
                ack: step_ack,
                gate_refused: s.diag_gate_refusals().0,
            });
        }
        tx.step_pkts = 0;
        tx.step_wire = 0;
        tx.step_pri_pkts = 0;
        tx.step_pri_wire = 0;
        step_ack = false;
        let voice_left = (0..voice_offered.len() as u64).any(|v| {
            !voice_arrived.contains_key(&v) && !ptt_expired.iter().any(|&(u, _, _)| u == v)
        });
        let text_left = (sc.text_at_ns.is_some_and(|t| now >= t) && text_arrived.is_none())
            || extra_text_arrived.len() < extra_text_offered.len();
        let file_left = file_done_ns.is_none() && sc.file_units > 0;
        if now >= sc.voice_until_ns && !voice_left && !text_left && !file_left {
            break;
        }
        now += MS;
    }
    // Sıralı teslim: demet k, 0..=k'nin hepsi geldiğinde çalınır.
    let mut played_at = 0u64;
    let mut voice = Vec::new();
    let mut gap = false;
    for (k, &planned) in voice_offered.iter().enumerate() {
        match voice_arrived.get(&(k as u64)) {
            Some(&t) if !gap => {
                played_at = played_at.max(t);
                voice.push((planned, Some(played_at - planned)));
            }
            _ => {
                gap = true;
                voice.push((planned, None));
            }
        }
    }
    let ptt_tx = fwd
        .ptt_log
        .iter()
        .map(|&p| (p, tx.ptt_kind[&p.seq]))
        .collect();
    let voice_first = (0..voice_offered.len() as u64)
        .map(|v| voice_arrived.get(&v).copied())
        .collect();
    let emit_log = if sc.ledger {
        tx.drain(&mut s);
        std::mem::take(&mut tx.emit_all)
    } else {
        s.take_diag_emit_log()
    };
    if sc.ledger {
        ledger_check(&fwd, &tx, rx_bytes, held.map_or(0, |h| h.2 + OVH));
    }
    Outcome {
        file_done_ns,
        waits: fwd.waits.clone(),
        drops_queue: fwd.dropped_queue.len(),
        drop_times: fwd.dropped_queue.clone(),
        rate_trace,
        guard: s.diag_line_guard(),
        voice,
        text_delay_ns: sc.text_at_ns.zip(text_arrived).map(|(a, b)| b - a),
        ptt_wire_bytes: tx.ptt_wire_bytes,
        acks_at,
        ack_measured,
        interval: s.diag_line_guard_interval(),
        pace_trace,
        ptt_tx,
        acks: ack_log,
        voice_first,
        emit_log,
        per_ack,
        ptt_expired,
        text_tx: fwd.text_log.clone(),
        text_delays: sc
            .text_at_ns
            .map(|t| (t, text_arrived.map(|a| a - t)))
            .into_iter()
            .chain(extra_text_offered.iter().enumerate().map(|(k, &at)| {
                (
                    at,
                    extra_text_arrived
                        .get(&(TEXT_ID + 1 + k as u64))
                        .map(|&a| a - at),
                )
            }))
            .collect(),
        book: fwd.book.clone(),
        line: fwd.line,
        ceil_trace,
        backlog,
        ack_types: ret.types.clone(),
        ack_wire_bytes: ret.bytes,
        gate_acks,
        gate_steps,
        gate_tx: std::mem::take(&mut fwd.fates),
        passes: std::mem::take(&mut fwd.passes),
        oversize_drops: fwd.oversize_drops,
    }
}

/// [BUA · DEFTER] Defterin hat modeliyle sağlaması (ölçüm aracının doğrulaması, kabul kapısı
/// değil): sınıf toplamları hat sayaçlarına, hat sayaçları birbirine ve alıcıya varan bayta eşit;
/// her iletimin nedeni izde bulundu.
fn ledger_check(fwd: &Bottleneck, tx: &TxCount, rx_bytes: u64, held_bytes: u64) {
    let sum = |f: fn(&ClassBytes) -> u64| fwd.book.values().map(f).sum::<u64>();
    let l = fwd.line;
    assert_eq!(sum(|b| b.bytes), l.offered_bytes, "defter: giren bayt");
    assert_eq!(sum(|b| b.pkts), l.offered_pkts, "defter: giren paket");
    assert_eq!(
        l.offered_bytes, tx.wire_bytes,
        "defter: gönderici ↔ darboğaz"
    );
    assert_eq!(l.offered_pkts, tx.datagrams, "defter: gönderici paket");
    assert_eq!(
        sum(|b| b.queue_drop_bytes),
        l.queue_drop_bytes,
        "defter: kuyrukta düşen"
    );
    assert_eq!(
        sum(|b| b.queue_drop_pkts),
        fwd.dropped_queue.len() as u64,
        "defter: kuyrukta düşen paket"
    );
    assert_eq!(sum(|b| b.lost_bytes), l.lost_bytes, "defter: hatta düşen");
    assert_eq!(
        l.offered_bytes,
        l.serialized_bytes + l.queue_drop_bytes,
        "hat: sunulan = serileşen + kuyrukta düşen"
    );
    assert_eq!(
        l.serialized_pkts,
        fwd.waits.len() as u64,
        "hat: serileşen paket"
    );
    assert_eq!(
        l.serialized_bytes,
        l.lost_bytes + l.delivered_bytes,
        "hat: serileşen = düşen + yayılan"
    );
    let on_wire: u64 = fwd.wire.iter().map(|w| w.3 + OVH).sum();
    assert_eq!(
        l.delivered_bytes,
        rx_bytes + on_wire + held_bytes,
        "hat: yayılan = alıcıya varan + yolda + sıra bekleyen"
    );
    let ptt: u64 = fwd
        .book
        .iter()
        .filter(|((_, p), _)| *p == Payload::Ptt)
        .map(|(_, b)| b.bytes)
        .sum();
    assert_eq!(
        ptt + tx.fec_low_bytes,
        tx.ptt_wire_bytes,
        "defter: PTT baytı"
    );
    let max_wait = fwd.waits.iter().map(|&(_, w)| w).max().unwrap_or(0);
    assert_eq!(l.max_wait_ns, max_wait, "hat: en büyük bekleme");
    assert!(
        l.max_occ_pkts <= fwd.queue_pkts as u64,
        "hat: doluluk FIFO tavanını aştı"
    );
    let unknown: Vec<_> = fwd
        .book
        .keys()
        .filter(|(c, _)| matches!(c, TxClass::Unknown(_)))
        .collect();
    assert!(
        unknown.is_empty(),
        "defter: nedeni izde olmayan iletim {unknown:?}"
    );
}

fn secs(ns: u64) -> String {
    if ns == u64::MAX {
        return "YOK".to_string();
    }
    format!("{:.2} s", ns as f64 / SEC as f64)
}

fn report(tag: &str, sc: &Scenario, o: &Outcome) {
    println!(
        "SİM {tag} tohum {} aralık={} dosya={} kuyruk_max={} kuyruk_düşen={} hız_izi={:?} \
         koruma={:?} ptt_max={} metin={} ptt_tel={} B ack={}",
        sc.seed,
        sc.interval,
        o.file_done_ns.map_or("YOK".to_string(), secs),
        secs(o.waits.iter().map(|&(_, w)| w).max().unwrap_or(0)),
        o.drops_queue,
        o.rate_trace
            .iter()
            .map(|&(t, r)| format!(
                "{}:{}",
                secs(t),
                r.map_or("-".to_string(), |r| r.to_string())
            ))
            .collect::<Vec<_>>(),
        o.guard,
        secs(o.voice_max_from(0)),
        o.text_delay_ns.map_or("-".to_string(), secs),
        o.ptt_wire_bytes,
        o.acks_at.len(),
    );
}

/// Korumanın hiç aşağı yönlü örnek almadığı (ya da beyanın %5'inden fazla indirmediği) koşu.
fn never_cut(o: &Outcome) -> bool {
    o.min_rate().is_none_or(|m| m * 20 >= PROFILE_UP * 19)
}

/// (a) İleri hat sabit (dürüst 12 500), yalnız dönüş gecikmesi artıyor (ürün hücresinin çizelgesi),
/// ACK'ler sık (bitmap kolu), dosya erken biter ve gönderici sonra yalnız PTT gönderir (tavanın
/// altında): kapasite düşürülmez.
///
/// Karşı-olgu (aynı koşu, ölçüm aralığı verilmeden: eski cephe tahmincisi): en az bir tohumda hat
/// beyanın %5'inden fazla düşürülür. Bu kol senaryonun kusuru gerçekten ürettiğini gösterir.
#[test]
fn a_return_delay_rise_does_not_cut_an_honest_line() {
    let mut control_cut = false;
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("a/aday", &sc, &o);
        assert!(
            never_cut(&o),
            "tohum {seed}: dürüst hat düşürüldü: {:?}",
            o.rate_trace
        );
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("a/karşı-olgu", &control, &oc);
        control_cut |= !never_cut(&oc);
    }
    assert!(
        control_cut,
        "karşı-olgu: eski tahminci bu senaryoda kesmedi"
    );
}

/// Bir koşunun ölçülen hatta yakınsadığı ilk an: tahmin `real`'in %10 içine girdiği an.
fn converged_at(o: &Outcome, real: u64) -> Option<u64> {
    o.rate_trace
        .iter()
        .find(|&&(_, r)| r.is_some_and(|r| r * 10 <= real * 11 && r * 10 >= real * 9))
        .map(|&(t, _)| t)
}

const DROP_AT_NS: u64 = 8 * SEC;
const DROP_TO: u64 = 6_250;
/// Düşüşten hemen sonra sunulan metin birimi (b).
const DROP_TEXT_AT_NS: u64 = 8_400 * MS;

fn drop_scenario(seed: u64, interval: bool) -> Scenario {
    Scenario {
        fwd_plan: vec![(0, PROFILE_UP), (DROP_AT_NS, DROP_TO)],
        file_units: 150,
        voice_until_ns: 40 * SEC,
        ack_return_bw: 4 * PROFILE_DOWN,
        interval,
        ..Scenario::honest(seed)
    }
}

/// (b) Gerçek ileri kapasite 8 s'de 12 500'den 6 250'ye düşüyor (gönderici dosyayla tavanda):
/// daralma algılanır ve hat ölçülen hızda kalır.
///
/// Sınırlar (varsayımlarla): ACK aralığı `T_ack` = düşüşten önceki en büyük ACK aralığı
/// (göndericide; alıcının ACK kadansı, bu düzenekte ≈ 2,1 s), dönüş geçişi `T_ret` = en büyük
/// ACK'in (129 B + 28) 62 B/s'de serileşmesi + 300 ms. İlk yazımda `T_ack` bütçe formülündendi
/// (`budgeted_ack_interval_ns`, ≈ 1,3 s); düzeneğin gerçek kadansı daha seyrek çıktı, formül aynı
/// kaldı. Düşüşten sonra M'si tamamen düşüş sonrası varışlardan oluşan ilk ACK en
/// geç üç ACK aralığı + bir dönüş geçişi sonra gelir:
/// * yakınsama (tahmin gerçek hattın %10 içinde) ≤ düşüş + 3·T_ack + T_ret;
/// * yakınsamadan 2 s sonra darboğaz beklemesi ≤ iki tam datagramın gerçek hatta serileşmesi;
/// * yakınsamadan 9 s sonra planlanan PTT demetleri sıralı teslimde ≤ 1 s (düşüş anında kuyrukta
///   düşen demetlerin ARQ kurtarması iki ACK turu sürer).
///
/// **Daralma anı (İŞ-35 AÇIK 3).** Düşüşten (8 s) itibaren en kötü PTT demeti ve düşüşten hemen
/// sonra sunulan metin ([`DROP_TEXT_AT_NS`], 8,4 s) kabul sınırlarıyla (1 s, 1 s) iddia edilir.
/// PTT iddiası bu düzenekte FİZİKSEL OLARAK SAĞLANAMAZ ve test bu yüzden kırmızıdır (gizlenmez).
/// Alt sınır, hiçbir gönderici politikasına bağlı olmadan:
/// * Düşüşe dek gönderici `S` = 12 500 B/s'de gönderir (dürüst dosya sınırı 14 s bunu ister: bu
///   düzenekte dürüst dosya `S` = 12 500'de 12,26–14,74 s sürüyor, (a)'nın aday kolu). Gerçek hat `L` = 6 250: FIFO
///   kuyruğu göndericinin tepkisine dek `S/L − 1` = 1 s/s büyür.
/// * Algılama gecikmesi `D` (düşüşten göndericinin ilk karar verebildiği ana): ileri yayılım
///   0,3 s, çözünürlüğü aşan artış (≥ bir MTU serileşmesi ≈ 0,1 s), ACK kadansı (0…`T_ack`),
///   ACK'in 62 B/s'de serileşmesi (en küçük çerçeve 37 + 28 B → 1,05 s; burada 65–129 B → 1,5–2,5 s)
///   ve dönüş yayılımı 0,3 s toplamı. En iyi durumda `D` ≥ 0,3 + 0,1 + 0 + 1,05 + 0,3 = 1,75 s; bu düzenekte
///   (ACK aralığı ≈ 2,1 s, çerçeve ≈ 2 s) ≈ 3,3–4,1 s (ölçülen ilk örnek 11,25–12,08 s).
/// * Algılama anında kuyruk ≥ `(S/L − 1)·D` ≥ 1,75 s. FIFO'dur: o anda kuyruğa girmiş bir PTT
///   demeti, gönderici her şeyi durdursa da, kuyruğun önündeki baytları bekler: gecikme ≥ 1,75 s +
///   0,3 s yayılım ≥ 2,05 s > 1 s. FIFO 64 paketle dolunca (≈ 2,9 s) kuyrukta düşen PTT kopyaları
///   ARQ turunu bekler (daha da geç).
/// * PTT ≤ 1 s için kuyruk ≤ 1 − 0,3 − demetin serileşmesi (≈ 0,01 s) ≈ 0,69 s kalmalıdır; buna
///   düşüş anında izinli gönderim `S ≤ L·(1 + 0,69/D)`: en iyi `D` = 1,75 s ile 8 714 B/s, bu
///   düzeneğin `D` ≈ 3,3 s'siyle ≈ 7 560 B/s. İkisi de dürüst dosya sınırının istediği 12 500'ün
///   altında: iki kabul sınırı (dürüst dosya 14 s ve daralma anında PTT 1 s) bu fizik altında
///   birlikte sağlanamaz. Sınırlar sonuca göre değiştirilmedi.
/// * Metin (8,4 s, kopyasız öncelikli birim): kuyruk 8,4 s'de ≈ 0,4 s → ≈ 0,7 s; ilk iletimi
///   rastgele kayıpta düşerse (%20) kurtarma bir geri bildirim turu (≥ `D`) bekler ve 1 s'yi aşar:
///   metin iddiası tohuma bağlıdır, fiziksel olarak garanti edilemez.
///
/// Düşüşten yakınsamaya dek kuyruk FIFO'yla (64 paket) sınırlıdır; bu aralıkta PTT 1 s'yi aşar.
/// Bu, kullanılan geri bildirim gecikmesinin (≈ 3–4 s) ve FIFO modelinin sonucudur.
///
/// Karşı-olgu: eski cephe tahmincisi aynı koşuda yakınsama sınırını üç tohumda da aşar (tahmin
/// düşüşten 24–26 s sonra oturur).
///
/// **Alıcı: ürün alıcısı ([`RxModel::PRODUCT`]; koordinatör kararı #175/#236 7088316519).** Bu
/// test ve (d-2) aday ve karşı-olgu kollarında ürün çalışma zamanının 0x15 alıcısıyla ölçer
/// (bütçe dönüş hattı 62 B/s). Neden: [`RxModel::Mirror`] ACK bütçesini senaryonun
/// `ack_return_bw`'sinden (4 × 62 B/s) alır ve ACK aralığı ortalaması ≈ 2,0–2,1 s verir; ürün
/// alıcısı aynı koşuda ≈ 4,9–5,3 s verir (#236 bulgusu: ayna ürünün geri bildirimini iyimser
/// ölçüyordu). Senaryo, tohum, kayıp modeli (LCG), kopyalı PTT yükü, sınırların formülleri ve
/// iddia metinleri değişmedi; yukarıdaki "≈ 2,1 s" gibi kadans değerleri aynanın ölçümüdür.
#[test]
fn b_real_capacity_drop_is_detected_and_the_queue_drains() {
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    let mut from_drop = Vec::new();
    for seed in 1..=3u64 {
        let sc = Scenario {
            text_at_ns: Some(DROP_TEXT_AT_NS),
            rx: RxModel::PRODUCT,
            ..drop_scenario(seed, true)
        };
        let o = run(&sc);
        report("b/aday", &sc, &o);
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max()
            .expect("düşüşten önce ACK'ler");
        let conv_bound = DROP_AT_NS + 3 * t_ack + t_ret;
        let conv = converged_at(&o, DROP_TO).expect("yakınsadı");
        assert!(
            conv <= conv_bound,
            "tohum {seed}: yakınsama {} (sınır {})",
            secs(conv),
            secs(conv_bound)
        );
        let end = o.rate_at_end().expect("ölçüldü");
        assert!(end * 10 <= DROP_TO * 11, "tohum {seed}: son tahmin {end}");
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        assert!(
            w <= wait_bound,
            "tohum {seed}: yakınsama sonrası bekleme {}",
            secs(w)
        );
        let ptt = o.voice_max_from(conv + 9 * SEC);
        assert!(
            ptt <= PTT_BOUND_NS,
            "tohum {seed}: yakınsama sonrası PTT {}",
            secs(ptt)
        );
        let ptt_drop = o.voice_max_from(DROP_AT_NS);
        let text = o.text_delay_ns.unwrap_or(u64::MAX);
        println!(
            "B/daralma-anı tohum {seed}: PTT(≥ 8 s) en kötü {} metin(8,4 s) {} yakınsama {} \
             düşüşten {}",
            secs(ptt_drop),
            secs(text),
            secs(conv),
            secs(conv - DROP_AT_NS)
        );
        from_drop.push((seed, ptt_drop, text));
        let control_sc = Scenario {
            rx: RxModel::PRODUCT,
            ..drop_scenario(seed, false)
        };
        let control = run(&control_sc);
        report("b/karşı-olgu", &control_sc, &control);
        let c_conv = converged_at(&control, DROP_TO).unwrap_or(u64::MAX);
        assert!(
            c_conv > conv_bound,
            "karşı-olgu tohum {seed}: eski tahminci de sınırda yakınsadı ({})",
            secs(c_conv)
        );
    }
    // [İŞ-35 AÇIK 3] Daralma anından itibaren kabul sınırları (belge: fiziksel alt sınır).
    for &(seed, ptt, text) in &from_drop {
        assert!(
            ptt <= PTT_BOUND_NS && text <= TEXT_BOUND_NS,
            "ÇÖZÜLMEDİ (fiziksel alt sınır, test belgesi) tohum {seed}: düşüşten itibaren \
             PTT {} metin {}: {:?}",
            secs(ptt),
            secs(text),
            from_drop
                .iter()
                .map(|&(s, p, t)| (s, secs(p), secs(t)))
                .collect::<Vec<_>>()
        );
    }
}

/// (c) Gönderici kapasitenin altında sunuyor (dosya birimleri 400 ms'de bir, ≈ 2,6 kB/s, artı
/// PTT), dönüş gecikmesi artıyor: düşük sunum hızı hat kapasitesi sanılmaz.
///
/// Karşı-olgu: eski cephe tahmincisi hattı üç tohumda da sunum hızına indirir.
#[test]
fn c_a_low_offered_rate_is_not_taken_as_capacity() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            file_units: 30,
            file_gap_ns: 400 * MS,
            voice_until_ns: 22 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("c/aday", &sc, &o);
        assert!(never_cut(&o), "tohum {seed}: {:?}", o.rate_trace);
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("c/karşı-olgu", &control, &oc);
        assert!(
            !never_cut(&oc),
            "karşı-olgu tohum {seed}: eski tahminci kesmedi"
        );
    }
}

/// (d-1) %20 rastgele kayıp, dürüst hat, gönderici tavanda, dönüş gecikmesi sabit: kayıp alınan
/// baytı düşürür ama kuyruk artışı üretmez; ölçüm geçerli ve hat düşürülmez. Davranışı gösterir
/// (eski tahminci de bu koşuda kesmez: gönderimin baytıyla sayar).
#[test]
fn d1_random_loss_is_not_queue_growth() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            file_units: 120,
            voice_until_ns: 15 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("d1/aday", &sc, &o);
        let (checks, samples, _) = o.interval.expect("kural kurulu");
        assert!(checks >= 5, "tohum {seed}: ölçülen aralık {checks}");
        assert_eq!(samples, 0, "tohum {seed}: rastgele kayıp örnek verdi");
        assert!(never_cut(&o), "tohum {seed}: {:?}", o.rate_trace);
    }
}

/// (d-2) Kuyruk taşması: (b)'nin düşüşünde 64 paketlik FIFO dolar ve kuyrukta datagram düşer. M'de
/// kuyrukta düşen seq'ler varış süresine girmez, tahmin göndericinin baytıyla yapılır (taşmanın
/// olduğu aralıkta tahmin gerçek hattın üstünde kalır). Davranış: taşma olur, yine de tahmin
/// gerçek hatta oturur ve bir daha taşma olmaz.
///
/// Alıcı: ürün alıcısı ([`RxModel::PRODUCT`]); neden ve karar (b)'nin belgesinde.
#[test]
fn d2_queue_overflow_still_converges_and_stops_overflowing() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            rx: RxModel::PRODUCT,
            ..drop_scenario(seed, true)
        };
        let o = run(&sc);
        assert!(
            o.drops_queue > 0,
            "tohum {seed}: taşma olmadı (senaryo sınamıyor)"
        );
        let conv = converged_at(&o, DROP_TO).expect("yakınsadı");
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        let fifo_ns = 64 * (VOICE_UNIT + OVH) * SEC / DROP_TO;
        assert!(
            w < fifo_ns,
            "tohum {seed}: yakınsamadan sonra kuyruk dolu ({})",
            secs(w)
        );
    }
}

/// (d-3) Yeniden sıralama (%3: bir datagram bir sonrakinden sonra işlenir): m_first geç gelirse
/// `rx_span` kısalır, ölçüm daralmayı küçük gösterir (güvenli taraf). Dürüst hatta dönüş
/// gecikmesi artarken hat düşürülmez; gerçek 6 250'de koruma yine ölçülen hatta oturur.
///
/// Karşı-olgu (dürüst hat): eski cephe tahmincisi üç tohumda da keser.
#[test]
fn d3_reordering_keeps_the_measurement_on_the_safe_side() {
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            reorder_permille: 30,
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("d3/aday", &sc, &o);
        assert!(never_cut(&o), "tohum {seed}: {:?}", o.rate_trace);
        let control = Scenario {
            interval: false,
            ..sc.clone()
        };
        let oc = run(&control);
        report("d3/karşı-olgu", &control, &oc);
        assert!(
            !never_cut(&oc),
            "karşı-olgu tohum {seed}: eski tahminci kesmedi"
        );
        let slow = Scenario {
            fwd_plan: vec![(0, DROP_TO)],
            file_units: 80,
            voice_until_ns: 20 * SEC,
            rev_delay_plan: vec![(0, ONE_WAY_NS)],
            ..sc
        };
        let os = run(&slow);
        report("d3/6250", &slow, &os);
        let end = os.rate_at_end().expect("ölçüldü");
        assert!(
            end * 10 <= DROP_TO * 11,
            "tohum {seed}: 6 250'de son tahmin {end}"
        );
    }
}

/// [İŞ-35 AÇIK 1] Aralık kipinde ölçümsüz ACK dizisi + dönüş gecikmesi artışı, ileri hat sabit
/// (dürüst 12 500): (a)'nın koşusu, alıcı 5–13 s arasında çıkardığı ACK'lerde ölçüm taşımaz
/// (göndericiye üst üste en az iki ölçümsüz ACK; dizi ürün çizelgesinin 6 s ve 9 s basamaklarını
/// kapsar). Ölçüm yokluğu cephe tahmininin doğru olduğu anlamına gelmez: hat düşürülmez (korunan
/// hız ve gönderim tavanı beyanın %95'inin altına inmez).
///
/// Karşı-olgu (aynı koşu, ölçüm aralığı hiç verilmeden): cephe tahmincisi en az bir tohumda keser
/// (senaryo kusuru üretir).
#[test]
fn e1_unmeasured_acks_do_not_bring_back_the_return_delay_cut() {
    let mut control_cut = false;
    for seed in 1..=3u64 {
        let sc = Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            no_interval: Some((5 * SEC, 13 * SEC)),
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        report("e1/aday", &sc, &o);
        let (run_len, _, _) = o.longest_unmeasured_run();
        println!(
            "E1 tohum {seed}: ölçümsüz dizi {run_len} ACK, tavan izi {:?}",
            o.pace_trace
        );
        assert!(run_len >= 2, "tohum {seed}: düzenek ölçümsüz dizi kurmadı");
        assert!(
            never_cut(&o),
            "tohum {seed}: dürüst hat ölçümsüz dizide düşürüldü: {:?}",
            o.rate_trace
        );
        assert!(
            o.min_pace() * 20 >= PROFILE_UP * 19,
            "tohum {seed}: gönderim tavanı düşürüldü: {:?}",
            o.pace_trace
        );
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("e1/karşı-olgu", &control, &oc);
        control_cut |= !never_cut(&oc);
    }
    assert!(
        control_cut,
        "karşı-olgu: cephe tahmincisi bu senaryoda kesmedi"
    );
}

/// [İŞ-35 AÇIK 1] Ölçüm uzun süre yokken gerçek daralma: (b)'nin düşüşü (8 s, 12 500 → 6 250),
/// alıcı 8–19 s arasında çıkardığı ACK'lerde ölçüm taşımaz (göndericiye üst üste en az üç ölçümsüz
/// ACK). Koruma kuyruğu sınırlı tutar; ölçüm dönünce hat ölçülür.
///
/// Sınırlar (varsayımlarla):
/// * `t_gap` = dizinin ikinci ölçümsüz ACK'inin göndericiye varışı (ölçüm boşluğunun göndericide
///   ilk kanıtlanabildiği an; tek ölçümsüz ACK alıcının o ACK'te ölçemediği olağan durumdur).
///   O ana dek kuyruk (b)'deki gibi büyür (algılama yok); `t_gap`'teki kuyruk `Q_gap` ≤ o ana dek
///   ölçülen en büyük darboğaz beklemesi (FIFO ile sınırlı).
/// * `t_gap + Q_gap + 1 s`'den ölçümün göndericiye döndüğü ana (`t_back`) dek darboğaz beklemesi
///   ≤ 1 s − 0,3 s ileri yayılım = 0,7 s (PTT sınırının yapısal payı). Varsayım: gerçek hat gönderim
///   tavanının yarısından yavaş değil (başlangıç bütçesinin varsayımı; `line_guard` modül belgesi);
///   kuyruk kipe girişte `Q_gap` ile boşaltılır, 1 s boşaltmanın ve ilk patlamanın payıdır.
/// * Ölçüm dönünce: yakınsama ≤ `t_back` + 3·T_ack + T_ret ((b)'nin formülü, ölçümün dönüşünden);
///   yakınsamadan 2 s sonra bekleme ≤ iki tam datagram; yakınsamadan 9 s sonra PTT ≤ 1 s.
#[test]
fn e2_a_long_unmeasured_gap_on_a_real_drop_keeps_the_queue_bounded() {
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    let gap_wait_bound = PTT_BOUND_NS - ONE_WAY_NS;
    for seed in 1..=3u64 {
        let sc = Scenario {
            no_interval: Some((DROP_AT_NS, 19 * SEC)),
            ..drop_scenario(seed, true)
        };
        let o = run(&sc);
        report("e2/aday", &sc, &o);
        let (run_len, t_gap, t_back) = o.longest_unmeasured_run();
        let (t_gap, t_back) = (
            t_gap.expect("ikinci ölçümsüz ACK"),
            t_back.expect("ölçüm döndü"),
        );
        let q_gap = o.max_wait_between(0, t_gap);
        let gap_from = t_gap + q_gap + SEC;
        let w_gap = o.max_wait_between(gap_from, t_back);
        println!(
            "E2 tohum {seed}: ölçümsüz dizi {run_len} ACK, t_gap {} Q_gap {} boşluk beklemesi \
             ({}–{}) {} tavan izi {:?}",
            secs(t_gap),
            secs(q_gap),
            secs(gap_from),
            secs(t_back),
            secs(w_gap),
            o.pace_trace
        );
        assert!(
            run_len >= 3,
            "tohum {seed}: düzenek uzun ölçümsüz dizi kurmadı"
        );
        assert!(
            w_gap <= gap_wait_bound,
            "tohum {seed}: ölçüm boşluğunda kuyruk sınırsız: {} (sınır {})",
            secs(w_gap),
            secs(gap_wait_bound)
        );
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max()
            .expect("düşüşten önce ACK'ler");
        let conv_bound = t_back + 3 * t_ack + t_ret;
        let conv = converged_at(&o, DROP_TO).expect("yakınsadı");
        assert!(
            conv <= conv_bound,
            "tohum {seed}: yakınsama {} (sınır {})",
            secs(conv),
            secs(conv_bound)
        );
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        assert!(
            w <= wait_bound,
            "tohum {seed}: yakınsama sonrası bekleme {}",
            secs(w)
        );
        let ptt = o.voice_max_from(conv + 9 * SEC);
        assert!(
            ptt <= PTT_BOUND_NS,
            "tohum {seed}: yakınsama sonrası PTT {}",
            secs(ptt)
        );
    }
}

/// [İŞ-35 AÇIK 3] Küçük FIFO (8 paket), (b)'nin düşüşü (12 500 → 6 250): kuyruk hemen dolar ve
/// sabit kalır (`g ≈ 0`), fazlalık kuyrukta düşer. "Kuyruk büyümüyor" tek başına "hat yeterli"
/// demek değildir. Sınırlar (b)'ninkiler: yakınsama ≤ düşüş + 3·T_ack + T_ret; yakınsamadan 2 s
/// sonra kuyrukta düşme yok ve bekleme ≤ iki tam datagram; yakınsamadan 9 s sonra PTT ≤ 1 s.
///
/// Kanıt girdisi: aralığın teslim kesri, artış örneğinden sonra rastgele kayıp tabanına göre aşırı
/// ise taşma örneği (`line_guard` modül belgesi, "Taşma"). İŞ-34 kuralıyla tohum 1'de yakınsama
/// 53,7 s; taşma örneği kapatılınca (karşı-olgu, cephe dönüşü olmadan) hiç yakınsamıyor.
#[test]
fn d4_a_full_small_fifo_is_detected_and_stops_overflowing() {
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    for seed in 1..=3u64 {
        let sc = Scenario {
            queue_pkts: 8,
            ..drop_scenario(seed, true)
        };
        let o = run(&sc);
        report("d4/aday", &sc, &o);
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max()
            .expect("düşüşten önce ACK'ler");
        let conv_bound = DROP_AT_NS + 3 * t_ack + t_ret;
        let conv = converged_at(&o, DROP_TO);
        println!(
            "D4 tohum {seed}: yakınsama {} (sınır {}) kuyrukta düşen {} (yakınsamadan 2 s sonra {})",
            conv.map_or("YOK".to_string(), secs),
            secs(conv_bound),
            o.drops_queue,
            conv.map_or(0, |c| o.drops_between(c + 2 * SEC, u64::MAX))
        );
        let conv = conv.expect("yakınsadı");
        assert!(
            conv <= conv_bound,
            "tohum {seed}: yakınsama {} (sınır {})",
            secs(conv),
            secs(conv_bound)
        );
        assert_eq!(
            o.drops_between(conv + 2 * SEC, u64::MAX),
            0,
            "tohum {seed}: yakınsamadan sonra taşma sürüyor"
        );
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        assert!(
            w <= wait_bound,
            "tohum {seed}: yakınsama sonrası bekleme {}",
            secs(w)
        );
        let ptt = o.voice_max_from(conv + 9 * SEC);
        assert!(
            ptt <= PTT_BOUND_NS,
            "tohum {seed}: yakınsama sonrası PTT {}",
            secs(ptt)
        );
    }
}

/// [Görev 3] Dürüst profil, dosya ve PTT; bir PTT demetinin (demet 40, 8,5 s) ilk beş iletimi hatta
/// düşer, başka kayıp yok. Ölçütler ürünün kabul ölçütleridir: bütün demetler sıralı teslimde
/// ≤ 1 s, metin ≤ 1 s, dosya ≤ 14 s.
///
/// * Ürün kuralı (beyan hızında 7 iletim): altıncı ve yedinci iletim demeti 1 s içinde taşır.
/// * Karşı-olgu (2026-11-16 kuralı, 5 iletim): kurtarma ACK turunu bekler; demet ve ardındakiler
///   1 s'yi aşar.
///
/// Bant bedeli raporlanır (ileri hatta PTT tel baytı ve dosya süresi, iki kural).
#[test]
fn five_lost_ptt_transmissions_are_covered_within_the_bounds() {
    let five_lost = |copies, spacing_ns, scale| Scenario {
        loss_permille: 0,
        targeted_drop: Some((40, 5)),
        text_at_ns: Some(8_500 * MS),
        voice_until_ns: 20 * SEC,
        copies,
        spacing_ns,
        copy_scale_min: scale,
        ..Scenario::honest(1)
    };
    let sc = five_lost(PRODUCT_COPIES, PRODUCT_SPACING_NS, Some(PRODUCT_MIN_COPIES));
    let o = run(&sc);
    report("görev3/ürün", &sc, &o);
    let old = five_lost(OLD_COPIES, OLD_SPACING_NS, None);
    let oo = run(&old);
    report("görev3/eski", &old, &oo);
    let file = o.file_done_ns.expect("dosya bitti");
    println!(
        "GÖREV3 bant: PTT tel {} B (eski {} B, +{} B); dosya {} (eski {})",
        o.ptt_wire_bytes,
        oo.ptt_wire_bytes,
        o.ptt_wire_bytes - oo.ptt_wire_bytes,
        secs(file),
        oo.file_done_ns.map_or("YOK".to_string(), secs)
    );
    assert!(
        o.voice_max_from(0) <= PTT_BOUND_NS,
        "PTT {}",
        secs(o.voice_max_from(0))
    );
    assert!(o.text_delay_ns.is_some_and(|d| d <= TEXT_BOUND_NS), "metin");
    assert!(file <= FILE_BOUND_NS, "dosya {}", secs(file));
    assert!(
        oo.voice_max_from(0) > PTT_BOUND_NS,
        "karşı-olgu: beş iletimle demet sınırda kaldı"
    );
}

// ---------------------------------------------------------------------------------------------
// [İŞ-35 AÇIK 2] PTT koruması tükendiğinde kurtarma.
// ---------------------------------------------------------------------------------------------

/// Bütün koruma iletimleri düşen demet (8,5 s'de planlanır).
const LOST_BUNDLE: u64 = 40;
/// Ürün kuralında beyan hızındaki toplam iletim (asıl + 6 kopya).
const PRODUCT_TX: u32 = PRODUCT_COPIES as u32 + 1;

/// Tam kaybın kurtarma izi.
#[derive(Debug)]
struct Recovery {
    planned: u64,
    /// Demetin ilk `n` iletimi (hepsi hatta düştü).
    protect: Vec<(PttTx, TransmitKind)>,
    /// Alıcının deliği gördüğünü gösteren ilk ACK: en büyüğü asıl iletimin seq'inden büyük.
    hole_ack: Option<AckRec>,
    /// Bütün koruma iletimlerinin ardından alınanı gösteren ilk ACK: en büyüğü son koruma
    /// iletiminin seq'inden büyük.
    gap_ack: Option<AckRec>,
    /// Göndericinin asıl iletimi kayıp ilan ettiği an (iz).
    declared: Option<(u64, EmitCause)>,
    /// Koruma iletimlerinden sonraki ilk iletim (kurtarma).
    rtx: Option<(PttTx, TransmitKind)>,
    /// Demetin alıcıya ilk varışı.
    arrival: Option<u64>,
    /// Sıralı teslim gecikmesi.
    played: Option<u64>,
}

fn recovery_of(o: &Outcome, bundle: u64, n: usize) -> Recovery {
    let txs: Vec<(PttTx, TransmitKind)> = o
        .ptt_tx
        .iter()
        .copied()
        .filter(|(p, _)| p.unit == bundle)
        .collect();
    let protect: Vec<_> = txs.iter().copied().take(n).collect();
    let s0 = protect.first().map_or(u64::MAX, |(p, _)| p.seq);
    let s_last = protect.last().map_or(u64::MAX, |(p, _)| p.seq);
    let hole_ack = o.acks.iter().copied().find(|a| a.largest > s0);
    let gap_ack = o.acks.iter().copied().find(|a| a.largest > s_last);
    let declared = o
        .emit_log
        .iter()
        .find(|e| {
            e.seq == s0
                && matches!(
                    e.cause,
                    EmitCause::LossPacketThreshold | EmitCause::LossTimeThreshold
                )
        })
        .map(|e| (e.at_ns, e.cause));
    let planned = VOICE_START_NS + bundle * VOICE_PERIOD_NS;
    let k = usize::try_from(bundle).expect("küçük");
    Recovery {
        planned,
        protect,
        hole_ack,
        gap_ack,
        declared,
        rtx: txs.get(n).copied(),
        arrival: o.voice_first.get(k).copied().flatten(),
        played: o.voice.get(k).and_then(|&(_, d)| d),
    }
}

fn print_recovery(tag: &str, o: &Outcome, r: &Recovery) {
    let rel = |t: u64| format!("{:+.3}", (t as f64 - r.planned as f64) / SEC as f64);
    println!(
        "KURTARMA {tag}: demet {} planlı {}",
        (r.planned - VOICE_START_NS) / VOICE_PERIOD_NS,
        secs(r.planned)
    );
    for (p, k) in &r.protect {
        println!(
            "  koruma seq {} {:?} ver {} bit {} varır {}",
            p.seq,
            k,
            rel(p.at),
            p.done.map_or("kuyrukta düştü".to_string(), rel),
            p.delivered
        );
    }
    let ack = |a: &Option<AckRec>| {
        a.map_or("YOK".to_string(), |a| {
            format!(
                "alıcı verdi {} serileşme {} varış {} udp {} en büyük {}",
                rel(a.sent),
                rel(a.start),
                rel(a.arrival),
                a.udp,
                a.largest
            )
        })
    };
    println!("  delik ACK'i: {}", ack(&r.hole_ack));
    println!("  boşluk ACK'i: {}", ack(&r.gap_ack));
    println!("  ilan: {:?}", r.declared.map(|(t, c)| (rel(t), c)));
    println!(
        "  kurtarma: {:?}",
        r.rtx
            .map(|(p, k)| (p.seq, k, rel(p.at), p.done.map(rel), p.delivered))
    );
    println!(
        "  varış {} sıralı teslim {}",
        r.arrival.map_or("YOK".to_string(), rel),
        r.played.map_or("YOK".to_string(), secs)
    );
    let behind: Vec<String> = o
        .voice
        .iter()
        .filter(|&&(p, d)| p > r.planned && d.is_none_or(|d| d > PTT_BOUND_NS))
        .map(|&(p, d)| {
            format!(
                "{}:{}",
                (p - VOICE_START_NS) / VOICE_PERIOD_NS,
                d.map_or("YOK".to_string(), secs)
            )
        })
        .collect();
    println!(
        "  ardındaki 1 s aşan demetler ({}): {:?}",
        behind.len(),
        behind
    );
    let gaps: Vec<u64> = o
        .acks
        .windows(2)
        .map(|w| w[1].arrival - w[0].arrival)
        .collect();
    println!(
        "  ACK sayısı {} aralık min {} max {}  udp min {} max {}",
        o.acks.len(),
        secs(gaps.iter().copied().min().unwrap_or(0)),
        secs(gaps.iter().copied().max().unwrap_or(0)),
        o.acks.iter().map(|a| a.udp).min().unwrap_or(0),
        o.acks.iter().map(|a| a.udp).max().unwrap_or(0),
    );
}

fn all_lost(ack_bw: u64, file_at_ns: u64) -> Scenario {
    Scenario {
        loss_permille: 0,
        targeted_drop: Some((LOST_BUNDLE, PRODUCT_TX)),
        text_at_ns: Some(8_600 * MS),
        file_at_ns,
        voice_until_ns: 20 * SEC,
        ack_return_bw: ack_bw,
        diag: true,
        ..Scenario::honest(1)
    }
}

/// ACK kadansının iki kolu (alıcının ACK bütçesinin hesaplandığı dönüş hızı) ve dosyanın PTT'yle
/// eşzamanlı akıp akmadığı: (ad, ACK bütçesi B/s, dosyanın sunum anı).
const RECOVERY_ARMS: [(&str, u64, u64); 4] = [
    ("seyrek/boşta", PROFILE_DOWN, 0),
    ("sık/boşta", 4 * PROFILE_DOWN, 0),
    ("seyrek/dosya", PROFILE_DOWN, 7 * SEC),
    ("sık/dosya", 4 * PROFILE_DOWN, 7 * SEC),
];

/// Göndericide boşluğu gösteren ACK'ten sonra kurtarma iletiminin hattı terk etmesine dek izin
/// verilen en büyük süre: hatta o an serileşen bir tam datagram (kesilemez) + kurtarma iletiminin
/// kendi serileşmesi (beyan hızında) + bir benzetim adımı.
const HOLE_TO_LINE_EXIT_NS: u64 =
    (MTU + OVH) * SEC / PROFILE_UP + (VOICE_UNIT + OVH) * SEC / PROFILE_UP + MS;

/// [İŞ-35 AÇIK 2 · (1)] Bir PTT demetinin bütün koruma iletimleri (ürün kuralı: 7) düşer. Alıcının
/// deliği gördüğünü gösteren İLK ACK göndericiye vardıktan sonra kurtarma iletimi gecikmeden ve
/// öncelikli çıkar: hattı terk etmesi ≤ ACK varışı + [`HOLE_TO_LINE_EXIT_NS`].
///
/// Tarama: dört kol × 30 demet (30..60; demetin planlı anı ACK kadansına göre kayar). Bazı demetlerde
/// deliği gösteren ilk ACK'in en büyük seq'i asıl iletimden yalnız 1–2 büyüktür (alıcının sırasız
/// varışta verdiği ACK): paket eşiği (3) tutmaz. Düzeltmeden önce bu demetlerin kurtarması zaman
/// eşiğini (9/8·srtt) bekliyordu (129–175 ms; ACK gelmiş, hat boş). Test o kümenin var olduğunu da
/// iddia eder (senaryo kusuru sınıyor).
///
/// Ayrıca: deliği gösteren ACK'ten sonraki 1 s içinde birim tam bir kez yeniden iletilir (erken
/// kayıt ve dedektörün sonraki ilanı çift gönderim üretmez).
///
/// PTO probu deliği gösteren ACK'ten ÖNCE çıktıysa (seyrek kolda ACK aralığı ≈ 5 s, PTO ≈ 3,4 s) bu
/// iddia o demet için boştur ve sayılmaz.
#[test]
fn all_lost_ptt_recovery_leaves_on_the_first_ack_that_shows_the_hole() {
    let mut below_threshold = 0usize;
    let mut checked = 0usize;
    let mut worst: Vec<String> = Vec::new();
    let mut unmarked: Vec<String> = Vec::new();
    let mut doubled: Vec<String> = Vec::new();
    for (tag, bw, fat) in RECOVERY_ARMS {
        for b in 30..60u64 {
            let sc = Scenario {
                targeted_drop: Some((b, PRODUCT_TX)),
                ..all_lost(bw, fat)
            };
            let o = run(&sc);
            let r = recovery_of(&o, b, PRODUCT_TX as usize);
            assert_eq!(
                r.protect.len(),
                PRODUCT_TX as usize,
                "{tag} demet {b}: iletim sayısı"
            );
            assert!(
                r.protect.iter().all(|(p, _)| !p.delivered),
                "{tag} demet {b}: koruma iletimi vardı"
            );
            let hole = r.hole_ack.expect("deliği gösteren ACK");
            let (rtx, _) = r.rtx.expect("kurtarma iletimi");
            assert!(rtx.delivered, "{tag} demet {b}: kurtarma iletimi düştü");
            if rtx.at < hole.arrival {
                continue;
            }
            checked += 1;
            let s0 = r.protect[0].0.seq;
            if hole.largest - s0 < adaptive_transport::loss::PACKET_THRESHOLD {
                below_threshold += 1;
            }
            if hole.largest - s0 < adaptive_transport::loss::PACKET_THRESHOLD
                && !o.emit_log.iter().any(|e| {
                    e.seq == s0 && e.cause == EmitCause::PriorityHeadHole && e.at_ns == hole.arrival
                })
            {
                unmarked.push(format!("{tag} demet {b}"));
            }
            // Erken kayıt ile dedektörün sonraki ilanı birimi iki kez yeniden göndermez: deliği
            // gösteren ACK'ten sonraki 1 s içinde (sonraki ACK ≥ 1 s sonra) tek yeniden iletim.
            let retx = o
                .ptt_tx
                .iter()
                .filter(|(p, k)| {
                    p.unit == b
                        && *k == TransmitKind::Retransmit
                        && p.at >= hole.arrival
                        && p.at < hole.arrival + SEC
                })
                .count();
            if retx != 1 {
                doubled.push(format!("{tag} demet {b}: {retx}"));
            }
            let exit = rtx.done.expect("hatta") - hole.arrival;
            if hole.largest - s0 < adaptive_transport::loss::PACKET_THRESHOLD {
                println!(
                    "İŞ35/1 eşik altı {tag} demet {b}: en büyük−asıl {} ACK varışı {} → hattı terk {} \
                     (+{} ms)",
                    hole.largest - s0,
                    secs(hole.arrival),
                    secs(rtx.done.unwrap_or(0)),
                    exit / MS
                );
            }
            if exit > HOLE_TO_LINE_EXIT_NS {
                worst.push(format!(
                    "{tag} demet {b}: en büyük−asıl {} ACK varışı {} → hattı terk {} (+{} ms)",
                    hole.largest - s0,
                    secs(hole.arrival),
                    secs(rtx.done.unwrap_or(0)),
                    exit / MS
                ));
            }
        }
    }
    println!("İŞ35/1: sınanan {checked} demet, paket eşiğinin altında delik {below_threshold}");
    for w in &worst {
        println!("İŞ35/1 GECİKME {w}");
    }
    assert!(
        below_threshold > 0,
        "senaryo sınamıyor: deliği gösteren ilk ACK hep paket eşiğinde"
    );
    assert!(checked >= 60, "sınanan demet az: {checked}");
    assert!(
        worst.is_empty(),
        "{} demette kurtarma ACK'ten sonra gecikti (sınır {} ms)",
        worst.len(),
        HOLE_TO_LINE_EXIT_NS / MS
    );
    assert!(
        unmarked.is_empty(),
        "şerit başının deliği ACK'te kaydedilmedi: {unmarked:?}"
    );
    assert!(
        doubled.is_empty(),
        "deliği gösteren ACK'ten sonra 1 s içinde yeniden iletim sayısı 1 değil: {doubled:?}"
    );
}

/// Tam kayıptan sonra ACK'li kurtarmanın fiziksel alt sınırı (demetin planlı anına göre değil, asıl
/// iletimin hattı terk ettiği andan): delik alıcıda en erken asıl iletimin varacağı anda görülür
/// (`ONE_WAY`), en küçük ACK (`ack_udp` + 28 B zarf) 62 B/s dönüş hattında serileşir ve göndericiye
/// varır (`ONE_WAY`), kurtarma iletimi (86 + 28 B) beyan hızında serileşir ve alıcıya varır
/// (`ONE_WAY`). Alıcının ACK bekletmesi, ACK bütçesi ve dönüş hattının kuyruğu SIFIR alınır.
fn ack_recovery_floor_ns(ack_udp: u64) -> u64 {
    3 * ONE_WAY_NS + (ack_udp + OVH) * SEC / PROFILE_DOWN + (VOICE_UNIT + OVH) * SEC / PROFILE_UP
}

/// [İŞ-35 AÇIK 2 · (1)(2)] Ürün kuralı, demet 40'ın yedi iletimi de düşer, başka kayıp yok; ACK
/// kadansı seyrek (bütçe 62 B/s'den, ACK aralığı ≈ 5 s) ve sık (4×62, ≈ 1,1 s), dosya boşta ve
/// PTT'yle eşzamanlı. İddialar:
///
/// * Demet, deliği gösteren ilk ACK'in göndericiye varışından sonra en geç bir ileri geçişte
///   (`HOLE_TO_LINE_EXIT_NS` + 300 ms) alıcıdadır (PTO probu daha önce taşıdıysa daha erken).
/// * Varış fiziksel alt sınırın ([`ack_recovery_floor_ns`], o koşudaki en küçük ACK'le) altında
///   değildir (düzenek sağlaması).
/// * Ardındaki demetler yalnız demet 40'ın arkasında bekler: demet k'nin sıralı teslim gecikmesi
///   ≤ max(demet 40'ın varışı − k'nin planı, 1 s); demet 40 vardıktan sonra planlanan her demet
///   ≤ 1 s. Metin ≤ 1 s, dosya ≤ 14 s.
///
/// Gecikmenin ayrıştırması (alıcının bekletmesi, dönüş kuyruğu, ACK serileşmesi, gönderici, ileri
/// geçiş) raporlanır.
#[test]
fn all_lost_ptt_recovery_follows_the_first_ack_and_only_it_blocks_the_lane() {
    for (tag, bw, fat) in RECOVERY_ARMS {
        let sc = all_lost(bw, fat);
        let o = run(&sc);
        report(tag, &sc, &o);
        let r = recovery_of(&o, LOST_BUNDLE, PRODUCT_TX as usize);
        print_recovery(tag, &o, &r);
        let (p0, _) = r.protect[0];
        let p0_done = p0.done.expect("asıl iletim hatta");
        let hole = r.hole_ack.expect("deliği gösteren ACK");
        let (rtx, rtx_kind) = r.rtx.expect("kurtarma");
        let arrival = r.arrival.expect("demet vardı");
        let ack_min = o.acks.iter().map(|a| a.udp).min().expect("ACK");
        let floor = p0_done + ack_recovery_floor_ns(ack_min);
        println!(
            "İŞ35/2 {tag}: asıl hattı terk {} | delik alıcıda en erken {} | ACK verildi {} \
             (alıcı bekletmesi {} ms) serileşme {} (dönüş kuyruğu {} ms) varış {} (ACK {} B, \
             serileşme {} ms) | kurtarma {:?} çıktı {} hattı terk {} | varış {} | alt sınır {} \
             (en küçük ACK {} B) | sıralı teslim {}",
            secs(p0_done),
            secs(p0_done + ONE_WAY_NS),
            secs(hole.sent),
            hole.sent.saturating_sub(p0_done + ONE_WAY_NS) / MS,
            secs(hole.start),
            (hole.start - hole.sent) / MS,
            secs(hole.arrival),
            hole.udp + OVH,
            (hole.udp + OVH) * 1000 / PROFILE_DOWN,
            rtx_kind,
            secs(rtx.at),
            secs(rtx.done.unwrap_or(0)),
            secs(arrival),
            secs(floor),
            ack_min,
            r.played.map_or("YOK".to_string(), secs)
        );
        assert!(rtx.delivered, "{tag}: kurtarma iletimi düştü");
        assert!(
            arrival >= floor,
            "{tag}: varış fiziksel alt sınırın altında"
        );
        assert!(
            arrival <= hole.arrival + HOLE_TO_LINE_EXIT_NS + ONE_WAY_NS,
            "{tag}: demet ilk ACK'ten bir ileri geçiş sonra yok ({} > {})",
            secs(arrival),
            secs(hole.arrival + HOLE_TO_LINE_EXIT_NS + ONE_WAY_NS)
        );
        for &(planned, d) in o.voice.iter().filter(|&&(p, _)| p > r.planned) {
            let d = d.expect("demet teslim edildi");
            let behind = arrival.saturating_sub(planned);
            assert!(
                d <= behind.max(PTT_BOUND_NS),
                "{tag}: demet {} ayrıca bekledi ({} > {})",
                (planned - VOICE_START_NS) / VOICE_PERIOD_NS,
                secs(d),
                secs(behind)
            );
        }
        assert!(
            o.voice_max_from(arrival) <= PTT_BOUND_NS,
            "{tag}: kurtarmadan sonra PTT {}",
            secs(o.voice_max_from(arrival))
        );
        assert!(
            o.text_delay_ns.is_some_and(|d| d <= TEXT_BOUND_NS),
            "{tag}: metin {:?}",
            o.text_delay_ns
        );
        let file = o.file_done_ns.expect("dosya bitti");
        assert!(file <= FILE_BOUND_NS + fat, "{tag}: dosya {}", secs(file));
    }
    // Rapor: aynı tam kayıp, dürüst profilin %20 bağımsız kaybıyla (kurtarma iletimi de düşebilir).
    for (tag, bw) in [("seyrek", PROFILE_DOWN), ("sık", 4 * PROFILE_DOWN)] {
        for seed in 1..=3u64 {
            let sc = Scenario {
                loss_permille: 200,
                seed,
                ..all_lost(bw, 7 * SEC)
            };
            let o = run(&sc);
            let r = recovery_of(&o, LOST_BUNDLE, PRODUCT_TX as usize);
            println!(
                "İŞ35/2 %20 kayıp {tag} tohum {seed}: demet 40 sıralı teslim {} (kurtarmaya dek iletim {}) \
                 PTT max {} dosya {}",
                r.played.map_or("YOK".to_string(), secs),
                o.ptt_tx
                    .iter()
                    .filter(|(p, _)| p.unit == LOST_BUNDLE && r.arrival.is_none_or(|a| p.at <= a))
                    .count(),
                secs(o.voice_max_from(0)),
                o.file_done_ns.map_or("YOK".to_string(), secs)
            );
        }
    }
}

/// [İŞ-35 AÇIK 2 · ÇÖZÜLMEDİ] Bütün koruma iletimleri düşen demet PTT sınırında (1 s) teslim
/// edilmez; bu test o iddiayı yapar ve KIRMIZIDIR (gizlenmez).
///
/// Fiziksel alt sınır ([`ack_recovery_floor_ns`]; varsayımlar: tek yön 300 ms üç geçiş — asıl
/// iletim, ACK, kurtarma —, dönüş 62 B/s, alıcı deliği asıl iletimin varacağı anda görür ve
/// beklemeden ACK'ler, ACK bütçesi ve dönüş kuyruğu yok, ileri beyan 12 500 B/s):
///
/// * yalnız zarflı (28 B, gövdesiz) bir ACK'le: 3 × 300 + 28 / 62 s + 114 / 12 500 s ≈ 1 361 ms;
/// * bu düzenekte görülen en küçük ACK'le (37 + 28 B): 900 + 1 048 + 9 ≈ 1 957 ms.
///
/// İkisi de PTT sınırının (1 s, planlı andan) üstündedir; ACK'li kurtarmayla 1 s bu varsayımlar
/// altında mümkün değildir. ACK kadansı payı da eklenir: seyrek kolda ACK aralığı ≈ 5 s, sık kolda
/// ≈ 1,1 s; dönüş hattında önceki ACK'in serileşmesi ≤ 1,3 s. Sınırı tutmanın yolu ACK'siz
/// korumadır (iletim sayısı, aralığı ya da FEC); bu görevin kapsamı dışında (kopya sayısı
/// artırılmaz).
#[test]
fn all_lost_ptt_bundle_within_the_1s_bound() {
    let header_only = ack_recovery_floor_ns(0);
    assert!(
        header_only > PTT_BOUND_NS,
        "alt sınır 1 s'nin altında: {}",
        secs(header_only)
    );
    let mut over = Vec::new();
    for (tag, bw, fat) in RECOVERY_ARMS {
        let o = run(&all_lost(bw, fat));
        let r = recovery_of(&o, LOST_BUNDLE, PRODUCT_TX as usize);
        let d = r.played.unwrap_or(u64::MAX);
        if d > PTT_BOUND_NS {
            over.push(format!("{tag}: {}", secs(d)));
        }
    }
    assert!(
        over.is_empty(),
        "ÇÖZÜLMEDİ: tam kayıpta demet 1 s'yi aşıyor ({}); ACK'li kurtarmanın alt sınırı {} \
         (gövdesiz ACK) / {} (37 B ACK)",
        over.join(", "),
        secs(header_only),
        secs(ack_recovery_floor_ns(37))
    );
}

/// [İŞ-35 AÇIK 2 · bant bedeli] 7 iletim (ürün kuralı: 6 kopya, 200/7 ms, ölçülen yavaş hatta en az
/// 6) ve 5 iletim (2026-11-16 kuralı: 4 kopya, 40 ms, ölçeksiz): ileri hatta PTT tel baytı ve payı,
/// dürüst dosya süresi (%20 kayıp, 41 kB, sık ACK kolu 4×62; düzenekte FEC yok, kurtarma yalnız
/// ARQ), daralma senaryosunda (12 500 → 6 250, 8 s) daralma başlangıcından en kötü PTT. Rapor;
/// sağlama yalnız bayt sırası.
#[test]
fn ptt_band_cost_seven_versus_five_transmissions() {
    let arms: [(&str, u8, u64, Option<u8>); 2] = [
        (
            "7 iletim",
            PRODUCT_COPIES,
            PRODUCT_SPACING_NS,
            Some(PRODUCT_MIN_COPIES),
        ),
        ("5 iletim", OLD_COPIES, OLD_SPACING_NS, None),
    ];
    let voice_until = 20 * SEC;
    let mut bytes = [0u64; 2];
    for (i, &(tag, copies, spacing_ns, scale)) in arms.iter().enumerate() {
        for seed in 1..=3u64 {
            let sc = Scenario {
                voice_until_ns: voice_until,
                ack_return_bw: 4 * PROFILE_DOWN,
                copies,
                spacing_ns,
                copy_scale_min: scale,
                ..Scenario::honest(seed)
            };
            let o = run(&sc);
            bytes[i] += o.ptt_wire_bytes;
            let voice_s = (voice_until - VOICE_START_NS) as f64 / SEC as f64;
            let ptt_bps = o.ptt_wire_bytes as f64 / voice_s;
            let drop = Scenario {
                copies,
                spacing_ns,
                copy_scale_min: scale,
                ..drop_scenario(seed, true)
            };
            let od = run(&drop);
            println!(
                "İŞ35/BANT {tag} tohum {seed}: PTT tel {} B ({:.0} B/s, hattın %{:.1}) dosya {} PTT max {} | \
                 daralma: PTT max (8 s'den) {} dosya {} kuyruk_düşen {}",
                o.ptt_wire_bytes,
                ptt_bps,
                100.0 * ptt_bps / PROFILE_UP as f64,
                o.file_done_ns.map_or("YOK".to_string(), secs),
                secs(o.voice_max_from(0)),
                secs(od.voice_max_from(DROP_AT_NS)),
                od.file_done_ns.map_or("YOK".to_string(), secs),
                od.drops_queue
            );
        }
    }
    assert!(bytes[0] > bytes[1], "7 iletim 5'ten az bayt: {bytes:?}");
}

/// [İŞ-35 GECE 3 · 4] Hattın toparlanma anı (düşüş-toparlanma hücresi).
const RECOVER_AT_NS: u64 = 20 * SEC;

/// PTT tek iletim biçiminin UDP gövdesi (B): 106 B tel (#175 7070372531) − zarf.
const PTT_SINGLE_UNIT: u64 = 106 - OVH;

/// Düşüş-toparlanma hücresinin PTT yükü ([`drop_recovery`]).
#[derive(Debug, Clone, Copy)]
enum PttLoad {
    /// Bugünkü ürün kopya kuralı ([`PRODUCT_COPIES`] iletim).
    Copies,
    /// Demet başına tek iletim, 106 B tel (koordinatörün bu geceki PTT biçimi).
    Single,
    /// PTT yok (yalnız dosya).
    None,
}

/// Düşüş-toparlanma: dürüst 12 500, 8 s'de 6 250, 20 s'de yine 12 500; dosya sürekli (gönderici
/// tavanda; 600 birim 40 s'de bitmez), (b)'nin geri kalanı, PTT yükü `load`.
fn drop_recovery(seed: u64, load: PttLoad) -> Scenario {
    let base = Scenario {
        fwd_plan: vec![
            (0, PROFILE_UP),
            (DROP_AT_NS, DROP_TO),
            (RECOVER_AT_NS, PROFILE_UP),
        ],
        file_units: 600,
        t_max_ns: 40 * SEC,
        ..drop_scenario(seed, true)
    };
    match load {
        PttLoad::Copies => base,
        PttLoad::Single => Scenario {
            copies: 0,
            copy_scale_min: None,
            voice_unit: PTT_SINGLE_UNIT,
            ..base
        },
        PttLoad::None => Scenario {
            voice_until_ns: 0,
            ..base
        },
    }
}

/// [İŞ-35 GECE 3 · 4] Düşüş ve toparlanma (dürüst 12 500 → 8 s'de 6 250 → 20 s'de 12 500, dosya
/// sürekli, tohum 1–3; üç PTT yükü: bugünkü kopya kuralı, tek iletim, PTT yok). Sayım: olaydan
/// SONRA serileşmeye başlamış bir iletimi en büyük teyitli seq olarak taşıyan ACK'ler (göndericiye
/// varış sırasıyla; olaydan önceki iletimi teyit eden ACK olay hakkında bilgi taşıyamaz).
///
/// PTT'siz yükte alıcının çift alt sınırı (`pair_rate`) yalnız göndericinin kendi çiftlerinden
/// gelir (`pacer::TokenBucket::set_line_pairs`): dosya–dosya ardışık iletimi tekli pacing'de
/// yalnız gönderim hızını verir.
/// * (4) Bu ACK'lerin en çok ikincisinde korunan hız 6 250 ± %10 (ilkinin ölçüm aralığı bir ACK
///   geriye, olaydan öncesine uzanabilir; ikincininki tamamen olaydan sonradır).
/// * (3) Toparlanmadan sonra en çok üçüncüsünden itibaren gönderim tavanı kalıcı olarak (koşu
///   sonuna dek her ACK'te) ≥ %95 × 12 500.
/// * Yükseltmeden sonra darboğaz beklemesi ≤ iki tam datagramın 12 500'de serileşmesi (yanlış
///   yükseltme kuyruk kurmaz).
#[test]
fn drop_and_recovery_are_followed_within_two_and_three_acks() {
    let ser2 = 2 * (MTU + OVH) * SEC / PROFILE_UP;
    let mut bad = Vec::new();
    for (load, seed) in [PttLoad::Copies, PttLoad::Single, PttLoad::None]
        .into_iter()
        .flat_map(|l| (1..=3u64).map(move |s| (l, s)))
    {
        let sc = drop_recovery(seed, load);
        let o = run(&sc);
        report(&format!("düşüş-toparlanma/{load:?}"), &sc, &o);
        let after = |from: u64, to: u64| {
            o.per_ack
                .iter()
                .filter(move |a| a.1 >= from && a.1 < to)
                .copied()
                .collect::<Vec<_>>()
        };
        let down = after(DROP_AT_NS, RECOVER_AT_NS);
        let up = after(RECOVER_AT_NS, u64::MAX);
        let down_k = down.iter().position(|a| {
            a.2.is_some_and(|r| r * 10 <= DROP_TO * 11 && r * 10 >= DROP_TO * 9)
        });
        // Kalıcı dönüş: bu ACK'ten sonra tavan hiç %95'in altına inmez (geçici yoklama sayılmaz).
        let up_k = (0..up.len()).find(|&k| up[k..].iter().all(|a| a.3 * 100 >= PROFILE_UP * 95));
        let down_at = down_k.map(|k| down[k].0);
        let up_at = up_k.map(|k| up[k].0);
        let q_down = o.max_wait_between(DROP_AT_NS, down_at.unwrap_or(u64::MAX));
        let w_up = up_at.map(|t| o.max_wait_between(t, u64::MAX));
        println!(
            "DÜŞÜŞ-TOPARLANMA {load:?} tohum {seed}: düşüş ACK#{:?} ({}) kuyruk o ana dek {} | toparlanma \
             ACK#{:?} ({}) sonrası bekleme {} (sınır {}) | düşüş izi {:?} | toparlanma izi {:?}",
            down_k.map(|k| k + 1),
            down_at.map_or("YOK".to_string(), secs),
            secs(q_down),
            up_k.map(|k| k + 1),
            up_at.map_or("YOK".to_string(), secs),
            w_up.map_or("-".to_string(), secs),
            secs(ser2),
            down.iter()
                .take(4)
                .map(|a| (secs(a.0), a.2, a.3))
                .collect::<Vec<_>>(),
            up.iter()
                .take(5)
                .map(|a| (secs(a.0), a.2, a.3))
                .collect::<Vec<_>>(),
        );
        if down_k.is_none_or(|k| k >= 2) {
            bad.push(format!(
                "{load:?} tohum {seed}: düşüş ACK#{:?}",
                down_k.map(|k| k + 1)
            ));
        }
        if up_k.is_none_or(|k| k >= 3) {
            bad.push(format!(
                "{load:?} tohum {seed}: toparlanma ACK#{:?}",
                up_k.map(|k| k + 1)
            ));
        }
        if w_up.is_some_and(|w| w > ser2) {
            bad.push(format!(
                "{load:?} tohum {seed}: yükseltmeden sonra bekleme {}",
                secs(w_up.unwrap_or(0))
            ));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

// ---------------------------------------------------------------------------------------------
// [PTT-YALIN · sahip hedefi 2026-11-22] PTT tek iletim, yalın 0x09 çerçevesi, son tarihte bırakma.
// ---------------------------------------------------------------------------------------------

/// Yalın PTT biriminin UDP gövdesi: 22 B zarf + 3 B 0x09 başlığı + 53 B (tel 106 B).
const LEAN_PTT_UNIT: u64 = 78;
/// Ürünün PTT son tarihi (`client_core::runtime::PTT_DEADLINE_NS`).
const PTT_DEADLINE_NS: u64 = SEC;
/// Sahip bütçesi: PTT bir yönde ≤ 4,5 kbit/s (UDP/IP dahil).
const PTT_BUDGET_BPS: u64 = 4_500;

/// Bir kolun PTT ölçüsü.
#[derive(Debug)]
struct LeanPtt {
    /// PTT iletimleri (bütün kökenler).
    tx: usize,
    /// Kökeni `Data` olmayan PTT iletimleri (ARQ yeniden iletimi, kopya, PTO probu).
    retx: usize,
    /// Bunlardan kökeni `Retransmit` (ARQ / PTO) ve `Duplicate` (kopya) olanlar.
    retx_arq: usize,
    retx_dup: usize,
    /// Kökeni `Data` olmayan ve alıcıya sunumundan ≤ son tarih içinde varan iletim.
    retx_in_time: usize,
    /// Konuşma süresine bölünmüş bütün PTT tel bitleri (bit/s).
    bps: u64,
    /// Sunumundan ≥ son tarih sonra verilen PTT iletimi.
    late_tx: usize,
    /// Bırakmadan sonra bırakılan birimin iletimi (seq bırakma anındaki en büyük seq'ten büyük).
    after_expiry_tx: usize,
    /// Bir `Retransmit` iletiminin sunumdan en erken gecikmesi.
    first_retx_ns: Option<u64>,
    /// Alıcıya varan demetler ve en kötü ilk varış gecikmesi.
    arrived: usize,
    worst_ns: u64,
}

fn lean_ptt(o: &Outcome, voice_until_ns: u64) -> LeanPtt {
    let planned = |u: u64| VOICE_START_NS + u * VOICE_PERIOD_NS;
    let tx = o.ptt_tx.len();
    // Ölçü aracının sınaması: her PTT iletimi yalın birimin tel boyunda.
    assert_eq!(
        o.ptt_wire_bytes,
        tx as u64 * (LEAN_PTT_UNIT + OVH),
        "PTT iletim boyu"
    );
    let retx = o
        .ptt_tx
        .iter()
        .filter(|(_, k)| *k != TransmitKind::Data)
        .count();
    let late_tx = o
        .ptt_tx
        .iter()
        .filter(|(p, _)| p.at >= planned(p.unit) + PTT_DEADLINE_NS)
        .count();
    let after_expiry_tx = o
        .ptt_tx
        .iter()
        .filter(|(p, _)| {
            o.ptt_expired
                .iter()
                .any(|&(u, _, last)| u == p.unit && p.seq > last)
        })
        .count();
    let kind_count = |kind| o.ptt_tx.iter().filter(|(_, k)| *k == kind).count();
    let retx_in_time = o
        .ptt_tx
        .iter()
        .filter(|(p, k)| {
            *k != TransmitKind::Data
                && p.delivered
                && p.done
                    .is_some_and(|d| d + ONE_WAY_NS <= planned(p.unit) + PTT_DEADLINE_NS)
        })
        .count();
    let first_retx_ns = o
        .ptt_tx
        .iter()
        .filter(|(_, k)| *k == TransmitKind::Retransmit)
        .map(|(p, _)| p.at - planned(p.unit))
        .min();
    let delays: Vec<u64> = o
        .voice_first
        .iter()
        .enumerate()
        .filter_map(|(k, a)| a.map(|a| a - planned(k as u64)))
        .collect();
    LeanPtt {
        tx,
        retx,
        retx_arq: kind_count(TransmitKind::Retransmit),
        retx_dup: kind_count(TransmitKind::Duplicate),
        retx_in_time,
        bps: o.ptt_wire_bytes * 8 * SEC / (voice_until_ns - VOICE_START_NS),
        late_tx,
        after_expiry_tx,
        first_retx_ns,
        arrived: delays.len(),
        worst_ns: delays.iter().copied().max().unwrap_or(0),
    }
}

/// [PTT-YALIN] Ürün profili (dürüst 12 500, %20 iid kayıp, ACK bütçesi 62 B/s, 300 ms tek yön), dosya
/// yok; PTT 0,5–15,5 s, 200 ms'de bir 78 B gövde (tel 106 B), kural `copies = 0` + öncelik
/// (`ptt_forward_copy_rule(0, 0)`, ölçek yok); 5 s'de bir öncelikli metin birimi. Bırakma kolu:
/// sunumundan 1 s sonra teyitsiz PTT birimi `RecoveredControl` ile bırakılır. Tohum başına:
/// (a) PTT tel hızı (bütün iletimler / 15 s) ≤ 4 500 bit/s; (b) bırakılan birim bırakmadan sonra
/// tele çıkmaz (köken ne olursa olsun); (c) öncelik takılmadı: her metnin ilk iletimi sunumundan en
/// geç bir tam datagramın beyan hattında serileşmesi kadar sonra tele çıkar ve ilk iletimi hatta
/// düşmeyen metin sunumundan ≤ 1 s içinde teslim edilir.
///
/// (c) "her metin ≤ 1 s" olarak SAĞLANAMAZ ve öyle iddia edilmez: ilk iletimi %20 iid kayıpta düşen
/// metnin kurtarması ya ACK turunu (geri bildirim ≥ 1 s) ya da göndericinin 1 s sonraki canlılık
/// kopyasını bekler; ölçüm: tohum 1 bırakma kolunda üç metnin üçünün de ilk iletimi düştü, teslim
/// 1,31 s (kopya 1,00 s + 0,31 s yayılım/serileşme). Teslim gecikmeleri raporlanır.
///
/// Karşı-olgu (aynı tohum, bırakma yok; #175 H1'in ölçümü): PTT tel hızı ve yeniden iletim sayısı
/// raporlanır, sınırla kıyaslanmaz. Sağlanan yalnız: sunumundan ≥ 1 s sonra tele çıkan PTT iletimi
/// vardır (bırakılmayan birim yeniden iletilir).
#[test]
fn lean_ptt_one_transmission_stays_within_budget_and_expired_units_are_not_retransmitted() {
    let voice_until = 15_500 * MS;
    for seed in 1..=3u64 {
        let sc = Scenario {
            file_units: 0,
            voice_until_ns: voice_until,
            text_at_ns: Some(5 * SEC),
            text_period_ns: 5 * SEC,
            copies: 0,
            spacing_ns: 0,
            copy_scale_min: None,
            raw_copy_rule: true,
            voice_unit: LEAN_PTT_UNIT,
            ptt_deadline_ns: Some(PTT_DEADLINE_NS),
            ..Scenario::honest(seed)
        };
        let o = run(&sc);
        let m = lean_ptt(&o, voice_until);
        let control = Scenario {
            ptt_deadline_ns: None,
            ..sc.clone()
        };
        let oc = run(&control);
        let mc = lean_ptt(&oc, voice_until);
        let texts: Vec<String> = o
            .text_delays
            .iter()
            .map(|&(at, d)| format!("{}:{}", secs(at), d.map_or("YOK".to_string(), secs)))
            .collect();
        let texts_c: Vec<String> = oc
            .text_delays
            .iter()
            .map(|&(at, d)| format!("{}:{}", secs(at), d.map_or("YOK".to_string(), secs)))
            .collect();
        println!(
            "PTT-YALIN tohum {seed} bırakma: tel {} bit/s, iletim {}, yeniden {} (ARQ {}, kopya {}, \
             vaktinde varan {}), bırakılan {}, bırakma sonrası iletim {}, son tarih sonrası iletim \
             {}, ilk ARQ {}, varan {}/{} (en kötü {}), metin {texts:?} | karşı-olgu: tel {} bit/s, \
             iletim {}, yeniden {} (ARQ {}, kopya {}, vaktinde varan {}), son tarih sonrası iletim \
             {}, ilk ARQ {}, varan {}/{} (en kötü {}), metin {texts_c:?}",
            m.bps,
            m.tx,
            m.retx,
            m.retx_arq,
            m.retx_dup,
            m.retx_in_time,
            o.ptt_expired.len(),
            m.after_expiry_tx,
            m.late_tx,
            m.first_retx_ns.map_or("-".to_string(), secs),
            m.arrived,
            o.voice_first.len(),
            secs(m.worst_ns),
            mc.bps,
            mc.tx,
            mc.retx,
            mc.retx_arq,
            mc.retx_dup,
            mc.retx_in_time,
            mc.late_tx,
            mc.first_retx_ns.map_or("-".to_string(), secs),
            mc.arrived,
            oc.voice_first.len(),
            secs(mc.worst_ns),
        );
        assert!(
            m.bps <= PTT_BUDGET_BPS,
            "tohum {seed}: PTT tel {} bit/s > {PTT_BUDGET_BPS}",
            m.bps
        );
        assert_eq!(
            m.after_expiry_tx, 0,
            "tohum {seed}: bırakılan birim yeniden tele çıktı"
        );
        assert_eq!(o.text_delays.len(), 3, "tohum {seed}: metin sayısı");
        for (k, &(at, d)) in o.text_delays.iter().enumerate() {
            let first = o
                .text_tx
                .iter()
                .filter(|p| p.unit == TEXT_ID + k as u64)
                .min_by_key(|p| p.seq)
                .expect("metin tele çıktı");
            assert!(
                first.at - at <= (MTU + OVH) * SEC / PROFILE_UP,
                "tohum {seed}: metin {k} ilk iletimi {} gecikti",
                secs(first.at - at)
            );
            assert!(
                !first.delivered || d.is_some_and(|d| d <= TEXT_BOUND_NS),
                "tohum {seed}: ilk iletimi varan metin {k}: {texts:?}"
            );
        }
        assert!(
            mc.late_tx > 0,
            "tohum {seed}: karşı-olgu: son tarihten sonra PTT iletimi yok"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// [BUA] Ürün alıcısı + anahtarlı kayıp + darboğaz defteri: rapor kolları (yalnız sayı basar).
// ---------------------------------------------------------------------------------------------

/// [BUA] Düşüş-toparlanma kolunun düşüş ve toparlanma anları.
const BUA_DROP_AT_NS: u64 = 30 * SEC;
const BUA_RECOVER_AT_NS: u64 = 45 * SEC;
const BUA_T_MAX_NS: u64 = 60 * SEC;

/// [BUA] Rapor kolu: (ad, tohumdan senaryo).
type BuaArm = (&'static str, fn(u64) -> Scenario);

/// [BUA] Rapor kolları. Yük bugünkü ürün kopya kuralıdır (yalın PTT değil).
fn bua_arms() -> [BuaArm; 4] {
    [
        // (a) 12 500 → 30 s'de 6 250 → 45 s'de 12 500; dosya koşu boyunca tavanda.
        ("dusus-toparlanma", |seed| Scenario {
            fwd_plan: vec![
                (0, PROFILE_UP),
                (BUA_DROP_AT_NS, DROP_TO),
                (BUA_RECOVER_AT_NS, PROFILE_UP),
            ],
            file_units: 900,
            voice_until_ns: BUA_T_MAX_NS,
            t_max_ns: BUA_T_MAX_NS,
            ..drop_scenario(seed, true)
        }),
        // (b) Dürüst sabit 6 250 (beyan 12 500).
        ("sabit-6250", |seed| Scenario {
            fwd_plan: vec![(0, DROP_TO)],
            ..drop_scenario(seed, true)
        }),
        // (c) Var olan (b) ve (d-2) testlerinin senaryoları.
        ("b", |seed| Scenario {
            text_at_ns: Some(DROP_TEXT_AT_NS),
            ..drop_scenario(seed, true)
        }),
        ("d2", |seed| drop_scenario(seed, true)),
    ]
}

/// [BUA] PTT evreleri (ileri hat çizelgesinin basamakları): `ad:zamanında/sunulan` (zamanında =
/// ilk varış ≤ plan + 1 s).
fn bua_ptt_phases(sc: &Scenario, o: &Outcome) -> String {
    let mut edges: Vec<u64> = sc.fwd_plan.iter().map(|&(t, _)| t).collect();
    edges.push(u64::MAX);
    edges
        .windows(2)
        .map(|w| {
            let (mut n, mut ok) = (0, 0);
            for (k, a) in o.voice_first.iter().enumerate() {
                let plan = VOICE_START_NS + k as u64 * VOICE_PERIOD_NS;
                if plan >= w[0] && plan < w[1] {
                    n += 1;
                    ok += u32::from(a.is_some_and(|a| a <= plan + PTT_BOUND_NS));
                }
            }
            format!("{}s:{ok}/{n}", w[0] / SEC)
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// [BUA] Kolun özet ölçüleri (FARK satırı için).
#[derive(Debug, Clone)]
struct BuaSummary {
    acks: usize,
    ack_gap_mean_ms: u64,
    ack_wire: u64,
    guard_end: Option<u64>,
    pace_min: u64,
    drops: usize,
    wait_max_ms: u64,
    ptt: String,
    file: String,
}

fn bua_opt(v: Option<u64>) -> String {
    v.map_or("-".to_string(), |v| v.to_string())
}

fn bua_s(v: Option<u64>) -> String {
    v.map_or("YOK".to_string(), |v| {
        format!("{:.2}", v as f64 / SEC as f64)
    })
}

/// [BUA] Bir koşunun bütün satırları; özet döner.
#[allow(clippy::too_many_lines)]
fn bua_print(arm: &str, sc: &Scenario, o: &Outcome) -> BuaSummary {
    let rx = sc.rx.label();
    let seed = sc.seed;
    let gaps: Vec<u64> = o.acks_at.windows(2).map(|w| w[1] - w[0]).collect();
    let gap_mean = if gaps.is_empty() {
        0
    } else {
        gaps.iter().sum::<u64>() / gaps.len() as u64
    };
    let types = o
        .ack_types
        .iter()
        .map(|(t, n)| format!("{t:02x}:{n}"))
        .collect::<Vec<_>>()
        .join(",");
    let sum = BuaSummary {
        acks: o.acks_at.len(),
        ack_gap_mean_ms: gap_mean / MS,
        ack_wire: o.ack_wire_bytes,
        guard_end: o.rate_at_end(),
        pace_min: o.min_pace(),
        drops: o.drops_queue,
        wait_max_ms: o.line.max_wait_ns / MS,
        ptt: bua_ptt_phases(sc, o),
        file: bua_s(o.file_done_ns),
    };
    println!(
        "BUA {arm} {rx} tohum={seed} kayip=anahtarli ack={} ack_olculu={} ack_aralik_ort_ms={} \
         ack_aralik_max_ms={} ack_tel_B={} ack_tur={types} koruma_son={} koruma_min={} \
         tavan_min={} kuyruk_max_ms={} kuyruk_dusen={} fifo_max_pkt={} fifo_max_B={} dosya_s={} \
         ptt_tel_B={} ptt_zamaninda={} ptt_max_s={}",
        sum.acks,
        o.ack_measured.iter().filter(|&&m| m).count(),
        sum.ack_gap_mean_ms,
        gaps.iter().copied().max().unwrap_or(0) / MS,
        sum.ack_wire,
        bua_opt(sum.guard_end),
        bua_opt(o.min_rate()),
        sum.pace_min,
        sum.wait_max_ms,
        sum.drops,
        o.line.max_occ_pkts,
        o.line.max_occ_bytes,
        sum.file,
        o.ptt_wire_bytes,
        sum.ptt,
        bua_s(Some(o.voice_max_from(0)).filter(|&v| v != u64::MAX)),
    );
    let l = o.line;
    println!(
        "BUA-HAT {arm} {rx} tohum={seed} sunulan_B={} sunulan_pkt={} serilesen_B={} \
         kuyrukta_dusen_B={} hatta_dusen_B={} yayilan_B={} fifo_max_pkt={} fifo_max_B={} \
         bekleme_max_ms={}",
        l.offered_bytes,
        l.offered_pkts,
        l.serialized_bytes,
        l.queue_drop_bytes,
        l.lost_bytes,
        l.delivered_bytes,
        l.max_occ_pkts,
        l.max_occ_bytes,
        l.max_wait_ns / MS
    );
    for ((class, payload), b) in &o.book {
        println!(
            "BUA-DEFTER {arm} {rx} tohum={seed} sinif={} yuk={} paket={} bayt={} \
             kuyrukta_dusen_pkt={} kuyrukta_dusen_B={} hatta_dusen_pkt={} hatta_dusen_B={}",
            class.label(),
            format!("{payload:?}").to_lowercase(),
            b.pkts,
            b.bytes,
            b.queue_drop_pkts,
            b.queue_drop_bytes,
            b.lost_pkts,
            b.lost_bytes
        );
    }
    println!(
        "BUA-TAVAN {arm} {rx} tohum={seed} iz={}",
        o.ceil_trace
            .iter()
            .map(|&(t, p, k)| format!("{}:{p}:{k}", t / MS))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "BUA-BIRIKIM {arm} {rx} tohum={seed} iz={}",
        o.backlog
            .iter()
            .map(|&(t, u, b)| format!("{}:{u}:{b}", t / SEC))
            .collect::<Vec<_>>()
            .join(",")
    );
    // Var olan (b) ve (d-2) testlerinin ölçüleri, kendi sınırlarıyla yan yana (karar yok).
    if arm == "b" || arm == "d2" {
        let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
        let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max();
        let conv = converged_at(o, DROP_TO);
        let after = |d: u64| conv.map(|c| c + d);
        if arm == "b" {
            println!(
                "BUA-B {arm} {rx} tohum={seed} yakinsama_s={} yakinsama_siniri_s={} \
                 koruma_son={} bekleme_yakinsama+2s_ms={} bekleme_siniri_ms={} \
                 ptt_yakinsama+9s_s={} ptt_dususten_s={} metin_s={}",
                bua_s(conv),
                bua_s(t_ack.map(|t| DROP_AT_NS + 3 * t + t_ret)),
                bua_opt(o.rate_at_end()),
                after(2 * SEC).map_or("-".to_string(), |t| (o.max_wait_between(t, u64::MAX) / MS)
                    .to_string()),
                wait_bound / MS,
                bua_s(after(9 * SEC).map(|t| o.voice_max_from(t))),
                bua_s(Some(o.voice_max_from(DROP_AT_NS))),
                bua_s(o.text_delay_ns),
            );
        } else {
            let fifo_ns = 64 * (VOICE_UNIT + OVH) * SEC / DROP_TO;
            println!(
                "BUA-D2 {arm} {rx} tohum={seed} kuyruk_dusen={} yakinsama_s={} \
                 bekleme_yakinsama+2s_ms={} fifo_siniri_ms={}",
                o.drops_queue,
                bua_s(conv),
                after(2 * SEC).map_or("-".to_string(), |t| (o.max_wait_between(t, u64::MAX) / MS)
                    .to_string()),
                fifo_ns / MS
            );
        }
    }
    sum
}

/// [BUA] Rapor kolları: dört kol (düşüş-toparlanma 12 500 → 6 250 [30 s] → 12 500 [45 s]; dürüst
/// sabit 6 250; (b) ve (d-2) testlerinin senaryoları) × tohum 1–3 × alıcı, hepsi ANAHTARLI kayıpla
/// ve defter açık. Alıcılar: `urun` (ürün çalışma zamanının kurulumu, bütçe 62 B/s), `ayna` (var
/// olan testlerin melez alıcısı, bütçe senaryonun `ack_return_bw`'si: bu kollarda 4 × 62 B/s, sık
/// ACK'in yerine geçen kurulum) ve ayrıştırma kolu `urun-bw248` (ürün alıcısı, aynanın bütçesiyle:
/// farkın ne kadarı ACK bütçesinden, ne kadarı alıcı kuralından). Yalnız rapor basar (biçim: modül
/// belgesi); kabul iddiası yok. Sağlanan yalnız ölçüm aracı: defter hat sayaçlarıyla birebir
/// (`ledger_check`, her koşuda), defter kapalı aynı koşu aynı ACK/kuyruk/tavan/PTT/dosya izini
/// verir (defter davranışı değiştirmez), ürün alıcısı yalnız 0x15, ayna yalnız 0x14 yazdı. Fark satırı
/// (`BUA-FARK`): `urun/urun-bw248/ayna`.
#[test]
fn bua_product_receiver_report() {
    // Satırlar sütun 0'dan başlasın (`--nocapture`'da test adı satırının ardına yazılmasın).
    println!();
    for (arm, make) in bua_arms() {
        for seed in 1..=3u64 {
            let mut sums = Vec::new();
            let mirror_bw = make(seed).ack_return_bw;
            for rx in [
                RxModel::PRODUCT,
                RxModel::Product {
                    ptt_lean: false,
                    return_bw: mirror_bw,
                },
                RxModel::Mirror,
            ] {
                let sc = Scenario {
                    rx,
                    loss: LossModel::Keyed,
                    ledger: true,
                    ..make(seed)
                };
                let o = run(&sc);
                // Aracın sağlaması: defter (emisyon izinin boşaltılması) göndericinin ve hattın
                // davranışını değiştirmez.
                let bare = run(&Scenario {
                    ledger: false,
                    ..sc.clone()
                });
                assert!(
                    o.acks_at == bare.acks_at
                        && o.waits == bare.waits
                        && o.drop_times == bare.drop_times
                        && o.pace_trace == bare.pace_trace
                        && o.voice_first == bare.voice_first
                        && o.file_done_ns == bare.file_done_ns
                        && o.ptt_wire_bytes == bare.ptt_wire_bytes
                        && o.line.offered_bytes == bare.line.offered_bytes
                        && o.line.lost_bytes == bare.line.lost_bytes,
                    "{arm} {} tohum {seed}: defter davranışı değiştirdi",
                    rx.label()
                );
                let want = if rx == RxModel::Mirror { 0x14 } else { 0x15 };
                assert!(
                    o.ack_types.keys().all(|&t| t == want) && !o.ack_types.is_empty(),
                    "{arm} {} tohum {seed}: ACK türleri {:?}",
                    rx.label(),
                    o.ack_types
                );
                sums.push(bua_print(arm, &sc, &o));
            }
            let f = |g: fn(&BuaSummary) -> String| sums.iter().map(g).collect::<Vec<_>>().join("/");
            println!(
                "BUA-FARK {arm} tohum={seed} alicilar=urun/urun-bw{mirror_bw}/ayna ack={} \
                 ack_aralik_ort_ms={} ack_tel_B={} koruma_son={} tavan_min={} kuyruk_dusen={} \
                 kuyruk_max_ms={} ptt_zamaninda={} dosya_s={}",
                f(|x| x.acks.to_string()),
                f(|x| x.ack_gap_mean_ms.to_string()),
                f(|x| x.ack_wire.to_string()),
                f(|x| bua_opt(x.guard_end)),
                f(|x| x.pace_min.to_string()),
                f(|x| x.drops.to_string()),
                f(|x| x.wait_max_ms.to_string()),
                f(|x| x.ptt.clone()),
                f(|x| x.file.clone()),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// [KAPI] Uçuş penceresi kapısı gözlemi: sayılan ilk üç ACK, yoklama kapsamı, kalıcı tavan.
// ---------------------------------------------------------------------------------------------

/// Bir tam datagramın tel baytı: yoklama bütçesinin birimi ve ön kaydın boşluk eşiği (#175
/// 7174902145).
const FULL_WIRE: u64 = MTU + OVH;
/// Ön kaydın kalıcı tavan eşiği: profil hattının %95'i.
const CEIL_95: u64 = PROFILE_UP * 95 / 100;

/// [KAPI] Koşudaki bir yoklama; korumanın hüküm kayıtlarından (`ProbeVerdictDiag`) kurulur.
/// Hükmü koşu bitmeden gelmeyen yoklama burada yoktur ([`temp_spans`] onu tavan izinden gösterir).
#[derive(Debug, Clone)]
struct ProbeRec {
    from_pair: bool,
    start: u64,
    /// Pencerenin sonu, son hüküm kaydındaki değer (çift yoklamasında bütçe dolunca kısalır).
    until: u64,
    base: u64,
    pace: u64,
    /// Yoklamayı kaydeden ACK (genel sıra): varışı pencerenin başından sonra olmayan, ilk hüküm
    /// ACK'inden önceki son ACK.
    reg_ack: Option<usize>,
    /// (ACK'in genel sırası, hüküm kaydı), varış sırasıyla.
    chain: Vec<(usize, ProbeVerdictDiag)>,
}

fn probes_of(o: &Outcome) -> Vec<ProbeRec> {
    let mut out: Vec<ProbeRec> = Vec::new();
    for a in &o.gate_acks {
        let Some(v) = a.trace.and_then(|t| t.diag.verdict) else {
            continue;
        };
        let known = out.iter_mut().find(|p| {
            p.from_pair == v.from_pair
                && p.start == v.probe_start_ns
                && p.base == v.base_pace
                && p.pace == v.probe_pace
        });
        if let Some(p) = known {
            p.until = v.probe_until_ns;
            p.chain.push((a.index, v));
        } else {
            out.push(ProbeRec {
                from_pair: v.from_pair,
                start: v.probe_start_ns,
                until: v.probe_until_ns,
                base: v.base_pace,
                pace: v.probe_pace,
                reg_ack: o
                    .gate_acks
                    .iter()
                    .rev()
                    .find(|b| b.index < a.index && b.arrival <= v.probe_start_ns)
                    .map(|b| b.index),
                chain: vec![(a.index, v)],
            });
        }
    }
    out
}

/// [KAPI] Geçici tavan aralıkları, yalnız adım izinden (hüküm kayıtlarından bağımsız): tavanın
/// sabit kaldığı ve ACK verilmeyen bir adımda (yani `LineGuard::tick` ile) daha düşük bir değere
/// inerek biten her aralık `(başlangıç, bitiş, sonraki tavan, aralıktaki tavan)`. Kalıcı tavan bu
/// aralıkların dışında tavanın kendisi, içinde aralıktan sonraki değerdir. Sınır: tavanın indiği
/// adımda bir ACK de işlendiyse iniş ACK'e yazılır (aralık geçici sayılmaz).
fn temp_spans(o: &Outcome) -> Vec<(u64, u64, u64, u64)> {
    let st = &o.gate_steps;
    let mut out = Vec::new();
    let mut run_start = 0usize;
    for i in 1..st.len() {
        if st[i].pace != st[i - 1].pace {
            if st[i].pace < st[i - 1].pace && !st[i].ack {
                out.push((st[run_start].now, st[i].now, st[i].pace, st[i - 1].pace));
            }
            run_start = i;
        }
    }
    out
}

/// [KAPI] `t` anında tavan `pace` iken kalıcı tavan ([`temp_spans`]).
fn permanent_ceiling(spans: &[(u64, u64, u64, u64)], t: u64, pace: u64) -> u64 {
    spans
        .iter()
        .find(|&&(a, b, _, p)| a <= t && t < b && p == pace)
        .map_or(pace, |&(_, _, base, _)| base)
}

/// [KAPI] Bir zaman aralığında kapının görünümü (adım izinden). Aralık `[from, to)`: bir adımın
/// kaydı o adımın SONUNDAKİ durumdur, bitiş adımının sonunda aralığı kapatan olay (yoklamada
/// tavanın tabana dönmesi) işlenmiştir. Yalnız `pkts_incl` / `wire_incl` bitiş adımını da sayar
/// (çift yoklamasında bütçeyi dolduran datagramın anı pencerenin bitişidir).
#[derive(Debug, Clone, Copy, Default)]
struct GateView {
    steps: u64,
    /// `[from, to)` içinde darboğaza giren datagram / tel baytı ve öncelikli şerit payı.
    pkts: u64,
    wire: u64,
    pri_pkts: u64,
    pri_wire: u64,
    /// `[from, to]` (bitiş adımı dahil) içinde giren datagram / tel baytı ve öncelikli payı.
    pkts_incl: u64,
    wire_incl: u64,
    pri_pkts_incl: u64,
    /// En küçük boşluk (pencere − uçuş; uçuş pencereyi aşmışsa 0) ve anı.
    min_gap: u64,
    min_gap_at: u64,
    /// Veri beklerken (bekleyen birim > 0) en küçük boşluk.
    min_gap_waiting: Option<u64>,
    min_unsent: u64,
    /// Veri beklerken boşluğun bir tam datagramın tel baytından ([`FULL_WIRE`], ön kayıt eşiği)
    /// ve dosya biriminin gövdesinden ([`FILE_UNIT`], kapının bu düzenekteki tam koşulu) küçük
    /// olduğu adım sayısı.
    bound_steps: u64,
    bound_exact_steps: u64,
    /// En uzun gönderim duraklaması (adım): ardışık, veri bekleyen, boşluğu [`FULL_WIRE`]'dan
    /// küçük ve hiçbir datagramın çıkmadığı adımlar.
    stall_max: u64,
    max_inflight: u64,
    window_min: u64,
    window_max: u64,
    bbr_min: u64,
    bbr_max: u64,
    floor_min: u64,
    floor_max: u64,
    /// Tavanın verilen değerde olduğu adım sayısı (yoklama tavanı fiilen açık).
    pace_steps: u64,
}

impl GateView {
    /// Kapı bu aralıkta bağladı: veri beklerken boşluk bir tam datagramın altına indi VE gönderim
    /// durakladı (ön kaydın ret ölçütünün aralık biçimi).
    fn bound(&self) -> bool {
        self.min_gap_waiting.is_some_and(|g| g < FULL_WIRE) && self.stall_max > 0
    }
}

fn gate_view(o: &Outcome, from: u64, to: u64, pace: Option<u64>) -> GateView {
    let mut v = GateView {
        min_gap: u64::MAX,
        min_unsent: u64::MAX,
        window_min: u64::MAX,
        bbr_min: u64::MAX,
        floor_min: u64::MAX,
        ..GateView::default()
    };
    let mut stall = 0u64;
    for st in o.gate_steps.iter().filter(|s| s.now >= from && s.now <= to) {
        v.pkts_incl += st.pkts;
        v.wire_incl += st.wire;
        v.pri_pkts_incl += st.pri_pkts;
        if st.now >= to {
            continue;
        }
        v.pkts += st.pkts;
        v.wire += st.wire;
        v.pri_pkts += st.pri_pkts;
        v.pri_wire += st.pri_wire;
        v.steps += 1;
        let gap = st.window.saturating_sub(st.inflight);
        if gap < v.min_gap {
            v.min_gap = gap;
            v.min_gap_at = st.now;
        }
        v.min_unsent = v.min_unsent.min(st.unsent);
        v.max_inflight = v.max_inflight.max(st.inflight);
        v.window_min = v.window_min.min(st.window);
        v.window_max = v.window_max.max(st.window);
        v.bbr_min = v.bbr_min.min(st.bbr);
        v.bbr_max = v.bbr_max.max(st.bbr);
        v.floor_min = v.floor_min.min(st.floor);
        v.floor_max = v.floor_max.max(st.floor);
        if pace == Some(st.pace) {
            v.pace_steps += 1;
        }
        if st.unsent > 0 {
            v.min_gap_waiting = Some(v.min_gap_waiting.map_or(gap, |g| g.min(gap)));
            if gap < FULL_WIRE {
                v.bound_steps += 1;
            }
            if gap < FILE_UNIT {
                v.bound_exact_steps += 1;
            }
        }
        if st.unsent > 0 && gap < FULL_WIRE && st.pkts == 0 {
            stall += 1;
            v.stall_max = v.stall_max.max(stall);
        } else {
            stall = 0;
        }
    }
    v
}

fn s3(ns: u64) -> String {
    format!("{:.3}", ns as f64 / SEC as f64)
}

fn opt_s3(v: Option<u64>) -> String {
    v.map_or("-".to_string(), s3)
}

fn arrow(a: u64, b: u64) -> String {
    format!("{a}→{b}")
}

/// [KAPI] Evre başına kapı görünümü (`KAPI-EVRE`).
fn gate_phase_print(id: &str, phase: &str, o: &Outcome, from: u64, to: u64) -> GateView {
    let v = gate_view(o, from, to, None);
    println!(
        "KAPI-EVRE {id} evre={phase} aralik_s={}–{} adim={} giren_dgram={} giren_tel_B={} \
         ucus_max_B={} pencere_B={}..{} bbr_penceresi_B={}..{} lg_tabani_B={}..{} \
         min_bosluk_B={} min_bosluk_an_s={} veri_beklerken_min_bosluk_B={} \
         bagli_adim_lt1228={} bagli_adim_lt1000={} en_uzun_duraklama_ms={} bekleyen_min={}",
        s3(from),
        s3(to),
        v.steps,
        v.pkts,
        v.wire,
        v.max_inflight,
        v.window_min,
        v.window_max,
        v.bbr_min,
        v.bbr_max,
        v.floor_min,
        v.floor_max,
        v.min_gap,
        s3(v.min_gap_at),
        bua_opt(v.min_gap_waiting),
        v.bound_steps,
        v.bound_exact_steps,
        v.stall_max,
        v.min_unsent,
    );
    v
}

/// [KAPI] Aracın tutarlılık sınaması (kabul kapısı değil): adım izi hat sayaçlarına eşit, her ACK
/// satırı var, kapının penceresi alanlarından kurulabiliyor, çift yoklamasının penceresinde
/// düzeneğin saydığı datagram korumanın saydığını sarıyor.
fn gate_selfcheck(id: &str, sc: &Scenario, o: &Outcome) {
    assert_eq!(
        o.gate_steps.iter().map(|s| s.pkts).sum::<u64>(),
        o.line.offered_pkts,
        "{id}: adım izi ↔ hat (datagram)"
    );
    assert_eq!(
        o.gate_steps.iter().map(|s| s.wire).sum::<u64>(),
        o.line.offered_bytes,
        "{id}: adım izi ↔ hat (tel baytı)"
    );
    assert_eq!(
        o.gate_acks.len(),
        o.acks_at.len(),
        "{id}: ACK satırı sayısı"
    );
    for a in &o.gate_acks {
        for (when, g) in [("önce", a.before), ("sonra", a.after)] {
            let floor = g.line_floor.unwrap_or_else(|| panic!("{id}: kural yok"));
            assert!(
                g.effective_window >= floor && g.effective_window >= g.bbr_window,
                "{id} ACK#{} {when}: pencere {} < taban {floor} ya da BBR {}",
                a.index,
                g.effective_window,
                g.bbr_window
            );
            assert_eq!(
                g.effective_window,
                g.bbr_window.max(floor),
                "{id} ACK#{} {when}: pencere alanlarından kurulamadı",
                a.index
            );
            assert!(g.pace_bps.is_some(), "{id}: koruma yok");
        }
    }
    for p in probes_of(o).iter().filter(|p| p.from_pair) {
        let Some(&(idx, last)) = p.chain.last() else {
            continue;
        };
        let ended = o
            .gate_acks
            .iter()
            .find(|a| a.index == idx)
            .is_some_and(|a| a.arrival >= p.until);
        let v = gate_view(o, p.start, p.until, Some(p.pace));
        // Koruma penceredeki gönderimi `başlangıç ≤ an < bitiş` ile sayar; bütçeyi dolduran
        // datagramın anı pencerenin yeni bitişi olur (pencere o ana çekilir) ve o adımda başka
        // datagram da çıkmış olabilir: korumanın sayısı düzeneğin `[baş, bitiş)` ve `[baş, bitiş]`
        // sayıları arasındadır.
        if ended {
            assert!(
                v.pkts <= last.probe_sent_datagrams && last.probe_sent_datagrams <= v.pkts_incl,
                "{id}: yoklama [{}–{}] datagram: düzenek {}..{} ↔ kayıt {}",
                s3(p.start),
                s3(p.until),
                v.pkts,
                v.pkts_incl,
                last.probe_sent_datagrams
            );
            assert!(
                v.wire <= last.probe_sent_wire && last.probe_sent_wire <= v.wire_incl,
                "{id}: yoklama [{}–{}] tel baytı: düzenek {}..{} ↔ kayıt {}",
                s3(p.start),
                s3(p.until),
                v.wire,
                v.wire_incl,
                last.probe_sent_wire
            );
        }
        if let Some(n) = sc.pair_probe {
            assert!(
                last.probe_sent_datagrams <= n && last.probe_sent_wire <= n * FULL_WIRE,
                "{id}: yoklama bütçeyi aştı: {} datagram / {} B",
                last.probe_sent_datagrams,
                last.probe_sent_wire
            );
        }
    }
}

/// [KAPI] Hücre özeti.
#[derive(Debug, Clone)]
struct GateCell {
    class: &'static str,
    /// Toparlanmadan sonra penceresi başlayan ilk çift yoklaması açıldı mı.
    opened: bool,
    /// Yoklama penceresinde en küçük boşluk ve düzeneğin saydığı (bitiş adımı dahil) / korumanın
    /// saydığı datagram ve tel baytı.
    min_gap: Option<u64>,
    sent_harness: Option<(u64, u64)>,
    sent_guard: Option<(u64, u64)>,
    /// Sayılan ilk üç ACK'te (önce, sonra) en küçük boşluk.
    ack_min_gap: Option<u64>,
    /// Sayılan üçüncü ACK'in sonunda kalıcı tavan.
    ceiling_at_3: Option<u64>,
    chain: String,
    /// Yoklama penceresinin ve üç evrenin (dürüst, dar, toparlanma) kapı görünümü.
    view: Option<GateView>,
    phases: [GateView; 3],
}

/// [KAPI] Bir hücrenin raporu: toparlanmadan sonra sayılan ilk üç ACK'in satırı (`KAPI-ACK`),
/// koşudaki yoklamalar (`KAPI-YOKLAMA`), evreler (`KAPI-EVRE`) ve özet (`KAPI-HÜCRE`).
#[allow(clippy::too_many_lines)]
fn gate_cell(id: &str, sc: &Scenario, o: &Outcome) -> GateCell {
    let probes = probes_of(o);
    let spans = temp_spans(o);
    let counted: Vec<&GateAck> = o
        .gate_acks
        .iter()
        .filter(|a| a.ser_start >= RECOVER_AT_NS)
        .collect();
    let first3: Vec<&GateAck> = counted.iter().copied().take(3).collect();
    let perm_after =
        |a: &GateAck| permanent_ceiling(&spans, a.arrival, a.after.pace_bps.unwrap_or(0));
    let perm_before = |a: &GateAck| {
        permanent_ceiling(
            &spans,
            a.arrival.saturating_sub(MS),
            a.before.pace_bps.unwrap_or(0),
        )
    };
    let gap = |g: FlightGateDiag| g.effective_window.saturating_sub(g.inflight);
    for (k, a) in first3.iter().enumerate() {
        let verdict = a.trace.and_then(|t| t.diag.verdict);
        let judged = probes
            .iter()
            .find(|p| p.chain.iter().any(|&(i, _)| i == a.index));
        let registered = probes.iter().find(|p| p.reg_ack == Some(a.index));
        let kind = |p: &ProbeRec| if p.from_pair { "çift" } else { "olağan" };
        let mut windows = Vec::new();
        if let Some(p) = judged {
            windows.push(format!(
                "hüküm:{} [{}–{}]",
                kind(p),
                s3(p.start),
                s3(p.until)
            ));
        }
        if let Some(p) = registered {
            windows.push(format!(
                "kayıt:{} [{}–{}]",
                kind(p),
                s3(p.start),
                s3(p.until)
            ));
        }
        let shown = judged.or(registered);
        let view = shown.map(|p| gate_view(o, p.start, p.until, Some(p.pace)));
        let guard_sent = verdict
            .map(|v| (v.probe_sent_datagrams, v.probe_sent_wire))
            .or_else(|| {
                shown
                    .and_then(|p| p.chain.last())
                    .map(|&(_, v)| (v.probe_sent_datagrams, v.probe_sent_wire))
            });
        println!(
            "KAPI-ACK | {id} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} / {} | {} | {} | {} | {} | \
             {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            k + 1,
            a.index,
            s3(a.arrival),
            if a.measured { "var" } else { "yok" },
            bua_opt(a.pair_rate),
            a.trace
                .map_or("kayıt yok".to_string(), |t| format!("{:?}", t.decision)),
            verdict.map_or("-".to_string(), |v| format!("{:?}", v.outcome)),
            verdict.map_or("-".to_string(), |v| v.covered.to_string()),
            verdict.map_or("-".to_string(), |v| v.acks_after_start.to_string()),
            if windows.is_empty() {
                "-".to_string()
            } else {
                windows.join("; ")
            },
            verdict.map_or("-".to_string(), |v| opt_s3(v.iv_first_send_ns)),
            verdict.map_or("-".to_string(), |v| opt_s3(v.iv_last_send_ns)),
            arrow(perm_before(a), perm_after(a)),
            arrow(
                a.before.pace_bps.unwrap_or(0),
                a.after.pace_bps.unwrap_or(0)
            ),
            shown.map_or("-".to_string(), |p| format!("{} (taban {})", p.pace, p.base)),
            arrow(a.before.inflight, a.after.inflight),
            arrow(a.before.bbr_window, a.after.bbr_window),
            arrow(
                a.before.line_floor.unwrap_or(0),
                a.after.line_floor.unwrap_or(0)
            ),
            arrow(a.before.effective_window, a.after.effective_window),
            arrow(gap(a.before), gap(a.after)),
            guard_sent.map_or("-".to_string(), |(d, w)| format!("{d} / {w}")),
            view.map_or("-".to_string(), |v| format!(
                "{} / {} (bitiş adımı hariç {} / {}; öncelikli {})",
                v.pkts_incl, v.wire_incl, v.pkts, v.wire, v.pri_pkts_incl
            )),
            view.map_or("-".to_string(), |v| v.min_gap.to_string()),
            arrow(a.unsent_before as u64, a.unsent_after as u64),
            format_args!("{} / {}", a.emitted_pkts, a.emitted_wire),
        );
    }
    for p in &probes {
        let v = gate_view(o, p.start, p.until, Some(p.pace));
        println!(
            "KAPI-YOKLAMA {id} tur={} kayit_ack={} pencere_s={}–{} sure_ms={} taban_tavan={} \
             yoklama_tavani={} tavan_acik_adim={} cikan_kayit_dgram={} cikan_kayit_tel_B={} \
             cikan_duzenek_dgram={}..{} cikan_duzenek_tel_B={}..{} oncelikli_dgram={} \
             oncelikli_tel_B={} min_bosluk_B={} veri_beklerken_min_bosluk_B={} bekleyen_min={} \
             bagli_adim_lt1228={} pencere_iletimleri=[{}] zincir={}",
            if p.from_pair { "cift" } else { "olagan" },
            p.reg_ack.map_or("-".to_string(), |i| i.to_string()),
            s3(p.start),
            s3(p.until),
            p.until.saturating_sub(p.start) / MS,
            p.base,
            p.pace,
            v.pace_steps,
            p.chain.last().map_or(0, |c| c.1.probe_sent_datagrams),
            p.chain.last().map_or(0, |c| c.1.probe_sent_wire),
            v.pkts,
            v.pkts_incl,
            v.wire,
            v.wire_incl,
            v.pri_pkts,
            v.pri_wire,
            v.min_gap,
            bua_opt(v.min_gap_waiting),
            v.min_unsent,
            v.bound_steps,
            o.gate_tx
                .iter()
                .filter(|t| t.at >= p.start && t.at <= p.until)
                .map(|t| format!("{}@{}:{}", t.seq, s3(t.at), t.fate))
                .collect::<Vec<_>>()
                .join(" "),
            p.chain
                .iter()
                .map(|&(i, d)| format!(
                    "ACK#{i}:{:?}(covered={},acks_after_start={},aralik_s={}..{},aralik_seq={})",
                    d.outcome,
                    d.covered,
                    d.acks_after_start,
                    opt_s3(d.iv_first_send_ns),
                    opt_s3(d.iv_last_send_ns),
                    o.gate_acks
                        .iter()
                        .find(|a| a.index == i)
                        .and_then(|a| a.trace)
                        .and_then(|t| t.note)
                        .map_or("-".to_string(), |n| format!(
                            "{}..{}",
                            n.first_seq, n.last_seq
                        )),
                ))
                .collect::<Vec<_>>()
                .join("→"),
        );
    }
    let phases = [
        gate_phase_print(id, "dürüst", o, 0, DROP_AT_NS),
        gate_phase_print(id, "dar", o, DROP_AT_NS, RECOVER_AT_NS),
        gate_phase_print(id, "toparlanma", o, RECOVER_AT_NS, sc.t_max_ns),
    ];

    // Toparlanmadan sonra penceresi başlayan ilk çift yoklaması.
    let probe = probes
        .iter()
        .find(|p| p.from_pair && p.start >= RECOVER_AT_NS);
    // Hükmü gelmemiş geçici tavan aralıkları (hiçbir hüküm kaydının penceresiyle örtüşmeyen).
    let orphan: Vec<String> = spans
        .iter()
        .filter(|&&(a, b, _, _)| !probes.iter().any(|p| a <= p.until && p.start <= b))
        .map(|&(a, b, base, p)| format!("[{}–{}] {p}→{base}", s3(a), s3(b)))
        .collect();
    // Çift ölçüsünün yoklama koşulu (yaklaşık: `LineGuard::pair_probe_target`; ret seviyesi ve tek
    // atış işareti kayıtta yok): tel baytına çevrilmiş, profile kırpılmış çift > 1,05 × kalıcı tavan.
    let pair_wire = |p: u64| (p * FULL_WIRE / MTU).min(PROFILE_UP);
    let eligible = |a: &GateAck| {
        a.pair_rate
            .is_some_and(|p| pair_wire(p) * 100 > perm_after(a) * 105)
    };
    let pairs: Vec<String> = first3
        .iter()
        .map(|a| {
            format!(
                "ACK#{}:{}",
                a.index,
                a.pair_rate.map_or("yok".to_string(), |p| format!(
                    "{p}(tel {} ; tavan {} ; %5 üstü={})",
                    pair_wire(p),
                    perm_after(a),
                    eligible(a)
                ))
            )
        })
        .collect();
    let (class, note): (&'static str, String) = match probe {
        None if sc.pair_probe.is_none() => ("YOKLAMA AÇILMADI", "anahtar kapalı".to_string()),
        None => {
            let reason = if first3.is_empty() {
                "sayılan ACK yok".to_string()
            } else if first3.iter().all(|a| a.pair_rate.is_none()) {
                "çift ölçüsü yok (sayılan ilk üç ACK'te pair_rate alanı boş)".to_string()
            } else if !first3.iter().any(|a| eligible(a)) {
                if first3.iter().all(|a| perm_after(a) >= PROFILE_UP) {
                    "tavan zaten profil hattında".to_string()
                } else {
                    "çift ölçüsü tavanın %5 üstünde değil".to_string()
                }
            } else {
                let a = first3
                    .iter()
                    .copied()
                    .find(|a| eligible(a))
                    .expect("uygun ACK var");
                let down = a
                    .trace
                    .is_some_and(|t| t.decision == LineGuardAckDecision::IntervalSample);
                let earlier = probes
                    .iter()
                    .rfind(|p| p.from_pair && p.start < RECOVER_AT_NS);
                if down {
                    format!("ACK#{} aynı anda aşağı yönlü örnek verdi", a.index)
                } else if let Some(p) = earlier {
                    format!(
                        "çift %5 üstünde ama açılmadı; toparlanmadan önce çift yoklaması var \
                         [{}–{}] son hüküm {:?} (tek atış / ret seviyesi; kayıtta alanı yok)",
                        s3(p.start),
                        s3(p.until),
                        p.chain.last().map(|c| c.1.outcome)
                    )
                } else if !orphan.is_empty() {
                    format!("hükmü gelmemiş geçici tavan aralığı var: {orphan:?}")
                } else {
                    "çift %5 üstünde ama açılmadı; neden kayıttan okunamıyor".to_string()
                }
            };
            ("YOKLAMA AÇILMADI", reason)
        }
        Some(p) => {
            let v = gate_view(o, p.start, p.until, Some(p.pace));
            let last = p.chain.last().map(|c| c.1);
            let n = sc.pair_probe.unwrap_or(0);
            let budget_met = last
                .is_some_and(|l| l.probe_sent_datagrams >= n || l.probe_sent_wire >= n * FULL_WIRE);
            let waiting = v.min_unsent > 0;
            let covered = last.is_some_and(|l| l.covered);
            let outcome = last.map(|l| l.outcome);
            let class = if !budget_met && v.min_gap < FULL_WIRE && waiting {
                "KAPI"
            } else if outcome == Some(ProbeOutcome::Passed) {
                "GEÇTİ"
            } else if (covered && outcome == Some(ProbeOutcome::Failed))
                || outcome == Some(ProbeOutcome::Superseded)
            {
                "HAT REDDİ"
            } else if (budget_met || !waiting) && !covered {
                "KAPSAMA"
            } else {
                "AYIRT EDİLEMEZ"
            };
            (
                class,
                format!(
                    "bütçe_doldu={budget_met} veri_bekliyor={waiting} covered={covered} \
                     son_hüküm={outcome:?}"
                ),
            )
        }
    };
    let chain = probe.map_or("-".to_string(), |p| {
        p.chain
            .iter()
            .map(|c| format!("{:?}", c.1.outcome))
            .collect::<Vec<_>>()
            .join("→")
    });
    // Kapsamayan her hüküm ACK'inde eksik uç: aralık yok / son uç yoklama bitmeden gönderilmiş /
    // ilk uç yoklama başladıktan sonra gönderilmiş (koşul: ilk uç ≤ başlangıç VE son uç ≥ bitiş).
    let deficit = probe.map_or(String::new(), |p| {
        p.chain
            .iter()
            .filter(|c| !c.1.covered)
            .map(|&(i, d)| {
                let mut why = Vec::new();
                match (d.iv_first_send_ns, d.iv_last_send_ns) {
                    (Some(f), Some(l)) => {
                        if l < d.probe_until_ns {
                            why.push(format!("son uç {} < bitiş {}", s3(l), s3(d.probe_until_ns)));
                        }
                        if f > d.probe_start_ns {
                            why.push(format!(
                                "ilk uç {} > başlangıç {}",
                                s3(f),
                                s3(d.probe_start_ns)
                            ));
                        }
                        if why.is_empty() {
                            why.push("uçlar kapsıyor; hüküm cephesi ya da aşağı örnek".to_string());
                        }
                    }
                    _ => why.push("ölçüm aralığı yok".to_string()),
                }
                format!("ACK#{i}: {}", why.join(" ve "))
            })
            .collect::<Vec<_>>()
            .join("; ")
    });
    let view = probe.map(|p| gate_view(o, p.start, p.until, Some(p.pace)));
    let ack_min_gap = first3
        .iter()
        .flat_map(|a| [gap(a.before), gap(a.after)])
        .min();
    let ceiling_at_3 = first3.get(2).map(|a| perm_after(a));
    // Var olan testin tanımıyla kalıcı dönüş: bu sayılan ACK'ten koşu sonuna dek tavan ≥ %95.
    let up_k = (0..counted.len()).find(|&k| {
        counted[k..]
            .iter()
            .all(|a| a.after.pace_bps.unwrap_or(0) * 100 >= PROFILE_UP * 95)
    });
    println!(
        "KAPI-HÜCRE {id} ack_toplam={} sayilan_ack={} cift_olculeri=[{}] yoklama={} \
         kayit_ack={} sayilan_sira={} pencere_s={} cikan_kayit={} cikan_duzenek={} butce={} \
         pencere_min_bosluk_B={} pencere_bekleyen_min={} tavan_acik_adim={} zincir={} \
         ilk_uc_ack_min_bosluk_B={} kalici_tavan_3ack={} ge_11875={} testin_kalici_donus_ack={} \
         hukumsuz_gecici_tavan={:?} kapsama_eksigi=[{deficit}] sinif=\"{class}\" \
         not=\"{note}\"",
        o.gate_acks.len(),
        counted.len(),
        pairs.join(", "),
        if probe.is_some() {
            "açıldı"
        } else {
            "açılmadı"
        },
        probe
            .and_then(|p| p.reg_ack)
            .map_or("-".to_string(), |i| i.to_string()),
        probe
            .and_then(|p| p.reg_ack)
            .and_then(|i| counted.iter().position(|a| a.index == i))
            .map_or("-".to_string(), |k| (k + 1).to_string()),
        probe.map_or("-".to_string(), |p| format!(
            "{}–{}",
            s3(p.start),
            s3(p.until)
        )),
        probe
            .and_then(|p| p.chain.last())
            .map_or("-".to_string(), |c| format!(
                "{}dgram/{}B",
                c.1.probe_sent_datagrams, c.1.probe_sent_wire
            )),
        view.map_or("-".to_string(), |v| format!(
            "{}dgram/{}B(oncelikli {}dgram)",
            v.pkts_incl, v.wire_incl, v.pri_pkts_incl
        )),
        sc.pair_probe
            .map_or("-".to_string(), |n| format!("{n}dgram/{}B", n * FULL_WIRE)),
        bua_opt(view.map(|v| v.min_gap)),
        bua_opt(view.map(|v| v.min_unsent)),
        bua_opt(view.map(|v| v.pace_steps)),
        chain,
        bua_opt(ack_min_gap),
        bua_opt(ceiling_at_3),
        ceiling_at_3.map_or("-".to_string(), |c| (c >= CEIL_95).to_string()),
        up_k.map_or("YOK".to_string(), |k| (k + 1).to_string()),
        orphan,
    );
    GateCell {
        class,
        opened: probe.is_some(),
        min_gap: view.map(|v| v.min_gap),
        sent_harness: view.map(|v| (v.pkts_incl, v.wire_incl)),
        sent_guard: probe
            .and_then(|p| p.chain.last())
            .map(|c| (c.1.probe_sent_datagrams, c.1.probe_sent_wire)),
        ack_min_gap,
        ceiling_at_3,
        chain,
        view,
        phases,
    }
}

const GATE_ACK_HEADER: &str = "KAPI-ACK-BAŞLIK | hücre (tohum/yük/anahtar/kayıp/alıcı) | ACK \
    (sayılan/genel) | varış s | ölçüm aralığı | çift B/s UDP | koruma kararı | hüküm | covered | \
    acks_after_start | yoklama penceresi s | aralık ilk / son uç gönderimi s | kalıcı tavan \
    önce→sonra B/s | ham tavan önce→sonra B/s | yoklama hızı B/s | inflight önce→sonra B | BBR \
    penceresi önce→sonra B | LG tabanı önce→sonra B | etkin pencere önce→sonra B | boşluk \
    önce→sonra B | çıkan yoklama dgram / tel B (koruma kaydı) | çıkan yoklama dgram / tel B \
    (düzenek) | pencere içi en küçük boşluk B | bekleyen birim önce→sonra | ACK olayında çıkan \
    dgram / tel B |";

fn load_label(l: PttLoad) -> &'static str {
    match l {
        PttLoad::None => "pttsiz",
        PttLoad::Single => "tek",
        PttLoad::Copies => "kopya",
    }
}

/// [KAPI] Çift yoklaması: toparlanmadan sonra sayılan ilk üç ACK'te sıra, yoklama kapsamı, kalıcı
/// tavan (geçici yoklama hızından ayrı) ve aynı kayıtta uçuş, kapının penceresi, fiilen çıkan
/// yoklama datagramları. Soru (sahip, #175): yoklama datagramları uçuş penceresi kapısında mı
/// kalıyor, yoksa çıktıktan sonra ACK'in ölçüm aralığı yoklamayı kapsamıyor mu.
///
/// Hücreler: tohum 1–3 × PTT yükü (yok / tek iletim / kopyalı) × anahtar (kapalı / 6 datagram) ×
/// kayıp (senaryonun bugünkü kipi: LCG ‰200 / anahtarlı ‰200) × ürün alıcısı; ayrıca
/// [`drop_recovery`]'nin bugünkü alıcısı (ayna) yalnız tohum 1'de.
///
/// YALNIZ RAPOR: ön kayıttaki öngörüler (#175 7174902145) iddia edilmez, sayılar basılır. Sınanan
/// yalnız aracın tutarlılığıdır ([`gate_selfcheck`]; gözlem açıkken ve kapalıyken koşunun aynı izi
/// vermesi; deney anahtarının ortamdan gelmemesi). Sınıflar (hücre özeti):
/// * `KAPI`: çıkan < bütçe VE pencere içi en küçük boşluk < 1 228 B VE veri bekliyor;
/// * `GEÇTİ`: hüküm `Passed`;
/// * `HAT REDDİ`: kapsayan aralıkla `Failed` ya da aynı ACK aşağı yönlü örnek verdi (`Superseded`);
/// * `KAPSAMA`: çıkan = bütçe (ya da veri bitti) ve son hükümde `covered = false`;
/// * `YOKLAMA AÇILMADI`: toparlanmadan sonra penceresi başlayan çift yoklaması yok (nedeni notta;
///   bu hücre soruyu SINAMAZ);
/// * `AYIRT EDİLEMEZ`: yukarıdakilerin hiçbiri.
///
/// Koşu: `cargo test --release -p adaptive-transport --test starved_line_sim
/// pair_probe_first_three_acks_flight_gate_report -- --nocapture`.
#[test]
fn pair_probe_first_three_acks_flight_gate_report() {
    println!();
    // Aracın sınaması: anahtar yalnız senaryodan gelir (kural ortam değişkenini de okur).
    assert!(
        std::env::var("KAMA_PAIR_PROBE_DATAGRAMS").is_err(),
        "KAMA_PAIR_PROBE_DATAGRAMS ortamda kurulu: anahtar kapalı hücreler kapalı olmaz"
    );
    println!("{GATE_ACK_HEADER}");
    let mut tally: BTreeMap<(String, &'static str, &'static str, &'static str), u64> =
        BTreeMap::new();
    let mut lines = Vec::new();
    for rx in [RxModel::PRODUCT, RxModel::Mirror] {
        for seed in 1..=3u64 {
            if rx == RxModel::Mirror && seed != 1 {
                continue;
            }
            for load in [PttLoad::None, PttLoad::Single, PttLoad::Copies] {
                for loss in [LossModel::Lcg, LossModel::Keyed] {
                    for key in [None, Some(6u64)] {
                        let sc = Scenario {
                            rx,
                            loss,
                            loss_permille: 200,
                            pair_probe: key,
                            gate_diag: true,
                            ..drop_recovery(seed, load)
                        };
                        let key_label = if key.is_some() { "açık6" } else { "kapalı" };
                        let loss_label = if loss == LossModel::Lcg {
                            "lcg200"
                        } else {
                            "anahtarlı200"
                        };
                        let id = format!(
                            "t{seed}/{}/{key_label}/{loss_label}/{}",
                            load_label(load),
                            rx.label()
                        );
                        let o = run(&sc);
                        gate_selfcheck(&id, &sc, &o);
                        // Aracın sınaması: gözlem koşuyu değiştirmez (anahtar kapalı hücre,
                        // gözlemsiz hâliyle var olan yolun kendisidir).
                        let bare = run(&Scenario {
                            gate_diag: false,
                            ..sc.clone()
                        });
                        assert!(
                            o.acks_at == bare.acks_at
                                && o.per_ack == bare.per_ack
                                && o.waits == bare.waits
                                && o.drop_times == bare.drop_times
                                && o.pace_trace == bare.pace_trace
                                && o.rate_trace == bare.rate_trace
                                && o.file_done_ns == bare.file_done_ns
                                && o.ptt_wire_bytes == bare.ptt_wire_bytes
                                && o.line.offered_bytes == bare.line.offered_bytes
                                && o.line.lost_bytes == bare.line.lost_bytes,
                            "{id}: gözlem koşuyu değiştirdi"
                        );
                        let c = gate_cell(&id, &sc, &o);
                        *tally
                            .entry((rx.label(), key_label, loss_label, c.class))
                            .or_insert(0) += 1;
                        lines.push((id, c));
                    }
                }
            }
        }
    }
    for ((rx, key, loss, class), n) in &tally {
        println!("KAPI-ÖZET alici={rx} anahtar={key} kayip={loss} sinif=\"{class}\" hucre={n}");
    }
    // Ön kayıt karşılaştırması (iddia değil, sayım): anahtar açık hücrelerde.
    let on: Vec<&(String, GateCell)> = lines
        .iter()
        .filter(|(id, _)| id.contains("açık6"))
        .collect();
    let opened: Vec<&&(String, GateCell)> = on.iter().filter(|(_, c)| c.opened).collect();
    println!(
        "KAPI-ÖNKAYIT anahtar_acik_hucre={} yoklama_acilan={} pencere_bosluk_ge_7368={} \
         pencere_bosluk_lt_1228={} ilk_uc_ack_bosluk_ge_7368={} cikan_6dgram={} \
         kalici_tavan_3ack_ge_11875={} zincirler={:?}",
        on.len(),
        opened.len(),
        opened
            .iter()
            .filter(|(_, c)| c.min_gap.is_some_and(|g| g >= 6 * FULL_WIRE))
            .count(),
        opened
            .iter()
            .filter(|(_, c)| c.min_gap.is_some_and(|g| g < FULL_WIRE))
            .count(),
        on.iter()
            .filter(|(_, c)| c.ack_min_gap.is_some_and(|g| g >= 6 * FULL_WIRE))
            .count(),
        opened
            .iter()
            .filter(|(_, c)| c.sent_guard.is_some_and(|s| s.0 == 6)
                && c.sent_harness.is_some_and(|s| s.0 == 6))
            .count(),
        on.iter()
            .filter(|(_, c)| c.ceiling_at_3.is_some_and(|p| p >= CEIL_95))
            .count(),
        {
            let mut m: BTreeMap<&str, u64> = BTreeMap::new();
            for (_, c) in &on {
                *m.entry(c.chain.as_str()).or_insert(0) += 1;
            }
            m
        },
    );
}

/// [KAPI] Aracın kendi sınaması için toparlanma hattı (B/s): yoklama hedefi tabanın ≈ 1,2 katı.
const BOUND_ARM_RECOVER_TO: u64 = 7_500;
/// [KAPI] Aracın kendi sınaması için geri bildirim ufku (ürün değeri ≈ 10,7 s).
const BOUND_ARM_HORIZON_NS: u64 = 300 * MS;

/// [KAPI] Aracın kendi sınaması: kapının bağladığı bilinen bir kol raporda öyle görünür (ve
/// bağlamadığı kontrol kolu öyle görünmez). Senaryo [`drop_recovery`] (PTT'siz, ürün alıcısı,
/// anahtar 6), iki değişiklikle:
///
/// * geri bildirim ufku 0,3 s ([`BOUND_ARM_HORIZON_NS`]; ürün değeri ≈ 10,7 s): uçuş tabanı tavan
///   6 250'de ≈ 12,7 kB'a iner, BBR penceresi de (toparlanmasız) ≈ 7–58 kB'tadır → kapı bağlar;
/// * hat 20 s'de 7 500'e toparlanır ([`BOUND_ARM_RECOVER_TO`]; 12 500'e değil): yoklama hedefi
///   ≈ 7 469, taban yalnız ≈ 1,2 katına açılır.
///
/// İkinci değişiklik neden gerekli (ölçüldü, kol C): uçuş tabanı tavanla ölçeklenir
/// (`cc_feedback_floor = cc_feedback_cap(tavan) × ufuk`). Hedef 12 500 iken yoklama tavanı açıldığı
/// anda taban da iki katına çıkar (12 714 → 25 429 B) ve kapı, pencerenin dışında saniyelerce
/// bağladığı hâlde, yoklamanın altı datagramını tutmaz. Kol C bunu olduğu gibi kaydeder.
///
/// Kollar:
/// * A (bağlı): tohum 1 ve 2. Rapor `KAPI` sınıfını verir; korumanın ve düzeneğin saydığı yoklama
///   datagramı aynı ve bütçenin (6) altında; pencere içinde veri beklerken boşluk < 1 228 B ve
///   gönderim duraklamış.
/// * B (kontrol): aynı senaryo, ürün ufkuyla. Rapor `KAPI` vermez; pencere içinde bağlı adım yok;
///   altı datagram çıkar.
/// * C: [`drop_recovery`]'nin kendisi (hat 12 500'e döner), ufuk 0,3 s ve ürün ufku. Ufuk 0,3 s'de
///   toparlanma evresinde kapı bağlı görünür (`KAPI-EVRE`); ürün ufkunda hiçbir evrede bağlı adım
///   yoktur.
#[test]
fn flight_gate_report_sees_a_binding_gate() {
    println!();
    let arm = |seed: u64, to: u64, horizon: Option<u64>| Scenario {
        fwd_plan: vec![(0, PROFILE_UP), (DROP_AT_NS, DROP_TO), (RECOVER_AT_NS, to)],
        rx: RxModel::PRODUCT,
        pair_probe: Some(6),
        gate_diag: true,
        feedback_ns_override: horizon,
        ..drop_recovery(seed, PttLoad::None)
    };
    let cell = |tag: &str, sc: &Scenario| {
        let o = run(sc);
        gate_selfcheck(tag, sc, &o);
        let c = gate_cell(tag, sc, &o);
        println!(
            "KAPI-SINAMA {tag} sinif=\"{}\" cikan_kayit={:?} cikan_duzenek={:?} \
             pencere_min_bosluk_B={} pencere_bagli_adim={} pencere_duraklama_ms={} \
             evre_bagli_adim={:?} evre_duraklama_ms={:?} bbr_penceresi_B={:?} lg_tabani_B={:?}",
            c.class,
            c.sent_guard,
            c.sent_harness,
            bua_opt(c.min_gap),
            bua_opt(c.view.map(|v| v.bound_steps)),
            bua_opt(c.view.map(|v| v.stall_max)),
            c.phases.map(|v| v.bound_steps),
            c.phases.map(|v| v.stall_max),
            c.phases.map(|v| (v.bbr_min, v.bbr_max)),
            c.phases.map(|v| (v.floor_min, v.floor_max)),
        );
        c
    };
    for seed in [1u64, 2] {
        // A: kapının bağladığı kol.
        let tag = format!("sinama-A/bagli/t{seed}");
        let c = cell(
            &tag,
            &arm(seed, BOUND_ARM_RECOVER_TO, Some(BOUND_ARM_HORIZON_NS)),
        );
        let view = c.view.expect("A: yoklama açıldı");
        assert_eq!(c.class, "KAPI", "{tag}: rapor kapıyı görmedi");
        assert!(
            c.sent_guard == c.sent_harness && c.sent_guard.is_some_and(|s| s.0 < 6),
            "{tag}: çıkan yoklama datagramı kayıt {:?} ↔ düzenek {:?}",
            c.sent_guard,
            c.sent_harness
        );
        assert!(
            view.min_gap_waiting.is_some_and(|g| g < FULL_WIRE) && view.bound() && view.pkts < 6,
            "{tag}: pencere içi boşluk {:?} B, duraklama {} ms",
            view.min_gap_waiting,
            view.stall_max
        );
        // B: kontrol (ürün ufku).
        let tag = format!("sinama-B/kontrol/t{seed}");
        let c = cell(&tag, &arm(seed, BOUND_ARM_RECOVER_TO, None));
        assert_ne!(c.class, "KAPI", "{tag}: kontrol kolunda kapı göründü");
        assert!(
            c.phases.iter().all(|v| v.bound_steps == 0),
            "{tag}: kontrol kolunda bağlı adım {:?}",
            c.phases.map(|v| v.bound_steps)
        );
        if let Some(view) = c.view {
            assert!(
                view.bound_steps == 0 && c.sent_guard.is_some_and(|s| s.0 == 6),
                "{tag}: kontrol yoklaması {:?}, bağlı adım {}",
                c.sent_guard,
                view.bound_steps
            );
        }
    }
    // C: senaryonun kendisi (hat 12 500'e döner).
    let c = cell(
        "sinama-C/ufuk-0.3s/t1",
        &arm(1, PROFILE_UP, Some(BOUND_ARM_HORIZON_NS)),
    );
    assert!(
        c.phases[2].bound(),
        "C: ufuk 0,3 s'de toparlanma evresinde kapı bağlı görünmedi"
    );
    let c = cell("sinama-C/urun-ufku/t1", &arm(1, PROFILE_UP, None));
    assert!(
        c.phases.iter().all(|v| v.bound_steps == 0),
        "C: ürün ufkunda bağlı adım {:?}",
        c.phases.map(|v| v.bound_steps)
    );
}

// ---------------------------------------------------------------------------------------------
// [K2] Kovalı darboğazda iki karşı örnek: gerçek toparlanma ve yanlış yükseliş.
// ---------------------------------------------------------------------------------------------

/// [K2] Kovanın burst'ü (B, IP boyu): gerçek düzenek `tbf rate <hız> burst 1200 limit 64` +
/// `stab overhead -14`, ardında `pfifo limit 64`.
const K2_BURST: u64 = 1200;
/// [K2] Ürün dosya datagramının UDP gövdesi (1 074 B IP).
const K2_FILE_UNIT: u64 = 1046;
/// [K2] Ürün metin datagramının UDP gövdesi (136 B IP). Düzeneğin öteki testlerinde metin 80 B.
const K2_TEXT_UNIT: u64 = 108;
/// [K2] Dosya birimi sayısı: koşu boyunca bitmez (gönderici hep tavanda; `assert`).
const K2_FILE_UNITS: u64 = 1200;
/// [K2] Olay anı (iki kolda da): toparlanma kolunda hattın 12 500'e döndüğü an. ACK sırası
/// buradan sayılır.
const K2_EVENT_NS: u64 = RECOVER_AT_NS;
/// [K2] Kuyruk kapısı: olaydan sonra darboğaza giren datagramın beklemesi (kendi boyu hariç).
const K2_QUEUE_GATE_NS: u64 = 393 * MS;
/// [K2] Yanlış yükseliş eşiği (sahte kol): kalıcı tavan > dar hat × 1,1.
const K2_FALSE_RISE: u64 = DROP_TO * 110 / 100;
/// [K2] Metin akışı (PTT'siz yük): ilk birim ve aralık.
const K2_TEXT_AT_NS: u64 = SEC;
const K2_TEXT_PERIOD_NS: u64 = 2 * SEC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum K2Arm {
    /// 12 500 → 8 s'de 6 250 → 20 s'de 12 500 (gerçek toparlanma).
    Recover,
    /// 12 500 → 8 s'de 6 250 ve kalır (yanlış yükseliş kolu).
    False,
}

impl K2Arm {
    fn label(self) -> &'static str {
        match self {
            Self::Recover => "toparlanma",
            Self::False => "sahte",
        }
    }

    fn t_max(self) -> u64 {
        match self {
            Self::Recover => 70 * SEC,
            // [EK 9 · DÜZENEK] 80 s → 100 s: yükseliş kanıtının doğduğu ACK'ten sonra en az altı
            // ACK okunsun (ikinci yoklama ancak o zaman sınanmış sayılır; `kanit_sonrasi_ack`).
            Self::False => 100 * SEC,
        }
    }
}

/// [K2] Kolun senaryosu: [`drop_recovery`] (dosya sürekli, ürün alıcısı), ürün datagram boyları,
/// anahtarlı kayıp, gözlem açık. Yük: PTT'siz + 2 s'de bir metin (kopyasız öncelikli birim,
/// düzeneğin metin akışı) ya da tek iletim PTT (koşu boyunca; metin yok).
fn k2_scenario(
    arm: K2Arm,
    probe: bool,
    bucket: bool,
    loss_permille: u64,
    seed: u64,
    load: PttLoad,
) -> Scenario {
    let t_max = arm.t_max();
    let mut sc = Scenario {
        fwd_plan: match arm {
            K2Arm::Recover => vec![
                (0, PROFILE_UP),
                (DROP_AT_NS, DROP_TO),
                (K2_EVENT_NS, PROFILE_UP),
            ],
            K2Arm::False => vec![(0, PROFILE_UP), (DROP_AT_NS, DROP_TO)],
        },
        rx: RxModel::PRODUCT,
        loss: LossModel::Keyed,
        loss_permille,
        pair_probe: probe.then_some(6),
        gate_diag: true,
        bucket_burst: bucket.then_some(K2_BURST),
        file_unit: Some(K2_FILE_UNIT),
        text_unit: Some(K2_TEXT_UNIT),
        file_units: K2_FILE_UNITS,
        t_max_ns: t_max,
        ..drop_recovery(seed, load)
    };
    match load {
        PttLoad::None => {
            sc.text_at_ns = Some(K2_TEXT_AT_NS);
            sc.text_period_ns = K2_TEXT_PERIOD_NS;
            sc.text_until_ns = Some(t_max);
        }
        PttLoad::Single | PttLoad::Copies => sc.voice_until_ns = t_max,
    }
    sc
}

/// [K2] Aracın doğrulaması için yalın hat (kayıpsız).
fn k2_line(plan: Vec<(u64, u64)>, burst: Option<u64>) -> Bottleneck {
    Bottleneck::new(&Scenario {
        fwd_plan: plan,
        loss_permille: 0,
        bucket_burst: burst,
        ..Scenario::honest(1)
    })
}

/// [K2] `at` anında art arda verilen datagramların (UDP gövdesi) darboğazdan çıkış anları.
fn k2_exits(line: &mut Bottleneck, at: u64, udps: &[u64]) -> Vec<u64> {
    let from = line.passes.len();
    for &udp in udps {
        let n = line.line.offered_pkts;
        line.send(
            at,
            n,
            udp,
            FILE_BASE_ID + n,
            TxMeta {
                key_class: KEY_CLASS_UNIT,
                key_id: FILE_BASE_ID + n,
                class: TxClass::NewData,
                payload: Payload::File,
            },
        );
    }
    line.passes[from..].iter().map(|p| p.1).collect()
}

fn ms3(ns: u64) -> String {
    format!("{:.3}", ns as f64 / MS as f64)
}

/// [K2 · ARAÇ DOĞRULAMASI] Kovalı darboğaz modeli gerçek hatta ölçülen aralıkları verir.
///
/// Model: jeton kovası, hız = hat hızı, burst 1 200 B, IP boyu sayılır (UDP gövdesi + 28), paket
/// tanecikli. Dolu kovadan art arda iki 1 074 B IP datagram: birincisi hemen çıkar (kovada 126 B
/// kalır), ikincisi eksik 948 B dolunca: 948 / hat. Kova boşken aralık tam datagramın dolma süresi:
/// 1 074 / hat. 136 B'ın ardındaki 1 074 B: eksik 10 B.
///
/// | hat B/s | dolu kova | boş kova | 136 B → 1 074 B (dolu kova) |
/// |---|---|---|---|
/// | 12 500 | 75,84 ms | 85,92 ms | 0,8 ms |
/// | 6 250 | 151,68 ms | 171,84 ms | 1,6 ms |
///
/// Gerçek hat ölçümü (12 500, dolu kova): 75,5–77,9 ms (#175 7175249841). Ayrıca: hız değişince
/// dolma hızı o andan itibaren yeni hız; boşta kova burst'te doyar; kuyruk 64 paket; burst'ten
/// büyük datagram düşer; kovasız (FIFO) kipte aralık kova durumundan bağımsız tam serileşmedir
/// (teşhis kolu).
#[test]
fn k2_bucket_model_matches_the_measured_gaps() {
    println!();
    let tol = 50_000u64;
    let full = K2_FILE_UNIT;
    for (rate, want_full, want_empty, want_small) in [
        (PROFILE_UP, 75_840_000u64, 85_920_000u64, 800_000u64),
        (DROP_TO, 151_680_000, 171_840_000, 1_600_000),
    ] {
        let mut l = k2_line(vec![(0, rate)], Some(K2_BURST));
        let e = k2_exits(&mut l, 0, &[full, full, full]);
        let (g_full, g_empty) = (e[1] - e[0], e[2] - e[1]);
        let mut l = k2_line(vec![(0, rate)], Some(K2_BURST));
        let e = k2_exits(&mut l, 0, &[K2_TEXT_UNIT, full]);
        let g_small = e[1] - e[0];
        let mut l = k2_line(vec![(0, rate)], None);
        let e = k2_exits(&mut l, 0, &[full, full, full]);
        let (f1, f2) = (e[1] - e[0], e[2] - e[1]);
        println!(
            "K2ARAC hat={rate} dolu_kova_ms={} (beklenen {}) bos_kova_ms={} (beklenen {}) \
             kucuk136_tam1074_dolu_kova_ms={} (beklenen {}) fifo_teshis_ms={}/{}",
            ms3(g_full),
            ms3(want_full),
            ms3(g_empty),
            ms3(want_empty),
            ms3(g_small),
            ms3(want_small),
            ms3(f1),
            ms3(f2),
        );
        assert!(
            g_full.abs_diff(want_full) <= tol,
            "hat {rate}: dolu kova {g_full}"
        );
        assert!(
            g_empty.abs_diff(want_empty) <= tol,
            "hat {rate}: boş kova {g_empty}"
        );
        assert!(
            g_small.abs_diff(want_small) <= tol,
            "hat {rate}: 136 → 1 074 {g_small}"
        );
        assert!(
            f1 == f2 && f1.abs_diff(want_empty) <= tol,
            "hat {rate}: FIFO aralığı {f1}/{f2}"
        );
    }
    // Gerçek hat ölçümü (12 500 B/s, dolu kova): 75,5–77,9 ms; model 75,84 ms bu aralıkta.
    assert!((75_500_000..=77_900_000).contains(&75_840_000u64));

    // Boşta kova burst'te doyar: kova boşaldıktan 10 s sonra gelen çift yine dolu kova aralığını
    // verir (jeton 1 200 B'ı aşmaz).
    let mut l = k2_line(vec![(0, PROFILE_UP)], Some(K2_BURST));
    let _ = k2_exits(&mut l, 0, &[full, full, full]);
    let e = k2_exits(&mut l, 10 * SEC, &[full, full]);
    println!(
        "K2ARAC bosta_doyma hat={PROFILE_UP} 10s_sonra_ilk_cikis_gecikmesi_ms={} aralik_ms={}",
        ms3(e[0] - 10 * SEC),
        ms3(e[1] - e[0])
    );
    assert_eq!(e[0], 10 * SEC, "dolu kovadan ilk datagram hemen çıkmadı");
    assert!((e[1] - e[0]).abs_diff(75_840_000) <= tol);

    // Hız değişince dolma hızı o andan itibaren yeni hız: 6 250'de 50 ms (312,5 B), kalan
    // 635,5 B 12 500'de 50,84 ms → ikinci datagram 100,84 ms'de çıkar. Tersi: 12 500'de 50 ms
    // (625 B), kalan 323 B 6 250'de 51,68 ms → 101,68 ms.
    for (a, b, want) in [
        (DROP_TO, PROFILE_UP, 100_840_000u64),
        (PROFILE_UP, DROP_TO, 101_680_000),
    ] {
        let mut l = k2_line(vec![(0, a), (50 * MS, b)], Some(K2_BURST));
        let e = k2_exits(&mut l, 0, &[full, full]);
        println!(
            "K2ARAC hiz_degisimi {a}→{b}@50ms ikinci_cikis_ms={} (beklenen {})",
            ms3(e[1]),
            ms3(want)
        );
        assert!(e[1].abs_diff(want) <= tol, "{a}→{b}: {}", e[1]);
    }

    // Kuyruk 64 paket: aynı anda verilen 70 datagramın ilki hemen çıkar, 64'ü kuyruğa girer, 5'i
    // girişte düşer.
    let mut l = k2_line(vec![(0, PROFILE_UP)], Some(K2_BURST));
    let e = k2_exits(&mut l, 0, &[full; 70]);
    println!(
        "K2ARAC kuyruk verilen=70 gecen={} giriste_dusen={} son_cikis_ms={}",
        e.len(),
        l.dropped_queue.len(),
        ms3(e.last().copied().unwrap_or(0))
    );
    assert_eq!((e.len(), l.dropped_queue.len()), (65, 5));
    // 64 datagramın jetonu: 948 B + 63 × 1 074 B.
    assert!(e[64].abs_diff((948 + 63 * 1_074) * SEC / PROFILE_UP) <= tol);

    // Burst'ten büyük datagram (1 228 B IP) düşer.
    let mut l = k2_line(vec![(0, PROFILE_UP)], Some(K2_BURST));
    let e = k2_exits(&mut l, 0, &[MTU]);
    assert!(e.is_empty() && l.oversize_drops == 1);
}

/// [K2] Kovalı kipte hat modelinin korunum sınaması (aracın tutarlılığı): çıkışlar sıralı ve her
/// `i < j ≤ i + 8` için `(çıkış_i, çıkış_j]` aralığında çıkan IP baytı ≤ (burst − boy_i) + o aralıkta
/// dolan jeton (yuvarlama payı: datagram başına 1 ns'lik dolum).
fn k2_bucket_check(id: &str, sc: &Scenario, o: &Outcome) {
    let Some(burst) = sc.bucket_burst else {
        return;
    };
    assert_eq!(o.oversize_drops, 0, "{id}: burst'ten büyük datagram");
    let p = &o.passes;
    for w in p.windows(2) {
        assert!(
            w[0].0 <= w[1].0 && w[0].1 <= w[1].1,
            "{id}: çıkış sırası bozuk"
        );
    }
    for i in 0..p.len() {
        let mut sum = 0u128;
        for j in i + 1..p.len().min(i + 9) {
            sum += u128::from(p[j].2) * u128::from(SEC);
            let room = u128::from(burst - p[i].2) * u128::from(SEC)
                + plan_fill(&sc.fwd_plan, p[i].1, p[j].1)
                + u128::from(PROFILE_UP) * (j - i) as u128;
            assert!(sum <= room, "{id}: kova korunumu {i}..{j}: {sum} > {room}");
        }
    }
}

/// [K2] Bir ACK satırındaki yoklama.
#[derive(Debug, Clone, Copy)]
struct K2Probe {
    pair: bool,
    target: u64,
    start: u64,
    until: u64,
    dgrams: u64,
    wire: u64,
}

/// [K2] Olaydan sonra göndericinin işlediği bir ACK'in satırı.
#[derive(Debug, Clone, Copy)]
struct K2Row {
    /// Olaydan sonraki sıra (1'den) ve genel sıra (1'den).
    sira: usize,
    index: usize,
    t: u64,
    measured: bool,
    front: bool,
    /// Çift alt sınırı, tel B/s (`LineGuard::pair_wire_bps`: × 1 228 / 1 200; kırpılmamış).
    pair_wire: Option<u64>,
    /// Alıcının çift sayımı; ayna çerçevenin alanını tutmadıysa `None`.
    obs: Option<PairObs>,
    probe: Option<K2Probe>,
    covered: Option<bool>,
    verdict: Option<ProbeOutcome>,
    /// Bu ACK'teki hüküm çift yoklamasının mı (`false`: olağan yoklama); hüküm yoksa `None`.
    verdict_pair: Option<bool>,
    /// Kalıcı tavan ACK'ten hemen önce / sonra; ham tavan (sonra).
    ceil_before: u64,
    ceil: u64,
    raw_pace: u64,
    inflight: u64,
    window: u64,
    refusals: u64,
    pair_probes: u64,
    queue_ns: u64,
    /// [EK 9 · A1] Korumanın deney anahtarı (`AckDiag::pair_probe_on`), ACK'in iz kaydından; kayıt
    /// yazılmadıysa `None`.
    switch_on: Option<bool>,
}

impl K2Row {
    /// Çift ölçüsü kalıcı tavanın (ACK'ten önceki) %5 üstünde: yükseliş kanıtı. Korumanın koşulu
    /// gibi çift önce profil hattına kırpılır; korumanın ret seviyesi kayıtta yoktur (sayılmaz).
    fn eligible(&self) -> bool {
        self.pair_wire
            .is_some_and(|p| p.min(PROFILE_UP) * 100 > self.ceil_before * 105)
    }
}

/// [K2] Darboğaza `[from, to)` aralığında giren datagramların en büyük beklemesi (ns): çıkış −
/// giriş − kendi IP boyunun çıkış anındaki hat hızında süresi (alt sınır 0).
fn k2_queue_peak(o: &Outcome, from: u64, to: u64) -> u64 {
    o.passes
        .iter()
        .filter(|p| p.0 >= from && p.0 < to)
        .map(|&(entry, exit, ip, rate)| (exit - entry).saturating_sub(ip * SEC / rate.max(1)))
        .max()
        .unwrap_or(0)
}

/// [K2] `t` anında tavan `pace` iken kalıcı tavan: [`permanent_ceiling`] (adım izi) ve hükmü
/// `index` sıralı ACK'e dek `Passed` olmamış, penceresi `t`'yi içeren yoklamanın tabanı.
fn k2_permanent(
    spans: &[(u64, u64, u64, u64)],
    probes: &[ProbeRec],
    t: u64,
    pace: u64,
    index: usize,
) -> u64 {
    let mut c = permanent_ceiling(spans, t, pace);
    for p in probes {
        let passed = p
            .chain
            .iter()
            .any(|&(i, v)| i <= index && v.outcome == ProbeOutcome::Passed);
        if p.pace == pace && p.start <= t && t <= p.until && !passed {
            c = c.min(p.base);
        }
    }
    c
}

fn k2_rows(o: &Outcome) -> Vec<K2Row> {
    let spans = temp_spans(o);
    let probes = probes_of(o);
    let mut rows: Vec<K2Row> = Vec::new();
    let mut max_largest: Option<u64> = None;
    let mut prev_t = K2_EVENT_NS;
    let mut prev_refused = o
        .gate_steps
        .iter()
        .rev()
        .find(|s| s.now < K2_EVENT_NS)
        .map_or(0, |s| s.gate_refused);
    for (i, a) in o.gate_acks.iter().enumerate() {
        let largest = o.acks[i].largest;
        let front = max_largest.is_none_or(|m| largest > m);
        max_largest = Some(max_largest.map_or(largest, |m| m.max(largest)));
        if a.arrival < K2_EVENT_NS {
            continue;
        }
        let verdict = a.trace.and_then(|t| t.diag.verdict);
        let shown = probes
            .iter()
            .find(|p| {
                p.from_pair
                    && p.reg_ack.is_some_and(|r| r <= a.index)
                    && p.chain.last().is_some_and(|c| a.index <= c.0)
            })
            .or_else(|| {
                probes
                    .iter()
                    .find(|p| p.chain.iter().any(|c| c.0 == a.index))
            });
        let probe = shown.map(|p| {
            let (dgrams, wire) = p
                .chain
                .iter()
                .rev()
                .find(|c| c.0 <= a.index)
                .map(|c| (c.1.probe_sent_datagrams, c.1.probe_sent_wire))
                .unwrap_or_else(|| {
                    if a.arrival < p.start {
                        (0, 0)
                    } else {
                        let v = gate_view(o, p.start, p.until.min(a.arrival), None);
                        (v.pkts_incl, v.wire_incl)
                    }
                });
            K2Probe {
                pair: p.from_pair,
                target: p.pace,
                start: p.start,
                until: p.until,
                dgrams,
                wire,
            }
        });
        let raw_pace = a.after.pace_bps.unwrap_or(0);
        rows.push(K2Row {
            sira: rows.len() + 1,
            index: a.index,
            t: a.arrival,
            measured: a.measured,
            front,
            pair_wire: a.pair_rate.map(|p| p * FULL_WIRE / MTU),
            obs: a.pair_obs.filter(|x| x.matches),
            probe,
            covered: verdict.map(|v| v.covered),
            verdict: verdict.map(|v| v.outcome),
            verdict_pair: verdict.map(|v| v.from_pair),
            ceil_before: k2_permanent(
                &spans,
                &probes,
                a.arrival.saturating_sub(MS),
                a.before.pace_bps.unwrap_or(0),
                a.index.saturating_sub(1),
            ),
            ceil: k2_permanent(&spans, &probes, a.arrival, raw_pace, a.index),
            raw_pace,
            inflight: a.after.inflight,
            window: a.after.effective_window,
            refusals: a.gate_refused - prev_refused,
            pair_probes: a.pair_probes,
            queue_ns: k2_queue_peak(o, prev_t, a.arrival),
            switch_on: a.trace.map(|t| t.diag.pair_probe_on),
        });
        prev_t = a.arrival;
        prev_refused = a.gate_refused;
    }
    rows
}

fn jo(v: Option<u64>) -> String {
    v.map_or("null".to_string(), |x| x.to_string())
}

fn jms(ns: u64) -> String {
    format!("{:.1}", ns as f64 / MS as f64)
}

fn k2_row_json(key: &str, r: &K2Row) -> String {
    let obs = r.obs.map_or("null".to_string(), |x| {
        format!(
            "{{\"sayilan\":{},\"sikismis\":{},\"sifirlama\":{},\"atlanan\":{},\"silinen\":{},\
             \"sifirsiz_tel\":{}}}",
            x.counted,
            x.squeezed,
            x.resets,
            x.skipped,
            x.erased,
            jo(x.no_reset.map(|p| p * FULL_WIRE / MTU))
        )
    });
    let probe = r.probe.map_or("null".to_string(), |p| {
        format!(
            "{{\"tur\":\"{}\",\"hedef\":{},\"bas_ms\":{},\"bit_ms\":{},\"cikan_dgram\":{},\
             \"cikan_tel\":{}}}",
            if p.pair { "cift" } else { "olagan" },
            p.target,
            p.start / MS,
            p.until / MS,
            p.dgrams,
            p.wire
        )
    });
    format!(
        "{{{key},\"ack_sira\":{},\"ack_genel\":{},\"t_ms\":{},\"olculu\":{},\"cephe_ilerledi\":{},\
         \"cift_tel\":{},\"cift_alici\":{obs},\"yoklama\":{probe},\"kapsadi\":{},\"hukum\":{},\"hukum_tur\":{},\
         \"yoklama_hizi\":{},\"kalici_tavan\":{},\"kalici_tavan_once\":{},\"ham_tavan\":{},\
         \"inflight\":{},\"etkin_pencere\":{},\"kapi_reddi\":{},\"cift_yoklama_sayisi\":{},\
         \"kuyruk_tepe_ms\":{},\"anahtar_acik\":{}}}",
        r.sira,
        r.index,
        r.t / MS,
        r.measured,
        r.front,
        jo(r.pair_wire),
        r.covered.map_or("null".to_string(), |c| c.to_string()),
        r.verdict
            .map_or("null".to_string(), |v| format!("\"{v:?}\"")),
        r.verdict_pair.map_or("null", |p| if p {
            "\"cift\""
        } else {
            "\"olagan\""
        }),
        jo(r.probe.map(|p| p.target)),
        r.ceil,
        r.ceil_before,
        r.raw_pace,
        r.inflight,
        r.window,
        r.refusals,
        r.pair_probes,
        jms(r.queue_ns),
        r.switch_on.map_or("null".to_string(), |b| b.to_string()),
    )
}

/// [K2] Yükseliş kanıtı doğmadıysa neden (alıcının çift sayımından): (sınıf, ayrıntı).
/// `cift_kaniti_silindi`: alıcının sıfırlama olayı (daralma algısı / kanıtlı bırakma) olmasaydı
/// alan tavanın %5 üstünde olurdu; değilse `cift_kaniti_dogmadi`.
fn k2_no_evidence(rows: &[K2Row]) -> (&'static str, String) {
    if rows.iter().any(|r| r.obs.is_none()) {
        return (
            "cift_kaniti_dogmadi",
            "cift_alici yok (ayna tutmadi): silindi ile ayirt edilemedi".to_string(),
        );
    }
    let sum = |f: fn(&PairObs) -> u64| {
        rows.iter()
            .filter_map(|r| r.obs.as_ref())
            .map(f)
            .sum::<u64>()
    };
    let (counted, resets) = (sum(|x| x.counted), sum(|x| x.resets));
    let detail = format!(
        "sayilan={counted} sikismis={} atlanan={} sifirlama={resets} silinen={} cift_tel=[{}] \
         sifirsiz_tel=[{}] kalici_tavan_once=[{}]",
        sum(|x| x.squeezed),
        sum(|x| x.skipped),
        sum(|x| x.erased),
        rows.iter()
            .map(|r| jo(r.pair_wire))
            .collect::<Vec<_>>()
            .join(","),
        rows.iter()
            .map(|r| jo(r.obs.and_then(|x| x.no_reset).map(|p| p * FULL_WIRE / MTU)))
            .collect::<Vec<_>>()
            .join(","),
        rows.iter()
            .map(|r| r.ceil_before.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );
    // "Silindi": sıfırlama olmasaydı alan tavanın %5 üstünde olurdu (aynanın sıfırlamasız alanı).
    // Kaba ölçü (sayılan çift var VE sıfırlama olayı var) alanı değiştirmeyen sıfırlamayı da silme
    // sayar; ikisi ayrışırsa ayrıntıya yazılır.
    let erased = rows.iter().any(|r| {
        r.obs
            .and_then(|x| x.no_reset)
            .is_some_and(|p| (p * FULL_WIRE / MTU).min(PROFILE_UP) * 100 > r.ceil_before * 105)
    });
    let coarse = counted > 0 && resets > 0;
    let detail = if coarse == erased {
        detail
    } else {
        format!("{detail} kaba_olcu_silindi={coarse}")
    };
    if erased {
        ("cift_kaniti_silindi", detail)
    } else {
        ("cift_kaniti_dogmadi", detail)
    }
}

/// [K2] Uçuş kapısının `[from, to]` aralığındaki gönderim denemesi reddi (adım izinden).
fn k2_refusals_between(o: &Outcome, from: u64, to: u64) -> u64 {
    let at = |t: u64| {
        o.gate_steps
            .iter()
            .rev()
            .find(|s| s.now <= t)
            .map_or(0, |s| s.gate_refused)
    };
    at(to) - at(from.saturating_sub(MS))
}

/// [K2] Koşunun özeti (`K2OZET`). Kurallar test belgesinde.
#[allow(clippy::too_many_lines)]
fn k2_summary_json(key: &str, arm: K2Arm, probe_on: bool, o: &Outcome, rows: &[K2Row]) -> String {
    let probes = probes_of(o);
    let spans = temp_spans(o);
    let before_event = o.gate_acks.iter().rfind(|a| a.arrival < K2_EVENT_NS);
    let probes_before = before_event.map_or(0, |a| a.pair_probes);
    let ceil_before_event = o
        .gate_steps
        .iter()
        .rev()
        .find(|s| s.now < K2_EVENT_NS)
        .map_or(0, |s| {
            k2_permanent(
                &spans,
                &probes,
                s.now,
                s.pace,
                before_event.map_or(0, |a| a.index),
            )
        });
    let queue_after = k2_queue_peak(o, K2_EVENT_NS, u64::MAX);
    let queue_at_event = k2_queue_peak(o, K2_EVENT_NS - SEC, K2_EVENT_NS);
    let at6 = rows.get(5).or(rows.last());
    let probes_at6 = at6.map_or(probes_before, |r| r.pair_probes);
    // [EK 9 · DÜZENEK] İkinci yoklama, yükseliş kanıtının doğduğu ACK'ten (o ACK dahil) sonraki
    // altı ACK'e dek başlayan çift yoklamalarından sayılır; kanıt yoksa ya da ondan sonra altıdan
    // az ACK okunduysa SINANMADI.
    let evidence_at = rows.iter().position(K2Row::eligible);
    let after_evidence = evidence_at.map(|k| rows.len() - 1 - k);
    let probes_evidence6 = evidence_at.filter(|&k| k + 6 < rows.len()).map(|k| {
        let before = if k == 0 {
            probes_before
        } else {
            rows[k - 1].pair_probes
        };
        rows[k + 6].pair_probes - before
    });
    let second = match probes_evidence6 {
        None => "SINANMADI",
        Some(n) if n >= 2 => "VAR",
        Some(_) => "YOK",
    };
    let ceil_ack = rows.iter().find(|r| r.ceil >= CEIL_95).map(|r| r.sira);
    // [EK 10 · DÜZENEK] Tel kullanımı: kalıcı tavanın ≥ 11 875 olduğu ilk ACK'ten (o ACK'in adımı
    // dahil) koşu sonuna dek darboğaza GİREN tel baytı (IP boyu; adım izi) × 1000 ÷ (profil hattı ×
    // süre). Yalnız toparlanma kolunda; o ACK yoksa `None`.
    let wire_use_permille = rows
        .iter()
        .find(|r| r.ceil >= CEIL_95)
        .filter(|_| arm == K2Arm::Recover)
        .and_then(|r| {
            let end = o.gate_steps.last()?.now + MS;
            let wire: u64 = o
                .gate_steps
                .iter()
                .filter(|s| s.now >= r.t)
                .map(|s| s.wire)
                .sum();
            let span = end.checked_sub(r.t).filter(|&d| d > 0)?;
            u64::try_from(
                u128::from(wire) * 1000 * u128::from(SEC)
                    / (u128::from(PROFILE_UP) * u128::from(span)),
            )
            .ok()
        });
    let ceil_stable = (0..rows.len())
        .find(|&k| rows[k..].iter().all(|r| r.ceil >= CEIL_95))
        .map(|k| k + 1);
    let ceil_max = rows.iter().map(|r| r.ceil).max();
    let first_eligible = rows.iter().find(|r| r.eligible());
    // Tam kanıt: çift alt sınırı (tel, profile kırpılmış) ≥ 11 875 olan ilk ACK; çerçevenin alanıyla
    // ve aynanın sıfırlamasız alanıyla.
    let full = |p: Option<u64>| p.is_some_and(|p| p.min(PROFILE_UP) >= CEIL_95);
    let full_ack = rows.iter().find(|r| full(r.pair_wire)).map(|r| r.sira);
    let full_no_reset_ack = rows
        .iter()
        .find(|r| full(r.obs.and_then(|x| x.no_reset).map(|p| p * FULL_WIRE / MTU)))
        .map(|r| r.sira);
    // Hüküm zinciri: ACK'in genel sırası, hüküm, kapsama, ölçüm aralığının ilk ve son ucunun
    // gönderim anı (ms; koşul: ilk uç ≤ yoklama başı VE son uç ≥ yoklama sonu) ve alıcının o ACK
    // aralığındaki sıfırlama olayı sayısı (alarm ACK'inin aralığı kanıtın başından başlar).
    let ms_opt = |v: Option<u64>| v.map_or("-".to_string(), |x| (x / MS).to_string());
    let chain = |p: &ProbeRec| {
        p.chain
            .iter()
            .map(|c| {
                format!(
                    "ACK#{}:{:?}(kapsadi={} aralik_gonderim_ms={}..{} alici_sifirlama={})",
                    c.0,
                    c.1.outcome,
                    c.1.covered,
                    ms_opt(c.1.iv_first_send_ns),
                    ms_opt(c.1.iv_last_send_ns),
                    o.gate_acks
                        .iter()
                        .find(|a| a.index == c.0)
                        .and_then(|a| a.pair_obs)
                        .map_or("-".to_string(), |x| x.resets.to_string()),
                )
            })
            .collect::<Vec<_>>()
            .join(">")
    };
    // Kanıtın doğduğu ACK'ten sonra darboğaza giren datagramların kuyruk tepesi (kanıt yoksa
    // `None`): olaydan sonraki tepe, kanıttan önceki bir geçişin (daralmanın kuyruğu) payını da
    // taşıyabilir.
    let queue_after_evidence = first_eligible.map(|e| k2_queue_peak(o, e.t, u64::MAX));
    let gate = if queue_after <= K2_QUEUE_GATE_NS {
        "GECTI"
    } else {
        "KALDI"
    };
    let (target4, broken, false_rise, gate393, why): (&str, &str, &str, &str, String) = match arm {
        K2Arm::Recover => {
            let target4 = if rows.len() < 3 {
                "SINANMADI"
            } else if rows[..3].iter().any(|r| r.ceil >= CEIL_95) {
                "GECTI"
            } else {
                "KALDI"
            };
            let (broken, why) = if target4 != "KALDI" {
                (
                    "yok",
                    if target4 == "SINANMADI" {
                        "olaydan sonra 3'ten az ACK".to_string()
                    } else {
                        String::new()
                    },
                )
            } else {
                let first3 = &rows[..3];
                match first3.iter().find(|r| r.eligible()) {
                    None => k2_no_evidence(first3),
                    Some(e) if probe_on => {
                        let p = probes
                            .iter()
                            .find(|p| p.from_pair && p.reg_ack.is_some_and(|r| r >= e.index));
                        let before_e = o
                            .gate_acks
                            .iter()
                            .rfind(|a| a.index < e.index)
                            .map_or(0, |a| a.pair_probes);
                        let started = rows.last().map_or(0, |r| r.pair_probes) > before_e;
                        match p {
                            None if started => (
                                "ack_kapsamadi",
                                format!(
                                    "cift ACK sira {}: yoklama basladi, hukum kaydi yok",
                                    e.sira
                                ),
                            ),
                            None => (
                                "yoklama_cikmadi",
                                format!(
                                    "cift ACK sira {} (tel {} > 1,05 x {}): yoklama kaydedilmedi; \
                                     olay oncesi cift yoklamasi {probes_before} (tek atis), kapi \
                                     reddi (ilk uc ACK) {}",
                                    e.sira,
                                    jo(e.pair_wire),
                                    e.ceil_before,
                                    first3.iter().map(|r| r.refusals).sum::<u64>()
                                ),
                            ),
                            Some(p) => {
                                let last = p.chain.last().map(|c| c.1);
                                let sent = last.map_or(0, |l| l.probe_sent_datagrams);
                                let refused = k2_refusals_between(o, p.start, p.until);
                                let detail = format!(
                                    "cift ACK sira {}; yoklama [{}-{} ms] hedef {} cikan {} dgram / \
                                     {} B; pencerede kapi reddi {refused}; zincir {}",
                                    e.sira,
                                    p.start / MS,
                                    p.until / MS,
                                    p.pace,
                                    sent,
                                    last.map_or(0, |l| l.probe_sent_wire),
                                    chain(p)
                                );
                                let class = if sent == 0 {
                                    if refused > 0 {
                                        "gonderim_engellendi"
                                    } else {
                                        "yoklama_cikmadi"
                                    }
                                } else if !p.chain.iter().any(|c| c.1.covered) {
                                    "ack_kapsamadi"
                                } else if p
                                    .chain
                                    .iter()
                                    .any(|c| c.1.covered && c.1.outcome == ProbeOutcome::Failed)
                                {
                                    "yanlis_hukum"
                                } else {
                                    "yok"
                                };
                                (class, detail)
                            }
                        }
                    }
                    Some(e) => {
                        if e.ceil > e.ceil_before {
                            (
                                "yok",
                                format!(
                                    "deney kapali: cift ACK sira {} tavani {} > {} acti; ilk uc \
                                     ACK'te 11 875'e varmadi (ilk uc ACK: {})",
                                    e.sira,
                                    e.ceil_before,
                                    e.ceil,
                                    k2_no_evidence(first3).1
                                ),
                            )
                        } else {
                            (
                                "yoklama_cikmadi",
                                format!(
                                    "deney kapali: cift ACK sira {} (tel {}) geldi, tavan {} \
                                     acilmadi",
                                    e.sira,
                                    jo(e.pair_wire),
                                    e.ceil_before
                                ),
                            )
                        }
                    }
                }
            };
            (target4, broken, "null", gate, why)
        }
        K2Arm::False => match first_eligible {
            None => {
                let (_, detail) = k2_no_evidence(rows);
                (
                    "SINANMADI",
                    "yok",
                    "\"SINANMADI\"",
                    "SINANMADI",
                    format!("yanlis yukselis kaniti dogmadi: {detail}"),
                )
            }
            Some(e) => {
                let wrong = probes.iter().find(|p| {
                    p.from_pair
                        && p.start >= K2_EVENT_NS
                        && p.chain
                            .iter()
                            .any(|c| c.1.covered && c.1.outcome == ProbeOutcome::Passed)
                });
                let after: Vec<String> = probes
                    .iter()
                    .filter(|p| p.from_pair && p.start >= K2_EVENT_NS)
                    .map(|p| {
                        format!(
                            "[{}-{} ms hedef {} cikan {} dgram: {}]",
                            p.start / MS,
                            p.until / MS,
                            p.pace,
                            p.chain.last().map_or(0, |c| c.1.probe_sent_datagrams),
                            chain(p)
                        )
                    })
                    .collect();
                (
                    "SINANMADI",
                    if wrong.is_some() {
                        "yanlis_hukum"
                    } else {
                        "yok"
                    },
                    if rows.iter().any(|r| r.ceil > K2_FALSE_RISE) {
                        "\"VAR\""
                    } else {
                        "\"YOK\""
                    },
                    gate,
                    format!(
                        "hedef4 sahte kolda tanimsiz (toparlanma yok); yanlis yukselis kaniti ilk \
                         ACK sira {} (tel {} > 1,05 x {}); olaydan sonraki cift yoklamalari {}",
                        e.sira,
                        jo(e.pair_wire),
                        e.ceil_before,
                        if after.is_empty() {
                            "yok".to_string()
                        } else {
                            after.join(" ")
                        }
                    ),
                )
            }
        },
    };
    format!(
        "{{{key},\"ilk_kirilan\":\"{broken}\",\"kalici_tavan_3ack\":{},\"kalici_tavan_ack\":{},\
         \"hedef4\":\"{target4}\",\"kuyruk_tepe_ms\":{},\"kapi393\":\"{gate393}\",\
         \"kalici_sahte_artis\":{false_rise},\"ikinci_yoklama\":\"{second}\",\
         \"cift_yoklama_sayisi_6ack\":{probes_at6},\"ack_sayisi\":{},\
         \"kalici_tavan_surekli_ack\":{},\"tavan_olay_oncesi\":{ceil_before_event},\
         \"tavan_max_olay_sonrasi\":{},\"cift_yoklama_olay_oncesi\":{probes_before},\
         \"cift_yoklama_sayisi_son\":{},\"kanit_ack\":{},\"kuyruk_olayda_ms\":{},\
         \"kuyruk_dusen\":{},\"kapi_reddi_toplam\":{},\"ayna_tutmadi\":{},\"kapi_son_ret\":{},\
         \"kuyruk_tepe_kanit_sonrasi_ms\":{},\"kapi393_kanit_sonrasi\":{},\"tam_kanit_ack\":{},\
         \"tam_kanit_sifirsiz_ack\":{},\"kanit_sonrasi_ack\":{},\
         \"cift_yoklama_kanit_sonrasi_6ack\":{},\"tel_kullanimi_permil\":{},\
         \"neden\":\"{why}\"}}",
        jo(rows.get(2).map(|r| r.ceil)),
        jo(ceil_ack.map(|k| k as u64)),
        jms(queue_after),
        rows.len(),
        jo(ceil_stable.map(|k| k as u64)),
        jo(ceil_max),
        rows.last().map_or(probes_before, |r| r.pair_probes),
        jo(first_eligible.map(|r| r.sira as u64)),
        jms(queue_at_event),
        o.drop_times.iter().filter(|&&t| t >= K2_EVENT_NS).count(),
        rows.iter().map(|r| r.refusals).sum::<u64>(),
        o.gate_acks
            .iter()
            .filter(|a| a.arrival >= K2_EVENT_NS && !a.pair_obs.is_some_and(|x| x.matches))
            .count(),
        // Koşudaki son ret: [boy, uçuştaki bayt, etkin pencere] (olaydan önce de olabilir).
        o.gate_acks
            .last()
            .and_then(|a| a.gate_last)
            .map_or("null".to_string(), |(size, inflight, window)| format!(
                "[{size},{inflight},{window}]"
            )),
        queue_after_evidence.map_or("null".to_string(), jms),
        queue_after_evidence.map_or("null", |q| if q <= K2_QUEUE_GATE_NS {
            "\"GECTI\""
        } else {
            "\"KALDI\""
        }),
        jo(full_ack.map(|k| k as u64)),
        jo(full_no_reset_ack.map(|k| k as u64)),
        jo(after_evidence.map(|k| k as u64)),
        jo(probes_evidence6),
        jo(wire_use_permille),
    )
}

/// [K2 · EK 9 A6 (10) · GÖNDERİCİ DÜZEYİ BÜTÇE · sahip kararı: «Tek yoklama, en fazla 6 fiilî
/// datagram ve 7.368 tel baytı.» «Küçük datagramlar adede girer.»] Gerçek `Sender`'ın darboğaza
/// verdiği datagramlar adım izinden sayılır (korumanın kendi sayacından bağımsız). Hükmü kayıtlı
/// her çift yoklaması için:
/// * pencerede (`[baş, son]`; pencere açıkken ya da açılmadan bir ACK ile kapandıysa o ACK'in
///   olayına dek) çıkan datagram ≤ 6 ve tel baytı ≤ 7 368;
/// * düzeneğin sayımı korumanın son hüküm kaydındaki sayıyla aynı («fiilen gönderilen tel
///   baytlarını kaydedin»);
/// * bütçeyle kapanan pencerede bütçeyi dolduran datagramdan sonraki ilk datagram yoklama
///   hızında çıkmadı: aradaki süre, o datagramın TABAN tavanındaki serileşmesinden kısa değil
///   (pay 3 ms: pacer'ın 2 ms'lik burst payı + 1 ms'lik adım);
/// * [EK 11 · F3 (d)] pencere başından HEMEN ÖNCE çıkan datagram da sayılır: pencere başına, kendi
///   tel baytının yoklama HEDEFİNDEKİ serileşmesinden (bir yoklama aralığı; taban tavanındaki
///   serileşmesinden kısadır) daha yakın çıkan datagram, pencerenin ilk datagramıyla yoklama
///   aralığında dizilir (F1 deseni: pacer pencere başından ≈ 2 ms önce açılır, o datagram taban
///   tavanında ve korumanın sayımına girmeden çıkar, pencere ardına bütçe kadar datagram dizer).
///   Ön datagramlar + pencere ≤ 6 datagram ve ≤ 7 368 tel baytı (aynı 3 ms pay). Doğru koşuda ön
///   datagram yoktur: pencere başı pacer'ın açılma anıdır ve pacer bir datagramı öncekinin TABAN
///   tavanındaki serileşmesi bitince bırakır.
///
/// Dönen: [`K2Budget`].
///
/// Not: `Sender::track_sent`'teki gönderim sonrası `tick_pair_probe` çağrısı silinirse bu
/// benzetimde (1 ms'lik olay adımı, her datagram hat modelini kendi serileşmesi kadar kapatır)
/// bütçe bir sonraki olayın başındaki `tick`'te, en çok 1 ms geç kapanır ve bu denetimler yeşil
/// kalır; o çağrıyı sabitleyen denetim gönderici birim düzeneğindedir
/// (`sender::pair_probe_unmeasured_ack_tests`: «pencerenin 6. datagramından sonra tavan taban»,
/// arada olay yokken).
fn k2_budget_check(key: &str, o: &Outcome) -> K2Budget {
    const TOLERANCE_NS: i128 = 3_000_000;
    let (mut windows, mut max_pkts, mut max_wire, mut gaps) = (0usize, 0u64, 0u64, 0usize);
    let mut min_slack: Option<i128> = None;
    let (mut lead_windows, mut max_train_pkts, mut max_train_wire) = (0usize, 0u64, 0u64);
    let mut min_lead_gap: Option<u64> = None;
    for p in probes_of(o).iter().filter(|p| p.from_pair) {
        let &(last_ack, last) = p.chain.last().expect("hüküm zinciri boş olamaz");
        let closing = o
            .gate_acks
            .iter()
            .find(|a| a.index == last_ack)
            .expect("hüküm ACK'i kayıtta");
        let budget_closed = last.probe_sent_datagrams >= 6 || last.probe_sent_wire >= 6 * FULL_WIRE;
        // Pencerenin fiilî sonu: bütçeyle kapandıysa kayıttaki son (bütçeyi dolduran gönderimin
        // anı, dahil); kapanış ACK'i pencere bitmeden geldiyse o ACK'in olayına dek; değilse
        // kayıttaki son (hariç).
        let by_ack = !budget_closed && closing.arrival < p.until;
        let end = if by_ack { closing.arrival } else { p.until };
        let (mut pkts, mut wire) = (0u64, 0u64);
        for st in o.gate_steps.iter().filter(|s| s.now >= p.start) {
            if st.now > end || (st.now == end && !budget_closed && !by_ack) {
                break;
            }
            pkts += st.pkts;
            wire += st.wire;
        }
        if by_ack && closing.arrival >= p.start {
            // Kapanış ACK'inin olayında (kapanıştan sonra) çıkanlar pencereye ait değildir.
            pkts -= closing.emitted_pkts;
            wire -= closing.emitted_wire;
        }
        windows += 1;
        max_pkts = max_pkts.max(pkts);
        max_wire = max_wire.max(wire);
        assert!(
            pkts <= 6 && wire <= 6 * FULL_WIRE,
            "{key}: yoklama [{}-{} ms] penceresinde {pkts} datagram / {wire} B (bütçe 6 / {})",
            p.start / MS,
            p.until / MS,
            6 * FULL_WIRE
        );
        assert_eq!(
            (pkts, wire),
            (last.probe_sent_datagrams, last.probe_sent_wire),
            "{key}: yoklama [{}-{} ms]: düzeneğin sayımı ≠ korumanın kaydı",
            p.start / MS,
            p.until / MS
        );
        // [EK 11 · F3 (d)] Pencere başından hemen önce (kendi tel baytının yoklama hedefindeki
        // serileşmesinden kısa süre önce) çıkan datagramlar.
        let (mut lead_pkts, mut lead_wire) = (0u64, 0u64);
        for st in o
            .gate_steps
            .iter()
            .filter(|s| s.now < p.start && s.pkts > 0)
        {
            let probe_ns = i128::from(st.wire) * i128::from(SEC) / i128::from(p.pace.max(1));
            if i128::from(p.start - st.now) + TOLERANCE_NS < probe_ns {
                lead_pkts += st.pkts;
                lead_wire += st.wire;
            }
        }
        lead_windows += usize::from(lead_pkts > 0);
        // Pencerede datagram çıktıysa: pencere başından önceki son datagramın başa uzaklığı.
        if let Some(prev) = o
            .gate_steps
            .iter()
            .rev()
            .find(|s| s.now < p.start && s.pkts > 0)
            .filter(|_| pkts > 0)
        {
            let gap = p.start - prev.now;
            min_lead_gap = Some(min_lead_gap.map_or(gap, |m| m.min(gap)));
        }
        max_train_pkts = max_train_pkts.max(lead_pkts + pkts);
        max_train_wire = max_train_wire.max(lead_wire + wire);
        assert!(
            lead_pkts + pkts <= 6 && lead_wire + wire <= 6 * FULL_WIRE,
            "{key}: yoklama [{}-{} ms]: pencere başından hemen önce {lead_pkts} datagram / \
             {lead_wire} B + pencerede {pkts} datagram / {wire} B (bütçe 6 / {})",
            p.start / MS,
            p.until / MS,
            6 * FULL_WIRE
        );
        if !budget_closed {
            continue;
        }
        // Bütçeyi dolduran datagram: pencerenin son adımındaki tek datagram.
        let Some(fill) = o
            .gate_steps
            .iter()
            .find(|s| s.now == p.until)
            .filter(|s| s.pkts == 1)
        else {
            continue;
        };
        let Some(next) = o.gate_steps.iter().find(|s| s.now > p.until && s.pkts > 0) else {
            continue;
        };
        gaps += 1;
        let base_ns = i128::from(fill.wire) * i128::from(SEC) / i128::from(p.base.max(1));
        let slack = i128::from(next.now - fill.now) - base_ns;
        min_slack = Some(min_slack.map_or(slack, |m| m.min(slack)));
        assert!(
            slack >= -TOLERANCE_NS,
            "{key}: yoklama [{}-{} ms]: bütçeyi dolduran datagramdan ({} B) sonraki datagram {} ms \
             sonra çıktı; taban tavanında ({} B/s) serileşmesi {} ms",
            p.start / MS,
            p.until / MS,
            fill.wire,
            (next.now - fill.now) / MS,
            p.base,
            base_ns / i128::from(MS)
        );
    }
    K2Budget {
        windows,
        max_pkts,
        max_wire,
        gaps,
        min_slack,
        lead_windows,
        max_train_pkts,
        max_train_wire,
        min_lead_gap,
    }
}

/// [K2 · `K2BUTCE`] [`k2_budget_check`]'in koşu özeti.
#[derive(Debug, Clone, Copy)]
struct K2Budget {
    /// Denetlenen çift yoklaması penceresi; pencerede en çok datagram / tel baytı.
    windows: usize,
    max_pkts: u64,
    max_wire: u64,
    /// Bütçeyle kapanıp sonraki datagramın aralığı denetlenen pencere; en kısa «aralık − taban
    /// serileşmesi» farkı (ns).
    gaps: usize,
    min_slack: Option<i128>,
    /// [EK 11 · F3 (d)] Başından hemen önce datagram çıkan pencere; ön datagramlar + pencerede en
    /// çok datagram / tel baytı.
    lead_windows: usize,
    max_train_pkts: u64,
    max_train_wire: u64,
    /// Datagram çıkan pencerelerde, pencere başından önceki son datagramın başa en kısa uzaklığı
    /// (ns; gözlem: denetimin duyarlılığı).
    min_lead_gap: Option<u64>,
}

/// [K2] Kovalı darboğazda iki karşı örnek, aynı düzenekte deney açık ve kapalı (sahip talimatı,
/// #175): gerçek `Sender`, ürün alıcısı ([`RxModel::PRODUCT`]), kovalı darboğaz
/// (`Scenario::bucket_burst`; araç doğrulaması [`k2_bucket_model_matches_the_measured_gaps`]).
///
/// * kol `toparlanma`: hat 12 500 → 8 s'de 6 250 → 20 s'de 12 500; koşu 70 s;
/// * kol `sahte`: hat 12 500 → 8 s'de 6 250 ve kalır; koşu 100 s (EK 9: 80 s'den uzatıldı; kanıt
///   ACK'inden sonra en az altı ACK okunsun); olay anı yine 20 s;
/// * deney `kapali` (`pair_probe` yok) / `acik` (6 datagram); hat `kova` (ESAS) / `fifo` (TEŞHİS:
///   serileştiren FIFO, kova yok); kayıp 0 / anahtarlı ‰200; tohum 1–3; yük `pttsiz` (dosya + 2 s'de
///   bir 108 B metin) / `tek` (dosya + tek iletim PTT, 78 B gövde). Dosya birimi 1 046 B gövde
///   (1 074 B IP).
///
/// YALNIZ RAPOR (`K2ROW`: ACK başına satır, `K2OZET`: koşu başına). `assert` yalnız aracın
/// tutarlılığıdır ([`gate_selfcheck`], [`k2_bucket_check`], gözlemin koşuyu değiştirmemesi,
/// göndericinin koşu boyunca veri beklemesi).
///
/// **ACK sırası.** Olay anından (20 s) sonra göndericinin İŞLEDİĞİ her ACK 1'den sayılır (ölçüm
/// taşımayan ya da cephesi ilerlemeyen dahil). Bu ACK'lerin ilki olaydan önceki varışları da
/// taşıyabilir (ACK'in 62 B/s dönüşte serileşmesi ≥ 1 s).
///
/// **Satır alanları.** `cift_tel`: çerçevenin `pair_rate` alanı × 1 228 / 1 200
/// (`LineGuard::pair_wire_bps`). `cift_alici`: alıcının o ACK aralığındaki çift sayımı
/// ([`PairMirror`]; ayna çerçevenin alanını tutmadıysa `null`). `yoklama`: o ACK işlendiğinde
/// kayıtlı çift yoklaması ya da o ACK'te hükmedilen yoklama (`tur`: `cift` / `olagan`), korumanın
/// hüküm kayıtlarından ([`probes_of`]; hükmü hiç gelmeyen yoklama görünmez, sayısı
/// `cift_yoklama_sayisi`'ndan anlaşılır); `cikan_*`: o ACK'e dek korumanın son hüküm kaydındaki
/// sayı, kayıt yoksa düzeneğin pencere içi sayımı. `hukum` / `kapsadi`: o ACK'te verilen hüküm
/// (`hukum_tur`: hükmün ait olduğu yoklama, `cift` / `olagan`; aynı ACK bir olağan yoklamayı
/// hükmedip bir çift yoklaması kaydedebilir, o zaman `yoklama` çift yoklamasıdır). `kalici_tavan`: [`k2_permanent`] (geçici yoklama
/// tavanı sayılmaz); `ham_tavan`: korumanın o anki tavanı. `kapi_reddi`: önceki satırdan (ilk
/// satırda olaydan) beri `Sender::diag_gate_refusals` farkı (gönderim DENEMESİ sayısı).
/// `cift_yoklama_sayisi`: `LineGuard::pair_probes` (o ana dek başlayan). `kuyruk_tepe_ms`: önceki
/// satırdan beri darboğaza giren datagramlarda en büyük bekleme ([`k2_queue_peak`]).
/// `anahtar_acik` (EK 9 A1): korumanın deney anahtarı, ACK'in iz kaydından (`AckDiag::
/// pair_probe_on`; kayıt yazılmadıysa `null`). Aşağı yönlü örnekle kapanan çift yoklaması artık
/// hüküm kaydı bırakır: `hukum` = `Superseded`, `hukum_tur` = `cift`.
///
/// **`K2BUTCE`** (EK 9 A6 (10); koşu başına, `assert`'li): [`k2_budget_check`]. EK 11 F3 (d):
/// `on_datagramli_pencere` (başından hemen önce datagram çıkan pencere sayısı),
/// `en_cok_dgram_on_dahil` / `en_cok_tel_on_dahil` (ön datagramlar + pencere; ≤ 6 / 7 368),
/// `baslangic_oncesi_en_yakin_ms` (datagram çıkan pencerelerde, baştan önceki son datagramın başa
/// en kısa uzaklığı; gözlem).
///
/// **Özet kuralları.**
/// * `hedef4` (toparlanma kolu): olaydan sonraki ilk üç ACK'in birinde kalıcı tavan ≥ 11 875 →
///   `GECTI`; değilse `KALDI`; üçten az ACK → `SINANMADI`. Sahte kolda `SINANMADI` (tanımsız).
/// * `ilk_kirilan` (`hedef4` = `KALDI`): ilk üç ACK'te yükseliş kanıtı ([`K2Row::eligible`]) yoksa
///   alıcının sayımından `cift_kaniti_silindi` (sıfırlama olayı olmasaydı alan tavanın %5 üstünde
///   olurdu) / `cift_kaniti_dogmadi` (öteki her durum: sayılan çift yok, ya da var ama alan tavanın
///   %5 üstünde değil); kanıt var,
///   deney açık: o ACK'ten sonra kaydedilen ilk çift yoklaması yoksa ya da hiç datagram
///   çıkarmadıysa `yoklama_cikmadi`, datagram çıkmadıysa VE yoklama penceresinde uçuş kapısı bir
///   gönderim denemesini reddettiyse `gonderim_engellendi`; çıktı ama hiçbir hüküm ACK'i
///   kapsamadıysa `ack_kapsamadi`; kapsayan hüküm `Failed` ise `yanlis_hukum` (hat 12 500);
///   hiçbiri değilse `yok` (neden alanında zincir). Deney kapalı: kanıt var, tavan açılmadıysa
///   `yoklama_cikmadi`, açıldı ama 11 875'e varmadıysa `yok`. Sahte kolda: olaydan sonra başlayan
///   bir çift yoklamasının kapsayan hükmü `Passed` ise `yanlis_hukum`.
/// * sahte kol: yükseliş kanıtı olaydan sonraki hiçbir ACK'te doğmadıysa `kalici_sahte_artis` ve
///   `kapi393` `SINANMADI`; doğduysa `kalici_sahte_artis` = kalıcı tavan > 6 875 olan ACK var mı,
///   `kapi393` = olaydan sonraki kuyruk tepesi ≤ 393 ms.
/// * `ikinci_yoklama` (EK 9): yükseliş kanıtının doğduğu ACK'ten (`kanit_ack`; o ACK dahil)
///   sonraki altıncı ACK'e dek başlayan çift yoklaması (`cift_yoklama_kanit_sonrasi_6ack`) ≥ 2 →
///   `VAR`, değilse `YOK`; kanıt doğmadıysa ya da kanıt ACK'inden sonra altıdan az ACK okunduysa
///   (`kanit_sonrasi_ack` < 6) `SINANMADI`. (1a5fc80c kaydındaki kural olaydan sonraki altı ACK'i
///   sayıyordu; o sayı `cift_yoklama_sayisi_6ack` alanında durur.)
/// * Ek alanlar (kural dışı, okuma için): `kuyruk_tepe_kanit_sonrasi_ms` / `kapi393_kanit_sonrasi`
///   (kuyruk tepesi yalnız kanıtın doğduğu ACK'ten sonra giren datagramlarda; olaydan sonraki tepe
///   daralmanın kuyruğunu da taşıyabilir), `kuyruk_olayda_ms` (olaydan önceki son 1 s),
///   `tavan_olay_oncesi`, `tavan_max_olay_sonrasi`, `kalici_tavan_surekli_ack` (kalıcı tavanın
///   koşu sonuna dek ≥ 11 875 kaldığı ilk ACK), `kanit_ack`, `cift_yoklama_olay_oncesi`,
///   `kuyruk_dusen`, `kapi_reddi_toplam`, `kapi_son_ret`, `ayna_tutmadi`, `tam_kanit_ack` /
///   `tam_kanit_sifirsiz_ack` (çift alt sınırı ≥ 11 875 olan ilk ACK: çerçevenin alanıyla / alıcının
///   sıfırlama olayları olmasaydı), `tel_kullanimi_permil` (EK 10; yalnız toparlanma kolu: kalıcı
///   tavanın ≥ 11 875 olduğu ilk ACK'ten — `kalici_tavan_ack` — koşu sonuna dek darboğaza GİREN
///   tel baytı (IP boyu, kuyrukta düşenler dahil) × 1000 ÷ (12 500 × süre); o ACK yoksa ve sahte
///   kolda `null`), `neden`.
///
/// Koşu: `cargo test --release -p adaptive-transport --test starved_line_sim k2_ -- --nocapture
/// --test-threads=1`.
#[test]
fn k2_bucket_two_counterexamples_report() {
    println!();
    assert!(
        std::env::var("KAMA_PAIR_PROBE_DATAGRAMS").is_err(),
        "KAMA_PAIR_PROBE_DATAGRAMS ortamda kurulu: deney kapalı kollar kapalı olmaz"
    );
    println!(
        "K2NOT dosya_udp={K2_FILE_UNIT} dosya_ip={} metin_udp={K2_TEXT_UNIT} metin_ip={} \
         ptt_tek_udp={PTT_SINGLE_UNIT} ptt_tek_ip={} kova_burst={K2_BURST} kuyruk_paket=64 \
         yayilim_ms=300 olay_ms={} dusus_ms={} metin_ilk_ms={} metin_aralik_ms={}",
        K2_FILE_UNIT + OVH,
        K2_TEXT_UNIT + OVH,
        PTT_SINGLE_UNIT + OVH,
        K2_EVENT_NS / MS,
        DROP_AT_NS / MS,
        K2_TEXT_AT_NS / MS,
        K2_TEXT_PERIOD_NS / MS,
    );
    for arm in [K2Arm::Recover, K2Arm::False] {
        for probe in [false, true] {
            for bucket in [true, false] {
                for loss in [0u64, 200] {
                    for seed in 1..=3u64 {
                        for load in [PttLoad::None, PttLoad::Single] {
                            let sc = k2_scenario(arm, probe, bucket, loss, seed, load);
                            let key = format!(
                                "\"kol\":\"{}\",\"deney\":\"{}\",\"hat\":\"{}\",\
                                 \"kayip_permil\":{loss},\"tohum\":{seed},\"yuk\":\"{}\"",
                                arm.label(),
                                if probe { "acik" } else { "kapali" },
                                if bucket { "kova" } else { "fifo" },
                                load_label(load),
                            );
                            let o = run(&sc);
                            // Aracın tutarlılığı.
                            gate_selfcheck(&key, &sc, &o);
                            k2_bucket_check(&key, &sc, &o);
                            assert!(
                                o.file_done_ns.is_none()
                                    && o.gate_steps.iter().all(|s| s.unsent > 0),
                                "{key}: gönderici veri beklemedi (dosya bitti)"
                            );
                            assert_eq!(
                                o.gate_steps.last().map(|s| s.now),
                                Some(sc.t_max_ns - MS),
                                "{key}: koşu erken bitti"
                            );
                            if seed == 1 {
                                let bare = run(&Scenario {
                                    gate_diag: false,
                                    ..sc.clone()
                                });
                                assert!(
                                    o.acks_at == bare.acks_at
                                        && o.per_ack == bare.per_ack
                                        && o.passes == bare.passes
                                        && o.drop_times == bare.drop_times
                                        && o.pace_trace == bare.pace_trace
                                        && o.line.lost_bytes == bare.line.lost_bytes,
                                    "{key}: gözlem koşuyu değiştirdi"
                                );
                            }
                            let rows = k2_rows(&o);
                            // [EK 9 · A1] Kaydı yazılan her ACK korumanın anahtarını taşır ve
                            // senaryonun anahtarıyla aynıdır.
                            assert!(
                                rows.iter()
                                    .all(|r| r.switch_on.is_none_or(|on| on == probe)),
                                "{key}: korumanın anahtar kaydı senaryoyla uyuşmuyor"
                            );
                            for r in &rows {
                                println!("K2ROW {}", k2_row_json(&key, r));
                            }
                            println!("K2OZET {}", k2_summary_json(&key, arm, probe, &o, &rows));
                            // [EK 9 · A6 (10)] Gönderici düzeyi bütçe (deney kapalıyken çift
                            // yoklaması yoktur: denetlenen pencere 0).
                            let b = k2_budget_check(&key, &o);
                            assert!(
                                probe || b.windows == 0,
                                "{key}: deney kapalıyken çift yoklaması"
                            );
                            println!(
                                "K2BUTCE {{{key},\"pencere\":{},\"en_cok_dgram\":{},\
                                 \"en_cok_tel\":{},\"aralik_denetlenen\":{},\
                                 \"yedinci_en_kisa_fark_ms\":{},\"on_datagramli_pencere\":{},\
                                 \"en_cok_dgram_on_dahil\":{},\"en_cok_tel_on_dahil\":{},\
                                 \"baslangic_oncesi_en_yakin_ms\":{}}}",
                                b.windows,
                                b.max_pkts,
                                b.max_wire,
                                b.gaps,
                                b.min_slack.map_or("null".to_string(), |s| format!(
                                    "{:.1}",
                                    s as f64 / MS as f64
                                )),
                                b.lead_windows,
                                b.max_train_pkts,
                                b.max_train_wire,
                                b.min_lead_gap.map_or("null".to_string(), jms),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// [K2 · ARAÇ DOĞRULAMASI] Uçuş kapısının ret sayacı (`Sender::diag_gate_refusals`) boş sayaç
/// değildir: kapının bağladığı bilinen kolda ([`flight_gate_report_sees_a_binding_gate`] kol A:
/// geri bildirim ufku 0,3 s, hat 7 500'e toparlanır) sayaç artar ve son ret kapının koşulunu
/// sağlar (`uçuş + boy > etkin pencere`); kontrol kolunda (ürün ufku) sayaç 0 kalır.
#[test]
fn k2_gate_refusal_counter_sees_a_binding_gate() {
    println!();
    let arm = |horizon: Option<u64>| Scenario {
        fwd_plan: vec![
            (0, PROFILE_UP),
            (DROP_AT_NS, DROP_TO),
            (RECOVER_AT_NS, BOUND_ARM_RECOVER_TO),
        ],
        rx: RxModel::PRODUCT,
        pair_probe: Some(6),
        gate_diag: true,
        feedback_ns_override: horizon,
        ..drop_recovery(1, PttLoad::None)
    };
    let last = |o: &Outcome| {
        (
            o.gate_steps.last().map_or(0, |s| s.gate_refused),
            o.gate_acks.last().and_then(|a| a.gate_last),
        )
    };
    let (bound, bound_last) = last(&run(&arm(Some(BOUND_ARM_HORIZON_NS))));
    let (control, control_last) = last(&run(&arm(None)));
    println!(
        "K2ARAC kapi_ret_sayaci bagli_kol_ret={bound} son_ret_boy_ucus_pencere={bound_last:?} \
         kontrol_kolu_ret={control} son_ret={control_last:?}"
    );
    assert!(bound > 0, "bağlı kolda ret sayılmadı");
    let (size, inflight, window) = bound_last.expect("bağlı kolda son ret kaydı yok");
    assert!(
        inflight + size > window,
        "son ret kapının koşulunu sağlamıyor: {inflight} + {size} ≤ {window}"
    );
    assert_eq!(control, 0, "kontrol kolunda ret sayıldı");
    assert!(control_last.is_none());
}
