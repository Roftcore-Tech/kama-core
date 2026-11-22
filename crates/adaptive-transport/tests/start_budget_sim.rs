//! [İŞ-33 C1 · C4 · C5] Başlangıç bütçesi ve ilk ACK'in varış penceresi: geri bildirimi aç profil
//! gerçek ileri hattan iyimserken ilk ACK'e dek darboğaz kuyruğu kuyruk payını aşmaz, ilk ölçüm
//! ilk ACK'te gelir; profil dürüstken ilk ACK hattı doğrular ve koruma devreye girmez.
//!
//! Düzenek `tests/line_guard_sim.rs` ile aynıdır (ayrık olay, 1 ms adım, deterministik; gerçek
//! `Sender` + gerçek `Receiver`; ürünün geri bildirimi aç profil kurulumu 12 500 / 62 B/s; ileri
//! hat 64 paket FIFO, 300 ms, serileşmeden sonra %20 tohumlu kayıp; dönüş 62 B/s, 300 ms). Farklar
//! ürün çalışma zamanının yeni iki girişidir:
//! * alıcıya her datagramdan önce UDP gövde boyu verilir (`Receiver::set_next_datagram_bytes`;
//!   ürün `recv_from` boyu) → ilk ACK ilk varış penceresinin hızını taşır;
//! * göndericiye her ACK'ten önce çerçevenin `arrival_rate` alanı verilir
//!   (`Sender::note_ack_arrival_rate`; ürün ACK çözme yolu).
//!
//! * göndericiye her ACK'ten önce çerçevenin çift alt sınırı ve ölçüm aralığı verilir
//!   (`Sender::note_ack_pair_rate`, `Sender::note_ack_forward_interval`; sözleşme #175 7067964700
//!   §2–3; ürün çalışma zamanının sırası). Düzenek alıcının yerine hesaplar, alıcının bugünkü
//!   kuralıyla (`RxMirror`; İŞ-35 gece hizası: aralık bir ACK geriden, `[span_first, largest]`;
//!   önceki düzenek iki ACK geriden kuruyordu ve `pair_rate` vermiyordu).
//!
//! `old_receiver` kolu boy vermez ve ölçüm aralığı taşımaz: eski alıcı gibi alan daima
//! sentinel'dir (uyum).
//!
//! [İŞ-35 AÇIK 4] İlk pencerenin bozulmaları (`Perturb`): paket boyu farkı, yeniden sıralama,
//! alıcı damgası gecikmesi, kuyrukta düşme. Kabul ölçütleri `perturbed_violations`'da.
//!
//! Ölçülen: darboğazda her datagramın kuyruk beklemesi, kuyrukta düşmeler, kısa birimlerin
//! (ürünün PTT demeti) SIRALI teslim gecikmesi (ürün PTT şeridi sıralıdır: demet `k` ancak
//! `0..=k` geldikten sonra çalınır), dosyanın bitişi, ilk pencerenin sonucu.

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::ack::{
    ACK_BITMAP_MAX_BITS, ACK_DELAY_UNIT_US, PAIR_MIN_UDP_BYTES, PAIR_RATE_NONE, RX_SPAN_NONE,
};
use adaptive_transport::arrival_rate::FIRST_WINDOW_BYTES;
use adaptive_transport::line_guard::{FirstWindowOutcome, START_BURST_NS};
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, Event, FeedbackStarvedRule, ForwardCopyLineScale, ForwardCopyRule, OpaqueUnit, Sender,
    Timer, TransmitDecision, TransmitKind,
};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const PROFILE_UP: u64 = 12_500;
const PROFILE_DOWN: u64 = 62;
const OVH: u64 = 28;
const ACK_SEAL: u64 = 22;
const ONE_WAY_NS: u64 = 300 * MS;
const QUEUE_PKTS: usize = 64;
const LOSS_PERMILLE: u64 = 200;
const FILE_UNITS: u64 = 41;
const FILE_UNIT: u64 = 1000;
const FILE_BASE_ID: u64 = 1_000_000;
const VOICE_UNIT: u64 = 86;
const VOICE_PERIOD_NS: u64 = 200 * MS;
const VOICE_START_NS: u64 = 500 * MS;
const T_MAX_NS: u64 = 90 * SEC;
/// Ürün testinin başlangıç evresi (`asym_product_outcome` `START_PHASE`).
const START_PHASE_NS: u64 = 8 * SEC;
/// Ürün testinin PTT sınırı (`PTT_BOUND`).
const PTT_BOUND_NS: u64 = SEC;

/// PTT kopya kuralı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Copies {
    /// Düzeneğin ilk kuralı (2026-11-16 ürün kuralı): 5 iletim, 40 ms aralık, hatla ölçekleme yok.
    Harness,
    /// Ürün kuralı (`client_core::runtime::PTT_FORWARD_COPIES` = 7, `PTT_FORWARD_COPIES_MIN` = 6;
    /// aralık 200 ms / 7; ölçülen yavaş hatta orantılı, en az 6 iletim). [Y4 · 2026-11-22] Ürün
    /// bu kuralı artık yalnız eski uçla (0x15 yazan eş) ve ilk 0x16'ya dek kullanır; bu kolla koşan
    /// testler GERİLİM / eski uç kollarıdır, kapı [`Copies::Lean`]'dir.
    Product,
    /// [Y4 · PTT-YALIN] Ürünün yalın PTT sözleşmesi (#175 7080178069; `starved_line_sim.rs`
    /// `lean()` aynası): tek iletim (`ptt_forward_copy_rule(0, 0)`: kopya yok, öncelik kalır; kural
    /// PTT şeridinin üstünü [`PTT_LANE_TOP`] ile ayırır), yalın 0x09 birimi ([`LEAN_PTT_UNIT`]
    /// gövde), sunumdan [`PTT_DEADLINE_NS`] sonra teyitsiz birimin `Sender::abandon_units` ile
    /// bırakılması. [BİRLEŞİM · aday f4cd325c] Bırakma ilk 0x16'ya dek yapılmaz (vekili yeni alıcının
    /// göndericiye ulaşan ilk ACK'i; üründe 0x16 ölçümden bağımsız); o ACK'ten sonra son tarihi
    /// geçmiş teyitsiz demetler bırakılır (`starved_line_sim.rs` `ptt_deadline_from_first_ack` ile
    /// aynı).
    Lean,
}

impl Copies {
    /// (ilk iletime ek kopya, aralık, ölçeklemenin alt sınırı).
    fn rule(self) -> (u8, u64, Option<u8>) {
        match self {
            Self::Harness => (4, 40 * MS, None),
            Self::Product => (6, 200 * MS / 7, Some(5)),
            Self::Lean => (0, 0, None),
        }
    }

    /// PTT biriminin UDP gövdesi.
    fn voice_unit(self) -> u64 {
        if self == Self::Lean {
            LEAN_PTT_UNIT
        } else {
            VOICE_UNIT
        }
    }
}

/// [Y4] Yalın kuralda PTT şeridinin üstü (ürün: `unit_id::tagged(1, 0)`; düzenekte PTT kimlikleri
/// 0.., dosya [`FILE_BASE_ID`]'den). Kopyalı kurallar bugünkü gibi `FILE_BASE_ID` kullanır.
const PTT_LANE_TOP: u64 = 500_000;
/// [Y4] Yalın PTT biriminin UDP gövdesi (`starved_line_sim.rs` `LEAN_PTT_UNIT`; tel 106 B).
const LEAN_PTT_UNIT: u64 = 78;
/// [Y4] Ürünün PTT son tarihi (`client_core::runtime::PTT_DEADLINE_NS`).
const PTT_DEADLINE_NS: u64 = SEC;
/// [Y4] Sahibin PTT ölçütü (#175 7080178069): evre başına zamanında oranı ≥ %75
/// (`starved_line_sim.rs` `PTT_ON_TIME_MIN_PERMILLE`).
const PTT_ON_TIME_MIN_PERMILLE: u64 = 750;

/// [m. 2 bozulması] İlk varış penceresinin girdisine uygulanan tek bir sapma. Hepsi yalnız ilk
/// pencereyi etkiler (sonraki ACK'ler alanı taşımaz); hat ve gönderici değişmez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Perturb {
    None,
    /// Alıcıya verilen gövde boyu göndericinin kaydından bu kadar bayt büyük (her datagram): iki
    /// ucun boy sayımı farklı.
    SizeSkew(u64),
    /// Pencereyi açan datagramdan sonraki iki varış yer değiştirir.
    Reorder,
    /// Pencereyi açan datagramın alıcı damgası bu kadar geç (ns; alım birleştirme ya da zamanlama
    /// gecikmesi): o süre içinde gelen varışlar da aynı anda işlenir.
    LateFirstStamp(u64),
    /// Pencereyi açacak datagramdan sonra hatta sunulan ilk datagram darboğaz kuyruğunda düşer
    /// (hattı kullanmaz; gönderici kaydında var).
    QueueDropAfterOpener,
}

/// [m. 2] Kuyruk kaybı planı: açıcıyı bekle → sonrakini düşür → bitti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropPlan {
    Off,
    WaitOpener,
    DropNext,
    Done,
}

struct Bottleneck {
    rate_bps: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, u64, u64, u64)>,
    lcg: u64,
    dropped_queue: Vec<u64>,
    waits: Vec<(u64, u64)>,
    drop_plan: DropPlan,
    /// [Y4] PTT iletimleri (`unit_id < FILE_BASE_ID`): (gönderim anı, birim, serileşme bitişi —
    /// kuyrukta düştüyse `None`, hatta düşmedi mi).
    ptt_log: Vec<(u64, u64, Option<u64>, bool)>,
}

impl Bottleneck {
    fn new(rate_bps: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            dropped_queue: Vec::new(),
            waits: Vec::new(),
            drop_plan: DropPlan::Off,
            ptt_log: Vec::new(),
        }
    }

    fn send(&mut self, now: u64, seq: u64, udp: u64, unit_id: u64) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= QUEUE_PKTS || self.drop_plan == DropPlan::DropNext {
            if self.drop_plan == DropPlan::DropNext {
                self.drop_plan = DropPlan::Done;
            }
            self.dropped_queue.push(now);
            if unit_id < FILE_BASE_ID {
                self.ptt_log.push((now, unit_id, None, false));
            }
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + (udp + OVH) * SEC / self.rate_bps;
        self.busy.push_back(done);
        self.waits.push((now, start - now));
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let lost = (self.lcg >> 33) % 1000 < LOSS_PERMILLE;
        if unit_id < FILE_BASE_ID {
            self.ptt_log.push((now, unit_id, Some(done), !lost));
        }
        if lost {
            return;
        }
        if self.drop_plan == DropPlan::WaitOpener && udp >= FIRST_WINDOW_BYTES {
            self.drop_plan = DropPlan::DropNext;
        }
        self.wire.push_back((done + ONE_WAY_NS, seq, unit_id, udp));
    }
}

/// Ölçüm aralığı (sözleşme #175 7067964700 §2) ve çift alt sınırı: (m_first, largest, rx_span, M,
/// pair_rate).
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

struct ReturnLine {
    free_at: u64,
    wire: VecDeque<(u64, u64, u16, AckInfo, IntervalMsg)>,
}

impl ReturnLine {
    fn send(&mut self, now: u64, udp: u64, arrival_rate: u16, ack: AckInfo, iv: IntervalMsg) {
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        self.wire
            .push_back((self.free_at + ONE_WAY_NS, udp, arrival_rate, ack, iv));
    }
}

/// [m. 2] Alıcı tarafı bozulmalar (varış sırası, alıcı damgası, alıcıya verilen boy).
struct RxPerturb {
    kind: Perturb,
    /// Alıcı pencereyi açan datagramı (gövde ≥ `FIRST_WINDOW_BYTES`) gördü.
    opened: bool,
    held: Vec<(u64, u64, u64)>,
    held_until: u64,
    done: bool,
}

impl RxPerturb {
    fn new(kind: Perturb) -> Self {
        Self {
            kind,
            opened: false,
            held: Vec::new(),
            held_until: 0,
            done: false,
        }
    }

    /// Bu adımda alıcıya verilecek varışlar (sırayla).
    fn apply(&mut self, arrived: Vec<(u64, u64, u64)>, now: u64) -> Vec<(u64, u64, u64)> {
        let mut out = Vec::new();
        match self.kind {
            Perturb::Reorder => {
                for a in arrived {
                    if !self.done && self.opened && self.held.is_empty() {
                        self.held.push(a);
                        self.held_until = now + 300 * MS;
                        continue;
                    }
                    out.push(a);
                    if !self.done && !self.held.is_empty() {
                        out.append(&mut self.held);
                        self.done = true;
                    }
                    self.opened |= a.2 >= FIRST_WINDOW_BYTES;
                }
            }
            Perturb::LateFirstStamp(d) => {
                for a in arrived {
                    if !self.done && !self.held.is_empty() {
                        self.held.push(a);
                    } else if !self.done && !self.opened && a.2 >= FIRST_WINDOW_BYTES {
                        self.opened = true;
                        self.held.push(a);
                        self.held_until = now + d;
                    } else {
                        out.push(a);
                    }
                }
            }
            Perturb::None | Perturb::SizeSkew(_) | Perturb::QueueDropAfterOpener => out = arrived,
        }
        if !self.done && !self.held.is_empty() && now >= self.held_until {
            out.append(&mut self.held);
            self.done = true;
        }
        out
    }

    fn size_skew(&self) -> u64 {
        match self.kind {
            Perturb::SizeSkew(b) => b,
            _ => 0,
        }
    }
}

#[derive(Debug)]
struct Outcome {
    file_done_ns: Option<u64>,
    /// Dosyanın son yeni biriminin ilk iletim anı (bütçenin dosyaya bedeli; ARQ'dan bağımsız).
    file_last_new_sent_ns: Option<u64>,
    first_ack_ns: Option<u64>,
    /// İlk ACK'in göndericiye varışından ÖNCE gönderilen datagramların en uzun kuyruk beklemesi.
    max_wait_before_first_ack: u64,
    max_wait: u64,
    drops: usize,
    /// Kısa birimlerin sıralı teslim gecikmesinin en büyüğü (ns) ve 1 s'yi aşan birim sayısı.
    voice_max: u64,
    /// Başlangıç evresinde (plan < [`START_PHASE_NS`]) planlanan demetlerin en büyük sıralı teslim
    /// gecikmesi (ns).
    voice_max_start: u64,
    voice_over_1s: usize,
    voice_undelivered: usize,
    /// 1 s'yi aşan ilk demetler: (k, plan, kendi varışı, çalınma) (gözlem).
    voice_late: Vec<(u64, u64, u64, u64)>,
    first_window: Option<FirstWindowOutcome>,
    first_window_detail: Option<adaptive_transport::line_guard::FirstWindowDetail>,
    guard: Option<(Option<u64>, u64)>,
    /// Başlangıç bütçesini kaldıran kanıt.
    start_exit: Option<adaptive_transport::line_guard::StartExit>,
    /// Ölçüm aralığı gözlemi (ölçülen aralık, aralıktan örnek, son g).
    interval: Option<(u64, u64, Option<i64>)>,
    /// Korumanın hız tahmininin değiştiği anlar: (ACK anı, tahmin).
    rate_trace: Vec<(u64, Option<u64>)>,
    /// İlk ACK'ten sonra korumanın gönderim tavanının (hat modeli) en küçüğü (B/s, tel).
    min_pace: Option<u64>,
    /// Gönderim tavanının değiştiği anlar: (an, tavan) (gözlem).
    pace_trace: Vec<(u64, u64)>,
    /// [Y4] Demet başına planlı an ve alıcıya ilk varış (varmayan `None`).
    voice_first: Vec<(u64, Option<u64>)>,
    /// [Y4] PTT iletimlerinin darboğaz kaydı ([`Bottleneck::ptt_log`]).
    ptt_log: Vec<(u64, u64, Option<u64>, bool)>,
    /// [Y4] Yalın kuralda son tarihte bırakılan demet sayısı (teyitsiz; alıcıya varmış olanlar dahil:
    /// ACK son tarihten önce dönmediyse varmış demet de bırakılır).
    ptt_abandoned: usize,
    /// [Y4] Bırakılanlardan alıcıya (herhangi bir anda) varmış olanlar: yalnız rapor; zamanında
    /// sayımı [`ptt_phases`]'ın ilk varış kuralıyla, bundan bağımsız.
    ptt_abandoned_arrived: usize,
}

fn sender(copy_rule: Copies) -> Sender {
    let (copies, spacing_ns, scale_min) = copy_rule.rule();
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
    s.set_forward_copy_rule(Some(ForwardCopyRule {
        below_id: if copy_rule == Copies::Lean {
            PTT_LANE_TOP
        } else {
            FILE_BASE_ID
        },
        priority_below_id: FILE_BASE_ID,
        copies,
        spacing_ns,
    }));
    s.set_forward_copy_line_scale(scale_min.map(|m| ForwardCopyLineScale {
        declared_bps: PROFILE_UP,
        min_copies: m,
    }));
    s
}

fn run(real_up: u64, seed: u64, new_receiver: bool) -> Outcome {
    run_with(real_up, seed, new_receiver, Perturb::None, Copies::Harness)
}

fn run_with(
    real_up: u64,
    seed: u64,
    new_receiver: bool,
    perturb: Perturb,
    copy_rule: Copies,
) -> Outcome {
    run_timed(
        real_up,
        seed,
        new_receiver,
        perturb,
        copy_rule,
        VOICE_START_NS,
        0,
    )
}

/// [`run_with`] ve uygulamanın başlangıç zamanlaması: ilk PTT demeti `voice_start_ns`'te, dosya
/// `file_at_ns`'te sunulur (0 = oturum başında, PTT'den önce; [`run_with`]'in düzeni).
#[allow(clippy::too_many_lines)]
fn run_timed(
    real_up: u64,
    seed: u64,
    new_receiver: bool,
    perturb: Perturb,
    copy_rule: Copies,
    voice_start_ns: u64,
    file_at_ns: u64,
) -> Outcome {
    let mut s = sender(copy_rule);
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    let mut fwd = Bottleneck::new(real_up, seed);
    if perturb == Perturb::QueueDropAfterOpener {
        fwd.drop_plan = DropPlan::WaitOpener;
    }
    let mut rx_perturb = RxPerturb::new(perturb);
    let mut rx = RxMirror::default();
    let mut rate_trace: Vec<(u64, Option<u64>)> = Vec::new();
    let mut min_pace: Option<u64> = None;
    let mut pace_trace: Vec<(u64, u64)> = Vec::new();
    let mut ret = ReturnLine {
        free_at: 0,
        wire: VecDeque::new(),
    };
    let mut voice_offered: Vec<u64> = Vec::new();
    let mut voice_arrived: HashMap<u64, u64> = HashMap::new();
    let mut ptt_confirmed: HashSet<u64> = HashSet::new();
    let mut ptt_abandoned: HashSet<u64> = HashSet::new();
    // [PTT-YALIN · BAŞLANGIÇ] Göndericiye yeni alıcının ilk ACK'i ulaştı (ürünün ilk 0x16 vekili;
    // ölçümden bağımsız; eski alıcı 0x16 yazmaz).
    let mut first_ack_seen = false;
    let mut file_seen: HashSet<u64> = HashSet::new();
    let mut file_done_ns = None;
    let mut file_last_new_sent_ns = None;
    let mut file_new_sent = 0u64;
    let mut first_ack_ns = None;
    let mut tx_datagrams = 0u64;
    let emit = |decs: Vec<TransmitDecision>,
                now: u64,
                fwd: &mut Bottleneck,
                tx: &mut u64,
                file_new_sent: &mut u64,
                file_last: &mut Option<u64>| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            *tx += 1;
            if d.kind == TransmitKind::Data && d.unit_id >= FILE_BASE_ID {
                *file_new_sent += 1;
                if *file_new_sent == FILE_UNITS {
                    *file_last = Some(now);
                }
            }
            fwd.send(now, d.seq, d.size, d.unit_id);
        }
    };
    let mut file_offered = false;
    let mut now = 0u64;
    while now < T_MAX_NS {
        if !file_offered && now >= file_at_ns {
            file_offered = true;
            for i in 0..FILE_UNITS {
                let decs = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id: FILE_BASE_ID + i,
                        size: FILE_UNIT,
                    }),
                    now,
                );
                emit(
                    decs,
                    now,
                    &mut fwd,
                    &mut tx_datagrams,
                    &mut file_new_sent,
                    &mut file_last_new_sent_ns,
                );
            }
        }
        let next_voice = voice_offered.len() as u64;
        let voice_at = voice_start_ns + next_voice * VOICE_PERIOD_NS;
        if file_done_ns.is_none() && now >= voice_at {
            voice_offered.push(voice_at);
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: next_voice,
                    size: copy_rule.voice_unit(),
                }),
                now,
            );
            emit(
                decs,
                now,
                &mut fwd,
                &mut tx_datagrams,
                &mut file_new_sent,
                &mut file_last_new_sent_ns,
            );
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(
            decs,
            now,
            &mut fwd,
            &mut tx_datagrams,
            &mut file_new_sent,
            &mut file_last_new_sent_ns,
        );
        // [Y4] Yalın kural: ürünün son tarih yolu (`runtime`, pace tick'te Pto'dan sonra; aynı
        // sıra `starved_line_sim.rs`'te): teyitsiz ve sunumundan son tarih geçmiş PTT birimi
        // `abandon_units` ile bırakılır.
        if copy_rule == Copies::Lean {
            ptt_confirmed.extend(
                s.take_confirmed_units()
                    .into_iter()
                    .filter(|&u| u < FILE_BASE_ID),
            );
            let due: Vec<u64> = voice_offered
                .iter()
                .enumerate()
                .map(|(k, &at)| (k as u64, at))
                .filter(|&(k, at)| {
                    first_ack_seen
                        && now.saturating_sub(at) >= PTT_DEADLINE_NS
                        && !ptt_confirmed.contains(&k)
                        && !ptt_abandoned.contains(&k)
                })
                .map(|(k, _)| k)
                .collect();
            if !due.is_empty() {
                ptt_abandoned.extend(due.iter().copied());
                s.abandon_units(&due);
            }
        }
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(
                decs,
                now,
                &mut fwd,
                &mut tx_datagrams,
                &mut file_new_sent,
                &mut file_last_new_sent_ns,
            );
        }
        r.set_uplink_yield(ret.free_at > now);
        let mut acks = Vec::new();
        let mut arrived = Vec::new();
        while fwd.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, udp) = fwd.wire.pop_front().expect("var");
            arrived.push((seq, uid, udp));
        }
        for (seq, uid, udp) in rx_perturb.apply(arrived, now) {
            rx.on_arrival(seq, udp + rx_perturb.size_skew(), now);
            if new_receiver {
                r.set_next_datagram_bytes(udp + rx_perturb.size_skew());
            }
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
                    if file_seen.len() as u64 == FILE_UNITS && file_done_ns.is_none() {
                        file_done_ns = Some(now);
                    }
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
            first_ack_ns.get_or_insert(now);
            s.observe_ack_feedback(
                AckObservation {
                    wire_bytes: udp,
                    seen_count: 0,
                    v2: true,
                    sent_datagrams: tx_datagrams,
                },
                now,
            );
            s.note_ack_arrival_rate(arrival_rate);
            // Eski alıcı ölçüm aralığı da taşımaz (V2 çerçevesi).
            // Ürün çalışma zamanının sırası: `pair_rate`, ölçüm aralığı, sonra ACK.
            if new_receiver {
                first_ack_seen = true;
                s.note_ack_pair_rate(pair_rate);
                s.note_ack_forward_interval(first, last, rx_span, &m);
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(
                decs,
                now,
                &mut fwd,
                &mut tx_datagrams,
                &mut file_new_sent,
                &mut file_last_new_sent_ns,
            );
            let g = s.diag_line_guard().and_then(|g| g.0);
            if rate_trace.last().map(|&(_, r)| r) != Some(g) {
                rate_trace.push((now, g));
            }
        }
        if first_ack_ns.is_some() {
            if let Some((pace, _)) = s.diag_line_guard_pace() {
                min_pace = Some(min_pace.map_or(pace, |m: u64| m.min(pace)));
                if pace_trace.last().map(|&(_, p)| p) != Some(pace) {
                    pace_trace.push((now, pace));
                }
            }
        }
        let voice_left = (0..voice_offered.len() as u64)
            .any(|v| !voice_arrived.contains_key(&v) && !ptt_abandoned.contains(&v));
        if file_done_ns.is_some() && !voice_left {
            break;
        }
        now += MS;
    }
    // Sıralı teslim: demet k, 0..=k'nin hepsi geldiğinde çalınır.
    let mut played_at = 0u64;
    let mut voice_max = 0u64;
    let mut voice_max_start = 0u64;
    let mut voice_over_1s = 0usize;
    let mut voice_undelivered = 0usize;
    let mut voice_late = Vec::new();
    for (k, &planned) in voice_offered.iter().enumerate() {
        match voice_arrived.get(&(k as u64)) {
            Some(&t) if voice_undelivered == 0 => {
                played_at = played_at.max(t);
                let delay = played_at - planned;
                voice_max = voice_max.max(delay);
                if planned < START_PHASE_NS {
                    voice_max_start = voice_max_start.max(delay);
                }
                if delay > SEC {
                    voice_over_1s += 1;
                    if voice_late.len() < 6 {
                        voice_late.push((k as u64, planned, t, played_at));
                    }
                }
            }
            _ => voice_undelivered += 1,
        }
    }
    let first = first_ack_ns.unwrap_or(u64::MAX);
    Outcome {
        file_done_ns,
        file_last_new_sent_ns,
        first_ack_ns,
        max_wait_before_first_ack: fwd
            .waits
            .iter()
            .filter(|&&(t, _)| t < first)
            .map(|&(_, w)| w)
            .max()
            .unwrap_or(0),
        max_wait: fwd.waits.iter().map(|&(_, w)| w).max().unwrap_or(0),
        drops: fwd.dropped_queue.len(),
        voice_max,
        voice_max_start,
        voice_over_1s,
        voice_undelivered,
        voice_late,
        first_window: s.diag_line_guard_first_window(),
        first_window_detail: s.diag_line_guard_first_window_detail(),
        guard: s.diag_line_guard(),
        start_exit: s.diag_line_guard_start_exit(),
        min_pace,
        pace_trace,
        interval: s.diag_line_guard_interval(),
        rate_trace,
        voice_first: voice_offered
            .iter()
            .enumerate()
            .map(|(k, &p)| (p, voice_arrived.get(&(k as u64)).copied()))
            .collect(),
        ptt_log: fwd.ptt_log.clone(),
        ptt_abandoned: ptt_abandoned.len(),
        ptt_abandoned_arrived: ptt_abandoned
            .iter()
            .filter(|k| voice_arrived.contains_key(k))
            .count(),
    }
}

fn secs(ns: u64) -> String {
    format!("{:.2} s", ns as f64 / SEC as f64)
}

fn opt_secs(ns: Option<u64>) -> String {
    ns.map_or("YOK".to_string(), secs)
}

fn report(tag: &str, real_up: u64, seed: u64, o: &Outcome) {
    println!(
        "BAŞLANGIÇ-SİM {tag} gerçek {real_up} tohum {seed}: ilk_ACK={} ilk_pencere={:?} {:?} \
         koruma={:?} bütçe_çıkışı={:?} aralık={:?} en_küçük_tavan={:?} bekleme_max(ilk ACK öncesi)={} bekleme_max={} kuyruk_düşen={} \
         ptt_sıralı_max={} ptt_başlangıç_max={} ptt_1s_üstü={} ptt_teslimsiz={} dosya_son_yeni={} \
         dosya={} geç={} hız_izi={} tavan_izi={}",
        opt_secs(o.first_ack_ns),
        o.first_window,
        o.first_window_detail,
        o.guard,
        o.start_exit,
        o.interval,
        o.min_pace,
        secs(o.max_wait_before_first_ack),
        secs(o.max_wait),
        o.drops,
        secs(o.voice_max),
        secs(o.voice_max_start),
        o.voice_over_1s,
        o.voice_undelivered,
        opt_secs(o.file_last_new_sent_ns),
        opt_secs(o.file_done_ns),
        o.voice_late
            .iter()
            .map(|&(k, p, t, pl)| format!("#{k}@{}→{}/{}", secs(p), secs(t), secs(pl)))
            .collect::<Vec<_>>()
            .join(" "),
        o.rate_trace
            .iter()
            .map(|&(t, r)| format!("{}:{r:?}", secs(t)))
            .collect::<Vec<_>>()
            .join(" "),
        o.pace_trace
            .iter()
            .take(12)
            .map(|&(t, p)| format!("{}:{p}", secs(t)))
            .collect::<Vec<_>>()
            .join(" "),
    );
}

/// Gerçek hat beyanın yarısıyken (P/2) ilk ACK'e dek kurulabilecek en büyük kuyruk (ns):
/// patlamanın sonunda `START_BURST_NS × (P − gerçek) / gerçek`, artı bir tam datagramın gerçek
/// hatta serileşmesi (FIFO çözünürlüğü).
fn start_queue_bound_ns(real_up: u64) -> u64 {
    START_BURST_NS * (PROFILE_UP - real_up) / real_up + (MTU + OVH) * SEC / real_up
}

/// Profil iyimserken (gerçek 6 250 ve 10 000 B/s, beyan 12 500): ilk ACK ilk varış penceresiyle
/// hattı ölçer (±%2), ilk ACK'e dek ve bütün oturumda darboğaz kuyruğu başlangıç sınırını aşmaz,
/// kuyrukta düşme yok, başlangıç evresinde (ürün `START_PHASE` = 8 s) PTT demetleri sıralı
/// teslimde 1 s içinde.
///
/// Sınırlar ölçümden önce, fizikten (`start_queue_bound_ns`): 6 250'de 0,5 + 0,2 s, 10 000'de
/// 0,125 + 0,12 s. Ölçülen (2026-11-21, tohum 1–3): 6 250'de 0,49 s, 10 000'de 0,12 s.
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_optimistic_profile_is_measured_on_the_first_ack_and_the_start_queue_stays_bounded`].
#[test]
fn optimistic_profile_is_measured_on_the_first_ack_and_the_start_queue_stays_bounded() {
    for real_up in [6_250u64, 10_000] {
        for seed in 1..=3u64 {
            let o = run(real_up, seed, true);
            report("yeni-alıcı", real_up, seed, &o);
            let Some(FirstWindowOutcome::Sample(est)) = o.first_window else {
                panic!(
                    "gerçek {real_up} tohum {seed}: ilk pencere {:?}",
                    o.first_window
                );
            };
            assert!(
                est.abs_diff(real_up) * 50 <= real_up,
                "gerçek {real_up} tohum {seed}: ilk pencere tahmini {est}"
            );
            let bound = start_queue_bound_ns(real_up);
            assert!(
                o.max_wait_before_first_ack <= bound && o.max_wait <= bound,
                "gerçek {real_up} tohum {seed}: kuyruk {} / {} (sınır {})",
                secs(o.max_wait_before_first_ack),
                secs(o.max_wait),
                secs(bound)
            );
            assert_eq!(o.drops, 0, "gerçek {real_up} tohum {seed}: kuyrukta düşme");
            assert!(
                o.voice_max_start <= PTT_BOUND_NS,
                "gerçek {real_up} tohum {seed}: başlangıç PTT {}",
                secs(o.voice_max_start)
            );
            assert_eq!(o.voice_undelivered, 0);
            assert!(
                o.file_done_ns.is_some(),
                "gerçek {real_up} tohum {seed}: dosya bitmedi"
            );
        }
    }
}

/// Profil dürüstken: ilk ACK hattı doğrular, koruma hiç devreye girmez, kuyruk yok. Bütçenin
/// dosyaya bedeli ilk ACK'in gelişine dek bekleyen gönderimle sınırlıdır: dosyanın son yeni
/// birimi en geç `ilk ACK + (dosya + PTT tel baytı) / profil` anında çıkar (bütçe ilk ACK'ten önce
/// hiç gönderim yaptırmasaydı). Ölçülen (2026-11-21, tohum 1–3): 5,35–5,95 s; sınır ≈ 6,2–6,5 s.
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_honest_profile_is_verified_on_the_first_ack_and_the_budget_costs_at_most_the_wait`].
#[test]
fn honest_profile_is_verified_on_the_first_ack_and_the_budget_costs_at_most_the_wait() {
    for seed in 1..=3u64 {
        let o = run(PROFILE_UP, seed, true);
        report("yeni-alıcı", PROFILE_UP, seed, &o);
        assert_eq!(
            o.first_window,
            Some(FirstWindowOutcome::Verified),
            "tohum {seed}"
        );
        assert_eq!(
            o.guard,
            Some((None, 0)),
            "tohum {seed}: koruma devreye girdi"
        );
        assert!(
            o.max_wait <= (MTU + OVH) * SEC / PROFILE_UP,
            "tohum {seed}: kuyruk"
        );
        assert_eq!(o.drops, 0);
        let first_ack = o.first_ack_ns.expect("ilk ACK geldi");
        let file_wire = FILE_UNITS * (FILE_UNIT + OVH);
        // PTT: 200 ms'de bir demet, orijinal + dört kopya.
        let ptt_wire_bps = 5 * (VOICE_UNIT + OVH) * SEC / VOICE_PERIOD_NS;
        let bound = first_ack + file_wire * SEC / (PROFILE_UP - ptt_wire_bps);
        let last_new = o.file_last_new_sent_ns.expect("dosya gönderildi");
        assert!(
            last_new <= bound,
            "tohum {seed}: dosyanın son yeni birimi {} (sınır {})",
            secs(last_new),
            secs(bound)
        );
        assert!(
            o.voice_max <= PTT_BOUND_NS,
            "tohum {seed}: PTT {}",
            secs(o.voice_max)
        );
    }
}

/// Eski alıcı (boy verilmez, alan daima sentinel): ilk pencere yok, bütçe ilk ACK'le kalkar ve
/// gönderici bugünkü hat korumasına döner; ilk ACK'e dek kuyruk yine başlangıç sınırında.
///
/// **Eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** Yalın kolu yoktur: eski alıcı
/// 0x15 alanlarını taşımaz ve 0x16 yazamaz, ürün bu eşle yalın PTT'yi hiç açmaz (kopyalı kural
/// bu eşin ürün davranışıdır).
#[test]
fn old_receiver_falls_back_to_todays_guard() {
    for real_up in [6_250u64, PROFILE_UP] {
        for seed in 1..=3u64 {
            let o = run(real_up, seed, false);
            report("eski-alıcı", real_up, seed, &o);
            assert_eq!(o.first_window, None, "gerçek {real_up} tohum {seed}");
            assert!(
                o.max_wait_before_first_ack <= start_queue_bound_ns(real_up),
                "gerçek {real_up} tohum {seed}: ilk ACK öncesi kuyruk {}",
                secs(o.max_wait_before_first_ack)
            );
            assert!(
                o.file_done_ns.is_some(),
                "gerçek {real_up} tohum {seed}: dosya bitmedi"
            );
        }
    }
}

/// [İLK-PENCERE · İŞ-35 AÇIK 4] Bir bozulmalı koşunun kabul ölçütleri (gerçek hat `real_up`,
/// beyan 12 500). Ölçütler ürün kabulünden ve başlangıç bütçesinin fiziğinden, sonuçtan önce:
/// * başlangıç evresinde (plan < 8 s) PTT demetleri sıralı teslimde ≤ 1 s;
/// * yanlış DÜŞÜK örnek öncelikli şeridi aç bırakmaz: ilk ACK'ten sonra korumanın gönderim tavanı
///   (hat modeli) hiçbir zaman PTT şeridinin kendi tel hızının (iletim sayısı × 114 B × 5/s)
///   altına inmez;
/// * bütün oturumda darboğaz beklemesi ≤ `start_queue_bound_ns(real_up)` (başlangıç bütçesinin
///   gerçek hat beyanın yarısıyken kurduğu en büyük kuyruk);
/// * kuyrukta düşme yalnız enjekte edilen (`QueueDropAfterOpener`: 1);
/// * koşu sonunda korumanın tahmini gerçek hattın ±%10 içinde;
/// * `full` ile: bütün demetler teslim ve dosya [`T_MAX_NS`] içinde biter.
///
/// [Y4] [`Copies::Lean`] kolunda PTT iddiası sahibin ölçütüdür (her evrede zamanında oranı ≥ %75,
/// [`lean_ptt_violations`]); PTT şeridinin tel hızı yalın birimden (1 × 106 B × 5/s); teslim
/// ölçütü yoktur (son tarihte bırakma sözleşmedir). Diğer ölçütler aynen.
///
/// Bütün oturumun PTT'si raporlanır ama ölçüt değildir (kopyalı kollar): bu düzenekte (V2 ACK çerçevesi, %20 kayıp)
/// ACK aralığı sürekli evrede ≈ 8,6 s'ye çıkar; beş iletimi de düşen bir demetin ARQ kurtarması o
/// kadar sürer ve sürekli evrenin süreli yoklamaları (+%6,25) demetleri 1 s'nin biraz üstüne
/// itebilir. Ölçüldü, bozulmasız gerçek 6 250: İŞ-35 kuralıyla tohum 1'de demet 250 (50,5 s)
/// 9,5 s, tohum 3'te 21,9 s'deki yoklamadan sonra 1,17 s; kuralsız 0,81–0,92 s (zamanlama farkı,
/// ilk pencereden sonraki evre).
///
/// Dönen: ihlallerin metni (boş = geçti).
fn perturbed_violations(
    real_up: u64,
    seed: u64,
    p: Perturb,
    copy_rule: Copies,
    full: bool,
) -> Vec<String> {
    let o = run_with(real_up, seed, true, p, copy_rule);
    report(&format!("{p:?}/{copy_rule:?}"), real_up, seed, &o);
    let tag = format!("{p:?} {copy_rule:?} gerçek {real_up} tohum {seed}");
    let mut v = Vec::new();
    if copy_rule == Copies::Lean {
        // [Y4] Yalın kol: "başlangıç evresinde sıralı teslim ≤ 1 s" yerine sahibin ölçütü
        // (her evrede zamanında oranı ≥ %75, ıska ayrıştırmasıyla).
        v.extend(lean_ptt_violations(&tag, &o));
    } else if o.voice_max_start > PTT_BOUND_NS {
        v.push(format!(
            "{tag}: başlangıç PTT {} (ilk pencere {:?})",
            secs(o.voice_max_start),
            o.first_window
        ));
    }
    let (copies, _, _) = copy_rule.rule();
    let ptt_wire_bps =
        (u64::from(copies) + 1) * (copy_rule.voice_unit() + OVH) * SEC / VOICE_PERIOD_NS;
    if o.min_pace.is_some_and(|m| m < ptt_wire_bps) {
        v.push(format!(
            "{tag}: tavan {:?} < PTT şeridi {ptt_wire_bps} (oturum PTT {})",
            o.min_pace,
            secs(o.voice_max)
        ));
    }
    // Yalın kolda teslim edilmeyen demet sözleşmedir (son tarihte bırakma): teslim ölçütü yok.
    if full && copy_rule != Copies::Lean && o.voice_undelivered > 0 {
        v.push(format!("{tag}: teslimsiz demet {}", o.voice_undelivered));
    }
    let bound = start_queue_bound_ns(real_up);
    if o.max_wait > bound {
        v.push(format!(
            "{tag}: kuyruk {} (sınır {})",
            secs(o.max_wait),
            secs(bound)
        ));
    }
    let injected = usize::from(p == Perturb::QueueDropAfterOpener);
    if o.drops != injected {
        v.push(format!(
            "{tag}: kuyrukta düşen {} (enjekte {injected})",
            o.drops
        ));
    }
    match o.guard.and_then(|g| g.0) {
        Some(r) if r.abs_diff(real_up) * 10 <= real_up => {}
        other => v.push(format!("{tag}: son tahmin {other:?}")),
    }
    if full && o.file_done_ns.is_none() {
        v.push(format!("{tag}: dosya bitmedi"));
    }
    v
}

/// Gerçek 6 250 (beyan 12 500), tohum 1–3, iki PTT kuralıyla: düzeneğin kuralında bütün ölçütler,
/// ürün kuralında teslim ve dosya bitişi dışında hepsi (ürün kuralı bu düzenekte bozulmasız
/// koşuda da dosyayı 35–86 s'de bitirir: FEC'siz düzenek, ilk pencereden bağımsız).
fn assert_perturbation_meets_acceptance(p: Perturb) {
    assert_perturbation_meets_acceptance_for(p, 1..=3);
}

fn assert_perturbation_meets_acceptance_for(p: Perturb, seeds: std::ops::RangeInclusive<u64>) {
    let mut v = Vec::new();
    for seed in seeds {
        v.extend(perturbed_violations(6_250, seed, p, Copies::Harness, true));
        v.extend(perturbed_violations(6_250, seed, p, Copies::Product, false));
    }
    assert!(v.is_empty(), "{} ihlal:\n{}", v.len(), v.join("\n"));
}

/// Paket boyu farkı: alıcıya verilen boy her datagramda 100 B büyük (≈ +%10; iki ucun boy sayımı
/// farklı). İlk pencere hattı ≈ %10 yüksek okur (6 873–6 894).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_first_window_perturbations_meet_the_start_acceptance`].
#[test]
fn size_skew_in_the_first_window_meets_the_start_acceptance() {
    assert_perturbation_meets_acceptance(Perturb::SizeSkew(100));
}

/// Yeniden sıralama: pencereyi açan datagramdan sonraki iki varış yer değiştirir. Tohum 3'te
/// pencerenin son datagramı geç işlenir, ilk pencere hattı 2 215 B/s okur (yanlış DÜŞÜK).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_first_window_perturbations_meet_the_start_acceptance`].
#[test]
fn reordering_after_the_opener_meets_the_start_acceptance() {
    assert_perturbation_meets_acceptance(Perturb::Reorder);
}

/// Zamanlama sapması 40 ms: açıcının alıcı damgası geç; pencere kısa görünür, hat yüksek okunur
/// (+%14–33).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_first_window_perturbations_meet_the_start_acceptance`].
#[test]
fn a_40ms_late_first_stamp_meets_the_start_acceptance() {
    assert_perturbation_meets_acceptance(Perturb::LateFirstStamp(40 * MS));
}

/// Zamanlama sapması 200 ms: pencere `Δτ < Δs` (tohum 1, yanlış doğrulama) ya da kapanmaz
/// (tohum 2–3, sentinel).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_first_window_perturbations_meet_the_start_acceptance`].
#[test]
fn a_200ms_late_first_stamp_meets_the_start_acceptance() {
    assert_perturbation_meets_acceptance(Perturb::LateFirstStamp(200 * MS));
}

/// Kuyruk kaybı: açıcıdan sonra hatta sunulan ilk datagram darboğaz kuyruğunda düşer (hattı
/// kullanmaz). Tohum 1'de düşen datagram pencerenin içinde ama artış görünür: pencerenin gönderim
/// baytı şişer, örnek 9 372 (gerçek 6 250).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_first_window_perturbations_meet_the_start_acceptance`].
#[test]
fn a_queue_drop_after_the_opener_meets_the_start_acceptance() {
    assert_perturbation_meets_acceptance_for(Perturb::QueueDropAfterOpener, 1..=1);
}

/// [ÇÖZÜLMEDİ · İŞ-35 AÇIK 4] Kuyruk kaybı artışı gizliyor (tohum 2–3): açıcı seq 0, seq 1 kuyrukta
/// düşer, seq 2 pencereyi kapatır; `Δs` = 163 ms, `Δτ` = 164 ms → pencere hattı DOĞRULAR (yanlış),
/// gönderici ilk ACK'ten (2,04 s) sonra profil hızında gönderir; kuyruk 2,96–3,68 s, 35–58 düşme,
/// başlangıç PTT 3,4–19,3 s. Bu test kırmızıdır ve kırmızı kalır (gizlenmez).
///
/// Neden kapatılamadı (bu düzenekte ölçülen değerlerle alt sınır):
/// * Gözlem dürüst profilin tohum 1'iyle aynıdır: orada açıcı seq 2, seq 3 RASTGELE kayıpla düşer
///   (serileşmeden sonra; hattı kullandı), seq 4 kapatır; `Δs` = 164 ms, `Δτ` = 165 ms. İki
///   pencerede de bir boşluk ve `Δτ ≈ Δs` var; ilk ACK'in ölçüm aralığı ikisinde de düzdür (cephesi
///   kuyruksuz). Ne alıcı ne gönderici boşluğun kuyrukta mı yolda mı düştüğünü bilir. İlk ACK'in
///   bilgisiyle çalışan her kural iki koşuda aynı davranır.
/// * İkinci ölçüm en erken ikinci ACK'tedir: dürüst tohum 1'de ilk ACK 2,12 s, ikinci 4,56 s
///   (`t₂ − t₁` = 2,44 s); kuyruk kaybı tohum 2'de 2,04 → 4,17 s.
/// * Kuyruk kaybı koşusunda kuyruğun başlangıç sınırında (0,70 s) kalması için `[t₁, t₂]`'de hatta
///   fazladan verilebilecek bayt en çok 0,70 s × 6 250 B/s ≈ 4,4 kB'tır (gönderim ≈ P/2 + 4,4 kB).
///   Aynı gönderim dürüst tohum 1'de `[t₁, t₂]`'de profil hızına göre en az
///   6 250 × 2,44 − 4 375 ≈ 10,9 kB eksik gönderir: dosya en az 10,9 kB / 9 650 B/s ≈ 1,13 s geç
///   biter. Dürüst bütçe bedeli testinin (`honest_profile_is_verified_…`) payı tohum 1'de
///   6,49 − 5,96 = 0,53 s'dir: iki ölçüt birlikte sağlanamaz.
///
/// Kapatmak için gereken: ilk ACK'ten önce ikinci bir bağımsız ölçüm (ör. daha uzun ilk varış
/// penceresi — alıcı ve donmuş spec §2.2 `FIRST_WINDOW_BYTES`, bu işin dışında) ya da dürüst
/// profilde bütçe bedeli sınırının gevşetilmesi (sahip kararı).
#[test]
fn a_queue_drop_hidden_in_the_first_window_is_not_closed() {
    assert_perturbation_meets_acceptance_for(Perturb::QueueDropAfterOpener, 2..=3);
}

/// Dürüst profil (gerçek = beyan) aynı bozulmalarla: PTT ≤ 1 s, kuyruk bir datagramın
/// serileşmesini aşmaz, kuyrukta düşme yalnız enjekte edilen. Dosyanın son yeni biriminin anı
/// raporlanır (bozulmanın dürüst hatta bedeli; sınır yok).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_honest_profile_under_first_window_perturbations_keeps_ptt_and_queue`].
#[test]
fn honest_profile_under_first_window_perturbations_keeps_ptt_and_queue() {
    let mut v = Vec::new();
    for p in [
        Perturb::SizeSkew(100),
        Perturb::Reorder,
        Perturb::LateFirstStamp(40 * MS),
        Perturb::LateFirstStamp(200 * MS),
        Perturb::QueueDropAfterOpener,
    ] {
        for seed in 1..=3u64 {
            let o = run_with(PROFILE_UP, seed, true, p, Copies::Harness);
            report(&format!("dürüst/{p:?}"), PROFILE_UP, seed, &o);
            let tag = format!("{p:?} tohum {seed}");
            if o.voice_max > PTT_BOUND_NS || o.voice_undelivered > 0 {
                v.push(format!("{tag}: PTT {}", secs(o.voice_max)));
            }
            if o.max_wait > (MTU + OVH) * SEC / PROFILE_UP {
                v.push(format!("{tag}: kuyruk {}", secs(o.max_wait)));
            }
            let injected = usize::from(p == Perturb::QueueDropAfterOpener);
            if o.drops != injected {
                v.push(format!("{tag}: kuyrukta düşen {}", o.drops));
            }
        }
    }
    assert!(v.is_empty(), "{} ihlal:\n{}", v.len(), v.join("\n"));
}

/// [İŞ-35 · HAT-ÇIKIŞI] Dürüst profil, ürün benzeri başlangıç (PTT 0,1 s'den, dosya 0,4 s'de, ürün
/// kopya kuralı), tohum 1–3: ilk varış penceresine PTT kopyaları karışır ve açıcı kendi
/// kopyalarımızın arkasında hat kuyruğunda bekler. Pencere gönderim anlarıyla ölçülünce tohum 2–3'te
/// `Δτ < Δs` (sıkışık) ve gönderim hızı tavanın %90'ının altında görünüyordu: bilgi yok, bütçe
/// (`P/2`) ikinci ölçüme dek sürüyordu. Pencere göndericinin hat çıkış anlarıyla ölçülünce hattı
/// doğrular: bütçe ilk ACK'te kalkar ve dosyanın son yeni birimi, bütçenin her zaman ilk ACK'te
/// kalktığı karşı-olgunun en çok 0,5 s gerisindedir.
///
/// Karşı-olgu (aynı düzenek; `first_ack_trusted` daima doğru, ilk pencere gönderim anlarıyla):
/// dosyanın son yeni birimi 6,78 / 7,27 / 6,94 s. Bu kuraldan önce: 6,78 / 13,19 / 11,18 s
/// (tohum 2–3 `NoInfo` → `Flat(6250)`).
///
/// **GERİLİM / eski uç kolu (sahip kararı 2026-11-22, #175 7080178069).** PTT yükü kopyalı
/// ([`Copies::Harness`] / [`Copies::Product`]); iddiaları değişmedi. Kapı kolu: [`lean_honest_start_with_ptt_ahead_of_the_file_lifts_the_budget_on_the_first_ack`].
#[test]
fn honest_start_with_ptt_ahead_of_the_file_lifts_the_budget_on_the_first_ack() {
    const COUNTERFACTUAL_LAST_NEW_MS: [u64; 3] = [6_780, 7_270, 6_940];
    for seed in 1..=3u64 {
        let o = run_timed(
            PROFILE_UP,
            seed,
            true,
            Perturb::None,
            Copies::Product,
            100 * MS,
            400 * MS,
        );
        report("ptt@0,1/dosya@0,4", PROFILE_UP, seed, &o);
        assert_eq!(
            o.start_exit,
            Some(adaptive_transport::line_guard::StartExit::FirstAck),
            "tohum {seed}: ilk pencere {:?} {:?}",
            o.first_window,
            o.first_window_detail
        );
        let bound = (COUNTERFACTUAL_LAST_NEW_MS[seed as usize - 1] + 500) * MS;
        let last_new = o.file_last_new_sent_ns.expect("dosya gönderildi");
        assert!(
            last_new <= bound,
            "tohum {seed}: dosyanın son yeni birimi {} (sınır {})",
            secs(last_new),
            secs(bound)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// [Y4 · YALIN KOLLAR · sahip kararı 2026-11-22, #175 7080178069] PTT'yi YÜK olarak kullanan
// başlangıç testlerinin yalın PTT yüküyle aynası ([`Copies::Lean`]). Kapı yalın sözleşmedir;
// kopyalı kollar yerinde GERİLİM / eski uç kolu olarak kalır.
// * Hat ve bütçe iddiaları var olan testteki sınırlarla, değiştirilmeden (yalnız PTT şeridinin tel
//   hızı giren formüller yalın birimden: 1 iletim × 106 B × 5/s).
// * PTT iddiası (her yalın kolda) sahibin ölçütü: demet sunumundan ≤ 1 s içinde alıcıya İLK kez
//   varırsa zamanında; her evrede (başlangıç [0, 8 s), sürekli) zamanında oranı ≥ %75. Kırmızıda
//   ileti ıskaları dört sınıfa ayırır (`starved_line_sim.rs` `Misses` ile aynı tanım) ve ulaşım
//   kaynaklı ıska sayısını yazar. Yalnız `rastgele` ıskalı kırmızı kapı eşiğinin istatistiğidir
//   (tek iletimde tavan %80; sahip kararı bekliyor, #175 7080413643).
// * Yalın kol YOK: `old_receiver_falls_back_to_todays_guard` (eski alıcı 0x15 alanlarını
//   taşımaz, 0x16 yazamaz; ürün o eşle yalın PTT'yi hiç açmaz) ve
//   `a_queue_drop_hidden_in_the_first_window_is_not_closed` (kapsam dışı, bilinen kırmızı).
// ---------------------------------------------------------------------------------------------

/// Zamanında olmayan demetlerin nedeni (`starved_line_sim.rs` `Misses` / `miss_class` aynası; sınıf
/// demetin İLK iletiminin akıbetinden): rastgele kayıp · kuyrukta düşme · geç varış · bırakma
/// (hiç iletilmeden). Ulaşım kaynaklı = son üçü.
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

/// Bir evrenin PTT ölçüsü (`starved_line_sim.rs` `PhaseOnTime` aynası).
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
    fn passes(&self) -> bool {
        self.planned > 0
            && self.on_time as u64 * 1000 >= PTT_ON_TIME_MIN_PERMILLE * self.planned as u64
    }

    fn line(&self) -> String {
        format!(
            "{}[{}–{}] {}/{} (ıska: rastgele {}, kuyruk {}, geç {}, bırakma {}; ulaşım kaynaklı {})",
            self.name,
            secs(self.from),
            secs(self.to),
            self.on_time,
            self.planned,
            self.misses.random,
            self.misses.queue,
            self.misses.late,
            self.misses.abandoned,
            self.misses.transport()
        )
    }
}

/// Evreler: `başlangıç` [0, 8 s) ve `sürekli` [8 s, son demetin planı + 200 ms) (bu düzenekte hat
/// hızı sabittir; konuşma dosya bitince durur).
fn ptt_phases(o: &Outcome) -> Vec<PhaseOnTime> {
    let end = o
        .voice_first
        .last()
        .map_or(0, |&(p, _)| p + VOICE_PERIOD_NS)
        .max(START_PHASE_NS);
    [
        ("başlangıç", 0, START_PHASE_NS),
        ("sürekli", START_PHASE_NS, end),
    ]
    .into_iter()
    .filter(|&(_, from, to)| from < to)
    .map(|(name, from, to)| {
        let mut ph = PhaseOnTime {
            name,
            from,
            to,
            planned: 0,
            on_time: 0,
            misses: Misses::default(),
        };
        for (v, &(plan, first)) in o.voice_first.iter().enumerate() {
            if plan < from || plan >= to {
                continue;
            }
            ph.planned += 1;
            if first.is_some_and(|a| a <= plan + PTT_BOUND_NS) {
                ph.on_time += 1;
                continue;
            }
            match o.ptt_log.iter().find(|&&(_, u, _, _)| u == v as u64) {
                None => ph.misses.abandoned += 1,
                Some(&(_, _, None, _)) => ph.misses.queue += 1,
                Some(&(_, _, Some(_), false)) => ph.misses.random += 1,
                Some(_) => ph.misses.late += 1,
            }
        }
        ph
    })
    .collect()
}

fn phases_line(ph: &[PhaseOnTime]) -> String {
    ph.iter()
        .map(PhaseOnTime::line)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Bütün evrelerin ölçütü: kalan evreler ıska ayrıştırmasıyla.
fn lean_ptt_violations(tag: &str, o: &Outcome) -> Vec<String> {
    let ph = ptt_phases(o);
    println!(
        "YALIN {tag}: {} | bırakılan {} (alıcıya varmış {})",
        phases_line(&ph),
        o.ptt_abandoned,
        o.ptt_abandoned_arrived
    );
    ph.iter()
        .filter(|p| !p.passes())
        .map(|p| format!("{tag}: PTT {}", p.line()))
        .collect()
}

fn assert_lean(line: &[String], ptt: &[String]) {
    assert!(
        line.is_empty() && ptt.is_empty(),
        "hat iddiaları ({}): {line:#?}\nPTT ölçütü ({}): {ptt:#?}",
        line.len(),
        ptt.len()
    );
}

/// [Y4] [`optimistic_profile_is_measured_on_the_first_ack_and_the_start_queue_stays_bounded`]'in
/// yalın kolu: aynı sınırlar (ilk pencere ±%2, kuyruk ≤ `start_queue_bound_ns`, kuyrukta düşme yok,
/// dosya biter); kopyalı kolun "başlangıç evresinde sıralı PTT ≤ 1 s" iddiası yerine her evrede
/// zamanında oranı ≥ %75. Teslim ölçütü yok (son tarihte bırakma sözleşmedir).
#[test]
fn lean_optimistic_profile_is_measured_on_the_first_ack_and_the_start_queue_stays_bounded() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for real_up in [6_250u64, 10_000] {
        for seed in 1..=3u64 {
            let o = run_with(real_up, seed, true, Perturb::None, Copies::Lean);
            let tag = format!("gerçek {real_up} tohum {seed}");
            report("yalın", real_up, seed, &o);
            match o.first_window {
                Some(FirstWindowOutcome::Sample(est)) if est.abs_diff(real_up) * 50 <= real_up => {}
                other => line.push(format!("{tag}: ilk pencere {other:?}")),
            }
            let bound = start_queue_bound_ns(real_up);
            if o.max_wait_before_first_ack > bound || o.max_wait > bound {
                line.push(format!(
                    "{tag}: kuyruk {} / {} (sınır {})",
                    secs(o.max_wait_before_first_ack),
                    secs(o.max_wait),
                    secs(bound)
                ));
            }
            if o.drops != 0 {
                line.push(format!("{tag}: kuyrukta düşme {}", o.drops));
            }
            if o.file_done_ns.is_none() {
                line.push(format!("{tag}: dosya bitmedi"));
            }
            ptt.extend(lean_ptt_violations(&tag, &o));
        }
    }
    assert_lean(&line, &ptt);
}

/// [Y4] [`honest_profile_is_verified_on_the_first_ack_and_the_budget_costs_at_most_the_wait`]'in
/// yalın kolu: aynı iddialar (ilk ACK doğrular, koruma girmez, kuyruk ≤ bir datagramın
/// serileşmesi, düşme yok); bütçe bedeli sınırı aynı formül `ilk ACK + dosya / (profil − PTT tel
/// hızı)`, PTT tel hızı yalın kuraldan (1 × 106 B × 5/s = 530 B/s; kopyalı kolda 5 × 114 B × 5/s).
/// Kopyalı kolun "oturum PTT ≤ 1 s" iddiası yerine her evrede zamanında oranı ≥ %75.
#[test]
fn lean_honest_profile_is_verified_on_the_first_ack_and_the_budget_costs_at_most_the_wait() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let o = run_with(PROFILE_UP, seed, true, Perturb::None, Copies::Lean);
        report("yalın", PROFILE_UP, seed, &o);
        let tag = format!("tohum {seed}");
        if o.first_window != Some(FirstWindowOutcome::Verified) {
            line.push(format!("{tag}: ilk pencere {:?}", o.first_window));
        }
        if o.guard != Some((None, 0)) {
            line.push(format!("{tag}: koruma devreye girdi {:?}", o.guard));
        }
        if o.max_wait > (MTU + OVH) * SEC / PROFILE_UP {
            line.push(format!("{tag}: kuyruk {}", secs(o.max_wait)));
        }
        if o.drops != 0 {
            line.push(format!("{tag}: kuyrukta düşme {}", o.drops));
        }
        let first_ack = o.first_ack_ns.expect("ilk ACK geldi");
        let file_wire = FILE_UNITS * (FILE_UNIT + OVH);
        let ptt_wire_bps = (LEAN_PTT_UNIT + OVH) * SEC / VOICE_PERIOD_NS;
        let bound = first_ack + file_wire * SEC / (PROFILE_UP - ptt_wire_bps);
        let last_new = o.file_last_new_sent_ns.expect("dosya gönderildi");
        if last_new > bound {
            line.push(format!(
                "{tag}: dosyanın son yeni birimi {} (sınır {})",
                secs(last_new),
                secs(bound)
            ));
        }
        ptt.extend(lean_ptt_violations(&tag, &o));
    }
    assert_lean(&line, &ptt);
}

/// [Y4] İlk pencere bozulmalarının yalın kolu: `size_skew_…`, `reordering_…`, `a_40ms_…`,
/// `a_200ms_…` (tohum 1–3) ve `a_queue_drop_after_the_opener_…` (tohum 1) ile aynı koşular ve
/// [`perturbed_violations`]'ın aynı ölçütleri (`full` = dosya biter; yalın kolda teslim ölçütü yok,
/// PTT her evrede ≥ %75). [`a_queue_drop_hidden_in_the_first_window_is_not_closed`]'in tohumları
/// (2–3) kapsam dışıdır, yalın kolu yoktur.
#[test]
fn lean_first_window_perturbations_meet_the_start_acceptance() {
    let mut v = Vec::new();
    for (p, seeds) in [
        (Perturb::SizeSkew(100), 1..=3u64),
        (Perturb::Reorder, 1..=3),
        (Perturb::LateFirstStamp(40 * MS), 1..=3),
        (Perturb::LateFirstStamp(200 * MS), 1..=3),
        (Perturb::QueueDropAfterOpener, 1..=1),
    ] {
        for seed in seeds {
            v.extend(perturbed_violations(6_250, seed, p, Copies::Lean, true));
        }
    }
    assert!(v.is_empty(), "{} ihlal:\n{}", v.len(), v.join("\n"));
}

/// [Y4] [`honest_profile_under_first_window_perturbations_keeps_ptt_and_queue`]'in yalın kolu:
/// aynı bozulmalar ve tohumlar, aynı kuyruk ve düşme sınırları; kopyalı kolun "PTT ≤ 1 s ve
/// bütün demetler teslim" iddiası yerine her evrede zamanında oranı ≥ %75.
#[test]
fn lean_honest_profile_under_first_window_perturbations_keeps_ptt_and_queue() {
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for p in [
        Perturb::SizeSkew(100),
        Perturb::Reorder,
        Perturb::LateFirstStamp(40 * MS),
        Perturb::LateFirstStamp(200 * MS),
        Perturb::QueueDropAfterOpener,
    ] {
        for seed in 1..=3u64 {
            let o = run_with(PROFILE_UP, seed, true, p, Copies::Lean);
            report(&format!("dürüst/{p:?}/yalın"), PROFILE_UP, seed, &o);
            let tag = format!("{p:?} tohum {seed}");
            if o.max_wait > (MTU + OVH) * SEC / PROFILE_UP {
                line.push(format!("{tag}: kuyruk {}", secs(o.max_wait)));
            }
            let injected = usize::from(p == Perturb::QueueDropAfterOpener);
            if o.drops != injected {
                line.push(format!("{tag}: kuyrukta düşen {}", o.drops));
            }
            ptt.extend(lean_ptt_violations(&tag, &o));
        }
    }
    assert_lean(&line, &ptt);
}

/// [Y4] [`honest_start_with_ptt_ahead_of_the_file_lifts_the_budget_on_the_first_ack`]'in yalın
/// kolu: aynı başlangıç (PTT 0,1 s'den, dosya 0,4 s'de), aynı iddialar (bütçe ilk ACK'te kalkar;
/// dosyanın son yeni birimi kopyalı karşı-olgunun 6,78 / 7,27 / 6,94 s'sinin en çok 0,5 s
/// gerisinde). Sınır kopyalı koldan aynen alındı. Yalın PTT hattan daha az bayt alır (530 B/s'ye
/// karşı 7 × 114 B × 5/s ≈ 3 990 B/s); bu yüzden aynı sınır yalın kol için daha GEVŞEKtir (yalın
/// karşı-olgu ölçülmedi, sınır sıkılaştırılmadı). PTT her evrede ≥ %75.
#[test]
fn lean_honest_start_with_ptt_ahead_of_the_file_lifts_the_budget_on_the_first_ack() {
    const COUNTERFACTUAL_LAST_NEW_MS: [u64; 3] = [6_780, 7_270, 6_940];
    let (mut line, mut ptt) = (Vec::new(), Vec::new());
    for seed in 1..=3u64 {
        let o = run_timed(
            PROFILE_UP,
            seed,
            true,
            Perturb::None,
            Copies::Lean,
            100 * MS,
            400 * MS,
        );
        report("ptt@0,1/dosya@0,4/yalın", PROFILE_UP, seed, &o);
        if o.start_exit != Some(adaptive_transport::line_guard::StartExit::FirstAck) {
            line.push(format!(
                "tohum {seed}: bütçe çıkışı {:?} (ilk pencere {:?} {:?})",
                o.start_exit, o.first_window, o.first_window_detail
            ));
        }
        let bound = (COUNTERFACTUAL_LAST_NEW_MS[seed as usize - 1] + 500) * MS;
        let last_new = o.file_last_new_sent_ns.expect("dosya gönderildi");
        if last_new > bound {
            line.push(format!(
                "tohum {seed}: dosyanın son yeni birimi {} (sınır {})",
                secs(last_new),
                secs(bound)
            ));
        }
        ptt.extend(lean_ptt_violations(&format!("ptt@0,1 tohum {seed}"), &o));
    }
    assert_lean(&line, &ptt);
}
