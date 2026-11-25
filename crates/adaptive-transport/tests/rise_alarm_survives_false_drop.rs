//! [YÜKSELİŞ ALARMI KORUNUR] Yükseliş alarmını tetikleyen çift değerleri, aynı ACK aralığında
//! gelen daralma algısıyla silinmez; alarmın ACK'i onları göndericiye taşır.
//!
//! Gerçek hat kaydı (iki süreç, cc-port, bitmap ACK, ileri 12 500 → 6 250 → 12 500 B/s, dönüş
//! 62 B/s; `yerel/ham-izler-netem-v1` dalı, `netem-v1/kancali-1/1/`, tohum 1): hat 60. saniyede
//! 12 500 B/s'ye döndü, gönderici 6 223 B/s'de gönderiyordu.
//! * +527 ve +613 ms'de iki datagram 86 ms arayla vardı (darboğaz meşgul, aralık tam serileşme):
//!   çift değerleri 12 176 ve 12 168 B/s. Alıcı +613 ms'de yükseliş alarmı verdi.
//! * Aynı patlama daralma dedektörünün referansını ≈ 12 100 B/s'ye çıkardı. +886 ms'de varışlar
//!   göndericinin kendi aralığına döndü; pencere hızı referansın %70'inin altında kaldı, dedektör
//!   tetiklendi. Çift değerleri saklanıp sıfırlandı, +907 ms'de kanıt yeterli sayıldı ve saklanan
//!   değerler atıldı.
//! * Dönüş hattı doluydu; alarm ACK'i +1 699 ms'de çıktı ve yalnız +886 ms'den sonraki çiftleri
//!   (3 901 ve 6 045 B/s) taşıdı. Gönderici tavanı açmadı; tavan 40 s sonra hâlâ 6 608 B/s'ydi.
//!
//! Daralma kanıtının türetmesi (`arrival_rate::AlarmEvidence`) göndericinin aralık boyunca en az
//! referans hızıyla gönderdiğini varsayar. Referans, hattın açılmasıyla boşalan bir kuyruğun
//! varışlarından kurulduğunda bu ön koşul tutmaz: varış hızı gönderim hızını geçici olarak aşar.
//! Yükseliş alarmı bekliyorken gelen daralma algısı bu durumun işaretidir.
//!
//! Bu test kayıttaki varış anlarını ve boylarını (+/− 1,8 s) alıcıya verir; öncesi düzenli
//! varışlarla kurulur (12 500 B/s, sonra 6 250 B/s: gerçek bir daralma, yükseliş alarmını kuran).

use adaptive_transport::ack::AckFrame;
use adaptive_transport::receiver::{Receiver, RxDatagram, RxKind};

const US: u64 = 1_000;
const MS: u64 = 1_000_000;
const SEC: u64 = 1_000_000_000;
/// Datagram zarfı (B; IPv4 + UDP): kayıt IP boyunu verir, alıcı UDP gövdesini görür.
const OVH: u64 = 28;
/// Tam boy datagram (B, IP).
const FULL: u64 = 1_074;
/// Beyan edilen ileri hat (B/s, tel).
const DECLARED: u64 = 12_500;
/// Hattın 12 500 B/s'ye döndüğü an.
const RECOVERY: u64 = 60 * SEC;
/// Gerçek daralmanın anı (yükseliş alarmını kurar).
const DROP: u64 = 20 * SEC;

/// Kayıt: toparlanma anına göre varış anı (µs) ve IP boyu (B). Kaynak `cap/B_vb0.jsonl`, giriş yönü.
const RECORDED: &[(i64, u64)] = &[
    (-1_086_900, 1_074),
    (-1_065_200, 136),
    (-1_043_300, 136),
    (-999_800, 136),
    (-827_900, 1_074),
    (-807_400, 129),
    (-635_600, 1_074),
    (-461_900, 1_086),
    (-289_900, 1_074),
    (-118_100, 1_074),
    (53_900, 1_074),
    (344_300, 1_074),
    (420_100, 1_074),
    (441_400, 136),
    (527_300, 1_074),
    (613_200, 1_074),
    (700_100, 1_086),
    (885_800, 136),
    (907_400, 136),
    (928_500, 136),
    (972_900, 1_086),
    (1_147_100, 1_086),
    (1_225_000, 1_086),
    (1_235_200, 129),
    (1_503_300, 1_074),
    (1_676_400, 1_074),
    (1_752_200, 1_074),
];

/// Kayıtta alarm ACK'i dolu dönüş hattında bir kez yol verdi: +614 ms'de hazır, +1 699 ms'de çıktı.
const RETURN_BUSY_FROM: u64 = RECOVERY + 500 * MS;
const RETURN_BUSY_UNTIL: u64 = RECOVERY + 1_800 * MS;
const RETURN_AHEAD: u64 = 1_085 * MS;

/// Varış çizelgesi `(an ns, IP boyu)`: 12 500 B/s'de 86 ms arayla, [`DROP`]'tan sonra 6 250 B/s'de
/// 172 ms arayla, toparlanma çevresinde kayıt, sonra yine 172 ms arayla (gönderici henüz
/// hızlanmadı).
fn arrivals() -> Vec<(u64, u64)> {
    let first_recorded = RECOVERY - 1_086_900 * US;
    let mut out = Vec::new();
    let mut t = SEC;
    while t < DROP {
        out.push((t, FULL));
        t += 86 * MS;
    }
    while t + 172 * MS <= first_recorded {
        out.push((t, FULL));
        t += 172 * MS;
    }
    for &(at_us, ip) in RECORDED {
        let at = if at_us < 0 {
            RECOVERY - at_us.unsigned_abs() * US
        } else {
            RECOVERY + at_us.unsigned_abs() * US
        };
        out.push((at, ip));
    }
    let mut t = out.last().map_or(RECOVERY, |a| a.0) + 172 * MS;
    while t < RECOVERY + 4 * SEC {
        out.push((t, FULL));
        t += 172 * MS;
    }
    out
}

struct Emitted {
    at: u64,
    frame: AckFrame,
    rise_alarms: u64,
}

/// Alıcıyı çizelgeyle besler; arada zamanlayıcıyı 1 ms adımla çağırır. `busy` = dönüş hattının
/// dolu penceresi (yol verme işareti ve önündeki serileşme).
fn drive(arrivals: &[(u64, u64)], busy: (u64, u64)) -> Vec<Emitted> {
    let mut r = Receiver::with_cc_port();
    r.set_return_bandwidth(Some(62));
    r.set_ack_bitmap(true);
    r.set_pair_squeeze_line(Some(DECLARED));
    let mut acks = Vec::new();
    let set_busy = |r: &mut Receiver, t: u64| {
        let b = t >= busy.0 && t < busy.1;
        r.set_return_line_ahead_ns(if b { RETURN_AHEAD } else { 0 });
        r.set_uplink_yield(b);
    };
    let push = |acks: &mut Vec<Emitted>, r: &Receiver, at: u64, frame: AckFrame| {
        acks.push(Emitted {
            at,
            frame,
            rise_alarms: r.diag_rise_alarms(),
        });
    };
    let mut next_poll = arrivals.first().map_or(0, |a| a.0);
    for (seq, &(at, ip)) in arrivals.iter().enumerate() {
        while next_poll < at {
            set_busy(&mut r, next_poll);
            if let Some(f) = r.on_ack_timer(next_poll) {
                push(&mut acks, &r, next_poll, f);
            }
            next_poll += MS;
        }
        set_busy(&mut r, at);
        r.set_next_datagram_bytes(ip - OVH);
        let seq = seq as u64;
        let out = r.on_datagram(
            RxDatagram {
                seq,
                unit_id: seq,
                kind: RxKind::Data,
            },
            at,
        );
        if let Some(f) = out.ack {
            push(&mut acks, &r, at, f);
        }
        if let Some(f) = r.on_ack_timer(at) {
            push(&mut acks, &r, at, f);
        }
        next_poll = at + MS;
    }
    acks
}

fn pair_rate(f: &AckFrame) -> u16 {
    f.bitmap.as_ref().expect("bitmap ACK").pair_rate
}

/// Kayıttaki dizi: yükseliş alarmının ACK'i, alarmı tetikleyen çift değerini taşır.
#[test]
fn rise_alarm_ack_carries_the_pairs_that_raised_it() {
    let acks = drive(&arrivals(), (RETURN_BUSY_FROM, RETURN_BUSY_UNTIL));
    let before = acks
        .iter()
        .rev()
        .find(|a| a.at < RECOVERY)
        .expect("toparlanmadan önce ACK");
    // Ön koşul: gerçek daralmadan sonra bildirilen değer 6 250 B/s hattın çift değeri.
    let reported = pair_rate(&before.frame);
    assert!(
        (5_600..=6_300).contains(&reported),
        "toparlanmadan önce bildirilen çift değeri: {reported}"
    );
    let ack = acks
        .iter()
        .find(|a| a.at >= RECOVERY + 600 * MS)
        .expect("toparlanmadan sonra ACK");
    assert_eq!(
        ack.rise_alarms,
        before.rise_alarms + 1,
        "yükseliş alarmı bir kez"
    );
    // Kayıttaki gibi ACK dolu dönüş hattında bekledi (sahte daralma algısı bu sürede geldi).
    assert!(
        ack.at >= RECOVERY + 886 * MS,
        "ACK algıdan önce çıktı: +{} ms",
        (ack.at - RECOVERY) / MS
    );
    let got = pair_rate(&ack.frame);
    // 12 168 B/s (UDP gövdesi): göndericide tel baytına çevrilince profil hattının %95'ini geçer,
    // tavan profil hattına açılır (`LineGuard::on_pair_rate`).
    assert!(
        got >= 11_900,
        "yükseliş alarmı ACK'inin çift değeri {got}: alarmı tetikleyen ölçü (≈ 12 168) taşınmadı"
    );
}

/// Karşı örnek: yükseliş alarmı yokken gerçek daralmanın ACK'i yalnız daralmadan sonraki çiftleri
/// taşır (bugünkü kural değişmedi).
#[test]
fn real_drop_without_a_pending_rise_still_reports_only_post_drop_pairs() {
    let all = arrivals();
    let until = RECOVERY - 2 * SEC;
    let acks = drive(
        &all.iter()
            .copied()
            .filter(|a| a.0 < until)
            .collect::<Vec<_>>(),
        (0, 0),
    );
    let ack = acks
        .iter()
        .find(|a| a.at > DROP)
        .expect("daralmadan sonra ACK");
    // Alarm ACK'i daralmadan hemen sonra çıkar; o ana dek iki çift yoksa alan boştur.
    let got = pair_rate(&ack.frame);
    assert!(
        got == adaptive_transport::ack::PAIR_RATE_NONE || got <= 6_875,
        "daralma ACK'i düşüş öncesi çift değerini taşıdı: {got}"
    );
}

/// Karşı örnek: hat 6 250 B/s'de kalırsa yükseliş alarmı yok, bildirilen değer hattı aşmaz.
#[test]
fn a_line_that_stays_narrow_raises_no_rise_alarm() {
    let first_recorded = RECOVERY - 1_086_900 * US;
    let mut all: Vec<(u64, u64)> = arrivals()
        .into_iter()
        .filter(|a| a.0 < first_recorded)
        .collect();
    let mut t = all.last().map_or(first_recorded, |a| a.0) + 172 * MS;
    while t < RECOVERY + 4 * SEC {
        all.push((t, FULL));
        t += 172 * MS;
    }
    let acks = drive(&all, (RETURN_BUSY_FROM, RETURN_BUSY_UNTIL));
    let last = acks.last().expect("ACK");
    assert_eq!(last.rise_alarms, 0, "dar kalan hatta yükseliş alarmı");
    for a in acks.iter().filter(|a| a.at > DROP + 5 * SEC) {
        let p = pair_rate(&a.frame);
        assert!(
            p == adaptive_transport::ack::PAIR_RATE_NONE || p <= 6_300,
            "dar hatta çift değeri {p}"
        );
    }
}
