//! [ÇİFT YOKLAMASI · DENEY × YÜKSELİŞ KANITI · K1] Kapalı döngü dar test: göndericinin hat koruması
//! (`LineGuard`, çift yoklaması deneyi) ile ÜRÜN alıcısı (`Receiver`, bitmap ACK, #256'nın
//! `alarm_pair_carry` kuralı) aynı döngüde.
//!
//! Benzetim/birim testidir; kabul değildir. Gerçek hat (netem) koşusunun yerini tutmaz. Test
//! bugünkü davranışı BELGELER: iddialar gözlenen sonuçtur; ön kayıtla çelişen sonuç adında
//! `on_kayitla_celisti` taşıyan ayrı testte durur.
//!
//! # Düzenek (belirlenimli, RNG yok, 1 ms adım)
//!
//! * **Gönderici:** sürekli iş (tam boy datagram: 1 046 B gövde, 1 074 B tel). Hız sınırı ürünün
//!   kendi parçalarıyla: `pacer::TokenBucket` hat modeli (`set_line`, `set_line_rate`,
//!   `set_line_pairs`, `line_raise_backlog`, `line_add_backlog`) ve `LineGuard` (tavan `pace_bps`,
//!   `tick`, başlangıç bütçesi `start_budget_admits`, `on_sent_classed` + `ahead_ns`). Çağrı sırası
//!   `Sender::on_event_core` / `Sender::cc_line_guard_on_ack` ile aynı. Kova hızı profil hattı,
//!   burst 10 × MTU: bağlayan hat modelidir. BBR, uçuş penceresi, yeniden iletim ve FEC YOK.
//! * **Darboğaz:** tek FIFO, sınırsız kuyruk, kayıp yok; hız zamanla değişir (`plan`). İsteğe bağlı
//!   jeton kovası (`burst` B; 0 = saf serileştirici): kuyruk boşken kredi `burst`'e dek birikir,
//!   baştaki datagram kredi boyuna ulaşınca çıkar. Hız değişiminin serileşmekte olan datagrama
//!   etkisi iki modelle: `fixed_at_head` (varsayılan; hız datagram başa geçtiğinde sabitlenir,
//!   `ack_alarm_interval.rs` `fifo` ile aynı) ve tümlev (değişim serileşen datagrama yansır).
//!   İsteğe bağlı darboğazdan SONRA toplu teslim (`Batch`): bir datagram `hold_ns` tutulur,
//!   ardındakiyle yakın anda teslim edilir (sıra değişmez).
//! * Kayıp olmadığı için taşma örneği (`overflow_estimate`) koşulu bu düzenekte hiç oluşmaz.
//! * **İleri yayılım** 300 ms. **Alıcı:** `Receiver::with_cc_port()`, dönüş 62 B/s
//!   (`set_return_bandwidth`), bitmap açık, sıkışma beyanı 12 500 (`ack_alarm_interval.rs`
//!   `measure_flow` kurulumu); zamanlayıcı 1 ms'de bir.
//! * **Dönüş hattı:** 62 B/s serileşme (ACK datagramı = çerçeve + 22 B oturum zarfı + 28 B UDP/IP),
//!   ardından 300 ms yayılım. ACK göndericiye varınca ürün sırasıyla verilir: `note_ack_wire` →
//!   `on_first_window` → `note_pair_lower_bound` → `on_ack_measured` → `on_pair_rate` →
//!   `settle_start_budget` → hat modeli (hız, çift kipi, tutma).
//!
//! # Yoklama penceresi nasıl okunur
//!
//! `LineGuard` yoklamanın `start_ns` / `until_ns` değerini dışarı vermez (`src/` bu işte değişmez).
//! Pencere `pair_probe_target` formülüyle yeniden kurulur: `start = ACK anı + tutma / hedef`,
//! `until = start + 6 × 1 228 B / hedef`; tutma `ack_diag` (`backlog_bytes + extra_backlog_bytes`).
//! İki bağımsız sağlama: (1) `LineGuard::tick` tavanı tabana tam `until`'i izleyen ilk 1 ms adımında
//! indirir; (2) hükmün `probe_sent_wire` alanı, düzeneğin `[start, until)` içinde saydığı tel
//! baytına eşittir.
//!
//! # Kuyruk beklemesi ve 393 ms kapısı
//!
//! Bekleme = datagramın darboğaza girişinden kuyruğun başına geçmesine dek geçen süre
//! (`starved_line_sim.rs` `waits` ile aynı tanım); kapı `2 × (1 200 + 28) B / 6 250 B/s` = 392,96 ms.
//! Ayrıca darboğazda toplam kalış (giriş → çıkış) basılır.

use std::collections::VecDeque;
use std::time::Duration;

use adaptive_transport::ack::{
    AckFrame, ACK_DATAGRAM_ENVELOPE_BYTES, ACK_DELAY_UNIT_US, PAIR_RATE_NONE, RX_SPAN_NONE,
};
use adaptive_transport::line_guard::{AckDiag, ForwardInterval, LineGuard, ProbeOutcome};
use adaptive_transport::loss::E2A_ACK_DELAY_SATURATED;
use adaptive_transport::pacer::{LineModel, TokenBucket};
use adaptive_transport::rate::DEFAULT_MTU;
use adaptive_transport::receiver::{AckEmitObs, Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// Tam boy datagram: UDP gövdesi ve tel boyu (B).
const BODY: u64 = 1_046;
const OVH: u64 = 28;
const WIRE: u64 = BODY + OVH;
/// Profil hattı ve dar hat (B/s, tel).
const PROFILE: u64 = 12_500;
const NARROW: u64 = 6_250;
const RETURN_BPS: u64 = 62;
const ONE_WAY: u64 = 300 * MS;
/// Deneyin datagram bütçesi (sahip kararı: 6 datagram, tek yoklama).
const PROBE_DATAGRAMS: u64 = 6;
const MTU_WIRE: u64 = DEFAULT_MTU + OVH;
/// 393 ms kapısı: iki tam datagramın 6 250 B/s'de serileşmesi (ns).
const GATE_NS: u64 = 2 * MTU_WIRE * SEC / NARROW;
/// Kovalı darboğazın burst'ü (B).
const BUCKET: u64 = 1_200;

/// Tek FIFO darboğaz; `burst` > 0 ise jeton kovalı (modül belgesi). Kredi birimi bayt × 10⁹.
struct Bottleneck {
    plan: Vec<(u64, u64)>,
    burst: u64,
    /// `true`: baştaki datagramın hızı başa geçtiği anda sabitlenir (hız değişimi serileşmekte olan
    /// datagrama yansımaz); `false`: hız değişimi serileşen datagrama da yansır (tümlev).
    fixed_at_head: bool,
    credit: u128,
    /// Kredinin geçerli olduğu an = son çıkış anı.
    at: u64,
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

    /// Datagram `t` anında girer: `(kuyruğun başına geçtiği an, çıkış anı)`.
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
}

/// Darboğazdan sonra toplu teslim: çıkışı `from_ns`'ten sonraki ilk datagram `hold_ns` tutulur.
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
    /// `LineGuard::set_pair_probe` (deney kolu); `None` = bugünkü doğrudan yol.
    probe: Option<u64>,
    until: u64,
    batch: Option<Batch>,
}

#[derive(Debug, Clone, Copy)]
struct Sent {
    send_ns: u64,
    head_ns: u64,
    depart_ns: u64,
    arrive_ns: u64,
    /// Alıcıda bir öncekinden sonra geçen süre (ns; ilk datagramda 0).
    rx_gap_ns: u64,
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
    /// Bu ACK çift yoklaması başlattıysa yeniden kurulan pencere `(start, until)`.
    probe_window: Option<(u64, u64)>,
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
}

struct Out {
    sent: Vec<Sent>,
    rows: Vec<Row>,
    /// Tavanın `tick` ile tabana indiği anlar.
    tick_downs: Vec<u64>,
    /// `(an, tavan)`: her 1 ms adımın sonunda tavan değiştiyse.
    pace: Vec<(u64, u64)>,
    /// Alıcıda yükseliş alarmı, daralma algılaması ve kanıtlı daralma anları.
    rise_at: Vec<u64>,
    detect_at: Vec<u64>,
    drop_at: Vec<u64>,
    /// Toplu teslim: `(tutulan seq, alıcıda ardındakine aralık ns)`.
    batched: Option<(u64, u64)>,
    probe_failures: u64,
    samples: u64,
}

impl Out {
    fn send_ns(&self, seq: u64) -> u64 {
        self.sent[usize::try_from(seq).expect("seq")].send_ns
    }

    /// `[from, to)` içinde gönderilen tel baytı.
    fn wire_between(&self, from: u64, to: u64) -> u64 {
        self.sent
            .iter()
            .filter(|x| x.send_ns >= from && x.send_ns < to)
            .map(|_| WIRE)
            .sum()
    }

    /// `[from, to)` içinde gönderilenlerin darboğazdaki en büyük `(kuyruk beklemesi, toplam kalış)`.
    fn queue_peak(&self, from: u64, to: u64) -> (u64, u64) {
        self.sent
            .iter()
            .filter(|x| x.send_ns >= from && x.send_ns < to)
            .fold((0, 0), |(w, s), x| {
                (w.max(x.head_ns - x.send_ns), s.max(x.depart_ns - x.send_ns))
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

    /// Çift yoklamasını başlatan ACK'in sırası (satır dizini).
    fn probe_start(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.probe_window.is_some())
    }
}

struct InFlightAck {
    reach_ns: u64,
    emit_ns: u64,
    frame: AckFrame,
    obs: AckEmitObs,
}

fn simulate(s: &Scn) -> Out {
    let line = LineModel {
        rate_bps: PROFILE,
        datagram_overhead: OVH,
    };
    let ret = LineModel {
        rate_bps: RETURN_BPS,
        datagram_overhead: OVH,
    };
    // `Sender::set_cc_feedback_starved` ile aynı kuruluş.
    let mut g = LineGuard::new(line, ret, LineGuard::conservative_start(line));
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
    };
    let mut out = Out {
        sent: Vec::new(),
        rows: Vec::new(),
        tick_downs: Vec::new(),
        pace: vec![(0, g.pace_bps())],
        rise_at: Vec::new(),
        detect_at: Vec::new(),
        drop_at: Vec::new(),
        batched: None,
        probe_failures: 0,
        samples: 0,
    };
    let mut flight: VecDeque<(u64, u64)> = VecDeque::new();
    let mut acks: VecDeque<InFlightAck> = VecDeque::new();
    let mut ret_free = 0u64;
    let mut batch = s.batch;
    let mut held: Option<u64> = None;
    let mut last_arrive = 0u64;
    let mut last_rx: Option<u64> = None;
    let (mut rises, mut detects, mut drops) = (0u64, 0u64, 0u64);
    let mut seq = 0u64;
    let mut now = 0u64;

    while now < s.until {
        // --- Alıcı: bu adıma dek varanlar, sonra zamanlayıcı.
        while flight.front().is_some_and(|&(t, _)| t <= now) {
            let (t, q) = flight.pop_front().expect("var");
            let gap = last_rx.map_or(0, |p| t - p);
            last_rx = Some(t);
            out.sent[usize::try_from(q).expect("seq")].rx_gap_ns = gap;
            if held.is_some_and(|h| q == h + 1) {
                out.batched = held.take().map(|h| (h, gap));
            }
            // Açık dönüş: hattın önünde yalnız bu alıcının önceki ACK'leri var, veri yok.
            rx.set_return_line_ahead_ns(ret_free.saturating_sub(t));
            rx.set_uplink_yield(false);
            rx.set_next_datagram_bytes(BODY);
            let o = rx.on_datagram(
                RxDatagram {
                    seq: q,
                    unit_id: q,
                    kind: RxKind::Data,
                },
                t,
            );
            if rx.diag_rise_alarms() > rises {
                rises = rx.diag_rise_alarms();
                out.rise_at.push(t);
            }
            if rx.diag_drop_detects() > detects {
                detects = rx.diag_drop_detects();
                out.detect_at.push(t);
            }
            if rx.diag_drop_alarms() > drops {
                drops = rx.diag_drop_alarms();
                out.drop_at.push(t);
            }
            if let Some(f) = o.ack {
                push_ack(&rx, &mut acks, &mut ret_free, t, f);
            }
            if let Some(f) = rx.on_ack_timer(t) {
                push_ack(&rx, &mut acks, &mut ret_free, t, f);
            }
        }
        rx.set_return_line_ahead_ns(ret_free.saturating_sub(now));
        rx.set_uplink_yield(false);
        if let Some(f) = rx.on_ack_timer(now) {
            push_ack(&rx, &mut acks, &mut ret_free, now, f);
        }

        // --- Gönderici: `Sender::on_event_core` sırası (dolum → tick → ACK → iletim).
        bucket.refill(Duration::from_nanos(MS));
        if g.tick(now) {
            bucket.set_line_rate(g.pace_bps());
            bucket.set_line_pairs(g.pair_pacing());
            out.tick_downs.push(now);
        }
        while acks.front().is_some_and(|a| a.reach_ns <= now) {
            let a = acks.pop_front().expect("var");
            let row = on_ack(&mut g, &mut bucket, &a, now, seq, out.rows.len() + 1);
            out.rows.push(row);
        }
        loop {
            if g.start_budget_active() && !g.start_budget_admits(now, BODY) {
                break;
            }
            if !bucket.try_send(BODY) {
                break;
            }
            g.on_sent_classed(seq, now, BODY, bucket.line_ns_until_drained(), false);
            let (head_ns, depart_ns) = bn.enqueue(now, WIRE);
            let mut arrive_ns = depart_ns + ONE_WAY;
            if let Some(bt) = batch.filter(|bt| depart_ns >= bt.from_ns) {
                arrive_ns += bt.hold_ns;
                held = Some(seq);
                batch = None;
            }
            // Teslim sırası korunur (toplu teslim sıra değiştirmez).
            arrive_ns = arrive_ns.max(last_arrive + 1);
            last_arrive = arrive_ns;
            flight.push_back((arrive_ns, seq));
            out.sent.push(Sent {
                send_ns: now,
                head_ns,
                depart_ns,
                arrive_ns,
                rx_gap_ns: 0,
            });
            seq += 1;
        }
        if out.pace.last().is_some_and(|&(_, p)| p != g.pace_bps()) {
            out.pace.push((now, g.pace_bps()));
        }
        now += MS;
    }
    out.probe_failures = g.probe_failures();
    out.samples = g.samples();
    out
}

/// Çıkan ACK'i dönüş hattına koyar: 62 B/s serileşme + yayılım.
fn push_ack(
    rx: &Receiver,
    acks: &mut VecDeque<InFlightAck>,
    ret_free: &mut u64,
    at: u64,
    frame: AckFrame,
) {
    let wire = (frame.encode().len() + ACK_DATAGRAM_ENVELOPE_BYTES) as u64;
    *ret_free = (*ret_free).max(at) + wire * SEC / RETURN_BPS;
    acks.push_back(InFlightAck {
        reach_ns: *ret_free + ONE_WAY,
        emit_ns: at,
        frame,
        obs: rx.diag_ack_obs(),
    });
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

    // Çift yoklaması bu ACK'te başladıysa pencere `pair_probe_target` formülüyle (modül belgesi).
    let probe_window = (g.pair_probes() > probes0).then(|| {
        let up = g.pace_bps();
        let held = diag.backlog_bytes.unwrap_or(0) + diag.extra_backlog_bytes.unwrap_or(0);
        let start = now + held * SEC / up;
        (start, start + PROBE_DATAGRAMS * MTU_WIRE * SEC / up)
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
        probe_window,
        pair_probes: g.pair_probes(),
    }
}

fn s3(ns: u64) -> String {
    format!("{:.3}", ns as f64 / 1e9)
}

fn ms1(ns: u64) -> String {
    format!("{:.1}", ns as f64 / 1e6)
}

/// Zaman çizelgesinin bir satırı.
fn line_of(r: &Row) -> String {
    format!(
        "ACK #{} [{}] çıkış {} s, göndericide {} s, largest {}, aralık {}, g {} ms, çift {:?}, \
         alt sınır(tel) {:?}, örnek {}{}, tavan {} → {}, hız {:?}, tutma {:?}+{:?}, hüküm {}",
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
        r.diag.verdict.map_or_else(
            || "-".to_string(),
            |v| format!(
                "{:?} (çift {}, g {:?} ns, gönderilen {} B, beklenen {} ns, {} → {})",
                v.outcome,
                v.from_pair,
                v.interval_g_ns,
                v.probe_sent_wire,
                v.expect_ns,
                v.base_pace,
                v.probe_pace
            )
        ),
    )
}

fn print_timeline(name: &str, o: &Out, from_ns: u64) {
    println!("--- {name}: zaman çizelgesi ({} s'den) ---", s3(from_ns));
    for r in o.rows.iter().filter(|r| r.proc_ns >= from_ns) {
        println!("{}", line_of(r));
    }
    println!(
        "alıcı: yükseliş alarmı {:?} s; daralma algılaması {:?} s; kanıtlı daralma {:?} s | gönderici: \
         örnek {}, başarısız yoklama {}, iletim {}",
        o.rise_at.iter().map(|&t| s3(t)).collect::<Vec<_>>(),
        o.detect_at.iter().map(|&t| s3(t)).collect::<Vec<_>>(),
        o.drop_at.iter().map(|&t| s3(t)).collect::<Vec<_>>(),
        o.samples,
        o.probe_failures,
        o.sent.len(),
    );
}

/// Alıcının çift sayımının aynası (bütün datagramlar tam boy; `receiver.rs` `PairRate::on_arrival`):
/// `(from, to]` içinde varanların SAYILAN çift değerleri (B/s gövde), büyükten küçüğe. Sıkışmış çift
/// (aralık < 0,9 × 1 074 B / 12 500 B/s) ve ardındaki sayılmaz; 2 ms'den kısa aralık sayılmaz.
fn counted_pairs(o: &Out, from: u64, to: u64) -> Vec<u64> {
    let mut v = Vec::new();
    let mut after_squeeze = false;
    for x in o.sent.iter().skip(1) {
        let g = x.rx_gap_ns;
        let squeezed =
            u128::from(g) * 1_000 * u128::from(PROFILE) < 900 * u128::from(WIRE) * u128::from(SEC);
        let skip = squeezed || after_squeeze;
        after_squeeze = squeezed;
        if !skip && g >= 2 * MS && x.arrive_ns > from && x.arrive_ns <= to {
            v.push(BODY * SEC / g);
        }
    }
    v.sort_unstable_by(|a, b| b.cmp(a));
    v
}

/// ACK'in çift alanı saklanan değerden mi (`alarm_pair_carry`): alan, son sıfırlamadan (bu ACK
/// aralığındaki son daralma algılaması; yoksa önceki ACK) bu yana sayılan çiftlerin en büyük
/// ikincisinden büyükse evet. Dönen: `(aynanın en büyük ikincisi, taşındı mı)`.
fn carried(o: &Out, i: usize) -> (u64, bool) {
    let r = &o.rows[i];
    let prev = if i == 0 { 0 } else { o.rows[i - 1].emit_ns };
    let from = r.obs.detect_ns.map_or(prev, |d| d.max(prev));
    let second = counted_pairs(o, from, r.emit_ns)
        .get(1)
        .copied()
        .unwrap_or(0);
    (second, r.pair.is_some_and(|p| u64::from(p) > second))
}

fn scn(plan: Vec<(u64, u64)>, burst: u64, probe: Option<u64>, until: u64) -> Scn {
    Scn {
        plan,
        burst,
        fixed_at_head: true,
        probe,
        until,
        batch: None,
    }
}

/// Düşüş 20 s'de (12 500 → 6 250), toparlanma `rec`'te (→ 12 500).
fn recovery_plan(rec: u64) -> Vec<(u64, u64)> {
    vec![(0, PROFILE), (20 * SEC, NARROW), (rec, PROFILE)]
}

/// Aralığın uçlarının gönderim anları ve yoklamanın tamamını kapsama (sahibin kuralı: ilk uç
/// yoklama başlamadan, son uç yoklama bittikten sonra gönderilmiş). ACK'in türüne bakılmaz.
fn covers(o: &Out, r: &Row, (start, until): (u64, u64)) -> Option<(u64, u64, bool, bool)> {
    let iv = r.interval?;
    let (fs, ls) = (o.send_ns(iv.first_seq), o.send_ns(iv.last_seq));
    Some((fs, ls, fs <= start, ls >= until))
}

/// Sahibin kuralı, koddan bağımsız: yoklamayı başlatan ACK 1. ACK; 2. ACK kapsıyorsa hüküm orada,
/// değilse 3. ACK; ikisi de kapsamıyorsa `None` (SONUÇSUZ).
fn owner_rule(o: &Out, i: usize, w: (u64, u64)) -> Option<usize> {
    (2..=3).find(|&k| {
        o.rows
            .get(i + k - 1)
            .and_then(|r| covers(o, r, w))
            .is_some_and(|(_, _, a, b)| a && b)
    })
}

/// Kodun çift yoklaması hükmü: `(kaçıncı ACK, sonuç)`; erteleme hüküm sayılmaz.
fn code_verdict(o: &Out, i: usize) -> Option<(usize, ProbeOutcome)> {
    o.rows.iter().enumerate().skip(i + 1).find_map(|(j, r)| {
        r.diag
            .verdict
            .filter(|v| v.from_pair && v.outcome != ProbeOutcome::Deferred)
            .map(|v| (j - i + 1, v.outcome))
    })
}

/// Tavanın koşu sonuna dek profil hattında kaldığı ilk an.
fn full_since(o: &Out) -> Option<u64> {
    o.pace
        .iter()
        .rev()
        .take_while(|&&(_, p)| p == PROFILE)
        .last()
        .map(|&(t, _)| t)
}

/// Yeniden kurulan pencerenin iki sağlaması (modül belgesi): `tick` anı ve hükmün bayt sayımı.
fn check_window(name: &str, o: &Out, i: usize, (start, until): (u64, u64)) {
    let tick = o.tick_downs.iter().copied().find(|&t| t >= start);
    assert!(
        tick.is_some_and(|t| t >= until && t < until + MS),
        "{name}: tick {tick:?}, until {until}"
    );
    let v = o.rows[i + 1..]
        .iter()
        .find_map(|r| r.diag.verdict.filter(|v| v.from_pair))
        .unwrap_or_else(|| panic!("{name}: çift yoklamasının hükmü yok"));
    assert_eq!(
        v.probe_sent_wire,
        o.wire_between(start, until),
        "{name}: pencerede gönderilen tel baytı"
    );
}

/// Kapsama tablosu: yoklamayı başlatan ACK (1.) ve sonraki üç ACK.
fn cover_table(name: &str, o: &Out) -> (usize, (u64, u64)) {
    let i = o
        .probe_start()
        .unwrap_or_else(|| panic!("{name}: yoklama yok"));
    let w = o.rows[i].probe_window.expect("pencere");
    println!(
        "### {name}: yoklama {} → {} B/s, pencere start {} s, until {} s ({} ms)",
        o.rows[i].before,
        o.rows[i].after,
        s3(w.0),
        s3(w.1),
        ms1(w.1 - w.0)
    );
    println!(
        "| k | ACK | tür (bilgi) | çıkış s | göndericide s | aralık ilk..son seq | ilk uç gönderim s | \
         son uç gönderim s | ilk ≤ start | son ≥ until | kapsama | hüküm | interval_g_ns | \
         probe_sent_wire | expect_ns | tavan önce → sonra |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (k, r) in o.rows[i..].iter().take(4).enumerate() {
        let c = covers(o, r, w);
        let v = r.diag.verdict.filter(|v| v.from_pair);
        println!(
            "| {} | #{} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} → {} |",
            k + 1,
            r.n,
            r.kind(),
            s3(r.emit_ns),
            s3(r.proc_ns),
            r.interval
                .map_or_else(|| "yok".into(), |iv| format!("{}..{}", iv.first_seq, iv.last_seq)),
            c.map_or_else(|| "-".into(), |c| s3(c.0)),
            c.map_or_else(|| "-".into(), |c| s3(c.1)),
            c.map_or("-", |c| if c.2 { "evet" } else { "HAYIR" }),
            c.map_or("-", |c| if c.3 { "evet" } else { "HAYIR" }),
            if k == 0 {
                "(başlatan)"
            } else if c.is_some_and(|c| c.2 && c.3) {
                "TAM"
            } else {
                "yok"
            },
            v.map_or_else(|| "-".into(), |v| format!("{:?}", v.outcome)),
            v.map_or_else(|| "-".into(), |v| format!("{:?}", v.interval_g_ns)),
            v.map_or_else(|| "-".into(), |v| v.probe_sent_wire.to_string()),
            v.map_or_else(|| "-".into(), |v| v.expect_ns.to_string()),
            r.before,
            r.after,
        );
    }
    println!(
        "sahibin kuralı: {}; kod: {:?}; çift yoklaması sayısı {}; tavan kalıcı 12 500: {:?} s; \
         alıcı (toparlanmadan sonra): yükseliş {:?}, daralma algılaması {:?}, kanıtlı daralma {:?}",
        owner_rule(o, i, w).map_or_else(|| "SONUÇSUZ".into(), |k| format!("hüküm {k}. ACK'te")),
        code_verdict(o, i),
        o.rows.last().map_or(0, |r| r.pair_probes),
        full_since(o).map(s3),
        o.rise_at
            .iter()
            .filter(|&&t| t > 40 * SEC)
            .map(|&t| s3(t))
            .collect::<Vec<_>>(),
        o.detect_at
            .iter()
            .filter(|&&t| t > 40 * SEC)
            .map(|&t| s3(t))
            .collect::<Vec<_>>(),
        o.drop_at
            .iter()
            .filter(|&&t| t > 40 * SEC)
            .map(|&t| s3(t))
            .collect::<Vec<_>>(),
    );
    check_window(name, o, i, w);
    (i, w)
}

/// SENARYO A — kapsama kanıtı, gerçek toparlanma (12 500 → 6 250 @ 20 s → 12 500). Üç koşu:
/// kovasız (toparlanma 45,000 s ve 45,043 s) ve kovalı (45,000 s). Her birinde yoklamayı başlatan
/// ACK ve sonraki üç ACK için aralığın uçlarının GÖNDERİM ANLARI ile pencere kıyaslanır; hüküm
/// kapsamadan okunur, türden değil.
#[test]
fn a_kapsama_tablosu_gercek_toparlanma() {
    // (ad, burst, toparlanma, beklenen: sahibin kuralı, kodun hükmü, kalıcı tavan)
    let cells = [
        ("A kovasız, toparlanma 45,000 s", 0, 45_000 * MS),
        ("A kovasız, toparlanma 45,043 s", 0, 45_043 * MS),
        (
            "A kovalı (1 200 B), toparlanma 45,000 s",
            BUCKET,
            45_000 * MS,
        ),
    ];
    let mut got = Vec::new();
    for (name, burst, rec) in cells {
        let o = simulate(&scn(
            recovery_plan(rec),
            burst,
            Some(PROBE_DATAGRAMS),
            110 * SEC,
        ));
        print_timeline(name, &o, 40 * SEC);
        let (i, w) = cover_table(name, &o);
        // Yoklamadan önce hat 6 250'de ölçülmüş.
        assert!(
            o.rows[i].before <= 6_875 && o.rows[i].after == PROFILE,
            "{name}: yoklama {} → {}",
            o.rows[i].before,
            o.rows[i].after
        );
        // Kodun kapsama kararı gönderim anlarından okunanla aynı: kapsamayan aralık kalıcı artış
        // vermez, kapsayan aralık hükmü verir.
        for r in &o.rows[i + 1..] {
            if let Some(v) = r.diag.verdict.filter(|v| v.from_pair) {
                let full = covers(&o, r, w).is_some_and(|(_, _, a, b)| a && b);
                match v.outcome {
                    ProbeOutcome::Passed | ProbeOutcome::Failed => {
                        assert!(full, "{name}: ACK #{} kapsamadan hüküm verdi", r.n);
                    }
                    ProbeOutcome::Deferred | ProbeOutcome::Inconclusive => {
                        assert!(
                            !full,
                            "{name}: ACK #{} kapsadığı hâlde {:?}",
                            r.n, v.outcome
                        );
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            o.rows.last().map(|r| r.pair_probes),
            Some(1),
            "{name}: ikinci çift yoklaması"
        );
        got.push((
            owner_rule(&o, i, w),
            code_verdict(&o, i),
            o.rows[i + 1].kind(),
            full_since(&o).map(|t| t / SEC),
            o.rows[i + 2].after,
        ));
    }
    println!("A özeti: {got:?}");
    // Kovasız, 45,000 s: 2. ACK (türü kanıtlı daralma) yoklamanın tamamını kapsar → hüküm 2. ACK'te
    // `Passed`, tavan kalıcı 12 500.
    assert_eq!(
        got[0],
        (
            Some(2),
            Some((2, ProbeOutcome::Passed)),
            "kanıtlı daralma",
            Some(49),
            PROFILE
        )
    );
    // Kovasız, 45,043 s: 2. ACK (kanıtlı daralma) kapsamaz → ertelenir; 3. ACK de kapsamaz →
    // SONUÇSUZ; tavan açılmaz (bu koşuda hedef 4 KALDI).
    assert_eq!(got[1].0, None);
    assert_eq!(got[1].1, Some((3, ProbeOutcome::Inconclusive)));
    assert_eq!(got[1].2, "kanıtlı daralma");
    assert!(got[1].4 <= 6_875, "tavan açıldı: {}", got[1].4);
    // Kovalı, 45,000 s: aynı dal (SONUÇSUZ).
    assert_eq!(got[2].0, None);
    assert_eq!(got[2].1, Some((3, ProbeOutcome::Inconclusive)));
    assert!(got[2].4 <= 6_875, "tavan açıldı: {}", got[2].4);
}

/// Faz taramasının bir hücresi: toparlanma anı 45,000 s'den 43 ms adımla 12 faz.
#[derive(Debug, Default, PartialEq, Eq)]
struct PhaseCount {
    /// Yoklama hedefi profil hattı (12 500) / ara değer.
    target_full: u32,
    target_partial: u32,
    /// Kodun hükmü.
    passed_full: u32,
    passed_partial: u32,
    inconclusive: u32,
    other: u32,
    /// 2. ACK'in türü kanıtlı daralma; o ACK yoklamayı kapsıyor.
    ack2_drop_alarm: u32,
    ack2_drop_alarm_covers: u32,
    /// Sahibin kuralı ile kodun hükmü ayrıştı (ACK sırası ya da sonuç).
    rule_mismatch: u32,
    /// Hüküm 4. ya da sonraki ACK'te (2. ACK'in cephesi yoklama bitmeden gönderilmiş: vadesi
    /// gelmemiş ACK erteleme hakkını harcamıyor).
    verdict_after_third: u32,
    /// Bunlardan `Passed` olanlar (sahibin kuralına göre SONUÇSUZ olması gerekirken kalıcı artış).
    passed_after_third: u32,
    /// Deney kolunda / kapalı kolda tavanın 100 s içinde kalıcı 12 500 olduğu koşu sayısı.
    full_open: u32,
    full_closed: u32,
}

fn phase_cell(fixed_at_head: bool, burst: u64) -> (PhaseCount, [Option<u64>; 2], [Option<u64>; 2]) {
    let mut c = PhaseCount::default();
    // Kalıcı 12 500'e varış (toparlanmadan sonra, ns): [en erken, en geç], deney / kapalı kol.
    let (mut open, mut closed) = ([None, None], [None, None]);
    let note = |slot: &mut [Option<u64>; 2], d: u64| {
        slot[0] = Some(slot[0].map_or(d, |x: u64| x.min(d)));
        slot[1] = Some(slot[1].map_or(d, |x: u64| x.max(d)));
    };
    for k in 0..12u64 {
        let rec = 45 * SEC + k * 43 * MS;
        let mut s = scn(recovery_plan(rec), burst, Some(PROBE_DATAGRAMS), 100 * SEC);
        s.fixed_at_head = fixed_at_head;
        let o = simulate(&s);
        s.probe = None;
        let d = simulate(&s);
        if let Some(t) = full_since(&d) {
            c.full_closed += 1;
            note(&mut closed, t.saturating_sub(rec));
        }
        if let Some(t) = full_since(&o) {
            c.full_open += 1;
            note(&mut open, t.saturating_sub(rec));
        }
        let Some(i) = o.probe_start() else {
            c.other += 1;
            continue;
        };
        let w = o.rows[i].probe_window.expect("pencere");
        let full = o.rows[i].after == PROFILE;
        if full {
            c.target_full += 1;
        } else {
            c.target_partial += 1;
        }
        let rule = owner_rule(&o, i, w);
        let code = code_verdict(&o, i);
        match code {
            Some((_, ProbeOutcome::Passed)) if full => c.passed_full += 1,
            Some((_, ProbeOutcome::Passed)) => c.passed_partial += 1,
            Some((_, ProbeOutcome::Inconclusive)) => c.inconclusive += 1,
            _ => c.other += 1,
        }
        if o.rows[i + 1].obs.evidence_ns.is_some() {
            c.ack2_drop_alarm += 1;
            if covers(&o, &o.rows[i + 1], w).is_some_and(|(_, _, a, b)| a && b) {
                c.ack2_drop_alarm_covers += 1;
            }
        }
        let agree = match (rule, code) {
            (Some(k), Some((j, out))) => k == j && out != ProbeOutcome::Inconclusive,
            (None, Some((j, ProbeOutcome::Inconclusive))) => j <= 3,
            _ => false,
        };
        if !agree {
            c.rule_mismatch += 1;
        }
        if code.is_some_and(|(j, _)| j > 3) {
            c.verdict_after_third += 1;
        }
        if code.is_some_and(|(j, out)| j > 3 && out == ProbeOutcome::Passed) {
            c.passed_after_third += 1;
        }
    }
    (c, open, closed)
}

/// SENARYO A — faz taraması (12 faz × kovasız/kovalı × iki darboğaz modeli × deney/kapalı kol).
/// ÖN KAYITLA ÇELİŞTİ: ön kayıt "kapsayan ilk ACK'te `Passed` ve tavan kalıcı 12 500" idi. Ürün
/// alıcısıyla çoğu fazda yoklamanın bitişi (tavan tabana döner, varış hızı yarıya iner) alıcının
/// daralma dedektörünü tetikler; 2. ACK kanıtlı daralma alarmıdır, aralığı yoklamadan sonra başlar
/// ve kapsamaz; 3. ACK de kapsamaz → SONUÇSUZ. Hız değişimi serileşen datagrama yansıyan modelde
/// yükseliş alarmının ACK'i ara bir çift değeri taşır: yoklama geçer ama tavan 12 500'e açılmaz,
/// tek atış hakkı da harcanmıştır.
#[test]
fn a2_on_kayitla_celisti_faz_taramasi() {
    let mut all = Vec::new();
    for fixed in [true, false] {
        for burst in [0, BUCKET] {
            let (c, open, closed) = phase_cell(fixed, burst);
            println!(
                "A faz taraması | model {} | {} | hedef 12 500: {} / ara: {} | Passed→12 500: {} | \
                 Passed→ara: {} | SONUÇSUZ: {} | diğer: {} | 2. ACK kanıtlı daralma: {} (kapsayan {}) | \
                 kural≠kod: {} | hüküm 4.+ ACK'te: {} (Passed {}) | kalıcı 12 500 (100 s içinde): deney {}/12 \
                 [{:?}..{:?} s], kapalı {}/12 [{:?}..{:?} s]",
                if fixed { "başta sabit" } else { "tümlev" },
                if burst == 0 { "kovasız" } else { "kovalı" },
                c.target_full,
                c.target_partial,
                c.passed_full,
                c.passed_partial,
                c.inconclusive,
                c.other,
                c.ack2_drop_alarm,
                c.ack2_drop_alarm_covers,
                c.rule_mismatch,
                c.verdict_after_third,
                c.passed_after_third,
                c.full_open,
                open[0].map(s3),
                open[1].map(s3),
                c.full_closed,
                closed[0].map(s3),
                closed[1].map(s3),
            );
            all.push(c);
        }
    }
    let sum = |f: fn(&PhaseCount) -> u32| all.iter().map(f).sum::<u32>();
    println!(
        "A faz taraması toplam (48 koşu): Passed→12 500 {}, Passed→ara {}, SONUÇSUZ {}, diğer {}, \
         kural≠kod {}, hüküm 4.+ ACK'te {} (Passed {})",
        sum(|c| c.passed_full),
        sum(|c| c.passed_partial),
        sum(|c| c.inconclusive),
        sum(|c| c.other),
        sum(|c| c.rule_mismatch),
        sum(|c| c.verdict_after_third),
        sum(|c| c.passed_after_third),
    );
    // Ön kayıt tutsaydı her fazda Passed→12 500 olurdu.
    assert!(
        sum(|c| c.passed_full) < 48,
        "ön kayıt tuttu: bu testin adı ve belgesi güncellenmeli"
    );
    assert!(sum(|c| c.inconclusive) > sum(|c| c.passed_full));
    // Kapalı kol (bugünkü doğrudan yol) her fazda 100 s içinde kalıcı 12 500'e varır.
    assert_eq!(sum(|c| c.full_closed), 48);
}

/// SENARYO B'nin hat planı: 12 500 → 6 250 (20 s) → 12 500 (`t2`) → GERÇEK 6 250 (`t3`).
fn flap_plan(t2: u64, t3: u64) -> Vec<(u64, u64)> {
    vec![
        (0, PROFILE),
        (20 * SEC, NARROW),
        (t2, PROFILE),
        (t3, NARROW),
    ]
}

const B_T2: u64 = 26_600 * MS;
const B_T3: u64 = 27_600 * MS;
/// Bulut-D'nin koşulu: darboğazdan sonra ≥ ≈ 95 ms sıkıştırma, tam + tam.
const B_HOLD: u64 = 95 * MS;

fn b_run(probe: Option<u64>, batch: bool, until: u64) -> Out {
    let mut s = scn(flap_plan(B_T2, B_T3), 0, probe, until);
    s.batch = batch.then_some(Batch {
        from_ns: B_T3,
        hold_ns: B_HOLD,
    });
    simulate(&s)
}

/// SENARYO B — iki tam datagramın toplu teslimi (#256 karşı örneği), kapalı döngüde.
/// Hat 26,6 s'de toparlanır (yükseliş alarmı ölçülür ve ACK'i çıkar), 27,6 s'de GERÇEKTEN 6 250'ye
/// döner; darboğazdan 27,6 s'den sonra çıkan ilk datagram 95 ms tutulur (ardındakiyle aralık
/// 76,8 ms < 77,3 ms: sıkışmış tam + tam çift). Dört koşu: deney açık / kapalı × toplu teslim var /
/// yok (eş koşu).
#[test]
fn b_iki_tam_datagramin_toplu_teslimi() {
    let until = 70 * SEC;
    let open = b_run(Some(PROBE_DATAGRAMS), true, until);
    let open_twin = b_run(Some(PROBE_DATAGRAMS), false, until);
    let closed = b_run(None, true, until);
    let closed_twin = b_run(None, false, until);
    for (name, o) in [
        ("B deney açık + toplu teslim", &open),
        ("B deney açık, eş koşu (toplu teslim yok)", &open_twin),
        ("B deney kapalı + toplu teslim", &closed),
        ("B deney kapalı, eş koşu (toplu teslim yok)", &closed_twin),
    ] {
        print_timeline(name, o, 25 * SEC);
    }
    let (held, gap) = open.batched.expect("toplu teslim olmadı");
    assert!((2 * MS..77_328_000).contains(&gap), "aralık {gap} ns");
    let batch_at = open.sent[usize::try_from(held).expect("seq") + 1].arrive_ns;
    // "O ACK": toplu teslimden sonra alıcıdan çıkan ilk ACK; eş koşuda aynı andan sonraki ilk ACK.
    let after = |o: &Out| {
        o.rows
            .iter()
            .position(|r| r.emit_ns >= batch_at)
            .expect("toplu teslimden sonra ACK yok")
    };
    let (i, it) = (after(&open), after(&open_twin));
    let (r, rt) = (&open.rows[i], &open_twin.rows[it]);
    let (c, ct) = (
        &closed.rows[after(&closed)],
        &closed_twin.rows[after(&closed_twin)],
    );

    // (B1) Alan.
    let (second, was_carried) = carried(&open, i);
    println!(
        "B1 | toplu teslim: seq {held}+{} alıcıda {} ms arayla ({} s) | o ACK #{} [{}] çıkış {} s: \
         pair_rate {:?}; son sıfırlamadan beri sayılan çiftlerin en büyük ikincisi {second} → \
         saklanandan taşındı: {was_carried}; bekleme açık {}, kanıt {:?}, yükseliş {} | eş koşu ACK \
         #{} [{}] çıkış {} s: pair_rate {:?}",
        held + 1,
        ms1(gap),
        s3(batch_at),
        r.n,
        r.kind(),
        s3(r.emit_ns),
        r.pair,
        r.obs.wait_open,
        r.obs.evidence_ns.map(s3),
        r.obs.rise,
        rt.n,
        rt.kind(),
        s3(rt.emit_ns),
        rt.pair,
    );
    // Gözlenen: eski yükseliş değeri (12 174 gövde) taşınır; yol bekleme açıkken çıkan planlı
    // ACK'in birleştirmesidir (`drop_alarm_wait`), kanıtlı bırakma + sıkışma bayrağı kolu değil.
    assert_eq!(r.pair, Some(12_174), "o ACK'in alanı");
    assert!(was_carried && second == 0);
    assert!(r.obs.wait_open && r.obs.evidence_ns.is_none());
    // Eş koşu: kanıtlı daralma ACK'i, alan NONE.
    assert!(rt.obs.evidence_ns.is_some() && rt.pair.is_none());

    // (B2) Aşağı yön, aynı ACK'te.
    println!(
        "B2 | deney açık: o ACK'te örnek {} (taşma {}), g {:?} ns, alt sınır(tel) {:?}, tavan {} → {} \
         | eş koşu: örnek {} (taşma {}), g {:?} ns, alt sınır(tel) {:?}, tavan {} → {} || deney \
         kapalı: örnek {} (taşma {}), tavan {} → {} | eş koşu: örnek {} (taşma {}), tavan {} → {}",
        r.sample,
        r.overflow,
        r.g_ns,
        r.diag.pair_bound_wire,
        r.before,
        r.after,
        rt.sample,
        rt.overflow,
        rt.g_ns,
        rt.diag.pair_bound_wire,
        rt.before,
        rt.after,
        c.sample,
        c.overflow,
        c.before,
        c.after,
        ct.sample,
        ct.overflow,
        ct.before,
        ct.after,
    );
    // Gözlenen: iki koşuda da o ACK aşağı yönlü örnek vermez; taşma örneği iki koşuda da yoktur
    // (düzenek kayıpsız: taşma koşulu hiç oluşmaz, bastırma bu düzenekte SINANAMAZ).
    assert!(!r.sample && !rt.sample && !c.sample && !ct.sample);
    assert!(!r.overflow && !rt.overflow && !c.overflow && !ct.overflow);
    assert_eq!(r.diag.pair_bound_wire, Some(12_458));
    assert_eq!(rt.diag.pair_bound_wire, None);
    // İlk aşağı yönlü örnek iki koşuda da bir sonraki ACK'te.
    let first_down = |o: &Out, from: usize| {
        o.rows[from..]
            .iter()
            .find(|r| r.sample)
            .map(|r| (r.n, r.proc_ns))
            .expect("örnek yok")
    };
    let (d, dt) = (
        first_down(&closed, after(&closed)),
        first_down(&closed_twin, after(&closed_twin)),
    );
    println!(
        "B2 | deney kapalı: ilk aşağı yönlü örnek ACK #{} göndericide {} s; eş koşu ACK #{} {} s",
        d.0,
        s3(d.1),
        dt.0,
        s3(dt.1)
    );

    // (B3) Yukarı yön.
    let up = |o: &Out| {
        o.rows
            .iter()
            .find(|r| r.proc_ns >= B_T3 && r.after > r.before && r.after > 6_875)
            .map(|r| (r.n, r.kind(), r.proc_ns, r.before, r.after, r.pair))
    };
    let (above, peak) = closed.pace_above(6_875, B_T3, until);
    let (above_t, peak_t) = closed_twin.pace_above(6_875, B_T3, until);
    println!(
        "B3 | deney kapalı: tavanı açan ACK {:?}; gerçek 6 250 hatta tavan > 6 875: {} s, tepe {} \
         (eş koşu: {} s, tepe {}); o ACK'te tavan {} → {} (eş koşu {} → {})",
        up(&closed).map(|u| (u.0, u.1, s3(u.2), u.3, u.4, u.5)),
        s3(above),
        peak,
        s3(above_t),
        peak_t,
        c.before,
        c.after,
        ct.before,
        ct.after,
    );
    let ps = open.probe_start().expect("deney kolunda yoklama yok");
    let w = open.rows[ps].probe_window.expect("pencere");
    let verdict = code_verdict(&open, ps).expect("hüküm yok");
    let vrow = &open.rows[ps + verdict.0 - 1];
    println!(
        "B3 | deney açık: çift yoklaması {} kez; başlatan ACK #{} [{}] (çıkış {} s, göndericide {} s, \
         pair_rate {:?}), hedef {} → {}, pencere {}–{} s; o ACK (#{}) yeni yoklama başlatmadı (tavan \
         {} → {}); hüküm ACK #{}: {:?}",
        open.rows.last().map_or(0, |r| r.pair_probes),
        open.rows[ps].n,
        open.rows[ps].kind(),
        s3(open.rows[ps].emit_ns),
        s3(open.rows[ps].proc_ns),
        open.rows[ps].pair,
        open.rows[ps].before,
        open.rows[ps].after,
        s3(w.0),
        s3(w.1),
        r.n,
        r.before,
        r.after,
        vrow.n,
        verdict.1,
    );
    // Gözlenen: yanlış yükselişi/yoklamayı tetikleyen, toplu teslimin ACK'i DEĞİL; ondan önce çıkan
    // yükseliş alarmının ACK'idir (çıktığında hat 12 500'dü; 62 B/s dönüşte yoldayken hat düştü).
    assert!(open.rows[ps].emit_ns < B_T3 + ONE_WAY && open.rows[ps].proc_ns > B_T3);
    assert!(ps < i, "yoklamayı toplu teslimin ACK'i başlattı");
    assert_eq!(open.rows.last().map(|r| r.pair_probes), Some(1));
    assert_eq!((r.before, r.after), (6_250, 6_250));
    assert_eq!(verdict.1, ProbeOutcome::Superseded);
    assert_eq!(up(&closed).map(|u| u.0), Some(open.rows[ps].n));
    assert!(above > 6 * SEC && above_t > 6 * SEC);
    check_window("B deney açık", &open, ps, w);

    // (B4) Kuyruk: yoklama başlangıcından hüküm ACK'ine dek ve hükümden sonraki 10 s.
    let base_pace = vrow.diag.verdict.map_or(0, |v| v.base_pace);
    let base_bytes = base_pace * (w.1 - w.0) / SEC;
    let sent = open.wire_between(w.0, w.1);
    let v = vrow.proc_ns;
    let flat = simulate(&scn(
        vec![(0, PROFILE), (20 * SEC, NARROW)],
        0,
        Some(PROBE_DATAGRAMS),
        until,
    ));
    for (name, o) in [
        ("deney açık + toplu teslim", &open),
        ("deney açık, eş koşu", &open_twin),
        ("deney kapalı + toplu teslim", &closed),
        ("taban (hat 20 s'den beri 6 250, toparlanma yok)", &flat),
    ] {
        let (a, b) = (o.queue_peak(w.0, v), o.queue_peak(v, v + 10 * SEC));
        let late = o.queue_peak(v + 10 * SEC, until);
        println!(
            "B4 | {name}: kuyruk beklemesi tepesi [{}–{} s) {} ms (toplam kalış {} ms), hükümden \
             sonraki 10 s {} ms (toplam kalış {} ms), sonra koşu sonuna ({} s) dek {} ms; kapı {} ms → \
             {} / {} / {}",
            s3(w.0),
            s3(v),
            ms1(a.0),
            ms1(a.1),
            ms1(b.0),
            ms1(b.1),
            until / SEC,
            ms1(late.0),
            ms1(GATE_NS),
            if a.0 <= GATE_NS { "içinde" } else { "AŞILDI" },
            if b.0 <= GATE_NS { "içinde" } else { "AŞILDI" },
            if late.0 <= GATE_NS { "içinde" } else { "AŞILDI" },
        );
    }
    println!(
        "B4 | yoklamanın fiilen gönderdiği tel {} B (hüküm kaydı {} B), taban {} B/s × {} ms = {} B, \
         tabanın üstündeki fazla {} B",
        sent,
        vrow.diag.verdict.map_or(0, |v| v.probe_sent_wire),
        base_pace,
        ms1(w.1 - w.0),
        base_bytes,
        sent.saturating_sub(base_bytes),
    );
    let during = open.queue_peak(w.0, v);
    assert!(
        during.0 > GATE_NS,
        "yoklama sırasında kuyruk {} ns",
        during.0
    );
    assert_eq!(sent, 6 * WIRE);
    assert_eq!(sent - base_bytes, 2_760);
    // Taban koşuda (yanlış yoklama yok) aynı pencerelerde kuyruk kapının içinde.
    assert!(flat.queue_peak(w.0, until).0 <= GATE_NS);
}

/// SENARYO B — ÖN KAYITLA ÇELİŞTİ (#262 gövdesi: "393 ms'yi bir ACK aralığı boyunca aşar";
/// `pair_probe_on_an_unchanged_narrow_line_fails_once_and_drains`: hükümden sonra boşalır). Ürün
/// alıcısıyla kapalı döngüde yanlış çift yoklamasının hükmü `Superseded`'dir (aynı ACK aşağı yönlü
/// örnek verir); o ACK hat modeline tutma yazmaz (`backlog 0 + extra 0`). Gönderici tam hat hızında
/// (6 250) sürdüğü için yoklamanın fazlası darboğazda DURAN kuyruk olarak kalır: hükümden en az
/// 9 s sonra da (iki ACK aralığına yakın) kuyruk beklemesi 393 ms kapısının üstündedir; kuyruğu
/// sonraki başarısız merdiven yoklamalarının boşaltması indirir.
#[test]
fn b3_on_kayitla_celisti_yanlis_yoklamanin_kuyrugu_hukumden_sonra_bosalmaz() {
    let until = 70 * SEC;
    for (name, batch) in [("toplu teslim", true), ("eş koşu", false)] {
        let o = b_run(Some(PROBE_DATAGRAMS), batch, until);
        let ps = o.probe_start().expect("yoklama yok");
        let (j, outcome) = code_verdict(&o, ps).expect("hüküm yok");
        let vrow = &o.rows[ps + j - 1];
        let v = vrow.proc_ns;
        let post = o.queue_peak(v, v + 10 * SEC).0;
        // Kuyruk beklemesi kapının üstünde olan son iletimin anı.
        let last_over = o
            .sent
            .iter()
            .rev()
            .find(|x| x.head_ns - x.send_ns > GATE_NS)
            .map_or(0, |x| x.send_ns);
        println!(
            "B3' | {name}: hüküm ACK #{} {outcome:?} ({} s), yazılan tutma {:?} + {:?} B; hükümden \
             sonraki 10 s kuyruk tepesi {} ms (kapı {} ms); kapının üstündeki son iletim {} s \
             (hükümden {} s sonra; koşu {} s)",
            vrow.n,
            s3(v),
            vrow.diag.backlog_bytes,
            vrow.diag.extra_backlog_bytes,
            ms1(post),
            ms1(GATE_NS),
            s3(last_over),
            s3(last_over.saturating_sub(v)),
            until / SEC,
        );
        assert_eq!(outcome, ProbeOutcome::Superseded, "{name}");
        assert_eq!(
            (vrow.diag.backlog_bytes, vrow.diag.extra_backlog_bytes),
            (Some(0), Some(0)),
            "{name}: hüküm ACK'i tutma yazdı"
        );
        // Bir ACK aralığından (≈ 5 s) uzun: hükümden en az 9 s sonra da kapının üstünde.
        assert!(
            post > GATE_NS && last_over >= v + 9 * SEC,
            "{name}: {post} ns, son {last_over}"
        );
    }
}

/// Izgaranın sayımı (SENARYO B taraması).
#[derive(Debug, Default, PartialEq, Eq)]
struct CarryCount {
    runs: u32,
    /// 27 s'den sonra çıkan ACK sayısı.
    acks: u32,
    /// Saklanandan taşınan yüksek (> 7 000) alan: bekleme açıkken çıkan ACK / kanıtlı bırakma.
    carried_wait_open: u32,
    carried_after_evidence: u32,
    /// K1'in ön koşulu oluşan ACK: aynı aralıkta yükseliş alarmı VE kanıtlı bırakma.
    rise_and_evidence: u32,
    /// Taşınan alanı işleyen göndericide o ACK'te tavan artışı (kapalı kol) / yeni çift yoklaması
    /// (deney kolu).
    raised_by_carried: u32,
    probed_by_carried: u32,
}

fn carry_grid(probe: Option<u64>) -> CarryCount {
    let mut n = CarryCount::default();
    for t2 in (0..10u64).map(|k| 26_000 * MS + k * 200 * MS) {
        for delta in [400 * MS, 600 * MS, 800 * MS, 1_000 * MS] {
            for k in 0..4u64 {
                for hold in [B_HOLD, 165 * MS] {
                    let t3 = t2 + delta;
                    let mut s = scn(flap_plan(t2, t3), 0, probe, 36 * SEC);
                    s.batch = Some(Batch {
                        from_ns: t3 + k * WIRE * SEC / NARROW,
                        hold_ns: hold,
                    });
                    let o = simulate(&s);
                    n.runs += 1;
                    for (i, r) in o.rows.iter().enumerate() {
                        if r.emit_ns <= t3 + ONE_WAY {
                            continue;
                        }
                        n.acks += 1;
                        if r.obs.rise && r.obs.evidence_ns.is_some() {
                            n.rise_and_evidence += 1;
                        }
                        if !(carried(&o, i).1 && r.pair.is_some_and(|p| p > 7_000)) {
                            continue;
                        }
                        if r.obs.evidence_ns.is_some() {
                            n.carried_after_evidence += 1;
                        } else {
                            n.carried_wait_open += 1;
                        }
                        if r.after > r.before {
                            n.raised_by_carried += 1;
                        }
                        if i > 0 && r.pair_probes > o.rows[i - 1].pair_probes {
                            n.probed_by_carried += 1;
                        }
                    }
                }
            }
        }
    }
    n
}

/// SENARYO B — ızgara: toparlanma 26,0–27,8 s (0,2 s adım) × ikinci düşüş +0,4/0,6/0,8/1,0 s ×
/// toplu teslim ikinci düşüşten sonraki 0–3. datagram × tutma 95 / 165 ms; deney açık ve kapalı.
/// Sayılan: saklanan (eski yükseliş) değerin ACK'e taşındığı durumlar ve o ACK'in göndericide
/// tavanı açıp açmadığı / yoklama başlatıp başlatmadığı.
#[test]
fn b2_toplu_teslim_izgarasi() {
    let open = carry_grid(Some(PROBE_DATAGRAMS));
    let closed = carry_grid(None);
    println!("B ızgara | deney açık: {open:?}");
    println!("B ızgara | deney kapalı: {closed:?}");
    // Gözlenen: taşıma yalnız bekleme açıkken çıkan ACK'te; kanıtlı bırakmadan sonra (K1'in
    // sıkışma bayrağı kolu) hiç oluşmadı. Taşınan alan hiçbir koşuda tavanı açmadı, yoklama
    // başlatmadı.
    for n in [&open, &closed] {
        assert_eq!(n.runs, 320);
        assert_eq!(n.carried_after_evidence, 0);
        assert_eq!(n.raised_by_carried, 0);
        assert_eq!(n.probed_by_carried, 0);
    }
    assert!(open.carried_wait_open > 0 && closed.carried_wait_open > 0);
}
