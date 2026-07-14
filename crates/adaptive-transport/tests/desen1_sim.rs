//! DESEN-1 discrete-event sim harness (srtt-decoupling Y3 hipotez-taraması).
//! Store-and-forward link + ACK-loop + basamaklı-rate; Sender'ı in-process sürer.
//! KALİBRASYON-ŞARTI: fix-öncesi motor 3A2'yi repro etmeli (infl~63×, srtt~53s) —
//! etmezse harness güvenilmez. Deterministik (RNG yok); netem DEĞİL — teşhis-GO/NO-GO.
//! Koş: cargo test --test desen1_sim -- --ignored --nocapture

use adaptive_transport::sender::{AckInfo, Event, OpaqueUnit, Sender, Timer};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

const NS: u64 = 1; // ns birimi
const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const UNIT: u64 = 267; // wire birim (payload 256 + overhead ~ tur5 ölçüsü)

/// Store-and-forward link: rate B/s (basamaklı), tek-yön gecikme, FIFO inf-buffer.
struct Link {
    rate_bps: u64, // 0 = kopma (link-down: yeni+kuyruk düşer)
    delay_ns: u64,
    busy_until: u64,         // serializasyon-kuyruğu bitiş-anı
    q: VecDeque<(u64, u64)>, // (arrival_ns, seq)
}
impl Link {
    fn new(rate_bps: u64, delay_ns: u64) -> Self {
        Self {
            rate_bps,
            delay_ns,
            busy_until: 0,
            q: VecDeque::new(),
        }
    }
    fn send(&mut self, seq: u64, size: u64, now: u64) {
        if self.rate_bps == 0 {
            return; // kopma: paket düşer
        }
        let serialize = size.saturating_mul(SEC) / self.rate_bps;
        let start = now.max(self.busy_until);
        self.busy_until = start + serialize;
        self.q.push_back((self.busy_until + self.delay_ns, seq));
    }
    /// Kopma başında (rate 0'a düşünce) uçuştaki kuyruğu boşalt (link-down semantiği).
    fn drop_inflight(&mut self) {
        self.q.clear();
        self.busy_until = 0;
    }
    fn poll(&mut self, now: u64) -> Vec<u64> {
        let mut out = Vec::new();
        while let Some(&(arr, seq)) = self.q.front() {
            if arr <= now {
                out.push(seq);
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
    infl_max: u64,
    srtt_max_us: u64,
    btlbw_at_daralma_end: u64, // t≈69s (3k-basamak sonu)
    shrink_resets: u64,
    delivered: usize,
}

/// Sender'ı 3A2-senaryosunda in-process sürer. wmode: 0=legacy 1=delivered-EAI 2=rtprop-swap.
fn run_3a2(n_units: usize, t_max_s: u64, wmode: u8) -> SimResult {
    let mut s = Sender::new();
    s.set_wbtlbw_mode(wmode);
    let mut fwd = Link::new(rate_3a2(0), 5 * MS); // forward: basamaklı
    let mut rev = Link::new(1_000_000, 5 * MS); // reverse: hızlı (ACK küçük, 1MB/s)
    let mut recv: HashSet<u64> = HashSet::new(); // alıcıda teslim-olmuş seq'ler
    let mut acked_reported: HashSet<u64> = HashSet::new();
    let mut rev_inflight: VecDeque<(u64, Vec<u64>)> = VecDeque::new(); // (ack_arrival, seqs)

    // t=0: tüm birimleri enqueue (backlog).
    for i in 0..n_units {
        for d in s.on_event(
            Event::AppData(OpaqueUnit {
                id: i as u64,
                size: UNIT,
            }),
            0,
        ) {
            fwd.send(d.seq, d.size, 0);
        }
    }

    let mut infl_max = 0u64;
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
            fwd.send(d.seq, d.size, now);
        }
        // rtprop-tick (200ms).
        if now % (200 * MS) == 0 {
            s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
        }
        // forward-varışlar → alıcı teslim-set.
        for seq in fwd.poll(now) {
            recv.insert(seq);
        }
        // ack-tick (20ms): yeni-teslim seq'leri kümülatif-ACK → reverse-link.
        if now % (20 * MS) == 0 {
            let new_seqs: Vec<u64> = recv.difference(&acked_reported).copied().collect();
            if !new_seqs.is_empty() {
                for &sq in &new_seqs {
                    acked_reported.insert(sq);
                }
                // ACK'in kendisi reverse-link'te serialize olur (küçük); varış-anını hesapla.
                rev.send(0, 40, now); // 40B ACK-frame; seq-önemsiz
                let ack_arr = rev.q.back().map(|&(a, _)| a).unwrap_or(now);
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

        now += MS;
    }

    let (shrink_resets, _, _, _, _) = s.diag_shrink();
    SimResult {
        expiry_s: expiry_ns as f64 / 1e9,
        infl_max,
        srtt_max_us,
        btlbw_at_daralma_end: btlbw_daralma,
        shrink_resets,
        delivered: recv.len(),
    }
}

#[test]
#[ignore = "ağır sim; manuel: cargo test --test desen1_sim -- --ignored --nocapture"]
fn calibrate_3a2_reproduces_netem_flood() {
    // KALİBRASYON: fix-öncesi mevcut motor netem-3A2'yi repro etmeli.
    // netem-referans: infl-max ~16821B (63×267), srtt-max 24-53s, btlbw bayat-4119-sınıfı.
    let r = run_3a2(1500, 145, 0);
    println!(
        "[CALIB 3a2] infl_max={}B ({}×) srtt_max={:.1}s btlbw@daralma-end={} shrink={} delivered={}",
        r.infl_max,
        r.infl_max / UNIT,
        r.srtt_max_us as f64 / 1e6,
        r.btlbw_at_daralma_end,
        r.shrink_resets,
        r.delivered,
    );
    // Kalibrasyon-eşiği (mertebe; netem-varyansı kabul): flood GERÇEKTEN oluşmalı.
    assert!(
        r.infl_max > 20 * UNIT,
        "flood repro: infl-max ≫ birim beklenir, ölçülen {}",
        r.infl_max
    );
    assert!(
        r.srtt_max_us > 5 * MS,
        "srtt-runaway repro beklenir (>5s), ölçülen {}us",
        r.srtt_max_us
    ); // 5s = 5e6 us
}

#[test]
#[ignore = "ağır sim; manuel"]
fn scan_wbtlbw_hypotheses() {
    // H1+H2: üç W-modu 3A2'de — flood-durma + expiry + srtt-runaway kesimi.
    // 0=legacy(mevcut), 1=delivered-EAI (~28s-teori), 2=rtprop-swap.
    for (mode, name) in [(0u8, "legacy"), (1, "delivered-EAI"), (2, "rtprop-swap")] {
        let r = run_3a2(1500, 145, mode);
        println!(
            "[H1/H2 mode={} {}] infl_max={}× srtt_max={:.1}s expiry={:.1}s btlbw@70s={} delivered={}",
            mode, name, r.infl_max / UNIT, r.srtt_max_us as f64 / 1e6,
            r.expiry_s, r.btlbw_at_daralma_end, r.delivered,
        );
    }
}

#[test]
#[ignore = "ağır sim; manuel"]
fn scan_subkbps_anchor() {
    // H2 decade-körlük kontrolü: 0.25k sabit-kanalda her-mod btlbw-anchor'ı bozuyor mu?
    // (delivered-EAI term2=2.097s ZM-2-riski). Sabit 250bit = 31 B/s.
    for (mode, name) in [(0u8, "legacy"), (1, "delivered-EAI"), (2, "rtprop-swap")] {
        let r = run_const(200, 90, 31, mode);
        println!(
            "[H2-anchor mode={} {}] btlbw-son={} srtt_max={:.1}s delivered={}/200",
            mode,
            name,
            r.btlbw_at_daralma_end,
            r.srtt_max_us as f64 / 1e6,
            r.delivered,
        );
    }
}

/// Sabit-rate kanal (sub-kbps anchor kontrolü). btlbw_at_daralma_end = son-btlbw.
fn run_const(n_units: usize, t_max_s: u64, rate_bps: u64, wmode: u8) -> SimResult {
    let mut s = Sender::new();
    s.set_wbtlbw_mode(wmode);
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
            fwd.send(d.seq, d.size, 0);
        }
    }
    let mut srtt_max = 0u64;
    let mut last_btlbw = 0u64;
    let mut now = 0u64;
    let t_max = t_max_s * SEC;
    while now < t_max {
        for d in s.on_event(Event::TimerFired(Timer::Pto), now) {
            fwd.send(d.seq, d.size, now);
        }
        if now % (200 * MS) == 0 {
            s.on_event(Event::TimerFired(Timer::RtpropProbe), now);
        }
        for seq in fwd.poll(now) {
            recv.insert(seq);
        }
        if now % (20 * MS) == 0 {
            let ns: Vec<u64> = recv.difference(&acked).copied().collect();
            if !ns.is_empty() {
                for &q in &ns {
                    acked.insert(q);
                }
                rev.send(0, 40, now);
                let a = rev.q.back().map(|&(x, _)| x).unwrap_or(now);
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
        infl_max: 0,
        srtt_max_us: srtt_max,
        btlbw_at_daralma_end: last_btlbw,
        shrink_resets: 0,
        delivered: recv.len(),
    }
}
