//! DESEN-1 discrete-event sim harness (srtt-decoupling Y3 hipotez-taraması).
//! Store-and-forward link + ACK-loop + basamaklı-rate; Sender'ı in-process sürer.
//! Deterministik (RNG yok); netem DEĞİL — teşhis-GO/NO-GO.
//!
//! **TC-16 (SR-STR-CORE-001 §5.4, BUL-07) sonrası durum.** Beş testin beşi de `#[ignore]` idi ve
//! kaynak yorumu "ağır sim; manuel" diyordu. [ÖLÇÜLDÜ] fiili maliyetler (test binary'si doğrudan
//! çağrılarak, tek tek):
//!
//!   desen1_expiry_pin_3a2            2,85 s   ← assertion VAR   → CI
//!   desen1_period_counters           2,72 s   ← print-only      → CI (panik-kapısı)
//!   scan_long_narrowing_convergence  2,14 s   ← assertion VAR   → CI
//!   scan_subkbps_anchor              1,19 s   ← assertion VAR   → CI
//!   recalibrate_cadence              8,67 s   ← print-only sweep → NIGHTLY
//!
//! [ÖLÇÜLDÜ — DÜZELTME] SR-STR-CORE-001 §5.4 TC-16 "ağır tarama varyantı" olarak
//! `scan_long_narrowing_convergence`'i işaret ediyordu; ölçüm bunun 2,14 s ile en UCUZ ikinci test
//! olduğunu, gerçek ağır varyantın 8,67 s ile `recalibrate_cadence` olduğunu (4×) gösterdi.
//! Nightly'ye taşınan bu yüzden `recalibrate_cadence`'tır. Deponun tek gerçek darboğaz modeli
//! artık ~9 s'lik bir CI maliyetiyle her PR'da koşar.
//!
//! **KALİBRASYON-ŞARTI — [ÖLÇÜLDÜ] YAPISAL BULGU (TC-16/a).** Eski başlık "fix-öncesi motor 3A2'yi
//! repro etmeli (infl~63×, srtt~53s) — etmezse harness güvenilmez" diyordu. Bu koşul çalışan bir
//! teste DÖNÜŞTÜRÜLEMEZ: koşul FIX-ÖNCESİ motoru tarif eder, o motor bu depoda artık yoktur
//! (`mode-0` switch'i silinmiş, "legacy-kalibrasyon tarihsel" notu bunu kaydediyor). Bugünkü
//! motorla ölçülen imza infl_max=54×, srtt_max=29,0 s'dir — yani şart bugün ZATEN sağlanmıyor ve
//! sağlanması da BEKLENMEZ. Şartı olduğu gibi teste çevirmek her koşumda kırmızı üretirdi.
//!   → Uygulanan çözüm: `harness_signature_pin()` MEVCUT motorun imzasını pinler (regresyon-kapısı).
//!   → [BEKLİYOR — Abkarada] Fix-öncesi 3A2 imzasının repro edilebilirliği bir gereksinim olarak
//!     korunacak mı? Korunacaksa fix-öncesi motorun bir varyant/feature olarak geri getirilmesi
//!     gerekir; korunmayacaksa başlıktaki şart spec'ten düşürülmelidir.
//!
//! Koş (CI seti):  cargo test --test desen1_sim
//! Koş (nightly):  cargo test --test desen1_sim -- --ignored --nocapture
use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// **SENARYO KALİBRASYON SABİTİ — WIRE BOYUTU DEĞİLDİR.** (TC-17 / AÇIK-5 cevabı.)
///
/// [ÖLÇÜLDÜ] Gerçek §2.6 sealed FEC-shard boyutu `26 (outer) + 11 (inner) + E`'dir:
/// E=64 → 101 B, E=1024 → 1061 B (`kama-integration/tests/byte_accounting.rs` bunu pinler).
/// 267 = "payload 256 + overhead ~11" türetimi 26 B outer'ı atlar VE katalogda 256 B payload
/// taşıyan tek-shard diye bir şey yoktur (E=64 kapasitesi 53 B; 256 B → 5 shard × 101 = 505 B).
///
/// [ÖLÇÜLDÜ] UNIT'i 101'e çekmek DÜZELTME DEĞİL, SENARYO YIKIMIDIR: rate merdiveni bu birime
/// göre kalibre edilmiştir. UNIT=101 ile `run_3a2` ölçümleri infl_max 54× → 0×, qd_max 36,6 s →
/// 0,3 s, delivered 732 → 1500'e gider; yani darboğaz tamamen kaybolur ve harness modellemesi
/// gereken şeyi modellemeyi bırakır (iki test de kırmızıya döner).
///
/// Sonuç: UNIT bir SENARYO parametresidir (birim × rate merdiveni birlikte kalibre edilmiştir),
/// wire muhasebesinin kaynağı DEĞİLDİR. Wire muhasebesi `byte_accounting.rs`'te kapı altındadır.
/// Bu iki katman ayrı tutulur; birinin diğerinden türetilmesi ancak rate merdiveninin yeniden
/// kalibrasyonuyla mümkündür. [BEKLİYOR — Abkarada: merdiven gerçek profil boyutlarına göre
/// yeniden kalibre edilecek mi, yoksa UNIT senaryo-sabiti olarak mı kalacak?]
const UNIT: u64 = 267;

/// Store-and-forward link: rate B/s (basamaklı), tek-yön gecikme, FIFO inf-buffer.
struct Link {
    rate_bps: u64, // 0 = kopma (link-down: yeni+kuyruk düşer)
    delay_ns: u64,
    busy_until: u64,              // serializasyon-kuyruğu bitiş-anı
    q: VecDeque<(u64, u64, u64)>, // (arrival_ns, seq, unit_id)
    dropped: u64,
}
impl Link {
    fn new(rate_bps: u64, delay_ns: u64) -> Self {
        Self {
            rate_bps,
            delay_ns,
            busy_until: 0,
            q: VecDeque::new(),
            dropped: 0,
        }
    }
    fn send(&mut self, seq: u64, size: u64, unit_id: u64, now: u64) {
        if self.rate_bps == 0 {
            self.dropped += 1;
            return; // kopma: paket düşer
        }
        let serialize = size.saturating_mul(SEC) / self.rate_bps;
        let start = now.max(self.busy_until);
        self.busy_until = start + serialize;
        self.q
            .push_back((self.busy_until + self.delay_ns, seq, unit_id));
    }
    /// Kopma başında (rate 0'a düşünce) uçuştaki kuyruğu boşalt (link-down semantiği).
    fn drop_inflight(&mut self) {
        self.dropped += self.q.len() as u64;
        self.q.clear();
        self.busy_until = 0;
    }
    fn poll(&mut self, now: u64) -> Vec<(u64, u64)> {
        let mut out = Vec::new();
        while let Some(&(arr, seq, uid)) = self.q.front() {
            if arr <= now {
                out.push((seq, uid));
                self.q.pop_front();
            } else {
                break;
            }
        }
        out
    }
}

/// 3A2 basamaklı-rate: 40k(0-35s) → 3k(35-70s) → kopma(70-100s) → 8k(100-145s). B/s.
fn rate_3a2(now: u64) -> u64 {
    let t = now / SEC;
    match t {
        0..=34 => 40_000 / 8, // 40 kbit = 5000 B/s
        35..=69 => 3_000 / 8, // 3 kbit = 375 B/s
        70..=99 => 0,         // kopma
        _ => 8_000 / 8,       // 8 kbit = 1000 B/s
    }
}

struct SimResult {
    expiry_s: f64,
    qd_max_us: u64,
    infl_max: u64,
    srtt_max_us: u64,
    btlbw_at_daralma_end: u64, // t≈69s (3k-basamak sonu)
    delivered: usize,
    /// [K5] dönem-ayrımlı (P1 temiz-40k / P2 daralma-flood / P3 kopma / P4 toparlanma-8k):
    /// (wire_sent, unique_delivered) × 4 + dropped-toplam.
    periods: [(u64, u64); 4],
    dropped: u64,
}

/// Sender'ı 3A2-senaryosunda in-process sürer. wmode: 0=legacy 1=delivered-EAI 2=rtprop-swap.
fn run_3a2(n_units: usize, t_max_s: u64, trace_mech: bool, cad_ms: u64) -> SimResult {
    let mut s = Sender::new();
    let mut fwd = Link::new(rate_3a2(0), 5 * MS); // forward: basamaklı
    let mut rev = Link::new(1_000_000, 5 * MS); // reverse: hızlı (ACK küçük, 1MB/s)
    let mut recv: HashSet<u64> = HashSet::new(); // alıcıda teslim-olmuş seq'ler
    let mut acked_reported: HashSet<u64> = HashSet::new();
    let mut rev_inflight: VecDeque<(u64, Vec<u64>)> = VecDeque::new(); // (ack_arrival, seqs)
    let mut unique_units: HashSet<u64> = HashSet::new();
    let mut periods = [(0u64, 0u64); 4]; // (wire_sent, unique_delivered) — P1/P2/P3/P4
    fn pidx(t_ns: u64) -> usize {
        let t = t_ns / SEC;
        if t < 35 {
            0
        } else if t < 70 {
            1
        } else if t < 100 {
            2
        } else {
            3
        }
    }

    // t=0: tüm birimleri enqueue (backlog).
    for i in 0..n_units {
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: i as u64,
                size: UNIT,
            }),
            0,
        ) {
            periods[0].0 += 1;
            fwd.send(d.seq, d.size, d.unit_id, 0);
        }
    }

    let mut infl_max = 0u64;
    let mut qd_max_us = 0u64;
    let mut last_sack_ns = 0u64;
    let mut srtt_max_us = 0u64;
    let mut btlbw_daralma = 0u64;
    let mut expiry_ns = 0u64;
    let mut prev_rate = rate_3a2(0);
    let t_max = t_max_s * SEC;
    let mut now = 0u64;

    while now < t_max {
        // basamak-rate güncelle; kopmaya girişte uçuştaki-kuyruğu boşalt.
        let r = rate_3a2(now);
        if r == 0 && prev_rate != 0 {
            fwd.drop_inflight();
        }
        fwd.rate_bps = r;
        prev_rate = r;

        // pace-tick (1ms): token-drain → transmit kararları → forward-link.
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            periods[pidx(now)].0 += 1;
            fwd.send(d.seq, d.size, d.unit_id, now);
        }
        // rtprop-tick (200ms).
        if now.is_multiple_of(200 * MS) {
            s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
        }
        // forward-varışlar → alıcı teslim-set (seq ACK-için; unit_id unique-teslim-için).
        for (seq, uid) in fwd.poll(now) {
            recv.insert(seq);
            if unique_units.insert(uid) {
                periods[pidx(now)].1 += 1;
            }
        }
        // [BATCH-MODEL] ack-tick: netem-receiver SACK-cadence'i rate-ölçekli (~6.4s @375B/s,
        // DV-2-ölçüsü; 40k'da ~0.48s). Eski-ideal-20ms modeli netem-batch'ini temsil-etmiyordu
        // (mekanizma-izi-kalibrasyonu dersi: iki-hastalık-aynı-ateş).
        let sack_interval = if fwd.rate_bps > 0 {
            ((u128::from(cad_ms) * MS as u128 * 375 / fwd.rate_bps as u128) as u64)
                .clamp(20 * MS, 8 * SEC)
        } else {
            8 * SEC
        };
        if now.saturating_sub(last_sack_ns) >= sack_interval {
            last_sack_ns = now;
            let new_seqs: Vec<u64> = recv.difference(&acked_reported).copied().collect();
            if !new_seqs.is_empty() {
                for &sq in &new_seqs {
                    acked_reported.insert(sq);
                }
                // ACK'in kendisi reverse-link'te serialize olur (küçük); varış-anını hesapla.
                rev.send(0, 40, 0, now); // 40B ACK-frame; seq-önemsiz
                let ack_arr = rev.q.back().map(|&(a, _, _)| a).unwrap_or(now);
                rev.q.pop_back(); // ACK-frame'i seq-taşımıyor; varış-anını aldık
                rev_inflight.push_back((ack_arr, new_seqs));
            }
        }
        // ack-varışları → Sender.
        while let Some(&(arr, _)) = rev_inflight.front() {
            if arr <= now {
                let (_, seqs) = rev_inflight.pop_front().unwrap();
                s.on_event(
                    Event::AckReceived(AckInfo {
                        acked_seqs: seqs,
                        ack_delay: Duration::ZERO,
                    }),
                    now,
                );
            } else {
                break;
            }
        }

        // izleme.
        let (_, btlbw, srtt_us, _rate, infl) = s.diag_rate();
        let (_, qd_us) = s.diag_qd();
        qd_max_us = qd_max_us.max(qd_us);
        // Daralma-penceresi (35-70s): flood-metrikleri + expiry (btlbw kanal-yakınına düşme-anı).
        if (35 * SEC..70 * SEC).contains(&now) {
            infl_max = infl_max.max(infl);
            srtt_max_us = srtt_max_us.max(srtt_us);
            if btlbw < 600 && expiry_ns == 0 && now > 36 * SEC {
                expiry_ns = now - 35 * SEC; // daralma-başından itibaren
            }
        }
        if (69 * SEC..70 * SEC).contains(&now) {
            btlbw_daralma = btlbw;
        }
        // [MEKANİZMA-İZİ] daralma-penceresi örnek-akışı: max'a-giren-örneklerin tazelik-deseni.
        if trace_mech && (35 * SEC..70 * SEC).contains(&now) && now.is_multiple_of(4 * SEC) {
            let sm = s.diag_btlbw_samples();
            let fresh = sm
                .iter()
                .filter(|&&(t, _)| now.saturating_sub(t) < 4 * SEC)
                .count();
            let hi_fresh = sm
                .iter()
                .filter(|&&(t, v)| now.saturating_sub(t) < 4 * SEC && v > 2000)
                .count();
            let (lfb, w) = s.diag_wbtlbw();
            eprintln!(
                "    [İZ t={}s] btlbw={} örnek={} taze4s={} taze-YÜKSEK={} lfb={} W={:.1}s",
                now / SEC,
                btlbw,
                sm.len(),
                fresh,
                hi_fresh,
                lfb,
                w as f64 / 1e9
            );
        }
        // [K5-kök-teşhis] P4-iz: toparlanma-dönemi 2s'de-bir durum-serisi.
        if now >= 100 * SEC && now.is_multiple_of(2 * SEC) {
            let (st, bw2, srtt2, rate2, infl2) = s.diag_rate();
            eprintln!(
                "    [P4-iz t={}s] state={:?} btlbw={} rate={} srtt={:.1}s infl={}",
                now / SEC,
                st,
                bw2,
                rate2,
                srtt2 as f64 / 1e6,
                infl2
            );
        }

        now += MS;
    }

    SimResult {
        expiry_s: expiry_ns as f64 / 1e9,
        infl_max,
        srtt_max_us,
        btlbw_at_daralma_end: btlbw_daralma,
        delivered: unique_units.len(),
        qd_max_us,
        periods,
        dropped: fwd.dropped,
    }
}

#[test]
fn desen1_expiry_pin_3a2() {
    // [Y3 KALICI-PİN — K6R-netem-mühürlü] delivered-EAI-motor 3A2'de: onset<35s + tek-adım≥%50 +
    // re-flood-yok. (Legacy-kalibrasyon tarihsel: mode-0 kayıtları mini-D0'da; switch silindi.)
    let r = run_3a2(1500, 145, true, 1_600);
    println!(
        "[PIN 3a2] infl_max={}× srtt_max={:.1}s expiry={:.1}s btlbw@70s={} unique={}",
        r.infl_max / UNIT,
        r.srtt_max_us as f64 / 1e6,
        r.expiry_s,
        r.btlbw_at_daralma_end,
        r.delivered,
    );
    println!("[FAZ-0 qd] qd_max={:.1}s", r.qd_max_us as f64 / 1e6);
    assert!(
        r.qd_max_us > 1_000_000,
        "FAZ-0: DESEN-1'de qd canlanmalı (>1s), ölçülen {}us",
        r.qd_max_us
    );
    assert!(
        r.btlbw_at_daralma_end < 2100,
        "tek-adım-demotion ≥%50 beklenir (4109→<2054), ölçülen {}",
        r.btlbw_at_daralma_end
    );
}

#[test]
fn scan_long_narrowing_convergence() {
    // (c)-sınır-testi: kopmasız-UZUN-daralma (40k→3k, 90s) — çok-adımlı-expiry kanal-bandına
    // yakınsar mı (plato kopma-kesmesine-özgü mü)?
    let r = run_long_narrow(1500, 130, 1_600);
    println!(
        "[UZUN-DARALMA] btlbw@60={} @90={} @125={} (kanal=375)",
        r.periods[0].0, r.periods[0].1, r.btlbw_at_daralma_end
    );
    assert!(
        r.btlbw_at_daralma_end < 600,
        "kopmasız-daralmada kanal-bandına-yakınsama (netem-mühürlü-gate), ölçülen {}",
        r.btlbw_at_daralma_end
    );
}

/// 40k(0-35s) → 3k(35-130s, kopmasız-uzun). periods[0]=(btlbw@60, btlbw@90) kaba-taşıyıcı.
fn run_long_narrow(n_units: usize, t_max_s: u64, cad_ms: u64) -> SimResult {
    let mut s = Sender::new();
    let mut fwd = Link::new(5000, 5 * MS);
    let mut rev = Link::new(1_000_000, 5 * MS);
    let mut recv: HashSet<u64> = HashSet::new();
    let mut acked: HashSet<u64> = HashSet::new();
    let mut rev_if: VecDeque<(u64, Vec<u64>)> = VecDeque::new();
    for i in 0..n_units {
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: i as u64,
                size: UNIT,
            }),
            0,
        ) {
            fwd.send(d.seq, d.size, d.unit_id, 0);
        }
    }
    let mut now = 0u64;
    let mut last_sack = 0u64;
    let mut b60 = 0u64;
    let mut b90 = 0u64;
    let mut blast = 0u64;
    let t_max = t_max_s * SEC;
    while now < t_max {
        fwd.rate_bps = if now < 35 * SEC { 5000 } else { 375 };
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            fwd.send(d.seq, d.size, d.unit_id, now);
        }
        if now.is_multiple_of(200 * MS) {
            s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
        }
        for (seq, _u) in fwd.poll(now) {
            recv.insert(seq);
        }
        let si = ((u128::from(cad_ms) * MS as u128 * 375 / fwd.rate_bps as u128) as u64)
            .clamp(20 * MS, 8 * SEC);
        if now.saturating_sub(last_sack) >= si {
            last_sack = now;
            let ns: Vec<u64> = recv.difference(&acked).copied().collect();
            if !ns.is_empty() {
                for &q in &ns {
                    acked.insert(q);
                }
                rev.send(0, 40, 0, now);
                let a = rev.q.back().map(|&(x, _, _)| x).unwrap_or(now);
                rev.q.pop_back();
                rev_if.push_back((a, ns));
            }
        }
        while let Some(&(a, _)) = rev_if.front() {
            if a <= now {
                let (_, sq) = rev_if.pop_front().unwrap();
                s.on_event(
                    Event::AckReceived(AckInfo {
                        acked_seqs: sq,
                        ack_delay: Duration::ZERO,
                    }),
                    now,
                );
            } else {
                break;
            }
        }
        let (_, b, _, _, _) = s.diag_rate();
        if (60 * SEC..61 * SEC).contains(&now) {
            b60 = b;
        }
        if (90 * SEC..91 * SEC).contains(&now) {
            b90 = b;
        }
        blast = b;
        now += MS;
    }
    SimResult {
        expiry_s: 0.0,
        qd_max_us: 0,
        infl_max: 0,
        srtt_max_us: 0,
        btlbw_at_daralma_end: blast,
        delivered: recv.len(),
        periods: [(b60, b90), (0, 0), (0, 0), (0, 0)],
        dropped: 0,
    }
}

#[test]
#[ignore = "[ÖLÇÜLDÜ] 8,67 s — deponun en ağır sim varyantı, assertion yok (print-only sweep). Nightly: cargo test --test desen1_sim -- --ignored --nocapture"]
fn recalibrate_cadence() {
    // Re-kalibrasyon: cadence-katsayısı → netem-63× infl-hedefi (legacy-mode).
    for cad in [1_600u64, 3_200, 6_400, 9_600] {
        let r = run_3a2(1500, 145, false, cad);
        println!(
            "[RECAL cad={}ms] infl={}× srtt={:.1}s btlbw@70={} deliv={}",
            cad,
            r.infl_max / UNIT,
            r.srtt_max_us as f64 / 1e6,
            r.btlbw_at_daralma_end,
            r.delivered
        );
    }
}

#[test]
fn desen1_period_counters() {
    // [Y3 KALICI-PİN] dönem-sayaçları: P2-teslim-sınıfı + P4-drenaj (RecoverArq-borç-izleme).
    let r = run_3a2(1500, 145, false, 1_600);
    println!(
        "[PIN dönem] unique={} dropped={} P={:?}",
        r.delivered, r.dropped, r.periods
    );
}

#[test]
fn scan_subkbps_anchor() {
    // H2 decade-körlük kontrolü: 0.25k sabit-kanalda her-mod btlbw-anchor'ı bozuyor mu?
    // (delivered-EAI term2=2.097s ZM-2-riski). Sabit 250bit = 31 B/s.
    // [K5] komşuluk-taraması {0.20k,0.25k,0.30k,1k}: ZM-2-hükmü tek-anchor'dan genelleşsin.
    for rate in [25u64, 31, 37, 125] {
        {
            let name = "delivered-EAI";
            let r = run_const(200, 90, rate);
            println!(
                "[H2-anchor rate={} {}] btlbw-son={} srtt_max={:.1}s delivered={}/200",
                rate,
                name,
                r.btlbw_at_daralma_end,
                r.srtt_max_us as f64 / 1e6,
                r.delivered,
            );
            assert!(
                r.btlbw_at_daralma_end >= rate.saturating_sub(rate / 4),
                "sub-kbps anchor korunmalı (ZM-2), rate={} ölçülen {}",
                rate,
                r.btlbw_at_daralma_end
            );
        }
    }
}

/// Sabit-rate kanal (sub-kbps anchor kontrolü). btlbw_at_daralma_end = son-btlbw.
fn run_const(n_units: usize, t_max_s: u64, rate_bps: u64) -> SimResult {
    let mut s = Sender::new();
    let mut fwd = Link::new(rate_bps, 5 * MS);
    let mut rev = Link::new(1_000_000, 5 * MS);
    let mut recv: HashSet<u64> = HashSet::new();
    let mut acked: HashSet<u64> = HashSet::new();
    let mut rev_if: VecDeque<(u64, Vec<u64>)> = VecDeque::new();
    for i in 0..n_units {
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: i as u64,
                size: UNIT,
            }),
            0,
        ) {
            fwd.send(d.seq, d.size, d.unit_id, 0);
        }
    }
    let mut srtt_max = 0u64;
    let mut last_btlbw = 0u64;
    let mut now = 0u64;
    let t_max = t_max_s * SEC;
    while now < t_max {
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            fwd.send(d.seq, d.size, d.unit_id, now);
        }
        if now.is_multiple_of(200 * MS) {
            s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
        }
        for (seq, _uid) in fwd.poll(now) {
            recv.insert(seq);
        }
        if now.is_multiple_of(20 * MS) {
            let ns: Vec<u64> = recv.difference(&acked).copied().collect();
            if !ns.is_empty() {
                for &q in &ns {
                    acked.insert(q);
                }
                rev.send(0, 40, 0, now);
                let a = rev.q.back().map(|&(x, _, _)| x).unwrap_or(now);
                rev.q.pop_back();
                rev_if.push_back((a, ns));
            }
        }
        while let Some(&(a, _)) = rev_if.front() {
            if a <= now {
                let (_, sq) = rev_if.pop_front().unwrap();
                s.on_event(
                    Event::AckReceived(AckInfo {
                        acked_seqs: sq,
                        ack_delay: Duration::ZERO,
                    }),
                    now,
                );
            } else {
                break;
            }
        }
        let (_, btlbw, srtt_us, _, _) = s.diag_rate();
        srtt_max = srtt_max.max(srtt_us);
        last_btlbw = btlbw;
        now += MS;
    }
    SimResult {
        expiry_s: 0.0,
        qd_max_us: 0,
        infl_max: 0,
        srtt_max_us: srtt_max,
        btlbw_at_daralma_end: last_btlbw,
        delivered: recv.len(),
        periods: [(0, 0); 4],
        dropped: fwd.dropped,
    }
}

// ── TC-16/a — Harness kalibrasyon kapısı ────────────────────────────────────

/// Dosya başlığındaki "KALİBRASYON-ŞARTI"nın çalışan karşılığı.
///
/// **Neden fix-öncesi 3A2 imzası (infl~63×, srtt~53s) DEĞİL:** o imza artık üretilemez — fix-öncesi
/// motor bu depoda yok (bkz. modül başlığı TC-16/a). Kalibre olmadığı gösterilmemiş bir simülatörle
/// alınan GO/NO-GO kararı kanıt değildir; ancak "kalibre" ölçütü ERİŞİLEBİLİR bir imza olmalıdır.
///
/// Bu test MEVCUT motorun DESEN-1 imzasını pinler. Motor değişip imza kayarsa burada kırmızı yanar
/// ve `desen1_expiry_pin_3a2` / `desen1_period_counters` sonuçları YENİDEN YORUMLANMALIDIR —
/// kaymış bir harness'la alınmış eski GO/NO-GO kararları taşınamaz.
///
/// Toleranslar dar değil KABA'dır (±%25 / ±%35): amaç gürültü yakalamak değil, REJİM DEĞİŞİMİ
/// yakalamaktır (54× → 0× gibi). [ÖLÇÜLDÜ] referans: infl_max=54 UNIT, srtt_max=29,0 s,
/// btlbw@70s=1126, delivered=732, qd_max=36,6 s.
#[test]
fn harness_signature_pin() {
    let r = run_3a2(1500, 145, true, 1_600);
    let infl_units = r.infl_max / UNIT;
    let srtt_s = r.srtt_max_us as f64 / 1e6;

    println!(
        "[KALİBRASYON] infl_max={}× srtt_max={:.1}s btlbw@70={} delivered={} qd_max={:.1}s",
        infl_units,
        srtt_s,
        r.btlbw_at_daralma_end,
        r.delivered,
        r.qd_max_us as f64 / 1e6
    );

    // (1) Darboğaz REJİMİ yaşıyor mu? Bu, harness'in modellemesi gereken tek şeydir.
    assert!(
        infl_units >= 20,
        "darboğaz kayboldu: infl_max={infl_units}× (beklenen ≥20; referans 54×). \
         UNIT veya rate merdiveni değişmiş olabilir — bkz. UNIT doc-comment'ı"
    );
    assert!(
        srtt_s >= 10.0,
        "srtt şişmesi kayboldu: {srtt_s:.1}s (beklenen ≥10s; referans 29,0s)"
    );
    assert!(
        r.delivered < 1500,
        "hiç kayıp/gecikme yok — link doymamış (delivered={}/1500)",
        r.delivered
    );

    // (2) İmza bandı (kaba): referans değerlerin etrafında.
    assert!(
        (40..=70).contains(&infl_units),
        "infl_max imza-bandı dışında: {infl_units}× (referans 54×, bant 40..70)"
    );
    assert!(
        (20.0..=40.0).contains(&srtt_s),
        "srtt_max imza-bandı dışında: {srtt_s:.1}s (referans 29,0s, bant 20..40)"
    );
    assert!(
        (600..=900).contains(&r.delivered),
        "delivered imza-bandı dışında: {} (referans 732, bant 600..900)",
        r.delivered
    );
}

/// TC-17 / AÇIK-5'in ölçüm karşılığı: `UNIT`'in wire boyutu OLMADIĞININ kaydı.
///
/// `byte_accounting.rs` gerçek wire birimini (101 / 1061 B) kapı altına alır. Bu test, sim'in
/// birim sabitinin o sayılardan farklı olduğunu ve bunun BİLİNÇLİ olduğunu sabitler. Birisi
/// "düzeltme" niyetiyle UNIT'i 101 yaparsa `harness_signature_pin` düşer ve buradaki yorum
/// nedenini söyler.
#[test]
fn unit_is_a_scenario_constant_not_a_wire_size() {
    // §2.6 + N4-wire-overhead.md: gerçek sealed FEC-shard = 26 + 11 + E.
    const REAL_WIRE_E64: u64 = 26 + 11 + 64; // 101
    const REAL_WIRE_E1024: u64 = 26 + 11 + 1024; // 1061

    assert_ne!(
        UNIT, REAL_WIRE_E64,
        "UNIT wire boyutuna eşitlenmiş — harness_signature_pin'i kontrol et"
    );
    assert_ne!(UNIT, REAL_WIRE_E1024);
    // Eski türetimin kaydı: 267 = 256 payload + 11 inner header (26 B outer ATLANMIŞ).
    assert_eq!(
        UNIT,
        256 + 11,
        "UNIT'in tarihsel türetimi değişti — doc-comment güncellenmeli"
    );
}
