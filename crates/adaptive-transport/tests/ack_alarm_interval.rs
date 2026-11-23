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
//! [TUR 3–4 · #175 7086827627, 7087793188] (h) göndericiyle birlikte: alıcının bütün ACK'leri
//! göndericinin hat korumasına verilir, zaman çizelgesi basılır ve adayın (`f952a1a1`) çizelgesiyle
//! (`ack_alarm_candidate_timeline.txt`) her değişim anında kıyaslanır. Belirlenimli kollar ve
//! doğrulayıcının tohumlu %20 rastgele yoklaması (oturum başından / düşüşten, 16 tohum × dört
//! dönüş). Alarm ACK'i, göndericinin örnek koşulunun alıcıdaki aynası sağlanana dek bekler
//! (`arrival_rate::AlarmEvidence`; sıra farkı ya da datagram sayısı kanıt değildir). Karşı örnekler:
//! (x) darboğazdan önce düşen seq, (y) küçük datagram; (z) sahte alarmda planlı ACK adayın
//! çerçevesidir. Planlı ACK'in zamanı `ack_alarm_planned.rs`'te. (k) düşüş sonrası hızlanma.
//!
//! Düzenek: açık döngü gönderici (profil hattında pace eder, tavan iletimi yavaşlatmaz) + tek FIFO
//! darboğaz (`fifo`), alıcı varış sırasıyla ve aralarda 1 ms'de bir zamanlayıcıyla (`drive`).
//! Gerçek tel hızı bu düzenekte ölçülmez; tablolar göndericinin tavanını (hat korumasının pacer'a
//! verdiği hız) gösterir.

use std::collections::BTreeMap;

use adaptive_transport::ack::{AckFrame, ACK_DELAY_UNIT_US, PAIR_RATE_NONE, RX_SPAN_NONE};
use adaptive_transport::arrival_rate::RateDropDetector;
use adaptive_transport::line_guard::{ForwardInterval, LineGuard};
use adaptive_transport::loss::E2A_ACK_DELAY_SATURATED;
use adaptive_transport::pacer::LineModel;
use adaptive_transport::receiver::{AckEmitObs, Receiver, RxDatagram, RxKind};

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
    /// [ACK-GÖZLEM] Alıcının bu ACK için kaydı ve o andaki ertelenme sayaçları
    /// (`[bütçe, yol verme, bilgisiz]`); ACK çıktığı anda okunur. Yalnız (o) okur.
    obs: AckEmitObs,
    deferred: [u64; 3],
}

/// [ACK-GÖZLEM] Alıcının son ACK kaydı ve ertelenme sayaçları (salt okuma).
fn observed(r: &Receiver) -> (AckEmitObs, [u64; 3]) {
    (
        r.diag_ack_obs(),
        [
            r.diag_ack_budget_deferred(),
            r.diag_ack_yield_deferred(),
            r.diag_ack_no_info_deferred(),
        ],
    )
}

struct Run {
    acks: Vec<Emitted>,
    /// Alınan seq → ilk varış anı.
    arrival: BTreeMap<u64, u64>,
    /// Daralma alarmlarının verildiği anlar: alıcının kendi dedektörüyle aynı girdiyi alan
    /// bağımsız bir [`RateDropDetector`] (ilk ACK'ten sonraki her varış, her çıkan ACK'te
    /// `on_ack`). İlk tetiklenmeye dek iki alıcının ACK akışı aynıdır; an iki alıcıda aynıdır.
    alarms: Vec<u64>,
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
    let mut alarms = Vec::new();
    let mut t = SEC;
    let mut seq = 0u64;
    while t < until {
        let busy = busy_from.is_some_and(|b| t >= b);
        if !lost(seq) {
            arrival.insert(seq, t);
            if !acks.is_empty() && det.on_arrival(t, BODY) {
                alarms.push(t);
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
            let mut seen: Vec<(AckEmitObs, [u64; 3])> = out.iter().map(|_| observed(&r)).collect();
            let timer_at = if busy { t + 30 * MS } else { t };
            r.set_return_line_ahead_ns(ahead);
            r.set_uplink_yield(yield_);
            out.extend(r.on_ack_timer(timer_at).map(|f| (timer_at, f)));
            if out.len() > seen.len() {
                seen.push(observed(&r));
            }
            for ((at, frame), (obs, deferred)) in out.into_iter().zip(seen) {
                det.on_ack(at);
                acks.push(Emitted {
                    at,
                    frame,
                    drop_alarms: r.diag_drop_alarms(),
                    rise_alarms: r.diag_rise_alarms(),
                    obs,
                    deferred,
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
        alarms,
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
/// (80 ms'de 1 000 B = 12 500 B/s, düşüşten sonra 6 250) tel baytında aynı sayılar. Alıcı da bu
/// gövdeyi görür (ürünün mühürlü gövde boyu, `set_next_datagram_bytes`).
const SENDER_UDP: u64 = BODY - OVH;
/// Küçük datagramın gövdesi (B): 146 B tel, PTT demeti boyu.
const SMALL_UDP: u64 = 146 - OVH;
/// İleri ve dönüş yönü yayılımı (ns).
const ONE_WAY: u64 = 300 * MS;
/// Profilin dönüş hattı (B/s, tel).
const RETURN_BPS: u64 = 62;
/// Zaman çizelgesinin süresi: düşüşten sonra 60 s ((g) ile aynı).
const TIMELINE_UNTIL: u64 = DROP_AT + 60 * SEC;
/// Zamanlayıcı çağrı aralığı (`ack_alarm_planned.rs` ile aynı): planlı ACK yeni datagram gelmese de
/// kendi zamanında çıkar, an 1 ms çözünürlükle ölçülür.
const POLL: u64 = MS;

/// Datagramın hattaki yazgısı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    Delivered,
    /// Darboğazda serileşti, sonra düştü (rastgele kayıp: hattı kaplar, varmaz).
    LostOnLine,
    /// Darboğazdan önce düştü (hattı kaplamaz).
    LostBefore,
}

/// Göndericinin bir iletimi ve alıcıdaki varışı.
#[derive(Debug, Clone, Copy)]
struct Tx {
    seq: u64,
    send_ns: u64,
    udp: u64,
    arrival: Option<u64>,
}

/// Açık döngü gönderici + tek FIFO darboğaz. Gönderici profil hattında pace eder (ardışık iki
/// iletim öncekinin tel süresi kadar arayla; tavan iletimleri yavaşlatmaz, iki alıcıya aynı). Hat
/// hızı serileşmenin başladığı ana göre `line`'dan ((bu andan itibaren, tel B/s)); varış =
/// serileşme bitişi + yayılım. İlk varış 1 s'de (`run` ile aynı akış: düşüşe dek 80 ms arayla).
/// `line` düşüşü `DROP_AT − ONE_WAY`'de başlatırsa varışlar `run`'ınkiyle aynıdır.
fn fifo(
    until: u64,
    udp_of: impl Fn(u64) -> u64,
    fate: impl Fn(u64) -> Fate,
    line: &[(u64, u64)],
) -> Vec<Tx> {
    let rate_at = |t: u64| {
        line.iter()
            .rev()
            .find(|&&(from, _)| t >= from)
            .map_or(PROFILE_LINE, |&(_, r)| r)
    };
    let mut out = Vec::new();
    let mut send = SEC - ONE_WAY - OLD_GAP;
    let mut free = 0u64;
    let mut seq = 0u64;
    while send + ONE_WAY < until {
        let udp = udp_of(seq);
        let wire = udp + OVH;
        let f = fate(seq);
        let arrival = if f == Fate::LostBefore {
            None
        } else {
            let start = free.max(send);
            free = start + wire * SEC / rate_at(start);
            (f == Fate::Delivered).then_some(free + ONE_WAY)
        };
        out.push(Tx {
            seq,
            send_ns: send,
            udp,
            arrival,
        });
        send += wire * SEC / PROFILE_LINE;
        seq += 1;
    }
    out
}

/// Dönüş hattı dolu penceresi `[başlangıç, bitiş)`; `None` = açık.
type Busy = Option<(u64, u64)>;

fn busy_at(b: Busy, t: u64) -> bool {
    b.is_some_and(|(from, until)| t >= from && t < until)
}

/// Hat düşüşü: `DROP_AT`'te varan ilk datagramdan sonra 6 250 (serileşme `DROP_AT − ONE_WAY`'den).
fn drop_line() -> Vec<(u64, u64)> {
    vec![(0, PROFILE_LINE), (DROP_AT - ONE_WAY, NEW_LINE)]
}

/// Düşüş ve `recover_at`'te 12 500'e dönüş.
fn recovery_line(recover_at: u64) -> Vec<(u64, u64)> {
    vec![
        (0, PROFILE_LINE),
        (DROP_AT - ONE_WAY, NEW_LINE),
        (recover_at - ONE_WAY, PROFILE_LINE),
    ]
}

/// Alıcıyı varış sırasıyla besler (`run_declared` ile aynı alıcı ve bağımsız dedektör), arada
/// zamanlayıcıyı [`POLL`] adımla çağırır. Dolu dönüş penceresinde her denemede önündeki serileşme
/// [`AHEAD`] ve yol verme işareti.
fn drive(
    txs: &[Tx],
    until: u64,
    busy: Busy,
    squeeze: Option<u64>,
    sized: impl Fn(u64) -> bool,
) -> Run {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(RETURN_BPS));
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(squeeze);
    let mut arrivals: Vec<(u64, u64, u64)> = txs
        .iter()
        .filter_map(|x| x.arrival.map(|a| (a, x.seq, x.udp)))
        .collect();
    arrivals.sort_unstable();
    let mut acks = Vec::new();
    let mut arrival = BTreeMap::new();
    let mut det = RateDropDetector::default();
    let mut alarms = Vec::new();
    let set_busy = |r: &mut Receiver, t: u64| {
        let b = busy_at(busy, t);
        r.set_return_line_ahead_ns(if b { AHEAD } else { 0 });
        r.set_uplink_yield(b);
    };
    let push = |acks: &mut Vec<Emitted>, r: &Receiver, det: &mut RateDropDetector, at, frame| {
        det.on_ack(at);
        let (obs, deferred) = observed(r);
        acks.push(Emitted {
            at,
            frame,
            drop_alarms: r.diag_drop_alarms(),
            rise_alarms: r.diag_rise_alarms(),
            obs,
            deferred,
        });
    };
    let mut next_poll = arrivals.first().map_or(until, |a| a.0);
    for &(at, seq, udp) in arrivals.iter().filter(|a| a.0 < until) {
        while next_poll < at {
            set_busy(&mut r, next_poll);
            if let Some(f) = r.on_ack_timer(next_poll) {
                push(&mut acks, &r, &mut det, next_poll, f);
            }
            next_poll += POLL;
        }
        arrival.insert(seq, at);
        let sz = sized(seq);
        if sz && !acks.is_empty() && det.on_arrival(at, udp) {
            alarms.push(at);
        }
        set_busy(&mut r, at);
        if sz {
            r.set_next_datagram_bytes(udp);
        }
        let out = r.on_datagram(
            RxDatagram {
                seq,
                unit_id: seq,
                kind: RxKind::Data,
            },
            at,
        );
        if let Some(f) = out.ack {
            push(&mut acks, &r, &mut det, at, f);
        }
        if let Some(f) = r.on_ack_timer(at) {
            push(&mut acks, &r, &mut det, at, f);
        }
        next_poll = at + POLL;
    }
    while next_poll < until {
        set_busy(&mut r, next_poll);
        if let Some(f) = r.on_ack_timer(next_poll) {
            push(&mut acks, &r, &mut det, next_poll, f);
        }
        next_poll += POLL;
    }
    Run {
        acks,
        arrival,
        alarms,
    }
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
    /// Kanıtlı daralma alarmının ACK'i (alıcının alarm sayacını ilk artıran ACK).
    alarm: bool,
    /// Bu ACK'te alıcının kanıtlı daralma alarmı sayısı (planlı ACK'te artmaz).
    drop_alarms: u64,
    /// Bu ACK'te alıcının yükseliş alarmı sayısı.
    rise_alarms: u64,
    /// ACK'in `pair_rate` alanı (`None` = ölçüm yok).
    pair: Option<u16>,
    /// Göndericinin kurduğu ölçüm aralığı `(first, last]`; `None` = ölçüm yok.
    interval: Option<(u64, u64)>,
    /// Aralığın kuyruk artışı `g` (ns), aralık ölçüldüyse.
    g_ns: Option<i64>,
    /// Aşağı yönlü örnek: aralıkta gönderilen tel baytı / alıcıdaki varış süresi (B/s; artış
    /// örneğinin formülü, `line_guard::interval_sample`); örnek yoksa `None`.
    sample: Option<u64>,
    /// ACK'ten önceki ve sonraki gönderim tavanı (B/s, tel; hat korumasının pacer'a verdiği hız).
    before: u64,
    after: u64,
}

/// Alıcının bütün ACK'leri göndericinin çalışma zamanı ve motor sırasıyla hat korumasına verilir
/// (`client-core` runtime 0x15 dalı + `Sender::cc_line_guard_on_ack`): ölçüm aralığı
/// `[span_first, largest]` içindeki alınan seq'ler, `rx_span` alanı × 256 µs, `received_wire`,
/// `note_pair_lower_bound` → `on_ack_measured` → `on_pair_rate` → `settle_start_budget`.
///
/// Dönüş hattı modeli: ACK alıcıdan çıktığında dönüş hattında önündeki serileşmeyi (dolu dönüş
/// penceresinde [`AHEAD`]) ve önceki ACK'lerin serileşmesini bekler, kendi tel boyunun 62 B/s'de
/// serileşmesinden sonra 300 ms yayılımla göndericide işlenir. Gönderici o ana dek iletimlerini
/// (`txs`) kaydetmiştir. Model iki alıcıya aynı uygulanır. `alarm_first` verilirse kanıtlı alarm
/// ACK'inin aralığı o seq'ten kurulur (duyarlılık kolu; `rx_span` varışlardan, aynı birimle).
fn replay(o: &Run, txs: &[Tx], alarm_first: Option<u64>, busy: Busy) -> Vec<Step> {
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
    let wire_of = |s: u64| txs[usize::try_from(s).expect("seq")].udp + OVH;
    let mut next_send = 0usize;
    let mut line_free = 0u64;
    let mut steps = Vec::new();
    for (i, e) in o.acks.iter().enumerate() {
        let f = &e.frame;
        let wire = f.encode().len() as u64;
        let ahead = if busy_at(busy, e.at) { AHEAD } else { 0 };
        line_free = line_free.max(e.at + ahead) + (wire + OVH) * SEC / RETURN_BPS;
        let now = line_free + ONE_WAY;
        while next_send < txs.len() && txs[next_send].send_ns <= now {
            let x = txs[next_send];
            g.on_sent(x.seq, x.send_ns, x.udp);
            next_send += 1;
        }
        let sent_upto = next_send as u64;
        let hi = largest(f);
        let acked: Vec<u64> = f
            .acked_full(hi + 1)
            .into_iter()
            .filter(|&s| s < sent_upto)
            .collect();
        let before = g.pace_bps();
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
            let valid = first < hi && received.contains(&hi) && hi < sent_upto;
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
            drop_alarms: e.drop_alarms,
            rise_alarms: e.rise_alarms,
            pair,
            interval: interval.map(|iv| (iv.first_seq, iv.last_seq)),
            g_ns: g_ns.filter(|_| measured > measured_before),
            sample: interval.filter(|_| samples > samples_before).map(|iv| {
                let w: u64 = (iv.first_seq + 1..=iv.last_seq).map(wire_of).sum();
                w * SEC / iv.rx_span_ns.max(1)
            }),
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
/// tavan ölçütün üstüne çıkmaz. `None` = koşu boyunca ulaşmadı.
fn settle(ch: &[(u64, u64)]) -> Option<(u64, u64)> {
    let i = ch
        .iter()
        .rposition(|&(_, p)| p > CEILING_MAX)
        .map_or(0, |i| i + 1);
    ch.get(i).copied()
}

/// İlk indirim: tavanı düşüren ilk değişimin anı (göndericide).
fn first_down(ch: &[(u64, u64)]) -> Option<u64> {
    let mut prev = PROFILE_LINE;
    for &(t, p) in ch {
        if p < prev {
            return Some(t);
        }
        prev = p;
    }
    None
}

fn secs(ns: u64) -> String {
    format!("{:.3}", ns as f64 / MS as f64 / 1000.0)
}

fn step_line(s: &Step) -> String {
    format!(
        "ACK #{} ({}, çıkış {} s, göndericide {} s), aralık {:?}, g {:?} ms, örnek {:?}, tavan {} → {}",
        s.n,
        if s.alarm { "kanıtlı alarm" } else { "planlı" },
        secs(s.at),
        secs(s.proc),
        s.interval,
        s.g_ns.map(|g| g as f64 / MS as f64),
        s.sample,
        s.before,
        s.after
    )
}

/// Kayıp deseni: tohumlu %20 rastgele (splitmix64; hatta serileşip düşer).
fn random_lost(seed: u64, seq: u64) -> bool {
    let mut z = (seed << 32 ^ seq).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)).is_multiple_of(5)
}

/// Düşüşten sonra varan ilk seq (`DROP_AT`'te ya da sonra): düşüşten başlayan kayıp kolu.
const DROP_SEQ: u64 = (DROP_AT - SEC) / OLD_GAP + 1;

/// Zaman çizelgesi hücresi: ad, kayıp, dönüş penceresi.
struct Cell {
    name: String,
    lost: Box<dyn Fn(u64) -> bool>,
    busy: Busy,
}

/// Dönüş kolları: açık; düşüşten 1 s önce dolar, 32 s'de / 34 s'de boşalır; koşu sonuna dek dolu.
fn returns() -> [(&'static str, Busy); 4] {
    [
        ("açık dönüş", None),
        ("32 s'de boşalan dönüş", Some((DROP_AT - SEC, 32 * SEC))),
        ("34 s'de boşalan dönüş", Some((DROP_AT - SEC, 34 * SEC))),
        ("dolu dönüş", Some((DROP_AT - SEC, u64::MAX))),
    ]
}

/// Belirlenimli kollar: kayıpsız, her beşte bir, her yedide bir kayıp × dört dönüş.
fn deterministic_cells() -> Vec<Cell> {
    let losses: [(&str, Loss); 3] = [
        ("kayıpsız", |_| false),
        ("her beşte bir kayıp", every_fifth),
        ("her yedide bir kayıp", every_seventh),
    ];
    let mut cells = Vec::new();
    for (ret, busy) in returns() {
        for (name, lost) in losses {
            cells.push(Cell {
                name: format!("{name}, {ret}"),
                lost: Box::new(lost),
                busy,
            });
        }
    }
    cells
}

/// Doğrulayıcının yoklaması: tohumlu %20 rastgele kayıp, (A) oturum başından, (B) düşüşten
/// başlayan ([`DROP_SEQ`]'ten), 16 tohum × dört dönüş.
fn random_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for (kind, from) in [("A oturum başından", 0), ("B düşüşten", DROP_SEQ)] {
        for seed in 1..=16u64 {
            for (ret, busy) in returns() {
                cells.push(Cell {
                    name: format!("%20 rastgele {kind} tohum {seed}, {ret}"),
                    lost: Box::new(move |s| s >= from && random_lost(seed, s)),
                    busy,
                });
            }
        }
    }
    cells
}

/// Aday alıcısının (`f952a1a1`: alarm ACK'i alarm anında, aralık bir ACK geriye, çift alanı son
/// ACK'ten bu yana bütün çiftler) aynı düzenekteki tavan değişimleri. Satır: `hücre|an:tavan,…`
/// (an ns, göndericide). Aynı test gövdesi adayın `receiver.rs` ve `arrival_rate.rs`'i ile koşuldu
/// (dosyalar geçici olarak değiştirildi, sonra geri alınıp bayt bayt karşılaştırıldı); satırlar
/// `ADAY-VERİ|` çıktısından.
const CANDIDATE_TIMELINE: &str = include_str!("ack_alarm_candidate_timeline.txt");

fn candidate_changes(name: &str) -> Option<Vec<(u64, u64)>> {
    CANDIDATE_TIMELINE.lines().find_map(|l| {
        let (cell, rest) = l.split_once('|')?;
        (cell == name).then(|| {
            rest.split(',')
                .filter(|x| !x.is_empty())
                .map(|x| {
                    let (t, p) = x.split_once(':').expect("an:tavan");
                    (t.parse().expect("an"), p.parse().expect("tavan"))
                })
                .collect()
        })
    })
}

/// Bir hücrenin ölçümü ve adayla kıyası.
struct Timeline {
    name: String,
    /// Düşüşün daralma alarmı (bağımsız dedektör, `DROP_AT`'ten sonraki ilk tetiklenme).
    alarm_ns: Option<u64>,
    /// Düşüşten önceki (sahte) daralma alarmı sayısı.
    false_alarms: usize,
    steps: Vec<Step>,
    cand: Option<Vec<(u64, u64)>>,
}

impl Timeline {
    fn measure(cell: &Cell) -> (Self, Run, Vec<Tx>) {
        let txs = fifo(
            TIMELINE_UNTIL,
            |_| SENDER_UDP,
            |s| {
                if (cell.lost)(s) {
                    Fate::LostOnLine
                } else {
                    Fate::Delivered
                }
            },
            &drop_line(),
        );
        let o = drive(&txs, TIMELINE_UNTIL, cell.busy, Some(PROFILE_LINE), |_| {
            true
        });
        let steps = replay(&o, &txs, None, cell.busy);
        let ch = changes(&steps);
        println!(
            "ADAY-VERİ|{}|{}",
            cell.name,
            ch.iter()
                .map(|(t, p)| format!("{t}:{p}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let tl = Timeline {
            name: cell.name.clone(),
            alarm_ns: o.alarms.iter().copied().find(|&t| t >= DROP_AT),
            false_alarms: o.alarms.iter().filter(|&&t| t < DROP_AT).count(),
            steps,
            cand: candidate_changes(&cell.name),
        };
        (tl, o, txs)
    }

    /// Alarmdan sonra alıcıdan çıkan ilk ACK (kanıtlı alarm ya da planlı).
    fn alarm_exit(&self) -> Option<&Step> {
        let a = self.alarm_ns?;
        self.steps.iter().find(|s| s.at >= a)
    }

    /// Alarmdan sonra tavanı düşüren ilk ACK.
    fn first_down_step(&self) -> Option<&Step> {
        let a = self.alarm_ns.unwrap_or(0);
        self.steps.iter().find(|s| s.at >= a && s.after < s.before)
    }

    fn print(&self) {
        let ch = changes(&self.steps);
        let since = |t: u64| {
            self.alarm_ns.map_or_else(
                || "—".to_string(),
                |a| format!("{}", (t.saturating_sub(a)) / MS),
            )
        };
        println!(
            "== ZAMAN ÇİZELGESİ {}: alarm {} s (düşüşten önce {} sahte alarm)",
            self.name,
            self.alarm_ns.map_or_else(|| "yok".to_string(), secs),
            self.false_alarms
        );
        if let Some(a) = self.alarm_exit() {
            println!(
                "   alarmdan sonraki ilk ACK: {}; alarmdan {} ms",
                step_line(a),
                since(a.at)
            );
        }
        if let Some(f) = self.first_down_step() {
            println!(
                "   ilk indirim: {}; alarmdan {} ms",
                step_line(f),
                since(f.proc)
            );
        }
        for s in self.steps.iter().filter(|s| s.after != s.before) {
            println!("   değişim: {}", step_line(s));
        }
        match settle(&ch) {
            Some((t, p)) => println!(
                "   oturma (≤ {CEILING_MAX}): göndericide {} s, tavan {p}; alarmdan {} ms",
                secs(t),
                since(t)
            ),
            None => println!(
                "   koşu boyunca ulaşmadı (≤ {CEILING_MAX}); son tavan {}",
                self.steps.last().map_or(PROFILE_LINE, |s| s.after)
            ),
        }
        if let Some(c) = &self.cand {
            println!(
                "   aday: ilk indirim {:?} s, oturma {}, değişimler {:?}",
                first_down(c).map(secs),
                settle(c).map_or_else(
                    || "koşu boyunca ulaşmadı".to_string(),
                    |(t, p)| format!("{} s ({p})", secs(t))
                ),
                c.iter().map(|&(t, p)| (secs(t), p)).collect::<Vec<_>>()
            );
        }
    }

    /// Tablo satırı (PR gövdesi).
    fn row(&self) -> String {
        let ch = changes(&self.steps);
        let since = |t: u64| self.alarm_ns.map_or(0, |a| t.saturating_sub(a) / MS);
        let exit = self.alarm_exit().map_or_else(
            || "yok".to_string(),
            |s| {
                format!(
                    "#{} {}, {}, {}",
                    s.n,
                    if s.alarm { "kanıtlı" } else { "planlı" },
                    secs(s.at),
                    since(s.at)
                )
            },
        );
        let first = self.first_down_step().map_or_else(
            || "yok".to_string(),
            |f| {
                format!(
                    "#{}, {}, {:?}, {:?}, {} → {}",
                    f.n,
                    secs(f.proc),
                    f.interval,
                    f.sample,
                    f.before,
                    f.after
                )
            },
        );
        let st = settle(&ch).map_or_else(
            || {
                format!(
                    "koşu boyunca ulaşmadı (son {})",
                    self.steps.last().map_or(PROFILE_LINE, |s| s.after)
                )
            },
            |(t, _)| format!("{}, {}", secs(t), since(t)),
        );
        let cand = self.cand.as_ref().map_or_else(
            || "—".to_string(),
            |c| {
                format!(
                    "{} / {}",
                    first_down(c).map_or_else(|| "yok".to_string(), secs),
                    settle(c).map_or_else(|| "ulaşmadı".to_string(), |(t, _)| secs(t))
                )
            },
        );
        let list: Vec<String> = self
            .steps
            .iter()
            .filter(|s| s.after != s.before)
            .map(|s| format!("#{}: {}", s.n, s.after))
            .collect();
        format!(
            "| {} | {} | {exit} | {first} | {st} | {cand} | {} |",
            self.name,
            self.alarm_ns.map_or_else(|| "yok".to_string(), secs),
            list.join("; ")
        )
    }

    /// Sahibin ölçütü: (i) dal tavanı adayınkinden yalnız adayın ilk indirimi ile dalın ilk
    /// indirimi arasındaki pencerede yüksek olabilir; (ii) dal adaydan geç oturmaz, ilk indirim
    /// gecikmişse kesinlikle daha erken oturur (aday oturmuyorsa dal oturmalıdır). Tutmayanlar
    /// sayılarıyla döner.
    fn criteria(&self) -> Vec<String> {
        let name = &self.name;
        let Some(cand) = &self.cand else {
            return vec![format!("{name}: adayın çizelgesi yok")];
        };
        let ch = changes(&self.steps);
        let mut bad = Vec::new();
        let (ours_first, cand_first) = (first_down(&ch), first_down(cand));
        let mut instants: Vec<u64> = ch.iter().chain(cand.iter()).map(|&(t, _)| t).collect();
        instants.sort_unstable();
        instants.dedup();
        for (k, &t) in instants.iter().enumerate() {
            let (b, c) = (pace_at(&ch, t), pace_at(cand, t));
            if b <= c {
                continue;
            }
            let until = instants.get(k + 1).copied().unwrap_or(TIMELINE_UNTIL);
            let in_window =
                cand_first.is_some_and(|cf| cf <= t) && ours_first.is_none_or(|of| t < of);
            println!(
                "{name}: {} – {} s arası dal {b} > aday {c} ({} ms){}",
                secs(t),
                secs(until),
                (until - t) / MS,
                if in_window {
                    ", ilk indirim penceresinde"
                } else {
                    ""
                }
            );
            if !in_window {
                bad.push(format!(
                    "(i) {name}: {} – {} s dal {b} > aday {c} ({} ms)",
                    secs(t),
                    secs(until),
                    (until - t) / MS
                ));
            }
        }
        let opt = |t: Option<u64>| t.map_or_else(|| "—".to_string(), secs);
        println!(
            "KIYAS|{name}|ilk indirim dal {} aday {}|oturma dal {} aday {}|dal > aday {} ms",
            opt(ours_first),
            opt(cand_first),
            opt(settle(&ch).map(|x| x.0)),
            opt(settle(cand).map(|x| x.0)),
            instants
                .iter()
                .enumerate()
                .filter(|&(_, &t)| pace_at(&ch, t) > pace_at(cand, t))
                .map(|(k, &t)| instants.get(k + 1).copied().unwrap_or(TIMELINE_UNTIL) - t)
                .sum::<u64>()
                / MS
        );
        match (settle(&ch), settle(cand)) {
            (None, Some((tc, _))) => bad.push(format!(
                "(ii) {name}: dal koşu boyunca ulaşmadı, aday {} s'de oturdu",
                secs(tc)
            )),
            (Some((tb, _)), Some((tc, _))) if tb > tc => bad.push(format!(
                "(ii) {name}: dal {} s, aday {} s'de oturdu ({} ms geç)",
                secs(tb),
                secs(tc),
                (tb - tc) / MS
            )),
            (Some((tb, _)), Some((tc, _)))
                if tb == tc && ours_first.zip(cand_first).is_some_and(|(o, c)| o > c) =>
            {
                bad.push(format!(
                    "(ii) {name}: ilk indirim gecikti, oturma erkenleşmedi ({} s)",
                    secs(tb)
                ));
            }
            _ => {}
        }
        bad
    }
}

/// (h) [ALARM ARALIĞI · TUR 4 · #175 7087793188] Göndericiyle birlikte, zaman çizelgesi;
/// belirlenimli kollar (kayıpsız, her beşte bir, her yedide bir kayıp × açık / 32 s'de boşalan /
/// 34 s'de boşalan / dolu dönüş), beyan hat profil (ürünün sunucu yuvası; kural kapalı kol aynı
/// çizelgeyi verir). Her hücrede alarm anı, alarmdan sonraki ilk ACK (kanıtlı ya da planlı) ve
/// farkı, tavanı ilk düşüren ACK (aralık, örnek, tavan), ≤ 6 875'e oturma ya da "koşu boyunca
/// ulaşmadı" ve her tavan değişimi basılır; adayın çizelgesiyle ([`CANDIDATE_TIMELINE`]) her
/// değişim anında kıyaslanır.
///
/// Sınırlar sahibin ölçütünden ([`Timeline::criteria`] (i)–(ii)) ve (iii): kayıpsız açık dönüşte
/// alarm anındaki ACK kanıtlı alarm ACK'idir (gecikme yok) ve tavanı ≤ 6 875'e indirir. Açık
/// dönüşte kanıtlı alarm ACK'i göndericide örnek verir (kanıtın tanımı). Duyarlılık: kanıtlı alarm
/// ACK'inin aralığı bir ACK geriye kurulsaydı (adayın kuralı) tavan ölçütün üstünde kalır.
#[test]
fn h_alarm_ack_timeline_at_the_sender() {
    let mut rows = Vec::new();
    let mut bad = Vec::new();
    // Önce bütün hücreler ölçülür (adayın çizelgesi aynı gövdeden üretilir), sonra sınanır.
    let measured: Vec<_> = deterministic_cells()
        .into_iter()
        .map(|cell| {
            let m = Timeline::measure(&cell);
            (cell, m)
        })
        .collect();
    for (cell, (tl, o, txs)) in measured {
        let off = replay(
            &drive(&txs, TIMELINE_UNTIL, cell.busy, None, |_| true),
            &txs,
            None,
            cell.busy,
        );
        assert_eq!(
            tl.steps, off,
            "{}: beyan hat kapalı kol başka çizelge verdi",
            tl.name
        );
        tl.print();
        let name = &tl.name;
        let alarm_ns = tl
            .alarm_ns
            .unwrap_or_else(|| panic!("{name}: daralma alarmı yok"));
        assert_eq!(tl.false_alarms, 0, "{name}: düşüşten önce alarm");
        assert!(
            tl.steps
                .iter()
                .take_while(|s| s.at < alarm_ns)
                .all(|s| s.after == PROFILE_LINE),
            "{name}: alarmdan önce tavan değişti"
        );
        let alarm = tl.steps.iter().find(|s| s.alarm);
        if cell.busy.is_none() {
            let a = alarm.unwrap_or_else(|| panic!("{name}: kanıtlı alarm ACK'i yok"));
            assert!(
                a.sample.is_some(),
                "{name}: kanıtlı alarm ACK'i örnek vermedi"
            );
            assert!(
                a.after <= CEILING_MAX,
                "{name}: alarm ACK'inde tavan {}",
                a.after
            );
            let prev = o
                .bitmap()
                .iter()
                .rev()
                .find(|e| e.at < a.at)
                .map(|e| largest(&e.frame))
                .expect("önceki ACK");
            let back = replay(&o, &txs, Some(o.first_received_after(prev)), cell.busy);
            let b = back.iter().find(|s| s.alarm).expect("duyarlılık kolu");
            println!("   bir ACK geriye (duyarlılık): {}", step_line(b));
            assert!(
                b.after > CEILING_MAX,
                "{name}: duyarlılık kolu tavanı {}",
                b.after
            );
        }
        // (iii) Kayıpsız açık dönüş: alarm anındaki ACK kanıtlı ve ilk ACK'te ≤ 6 875.
        if name.starts_with("kayıpsız, açık") {
            let exit = tl.alarm_exit().expect("alarmdan sonra ACK");
            assert!(
                exit.alarm && exit.at == alarm_ns && exit.after <= CEILING_MAX,
                "{name}: (iii) alarm ACK'i {} ms gecikti ya da tavan {}",
                (exit.at - alarm_ns) / MS,
                exit.after
            );
        }
        bad.extend(tl.criteria());
        rows.push(tl.row());
    }
    print_table(&rows);
    assert!(bad.is_empty(), "sahibin ölçütü tutmadı: {bad:#?}");
}

fn print_table(rows: &[String]) {
    println!(
        "| hücre | alarm (s) | alarmdan sonraki ilk ACK: no tür, çıkış (s), alarmdan (ms) | tavanı ilk \
         düşüren ACK: no, göndericide (s), aralık, örnek, tavan | ≤ {CEILING_MAX} oturma: göndericide \
         (s), alarmdan (ms) | aday: ilk indirim / oturma (s) | değişimler (no: tavan) |"
    );
    println!("|---|---|---|---|---|---|---|");
    for r in rows {
        println!("{r}");
    }
}

/// (h, rastgele) [TUR 4] Doğrulayıcının yoklaması (h)'nin parçası: tohumlu %20 rastgele kayıp,
/// (A) oturum başından ve (B) düşüşten başlayan, 16 tohum × dört dönüş (128 hücre), aynı ölçüt
/// (i)–(ii). Sahte daralma alarmında (düşüşten önce) kanıt gelmez, planlı ACK adayın kuralıyla
/// çıkar. Ölçüt gevşetilmez: tutmayan hücreler sayılarıyla basılır, test kırmızıdır.
#[test]
fn h_random_loss_probe_timeline() {
    let mut rows = Vec::new();
    let mut bad = Vec::new();
    let mut differ = 0;
    for cell in random_cells() {
        let (tl, _, _) = Timeline::measure(&cell);
        tl.print();
        if tl.cand.as_ref() != Some(&changes(&tl.steps)) {
            differ += 1;
        }
        bad.extend(tl.criteria());
        rows.push(tl.row());
    }
    print_table(&rows);
    println!(
        "rastgele yoklama: {} hücre, adaydan farklı çizelge {differ}, ölçütü bozan {}",
        rows.len(),
        bad.len()
    );
    assert!(bad.is_empty(), "sahibin ölçütü tutmadı: {bad:#?}");
}

// ---------------------------------------------------------------------------------------------
// (x)(y)(z) [TUR 4] Bırakma ölçütü alıcının gerçek varış kanıtı; planlı ACK adayın kuralıyla.
// ---------------------------------------------------------------------------------------------

/// Bir akışın ölçümü: belirlenimli kayıp `fate`, gövde `udp_of`, hat `line`.
fn measure_flow(
    udp_of: impl Fn(u64) -> u64,
    fate: impl Fn(u64) -> Fate,
    line: &[(u64, u64)],
    busy: Busy,
) -> (Run, Vec<Tx>, Vec<Step>) {
    let txs = fifo(TIMELINE_UNTIL, udp_of, fate, line);
    let o = drive(&txs, TIMELINE_UNTIL, busy, Some(PROFILE_LINE), |_| true);
    let steps = replay(&o, &txs, None, busy);
    (o, txs, steps)
}

/// Düşüş alarmından (bağımsız dedektör, `DROP_AT`'ten sonraki ilk tetiklenme) sonra alıcıdan çıkan
/// ilk ACK, göndericide: kanıtlı alarm ACK'iyse örnek vermiş olmalı; planlıysa (alarm sayacı
/// artmamış) aralığı adayın kuralıyla (son ACK'in en büyüğünden sonraki ilk alınan seq) olmalı.
/// Dönüş: o ACK'in satırı.
fn first_ack_after_drop_alarm<'a>(name: &str, o: &Run, steps: &'a [Step]) -> &'a Step {
    let alarm = o
        .alarms
        .iter()
        .copied()
        .find(|&t| t >= DROP_AT)
        .unwrap_or_else(|| panic!("{name}: düşüş alarmı yok"));
    let i = steps
        .iter()
        .position(|s| s.at >= alarm)
        .unwrap_or_else(|| panic!("{name}: alarmdan sonra ACK yok"));
    let s = &steps[i];
    println!(
        "{name}: alarm {} s; ilk ACK {}; alarmdan {} ms",
        secs(alarm),
        step_line(s),
        (s.at - alarm) / MS
    );
    if s.alarm {
        assert!(
            s.sample.is_some(),
            "{name}: kanıtlı alarm ACK'i göndericide örnek vermedi (aralık {:?}, g {:?} ms)",
            s.interval,
            s.g_ns.map(|g| g as f64 / MS as f64)
        );
    } else {
        let e = &o.acks[i];
        let prev = largest(&o.acks[i - 1].frame);
        assert_eq!(
            e.drop_alarms,
            steps[i - 1].drop_alarms,
            "{name}: planlı ACK alarm sayıldı"
        );
        assert_eq!(
            span_first(&e.frame),
            o.first_received_after(prev),
            "{name}: planlı ACK'in aralığı adayın kuralı değil"
        );
    }
    s
}

/// (x) [TUR 4] Darboğazdan ÖNCE düşen seq daralma başlangıcından sonraki aralıkta. Her beşte bir
/// kayıp kolu (alarm 365'te, başlangıç 364, kanıt yetersiz) ve seq 366 darboğazdan önce düşer
/// (hattı kaplamaz; aynı mantıksal paket, belirlenimli). Tur 3 kuralı (sıra farkı ≥ 2) 367'de
/// bırakırdı: aralık (364, 367], `rx_span` 320 ms, gönderim 240 ms, g 80 ms < eşik ≈ 98 ms, örnek
/// yok. Yeni ölçüt alınmayan 366'yı `DEFAULT_MTU` tel üst sınırıyla sayar ve bekler: alarm ACK'i
/// göndericide örnek verir (ya da planlı ACK adayın kuralıyla çıkar). Açık dönüş.
#[test]
fn x_seq_lost_before_the_bottleneck_is_not_evidence() {
    const LOST_BEFORE: u64 = 366;
    let (o, _, steps) = measure_flow(
        |_| SENDER_UDP,
        |s| {
            if s == LOST_BEFORE {
                Fate::LostBefore
            } else if every_fifth(s) {
                Fate::LostOnLine
            } else {
                Fate::Delivered
            }
        },
        &drop_line(),
        None,
    );
    let s = first_ack_after_drop_alarm("(x)", &o, &steps);
    if let Some((f, l)) = s.interval.filter(|_| s.alarm) {
        assert!(
            f < LOST_BEFORE && LOST_BEFORE <= l,
            "(x) aralık 366'yı kapsamıyor: {f}..{l}"
        );
    }
}

/// (y) [TUR 4] Daralmadan sonra küçük datagramlar (146 B tel, PTT demeti boyu). Her beşte bir kayıp
/// kolu (alarm 365'te, başlangıç 364, kanıt yetersiz); seq 366'dan itibaren gönderici küçük datagram
/// gönderir (pacer profil hattında: 11,7 ms arayla; 6 250 hatta serileşme 23,4 ms). Tur 3 kuralı
/// ilk küçük datagramda (366, sıra farkı 2) bırakırdı: aralık (364, 366], `rx_span` ≈ 183 ms,
/// gönderim 160 ms, seri farkı ≈ −68 ms, g ≈ 92 ms < eşik ≈ 98 ms, örnek yok. Yeni ölçüt yeterli
/// kanıta dek bekler: alarm ACK'i göndericide örnek verir ya da planlı ACK adayın kuralıyla çıkar.
/// İki kol: kayıp küçük datagramlarda da sürer (alınmayan her seq `DEFAULT_MTU` üst sınırıyla
/// sayılır, kanıt yetmez, planlı ACK üstlenir) / kayıp 366'dan sonra yok (kanıt yeterli olur). Açık
/// dönüş.
#[test]
fn y_small_datagrams_after_the_drop_wait_for_enough_evidence() {
    const SMALL_FROM: u64 = 366;
    let arms: [(&str, Loss); 2] = [
        ("(y) kayıp sürer", every_fifth),
        ("(y) küçüklerde kayıp yok", |s| {
            s < SMALL_FROM && every_fifth(s)
        }),
    ];
    for (name, lost) in arms {
        let (o, _, steps) = measure_flow(
            |s| {
                if s >= SMALL_FROM {
                    SMALL_UDP
                } else {
                    SENDER_UDP
                }
            },
            |s| {
                if lost(s) {
                    Fate::LostOnLine
                } else {
                    Fate::Delivered
                }
            },
            &drop_line(),
            None,
        );
        let s = first_ack_after_drop_alarm(name, &o, &steps);
        if let Some((f, l)) = s.interval.filter(|_| s.alarm) {
            assert!(
                l - f > 2,
                "{name}: aralık iki datagramdan uzun değil: {f}..{l}"
            );
        }
    }
}

/// Dürüst sabit 12 500 hat, tohumlu %20 rastgele kayıp oturum başından: (z) akışı.
fn honest_random_txs(seed: u64, until: u64) -> Vec<Tx> {
    fifo(
        until,
        |_| SENDER_UDP,
        |s| {
            if random_lost(seed, s) {
                Fate::LostOnLine
            } else {
                Fate::Delivered
            }
        },
        &[(0, PROFILE_LINE)],
    )
}

/// (z) [TUR 4] Sahte daralma alarmı (dürüst sabit hat, tohumlu %20 rastgele kayıp oturum başından,
/// 16 tohum, açık dönüş): kanıt gelmez, planlı ACK adayın ACK'iyle aynı çerçeveyi taşır (aralık +
/// çift alanı). İkiz alıcı aynı akışı alır, yalnız ilk sahte alarmı tetikleyen datagramın boyu
/// verilmez (dedektör o varışı görmez, alarm yok; `ack_alarm_planned.rs` ile aynı). İki kol:
/// (z1) alarmdan sonra datagram gelmez; (z2) akış sürer. Her kolda alarmdan sonraki ilk ACK iki
/// alıcıda aynı anda ve bayt bayt aynı çerçeveyle çıkar, alarm sayılmaz.
///
/// İkizde alarm datagramının çifti ölçülmez (boy yok). Değeri `x` = gövde / önceki varıştan
/// aralık. `x` ikizin alanından büyük değilse iki alıcının en büyük ikinci değeri aynıdır: ön koşul
/// olarak sınanır. Karşı-olgu (planlı ACK'te çift birleştirmesi yok): z1'de alarmdan sonra çift
/// yoktur, alan `PAIR_RATE_NONE` olur, ikizinki düşüş öncesi değerdir.
#[test]
fn z_false_alarm_planned_ack_is_the_candidate_frame() {
    const WATCH: u64 = 30 * SEC;
    let mut compared = 0;
    for seed in 1..=16u64 {
        let full = honest_random_txs(seed, 60 * SEC);
        let o = drive(&full, 60 * SEC, None, Some(PROFILE_LINE), |_| true);
        let Some(&t_alarm) = o.alarms.first() else {
            println!("tohum {seed}: sahte alarm yok");
            continue;
        };
        let (&alarm_seq, _) = o
            .arrival
            .iter()
            .find(|&(_, &a)| a == t_alarm)
            .expect("alarm datagramı");
        let prev_arrival = o
            .arrival
            .values()
            .copied()
            .filter(|&a| a < t_alarm)
            .max()
            .expect("önceki varış");
        let x = SENDER_UDP * SEC / (t_alarm - prev_arrival);
        let stop: Vec<Tx> = full
            .iter()
            .map(|t| Tx {
                arrival: t.arrival.filter(|&a| a <= t_alarm),
                ..*t
            })
            .collect();
        for (arm, txs) in [("z1 akış durur", &stop), ("z2 akış sürer", &full)] {
            let until = t_alarm + WATCH;
            let ours = drive(txs, until, None, Some(PROFILE_LINE), |_| true);
            let twin = drive(txs, until, None, Some(PROFILE_LINE), |s| s != alarm_seq);
            let first = |r: &Run| {
                r.acks.iter().position(|e| e.at >= t_alarm).map(|i| {
                    (
                        i,
                        r.acks[i].at,
                        r.acks[i].frame.clone(),
                        r.acks[i].drop_alarms,
                    )
                })
            };
            let (i, at, f, alarms) =
                first(&ours).unwrap_or_else(|| panic!("{seed} {arm}: ACK yok"));
            let (_, tat, tf, _) =
                first(&twin).unwrap_or_else(|| panic!("{seed} {arm}: ikizde ACK yok"));
            let field = |f: &AckFrame| f.bitmap.as_ref().expect("bm").pair_rate;
            println!(
                "tohum {seed} {arm}: sahte alarm {} s (seq {alarm_seq}, x {x}); ACK {} s / ikiz {} s, \
                 aralık {} / {}, çift {} / {}",
                secs(t_alarm),
                secs(at),
                secs(tat),
                span_first(&f),
                span_first(&tf),
                field(&f),
                field(&tf)
            );
            assert!(
                field(&tf) != PAIR_RATE_NONE && x <= u64::from(field(&tf)),
                "tohum {seed} {arm}: ön koşul (x {x} ≤ ikizin alanı {})",
                field(&tf)
            );
            assert_eq!(
                alarms, 0,
                "tohum {seed} {arm}: sahte alarmda kanıtlı alarm ACK'i"
            );
            assert_eq!(
                at, tat,
                "tohum {seed} {arm}: planlı ACK ikizle aynı anda değil"
            );
            assert_eq!(
                span_first(&f),
                ours.first_received_after(largest(&ours.acks[i - 1].frame)),
                "tohum {seed} {arm}: aralık adayın kuralı değil"
            );
            assert_eq!(
                f.encode(),
                tf.encode(),
                "tohum {seed} {arm}: çerçeve ikizinkinden farklı"
            );
            compared += 1;
        }
    }
    println!("(z) kıyaslanan: {compared}");
    assert!(compared >= 16, "(z) yeterli sahte alarm yok ({compared})");
}

// ---------------------------------------------------------------------------------------------
// (k) [HEDEF 4] Düşüş sonrası hızlanma: toparlanma referansı, ilk yükseliş, göndericide tavan.
// ---------------------------------------------------------------------------------------------

/// (k) Hızlanma: 12 500 → 6 250 → 12 500 (toparlanma 33 s / 45 s), açık ve dolu dönüş ((f)
/// düzeneği), üç belirlenimli kayıp deseni; gerçek `LineGuard` ile. Her kolda yükseliş referansı
/// (değer ve kurulduğu ACK: daralma alarmından sonra çift değeri bildiren ilk ACK), ilk yükseliş
/// alarmının ACK'i ve anı, göndericide tavanın toparlanmadan sonra ilk açıldığı an ve yeni tavan
/// (hat korumasının pacer'a verdiği hız) basılır. Gerçek tel hızı bu açık döngü düzenekte ölçülemez
/// (gönderici tavanla yavaşlamaz): ürün hücresinin işi.
///
/// Sınırlar (f) ile aynı (dolu dönüş): referans toparlanmadan önce ve ≤ 6 875, yükseliş alarmı
/// toparlanmadan sonra < 3 s. Göndericide tavanın açılışı ölçülür, sınanmaz: dolu dönüşte yükseliş
/// ACK'inin aralığı (bir ACK geriye, (d)) toparlanma öncesi kuyruk artışını da kapsar, gönderici
/// aşağı yönlü örneği öncelikli tutar ve çift alt sınırını kullanmaz (`line_guard`, kapsam dışı);
/// sonra ACK çıkmaz. Bulgu PR gövdesinde.
#[test]
fn k_recovery_rise_reference_and_first_rise_at_the_sender() {
    let losses: [(&str, Loss); 3] = [
        ("kayıpsız", |_| false),
        ("her beşte bir kayıp", every_fifth),
        ("her yedide bir kayıp", every_seventh),
    ];
    let mut rows = Vec::new();
    let mut checks = Vec::new();
    for (ret, busy) in [
        ("açık dönüş", None),
        ("dolu dönüş", Some((DROP_AT - SEC, u64::MAX))),
    ] {
        for (lname, lost) in losses {
            for recover_at in [33 * SEC, 45 * SEC] {
                let name = format!("{lname}, {ret}, toparlanma {} s", recover_at / SEC);
                let (o, _, steps) = measure_flow(
                    |_| SENDER_UDP,
                    |s| {
                        if lost(s) {
                            Fate::LostOnLine
                        } else {
                            Fate::Delivered
                        }
                    },
                    &recovery_line(recover_at),
                    busy,
                );
                let alarm = o.alarms.iter().copied().find(|&t| t >= DROP_AT);
                let reference =
                    alarm.and_then(|a| steps.iter().find(|s| s.at >= a && s.pair.is_some()));
                let rise = steps.iter().find(|s| s.rise_alarms >= 1);
                // Yükseliş sınamasının karşılaştırdığı değer: yükselişten önce son bildirilen çift
                // değeri (`PairRate::reported` her ACK'te yenilenir).
                let last_ref = steps
                    .iter()
                    .filter(|s| alarm.is_some_and(|a| s.at >= a))
                    .take_while(|s| rise.is_none_or(|k| s.n < k.n))
                    .filter(|s| s.pair.is_some() && s.at < recover_at)
                    .last();
                let open = steps
                    .iter()
                    .find(|s| s.proc >= recover_at && s.after > s.before);
                for s in steps.iter().filter(|s| alarm.is_some_and(|a| s.at >= a)) {
                    println!(
                        "   {name}: {} pair {:?} yükseliş {}",
                        step_line(s),
                        s.pair,
                        s.rise_alarms
                    );
                }
                println!(
                    "{name}: daralma alarmı {:?} s; referans {:?} (ACK #{:?}, çıkış {:?} s); ilk \
                     yükseliş ACK'i #{:?} çıkış {:?} s, pair {:?}; tavan açılışı {:?}; son tavan {}",
                    alarm.map(secs),
                    reference.and_then(|s| s.pair),
                    reference.map(|s| s.n),
                    reference.map(|s| secs(s.at)),
                    rise.map(|s| s.n),
                    rise.map(|s| secs(s.at)),
                    rise.and_then(|s| s.pair),
                    open.map(|s| format!(
                        "ACK #{} göndericide {} s, {} → {}",
                        s.n,
                        secs(s.proc),
                        s.before,
                        s.after
                    )),
                    steps.last().map_or(PROFILE_LINE, |s| s.after)
                );
                let ref_txt = |r: Option<&Step>| {
                    r.map_or_else(
                        || "yok".to_string(),
                        |s| format!("{} (#{}, {} s)", s.pair.unwrap_or(0), s.n, secs(s.at)),
                    )
                };
                rows.push(format!(
                    "| {name} | {} | {} | {} | {} | {} | {} |",
                    ref_txt(reference),
                    ref_txt(last_ref),
                    rise.map_or_else(
                        || "yok".to_string(),
                        |s| format!("#{}, {} s, pair {}", s.n, secs(s.at), s.pair.unwrap_or(0))
                    ),
                    open.map_or_else(
                        || "koşu boyunca açılmadı".to_string(),
                        |s| format!("#{}, {} s, {} → {}", s.n, secs(s.proc), s.before, s.after)
                    ),
                    steps.last().map_or(PROFILE_LINE, |s| s.after),
                    changes(&steps)
                        .iter()
                        .map(|&(t, p)| format!("{}: {p}", secs(t)))
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
                if busy.is_some() {
                    checks.push((
                        name,
                        recover_at,
                        reference.map(|s| (s.at, s.pair)),
                        rise.map(|s| s.at),
                    ));
                }
            }
        }
    }
    println!(
        "| kol | daralmadan sonra ilk referans (ACK, çıkış) | toparlanmadan önce son referans | ilk \
         yükseliş ACK'i | göndericide tavan açılışı | son tavan | tavan değişimleri (göndericide s: \
         tavan) |"
    );
    println!("|---|---|---|---|---|---|---|");
    for r in rows {
        println!("{r}");
    }
    for (name, recover_at, reference, rise) in checks {
        let (at, pair) = reference.unwrap_or_else(|| panic!("{name}: referans yok"));
        assert!(
            at < recover_at && pair.is_some_and(|p| u64::from(p) <= CEILING_MAX),
            "{name}: referans {pair:?} @ {} s",
            secs(at)
        );
        let k = rise.unwrap_or_else(|| panic!("{name}: yükseliş alarmı yok"));
        assert!(
            k >= recover_at && k < recover_at + 3 * SEC,
            "{name}: yükseliş {} s",
            secs(k)
        );
    }
}

/// (k2) Karşı test: dürüst sabit 6 250 hatta sahte yükseliş yok. Hat oturum başından 6 250 ya da
/// 30 s'de 6 250'ye düşer ve toparlanmaz; üç belirlenimli kayıp deseni × açık / dolu dönüş.
/// Yükseliş alarmı yok ve göndericide tavan ilk indirimden sonra hiçbir ACK'te artmaz.
#[test]
fn k2_honest_6250_line_never_opens_the_ceiling() {
    let losses: [(&str, Loss); 3] = [
        ("kayıpsız", |_| false),
        ("her beşte bir kayıp", every_fifth),
        ("her yedide bir kayıp", every_seventh),
    ];
    let lines: [(&str, Vec<(u64, u64)>); 2] = [
        ("oturum başından 6 250", vec![(0, NEW_LINE)]),
        ("30 s'de 6 250", drop_line()),
    ];
    for (lname, line) in &lines {
        for (ret, busy) in [
            ("açık dönüş", None),
            ("dolu dönüş", Some((DROP_AT - SEC, u64::MAX))),
        ] {
            for (pname, lost) in losses {
                let name = format!("{lname}, {pname}, {ret}");
                let (_, _, steps) = measure_flow(
                    |_| SENDER_UDP,
                    |s| {
                        if lost(s) {
                            Fate::LostOnLine
                        } else {
                            Fate::Delivered
                        }
                    },
                    line,
                    busy,
                );
                let ch = changes(&steps);
                let down = steps.iter().position(|s| s.after < s.before);
                let rises: Vec<&Step> = down
                    .map(|d| steps[d..].iter().filter(|s| s.after > s.before).collect())
                    .unwrap_or_default();
                println!(
                    "{name}: yükseliş alarmı {}, ilk indirim {:?}, sonra artış {}, değişimler {:?}",
                    steps.last().map_or(0, |s| s.rise_alarms),
                    down.map(|d| step_line(&steps[d])),
                    rises.len(),
                    ch.iter().map(|&(t, p)| (secs(t), p)).collect::<Vec<_>>()
                );
                assert_eq!(
                    steps.last().map_or(0, |s| s.rise_alarms),
                    0,
                    "{name}: yükseliş alarmı"
                );
                assert!(down.is_some(), "{name}: tavan hiç inmedi (boş test)");
                assert!(
                    rises.is_empty(),
                    "{name}: tavan açıldı: {:?}",
                    rises.iter().map(|s| step_line(s)).collect::<Vec<_>>()
                );
            }
        }
    }
}

/// (o) [ACK-GÖZLEM · salt gözlem] Alıcının ACK kaydı (`Receiver::diag_ack_obs`) kanıt, hazır olma
/// ve çıkış anlarını yazar. Düzenek (a) ve (b)'ninkidir (30 s'de hat yarıya; açık / dolu dönüş);
/// sınır eklenmez, var olan mekanizmalardan okunur:
/// * her ACK'te sıra bir artar, `emit_ns` ACK'in çıktığı andır, ertelenme sayıları alıcının
///   kümülatif sayaçlarıyla (`diag_ack_*_deferred`) ACK ACK aynı toplamı verir;
/// * `evidence_ns` yalnız kanıtla bırakılan ACK'te doludur (`diag_drop_alarms` o ACK'te arttı);
/// * alarm ACK'inde algılama ≤ kanıt ≤ çıkış ve kanıt düşüşten sonradır; açık dönüşte ACK kanıtın
///   geldiği varışta çıkar, dolu dönüşte bir yol verişten sonra (`AHEAD`) ve bekleme nedeni
///   "yol verme" olarak sayılır.
#[test]
fn o_ack_observation_records_evidence_ready_and_emit_moments() {
    for (name, busy_from) in [("açık dönüş", None), ("dolu dönüş", Some(DROP_AT - SEC))] {
        let o = run(&drop_plan(), 40 * SEC, |_| false, busy_from);
        let mut prev_alarms = 0u64;
        let mut sums = [0u64; 3];
        for (i, e) in o.acks.iter().enumerate() {
            let ob = e.obs;
            assert_eq!(ob.index, i as u64 + 1, "{name}: ACK sırası");
            assert_eq!(ob.emit_ns, e.at, "{name}: çıkış anı");
            assert!(ob.first_try_ns <= ob.emit_ns, "{name}: ilk deneme");
            assert_eq!(
                ob.evidence_ns.is_some(),
                e.drop_alarms > prev_alarms,
                "{name}: ACK {} kanıt anı ↔ kanıtlı bırakma sayacı",
                ob.index
            );
            prev_alarms = e.drop_alarms;
            for (sum, d) in sums
                .iter_mut()
                .zip([ob.defer_budget, ob.defer_yield, ob.defer_no_info])
            {
                *sum += u64::from(d);
            }
            assert_eq!(
                sums, e.deferred,
                "{name}: ACK {} ertelenme toplamı",
                ob.index
            );
            assert_eq!(
                ob.first_defer_ns.is_some(),
                ob.defer_budget + ob.defer_yield + ob.defer_no_info + ob.defer_first_window > 0,
                "{name}: ilk ertelenme anı"
            );
        }
        let (alarm, _) = o.alarm_ack();
        let ob = alarm.obs;
        let (detect, evidence) = (
            ob.detect_ns.expect("algılama anı"),
            ob.evidence_ns.expect("kanıt anı"),
        );
        println!(
            "{name}: alarm ACK no {} algılama {} ms, kanıt {} ms, çıkış {} ms; alarm sonrası \
             ertelenme bütçe {} / yol verme {}",
            ob.index,
            detect / MS,
            evidence / MS,
            ob.emit_ns / MS,
            ob.alarm_defer_budget,
            ob.alarm_defer_yield
        );
        assert!(ob.alarm && !ob.rise && !ob.wait_open, "{name}: alarm ACK'i");
        assert_eq!(ob.alarm_ns, Some(evidence), "{name}: alarm kuralı kanıtla");
        assert!(
            DROP_AT <= detect && detect <= evidence && evidence <= ob.emit_ns,
            "{name}: sıra düşüş ≤ algılama ≤ kanıt ≤ çıkış"
        );
        if ob.detects == 1 {
            assert_eq!(Some(&detect), o.alarms.first(), "{name}: algılama anı");
        }
        // Alarm ACK'inden önceki ACK'lerin kanıt anı yok.
        assert!(
            o.acks
                .iter()
                .take_while(|e| e.obs.index < ob.index)
                .all(|e| e.obs.evidence_ns.is_none() && !e.obs.alarm),
            "{name}: alarmdan önce kanıt anı"
        );
        if busy_from.is_some() {
            assert!(ob.alarm_defer_yield >= 1, "{name}: yol verme sayılmadı");
            assert!(
                ob.emit_ns >= evidence + AHEAD,
                "{name}: alarm ACK'i yol verişten önce çıktı"
            );
            assert!(ob.first_defer_ns.is_some(), "{name}: ilk ertelenme anı");
        } else {
            assert_eq!(
                ob.emit_ns, evidence,
                "{name}: kanıtın geldiği varışta çıkış"
            );
            assert_eq!(
                (ob.alarm_defer_budget, ob.alarm_defer_yield),
                (0, 0),
                "{name}: alarm sonrası ertelenme"
            );
        }
    }
}
