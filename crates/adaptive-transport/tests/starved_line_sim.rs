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

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::ack::{
    ACK_BITMAP_MAX_BITS, ACK_DELAY_UNIT_US, PAIR_MIN_UDP_BYTES, PAIR_RATE_NONE, RX_SPAN_NONE,
};
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, EmitCause, EmitDiag, Event, FeedbackStarvedRule, ForwardCopyLineScale,
    ForwardCopyRule, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind,
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
/// Ürünün KOPYALI PTT kuralı (`client_core::runtime::PTT_FORWARD_COPIES` = 7,
/// `PTT_FORWARD_COPIES_MIN` = 6; aralık 200 ms / 7). 2026-11-16 kuralı: 5 iletim, 40 ms.
///
/// [Y4 · sahip kararı 2026-11-22, #175 7080178069] Bu kural artık ürünün KAPI sözleşmesi değildir.
/// Ürün eşten ilk 0x16 ACK'ini görünce (CAPS bit 1: eş 0x09 yalın PTT'yi çözer) PTT kuralını
/// `ptt_forward_copy_rule(0, 0)`'a çevirir: tek iletim + öncelik, yalın 0x09 çerçevesi, son tarihte
/// bırakma (bu düzenekte [`lean`]). Kopyalı kural yalnız eski uçla (0x15 yazan eş) ve ilk 0x16'ya
/// dek kalır. Sabitlerin değeri değişmez; [`Scenario::honest`]'in varsayılanı olarak kopyalı kolları
/// (GERİLİM / eski uç kolu) kurar. Kapı kolları dosyanın sonundaki yalın bölümdedir.
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
    /// `Sender::abandon_units` ile bildirilir (ürünün son tarih yolu).
    ptt_deadline_ns: Option<u64>,
    /// [PTT-YALIN · BAŞLANGIÇ · aday f4cd325c] `true`: bırakma göndericinin aldığı ilk ölçülü ACK'ten
    /// (`rx_span` taşıyan ilk ACK; benzetimde ürünün ilk 0x16'sının vekili) önce yapılmaz; o ACK'te
    /// sunumundan beri son tarihi geçmiş teyitsiz demetler bırakılır (ürünün `ptt_pre_lean` →
    /// `ptt_offered_at` geçişi). `false`: bırakma t = 0'dan (eski ayna).
    ptt_deadline_from_first_measured_ack: bool,
    /// `> 0`: `text_at_ns`'ten sonra bu aralıkla konuşma bitene dek ek metin birimleri
    /// (`TEXT_ID + k`). 0: tek metin birimi.
    text_period_ns: u64,
    seed: u64,
    t_max_ns: u64,
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
            ptt_deadline_from_first_measured_ack: false,
            text_period_ns: 0,
            seed,
            t_max_ns: 60 * SEC,
        }
    }
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

    fn send(&mut self, now: u64, seq: u64, udp: u64, unit_id: u64) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.queue_pkts {
            self.dropped_queue.push(now);
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
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + (udp + OVH) * SEC / self.rate_at(start);
        self.ser_start.insert(seq, start);
        self.busy.push_back(done);
        self.waits.push((now, start - now));
        let n = self.tx_of_unit.entry(unit_id).or_insert(0);
        *n += 1;
        let targeted = self.targeted.is_some_and(|(u, k)| u == unit_id && *n <= k);
        let lost = self.rand_permille() < self.loss_permille;
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
        if lost || targeted {
            return;
        }
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
}

struct ReturnLine {
    plan: Vec<(u64, u64)>,
    free_at: u64,
    last_arrival: u64,
    wire: VecDeque<(u64, u64, u16, AckInfo, IntervalMsg)>,
    /// Her ACK: (alıcının verdiği an, serileşmenin başladığı an).
    log: Vec<(u64, u64)>,
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
///   [SIKIŞMA · aday 94c7c6e8] Alıcı aralığın kendi varış hızını da tutar: önceki varışı olan her
///   datagramın gövdesi `win_bytes`'a, varış aralığı (en çok [`PAIR_IDLE_GAP_NS`]) `win_ns`'e
///   yazılır; en büyük ikinci [`PAIR_WINDOW_RATIO_MAX`] × `win_bytes / win_ns`'i aşarsa alan
///   `PAIR_RATE_NONE` (`PairRate::bound`). Aralık ACK'te sıfırlanır.
///
/// Alıcının 0x15 çerçevesinin kendisi (boyu, daralma alarmının erken ACK'i) bu düzenekte yoktur: ACK
/// kadansı 0x14 alıcısınındır. (Gerçek alıcı bitmap kipinde bu düzenekte yeniden sıralamada panik
/// verdi: en büyüğü ilerlemeyen ACK'te `span_first` aralığı `largest + 1 ..= largest`.)
#[derive(Default)]
struct RxMirror {
    first_arrival: HashMap<u64, u64>,
    received: std::collections::BTreeSet<u64>,
    /// Son iki çıkan ACK'in largest'ı (alıcının `bitmap_emitted`).
    emitted: VecDeque<u64>,
    pair_prev: Option<u64>,
    pair_top: [u64; 2],
    /// [SIKIŞMA] Bu ACK aralığında varan UDP gövde baytı (önceki varışı olan datagramlar).
    pair_win_bytes: u64,
    /// [SIKIŞMA] Aynı datagramların varış aralıkları toplamı (ns; her aralık en çok
    /// [`PAIR_IDLE_GAP_NS`]).
    pair_win_ns: u64,
}

/// [SIKIŞMA] Çift değerinin aralığın kendi varış hızına oranının üst sınırı; `receiver.rs`
/// `PAIR_WINDOW_RATIO_MAX` ile aynı (orada crate içi sabit).
const PAIR_WINDOW_RATIO_MAX: u64 = 4;

/// [SIKIŞMA] Aralık süresine yazılan tek varış aralığının üst sınırı (ns); `receiver.rs`
/// `PAIR_IDLE_GAP_NS` ile aynı (orada crate içi sabit).
const PAIR_IDLE_GAP_NS: u64 = SEC;

impl RxMirror {
    fn on_arrival(&mut self, seq: u64, udp: u64, now: u64) {
        if let Some(p) = self.pair_prev {
            let g = now.saturating_sub(p);
            self.pair_win_bytes = self.pair_win_bytes.saturating_add(udp);
            self.pair_win_ns = self.pair_win_ns.saturating_add(g.min(PAIR_IDLE_GAP_NS));
            if udp >= PAIR_MIN_UDP_BYTES && g >= 2 * MS {
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
        // [SIKIŞMA] `PairRate::bound`: en büyük ikinci aralığın varış hızının
        // `PAIR_WINDOW_RATIO_MAX` katını aşarsa alan yok.
        let v = self.pair_top[1];
        let plausible = u128::from(v) * u128::from(self.pair_win_ns)
            <= u128::from(PAIR_WINDOW_RATIO_MAX)
                * u128::from(self.pair_win_bytes)
                * u128::from(SEC);
        let pair = if v == 0 || !plausible {
            PAIR_RATE_NONE
        } else {
            u16::try_from(v.min(u64::from(PAIR_RATE_NONE) - 1)).unwrap_or(0)
        };
        self.pair_top = [0, 0];
        self.pair_win_bytes = 0;
        self.pair_win_ns = 0;
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

    fn send(&mut self, now: u64, udp: u64, arrival_rate: u16, ack: AckInfo, iv: IntervalMsg) {
        let start = self.free_at.max(now);
        self.log.push((now, start));
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        let arrival = (self.free_at + self.delay_at(self.free_at)).max(self.last_arrival);
        self.last_arrival = arrival;
        self.wire.push_back((arrival, udp, arrival_rate, ack, iv));
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
        feedback_ns: feedback_horizon_ns(PROFILE_DOWN),
        line: LineModel {
            rate_bps: PROFILE_UP,
            datagram_overhead: OVH,
        },
        return_line: LineModel {
            rate_bps: PROFILE_DOWN,
            datagram_overhead: OVH,
        },
    }));
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
    r.set_return_bandwidth(Some(sc.ack_return_bw));
    let mut fwd = Bottleneck {
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
    };
    s.set_diag_emit_log(sc.diag);
    let mut reorder_lcg = sc.seed ^ 0x5A5A_5A5A;
    let mut ret = ReturnLine {
        plan: sc.rev_delay_plan.clone(),
        free_at: 0,
        last_arrival: 0,
        wire: VecDeque::new(),
        log: Vec::new(),
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
    // [PTT-YALIN · BAŞLANGIÇ] Gönderici ilk ölçülü ACK'i aldı (ürünün ilk 0x16 vekili).
    let mut measured_ack_seen = false;
    let mut file_seen: HashSet<u64> = HashSet::new();
    let mut file_offered = 0u64;
    let mut file_done_ns = None;
    let mut tx = TxCount::default();
    let mut rate_trace: Vec<(u64, Option<u64>)> = Vec::new();
    let mut acks_at = Vec::new();
    let mut pace_trace: Vec<(u64, u64)> = Vec::new();
    let mut ack_measured = Vec::new();
    let mut per_ack = Vec::new();
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Bottleneck, tx: &mut TxCount| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            tx.datagrams += 1;
            tx.last_seq = tx.last_seq.max(d.seq);
            if d.unit_id < TEXT_ID {
                tx.ptt_wire_bytes += d.size + OVH;
                tx.ptt_kind.insert(d.seq, d.kind);
            }
            fwd.send(now, d.seq, d.size, d.unit_id);
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
                    size: FILE_UNIT,
                }),
                now,
            );
            file_offered += 1;
            emit(decs, now, &mut fwd, &mut tx);
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
            emit(decs, now, &mut fwd, &mut tx);
        }
        if sc.text_at_ns == Some(now) {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: TEXT_ID,
                    size: 80,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut tx);
        }
        if let Some(t0) = sc.text_at_ns.filter(|_| sc.text_period_ns > 0) {
            let at = t0 + (extra_text_offered.len() as u64 + 1) * sc.text_period_ns;
            if at < sc.voice_until_ns && now >= at {
                let id = TEXT_ID + 1 + extra_text_offered.len() as u64;
                extra_text_offered.push(at);
                let decs = s.on_event(Event::AppData(OpaqueUnit { id, size: 80 }), now);
                emit(decs, now, &mut fwd, &mut tx);
            }
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut tx);
        // [PTT-YALIN] Ürünün son tarih yolu (`runtime`, pace tick'te Pto'dan sonra): teyitsiz ve
        // sunumundan `deadline` geçmiş PTT birimleri motora teyitli bildirilir. [BİRLEŞİM]
        // `ptt_deadline_from_first_measured_ack`: teyit drenajı her tick'te, bırakma yalnız ilk
        // ölçülü ACK'ten sonra (ürünün ilk 0x16'ya dek `ptt_pre_lean`'i).
        if let Some(deadline) = sc.ptt_deadline_ns {
            ptt_confirmed.extend(
                s.take_confirmed_units()
                    .into_iter()
                    .filter(|&u| u < TEXT_ID),
            );
            let lean_started = measured_ack_seen || !sc.ptt_deadline_from_first_measured_ack;
            let due: Vec<u64> = voice_offered
                .iter()
                .enumerate()
                .map(|(k, &at)| (k as u64, at))
                .filter(|&(k, at)| {
                    lean_started
                        && now.saturating_sub(at) >= deadline
                        && !ptt_confirmed.contains(&k)
                        && !ptt_expired.iter().any(|&(u, _, _)| u == k)
                })
                .map(|(k, _)| k)
                .collect();
            if !due.is_empty() {
                for &u in &due {
                    ptt_expired.push((u, now, tx.last_seq));
                }
                // Ürün 8c505346'dan beri `Sender::abandon_units` ile bırakır (FEC defterine
                // dokunmaz); önceki `RecoveredControl { block_id 0, esi 0 }` yolu emekli.
                s.abandon_units(&due);
            }
        }
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut tx);
        }
        // Alıcı.
        r.set_uplink_yield(ret.free_at > now);
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
        let mut acks = Vec::new();
        for (seq, uid, udp) in deliver {
            rx.on_arrival(seq, udp, now);
            r.set_next_datagram_bytes(udp);
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: RxKind::Data,
                },
                now,
            );
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
            acks.extend(
                out.ack
                    .map(|f| (f, rx.on_ack(r.largest_seq().unwrap_or(0)))),
            );
        }
        acks.extend(
            r.on_ack_timer(now)
                .map(|f| (f, rx.on_ack(r.largest_seq().unwrap_or(0)))),
        );
        for (frame, (m_first, largest, rx_span, m, pair_rate)) in acks {
            let expected = largest + 1;
            let udp = frame.encode().len() as u64 + ACK_SEAL;
            // Ölçüm aralığı ve çift alt sınırı alıcının kuralıyla (`RxMirror`).
            let rx_span =
                rx_span.filter(|_| !sc.no_interval.is_some_and(|(a, b)| now >= a && now < b));
            ret.send(
                now,
                udp,
                frame.arrival_rate,
                AckInfo {
                    acked_seqs: frame.acked_full(expected),
                    ack_delay: frame.ack_delay_duration(),
                },
                (m_first, largest, rx_span, m, pair_rate),
            );
        }
        while ret.wire.front().is_some_and(|&(t, _, _, _, _)| t <= now) {
            let (_, udp, arrival_rate, ack, (first, last, rx_span, m, pair_rate)) =
                ret.wire.pop_front().expect("var");
            let (sent, start) = ret.log[ack_log.len()];
            ack_log.push(AckRec {
                sent,
                start,
                arrival: now,
                udp,
                largest: ack.acked_seqs.iter().copied().max().unwrap_or(0),
            });
            acks_at.push(now);
            ack_measured.push(rx_span.is_some());
            measured_ack_seen |= rx_span.is_some();
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
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut tx);
            let g = s.diag_line_guard().and_then(|g| g.0);
            if rate_trace.last().map(|&(_, r)| r) != Some(g) {
                rate_trace.push((now, g));
            }
            let p = s.diag_line_guard_pace().map_or(0, |p| p.0);
            if pace_trace.last().map(|&(_, q)| q) != Some(p) {
                pace_trace.push((now, p));
            }
            let largest = ack_log.last().map_or(0, |a| a.largest);
            let ser = fwd.ser_start.get(&largest).copied().unwrap_or(0);
            per_ack.push((now, ser, g, p));
        }
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
        emit_log: s.take_diag_emit_log(),
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
    }
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`PRODUCT_COPIES`]; ürün bunu yalnız eski uçla kullanır); iddiaları değişmedi. Kapı kolu:
/// [`lean_a_return_delay_rise_does_not_cut_an_honest_line`].
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
#[test]
fn b_real_capacity_drop_is_detected_and_the_queue_drains() {
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    let mut from_drop = Vec::new();
    for seed in 1..=3u64 {
        let sc = Scenario {
            text_at_ns: Some(DROP_TEXT_AT_NS),
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
        let control = run(&drop_scenario(seed, false));
        report("b/karşı-olgu", &drop_scenario(seed, false), &control);
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`PRODUCT_COPIES`]; ürün bunu yalnız eski uçla kullanır); iddiaları değişmedi. Kapı kolu:
/// [`lean_d1_random_loss_is_not_queue_growth`].
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
#[test]
fn d2_queue_overflow_still_converges_and_stops_overflowing() {
    for seed in 1..=3u64 {
        let sc = drop_scenario(seed, true);
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`PRODUCT_COPIES`]; ürün bunu yalnız eski uçla kullanır); iddiaları değişmedi. Kapı kolu:
/// [`lean_e1_unmeasured_acks_do_not_bring_back_the_return_delay_cut`].
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
/// **GERİLİM TESTİ — sahip kararı 2026-11-22 (#175 7080178069): kapı yalın sözleşme.** Bu kol
/// kopyalı PTT yüküyle (ürünün eski uçla konuştuğu kural) koşar; kapı aynı senaryonun yalın kolu
/// [`e2_lean_a_long_unmeasured_gap_on_a_real_drop_keeps_the_queue_bounded`]'dır. Hat korumasının
/// üç iddiası ([`E2Guard::assert_bounds`]) aynı sınırlarla kalır. Eski "yakınsamadan 9 s sonra
/// PTT ≤ 1 s (sıralı teslim, en kötü demet)" iddiası kopyalı PTT sözleşmesinin kapısıydı (sıralı
/// şerit, her demet); onun yerine (nedensel kayıt #175 7080561058):
/// * CANLILIK: her demetin sıralı teslimi planından ≤ [`LIVENESS_HORIZON_NS`] (ürün testinin
///   `LIVENESS_HORIZON`, 15 s);
/// * yakınsamadan 9 s sonrasından konuşmanın sonuna dek planlanan her demet İLK kez planından ≤ 1 s
///   içinde varır: pencere ve sınır eski iddianınki, anlam sahibin media ölçütü (geç demet atlanır,
///   baş-sıra beklenmez). Yalnız canlılık ölçülenden zayıftı (ölçüm, taban 1482c197: bu pencerede
///   ilk varış 22/22, 24/24, 26/26; sıralı en kötü 2,10 / 3,89 / 0,49 s).
///
/// Sıralı en kötü gecikme ve evre başına zamanında oranı raporlanır (iddia değil). Sıralı
/// gecikmenin kaynağı borç evresi kuralının ("borç ödeme evresini kapsayan aralık artış örneği
/// vermez", `line_guard`) yakınsamayı bir ACK aralığı geciktirmesi; karşı-olgu (kural kapalı):
/// tohum 1/2 yakınsama 26,65 → 24,57 / 26,27 → 24,19 s, sıralı PTT 0,68 / 0,67 s; yalın kol değişmez.
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
///   yakınsamadan 2 s sonra bekleme ≤ iki tam datagram; yakınsamadan 9 s sonra PTT (yukarıda).
#[test]
fn e2_a_long_unmeasured_gap_on_a_real_drop_keeps_the_queue_bounded() {
    for seed in 1..=3u64 {
        let sc = e2_scenario(seed);
        let o = run(&sc);
        report("e2/aday", &sc, &o);
        let g = E2Guard::of(&o);
        g.print(&format!("E2 tohum {seed}"), &o);
        g.assert_bounds(seed);
        let ptt_from = g.conv + 9 * SEC;
        let inorder_worst = o.voice_max_from(0);
        let window: Vec<(u64, Option<u64>)> = o
            .voice
            .iter()
            .enumerate()
            .filter(|&(_, &(plan, _))| plan >= ptt_from)
            .map(|(v, &(plan, _))| (plan, o.voice_first[v].map(|a| a - plan)))
            .collect();
        let window_on_time = window
            .iter()
            .filter(|&&(_, d)| d.is_some_and(|d| d <= PTT_BOUND_NS))
            .count();
        println!(
            "E2-GERİLİM tohum {seed}: sıralı en kötü (bütün demetler) {} (canlılık ufku {}), \
             yakınsamadan 9 s sonra sıralı en kötü {}, aynı pencerede ilk varış ≤ 1 s {}/{} | {}",
            secs(inorder_worst),
            secs(LIVENESS_HORIZON_NS),
            secs(o.voice_max_from(ptt_from)),
            window_on_time,
            window.len(),
            phases_line(&ptt_phases(&sc, &o))
        );
        assert!(
            inorder_worst <= LIVENESS_HORIZON_NS,
            "tohum {seed}: canlılık: en kötü sıralı teslim {} > {}",
            secs(inorder_worst),
            secs(LIVENESS_HORIZON_NS)
        );
        assert!(
            !window.is_empty() && window_on_time == window.len(),
            "tohum {seed}: yakınsamadan 9 s sonra ilk varışı 1 s'yi aşan demet: {:?}",
            window
                .iter()
                .filter(|&&(_, d)| d.is_none_or(|d| d > PTT_BOUND_NS))
                .map(|&(p, d)| (secs(p), d.map_or("YOK".to_string(), secs)))
                .collect::<Vec<_>>()
        );
    }
}

/// [CANLILIK] Ürün testinin `LIVENESS_HORIZON`'ı (`kama-integration/tests/asym_product_outcome.rs`):
/// planlanan her PTT demeti planından en çok 15 s içinde teslim edilir.
const LIVENESS_HORIZON_NS: u64 = 15 * SEC;

/// (e2)'nin senaryosu: (b)'nin düşüşü, alıcı 8–19 s arasında ölçüm taşımaz.
fn e2_scenario(seed: u64) -> Scenario {
    Scenario {
        no_interval: Some((DROP_AT_NS, 19 * SEC)),
        ..drop_scenario(seed, true)
    }
}

/// (e2)'nin hat koruması ölçüsü (sınırların türetimi: e2 belgesi).
struct E2Guard {
    run_len: usize,
    t_gap: u64,
    q_gap: u64,
    gap_from: u64,
    t_back: u64,
    w_gap: u64,
    conv: u64,
    conv_bound: u64,
    w_after: u64,
}

impl E2Guard {
    fn of(o: &Outcome) -> Self {
        let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
        let (run_len, t_gap, t_back) = o.longest_unmeasured_run();
        let (t_gap, t_back) = (
            t_gap.expect("ikinci ölçümsüz ACK"),
            t_back.expect("ölçüm döndü"),
        );
        let q_gap = o.max_wait_between(0, t_gap);
        let gap_from = t_gap + q_gap + SEC;
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max()
            .expect("düşüşten önce ACK'ler");
        let conv = converged_at(o, DROP_TO).expect("yakınsadı");
        Self {
            run_len,
            t_gap,
            q_gap,
            gap_from,
            t_back,
            w_gap: o.max_wait_between(gap_from, t_back),
            conv,
            conv_bound: t_back + 3 * t_ack + t_ret,
            w_after: o.max_wait_between(conv + 2 * SEC, u64::MAX),
        }
    }

    fn print(&self, tag: &str, o: &Outcome) {
        println!(
            "{tag}: ölçümsüz dizi {} ACK, t_gap {} Q_gap {} boşluk beklemesi ({}–{}) {} \
             yakınsama {} (sınır {}) yakınsama sonrası bekleme {} tavan izi {:?}",
            self.run_len,
            secs(self.t_gap),
            secs(self.q_gap),
            secs(self.gap_from),
            secs(self.t_back),
            secs(self.w_gap),
            secs(self.conv),
            secs(self.conv_bound),
            secs(self.w_after),
            o.pace_trace
        );
    }

    /// Düzenek iddiası (ölçümsüz dizi ≥ 3) + hat korumasının üç iddiası.
    fn assert_bounds(&self, seed: u64) {
        let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
        let gap_wait_bound = PTT_BOUND_NS - ONE_WAY_NS;
        assert!(
            self.run_len >= 3,
            "tohum {seed}: düzenek uzun ölçümsüz dizi kurmadı"
        );
        assert!(
            self.w_gap <= gap_wait_bound,
            "tohum {seed}: ölçüm boşluğunda kuyruk sınırsız: {} (sınır {})",
            secs(self.w_gap),
            secs(gap_wait_bound)
        );
        assert!(
            self.conv <= self.conv_bound,
            "tohum {seed}: yakınsama {} (sınır {})",
            secs(self.conv),
            secs(self.conv_bound)
        );
        assert!(
            self.w_after <= wait_bound,
            "tohum {seed}: yakınsama sonrası bekleme {}",
            secs(self.w_after)
        );
    }

    /// Zamanında olmayan demetlerin pencerelere göre yeri: düşüş → `t_gap` (koruma yok) →
    /// `gap_from` (kuyruk boşalıyor) → `t_back` (boşluk korumalı) → yakınsama → konuşma sonu.
    fn windows_line(&self, sc: &Scenario, o: &Outcome) -> String {
        let cuts = [
            ("düşüş→t_gap", DROP_AT_NS, self.t_gap),
            ("t_gap→boşaltma", self.t_gap, self.gap_from),
            ("korumalı", self.gap_from, self.t_back),
            ("dönüş→yakınsama", self.t_back, self.conv),
            ("yakınsamadan sonra", self.conv, sc.voice_until_ns),
        ];
        cuts.iter()
            .map(|&(name, from, to)| {
                let mut planned = 0;
                let mut on_time = 0;
                let mut m = Misses::default();
                for (v, &(plan, _)) in o.voice.iter().enumerate() {
                    if plan >= from && plan < to {
                        planned += 1;
                        if o.voice_first[v].is_some_and(|a| a <= plan + PTT_BOUND_NS) {
                            on_time += 1;
                        } else {
                            miss_class(o, v as u64, &mut m);
                        }
                    }
                }
                format!(
                    "{name}[{}–{}] {on_time}/{planned} (rastgele {}, kuyruk {}, geç {}, bırakma {})",
                    secs(from),
                    secs(to),
                    m.random,
                    m.queue,
                    m.late,
                    m.abandoned
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// [PTT-YALIN · KAPI] (e2) yalın yükte ([`lean`]: ürünün yalın PTT sözleşmesi; sahip kararı
/// 2026-11-22, #175 7080178069): hat korumasının üç iddiası (e2) ile AYNI sınırlarla
/// ([`E2Guard::assert_bounds`]; türetim e2 belgesinde). "e2 yalın 9/9" = bu üç iddia × tohum 1–3;
/// ölçümsüz dizi ≥ 3 düzenek iddiasıdır, sayılmaz. PTT ölçütü ayrı testte
/// ([`e2_lean_ptt_on_time_per_phase_with_miss_breakdown`]): biri kırılınca öbürünün sinyali
/// kaybolmasın.
///
/// Kuyruk borcunun kanıtı (karşı-olgu: `line_guard` + `sender` c07b1305 öncesi, tek seferlik
/// boşaltma hat modeline): bu kolun boşluk beklemesi tohum 1'de 3,84 s (sınır 0,70 s; test ilk
/// tohumda durur; gece ölçümü 3,84 / 4,69 / 4,33 s, #175 7080391047). Dürüst hatta bedeli (`e1`)
/// dosya 15,10 / 11,86 / 10,27 → 16,31 / 14,38 / 10,27 s (sınır yok).
#[test]
fn e2_lean_a_long_unmeasured_gap_on_a_real_drop_keeps_the_queue_bounded() {
    for seed in 1..=3u64 {
        let sc = lean(e2_scenario(seed));
        let o = run(&sc);
        report("e2-yalın/aday", &sc, &o);
        let g = E2Guard::of(&o);
        g.print(&format!("E2-YALIN tohum {seed}"), &o);
        g.assert_bounds(seed);
    }
}

/// [PTT-YALIN · KAPI · PTT-ÖLÇÜT] (e2) yalın yükte PTT, sahibin ölçütüyle: her evrede (başlangıç
/// 0–8 s, daralma 8–40 s; [`ptt_phases`]) zamanında oranı ≥ %75 ([`PhaseOnTime::passes`]). Eşik
/// kararı sahipte (koordinatör #175 7080413643: (a) ulaşım kaynaklı ıska 0, (b) %65); karar gelene
/// dek ≥ %75 iddia edilir ve her evrenin ıskaları dört sınıfa ([`Misses`]) ve e2'nin pencerelerine
/// ([`E2Guard::windows_line`]) ayrılarak yazılır.
///
/// Ölçüm (taban 1482c197; nedensel kayıt #175 7080561058): KIRMIZI. Daralma evresi 57/160,
/// 77/160, 79/160; geç varışların hepsi boşluk korumasından önce (düşüş → kuyruk boşalana dek;
/// ölçüm yokken boşluk kuralı ancak ikinci ölçümsüz ACK'te kurulur, o anda FIFO'da 5–6 s kuyruk)
/// ve tohum 1'de ölçüm dönüşü → yakınsama arasında; boşluk korumalı pencerede geç varış 0. Bu
/// düzenekte daralma evresi ≥ %75 ulaşımla sağlanamaz ((b)'nin daralma anı alt sınırının ölçümsüz
/// hâli). Başlangıç evresi tohum 2'de 28/38, ıskaların hepsi rastgele kayıp. Gizlenmez.
/// Karşı-olgu (kuyruk borcu yok, yukarıdaki gibi): daralma 19/160, 17/160, 27/160 (geç 72 / 100 /
/// 91, bırakma 30 / 8 / 6).
#[test]
fn e2_lean_ptt_on_time_per_phase_with_miss_breakdown() {
    let mut rows = Vec::new();
    for seed in 1..=3u64 {
        let sc = lean(e2_scenario(seed));
        let o = run(&sc);
        let m = lean_ptt(&o, sc.voice_until_ns);
        let g = E2Guard::of(&o);
        let ph = ptt_phases(&sc, &o);
        let row = format!(
            "tohum {seed}: {} | ulaşım kaynaklı ıska {} | pencereler: {} | PTT tel {} bit/s",
            phases_line(&ph),
            ph.iter().map(|p| p.misses.transport()).sum::<usize>(),
            g.windows_line(&sc, &o),
            m.bps
        );
        println!("E2-YALIN-PTT {row}");
        rows.push((ph.iter().all(PhaseOnTime::passes), row));
    }
    assert!(
        rows.iter().all(|(ok, _)| *ok),
        "PTT evre başına zamanında < %75 (sahip ölçütü; eşik kararı açık):\n{}",
        rows.iter()
            .map(|(_, r)| r.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** Konusu kopyalı PTT
/// kuralıdır (ürün yalnız eski uçla); yalın sözleşmede karşılığı yoktur (tek iletim, son tarihte
/// bırakma: [`lean_ptt_one_transmission_stays_within_budget_and_expired_units_are_not_retransmitted`]).
/// İddiaları değişmedi.
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** Konusu kopyalı PTT
/// kuralıdır (ürün yalnız eski uçla); yalın sözleşmede karşılığı yoktur (tek iletim, son tarihte
/// bırakma: [`lean_ptt_one_transmission_stays_within_budget_and_expired_units_are_not_retransmitted`]).
/// İddiaları değişmedi.
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** Konusu kopyalı PTT
/// kuralıdır (ürün yalnız eski uçla); yalın sözleşmede karşılığı yoktur (tek iletim, son tarihte
/// bırakma: [`lean_ptt_one_transmission_stays_within_budget_and_expired_units_are_not_retransmitted`]).
/// İddiaları değişmedi.
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** Kopyalı sözleşmenin kanıt
/// testi (fiziksel alt sınır; bilinen kırmızı). Yalın sözleşmede bütün iletimleri düşen demet
/// atlanır ve evre ölçütünde `rastgele` ıska sayılır ([`Misses`]). İddiası değişmedi.
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** Konusu kopyalı PTT
/// kuralıdır (ürün yalnız eski uçla); yalın sözleşmede karşılığı yoktur (tek iletim, son tarihte
/// bırakma: [`lean_ptt_one_transmission_stays_within_budget_and_expired_units_are_not_retransmitted`]).
/// İddiaları değişmedi.
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
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`PRODUCT_COPIES`]; ürün bunu yalnız eski uçla kullanır); iddiaları değişmedi. Kapı kolu:
/// [`lean_drop_and_recovery_are_followed_within_two_and_three_acks`].
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

/// [PTT-ÖLÇÜT · sahip kararı 2026-11-22, #175 7080178069] Kapı yalın sözleşmedir; PTT ölçütü
/// media'daki gibidir: demet sunumundan sonra ≤ 1 s ([`PTT_BOUND_NS`]) içinde alıcıya İLK kez
/// varırsa zamanında; geç kalan ya da hiç gelmeyen demet atlanır (kayıp decoder'da örtülür). Kapı:
/// evre başına zamanında oranı ≥ %75. Bu düzenek ilk varışı ölçer (`Outcome::voice_first`); ürün
/// alıcısının sıra-başı atlama beklemesi (client-core) burada yoktur, ürün testi onu ayrıca ölçer.
const PTT_ON_TIME_MIN_PERMILLE: u64 = 750;
/// Ürün testinin evre sınırı (`asym_product_outcome`): `başlangıç` = 0–8 s, sonrası hız
/// çizelgesinin değişim anlarıyla `sürekli` / `daralma` / `toparlanma`.
const START_PHASE_END_NS: u64 = 8 * SEC;

/// Zamanında olmayan demetlerin nedeni (koordinatör #175 7080413643: sahibin eşik kararına dek
/// her evrede ayrıştırılarak raporlanır). Sınıf demetin İLK iletiminin akıbetinden:
/// * `random`: ilk iletim hatta serileşti, rastgele kayıpta düştü (LCG; ulaşımın elinde değil);
/// * `queue`: ilk iletim darboğaz FIFO'sunda düştü;
/// * `late`: ilk iletim hatta düşmedi ama demet plan + 1 s'den sonra vardı (kuyruk beklemesi ya da
///   geç gönderim);
/// * `abandoned`: demet son tarihte hiç iletilmeden bırakıldı.
///
/// Ulaşım kaynaklı = `queue` + `late` + `abandoned`.
#[derive(Debug, Clone, Copy, Default)]
struct Misses {
    random: usize,
    queue: usize,
    late: usize,
    abandoned: usize,
}

impl Misses {
    fn transport(&self) -> usize {
        self.queue + self.late + self.abandoned
    }
}

/// Bir evrenin PTT ölçüsü (demet, planlandığı ana göre evreye düşer).
#[derive(Debug, Clone)]
struct PhaseOnTime {
    name: &'static str,
    from: u64,
    to: u64,
    planned: usize,
    on_time: usize,
    misses: Misses,
}

impl PhaseOnTime {
    /// Evrede planlanmış demet yoksa kanıt yoktur: geçmiş sayılmaz.
    fn passes(&self) -> bool {
        self.planned > 0
            && self.on_time as u64 * 1000 >= PTT_ON_TIME_MIN_PERMILLE * self.planned as u64
    }
}

/// `v`. demetin (birim kimliği = demet sırası) zamanında olmayış nedeni.
fn miss_class(o: &Outcome, v: u64, m: &mut Misses) {
    let first = o
        .ptt_tx
        .iter()
        .filter(|(p, _)| p.unit == v)
        .min_by_key(|(p, _)| (p.at, p.seq));
    match first {
        None => m.abandoned += 1,
        Some((p, _)) if p.done.is_none() => m.queue += 1,
        Some((p, _)) if !p.delivered => m.random += 1,
        Some(_) => m.late += 1,
    }
}

/// Evreler: `başlangıç` [0, 8 s); 8 s'den sonra hız çizelgesinin her değişim anında yeni evre —
/// çizelgenin ilk hızındaysa `sürekli`, önceki hızdan düşükse `daralma`, yüksekse `toparlanma`.
/// Son evre konuşmanın sonunda (`voice_until_ns`) biter.
fn ptt_phases(sc: &Scenario, o: &Outcome) -> Vec<PhaseOnTime> {
    let end = sc.voice_until_ns.max(START_PHASE_END_NS);
    let mut bounds = vec![(0u64, "başlangıç")];
    let name_at = |i: usize| -> &'static str {
        if i == 0 {
            "sürekli"
        } else if sc.fwd_plan[i].1 < sc.fwd_plan[i - 1].1 {
            "daralma"
        } else {
            "toparlanma"
        }
    };
    let active = |t: u64| {
        sc.fwd_plan
            .iter()
            .rposition(|&(at, _)| at <= t)
            .unwrap_or(0)
    };
    bounds.push((START_PHASE_END_NS, name_at(active(START_PHASE_END_NS))));
    for (i, &(at, _)) in sc.fwd_plan.iter().enumerate() {
        if at > START_PHASE_END_NS && at < end {
            bounds.push((at, name_at(i)));
        }
    }
    bounds
        .iter()
        .enumerate()
        .map(|(k, &(from, name))| {
            let to = bounds.get(k + 1).map_or(end, |b| b.0);
            let mut planned = 0;
            let mut on_time = 0;
            let mut misses = Misses::default();
            for (v, &(plan, _)) in o.voice.iter().enumerate() {
                if plan >= from && plan < to {
                    planned += 1;
                    if o.voice_first
                        .get(v)
                        .copied()
                        .flatten()
                        .is_some_and(|a| a <= plan + PTT_BOUND_NS)
                    {
                        on_time += 1;
                    } else {
                        miss_class(o, v as u64, &mut misses);
                    }
                }
            }
            PhaseOnTime {
                name,
                from,
                to,
                planned,
                on_time,
                misses,
            }
        })
        .filter(|p| p.from < p.to)
        .collect()
}

fn phases_line(ph: &[PhaseOnTime]) -> String {
    ph.iter()
        .map(|p| {
            format!(
                "{}[{}–{}] {}/{} (ıska: rastgele {}, kuyruk {}, geç {}, bırakma {})",
                p.name,
                secs(p.from),
                secs(p.to),
                p.on_time,
                p.planned,
                p.misses.random,
                p.misses.queue,
                p.misses.late,
                p.misses.abandoned
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ürünün yalın PTT sözleşmesinin aynası (#175 7080178069; `feat/ptt-lean`): tek iletim
/// (`copies = 0` + öncelik, `raw_copy_rule`), yalın 0x09 çerçevesi ([`LEAN_PTT_UNIT`] gövde),
/// sunumdan [`PTT_DEADLINE_NS`] sonra teyitsiz birimin bırakılması. Senaryonun geri kalanı aynen.
///
/// [BİRLEŞİM · aday b163bd90 + f4cd325c] Kopya kuralı t = 0'dan `copies = 0` + öncelik (ürün yalın
/// PTT'yi CAPS ile bildirecekse ilk 0x16'dan önce de kopya yok). Bırakma ilk 0x16'ya dek yapılmaz
/// (benzetimde vekili ilk ölçülü ACK: `ptt_deadline_from_first_measured_ack`); o ACK'te son tarihi
/// geçmiş teyitsiz demetler bırakılır. Aynalanmayan kalıntı: üründe 0x16'dan önceki demet veri
/// çerçevesiyle gider (gövde [`VOICE_UNIT`]); burada gövde baştan [`LEAN_PTT_UNIT`].
fn lean(sc: Scenario) -> Scenario {
    Scenario {
        copies: 0,
        spacing_ns: 0,
        copy_scale_min: None,
        raw_copy_rule: true,
        voice_unit: LEAN_PTT_UNIT,
        ptt_deadline_ns: Some(PTT_DEADLINE_NS),
        ptt_deadline_from_first_measured_ack: true,
        ..sc
    }
}

/// [PTT-YALIN] Ürün profili (dürüst 12 500, %20 iid kayıp, ACK bütçesi 62 B/s, 300 ms tek yön), dosya
/// yok; PTT 0,5–15,5 s, 200 ms'de bir 78 B gövde (tel 106 B), kural `copies = 0` + öncelik
/// (`ptt_forward_copy_rule(0, 0)`, ölçek yok); 5 s'de bir öncelikli metin birimi. Bırakma kolu:
/// sunumundan 1 s sonra teyitsiz PTT birimi `abandon_units` ile bırakılır. Tohum başına:
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
        let sc = lean(Scenario {
            file_units: 0,
            voice_until_ns: voice_until,
            text_at_ns: Some(5 * SEC),
            text_period_ns: 5 * SEC,
            ..Scenario::honest(seed)
        });
        let o = run(&sc);
        let m = lean_ptt(&o, voice_until);
        // [PTT-ÖLÇÜT] Evre başına zamanında oranı: bu testte yalnız rapor (iddiası yok).
        let ph = ptt_phases(&sc, &o);
        println!(
            "PTT-ÖLÇÜT tohum {seed} (rapor): {} → evrelerin hepsi ≥ %75: {}, ulaşım kaynaklı ıska {}",
            phases_line(&ph),
            ph.iter().all(PhaseOnTime::passes),
            ph.iter().map(|p| p.misses.transport()).sum::<usize>()
        );
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
// [Y4 · YALIN KOLLAR · sahip kararı 2026-11-22, #175 7080178069] PTT'yi YÜK olarak kullanan
// testlerin yalın PTT yüküyle aynası. Kapı yalın sözleşmedir; kopyalı kollar yerinde GERİLİM / eski
// uç kolu olarak kalır (eski uçla konuşma: 0x16 görülene dek kopyalı PTT).
// * Senaryo var olan testinkiyle aynı; yalnız PTT yükü [`lean`] (ürünün yalın sözleşmesi: tek
//   iletim + öncelik, 78 B gövde, sunumdan 1 s sonra `abandon_units` ile bırakma).
// * Hat koruması iddiaları ve karşı-olgular var olan testteki sınırlarla, değiştirilmeden.
// * PTT iddiası (her yalın kolda, kopyalı kol PTT iddia etse de etmese de) sahibin ölçütü: her
//   evrede ([`ptt_phases`]) zamanında oranı ≥ %75 ([`PhaseOnTime::passes`]). Kırmızıda ileti her
//   kalan evrenin ıskalarını dört sınıfa ayırır ([`Misses`]) ve ulaşım kaynaklı ıska sayısını
//   yazar; hat ve PTT ihlalleri ayrı listelerde ([`assert_lean`]).
// * Tek iletimde zamanında tavanı 1 − p = %80'dir: 38 demetlik evre kusursuz ulaşımda bile kesin
//   binomla yalnız ≈ 0,78 olasılıkla ≥ %75 olur. Yalnız `rastgele` ıskalı kırmızı kapı eşiğinin
//   istatistiğidir (sahip kararı bekliyor, #175 7080413643); ulaşım kaynaklı ıska gerçek kusurdur.
// * GERİLİM / eski uç kolları (PTT yükü kopyalı, [`PRODUCT_COPIES`]; iddiaları değişmedi): a, c,
//   d1, d3, e1, d4 ve düşüş-toparlanma testleri; kapı kolları bu bölümde `lean_` önekiyle. c, d3 ve
//   d4'ün belgesine not eklenmedi: o belgeler (b), (d-2) ve (e-2) testlerinin hemen ardında, Y1/Y2
//   bölgesi. (b), (d-2), (e-2) ve onların yalın kolları Y1/Y2'nindir.
// ---------------------------------------------------------------------------------------------

/// Yalın kolun PTT ölçütü: evre başına ≥ %75; kalan evreler ıska ayrıştırmasıyla döner. PTT tel
/// hızı raporlanır (yalın birim boyu [`lean_ptt`] içinde sınanır).
fn lean_ptt_violations(tag: &str, sc: &Scenario, o: &Outcome) -> Vec<String> {
    let ph = ptt_phases(sc, o);
    let m = lean_ptt(o, sc.voice_until_ns);
    println!(
        "YALIN {tag} tohum {}: {} | ulaşım kaynaklı ıska {} | PTT tel {} bit/s",
        sc.seed,
        phases_line(&ph),
        ph.iter().map(|p| p.misses.transport()).sum::<usize>(),
        m.bps
    );
    ph.iter()
        .filter(|p| !p.passes())
        .map(|p| {
            format!(
                "{tag} tohum {}: PTT {}[{}–{}] {}/{} < %75 (ıska: rastgele {}, kuyruk {}, geç {}, \
                 bırakma {}; ulaşım kaynaklı {})",
                sc.seed,
                p.name,
                secs(p.from),
                secs(p.to),
                p.on_time,
                p.planned,
                p.misses.random,
                p.misses.queue,
                p.misses.late,
                p.misses.abandoned,
                p.misses.transport()
            )
        })
        .collect()
}

/// [Y4] Raporlama: planı `[from, to)` içindeki demetlerden zamanında olanlar ve kalanların ıska
/// sınıfları ([`miss_class`]); evre sınırından bağımsız bir pencere (ör. yakınsamadan sonrası)
/// için. Ölçüt değildir.
fn window_misses(o: &Outcome, from: u64, to: u64) -> (usize, usize, Misses) {
    let (mut planned, mut on_time, mut m) = (0, 0, Misses::default());
    for (v, &(plan, _)) in o.voice.iter().enumerate() {
        if plan < from || plan >= to {
            continue;
        }
        planned += 1;
        if o.voice_first
            .get(v)
            .copied()
            .flatten()
            .is_some_and(|a| a <= plan + PTT_BOUND_NS)
        {
            on_time += 1;
        } else {
            miss_class(o, v as u64, &mut m);
        }
    }
    (planned, on_time, m)
}

/// Hat iddiaları ve PTT ölçütü ayrı listelerle: kırmızının hangisinden geldiği iletide görünür.
fn assert_lean(line: &[String], ptt: &[String]) {
    assert!(
        line.is_empty() && ptt.is_empty(),
        "hat iddiaları ({}): {line:#?}\nPTT ölçütü ({}): {ptt:#?}",
        line.len(),
        ptt.len()
    );
}

/// [Y4] [`a_return_delay_rise_does_not_cut_an_honest_line`]'ın yalın kolu: aynı senaryo, aynı hat
/// iddiası (hat beyanın %5'inden fazla düşürülmez) ve aynı karşı-olgu (eski tahminci en az bir
/// tohumda keser); PTT her evrede ≥ %75.
#[test]
fn lean_a_return_delay_rise_does_not_cut_an_honest_line() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    let mut control_cut = false;
    for seed in 1..=3u64 {
        let sc = lean(Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        });
        let o = run(&sc);
        report("a/yalın", &sc, &o);
        if !never_cut(&o) {
            line.push(format!(
                "tohum {seed}: dürüst hat düşürüldü: {:?}",
                o.rate_trace
            ));
        }
        ptt.extend(lean_ptt_violations("a", &sc, &o));
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("a/yalın/karşı-olgu", &control, &oc);
        control_cut |= !never_cut(&oc);
    }
    if !control_cut {
        line.push("karşı-olgu: eski tahminci bu senaryoda kesmedi".to_string());
    }
    assert_lean(&line, &ptt);
}

/// [Y4] [`c_a_low_offered_rate_is_not_taken_as_capacity`]'ın yalın kolu: aynı senaryo, hat iddiası
/// ve karşı-olgu (eski tahminci üç tohumda da keser); PTT her evrede ≥ %75.
#[test]
fn lean_c_a_low_offered_rate_is_not_taken_as_capacity() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let sc = lean(Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            file_units: 30,
            file_gap_ns: 400 * MS,
            voice_until_ns: 22 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        });
        let o = run(&sc);
        report("c/yalın", &sc, &o);
        if !never_cut(&o) {
            line.push(format!("tohum {seed}: {:?}", o.rate_trace));
        }
        ptt.extend(lean_ptt_violations("c", &sc, &o));
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("c/yalın/karşı-olgu", &control, &oc);
        if never_cut(&oc) {
            line.push(format!("karşı-olgu tohum {seed}: eski tahminci kesmedi"));
        }
    }
    assert_lean(&line, &ptt);
}

/// [Y4] [`d1_random_loss_is_not_queue_growth`]'ın yalın kolu: ölçülen aralık ≥ 5, rastgele kayıp
/// örnek vermez, hat düşürülmez; PTT her evrede ≥ %75.
#[test]
fn lean_d1_random_loss_is_not_queue_growth() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let sc = lean(Scenario {
            file_units: 120,
            voice_until_ns: 15 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        });
        let o = run(&sc);
        report("d1/yalın", &sc, &o);
        let (checks, samples, _) = o.interval.expect("kural kurulu");
        if checks < 5 {
            line.push(format!("tohum {seed}: ölçülen aralık {checks}"));
        }
        if samples != 0 {
            line.push(format!(
                "tohum {seed}: rastgele kayıp örnek verdi ({samples})"
            ));
        }
        if !never_cut(&o) {
            line.push(format!("tohum {seed}: {:?}", o.rate_trace));
        }
        ptt.extend(lean_ptt_violations("d1", &sc, &o));
    }
    assert_lean(&line, &ptt);
}

/// [Y4] [`d3_reordering_keeps_the_measurement_on_the_safe_side`]'ın yalın kolu: dürüst hat
/// düşürülmez, karşı-olgu üç tohumda keser, gerçek 6 250'de son tahmin ≤ %110 × 6 250; PTT her
/// evrede ≥ %75 (dürüst ve 6 250 koşusu ayrı ayrı).
#[test]
fn lean_d3_reordering_keeps_the_measurement_on_the_safe_side() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let sc = lean(Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            reorder_permille: 30,
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            ..Scenario::honest(seed)
        });
        let o = run(&sc);
        report("d3/yalın", &sc, &o);
        if !never_cut(&o) {
            line.push(format!("tohum {seed}: {:?}", o.rate_trace));
        }
        ptt.extend(lean_ptt_violations("d3", &sc, &o));
        let control = Scenario {
            interval: false,
            ..sc.clone()
        };
        let oc = run(&control);
        report("d3/yalın/karşı-olgu", &control, &oc);
        if never_cut(&oc) {
            line.push(format!("karşı-olgu tohum {seed}: eski tahminci kesmedi"));
        }
        let slow = Scenario {
            fwd_plan: vec![(0, DROP_TO)],
            file_units: 80,
            voice_until_ns: 20 * SEC,
            rev_delay_plan: vec![(0, ONE_WAY_NS)],
            ..sc
        };
        let os = run(&slow);
        report("d3/yalın/6250", &slow, &os);
        match os.rate_at_end() {
            Some(end) if end * 10 <= DROP_TO * 11 => {}
            other => line.push(format!("tohum {seed}: 6 250'de son tahmin {other:?}")),
        }
        ptt.extend(lean_ptt_violations("d3/6250", &slow, &os));
    }
    assert_lean(&line, &ptt);
}

/// [Y4] [`e1_unmeasured_acks_do_not_bring_back_the_return_delay_cut`]'ın yalın kolu: düzenek ≥ 2
/// ölçümsüz ACK dizisi kurar, hat ve gönderim tavanı beyanın %95'inin altına inmez, karşı-olgu en
/// az bir tohumda keser; PTT her evrede ≥ %75.
#[test]
fn lean_e1_unmeasured_acks_do_not_bring_back_the_return_delay_cut() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    let mut control_cut = false;
    for seed in 1..=3u64 {
        let sc = lean(Scenario {
            rev_delay_plan: REV_DELAY_PLAN.to_vec(),
            voice_until_ns: 25 * SEC,
            ack_return_bw: 4 * PROFILE_DOWN,
            no_interval: Some((5 * SEC, 13 * SEC)),
            ..Scenario::honest(seed)
        });
        let o = run(&sc);
        report("e1/yalın", &sc, &o);
        let (run_len, _, _) = o.longest_unmeasured_run();
        if run_len < 2 {
            line.push(format!("tohum {seed}: düzenek ölçümsüz dizi kurmadı"));
        }
        if !never_cut(&o) {
            line.push(format!(
                "tohum {seed}: dürüst hat düşürüldü: {:?}",
                o.rate_trace
            ));
        }
        if o.min_pace() * 20 < PROFILE_UP * 19 {
            line.push(format!(
                "tohum {seed}: gönderim tavanı düşürüldü: {:?}",
                o.pace_trace
            ));
        }
        ptt.extend(lean_ptt_violations("e1", &sc, &o));
        let control = Scenario {
            interval: false,
            ..sc
        };
        let oc = run(&control);
        report("e1/yalın/karşı-olgu", &control, &oc);
        control_cut |= !never_cut(&oc);
    }
    if !control_cut {
        line.push("karşı-olgu: cephe tahmincisi bu senaryoda kesmedi".to_string());
    }
    assert_lean(&line, &ptt);
}

/// [Y4] (d-4)'ün yalın kolu. Hat iddiaları [`d4_a_full_small_fifo_is_detected_and_stops_overflowing`]
/// ile aynı sınırlarla: yakınsama ≤ düşüş + 3·T_ack + T_ret; yakınsamadan 2 s sonra taşma yok ve
/// bekleme ≤ iki tam datagramın 6 250'de serileşmesi. Kopyalı kolun "yakınsamadan 9 s sonra sıralı
/// PTT ≤ 1 s" iddiası yerine sahibin ölçütü: her evrede (başlangıç [0, 8 s), daralma [8 s, 40 s))
/// zamanında oranı ≥ %75. Daralma evresi düşüş geçişini de kapsar; (b)'nin belgelediği fiziksel alt
/// sınır (algılamaya dek FIFO beklemesi) o geçişteki demetleri geç kılar ve ıska ayrıştırmasında
/// `kuyruk` / `geç` olarak görünür.
#[test]
fn lean_d4_a_full_small_fifo_is_detected_and_stops_overflowing() {
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let sc = lean(Scenario {
            queue_pkts: 8,
            ..drop_scenario(seed, true)
        });
        let o = run(&sc);
        report("d4/yalın", &sc, &o);
        let t_ack = o
            .acks_at
            .windows(2)
            .filter(|w| w[1] <= DROP_AT_NS)
            .map(|w| w[1] - w[0])
            .max()
            .expect("düşüşten önce ACK'ler");
        let conv_bound = DROP_AT_NS + 3 * t_ack + t_ret;
        let Some(conv) = converged_at(&o, DROP_TO) else {
            line.push(format!("tohum {seed}: yakınsamadı"));
            continue;
        };
        if conv > conv_bound {
            line.push(format!(
                "tohum {seed}: yakınsama {} (sınır {})",
                secs(conv),
                secs(conv_bound)
            ));
        }
        let late_drops = o.drops_between(conv + 2 * SEC, u64::MAX);
        if late_drops != 0 {
            line.push(format!(
                "tohum {seed}: yakınsamadan sonra taşma sürüyor ({late_drops})"
            ));
        }
        let w = o.max_wait_between(conv + 2 * SEC, u64::MAX);
        if w > wait_bound {
            line.push(format!(
                "tohum {seed}: yakınsama sonrası bekleme {}",
                secs(w)
            ));
        }
        let (n0, t0, m0) = window_misses(&o, DROP_AT_NS, conv + 2 * SEC);
        let (n1, t1, m1) = window_misses(&o, conv + 2 * SEC, u64::MAX);
        println!(
            "D4/yalın tohum {seed}: yakınsama {} (sınır {}), kuyrukta düşen {} | PTT düşüş → \
             yakınsama + 2 s: {t0}/{n0} {m0:?} (ulaşım {}) | sonrası: {t1}/{n1} {m1:?} (ulaşım {})",
            secs(conv),
            secs(conv_bound),
            o.drops_queue,
            m0.transport(),
            m1.transport()
        );
        ptt.extend(lean_ptt_violations("d4", &sc, &o));
    }
    assert_lean(&line, &ptt);
}

/// Düşüş-toparlanma iddialarının ihlalleri ([`drop_and_recovery_are_followed_within_two_and_three_acks`]
/// ile aynı tanım ve sınırlar).
fn drop_recovery_violations(tag: &str, sc: &Scenario, o: &Outcome) -> Vec<String> {
    let ser2 = 2 * (MTU + OVH) * SEC / PROFILE_UP;
    let seed = sc.seed;
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
    let up_k = (0..up.len()).find(|&k| up[k..].iter().all(|a| a.3 * 100 >= PROFILE_UP * 95));
    let up_at = up_k.map(|k| up[k].0);
    let w_up = up_at.map(|t| o.max_wait_between(t, u64::MAX));
    println!(
        "DÜŞÜŞ-TOPARLANMA {tag} tohum {seed}: düşüş ACK#{:?} toparlanma ACK#{:?} ({}) sonrası \
         bekleme {} (sınır {})",
        down_k.map(|k| k + 1),
        up_k.map(|k| k + 1),
        up_at.map_or("YOK".to_string(), secs),
        w_up.map_or("-".to_string(), secs),
        secs(ser2)
    );
    let mut bad = Vec::new();
    if down_k.is_none_or(|k| k >= 2) {
        bad.push(format!(
            "{tag} tohum {seed}: düşüş ACK#{:?}",
            down_k.map(|k| k + 1)
        ));
    }
    if up_k.is_none_or(|k| k >= 3) {
        bad.push(format!(
            "{tag} tohum {seed}: toparlanma ACK#{:?}",
            up_k.map(|k| k + 1)
        ));
    }
    if w_up.is_some_and(|w| w > ser2) {
        bad.push(format!(
            "{tag} tohum {seed}: yükseltmeden sonra bekleme {}",
            secs(w_up.unwrap_or(0))
        ));
    }
    bad
}

/// [Y4] Düşüş-toparlanmanın yalın kolu: [`drop_recovery`] senaryosu (dosya sürekli, tohum 1–3),
/// PTT yükü [`lean`]. Hat iddiaları kopyalı/tek/PTT'siz kollarla aynı ([`drop_recovery_violations`]:
/// düşüş en çok ikinci ACK'te, toparlanma en çok üçüncü ACK'ten itibaren kalıcı, yükseltmeden sonra
/// bekleme ≤ iki tam datagramın 12 500'de serileşmesi). Var olan test PTT iddiası yapmıyordu; yalın
/// kolda sahibin ölçütü her evrede (başlangıç, daralma [8, 20 s), toparlanma [20, 40 s)) ≥ %75.
#[test]
fn lean_drop_and_recovery_are_followed_within_two_and_three_acks() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let sc = lean(drop_recovery(seed, PttLoad::Copies));
        let o = run(&sc);
        report("düşüş-toparlanma/yalın", &sc, &o);
        line.extend(drop_recovery_violations("yalın", &sc, &o));
        if let Some(conv) = converged_at(&o, DROP_TO) {
            let (n0, t0, m0) = window_misses(&o, DROP_AT_NS, conv + 2 * SEC);
            let (n1, t1, m1) = window_misses(&o, conv + 2 * SEC, RECOVER_AT_NS);
            println!(
                "DÜŞÜŞ-TOPARLANMA/yalın tohum {seed}: yakınsama {} | PTT düşüş → yakınsama + 2 s: \
                 {t0}/{n0} {m0:?} (ulaşım {}) | yakınsama + 2 s → toparlanma: {t1}/{n1} {m1:?} \
                 (ulaşım {})",
                secs(conv),
                m0.transport(),
                m1.transport()
            );
        }
        ptt.extend(lean_ptt_violations("düşüş-toparlanma", &sc, &o));
    }
    assert_lean(&line, &ptt);
}
