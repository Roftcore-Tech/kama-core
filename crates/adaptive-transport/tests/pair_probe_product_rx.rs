//! [ÇİFT YOKLAMASI · DENEY × YÜKSELİŞ KANITI · K1] Kapalı döngü dar test: göndericinin hat koruması
//! (`LineGuard`, çift yoklaması deneyi: 6 datagram, tek yoklama) ile ÜRÜN alıcısı (`Receiver`,
//! bitmap ACK, `alarm_pair_carry` kuralı) aynı döngüde.
//!
//! Benzetim/birim testidir; kabul değildir. Gerçek hat (netem) koşusunun yerini tutmaz. Testler
//! bugünkü davranışı BELGELER: iddialar gözlenen sonuçtur; ön kayıtla çelişen sonuç adında
//! `on_kayitla_celisti` taşıyan ayrı testte durur. Hüküm kapsamadan (aralığın uçlarının GÖNDERİM
//! anları ↔ yoklama penceresi) okunur; ACK'in türü yalnız bilgi olarak basılır.
//!
//! # Ürünle aynı / farklı
//!
//! | Parça | Düzenek | Ürün | Durum |
//! |---|---|---|---|
//! | Hat koruması | `LineGuard::new(hat, dönüş, conservative_start)`, `set_pair_probe` | `Sender::set_cc_feedback_starved` | AYNI kod |
//! | Hat modeli / çift kipi | `pacer::TokenBucket` (`set_line`, `set_line_rate`, `set_line_pairs(g.pair_pacing())` tick'te ve ACK'te; kural `try_send` içinde) | `Sender::on_event_core`, `cc_line_guard_on_ack` | AYNI kod, aynı çağrı sırası |
//! | ACK'in göndericide işlenişi | `note_ack_wire` → `on_first_window` → `note_pair_lower_bound` → `on_ack_measured` → `on_pair_rate` → `settle_start_budget` → hat modeli (hız, çift kipi, tutma, boşluk boşaltması) | `client_core::runtime` 0x15 kolu + `cc_line_guard_on_ack` | AYNI sıra |
//! | Öncelikli şerit | metin datagramı tam boydan önce, aynı kovadan (`try_send`), `on_sent_classed(.., priority = true)`; başlangıç bütçesini beklemez | `Sender::start_budget_holds`, şerit sırası | AYNI kural |
//! | Alıcı | `Receiver::with_cc_port`, `set_return_bandwidth(62)`, `set_ack_bitmap(true)`, `set_pair_squeeze_line(12 500)`; her varışta `set_next_datagram_bytes` | sunucu yuvası (CAPS kolu) | AYNI kod |
//! | Yukarı yön yol verme | dönüş hattının doluluğu tek sayı (`busy_until`); hatta çıkan HER datagram (ACK, alındı) `tel / 62 B/s` yazar; alıcıya her `on_datagram` ve her ACK tikinden önce `set_return_line_ahead_ns(busy − şimdi)` ve `set_uplink_yield(dar hat ∧ busy > şimdi)` | `runtime::UplinkShare` (`charge`, `ns_until_clear`, `control_should_yield`), `srv_ack_tick`, veri alım kolu | AYNI anlam |
//! | ACK tiki | 20 ms | `runtime::ACK_TICK` | AYNI |
//! | Dar hat sınaması | `Sender::line_is_narrow(62 B/s)` | `UplinkShare::for_config` | AYNI kod |
//! | Gönderici motoru | yalnız `LineGuard` + hat modeli; kova hızı sabit profil hattı, burst 10 × MTU | `Sender`: BBR, uçuş penceresi, PTO, ARQ, FEC, PARAM/CAPS | FARKLI |
//! | Yük | sürekli tam boy (1 046 B gövde) + 2 s'de bir metin: 1 orijinal + 4 kopya, 108 B gövde (136 B IP), kopyalar 350 / 450 / 500 / 400 ms arayla (sabit takvim) | şerit katmanı, kopya kuralı (`forward_copy_rule`) | FARKLI (takvim sabit) |
//! | Dönüş yükü | metnin ilk varışında bir alındı (71 B IP); hat boşalınca çıkar, ACK'ten önce denenir; göndericide işlenmez | ters oturumun `Sender`'ı (kendi hat modeli, yinelenen alındı, PARAM_ACK, RECOVERED) | FARKLI (tek alındı, başka denetim yok) |
//! | Darboğaz | tek FIFO; ESAS kol jeton kovalı (burst 1 200 B), teşhis kolu saf serileştirici; sonlu kuyruk 64 paket (dolu ise girişte düşer) | netem `tbf` + `limit` | FARKLI (model) |
//! | Kayıp | ayrı hücrede darboğazdan SONRA ‰ 200, karar `splitmix64(tohum, seq)` (RNG yok; `starved_line_sim.rs` anahtarlı kipiyle aynı karma ailesi) | netem `loss 20%` | FARKLI (belirlenimli; yeniden iletim yok) |
//! | Toplu teslim | darboğazdan sonra tek tam datagram `hold_ns` tutulur (sıra değişmez) | ana makine takılması | FARKLI (enjekte) |
//! | Zaman | 1 ms adım; varış anı ns çözünürlükte alıcıya verilir | olay güdümlü | FARKLI |
//! | ACK teli | çerçeve + 50 B (`ACK_DATAGRAM_ENVELOPE_BYTES`: 22 B mühür + 28 B UDP/IP); PTT-yalın biti verilmez (0x15) | mühürlü boy, şeritli oturumda 0x16 | FARKLI (sabit zarf) |
//! | Dönüş hattı | 62 B/s serileşme + 300 ms yayılım; kuyruk sınırsız, kayıpsız | netem | FARKLI |
//!
//! # Yoklama penceresi nasıl okunur
//!
//! `LineGuard` yoklamanın `start_ns` / `until_ns` değerini dışarı vermez (`src/` bu işte değişmez).
//! Pencere iki bağımsız yoldan kurulur ve birbirine `assert` ile bağlanır:
//! * formül (`pair_probe_target`): `start = ACK anı + tutma / taban`, tasarım bitişi
//!   `start + 6 × 1 228 B / hedef`; tutma `ack_diag` (`backlog_bytes + extra_backlog_bytes`);
//! * gözlem: `LineGuard::tick` tavanı `start`'ı izleyen ilk 1 ms adımda hedefe açar, süre dolunca
//!   ya da bütçeye bir tam datagram daha sığmayınca tabana indirir. Bütçeyle erken bitişte kod
//!   `until`'i o adıma çeker; düzenek de öyle okur.
//!
//! Fiilî gönderim hükmün kaydından (`ack_diag().verdict`: `probe_sent_datagrams`,
//! `probe_sent_wire`) okunur ve düzeneğin `[start, until)` içindeki kendi sayımıyla sağlanır.
//!
//! # Kuyruk ölçüsü ve 393 ms kapısı
//!
//! İki tanım yan yana:
//! * **öndeki bekleme (kapının tanımı)** = datagram darboğaza girdiği anda kuyrukta duran tel baytı
//!   (henüz çıkmamış bütün datagramlar) / o andaki hat hızı; kendi serileşmesi hariç;
//! * **toplam kalış** = giriş → çıkış.
//!
//! Kapı `2 × (1 200 + 28) B / 6 250 B/s` = 392,96 ms.

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use adaptive_transport::ack::{
    AckFrame, ACK_DATAGRAM_ENVELOPE_BYTES, ACK_DELAY_UNIT_US, PAIR_MIN_UDP_BYTES, PAIR_RATE_NONE,
    RX_SPAN_NONE,
};
use adaptive_transport::arrival_rate::{
    DROP_MIN_ARRIVALS, DROP_PERMILLE, DROP_REF_HOLD_NS, DROP_WINDOW_NS,
};
use adaptive_transport::line_guard::{
    AckDiag, ForwardInterval, LineGuard, ProbeOutcome, ProbeVerdictDiag, PAIR_PROBE_MAX_DATAGRAMS,
};
use adaptive_transport::loss::E2A_ACK_DELAY_SATURATED;
use adaptive_transport::pacer::{LineModel, TokenBucket};
use adaptive_transport::rate::DEFAULT_MTU;
use adaptive_transport::receiver::{AckEmitObs, Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::Sender;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// Tam boy datagram: UDP gövdesi ve tel boyu (B).
const BODY: u64 = 1_046;
const OVH: u64 = 28;
const WIRE: u64 = BODY + OVH;
/// Metin datagramı: 108 B gövde = 136 B IP.
const SMALL_BODY: u64 = 108;
/// Metin: 2 s'de bir; 1 orijinal + 4 kopya; kopyaların orijinale göre anları (ms).
const TEXT_PERIOD: u64 = 2 * SEC;
const COPY_OFFSETS_MS: [u64; 5] = [0, 350, 800, 1_300, 1_700];
/// Metin alındısı (tel, B) ve alıcının ACK tiki.
const RECEIPT_WIRE: u64 = 71;
const ACK_TICK: u64 = 20 * MS;
/// Profil hattı ve dar hat (B/s, tel).
const PROFILE: u64 = 12_500;
const NARROW: u64 = 6_250;
const RETURN_BPS: u64 = 62;
const ONE_WAY: u64 = 300 * MS;
/// Deneyin datagram bütçesi (sahip kararı: 6 datagram, tek yoklama).
const PROBE_DATAGRAMS: u64 = PAIR_PROBE_MAX_DATAGRAMS;
const MTU_WIRE: u64 = DEFAULT_MTU + OVH;
/// 393 ms kapısı: iki tam datagramın 6 250 B/s'de serileşmesi (ns).
const GATE_NS: u64 = 2 * MTU_WIRE * SEC / NARROW;
/// Kovalı darboğazın burst'ü (B) ve sonlu kuyruk (paket).
const BUCKET: u64 = 1_200;
const QUEUE_PKTS: usize = 64;
/// Kayıplı hücre: darboğazdan sonra ‰ 200, tohum 1.
const LOSS_PERMILLE: u64 = 200;
const LOSS_SEED: u64 = 1;
/// Hedef 4'ün eşiği: tavan ≥ profil hattının %95'i (B/s).
const TARGET4: u64 = 11_875;
/// Ölçülmüş dar hattın üst bandı (B/s): bunun üstündeki tavan / alt sınır "yüksek"tir.
const NARROW_BAND: u64 = 6_875;

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Darboğazdan sonra anahtarlı kayıp: tohum ve seq'in saf işlevi (RNG yok).
fn keyed_lost(seq: u64, permille: u64) -> bool {
    permille > 0 && splitmix64(splitmix64(LOSS_SEED) ^ seq) % 1000 < permille
}

/// Tek FIFO darboğaz; `burst` > 0 ise jeton kovalı. Kredi birimi bayt × 10⁹. Sonlu kuyruk.
struct Bottleneck {
    plan: Vec<(u64, u64)>,
    burst: u64,
    /// `true`: baştaki datagramın hızı başa geçtiği anda sabitlenir (hız değişimi serileşmekte olan
    /// datagrama yansımaz); `false`: hız değişimi serileşen datagrama da yansır (tümlev).
    fixed_at_head: bool,
    credit: u128,
    /// Kredinin geçerli olduğu an = son çıkış anı.
    at: u64,
    /// Henüz çıkmamış datagramlar: `(çıkış anı, tel)`.
    queue: VecDeque<(u64, u64)>,
    cap_pkts: usize,
    max_occ_pkts: usize,
}

impl Bottleneck {
    fn rate_at(&self, t: u64) -> (u64, u64) {
        let i = self
            .plan
            .iter()
            .rposition(|&(from, _)| t >= from)
            .unwrap_or(0);
        let end = self.plan.get(i + 1).map_or(u64::MAX, |&(from, _)| from);
        (self.plan[i].1, end)
    }

    /// `[from, to)` içinde hattın geçirdiği bayt × 10⁹.
    fn served(&self, mut from: u64, to: u64) -> u128 {
        let mut sum = 0u128;
        while from < to {
            let (rate, end) = self.rate_at(from);
            let stop = end.min(to);
            sum += u128::from(rate) * u128::from(stop - from);
            from = stop;
        }
        sum
    }

    /// `from`'dan sonra hattın `need` (bayt × 10⁹) geçirdiği ilk an.
    fn when(&self, mut from: u64, mut need: u128) -> u64 {
        loop {
            let (rate, end) = self.rate_at(from);
            let cap = u128::from(rate) * u128::from(end - from);
            if cap >= need {
                let dt = need.div_ceil(u128::from(rate));
                return from + u64::try_from(dt).expect("süre");
            }
            need -= cap;
            from = end;
        }
    }

    /// Datagram `t` anında kuyruğa girer: `(kuyruğun başına geçtiği an, çıkış anı)`.
    fn enqueue(&mut self, t: u64, wire: u64) -> (u64, u64) {
        let head = t.max(self.at);
        if head > self.at {
            // Kuyruk boş kaldı: kredi `burst`'e dek birikir.
            let cap = u128::from(self.burst) * u128::from(SEC);
            self.credit = (self.credit + self.served(self.at, head)).min(cap);
        }
        let need = u128::from(wire) * u128::from(SEC);
        let depart = if self.credit >= need {
            head
        } else if self.fixed_at_head {
            let rate = u128::from(self.rate_at(head).0);
            head + u64::try_from((need - self.credit).div_ceil(rate)).expect("süre")
        } else {
            self.when(head, need - self.credit)
        };
        self.credit = if depart == head {
            self.credit - need
        } else if self.fixed_at_head {
            0
        } else {
            self.credit + self.served(head, depart) - need
        };
        self.at = depart;
        (head, depart)
    }

    /// Datagram `t` anında sunulur. Kuyruk doluysa `None` (girişte düşer); değilse
    /// `(girişte önündeki tel baytı, başa geçiş, çıkış)`.
    fn offer(&mut self, t: u64, wire: u64) -> Option<(u64, u64, u64)> {
        while self.queue.front().is_some_and(|&(d, _)| d <= t) {
            self.queue.pop_front();
        }
        if self.queue.len() >= self.cap_pkts {
            return None;
        }
        let ahead: u64 = self.queue.iter().map(|&(_, w)| w).sum();
        let (head, depart) = self.enqueue(t, wire);
        self.queue.push_back((depart, wire));
        self.max_occ_pkts = self.max_occ_pkts.max(self.queue.len());
        Some((ahead, head, depart))
    }
}

/// Darboğazdan sonra toplu teslim: çıkışı `from_ns`'ten sonraki ilk TAM datagram `hold_ns` tutulur.
#[derive(Debug, Clone, Copy)]
struct Batch {
    from_ns: u64,
    hold_ns: u64,
}

#[derive(Debug, Clone)]
struct Scn {
    plan: Vec<(u64, u64)>,
    burst: u64,
    fixed_at_head: bool,
    queue_pkts: usize,
    /// Darboğazdan sonra anahtarlı kayıp (‰).
    loss_permille: u64,
    /// `LineGuard::set_pair_probe` (deney kolu); `None` = bugünkü doğrudan yol.
    probe: Option<u64>,
    until: u64,
    batch: Option<Batch>,
    /// Metin + kopya yükü ve takvimin fazı (ns).
    texts: bool,
    text_phase: u64,
    /// Dönüş hattında metin alındısı.
    receipts: bool,
    /// Ürünün yukarı yön yol verme kuralı (`false` yalnız teşhis kolu).
    yield_on: bool,
}

impl Scn {
    /// Ürün ayarı: kovalı darboğaz, 64 paket kuyruk, kayıpsız, metin + alındı + yol verme açık.
    fn product(plan: Vec<(u64, u64)>, probe: Option<u64>, until: u64) -> Self {
        Self {
            plan,
            burst: BUCKET,
            fixed_at_head: true,
            queue_pkts: QUEUE_PKTS,
            loss_permille: 0,
            probe,
            until,
            batch: None,
            texts: true,
            text_phase: 0,
            receipts: true,
            yield_on: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// Alıcıya varış anı (ns).
    Delivered(u64),
    /// Dolu kuyrukta girişte düştü.
    QueueDrop,
    /// Darboğazdan sonra kayıp.
    Lost,
}

#[derive(Debug, Clone, Copy)]
struct Sent {
    send_ns: u64,
    body: u64,
    /// Metin datagramıysa metnin kimliği.
    text: Option<u64>,
    /// Kapının tanımıyla öndeki bekleme (ns): girişte kuyrukta duran tel baytı / hat hızı.
    ahead_ns: u64,
    depart_ns: u64,
    fate: Fate,
}

/// Alıcının daralma dedektörünün (`arrival_rate::RateDropDetector`) bir algılaması: tetikleyen
/// varış ve o andaki pencere içeriği.
#[derive(Debug, Clone, Copy)]
struct Detect {
    at_ns: u64,
    seq: u64,
    /// Penceredeki varış sayısı (ve içindeki metin datagramı sayısı), ilk varışın seq'i, süresi
    /// (ns), hızı (B/s gövde).
    n: usize,
    small_n: usize,
    first_seq: u64,
    span_ns: u64,
    rate: u64,
    /// Referans (B/s gövde) ve referansı son kuran varışın seq'i.
    reference: u64,
    ref_seq: u64,
}

/// `RateDropDetector`'ın düzenekteki gölgesi (aynı sabitler, aynı sıra). Her varışta ürün
/// alıcısının sayacıyla (`diag_drop_detects`) karşılaştırılır; ayrışırsa test durur.
#[derive(Default)]
struct DetectorShadow {
    ring: VecDeque<(u64, u64, u64)>,
    reference_bps: u64,
    reference_ns: u64,
    ref_seq: u64,
    fired: bool,
}

impl DetectorShadow {
    fn on_arrival(&mut self, now: u64, body: u64, seq: u64) -> Option<Detect> {
        self.ring.push_back((now, body, seq));
        while self
            .ring
            .front()
            .is_some_and(|&(t, _, _)| t.saturating_add(DROP_WINDOW_NS) < now)
        {
            self.ring.pop_front();
        }
        if self.ring.len() < DROP_MIN_ARRIVALS {
            return None;
        }
        let (first, _, first_seq) = *self.ring.front()?;
        let span = now.saturating_sub(first);
        if span < DROP_WINDOW_NS / 2 {
            return None;
        }
        let n = self.ring.len() as u64;
        let mean_gap = span / (n - 1);
        let max_gap = self
            .ring
            .iter()
            .zip(self.ring.iter().skip(1))
            .map(|(a, b)| b.0.saturating_sub(a.0))
            .max()
            .unwrap_or(0);
        if max_gap.saturating_mul(2) > mean_gap.saturating_mul(5) {
            return None;
        }
        let bytes: u64 = self.ring.iter().skip(1).map(|&(_, b, _)| b).sum();
        let rate = u64::try_from(u128::from(bytes) * u128::from(SEC) / u128::from(span.max(1)))
            .unwrap_or(u64::MAX);
        if rate >= self.reference_bps || now.saturating_sub(self.reference_ns) > DROP_REF_HOLD_NS {
            if rate >= self.reference_bps || !self.fired {
                self.reference_bps = rate;
                self.reference_ns = now;
                self.ref_seq = seq;
            }
            return None;
        }
        if !self.fired
            && rate.saturating_mul(1000) < self.reference_bps.saturating_mul(DROP_PERMILLE)
        {
            self.fired = true;
            return Some(Detect {
                at_ns: now,
                seq,
                n: self.ring.len(),
                small_n: self
                    .ring
                    .iter()
                    .filter(|x| x.1 < PAIR_MIN_UDP_BYTES)
                    .count(),
                first_seq,
                span_ns: span,
                rate,
                reference: self.reference_bps,
                ref_seq: self.ref_seq,
            });
        }
        None
    }

    fn on_ack(&mut self, now: u64) {
        if self.fired {
            self.fired = false;
            self.reference_bps = 0;
            self.reference_ns = now;
        }
    }
}

/// Çift yoklamasının başlatıldığı ACK'te formülden kurulan plan.
#[derive(Debug, Clone, Copy)]
struct ProbePlan {
    start: u64,
    design_until: u64,
    base: u64,
    target: u64,
}

/// Göndericinin bir ACK'i işleyişi.
#[derive(Debug, Clone)]
struct Row {
    n: usize,
    emit_ns: u64,
    proc_ns: u64,
    obs: AckEmitObs,
    largest: u64,
    pair: Option<u16>,
    interval: Option<ForwardInterval>,
    before: u64,
    after: u64,
    rate_after: Option<u64>,
    diag: AckDiag,
    /// Bu ACK'te aşağı yönlü örnek / taşma örneği alındı mı; aralığın kuyruk artışı `g` (ns).
    sample: bool,
    overflow: bool,
    g_ns: Option<i64>,
    /// Bu ACK çift yoklaması başlattıysa planı.
    plan: Option<ProbePlan>,
    pair_probes: u64,
}

impl Row {
    /// ACK'in türü: yalnız bilgi (hüküm türden çıkarılmaz).
    fn kind(&self) -> &'static str {
        match (
            self.obs.evidence_ns.is_some(),
            self.obs.alarm,
            self.obs.rise,
        ) {
            (true, _, true) => "kanıtlı daralma + yükseliş",
            (true, _, false) => "kanıtlı daralma",
            (false, true, _) => "yükseliş alarmı",
            (false, false, _) if self.obs.wait_open => "planlı (bekleme açık)",
            _ => "planlı",
        }
    }

    fn pair_verdict(&self) -> Option<ProbeVerdictDiag> {
        self.diag.verdict.filter(|v| v.from_pair)
    }
}

struct Out {
    sent: Vec<Sent>,
    /// Alıcıya varışlar, sırayla: `(an, seq, gövde)`.
    rx: Vec<(u64, u64, u64)>,
    rows: Vec<Row>,
    /// `LineGuard::tick`'in tavanı değiştirdiği adımlar: `(an, önce, sonra)`.
    ticks: Vec<(u64, u64, u64)>,
    /// `(an, tavan)`: her 1 ms adımın sonunda tavan değiştiyse.
    pace: Vec<(u64, u64)>,
    /// Alıcıda yükseliş alarmı ve kanıtlı daralma anları; daralma algılamaları (pencere içeriğiyle).
    rise_at: Vec<u64>,
    drop_at: Vec<u64>,
    detects: Vec<Detect>,
    /// Toplu teslim: `(tutulan seq, ardındaki seq, alıcıda aralık ns)`.
    batched: Option<(u64, u64, u64)>,
    /// Aynı 1 ms adımda art arda çıkan datagram çifti sayısı (çift kipinin fiilî izi).
    paired_sends: u64,
    receipts_sent: u64,
    /// Alıcının yol verdiği için o an çıkarmadığı ACK denemesi.
    yield_deferred: u64,
    queue_drops: u64,
    lost: u64,
    max_occ_pkts: usize,
    probe_failures: u64,
    samples: u64,
    overflow_samples: u64,
    pair_probes: u64,
    end_ns: u64,
}

impl Out {
    fn send_ns(&self, seq: u64) -> u64 {
        self.sent[usize::try_from(seq).expect("seq")].send_ns
    }

    /// `[from, to)` içinde gönderilen `(datagram sayısı, tel baytı, metin datagramı sayısı)`.
    fn sent_between(&self, from: u64, to: u64) -> (u64, u64, u64) {
        self.sent
            .iter()
            .filter(|x| x.send_ns >= from && x.send_ns < to)
            .fold((0, 0, 0), |(n, w, s), x| {
                (n + 1, w + x.body + OVH, s + u64::from(x.text.is_some()))
            })
    }

    /// `[from, to)` içinde darboğaza girenlerin en büyük `(öndeki bekleme, toplam kalış)` (ns).
    fn queue_peak(&self, from: u64, to: u64) -> (u64, u64) {
        self.sent
            .iter()
            .filter(|x| x.send_ns >= from && x.send_ns < to && x.fate != Fate::QueueDrop)
            .fold((0, 0), |(a, s), x| {
                (a.max(x.ahead_ns), s.max(x.depart_ns - x.send_ns))
            })
    }

    /// Tavanın `[from, to)` içinde `level`'ın üstünde kaldığı toplam süre (ns) ve en büyük değeri.
    fn pace_above(&self, level: u64, from: u64, to: u64) -> (u64, u64) {
        let mut total = 0u64;
        let mut peak = 0u64;
        for (i, &(t, p)) in self.pace.iter().enumerate() {
            let end = self.pace.get(i + 1).map_or(to, |&(t2, _)| t2).min(to);
            let start = t.max(from);
            if p > level && end > start {
                total += end - start;
                peak = peak.max(p);
            }
        }
        (total, peak)
    }

    /// Tavanın koşu sonuna dek `level`'ın altına inmediği ilk an.
    fn permanent_since(&self, level: u64) -> Option<u64> {
        self.pace
            .iter()
            .rev()
            .take_while(|&&(_, p)| p >= level)
            .last()
            .map(|&(t, _)| t)
    }

    /// Çift yoklamasını başlatan ACK'in satır dizini.
    fn probe_start(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.plan.is_some())
    }
}

struct InFlightAck {
    reach_ns: u64,
    emit_ns: u64,
    frame: AckFrame,
    obs: AckEmitObs,
}

/// Dönüş hattı: cihazın yukarı yönünün paylaşılan doluluğu (`runtime::UplinkShare` aynası).
struct ReturnLine {
    busy_until: u64,
    narrow: bool,
    yield_on: bool,
}

impl ReturnLine {
    fn ahead(&self, now: u64) -> u64 {
        self.busy_until.saturating_sub(now)
    }

    /// `UplinkShare::control_should_yield`.
    fn should_yield(&self, now: u64) -> bool {
        self.yield_on && self.narrow && self.busy_until > now
    }

    /// `UplinkShare::charge`: datagram hatta serileşir; dönen değer serileşmenin bittiği an.
    fn charge(&mut self, now: u64, wire: u64) -> u64 {
        self.busy_until = self.busy_until.max(now) + wire * SEC / RETURN_BPS;
        self.busy_until
    }
}

#[allow(clippy::too_many_lines)]
fn simulate(s: &Scn) -> Out {
    let line = LineModel {
        rate_bps: PROFILE,
        datagram_overhead: OVH,
    };
    let ret_model = LineModel {
        rate_bps: RETURN_BPS,
        datagram_overhead: OVH,
    };
    // `Sender::set_cc_feedback_starved` ile aynı kuruluş.
    let mut g = LineGuard::new(line, ret_model, LineGuard::conservative_start(line));
    g.set_pair_probe(s.probe);
    let mut bucket = TokenBucket::new(PROFILE, 10 * DEFAULT_MTU);
    bucket.set_line(Some(line));
    bucket.set_line_rate(g.pace_bps());

    let mut rx = Receiver::with_cc_port();
    rx.set_return_bandwidth(Some(RETURN_BPS));
    rx.set_ack_bitmap(true);
    rx.set_pair_squeeze_line(Some(PROFILE));

    let mut bn = Bottleneck {
        plan: s.plan.clone(),
        burst: s.burst,
        fixed_at_head: s.fixed_at_head,
        credit: u128::from(s.burst) * u128::from(SEC),
        at: 0,
        queue: VecDeque::new(),
        cap_pkts: s.queue_pkts,
        max_occ_pkts: 0,
    };
    let mut ret = ReturnLine {
        busy_until: 0,
        narrow: Sender::line_is_narrow(&ret_model),
        yield_on: s.yield_on,
    };
    assert!(ret.narrow, "62 B/s dönüş hattı dar sayılmalı");
    let mut out = Out {
        sent: Vec::new(),
        rx: Vec::new(),
        rows: Vec::new(),
        ticks: Vec::new(),
        pace: vec![(0, g.pace_bps())],
        rise_at: Vec::new(),
        drop_at: Vec::new(),
        detects: Vec::new(),
        batched: None,
        paired_sends: 0,
        receipts_sent: 0,
        yield_deferred: 0,
        queue_drops: 0,
        lost: 0,
        max_occ_pkts: 0,
        probe_failures: 0,
        samples: 0,
        overflow_samples: 0,
        pair_probes: 0,
        end_ns: s.until,
    };
    let mut det = DetectorShadow::default();
    let mut flight: VecDeque<(u64, u64)> = VecDeque::new();
    let mut acks: VecDeque<InFlightAck> = VecDeque::new();
    let mut receipts_due = 0u64;
    let mut texts_seen: BTreeSet<u64> = BTreeSet::new();
    let mut batch = s.batch;
    let mut held: Option<u64> = None;
    let mut last_arrive = 0u64;
    let mut last_rx: Option<u64> = None;
    let (mut rises, mut detects, mut drops) = (0u64, 0u64, 0u64);
    let mut acks_emitted = 0u64;
    let mut smalls: VecDeque<u64> = VecDeque::new();
    let mut text_events = 0u64;
    let mut seq = 0u64;
    let mut now = 0u64;

    while now < s.until {
        // --- Alıcı ucu. Veri önce: bekleyen alındı, hat boşsa çıkar.
        if receipts_due > 0 && ret.busy_until <= now {
            receipts_due -= 1;
            ret.charge(now, RECEIPT_WIRE);
            out.receipts_sent += 1;
        }
        // Bu adıma dek varanlar (her çağrıdan önce dönüş hattının durumu verilir).
        while flight.front().is_some_and(|&(t, _)| t <= now) {
            let (t, q) = flight.pop_front().expect("var");
            let rec = out.sent[usize::try_from(q).expect("seq")];
            let gap = last_rx.map_or(0, |p| t - p);
            last_rx = Some(t);
            // Tutulan datagramın ardındaki varış: toplu teslimin alıcıdaki aralığı.
            if held.is_some_and(|h| out.rx.last().is_some_and(|&(_, p, _)| p == h)) {
                out.batched = held.take().map(|h| (h, q, gap));
            }
            out.rx.push((t, q, rec.body));
            rx.set_return_line_ahead_ns(ret.ahead(t));
            rx.set_uplink_yield(ret.should_yield(t));
            rx.set_next_datagram_bytes(rec.body);
            // Dedektör yalnız ilk ACK'ten sonra beslenir (`ack_budget_first_done`).
            let shadow = if acks_emitted > 0 {
                det.on_arrival(t, rec.body, q)
            } else {
                None
            };
            let o = rx.on_datagram(
                RxDatagram {
                    seq: q,
                    unit_id: rec.text.unwrap_or((1 << 32) | q),
                    kind: RxKind::Data,
                },
                t,
            );
            if rx.diag_rise_alarms() > rises {
                rises = rx.diag_rise_alarms();
                out.rise_at.push(t);
            }
            let fired = rx.diag_drop_detects() > detects;
            detects = rx.diag_drop_detects();
            assert_eq!(
                shadow.is_some(),
                fired,
                "dedektör gölgesi üründen ayrıştı ({} s, seq {q})",
                s3(t)
            );
            out.detects.extend(shadow);
            if rx.diag_drop_alarms() > drops {
                drops = rx.diag_drop_alarms();
                out.drop_at.push(t);
            }
            if let Some(id) = rec.text {
                if texts_seen.insert(id) && s.receipts {
                    receipts_due += 1;
                }
            }
            if let Some(f) = o.ack {
                push_ack(&rx, &mut acks, &mut ret, t, f);
                det.on_ack(t);
                acks_emitted += 1;
            }
        }
        // ACK tiki (`runtime::ACK_TICK`, `srv_ack_tick`).
        if now.is_multiple_of(ACK_TICK) {
            rx.set_return_line_ahead_ns(ret.ahead(now));
            rx.set_uplink_yield(ret.should_yield(now));
            if let Some(f) = rx.on_ack_timer(now) {
                push_ack(&rx, &mut acks, &mut ret, now, f);
                det.on_ack(now);
                acks_emitted += 1;
            }
        }

        // --- Gönderici: `Sender::on_event_core` sırası (dolum → tick → ACK → iletim).
        bucket.refill(Duration::from_nanos(MS));
        let before_tick = g.pace_bps();
        if g.tick(now) {
            bucket.set_line_rate(g.pace_bps());
            bucket.set_line_pairs(g.pair_pacing());
            out.ticks.push((now, before_tick, g.pace_bps()));
        }
        while acks.front().is_some_and(|a| a.reach_ns <= now) {
            let a = acks.pop_front().expect("var");
            let row = on_ack(&mut g, &mut bucket, &a, now, seq, out.rows.len() + 1);
            out.rows.push(row);
        }
        // Metin takvimi: n. olay = metin n / 5'in kopya n % 5'i.
        let text_due = |n: u64| {
            let j = usize::try_from(n % 5).expect("kopya");
            s.text_phase + (n / 5) * TEXT_PERIOD + COPY_OFFSETS_MS[j] * MS
        };
        while s.texts && text_due(text_events) <= now {
            smalls.push_back(text_events / 5);
            text_events += 1;
        }
        let mut sent_this_step = 0u64;
        loop {
            // Öncelikli şerit (metin) tam boydan önce; başlangıç bütçesini beklemez.
            let text = smalls.front().copied();
            let body = if text.is_some() { SMALL_BODY } else { BODY };
            if text.is_none() && g.start_budget_active() && !g.start_budget_admits(now, body) {
                break;
            }
            if !bucket.try_send(body) {
                break;
            }
            if text.is_some() {
                smalls.pop_front();
            }
            g.on_sent_classed(
                seq,
                now,
                body,
                bucket.line_ns_until_drained(),
                text.is_some(),
            );
            sent_this_step += 1;
            if sent_this_step == 2 {
                out.paired_sends += 1;
            }
            let wire = body + OVH;
            let mut rec = Sent {
                send_ns: now,
                body,
                text,
                ahead_ns: 0,
                depart_ns: now,
                fate: Fate::QueueDrop,
            };
            if let Some((ahead, _head, depart_ns)) = bn.offer(now, wire) {
                rec.ahead_ns = ahead * SEC / bn.rate_at(now).0;
                rec.depart_ns = depart_ns;
                if keyed_lost(seq, s.loss_permille) {
                    rec.fate = Fate::Lost;
                    out.lost += 1;
                } else {
                    let mut arrive_ns = depart_ns + ONE_WAY;
                    if text.is_none() {
                        if let Some(bt) = batch.filter(|bt| depart_ns >= bt.from_ns) {
                            arrive_ns += bt.hold_ns;
                            held = Some(seq);
                            batch = None;
                        }
                    }
                    // Teslim sırası korunur (toplu teslim sıra değiştirmez).
                    arrive_ns = arrive_ns.max(last_arrive + 1);
                    last_arrive = arrive_ns;
                    flight.push_back((arrive_ns, seq));
                    rec.fate = Fate::Delivered(arrive_ns);
                }
            } else {
                out.queue_drops += 1;
            }
            out.sent.push(rec);
            seq += 1;
        }
        if out.pace.last().is_some_and(|&(_, p)| p != g.pace_bps()) {
            out.pace.push((now, g.pace_bps()));
        }
        now += MS;
    }
    assert!(seq < 60_000, "seq u16 tel alanını aşıyor");
    out.probe_failures = g.probe_failures();
    out.samples = g.samples();
    out.overflow_samples = g.overflow_samples();
    out.pair_probes = g.pair_probes();
    out.yield_deferred = rx.diag_ack_yield_deferred();
    out.max_occ_pkts = bn.max_occ_pkts;
    check_pair_mirror(&out);
    out
}

/// Çıkan ACK'i dönüş hattına koyar: hattın paylaşılan doluluğuna yazılır (62 B/s), sonra yayılım.
fn push_ack(
    rx: &Receiver,
    acks: &mut VecDeque<InFlightAck>,
    ret: &mut ReturnLine,
    at: u64,
    frame: AckFrame,
) {
    let wire = (frame.encode().len() + ACK_DATAGRAM_ENVELOPE_BYTES) as u64;
    let done = ret.charge(at, wire);
    acks.push_back(InFlightAck {
        reach_ns: done + ONE_WAY,
        emit_ns: at,
        frame,
        obs: rx.diag_ack_obs(),
    });
}

/// `LineGuard::pair_wire_bps` aynası (tam datagramın zarf oranı).
fn pair_wire(pair_udp_bps: u64) -> u64 {
    pair_udp_bps * (DEFAULT_MTU + OVH) / DEFAULT_MTU
}

/// ACK göndericide: çalışma zamanının 0x15 dalı + `Sender::cc_line_guard_on_ack` sırası.
fn on_ack(
    g: &mut LineGuard,
    bucket: &mut TokenBucket,
    a: &InFlightAck,
    now: u64,
    next_seq: u64,
    n: usize,
) -> Row {
    let f = &a.frame;
    let hi = u64::from(f.largest_seq);
    let mut acked: Vec<u64> = f
        .acked_full(hi + 1)
        .into_iter()
        .filter(|&q| q < next_seq)
        .collect();
    acked.sort_unstable();
    let pair = f
        .bitmap
        .as_ref()
        .map(|b| b.pair_rate)
        .filter(|&p| p != PAIR_RATE_NONE);
    let interval = f.bitmap.as_ref().and_then(|bm| {
        let from = f.span_first_full(hi + 1)?;
        let received: Vec<u64> = acked
            .iter()
            .copied()
            .filter(|&q| q >= from && q <= hi)
            .collect();
        let first = *received.first()?;
        let valid = first < hi && received.contains(&hi) && hi < next_seq;
        let rx_wire = g.received_wire(first, hi, &received);
        (bm.rx_span != RX_SPAN_NONE && valid).then(|| ForwardInterval {
            first_seq: first,
            last_seq: hi,
            rx_span_ns: u64::from(bm.rx_span) * ACK_DELAY_UNIT_US * 1000,
            rx_wire,
        })
    });
    let delay = f.ack_delay_duration();
    let largest = acked.iter().copied().max().unwrap_or(hi);
    let before = g.pace_bps();
    let (samples0, overflow0, probes0) = (g.samples(), g.overflow_samples(), g.pair_probes());
    let (measured0, ..) = g.interval_diag();

    g.note_ack_wire((f.encode().len() + ACK_DATAGRAM_ENVELOPE_BYTES) as u64 - OVH);
    if g.start_budget_active() {
        let _ = g.on_first_window(&acked, f.arrival_rate);
    }
    g.note_pair_lower_bound(pair.map(u64::from));
    let update = g.on_ack_measured(
        now,
        u64::try_from(delay.as_nanos()).unwrap_or(u64::MAX),
        delay >= E2A_ACK_DELAY_SATURATED,
        largest,
        interval,
    );
    let diag = g.ack_diag();
    if let Some(p) = pair {
        g.on_pair_rate(u64::from(p));
    }
    g.settle_start_budget();
    bucket.set_line_rate(g.pace_bps());
    bucket.set_line_pairs(g.pair_pacing());
    if let Some(u) = update {
        bucket.line_raise_backlog(u.backlog_bytes);
        bucket.line_add_backlog(u.extra_backlog_bytes);
    }
    let drain = g.take_gap_drain();
    if drain > 0 {
        bucket.line_raise_backlog(drain);
    }

    // Çift yoklaması bu ACK'te başladıysa plan `pair_probe_target` formülüyle (modül belgesi):
    // tavan pencere başına dek tabandadır; tutma taban hızında erir.
    let plan = (g.pair_probes() > probes0).then(|| {
        let base = g.pace_bps();
        let wire = pair_wire(u64::from(pair.unwrap_or(0))).min(PROFILE);
        let target = if wire * 100 >= PROFILE * 95 {
            PROFILE
        } else {
            wire
        };
        let held = diag.backlog_bytes.unwrap_or(0) + diag.extra_backlog_bytes.unwrap_or(0);
        let start = now + held * SEC / base;
        ProbePlan {
            start,
            design_until: start + PROBE_DATAGRAMS * MTU_WIRE * SEC / target,
            base,
            target,
        }
    });
    let (measured, _, g_ns) = g.interval_diag();
    Row {
        n,
        emit_ns: a.emit_ns,
        proc_ns: now,
        obs: a.obs,
        largest,
        pair,
        interval,
        before,
        after: g.pace_bps(),
        rate_after: g.rate_bps(),
        diag,
        sample: g.samples() > samples0,
        overflow: g.overflow_samples() > overflow0,
        g_ns: g_ns.filter(|_| measured > measured0),
        plan,
        pair_probes: g.pair_probes(),
    }
}

fn s3(ns: u64) -> String {
    format!("{:.3}", ns as f64 / 1e9)
}

fn ms1(ns: u64) -> String {
    format!("{:.1}", ns as f64 / 1e6)
}

fn verdict_str(v: Option<ProbeVerdictDiag>) -> String {
    v.map_or_else(
        || "-".to_string(),
        |v| {
            format!(
                "{:?} (çift {}, g {:?} ns, gönderilen {} datagram / {} B, beklenen {} ns, {} → {})",
                v.outcome,
                v.from_pair,
                v.interval_g_ns,
                v.probe_sent_datagrams,
                v.probe_sent_wire,
                v.expect_ns,
                v.base_pace,
                v.probe_pace
            )
        },
    )
}

/// Zaman çizelgesinin bir satırı.
fn line_of(r: &Row) -> String {
    format!(
        "ACK #{} [{}] çıkış {} s, göndericide {} s, largest {}, aralık {}, g {} ms, çift {:?}, \
         alt sınır(tel) {:?}, örnek {}{}, tavan {} → {}, hız {:?}, tutma {:?}+{:?}, yol verme \
         ertelemesi {} (alarm {}), hüküm {}",
        r.n,
        r.kind(),
        s3(r.emit_ns),
        s3(r.proc_ns),
        r.largest,
        r.interval.map_or_else(
            || "yok".to_string(),
            |iv| format!(
                "[{}, {}] {} ms",
                iv.first_seq,
                iv.last_seq,
                ms1(iv.rx_span_ns)
            )
        ),
        r.g_ns
            .map_or_else(|| "-".to_string(), |g| format!("{:.1}", g as f64 / 1e6)),
        r.pair,
        r.diag.pair_bound_wire,
        if r.sample { "VAR" } else { "yok" },
        if r.overflow { " (taşma)" } else { "" },
        r.before,
        r.after,
        r.rate_after,
        r.diag.backlog_bytes,
        r.diag.extra_backlog_bytes,
        r.obs.defer_yield,
        r.obs.alarm_defer_yield,
        verdict_str(r.diag.verdict),
    )
}

fn print_timeline(name: &str, o: &Out, from_ns: u64, to_ns: u64) {
    println!(
        "--- {name}: zaman çizelgesi ({}–{} s) ---",
        s3(from_ns),
        s3(to_ns)
    );
    for r in o
        .rows
        .iter()
        .filter(|r| r.proc_ns >= from_ns && r.emit_ns < to_ns)
    {
        println!("{}", line_of(r));
    }
    println!(
        "alıcı: yükseliş alarmı {:?} s; daralma algılaması {:?} s; kanıtlı daralma {:?} s | gönderici: \
         örnek {}, taşma örneği {}, başarısız yoklama {}, çift yoklaması {}, iletim {} (aynı adımda \
         çift {}), kuyrukta düşen {}, hatta kayıp {}, kuyruk tepesi {} paket | dönüş: alındı {}, yol \
         verme ertelemesi {}",
        o.rise_at.iter().map(|&t| s3(t)).collect::<Vec<_>>(),
        o.detects.iter().map(|d| s3(d.at_ns)).collect::<Vec<_>>(),
        o.drop_at.iter().map(|&t| s3(t)).collect::<Vec<_>>(),
        o.samples,
        o.overflow_samples,
        o.probe_failures,
        o.pair_probes,
        o.sent.len(),
        o.paired_sends,
        o.queue_drops,
        o.lost,
        o.max_occ_pkts,
        o.receipts_sent,
        o.yield_deferred,
    );
}

/// Alıcının çift sayımının aynası (`receiver.rs` `PairRate::on_arrival`): bir varış çifti.
#[derive(Debug, Clone, Copy)]
struct PairObs {
    at_ns: u64,
    prev_body: u64,
    body: u64,
    gap_ns: u64,
    /// B/s gövde (`b / g`).
    value: u64,
    /// Sıkışma sınamasına takıldı / sayıldı (en büyük iki değere aday).
    squeezed: bool,
    counted: bool,
}

/// `(from, to]` içinde varan çiftler. Sıkışmış çift (aralık < 0,9 × tel / 12 500 B/s) ve ardındaki
/// sayılmaz; ikincisi 800 B'den küçük ya da aralığı 2 ms'den kısa çift sayılmaz.
fn pairs(o: &Out, from: u64, to: u64) -> Vec<PairObs> {
    let mut v = Vec::new();
    let mut after_squeeze = false;
    for w in o.rx.windows(2) {
        let ((pt, _, pb), (t, _, b)) = (w[0], w[1]);
        let g = t - pt;
        let squeezed = u128::from(g) * 1_000 * u128::from(PROFILE)
            < 900 * u128::from(b + OVH) * u128::from(SEC);
        let skip = squeezed || after_squeeze;
        after_squeeze = squeezed;
        if t > from && t <= to {
            v.push(PairObs {
                at_ns: t,
                prev_body: pb,
                body: b,
                gap_ns: g,
                value: b * SEC / g.max(1),
                squeezed,
                counted: !skip && b >= PAIR_MIN_UDP_BYTES && g >= 2 * MS,
            });
        }
    }
    v
}

/// `(from, to]` içinde SAYILAN çift değerlerinin en büyük ikincisi (yoksa 0).
fn second_largest(o: &Out, from: u64, to: u64) -> u64 {
    let mut v: Vec<u64> = pairs(o, from, to)
        .iter()
        .filter(|p| p.counted)
        .map(|p| p.value)
        .collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v.get(1).copied().unwrap_or(0)
}

/// ACK'in çift alanı saklanan değerden mi (`alarm_pair_carry`): alan, son sıfırlamadan (bu ACK
/// aralığındaki son daralma algılaması; yoksa önceki ACK) bu yana sayılan çiftlerin en büyük
/// ikincisinden büyükse evet. Dönen: `(aynanın en büyük ikincisi, taşındı mı)`.
fn carried(o: &Out, i: usize) -> (u64, bool) {
    let r = &o.rows[i];
    let prev = if i == 0 { 0 } else { o.rows[i - 1].emit_ns };
    let from = r.obs.detect_ns.map_or(prev, |d| d.max(prev));
    let second = second_largest(o, from, r.emit_ns);
    (second, r.pair.is_some_and(|p| u64::from(p) > second))
}

/// Aynanın sağlaması: aralığında daralma algılaması OLMAYAN her ACK'in alanı, önceki ACK'ten bu
/// yana sayılan çiftlerin en büyük ikincisidir (saklanan değer yok).
fn check_pair_mirror(o: &Out) {
    for (i, r) in o.rows.iter().enumerate() {
        if r.obs.detects > 0 {
            continue;
        }
        let prev = if i == 0 { 0 } else { o.rows[i - 1].emit_ns };
        let second = second_largest(o, prev, r.emit_ns).min(u64::from(PAIR_RATE_NONE) - 1);
        assert_eq!(
            r.pair.map_or(0, u64::from),
            second,
            "çift aynası üründen ayrıştı: ACK #{} ({} s)",
            r.n,
            s3(r.emit_ns)
        );
    }
}

/// Düşüş 20 s'de (12 500 → 6 250), toparlanma `rec`'te (→ 12 500).
fn recovery_plan(rec: u64) -> Vec<(u64, u64)> {
    vec![(0, PROFILE), (20 * SEC, NARROW), (rec, PROFILE)]
}

/// Düşüş 20 s'de, toparlanma yok.
fn flat_plan() -> Vec<(u64, u64)> {
    vec![(0, PROFILE), (20 * SEC, NARROW)]
}

/// Yoklama penceresi: formül + `tick` gözlemi (modül belgesi).
#[derive(Debug, Clone, Copy)]
struct Win {
    start: u64,
    until: u64,
    design_until: u64,
    base: u64,
    target: u64,
    /// Tavanın hedefe açıldığı ve tabana indiği adımlar (yoklama başlamadan düştüyse `None`).
    tick_up: Option<u64>,
    tick_down: Option<u64>,
    /// Bütçe dolduğu için süre dolmadan bitti.
    by_budget: bool,
}

fn window(o: &Out, i: usize) -> Win {
    let r = &o.rows[i];
    let p = r.plan.expect("yoklama planı");
    let up = o
        .ticks
        .iter()
        .find(|&&(t, b, a)| t >= r.proc_ns && b == p.base && a == p.target)
        .map(|&(t, ..)| t);
    let down = up.and_then(|u| {
        o.ticks
            .iter()
            .find(|&&(t, b, a)| t > u && b == p.target && a == p.base)
            .map(|&(t, ..)| t)
    });
    let by_budget = down.is_some_and(|d| d < p.design_until);
    Win {
        start: p.start,
        until: if by_budget {
            down.unwrap_or(p.design_until)
        } else {
            p.design_until
        },
        design_until: p.design_until,
        base: p.base,
        target: p.target,
        tick_up: up,
        tick_down: down,
        by_budget,
    }
}

/// Aralığın uçlarının gönderim anları ve yoklamanın tamamını kapsama (sahibin kuralı: ilk uç
/// yoklama başlamadan, son uç yoklama bittikten sonra gönderilmiş). ACK'in türüne bakılmaz.
fn covers(o: &Out, r: &Row, w: &Win) -> Option<(u64, u64, bool, bool)> {
    let iv = r.interval?;
    let (fs, ls) = (o.send_ns(iv.first_seq), o.send_ns(iv.last_seq));
    Some((fs, ls, fs <= w.start, ls >= w.until))
}

fn full_cover(o: &Out, r: &Row, w: &Win) -> bool {
    covers(o, r, w).is_some_and(|(_, _, a, b)| a && b)
}

/// Sahibin kuralı, koddan bağımsız: yoklamayı başlatan ACK 1. ACK; 2. ACK kapsıyorsa hüküm orada,
/// değilse 3. ACK; ikisi de kapsamıyorsa `None` (SONUÇSUZ).
fn owner_rule(o: &Out, i: usize, w: &Win) -> Option<usize> {
    (2..=3).find(|&k| o.rows.get(i + k - 1).is_some_and(|r| full_cover(o, r, w)))
}

/// Kodun çift yoklaması hükmü: `(kaçıncı ACK, sonuç)`; erteleme hüküm sayılmaz.
fn code_verdict(o: &Out, i: usize) -> Option<(usize, ProbeVerdictDiag)> {
    o.rows.iter().enumerate().skip(i + 1).find_map(|(j, r)| {
        r.pair_verdict()
            .filter(|v| v.outcome != ProbeOutcome::Deferred)
            .map(|v| (j - i + 1, v))
    })
}

/// Bir anın pencereye göre yeri.
fn rel(t: u64, w: &Win) -> &'static str {
    if t < w.start {
        "önce"
    } else if t < w.until {
        "İÇİNDE"
    } else {
        "sonra"
    }
}

/// Pencerenin iki sağlaması (modül belgesi): `tick` adımları ve hükmün fiilî gönderim sayımı.
/// Dönen: hüküm kaydındaki `(datagram, tel)`.
fn check_window(name: &str, o: &Out, i: usize, w: &Win) -> Option<(u64, u64)> {
    let up = w.tick_up?;
    assert!(
        up >= w.start && up <= w.start + MS,
        "{name}: tavan {up}'de açıldı, start {}",
        w.start
    );
    // Yoklama sürerken aşağı yönlü örnekle düştüyse tavan `tick` ile inmez: sayım sağlanamaz.
    let down = w.tick_down?;
    assert!(
        down < w.design_until + MS,
        "{name}: tavan tasarım bitişinden sonra indi"
    );
    let v = o.rows[i + 1..].iter().find_map(Row::pair_verdict)?;
    let (n, wire, _) = o.sent_between(w.start, w.until);
    assert_eq!(
        (v.probe_sent_datagrams, v.probe_sent_wire),
        (n, wire),
        "{name}: hüküm kaydı ↔ düzeneğin pencere sayımı"
    );
    assert!(
        wire <= PROBE_DATAGRAMS * MTU_WIRE,
        "{name}: bütçe aşıldı: {wire} B"
    );
    assert_eq!((v.base_pace, v.probe_pace), (w.base, w.target), "{name}");
    Some((n, wire))
}

/// A koşusunun özeti.
#[derive(Debug, Clone)]
struct ARun {
    /// Yoklama hedefi (B/s) ve fiilî gönderim `(datagram, tel, metin datagramı)`.
    target: u64,
    sent: (u64, u64, u64),
    by_budget: bool,
    /// 2. ve 3. ACK: tam kapsama.
    cover2: bool,
    cover3: bool,
    kind2: &'static str,
    /// Kodun hükmü: ham ACK sırası, cephesi yoklama bitmeden gönderilmiş (vadesi gelmemiş) ACK
    /// sayısı, sonuç.
    verdict: Option<(usize, usize, ProbeOutcome)>,
    rule: Option<usize>,
    /// Tavan 3. ACK işlendiğinde kalıcı ≥ 11 875 mi; kalıcı olduğu an.
    permanent_by_third: bool,
    permanent_at: Option<u64>,
    /// SONUÇSUZ ise kalan koşul.
    left: &'static str,
    /// Yoklamayı başlatan ACK: toparlanmadan sonra çıkan kaçıncı ACK ve göndericide işlendiği anın
    /// toparlanmaya uzaklığı (ns).
    init_order: usize,
    init_delay: u64,
    /// İzin imzası (ayrık iz sayımı).
    sig: (u64, u64, usize),
}

/// Bir ACK'in kapsama satırı (A tablosu).
fn cover_line(o: &Out, k: usize, r: &Row, w: &Win) -> String {
    let c = covers(o, r, w);
    let det = r
        .obs
        .detect_ns
        .and_then(|t| o.detects.iter().find(|d| d.at_ns == t));
    format!(
        "  {k}. ACK #{} [{}] çıkış {} s, göndericide {} s | aralık {} | ilk uç gönderim {} s ({}), \
         son uç gönderim {} s ({}) | tam kapsama: {} | cephe (largest {}) gönderim {} s: {} | \
         daralma algısı: {} | kanıt {} | hüküm {} | tavan {} → {}",
        r.n,
        r.kind(),
        s3(r.emit_ns),
        s3(r.proc_ns),
        r.interval.map_or_else(
            || "YOK".into(),
            |iv| format!(
                "seq {}..{} ({} ms)",
                iv.first_seq,
                iv.last_seq,
                ms1(iv.rx_span_ns)
            )
        ),
        c.map_or_else(|| "-".into(), |c| s3(c.0)),
        c.map_or("-", |c| if c.2 {
            "≤ start"
        } else {
            "start'tan SONRA"
        }),
        c.map_or_else(|| "-".into(), |c| s3(c.1)),
        c.map_or("-", |c| if c.3 {
            "≥ until"
        } else {
            "until'den ÖNCE"
        }),
        if c.is_some_and(|c| c.2 && c.3) {
            "EVET"
        } else {
            "HAYIR"
        },
        r.largest,
        s3(o.send_ns(r.largest)),
        if o.send_ns(r.largest) >= w.until {
            "yoklama bittikten sonra (vadesi gelmiş)"
        } else {
            "yoklama bitmeden (vadesi gelmemiş)"
        },
        det.map_or_else(
            || "yok".into(),
            |d| format!(
                "{} s'de seq {} (gönderimi pencereden {}); pencere içeriği {} varış ({} metin) seq \
                 {}..{} {} ms, hız {} B/s < %70 × referans {} (referansı kuran seq {}, gönderimi \
                 pencereden {})",
                s3(d.at_ns),
                d.seq,
                rel(o.send_ns(d.seq), w),
                d.n,
                d.small_n,
                d.first_seq,
                d.seq,
                ms1(d.span_ns),
                d.rate,
                d.reference,
                d.ref_seq,
                rel(o.send_ns(d.ref_seq), w),
            )
        ),
        r.obs.evidence_ns.map_or_else(|| "yok".into(), s3),
        verdict_str(r.pair_verdict()),
        r.before,
        r.after,
    )
}

/// A koşusu: yoklamayı başlatan ACK ve sonraki ACK'lerin kapsama dökümü + özet.
fn a_report(name: &str, o: &Out, rec: u64, print: bool) -> Option<ARun> {
    let i = o.probe_start()?;
    let w = window(o, i);
    let init = &o.rows[i];
    let checked = check_window(name, o, i, &w);
    let sent = o.sent_between(w.start, w.until);
    let code = code_verdict(o, i);
    let rule = owner_rule(o, i, &w);
    let last_shown = code.map_or(3, |(k, _)| k.max(3));
    // Hükme dek (hüküm dahil) cephesi yoklama bitmeden gönderilmiş ACK sayısı.
    let not_due = code.map_or(0, |(k, _)| {
        o.rows[i + 1..i + k]
            .iter()
            .filter(|r| o.send_ns(r.largest) < w.until)
            .count()
    });
    let third = o.rows.get(i + 2);
    let permanent_at = o.permanent_since(TARGET4);
    let permanent_by_third = permanent_at.is_some_and(|t| third.is_some_and(|r| t <= r.proc_ns));
    let cover_of = |k: usize| o.rows.get(i + k - 1).is_some_and(|r| full_cover(o, r, &w));
    let left = match code {
        Some((_, v)) if v.outcome == ProbeOutcome::Passed => "-",
        Some((_, v)) if v.outcome == ProbeOutcome::NoEvidence => "gönderim kanıtı",
        Some((k, v)) if v.outcome == ProbeOutcome::Inconclusive => {
            if full_cover(o, &o.rows[i + k - 1], &w) {
                "başka (aralık kapsıyor)"
            } else {
                "kısmi aralık"
            }
        }
        Some((_, v)) if v.outcome == ProbeOutcome::Superseded => "başka (aşağı yönlü örnek)",
        Some((_, v)) if v.outcome == ProbeOutcome::Failed => "başka (artış eşiğin üstünde)",
        Some(_) => "başka",
        None if w.tick_up.is_none() => "başka (yoklama başlamadan düştü)",
        None => "başka (hüküm yok)",
    };
    // Toparlanmadan sonra çıkan ACK'ler (yoklamayı başlatana dek): türü ve çift alanı (tel).
    let after: Vec<&Row> = o.rows[..i]
        .iter()
        .filter(|r| r.emit_ns > rec + ONE_WAY)
        .collect();
    let init_order = after.len() + 1;
    if print {
        println!(
            "### {name}\n  toparlanmadan sonra, yoklamadan önce çıkan ACK'ler: {:?}\n  başlatan ACK \
             (toparlanmadan sonraki {init_order}. ACK, toparlanmadan {} s sonra işlendi) #{} [{}] \
             çıkış {} s, göndericide {} s, çift {:?} → hedef {} \
             (taban {}) | pencere start {} s, until {} s ({} ms; tasarım {} ms; bitiş: {}) | tavan \
             açıldı {:?} s, indi {:?} s | fiilî gönderim (hüküm kaydı) {:?}; düzeneğin sayımı {} \
             datagram ({} metin) / {} B; bütçe {} datagram / {} B",
            after
                .iter()
                .map(|r| format!(
                    "#{} {} s [{}] çift(tel) {}",
                    r.n,
                    s3(r.emit_ns),
                    r.kind(),
                    r.diag
                        .pair_bound_wire
                        .map_or_else(|| "YOK".into(), |p| p.to_string())
                ))
                .collect::<Vec<_>>(),
            s3(init.proc_ns.saturating_sub(rec)),
            init.n,
            init.kind(),
            s3(init.emit_ns),
            s3(init.proc_ns),
            init.pair,
            w.target,
            w.base,
            s3(w.start),
            s3(w.until),
            ms1(w.until - w.start),
            ms1(w.design_until - w.start),
            if w.by_budget { "bütçe" } else { "süre" },
            w.tick_up.map(s3),
            w.tick_down.map(s3),
            checked,
            sent.0,
            sent.2,
            sent.1,
            PROBE_DATAGRAMS,
            PROBE_DATAGRAMS * MTU_WIRE,
        );
        for k in 2..=last_shown {
            if let Some(r) = o.rows.get(i + k - 1) {
                println!("{}", cover_line(o, k, r, &w));
            }
        }
        println!(
            "  sahibin kuralı: {} | kod: {} | çift yoklaması sayısı {} | tavan kalıcı ≥ {TARGET4}: \
             {:?} s (3. ACK'e dek: {}) | kalan koşul: {left}",
            rule.map_or_else(|| "SONUÇSUZ".into(), |k| format!("hüküm {k}. ACK'te")),
            code.map_or_else(
                || "hüküm yok".into(),
                |(k, v)| format!(
                    "{:?}, {k}. ACK'te ({not_due} ACK'in cephesi yoklama bitmeden gönderilmiş)",
                    v.outcome
                )
            ),
            o.pair_probes,
            permanent_at.map(s3),
            if permanent_by_third { "EVET" } else { "HAYIR" },
        );
    }
    Some(ARun {
        target: w.target,
        sent,
        by_budget: w.by_budget,
        cover2: cover_of(2),
        cover3: cover_of(3),
        kind2: o.rows.get(i + 1).map_or("-", Row::kind),
        verdict: code.map(|(k, v)| (k, not_due, v.outcome)),
        rule,
        permanent_by_third,
        permanent_at,
        left,
        init_order,
        init_delay: init.proc_ns.saturating_sub(rec),
        sig: (init.emit_ns, w.start, code.map_or(0, |(k, _)| k)),
    })
}

/// Tek yoklama: bir çift yoklamasından sonra yenisi ancak araya yeni bir aşağı yönlü örnek girdiyse
/// başlar (aynı ölçüm döneminde ikinci yoklama yok).
fn assert_single_shot(name: &str, o: &Out) {
    let probes: Vec<usize> = (0..o.rows.len())
        .filter(|&i| o.rows[i].plan.is_some())
        .collect();
    for w in probes.windows(2) {
        assert!(
            o.rows[w[0] + 1..=w[1]].iter().any(|r| r.sample),
            "{name}: aşağı yönlü örnek olmadan ikinci çift yoklaması (ACK #{} → #{})",
            o.rows[w[0]].n,
            o.rows[w[1]].n
        );
    }
}

/// Faz taramasının bir hücresinin sayımı.
#[derive(Debug, Default, Clone)]
struct ACount {
    runs: u32,
    /// Yoklama başladı; hedef profil hattı / ara değer.
    probed: u32,
    target_full: u32,
    target_partial: u32,
    /// Kodun hükmü.
    passed: u32,
    inconclusive: u32,
    failed: u32,
    superseded: u32,
    no_evidence: u32,
    none: u32,
    /// 2. / 3. ACK yoklamanın tamamını kapsıyor.
    cover2: u32,
    cover3: u32,
    /// 2. ACK'in türü kanıtlı daralma.
    ack2_drop: u32,
    /// Hükmün düştüğü ham ACK sırası: 2. / 3. / 4. ve sonrası.
    at2: u32,
    at3: u32,
    at4plus: u32,
    /// Hükme dek cephesi yoklama bitmeden gönderilmiş ACK bulunan koşu.
    with_not_due: u32,
    /// Sahibin kuralı ile kodun hükmü ayrıştı.
    rule_mismatch: u32,
    /// 3. ACK'e dek kalıcı ≥ 11 875; koşu sonuna dek kalıcı ≥ 11 875.
    permanent_by_third: u32,
    permanent_ever: u32,
    /// SONUÇSUZ koşularda kalan koşul.
    left_partial: u32,
    left_evidence: u32,
    left_other: u32,
    /// Pencere bütçeyle bitti; fiilen gönderilen en az / en çok datagram ve tel.
    by_budget: u32,
    min_datagrams: u64,
    max_datagrams: u64,
    max_wire: u64,
    /// Deney kapalı kolda koşu sonuna dek kalıcı ≥ 11 875.
    closed_permanent: u32,
    /// Yoklamayı başlatan ACK'in toparlanmadan sonraki sırası (en az / en çok) ve toparlanmadan
    /// işlenmesine dek geçen süre (ms; en az / en çok).
    init_order: (usize, usize),
    init_delay_ms: (u64, u64),
    /// Tam boy datagram sayısı (pencerede; en çok).
    max_full: u64,
    /// Toparlanmadan kalıcı ≥ 11 875'e dek geçen süre (ms; en az / en çok), deney açık / kapalı.
    open_delay_ms: Option<(u64, u64)>,
    closed_delay_ms: Option<(u64, u64)>,
    /// Ayrık iz sayısı (başlatan ACK'in çıkışı, pencere başı, hükmün sırası).
    sigs: BTreeSet<(u64, u64, usize)>,
}

fn widen(slot: &mut Option<(u64, u64)>, v: u64) {
    *slot = Some(slot.map_or((v, v), |(a, b)| (a.min(v), b.max(v))));
}

impl ACount {
    fn add(&mut self, a: Option<&ARun>) {
        self.runs += 1;
        let Some(a) = a else {
            self.none += 1;
            return;
        };
        self.probed += 1;
        if a.target == PROFILE {
            self.target_full += 1;
        } else {
            self.target_partial += 1;
        }
        match a.verdict.map(|v| v.2) {
            Some(ProbeOutcome::Passed) => self.passed += 1,
            Some(ProbeOutcome::Inconclusive) => self.inconclusive += 1,
            Some(ProbeOutcome::Failed) => self.failed += 1,
            Some(ProbeOutcome::Superseded) => self.superseded += 1,
            Some(ProbeOutcome::NoEvidence) => self.no_evidence += 1,
            _ => self.none += 1,
        }
        self.cover2 += u32::from(a.cover2);
        self.cover3 += u32::from(a.cover3);
        self.ack2_drop += u32::from(a.kind2.starts_with("kanıtlı daralma"));
        match a.verdict.map(|v| v.0) {
            Some(2) => self.at2 += 1,
            Some(3) => self.at3 += 1,
            Some(_) => self.at4plus += 1,
            None => {}
        }
        self.with_not_due += u32::from(a.verdict.is_some_and(|v| v.1 > 0));
        let agree = match (a.rule, a.verdict) {
            (Some(k), Some((j, _, out))) => {
                k == j && matches!(out, ProbeOutcome::Passed | ProbeOutcome::Failed)
            }
            (None, Some((j, _, out))) => j <= 3 && out == ProbeOutcome::Inconclusive,
            _ => false,
        };
        self.rule_mismatch += u32::from(!agree);
        self.permanent_by_third += u32::from(a.permanent_by_third);
        self.permanent_ever += u32::from(a.permanent_at.is_some());
        if a.verdict.is_some_and(|v| v.2 != ProbeOutcome::Passed) || a.verdict.is_none() {
            match a.left {
                "kısmi aralık" => self.left_partial += 1,
                "gönderim kanıtı" => self.left_evidence += 1,
                _ => self.left_other += 1,
            }
        }
        self.by_budget += u32::from(a.by_budget);
        self.min_datagrams = if self.probed == 1 {
            a.sent.0
        } else {
            self.min_datagrams.min(a.sent.0)
        };
        self.max_datagrams = self.max_datagrams.max(a.sent.0);
        self.max_wire = self.max_wire.max(a.sent.1);
        self.max_full = self.max_full.max(a.sent.0 - a.sent.2);
        let d = a.init_delay / MS;
        if self.probed == 1 {
            self.init_order = (a.init_order, a.init_order);
            self.init_delay_ms = (d, d);
        } else {
            self.init_order = (
                self.init_order.0.min(a.init_order),
                self.init_order.1.max(a.init_order),
            );
            self.init_delay_ms = (self.init_delay_ms.0.min(d), self.init_delay_ms.1.max(d));
        }
        self.sigs.insert(a.sig);
    }
}

/// A'nın faz sayısı ve adımı: toparlanma anı 45,000 s'den 397 ms adımla (13 faz, 45,000–49,764 s:
/// ACK aralığının (≈ 4,8 s) tamamı; metin takvimine (2 s) ve çift kipine (0,5 s) göre dağınık).
const A_PHASES: u64 = 13;
const A_STEP: u64 = 397 * MS;
const A_UNTIL: u64 = 140 * SEC;

fn a_scn(burst: u64, fixed_at_head: bool, k: u64, probe: Option<u64>) -> Scn {
    let mut s = Scn::product(recovery_plan(45 * SEC + k * A_STEP), probe, A_UNTIL);
    s.burst = burst;
    s.fixed_at_head = fixed_at_head;
    s
}

/// Önceki turun düzenek ayarı (teşhis; ürün ayarı DEĞİL): metin, alındı ve yol verme kapalı.
fn bare(mut s: Scn) -> Scn {
    s.texts = false;
    s.receipts = false;
    s.yield_on = false;
    s
}

fn a_cell(
    burst: u64,
    fixed_at_head: bool,
    product: bool,
    print: bool,
) -> (ACount, Vec<Option<ARun>>) {
    let mut c = ACount::default();
    let mut runs = Vec::new();
    let scn = |k: u64, probe: Option<u64>| {
        let s = a_scn(burst, fixed_at_head, k, probe);
        if product {
            s
        } else {
            bare(s)
        }
    };
    for k in 0..A_PHASES {
        let o = simulate(&scn(k, Some(PROBE_DATAGRAMS)));
        let d = simulate(&scn(k, None));
        let name = format!(
            "A | {} | {} | faz {k} (toparlanma {} s)",
            if burst == 0 {
                "serileştiren (teşhis)"
            } else {
                "kovalı 1 200 B (esas)"
            },
            if fixed_at_head {
                "hız başta sabit"
            } else {
                "tümlev"
            },
            s3(45 * SEC + k * A_STEP)
        );
        let rec = 45 * SEC + k * A_STEP;
        let a = a_report(&name, &o, rec, print);
        if print && a.is_none() {
            println!(
                "### {name}\n  çift yoklaması BAŞLAMADI (çift yoklaması sayısı {})",
                o.pair_probes
            );
        }
        c.add(a.as_ref());
        c.closed_permanent += u32::from(d.permanent_since(TARGET4).is_some());
        if let Some(t) = o.permanent_since(TARGET4) {
            widen(&mut c.open_delay_ms, t.saturating_sub(rec) / MS);
        }
        if let Some(t) = d.permanent_since(TARGET4) {
            widen(&mut c.closed_delay_ms, t.saturating_sub(rec) / MS);
        }
        if print {
            println!(
                "  toparlanmadan kalıcı ≥ {TARGET4}'e: deney AÇIK {} | deney KAPALI (eş koşu) {}",
                o.permanent_since(TARGET4).map_or_else(
                    || format!("yok ({} s içinde)", (A_UNTIL - rec) / SEC),
                    |t| format!("+{} s", s3(t.saturating_sub(rec)))
                ),
                d.permanent_since(TARGET4).map_or_else(
                    || format!("yok ({} s içinde)", (A_UNTIL - rec) / SEC),
                    |t| format!("+{} s", s3(t.saturating_sub(rec)))
                ),
            );
        }
        assert_single_shot(&name, &o);
        runs.push(a);
    }
    (c, runs)
}

fn print_acount(name: &str, c: &ACount) {
    println!(
        "A ÖZET | {name} | koşu {} | yoklama başladı {} (hedef 12 500: {}, ara: {}) | hüküm: Passed {}, \
         SONUÇSUZ {}, Failed {}, Superseded {}, NoEvidence {}, yok {} | 2. ACK tam kapsıyor {}, 3. ACK \
         tam kapsıyor {} | 2. ACK'in türü kanıtlı daralma {} | hükmün düştüğü ham sıra: 2. ACK {}, 3. \
         ACK {}, 4.+ ACK {} (hükme dek cephesi yoklama bitmeden gönderilmiş ACK bulunan koşu {}) | \
         kural ≠ kod {} | 3. ACK'e dek kalıcı ≥ {TARGET4}: {}/{} (koşu sonuna dek: {}) | SONUÇSUZ / \
         geçmeyenlerde kalan koşul: kısmi aralık {}, gönderim kanıtı {}, başka {} | pencere bütçeyle \
         bitti {} | fiilî gönderim {}–{} datagram (tam boy en çok {}), en çok {} B (bütçe {} B) | \
         deney KAPALI kolda koşu sonuna dek kalıcı ≥ {TARGET4}: {}/{} | başlatan ACK toparlanmadan \
         sonraki {}.–{}. ACK, toparlanmadan {}–{} ms sonra işlendi | toparlanmadan kalıcı ≥ \
         {TARGET4}'e (ms): deney AÇIK {:?}, KAPALI {:?} | ayrık iz {}",
        c.runs,
        c.probed,
        c.target_full,
        c.target_partial,
        c.passed,
        c.inconclusive,
        c.failed,
        c.superseded,
        c.no_evidence,
        c.none,
        c.cover2,
        c.cover3,
        c.ack2_drop,
        c.at2,
        c.at3,
        c.at4plus,
        c.with_not_due,
        c.rule_mismatch,
        c.permanent_by_third,
        c.runs,
        c.permanent_ever,
        c.left_partial,
        c.left_evidence,
        c.left_other,
        c.by_budget,
        c.min_datagrams,
        c.max_datagrams,
        c.max_full,
        c.max_wire,
        PROBE_DATAGRAMS * MTU_WIRE,
        c.closed_permanent,
        c.runs,
        c.init_order.0,
        c.init_order.1,
        c.init_delay_ms.0,
        c.init_delay_ms.1,
        c.open_delay_ms,
        c.closed_delay_ms,
        c.sigs.len(),
    );
}

/// SENARYO A — gerçek toparlanma (12 500 → 6 250 @ 20 s → 12 500 @ 45,000 s + faz), deney açık
/// (6 datagram), ürün ayarı (yol verme + alındı + metin kopyaları + çift kipi + sonlu kuyruk).
/// 13 faz × {kovalı (esas), serileştiren (teşhis)}; ayrıca hız değişimi serileşen datagrama yansıyan
/// (tümlev) darboğaz modeliyle aynı tarama (yalnız özet). Her koşu için kapsama dökümü basılır.
#[test]
fn a_gercek_toparlanma_faz_taramasi() {
    let mut cells = Vec::new();
    for (burst, fixed, product) in [
        (BUCKET, true, true),
        (0, true, true),
        (BUCKET, false, true),
        (0, false, true),
        (BUCKET, true, false),
        (0, true, false),
    ] {
        let (c, runs) = a_cell(burst, fixed, product, fixed && product);
        cells.push((burst, fixed, product, c, runs));
    }
    for (burst, fixed, product, c, _) in &cells {
        print_acount(
            &format!(
                "{} | {} | {}",
                if *burst == 0 {
                    "serileştiren (teşhis)"
                } else {
                    "kovalı (esas)"
                },
                if *fixed {
                    "hız başta sabit"
                } else {
                    "tümlev"
                },
                if *product {
                    "ürün ayarı"
                } else {
                    "önceki turun ayarı: metin / alındı / yol verme KAPALI (teşhis)"
                }
            ),
            c,
        );
    }
    for (_, _, _, c, _) in &cells {
        assert_eq!(c.runs, u32::try_from(A_PHASES).expect("faz"));
        assert_eq!(c.probed, c.runs, "her koşuda çift yoklaması başlar");
        // Bütçe kodda zorlanır: hiçbir koşuda tel baytı 6 × 1 228 B'yi, tam boy datagram sayısı
        // 6'yı aşmaz. Datagram SAYISI metin datagramlarıyla 6'yı aşar (bütçe tel baytındadır).
        assert!(c.max_wire <= PROBE_DATAGRAMS * MTU_WIRE);
        assert!(c.max_full <= PROBE_DATAGRAMS);
    }
    // Gözlenen (ürün ayarı, dört hücre): her koşuda 2. ACK yoklamanın tamamını kapsar ve hüküm
    // orada `Passed`; kod sahibin kuralıyla aynı ACK'te hükmeder; vadesi gelmemiş ACK yok.
    for (burst, fixed, _, c, _) in cells.iter().filter(|c| c.2) {
        assert_eq!(
            (c.passed, c.at2, c.cover2, c.rule_mismatch, c.with_not_due),
            (13, 13, 13, 0, 0),
            "burst {burst}, başta sabit {fixed}"
        );
    }
    // Fiilî gönderim: kovalı kolda 8–9 datagram (6 tam + 2–3 metin), en çok 6 852 B.
    assert_eq!(
        (
            cells[0].3.min_datagrams,
            cells[0].3.max_datagrams,
            cells[0].3.max_wire
        ),
        (8, 9, 6_852)
    );
    // Serileştiren (teşhis) kol: 11/13 koşuda 3. ACK'e dek kalıcı ≥ 11 875.
    assert_eq!(
        (cells[1].3.target_full, cells[1].3.permanent_by_third),
        (11, 11)
    );
    // Önceki turun ayarı (metin / alındı / yol verme kapalı): hüküm çoğunlukla SONUÇSUZ, 2. ACK
    // kapsamıyor (önceki turun sonucu bu ayarda yeniden üretilir).
    assert_eq!(
        (
            cells[4].3.passed,
            cells[4].3.inconclusive,
            cells[4].3.no_evidence,
            cells[4].3.cover2
        ),
        (0, 11, 2, 0)
    );
    assert_eq!((cells[5].3.passed, cells[5].3.inconclusive), (1, 12));
}

/// SENARYO A — ÖN KAYITLA ÇELİŞTİ. Ön kayıt: "kapsayan ACK'te `Passed` ve tavan kalıcı 12 500".
/// Kovalı (esas) kolda ürün ayarıyla hüküm 13/13 koşuda 2. ACK'te `Passed`'dir ama 12/13 koşuda
/// hedef ARA değerdir (7 043–7 432): toparlanmış 12 500 hatta çift kipinin iki tam datagramı dolu
/// kovadan (1 200 B) `(1 074 − 126) / 12 500` = 75,8 ms arayla çıkar; sıkışma eşiği
/// `0,9 × 1 074 / 12 500` = 77,3 ms'dir, çift sayılmaz. Alıcının çift alanı yükselmez; yoklama geç
/// ve düşük hedefle başlar, tek atış hakkı harcanır. 3. ACK'e dek kalıcı ≥ 11 875: 1/13. Deney
/// KAPALI kolda da 1/13 (aynı ölçü tavanı doğrudan açar).
#[test]
fn a2_on_kayitla_celisti_kovali_kolda_hedef_ara_degerde_kaliyor() {
    let (c, runs) = a_cell(BUCKET, true, true, false);
    println!(
        "A2 | kovalı (esas), ürün ayarı: hedef 12 500: {}, ara: {}; Passed {}; 3. ACK'e dek kalıcı ≥ \
         {TARGET4}: {}/{}; koşu sonuna dek ({} s): {}; deney KAPALI kolda: {} | ara hedefler: {:?}",
        c.target_full,
        c.target_partial,
        c.passed,
        c.permanent_by_third,
        c.runs,
        A_UNTIL / SEC,
        c.permanent_ever,
        c.closed_permanent,
        runs.iter()
            .flatten()
            .map(|a| a.target)
            .collect::<BTreeSet<_>>(),
    );
    assert_eq!((c.target_full, c.target_partial, c.passed), (1, 12, 13));
    assert_eq!(
        (c.permanent_by_third, c.permanent_ever, c.closed_permanent),
        (1, 1, 1)
    );
    // Kovanın sayısı: dolu kovadan sonra iki tam datagramın aralığı sıkışma eşiğinin altında.
    let gap = (WIRE - (BUCKET - WIRE)) * SEC / PROFILE;
    assert_eq!(gap, 75_840_000);
    assert!(
        u128::from(gap) * 1_000 * u128::from(PROFILE) < 900 * u128::from(WIRE) * u128::from(SEC)
    );
}

/// Düz dar hat koşusu (B1): metin takviminin fazı `k × 125 ms`.
fn b1_scn(k: u64, probe: Option<u64>, burst: u64) -> Scn {
    let mut s = Scn::product(flat_plan(), probe, 120 * SEC);
    s.text_phase = k * 125 * MS;
    s.burst = burst;
    s
}

/// B1'de sayımın başladığı an: 20 s'deki düşüşün kuyruğu boşaldıktan ve hat 6 250'de ölçüldükten
/// sonra.
const B1_FROM: u64 = 40 * SEC;

/// Hat 6 250'deyken (`B1_FROM`'dan sonra çıkan) ACK'lerden çift alanı yüksek olan ilki.
fn first_high_pair(o: &Out) -> Option<usize> {
    o.rows.iter().position(|r| {
        r.emit_ns >= B1_FROM && r.diag.pair_bound_wire.is_some_and(|p| p > NARROW_BAND)
    })
}

/// `B1_FROM`'dan sonra SAYILAN yüksek çiftler: `(küçük → tam sayısı, tam → tam sayısı, en büyük
/// değerin tel karşılığı)`.
fn high_counted(o: &Out) -> (u32, u32, u64) {
    pairs(o, B1_FROM, o.end_ns)
        .iter()
        .filter(|p| p.counted && pair_wire(p.value) > NARROW_BAND)
        .fold((0, 0, 0), |(s, f, m), p| {
            let small = p.prev_body < PAIR_MIN_UDP_BYTES;
            (
                s + u32::from(small),
                f + u32::from(!small),
                m.max(pair_wire(p.value)),
            )
        })
}

/// B1'de olay: tavanın ilk açılışı (kapalı kol) / ilk çift yoklamasının başı (açık kol); yoksa
/// koşu sonu.
fn b1_event(o: &Out) -> u64 {
    o.rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.emit_ns >= B1_FROM)
        .find_map(|(i, r)| {
            if r.plan.is_some() {
                Some(window(o, i).start)
            } else {
                (r.after > r.before && r.after > NARROW_BAND).then_some(r.proc_ns)
            }
        })
        .unwrap_or(o.end_ns)
}

/// B1 sayımı (faz taraması).
#[derive(Debug, Default, PartialEq, Eq)]
struct B1Count {
    runs: u32,
    /// Hat 6 250'deyken yüksek çift alanı taşıyan ACK bulunan koşu; yükseliş alarmı verilen koşu.
    high_pair_runs: u32,
    rise_runs: u32,
    /// Kapalı kol: tavanın 6 875'in üstüne açıldığı koşu; en büyük tavan; en uzun süre (ms).
    raised_runs: u32,
    max_pace: u64,
    max_above_ms: u64,
    /// Açık kol: çift yoklaması başlayan koşu, toplam yoklama, en çok yoklama / koşu.
    probe_runs: u32,
    probes: u64,
    max_probes: u64,
    /// Çift yoklamalarının hükümleri.
    passed: u32,
    failed: u32,
    superseded: u32,
    inconclusive: u32,
    /// Pencerede fiilen gönderilen en çok datagram / tel baytı (hüküm kaydı).
    max_datagrams: u64,
    max_wire: u64,
    /// Kuyruk tepesi (ms; kapının tanımı): olaydan (tavanın ilk açılışı / ilk yoklamanın başı)
    /// önce ve sonra; kapıyı aşan koşu sayısı.
    max_before_ms: u64,
    max_after_ms: u64,
    over_gate_before: u32,
    over_gate_after: u32,
    queue_drops: u64,
}

fn b1_count(probe: Option<u64>, burst: u64, print: bool) -> B1Count {
    let mut c = B1Count::default();
    for k in 0..16 {
        let o = simulate(&b1_scn(k, probe, burst));
        c.runs += 1;
        c.high_pair_runs += u32::from(first_high_pair(&o).is_some());
        c.rise_runs += u32::from(o.rise_at.iter().any(|&t| t >= B1_FROM));
        let (above, peak) = o.pace_above(NARROW_BAND, B1_FROM, o.end_ns);
        if print {
            let (small_full, full_full, top) = high_counted(&o);
            let verdicts: Vec<String> = o
                .rows
                .iter()
                .filter(|r| r.emit_ns >= B1_FROM)
                .filter_map(|r| {
                    r.pair_verdict().map(|v| {
                        format!(
                            "#{} {:?} {}→{} ({} datagram / {} B)",
                            r.n,
                            v.outcome,
                            v.base_pace,
                            v.probe_pace,
                            v.probe_sent_datagrams,
                            v.probe_sent_wire
                        )
                    })
                })
                .collect();
            let event = b1_event(&o);
            let before = o.queue_peak(B1_FROM, event).0;
            let (ahead, stay) = o.queue_peak(event, o.end_ns);
            println!(
                "B1 faz {k:2} ({} ms) | {} | {} | sayılan yüksek çift: küçük→tam {small_full}, \
                 tam→tam {full_full}, en büyük (tel) {top} | yükseliş alarmı {:?} s | tavan > \
                 {NARROW_BAND}: {} s, tepe {} | çift yoklaması {} {verdicts:?} | kuyruk tepesi: \
                 olaydan önce ({}–{} s) öndeki bekleme {} ms; olaydan sonra öndeki bekleme {} ms \
                 ({}), toplam kalış {} ms | kuyrukta düşen {}",
                k * 125,
                if burst == 0 {
                    "serileştiren"
                } else {
                    "kovalı"
                },
                if probe.is_some() { "AÇIK" } else { "KAPALI" },
                o.rise_at
                    .iter()
                    .filter(|&&t| t >= B1_FROM)
                    .map(|&t| s3(t))
                    .collect::<Vec<_>>(),
                s3(above),
                peak,
                o.pair_probes,
                B1_FROM / SEC,
                s3(event),
                ms1(before),
                ms1(ahead),
                if ahead <= GATE_NS {
                    "kapı içinde"
                } else {
                    "KAPI AŞILDI"
                },
                ms1(stay),
                o.queue_drops,
            );
        }
        if probe.is_none() && above > 0 {
            c.raised_runs += 1;
        }
        c.max_pace = c.max_pace.max(peak);
        c.max_above_ms = c.max_above_ms.max(above / MS);
        c.probe_runs += u32::from(o.pair_probes > 0);
        c.probes += o.pair_probes;
        c.max_probes = c.max_probes.max(o.pair_probes);
        assert_single_shot(&format!("B1 faz {k}"), &o);
        for v in o
            .rows
            .iter()
            .filter(|r| r.emit_ns >= B1_FROM)
            .filter_map(Row::pair_verdict)
        {
            match v.outcome {
                ProbeOutcome::Passed => c.passed += 1,
                ProbeOutcome::Failed => c.failed += 1,
                ProbeOutcome::Superseded => c.superseded += 1,
                ProbeOutcome::Inconclusive => c.inconclusive += 1,
                _ => {}
            }
            c.max_datagrams = c.max_datagrams.max(v.probe_sent_datagrams);
            c.max_wire = c.max_wire.max(v.probe_sent_wire);
            assert!(
                v.probe_sent_wire <= PROBE_DATAGRAMS * MTU_WIRE,
                "bütçe aşıldı"
            );
        }
        let event = b1_event(&o);
        let before = o.queue_peak(B1_FROM, event).0;
        let after = o.queue_peak(event, o.end_ns).0;
        c.max_before_ms = c.max_before_ms.max(before / MS);
        c.max_after_ms = c.max_after_ms.max(after / MS);
        c.over_gate_before += u32::from(before > GATE_NS);
        c.over_gate_after += u32::from(after > GATE_NS);
        c.queue_drops += o.queue_drops;
    }
    c
}

/// Kuyruk tepesi satırı: iki tanım ↔ 393 ms.
fn gate_line(o: &Out, from: u64, to: u64) -> String {
    let (ahead, stay) = o.queue_peak(from, to);
    format!(
        "[{}–{} s) öndeki bekleme {} ms ({}), toplam kalış {} ms ({})",
        s3(from),
        s3(to),
        ms1(ahead),
        if ahead <= GATE_NS {
            "kapı içinde"
        } else {
            "KAPI AŞILDI"
        },
        ms1(stay),
        if stay <= GATE_NS {
            "kapı içinde"
        } else {
            "kapı aşıldı"
        },
    )
}

/// Bir ACK aralığındaki yüksek (> 6 875 tel karşılığı) çiftlerin dökümü.
fn high_pairs_line(o: &Out, i: usize) -> String {
    let prev = if i == 0 { 0 } else { o.rows[i - 1].emit_ns };
    let v: Vec<String> = pairs(o, prev, o.rows[i].emit_ns)
        .iter()
        .filter(|p| pair_wire(p.value) > NARROW_BAND && p.gap_ns >= 2 * MS)
        .map(|p| {
            format!(
                "{} s: {} → {} B gövde, aralık {} ms, b/g {} ({})",
                s3(p.at_ns),
                p.prev_body,
                p.body,
                ms1(p.gap_ns),
                p.value,
                if p.counted {
                    "SAYILDI"
                } else if p.squeezed {
                    "sıkışmış, atıldı"
                } else {
                    "sayılmadı"
                }
            )
        })
        .collect();
    format!("{v:?}")
}

/// SENARYO B1 — küçük + tam çift karşı örneği (#258 yorum 7173692949): hat 20 s'de 6 250'ye iner ve
/// ORADA KALIR; kovalı darboğaz; gönderici metin datagramları (136 B IP) + tam boy. Metin
/// takviminin 16 fazı taranır; deney KAPALI ve AÇIK kolları; serileştiren darboğaz karşılaştırma.
#[test]
fn b1_kucuk_tam_cift_karsi_ornegi() {
    let closed = b1_count(None, BUCKET, true);
    let open = b1_count(Some(PROBE_DATAGRAMS), BUCKET, true);
    let closed_ser = b1_count(None, 0, false);
    let open_ser = b1_count(Some(PROBE_DATAGRAMS), 0, false);
    println!("B1 tarama | kovalı | deney KAPALI: {closed:?}");
    println!("B1 tarama | kovalı | deney AÇIK: {open:?}");
    println!("B1 tarama | serileştiren | deney KAPALI: {closed_ser:?}");
    println!("B1 tarama | serileştiren | deney AÇIK: {open_ser:?}");

    // Ayrıntı: kapalı kolda yükseliş alarmı verilen fazlardan tavanı en yükseğe açılan (yoksa
    // bütün fazlardan; eşitlikte ilki).
    let pick = (0..16)
        .map(|k| {
            let o = simulate(&b1_scn(k, None, BUCKET));
            (
                o.rise_at.iter().any(|&t| t >= B1_FROM),
                o.pace_above(NARROW_BAND, B1_FROM, o.end_ns).1,
                k,
            )
        })
        .filter(|&(_, peak, _)| peak > 0)
        .max_by_key(|&(rise, peak, k)| (rise, peak, std::cmp::Reverse(k)))
        .map(|(_, _, k)| k);
    let Some(k) = pick else {
        println!(
            "B1 | bu düzenekte 16 fazın hiçbirinde hat 6 250'deyken yüksek çift alanı oluşmadı"
        );
        return;
    };
    let c = simulate(&b1_scn(k, None, BUCKET));
    let o = simulate(&b1_scn(k, Some(PROBE_DATAGRAMS), BUCKET));
    println!("B1 ayrıntı | metin fazı {} ms", k * 125);
    print_timeline("B1 deney KAPALI", &c, B1_FROM, c.end_ns);
    print_timeline("B1 deney AÇIK", &o, B1_FROM, o.end_ns);

    // Deney kapalı.
    if let Some(i) = first_high_pair(&c) {
        let r = &c.rows[i];
        println!(
            "B1 KAPALI | ilk yüksek çift alanı: ACK #{} [{}] çıkış {} s: pair_rate {:?} (tel {:?}), \
             yükseliş alarmı {} | o aralıktaki yüksek çiftler: {} | tavan {} → {}",
            r.n,
            r.kind(),
            s3(r.emit_ns),
            r.pair,
            r.diag.pair_bound_wire,
            r.obs.rise,
            high_pairs_line(&c, i),
            r.before,
            r.after,
        );
    }
    let raise = c
        .rows
        .iter()
        .find(|r| r.emit_ns >= B1_FROM && r.after > r.before && r.after > NARROW_BAND);
    if let Some(r) = raise {
        let (above, peak) = c.pace_above(NARROW_BAND, r.proc_ns, c.end_ns);
        let down = c
            .pace
            .iter()
            .find(|&&(t, p)| t > r.proc_ns && p <= NARROW_BAND)
            .map_or(c.end_ns, |&(t, _)| t);
        println!(
            "B1 KAPALI | tavanı açan ACK #{} [{}] göndericide {} s: {} → {} (hat 6 250) | tavan > \
             {NARROW_BAND}: ilk dönüş {} s ({} s sonra), koşu sonuna dek toplam {} s, tepe {} | kuyruk: \
             {} ; {} | kuyrukta düşen {}",
            r.n,
            r.kind(),
            s3(r.proc_ns),
            r.before,
            r.after,
            s3(down),
            s3(down - r.proc_ns),
            s3(above),
            peak,
            gate_line(&c, r.proc_ns, down),
            gate_line(&c, down, (down + 10 * SEC).min(c.end_ns)),
            c.queue_drops,
        );
    } else {
        println!("B1 KAPALI | bu fazda tavan {NARROW_BAND}'in üstüne açılmadı");
    }
    // Tavanı en yükseğe açan ACK ve yükseliş alarmının ACK'i.
    let top = c
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.emit_ns >= B1_FROM && r.after > r.before)
        .max_by_key(|(i, r)| (r.after, std::cmp::Reverse(*i)));
    if let Some((i, r)) = top {
        let down = c
            .pace
            .iter()
            .find(|&&(t, p)| t > r.proc_ns && p < r.after)
            .map_or(c.end_ns, |&(t, _)| t);
        println!(
            "B1 KAPALI | tavanı en yükseğe açan ACK #{} [{}] çıkış {} s, göndericide {} s: pair_rate \
             {:?} (tel {:?}), yükseliş alarmı {} | o aralıktaki yüksek çiftler: {} | tavan {} → {} \
             (hat 6 250), {} s sürdü | kuyruk: {}",
            r.n,
            r.kind(),
            s3(r.emit_ns),
            s3(r.proc_ns),
            r.pair,
            r.diag.pair_bound_wire,
            r.obs.rise,
            high_pairs_line(&c, i),
            r.before,
            r.after,
            s3(down - r.proc_ns),
            gate_line(&c, r.proc_ns, down),
        );
    }
    for (i, r) in c
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.emit_ns >= B1_FROM && r.obs.rise)
    {
        println!(
            "B1 KAPALI | yükseliş alarmının ACK'i #{} [{}] çıkış {} s: pair_rate {:?} (tel {:?}) | o \
             aralıktaki yüksek çiftler: {} | tavan {} → {}",
            r.n,
            r.kind(),
            s3(r.emit_ns),
            r.pair,
            r.diag.pair_bound_wire,
            high_pairs_line(&c, i),
            r.before,
            r.after,
        );
    }

    // Deney açık.
    match o.probe_start() {
        None => println!(
            "B1 AÇIK | çift yoklaması başlamadı (pair_probes {}); kuyruk: {}",
            o.pair_probes,
            gate_line(&o, B1_FROM, o.end_ns)
        ),
        Some(i) => {
            let w = window(&o, i);
            let checked = check_window("B1 AÇIK", &o, i, &w);
            let sent = o.sent_between(w.start, w.until);
            let init = &o.rows[i];
            println!(
                "B1 AÇIK | başlatan ACK #{} [{}] çıkış {} s, göndericide {} s: pair_rate {:?} (tel \
                 {:?}), yükseliş alarmı {} | o aralıktaki yüksek çiftler: {} | hedef {} (taban {}) | \
                 pencere {}–{} s ({} ms; bitiş {}) | fiilî gönderim (hüküm kaydı) {:?}; düzeneğin \
                 sayımı {} datagram ({} metin) / {} B",
                init.n,
                init.kind(),
                s3(init.emit_ns),
                s3(init.proc_ns),
                init.pair,
                init.diag.pair_bound_wire,
                init.obs.rise,
                high_pairs_line(&o, i),
                w.target,
                w.base,
                s3(w.start),
                s3(w.until),
                ms1(w.until - w.start),
                if w.by_budget { "bütçe" } else { "süre" },
                checked,
                sent.0,
                sent.2,
                sent.1,
            );
            for (k, r) in o.rows[i + 1..].iter().take(3).enumerate() {
                println!("{}", cover_line(&o, k + 2, r, &w));
            }
            let code = code_verdict(&o, i);
            let probes_after: Vec<String> = o.rows[i + 1..]
                .iter()
                .filter(|r| r.plan.is_some())
                .map(|r| format!("ACK #{} {} s", r.n, s3(r.proc_ns)))
                .collect();
            println!(
                "B1 AÇIK | hüküm: {} | ret seviyesi: {} | aynı ölçüm döneminde ikinci yoklama: {} | \
                 koşu boyunca çift yoklaması {} (sonrakiler: {probes_after:?})",
                code.map_or_else(
                    || "yok".into(),
                    |(k, v)| format!("{k}. ACK'te {}", verdict_str(Some(v)))
                ),
                match code.map(|(_, v)| v.outcome) {
                    Some(ProbeOutcome::Failed) => "YAZILDI (Failed dalı: ret = yoklama hedefi)",
                    Some(ProbeOutcome::Superseded) =>
                        "YAZILDI (aşağı yönlü örnek dalı: ret = yoklama hedefi)",
                    Some(_) => "yazılmadı (bu sonuç ret yazan dallardan değil)",
                    None => "-",
                },
                {
                    let next_sample = o.rows[i + 1..].iter().position(|r| r.sample);
                    let second = o.rows[i + 1..].iter().position(|r| r.plan.is_some());
                    if second.is_some_and(|s| next_sample.is_none_or(|x| x >= s)) {
                        "VAR"
                    } else {
                        "yok"
                    }
                },
                o.pair_probes,
            );
            let vt = code.map_or(w.until, |(k, _)| o.rows[i + k - 1].proc_ns);
            println!(
                "B1 AÇIK | kuyruk tepesi ↔ {} ms kapısı: yoklama {} ; yoklama sonu → hüküm ACK'i {} ; \
                 hüküm + 10 s {} | kuyrukta düşen {}",
                ms1(GATE_NS),
                gate_line(&o, w.start, w.until),
                gate_line(&o, w.until, vt.max(w.until)),
                gate_line(&o, vt, (vt + 10 * SEC).min(o.end_ns)),
                o.queue_drops,
            );
        }
    }
    // Gözlenen (kovalı): hat 6 250'de kalırken 15/16 fazda çift alanı yüksek okur; deney kapalı
    // kolda tavan 15/16 fazda açılır (tepe 11 464), kuyruk 15/16 fazda kapıyı aşar. Deney açık kolda
    // tavan yalnız yoklama süresince açılır; 16 yoklamanın 10'u `Failed`, 4'ü aşağı yönlü örnekle
    // düşer, 1'i SONUÇSUZ, 1'i `Passed` (ayrı test); olaydan sonra kapıyı aşan koşu 1.
    assert_eq!(
        (
            closed.high_pair_runs,
            closed.rise_runs,
            closed.raised_runs,
            closed.max_pace
        ),
        (15, 4, 15, 11_464)
    );
    assert_eq!((closed.over_gate_after, closed.max_after_ms), (15, 3_911));
    assert_eq!((open.probe_runs, open.probes, open.max_probes), (15, 16, 2));
    assert_eq!(
        (open.passed, open.failed, open.superseded, open.inconclusive),
        (1, 10, 4, 1)
    );
    assert_eq!((open.max_datagrams, open.max_wire), (10, 6_988));
    assert_eq!((open.over_gate_after, open.max_after_ms), (1, 730));
    // Serileştiren darboğazda (kova yok) yüksek çift hiç oluşmaz.
    assert_eq!((closed_ser.high_pair_runs, open_ser.probe_runs), (0, 0));
}

/// SENARYO B1 — ÖN KAYITLA ÇELİŞTİ. Ön kayıt (#262): hat değişmediyse yoklama kapsayan aralıkla
/// `Failed` olur. Düz 6 250 hatta, kovalı darboğazda, metin fazı 625 ms koşusunda çift yoklaması
/// (hedef 7 015) kapsayan 2. ACK'te `Passed` olur: tavan gerçek hattın üstünde kalıcı açılır.
#[test]
fn b1b_on_kayitla_celisti_duz_dar_hatta_yoklama_gecti() {
    let mut passed = 0u32;
    for k in 0..16 {
        let o = simulate(&b1_scn(k, Some(PROBE_DATAGRAMS), BUCKET));
        for i in (0..o.rows.len()).filter(|&i| o.rows[i].plan.is_some()) {
            let Some((j, v)) = code_verdict(&o, i) else {
                continue;
            };
            if v.outcome != ProbeOutcome::Passed {
                continue;
            }
            passed += 1;
            let w = window(&o, i);
            let vrow = &o.rows[i + j - 1];
            let (above, peak) = o.pace_above(NARROW_BAND, vrow.proc_ns, o.end_ns);
            println!(
                "B1b | metin fazı {} ms: başlatan ACK #{} çift {:?} → hedef {} (taban {}), pencere \
                 {}–{} s, fiilî gönderim {} datagram / {} B",
                k * 125,
                o.rows[i].n,
                o.rows[i].pair,
                w.target,
                w.base,
                s3(w.start),
                s3(w.until),
                v.probe_sent_datagrams,
                v.probe_sent_wire,
            );
            for n in 2..=j {
                println!("{}", cover_line(&o, n, &o.rows[i + n - 1], &w));
            }
            println!(
                "  hüküm {j}. ACK'te Passed (hat 6 250, hedef {}) | hükümden sonra tavan > \
                 {NARROW_BAND}: {} s, tepe {peak} | kuyruk: {} ; {}",
                w.target,
                s3(above),
                gate_line(&o, w.start, vrow.proc_ns),
                gate_line(&o, vrow.proc_ns, (vrow.proc_ns + 10 * SEC).min(o.end_ns)),
            );
            assert!(full_cover(&o, vrow, &w), "kapsamayan aralıkla Passed");
            assert_eq!((k, w.target, w.base), (5, 7_015, 6_250));
        }
    }
    assert_eq!(passed, 1);
}

/// SENARYO B2'nin hat planı: 12 500 → 6 250 (20 s) → 12 500 (`t2`) → GERÇEK ikinci düşüş (`t3`,
/// `level`).
fn flap_plan(t2: u64, t3: u64, level: u64) -> Vec<(u64, u64)> {
    vec![(0, PROFILE), (20 * SEC, NARROW), (t2, PROFILE), (t3, level)]
}

/// Bulut-D'nin koşulu (#256 7172925320): darboğazdan sonra iki tam datagramı sıkışma eşiğinin
/// (77,3 ms) altına indiren tutma. Tutulan datagram ile ardındakinin alıcıdaki aralığı 76,84 ms
/// olacak biçimde: 6 250 B/s'de 95 ms, 3 125 B/s'de 266,84 ms.
const B_GAP: u64 = 76_840_000;

fn b_hold(level: u64) -> u64 {
    WIRE * SEC / level - B_GAP
}

#[derive(Debug, Clone, Copy)]
struct B2Cell {
    t2: u64,
    delta: u64,
    level: u64,
    /// Toplu teslim ikinci düşüşten sonraki kaçıncı tam datagram serileşmesinden sonra.
    k: u64,
}

fn b2_scn(c: B2Cell, probe: Option<u64>, batch: bool, loss: u64) -> Scn {
    let t3 = c.t2 + c.delta;
    let mut s = Scn::product(flap_plan(c.t2, t3, c.level), probe, 60 * SEC);
    s.loss_permille = loss;
    s.batch = batch.then_some(Batch {
        from_ns: t3 + c.k * WIRE * SEC / c.level,
        hold_ns: b_hold(c.level),
    });
    s
}

/// Toplu teslimin ardındaki datagramın alıcıya varış anı (toplu teslim oluştuysa).
fn batch_at(o: &Out) -> Option<u64> {
    let (_, follower, _) = o.batched?;
    o.rx.iter().find(|x| x.1 == follower).map(|x| x.0)
}

/// "O ACK": toplu teslimi kapsayan (ondan sonra çıkan ilk) ACK'in satır dizini.
fn batch_row(o: &Out) -> Option<usize> {
    let t = batch_at(o)?;
    o.rows.iter().position(|r| r.emit_ns >= t)
}

/// Toplu teslim iki TAM datagramı sıkışma eşiğinin (0,9 × 1 074 B / 12 500 B/s = 77,3 ms) altında mı
/// teslim etti.
fn batch_squeezed(o: &Out) -> bool {
    o.batched.is_some_and(|(_, follower, gap)| {
        o.sent[usize::try_from(follower).expect("seq")]
            .text
            .is_none()
            && u128::from(gap) * 1_000 * u128::from(PROFILE)
                < 900 * u128::from(WIRE) * u128::from(SEC)
    })
}

/// B2 ızgarasının sayımı. Sayılan tek satır "o ACK"tir: toplu teslimi kapsayan ACK.
#[derive(Debug, Default, PartialEq, Eq)]
struct B2Count {
    runs: u32,
    /// Toplu teslim tam + tam ve sıkışmış oluştu; "o ACK" var.
    squeezed: u32,
    rows: u32,
    /// "O ACK"in aralığında: yükseliş alarmı; alarm ACK'i yol verdiği için bekledi
    /// (`alarm_defer_yield` > 0); daralma algılandı; kanıtla bırakıldı.
    rise: u32,
    rise_yield: u32,
    rise_yield_detect: u32,
    /// K1'in ön koşulu: aynı aralıkta yükseliş alarmı VE kanıtlı bırakma (… ve yol verme beklemesi).
    rise_evidence: u32,
    rise_yield_evidence: u32,
    /// Alan saklanan (eski yükseliş) değeri taşıyor (> 7 000): kanıtlı bırakmadan sonra (K1 kolu) /
    /// bekleme açıkken.
    carried_after_evidence: u32,
    carried_wait_open: u32,
    /// Taşınan alanı işleyen göndericide AYNI ACK'te: tavan artışı (kapalı kol) / yeni çift
    /// yoklaması (açık kol); aşağı yönlü örnek VAR; eş koşuda (toplu teslimsiz) örnek VAR iken bu
    /// koşuda yok (bastırıldı); taşma örneği.
    raised_by_carried: u32,
    probed_by_carried: u32,
    carried_with_sample: u32,
    carried_sample_suppressed: u32,
    carried_with_overflow: u32,
    /// Taşma dalı: koşu başına kuyrukta düşen ve taşma örneği toplamı, en büyük doluluk (paket).
    queue_drops: u64,
    overflow_samples: u64,
    max_occ_pkts: usize,
}

fn b2_cells(level: u64) -> Vec<B2Cell> {
    let mut v = Vec::new();
    for t2 in (0..8u64).map(|i| 26 * SEC + i * 250 * MS) {
        for delta in (0..6u64).map(|i| 300 * MS + i * 200 * MS) {
            for k in 0..4 {
                v.push(B2Cell {
                    t2,
                    delta,
                    level,
                    k,
                });
            }
        }
    }
    v
}

/// Eş koşuda (toplu teslimsiz) "o ACK"in karşılığı: aynı cepheyi ilk kapsayan ACK.
fn twin_row<'a>(twin: &'a Out, r: &Row, t3: u64) -> Option<&'a Row> {
    twin.rows
        .iter()
        .find(|x| x.emit_ns > t3 + ONE_WAY && x.largest >= r.largest)
}

/// Dönen: sayım ve ayrıntı için seçilen hücre (öncelik: taşıyan ACK yanlış yükseliş / yoklama
/// tetikledi → kanıtlı bırakma + yol verme beklemesi → kanıtlı bırakma → bekleme açıkken).
fn b2_grid(level: u64, probe: Option<u64>, loss: u64) -> (B2Count, Option<B2Cell>) {
    let mut n = B2Count::default();
    let mut best: Option<(u8, B2Cell)> = None;
    for c in b2_cells(level) {
        let o = simulate(&b2_scn(c, probe, true, loss));
        let t3 = c.t2 + c.delta;
        n.runs += 1;
        n.queue_drops += o.queue_drops;
        n.overflow_samples += o.overflow_samples;
        n.max_occ_pkts = n.max_occ_pkts.max(o.max_occ_pkts);
        n.squeezed += u32::from(batch_squeezed(&o));
        let Some(i) = batch_row(&o) else { continue };
        let r = &o.rows[i];
        n.rows += 1;
        let waited = r.obs.alarm_defer_yield > 0;
        let evidenced = r.obs.evidence_ns.is_some();
        if r.obs.rise {
            n.rise += 1;
            n.rise_yield += u32::from(waited);
            n.rise_yield_detect += u32::from(waited && r.obs.detects > 0);
            n.rise_evidence += u32::from(evidenced);
            n.rise_yield_evidence += u32::from(waited && evidenced);
        }
        if !(carried(&o, i).1 && r.pair.is_some_and(|p| p > 7_000)) {
            continue;
        }
        if evidenced {
            n.carried_after_evidence += 1;
        } else {
            n.carried_wait_open += 1;
        }
        let raised = r.after > r.before;
        let probed = r.plan.is_some();
        n.raised_by_carried += u32::from(raised);
        n.probed_by_carried += u32::from(probed);
        n.carried_with_sample += u32::from(r.sample);
        n.carried_with_overflow += u32::from(r.overflow);
        if !r.sample {
            let twin = simulate(&b2_scn(c, probe, false, loss));
            n.carried_sample_suppressed +=
                u32::from(twin_row(&twin, r, t3).is_some_and(|x| x.sample));
        }
        let rank = if raised || probed {
            4
        } else if evidenced && waited {
            3
        } else if evidenced {
            2
        } else {
            1
        };
        if best.is_none_or(|(b, _)| rank > b) {
            best = Some((rank, c));
        }
    }
    (n, best.map(|(_, c)| c))
}

/// B2 ayrıntısı: seçilen hücrede toplu teslimli koşu ↔ eş koşu (toplu teslimsiz), bir kol.
#[allow(clippy::too_many_lines)]
fn b2_detail(arm: &str, c: B2Cell, probe: Option<u64>, loss: u64) {
    let o = simulate(&b2_scn(c, probe, true, loss));
    let twin = simulate(&b2_scn(c, probe, false, loss));
    let t3 = c.t2 + c.delta;
    let name = format!(
        "B2 {arm} | toparlanma {} s, ikinci düşüş {} s → {} B/s, toplu teslim k = {}, kayıp ‰ {loss}",
        s3(c.t2),
        s3(t3),
        c.level,
        c.k
    );
    print_timeline(&format!("{name} | toplu teslim"), &o, c.t2, t3 + 20 * SEC);
    print_timeline(&format!("{name} | eş koşu"), &twin, c.t2, t3 + 20 * SEC);
    println!(
        "{name} | toplu teslim: {:?} (tutulan seq, ardındaki, alıcıda aralık ns; tam + tam sıkışmış: \
         {}) | taşma dalı: kuyrukta düşen {} (eş {}), kuyruk tepesi {} / {QUEUE_PKTS} paket (eş {}), \
         overflow_samples {} (eş {})",
        o.batched,
        batch_squeezed(&o),
        o.queue_drops,
        twin.queue_drops,
        o.max_occ_pkts,
        twin.max_occ_pkts,
        o.overflow_samples,
        twin.overflow_samples,
    );
    let Some(i) = batch_row(&o) else {
        println!("{name} | toplu teslimi kapsayan ACK yok");
        return;
    };
    let r = &o.rows[i];
    let (second, was_carried) = carried(&o, i);
    let t = twin_row(&twin, r, t3);
    println!(
        "{name}\n  (alan) o ACK #{} [{}] çıkış {} s: pair_rate {:?} (tel {:?}); son sıfırlamadan beri \
         sayılan çiftlerin en büyük ikincisi {second} → saklanandan taşındı: {}; yol: {}; yükseliş \
         alarmı {} ({:?} s), alarm kurulduktan sonra erteleme: yol verme {}, bütçe {}; algılama {:?} \
         s, kanıt {:?} s\n  (yanlış yükseliş) göndericide {} s: tavan {} → {}; bu ACK'te yeni çift \
         yoklaması: {}\n  (aşağı yön) örnek {} (taşma {}), g {:?} ns, hüküm {} | eş koşu ACK {}",
        r.n,
        r.kind(),
        s3(r.emit_ns),
        r.pair,
        r.diag.pair_bound_wire,
        was_carried && r.pair.is_some_and(|p| p > 7_000),
        if !was_carried {
            "saklanan taşınmadı"
        } else if r.obs.evidence_ns.is_some() {
            "kanıtlı bırakma + sıkışma bayrağı (K1 kolu)"
        } else {
            "bekleme açıkken çıkan ACK (`drop_alarm_wait`)"
        },
        r.obs.rise,
        r.obs.alarm_ns.map(s3),
        r.obs.alarm_defer_yield,
        r.obs.alarm_defer_budget,
        r.obs.detect_ns.map(s3),
        r.obs.evidence_ns.map(s3),
        s3(r.proc_ns),
        r.before,
        r.after,
        if r.plan.is_some() { "EVET" } else { "hayır" },
        if r.sample { "VAR" } else { "yok" },
        r.overflow,
        r.g_ns,
        verdict_str(r.diag.verdict),
        t.map_or_else(
            || "yok".into(),
            |x| format!(
                "#{} [{}] çıkış {} s: pair_rate {:?}, örnek {} (taşma {}), g {:?} ns, tavan {} → {}",
                x.n,
                x.kind(),
                s3(x.emit_ns),
                x.pair,
                if x.sample { "VAR" } else { "yok" },
                x.overflow,
                x.g_ns,
                x.before,
                x.after
            )
        ),
    );
    // Yanlış yükseliş / yoklama tetiklendiyse kuyruk üç pencerede, iki tanımla.
    if r.plan.is_some() {
        let w = window(&o, i);
        let checked = check_window(&name, &o, i, &w);
        let code = code_verdict(&o, i);
        let vt = code.map_or(w.until, |(k, _)| o.rows[i + k - 1].proc_ns);
        println!(
            "  (yoklama) hedef {} (taban {}), pencere {}–{} s, fiilî gönderim {checked:?} | hüküm {} | \
             kuyruk ↔ {} ms: yoklama {} ; yoklama sonu → hüküm {} ; hüküm + 10 s {} ; eş koşu aynı \
             pencereler: {} ; {} ; {}",
            w.target,
            w.base,
            s3(w.start),
            s3(w.until),
            code.map_or_else(
                || "yok".into(),
                |(k, v)| format!("{k}. ACK'te {}", verdict_str(Some(v)))
            ),
            ms1(GATE_NS),
            gate_line(&o, w.start, w.until),
            gate_line(&o, w.until, vt.max(w.until)),
            gate_line(&o, vt, (vt + 10 * SEC).min(o.end_ns)),
            gate_line(&twin, w.start, w.until),
            gate_line(&twin, w.until, vt.max(w.until)),
            gate_line(&twin, vt, (vt + 10 * SEC).min(twin.end_ns)),
        );
    } else if r.after > r.before {
        let down = o
            .pace
            .iter()
            .find(|&&(t, p)| t > r.proc_ns && p <= r.before)
            .map_or(o.end_ns, |&(t, _)| t);
        println!(
            "  (tavan) {} → {} açık {}–{} s ({} s) | kuyruk ↔ {} ms: tavan açıkken {} ; sonraki 10 s {} \
             ; eş koşu aynı pencereler: {} ; {}",
            r.before,
            r.after,
            s3(r.proc_ns),
            s3(down),
            s3(down - r.proc_ns),
            ms1(GATE_NS),
            gate_line(&o, r.proc_ns, down),
            gate_line(&o, down, (down + 10 * SEC).min(o.end_ns)),
            gate_line(&twin, r.proc_ns, down),
            gate_line(&twin, down, (down + 10 * SEC).min(twin.end_ns)),
        );
    }
}

/// SENARYO B2 — #256 karşı örneği: yükseliş alarmının ACK'i yol verme nedeniyle beklerken GERÇEK
/// ikinci düşüş + iki tam datagramın toplu teslimi. Izgara: toparlanma 26,00–27,75 s (0,25 s adım)
/// × ikinci düşüş +0,3…+1,3 s (0,2 s adım) × toplu teslim ikinci düşüşten sonraki 0–3. datagram ×
/// ikinci düşüşün düzeyi {6 250, 3 125}; deney açık ve kapalı; kayıpsız ve kayıplı (‰ 200) hücre.
#[test]
fn b2_yukselis_alarmi_beklerken_ikinci_dusus_ve_toplu_teslim() {
    let mut picks = Vec::new();
    let mut grids = Vec::new();
    for level in [NARROW, NARROW / 2] {
        for loss in [0, LOSS_PERMILLE] {
            for (arm, probe) in [
                ("deney KAPALI", None),
                ("deney AÇIK", Some(PROBE_DATAGRAMS)),
            ] {
                let (n, pick) = b2_grid(level, probe, loss);
                println!("B2 ızgara | ikinci düşüş → {level} B/s | kayıp ‰ {loss} | {arm}: {n:?}");
                assert_eq!(n.runs, 192);
                // Taşma dalı: hiçbir hücrede taşma örneği alınmadı (kuyruk dolan hücreler dahil).
                assert_eq!((n.overflow_samples, n.carried_with_overflow), (0, 0));
                if let Some(c) = pick {
                    picks.push((arm, probe, loss, c));
                }
                grids.push((level, loss, probe.is_some(), n));
            }
        }
    }
    if picks.is_empty() {
        println!("B2 | ızgarada alanın saklanan değeri taşıdığı hücre yok");
    }
    for (arm, probe, loss, c) in picks {
        b2_detail(arm, c, probe, loss);
    }
    // Gözlenen (kayıpsız, ikinci düşüş 6 250): 192 koşunun 107'sinde toplu teslim tam + tam ve
    // sıkışmış; 90'ında "o ACK"in aralığında yükseliş alarmı + yol verme beklemesi + kanıtlı
    // bırakma; 5'inde alan saklanan değeri K1 koluyla, 4'ünde bekleme açıkken taşır. Taşıyan 9
    // ACK'in 6'sı aynı anda aşağı yönlü örnek verir (tavan açılmaz); kalan 3'ü deney kapalı kolda
    // tavanı açar, deney açık kolda çift yoklaması başlatır. Bastırılan aşağı yönlü örnek yok.
    let (closed, open) = (&grids[0].3, &grids[1].3);
    assert_eq!((grids[0].0, grids[0].1, grids[0].2), (NARROW, 0, false));
    assert_eq!((closed.squeezed, closed.rise_yield_evidence), (107, 90));
    assert_eq!(
        (closed.carried_after_evidence, closed.carried_wait_open),
        (5, 4)
    );
    assert_eq!(
        (
            closed.raised_by_carried,
            closed.carried_with_sample,
            closed.carried_sample_suppressed
        ),
        (3, 6, 0)
    );
    assert_eq!((open.raised_by_carried, open.probed_by_carried), (0, 3));
}

/// SENARYO B2 — ÖN KAYITLA ÇELİŞTİ. Ön kayıt (#262; 7173692949): "kanıt tetiklenen koşuda altı
/// datagramlık yoklama kapsayan aralıkla `Failed` olur". Toparlanma 26,0 s, GERÇEK ikinci düşüş
/// 26,7 s, toplu teslim ikinci düşüşten sonraki 1. datagramda: "o ACK" saklanan yükseliş değerini
/// (12 174) K1 koluyla taşır, deney açık kolda çift yoklamasını başlatır ve yoklama kapsayan 2.
/// ACK'te `Passed` olur (hat 6 250). Kapsayan aralık yoklamadan önce duran ve aynı aralıkta
/// boşalan kuyrukta başlar: aralığın net artışı negatiftir (−2 735 ms), yoklamanın izini örter.
#[test]
fn b2b_on_kayitla_celisti_tasinan_alan_yoklamayi_baslatti_ve_yoklama_gecti() {
    let c = B2Cell {
        t2: 26 * SEC,
        delta: 700 * MS,
        level: NARROW,
        k: 1,
    };
    let t3 = c.t2 + c.delta;
    let o = simulate(&b2_scn(c, Some(PROBE_DATAGRAMS), true, 0));
    let closed = simulate(&b2_scn(c, None, true, 0));
    let twin = simulate(&b2_scn(c, Some(PROBE_DATAGRAMS), false, 0));
    let i = batch_row(&o).expect("o ACK");
    let r = &o.rows[i];
    assert!(batch_squeezed(&o) && carried(&o, i).1);
    assert!(r.obs.rise && r.obs.evidence_ns.is_some(), "K1 kolu değil");
    assert_eq!(r.pair, Some(12_174));
    // Aynı ACK aşağı yönlü örnek vermedi; eş koşudaki karşılığı da vermedi (bastırma yok).
    let t = twin_row(&twin, r, t3).expect("eş koşu ACK'i");
    assert!(!r.sample && !t.sample && t.pair.is_none());
    // Deney açık: aynı ACK çift yoklamasını başlatır; hüküm kapsayan 2. ACK'te Passed.
    let w = window(&o, i);
    let (j, v) = code_verdict(&o, i).expect("hüküm");
    let vrow = &o.rows[i + j - 1];
    assert_eq!(
        (w.target, w.base, j, v.outcome),
        (PROFILE, NARROW, 2, ProbeOutcome::Passed)
    );
    assert!(full_cover(&o, vrow, &w));
    assert_eq!((v.probe_sent_datagrams, v.probe_sent_wire), (11, 6_186));
    assert!(
        vrow.g_ns.is_some_and(|g| g < 0),
        "aralığın net artışı {:?}",
        vrow.g_ns
    );
    let (during, to_verdict, after) = (
        o.queue_peak(w.start, w.until).0,
        o.queue_peak(w.until, vrow.proc_ns).0,
        o.queue_peak(vrow.proc_ns, vrow.proc_ns + 10 * SEC).0,
    );
    println!(
        "B2b | deney AÇIK: o ACK #{} → yoklama {}–{} s, {} datagram / {} B; hüküm ACK #{} Passed, \
         aralığın net artışı {:?} ns; öndeki bekleme: yoklama {} ms, yoklama sonu → hüküm {} ms, \
         hüküm + 10 s {} ms (kapı {} ms)",
        r.n,
        s3(w.start),
        s3(w.until),
        v.probe_sent_datagrams,
        v.probe_sent_wire,
        vrow.n,
        vrow.g_ns,
        ms1(during),
        ms1(to_verdict),
        ms1(after),
        ms1(GATE_NS),
    );
    assert!(during > GATE_NS && to_verdict > GATE_NS && after > GATE_NS);
    // Deney kapalı: aynı ACK tavanı doğrudan 12 500'e açar (hat 6 250); kuyruk dolar ve taşar.
    let ci = batch_row(&closed).expect("o ACK");
    let cr = &closed.rows[ci];
    assert_eq!((cr.before, cr.after), (NARROW, PROFILE));
    let (above, _) = closed.pace_above(NARROW_BAND, cr.proc_ns, closed.end_ns);
    println!(
        "B2b | deney KAPALI: o ACK #{} tavan {} → {}; tavan > {NARROW_BAND}: {} s; öndeki bekleme \
         tepesi {} ms; kuyrukta düşen {} (kuyruk tepesi {} / {QUEUE_PKTS} paket); taşma örneği {}",
        cr.n,
        cr.before,
        cr.after,
        s3(above),
        ms1(closed.queue_peak(cr.proc_ns, closed.end_ns).0),
        closed.queue_drops,
        closed.max_occ_pkts,
        closed.overflow_samples,
    );
    assert!(closed.queue_drops > 0 && closed.overflow_samples == 0);
}

/// SENARYO C — aynı SHA doğrulaması, ürün alıcılı ve ürün ayarlı koşularda: kapsamayan ACK'te
/// hüküm SONUÇSUZ; ret seviyesi yazan iki daldan (`Failed`; aşağı yönlü örnekle düşen yoklama)
/// hiçbiri işlemedi; ikinci çift yoklaması yok. Aday koşular: A'nın 26 koşusu (kovalı +
/// serileştiren) ve B1'in deney açık 16 fazı. `refuted_bps` alanı özeldir (`tests/` dizininden
/// okunmaz): alanın kendisini aynı SHA'da kütüphane birim testi
/// `pair_probe_defers_once_then_is_inconclusive_when_the_next_interval_starts_later` sınar.
#[test]
fn c_kapsamayan_ackte_sonucsuz_ret_yok_ikinci_yoklama_yok() {
    let mut cands: Vec<(String, Scn)> = Vec::new();
    for burst in [BUCKET, 0] {
        for k in 0..A_PHASES {
            cands.push((
                format!(
                    "A {} faz {k}",
                    if burst == 0 {
                        "serileştiren"
                    } else {
                        "kovalı"
                    }
                ),
                a_scn(burst, true, k, Some(PROBE_DATAGRAMS)),
            ));
        }
    }
    for k in 0..16 {
        cands.push((
            format!("B1 kovalı metin fazı {} ms", k * 125),
            b1_scn(k, Some(PROBE_DATAGRAMS), BUCKET),
        ));
    }
    let total = cands.len();
    let (mut found, mut deferred_first) = (0u32, 0u32);
    for (name, scn) in cands {
        let o = simulate(&scn);
        for i in 0..o.rows.len() {
            if o.rows[i].plan.is_none() {
                continue;
            }
            let Some((j, v)) = code_verdict(&o, i) else {
                continue;
            };
            if v.outcome != ProbeOutcome::Inconclusive {
                continue;
            }
            let w = window(&o, i);
            found += 1;
            let vrow = &o.rows[i + j - 1];
            // Hüküm kaydı: SONUÇSUZ ACK'in aralığı yoklamanın tamamını kapsamıyor.
            assert!(
                !full_cover(&o, vrow, &w),
                "{name}: kapsayan aralıkta SONUÇSUZ"
            );
            // Önceki ACK ertelendi ve o da kapsamıyordu (erteleme bir kez).
            let defers: Vec<&Row> = o.rows[i + 1..i + j - 1]
                .iter()
                .filter(|r| {
                    r.pair_verdict()
                        .is_some_and(|v| v.outcome == ProbeOutcome::Deferred)
                })
                .collect();
            assert!(defers.len() <= 1, "{name}: birden çok erteleme");
            deferred_first += u32::from(defers.len() == 1);
            for r in &defers {
                assert!(!full_cover(&o, r, &w), "{name}: kapsayan ACK ertelendi");
            }
            // Ret seviyesi yazan dallar bu yoklamada işlemedi; başarısız yoklama sayacı bu ACK'te
            // artmadı; tavan kalıcı açılmadı.
            assert!(
                !o.rows[i + 1..i + j]
                    .iter()
                    .any(|r| r.pair_verdict().is_some_and(|v| {
                        matches!(v.outcome, ProbeOutcome::Failed | ProbeOutcome::Superseded)
                    })),
                "{name}: ret yazan dal işledi"
            );
            assert!(
                vrow.after <= w.base,
                "{name}: tavan {} → {}",
                vrow.before,
                vrow.after
            );
            // İkinci çift yoklaması yok: hükümden sonra yeni aşağı yönlü örnek gelmeden başlamaz.
            let next_sample = o.rows[i + j..].iter().position(|r| r.sample);
            let second = o.rows[i + j..].iter().position(|r| r.plan.is_some());
            assert!(
                second.is_none_or(|s| next_sample.is_some_and(|x| x < s)),
                "{name}: aşağı yönlü örnek olmadan ikinci çift yoklaması"
            );
            println!(
                "C | {name}: başlatan ACK #{} (çift {:?} → hedef {}, taban {}) | pencere {}–{} s, \
                 fiilî gönderim {} datagram / {} B",
                o.rows[i].n,
                o.rows[i].pair,
                w.target,
                w.base,
                s3(w.start),
                s3(w.until),
                v.probe_sent_datagrams,
                v.probe_sent_wire,
            );
            for k in 2..=j {
                println!("{}", cover_line(&o, k, &o.rows[i + k - 1], &w));
            }
            println!(
                "  hüküm {j}. ACK'te SONUÇSUZ | ret yazan dal (Failed / aşağı yönlü örnek): işlemedi | \
                 yazılan tutma {:?} + {:?} B | tavan {} → {} | pair_probes: bu yoklamadan sonra {} → \
                 koşu sonunda {} | hükümden sonraki ilk aşağı yönlü örnek {:?}, sonraki çift yoklaması \
                 {:?} (hükümden sonraki ACK sırası; 0 = hemen sonraki)",
                vrow.diag.backlog_bytes,
                vrow.diag.extra_backlog_bytes,
                vrow.before,
                vrow.after,
                vrow.pair_probes,
                o.pair_probes,
                next_sample,
                second,
            );
        }
    }
    println!(
        "C | ürün ayarlı {total} koşuda SONUÇSUZ hükümlü çift yoklaması: {found} (önce bir kez \
         ertelenen: {deferred_first})"
    );
    assert!(
        found > 0,
        "ürün ayarlı koşularda SONUÇSUZ hüküm oluşmadı: C sınanamadı"
    );
}

/// Düzeneğin kendi sağlaması: ürün ayarının parçaları gerçekten devrede (yol verme, alındı, metin
/// kopyaları, çift kipi) ve kovalı darboğazın bilinen sayısı.
#[test]
fn duzenek_urun_ayarinda() {
    let o = simulate(&Scn::product(flat_plan(), None, 60 * SEC));
    let texts = o.sent.iter().filter(|x| x.text.is_some()).count();
    let acks = o.rows.len();
    println!(
        "düzenek | 60 s: iletim {} (metin datagramı {texts}), aynı adımda çift {}, alındı {} (hat \
         payı ‰ {}), ACK {}, yol verme ertelemesi {}, kuyrukta düşen {}, kuyruk tepesi {} paket",
        o.sent.len(),
        o.paired_sends,
        o.receipts_sent,
        o.receipts_sent * RECEIPT_WIRE * 1000 / (60 * RETURN_BPS),
        acks,
        o.yield_deferred,
        o.queue_drops,
        o.max_occ_pkts,
    );
    // 2 s'de bir metin × 5 iletim; her metne bir alındı (son metinlerinki yolda olabilir).
    assert!((145..=150).contains(&texts), "metin datagramı {texts}");
    assert!(
        (27..=30).contains(&o.receipts_sent),
        "alındı {}",
        o.receipts_sent
    );
    assert!(o.yield_deferred > 0, "yol verme hiç işlemedi");
    assert!(o.paired_sends > 0, "çift kipi hiç işlemedi");
    // Kovalı darboğaz: dolu kovadan (1 200 B) sonra 136 B + hemen ardından 1 074 B, 6 250 B/s:
    // tam boy (1 074 − 1 064) / 6 250 = 1,6 ms sonra çıkar.
    let mut bn = Bottleneck {
        plan: vec![(0, NARROW)],
        burst: BUCKET,
        fixed_at_head: true,
        credit: u128::from(BUCKET) * u128::from(SEC),
        at: 0,
        queue: VecDeque::new(),
        cap_pkts: QUEUE_PKTS,
        max_occ_pkts: 0,
    };
    let a = bn.offer(SEC, SMALL_BODY + OVH).expect("girer");
    let b = bn.offer(SEC, WIRE).expect("girer");
    assert_eq!((a.2, b.2 - a.2), (SEC, 1_600_000));
    // Kayıp hücresi: ‰ 200 anahtarlı karar 10 000 seq'te ‰ 180–220.
    let lost = (0..10_000)
        .filter(|&q| keyed_lost(q, LOSS_PERMILLE))
        .count();
    assert!((1_800..=2_200).contains(&lost), "kayıp {lost}");
}
