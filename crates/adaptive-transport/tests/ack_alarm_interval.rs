//! [ALARM ARALIĞI · #175 7084078903 / 7084084106] Daralma alarmının ACK'i ölçümü yalnız
//! daralmadan sonra gelenlerden taşır: ölçüm aralığı `[span_first, largest]` alarm penceresindeki
//! daralma başlangıcından (`RateDropDetector::onset`) başlar, `pair_rate` yalnız alarmdan sonraki
//! çiftlerdendir. Alarm dışı ACK'ler, yükseliş alarmının ACK'i, V2 (0x14) ve profil kapalı yol
//! bugünkü gibi. Yükseliş alarmının referansı böylece düşüş sonrası çift değeridir (`arm_rise`
//! belgesi; ek koşul #175 7084238356): (f) gerçek toparlanmada tetiklenir, (g) dürüst 6 250 hatta
//! tetiklenmez. Değişiklik öncesi alıcıda referans düşüş öncesi çiftti (12 500), dolu dönüşte
//! yükseliş alarmı hiç tetiklenmiyordu ((f)'nin karşı-olgusu).
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
//!
//! [TUR 3 · #175 7086827627] (h) göndericiyle birlikte: alıcının bütün ACK'leri göndericinin hat
//! korumasına verilir, zaman çizelgesi basılır ve adayın (`f952a1a1`) çizelgesiyle her değişim
//! anında kıyaslanır. Daralma başlangıcından sonra en az iki datagram serileşmeden alarm ACK'i
//! çıkmaz (`arrival_rate::ALARM_SPAN_MIN_SEQS`); tek datagramlık aralık göndericide yapısal olarak
//! örnek vermiyordu (tur 2, her beşte bir kayıp: [364, 365], g 80 ms, eşik ≈ 98 ms). Planlı ACK'in
//! zamanı ve kuralı `ack_alarm_planned.rs`'te.

use std::collections::BTreeMap;

use adaptive_transport::ack::{AckFrame, ACK_DELAY_UNIT_US, PAIR_RATE_NONE, RX_SPAN_NONE};
use adaptive_transport::arrival_rate::RateDropDetector;
use adaptive_transport::line_guard::{ForwardInterval, LineGuard};
use adaptive_transport::loss::E2A_ACK_DELAY_SATURATED;
use adaptive_transport::pacer::LineModel;
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
    /// Daralma alarmının verildiği an (ilk tetiklenme): alıcının kendi dedektörüyle aynı girdiyi
    /// alan bağımsız bir [`RateDropDetector`] (ilk ACK'ten sonraki her varış, her çıkan ACK'te
    /// `on_ack`). İlk tetiklenmeye dek iki alıcının ACK akışı aynıdır; an iki alıcıda aynıdır.
    alarm_ns: Option<u64>,
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
    run_declared(gaps, until, lost, busy_from, None)
}

/// [`run`], ileri yönün beyan hattıyla (`Receiver::set_pair_squeeze_line`; ürünün sunucu yuvası
/// profil hattını verir, `None` = kural kapalı).
fn run_declared(
    gaps: &[(u64, u64)],
    until: u64,
    lost: impl Fn(u64) -> bool,
    busy_from: Option<u64>,
    squeeze_line: Option<u64>,
) -> Run {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(squeeze_line);
    let mut acks = Vec::new();
    let mut arrival = BTreeMap::new();
    let mut det = RateDropDetector::default();
    let mut alarm_ns = None;
    let mut t = SEC;
    let mut seq = 0u64;
    while t < until {
        let busy = busy_from.is_some_and(|b| t >= b);
        if !lost(seq) {
            arrival.insert(seq, t);
            if !acks.is_empty() && det.on_arrival(t, BODY) && alarm_ns.is_none() {
                alarm_ns = Some(t);
            }
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
                det.on_ack(at);
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
    Run {
        acks,
        arrival,
        alarm_ns,
    }
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

/// Kayıp deseni: her yedide biri hatta serileşip düşer.
fn every_seventh(seq: u64) -> bool {
    seq % 7 == 3
}

/// Kayıp deseni: seq → hatta serileşip düştü mü.
type Loss = fn(u64) -> bool;

/// Yükseliş testlerinin kolları: beyan hat (kural kapalı / profil 12 500) × kayıp deseni. Her
/// üçte bir kayıp kolu yok: o desende daralma alarmı hiç verilmez (pencerenin en uzun aralığı
/// ortalamanın 2,5 katını aşar), yükseliş alarmı kurulmaz, test boş geçerdi.
fn rise_arms() -> Vec<(String, Option<u64>, Loss)> {
    let losses: [(&str, Loss); 3] = [
        ("kayıpsız", |_| false),
        ("her beşte bir kayıp", every_fifth),
        ("her yedide bir kayıp", every_seventh),
    ];
    let mut arms = Vec::new();
    for squeeze in [None, Some(PROFILE_LINE)] {
        for (name, lost) in losses {
            arms.push((format!("{name}, beyan hat {squeeze:?}"), squeeze, lost));
        }
    }
    arms
}

/// Profilin ileri hattı (B/s): düşüş öncesi hat.
const PROFILE_LINE: u64 = BODY * SEC / OLD_GAP;

/// (f) [ALARM ARALIĞI EK KOŞUL · #175 7084238356, yön 1] Gerçek 6 250 → 12 500 toparlanması,
/// dolu dönüş (alarm ACK'i yol verişten sonra, alarmdan sonraki çiftlerle çıkar): yükseliş
/// alarmının referansı daralmadan sonraki ilk bildirilen değerdir (`arm_rise` belgesi) ve yeni
/// hattı gösterir (≤ 6 875); toparlanmada yükseliş alarmı tetiklenir, ACK'inin `pair_rate`'i
/// referansın %25 üstünde, tek alarm. Zaman sınırı (d) ile aynı (toparlanmadan sonra < 3 s).
/// Toparlanma alarm ACK'inden sonraki bitmap ACK'ine dek beklenir (33 s, 45 s): daha önce
/// toparlanırsa referans toparlanmış değerle kurulur, alarm yoktur (bilgi sınırı; iki alıcıda
/// aynı). Karşı-olgu (alarmda çift değeri sıfırlanmazsa, değişiklik öncesi alıcı): alarm ACK'i
/// düşüş öncesi çifti (12 500) taşır, referans eski hat olur, yükseliş alarmı hiç tetiklenmez.
#[test]
fn f_rise_alarm_fires_on_a_real_recovery_with_the_post_drop_reference() {
    // Önce bütün kollar koşulup yazılır (karşı-olguda her kolun sayısı görünsün), sonra sınanır.
    let mut runs = Vec::new();
    for (name, squeeze, lost) in rise_arms() {
        for recover_at in [33 * SEC, 45 * SEC] {
            let o = run_declared(
                &[(0, OLD_GAP), (DROP_AT, NEW_GAP), (recover_at, OLD_GAP)],
                recover_at + 15 * SEC,
                lost,
                Some(DROP_AT - SEC),
                squeeze,
            );
            let name = format!("{name}, toparlanma {} ms", recover_at / MS);
            {
                let bm = o.bitmap();
                let alarm = bm.iter().find(|e| e.drop_alarms >= 1);
                let reference = bm
                    .iter()
                    .skip_while(|e| e.drop_alarms == 0)
                    .find(|e| e.frame.bitmap.as_ref().expect("bm").pair_rate != PAIR_RATE_NONE);
                let rise = bm.iter().find(|e| e.rise_alarms >= 1);
                println!(
                    "{name}: daralma ACK'i {:?} ms, referans {:?} ({:?} ms), yükseliş ACK'i \
                     {:?} ms pair {:?}, yükseliş {}",
                    alarm.map(|e| e.at / MS),
                    reference.map(|e| e.frame.bitmap.as_ref().expect("bm").pair_rate),
                    reference.map(|e| e.at / MS),
                    rise.map(|e| e.at / MS),
                    rise.map(|e| e.frame.bitmap.as_ref().expect("bm").pair_rate),
                    bm.last().map_or(0, |e| e.rise_alarms),
                );
            }
            runs.push((name, recover_at, o));
        }
    }
    for (name, recover_at, o) in &runs {
        let bm = o.bitmap();
        assert!(
            bm.iter().any(|e| e.drop_alarms >= 1),
            "{name}: daralma alarmı yok"
        );
        let reference = bm
            .iter()
            .skip_while(|e| e.drop_alarms == 0)
            .find(|e| e.frame.bitmap.as_ref().expect("bm").pair_rate != PAIR_RATE_NONE)
            .expect("daralmadan sonra bildirilen çift değeri yok");
        let ref_pair = u64::from(reference.frame.bitmap.as_ref().expect("bm").pair_rate);
        assert!(
            reference.at < *recover_at,
            "{name}: referans toparlanmadan sonra kuruldu"
        );
        assert!(
            ref_pair <= CEILING_MAX,
            "{name}: referans {ref_pair} düşüş öncesi (yeni hat {NEW_LINE})"
        );
        let rise = bm
            .iter()
            .find(|e| e.rise_alarms >= 1)
            .unwrap_or_else(|| panic!("{name}: yükseliş alarmı yok"));
        assert!(
            rise.at >= *recover_at && rise.at < recover_at + 3 * SEC,
            "{name}: yükseliş alarmı {} ms",
            rise.at / MS
        );
        let pair = rise.frame.bitmap.as_ref().expect("bm").pair_rate;
        assert!(
            pair != PAIR_RATE_NONE && u64::from(pair) * 100 > ref_pair * 125,
            "{name}: yükseliş ACK'inin pair_rate'i {pair}, referans {ref_pair}"
        );
        assert_eq!(
            bm.last().map(|e| e.rise_alarms),
            Some(1),
            "{name}: tek yükseliş alarmı"
        );
    }
}

/// (g) [ALARM ARALIĞI EK KOŞUL · #175 7084238356, yön 2] Dürüst 6 250 hatta (düşüş, toparlanma
/// yok, 60 s) yükseliş alarmı tetiklenmez: düşüş sonrası referans hattın kendisidir, aynı hattın
/// çiftleri onun %25 üstüne çıkmaz. Açık ve dolu dönüş, iki beyan hat, üç belirlenimli kayıp
/// deseni; her kolda daralma alarmı verilmiş olmalı (yükseliş alarmı kurulu, test boş değil).
#[test]
fn g_honest_post_drop_line_raises_no_rise_alarm() {
    for (name, squeeze, lost) in rise_arms() {
        for busy_from in [None, Some(DROP_AT - SEC)] {
            let o = run_declared(&drop_plan(), DROP_AT + 60 * SEC, lost, busy_from, squeeze);
            let last = o.acks.last().expect("ACK yok");
            println!(
                "{name}, dolu dönüş {:?}: daralma alarmı {}, yükseliş alarmı {}",
                busy_from.map(|b| b / MS),
                last.drop_alarms,
                last.rise_alarms
            );
            assert!(
                last.drop_alarms >= 1,
                "{name}: daralma alarmı yok (boş test)"
            );
            assert_eq!(
                last.rise_alarms, 0,
                "{name}, dolu dönüş {busy_from:?}: dürüst hatta yükseliş alarmı"
            );
        }
    }
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
/// çerçeveler üretir: özetler değişiklikten önceki alıcıyla (`9e853984`) aynı koşudan alındı;
/// güncel adayın (`c7c0a120`) alıcısı da aynı özetleri verir.
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

// ---------------------------------------------------------------------------------------------
// (h) Göndericiyle birlikte: alıcının bütün ACK'leri göndericinin hat korumasına; zaman çizelgesi.
// ---------------------------------------------------------------------------------------------

/// Datagram başına zarf (B; IPv4 + UDP), göndericinin hat modelinde.
const OVH: u64 = 28;
/// Göndericinin gövdesi: hatta serileşen datagram 1 000 B tel (gövde + zarf), düzeneğin hattı
/// (80 ms'de 1 000 B = 12 500 B/s, düşüşten sonra 6 250) tel baytında aynı sayılar.
const SENDER_UDP: u64 = BODY - OVH;
/// İleri ve dönüş yönü yayılımı (ns).
const ONE_WAY: u64 = 300 * MS;
/// Profilin dönüş hattı (B/s, tel).
const RETURN_BPS: u64 = 62;
/// Zaman çizelgesinin süresi: düşüşten sonra 60 s ((g) ile aynı).
const TIMELINE_UNTIL: u64 = DROP_AT + 60 * SEC;

/// Göndericinin iletim anı: profil hattında (kendi hızı 80 ms'de bir datagram); düşüşten önce varış
/// = iletim + serileşme (80 ms) + yayılım. Gönderici açık döngüdür: tavan iletimleri yavaşlatmaz
/// (`run`'ın varışları sabit; iki alıcıya aynı).
fn send_ns(seq: u64) -> u64 {
    SEC + seq * OLD_GAP - ONE_WAY - OLD_GAP
}

/// Göndericinin bir ACK'i işleyişi (zaman çizelgesinin satırı).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    /// ACK sıra no (alıcıdan çıkış sırası, 1'den).
    n: usize,
    /// Alıcıdan çıkış anı (ns).
    at: u64,
    /// Göndericide işlendiği an (ns; dönüş hattı modeli [`replay`]).
    proc: u64,
    /// Daralma alarmının ACK'i (alıcının alarm sayacını ilk artıran ACK).
    alarm: bool,
    /// Göndericinin kurduğu ölçüm aralığı `(first, last]`; `None` = ölçüm yok.
    interval: Option<(u64, u64)>,
    /// Aralığın kuyruk artışı `g` (ns), aralık ölçüldüyse.
    g_ns: Option<i64>,
    /// Aşağı yönlü örnek: aralıkta gönderilen tel baytı / alıcıdaki varış süresi (B/s; artış
    /// örneğinin formülü, `line_guard::interval_sample`); örnek yoksa `None`.
    sample: Option<u64>,
    /// ACK'ten önceki ve sonraki gönderim tavanı (B/s, tel).
    before: u64,
    after: u64,
}

/// Alıcının bütün ACK'leri göndericinin çalışma zamanı ve motor sırasıyla hat korumasına verilir
/// (`client-core` runtime 0x15 dalı + `Sender::cc_line_guard_on_ack`): ölçüm aralığı
/// `[span_first, largest]` içindeki alınan seq'ler, `rx_span` alanı × 256 µs, `received_wire`,
/// `note_pair_lower_bound` → `on_ack_measured` → `on_pair_rate` → `settle_start_budget`.
///
/// Dönüş hattı modeli: ACK alıcıdan çıktığında dönüş hattında önündeki serileşmeyi (dolu dönüşte
/// her denemede [`AHEAD`]) ve önceki ACK'lerin serileşmesini bekler, kendi tel boyunun 62 B/s'de
/// serileşmesinden sonra 300 ms yayılımla göndericide işlenir. Gönderici o ana dek iletimlerini
/// kaydetmiştir. Model iki alıcıya aynı uygulanır. `alarm_first` verilirse alarm ACK'inin aralığı
/// o seq'ten kurulur (duyarlılık kolu; `rx_span` varışlardan, aynı birimle).
fn replay(o: &Run, alarm_first: Option<u64>, busy_from: Option<u64>) -> Vec<Step> {
    let mut g = LineGuard::new(
        LineModel {
            rate_bps: PROFILE_LINE,
            datagram_overhead: OVH,
        },
        LineModel {
            rate_bps: RETURN_BPS,
            datagram_overhead: OVH,
        },
        PROFILE_LINE,
    );
    let alarm_at = o
        .acks
        .iter()
        .position(|e| e.frame.bitmap.is_some() && e.drop_alarms >= 1);
    let unit_ns = ACK_DELAY_UNIT_US * 1000;
    let mut next_send = 0u64;
    let mut line_free = 0u64;
    let mut steps = Vec::new();
    for (i, e) in o.acks.iter().enumerate() {
        let f = &e.frame;
        let wire = f.encode().len() as u64;
        let ahead = if busy_from.is_some_and(|b| e.at >= b) {
            AHEAD
        } else {
            0
        };
        line_free = line_free.max(e.at + ahead) + (wire + OVH) * SEC / RETURN_BPS;
        let now = line_free + ONE_WAY;
        while send_ns(next_send) <= now {
            g.on_sent(next_send, send_ns(next_send), SENDER_UDP);
            next_send += 1;
        }
        let hi = largest(f);
        let acked: Vec<u64> = f
            .acked_full(hi + 1)
            .into_iter()
            .filter(|&s| s < next_send)
            .collect();
        g.note_ack_wire(wire);
        if g.start_budget_active() {
            let _ = g.on_first_window(&acked, f.arrival_rate);
        }
        let pair = f
            .bitmap
            .as_ref()
            .map(|b| b.pair_rate)
            .filter(|&p| p != PAIR_RATE_NONE);
        let is_alarm = Some(i) == alarm_at;
        let interval = f.bitmap.as_ref().and_then(|bm| {
            let from = match alarm_first.filter(|_| is_alarm) {
                Some(s) => s,
                None => span_first(f),
            };
            let received: Vec<u64> = acked
                .iter()
                .copied()
                .filter(|&s| s >= from && s <= hi)
                .collect();
            let first = *received.first()?;
            let span_ns = if alarm_first.is_some() && is_alarm {
                Some((o.arrival[&hi] - o.arrival[&first]) / unit_ns * unit_ns)
            } else {
                (bm.rx_span != RX_SPAN_NONE).then(|| u64::from(bm.rx_span) * unit_ns)
            };
            let valid = first < hi && received.contains(&hi) && hi < next_send;
            let rx_wire = g.received_wire(first, hi, &received);
            span_ns.filter(|_| valid).map(|rx_span_ns| ForwardInterval {
                first_seq: first,
                last_seq: hi,
                rx_span_ns,
                rx_wire,
            })
        });
        let delay_ns = u64::try_from(f.ack_delay_duration().as_nanos()).unwrap_or(u64::MAX);
        let saturated = f.ack_delay_duration() >= E2A_ACK_DELAY_SATURATED;
        let largest_sent = acked.iter().copied().max().unwrap_or(hi);
        let before = g.pace_bps();
        let (measured_before, samples_before, _) = g.interval_diag();
        g.note_pair_lower_bound(pair.map(u64::from));
        let _ = g.on_ack_measured(now, delay_ns, saturated, largest_sent, interval);
        if let Some(p) = pair {
            g.on_pair_rate(u64::from(p));
        }
        g.settle_start_budget();
        let (measured, samples, g_ns) = g.interval_diag();
        steps.push(Step {
            n: i + 1,
            at: e.at,
            proc: now,
            alarm: is_alarm,
            interval: interval.map(|iv| (iv.first_seq, iv.last_seq)),
            g_ns: g_ns.filter(|_| measured > measured_before),
            sample: interval
                .filter(|_| samples > samples_before)
                .map(|iv| (iv.last_seq - iv.first_seq) * BODY * SEC / iv.rx_span_ns.max(1)),
            before,
            after: g.pace_bps(),
        });
    }
    steps
}

/// Tavanın `t` anındaki değeri: `t`'ye dek işlenmiş son değişim (değişim listesi `(an, tavan)`).
fn pace_at(changes: &[(u64, u64)], t: u64) -> u64 {
    changes
        .iter()
        .take_while(|&&(at, _)| at <= t)
        .last()
        .map_or(PROFILE_LINE, |&(_, p)| p)
}

/// Zaman çizelgesinin tavan değişimleri `(göndericide işlendiği an, yeni tavan)`.
fn changes(steps: &[Step]) -> Vec<(u64, u64)> {
    steps
        .iter()
        .filter(|s| s.after != s.before)
        .map(|s| (s.proc, s.after))
        .collect()
}

/// Tavanın doğru seviyeye (≤ [`CEILING_MAX`]) oturduğu ilk değişim: ondan sonra koşu sonuna dek
/// tavan ölçütün üstüne çıkmaz.
fn settle(ch: &[(u64, u64)]) -> Option<(u64, u64)> {
    let i = ch
        .iter()
        .rposition(|&(_, p)| p > CEILING_MAX)
        .map_or(0, |i| i + 1);
    ch.get(i).copied()
}

/// (h) kolları: kayıp deseni × dönüş hattı (açık; dolu: düşüşten 1 s önce başlar, (b) ile aynı).
fn timeline_arms() -> Vec<(String, Loss, Option<u64>)> {
    let losses: [(&str, Loss); 3] = [
        ("kayıpsız", |_| false),
        ("her beşte bir kayıp", every_fifth),
        ("her yedide bir kayıp", every_seventh),
    ];
    let mut arms = Vec::new();
    for (ret, busy) in [("açık dönüş", None), ("dolu dönüş", Some(DROP_AT - SEC))] {
        for (name, lost) in losses {
            arms.push((format!("{name}, {ret}"), lost, busy));
        }
    }
    arms
}

fn secs(ns: u64) -> String {
    format!("{:.3}", ns as f64 / MS as f64 / 1000.0)
}

fn step_line(s: &Step) -> String {
    format!(
        "ACK #{} (çıkış {} s, göndericide {} s), aralık {:?}, g {:?} ms, örnek {:?}, tavan {} → {}",
        s.n,
        secs(s.at),
        secs(s.proc),
        s.interval,
        s.g_ns.map(|g| g as f64 / MS as f64),
        s.sample,
        s.before,
        s.after
    )
}

/// Alarm ACK'inden önceki bitmap ACK'inin en büyüğünden sonraki ilk alınan seq (adayın alarm ACK'i
/// bu kuralla çıkar).
fn one_ack_back_first(o: &Run) -> u64 {
    let (_, prev) = o.alarm_ack();
    o.first_received_after(prev)
}

/// Aday alıcısının (`f952a1a1`: alarm ACK'i alarm anında, aralık bir ACK geriye) aynı düzenekteki
/// tavan değişimleri `(göndericide işlendiği an, yeni tavan)`, [`timeline_arms`] sırasıyla. Aynı test
/// gövdesi adayın `receiver.rs` ve `arrival_rate.rs`'i ile koşuldu (karşı-olgu kolu; PR gövdesi).
///
/// Dolu dönüş kollarında dönüş hattı düşüşten 1 s önceden koşu sonuna dek her denemede doludur;
/// alarm dışı ACK yol vermez (alarm tek yol verir kuralı), alarm ACK'inden sonra ACK çıkmaz. Adayın
/// o kollarda oturmaması bu düzeneğin sınırıdır, iki alıcıda aynı.
const CANDIDATE_CHANGES: [(&str, &[(u64, u64)]); 6] = [
    (
        "kayıpsız, açık dönüş",
        &[(31_418_064_516, 10_578), (36_442_580_645, 6_250)],
    ),
    (
        "her beşte bir kayıp, açık dönüş",
        &[(31_418_064_516, 10_416), (36_266_451_612, 6_250)],
    ),
    (
        "her yedide bir kayıp, açık dönüş",
        &[(36_106_451_612, 6_250)],
    ),
    ("kayıpsız, dolu dönüş", &[(33_699_354_838, 11_352)]),
    (
        "her beşte bir kayıp, dolu dönüş",
        &[(33_699_354_838, 11_340)],
    ),
    (
        "her yedide bir kayıp, dolu dönüş",
        &[(33_523_225_806, 11_447)],
    ),
];

/// (h) [ALARM ARALIĞI · TUR 3 · #175 7086827627] Göndericiyle birlikte, zaman çizelgesi. Alıcının
/// bütün ACK'leri göndericinin hat korumasına verilir ([`replay`]); kollar kayıpsız, her beşte bir,
/// her yedide bir kayıp × açık/dolu dönüş, beyan hat profil (ürünün sunucu yuvası; kural kapalı kol
/// aynı çizelgeyi verir). Her kolda alarm anı, alarm ACK'i ve alarmdan farkı (ölçülen), tavanı ilk
/// düşüren ACK, doğru seviyeye (≤ 6 875) oturma ve aradaki her tavan değişimi basılır.
///
/// Sınırlar ürün ölçütünden: kayıpsız açık dönüşte alarm anındaki ACK yeterli kanıttır (atlanmaz) ve
/// tavanı ≤ 6 875'e indirir. Açık dönüşte alarm ACK'i göndericide örnek verir, tavan ≤ 6 875. Adayın
/// çizelgesiyle ([`CANDIDATE_CHANGES`]) her değişim anında kıyas; oturma adaydan geç değil.
/// Duyarlılık: alarm ACK'inin aralığı bir ACK geriye kurulsaydı (adayın kuralı) tavan ölçütün
/// üstünde kalır.
#[test]
fn h_alarm_ack_timeline_at_the_sender() {
    struct Row {
        name: String,
        busy: bool,
        alarm_ns: u64,
        steps: Vec<Step>,
        back: Vec<Step>,
    }
    let mut rows = Vec::new();
    for (name, lost, busy) in timeline_arms() {
        let o = run_declared(&drop_plan(), TIMELINE_UNTIL, lost, busy, Some(PROFILE_LINE));
        let steps = replay(&o, None, busy);
        let off = replay(
            &run_declared(&drop_plan(), TIMELINE_UNTIL, lost, busy, None),
            None,
            busy,
        );
        assert_eq!(
            steps, off,
            "{name}: beyan hat kapalı kol başka çizelge verdi"
        );
        let back = replay(&o, Some(one_ack_back_first(&o)), busy);
        let alarm_ns = o.alarm_ns.expect("daralma alarmı yok");
        let ch = changes(&steps);
        println!("== ZAMAN ÇİZELGESİ {name}: alarm {} s", secs(alarm_ns));
        if let Some(a) = steps.iter().find(|s| s.alarm) {
            println!(
                "   alarm ACK'i: {}; alarmdan {} ms",
                step_line(a),
                (a.at - alarm_ns) / MS
            );
        }
        let first = steps.iter().find(|s| s.after < s.before);
        let st = settle(&ch);
        if let Some(f) = first {
            println!(
                "   ilk düşüş: {}; alarmdan {} ms",
                step_line(f),
                (f.proc - alarm_ns) / MS
            );
            for s in steps
                .iter()
                .filter(|s| s.n > f.n && s.after != s.before && st.is_none_or(|(t, _)| s.proc <= t))
            {
                println!("   değişim: {}", step_line(s));
            }
        }
        match st {
            Some((t, p)) => println!(
                "   oturma (≤ {CEILING_MAX}): göndericide {} s, tavan {p}; alarmdan {} ms",
                secs(t),
                (t - alarm_ns) / MS
            ),
            None => println!(
                "   oturmadı; son tavan {}",
                steps.last().map_or(PROFILE_LINE, |s| s.after)
            ),
        }
        println!(
            "   bir ACK geriye (duyarlılık): {}",
            back.iter()
                .find(|s| s.alarm)
                .map_or_else(String::new, step_line)
        );
        println!("ADAY-VERİ (\"{name}\", &{ch:?}),");
        rows.push(Row {
            name,
            busy: busy.is_some(),
            alarm_ns,
            steps,
            back,
        });
    }
    // Tablo (PR gövdesi): kol başına tek satır.
    println!(
        "| kol | alarm (s) | alarm ACK'i: no, çıkış (s), alarmdan (ms) | tavanı ilk düşüren ACK: no, \
         göndericide (s), aralık, g (ms), örnek, tavan | alarm → ilk düşüş (ms) | ≤ {CEILING_MAX} \
         oturma: göndericide (s), alarmdan (ms) | değişimler (no: tavan) |"
    );
    println!("|---|---|---|---|---|---|---|");
    for r in &rows {
        let a = r.steps.iter().find(|s| s.alarm).expect("alarm ACK'i");
        let f = r.steps.iter().find(|s| s.after < s.before);
        let ch = changes(&r.steps);
        let first = f.map_or_else(
            || "yok".to_string(),
            |f| {
                format!(
                    "#{}, {}, {:?}, {:?}, {:?}, {} → {}",
                    f.n,
                    secs(f.proc),
                    f.interval,
                    f.g_ns.map(|g| g as f64 / MS as f64),
                    f.sample,
                    f.before,
                    f.after
                )
            },
        );
        let settle_txt = settle(&ch).map_or_else(
            || {
                format!(
                    "oturmadı (son {})",
                    r.steps.last().map_or(PROFILE_LINE, |s| s.after)
                )
            },
            |(t, _)| format!("{}, {}", secs(t), (t - r.alarm_ns) / MS),
        );
        let list: Vec<String> = r
            .steps
            .iter()
            .filter(|s| s.after != s.before)
            .map(|s| format!("#{}: {}", s.n, s.after))
            .collect();
        println!(
            "| {} | {} | #{}, {}, {} | {first} | {} | {settle_txt} | {} |",
            r.name,
            secs(r.alarm_ns),
            a.n,
            secs(a.at),
            (a.at - r.alarm_ns) / MS,
            f.map_or_else(
                || "—".to_string(),
                |f| ((f.proc - r.alarm_ns) / MS).to_string()
            ),
            list.join("; ")
        );
    }
    let mut excess = Vec::new();
    for r in &rows {
        let name = &r.name;
        let ch = changes(&r.steps);
        let alarm = r
            .steps
            .iter()
            .find(|s| s.alarm)
            .unwrap_or_else(|| panic!("{name}: alarm ACK'i yok"));
        assert!(alarm.at >= r.alarm_ns, "{name}: alarm ACK'i alarmdan önce");
        assert!(
            r.steps
                .iter()
                .take_while(|s| !s.alarm)
                .all(|s| s.after == PROFILE_LINE),
            "{name}: alarmdan önce tavan değişti"
        );
        if !r.busy {
            assert!(alarm.sample.is_some(), "{name}: alarm ACK'i örnek vermedi");
            assert!(
                alarm.after <= CEILING_MAX,
                "{name}: alarm ACK'inde tavan {}",
                alarm.after
            );
        }
        if name.starts_with("kayıpsız, açık") {
            assert_eq!(alarm.at, r.alarm_ns, "{name}: alarm anındaki ACK atlandı");
        }
        let back = r.back.iter().find(|s| s.alarm).expect("duyarlılık kolu");
        assert!(
            back.after > CEILING_MAX,
            "{name}: duyarlılık kolu tavanı {}",
            back.after
        );
        let Some(&(_, cand)) = CANDIDATE_CHANGES.iter().find(|(arm, _)| arm == name) else {
            excess.push(format!("{name}: adayın çizelgesi yok"));
            continue;
        };
        // Her değişim anında kıyas (tavan basamaklıdır: değişim anları arasında sabit).
        let mut instants: Vec<u64> = ch.iter().chain(cand.iter()).map(|&(t, _)| t).collect();
        instants.sort_unstable();
        instants.dedup();
        let (ours_first, cand_first) = (ch[0].0, cand[0].0);
        let mut above_ns = 0u64;
        for (k, &t) in instants.iter().enumerate() {
            let (b, c) = (pace_at(&ch, t), pace_at(cand, t));
            if b <= c {
                continue;
            }
            let until = instants.get(k + 1).copied().unwrap_or(TIMELINE_UNTIL);
            above_ns += until - t;
            println!(
                "{name}: {} – {} s arası dal {b} > aday {c} ({} ms)",
                secs(t),
                secs(until),
                (until - t) / MS
            );
            // Sahibin kuralı (2026-11-23): alarm ACK'i yeterli kanıtı bekler. Dal adayın üstünde
            // yalnız adayın ilk düşüşünden dalın ilk düşüşüne dek olabilir (ilk düşüşün gecikmesi).
            if !(cand_first <= t && t < ours_first) {
                excess.push(format!("{name}: {} s'de dal {b} > aday {c}", secs(t)));
            }
        }
        let (ours, theirs) = (settle(&ch), settle(cand));
        let ours = ours.unwrap_or_else(|| panic!("{name}: dal oturmadı"));
        println!(
            "{name}: ilk düşüş dal {} s, aday {} s (gecikme {} ms); dal adayın üstünde {} ms; \
             oturma dal {} s, aday {}",
            secs(ours_first),
            secs(cand_first),
            ours_first.saturating_sub(cand_first) / MS,
            above_ns / MS,
            secs(ours.0),
            theirs.map_or_else(|| "oturmadı".to_string(), |(t, _)| format!("{} s", secs(t)))
        );
        assert!(
            theirs.is_none_or(|(tc, _)| ours.0 <= tc),
            "{name}: dal adaydan geç oturdu"
        );
        // İlk düşüş gecikmişse karşılığında oturma adaydan erken (ya da aday oturmuyor).
        if ours_first > cand_first {
            assert!(
                theirs.is_none_or(|(tc, _)| ours.0 < tc),
                "{name}: ilk düşüş gecikti, oturma erkenleşmedi"
            );
        }
    }
    assert!(
        excess.is_empty(),
        "dalın tavanı ilk düşüş gecikmesi dışında adayınkinden yüksek: {excess:#?}"
    );
}
