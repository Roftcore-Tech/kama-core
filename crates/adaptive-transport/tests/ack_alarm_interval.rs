//! [ALARM ARALIĞI · #175 7084078903 / 7084084106] Daralma alarmının ACK'i ölçümü yalnız
//! daralmadan sonra gelenlerden taşır: ölçüm aralığı `[span_first, largest]` alarm penceresindeki
//! daralma başlangıcından (`RateDropDetector::onset`) başlar, `pair_rate` yalnız alarmdan sonraki
//! çiftlerdendir. Alarm dışı ACK'ler, yükseliş alarmının ACK'i, V2 (0x14) ve profil kapalı yol
//! bugünkü gibi.
//!
//! Ürün gözlemi (koordinatör, 7084048541; düşüş-toparlanma, tohum 1): kanıtlı ilk ACK +2,2 s'de,
//! aralığı düşüş öncesi ve sonrası varışları karıştırdığı için tavan 7,2–10,5 k; ≤ 6 875 ancak
//! ≈ 5 s sonraki ACK'te. Bir ACK geriye uzanan aralık (bugünkü kural) düşüş öncesi 12 500'de varan
//! datagramları da kapsar.
//!
//! Düzenek (var olan alarm testleriyle aynı, `ack_bitmap_rx_span.rs`, `ack_uplink_yield_once.rs`):
//! 1 000 B gövde, 80 ms arayla (12 500 B/s), `DROP_AT`'te aralık 160 ms (hat yarıya, 6 250 B/s);
//! dönüş 62 B/s, bitmap açık. Kabul sınırları ürün ölçütünden: yeni hat ±%10 (tavan ölçütü
//! ≤ 6 875 = 1,1 × 6 250). Hız göndericinin muhasebesiyle: aralıkta gönderilen gövde baytı
//! (`(span_first, largest]`, kayıplar dahil) / alıcıdaki varış süresi.

use std::collections::BTreeMap;

use adaptive_transport::ack::{AckFrame, ACK_DELAY_UNIT_US, PAIR_RATE_NONE, RX_SPAN_NONE};
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
const BODY: u64 = 1_000;
const OLD_GAP: u64 = 80 * MS;
const NEW_GAP: u64 = 160 * MS;
const DROP_AT: u64 = 30 * SEC;
/// Yeni hat (B/s): 1 000 B / 160 ms.
const NEW_LINE: u64 = BODY * SEC / NEW_GAP;
/// Ürün ölçütü: tavan ≤ 6 875 = yeni hat × 1,1.
const CEILING_MAX: u64 = NEW_LINE * 11 / 10;
/// Dolu dönüş hattında her denemede önündeki serileşme (`ack_uplink_yield_once.rs` ile aynı).
const AHEAD: u64 = SEC;

struct Emitted {
    at: u64,
    frame: AckFrame,
    drop_alarms: u64,
    rise_alarms: u64,
}

struct Run {
    acks: Vec<Emitted>,
    /// Alınan seq → ilk varış anı.
    arrival: BTreeMap<u64, u64>,
}

impl Run {
    fn bitmap(&self) -> Vec<&Emitted> {
        self.acks
            .iter()
            .filter(|e| e.frame.bitmap.is_some())
            .collect()
    }

    /// İlk daralma alarmının ACK'i ve ondan önceki bitmap ACK'inin en büyüğü.
    fn alarm_ack(&self) -> (&Emitted, u64) {
        let bm = self.bitmap();
        let i = bm
            .iter()
            .position(|e| e.drop_alarms >= 1)
            .expect("daralma alarmının ACK'i yok");
        assert!(i >= 1, "alarm ACK'inden önce ACK yok");
        (bm[i], largest(&bm[i - 1].frame))
    }

    /// Bugünkü kural: son ACK'in en büyüğünden sonraki ilk alınan seq.
    fn first_received_after(&self, prev_largest: u64) -> u64 {
        *self
            .arrival
            .range(prev_largest + 1..)
            .next()
            .expect("alınan seq")
            .0
    }

    /// Göndericinin muhasebesi: `(first, last]` aralığında gönderilen gövde baytı / varış süresi.
    fn rate(&self, first: u64, last: u64) -> u64 {
        let span = self.arrival[&last] - self.arrival[&first];
        (last - first) * BODY * SEC / span
    }
}

fn largest(f: &AckFrame) -> u64 {
    u64::from(f.largest_seq)
}

fn span_first(f: &AckFrame) -> u64 {
    f.span_first_full(largest(f) + 1).expect("span_first")
}

/// `gaps`: (bu andan itibaren, varış aralığı). `lost(seq)`: datagram hatta serileşti ama düştü
/// (süre ilerler, varış yok). `busy_from`: o andan sonra dönüş hattı her denemede dolu.
fn run(gaps: &[(u64, u64)], until: u64, lost: impl Fn(u64) -> bool, busy_from: Option<u64>) -> Run {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    let mut acks = Vec::new();
    let mut arrival = BTreeMap::new();
    let mut t = SEC;
    let mut seq = 0u64;
    while t < until {
        let busy = busy_from.is_some_and(|b| t >= b);
        if !lost(seq) {
            arrival.insert(seq, t);
            let (ahead, yield_) = if busy { (AHEAD, true) } else { (0, false) };
            r.set_return_line_ahead_ns(ahead);
            r.set_uplink_yield(yield_);
            r.set_next_datagram_bytes(BODY);
            let mut out: Vec<(u64, AckFrame)> = r
                .on_datagram(
                    RxDatagram {
                        seq,
                        unit_id: seq,
                        kind: RxKind::Data,
                    },
                    t,
                )
                .ack
                .into_iter()
                .map(|f| (t, f))
                .collect();
            let timer_at = if busy { t + 30 * MS } else { t };
            r.set_return_line_ahead_ns(ahead);
            r.set_uplink_yield(yield_);
            out.extend(r.on_ack_timer(timer_at).map(|f| (timer_at, f)));
            for (at, frame) in out {
                acks.push(Emitted {
                    at,
                    frame,
                    drop_alarms: r.diag_drop_alarms(),
                    rise_alarms: r.diag_rise_alarms(),
                });
            }
        }
        seq += 1;
        let gap = gaps
            .iter()
            .rev()
            .find(|&&(from, _)| t >= from)
            .map_or(OLD_GAP, |&(_, g)| g);
        t += gap;
    }
    Run { acks, arrival }
}

fn drop_plan() -> Vec<(u64, u64)> {
    vec![(0, OLD_GAP), (DROP_AT, NEW_GAP)]
}

/// Kayıp deseni (`ack_bitmap_rx_span.rs` ile aynı): her beşte biri hatta serileşip düşer.
fn every_fifth(seq: u64) -> bool {
    seq % 5 == 3
}

/// (a) Alarm ACK'inin aralığı yalnız düşüşten sonra varanlardan, hızı yeni hat ±%10; `rx_span`
/// alanı o aralığın varış süresi. Karşı-olgu (bugünkü kural, bir ACK geriye): aralığın başı düşüş
/// öncesinde, hız ürün ölçütünün (≤ 6 875) üstünde.
#[test]
fn a_drop_alarm_ack_interval_holds_only_arrivals_after_the_drop() {
    for (name, lossy) in [("kayıpsız", false), ("her beşte bir kayıp", true)] {
        let o = run(&drop_plan(), 40 * SEC, |s| lossy && every_fifth(s), None);
        let (alarm, prev) = o.alarm_ack();
        let (sf, hi) = (span_first(&alarm.frame), largest(&alarm.frame));
        let rate = o.rate(sf, hi);
        let old_sf = o.first_received_after(prev);
        let old_rate = o.rate(old_sf, hi);
        println!(
            "{name}: alarm ACK {} ms: aralık [{sf}, {hi}] başı düşüş +{} ms, hız {rate}; \
             bugünkü kural [{old_sf}, {hi}] başı düşüş −{} ms, hız {old_rate}",
            alarm.at / MS,
            o.arrival[&sf].saturating_sub(DROP_AT) / MS,
            DROP_AT.saturating_sub(o.arrival[&old_sf]) / MS,
        );
        assert!(
            o.arrival[&sf] >= DROP_AT,
            "{name}: aralığın başı düşüş öncesinde ({} ms)",
            o.arrival[&sf] / MS
        );
        assert!(
            rate.abs_diff(NEW_LINE) * 10 <= NEW_LINE,
            "{name}: aralık hızı {rate}, yeni hat {NEW_LINE}"
        );
        let field = alarm.frame.bitmap.as_ref().expect("bitmap").rx_span;
        assert_ne!(field, RX_SPAN_NONE, "{name}: alarm ACK'inde ölçüm yok");
        assert_eq!(
            u64::from(field),
            (o.arrival[&hi] - o.arrival[&sf]) / (ACK_DELAY_UNIT_US * 1000),
            "{name}: rx_span alanı"
        );
        // Karşı-olgu: bugünkü kuralın aralığı karışık ve ölçütün üstünde.
        assert!(
            o.arrival[&old_sf] < DROP_AT,
            "{name}: karşı-olgu karışık değil"
        );
        assert!(
            old_rate > CEILING_MAX,
            "{name}: karşı-olgu hızı {old_rate} ölçütün altında"
        );
        // (b, açık hat) Alarm ACK'i alarmla hemen çıkar: alarmdan sonra çift yoksa alan yoktur;
        // varsa yeni hattın alt sınırıdır. Düşüş öncesi çiftin değeri (12 500) taşınmaz.
        let pair = alarm.frame.bitmap.as_ref().expect("bitmap").pair_rate;
        assert!(
            pair == PAIR_RATE_NONE || u64::from(pair) <= CEILING_MAX,
            "{name}: alarm ACK'inin pair_rate'i {pair} (düşüş öncesi çift)"
        );
    }
}

/// (b) Dolu dönüş hattında alarm ACK'i bir yol verişten sonra (≈ 1 s) çıkar; o sürede yeni hatla
/// gelen çiftler alanı taşır: `pair_rate` yeni hat ±%10. Aralık da yalnız düşüş sonrası. Karşı-olgu
/// (çift değeri alarmda sıfırlanmazsa): son ACK'ten bu yana en büyük ikinci düşüş öncesi çiftlerden
/// gelir (12 500).
#[test]
fn b_drop_alarm_ack_pair_rate_is_from_pairs_after_the_alarm_only() {
    let o = run(&drop_plan(), 40 * SEC, |_| false, Some(DROP_AT - SEC));
    let (alarm, _) = o.alarm_ack();
    let bm = alarm.frame.bitmap.as_ref().expect("bitmap");
    println!(
        "alarm ACK {} ms, pair_rate {}, aralık [{}, {}]",
        alarm.at / MS,
        bm.pair_rate,
        span_first(&alarm.frame),
        largest(&alarm.frame)
    );
    assert_ne!(bm.pair_rate, PAIR_RATE_NONE, "alarmdan sonra çift yok");
    assert!(
        u64::from(bm.pair_rate).abs_diff(NEW_LINE) * 10 <= NEW_LINE,
        "pair_rate {}, yeni hat {NEW_LINE}",
        bm.pair_rate
    );
    let (sf, hi) = (span_first(&alarm.frame), largest(&alarm.frame));
    assert!(o.arrival[&sf] >= DROP_AT, "aralığın başı düşüş öncesinde");
    let rate = o.rate(sf, hi);
    assert!(
        rate.abs_diff(NEW_LINE) * 10 <= NEW_LINE,
        "aralık hızı {rate}"
    );
}

/// (c) Alarm dışı bitmap ACK'lerinin aralığı bugünkü kural: son ACK'in en büyüğünden sonraki ilk
/// alınan seq (alarm ACK'inden sonraki ACK dahil).
#[test]
fn c_non_alarm_acks_keep_the_one_ack_back_interval() {
    for lossy in [false, true] {
        let o = run(&drop_plan(), 45 * SEC, |s| lossy && every_fifth(s), None);
        let bm = o.bitmap();
        let alarm_idx = bm.iter().position(|e| e.drop_alarms >= 1).expect("alarm");
        let mut checked = 0;
        for i in 1..bm.len() {
            if i == alarm_idx || bm[i].frame.bitmap.as_ref().expect("bm").rx_span == RX_SPAN_NONE {
                continue;
            }
            let want = o.first_received_after(largest(&bm[i - 1].frame));
            assert_eq!(
                span_first(&bm[i].frame),
                want,
                "alarm dışı ACK ({} ms) bir ACK geriye uzanmalı",
                bm[i].at / MS
            );
            checked += 1;
        }
        assert!(checked >= 3, "yeterli alarm dışı ACK yok ({checked})");
        assert!(alarm_idx + 1 < bm.len(), "alarm ACK'inden sonra ACK yok");
    }
}

/// (d) Yükseliş alarmı (daralma alarmından sonra kurulur): hat 45 s'de 12 500'e döner; yükseliş
/// alarmı verilir, ACK'inin aralığı bugünkü kural (bir ACK geriye) ve `pair_rate`'i toparlanan
/// hattı gösterir (referansın %25 üstü). Bu değişiklik yükseliş ACK'ine dokunmaz.
#[test]
fn d_rise_alarm_after_a_drop_is_unchanged() {
    let recover_at = 45 * SEC;
    let o = run(
        &[(0, OLD_GAP), (DROP_AT, NEW_GAP), (recover_at, OLD_GAP)],
        70 * SEC,
        |_| false,
        None,
    );
    let bm = o.bitmap();
    let i = bm
        .iter()
        .position(|e| e.rise_alarms >= 1)
        .expect("yükseliş alarmı yok");
    let rise = bm[i];
    println!(
        "yükseliş ACK'i {} ms, pair_rate {}, aralık [{}, {}]",
        rise.at / MS,
        rise.frame.bitmap.as_ref().expect("bm").pair_rate,
        span_first(&rise.frame),
        largest(&rise.frame)
    );
    assert!(rise.at >= recover_at, "yükseliş alarmı toparlanmadan önce");
    assert!(rise.at < recover_at + 3 * SEC, "yükseliş alarmı geç");
    assert_eq!(
        span_first(&rise.frame),
        o.first_received_after(largest(&bm[i - 1].frame)),
        "yükseliş ACK'i bir ACK geriye uzanır"
    );
    let pair = rise.frame.bitmap.as_ref().expect("bm").pair_rate;
    assert!(
        u64::from(pair) * 100 > NEW_LINE * 125 && pair != PAIR_RATE_NONE,
        "yükseliş ACK'inin pair_rate'i {pair}"
    );
    assert_eq!(
        bm.last().map(|e| e.rise_alarms),
        Some(1),
        "tek yükseliş alarmı"
    );
}

/// FNV-1a 64: çıkan çerçevelerin (an, bayt) dizisinin özeti.
fn digest(frames: &[(u64, Vec<u8>)]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (at, bytes) in frames {
        for b in at.to_be_bytes().iter().chain(bytes.iter()) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// (e) Bitmap kapalı (V2 0x14) ve profil kapalı (bütçesiz) alıcıda aynı düşüş akışı bayt-aynı
/// çerçeveler üretir: özetler değişiklikten önceki alıcıyla (`9e853984`) aynı koşudan alındı.
/// Bu yollarda daralma alarmı yoktur.
#[test]
fn e_v2_and_profile_off_frames_are_byte_identical() {
    let cases: [(&str, Option<u64>, u64); 2] = [
        ("V2 0x14 (bütçeli, bitmap kapalı)", Some(62), V2_DIGEST),
        ("profil kapalı (bütçesiz)", None, PROFILE_OFF_DIGEST),
    ];
    let mut got = Vec::new();
    for (name, ret, want) in cases {
        let mut r = Receiver::with_cc_port();
        r.set_return_bandwidth(ret);
        let mut frames = Vec::new();
        let mut t = SEC;
        let mut seq = 0u64;
        while t < 40 * SEC {
            if !every_fifth(seq) {
                r.set_next_datagram_bytes(BODY);
                let out = r.on_datagram(
                    RxDatagram {
                        seq,
                        unit_id: seq,
                        kind: RxKind::Data,
                    },
                    t,
                );
                frames.extend(out.ack.map(|f| (t, f.encode())));
                frames.extend(r.on_ack_timer(t).map(|f| (t, f.encode())));
            }
            seq += 1;
            t += if t < DROP_AT { OLD_GAP } else { NEW_GAP };
        }
        assert!(!frames.is_empty(), "{name}: ACK yok");
        assert_eq!(r.diag_drop_alarms(), 0, "{name}: alarm");
        let d = digest(&frames);
        println!("{name}: {} çerçeve, özet {d:#018x}", frames.len());
        got.push((name, d, want));
    }
    for (name, d, want) in got {
        assert_eq!(d, want, "{name}: çerçeveler değişti");
    }
}

/// (e) özetleri: `9e853984` alıcısıyla aynı test gövdesinden.
const V2_DIGEST: u64 = 0x32fb_5f40_2b6a_3629;
const PROFILE_OFF_DIGEST: u64 = 0xf1e2_18cf_de7c_e931;
