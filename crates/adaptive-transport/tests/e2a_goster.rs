//! [E2a gösterim] ISEMRI-YOL3-M1 — uçtan uca deterministik sahte bağlantı.
//!
//! İki iddia, ikisi de test (ölçüm DEĞİL; hiçbir sayı performans olarak okunmaz):
//! 1. **Bayrak kapalı = bit-aynı tel:** aynı sahte saatli senaryonun karar izi (her
//!    `TransmitDecision`'ın anı, türü, seq'i, birimi, boyu + son durum) `origin/main`
//!    5de1ce3f'de üretilen izle BİREBİR aynıdır. Taban izi bu dosyanın E2a'sız kopyasıyla
//!    5de1ce3f ağacında koşularak kaydedildi (`BASE_DIGEST_*`); dört kural (m1 zaman-kayıp
//!    tüketicileri, m6 Idle kovası, PTO 2 emisyonu, m8 RTT seçimi) bu senaryoda yol üstündedir.
//! 2. **Bayrak açık canlılık:** her bayrak tek başına ve hepsi birlikte açıkken senaryo
//!    deterministiktir ve sunulan her birim teslim edilir.
//!
//! Bağlantı (deterministik, RNG yok — ADR-17): tek yön gecikme 150 ms, veri kaybı seq karmasıyla
//! ~%20, ACK çerçevesi kaybı 1/10, ACK çerçevesi bir önceki çerçeveyi tekrarlar (bayat en büyük),
//! `ack_delay` döngüsü {0, 5 ms, 40 ms, doygun}. 5 ms PTO tiki; 60 s.

use adaptive_transport::loss::E2aFlags;
use adaptive_transport::sender::{
    AckInfo, Event, InitialWindowConfig, OpaqueUnit, Sender, Timer, TransmitDecision, TransmitKind,
};
use adaptive_transport::tail_recovery::TailRecoveryConfig;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const MS: u64 = 1_000_000;
const OWD_NS: u64 = 150 * MS;
const STEP_NS: u64 = 5 * MS;
const ACK_EVERY_NS: u64 = 20 * MS;
const END_NS: u64 = 60_000 * MS;
const UNITS: u64 = 120;
/// V2 `ack_delay` kodlama tavanı: 256 µs × 65535 (doygun).
const ACK_DELAY_SAT_NS: u64 = 256_000 * 65_535;

/// FNV-1a 64 (deterministik iz kimliği; bağımlılık yok).
struct Fnv(u64);
impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn u64(&mut self, v: u64) {
        for b in v.to_be_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn bytes(&mut self, v: &[u8]) {
        for &b in v {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

fn kind_code(k: TransmitKind) -> u64 {
    match k {
        TransmitKind::Data => 1,
        TransmitKind::Retransmit => 2,
        TransmitKind::Duplicate => 3,
        TransmitKind::FecParity => 4,
        TransmitKind::CsProbe => 5,
    }
}

/// Seq karmasıyla ~%20 veri kaybı (deterministik).
fn data_lost(seq: u64) -> bool {
    (seq.wrapping_mul(2_654_435_761) >> 7) % 100 < 20
}

struct Link {
    arrivals: BTreeMap<u64, Vec<(u64, u64)>>, // varış anı → (seq, unit_id)
    fresh: Vec<u64>,                          // varmış, henüz çerçevelenmemiş seq'ler
    last_frame: Vec<u64>,                     // bir önceki çerçeve (tekrar edilir)
    acks: BTreeMap<u64, Vec<(Vec<u64>, u64)>>, // gönderene varış → (seq'ler, ack_delay ns)
    frame_idx: u64,
    delivered: BTreeSet<u64>,
}

impl Link {
    fn new() -> Self {
        Self {
            arrivals: BTreeMap::new(),
            fresh: Vec::new(),
            last_frame: Vec::new(),
            acks: BTreeMap::new(),
            frame_idx: 0,
            delivered: BTreeSet::new(),
        }
    }

    fn emit(&mut self, h: &mut Fnv, now: u64, out: Vec<TransmitDecision>) {
        for d in out {
            h.u64(now);
            h.u64(kind_code(d.kind));
            h.u64(d.seq);
            h.u64(d.unit_id);
            h.u64(d.size);
            if d.kind == TransmitKind::CsProbe || data_lost(d.seq) {
                continue;
            }
            self.arrivals
                .entry(now + OWD_NS)
                .or_default()
                .push((d.seq, d.unit_id));
        }
    }

    /// `now`'a dek varanları alıcıya işler; ACK anıysa çerçeve üretir.
    fn receiver_tick(&mut self, now: u64) {
        let due: Vec<u64> = self.arrivals.range(..=now).map(|(&t, _)| t).collect();
        for t in due {
            if let Some(v) = self.arrivals.remove(&t) {
                for (seq, unit) in v {
                    self.fresh.push(seq);
                    self.delivered.insert(unit);
                }
            }
        }
        if !now.is_multiple_of(ACK_EVERY_NS)
            || (self.fresh.is_empty() && self.last_frame.is_empty())
        {
            return;
        }
        let mut frame: Vec<u64> = self.last_frame.clone();
        frame.append(&mut self.fresh);
        frame.sort_unstable();
        frame.dedup();
        self.frame_idx += 1;
        let delay = [0, 5 * MS, 40 * MS, ACK_DELAY_SAT_NS][(self.frame_idx % 4) as usize];
        self.last_frame = frame.clone();
        if self.frame_idx % 10 != 7 {
            self.acks
                .entry(now + OWD_NS)
                .or_default()
                .push((frame, delay));
        }
    }
}

/// Senaryoyu koşar; `(iz kimliği, teslim edilen birimler)`. `product` = ürün benzeri kurulum
/// (IW 2400, `validated_v1`, ilk olay Idle'da TimerFired); değilse çıplak `Sender::new()`.
fn run(product: bool, configure: &dyn Fn(&mut Sender)) -> (u64, BTreeSet<u64>) {
    let mut s = Sender::new();
    // Ürün runtime'ı gibi deneysel CS-probe şeridi kapalı (`experimental-cold-start` derlemesinde
    // feature-varsayılanı açık; ürün `set_cs_probe_lane(false)` ile kapatır) → iz özellik kümesinden
    // bağımsız kalır (varsayılan ve --all-features derlemesi aynı tabana karşı sınanır).
    s.set_cs_probe_lane(false);
    if product {
        s.set_initial_window(InitialWindowConfig { bytes: 2400 });
        s.set_tail_recovery_config(TailRecoveryConfig::validated_v1());
    }
    configure(&mut s);
    let mut h = Fnv::new();
    let mut link = Link::new();
    let mut next_id = 0u64;
    if product {
        // m6 yolu: ilk olay AppData'dan ÖNCE (Idle, ölçümsüz).
        let out = s.on_event(Event::TimerFired(Timer::Pto), 0);
        link.emit(&mut h, 0, out);
    }
    let mut now = 0u64;
    while now <= END_NS {
        link.receiver_tick(now);
        let due: Vec<u64> = link.acks.range(..=now).map(|(&t, _)| t).collect();
        for t in due {
            for (seqs, delay) in link.acks.remove(&t).unwrap_or_default() {
                let out = s.on_event(
                    Event::AckReceived(AckInfo {
                        acked_seqs: seqs,
                        ack_delay: Duration::from_nanos(delay),
                    }),
                    now,
                );
                link.emit(&mut h, now, out);
            }
        }
        let offer = if now == 0 {
            3
        } else {
            u64::from(now.is_multiple_of(200 * MS))
        };
        for _ in 0..offer {
            if next_id < UNITS {
                let size = 283 + (next_id * 37) % 700;
                let out = s.on_event(Event::AppData(OpaqueUnit { id: next_id, size }), now);
                link.emit(&mut h, now, out);
                next_id += 1;
            }
        }
        let out = s.on_event(Event::TimerFired(Timer::Pto), now);
        link.emit(&mut h, now, out);
        now += STEP_NS;
    }
    h.bytes(format!("{:?}", s.state()).as_bytes());
    let (lp, lt, pl) = s.diag_loss();
    for v in [
        lp,
        lt,
        pl,
        s.inflight_bytes(),
        u64::from(s.diag_rtt_sample_count()),
    ] {
        h.u64(v);
    }
    let (pc, pb, sc, sb) = s.tail_probe_diag();
    for v in [pc, pb, sc, sb] {
        h.u64(v);
    }
    h.u64(s.path_timing_snapshot().srtt_ns.unwrap_or(u64::MAX));
    (h.0, link.delivered)
}

// ── taban izleri: 5de1ce3f (origin/main) ağacında bu dosyanın E2a'sız kopyasıyla kaydedildi ──
// (2026-11-09; ayrı worktree, ayrı target; varsayılan ve --all-features derlemesinde AYNI değer;
// iki kurulumda da `delivered = 120`).
const BASE_DIGEST_PRODUCT: u64 = 0x1e49_5f75_283c_9675;
// [K4G-05b-R · sahip kararı 2026-11-11] Çıplak izin tabanı yeniden kaydedildi. Önceki değer
// 0xa9f2_a6bc_da15_8482 (5de1ce3f). Motor artık teyitli birimin bekleyen işini iptal eder: bu
// senaryoda birim 0'ın orijinali (seq 0) kaybolur, kuruluş kopyası (seq 1, Duplicate) ulaşır ve
// birimi teyit eder; önceki motor 10.635 s'de birim 0'a bir Retransmit daha veriyordu (teyitli
// birime karar), şimdi vermez. Sonraki seq'ler bir kaydığından kayıp karması (seq'e bağlı) izin
// geri kalanını değiştirir. Yeni değer bu dalın ağacında (bayrak kapalı) kaydedildi; ürün benzeri
// iz (`BASE_DIGEST_PRODUCT`) DEĞİŞMEDİ. "Bayrak kapalı = bayraksız" özdeşliği aşağıdaki ikinci
// iddiada (`E2aFlags::default()`) aynen sınanır.
const BASE_DIGEST_BARE: u64 = 0x9e4f_8cca_eac8_6803;

fn on(tt_rfc: bool, g_ns: u64, v11: bool, iw: bool, probes: u8) -> E2aFlags {
    E2aFlags::new(tt_rfc, g_ns, v11, iw, probes, false).expect("geçerli bayrak")
}

/// KK-M1-11 "bayrak kapalıyken tel baytı bit-aynı": setter'sız ve `E2aFlags::default()` ile iz,
/// 5de1ce3f tabanının iziyle birebir aynı.
#[test]
fn e2a_flags_off_trace_is_bit_identical_to_base() {
    for (product, base) in [(true, BASE_DIGEST_PRODUCT), (false, BASE_DIGEST_BARE)] {
        let (d0, del0) = run(product, &|_| {});
        let (d1, _) = run(product, &|s| s.set_e2a_flags(E2aFlags::default()));
        assert_eq!(
            d0, base,
            "product={product}: setter'sız iz = taban (0x{d0:016x})"
        );
        assert_eq!(
            d1, base,
            "product={product}: varsayılan bayraklı iz = taban"
        );
        assert_eq!(del0.len() as u64, UNITS);
    }
}

/// Her kural TEK BAŞINA açıkken: iz deterministik (iki koşu aynı), tabandan FARKLI (kural bu
/// senaryoda yol üstünde → kapalı-kol bit-aynılığı o kuralın kod yolunu kapsıyor) ve sunulan her
/// birim teslim edilir (canlılık). Hepsi birlikte açıkken de aynı iki şart.
#[test]
fn e2a_each_flag_on_is_deterministic_live_and_on_path() {
    let cases: [(&str, E2aFlags); 5] = [
        ("m1 tt_rfc g=1ms", on(true, MS, false, false, 1)),
        ("m8 rtt_rule_v11", on(false, MS, true, false, 1)),
        ("m6 iw_idle_hold", on(false, MS, false, true, 1)),
        ("pto_probes=2", on(false, MS, false, false, 2)),
        ("hepsi (g=1ms)", on(true, MS, true, true, 2)),
    ];
    for (name, f) in cases {
        let (a, del_a) = run(true, &|s| s.set_e2a_flags(f));
        let (b, _) = run(true, &|s| s.set_e2a_flags(f));
        assert_eq!(a, b, "{name}: deterministik");
        assert_ne!(
            a, BASE_DIGEST_PRODUCT,
            "{name}: kural bu senaryoda yol üstünde"
        );
        assert_eq!(del_a.len() as u64, UNITS, "{name}: her birim teslim edildi");
    }
}
