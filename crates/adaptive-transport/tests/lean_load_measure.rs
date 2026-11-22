//! [ÖLÇÜM · iddiasız] Hat koruması senaryoları (b, d2, e2, d4) yalın PTT yüküyle: yalnız rapor eder.
//!
//! Düzenek `starved_line_sim.rs`'in (night/lean-measure 49aff010) satır 32–957 ve 996–1018'inin
//! bayt-aynı kopyasıdır (`RxMirror` hizalı alıcı, `run`, `drop_scenario`); o dosyanın testleri
//! kopyalanmadı. Senaryoların hat/kayıp/FIFO/zaman parametreleri var olan testlerinkidir; yalnız PTT
//! yükü değişir: `copies = 0` + öncelik (`raw_copy_rule`), 78 B gövde (tel 106 B), sunumdan 1 s sonra
//! bırakma (`ptt_deadline_ns`). Yan kol: aynı senaryonun bugünkü kopya yükü (var olan testin kolu).
//!
//! PTT ölçütü (atlamalı): demet "zamanında" = alıcıya ilk varışı sunum + 1 s içinde; geç ya da hiç
//! varmayan atlandı sayılır (sonrakileri bloklamaz). Hat koruması ölçütleri var olan testlerin PTT
//! dışı ölçütleridir (sınır formülleri aynen). Hiçbir sonuç iddia edilmez: test yalnız yazdırır.
#![allow(dead_code, unused_imports)]

use std::collections::{HashMap, HashSet, VecDeque};

use adaptive_transport::ack::{
    ACK_BITMAP_MAX_BITS, ACK_DELAY_UNIT_US, PAIR_MIN_UDP_BYTES, PAIR_RATE_NONE, RX_SPAN_NONE,
};
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, EmitCause, EmitDiag, Event, FeedbackStarvedRule, ForwardCopyLineScale,
    ForwardCopyRule, OpaqueUnit, RecoveredInfo, RecoveredUnit, Sender, Timer, TransmitDecision,
    TransmitKind,
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
}

impl RxMirror {
    fn on_arrival(&mut self, seq: u64, udp: u64, now: u64) {
        if let Some(p) = self.pair_prev {
            let g = now.saturating_sub(p);
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
                emit(decs, now, &mut fwd, &mut tx);
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

// ---------------------------------------------------------------------------------------------
// Ölçüm (kopya değil).
// ---------------------------------------------------------------------------------------------

/// Yalın PTT biriminin UDP gövdesi: 22 B zarf + 3 B 0x09 başlığı + 53 B (tel 106 B).
const LEAN_PTT_UNIT: u64 = 78;
/// Ürünün PTT son tarihi (`client_core::runtime::PTT_DEADLINE_NS`).
const PTT_DEADLINE_NS: u64 = SEC;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cell {
    B,
    D2,
    E2,
    D4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Load {
    /// Bugünkü kopya yükü (var olan testin kolu).
    Copies,
    /// Koordinatörün yalın sözleşmesi.
    Lean,
}

fn cell_scenario(cell: Cell, seed: u64, load: Load) -> Scenario {
    let base = drop_scenario(seed, true);
    let sc = match cell {
        Cell::B => Scenario {
            text_at_ns: Some(DROP_TEXT_AT_NS),
            ..base
        },
        Cell::D2 => base,
        Cell::E2 => Scenario {
            no_interval: Some((DROP_AT_NS, 19 * SEC)),
            ..base
        },
        Cell::D4 => Scenario {
            queue_pkts: 8,
            ..base
        },
    };
    match load {
        Load::Copies => sc,
        Load::Lean => Scenario {
            copies: 0,
            spacing_ns: 0,
            copy_scale_min: None,
            raw_copy_rule: true,
            voice_unit: LEAN_PTT_UNIT,
            ptt_deadline_ns: Some(PTT_DEADLINE_NS),
            ..sc
        },
    }
}

fn opt_secs(t: Option<u64>) -> String {
    t.map_or("YOK".to_string(), secs)
}

fn yn(b: bool) -> &'static str {
    if b {
        "E"
    } else {
        "H"
    }
}

/// Bir koşunun hat koruması ölçütleri (var olan testin PTT dışı ölçütleri, sınırlar aynen) ve
/// atlamalı PTT ölçüsü; bir satır yazdırır, ölçütlerin hepsi sağlandı mı döndürür.
#[allow(clippy::too_many_lines)]
fn measure(cell: Cell, seed: u64, load: Load) -> bool {
    let sc = cell_scenario(cell, seed, load);
    let o = run(&sc);
    let t_ret = (129 + OVH) * SEC / PROFILE_DOWN + ONE_WAY_NS;
    let wait_bound = 2 * (MTU + OVH) * SEC / DROP_TO;
    let t_ack = o
        .acks_at
        .windows(2)
        .filter(|w| w[1] <= DROP_AT_NS)
        .map(|w| w[1] - w[0])
        .max()
        .unwrap_or(0);
    let conv = converged_at(&o, DROP_TO);
    let mut guard = String::new();
    let mut pass = true;
    let mut check = |name: &str, ok: bool, text: String| {
        pass &= ok;
        guard.push_str(&format!(" {name}[{}]={text}", yn(ok)));
    };
    // E2'nin yakınsama sınırı ölçümün dönüşünden; ötekiler düşüşten.
    let (run_len, t_gap, t_back) = o.longest_unmeasured_run();
    let conv_from = if cell == Cell::E2 {
        t_back.unwrap_or(u64::MAX)
    } else {
        DROP_AT_NS
    };
    let conv_bound = conv_from.saturating_add(3 * t_ack + t_ret);
    let w2 = conv.map(|c| o.max_wait_between(c + 2 * SEC, u64::MAX));
    let drops2 = conv.map(|c| o.drops_between(c + 2 * SEC, u64::MAX));
    if cell == Cell::E2 {
        let gap_wait_bound = PTT_BOUND_NS - ONE_WAY_NS;
        let w_gap = t_gap.zip(t_back).map(|(g, b)| {
            let q_gap = o.max_wait_between(0, g);
            o.max_wait_between(g + q_gap + SEC, b)
        });
        check("ölçümsüz_dizi≥3", run_len >= 3, format!("{run_len}"));
        check(
            "boşluk_beklemesi≤0.70s",
            w_gap.is_some_and(|w| w <= gap_wait_bound),
            opt_secs(w_gap),
        );
    }
    if cell == Cell::D2 {
        check(
            "taşma_oldu",
            o.drops_queue > 0,
            format!("{}", o.drops_queue),
        );
    }
    if cell != Cell::D2 {
        check(
            "yakınsama≤sınır",
            conv.is_some_and(|c| c <= conv_bound),
            format!("{} (sınır {})", opt_secs(conv), secs(conv_bound)),
        );
    } else {
        check("yakınsadı", conv.is_some(), opt_secs(conv));
    }
    if cell == Cell::B {
        let end = o.rate_at_end();
        check(
            "son_tahmin≤1.1×6250",
            end.is_some_and(|e| e * 10 <= DROP_TO * 11),
            format!("{end:?}"),
        );
    }
    if cell == Cell::D2 {
        let fifo_ns = 64 * (VOICE_UNIT + OVH) * SEC / DROP_TO;
        check(
            "bekleme(yak+2s)<fifo",
            w2.is_some_and(|w| w < fifo_ns),
            format!("{} (sınır <{})", opt_secs(w2), secs(fifo_ns)),
        );
    } else {
        check(
            "bekleme(yak+2s)≤2dg",
            w2.is_some_and(|w| w <= wait_bound),
            format!("{} (sınır {})", opt_secs(w2), secs(wait_bound)),
        );
    }
    if cell == Cell::D4 {
        check("düşen(yak+2s)=0", drops2 == Some(0), format!("{drops2:?}"));
    }
    // Atlamalı PTT ölçütü.
    let planned = |k: u64| VOICE_START_NS + k * VOICE_PERIOD_NS;
    let delays: Vec<(u64, Option<u64>)> = o
        .voice_first
        .iter()
        .enumerate()
        .map(|(k, a)| (planned(k as u64), a.map(|a| a - planned(k as u64))))
        .collect();
    let on_time = |from: u64| {
        let w: Vec<_> = delays.iter().filter(|&&(p, _)| p >= from).collect();
        let n = w
            .iter()
            .filter(|&&&(_, d)| d.is_some_and(|d| d <= PTT_DEADLINE_NS))
            .count();
        (n, w.len())
    };
    let (n_all, tot_all) = on_time(0);
    let worst_on_time = delays
        .iter()
        .filter_map(|&(_, d)| d.filter(|&d| d <= PTT_DEADLINE_NS))
        .max();
    let (n_conv, tot_conv) = conv.map_or((0, 0), on_time);
    let (n_conv9, tot_conv9) = conv.map_or((0, 0), |c| on_time(c + 9 * SEC));
    let bps = o.ptt_wire_bytes * 8 * SEC / (sc.voice_until_ns - VOICE_START_NS);
    let retx = o
        .ptt_tx
        .iter()
        .filter(|(_, k)| *k != TransmitKind::Data)
        .count();
    // Var olan testin PTT ölçütü (sıralı teslim, yakınsamadan 9 s sonra ≤ 1 s); yalnız yan bilgi.
    let ordered = conv.map(|c| o.voice_max_from(c + 9 * SEC));
    println!(
        "ÖLÇÜM {cell:?} tohum {seed} {load:?}: HAT[{}]{guard} | t_ack {} t_ret {} kuyrukta_düşen \
         toplam {} (yak+2s {}) | PTT zamanında {n_all}/{tot_all} en_kötü_zamanında {} \
         yak≥ {n_conv}/{tot_conv} yak+9s≥ {n_conv9}/{tot_conv9} tel {bps} bit/s iletim {} \
         (Data dışı {retx}) bırakılan {} | sıralı_teslim(yak+9s) en kötü {}",
        yn(pass),
        secs(t_ack),
        secs(t_ret),
        o.drops_queue,
        drops2.map_or("-".to_string(), |d| d.to_string()),
        opt_secs(worst_on_time),
        o.ptt_tx.len(),
        o.ptt_expired.len(),
        ordered.map_or("-".to_string(), secs),
    );
    if load == Load::Lean {
        // Ölçü aracının sınaması: her PTT iletimi yalın birimin tel boyunda.
        assert_eq!(
            o.ptt_wire_bytes,
            o.ptt_tx.len() as u64 * (LEAN_PTT_UNIT + OVH),
            "PTT iletim boyu"
        );
    }
    pass
}

/// İddiasız ölçüm: senaryo × tohum × yük; hat koruması ölçütlerinin geçip geçmediğini ve PTT
/// ölçüsünü yazdırır. Yalnız ölçü aracının kendi sınaması (yalın kolda PTT iletim boyu) assert eder.
#[test]
fn lean_load_line_guard_measurement() {
    for cell in [Cell::B, Cell::D2, Cell::E2, Cell::D4] {
        for seed in 1..=3u64 {
            let c = measure(cell, seed, Load::Copies);
            let l = measure(cell, seed, Load::Lean);
            println!(
                "ÖZET {cell:?} tohum {seed}: hat koruması kopya {} yalın {}",
                yn(c),
                yn(l)
            );
        }
    }
}
