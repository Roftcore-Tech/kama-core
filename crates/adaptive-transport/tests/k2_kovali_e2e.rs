//! [K2′ · #262 · kart 7175575398] Bağımsız ikinci düzenek: gerçek `Sender` × ürün `Receiver` ×
//! kovalı darboğaz, çift yoklaması deneyi açık / kapalı.
//!
//! Zemin `tests/starved_line_sim.rs`'tir (gerçek `Sender` kurulumu `sender()`, ürün alıcısının
//! sunucu yuvası kurulumu `RxModel::Product`, ACK'in göndericiye veriliş sırası
//! `product_ack_into_sender`). Gerekli parçalar buraya KOPYALANDI; o dosyaya dokunulmadı.
//!
//! # Düzenek
//!
//! * İleri darboğaz (`Hat::Kova`): tbf. Kova 1 200 B, jeton hızı hız çizelgesinden, bayt sayımı IP
//!   boyu (UDP gövdesi + 28; netem düzeneği #260 `stab overhead -14`). Paket, kovada boyu kadar
//!   jeton olduğu ilk anda çıkar; kova boşken jeton beklenir. Ardında 64 paketlik FIFO (dolunca
//!   kuyrukta düşer), çıkıştan sonra kayıp (anahtarlı) ve 300 ms yayılım. Kovadan büyük paket
//!   Linux tbf'te geçemez: sayılır ve düşürülür (`kova_asan`; ürünün en büyük datagramı 1 228 B,
//!   bu yükte dosya datagramı 1 046 + 28 = 1 074 B).
//! * Teşhis kolu (`Hat::Fifo`): saf serileştirici (her paket boyu / hız kadar), aynı FIFO sınırı.
//! * Dönüş hattı 62 B/s, 300 ms; ACK üretildiği anda kodlanır, mühürlü boyla (`ACK_SEAL`) serileşir.
//! * ACK göndericiye `client_core::runtime` ACK kolunun sırasıyla verilir (`runtime.rs` :4010
//!   `observe_ack_feedback` → :4021 `note_ack_arrival_rate` → :4024 `note_ack_pair_rate` → :4047
//!   `set_next_ack_evidence` → :4096 `note_ack_forward_interval` → :4126 `Event::AckReceived`).
//! * Yük: dosya sürekli (birimler t = 0'da sunulur, koşu boyunca bitmez) + 1 s'den itibaren 2 s'de
//!   bir metin (80 B; kopya kuralı bu düzenekte yalnız PTT şeridini kopyalar, metin kopyasız
//!   öncelikli birimdir: `starved_line_sim.rs` `sender()` ile aynı kural). PTT yok.
//!
//! # Kayıt
//!
//! `K2ROW` / `K2OZET`: #175 7175574778 + EK 7175586319 ORTAK KAYIT BİÇİMİ. Ek alanlar (biçimin
//! dışında, yalnız teşhis): `cikan_bagimsiz` (yoklama penceresinde düzeneğin kendi saydığı
//! datagram / tel baytı), `yoklama.kaynak` (`cift` | `olagan`), `olagan_hukum`, `kova_asan`.
//!
//! Koşu: `cargo test --release -p adaptive-transport --test k2_kovali_e2e -- --ignored
//! --nocapture --test-threads=1`. Rapor testi yalnız sayı basar (kabul kapısı değil); kova
//! aracının doğrulaması (`kova_iki_tam_datagram_araligi`) iddialıdır ve hep koşar.

#![allow(clippy::too_many_lines, clippy::cast_possible_truncation)]

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fmt::Write as _;

use adaptive_transport::ack::{
    AckFrame, ACK_DELAY_UNIT_US, PAIR_MIN_UDP_BYTES, PAIR_RATE_NONE, RX_SPAN_NONE,
};
use adaptive_transport::line_guard::ProbeVerdictDiag;
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{feedback_horizon_ns, Receiver, RxDatagram, RxKind};
use adaptive_transport::return_path::AckObservation;
use adaptive_transport::sender::{
    AckInfo, Event, FeedbackStarvedRule, ForwardCopyLineScale, ForwardCopyRule, OpaqueUnit, Sender,
    SrLogicalRef, Timer, TransmitDecision, TransmitKind,
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
const TEXT_SIZE: u64 = 80;
const TEXT_FIRST_NS: u64 = SEC;
const TEXT_PERIOD_NS: u64 = 2 * SEC;
const FILE_UNITS: u64 = 1_500;
/// `starved_line_sim.rs` ürün kopya kuralı (yalnız PTT şeridi; bu yükte PTT yok).
const PRODUCT_COPIES: u8 = 6;
const PRODUCT_MIN_COPIES: u8 = 5;
const PRODUCT_SPACING_NS: u64 = 200 * MS / 7;
/// tbf kovası (B, IP boyu).
const BUCKET_BYTES: u64 = 1_200;
const QUEUE_PKTS: usize = 64;
/// Çift yoklaması deneyinin bütçesi (sahip kararı 7172579876).
const PAIR_PROBE_DATAGRAMS: u64 = 6;
/// Hedef 4: 3. ACK'e dek kalıcı tavan ≥ bu (B/s tel).
const GOAL4_BPS: u64 = 11_875;
/// Sahte kol: kalıcı tavan bunu aşarsa sahte artış.
const FALSE_RAISE_BPS: u64 = 6_875;
/// Kuyruk kapısı (ns): öndeki iki tam datagramın 12 500'de serileşmesi ≈ 393 ms.
const GATE_393_NS: u64 = 393 * MS;
const DROP_AT_NS: u64 = 8 * SEC;
const RECOVER_AT_NS: u64 = 20 * SEC;
const DROP_TO: u64 = 6_250;
/// Alıcının sıkışma sınaması (`receiver.rs` `PAIR_SQUEEZE_PERMILLE`, `PAIR_SQUEEZE_OVERHEAD`;
/// crate içi sabitler) ve çiftin en kısa aralığı (`PAIR_MIN_GAP_NS`).
const PAIR_SQUEEZE_PERMILLE: u64 = 900;
const PAIR_SQUEEZE_OVERHEAD: u64 = 28;
const PAIR_MIN_GAP_NS: u64 = 2 * MS;

// ───────────────────────────── kayıp (anahtarlı) ─────────────────────────────

/// `starved_line_sim.rs` / ürün fikstürü `asym_product_outcome` karması.
fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn keyed_permille(seed: u64, class: u8, id: u64, ordinal: u32) -> u64 {
    let mut h = splitmix64(seed);
    h = splitmix64(h ^ (1u64 << 8 | u64::from(class)));
    h = splitmix64(h ^ id);
    h = splitmix64(h ^ u64::from(ordinal));
    h % 1000
}

fn key_of(d: &TransmitDecision) -> (u8, u64) {
    match d.send_record.map(|r| r.logical_ref) {
        Some(SrLogicalRef::Fec { block_id, esi }) if d.kind == TransmitKind::FecParity => {
            (0x01, u64::from(block_id) << 16 | u64::from(esi))
        }
        _ => (0x00, d.unit_id),
    }
}

// ───────────────────────────── ileri darboğaz ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hat {
    Kova,
    Fifo,
}

impl Hat {
    fn label(self) -> &'static str {
        match self {
            Self::Kova => "kova",
            Self::Fifo => "fifo",
        }
    }
}

struct Fwd {
    hat: Hat,
    plan: Vec<(u64, u64)>,
    loss_permille: u64,
    seed: u64,
    ordinal: HashMap<(u8, u64), u32>,
    /// Kuyruktaki paketlerin çıkış anları (doluluk).
    queued: VecDeque<u64>,
    /// Son paketin çıkış anı.
    last_out: u64,
    /// Kova: jeton (bayt × 1e9) ve o jetonun geçerli olduğu an.
    tokens_nb: u128,
    tokens_at: u64,
    /// (varış, seq, birim, UDP).
    wire: VecDeque<(u64, u64, u64, u64)>,
    /// (gönderim anı, öndeki bekleme ns — kendi serileşmesi hariç).
    waits: Vec<(u64, u64)>,
    queue_drops: u64,
    bucket_oversize: u64,
}

impl Fwd {
    fn new(hat: Hat, plan: Vec<(u64, u64)>, loss_permille: u64, seed: u64) -> Self {
        Self {
            hat,
            plan,
            loss_permille,
            seed,
            ordinal: HashMap::new(),
            queued: VecDeque::new(),
            last_out: 0,
            tokens_nb: u128::from(BUCKET_BYTES) * u128::from(SEC),
            tokens_at: 0,
            wire: VecDeque::new(),
            waits: Vec::new(),
            queue_drops: 0,
            bucket_oversize: 0,
        }
    }

    fn rate_at(&self, t: u64) -> u64 {
        self.plan
            .iter()
            .rev()
            .find(|&&(from, _)| from <= t)
            .map_or(self.plan[0].1, |&(_, r)| r)
    }

    /// `[a, b)` aralığında biriken jeton (bayt × 1e9), hız çizelgesine göre parça parça.
    fn accrue(&self, a: u64, b: u64) -> u128 {
        let mut sum = 0u128;
        let mut t = a;
        while t < b {
            let r = self.rate_at(t);
            let next = self
                .plan
                .iter()
                .map(|&(from, _)| from)
                .filter(|&from| from > t)
                .min()
                .unwrap_or(u64::MAX)
                .min(b);
            sum += u128::from(r) * u128::from(next - t);
            t = next;
        }
        sum
    }

    /// `from` anından itibaren `need` (bayt × 1e9) jetonun biriktiği ilk an.
    fn time_to_accrue(&self, from: u64, need: u128) -> u64 {
        let mut left = need;
        let mut t = from;
        loop {
            let r = u128::from(self.rate_at(t).max(1));
            let next = self.plan.iter().map(|&(f, _)| f).filter(|&f| f > t).min();
            let cap = next.map(|n| r * u128::from(n - t));
            match cap {
                Some(c) if c < left => {
                    left -= c;
                    t = next.expect("var");
                }
                _ => {
                    let dt = left.div_ceil(r);
                    return t + u64::try_from(dt).unwrap_or(u64::MAX);
                }
            }
        }
    }

    /// Paket darboğaza verildi; çıkış anını döner (`None` = kuyrukta / kovada düştü).
    fn send(&mut self, now: u64, seq: u64, udp: u64, unit: u64, key: (u8, u64)) -> Option<u64> {
        let wire = udp + OVH;
        let ordinal = {
            let n = self.ordinal.entry(key).or_insert(0);
            let o = *n;
            *n += 1;
            o
        };
        while self.queued.front().is_some_and(|&t| t <= now) {
            self.queued.pop_front();
        }
        if self.queued.len() >= QUEUE_PKTS {
            self.queue_drops += 1;
            return None;
        }
        let earliest = self.last_out.max(now);
        let out = match self.hat {
            Hat::Fifo => earliest + wire * SEC / self.rate_at(earliest),
            Hat::Kova => {
                if wire > BUCKET_BYTES {
                    self.bucket_oversize += 1;
                    return None;
                }
                let cap = u128::from(BUCKET_BYTES) * u128::from(SEC);
                let have = (self.tokens_nb + self.accrue(self.tokens_at, earliest)).min(cap);
                let need = u128::from(wire) * u128::from(SEC);
                let out = if have >= need {
                    earliest
                } else {
                    self.time_to_accrue(earliest, need - have)
                };
                let at_out = if out == earliest { have } else { need };
                self.tokens_nb = at_out - need;
                self.tokens_at = out;
                out
            }
        };
        // Öndeki bekleme: çıkış − sunum − kendi serileşmesi (çıkıştaki hızla).
        let own = wire * SEC / self.rate_at(out.saturating_sub(1).max(earliest));
        self.waits
            .push((now, out.saturating_sub(now).saturating_sub(own)));
        self.last_out = out;
        self.queued.push_back(out);
        let lost = self.loss_permille > 0
            && keyed_permille(self.seed, key.0, key.1, ordinal) < self.loss_permille;
        if !lost {
            self.wire.push_back((out + ONE_WAY_NS, seq, unit, udp));
        }
        Some(out)
    }
}

// ───────────────────────────── dönüş hattı ─────────────────────────────

struct Ret {
    free_at: u64,
    last_arrival: u64,
    /// (varış, UDP, çerçeve, ölçülü).
    wire: VecDeque<(u64, u64, Vec<u8>)>,
}

impl Ret {
    fn send(&mut self, now: u64, frame: &AckFrame) {
        let bytes = frame.encode();
        let udp = bytes.len() as u64 + ACK_SEAL;
        let start = self.free_at.max(now);
        self.free_at = start + (udp + OVH) * SEC / PROFILE_DOWN;
        let arrival = (self.free_at + ONE_WAY_NS).max(self.last_arrival);
        self.last_arrival = arrival;
        self.wire.push_back((arrival, udp, bytes));
    }
}

// ───────────────────────────── alıcının çift kuralının aynası ─────────────────────────────

/// `receiver.rs` `PairRate::on_arrival` (taban `125b9afa` = bu dalın alıcısı): sıkışma sınaması,
/// ardındaki çiftin atlanması, `b ≥ PAIR_MIN_UDP_BYTES` ve `g ≥ 2 ms`. Yalnız sayar (ACK başına
/// sayılan / sıkışmış tam boy çift); alıcının kendi değerini ETKİLEMEZ.
#[derive(Default)]
struct PairMirror {
    prev: Option<u64>,
    after_squeeze: bool,
    counted: u64,
    squeezed_full: u64,
    /// Alıcının alanının aynası: son ACK'ten bu yana sayılan çiftlerin en büyük iki değeri.
    top: [u64; 2],
}

impl PairMirror {
    fn on_arrival(&mut self, now: u64, udp: u64) {
        if let Some(p) = self.prev {
            let g = now.saturating_sub(p);
            let squeezed = u128::from(g) * 1_000 * u128::from(PROFILE_UP)
                < u128::from(PAIR_SQUEEZE_PERMILLE)
                    * u128::from(udp + PAIR_SQUEEZE_OVERHEAD)
                    * u128::from(SEC);
            let skip = squeezed || self.after_squeeze;
            self.after_squeeze = squeezed;
            if squeezed && udp >= PAIR_MIN_UDP_BYTES {
                self.squeezed_full += 1;
            }
            if !skip && udp >= PAIR_MIN_UDP_BYTES && g >= PAIR_MIN_GAP_NS {
                self.counted += 1;
                let r = udp * SEC / g;
                if r > self.top[0] {
                    self.top = [r, self.top[0]];
                } else if r > self.top[1] {
                    self.top[1] = r;
                }
            }
        }
        self.prev = Some(now);
    }

    /// (sayılan, sıkışmış tam boy, aynanın alanı — tel B/s; çift yoksa `None`).
    fn take(&mut self) -> (u64, u64, Option<u64>) {
        let field = (self.top[1] > 0).then(|| self.top[1] * (MTU + OVH) / MTU);
        let r = (self.counted, self.squeezed_full, field);
        self.counted = 0;
        self.squeezed_full = 0;
        self.top = [0, 0];
        r
    }
}

// ───────────────────────────── gönderici kurulumu ─────────────────────────────

/// `starved_line_sim.rs` `sender()` (ürünün geri bildirimi aç kurulumu, `raw_copy_rule = false`).
fn sender(pair_probe: bool) -> Sender {
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
        below_id: TEXT_ID,
        priority_below_id: FILE_BASE_ID,
        copies: PRODUCT_COPIES,
        spacing_ns: PRODUCT_SPACING_NS,
    }));
    s.set_forward_copy_line_scale(Some(ForwardCopyLineScale {
        declared_bps: PROFILE_UP,
        min_copies: PRODUCT_MIN_COPIES,
    }));
    s.set_pair_probe_datagrams(pair_probe.then_some(PAIR_PROBE_DATAGRAMS));
    s.set_diag_line_guard_trace(true);
    s
}

/// `starved_line_sim.rs` `RxModel::Product` (sunucu yuvası: kota 4, bütçe dönüş hattı, CAPS kolu).
fn receiver() -> Receiver {
    let mut r = Receiver::with_cc_port();
    r.set_quota_r(4);
    r.set_return_bandwidth(Some(PROFILE_DOWN));
    r.set_ack_rereport(false);
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(Some(PROFILE_UP));
    r.set_ack_bitmap_ptt_lean(false);
    r
}

/// Gönderilen seq'ler (çalışma zamanının PN → motor seq tablosu; düzenekte PN = seq).
#[derive(Default)]
struct Sent {
    seqs: BTreeSet<u64>,
    last_seq: u64,
    any: bool,
    datagrams: u64,
}

/// `starved_line_sim.rs` `product_ack_into_sender` (aralık kolu açık; `measured` hep doğru).
/// Döner: (ACK, ölçüm taşıyor mu, çift alanı).
fn ack_into_sender(
    s: &mut Sender,
    tx: &Sent,
    bytes: &[u8],
    udp: u64,
    now: u64,
) -> (AckInfo, bool, u16) {
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
    let pair = frame
        .bitmap
        .as_ref()
        .map_or(PAIR_RATE_NONE, |bm| bm.pair_rate);
    if let Some(bm) = frame.bitmap.as_ref() {
        s.note_ack_pair_rate(bm.pair_rate);
    }
    let expected = if tx.any { tx.last_seq + 1 } else { 0 };
    let pns = frame.acked_full(expected);
    let in_table = |p: u64| tx.seqs.contains(&p);
    let evidence = frame.evidence_span_full(expected);
    if let Some((lo, hi)) = evidence {
        let span = match (
            tx.seqs.range(lo..=hi).next().copied(),
            tx.seqs.range(lo..=hi).next_back().copied(),
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
            let span_ns = (bm.rx_span != RX_SPAN_NONE)
                .then(|| u64::from(bm.rx_span) * ACK_DELAY_UNIT_US * 1000);
            has_span = span_ns.is_some();
            s.note_ack_forward_interval(first, last, span_ns, &received);
        }
    }
    let acked_seqs = pns.into_iter().filter(|&p| in_table(p)).collect();
    (
        AckInfo {
            acked_seqs,
            ack_delay: frame.ack_delay_duration(),
        },
        has_span,
        pair,
    )
}

/// Alıcının `pair_rate` alanının (UDP gövde B/s) tel karşılığı (`LineGuard::pair_wire_bps`).
fn pair_wire(p: u16) -> Option<u64> {
    (p != PAIR_RATE_NONE).then(|| u64::from(p) * (MTU + OVH) / MTU)
}

// ───────────────────────────── kollar ─────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kol {
    Toparlanma,
    Sahte,
}

impl Kol {
    fn label(self) -> &'static str {
        match self {
            Self::Toparlanma => "toparlanma",
            Self::Sahte => "sahte",
        }
    }

    fn plan(self) -> Vec<(u64, u64)> {
        match self {
            Self::Toparlanma => vec![
                (0, PROFILE_UP),
                (DROP_AT_NS, DROP_TO),
                (RECOVER_AT_NS, PROFILE_UP),
            ],
            Self::Sahte => vec![(0, PROFILE_UP), (DROP_AT_NS, DROP_TO)],
        }
    }

    /// Olay anı: toparlanma = hattın 12 500'e döndüğü an; sahte = hattın 6 250'ye indiği an.
    fn t0(self) -> u64 {
        match self {
            Self::Toparlanma => RECOVER_AT_NS,
            Self::Sahte => DROP_AT_NS,
        }
    }

    fn t_max(self) -> u64 {
        match self {
            Self::Toparlanma => 60 * SEC,
            Self::Sahte => 110 * SEC,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Cell {
    kol: Kol,
    deney: bool,
    hat: Hat,
    loss_permille: u64,
    seed: u64,
}

/// Gözlenen çift yoklaması (düzeneğin kendi izi: tavanın ACK dışı bir olayda ya da ACK'te çift
/// hedefine çıkışı).
#[derive(Debug, Clone, Copy)]
struct ProbeSeen {
    target: u64,
    base: u64,
    start: u64,
    end: Option<u64>,
    dgrams: u64,
    wire: u64,
    from_pair: bool,
    /// Hüküm verildi (ertelemeden başka bir sonuç ya da aşağı örnekle düştü).
    closed: bool,
    /// Son hükmün (erteleme dahil) verildiği ACK'in anı.
    verdict_at: Option<u64>,
}

#[derive(Debug, Clone)]
struct Row {
    sira: u64,
    t: u64,
    olculu: bool,
    cephe: bool,
    cift_tel: Option<u64>,
    yoklama: Option<ProbeSeen>,
    verdict: Option<ProbeVerdictDiag>,
    pace: u64,
    kalici: u64,
    inflight: u64,
    pencere: Option<u64>,
    pair_probes: u64,
    kuyruk_tepe_ns: u64,
    sayilan: u64,
    sikismis: u64,
    sifirlama: u64,
    kanitli: u64,
    /// Alıcının çift kuralının aynasının bu ACK aralığındaki alanı (tel B/s).
    ayna: Option<u64>,
    /// Hat korumasının bu ACK'teki kararı (`LineGuardAckDecision`).
    karar: String,
}

fn json_probe(p: &ProbeSeen, v: Option<&ProbeVerdictDiag>) -> String {
    let (dg, wire, bas, bit) = match v.filter(|v| v.from_pair) {
        Some(v) => (
            v.probe_sent_datagrams,
            v.probe_sent_wire,
            v.probe_start_ns,
            Some(v.probe_until_ns),
        ),
        None => (p.dgrams, p.wire, p.start, p.end),
    };
    format!(
        "{{\"hedef\":{},\"bas_ms\":{},\"bit_ms\":{},\"cikan_dgram\":{dg},\"cikan_tel\":{wire},\"cikan_bagimsiz\":[{},{}],\"kaynak\":\"{}\"}}",
        p.target,
        bas / MS,
        bit.map_or("null".to_string(), |b| (b / MS).to_string()),
        p.dgrams,
        p.wire,
        if p.from_pair { "cift" } else { "olagan" },
    )
}

fn row_json(c: &Cell, r: &Row) -> String {
    let probe_active = r.yoklama.is_some_and(|p| !p.closed && p.end.is_none());
    let hukum = r
        .verdict
        .filter(|v| v.from_pair)
        .map_or("null".to_string(), |v| format!("\"{:?}\"", v.outcome));
    let olagan = r
        .verdict
        .filter(|v| !v.from_pair)
        .map_or("null".to_string(), |v| format!("\"{:?}\"", v.outcome));
    format!(
        "K2ROW {{\"kol\":\"{}\",\"deney\":\"{}\",\"hat\":\"{}\",\"kayip_permil\":{},\"tohum\":{},\"ack_sira\":{},\"t_ms\":{},\"olculu\":{},\"cephe_ilerledi\":{},\"cift_tel\":{},\"yoklama\":{},\"kapsadi\":{},\"hukum\":{hukum},\"olagan_hukum\":{olagan},\"yoklama_hizi\":{},\"kalici_tavan\":{},\"inflight\":{},\"etkin_pencere\":{},\"kapi_reddi\":null,\"cift_yoklama_sayisi\":{},\"kuyruk_tepe_ms\":{:.1},\"karar\":\"{}\",\"cift_alici\":{{\"sayilan\":{},\"sikismis\":{},\"sifirlama\":{},\"kanitli_birakma\":{},\"ayna_alan\":{}}}}}",
        c.kol.label(),
        if c.deney { "acik" } else { "kapali" },
        c.hat.label(),
        c.loss_permille,
        c.seed,
        r.sira,
        r.t / MS,
        r.olculu,
        r.cephe,
        r.cift_tel.map_or("null".to_string(), |v| v.to_string()),
        r.yoklama
            .map_or("null".to_string(), |p| json_probe(&p, r.verdict.as_ref())),
        r.verdict.map_or("null".to_string(), |v| v.covered.to_string()),
        if probe_active {
            r.pace.to_string()
        } else {
            "null".to_string()
        },
        r.kalici,
        r.inflight,
        r.pencere.map_or("null".to_string(), |w| w.to_string()),
        r.pair_probes,
        r.kuyruk_tepe_ns as f64 / 1e6,
        r.karar,
        r.sayilan,
        r.sikismis,
        r.sifirlama,
        r.kanitli,
        r.ayna.map_or("null".to_string(), |v| v.to_string()),
    )
}

struct Run {
    rows: Vec<Row>,
    bucket_oversize: u64,
    queue_drops: u64,
}

fn run(c: Cell) -> Run {
    let mut s = sender(c.deney);
    let mut r = receiver();
    let mut fwd = Fwd::new(c.hat, c.kol.plan(), c.loss_permille, c.seed);
    let mut ret = Ret {
        free_at: 0,
        last_arrival: 0,
        wire: VecDeque::new(),
    };
    let mut tx = Sent::default();
    let mut mirror = PairMirror::default();
    // Yoklama penceresinde çıkan datagramlar: (an, tel baytı).
    let mut sends: Vec<(u64, u64)> = Vec::new();
    let mut probes: Vec<ProbeSeen> = Vec::new();
    let mut pace = s.diag_line_guard_pace().map_or(PROFILE_UP, |p| p.0);
    let mut last_pair_tel: Option<u64> = None;
    let mut rows = Vec::new();
    let mut sira = 0u64;
    let mut max_largest: Option<u64> = None;
    let mut waits_seen = 0usize;
    let mut trace_seen = s.diag_line_guard_trace_total();
    let mut detects = r.diag_drop_detects();
    let mut alarms = r.diag_drop_alarms();
    let mut file_offered = false;
    let mut pair_starts: BTreeSet<u64> = BTreeSet::new();
    let mut texts = 0u64;
    let mut now = 0u64;

    let emit = |decs: Vec<TransmitDecision>,
                now: u64,
                fwd: &mut Fwd,
                tx: &mut Sent,
                sends: &mut Vec<(u64, u64)>| {
        for d in decs {
            if d.kind == TransmitKind::CsProbe {
                continue;
            }
            tx.datagrams += 1;
            tx.last_seq = tx.last_seq.max(d.seq);
            tx.seqs.insert(d.seq);
            tx.any = true;
            sends.push((now, d.size + OVH));
            let key = key_of(&d);
            let _ = fwd.send(now, d.seq, d.size, d.unit_id, key);
        }
    };
    // Tavan değişimini izler: ACK dışı olayda ya da ACK'te yükselen tavan yoklamanın başı,
    // yoklama tabanına inen tavan sonu.
    let track = |s: &Sender,
                 now: u64,
                 at_ack: bool,
                 pace: &mut u64,
                 probes: &mut Vec<ProbeSeen>,
                 last_pair_tel: Option<u64>| {
        let p = s.diag_line_guard_pace().map_or(PROFILE_UP, |p| p.0);
        if p > *pace {
            let open = probes.last().is_some_and(|q| !q.closed && q.end.is_none());
            // Aynı yoklamanın aynı ms'deki ikinci açılışı (tavan inip yeniden çıktı): kayıt birleşir.
            let reopen = probes
                .last_mut()
                .filter(|q| !q.closed && q.target == p && now.saturating_sub(q.start) <= 2 * MS);
            if let Some(q) = reopen {
                q.end = None;
                *pace = p;
                return;
            }
            // Aynı hedefle ≤ 2 ms önce açılmış bir kayıt varsa (hükmü verilmiş olsa da) yenisi açılmaz.
            // Hükmü bekleyen (tavanı tabana inmiş) yoklamanın hedefine çıkış = kalıcı yükseltme
            // (`Passed`), yeni yoklama değil.
            let dup = probes.iter().any(|q| {
                q.target == p
                    && (now.saturating_sub(q.start) <= 2 * MS || (!q.closed && q.end.is_some()))
            });
            if !open && !dup {
                // Çift yoklaması: tavan çift hedefine çıktı (min(çift, profil); ≥ %95 → profil).
                let pair_target = last_pair_tel.map(|w| {
                    let w = w.min(PROFILE_UP);
                    if w * 100 >= PROFILE_UP * 95 {
                        PROFILE_UP
                    } else {
                        w
                    }
                });
                let pair_hit = pair_target == Some(p);
                if !c.deney && at_ack && pair_hit {
                    // Deney kapalı: çift ölçüsü tavanı doğrudan açtı (yoklama değil).
                    *pace = p;
                    return;
                }
                let from_pair = c.deney && pair_hit;
                probes.push(ProbeSeen {
                    target: p,
                    base: *pace,
                    start: now,
                    end: None,
                    dgrams: 0,
                    wire: 0,
                    from_pair,
                    closed: false,
                    verdict_at: None,
                });
            }
        } else if p < *pace {
            if let Some(q) = probes.last_mut().filter(|q| q.end.is_none()) {
                q.end = Some(now);
            }
        }
        *pace = p;
    };

    while now < c.kol.t_max() {
        if !file_offered {
            file_offered = true;
            for k in 0..FILE_UNITS {
                let decs = s.on_event(
                    Event::AppData(OpaqueUnit {
                        id: FILE_BASE_ID + k,
                        size: FILE_UNIT,
                    }),
                    now,
                );
                emit(decs, now, &mut fwd, &mut tx, &mut sends);
            }
            track(&s, now, false, &mut pace, &mut probes, last_pair_tel);
        }
        if now >= TEXT_FIRST_NS + texts * TEXT_PERIOD_NS {
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: TEXT_ID + texts,
                    size: TEXT_SIZE,
                }),
                now,
            );
            texts += 1;
            emit(decs, now, &mut fwd, &mut tx, &mut sends);
            track(&s, now, false, &mut pace, &mut probes, last_pair_tel);
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut tx, &mut sends);
        track(&s, now, false, &mut pace, &mut probes, last_pair_tel);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut tx, &mut sends);
            track(&s, now, false, &mut pace, &mut probes, last_pair_tel);
        }
        // Alıcı.
        while fwd.wire.front().is_some_and(|&(t, ..)| t <= now) {
            let (_, seq, uid, udp) = fwd.wire.pop_front().expect("var");
            mirror.on_arrival(now, udp);
            r.set_return_line_ahead_ns(ret.free_at.saturating_sub(now));
            r.set_uplink_yield(ret.free_at > now);
            r.set_next_datagram_bytes(udp);
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: RxKind::Data,
                },
                now,
            );
            if let Some(f) = out.ack {
                ret.send(now, &f);
            }
        }
        r.set_return_line_ahead_ns(ret.free_at.saturating_sub(now));
        r.set_uplink_yield(ret.free_at > now);
        if let Some(f) = r.on_ack_timer(now) {
            ret.send(now, &f);
        }
        // Gönderici: varan ACK'ler.
        while ret.wire.front().is_some_and(|&(t, ..)| t <= now) {
            let (_, udp, bytes) = ret.wire.pop_front().expect("var");
            let (sayilan, sikismis, ayna) = mirror.take();
            let d_now = r.diag_drop_detects();
            let a_now = r.diag_drop_alarms();
            let (ack, olculu, pair) = ack_into_sender(&mut s, &tx, &bytes, udp, now);
            let largest = ack.acked_seqs.iter().copied().max();
            let cephe = largest.is_some_and(|l| max_largest.is_none_or(|m| l > m));
            if cephe {
                max_largest = largest;
            }
            last_pair_tel = pair_wire(pair);
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut tx, &mut sends);
            track(&s, now, true, &mut pace, &mut probes, last_pair_tel);
            let total = s.diag_line_guard_trace_total();
            let rec = (total > trace_seen)
                .then(|| s.diag_line_guard_trace().last().cloned())
                .flatten();
            let verdict = rec.as_ref().and_then(|t| t.diag.verdict);
            let karar = rec
                .as_ref()
                .map_or("kayit_yok".to_string(), |t| format!("{:?}", t.decision));
            trace_seen = total;
            // Hüküm (ertelemeden başka) kaydı kapatır; aşağı örnek kayıtlı yoklamayı düşürür.
            if let Some(v) = verdict {
                let done = v.outcome != adaptive_transport::line_guard::ProbeOutcome::Deferred;
                if let Some(q) = probes.iter_mut().rev().find(|q| !q.closed) {
                    // Hüküm kaydı yoklamanın türünü bilir: düzeneğin sınıflaması düzeltilir.
                    q.from_pair = v.from_pair;
                    q.verdict_at = Some(now);
                    if done {
                        q.closed = true;
                        q.end.get_or_insert(now);
                    }
                }
            }
            if now < c.kol.t0() {
                waits_seen = fwd.waits.len();
                detects = d_now;
                alarms = a_now;
                continue;
            }
            sira += 1;
            let tepe = fwd.waits[waits_seen..]
                .iter()
                .filter(|&&(t, _)| t >= c.kol.t0())
                .map(|&(_, w)| w)
                .max()
                .unwrap_or(0);
            waits_seen = fwd.waits.len();
            // Kayıtlı çift yoklaması (hüküm bu ACK'te verildiyse o da gösterilir).
            let yoklama = probes
                .iter()
                .rev()
                .find(|p| p.from_pair && (!p.closed || p.verdict_at == Some(now)))
                .copied();
            // Kalıcı tavan: tavanı yükseltmiş açık bir yoklama varsa onun tabanı.
            let kalici = probes
                .iter()
                .rev()
                .find(|p| !p.closed && p.end.is_none())
                .map_or(pace, |p| p.base);
            rows.push(Row {
                sira,
                t: now,
                olculu,
                cephe,
                cift_tel: last_pair_tel,
                yoklama,
                verdict,
                pace,
                kalici,
                inflight: s.inflight_bytes(),
                pencere: s.diag_cc().map(|d| d.1),
                pair_probes: {
                    if let Some(v) = verdict.filter(|v| v.from_pair) {
                        pair_starts.insert(v.probe_start_ns);
                    }
                    let open = probes
                        .iter()
                        .any(|p| p.from_pair && !p.closed && p.verdict_at.is_none());
                    pair_starts.len() as u64 + u64::from(open)
                },
                kuyruk_tepe_ns: tepe,
                sayilan,
                sikismis,
                sifirlama: d_now - detects,
                kanitli: a_now - alarms,
                ayna,
                karar,
            });
            detects = d_now;
            alarms = a_now;
        }
        // Yoklama penceresindeki gönderimleri say (düzeneğin kendi sayımı).
        if let Some(q) = probes.last_mut() {
            let end = q.end.unwrap_or(u64::MAX);
            let (n, w) = sends
                .iter()
                .filter(|&&(t, _)| t >= q.start && t <= end)
                .fold((0, 0), |(n, w), &(_, b)| (n + 1, w + b));
            q.dgrams = n;
            q.wire = w;
        }
        if sends.len() > 4096 {
            let keep = probes.last().map_or(now, |q| q.start);
            sends.retain(|&(t, _)| t >= keep.min(now.saturating_sub(SEC)));
        }
        now += MS;
    }
    Run {
        rows,
        bucket_oversize: fwd.bucket_oversize,
        queue_drops: fwd.queue_drops,
    }
}

fn verdict_label(ok: bool) -> &'static str {
    if ok {
        "GECTI"
    } else {
        "KALDI"
    }
}

/// Kol başına özet ve `ilk_kirilan` (#175 7175574778 + EK sırası).
fn summary(c: &Cell, run: &Run) -> String {
    let rows = &run.rows;
    let first3: Vec<&Row> = rows.iter().take(3).collect();
    let first6: Vec<&Row> = rows.iter().take(6).collect();
    let tepe = rows.iter().map(|r| r.kuyruk_tepe_ns).max().unwrap_or(0);
    let mut o = String::new();
    let _ = write!(
        o,
        "K2OZET {{\"kol\":\"{}\",\"deney\":\"{}\",\"hat\":\"{}\",\"kayip_permil\":{},\"tohum\":{}",
        c.kol.label(),
        if c.deney { "acik" } else { "kapali" },
        c.hat.label(),
        c.loss_permille,
        c.seed
    );
    let evidence = |r: &Row| r.cift_tel.is_some_and(|w| w * 100 > r.kalici * 105);
    match c.kol {
        Kol::Toparlanma => {
            // Kalıcı tavanın ≥ 11 875'e çıkıp koşu sonuna dek kaldığı ilk ACK.
            let kalici_ack = (0..rows.len())
                .find(|&i| rows[i..].iter().all(|r| r.kalici >= GOAL4_BPS))
                .map(|i| rows[i].sira);
            let hedef4 = if rows.len() < 3 {
                "SINANMADI"
            } else {
                verdict_label(kalici_ack.is_some_and(|a| a <= 3))
            };
            let ilk = if hedef4 == "GECTI" {
                "yok"
            } else if !first3.iter().any(|r| evidence(r)) {
                // Alıcının kuralının aynası eşiği geçen bir alan üretti ama ACK taşımadı → silindi.
                if first3
                    .iter()
                    .any(|r| r.ayna.is_some_and(|w| w * 100 > r.kalici * 105))
                {
                    "cift_kaniti_silindi"
                } else {
                    "cift_kaniti_dogmadi"
                }
            } else if first3
                .iter()
                .find(|r| evidence(r))
                .is_some_and(|r| r.karar == "IntervalSample")
            {
                // Kanıt geldi ama aynı ACK aşağı örnek verdi: çift kolu kapalı (`pair_allowed`).
                if c.deney {
                    "yoklama_cikmadi"
                } else {
                    "yanlis_hukum"
                }
            } else if !c.deney {
                "yanlis_hukum"
            } else {
                let probe = first3.iter().find_map(|r| r.yoklama);
                match probe {
                    None => "yoklama_cikmadi",
                    Some(p) if p.dgrams == 0 => "yoklama_cikmadi",
                    Some(_) => {
                        if first3
                            .iter()
                            .any(|r| r.verdict.is_some_and(|v| v.from_pair && v.covered))
                        {
                            "yanlis_hukum"
                        } else {
                            "ack_kapsamadi"
                        }
                    }
                }
            };
            let _ = write!(
                o,
                ",\"ilk_kirilan\":\"{ilk}\",\"kalici_tavan_3ack\":{},\"kalici_tavan_ack\":{},\"hedef4\":\"{hedef4}\",\"kuyruk_tepe_ms\":{:.1},\"kapi393\":\"{}\",\"cift_yoklama_sayisi_6ack\":{}",
                first3.last().map_or("null".to_string(), |r| r.kalici.to_string()),
                kalici_ack.map_or("null".to_string(), |a| a.to_string()),
                tepe as f64 / 1e6,
                verdict_label(tepe <= GATE_393_NS),
                first6.last().map_or(0, |r| r.pair_probes),
            );
        }
        Kol::Sahte => {
            // Yanlış kanıt yalnız koruma dar hatta yakınsadıktan SONRA sayılır (kalıcı tavan ≤ 6 875
            // olan ilk ACK'ten itibaren); öncesi kör pencere / yakınsama, yanlış yükseliş değil.
            // Yanlış kanıt: çift, gerçek hattın (6 250) ve tavanın %5 üstünde bir kapasite iddia eder.
            let evidence = |r: &Row| {
                r.cift_tel
                    .is_some_and(|w| w * 100 > r.kalici.max(DROP_TO) * 105)
            };
            let conv = rows.iter().position(|r| r.kalici <= FALSE_RAISE_BPS);
            // Yakınsama ACK'inin aralığı düşüş öncesine uzanabilir: kanıt bir SONRAKİ ACK'ten sayılır.
            let ev = conv.and_then(|k| {
                rows.get(k + 1..)
                    .and_then(|rest| rest.iter().position(evidence))
                    .map(|j| k + 1 + j)
            });
            match (conv, ev) {
                (None, _) => {
                    let _ = write!(
                        o,
                        ",\"ilk_kirilan\":\"SINANMADI\",\"neden\":\"koruma_dar_hatta_yakinsamadi\",\"hedef4\":\"SINANMADI\",\"kapi393\":\"SINANMADI\",\"kalici_sahte_artis\":\"SINANMADI\",\"ikinci_yoklama\":\"SINANMADI\""
                    );
                }
                (Some(k), None) => {
                    let _ = write!(
                        o,
                        ",\"ilk_kirilan\":\"SINANMADI\",\"neden\":\"yanlis_yukselis_kaniti_dogmadi\",\"yakinsama_ack\":{},\"hedef4\":\"SINANMADI\",\"kapi393\":\"SINANMADI\",\"kalici_sahte_artis\":\"SINANMADI\",\"ikinci_yoklama\":\"SINANMADI\"",
                        rows[k].sira
                    );
                }
                (Some(_), Some(i)) => {
                    let after = &rows[i..];
                    let tepe_after = after.iter().map(|r| r.kuyruk_tepe_ns).max().unwrap_or(0);
                    let sahte = after.iter().any(|r| r.kalici > FALSE_RAISE_BPS);
                    let six: Vec<&Row> = after.iter().take(6).collect();
                    let before = if i == 0 { 0 } else { rows[i - 1].pair_probes };
                    let n6 = six.last().map_or(0, |r| r.pair_probes) - before;
                    let ilk = if sahte {
                        if c.deney {
                            "yanlis_hukum"
                        } else {
                            "dogrudan_yol_acti"
                        }
                    } else {
                        "yok"
                    };
                    let _ = write!(
                        o,
                        ",\"ilk_kirilan\":\"{ilk}\",\"kanit_ack\":{},\"hedef4\":\"{}\",\"kuyruk_tepe_ms\":{:.1},\"kapi393\":\"{}\",\"kalici_sahte_artis\":\"{}\",\"ikinci_yoklama\":\"{}\",\"cift_yoklama_sayisi_6ack\":{n6}",
                        rows[i].sira,
                        verdict_label(!sahte),
                        tepe_after as f64 / 1e6,
                        verdict_label(tepe_after <= GATE_393_NS),
                        if sahte { "VAR" } else { "YOK" },
                        if n6 >= 2 { "VAR" } else { "YOK" },
                    );
                }
            }
        }
    }
    let _ = write!(
        o,
        ",\"kova_asan\":{},\"kuyruk_dusen\":{},\"ack\":{}}}",
        run.bucket_oversize,
        run.queue_drops,
        rows.len()
    );
    o
}

/// Kova aracının doğrulaması (kart 7175575398): boş kovada iki tam datagram (1 046 B gövde,
/// 1 074 B IP) arası 12 500'de 948 / 12 500 = 75,84 ms, 6 250'de 151,68 ms.
#[test]
fn kova_iki_tam_datagram_araligi() {
    for (rate, want_us) in [(12_500u64, 75_840u64), (6_250, 151_680)] {
        let mut f = Fwd::new(Hat::Kova, vec![(0, rate)], 0, 1);
        let a = f.send(10 * SEC, 1, 1_046, FILE_BASE_ID, (0, 1)).expect("1");
        let b = f
            .send(10 * SEC, 2, 1_046, FILE_BASE_ID + 1, (0, 2))
            .expect("2");
        assert_eq!(a, 10 * SEC, "dolu kovada ilk datagram hemen çıkar");
        assert_eq!((b - a) / 1_000, want_us, "{rate} B/s");
    }
    // Saf serileştirici: 1 074 / 6 250 = 171,84 ms.
    let mut f = Fwd::new(Hat::Fifo, vec![(0, 6_250)], 0, 1);
    let a = f.send(0, 1, 1_046, FILE_BASE_ID, (0, 1)).expect("1");
    let b = f.send(0, 2, 1_046, FILE_BASE_ID + 1, (0, 2)).expect("2");
    assert_eq!((b - a) / 1_000, 171_840);
}

#[test]
#[ignore = "rapor: cargo test --release -p adaptive-transport --test k2_kovali_e2e -- --ignored --nocapture --test-threads=1"]
fn k2_kovali_e2e_rapor() {
    let only = std::env::var("K2_ONLY").ok();
    for kol in [Kol::Toparlanma, Kol::Sahte] {
        for hat in [Hat::Kova, Hat::Fifo] {
            for loss in [0u64, 200] {
                for seed in 1..=3u64 {
                    for deney in [false, true] {
                        let c = Cell {
                            kol,
                            deney,
                            hat,
                            loss_permille: loss,
                            seed,
                        };
                        let tag = format!(
                            "{}/{}/{}/{}/{}",
                            kol.label(),
                            hat.label(),
                            loss,
                            seed,
                            if deney { "acik" } else { "kapali" }
                        );
                        if only.as_ref().is_some_and(|o| !tag.starts_with(o.as_str())) {
                            continue;
                        }
                        let r = run(c);
                        for row in &r.rows {
                            println!("{}", row_json(&c, row));
                        }
                        println!("{}", summary(&c, &r));
                    }
                }
            }
        }
    }
}
