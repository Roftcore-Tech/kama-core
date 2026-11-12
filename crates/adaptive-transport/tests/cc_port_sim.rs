//! CC-PORT sim kapısı (ISEMRI-CC-PORT §5 Ö-CC "6 paketlik kuyruk" hücresinin ayrık-olay
//! karşılığı). netem DEĞİL; deterministik (seed'li LCG, ADR-17 motor dışı), gerçek `Sender`
//! + gerçek `Receiver` + sınırlı FIFO darboğaz.
//!
//! İki motor AYNI süreçte, AYNI seed'lerle, eşli koşar (`Engine::Legacy` = `Sender::new()`,
//! `Engine::CcPort` = `Sender::with_cc_port()` + `Receiver::with_cc_port()`); her satır motoru
//! yazar:
//!
//! ```text
//! cargo test -p adaptive-transport --test cc_port_sim -- --nocapture
//! CC_MATRIX=1 cargo test -p adaptive-transport --test cc_port_sim matrix_explore -- --nocapture
//! CC_TRACE=1 CC_SEED=10 CC_LOSS=200 cargo test ... shallow_queue_file_no_loss -- --nocapture
//! # r25 ürün zarfı keşfi (kapı uygulanmaz, yalnız rapor): ~180 B ileti paketi, ~107 B ACK
//! CC_R25_WIRE_OVERHEAD=115 CC_ACK_OVERHEAD=90 CC_R25_TMAX_S=1500 cargo test ... r25 -- --nocapture
//! ```
//!
//! `CC_LOSS_BEFORE_SER=1`: kayıp hız sınırlayıcıdan ÖNCE düşer (netem düzeni; düşen paket hat süresi
//! tüketmez). Varsayılan telsiz düzeni: paket serileştirilir, yolda düşer (hat süresi tükenir). Ölçüm
//! (q6, 12,5 kB/s, %30, 150 kB, 3 tohum): telsiz düzeninde port 17,6–19,4 s (taban 17,1 s = 12 s / 0,7 →
//! taban + %5); netem düzeninde 16,2–16,4 s (taban 12 s → hat kullanımı %74 — BBR v1 BtlBw = teslim hızı
//! ≤ (1 − p)·hat). Kayıp telafili BtlBw denemesi (`btlbw / (1 − p)`): netem düzeni 15,1 s (%80), telsiz
//! düzeninde süre aynı, retx +40 %, sığ kuyruk taşması 30–50 → önerilmedi (INV-RATE-1 metniyle de çelişir).
//!
//! `CC_TRACE=1` izi her gönderimi (`GÖNDER tür seq birim bayt kuyruk`), her teslimi ve göndericiye
//! ulaşan her ACK'i yazar. r25 keşif anahtarları: `CC_R25_INTERVAL_MS` (ileti aralığı),
//! `CC_R25_WIRE_OVERHEAD` (ileri zarf), `CC_R25_TMAX_S` (ufuk), `CC_ACK_OVERHEAD` (SACK zarfı);
//! biri tanımlıysa r25 kapısı atlanır.
//!
//! Kabul (yalnız port kolu için bağlayıcı; legacy kolu rapor eder, kırmızı üretmez):
//! * 6 paketlik kuyruk, 12,5 kB/s, 300 ms RTT, kayıp %0: 150 kB dosya ≤ 25 s (≈1,5 ×
//!   quinn-bbr-rfc'nin netem'de ölçülen 16,4 s'i; `reports/yol3/2026-11-10`).
//! * Aynı hücre, kayıp %20, 10 seed: 10/10 teslim, p50 ≤ 60 s.
//! * Derin kuyruk (1000 paket), kayıp %20, 10 seed: 10/10 teslim.
//! * PTT demeti (6 × 400 B, %20 kayıp): ilk demet ≤ 3 s.
//! * FEC: blok başına 2 parite kuyruğa girer, ≥ %90'ı tele çıkar (eski motorda kova tavanı atar).
//!
//! Bu bir ÖLÇÜM değil, KAPIDIR: sayılar netem ile doğrulanmadan "geçti" sayılmaz; portun
//! motora doğru bağlandığını ve sığ kuyrukta çökmediğini pinler.

use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};
use adaptive_transport::sender::{
    AckInfo, Event, FecActuationMode, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind,
};
use std::collections::{HashMap, HashSet, VecDeque};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const MTU: u64 = 1200;
const FILE_BYTES: u64 = 150_000;

/// Hangi motor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Legacy,
    CcPort,
}

impl Engine {
    fn name(self) -> &'static str {
        match self {
            Engine::Legacy => "legacy (faz makinesi)",
            Engine::CcPort => "cc_port (quinn BBR + RFC 9002)",
        }
    }
    fn sender(self) -> Sender {
        match self {
            Engine::Legacy => Sender::new(),
            Engine::CcPort => Sender::with_cc_port(),
        }
    }
    fn receiver(self) -> Receiver {
        match self {
            Engine::Legacy => Receiver::new(),
            Engine::CcPort => Receiver::with_cc_port(),
        }
    }
}
const ENGINES: [Engine; 2] = [Engine::Legacy, Engine::CcPort];

/// Sınırlı FIFO darboğaz: `rate_bps` serileştirme, `limit` paket kuyruk, `delay_ns` yayılım.
struct Bottleneck {
    rate_bps: u64,
    limit: usize,
    delay_ns: u64,
    /// Serileştirmesi bitmemiş paketlerin bitiş anları (kuyruk doluluğu bunlardan sayılır).
    busy: VecDeque<u64>,
    /// Alıcıya varacaklar: (varış, seq, unit_id, kind).
    wire: VecDeque<(u64, u64, u64, TransmitKind)>,
    dropped_queue: u64,
    dropped_random: u64,
    lcg: u64,
    loss_permille: u64,
    /// Kesinti penceresi (ns): bu aralıkta ileri yönde %100 kayıp (gösterim "kesinti" profili).
    blackout: Option<(u64, u64)>,
    /// netem düzeni: kayıp serileştirmeden önce (bkz. `send`).
    loss_before_ser: bool,
    /// [H1] Gilbert-Elliott iki durumlu kümeli kayıp `(p_gb ppm, p_bg ppm, kötü_durumda_mı)`;
    /// `None` = Bernoulli `loss_permille` (mevcut testlerin yolu, bayt düzeyinde aynı).
    ge: Option<(u64, u64, bool)>,
}

impl Bottleneck {
    fn new(rate_bps: u64, limit: usize, delay_ns: u64, loss_permille: u64, seed: u64) -> Self {
        Self {
            rate_bps,
            limit,
            delay_ns,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            dropped_queue: 0,
            dropped_random: 0,
            lcg: seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407),
            loss_permille,
            blackout: None,
            loss_before_ser: std::env::var("CC_LOSS_BEFORE_SER").is_ok(),
            ge: None,
        }
    }

    /// [H1] Gilbert-Elliott: KÖTÜ durumda paket düşer, İYİ durumda geçer; geçiş paket başına.
    /// Durağan kayıp = p_gb / (p_gb + p_bg), ortalama patlama uzunluğu = 1 / p_bg.
    fn ge_drop(&mut self) -> Option<bool> {
        let (p_gb, p_bg, bad) = self.ge?;
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let u = (self.lcg >> 33) % 1_000_000;
        let next = if bad { u >= p_bg } else { u < p_gb };
        self.ge = Some((p_gb, p_bg, next));
        Some(bad)
    }

    fn rand_permille(&mut self) -> u64 {
        self.lcg = self
            .lcg
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.lcg >> 33) % 1000
    }

    fn send(&mut self, now: u64, seq: u64, size: u64, unit_id: u64, kind: TransmitKind) {
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            return;
        }
        // `CC_LOSS_BEFORE_SER=1`: netem düzeni — kayıp, hız sınırlayıcıdan ÖNCE düşer; düşen paket
        // hat süresi TÜKETMEZ (varsayılan: telsiz düzeni, düşen paket serileştirildi ama yolda
        // düştü). Keşif anahtarı; varsayılan davranış bayt düzeyinde aynı.
        if self.loss_before_ser
            && self.loss_permille > 0
            && self.rand_permille() < self.loss_permille
        {
            self.dropped_random += 1;
            return;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + size * SEC / self.rate_bps;
        self.busy.push_back(done);
        if self.blackout.is_some_and(|(a, b)| (a..b).contains(&now)) {
            self.dropped_random += 1;
            return;
        }
        if let Some(drop) = self.ge_drop() {
            if drop {
                self.dropped_random += 1;
                return;
            }
        } else if !self.loss_before_ser
            && self.loss_permille > 0
            && self.rand_permille() < self.loss_permille
        {
            self.dropped_random += 1;
            return; // serileştirildi ama yolda düştü
        }
        self.wire
            .push_back((done + self.delay_ns, seq, unit_id, kind));
    }

    /// Serileştirme kuyruğundaki paket sayısı (iz için).
    fn queue_len(&self) -> usize {
        self.busy.len()
    }

    fn poll(&mut self, now: u64) -> Vec<(u64, u64, TransmitKind)> {
        let mut out = Vec::new();
        while self.wire.front().is_some_and(|&(t, _, _, _)| t <= now) {
            let (_, seq, uid, kind) = self.wire.pop_front().unwrap();
            out.push((seq, uid, kind));
        }
        out
    }
}

/// Dönüş yolu (SACK) darboğazı: `rate_bps` serileştirme (0 = sınırsız), `limit` paket kuyruk,
/// `delay_ns` yayılım. ACK baytı = kodlanmış çerçeve + `ACK_WIRE_OVERHEAD`. Düşük bant hücrelerinde
/// (r25: 25 B/s iki yön) ACK'lerin dönüş hattını doldurması ancak bununla görünür.
struct RevPath {
    rate_bps: u64,
    limit: usize,
    delay_ns: u64,
    busy: VecDeque<u64>,
    wire: VecDeque<(u64, AckInfo)>,
    dropped_queue: u64,
    bytes: u64,
}

/// SACK çerçevesinin tel zarfı (Noise mühürlü başlık + UDP/IP) için yaklaşık ek — ürün ölçümü
/// ~65 B toplam ACK; kodlanmış çerçeve ~17 B.
const ACK_WIRE_OVERHEAD: u64 = 48;

impl RevPath {
    fn new(rate_bps: u64, limit: usize, delay_ns: u64) -> Self {
        Self {
            rate_bps,
            limit,
            delay_ns,
            busy: VecDeque::new(),
            wire: VecDeque::new(),
            dropped_queue: 0,
            bytes: 0,
        }
    }

    /// `size` bayt ACK'i kuyruğa koyar; kuyruk doluysa düşer (SACK birikimli: sonraki ACK kapsar).
    /// Döner: kuyruğa girdi mi.
    fn send(&mut self, now: u64, size: u64, ack: AckInfo) -> bool {
        if self.rate_bps == 0 {
            self.bytes += size;
            self.wire.push_back((now + self.delay_ns, ack));
            return true;
        }
        while self.busy.front().is_some_and(|&t| t <= now) {
            self.busy.pop_front();
        }
        if self.busy.len() >= self.limit {
            self.dropped_queue += 1;
            return false;
        }
        let start = self.busy.back().copied().unwrap_or(now).max(now);
        let done = start + size * SEC / self.rate_bps;
        self.busy.push_back(done);
        self.bytes += size;
        self.wire.push_back((done + self.delay_ns, ack));
        true
    }
}

#[derive(Debug, Clone, Copy)]
struct Outcome {
    completed_ns: Option<u64>,
    delivered_units: usize,
    sent_data: u64,
    sent_retx: u64,
    /// `sent_retx` içinde Duplicate kökenli (ileri kopya / kuruluş probu) sayısı.
    sent_dup: u64,
    sent_parity: u64,
    sent_bytes: u64,
    dropped_queue: u64,
    dropped_random: u64,
    /// Alıcının ürettiği SACK sayısı (dönüş yolu maliyeti).
    acks: u64,
    /// Dönüş yolunda gönderilen bayt (SACK + zarf).
    rev_bytes: u64,
    /// Dönüş yolu kuyruğunda düşen SACK sayısı.
    rev_dropped_queue: u64,
    /// Birim teslim gecikmesi p50 (sunumdan alıcıya teslime; ileti hücreleri için "ileti p50").
    unit_delay_p50_ns: u64,
    /// Birim teslim gecikmesi p95.
    unit_delay_p95_ns: u64,
}

/// Tek koşu. `units`: (birim sayısı, boyut); `parity_per_block > 0`: her 8 birimlik blok için
/// bu kadar parite kuyruğa girer (manuel FEC otoritesi; motor yalnız pace eder — DR-S5-1).
#[allow(clippy::too_many_arguments)]
#[allow(deprecated)] // legacy-enqueue: pace yolunu ölçer (üretimde otorite-bağlı close; Commit-K)
fn run_opt(
    eng: Engine,
    rate_bps: u64,
    queue_limit: usize,
    rtt_ms: u64,
    loss_permille: u64,
    seed: u64,
    t_max_s: u64,
    units: (u64, u64),
    parity_per_block: u16,
    blackout: Option<(u64, u64)>,
    rev_path: Option<(u64, usize)>,
    app_interval_ms: Option<u64>,
    wire_overhead: Option<u64>,
) -> Outcome {
    let one_way = rtt_ms * MS / 2;
    let mut s = eng.sender();
    // `CC_PTO_MAX_S=<s>`: port kolunda PTO geri çekilme tavanı (varsayılan validated_v1: 30 s).
    if let Some(m) = std::env::var("CC_PTO_MAX_S")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        if eng != Engine::Legacy {
            let mut c = adaptive_transport::tail_recovery::TailRecoveryConfig::rfc9002_cc_port();
            c.max_backoff_pto_ns = m * SEC;
            s.set_tail_recovery_config(c);
        }
    }
    // `CC_PTO_INIT_MS=<ms>`: port kolunda RTT örneksiz ilk PTO (varsayılan 1000 ms; RFC 9002 §6.2.2
    // "önceki ölçümden ilk RTT" deneyi).
    if let Some(m) = std::env::var("CC_PTO_INIT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        if eng != Engine::Legacy {
            let mut c = adaptive_transport::tail_recovery::TailRecoveryConfig::rfc9002_cc_port();
            c.initial_pto_ns = m * MS;
            s.set_tail_recovery_config(c);
        }
    }
    // `CC_IW=<bayt>`: port motorunun ilk uçuş penceresi (varsayılan kurucu: 10·MTU; ürün: 4·MTU).
    if let Some(iw) = std::env::var("CC_IW")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        s.set_cc_initial_window(iw);
    }
    let mut r = eng.receiver();
    if parity_per_block > 0 {
        s.set_fec_actuation_mode(FecActuationMode::Manual { enabled: true })
            .expect("manuel FEC kipi");
    }
    let mut fwd = Bottleneck::new(rate_bps, queue_limit, one_way, loss_permille, seed);
    // Kesinti penceresi `(başlangıç_s, süre_s)`: ileri yönde %100 kayıp (kesinti profili). Testler
    // parametreyle verir (süreç-geneli ortam değişkeni paralel testlere sızar — CI'da görüldü);
    // `CC_BLACKOUT=<başlangıç_s>,<süre_s>` yalnız elle koşular için yedek.
    let blackout = blackout.or_else(|| {
        let v = std::env::var("CC_BLACKOUT").ok()?;
        let (a, d) = v.split_once(',')?;
        Some((a.parse::<u64>().ok()?, d.parse::<u64>().ok()?))
    });
    if let Some((a, d)) = blackout {
        fwd.blackout = Some((a * SEC, (a + d) * SEC));
    }
    // `CC_REV_RATE=<bayt/s>` (0 = sınırsız) ve `CC_REV_Q=<paket>`: dönüş yolu darboğazı. Varsayılan
    // sınırsız (eski davranış, bayt düzeyinde aynı). `rev_path` parametresi önceliklidir.
    let (rev_rate, rev_q) = rev_path.unwrap_or_else(|| {
        let rr = std::env::var("CC_REV_RATE")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let rq = std::env::var("CC_REV_Q")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(queue_limit);
        (rr, rq)
    });
    let mut rev = RevPath::new(rev_rate, rev_q, one_way);
    // `CC_APP_INTERVAL_MS=<ms>`: birimler uygulamadan bu aralıkla sunulur (ileti hücresi);
    // varsayılan 0 = hepsi t=0'da (dosya). `app_interval_ms` parametresi önceliklidir.
    let app_interval = app_interval_ms
        .or_else(|| {
            std::env::var("CC_APP_INTERVAL_MS")
                .ok()?
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0)
        * MS;
    let mut offer_at: HashMap<u64, u64> = HashMap::new();
    let mut unit_delays: Vec<u64> = Vec::new();
    // `CC_REV_LOSS=<binde>`: dönüş yolunda (SACK) rastgele kayıp; varsayılan 0 (kayıpsız dönüş).
    let rev_loss: u64 = std::env::var("CC_REV_LOSS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut rev_lcg: u64 = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(12345);
    let mut acked_seen: HashSet<u64> = HashSet::new();
    let mut delivered: HashSet<u64> = HashSet::new();
    let (n_units, unit_size) = units;
    let mut o = Outcome {
        completed_ns: None,
        delivered_units: 0,
        sent_data: 0,
        sent_retx: 0,
        sent_dup: 0,
        sent_parity: 0,
        sent_bytes: 0,
        dropped_queue: 0,
        dropped_random: 0,
        acks: 0,
        rev_bytes: 0,
        rev_dropped_queue: 0,
        unit_delay_p50_ns: 0,
        unit_delay_p95_ns: 0,
    };
    // `CC_TRACE=1` iz; `CC_TRACE_MS=<ms>` iz aralığı (varsayılan 250).
    let trace = std::env::var("CC_TRACE").is_ok();
    let trace_every = std::env::var("CC_TRACE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(250)
        * MS;
    // `CC_WIRE_OVERHEAD=<bayt>`: ileri yönde her gönderime eklenen tel zarfı (Noise mühürlü başlık +
    // UDP/IP); varsayılan 0 (eski davranış). `wire_overhead` parametresi önceliklidir.
    let wire_overhead = wire_overhead
        .or_else(|| std::env::var("CC_WIRE_OVERHEAD").ok()?.parse::<u64>().ok())
        .unwrap_or(0);
    // `CC_ACK_OVERHEAD=<bayt>`: SACK çerçevesine eklenen tel zarfı (varsayılan `ACK_WIRE_OVERHEAD`;
    // ürün runtime'ında ölçülen ACK ortalama ~107 B → ~90).
    let ack_overhead = std::env::var("CC_ACK_OVERHEAD")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(ACK_WIRE_OVERHEAD);
    let emit = |decs: Vec<TransmitDecision>, now: u64, fwd: &mut Bottleneck, o: &mut Outcome| {
        for d in decs {
            match d.kind {
                TransmitKind::Data => o.sent_data += 1,
                TransmitKind::Retransmit => o.sent_retx += 1,
                TransmitKind::Duplicate => {
                    o.sent_retx += 1;
                    o.sent_dup += 1;
                }
                TransmitKind::FecParity => o.sent_parity += 1,
                TransmitKind::CsProbe => {}
            }
            o.sent_bytes += d.size + wire_overhead;
            if trace {
                println!(
                    "[{}] t={:.2}s GÖNDER {:?} seq={} birim={} bayt={} kuyruk={}",
                    eng.name(),
                    now as f64 / SEC as f64,
                    d.kind,
                    d.seq,
                    d.unit_id,
                    d.size + wire_overhead,
                    fwd.queue_len()
                );
            }
            fwd.send(now, d.seq, d.size + wire_overhead, d.unit_id, d.kind);
        }
    };
    // Dosya tek seferde kuyruğa girer (uygulama sınırlı değil); ileti hücresinde birimler
    // `app_interval` aralığıyla sunulur.
    let mut next_offer: u64 = 0;
    let offer = |i: u64,
                 now: u64,
                 s: &mut Sender,
                 fwd: &mut Bottleneck,
                 o: &mut Outcome,
                 offer_at: &mut HashMap<u64, u64>| {
        let decs = s.on_event(
            Event::AppData(OpaqueUnit {
                id: i,
                size: unit_size,
            }),
            now,
        );
        offer_at.insert(i, now);
        emit(decs, now, fwd, o);
        if parity_per_block > 0 && (i + 1).is_multiple_of(8) {
            let block_id = (i / 8) as u32;
            for esi in 0..parity_per_block {
                s.enqueue_fec_parity(block_id, 8 + esi, unit_size);
            }
        }
    };
    if app_interval == 0 {
        for i in 0..n_units {
            offer(i, 0, &mut s, &mut fwd, &mut o, &mut offer_at);
        }
        next_offer = n_units;
    }
    let push_ack = |frame: adaptive_transport::ack::AckFrame,
                    expected: u64,
                    now: u64,
                    acked_seen: &mut HashSet<u64>,
                    rev: &mut RevPath| {
        let new: Vec<u64> = frame
            .acked_full(expected)
            .into_iter()
            .filter(|q| acked_seen.insert(*q))
            .collect();
        // HER SACK çerçevesi tele çıkar (yinelenen bilgi taşısa da dönüş yolunun baytını ve
        // kuyruğunu işgal eder — inceleme 2026-11-11: eskiden yalnız yeni seq taşıyan çerçeve
        // kuyruğa giriyordu, ACK kadansının tel maliyeti eksik sayılıyordu). Gönderici yalnız
        // yeni bilgi taşıyan çerçeveyle beslenir (boş AckInfo göndericiye verilmez: eski
        // hücrelerde gönderici davranışı bayt düzeyinde aynı; dönüş yolu sınırsızken zamanlama da).
        let size = frame.encode().len() as u64 + ack_overhead;
        let ack = AckInfo {
            acked_seqs: new.clone(),
            ack_delay: frame.ack_delay_duration(),
        };
        if !rev.send(now, size, ack) {
            // Dönüş kuyruğunda düştü: seq'ler sonraki birikimli SACK'te yine gelir.
            for q in &new {
                acked_seen.remove(q);
            }
        }
    };
    let profile = std::env::var("CC_PROFILE").is_ok();
    let mut prof_marks: [Option<u64>; 3] = [None; 3];
    let mut prof_modes: Vec<(u64, adaptive_transport::cc::Mode)> = Vec::new();
    let mut prof_last_mode: Option<adaptive_transport::cc::Mode> = None;
    let t_max = t_max_s * SEC;
    let mut now = 0u64;
    while now < t_max {
        // Uygulama sunumu (ileti hücresi).
        while app_interval > 0 && next_offer < n_units && next_offer * app_interval <= now {
            offer(next_offer, now, &mut s, &mut fwd, &mut o, &mut offer_at);
            next_offer += 1;
        }
        // Gönderici saati: 1 ms tick (pacer dolumu + PTO) — desen1_sim ile aynı sürüş.
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut o);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut o);
        }
        // Alıcı: varışlar.
        for (seq, uid, kind) in fwd.poll(now) {
            if kind == TransmitKind::CsProbe {
                continue;
            }
            let rx_kind = if kind == TransmitKind::FecParity {
                RxKind::FecShard {
                    block_id: 0,
                    esi: 0,
                    k_actual: 8,
                    is_repair: true,
                }
            } else {
                RxKind::Data
            };
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: rx_kind,
                },
                now,
            );
            for u in out.delivered {
                if delivered.insert(u) {
                    if let Some(&t0) = offer_at.get(&u) {
                        unit_delays.push(now - t0);
                        if trace {
                            println!(
                                "[{}] t={:.2}s TESLİM birim={u} gecikme={:.2}s",
                                eng.name(),
                                now as f64 / SEC as f64,
                                (now - t0) as f64 / SEC as f64
                            );
                        }
                    }
                }
            }
            if let Some(frame) = out.ack {
                o.acks += 1;
                let expected = r.largest_seq().unwrap_or(seq) + 1;
                push_ack(frame, expected, now, &mut acked_seen, &mut rev);
            }
        }
        if let Some(frame) = r.on_ack_timer(now) {
            o.acks += 1;
            let expected = r.largest_seq().unwrap_or(0) + 1;
            push_ack(frame, expected, now, &mut acked_seen, &mut rev);
        }
        // Dönüş yolu (serileştirme + gecikme; `CC_REV_LOSS` ile rastgele SACK kaybı).
        while rev.wire.front().is_some_and(|(t, _)| *t <= now) {
            let (_, ack) = rev.wire.pop_front().unwrap();
            if ack.acked_seqs.is_empty() {
                continue; // yalnız tel maliyeti; gönderici için yeni bilgi yok
            }
            if rev_loss > 0 {
                rev_lcg = rev_lcg
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                if (rev_lcg >> 33) % 1000 < rev_loss {
                    // SACK yolda düştü: içindeki seq'ler bir sonraki SACK'te yine gelir
                    // (alıcı birikimli bildirir); `acked_seen` filtresi bu yüzden burada geri alınır.
                    for q in &ack.acked_seqs {
                        acked_seen.remove(q);
                    }
                    continue;
                }
            }
            if trace {
                println!(
                    "[{}] t={:.2}s ACK seqs={:?} ack_delay={:?} rtt_samples={} e2a={:?}",
                    eng.name(),
                    now as f64 / SEC as f64,
                    ack.acked_seqs,
                    ack.ack_delay,
                    s.diag_rtt_sample_count(),
                    s.diag_e2a()
                );
            }
            let decs = s.on_event(Event::AckReceived(ack), now);
            emit(decs, now, &mut fwd, &mut o);
        }
        if trace && now.is_multiple_of(trace_every) {
            println!(
                "[{}] t={:.2}s st={:?} cc={:?} inflight={} delivered={} tail={:?}",
                eng.name(),
                now as f64 / SEC as f64,
                s.state(),
                s.diag_cc(),
                s.inflight_bytes(),
                delivered.len(),
                s.tail_probe_diag()
            );
            let sd = s.diag_sched_summary();
            println!(
                "[{}] t={:.2}s pacer={:?} sched(data sel/deny/emit={}/{}/{} arq={}/{}/{} dup={}/{}/{}) loss={:?}",
                eng.name(),
                now as f64 / SEC as f64,
                s.diag_pacer(),
                sd[0], sd[1], sd[2], sd[3], sd[4], sd[5], sd[6], sd[7], sd[8],
                s.diag_loss()
            );
        }
        // `CC_PROFILE=1`: ilerleme profili (%10/%50/%90 teslim anı, son birim, kip geçişleri).
        if profile {
            let d = delivered.len() as u64;
            for (i, pct) in [10u64, 50, 90].iter().enumerate() {
                if prof_marks[i].is_none() && d * 100 >= n_units * pct {
                    prof_marks[i] = Some(now);
                }
            }
            if let Some((mode, ..)) = s.diag_cc() {
                if prof_last_mode != Some(mode) {
                    prof_modes.push((now, mode));
                    prof_last_mode = Some(mode);
                }
            }
        }
        if o.completed_ns.is_none() && delivered.len() as u64 >= n_units {
            o.completed_ns = Some(now);
            break;
        }
        now += MS;
    }
    if profile {
        let f =
            |t: Option<u64>| t.map_or("-".to_string(), |t| format!("{:.1}", t as f64 / SEC as f64));
        println!(
            "[{}] PROFİL seed{seed}: %10={} %50={} %90={} son={} retx={} prob={:?} kipler={:?}",
            eng.name(),
            f(prof_marks[0]),
            f(prof_marks[1]),
            f(prof_marks[2]),
            f(o.completed_ns),
            o.sent_retx,
            s.tail_probe_diag(),
            prof_modes
                .iter()
                .map(|(t, m)| format!("{:.1}s:{m:?}", *t as f64 / SEC as f64))
                .collect::<Vec<_>>()
        );
    }
    o.delivered_units = delivered.len();
    o.dropped_queue = fwd.dropped_queue;
    o.dropped_random = fwd.dropped_random;
    o.rev_bytes = rev.bytes;
    o.rev_dropped_queue = rev.dropped_queue;
    if !unit_delays.is_empty() {
        unit_delays.sort_unstable();
        o.unit_delay_p50_ns = unit_delays[unit_delays.len() / 2];
        o.unit_delay_p95_ns =
            unit_delays[(unit_delays.len() * 95 / 100).min(unit_delays.len() - 1)];
    }
    o
}

fn run(
    eng: Engine,
    rate_bps: u64,
    queue_limit: usize,
    rtt_ms: u64,
    loss_permille: u64,
    seed: u64,
    t_max_s: u64,
) -> Outcome {
    run_opt(
        eng,
        rate_bps,
        queue_limit,
        rtt_ms,
        loss_permille,
        seed,
        t_max_s,
        (FILE_BYTES.div_ceil(MTU), MTU),
        0,
        None,
        None,
        None,
        None,
    )
}

fn fmt_s(v: Option<u64>) -> String {
    v.map_or("YOK".to_string(), |t| {
        format!("{:.2} s", t as f64 / SEC as f64)
    })
}

/// `CC_GATE_OUT=<dosya>`: her rapor satırı JSON satırı olarak dosyaya eklenir (CI gerileme kapısı;
/// `scripts/ci/cc_gate_compare.py` taban ↔ aday karşılaştırır). Anahtar = (motor, hücre, tohum).
fn gate_record(eng: Engine, label: &str, o: &Outcome) {
    let Ok(path) = std::env::var("CC_GATE_OUT") else {
        return;
    };
    let (cell, seed) = match label.rsplit_once(" seed") {
        Some((c, s)) => (c, s.parse::<u64>().unwrap_or(0)),
        None => (label, 0),
    };
    let engine = match eng {
        Engine::Legacy => "legacy",
        Engine::CcPort => "cc_port",
    };
    let completed = o.completed_ns.map_or("null".to_string(), |t| t.to_string());
    let line = format!(
        "{{\"engine\":\"{engine}\",\"cell\":\"{cell}\",\"seed\":{seed},\"completed_ns\":{completed},\"delivered\":{},\"sent_bytes\":{},\"sent_retx\":{},\"rev_bytes\":{},\"unit_delay_p50_ns\":{}}}\n",
        o.delivered_units, o.sent_bytes, o.sent_retx, o.rev_bytes, o.unit_delay_p50_ns
    );
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

fn report(eng: Engine, label: &str, o: &Outcome) {
    gate_record(eng, label, o);
    println!(
        "[{}] {label}: tamamlanma={} teslim={}/{} data={} retx={} (dup={}) parite={} bayt={} kuyruk_düşen={} rastgele_düşen={} sack={} dönüş_bayt={} dönüş_düşen={} birim_p50={} birim_p95={}",
        eng.name(),
        fmt_s(o.completed_ns),
        o.delivered_units,
        FILE_BYTES.div_ceil(MTU),
        o.sent_data,
        o.sent_retx,
        o.sent_dup,
        o.sent_parity,
        o.sent_bytes,
        o.dropped_queue,
        o.dropped_random,
        o.acks,
        o.rev_bytes,
        o.rev_dropped_queue,
        fmt_s(Some(o.unit_delay_p50_ns)),
        fmt_s(Some(o.unit_delay_p95_ns)),
    );
}

fn p50(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    v[v.len() / 2]
}

/// 10 seed'lik hücre: (teslim sayısı, p50, max, gönderim/yük p50).
fn ten_seeds(
    eng: Engine,
    label: &str,
    rate: u64,
    q: usize,
    rtt: u64,
    loss: u64,
) -> (usize, Option<u64>, Option<u64>, f64) {
    let mut times = Vec::new();
    let mut bytes = Vec::new();
    for seed in 1..=10 {
        let o = run(eng, rate, q, rtt, loss, seed, 120);
        if std::env::var("CC_VERBOSE").is_ok() {
            report(eng, &format!("{label} seed{seed}"), &o);
        } else {
            gate_record(eng, &format!("{label} seed{seed}"), &o);
        }
        if let Some(t) = o.completed_ns {
            times.push(t);
        }
        bytes.push(o.sent_bytes);
    }
    let ok = times.len();
    let p = (!times.is_empty()).then(|| p50(times.clone()));
    let m = times.iter().copied().max();
    let overhead = p50(bytes) as f64 / FILE_BYTES as f64;
    println!(
        "[{}] {label}: teslim {ok}/10 p50={} max={} gönderim/yük p50={overhead:.2}",
        eng.name(),
        fmt_s(p),
        fmt_s(m)
    );
    (ok, p, m, overhead)
}

/// Hücre 1: 6 paketlik kuyruk, kayıp yok — sığ kuyruk çöküşü (ISEMRI-CC-PORT §1 #4).
#[test]
fn shallow_queue_file_no_loss() {
    let seed = std::env::var("CC_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let loss = std::env::var("CC_LOSS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    for eng in ENGINES {
        let o = run(eng, 12_500, 6, 300, loss, seed, 120);
        report(eng, "q6 r12.5k d300 l0 150kB", &o);
        if eng == Engine::CcPort && loss == 0 {
            let t = o.completed_ns.expect("port: dosya tamamlanmalı");
            assert!(
                t <= 25 * SEC,
                "port: sığ kuyrukta {:.1} s > 25 s",
                t as f64 / SEC as f64
            );
        }
    }
}

/// Hücre 2: 6 paketlik kuyruk, %20 rastgele kayıp, 10 seed.
#[test]
fn shallow_queue_file_loss20_ten_seeds() {
    for eng in ENGINES {
        let (ok, p, _, _) = ten_seeds(eng, "q6 r12.5k d300 l20", 12_500, 6, 300, 200);
        if eng == Engine::CcPort {
            assert_eq!(ok, 10, "port: 10/10 teslim");
            assert!(p.unwrap() <= 60 * SEC, "port: p50 > 60 s");
        }
    }
}

/// Hücre 3: derin kuyruk (1000), %20 kayıp — K1 profilinin benzeri; port burada da teslim etmeli.
#[test]
fn deep_queue_file_loss20_ten_seeds() {
    for eng in ENGINES {
        let (ok, _, _, _) = ten_seeds(eng, "q1000 r12.5k d300 l20", 12_500, 1000, 300, 200);
        if eng == Engine::CcPort {
            assert_eq!(ok, 10, "port: 10/10 teslim");
        }
    }
}

/// Hücre 4: PTT-benzeri kısa demet (6 × 400 B, %20 kayıp, sığ kuyruk) — ilk demet ≤ 3 s.
#[test]
fn ptt_bundle_first_delivery() {
    for eng in ENGINES {
        let o = run_opt(
            eng,
            12_500,
            6,
            300,
            200,
            7,
            30,
            (6, 400),
            0,
            None,
            None,
            None,
            None,
        );
        gate_record(eng, "ptt 6x400B q6 l20 seed7", &o);
        println!(
            "[{}] ptt 6×400B q6 l20: ilk demet={}",
            eng.name(),
            fmt_s(o.completed_ns)
        );
        if eng == Engine::CcPort {
            let t = o.completed_ns.expect("port: demet teslim edilmeli");
            assert!(
                t <= 3 * SEC,
                "port: ilk demet {:.2} s > 3 s",
                t as f64 / SEC as f64
            );
        }
    }
}

/// Hücre 5: FEC paritesi tele çıkıyor mu (ISEMRI-CC-PORT §1 #5). Blok başına 2 parite kuyruğa
/// girer; eski motorda kova tavanı pariteyi atar (26,45 kB/s altında), portta burst tabanı 10·MTU.
#[test]
fn fec_parity_reaches_the_wire_below_26kbps() {
    let expected = (FILE_BYTES.div_ceil(MTU) / 8) * 2;
    for eng in ENGINES {
        let o = run_opt(
            eng,
            12_500,
            1000,
            300,
            0,
            1,
            120,
            (FILE_BYTES.div_ceil(MTU), MTU),
            2,
            None,
            None,
            None,
            None,
        );
        report(eng, "q1000 r12.5k d300 l0 + 2 parite/8 birim", &o);
        println!(
            "[{}] parite: beklenen {expected}, tele çıkan {}",
            eng.name(),
            o.sent_parity
        );
        if eng == Engine::CcPort {
            assert!(
                o.sent_parity >= expected * 9 / 10,
                "port: parite tele çıkmalı ({} / {expected})",
                o.sent_parity
            );
        }
    }
}

/// Keşif matrisi (yalnız `CC_MATRIX=1` ile; kapı değil): hücre × 10 seed × iki motor.
#[test]
fn matrix_explore() {
    if std::env::var("CC_MATRIX").is_err() {
        return;
    }
    let cells: [(&str, u64, usize, u64, u64); 6] = [
        ("q6 r12.5k d300 l30", 12_500, 6, 300, 300),
        ("q6 r12.5k d1000 l20", 12_500, 6, 1000, 200),
        ("q1000 r12.5k d1000 l0", 12_500, 1000, 1000, 0),
        ("q6 r3k d600 l25", 3_000, 6, 600, 250),
        ("q6 r50k d100 l10", 50_000, 6, 100, 100),
        ("q1000 r12.5k d300 l30", 12_500, 1000, 300, 300),
    ];
    for (label, rate, q, rtt, loss) in cells {
        for eng in ENGINES {
            ten_seeds(eng, label, rate, q, rtt, loss);
        }
    }
}

/// r25 ileti hücresi (bench `b_r25_d300_l20_p65` yaklaşığı): 25 B/s İKİ yönde, 300 ms tek yön,
/// %20 kayıp, 60 × 65 B ileti 10 s aralıkla (tel 130 B ≈ 5,2 s; yük ≈ %52). Ölçü: ileti teslim gecikmesi p50 ve teslim/60.
/// Dönüş yolu serileştirilir (SACK ~65 B ≈ 2,6 s). Kapı: 60/60 teslim; port ileti p50 ≤ 8 s. `CC_R25_SEEDS=<n>` ile tohum sayısı (varsayılan 10).
#[test]
fn r25_lowband_messages() {
    let seeds: u64 = std::env::var("CC_R25_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    for eng in ENGINES {
        let (mut p50s, mut deliv, mut fwd_b, mut rev_b, mut retx) =
            (vec![], vec![], vec![], vec![], vec![]);
        for seed in 1..=seeds {
            let o = run_opt(
                eng,
                25,
                20,
                300,
                200,
                seed,
                std::env::var("CC_R25_TMAX_S")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(700),
                (60, 65),
                0,
                None,
                Some((25, 20)),
                Some(
                    std::env::var("CC_R25_INTERVAL_MS")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(10_000),
                ),
                Some(
                    std::env::var("CC_R25_WIRE_OVERHEAD")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(65),
                ),
            );
            report(eng, &format!("r25 seed{seed}"), &o);
            p50s.push(o.unit_delay_p50_ns);
            deliv.push(o.delivered_units as u64);
            fwd_b.push(o.sent_bytes);
            rev_b.push(o.rev_bytes);
            retx.push(o.sent_retx);
        }
        println!(
            "[{}] r25 ÖZET: ileti p50 medyan={} teslim medyan={}/60 ileri bayt medyan={} dönüş bayt medyan={} retx medyan={}",
            eng.name(),
            fmt_s(Some(p50(p50s.clone()))),
            p50(deliv.clone()),
            p50(fwd_b),
            p50(rev_b),
            p50(retx)
        );
        // Kapı: her motor 60/60 teslim (medyan); port ileti p50 ≤ 8 s (U10–U13 öncesi 95 s, sonrası
        // 8,5 s; U14 sonrası 6,9 s; eski motor 5,4 s — eşitlenince kapı sıkılaşır). Keşif anahtarları (`CC_R25_*`,
        // `CC_ACK_OVERHEAD`) varken kapı uygulanmaz (yalnız rapor). `CC_GATE_OUT` varken de uygulanmaz:
        // CI gerileme kapısı bu harness'ı PR TABANININ motoruyla da koşturur (ci.yml `cc-regression`);
        // mutlak eşik tabanda tutmayabilir (bu PR'ın düzelttiği gerileme gibi) — mutlak eşik `test`
        // işinde, taban ↔ aday karşılaştırması `cc_gate_compare.py`'de doğrulanır.
        let explore = [
            "CC_R25_INTERVAL_MS",
            "CC_R25_WIRE_OVERHEAD",
            "CC_R25_TMAX_S",
            "CC_ACK_OVERHEAD",
            "CC_GATE_OUT",
        ]
        .iter()
        .any(|k| std::env::var(k).is_ok());
        if explore {
            continue;
        }
        assert_eq!(
            p50(deliv),
            60,
            "[{}] r25 teslim medyanı < 60/60",
            eng.name()
        );
        if eng != Engine::Legacy {
            assert!(
                p50(p50s) <= 8 * SEC,
                "[{}] r25 ileti p50 medyanı > 8 s",
                eng.name()
            );
        }
    }
}

/// Hücre 6: kesinti — 150 kB dosya, 12,5 kB/s, 300 ms, %20 kayıp; t = 3 s'de 60 s tam kesinti
/// (gösterim "kesinti" profili: 60 s %100 kayıp, sonra eski profil). Oturum kesintiyi atlatmalı ve
/// dosya kesinti bittikten sonra makul sürede tamamlanmalı (kapı: port ≤ 63 + 30 s).
#[test]
fn blackout_60s_session_survives_and_completes() {
    for eng in ENGINES {
        let mut times = Vec::new();
        for seed in 1..=5 {
            let o = run_opt(
                eng,
                12_500,
                1000,
                300,
                200,
                seed,
                180,
                (FILE_BYTES.div_ceil(MTU), MTU),
                0,
                Some((3, 60)),
                None,
                None,
                None,
            );
            report(eng, &format!("kesinti 60s @3s q1000 l20 seed{seed}"), &o);
            if let Some(t) = o.completed_ns {
                times.push(t);
            }
        }
        println!(
            "[{}] kesinti: teslim {}/5 p50={}",
            eng.name(),
            times.len(),
            (!times.is_empty())
                .then(|| p50(times.clone()))
                .map_or("YOK".into(), |t| format!("{:.1} s", t as f64 / SEC as f64))
        );
        if eng == Engine::CcPort {
            assert_eq!(times.len(), 5, "port: kesinti sonrası 5/5 tamamlanmalı");
            assert!(
                p50(times) <= 93 * SEC,
                "port: kesinti sonrası ≤ 30 s içinde bitmeli"
            );
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// [H1 · FEC üç kol] FEC kapalı / sabit parite / fuzzy kontrolcü — aynı tohum, aynı kayıp izi.
//
// Ürün yolunun blok kapanışını AYNEN taklit eder (`client-core/src/runtime.rs` offer kolu): her
// `AppData`'dan hemen sonra blok birikir; `k_shape` (8) dolunca `open_fec_block_authority` +
// (`close_fec_block_manual` | `controller_decision_snapshot` → `fec_controller_latch`). Kısmi blok
// ürün yolunda YALNIZ oturum kapanışında kapanır; burada da kapanmaz (`partial_close` yalnız
// karşı-olgusal kol, ürün davranışı DEĞİL). Gönderici ürün ayarlarıyla kurulur
// (`RuntimeConfig::default`: port motoru, IW 2400 → port tabanı 4·MTU, `bounded_v1` kayıp riski,
// yapı kapısı 250‰, profil 0x05: k = 8, r_code_max = 4, E = 1024).
//
// ALICI ÇÖZÜMÜ BURADA (i) "ÇÖZÜLEBİLİR"dir: bir blokta ≥ k geçerli sembol (kaynak + parite) gelince
// o bloğun eksik kaynak birimleri o anda "kurtarılabilir" sayılır ve ürün alıcısının yaptığı gibi
// göndericiye `RecoveredControl` (tek yön gecikmesiyle) gider. Gerçek RS çözümü ve uygulamaya teslim
// (ii) burada YOKTUR; FEC hükmü bu sayılarla VERİLMEZ (iş emri H1 §2, §4).
//
// Yalnız `CC_FEC3=1` ile koşan ölçüm testi + her zaman koşan iki yapısal pin. Mevcut yedi test bu
// bölümden etkilenmez (`run_opt` ve `Bottleneck`'in Bernoulli yolu değişmedi).
// ════════════════════════════════════════════════════════════════════════════════════════════════

use adaptive_transport::sender::{
    FecLossRiskConfig, InitialWindowConfig, RecoveredInfo, RecoveredUnit, SrLogicalRef,
};

/// Profil 0x05 (`kama_fec_codec::profile`): k_shape, r_code_max, E.
const K_SHAPE: u64 = 8;
const R_CODE_MAX: u16 = 4;
const SYMBOL_E: u64 = 1024;
/// Tel ek yükü: `HDR_DATA` 11 / `HDR_FEC` 12 (client-core `wire.rs`) + mühür 22 B (legacy zarf).
const DATA_OVERHEAD: u64 = 11 + 22;
const PARITY_OVERHEAD: u64 = 12 + 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FecArm {
    /// FEC kapalı (ürün `manual_r: Some(0)`: çerçeve aynı, parite yok).
    Off,
    /// Sabit parite: her tam blokta `r` (ürün `manual_r: Some(r)`).
    Fixed(u16),
    /// Fuzzy kontrolcü, ürün yapılandırması (teslim süresi bildirilmez — ürün TEXT/PTT/dosya böyle).
    Fuzzy,
    /// Fuzzy kontrolcü + bildirilmiş teslim süresi (ns): paritenin devreye girebildiği kontrol kolu.
    FuzzyDeadline(u64),
}

impl FecArm {
    fn label(self) -> String {
        match self {
            Self::Off => "FEC kapalı".into(),
            Self::Fixed(r) => format!("sabit {r}/8"),
            Self::Fuzzy => "fuzzy (ürün)".into(),
            Self::FuzzyDeadline(d) => format!("fuzzy+süre {} ms", d / MS),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum LossModel {
    /// Bağımsız kayıp (binde).
    Bernoulli(u64),
    /// Gilbert-Elliott (ppm): p_gb, p_bg.
    Ge(u64, u64),
}

#[derive(Debug, Clone, Copy)]
struct H1Cell {
    label: &'static str,
    rate_bps: u64,
    queue: usize,
    rtt_ms: u64,
    loss: LossModel,
    units: u64,
    unit_bytes: u64,
    t_max_s: u64,
    /// Sunum aralığı (ms): 0 = hepsi t = 0'da (dosya / PTT demeti); > 0 = uygulama sınırlı akış
    /// (blok latch'leri ölçüm oluştuktan sonra düşer — kontrolcünün devreye girebildiği kol).
    offer_interval_ms: u64,
}

#[derive(Debug, Clone, Default)]
struct H1Out {
    /// Bütün birimler (ARQ ile teslim ya da (i) çözülebilir) — `None` = t_max içinde değil.
    completed_ns: Option<u64>,
    /// Birim başına ilk erişilebilirlik anı: min(ARQ teslimi, (i) blok çözülebilir).
    per_unit_ns: Vec<Option<u64>>,
    /// Birim sunum anı (aralık × i); gecikme = `per_unit_ns − offer_ns`.
    offer_ns: Vec<u64>,
    /// (i) ARQ teslimiNDEN ÖNCE çözülebilir olan kaynak birim sayısı.
    recovered_before_arq: u64,
    data_bytes: u64,
    retx_bytes: u64,
    parity_bytes: u64,
    total_bytes: u64,
    parity_pkts: u64,
    /// Kapatılan (latch / manuel close) tam blok sayısı ve planlanan toplam parite.
    blocks_closed: u64,
    planned_parity: u64,
    /// Hiç kapanmayan kısmi bloktaki birim sayısı (ürün yolunda parite almaz).
    tail_units_unprotected: u64,
    /// Latch baskılama nedenleri (kontrolcü kolu): yok-karar / olay-uyuşmazlığı / kural-yok /
    /// sıfıra-yuvarlama; motor parite sayaçları: kuyruklanan / tele çıkan / iptal edilen.
    sup_no_decision: u64,
    sup_event_mismatch: u64,
    sup_no_rule: u64,
    sup_rounded_zero: u64,
    fec_enqueued: u64,
    fec_emitted: u64,
    fec_canceled: u64,
    /// Son latch'in istenen oranı (‰) ve baskılama nedeni.
    last_latch: String,
    /// Hakem gerekçe histogramı (None/ConfirmedFuzzy/ConfirmedPolicy/StructureGate/SpecBoot/
    /// SpecPersistent/DeadlineAssisted), PreConverge'de bloklanan, bütçe clamp / sıfırlayacak.
    arbiter: [u64; 7],
    preconverge_blocked: u64,
    budget_clamped: u64,
    budget_would_zero: u64,
}

/// Alıcı blok defteri: (alınan kaynak esi'leri, alınan parite esi'leri, k_actual, çözülebilir anı).
type BlockRx = (HashSet<u16>, HashSet<u16>, u64, Option<u64>);

#[allow(clippy::too_many_lines)]
fn run_h1(cell: &H1Cell, arm: FecArm, seed: u64, partial_close: bool) -> H1Out {
    let one_way = cell.rtt_ms * MS / 2;
    let mut s = Sender::with_cc_port();
    // Ürün yapılandırması (`runtime.rs` connect yolu, `KamaV1Profile::runtime_config`).
    s.set_fec_loss_risk(FecLossRiskConfig::bounded_v1());
    s.set_structure_gate_level(Some(250));
    s.set_initial_window(InitialWindowConfig {
        bytes: adaptive_transport::pacer::DEFAULT_BURST_CAP,
    });
    s.set_cc_initial_window(
        adaptive_transport::pacer::DEFAULT_BURST_CAP.max(4 * adaptive_transport::rate::DEFAULT_MTU),
    );
    let mode = match arm {
        FecArm::Off | FecArm::Fixed(_) => FecActuationMode::Manual { enabled: true },
        FecArm::Fuzzy | FecArm::FuzzyDeadline(_) => FecActuationMode::Controller,
    };
    s.set_fec_actuation_mode(mode).expect("FEC kipi");
    let mut r = Receiver::with_cc_port();
    let mut fwd = Bottleneck::new(cell.rate_bps, cell.queue, one_way, 0, seed);
    match cell.loss {
        LossModel::Bernoulli(pm) => fwd.loss_permille = pm,
        LossModel::Ge(p_gb, p_bg) => fwd.ge = Some((p_gb, p_bg, false)),
    }
    let data_size = cell.unit_bytes + DATA_OVERHEAD;
    let parity_size = SYMBOL_E + PARITY_OVERHEAD;
    let n = cell.units;
    let mut o = H1Out {
        per_unit_ns: vec![None; n as usize],
        offer_ns: (0..n).map(|i| i * cell.offer_interval_ms * MS).collect(),
        ..H1Out::default()
    };
    let mut arq_ns: Vec<Option<u64>> = vec![None; n as usize];
    // Parite seq → (blok, esi); blok → (alınan kaynak esi'leri, alınan parite esi'leri, k, çözülebilir anı).
    let mut parity_of: HashMap<u64, (u32, u16)> = HashMap::new();
    let mut blk_rx: HashMap<u32, BlockRx> = HashMap::new();
    let mut closed_k: HashMap<u32, u64> = HashMap::new();
    let mut rev: VecDeque<(u64, Event)> = VecDeque::new();
    let mut acked_seen: HashSet<u64> = HashSet::new();

    let emit = |decs: Vec<TransmitDecision>,
                now: u64,
                fwd: &mut Bottleneck,
                o: &mut H1Out,
                parity_of: &mut HashMap<u64, (u32, u16)>| {
        for d in decs {
            match d.kind {
                TransmitKind::Data => o.data_bytes += d.size,
                TransmitKind::Retransmit | TransmitKind::Duplicate => o.retx_bytes += d.size,
                TransmitKind::FecParity => {
                    o.parity_bytes += d.size;
                    o.parity_pkts += 1;
                    if let Some(h) = d.send_record {
                        let SrLogicalRef::Fec { block_id, esi } = h.logical_ref;
                        parity_of.insert(d.seq, (block_id, esi));
                    }
                }
                TransmitKind::CsProbe => {}
            }
            o.total_bytes += d.size;
            fwd.send(now, d.seq, d.size, d.unit_id, d.kind);
        }
    };

    // Uygulama verisi t = 0'da (dosya / PTT demeti tek seferde); blok kapanışı ürün sırasıyla.
    if let FecArm::FuzzyDeadline(d) = arm {
        s.declare_delivery_deadline(d);
    }
    let mut cur: u64 = 0;
    let mut block_id: u32 = 0;
    let mut offered: u64 = 0;
    let close = |s: &mut Sender, block_id: u32, k: u64| -> u16 {
        let k8 = u8::try_from(k).unwrap_or(8);
        match arm {
            FecArm::Off => {
                let auth = s.open_fec_block_authority();
                s.close_fec_block_manual(auth, block_id, 0, parity_size)
                    .unwrap_or(0)
            }
            FecArm::Fixed(rr) => {
                let auth = s.open_fec_block_authority();
                s.close_fec_block_manual(auth, block_id, rr, parity_size)
                    .unwrap_or(0)
            }
            FecArm::Fuzzy | FecArm::FuzzyDeadline(_) => {
                let snap = s.controller_decision_snapshot();
                let auth = s.open_fec_block_authority();
                s.fec_controller_latch(auth, snap, k8, R_CODE_MAX, block_id, parity_size)
                    .planned_r
            }
        }
    };

    let t_max = cell.t_max_s * SEC;
    let trace = std::env::var("CC_TRACE").is_ok();
    let mut now = 0u64;
    let mut done = 0u64;
    while now < t_max {
        // Uygulama sunumu: aralık 0 → hepsi t = 0; blok kapanışı ürün sırasıyla (AppData'dan hemen
        // sonra, aynı olay — same-event latch guard'ı geçer).
        while offered < n && offered * cell.offer_interval_ms * MS <= now {
            let i = offered;
            offered += 1;
            let decs = s.on_event(
                Event::AppData(OpaqueUnit {
                    id: i,
                    size: data_size,
                }),
                now,
            );
            emit(decs, now, &mut fwd, &mut o, &mut parity_of);
            cur += 1;
            if cur == K_SHAPE {
                let planned = close(&mut s, block_id, K_SHAPE);
                o.blocks_closed += 1;
                o.planned_parity += u64::from(planned);
                closed_k.insert(block_id, K_SHAPE);
                cur = 0;
                block_id += 1;
            }
            if offered == n && cur > 0 {
                if partial_close {
                    // KARŞI-OLGUSAL: kısmi blok son birimden hemen sonra kapanır (ürün yolu YAPMAZ).
                    let planned = close(&mut s, block_id, cur);
                    o.blocks_closed += 1;
                    o.planned_parity += u64::from(planned);
                    closed_k.insert(block_id, cur);
                } else {
                    o.tail_units_unprotected = cur;
                }
            }
        }
        let decs = s.on_event(Event::TimerFired(Timer::Pto), now);
        emit(decs, now, &mut fwd, &mut o, &mut parity_of);
        if now.is_multiple_of(200 * MS) {
            let decs = s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
            emit(decs, now, &mut fwd, &mut o, &mut parity_of);
        }
        let mut ack_frames = Vec::new();
        for (seq, uid, kind) in fwd.poll(now) {
            if kind == TransmitKind::CsProbe {
                continue;
            }
            // Blok defteri: hangi bloğun hangi sembolü geldi.
            let (blk, esi, is_parity) = if kind == TransmitKind::FecParity {
                let Some(&(b, e)) = parity_of.get(&seq) else {
                    continue;
                };
                (b, e, true)
            } else {
                ((uid / K_SHAPE) as u32, (uid % K_SHAPE) as u16, false)
            };
            let rx_kind = if is_parity {
                RxKind::FecShard {
                    block_id: blk,
                    esi,
                    k_actual: u8::try_from(closed_k.get(&blk).copied().unwrap_or(K_SHAPE))
                        .unwrap_or(8),
                    is_repair: true,
                }
            } else {
                RxKind::Data
            };
            let out = r.on_datagram(
                RxDatagram {
                    seq,
                    unit_id: uid,
                    kind: rx_kind,
                },
                now,
            );
            for u in out.delivered {
                let ui = u as usize;
                if ui < arq_ns.len() && arq_ns[ui].is_none() {
                    arq_ns[ui] = Some(now);
                }
            }
            if let Some(frame) = out.ack {
                ack_frames.push(frame);
            }
            // (i) çözülebilirlik: yalnız kapanmış (parite planlanmış ya da planlanabilir) bloklar.
            if let Some(&k) = closed_k.get(&blk) {
                let e = blk_rx
                    .entry(blk)
                    .or_insert_with(|| (HashSet::new(), HashSet::new(), k, None));
                if is_parity {
                    e.1.insert(esi);
                } else {
                    e.0.insert(esi);
                }
                if e.3.is_none() && !e.1.is_empty() && (e.0.len() + e.1.len()) as u64 >= e.2 {
                    e.3 = Some(now);
                    // Eksik kaynakları "kurtarılabilir" say; ürün alıcısı gibi göndericiye bildir.
                    let mut units = Vec::new();
                    for x in 0..e.2 {
                        let uid = u64::from(blk) * K_SHAPE + x;
                        if !e.0.contains(&(x as u16)) && arq_ns[uid as usize].is_none() {
                            units.push(RecoveredUnit {
                                unit_id: uid,
                                block_id: blk,
                                esi: x as u16,
                            });
                        }
                    }
                    if !units.is_empty() {
                        o.recovered_before_arq += units.len() as u64;
                        for u in &units {
                            let p = &mut o.per_unit_ns[u.unit_id as usize];
                            if p.is_none() {
                                *p = Some(now);
                                done += 1;
                            }
                        }
                        rev.push_back((
                            now + one_way,
                            Event::RecoveredControl(RecoveredInfo { units }),
                        ));
                    }
                }
            }
        }
        for frame in ack_frames {
            let expected = r.largest_seq().unwrap_or(0) + 1;
            let new: Vec<u64> = frame
                .acked_full(expected)
                .into_iter()
                .filter(|q| acked_seen.insert(*q))
                .collect();
            if !new.is_empty() {
                rev.push_back((
                    now + one_way,
                    Event::AckReceived(AckInfo {
                        acked_seqs: new,
                        ack_delay: frame.ack_delay_duration(),
                    }),
                ));
            }
        }
        if let Some(frame) = r.on_ack_timer(now) {
            let expected = r.largest_seq().unwrap_or(0) + 1;
            let new: Vec<u64> = frame
                .acked_full(expected)
                .into_iter()
                .filter(|q| acked_seen.insert(*q))
                .collect();
            if !new.is_empty() {
                rev.push_back((
                    now + one_way,
                    Event::AckReceived(AckInfo {
                        acked_seqs: new,
                        ack_delay: frame.ack_delay_duration(),
                    }),
                ));
            }
        }
        // ARQ teslimleri birim zamanına işlenir.
        for (ui, t) in arq_ns.iter().enumerate() {
            if let Some(t) = *t {
                if o.per_unit_ns[ui].is_none() {
                    o.per_unit_ns[ui] = Some(t);
                    done += 1;
                }
            }
        }
        // Dönüş yolu: kayıpsız, tek yön gecikmesi (sıra korunur).
        while rev.front().is_some_and(|(t, _)| *t <= now) {
            let (_, ev) = rev.pop_front().expect("ön eleman");
            let decs = s.on_event(ev, now);
            emit(decs, now, &mut fwd, &mut o, &mut parity_of);
        }
        if trace && now.is_multiple_of(1000 * MS) {
            let missing: Vec<usize> = o
                .per_unit_ns
                .iter()
                .enumerate()
                .filter(|(_, t)| t.is_none())
                .map(|(i, _)| i)
                .take(8)
                .collect();
            println!(
                "[h1 {}] t={}s teslim={done}/{n} sunulan={offered} inflight={} st={:?} tail={:?} eksik(ilk 8)={missing:?} düşen(kuyruk/rastgele)={}/{} ge={:?}",
                arm.label(),
                now / SEC,
                s.inflight_bytes(),
                s.state(),
                s.tail_probe_diag(),
                fwd.dropped_queue,
                fwd.dropped_random,
                fwd.ge
            );
        }
        if done >= n {
            o.completed_ns = Some(now);
            break;
        }
        now += MS;
    }
    let d = s.diag_fec_actuation();
    o.sup_no_decision = d.no_eligible_decision_count;
    o.sup_event_mismatch = d.decision_event_mismatch_count;
    o.sup_no_rule = d.no_rule_zero_count;
    o.sup_rounded_zero = d.rounded_to_zero_count;
    o.fec_enqueued = d.enqueued_r_total;
    o.fec_emitted = d.emitted_r_total;
    o.fec_canceled = d.canceled_r_total;
    o.arbiter = s.diag_arbiter_reasons();
    o.preconverge_blocked = s.diag_preconverge_parity_blocked();
    o.budget_clamped = s.diag_parity_budget_clamped();
    o.budget_would_zero = s.diag_parity_budget_would_zero();
    o.last_latch = d.last_latch.map_or("yok".into(), |l| {
        format!(
            "istenen={}‰ planlanan={} neden={:?}",
            l.requested_ratio_permille, l.planned_r, l.suppression
        )
    });
    o
}

fn pct(v: &mut [u64], p: usize) -> Option<u64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[((v.len() - 1) * p) / 100])
}

/// Bir kolun 10 tohumluk özeti. `active_only`: yalnız parite tele çıkan koşular (alt küme).
#[derive(Debug, Default, Clone)]
struct H1Agg {
    runs: usize,
    completed: usize,
    comp_p50: Option<u64>,
    comp_p95: Option<u64>,
    comp_max: Option<u64>,
    unit_p50: Option<u64>,
    unit_p95: Option<u64>,
    recovered: u64,
    data_b: u64,
    retx_b: u64,
    parity_b: u64,
    total_b: u64,
    parity_runs: usize,
    planned: u64,
    tail_unprot: u64,
}

fn aggregate(outs: &[H1Out]) -> H1Agg {
    let mut a = H1Agg {
        runs: outs.len(),
        ..H1Agg::default()
    };
    let mut comp: Vec<u64> = Vec::new();
    let mut units: Vec<u64> = Vec::new();
    for o in outs {
        if let Some(t) = o.completed_ns {
            comp.push(t);
        }
        units.extend(
            o.per_unit_ns
                .iter()
                .zip(&o.offer_ns)
                .filter_map(|(t, off)| t.map(|t| t.saturating_sub(*off))),
        );
        a.recovered += o.recovered_before_arq;
        a.data_b += o.data_bytes;
        a.retx_b += o.retx_bytes;
        a.parity_b += o.parity_bytes;
        a.total_b += o.total_bytes;
        a.planned += o.planned_parity;
        a.tail_unprot += o.tail_units_unprotected;
        if o.parity_pkts > 0 {
            a.parity_runs += 1;
        }
    }
    a.completed = comp.len();
    a.comp_max = comp.iter().copied().max();
    a.comp_p50 = pct(&mut comp.clone(), 50);
    a.comp_p95 = pct(&mut comp, 95);
    a.unit_p50 = pct(&mut units.clone(), 50);
    a.unit_p95 = pct(&mut units, 95);
    a
}

fn fs(v: Option<u64>) -> String {
    v.map_or("—".into(), |t| format!("{:.2}", t as f64 / SEC as f64))
}

fn h1_row(cell: &H1Cell, arm_label: &str, a: &H1Agg) {
    let k = a.runs.max(1) as u64;
    println!(
        "| {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} |",
        cell.label,
        arm_label,
        a.completed,
        a.runs,
        fs(a.comp_p50),
        fs(a.comp_p95),
        fs(a.comp_max),
        fs(a.unit_p50),
        fs(a.unit_p95),
        a.recovered,
        a.data_b / k,
        a.retx_b / k,
        a.parity_b / k,
        a.total_b / k,
        a.parity_runs,
        a.runs,
    );
}

fn h1_cells() -> Vec<H1Cell> {
    // GE: ortalama kayıp %20, ortalama patlama 4 paket → p_bg = 1/4, p_gb = 0,25 × 0,2 / 0,8.
    let ge = LossModel::Ge(62_500, 250_000);
    let ptt = |label, loss| H1Cell {
        label,
        rate_bps: 12_500,
        queue: 6,
        rtt_ms: 300,
        loss,
        units: 6,
        unit_bytes: 400,
        t_max_s: 60,
        offer_interval_ms: 0,
    };
    let file = |label, loss| H1Cell {
        label,
        rate_bps: 12_500,
        queue: 6,
        rtt_ms: 300,
        loss,
        units: 150,
        unit_bytes: 1000,
        t_max_s: 240,
        offer_interval_ms: 0,
    };
    // Kontrol hücresi: aynı 150×1000 B, uygulama sınırlı akış (160 ms arayla ≈ 6,25 kB/s, bağın
    // yarısı) — blok latch'leri ölçüm oluştuktan sonra düşer; kontrolcünün devreye girip
    // girmediği burada görülür.
    let stream = |label, loss| H1Cell {
        label,
        rate_bps: 12_500,
        queue: 6,
        rtt_ms: 300,
        loss,
        units: 150,
        unit_bytes: 1000,
        t_max_s: 240,
        offer_interval_ms: 160,
    };
    vec![
        ptt("PTT 6×400 B · %20 rastgele", LossModel::Bernoulli(200)),
        file("dosya 150×1000 B · %20 rastgele", LossModel::Bernoulli(200)),
        ptt("PTT 6×400 B · %20 GE (patlama 4)", ge),
        file("dosya 150×1000 B · %20 GE (patlama 4)", ge),
        stream(
            "akış 150×1000 B / 160 ms · %20 rastgele",
            LossModel::Bernoulli(200),
        ),
        stream("akış 150×1000 B / 160 ms · %20 GE (patlama 4)", ge),
    ]
}

/// [H1] Tek koşu izi (yalnız `CC_FEC3_ONE=<hücre-indisi>,<kol: off|fixed|fuzzy>,<tohum>` + `CC_TRACE=1`).
#[test]
fn h1_one_trace() {
    let Ok(v) = std::env::var("CC_FEC3_ONE") else {
        return;
    };
    let parts: Vec<&str> = v.split(',').collect();
    let cell = h1_cells()[parts[0].parse::<usize>().expect("hücre")];
    let arm = match parts[1] {
        "off" => FecArm::Off,
        "fixed" => FecArm::Fixed(2),
        _ => FecArm::Fuzzy,
    };
    let o = run_h1(&cell, arm, parts[2].parse().expect("tohum"), false);
    println!(
        "{} {}: tamam={} retx_B={} parite_pkt={}",
        cell.label,
        arm.label(),
        fs(o.completed_ns),
        o.retx_bytes,
        o.parity_pkts
    );
}

/// [H1] Ölçüm (yalnız `CC_FEC3=1`): dört hücre × kollar × 10 tohum. Kapı değildir; markdown tablosu
/// basar. `CC_FEC3_DEADLINE_MS` (varsayılan 600) fuzzy kontrol kolunun bildirilen teslim süresi;
/// `CC_FEC3_PARTIAL=1` karşı-olgusal kısmi-blok kapanışlı ek satırlar.
#[test]
fn h1_fec_three_arms() {
    if std::env::var("CC_FEC3").is_err() {
        return;
    }
    let deadline_ms: u64 = std::env::var("CC_FEC3_DEADLINE_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let partial = std::env::var("CC_FEC3_PARTIAL").is_ok();
    let arms = [
        FecArm::Off,
        FecArm::Fixed(2),
        FecArm::Fuzzy,
        FecArm::FuzzyDeadline(deadline_ms * MS),
    ];
    println!("| hücre | kol | teslim | tamamlanma p50 s | p95 s | max s | birim gecikmesi p50 s (teslim − sunum) | birim gecikmesi p95 s | (i) ARQ'dan önce çözülebilir birim (Σ10) | veri B/koşu | retx B/koşu | parite B/koşu | toplam B/koşu | parite tele çıkan koşu |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for cell in h1_cells() {
        for arm in arms {
            let outs: Vec<H1Out> = (1..=10)
                .map(|seed| run_h1(&cell, arm, seed, false))
                .collect();
            let all = aggregate(&outs);
            h1_row(&cell, &arm.label(), &all);
            if matches!(arm, FecArm::Fuzzy | FecArm::FuzzyDeadline(_)) {
                let active: Vec<H1Out> =
                    outs.iter().filter(|o| o.parity_pkts > 0).cloned().collect();
                if !active.is_empty() && active.len() < outs.len() {
                    h1_row(
                        &cell,
                        &format!("{} — yalnız parite çıkan alt küme", arm.label()),
                        &aggregate(&active),
                    );
                }
            }
            if std::env::var("CC_FEC3_VERBOSE").is_ok() {
                for (i, o) in outs.iter().enumerate() {
                    println!(
                        "  seed{} {}: tamam={} bloklar={} planlanan={} parite_pkt={} korumasız_kuyruk={} \
                         kuyruk/çıkan/iptal={}/{}/{} baskı(yok-karar/olay/kural/yuvarla)={}/{}/{}/{} son_latch[{}] \
                         hakem(yok/cf/cp/yapı/sb/sp/süre)={:?} preconv_blok={} bütçe_clamp/sıfır={}/{}",
                        i + 1,
                        arm.label(),
                        fs(o.completed_ns),
                        o.blocks_closed,
                        o.planned_parity,
                        o.parity_pkts,
                        o.tail_units_unprotected,
                        o.fec_enqueued,
                        o.fec_emitted,
                        o.fec_canceled,
                        o.sup_no_decision,
                        o.sup_event_mismatch,
                        o.sup_no_rule,
                        o.sup_rounded_zero,
                        o.last_latch,
                        o.arbiter,
                        o.preconverge_blocked,
                        o.budget_clamped,
                        o.budget_would_zero
                    );
                }
            }
        }
        if partial {
            for arm in [FecArm::Fixed(2), FecArm::Fuzzy] {
                let outs: Vec<H1Out> = (1..=10)
                    .map(|seed| run_h1(&cell, arm, seed, true))
                    .collect();
                h1_row(
                    &cell,
                    &format!("{} · KARŞI-OLGUSAL kısmi blok kapanışı", arm.label()),
                    &aggregate(&outs),
                );
            }
        }
    }
}

/// [H1 yapısal pin] 6 birimlik PTT demeti 8'lik bloğu kapatmaz: ürün yolunda hiçbir kolda parite
/// planlanmaz ve tele çıkmaz (kısmi blok yalnız oturum kapanışında kapanır).
#[test]
fn h1_ptt_bundle_gets_no_parity_in_any_arm() {
    let cell = h1_cells()[0];
    for arm in [
        FecArm::Off,
        FecArm::Fixed(2),
        FecArm::Fuzzy,
        FecArm::FuzzyDeadline(600 * MS),
    ] {
        let o = run_h1(&cell, arm, 7, false);
        assert_eq!(o.blocks_closed, 0, "{}: blok kapanmamalı", arm.label());
        assert_eq!(o.parity_pkts, 0, "{}: parite tele çıkmamalı", arm.label());
        assert_eq!(o.tail_units_unprotected, 6, "{}", arm.label());
    }
}

/// [H1 yapısal pin] Sabit 2/8: 150 birim → 18 tam blok, 36 parite kuyruklanır ve en az biri tele
/// çıkar; FEC kapalı kol aynı tohumda hiç parite göndermez ve hiçbir birimi (i) çözülebilir saymaz.
/// (Paritenin ZAMANINDA ulaşıp ulaşmadığı bir ölçümdür, burada pinlenmez: `h1_fec_three_arms`.)
#[test]
fn h1_fixed_parity_reaches_the_wire_off_sends_none() {
    let cell = h1_cells()[1];
    let fixed = run_h1(&cell, FecArm::Fixed(2), 1, false);
    assert_eq!(fixed.blocks_closed, 18, "150 birim → 18 tam blok");
    assert_eq!(fixed.fec_enqueued, 36, "18 blok × 2 parite kuyruklanır");
    assert!(fixed.parity_pkts > 0, "parite tele çıkmalı");
    let off = run_h1(&cell, FecArm::Off, 1, false);
    assert_eq!(off.parity_pkts, 0);
    assert_eq!(off.recovered_before_arq, 0);
}
